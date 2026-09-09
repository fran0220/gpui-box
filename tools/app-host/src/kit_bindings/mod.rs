//! Data-only adapters. The host validates frames and supplies a revision-scoped
//! emitter. Native entities survive renders, but never process replacement.
use super::Node;
use gpui::{
    AnyElement, App, AppContext as _, Entity, IntoElement, SharedString, Subscription, Window,
};
use gpui_kit::prelude::*;
use gpui_kit_theme::ControlSize;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    rc::Rc,
};

mod agent;
mod canvas;
mod charts;
mod collections;
mod content;
mod controls_extra;
mod data_extra;
mod datetime;
mod display;
mod game_effects;
mod icon;
mod invocation;
mod layout;
mod layout_extra;
mod media;
mod navigation_extra;
mod overlay_extra;
mod overlays;
pub(crate) mod reference_dispatch;
mod structured;
mod validation;
pub(super) use validation::validate_descriptor;

#[cfg(all(test, feature = "capture"))]
mod resource_e2e_tests;
#[cfg(all(test, feature = "capture"))]
mod tests;

pub(super) static COMPONENTS: std::sync::LazyLock<Vec<&str>> = std::sync::LazyLock::new(|| {
    [
        CORE_COMPONENTS,
        controls_extra::COMPONENTS,
        navigation_extra::COMPONENTS,
        layout_extra::COMPONENTS,
        datetime::COMPONENTS,
        agent::COMPONENTS,
        canvas::COMPONENTS,
        charts::COMPONENTS,
        content::COMPONENTS,
        data_extra::COMPONENTS,
        display::COMPONENTS,
        game_effects::COMPONENTS,
        media::COMPONENTS,
        overlay_extra::COMPONENTS,
        structured::COMPONENTS,
    ]
    .concat()
});
const CORE_COMPONENTS: &[&str] = &[
    "Checkbox",
    "Radio",
    "Switch",
    "Slider",
    "SegmentedControl",
    "TextInput",
    "Select",
    "Pagination",
    "Tabs",
    "Accordion",
    "ScrollArea",
    "SplitPane",
    "Divider",
    "List",
    "Popover",
    "Dialog",
];
type Emit = Rc<dyn Fn(&str, Value)>;
type Key = (u64, String);
/// Host factories retain validated descriptors, not previously consumed elements.
/// They must preserve the source owner/generation and refuse revoked owners.
pub(super) type KitSlots = BTreeMap<String, Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>>;

struct Route {
    events: BTreeMap<String, String>,
    emit: Emit,
    disabled: bool,
}
impl Route {
    fn send(&self, event: &str, payload: Value) {
        if !self.disabled
            && let Some(action) = self.events.get(event)
        {
            (self.emit)(action, payload);
        }
    }
}
enum Control {
    Input(Entity<TextInput>),
    Select(Entity<Select>),
    Popover(Entity<Popover>, Rc<RefCell<KitSlots>>),
    Dialog(Entity<Dialog>, Rc<RefCell<KitSlots>>),
}
struct Retained {
    control: Control,
    route: Rc<RefCell<Route>>,
    props: RefCell<serde_json::Map<String, Value>>,
    slot_data: RefCell<BTreeMap<String, Vec<Node>>>,
    _subscriptions: Vec<Subscription>,
}
#[derive(Default)]
pub(super) struct KitState {
    retained: RefCell<HashMap<Key, Rc<Retained>>>,
    controls_extra: controls_extra::State,
    navigation_extra: navigation_extra::State,
    layout_extra: layout_extra::State,
    datetime: datetime::State,
    agent: agent::State,
    content: content::State,
    media: media::State,
    overlay_extra: overlay_extra::State,
    structured: structured::State,
}

fn flag(node: &Node, key: &str) -> bool {
    node.props
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
fn text(node: &Node, key: &str) -> String {
    node.props
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
fn number(node: &Node, key: &str, default: f32) -> f32 {
    node.props
        .get(key)
        .and_then(Value::as_f64)
        .map_or(default, |v| v as f32)
}
/// Bare native drag data, without target geometry or external-file authority.
pub(super) fn drag_item_payload(item: &gpui_kit::interaction::DragItem) -> Value {
    json!({
        "id":item.id.as_ref(), "source":item.source.as_ref(),
        "label":item.label.as_ref(), "kind":item.kind.as_ref(),
        "icon":item.icon.map(|icon| json!({
            "key":icon.name().source_name(),
            "weight":match icon.weight() {
                gpui_kit::assets::IconWeight::Regular => "regular",
                gpui_kit::assets::IconWeight::Fill => "fill",
            }
        })),
    })
}
fn drop_payload(intent: &gpui_kit::interaction::DropIntent) -> Value {
    json!({
        "id":intent.item.id.as_ref(), "source":intent.item.source.as_ref(),
        "label":intent.item.label.as_ref(), "kind":intent.item.kind.as_ref(),
        "anchor":intent.position.anchor().as_ref(), "position":intent.position.verb(),
        "velocity":{"x":intent.velocity.x,"y":intent.velocity.y},
    })
}
pub(crate) fn deferred_payload(node: &Node, intent: &gpui_kit::interaction::DropIntent) -> Value {
    if node.component.as_deref() == Some("Tree") {
        data_extra::drop_value(intent)
    } else {
        drop_payload(intent)
    }
}
fn size(node: &Node) -> ControlSize {
    match text(node, "size").as_str() {
        "xs" => ControlSize::Xs,
        "sm" => ControlSize::Sm,
        "lg" => ControlSize::Lg,
        _ => ControlSize::Md,
    }
}
/// Builds options after the shared option schema has validated the descriptor.
/// An absent optional array means no options; this is not an ingress validator.
pub(super) fn select_options(value: Option<&Value>) -> Vec<SelectOption> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| {
            let mut option = SelectOption::new(
                item["id"].as_str().unwrap_or_default().to_owned(),
                item["label"].as_str().unwrap_or_default().to_owned(),
            )
            .disabled(item["disabled"].as_bool().unwrap_or(false));
            if let Some(description) = item["description"].as_str() {
                option = option.description(description.to_owned());
            }
            if let Some(group) = item["group"].as_str() {
                option = option.group(group.to_owned());
            }
            option
        })
        .collect()
}

impl KitState {
    pub(super) fn native_entity_id(&self, node: &Node) -> Option<gpui::EntityId> {
        self.retained
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .map(|entry| match &entry.control {
                Control::Input(entity) => entity.entity_id(),
                Control::Select(entity) => entity.entity_id(),
                Control::Popover(entity, _) => entity.entity_id(),
                Control::Dialog(entity, _) => entity.entity_id(),
            })
            .or_else(|| self.controls_extra.native_entity_id(node))
            .or_else(|| self.overlay_extra.native_entity_id(node))
    }

    pub(super) fn reconcile(&self, root: &Node, _cx: &mut App) {
        self.controls_extra.reconcile(root, _cx);
        self.navigation_extra.reconcile(root, _cx);
        self.layout_extra.reconcile(root, _cx);
        self.datetime.reconcile(root, _cx);
        self.agent.reconcile(root, _cx);
        self.content.reconcile(root, _cx);
        self.media.reconcile(root, _cx);
        self.overlay_extra.reconcile(root, _cx);
        self.structured.reconcile(root, _cx);
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
        self.retained.borrow_mut().retain(|key, entry| {
            live.get(key).is_some_and(|component| {
                matches!(
                    (&entry.control, component.as_str()),
                    (Control::Input(_), "TextInput")
                        | (Control::Select(_), "Select")
                        | (Control::Popover(..), "Popover")
                        | (Control::Dialog(..), "Dialog")
                )
            })
        });
    }

    /// Build typed native children before erasing the parent's element type.
    pub(super) fn render_constructed(
        &self,
        node: &Node,
        context: crate::construction::NativeBuildContext,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> anyhow::Result<AnyElement> {
        Ok(match node.component.as_deref() {
            Some("List") => collections::render_list(node, context.slots, emit, context.deferred),
            Some("Tabs") => layout::render(node, context.slots, window, cx, emit, context.deferred),
            Some("Tree") => {
                data_extra::render_deferred(node, context.slots, window, cx, emit, context.deferred)
            }
            Some("ButtonGroup") => {
                controls_extra::button_group(node, context, window, cx)?.into_any_element()
            }
            Some("SettingsSection") => {
                controls_extra::settings_section(node, context, window, cx)?.into_any_element()
            }
            Some("SettingsList") => {
                controls_extra::settings_list(node, context, window, cx)?.into_any_element()
            }
            Some("UploadList") => {
                controls_extra::upload_list(node, context, window, cx, emit)?.into_any_element()
            }
            _ => self.render(node, context.slots, window, cx, emit),
        })
    }

    pub(super) fn apply_reference_props(
        &self,
        node: &Node,
        cx: &mut App,
        refs: &crate::references::Registration<'_>,
    ) -> anyhow::Result<()> {
        self.overlay_extra.apply_reference_props(node, cx, refs)
    }

    pub(super) fn render(
        &self,
        node: &Node,
        slots: KitSlots,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let component = node.component.as_deref().unwrap_or_default();
        if controls_extra::COMPONENTS.contains(&component) {
            return self.controls_extra.render(node, slots, window, cx, emit);
        }
        if navigation_extra::COMPONENTS.contains(&component) {
            return self.navigation_extra.render(node, slots, window, cx, emit);
        }
        if layout_extra::COMPONENTS.contains(&component) {
            return self.layout_extra.render(node, slots, window, cx, emit);
        }
        if datetime::COMPONENTS.contains(&component) {
            return self.datetime.render(node, slots, window, cx, emit);
        }
        if agent::COMPONENTS.contains(&component) {
            return self.agent.render(node, slots, window, cx, emit);
        }
        if content::COMPONENTS.contains(&component) {
            return self.content.render(node, slots, window, cx, emit);
        }
        if media::COMPONENTS.contains(&component) {
            return self.media.render(node, slots, window, cx, emit);
        }
        if overlay_extra::COMPONENTS.contains(&component) {
            return self.overlay_extra.render(node, slots, window, cx, emit);
        }
        if structured::COMPONENTS.contains(&component) {
            return self.structured.render(node, slots, window, cx, emit);
        }
        if display::COMPONENTS.contains(&component) {
            return display::render(node, slots, window, cx, emit);
        }
        if charts::COMPONENTS.contains(&component) {
            return charts::render(node, slots, window, cx, emit);
        }
        if canvas::COMPONENTS.contains(&component) {
            return canvas::render(node, slots, window, cx, emit);
        }
        if data_extra::COMPONENTS.contains(&component) {
            return data_extra::render(node, slots, window, cx, emit);
        }
        if game_effects::COMPONENTS.contains(&component) {
            return game_effects::render(node, slots, window, cx, emit);
        }
        let id = SharedString::from(node.id.clone());
        let disabled = flag(node, "disabled");
        let event = |name: &str| {
            if disabled {
                None
            } else {
                node.events.get(name).cloned()
            }
        };
        match node.component.as_deref().unwrap_or_default() {
            "Checkbox" => {
                let mut control = Checkbox::new(id)
                    .disabled(disabled)
                    .control_size(size(node));
                if node.props.contains_key("label") {
                    control = control.label(text(node, "label"));
                }
                if node.props.contains_key("description") {
                    control = control.description(text(node, "description"));
                }
                control = if node.props.get("checked") == Some(&Value::Null) {
                    control.mixed()
                } else {
                    control.checked(flag(node, "checked"))
                };
                if let Some(action) = event("change") {
                    control = control.on_change(move |value, _, _| emit(&action, json!(value)));
                }
                control.into_any_element()
            }
            "Radio" => {
                let mut control = Radio::new(id)
                    .selected(flag(node, "selected"))
                    .disabled(disabled)
                    .control_size(size(node));
                if node.props.contains_key("label") {
                    control = control.label(text(node, "label"));
                }
                if node.props.contains_key("description") {
                    control = control.description(text(node, "description"));
                }
                if let Some(action) = event("select") {
                    control = control.on_select(move |_, _| emit(&action, Value::Null));
                }
                control.into_any_element()
            }
            "Switch" => {
                let mut control = Switch::new(id)
                    .on(flag(node, "on"))
                    .invalid(flag(node, "invalid"))
                    .disabled(disabled)
                    .control_size(size(node));
                if node.props.contains_key("label") {
                    control = control.label(text(node, "label"));
                }
                if node.props.contains_key("description") {
                    control = control.description(text(node, "description"));
                }
                if node.props.contains_key("name") {
                    control = control.named(text(node, "name"));
                }
                if let Some(action) = event("change") {
                    control = control.on_change(move |value, _, _| emit(&action, json!(value)));
                }
                control.into_any_element()
            }
            "Slider" => {
                let min = number(node, "min", 0.);
                let mut control = Slider::new(id)
                    .label(text(node, "label"))
                    .range(min, number(node, "max", 1.))
                    .value(number(node, "value", min))
                    .disabled(disabled)
                    .control_size(size(node));
                if node.props.contains_key("high") {
                    control = control.high(number(node, "high", 1.));
                }
                if node.props.contains_key("step") {
                    control = control.step(number(node, "step", 0.01));
                }
                if node.props.contains_key("length") {
                    control = control.length(number(node, "length", 200.));
                }
                if node.props.contains_key("display") {
                    control = control.display(text(node, "display"));
                }
                if text(node, "orientation") == "vertical" {
                    control = control.orientation(SliderOrientation::Vertical);
                }
                if let Some(marks) = node.props.get("marks").and_then(Value::as_array) {
                    control =
                        control.marks(marks.iter().filter_map(Value::as_f64).map(|v| v as f32));
                }
                if let Some(action) = event("change") {
                    let emit = emit.clone();
                    control = control.on_change(move |value, _, _| emit(&action, json!(value)));
                }
                if let Some(action) = event("rangeChange") {
                    control = control.on_range_change(move |low, high, _, _| {
                        emit(&action, json!({"low":low,"high":high}))
                    });
                }
                control.into_any_element()
            }
            "SegmentedControl" => {
                let segments = node
                    .props
                    .get("segments")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(|item| {
                        Segment::new(
                            item["id"].as_str().unwrap_or_default().to_owned(),
                            item["label"].as_str().unwrap_or_default().to_owned(),
                        )
                        .disabled(item["disabled"].as_bool().unwrap_or(false))
                    });
                let mut control = SegmentedControl::new(id)
                    .label(text(node, "label"))
                    .segments(segments)
                    .disabled(disabled)
                    .control_size(size(node));
                if node.props.contains_key("selected") {
                    control = control.selected(text(node, "selected"));
                }
                if let Some(action) = event("select") {
                    control =
                        control.on_select(move |value, _, _| emit(&action, json!(value.as_ref())));
                }
                control.into_any_element()
            }
            "TextInput" | "Select" => {
                let key = (node.instance, node.id.clone());
                let entry = self
                    .retained
                    .borrow_mut()
                    .entry(key)
                    .or_insert_with(|| {
                        let route = Rc::new(RefCell::new(Route {
                            events: BTreeMap::new(),
                            emit: emit.clone(),
                            disabled: true,
                        }));
                        let callback = route.clone();
                        if node.component.as_deref() == Some("TextInput") {
                            let entity = cx.new(|cx| {
                                let mut input = TextInput::new(id, window, cx)
                                    .text(text(node, "text"))
                                    .placeholder(text(node, "placeholder"))
                                    .name(text(node, "name"))
                                    .required(flag(node, "required"))
                                    .secret(flag(node, "secret"))
                                    .bare(flag(node, "bare"))
                                    .control_size(size(node));
                                if let Some(max) =
                                    node.props.get("maxLength").and_then(Value::as_u64)
                                {
                                    input = input.max_length(max as usize);
                                }
                                input
                            });
                            let subscription =
                                cx.subscribe(&entity, move |_, event: &TextInputEvent, _| {
                                    let (name, value) = match event {
                                        TextInputEvent::Change(value) => {
                                            ("change", json!(value.as_ref()))
                                        }
                                        TextInputEvent::Submit => ("submit", Value::Null),
                                        TextInputEvent::Cancel => ("cancel", Value::Null),
                                        TextInputEvent::BackspaceAtStart => {
                                            ("backspaceAtStart", Value::Null)
                                        }
                                        TextInputEvent::Focus => ("focus", Value::Null),
                                        TextInputEvent::Blur => ("blur", Value::Null),
                                    };
                                    callback.borrow().send(name, value);
                                });
                            let callback = route.clone();
                            let denial_subscription = cx.subscribe(
                                &entity,
                                move |_, denial: &gpui::ClipboardDenied, _| {
                                    let reason = match denial {
                                        gpui::ClipboardDenied::MissingOwner => "missingOwner",
                                        gpui::ClipboardDenied::Denied => "denied",
                                    };
                                    callback.borrow().send("clipboardDenied", json!(reason));
                                },
                            );
                            Rc::new(Retained {
                                control: Control::Input(entity),
                                route,
                                props: Default::default(),
                                slot_data: Default::default(),
                                _subscriptions: vec![subscription, denial_subscription],
                            })
                        } else {
                            let entity = cx.new(|cx| {
                                Select::new(id, window, cx)
                                    .placeholder(text(node, "placeholder"))
                                    .clearable(flag(node, "clearable"))
                                    .control_size(size(node))
                            });
                            let subscription =
                                cx.subscribe(&entity, move |_, event: &SelectEvent, _| {
                                    let (name, value) = match event {
                                        SelectEvent::Selected(value) => {
                                            ("change", json!(value.as_ref()))
                                        }
                                        SelectEvent::Cleared => ("change", Value::Null),
                                        SelectEvent::Opened => ("open", Value::Null),
                                        SelectEvent::Closed => ("close", Value::Null),
                                    };
                                    callback.borrow().send(name, value);
                                });
                            Rc::new(Retained {
                                control: Control::Select(entity),
                                route,
                                props: Default::default(),
                                slot_data: Default::default(),
                                _subscriptions: vec![subscription],
                            })
                        }
                    })
                    .clone();
                // The map borrow ends above, before updating controls or evaluating slots.
                let previous_props = entry.props.borrow().clone();
                *entry.route.borrow_mut() = Route {
                    events: node.events.clone(),
                    emit,
                    disabled,
                };
                match &entry.control {
                    Control::Input(entity) => {
                        entity.update(cx, |input, cx| {
                            // Quiet reconciliation does not report a programmatic change as typing.
                            if node.props.contains_key("text")
                                && input.value().as_ref() != text(node, "text")
                            {
                                input.set_text_quietly(text(node, "text"), cx);
                            }
                            if previous_props == node.props {
                                return;
                            }
                            input.set_name(text(node, "name"), cx);
                            input.set_placeholder(text(node, "placeholder"), cx);
                            input.set_disabled(disabled, cx);
                            input.set_invalid(flag(node, "invalid"), cx);
                            input.set_read_only(flag(node, "readOnly"), cx);
                            input.set_required(flag(node, "required"), cx);
                            input.set_bare(flag(node, "bare"), cx);
                            input.set_secret(flag(node, "secret"), cx);
                            input.set_control_size(size(node), cx);
                            input.set_max_length(
                                node.props
                                    .get("maxLength")
                                    .and_then(Value::as_u64)
                                    .map(|v| v as usize),
                                cx,
                            );
                        });
                        *entry.props.borrow_mut() = node.props.clone();
                        entity.clone().into_any_element()
                    }
                    Control::Select(entity) => {
                        entity.update(cx, |select, cx| {
                            if previous_props.get("options") != node.props.get("options") {
                                select.set_options(select_options(node.props.get("options")), cx);
                            }
                            let selected = node
                                .props
                                .get("selected")
                                .and_then(Value::as_str)
                                .map(|v| SharedString::from(v.to_owned()));
                            if node.props.contains_key("selected")
                                && select.selected_id() != selected.as_ref()
                            {
                                select.set_selected(selected, cx);
                            }
                            if previous_props == node.props {
                                return;
                            }
                            select.set_name(text(node, "name"), cx);
                            select.set_invalid(flag(node, "invalid"), cx);
                            select.set_disabled(disabled, cx);
                            select.set_placeholder(
                                node.props
                                    .get("placeholder")
                                    .and_then(Value::as_str)
                                    .map(|v| SharedString::from(v.to_owned())),
                                cx,
                            );
                            select.set_clearable(flag(node, "clearable"), cx);
                            select.set_control_size(size(node), cx);
                        });
                        *entry.props.borrow_mut() = node.props.clone();
                        entity.clone().into_any_element()
                    }
                    _ => unreachable!("reconcile replaces changed component identity"),
                }
            }
            "List" => collections::render_list(node, slots, emit, None),
            "Popover" | "Dialog" => overlays::render(self, node, slots, window, cx, emit),
            _ => layout::render(node, slots, window, cx, emit, None),
        }
    }
}
