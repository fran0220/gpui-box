//! Native controls whose data and actions remain owned by the worker.
//! Slot factories are invoked only after releasing retained-state borrows.
use super::*;
use gpui::{Hsla, ParentElement};
use gpui_kit::controls::auth::{OneTimeCodeInput, PasswordInput};
use gpui_kit::controls::button::{ButtonJoin, ButtonStyle, IconPosition};
use gpui_kit::controls::copy_button::{CopyButton, CopyEvent, CopyState};
use gpui_kit::controls::keybinding_recorder::KeybindingRecorder;
use gpui_kit::controls::keymap_editor::{
    KeymapBinding, KeymapCommand, KeymapEditor, KeymapEditorEvent,
};
use gpui_kit::controls::number_input::{NumberInput, NumberInputEvent};
use gpui_kit::controls::split_button::SplitButton;
use gpui_kit::overlay::MenuEvent;
use gpui_kit::state::ValidationState;
use gpui_kit::strings::{ActiveStrings, StringKey};
use gpui_kit_theme::{ActiveTheme, ColorChoice, SemanticColor, Surface, Variant};

mod auth;
mod recorder;

#[cfg(all(test, feature = "capture"))]
mod tests;

pub(super) const COMPONENTS: &[&str] = &[
    "ColorPicker",
    "ColorSwatch",
    "FormField",
    "FilterBar",
    "Button",
    "IconButton",
    "Toggle",
    "ToggleGroup",
    "SearchInput",
    "SettingsRow",
    "TransferList",
    "NumberInput",
    "KeymapEditor",
    "ButtonGroup",
    "CopyButton",
    "SettingsSection",
    "SettingsList",
    "SplitButton",
    "InlineEdit",
    "KeybindingRecorder",
    "PasswordInput",
    "OneTimeCodeInput",
];

pub(super) fn settings_section(
    node: &Node,
    context: crate::construction::NativeBuildContext,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<SettingsSection> {
    let mut section = SettingsSection::new(node.id.clone(), text(node, "title"));
    if node.props.contains_key("description") {
        section = section.description(text(node, "description"));
    }
    if node.props.contains_key("labelWidth") {
        section = section.label_width(gpui::px(number(node, "labelWidth", 0.)));
    }
    if node.props.contains_key("dimmedBy") {
        // Do not construct controls or subscribe to events that cannot apply.
        return Ok(section.dimmed_by(text(node, "dimmedBy")));
    }
    section = section.rows(context.typed.build(
        "rows",
        "SettingsRow",
        window,
        cx,
        |child, context, _, _, window, cx| Ok(settings_row(child, context.slots, window, cx)),
    )?);
    // `rows` is the homogeneous convenience slot; `content` preserves arbitrary
    // native row/block interleaving, including every child's effect boundary.
    for content in context.typed.build_mixed(
        "content",
        "SettingsRow",
        window,
        cx,
        |child, context, _, _, window, cx| Ok(settings_row(child, context.slots, window, cx)),
    )? {
        section = match content {
            crate::construction::TypedSlotContent::Typed(row) => section.row(row),
            crate::construction::TypedSlotContent::Element(element) => section.child(element),
        };
    }
    if let Some(action) = context.slots.get("action").cloned() {
        section = section.action(move |window, cx| action(window, cx));
    }
    Ok(section)
}

pub(super) fn settings_list(
    node: &Node,
    context: crate::construction::NativeBuildContext,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<SettingsList> {
    let sections = context.typed.build(
        "sections",
        "SettingsSection",
        window,
        cx,
        |child, context, _, _, window, cx| settings_section(child, context, window, cx),
    )?;
    let mut list = SettingsList::new(node.id.clone())
        .query(text(node, "query"))
        .sections(sections);
    for name in ["empty", "header", "sidebar", "footer"] {
        if let Some(slot) = context.slots.get(name).cloned() {
            list = list.slot(name, move |window, cx| slot(window, cx));
        }
    }
    Ok(list)
}

/// The host supplies the mounted, revision-checked typed construction context.
/// Child handlers and scopes survive the native group's join/size transforms.
pub(super) fn button_group(
    node: &Node,
    context: crate::construction::NativeBuildContext,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<gpui_kit::controls::button::ButtonGroup> {
    let buttons = context.typed.build(
        "buttons",
        "Button",
        window,
        cx,
        |child, _, _, emit, _, _| {
            if matches!(child.kind, crate::Kind::Button) {
                let mut button = Button::new(child.id.clone())
                    .label(child.text.clone())
                    .disabled(child.disabled);
                if !child.disabled
                    && let Some(action) = child.action.clone()
                {
                    button = button.on_click(move |_, _| emit(&action, Value::Null));
                }
                Ok(button)
            } else {
                Ok(button(child, emit))
            }
        },
    )?;
    Ok(
        gpui_kit::controls::button::ButtonGroup::new(node.id.clone())
            .control_size(size(node))
            .disabled(flag(node, "disabled"))
            .children(buttons),
    )
}

struct Entry<T: 'static> {
    entity: Entity<T>,
    route: Rc<RefCell<Route>>,
    props: RefCell<serde_json::Map<String, Value>>,
    _subscription: Subscription,
}

/// The shared host registry, not the adapter, owns reference authority/lifetime.
fn focus_reference<T: gpui::Focusable + 'static>(
    entries: &RefCell<HashMap<Key, Rc<Entry<T>>>>,
    key: &Key,
    cx: &App,
    refs: &crate::references::Registration<'_>,
    allowed: fn(&T, &App) -> bool,
) -> anyhow::Result<Value> {
    let entity = entries
        .borrow()
        .get(key)
        .map(|entry| entry.entity.clone())
        .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
    let focus = entity.read(cx).focus_handle(cx);
    refs.focus(
        &entity,
        &focus,
        |control, focus, cx| control.focus_handle(cx) == *focus,
        allowed,
    )
}

#[derive(Default)]
pub(super) struct State {
    passwords: RefCell<HashMap<Key, Rc<Entry<PasswordInput>>>>,
    codes: RefCell<HashMap<Key, Rc<Entry<OneTimeCodeInput>>>>,
    recorders: RefCell<HashMap<Key, Rc<Entry<KeybindingRecorder>>>>,
    searches: RefCell<HashMap<Key, Rc<Entry<SearchInput>>>>,
    transfers: RefCell<HashMap<Key, Rc<Entry<TransferList>>>>,
    numbers: RefCell<HashMap<Key, Rc<Entry<NumberInput>>>>,
    keymaps: RefCell<HashMap<Key, Rc<Entry<KeymapEditor>>>>,
    copies: RefCell<HashMap<Key, Rc<Entry<CopyButton>>>>,
    splits: RefCell<HashMap<Key, Rc<Entry<SplitButton>>>>,
}

impl State {
    pub(super) fn native_entity_id(&self, node: &Node) -> Option<gpui::EntityId> {
        let key = (node.instance, node.id.clone());
        match node.component.as_deref()? {
            "PasswordInput" => self
                .passwords
                .borrow()
                .get(&key)
                .map(|entry| entry.entity.entity_id()),
            "OneTimeCodeInput" => self
                .codes
                .borrow()
                .get(&key)
                .map(|entry| entry.entity.entity_id()),
            "KeybindingRecorder" => self
                .recorders
                .borrow()
                .get(&key)
                .map(|entry| entry.entity.entity_id()),
            "SearchInput" => self
                .searches
                .borrow()
                .get(&key)
                .map(|entry| entry.entity.entity_id()),
            "TransferList" => self
                .transfers
                .borrow()
                .get(&key)
                .map(|entry| entry.entity.entity_id()),
            "NumberInput" => self
                .numbers
                .borrow()
                .get(&key)
                .map(|entry| entry.entity.entity_id()),
            "KeymapEditor" => self
                .keymaps
                .borrow()
                .get(&key)
                .map(|entry| entry.entity.entity_id()),
            "CopyButton" => self
                .copies
                .borrow()
                .get(&key)
                .map(|entry| entry.entity.entity_id()),
            "SplitButton" => self
                .splits
                .borrow()
                .get(&key)
                .map(|entry| entry.entity.entity_id()),
            _ => None,
        }
    }

    pub(super) fn reference_query(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        cx: &App,
        refs: &crate::references::Registration<'_>,
    ) -> Option<anyhow::Result<Value>> {
        let component = node.component.as_deref()?;
        if component == "SplitButton" && method == "menu" {
            return Some((|| {
                let schema = super::validation::invocation(component, method, args, true)?;
                let entity = self
                    .splits
                    .borrow()
                    .get(&(node.instance, node.id.clone()))
                    .map(|entry| entry.entity.clone())
                    .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
                let menu = entity.read(cx).menu().clone();
                let result = refs.entity(
                    "Menu",
                    &entity,
                    &menu,
                    |parent, _| Some(parent.menu().clone()),
                    |parent, _| !parent.is_disabled(),
                    super::reference_dispatch::menu,
                )?;
                super::validation::validate(&result, schema)?;
                Ok(result)
            })());
        }
        if method != "focus_handle"
            || !matches!(
                component,
                "SearchInput"
                    | "NumberInput"
                    | "CopyButton"
                    | "SplitButton"
                    | "KeybindingRecorder"
                    | "PasswordInput"
                    | "OneTimeCodeInput"
            )
        {
            return None;
        }
        Some((|| {
            let schema = super::validation::invocation(component, method, args, true)?;
            let key = (node.instance, node.id.clone());
            let result = match component {
                "PasswordInput" => {
                    focus_reference(&self.passwords, &key, cx, refs, |control, _| {
                        !control.is_disabled()
                    })
                }
                "OneTimeCodeInput" => focus_reference(&self.codes, &key, cx, refs, |control, _| {
                    !control.is_disabled()
                }),
                "KeybindingRecorder" => {
                    focus_reference(&self.recorders, &key, cx, refs, |control, _| {
                        !control.is_disabled()
                    })
                }
                "SearchInput" => focus_reference(&self.searches, &key, cx, refs, |control, _| {
                    !control.is_disabled()
                }),
                "NumberInput" => focus_reference(&self.numbers, &key, cx, refs, |control, _| {
                    !control.is_disabled()
                }),
                "CopyButton" => focus_reference(&self.copies, &key, cx, refs, |control, _| {
                    !control.is_disabled()
                }),
                "SplitButton" => focus_reference(&self.splits, &key, cx, refs, |control, _| {
                    !control.is_disabled()
                }),
                _ => unreachable!("matched retained focus component"),
            }?;
            super::validation::validate(&result, schema)?;
            Ok(result)
        })())
    }

    pub(super) fn reconcile(&self, root: &Node, _cx: &mut App) {
        fn visit(node: &Node, live: &mut HashMap<Key, String>) {
            if let Some(component) = &node.component {
                live.insert((node.instance, node.id.clone()), component.clone());
            }
            for child in node.children.iter().chain(node.slots.values().flatten()) {
                visit(child, live);
            }
        }
        let mut live = HashMap::new();
        visit(root, &mut live);
        self.passwords
            .borrow_mut()
            .retain(|key, _| live.get(key).is_some_and(|kind| kind == "PasswordInput"));
        self.codes
            .borrow_mut()
            .retain(|key, _| live.get(key).is_some_and(|kind| kind == "OneTimeCodeInput"));
        self.recorders.borrow_mut().retain(|key, _| {
            live.get(key)
                .is_some_and(|kind| kind == "KeybindingRecorder")
        });
        self.searches
            .borrow_mut()
            .retain(|key, _| live.get(key).is_some_and(|kind| kind == "SearchInput"));
        self.transfers
            .borrow_mut()
            .retain(|key, _| live.get(key).is_some_and(|kind| kind == "TransferList"));
        self.numbers
            .borrow_mut()
            .retain(|key, _| live.get(key).is_some_and(|kind| kind == "NumberInput"));
        self.keymaps
            .borrow_mut()
            .retain(|key, _| live.get(key).is_some_and(|kind| kind == "KeymapEditor"));
        self.copies
            .borrow_mut()
            .retain(|key, _| live.get(key).is_some_and(|kind| kind == "CopyButton"));
        self.splits
            .borrow_mut()
            .retain(|key, _| live.get(key).is_some_and(|kind| kind == "SplitButton"));
    }

    pub(super) fn render(
        &self,
        node: &Node,
        slots: KitSlots,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        if node.component.as_deref() == Some("PasswordInput") {
            return self.render_password(node, window, cx, emit);
        }
        if node.component.as_deref() == Some("OneTimeCodeInput") {
            return self.render_code(node, window, cx, emit);
        }
        if node.component.as_deref() == Some("KeybindingRecorder") {
            return self.render_recorder(node, window, cx, emit);
        }
        if node.component.as_deref() == Some("SplitButton") {
            return self.render_split(node, window, cx, emit);
        }
        if node.component.as_deref() == Some("CopyButton") {
            return self.render_copy(node, window, cx, emit);
        }
        if node.component.as_deref() == Some("KeymapEditor") {
            return self.render_keymap(node, window, cx, emit);
        }
        if node.component.as_deref() == Some("NumberInput") {
            return self.render_number(node, window, cx, emit);
        }
        if node.component.as_deref() == Some("TransferList") {
            return self.render_transfer(node, window, cx, emit);
        }
        if node.component.as_deref() != Some("SearchInput") {
            return render(node, slots, window, cx, emit);
        }
        let key = (node.instance, node.id.clone());
        let existing = self.searches.borrow().get(&key).cloned();
        let entry = existing.unwrap_or_else(|| {
            let entity = cx.new(|cx| SearchInput::new(node.id.clone(), window, cx));
            let route = Rc::new(RefCell::new(Route {
                events: node.events.clone(),
                emit: emit.clone(),
                disabled: flag(node, "disabled"),
            }));
            let callback = Rc::downgrade(&route);
            let subscription = cx.subscribe(&entity, move |_, event: &SearchInputEvent, _| {
                let (name, payload) = match event {
                    SearchInputEvent::Change(value) => ("change", json!(value.as_ref())),
                    SearchInputEvent::Submit => ("submit", Value::Null),
                    SearchInputEvent::Cancel => ("cancel", Value::Null),
                    SearchInputEvent::BackspaceAtStart => ("backspaceAtStart", Value::Null),
                    SearchInputEvent::Focus => ("focus", Value::Null),
                    SearchInputEvent::Blur => ("blur", Value::Null),
                };
                let target = callback.upgrade().and_then(|route| {
                    let route = route.borrow();
                    (!route.disabled)
                        .then(|| {
                            route
                                .events
                                .get(name)
                                .map(|action| (action.clone(), route.emit.clone()))
                        })
                        .flatten()
                });
                if let Some((action, emit)) = target {
                    emit(&action, payload);
                }
            });
            let entry = Rc::new(Entry {
                entity,
                route,
                props: Default::default(),
                _subscription: subscription,
            });
            self.searches.borrow_mut().insert(key, entry.clone());
            entry
        });
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if *entry.props.borrow() != node.props {
            entry.entity.update(cx, |search, cx| {
                search.set_presentation(
                    node.props
                        .get("name")
                        .and_then(Value::as_str)
                        .map(|value| value.to_owned().into()),
                    node.props
                        .get("placeholder")
                        .and_then(Value::as_str)
                        .map(|value| value.to_owned().into()),
                    size(node),
                    cx,
                );
                search.set_disabled(flag(node, "disabled"), cx);
                if node.props.contains_key("value") {
                    search.set_value(text(node, "value"), cx);
                }
            });
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }

    pub(super) fn invoke(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        _window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        if node.component.as_deref() == Some("PasswordInput") {
            return self.invoke_password(node, method, args, query, cx);
        }
        if node.component.as_deref() == Some("OneTimeCodeInput") {
            return self.invoke_code(node, method, args, query, cx);
        }
        if node.component.as_deref() == Some("KeybindingRecorder") {
            return self.invoke_recorder(node, method, args, query, _window, cx);
        }
        if node.component.as_deref() == Some("SplitButton") {
            return self.invoke_split(node, method, args, query, _window, cx);
        }
        if node.component.as_deref() == Some("CopyButton") {
            return self.invoke_copy(node, method, args, query, cx);
        }
        if node.component.as_deref() == Some("KeymapEditor") {
            return self.invoke_keymap(node, method, args, query, cx);
        }
        if node.component.as_deref() == Some("NumberInput") {
            return self.invoke_number(node, method, args, query, cx);
        }
        if node.component.as_deref() == Some("TransferList") {
            return self.invoke_transfer(node, method, args, query, cx);
        }
        if node.component.as_deref() != Some("SearchInput") {
            return invoke(node, method, args, query);
        }
        let entity = self
            .searches
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .map(|entry| entry.entity.clone())
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        anyhow::ensure!(
            query || (!flag(node, "disabled") && !entity.read(cx).is_disabled()),
            "disabled target refuses invocation"
        );
        if query {
            return match method {
                "value" => Ok(json!(entity.read(cx).value(cx).as_ref())),
                "is_disabled" => Ok(json!(entity.read(cx).is_disabled())),
                _ => anyhow::bail!("unsupported SearchInput query"),
            };
        }
        entity.update(cx, |search, cx| {
            match method {
                "set_value" => {
                    search.set_value(args["value"].as_str().unwrap_or_default().to_owned(), cx)
                }
                "set_name" => {
                    search.set_name(args["name"].as_str().unwrap_or_default().to_owned(), cx)
                }
                "set_placeholder" => search.set_placeholder(
                    args["placeholder"].as_str().unwrap_or_default().to_owned(),
                    cx,
                ),
                "set_disabled" => {
                    search.set_disabled(args["disabled"].as_bool().unwrap_or(false), cx)
                }
                "set_presentation" => {
                    let control_size = match args["size"].as_str() {
                        Some("xs") => ControlSize::Xs,
                        Some("sm") => ControlSize::Sm,
                        Some("lg") => ControlSize::Lg,
                        _ => ControlSize::Md,
                    };
                    search.set_presentation(
                        args["name"].as_str().map(|v| v.to_owned().into()),
                        args["placeholder"].as_str().map(|v| v.to_owned().into()),
                        control_size,
                        cx,
                    );
                }
                _ => anyhow::bail!("unsupported SearchInput command"),
            }
            Ok(Value::Null)
        })
    }
}

impl State {
    fn render_number(
        &self,
        node: &Node,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.numbers.borrow().get(&key).cloned();
        let entry = existing.unwrap_or_else(|| {
            let entity = cx.new(|cx| NumberInput::new(node.id.clone(), window, cx));
            let route = Rc::new(RefCell::new(Route {
                events: node.events.clone(),
                emit: emit.clone(),
                disabled: flag(node, "disabled"),
            }));
            let callback = Rc::downgrade(&route);
            let subscription =
                cx.subscribe(&entity, move |entity, event: &NumberInputEvent, cx| {
                    if entity.read(cx).is_disabled() {
                        return;
                    }
                    let (name, payload) = match event {
                        NumberInputEvent::Changed(value) => ("change", json!(value)),
                        NumberInputEvent::Unparsable(value) => {
                            ("unparsable", json!(value.as_ref()))
                        }
                        NumberInputEvent::Submit => ("submit", Value::Null),
                    };
                    let target = callback.upgrade().and_then(|route| {
                        let route = route.borrow();
                        (!route.disabled)
                            .then(|| {
                                route
                                    .events
                                    .get(name)
                                    .map(|action| (action.clone(), route.emit.clone()))
                            })
                            .flatten()
                    });
                    if let Some((action, emit)) = target {
                        emit(&action, payload);
                    }
                });
            let entry = Rc::new(Entry {
                entity,
                route,
                props: Default::default(),
                _subscription: subscription,
            });
            self.numbers.borrow_mut().insert(key, entry.clone());
            entry
        });
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if *entry.props.borrow() != node.props {
            entry.entity.update(cx, |number, cx| {
                number.set_range(
                    node.props.get("min").and_then(Value::as_f64),
                    node.props.get("max").and_then(Value::as_f64),
                    cx,
                );
                number.set_steps(
                    node.props.get("step").and_then(Value::as_f64).unwrap_or(1.),
                    node.props.get("pageStep").and_then(Value::as_f64),
                    cx,
                );
                number.set_precision(
                    node.props
                        .get("precision")
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as usize,
                    cx,
                );
                let optional = |key| {
                    node.props
                        .get(key)
                        .and_then(Value::as_str)
                        .map(|s| SharedString::from(s.to_owned()))
                };
                number.set_presentation(
                    optional("name"),
                    optional("unit"),
                    optional("prefix"),
                    size(node),
                    cx,
                );
                number.set_required(flag(node, "required"), cx);
                number.set_invalid(flag(node, "invalid"), cx);
                number.set_disabled(flag(node, "disabled"), cx);
                if let Some(value) = node.props.get("value").and_then(Value::as_f64)
                    && entry.props.borrow().get("value") != node.props.get("value")
                {
                    number.set_value(value, cx);
                }
            });
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }

    fn invoke_number(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        let entity = self
            .numbers
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .map(|entry| entry.entity.clone())
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        anyhow::ensure!(
            query || (!flag(node, "disabled") && !entity.read(cx).is_disabled()),
            "disabled target refuses invocation"
        );
        if query {
            let number = entity.read(cx);
            return match method {
                "current" => Ok(json!(number.current())),
                "shown" => Ok(json!(number.shown(cx))),
                "is_disabled" => Ok(json!(number.is_disabled())),
                "is_invalid" => Ok(json!(number.is_invalid(cx))),
                "invalid_reason" => Ok(json!(number.invalid_reason(cx).as_deref())),
                "can_step" => Ok(json!(
                    number.can_step(args["delta"].as_f64().unwrap_or_default(), cx)
                )),
                _ => anyhow::bail!("unsupported NumberInput query"),
            };
        }
        entity.update(cx, |number, cx| {
            match method {
                "set_value" => number.set_value(args["value"].as_f64().unwrap_or_default(), cx),
                "set_invalid" => number.set_invalid(args["invalid"].as_bool().unwrap_or(false), cx),
                "set_disabled" => {
                    number.set_disabled(args["disabled"].as_bool().unwrap_or(false), cx)
                }
                "set_required" => {
                    number.set_required(args["required"].as_bool().unwrap_or(false), cx)
                }
                "set_range" => number.set_range(args["min"].as_f64(), args["max"].as_f64(), cx),
                "set_steps" => number.set_steps(
                    args["step"].as_f64().unwrap_or(1.),
                    args["page_step"].as_f64(),
                    cx,
                ),
                "set_precision" => {
                    number.set_precision(args["precision"].as_u64().unwrap_or(0) as usize, cx)
                }
                "set_presentation" => {
                    let optional =
                        |key: &str| args[key].as_str().map(|s| SharedString::from(s.to_owned()));
                    let size = match args["size"].as_str() {
                        Some("xs") => ControlSize::Xs,
                        Some("sm") => ControlSize::Sm,
                        Some("lg") => ControlSize::Lg,
                        _ => ControlSize::Md,
                    };
                    number.set_presentation(
                        optional("name"),
                        optional("unit"),
                        optional("prefix"),
                        size,
                        cx,
                    );
                }
                _ => anyhow::bail!("unsupported NumberInput command"),
            }
            Ok(Value::Null)
        })
    }
}

fn keymap_commands(value: Option<&Value>) -> Vec<KeymapCommand> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|value| {
            let string = |key: &str| value[key].as_str().unwrap_or_default().to_owned();
            let bindings = value["bindings"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|binding| {
                    let mut item = KeymapBinding::new(
                        binding["id"].as_str().unwrap_or_default().to_owned(),
                        binding["keystroke"].as_str().unwrap_or_default().to_owned(),
                    );
                    if let Some(value) = binding["conflict"].as_str() {
                        item = item.conflict(value.to_owned());
                    }
                    if let Some(value) = binding["provenance"].as_str() {
                        item = item.provenance(value.to_owned());
                    }
                    item
                });
            let mut command = KeymapCommand::new(string("id"), string("label"))
                .defaults(strings(value.get("defaults")))
                .bindings(bindings)
                .searchable(string("searchText"), strings(value.get("keywords")));
            if let Some(value) = value["context"].as_str() {
                command = command.context(value.to_owned());
            }
            if let Some(value) = value["refusal"].as_str() {
                command = command.refused(value.to_owned());
            }
            command
        })
        .collect()
}

fn keymap_snapshot(commands: &[KeymapCommand]) -> Value {
    json!(commands.iter().map(|command| json!({
        "id":command.id().as_ref(), "label":command.label_text().as_ref(),
        "context":command.context_label().map(|s| s.as_ref()),
        "defaults":command.default_bindings().iter().map(|s| s.as_ref()).collect::<Vec<_>>(),
        "bindings":command.effective_bindings().iter().map(|binding| json!({
            "id":binding.id().as_ref(),"keystroke":binding.keystroke().as_ref(),
            "conflict":binding.conflict_reason().map(|s| s.as_ref()),
            "provenance":binding.provenance_label().map(|s| s.as_ref()),
        })).collect::<Vec<_>>(),
        "searchText":command.search_text().as_ref(),
        "keywords":command.keywords().iter().map(|s| s.as_ref()).collect::<Vec<_>>(),
        "refusal":command.refusal_reason().map(|s| s.as_ref()),
    })).collect::<Vec<_>>())
}

impl State {
    fn render_keymap(
        &self,
        node: &Node,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.keymaps.borrow().get(&key).cloned();
        let entry = existing.unwrap_or_else(|| {
            let entity = cx.new(|cx| KeymapEditor::new(node.id.clone(), window, cx));
            let route = Rc::new(RefCell::new(Route { events: node.events.clone(), emit: emit.clone(), disabled: flag(node, "disabled") }));
            let callback = Rc::downgrade(&route);
            let subscription = cx.subscribe(&entity, move |entity, event: &KeymapEditorEvent, cx| {
                if entity.read(cx).is_disabled() { return; }
                let (name, payload) = match event {
                    KeymapEditorEvent::AddCaptured {command_id,keystroke} => ("addCaptured", json!({"command_id":command_id.as_ref(),"keystroke":keystroke.as_ref()})),
                    KeymapEditorEvent::Remove {command_id,binding_id} => ("remove", json!({"command_id":command_id.as_ref(),"binding_id":binding_id.as_ref()})),
                    KeymapEditorEvent::Reset {command_id} => ("reset", json!({"command_id":command_id.as_ref()})),
                    KeymapEditorEvent::RecordingCancelled {command_id} => ("recordingCancelled", json!({"command_id":command_id.as_ref()})),
                };
                let target = callback.upgrade().and_then(|route| {
                    let route = route.borrow();
                    (!route.disabled).then(|| route.events.get(name).map(|action| (action.clone(),route.emit.clone()))).flatten()
                });
                if let Some((action,emit)) = target { emit(&action,payload); }
            });
            let entry = Rc::new(Entry {entity,route,props:Default::default(),_subscription:subscription});
            self.keymaps.borrow_mut().insert(key,entry.clone());
            entry
        });
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if *entry.props.borrow() != node.props {
            entry.entity.update(cx, |editor, cx| {
                editor.set_commands(keymap_commands(node.props.get("commands")), cx);
                editor.set_query(text(node, "query"), cx);
                editor.set_disabled(flag(node, "disabled"), cx);
            });
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }

    fn invoke_keymap(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        let entity = self
            .keymaps
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .map(|entry| entry.entity.clone())
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        anyhow::ensure!(
            query || (!flag(node, "disabled") && !entity.read(cx).is_disabled()),
            "disabled target refuses invocation"
        );
        if query {
            let editor = entity.read(cx);
            return match method {
                "current_commands" => Ok(keymap_snapshot(editor.current_commands())),
                "active_command" => Ok(json!(editor.active_command().map(|s| s.as_ref()))),
                "is_disabled" => Ok(json!(editor.is_disabled())),
                _ => anyhow::bail!("unsupported KeymapEditor query"),
            };
        }
        entity.update(cx, |editor, cx| {
            match method {
                "set_commands" => editor.set_commands(keymap_commands(args.get("commands")), cx),
                "set_query" => {
                    editor.set_query(args["query"].as_str().unwrap_or_default().to_owned(), cx)
                }
                "set_disabled" => {
                    editor.set_disabled(args["disabled"].as_bool().unwrap_or(false), cx)
                }
                _ => anyhow::bail!("unsupported KeymapEditor command"),
            }
            Ok(Value::Null)
        })
    }
}

impl State {
    fn render_split(
        &self,
        node: &Node,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.splits.borrow().get(&key).cloned();
        let entry = existing.unwrap_or_else(|| {
            let entity = cx.new(|cx| SplitButton::new(node.id.clone(), window, cx));
            let route = Rc::new(RefCell::new(Route {
                events: BTreeMap::new(),
                emit: emit.clone(),
                disabled: false,
            }));
            let callback = Rc::downgrade(&route);
            let parent = entity.downgrade();
            let menu = entity.read(cx).menu().clone();
            let subscription = cx.subscribe(&menu, move |_, event: &MenuEvent, cx| {
                if parent
                    .upgrade()
                    .is_none_or(|parent| parent.read(cx).is_disabled())
                {
                    return;
                }
                let (name, payload) = match event {
                    MenuEvent::Opened => ("open", Value::Null),
                    MenuEvent::Closed => ("close", Value::Null),
                    MenuEvent::Dismissed => ("dismiss", Value::Null),
                    MenuEvent::Invoked(id) => ("invoked", json!(id.as_ref())),
                };
                let target = callback.upgrade().and_then(|route| {
                    let route = route.borrow();
                    (!route.disabled)
                        .then(|| {
                            route
                                .events
                                .get(name)
                                .map(|action| (action.clone(), route.emit.clone()))
                        })
                        .flatten()
                });
                if let Some((action, emit)) = target {
                    emit(&action, payload);
                }
            });
            let entry = Rc::new(Entry {
                entity,
                route,
                props: Default::default(),
                _subscription: subscription,
            });
            self.splits.borrow_mut().insert(key, entry.clone());
            entry
        });
        let handler_changed =
            entry.route.borrow().events.contains_key("click") != node.events.contains_key("click");
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if handler_changed {
            let callback = Rc::downgrade(&entry.route);
            entry.entity.update(cx, |split, cx| {
                if node.events.contains_key("click") {
                    split.set_on_click(
                        move |_, _| {
                            let target = callback.upgrade().and_then(|route| {
                                let route = route.borrow();
                                (!route.disabled)
                                    .then(|| {
                                        route
                                            .events
                                            .get("click")
                                            .map(|action| (action.clone(), route.emit.clone()))
                                    })
                                    .flatten()
                            });
                            if let Some((action, emit)) = target {
                                emit(&action, Value::Null);
                            }
                        },
                        cx,
                    );
                } else {
                    split.clear_on_click(cx);
                }
            });
        }
        if *entry.props.borrow() != node.props {
            let items_changed = entry.props.borrow().get("items") != node.props.get("items");
            let menu_name = node
                .props
                .get("menuName")
                .and_then(Value::as_str)
                .map(|s| SharedString::from(s.to_owned()))
                .unwrap_or_else(|| cx.strings().text(StringKey::MoreActions));
            entry.entity.update(cx, |split, cx| {
                split.set_label(text(node, "label"), cx);
                split.set_icon(
                    node.props
                        .get("icon")
                        .map(|icon| super::icon::resolve(icon).expect("validated builtin icon")),
                    cx,
                );
                split.set_variant(
                    if node.props.contains_key("variant") {
                        variant(node)
                    } else {
                        ButtonVariant::Secondary
                    },
                    cx,
                );
                split.set_control_size(size(node), cx);
                split.set_default_disabled(flag(node, "defaultDisabled"), cx);
                split.set_disabled(flag(node, "disabled"), window, cx);
                split.set_menu_name(menu_name, cx);
                if items_changed {
                    split.set_items(
                        super::overlay_extra::menu_items(
                            node.props.get("items").unwrap_or(&Value::Null),
                        ),
                        cx,
                    );
                }
            });
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }

    fn invoke_split(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        let entity = self
            .splits
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .map(|entry| entry.entity.clone())
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        anyhow::ensure!(
            query || (!flag(node, "disabled") && !entity.read(cx).is_disabled()),
            "disabled target refuses invocation"
        );
        if query {
            return match method {
                "is_open" => Ok(json!(entity.read(cx).is_open(cx))),
                "is_disabled" => Ok(json!(entity.read(cx).is_disabled())),
                _ => anyhow::bail!("unsupported SplitButton query"),
            };
        }
        if method == "set_items" {
            super::overlay_extra::validate_menu_items(&args["items"])?;
        }
        entity.update(cx, |split, cx| {
            match method {
                "open_menu" => split.open_menu(window, cx),
                "set_label" => {
                    split.set_label(args["label"].as_str().unwrap_or_default().to_owned(), cx)
                }
                "set_icon" => split.set_icon(
                    if args["icon"].is_null() {
                        None
                    } else {
                        Some(super::icon::resolve(&args["icon"])?)
                    },
                    cx,
                ),
                "set_variant" => split.set_variant(
                    match args["variant"].as_str() {
                        Some("primary") => ButtonVariant::Primary,
                        Some("ghost") => ButtonVariant::Ghost,
                        Some("danger") => ButtonVariant::Danger,
                        Some("link") => ButtonVariant::Link,
                        _ => ButtonVariant::Secondary,
                    },
                    cx,
                ),
                "set_control_size" => split.set_control_size(
                    match args["size"].as_str() {
                        Some("xs") => ControlSize::Xs,
                        Some("sm") => ControlSize::Sm,
                        Some("lg") => ControlSize::Lg,
                        _ => ControlSize::Md,
                    },
                    cx,
                ),
                "set_default_disabled" => {
                    split.set_default_disabled(args["disabled"].as_bool().unwrap_or(false), cx)
                }
                "set_disabled" => {
                    split.set_disabled(args["disabled"].as_bool().unwrap_or(false), window, cx)
                }
                "set_items" => {
                    split.set_items(super::overlay_extra::menu_items(&args["items"]), cx)
                }
                "set_menu_name" => {
                    split.set_menu_name(args["name"].as_str().unwrap_or_default().to_owned(), cx)
                }
                _ => anyhow::bail!("unsupported SplitButton command"),
            }
            Ok(Value::Null)
        })
    }
}

fn copy_state(state: &CopyState) -> Value {
    match state {
        CopyState::Idle => json!({"state":"idle","reason":null}),
        CopyState::Copied => json!({"state":"copied","reason":null}),
        CopyState::Failed(reason) => json!({"state":"failed","reason":reason.as_ref()}),
    }
}

impl State {
    fn render_copy(
        &self,
        node: &Node,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.copies.borrow().get(&key).cloned();
        let entry = existing.unwrap_or_else(|| {
            let entity = cx.new(|cx| CopyButton::new(node.id.clone(), window, cx));
            let route = Rc::new(RefCell::new(Route {
                events: node.events.clone(),
                emit: emit.clone(),
                disabled: flag(node, "disabled"),
            }));
            let callback = Rc::downgrade(&route);
            let subscription = cx.subscribe(&entity, move |entity, event: &CopyEvent, cx| {
                if entity.read(cx).is_disabled() {
                    return;
                }
                let (name, payload) = match event {
                    CopyEvent::Copied => ("copied", Value::Null),
                    CopyEvent::Failed(reason) => ("failed", json!(reason.as_ref())),
                };
                let target = callback.upgrade().and_then(|route| {
                    let route = route.borrow();
                    (!route.disabled)
                        .then(|| {
                            route
                                .events
                                .get(name)
                                .map(|action| (action.clone(), route.emit.clone()))
                        })
                        .flatten()
                });
                if let Some((action, emit)) = target {
                    emit(&action, payload);
                }
            });
            let entry = Rc::new(Entry {
                entity,
                route,
                props: Default::default(),
                _subscription: subscription,
            });
            self.copies.borrow_mut().insert(key, entry.clone());
            entry
        });
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if *entry.props.borrow() != node.props {
            let confirmation = node
                .props
                .get("confirmationMs")
                .and_then(Value::as_u64)
                .unwrap_or(cx.theme().motion.confirmation_ms);
            entry.entity.update(cx, |copy, cx| {
                copy.set_text(text(node, "text"), cx);
                copy.set_label(
                    node.props
                        .get("label")
                        .and_then(Value::as_str)
                        .map(|s| s.to_owned().into()),
                    cx,
                );
                copy.set_glyph_only(
                    node.props
                        .get("glyphOnly")
                        .and_then(Value::as_str)
                        .map(|s| s.to_owned().into()),
                    cx,
                );
                copy.set_variant(
                    if node.props.contains_key("variant") {
                        variant(node)
                    } else {
                        ButtonVariant::Secondary
                    },
                    cx,
                );
                copy.set_control_size(size(node), cx);
                copy.set_confirmation(std::time::Duration::from_millis(confirmation), cx);
                copy.set_disabled(flag(node, "disabled"), cx);
            });
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }

    fn invoke_copy(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        let entity = self
            .copies
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .map(|entry| entry.entity.clone())
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        anyhow::ensure!(
            query || (!flag(node, "disabled") && !entity.read(cx).is_disabled()),
            "disabled target refuses invocation"
        );
        if query {
            return match method {
                "state" => Ok(copy_state(entity.read(cx).state())),
                "is_disabled" => Ok(json!(entity.read(cx).is_disabled())),
                _ => anyhow::bail!("unsupported CopyButton query"),
            };
        }
        entity.update(cx, |copy, cx| {
            match method {
                "copy" => copy.copy(cx),
                "set_text" => {
                    copy.set_text(args["text"].as_str().unwrap_or_default().to_owned(), cx)
                }
                "set_label" => {
                    copy.set_label(args["label"].as_str().map(|s| s.to_owned().into()), cx)
                }
                "set_glyph_only" => {
                    copy.set_glyph_only(args["name"].as_str().map(|s| s.to_owned().into()), cx)
                }
                "set_confirmation" => copy.set_confirmation(
                    std::time::Duration::from_millis(
                        args["confirmation_ms"].as_u64().unwrap_or_default(),
                    ),
                    cx,
                ),
                "set_disabled" => {
                    copy.set_disabled(args["disabled"].as_bool().unwrap_or(false), cx)
                }
                "set_variant" => copy.set_variant(
                    match args["variant"].as_str() {
                        Some("primary") => ButtonVariant::Primary,
                        Some("ghost") => ButtonVariant::Ghost,
                        Some("danger") => ButtonVariant::Danger,
                        Some("link") => ButtonVariant::Link,
                        _ => ButtonVariant::Secondary,
                    },
                    cx,
                ),
                "set_control_size" => copy.set_control_size(
                    match args["size"].as_str() {
                        Some("xs") => ControlSize::Xs,
                        Some("sm") => ControlSize::Sm,
                        Some("lg") => ControlSize::Lg,
                        _ => ControlSize::Md,
                    },
                    cx,
                ),
                _ => anyhow::bail!("unsupported CopyButton command"),
            }
            Ok(Value::Null)
        })
    }
}

/// Relational checks supplement the shared closed shape grammar.
pub(super) fn validate(node: &Node) -> anyhow::Result<()> {
    if node.component.as_deref() == Some("SplitButton") {
        super::overlay_extra::validate_menu_items(node.props.get("items").unwrap_or(&Value::Null))?;
    }
    if let Some(value) = node.props.get("color")
        && matches!(node.component.as_deref(), Some("Button" | "IconButton"))
    {
        anyhow::ensure!(
            value.as_object().is_some_and(|value| value.len() == 1),
            "color requires exactly one source"
        );
    }
    if flag(node, "iconOnly") {
        anyhow::ensure!(
            node.props.contains_key("icon") && !text(node, "accessibleName").is_empty(),
            "iconOnly requires icon and accessibleName"
        );
    }
    if node.component.as_deref() == Some("ToggleGroup") {
        for item in values(node, "items") {
            anyhow::ensure!(
                item["iconOnly"] != true || item.get("icon").is_some(),
                "iconOnly item requires icon"
            );
        }
    }
    if node.component.as_deref() == Some("FormField") && text(node, "validation") == "invalid" {
        anyhow::ensure!(
            node.props.contains_key("reason") || node.props.contains_key("error"),
            "invalid form requires reason"
        );
    }
    if node.component.as_deref() == Some("FilterBar") {
        match text(node, "countState").as_str() {
            "known" => anyhow::ensure!(
                node.props.contains_key("count"),
                "known count requires count"
            ),
            "unavailable" => anyhow::ensure!(
                node.props.contains_key("countReason"),
                "unavailable count requires reason"
            ),
            _ => {}
        }
    }
    Ok(())
}

fn ground(node: &Node) -> Surface {
    match text(node, "ground").as_str() {
        "backdrop" => Surface::Backdrop,
        "sunken" => Surface::Sunken,
        "panel" => Surface::Panel,
        "raised" => Surface::Raised,
        "overlay" => Surface::Overlay,
        _ => Surface::Canvas,
    }
}

fn join(node: &Node) -> ButtonJoin {
    match text(node, "join").as_str() {
        "leading" => ButtonJoin::Leading,
        "middle" => ButtonJoin::Middle,
        "trailing" => ButtonJoin::Trailing,
        _ => ButtonJoin::Alone,
    }
}

fn variant(node: &Node) -> ButtonVariant {
    match text(node, "variant").as_str() {
        "secondary" => ButtonVariant::Secondary,
        "ghost" => ButtonVariant::Ghost,
        "danger" => ButtonVariant::Danger,
        "link" => ButtonVariant::Link,
        _ => ButtonVariant::Primary,
    }
}

fn style(node: &Node) -> ButtonStyle {
    match text(node, "variant").as_str() {
        "filled" => Variant::Filled.into(),
        "light" => Variant::Light.into(),
        "subtle" => Variant::Subtle.into(),
        "default" => Variant::Default.into(),
        "transparent" => Variant::Transparent.into(),
        "white" => Variant::White.into(),
        _ => variant(node).into(),
    }
}

fn color_choice(value: &Value) -> ColorChoice {
    if let Some(palette) = value["palette"].as_str() {
        ColorChoice::Palette(palette.to_owned().into())
    } else if let Some(role) = value["semantic"].as_str() {
        ColorChoice::Semantic(match role {
            "accentStrong" => SemanticColor::AccentStrong,
            "danger" => SemanticColor::Danger,
            "warning" => SemanticColor::Warning,
            "success" => SemanticColor::Success,
            "info" => SemanticColor::Info,
            _ => SemanticColor::Accent,
        })
    } else {
        ColorChoice::Custom(color(&value["custom"]))
    }
}

fn button(node: &Node, emit: Emit) -> Button {
    let mut button = Button::new(node.id.clone())
        .disabled(flag(node, "disabled"))
        .control_size(size(node))
        .loading(flag(node, "loading"))
        .full_width(flag(node, "fullWidth"))
        .variant(style(node))
        .join(join(node));
    if node.props.contains_key("ground") {
        button = button.ground(ground(node));
    }
    if node.props.contains_key("label") {
        button = button.label(text(node, "label"));
    }
    if node.props.contains_key("accessibleName") {
        button = button.accessible_name(text(node, "accessibleName"));
    }
    if node.props.contains_key("accessibleDescription") {
        button = button.accessible_description(text(node, "accessibleDescription"));
    }
    if node.props.contains_key("semanticParent") {
        button = button.semantic_parent(text(node, "semanticParent"));
    }
    if node.props.contains_key("checkedState") {
        button = button.checked_state(flag(node, "checkedState"));
    }
    if let Some(value) = node.props.get("color") {
        button = button.color(color_choice(value));
    }
    if let Some(value) = node.props.get("icon") {
        let glyph = super::icon::resolve(value).expect("validated builtin icon");
        button = if flag(node, "iconOnly") {
            button.icon_only(glyph, text(node, "accessibleName"))
        } else {
            button.icon(glyph)
        };
    }
    if text(node, "iconPosition") == "trailing" {
        button = button.icon_position(IconPosition::Trailing);
    }
    if !flag(node, "disabled")
        && !flag(node, "loading")
        && let Some(action) = node.events.get("click").cloned()
    {
        button = button.on_click(move |_, _| emit(&action, Value::Null));
    }
    button
}

fn color(value: &Value) -> Hsla {
    gpui::hsla(
        value["h"].as_f64().unwrap_or_default() as f32,
        value["s"].as_f64().unwrap_or_default() as f32,
        value["l"].as_f64().unwrap_or_default() as f32,
        value["a"].as_f64().unwrap_or(1.) as f32,
    )
}

fn color_value(color: Hsla) -> Value {
    json!({"h":color.h,"s":color.s,"l":color.l,"a":color.a})
}

fn values<'a>(node: &'a Node, key: &str) -> impl Iterator<Item = &'a Value> {
    node.props
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn form(node: &Node) -> FormField {
    let mut field =
        FormField::new(node.id.clone(), text(node, "label")).required(flag(node, "required"));
    for key in ["control", "description", "hint"] {
        if node.props.contains_key(key) {
            field = match key {
                "control" => field.control(text(node, key)),
                "description" => field.description(text(node, key)),
                _ => field.hint(text(node, key)),
            };
        }
    }
    field = field.validation(match text(node, "validation").as_str() {
        "validating" => ValidationState::Validating,
        "valid" => ValidationState::Valid,
        "invalid" => ValidationState::invalid(text(node, "reason")),
        _ => ValidationState::Pending,
    });
    if node.props.contains_key("error") {
        field = field.error(text(node, "error"));
    }
    field
}

pub(super) fn invoke(
    node: &Node,
    method: &str,
    _args: &Value,
    query: bool,
) -> anyhow::Result<Value> {
    anyhow::ensure!(query, "Control has no commands");
    match (node.component.as_deref(), method) {
        (Some("FormField"), "is_invalid") => Ok(json!(form(node).is_invalid())),
        (Some("FormField"), "is_validating") => Ok(json!(form(node).is_validating())),
        _ => anyhow::bail!("Unsupported control query"),
    }
}

pub(super) fn render(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
) -> AnyElement {
    let event = |name: &str| {
        (!flag(node, "disabled"))
            .then(|| node.events.get(name).cloned())
            .flatten()
    };
    match node.component.as_deref().unwrap_or_default() {
        "InlineEdit" => {
            let mut control = gpui_kit::controls::inline_edit::InlineEdit::new(
                node.id.clone(),
                text(node, "value"),
            )
            .editing(flag(node, "editing"))
            .multiline(flag(node, "multiline"))
            .rows(number(node, "rows", 3.) as usize)
            .disabled(flag(node, "disabled"))
            .control_size(size(node));
            if node.props.contains_key("placeholder") {
                control = control.placeholder(text(node, "placeholder"));
            }
            if node.props.contains_key("failure") {
                control = control.failure(text(node, "failure"));
            }
            if let Some(action) = event("edit") {
                let emit = emit.clone();
                control = control.on_edit(move |_, _| emit(&action, Value::Null));
            }
            if let Some(action) = event("commit") {
                let emit = emit.clone();
                control =
                    control.on_commit(move |value, _, _| emit(&action, json!(value.as_ref())));
            }
            if let Some(action) = event("cancel") {
                control = control.on_cancel(move |_, _| emit(&action, Value::Null));
            }
            control.into_any_element()
        }
        "SettingsRow" => settings_row(node, slots, window, cx).into_any_element(),
        "Button" => button(node, emit).into_any_element(),
        "IconButton" => {
            let glyph = super::icon::resolve(&node.props["icon"]).expect("validated builtin icon");
            let mut button = IconButton::new(node.id.clone(), glyph, text(node, "accessibleName"))
                .disabled(flag(node, "disabled"))
                .control_size(size(node))
                .loading(flag(node, "loading"))
                .variant(style(node))
                .join(join(node));
            if node.props.contains_key("ground") {
                button = button.ground(ground(node));
            }
            if node.props.contains_key("semanticParent") {
                button = button.semantic_parent(text(node, "semanticParent"));
            }
            if let Some(value) = node.props.get("color") {
                button = button.color(color_choice(value));
            }
            if !flag(node, "loading")
                && let Some(action) = event("click")
            {
                button = button.on_click(move |_, _| emit(&action, Value::Null));
            }
            button.into_any_element()
        }
        "Toggle" => {
            let mut toggle = Toggle::new(node.id.clone())
                .disabled(flag(node, "disabled"))
                .control_size(size(node))
                .pressed(flag(node, "pressed"))
                .join(join(node));
            if node.props.contains_key("variant") {
                toggle = toggle.variant(variant(node));
            }
            if node.props.contains_key("ground") {
                toggle = toggle.ground(ground(node));
            }
            if node.props.contains_key("label") {
                toggle = toggle.label(text(node, "label"));
            }
            if node.props.contains_key("accessibleName") {
                toggle = toggle.accessible_name(text(node, "accessibleName"));
            }
            if node.props.contains_key("semanticParent") {
                toggle = toggle.semantic_parent(text(node, "semanticParent"));
            }
            if let Some(value) = node.props.get("icon") {
                let glyph = super::icon::resolve(value).expect("validated builtin icon");
                toggle = if flag(node, "iconOnly") {
                    toggle.icon_only(glyph, text(node, "accessibleName"))
                } else {
                    toggle.icon(glyph)
                };
            }
            if let Some(action) = event("press") {
                toggle = toggle.on_press(move |pressed, _, _| emit(&action, json!(pressed)));
            }
            toggle.into_any_element()
        }
        "ToggleGroup" => {
            let items = values(node, "items").map(|value| {
                let id = value["id"].as_str().unwrap_or_default().to_owned();
                let label = value["label"].as_str().unwrap_or_default().to_owned();
                let mut item = if value["iconOnly"].as_bool() == Some(true) {
                    ToggleItem::glyph(
                        id,
                        super::icon::resolve(&value["icon"]).expect("validated builtin icon"),
                        label,
                    )
                } else {
                    ToggleItem::new(id, label)
                };
                if let Some(icon) = value.get("icon") {
                    item = item.icon(super::icon::resolve(icon).expect("validated builtin icon"));
                }
                item.disabled(value["disabled"].as_bool().unwrap_or(false))
            });
            let pressed = values(node, "pressed")
                .filter_map(Value::as_str)
                .map(|value| SharedString::from(value.to_owned()));
            let mut group = ToggleGroup::new(node.id.clone())
                .items(items)
                .pressed(pressed)
                .disabled(flag(node, "disabled"))
                .control_size(size(node));
            if text(node, "selection") == "atMostOne" {
                group = group.selection(ToggleSelection::AtMostOne);
            }
            if node.props.contains_key("label") {
                group = group.label(text(node, "label"));
            }
            if node.props.contains_key("variant") {
                group = group.variant(variant(node));
            }
            if node.props.contains_key("ground") {
                group = group.ground(ground(node));
            }
            if let Some(action) = event("change") {
                group = group.on_change(move |ids, changed, _, _| emit(&action, json!({"pressed":ids.iter().map(AsRef::<str>::as_ref).collect::<Vec<_>>(),"changed":changed.as_ref()})));
            }
            group.into_any_element()
        }
        "ColorPicker" => {
            let mut picker = ColorPicker::new(node.id.clone(), color(&node.props["value"]))
                .disabled(flag(node, "disabled"))
                .alpha(flag(node, "alpha"))
                .presets(values(node, "presets").map(color))
                .recent(values(node, "recent").map(color));
            if let Some(action) = event("change") {
                picker = picker.on_change(move |value, _, _| emit(&action, color_value(value)));
            }
            picker.into_any_element()
        }
        "ColorSwatch" => {
            let mut swatch = ColorSwatch::new(node.id.clone(), color(&node.props["color"]))
                .disabled(flag(node, "disabled"))
                .selected(flag(node, "selected"));
            if let Some(action) = event("click") {
                swatch = swatch.on_click(move |value, _, _| emit(&action, color_value(value)));
            }
            swatch.into_any_element()
        }
        "FormField" => {
            let mut field = form(node);
            if let Some(content) = slots.get("content") {
                field = field.child(content(window, cx));
            }
            field.into_any_element()
        }
        "FilterBar" => {
            let conditions = values(node, "conditions").map(|value| {
                let mut condition = FilterCondition::new(
                    value["id"].as_str().unwrap_or_default().to_owned(),
                    value["field"].as_str().unwrap_or_default().to_owned(),
                    value["operator"].as_str().unwrap_or_default().to_owned(),
                    value["value"].as_str().unwrap_or_default().to_owned(),
                );
                condition = condition.tone(match value["tone"].as_str() {
                    Some("accent") => Tone::Accent,
                    Some("success") => Tone::Success,
                    Some("warning") => Tone::Warning,
                    Some("danger") => Tone::Danger,
                    Some("info") => Tone::Info,
                    _ => Tone::Neutral,
                });
                condition
            });
            let count = match text(node, "countState").as_str() {
                "counting" => ResultCount::Counting,
                "known" => ResultCount::Known(number(node, "count", 0.) as usize),
                "unavailable" => ResultCount::Unavailable(text(node, "countReason").into()),
                _ => ResultCount::Unknown,
            };
            let mut bar = FilterBar::new(node.id.clone())
                .conditions(conditions)
                .count(count)
                .disabled(flag(node, "disabled"))
                .control_size(size(node));
            if node.props.contains_key("noun") {
                bar = bar.noun(text(node, "noun"));
            }
            if node.props.contains_key("addLabel") {
                bar = bar.add_label(text(node, "addLabel"));
            }
            if node.props.contains_key("clearLabel") {
                bar = bar.clear_label(text(node, "clearLabel"));
            }
            if let Some(content) = slots.get("add_control") {
                bar = bar.add_control(content(window, cx));
            }
            if let Some(action) = event("add") {
                let emit = emit.clone();
                bar = bar.on_add(move |_, _| emit(&action, Value::Null));
            }
            if let Some(action) = event("remove") {
                let emit = emit.clone();
                bar = bar.on_remove(move |id, _, _| emit(&action, json!(id.as_ref())));
            }
            if let Some(action) = event("clear") {
                bar = bar.on_clear(move |_, _| emit(&action, Value::Null));
            }
            bar.into_any_element()
        }
        _ => unreachable!("family dispatch validates component membership"),
    }
}

/// Also used by the host's guarded typed-child factory before erasure.
pub(super) fn settings_row(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
) -> SettingsRow {
    let mut row = SettingsRow::new(node.id.clone(), text(node, "label"));
    if node.props.contains_key("description") {
        row = row.description(text(node, "description"));
    }
    if node.props.contains_key("labelWidth") {
        row = row.label_width(gpui::px(number(node, "labelWidth", 0.)));
    }
    if node.props.contains_key("badge") {
        row = row.badge(text(node, "badge"));
    }
    if node.props.contains_key("value") {
        row = row.value(text(node, "value"));
    }
    row = row.search_terms(
        values(node, "searchTerms")
            .filter_map(Value::as_str)
            .map(str::to_owned),
    );
    if node.props.contains_key("managed") {
        // A refused control must not even construct its child or subscriptions.
        row = row.managed(text(node, "managed"));
    } else if let Some(control) = slots.get("control") {
        row = row.control(control(window, cx));
    }
    row
}

fn transfer_items(value: Option<&Value>) -> Vec<TransferItem> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| {
            TransferItem::new(
                item["id"].as_str().unwrap_or_default().to_owned(),
                item["label"].as_str().unwrap_or_default().to_owned(),
            )
            .disabled(item["disabled"].as_bool().unwrap_or(false))
        })
        .collect()
}

fn strings(value: Option<&Value>) -> Vec<SharedString> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|value| value.to_owned().into())
        .collect()
}

impl State {
    fn render_transfer(
        &self,
        node: &Node,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.transfers.borrow().get(&key).cloned();
        let entry = existing.unwrap_or_else(|| {
            let entity = cx.new(|cx| TransferList::new(node.id.clone(), window, cx));
            let route = Rc::new(RefCell::new(Route {
                events: node.events.clone(),
                emit: emit.clone(),
                disabled: flag(node, "disabled"),
            }));
            let callback = Rc::downgrade(&route);
            let subscription = cx.subscribe(&entity, move |_, event: &TransferListEvent, _| {
                let (name, payload) = match event {
                    TransferListEvent::ToggleSource(id) => ("toggleSource", json!(id.as_ref())),
                    TransferListEvent::ToggleTarget(id) => ("toggleTarget", json!(id.as_ref())),
                    TransferListEvent::MoveToTarget => ("moveToTarget", Value::Null),
                    TransferListEvent::MoveToSource => ("moveToSource", Value::Null),
                    TransferListEvent::QueryChanged(query) => {
                        ("queryChange", json!(query.as_ref()))
                    }
                };
                let target = callback.upgrade().and_then(|route| {
                    let route = route.borrow();
                    (!route.disabled)
                        .then(|| {
                            route
                                .events
                                .get(name)
                                .map(|action| (action.clone(), route.emit.clone()))
                        })
                        .flatten()
                });
                if let Some((action, emit)) = target {
                    emit(&action, payload);
                }
            });
            let entry = Rc::new(Entry {
                entity,
                route,
                props: Default::default(),
                _subscription: subscription,
            });
            self.transfers.borrow_mut().insert(key, entry.clone());
            entry
        });
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if *entry.props.borrow() != node.props {
            entry.entity.update(cx, |list, cx| {
                list.set_items(
                    transfer_items(node.props.get("source")),
                    transfer_items(node.props.get("target")),
                    cx,
                );
                list.set_selection(
                    strings(node.props.get("sourceSelected")),
                    strings(node.props.get("targetSelected")),
                    cx,
                );
                list.set_labels(
                    text(node, "sourceLabel").into(),
                    text(node, "targetLabel").into(),
                    cx,
                );
                list.set_control_size(size(node), cx);
                list.set_disabled(flag(node, "disabled"), cx);
                if node.props.contains_key("query") {
                    list.set_query(text(node, "query"), cx);
                }
            });
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }

    fn invoke_transfer(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        let entity = self
            .transfers
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .map(|entry| entry.entity.clone())
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        anyhow::ensure!(
            query || (!flag(node, "disabled") && !entity.read(cx).is_disabled()),
            "disabled target refuses invocation"
        );
        if query {
            anyhow::ensure!(method == "is_disabled", "unsupported TransferList query");
            return Ok(json!(entity.read(cx).is_disabled()));
        }
        entity.update(cx, |list, cx| {
            match method {
                "set_query" => {
                    list.set_query(args["query"].as_str().unwrap_or_default().to_owned(), cx)
                }
                "set_items" => list.set_items(
                    transfer_items(args.get("source")),
                    transfer_items(args.get("target")),
                    cx,
                ),
                "set_selection" => {
                    list.set_selection(strings(args.get("source")), strings(args.get("target")), cx)
                }
                "set_labels" => list.set_labels(
                    args["source"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned()
                        .into(),
                    args["target"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned()
                        .into(),
                    cx,
                ),
                "set_control_size" => list.set_control_size(
                    match args["size"].as_str() {
                        Some("xs") => ControlSize::Xs,
                        Some("sm") => ControlSize::Sm,
                        Some("lg") => ControlSize::Lg,
                        _ => ControlSize::Md,
                    },
                    cx,
                ),
                "set_disabled" => {
                    list.set_disabled(args["disabled"].as_bool().unwrap_or(false), cx)
                }
                _ => anyhow::bail!("unsupported TransferList command"),
            }
            Ok(Value::Null)
        })
    }
}
