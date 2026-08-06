//! A per-frame semantic tree for native GPUI applications.
//!
//! Native windows have no DOM. Views attach a zero-paint probe to meaningful
//! elements; prepaint records the bounds GPUI actually produced. Nodes absent
//! from the next frame disappear instead of lingering as stale claims.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};

use gpui::{
    App, Bounds, FocusHandle, Global, IntoElement, ParentElement, Pixels, SharedString, Styled,
    canvas,
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
    pub text: Option<String>,
    pub bounds: Rect,
    pub visible: bool,
    pub focused: bool,
    pub disabled: bool,
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
    text: Option<SharedString>,
    focus: Option<FocusHandle>,
    disabled: bool,
    selected: bool,
    hovered: bool,
    pressed: bool,
    checked: Option<bool>,
    expanded: Option<bool>,
    value: Option<SharedString>,
    placeholder: Option<SharedString>,
    range: Option<(f32, f32, f32)>,
    level: Option<u32>,
    busy: bool,
    invalid: bool,
    required: bool,
}

impl NodeSpec {
    pub fn new(id: impl Into<SharedString>, role: Role) -> Self {
        Self {
            id: id.into(),
            role,
            parent: None,
            labels: None,
            text: None,
            focus: None,
            disabled: false,
            selected: false,
            hovered: false,
            pressed: false,
            checked: None,
            expanded: None,
            value: None,
            placeholder: None,
            range: None,
            level: None,
            busy: false,
            invalid: false,
            required: false,
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

    pub fn parent(mut self, parent: impl Into<SharedString>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
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

    pub fn focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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

pub trait Semantic: Styled + ParentElement + Sized {
    fn semantic(self, registry: &SemanticRegistry, spec: NodeSpec) -> Self {
        self.relative().child(probe(registry, spec))
    }

    /// Registers into the global registry, or does nothing when a host has not
    /// installed one. Hosts that opt out get no semantic tree, not a panic.
    fn semantic_in(self, cx: &App, spec: NodeSpec) -> Self {
        match SemanticRegistry::try_global(cx) {
            Some(registry) => self.semantic(&registry, spec),
            None => self,
        }
    }
}

impl Semantic for gpui::Div {}
impl Semantic for gpui::Stateful<gpui::Div> {}

fn probe(registry: &SemanticRegistry, spec: NodeSpec) -> impl IntoElement {
    let registry = registry.clone();
    canvas(
        move |bounds: Bounds<Pixels>, window, _| {
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
                text: spec.text.as_ref().map(|text| redact_sensitive_text(text)),
                bounds: rect,
                visible: rect.area() > 0.0,
                focused: spec
                    .focus
                    .as_ref()
                    .is_some_and(|handle| handle.is_focused(window)),
                disabled: spec.disabled,
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
}
