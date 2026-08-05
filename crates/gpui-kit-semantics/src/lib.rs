//! A per-frame semantic tree for native GPUI applications.
//!
//! Native windows have no DOM. Views attach a zero-paint probe to meaningful
//! elements; prepaint records the bounds GPUI actually produced. Nodes absent
//! from the next frame disappear instead of lingering as stale claims.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};

use gpui::{Bounds, FocusHandle, IntoElement, ParentElement, Pixels, SharedString, Styled, canvas};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    Window,
    Region,
    List,
    Row,
    Button,
    Tab,
    Input,
    Text,
    Dialog,
    Menu,
    MenuItem,
    Status,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub role: Role,
    pub parent: Option<String>,
    pub text: Option<String>,
    pub bounds: Rect,
    pub visible: bool,
    pub focused: bool,
    pub disabled: bool,
    pub selected: bool,
    pub hovered: bool,
    pub pressed: bool,
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

    pub fn begin_frame(&self) {
        let mut inner = self.lock();
        inner.generation += 1;
        inner.next_sequence = 0;
    }

    pub fn generation(&self) -> u64 {
        self.lock().generation
    }

    pub fn snapshot(&self) -> Snapshot {
        let inner = self.lock();
        let Some(published) = inner.nodes.values().map(|(frame, _)| *frame).max() else {
            return Snapshot {
                generation: inner.generation,
                nodes: Vec::new(),
            };
        };
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
    text: Option<SharedString>,
    focus: Option<FocusHandle>,
    disabled: bool,
    selected: bool,
    hovered: bool,
    pressed: bool,
}

impl NodeSpec {
    pub fn new(id: impl Into<SharedString>, role: Role) -> Self {
        Self {
            id: id.into(),
            role,
            parent: None,
            text: None,
            focus: None,
            disabled: false,
            selected: false,
            hovered: false,
            pressed: false,
        }
    }

    pub fn parent(mut self, parent: impl Into<SharedString>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
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

pub trait Semantic: Styled + ParentElement + Sized {
    fn semantic(self, registry: &SemanticRegistry, spec: NodeSpec) -> Self {
        self.relative().child(probe(registry, spec))
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
                text: spec.text.as_ref().map(ToString::to_string),
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
            text: None,
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            visible: true,
            focused: false,
            disabled: false,
            selected: false,
            hovered: false,
            pressed: false,
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
    fn snapshots_are_serializable_protocol_data() {
        let snapshot = Snapshot {
            generation: 4,
            nodes: vec![node("window", None)],
        };
        let encoded = serde_json::to_string(&snapshot).expect("serialize");
        let decoded: Snapshot = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, snapshot);
    }
}
