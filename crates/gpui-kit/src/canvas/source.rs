//! Caller-owned canonical graph metadata and reusable lazy content.
//!
//! Cloned handles share one identity and revision. Mutation does not notify a
//! window: the host publishes its accepted change and requests its own redraw.

use super::{GraphEdge, Placed};
use gpui::{AnyElement, App, IntoElement, RenderOnce, SharedString, Window};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    rc::Rc,
};

type Factory = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

#[derive(Default)]
struct Factories {
    content: Option<Factory>,
    thumbnail: Option<Factory>,
}

/// Rejected source mutation. Validation is atomic: neither metadata, factory
/// ownership, ordering nor revision changes on error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphSourceError {
    DuplicateNode(SharedString),
    DuplicateEdge(SharedString),
    DuplicatePort {
        node: SharedString,
        port: SharedString,
    },
    MissingNode(SharedString),
    InvalidGeometry(SharedString),
    MissingHeight(SharedString),
    OpaqueContent(SharedString),
}

impl std::fmt::Display for GraphSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for GraphSourceError {}

#[derive(Default)]
pub(super) struct SourceData {
    // Ordinals preserve insertion order without shifting every node on removal.
    nodes: BTreeMap<u64, Placed>,
    ordinals: HashMap<SharedString, u64>,
    factories: HashMap<SharedString, Factories>,
    pub(super) edges: Vec<GraphEdge>,
    next: u64,
    revision: u64,
    node_revisions: HashMap<u64, u64>,
}

impl SourceData {
    pub(super) fn node_revision(&self, id: &SharedString) -> Option<(u64, u64)> {
        let ordinal = *self.ordinals.get(id)?;
        Some((
            ordinal,
            self.node_revisions.get(&ordinal).copied().unwrap_or(0),
        ))
    }
}

/// Reusable graph data. `GraphNode` remains the metadata authority; explicit
/// finite sizes make offscreen nodes routable without mounting their content.
/// One-use children/thumbnails are rejected; use lazy factory setters instead.
/// Edges may name absent nodes and remain caller-owned through node removal.
#[derive(Clone, Default)]
pub struct GraphSource {
    pub(super) data: Rc<RefCell<SourceData>>,
}

impl std::fmt::Debug for GraphSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = self.data.borrow();
        formatter
            .debug_struct("GraphSource")
            .field("nodes", &data.nodes.len())
            .field("edges", &data.edges.len())
            .field("revision", &data.revision)
            .finish()
    }
}

fn validate(placed: &Placed) -> Result<SharedString, GraphSourceError> {
    let id = placed.node.ident().semantic_id();
    let height = placed
        .height
        .ok_or_else(|| GraphSourceError::MissingHeight(id.clone()))?;
    let width = placed.node.node_width();
    if id.is_empty()
        || ![
            placed.x,
            placed.y,
            width,
            height,
            placed.x + width,
            placed.y + height,
        ]
        .into_iter()
        .all(f32::is_finite)
        || width <= 0.
        || height <= 0.
    {
        return Err(GraphSourceError::InvalidGeometry(id));
    }
    if !placed.node.reusable() {
        return Err(GraphSourceError::OpaqueContent(id));
    }
    let mut ports = HashSet::new();
    for port in placed.node.graph_ports() {
        if !ports.insert(port.id()) {
            return Err(GraphSourceError::DuplicatePort {
                node: id,
                port: port.id().clone(),
            });
        }
    }
    Ok(id)
}

fn validate_edges(edges: &[GraphEdge]) -> Result<(), GraphSourceError> {
    let mut ids = HashSet::new();
    for edge in edges {
        let id = edge.edge_id();
        if !ids.insert(id.clone()) {
            return Err(GraphSourceError::DuplicateEdge(id));
        }
    }
    Ok(())
}

impl GraphSource {
    pub fn new(
        nodes: impl IntoIterator<Item = Placed>,
        edges: impl IntoIterator<Item = GraphEdge>,
    ) -> Result<Self, GraphSourceError> {
        let mut data = SourceData::default();
        for placed in nodes {
            let id = validate(&placed)?;
            if data.ordinals.contains_key(&id) {
                return Err(GraphSourceError::DuplicateNode(id));
            }
            data.ordinals.insert(id, data.next);
            data.nodes.insert(data.next, placed);
            data.next += 1;
        }
        data.edges = edges.into_iter().collect();
        validate_edges(&data.edges)?;
        Ok(Self {
            data: Rc::new(RefCell::new(data)),
        })
    }

    /// Monotonic accepted-mutation revision shared by every cloned handle.
    pub fn revision(&self) -> u64 {
        self.data.borrow().revision
    }

    pub fn len(&self) -> usize {
        self.data.borrow().nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.borrow().nodes.is_empty()
    }

    /// Identity is stable across mutations, and distinct from equal data.
    pub fn same_source(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.data, &other.data)
    }

    /// Replaces canonical metadata while preserving existing factory ownership
    /// and insertion order. New identities append. Failed validation is atomic.
    pub fn upsert(&self, placed: Placed) -> Result<(), GraphSourceError> {
        let id = validate(&placed)?;
        let mut data = self.data.borrow_mut();
        let ordinal = if let Some(ordinal) = data.ordinals.get(&id) {
            *ordinal
        } else {
            let ordinal = data.next;
            data.next += 1;
            data.ordinals.insert(id, ordinal);
            ordinal
        };
        data.nodes.insert(ordinal, placed);
        data.revision += 1;
        let revision = data.revision;
        data.node_revisions.insert(ordinal, revision);
        Ok(())
    }

    /// Removes this identity and its factories, not the caller's edges. A
    /// reinserted identity is new content at the end of the source order.
    pub fn remove(&self, id: &str) -> bool {
        let mut data = self.data.borrow_mut();
        let Some(ordinal) = data.ordinals.remove(id) else {
            return false;
        };
        data.nodes.remove(&ordinal);
        data.node_revisions.remove(&ordinal);
        data.factories.remove(id);
        data.revision += 1;
        true
    }

    pub fn replace_edges(
        &self,
        edges: impl IntoIterator<Item = GraphEdge>,
    ) -> Result<(), GraphSourceError> {
        let edges: Vec<_> = edges.into_iter().collect();
        validate_edges(&edges)?;
        let mut data = self.data.borrow_mut();
        data.edges = edges;
        data.revision += 1;
        Ok(())
    }

    /// Runs only when this card mounts. Captured host data remains caller-owned;
    /// replace the factory after changing its data to publish a new revision.
    pub fn set_content(
        &self,
        id: &str,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Result<(), GraphSourceError> {
        self.factory(id, Rc::new(build), false)
    }

    /// Lazy thumbnail with the canonical node's thumbnail aspect ratio.
    pub fn set_thumbnail(
        &self,
        id: &str,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Result<(), GraphSourceError> {
        self.factory(id, Rc::new(build), true)
    }

    fn factory(&self, id: &str, build: Factory, thumbnail: bool) -> Result<(), GraphSourceError> {
        let mut data = self.data.borrow_mut();
        if !data.ordinals.contains_key(id) {
            return Err(GraphSourceError::MissingNode(id.into()));
        }
        let factories = data.factories.entry(id.into()).or_default();
        if thumbnail {
            factories.thumbnail = Some(build);
        } else {
            factories.content = Some(build);
        }
        data.revision += 1;
        Ok(())
    }
}

#[derive(IntoElement)]
struct LazyContent(Factory);
impl RenderOnce for LazyContent {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        (self.0)(window, cx)
    }
}

pub(super) enum NodeItems<'a> {
    Owned(Vec<Placed>),
    Source(&'a SourceData),
}

impl NodeItems<'_> {
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Owned(nodes) => nodes.len(),
            Self::Source(data) => data.nodes.len(),
        }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &Placed> + Clone {
        let (owned, source) = match self {
            Self::Owned(nodes) => (Some(nodes), None),
            Self::Source(data) => (None, Some(&data.nodes)),
        };
        owned
            .into_iter()
            .flatten()
            .chain(source.into_iter().flat_map(|nodes| nodes.values()))
    }

    pub(super) fn visible(self, visible: &HashSet<SharedString>) -> Vec<Placed> {
        match self {
            Self::Owned(nodes) => nodes
                .into_iter()
                .filter(|node| visible.contains(&node.node.ident().semantic_id()))
                .collect(),
            Self::Source(data) => {
                // Geometry already established valid visible business identities.
                // Resolve those identities directly; never walk all source cards
                // again just to mount a viewport-sized subset. Ordinals preserve
                // painter order, including remove/reinsert append semantics.
                let mut ordinals: Vec<_> = visible
                    .iter()
                    .filter_map(|id| data.ordinals.get(id))
                    .copied()
                    .collect();
                ordinals.sort_unstable();
                ordinals
                    .into_iter()
                    .filter_map(|ordinal| data.nodes.get(&ordinal))
                    .map(|placed| {
                        let mut node = placed.node.clone_metadata();
                        if let Some(factories) = data.factories.get(&node.ident().semantic_id()) {
                            if let Some(factory) = &factories.content {
                                node = node.child(LazyContent(factory.clone()));
                            }
                            if let Some(factory) = &factories.thumbnail {
                                node = node.thumbnail(LazyContent(factory.clone()));
                            }
                        }
                        Placed {
                            node,
                            x: placed.x,
                            y: placed.y,
                            height: placed.height,
                        }
                    })
                    .collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{GraphNode, GraphPort, NodeState};
    use gpui::div;

    #[test]
    fn visible_lookup_preserves_paint_order_and_current_metadata_after_reinsert() {
        let source = GraphSource::new(
            (0..10_000).map(|i| placed(&format!("node-{i}"), i as f32)),
            [],
        )
        .expect("source");
        let selected: HashSet<_> = ["node-9981", "node-73", "absent", "node-301"]
            .into_iter()
            .map(SharedString::from)
            .collect();
        let read = || {
            NodeItems::Source(&source.data.borrow())
                .visible(&selected)
                .into_iter()
                .map(|node| (node.node.ident().semantic_id(), node.x))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            read(),
            vec![
                ("node-73".into(), 73.),
                ("node-301".into(), 301.),
                ("node-9981".into(), 9981.)
            ]
        );
        source.upsert(placed("node-301", -19.)).expect("update");
        assert!(source.remove("node-73"));
        source.upsert(placed("node-73", -117.)).expect("reinsert");
        assert_eq!(
            read(),
            vec![
                ("node-301".into(), -19.),
                ("node-9981".into(), 9981.),
                ("node-73".into(), -117.)
            ]
        );
        assert!(
            NodeItems::Source(&source.data.borrow())
                .visible(&HashSet::new())
                .is_empty()
        );
    }

    fn placed(id: &str, x: f32) -> Placed {
        Placed::new(
            GraphNode::new(id.to_owned(), "Canonical").width(113.),
            x,
            17.,
        )
        .height(79.)
    }

    #[test]
    fn source_handles_share_atomic_revisions_and_keep_survivor_order() {
        let source = GraphSource::new(
            [placed("b", -11.), placed("a", 130.), placed("z", 270.)],
            [],
        )
        .expect("valid source");
        let alias = source.clone();
        assert!(source.same_source(&alias));
        assert!(!source.same_source(&GraphSource::default()));
        let lifetime = Rc::new(());
        let retained = lifetime.clone();
        source
            .set_content("a", move |_, _| {
                let _ = &retained;
                div().into_any_element()
            })
            .expect("known id");
        let revision = source.revision();
        for invalid in [
            Placed::new(GraphNode::new("a", "Bad").width(f32::NAN), 9., 10.).height(73.),
            Placed::new(GraphNode::new("a", "Bad").child(div()), 9., 10.).height(73.),
            Placed::new(
                GraphNode::new("a", "Bad")
                    .port(GraphPort::input("in", "One"))
                    .port(GraphPort::input("in", "Two")),
                9.,
                10.,
            )
            .height(73.),
            Placed::new(GraphNode::new("new", "Bad"), 9., 10.),
        ] {
            assert!(alias.upsert(invalid).is_err());
            assert_eq!(source.revision(), revision);
            let data = source.data.borrow();
            assert_eq!(data.nodes.len(), 3);
            assert_eq!(data.next, 3);
            let original = &data.nodes[&data.ordinals["a"]];
            assert_eq!(
                (
                    original.x,
                    original.y,
                    original.node.node_width(),
                    original.height
                ),
                (130., 17., 113., Some(79.))
            );
            assert_eq!(Rc::strong_count(&lifetime), 2);
        }
        alias
            .upsert(
                Placed::new(
                    GraphNode::new("a", "Changed")
                        .width(191.)
                        .state(NodeState::Failed),
                    -83.,
                    29.,
                )
                .height(117.),
            )
            .expect("valid replacement");
        assert_eq!(source.revision(), revision + 1);
        assert_eq!(
            source.data.borrow().nodes[&1].node.node_state(),
            NodeState::Failed
        );
        assert!(alias.remove("a"));
        assert_eq!(Rc::strong_count(&lifetime), 1);
        assert!(!alias.remove("a"));
        alias.upsert(placed("a", 17.)).expect("reinsert");
        let data = source.data.borrow();
        let ids: Vec<_> = NodeItems::Source(&data)
            .iter()
            .map(|p| p.node.ident().semantic_id())
            .collect();
        assert_eq!(ids, ["b", "z", "a"]);
        assert!(!data.factories.contains_key("a"));
    }

    #[test]
    fn edge_replacement_is_atomic_and_node_removal_does_not_rewrite_topology() {
        let edge = GraphEdge::new("a", "missing").id("wire");
        let source =
            GraphSource::new([placed("a", 0.)], [edge.clone()]).expect("dangling edge retained");
        assert!(source.replace_edges([edge.clone(), edge.clone()]).is_err());
        assert_eq!(source.revision(), 0);
        assert_eq!(
            source.data.borrow().edges.as_slice(),
            std::slice::from_ref(&edge)
        );
        assert!(source.remove("a"));
        assert_eq!(source.data.borrow().edges, [edge]);
        source.replace_edges([]).expect("empty edge set");
        assert!(source.data.borrow().edges.is_empty());
        assert!(matches!(
            GraphSource::new([placed("same", 0.), placed("same", 300.)], []),
            Err(GraphSourceError::DuplicateNode(_))
        ));
    }
}
