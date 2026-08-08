//! A per-frame semantic tree for native GPUI applications.
//!
//! Native windows have no DOM. Views attach a zero-paint probe to meaningful
//! elements; prepaint records the bounds GPUI actually produced. Nodes absent
//! from the next frame disappear instead of lingering as stale claims.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};

use gpui::{
    App, Bounds, FocusHandle, Global, InteractiveElement, IntoElement, ParentElement, Pixels,
    SharedString, StatefulInteractiveElement, Styled, Toggled, canvas,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    Window,
    #[default]
    Region,
    Group,
    List,
    Row,
    Button,
    Link,
    Tab,
    TabPanel,
    Input,
    MultilineInput,
    PasswordInput,
    Text,
    Heading,
    Dialog,
    Menu,
    MenuItem,
    Status,
    Checkbox,
    Radio,
    Switch,
    Slider,
    Table,
    Cell,
    Tree,
    TreeItem,
    Progress,
    Toast,
    Tooltip,
    Separator,
    Splitter,
    Toolbar,
    Scrollbar,
    Combobox,
    Option,
    Form,
    Field,
    Image,
    /// A drag in flight: what is being carried, and where it would land.
    Drag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LiveRegion {
    Polite,
    Assertive,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn area(self) -> f32 {
        self.width.max(0.0) * self.height.max(0.0)
    }

    pub fn center(self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn overlaps(self, other: Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

/// A single assertion target published for one frame.
///
/// Fields added after the initial protocol are omitted from serialized
/// snapshots unless a component sets them, so recorded baselines stay stable
/// as new roles gain state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Node {
    pub id: String,
    pub role: Role,
    pub parent: Option<String>,
    /// The control this node names, for a label that belongs to a field it is
    /// not the parent of. A test finds the field by reading its label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<String>,
    /// The diagnostic identity this node describes. This does not imply
    /// native tree parentage or a platform described-by relationship.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub describes: Option<String>,
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub bounds: Rect,
    pub visible: bool,
    pub focused: bool,
    pub disabled: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub read_only: bool,
    pub selected: bool,
    pub hovered: bool,
    pub pressed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_min: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_max: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_now: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    #[serde(skip_serializing_if = "is_false")]
    pub busy: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub invalid: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<LiveRegion>,
    #[serde(skip_serializing_if = "is_false")]
    pub live_atomic: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub modal: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub generation: u64,
    pub nodes: Vec<Node>,
}

impl Snapshot {
    pub fn find(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.find(id).is_some()
    }

    pub fn ids(&self) -> Vec<&str> {
        self.nodes.iter().map(|node| node.id.as_str()).collect()
    }

    pub fn under(&self, prefix: &str) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|node| node.id.starts_with(prefix))
            .collect()
    }

    pub fn children_of(&self, parent: &str) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|node| node.parent.as_deref() == Some(parent))
            .collect()
    }

    pub fn descendants_of(&self, parent: &str) -> Vec<&Node> {
        let mut found = Vec::new();
        let mut frontier = vec![parent];
        let mut visited = BTreeSet::from([parent.to_string()]);
        while let Some(next) = frontier.pop() {
            for node in self.children_of(next) {
                if visited.insert(node.id.clone()) {
                    found.push(node);
                    frontier.push(&node.id);
                }
            }
        }
        found
    }

    /// Re-applies redaction.
    ///
    /// The probe already redacts recorded text and values; this covers nodes a
    /// host constructed directly, and is idempotent.
    pub fn redacted(mut self) -> Self {
        for node in &mut self.nodes {
            if let Some(text) = &mut node.text {
                *text = redact_sensitive_text(text);
            }
            if let Some(description) = &mut node.description {
                *description = redact_sensitive_text(description);
            }
            if let Some(value) = &mut node.value {
                *value = redact_sensitive_text(value);
            }
        }
        self
    }
}

#[derive(Clone, Default)]
pub struct SemanticRegistry {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    generation: u64,
    next_sequence: u64,
    order: BTreeMap<String, u64>,
    nodes: BTreeMap<String, (u64, Node)>,
}

impl std::fmt::Debug for SemanticRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SemanticRegistry")
            .field("generation", &self.generation())
            .field("nodes", &self.snapshot().nodes.len())
            .finish()
    }
}

impl SemanticRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens a frame, discarding everything the previous one published.
    ///
    /// Without the discard, a frame that publishes nothing would still report
    /// the previous tree, and a test asserting that an element disappeared
    /// would pass against a stale snapshot.
    pub fn begin_frame(&self) {
        let mut inner = self.lock();
        let generation = inner.generation;
        inner.nodes.retain(|_, (frame, _)| *frame == generation);
        let live: Vec<String> = inner.nodes.keys().cloned().collect();
        inner.order.retain(|id, _| live.contains(id));
        inner.generation = generation + 1;
        inner.next_sequence = 0;
    }

    pub fn generation(&self) -> u64 {
        self.lock().generation
    }

    /// The tree published by the most recent completed frame.
    pub fn snapshot(&self) -> Snapshot {
        let inner = self.lock();
        let published = inner.generation;
        let mut nodes: Vec<(u64, Node)> = inner
            .nodes
            .values()
            .filter(|(frame, _)| *frame == published)
            .map(|(_, node)| {
                (
                    inner.order.get(&node.id).copied().unwrap_or(u64::MAX),
                    node.clone(),
                )
            })
            .collect();
        nodes.sort_by_key(|(sequence, _)| *sequence);
        Snapshot {
            generation: published,
            nodes: nodes.into_iter().map(|(_, node)| node).collect(),
        }
    }

    /// The registry components self-register into.
    ///
    /// Panics when [`install`] has not run, because a missing registry would
    /// otherwise silently produce empty snapshots that tests read as passes.
    pub fn global(cx: &App) -> Self {
        Self::try_global(cx)
            .expect("call gpui_kit_semantics::install(cx) before rendering components")
    }

    pub fn try_global(cx: &App) -> Option<Self> {
        cx.try_global::<GlobalRegistry>()
            .map(|global| global.0.clone())
    }

    fn record(&self, node: Node) {
        let mut inner = self.lock();
        let generation = inner.generation;
        let sequence = inner.next_sequence;
        inner.next_sequence += 1;
        inner.order.insert(node.id.clone(), sequence);
        inner.nodes.insert(node.id.clone(), (generation, node));
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Debug, Clone)]
pub struct NodeSpec {
    id: SharedString,
    role: Role,
    parent: Option<SharedString>,
    labels: Option<SharedString>,
    describes: Option<SharedString>,
    text: Option<SharedString>,
    description: Option<SharedString>,
    focus: Option<FocusHandle>,
    disabled: bool,
    read_only: bool,
    selected: bool,
    hovered: bool,
    pressed: bool,
    checked: Option<bool>,
    expanded: Option<bool>,
    value: Option<SharedString>,
    placeholder: Option<SharedString>,
    range: Option<(f32, f32, f32)>,
    orientation: Option<gpui::accesskit::Orientation>,
    level: Option<u32>,
    busy: bool,
    invalid: bool,
    required: bool,
    live: Option<LiveRegion>,
    live_atomic: bool,
    modal: bool,
}

impl NodeSpec {
    pub fn new(id: impl Into<SharedString>, role: Role) -> Self {
        Self {
            id: id.into(),
            role,
            parent: None,
            labels: None,
            describes: None,
            text: None,
            description: None,
            focus: None,
            disabled: false,
            read_only: false,
            selected: false,
            hovered: false,
            pressed: false,
            checked: None,
            expanded: None,
            value: None,
            placeholder: None,
            range: None,
            orientation: None,
            level: None,
            busy: false,
            invalid: false,
            required: false,
            live: None,
            live_atomic: false,
            modal: false,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    pub fn tristate(mut self, checked: Option<bool>) -> Self {
        self.checked = checked;
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Records the committed value of an editable control.
    ///
    /// Values pass through [`redact_sensitive_text`] before publication so a
    /// snapshot never carries a credential typed by a user.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn range(mut self, min: f32, max: f32, now: f32) -> Self {
        self.range = Some((min, max, now));
        self
    }

    pub fn orientation(mut self, orientation: gpui::accesskit::Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    pub fn level(mut self, level: u32) -> Self {
        self.level = Some(level);
        self
    }

    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn live(mut self, live: LiveRegion) -> Self {
        self.live = Some(live);
        self
    }

    pub fn live_atomic(mut self, atomic: bool) -> Self {
        self.live_atomic = atomic;
        self
    }

    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    pub fn parent(mut self, parent: impl Into<SharedString>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Publishes supplementary literal help on the same native node.
    ///
    /// This maps to AccessKit's description property. It does not claim a
    /// cross-tree described-by relationship.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Names the control this node labels.
    ///
    /// A label is rarely an ancestor of the field it names, so the
    /// association is published rather than inferred from the tree.
    pub fn labels(mut self, control: impl Into<SharedString>) -> Self {
        self.labels = Some(control.into());
        self
    }

    /// Records which semantic node this node describes without claiming a
    /// native relationship or changing actual tree topology.
    pub fn describes(mut self, control: impl Into<SharedString>) -> Self {
        self.describes = Some(control.into());
        self
    }

    pub fn focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }
}

#[derive(Debug, Clone, Default)]
struct GlobalRegistry(SemanticRegistry);

impl Global for GlobalRegistry {}

/// Installs the process-wide registry that components self-register into.
pub fn install(cx: &mut App) {
    if !cx.has_global::<GlobalRegistry>() {
        cx.set_global(GlobalRegistry(SemanticRegistry::new()));
    }
}

pub trait Semantic: Sized {
    type Output;

    fn semantic(self, registry: &SemanticRegistry, spec: NodeSpec) -> Self::Output;

    /// Registers into the global registry when installed. Platform
    /// accessibility remains active when a host opts out of test semantics.
    fn semantic_in(self, cx: &App, spec: NodeSpec) -> Self::Output;
}

impl Semantic for gpui::Div {
    type Output = gpui::Stateful<gpui::Div>;

    fn semantic(self, registry: &SemanticRegistry, spec: NodeSpec) -> Self::Output {
        let mut self_ = self.id(spec.id.clone());
        if self_.style().position.is_none() {
            self_ = self_.relative();
        }
        self_ = platform_accessible(self_, &spec);
        self_.child(diagnostic_probe(Some(registry), spec))
    }

    fn semantic_in(self, cx: &App, spec: NodeSpec) -> Self::Output {
        let mut self_ = self.id(spec.id.clone());
        if self_.style().position.is_none() {
            self_ = self_.relative();
        }
        self_ = platform_accessible(self_, &spec);
        let registry = SemanticRegistry::try_global(cx);
        self_.child(diagnostic_probe(registry.as_ref(), spec))
    }
}

impl Semantic for gpui::Stateful<gpui::Div> {
    type Output = Self;

    fn semantic(mut self, registry: &SemanticRegistry, spec: NodeSpec) -> Self::Output {
        if self.style().position.is_none() {
            self = self.relative();
        }
        self = platform_accessible(self, &spec);
        self.child(diagnostic_probe(Some(registry), spec))
    }

    fn semantic_in(mut self, cx: &App, spec: NodeSpec) -> Self::Output {
        if self.style().position.is_none() {
            self = self.relative();
        }
        self = platform_accessible(self, &spec);
        let registry = SemanticRegistry::try_global(cx);
        self.child(diagnostic_probe(registry.as_ref(), spec))
    }
}

fn platform_accessible<E>(mut element: E, spec: &NodeSpec) -> E
where
    E: StatefulInteractiveElement,
{
    let expected = gpui::ElementId::Name(spec.id.clone());
    let actual = element.interactivity().element_id.as_ref();
    assert_eq!(
        actual,
        Some(&expected),
        "semantic and GPUI element ids must match"
    );
    let Some(role) = platform_role(spec.role) else {
        return element;
    };
    element = element.role(role);
    if let Some(text) = &spec.text {
        element = element.aria_label(redact_sensitive_text(text));
    }
    if let Some(description) = &spec.description {
        element = element.aria_description(redact_sensitive_text(description));
    }
    if let Some(focus) = &spec.focus {
        element = element.track_focus(focus);
    }
    if let Some(expanded) = spec.expanded {
        element = element.aria_expanded(expanded);
    }
    if supports_selection(spec.role) {
        element = element.aria_selected(spec.selected);
    }
    if let Some(checked) = spec.checked {
        element = element.aria_toggled(if checked {
            Toggled::True
        } else {
            Toggled::False
        });
    }
    if let Some(value) = &spec.value {
        element = element.aria_value(redact_sensitive_text(value));
    }
    if let Some(placeholder) = &spec.placeholder {
        element = element.aria_placeholder(placeholder.clone());
    }
    if let Some((min, max, now)) = spec.range {
        element = element
            .aria_min_numeric_value(min.into())
            .aria_max_numeric_value(max.into())
            .aria_numeric_value(now.into());
    }
    if let Some(orientation) = spec.orientation {
        element = element.aria_orientation(orientation);
    }
    if let Some(level) = spec.level {
        element = element.aria_level(level as usize);
    }
    if let Some(live) = spec.live {
        element = element.aria_live(match live {
            LiveRegion::Polite => gpui::accesskit::Live::Polite,
            LiveRegion::Assertive => gpui::accesskit::Live::Assertive,
        });
    }
    element
        .aria_disabled(spec.disabled)
        .aria_read_only(spec.read_only)
        .aria_invalid(spec.invalid)
        .aria_required(spec.required)
        .aria_busy(spec.busy)
        .aria_live_atomic(spec.live_atomic)
        .aria_modal(spec.modal)
}

fn supports_selection(role: Role) -> bool {
    matches!(
        role,
        Role::Row | Role::Tab | Role::Cell | Role::TreeItem | Role::Option
    )
}

fn platform_role(role: Role) -> Option<gpui::Role> {
    Some(match role {
        Role::Window => gpui::Role::Window,
        Role::Region => gpui::Role::Region,
        Role::Group | Role::Field | Role::Drag => gpui::Role::Group,
        Role::List => gpui::Role::List,
        Role::Row => gpui::Role::Row,
        Role::Button => gpui::Role::Button,
        Role::Link => gpui::Role::Link,
        Role::Tab => gpui::Role::Tab,
        Role::TabPanel => gpui::Role::TabPanel,
        Role::Input => gpui::Role::TextInput,
        Role::MultilineInput => gpui::Role::MultilineTextInput,
        Role::PasswordInput => gpui::Role::PasswordInput,
        Role::Text => gpui::Role::Label,
        Role::Heading => gpui::Role::Heading,
        Role::Dialog => gpui::Role::Dialog,
        Role::Menu => gpui::Role::Menu,
        Role::MenuItem => gpui::Role::MenuItem,
        Role::Status | Role::Toast => gpui::Role::Status,
        Role::Checkbox => gpui::Role::CheckBox,
        Role::Radio => gpui::Role::RadioButton,
        Role::Switch => gpui::Role::Switch,
        Role::Slider => gpui::Role::Slider,
        Role::Table => gpui::Role::Table,
        Role::Cell => gpui::Role::Cell,
        Role::Tree => gpui::Role::Tree,
        Role::TreeItem => gpui::Role::TreeItem,
        Role::Progress => gpui::Role::ProgressIndicator,
        Role::Tooltip => gpui::Role::Tooltip,
        Role::Separator => return None,
        Role::Splitter => gpui::Role::Splitter,
        Role::Toolbar => gpui::Role::Toolbar,
        Role::Scrollbar => gpui::Role::ScrollBar,
        Role::Combobox => gpui::Role::ComboBox,
        Role::Option => gpui::Role::ListBoxOption,
        Role::Form => gpui::Role::Form,
        Role::Image => gpui::Role::Image,
    })
}

fn diagnostic_probe(registry: Option<&SemanticRegistry>, spec: NodeSpec) -> impl IntoElement {
    let registry = registry.cloned();
    canvas(
        move |bounds: Bounds<Pixels>, window, _| {
            let Some(registry) = &registry else {
                return;
            };
            let rect = Rect {
                x: f32::from(bounds.origin.x),
                y: f32::from(bounds.origin.y),
                width: f32::from(bounds.size.width),
                height: f32::from(bounds.size.height),
            };
            registry.record(Node {
                id: spec.id.to_string(),
                role: spec.role,
                parent: spec.parent.as_ref().map(ToString::to_string),
                labels: spec.labels.as_ref().map(ToString::to_string),
                describes: spec.describes.as_ref().map(ToString::to_string),
                text: spec.text.as_ref().map(|text| redact_sensitive_text(text)),
                description: spec
                    .description
                    .as_ref()
                    .map(|description| redact_sensitive_text(description)),
                bounds: rect,
                visible: rect.area() > 0.0,
                focused: spec
                    .focus
                    .as_ref()
                    .is_some_and(|handle| handle.is_focused(window)),
                disabled: spec.disabled,
                read_only: spec.read_only,
                selected: spec.selected,
                hovered: spec.hovered,
                pressed: spec.pressed,
                checked: spec.checked,
                expanded: spec.expanded,
                value: spec
                    .value
                    .as_ref()
                    .map(|value| redact_sensitive_text(value)),
                placeholder: spec.placeholder.as_ref().map(ToString::to_string),
                value_min: spec.range.map(|(min, _, _)| min),
                value_max: spec.range.map(|(_, max, _)| max),
                value_now: spec.range.map(|(_, _, now)| now),
                level: spec.level,
                busy: spec.busy,
                invalid: spec.invalid,
                required: spec.required,
                live: spec.live,
                live_atomic: spec.live_atomic,
                modal: spec.modal,
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

pub fn redact_sensitive_text(text: &str) -> String {
    let sensitive_prefixes = ["sk-", "xai-", "ogp_", "Bearer "];
    if sensitive_prefixes
        .iter()
        .any(|prefix| text.contains(prefix))
        || looks_like_jwt(text)
        || looks_like_secret_assignment(text)
    {
        "[REDACTED]".into()
    } else {
        text.into()
    }
}

fn looks_like_jwt(text: &str) -> bool {
    text.split('.').count() == 3 && text.len() >= 32
}

fn looks_like_secret_assignment(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    ["api_key=", "apikey=", "token=", "password=", "secret="]
        .iter()
        .any(|needle| lower.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        AnyWindowHandle, AppContext as _, Context, Render, TestAppContext, Window, div, px,
    };
    use std::cell::Cell;
    use std::rc::Rc;

    fn node(id: &str, parent: Option<&str>) -> Node {
        Node {
            id: id.into(),
            role: Role::Region,
            parent: parent.map(str::to_string),
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            visible: true,
            ..Node::default()
        }
    }

    #[test]
    fn absent_nodes_leave_the_next_frame() {
        let registry = SemanticRegistry::new();
        registry.begin_frame();
        registry.record(node("window", None));
        registry.record(node("old", Some("window")));
        assert!(registry.snapshot().contains("old"));

        registry.begin_frame();
        registry.record(node("window", None));
        assert!(!registry.snapshot().contains("old"));
    }

    #[test]
    fn registration_order_is_stable_and_not_alphabetical() {
        let registry = SemanticRegistry::new();
        registry.begin_frame();
        registry.record(node("z", None));
        registry.record(node("a", None));
        assert_eq!(registry.snapshot().ids(), vec!["z", "a"]);
    }

    #[test]
    fn descendants_follow_the_declared_parent_chain() {
        let snapshot = Snapshot {
            generation: 1,
            nodes: vec![
                node("root", None),
                node("child", Some("root")),
                node("grandchild", Some("child")),
            ],
        };
        assert_eq!(
            snapshot
                .descendants_of("root")
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["child", "grandchild"]
        );
    }

    #[test]
    fn descendants_do_not_loop_on_a_malformed_parent_cycle() {
        let snapshot = Snapshot {
            generation: 1,
            nodes: vec![node("a", Some("b")), node("b", Some("a"))],
        };
        assert_eq!(
            snapshot
                .descendants_of("a")
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["b"]
        );
    }

    #[test]
    fn adjacent_bounds_do_not_overlap() {
        let left = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let right = Rect { x: 100.0, ..left };
        assert!(!left.overlaps(right));
    }

    #[test]
    fn exported_text_redacts_credential_shapes() {
        for secret in [
            "sk-secret-value",
            "Bearer credential",
            "api_key=hunter2",
            "eyJaaaaaaaaaa.bbbbbbbbbbbb.cccccccccccc",
        ] {
            assert_eq!(redact_sensitive_text(secret), "[REDACTED]");
        }
        assert_eq!(redact_sensitive_text("Token usage"), "Token usage");
    }

    #[test]
    fn a_frame_that_publishes_nothing_reports_an_empty_tree() {
        let registry = SemanticRegistry::new();
        registry.begin_frame();
        registry.record(node("toast", None));
        assert_eq!(registry.snapshot().ids(), vec!["toast"]);

        registry.begin_frame();
        assert!(
            registry.snapshot().nodes.is_empty(),
            "a removed element must not linger in the next frame"
        );
    }

    #[test]
    fn a_node_that_stops_rendering_leaves_the_snapshot() {
        let registry = SemanticRegistry::new();
        registry.begin_frame();
        registry.record(node("row.a", None));
        registry.record(node("row.b", None));

        registry.begin_frame();
        registry.record(node("row.a", None));
        assert_eq!(registry.snapshot().ids(), vec!["row.a"]);
    }

    #[test]
    fn snapshots_are_serializable_protocol_data() {
        let snapshot = Snapshot {
            generation: 4,
            nodes: vec![node("window", None)],
        };
        let encoded = serde_json::to_string(&snapshot).expect("serialize");
        let decoded: Snapshot = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn unused_state_fields_stay_out_of_serialized_snapshots() {
        let encoded = serde_json::to_string(&node("window", None)).expect("serialize");
        for absent in ["checked", "expanded", "value", "busy", "invalid", "level"] {
            assert!(!encoded.contains(absent), "{absent} must not be emitted");
        }
    }

    #[test]
    fn older_snapshots_still_deserialize() {
        let legacy = r#"{
            "generation": 2,
            "nodes": [{
                "id": "run", "role": "button", "parent": null, "text": "Run",
                "bounds": {"x": 0, "y": 0, "width": 10, "height": 10},
                "visible": true, "focused": false, "disabled": false,
                "selected": false, "hovered": false, "pressed": false
            }]
        }"#;
        let snapshot: Snapshot = serde_json::from_str(legacy).expect("legacy snapshot");
        let node = snapshot.find("run").expect("node");
        assert_eq!(node.checked, None);
        assert!(!node.busy);
    }

    #[test]
    fn recorded_values_are_redacted_like_text() {
        assert_eq!(redact_sensitive_text("sk-live-value"), "[REDACTED]");
    }

    #[test]
    fn host_constructed_snapshot_text_descriptions_and_values_are_redacted() {
        let mut exposed = node("credential", None);
        exposed.text = Some("Bearer text-secret".into());
        exposed.description = Some("sk-description-secret".into());
        exposed.value = Some("xai-value-secret".into());

        let snapshot = Snapshot {
            generation: 1,
            nodes: vec![exposed],
        }
        .redacted();
        let protected = snapshot.find("credential").expect("protected node");
        assert_eq!(protected.text.as_deref(), Some("[REDACTED]"));
        assert_eq!(protected.description.as_deref(), Some("[REDACTED]"));
        assert_eq!(protected.value.as_deref(), Some("[REDACTED]"));
    }

    struct PlatformTreeFixture {
        focus: FocusHandle,
        clicks: Rc<Cell<usize>>,
    }

    impl Render for PlatformTreeFixture {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .child(
                    div().w(px(160.0)).h(px(24.0)).semantic_in(
                        cx,
                        NodeSpec::new("volume", Role::Slider)
                            .text("Volume")
                            .value("40 percent")
                            .range(0.0, 100.0, 40.0)
                            .focus(&self.focus)
                            .disabled(true)
                            .invalid(true)
                            .required(true)
                            .busy(true),
                    ),
                )
                .child(
                    div()
                        .child({
                            let clicks = self.clicks.clone();
                            div()
                                .id("choice")
                                .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
                                .w(px(100.0))
                                .h(px(24.0))
                                .semantic_in(
                                    cx,
                                    NodeSpec::new("choice", Role::Checkbox)
                                        .text("Use system setting")
                                        .checked(true),
                                )
                        })
                        .semantic_in(cx, NodeSpec::new("settings", Role::Group).text("Settings")),
                )
                .child(
                    div().w(px(100.0)).h(px(24.0)).semantic_in(
                        cx,
                        NodeSpec::new("quality", Role::Option)
                            .text("High quality")
                            .selected(true),
                    ),
                )
        }
    }

    #[gpui::test]
    fn semantics_reach_the_deterministic_platform_tree(cx: &mut TestAppContext) {
        let clicks = Rc::new(Cell::new(0));
        let fixture_clicks = clicks.clone();
        let window = cx.add_window(|window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            PlatformTreeFixture {
                focus,
                clicks: fixture_clicks,
            }
        });
        let window = AnyWindowHandle::from(window);

        cx.activate_accessibility(window);
        let json = cx
            .update_window(window, |_, window, cx| {
                window.draw(cx).clear(cx);
                window
                    .debug_a11y_tree_json()
                    .expect("active accessibility tree")
            })
            .expect("test window");
        let tree: serde_json::Value = serde_json::from_str(&json).expect("valid tree JSON");
        let (node_id, node) = tree["nodes"]
            .as_object()
            .and_then(|nodes| {
                nodes.iter().find(|(_, node)| {
                    node["element_id"]
                        .as_str()
                        .is_some_and(|id| id.contains("volume"))
                })
            })
            .unwrap_or_else(|| panic!("semantic node missing from AccessKit tree: {json}"));

        assert_eq!(tree["gpui_focus"], node_id.as_str());
        assert_eq!(node["aria"]["role"], "Slider");
        assert_eq!(node["aria"]["label"], "Volume");
        assert_eq!(node["aria"]["value"], "40 percent");
        assert_eq!(node["aria"]["numeric_value"], 40.0);
        assert_eq!(node["aria"]["min_numeric_value"], 0.0);
        assert_eq!(node["aria"]["max_numeric_value"], 100.0);
        assert_eq!(node["aria"]["disabled"], true);
        assert_eq!(node["aria"]["invalid"], "True");
        assert_eq!(node["aria"]["required"], true);
        assert_eq!(node["aria"]["busy"], true);

        let nodes = tree["nodes"].as_object().expect("nodes object");
        let choice = nodes
            .iter()
            .find(|(_, node)| node["aria"]["label"] == "Use system setting")
            .expect("stateful checkbox node");
        assert_eq!(choice.1["aria"]["role"], "CheckBox");
        assert_eq!(choice.1["aria"]["toggled"], "True");
        assert!(
            choice.1["aria"]["on_action"]
                .as_array()
                .is_some_and(|actions| actions.iter().any(|action| action == "Click"))
        );
        let settings = nodes
            .iter()
            .find(|(_, node)| node["aria"]["label"] == "Settings")
            .expect("plain semantic parent is a platform node");
        assert_eq!(settings.1["children"][0], choice.0.as_str());

        cx.dispatch_accessibility_action(
            window,
            gpui::accesskit::ActionRequest {
                action: gpui::accesskit::Action::Click,
                target_tree: gpui::accesskit::TreeId::ROOT,
                target_node: gpui::accesskit::NodeId(
                    choice.1["accesskit_id"]
                        .as_str()
                        .expect("raw AccessKit node id")
                        .parse()
                        .expect("numeric node id"),
                ),
                data: None,
            },
        );
        assert_eq!(clicks.get(), 1);

        let option = nodes
            .values()
            .find(|node| node["aria"]["label"] == "High quality")
            .expect("selected option node");
        assert_eq!(option["aria"]["role"], "ListBoxOption");
        assert_eq!(option["aria"]["selected"], true);
    }

    #[test]
    #[should_panic(expected = "semantic and GPUI element ids must match")]
    fn stateful_semantics_reject_mismatched_identity() {
        let element = div().id("actual");
        let _ = platform_accessible(element, &NodeSpec::new("claimed", Role::Button));
    }

    #[test]
    fn static_status_is_not_a_live_region_without_explicit_ownership() {
        assert_eq!(NodeSpec::new("count", Role::Status).live, None);
        assert_eq!(NodeSpec::new("toast", Role::Toast).live, None);
        assert_eq!(
            NodeSpec::new("announcement", Role::Status)
                .live(LiveRegion::Polite)
                .live,
            Some(LiveRegion::Polite)
        );
    }
}
