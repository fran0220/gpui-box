//! Native controls whose data and actions remain owned by the worker.
//! Slot factories are invoked only after releasing retained-state borrows.
use super::*;
use gpui::{Hsla, ParentElement};
use gpui_kit::controls::button::{ButtonJoin, ButtonStyle, IconPosition};
use gpui_kit::state::ValidationState;
use gpui_kit_theme::{ColorChoice, SemanticColor, Surface, Variant};

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
];

struct SearchEntry {
    entity: Entity<SearchInput>,
    route: Rc<RefCell<Route>>,
    props: RefCell<serde_json::Map<String, Value>>,
    _subscription: Subscription,
}

#[derive(Default)]
pub(super) struct State {
    searches: RefCell<HashMap<Key, Rc<SearchEntry>>>,
}

impl State {
    pub(super) fn reconcile(&self, root: &Node, _cx: &mut App) {
        fn visit(node: &Node, live: &mut Vec<Key>) {
            if node.component.as_deref() == Some("SearchInput") {
                live.push((node.instance, node.id.clone()));
            }
            for child in node.children.iter().chain(node.slots.values().flatten()) {
                visit(child, live);
            }
        }
        let mut live = Vec::new();
        visit(root, &mut live);
        self.searches
            .borrow_mut()
            .retain(|key, _| live.contains(key));
    }

    pub(super) fn render(
        &self,
        node: &Node,
        slots: KitSlots,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
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
            let entry = Rc::new(SearchEntry {
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

/// Relational checks supplement the shared closed shape grammar.
pub(super) fn validate(node: &Node) -> anyhow::Result<()> {
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
