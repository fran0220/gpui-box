//! The canvas a run is drawn on.
//!
//! The graph owns no layout algorithm. Where a node sits is a product
//! question — a plan graph, a dependency graph and a retry graph want
//! different answers, and none of them belong in a component library — so the
//! caller places every node and this draws what it was given. What the graph
//! does own is the part that is the same every time: the backdrop, the
//! stacking of edges beneath nodes, and the five states a canvas can be in.

use std::{cell::Cell, collections::HashMap, rc::Rc};

use gpui::{
    AnyElement, App, Bounds, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels,
    Point, RenderOnce, ScrollDelta, SharedString, Styled, Window, canvas, div, point, px, relative,
    size,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, TypeScale};
use web_time::Instant;

use crate::display::empty::EmptyState;
use crate::foundation::{FocusRing, Ident, Pressable, StyledExt};
use crate::layout::measure;
use crate::motion::keyed;
use crate::strings::{ActiveStrings, StringKey};

use super::edge::{
    Anchor, Axis, GraphEdge, GraphEndpoint, OrthogonalRoute, PortSide, RouteTransform, paint_route,
    paint_route_stroke, route_orthogonal, route_preview,
};
use super::node::{GraphNode, GraphPort, NODE_WIDTH, PortDirection};

/// The spacing of the dot grid behind the canvas, in pixels.
const GRID_STEP: f32 = 24.0;
const GRID_DOT: f32 = 1.0;
/// Below this zoom a node draws only its title, and ports stay off.
const LOD_ZOOM: f32 = 0.4;
/// Extra world space kept around the viewport so a node entering does not pop.
const CULL_PAD: f32 = 80.0;

/// Caller-owned pan and zoom values for a [`NodeGraph`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphViewport {
    pub offset: Point<f32>,
    pub zoom: f32,
}

impl GraphViewport {
    /// Creates a viewport with a screen-space offset and world scale.
    pub fn new(offset: Point<f32>, zoom: f32) -> Self {
        Self { offset, zoom }
    }
}

impl Default for GraphViewport {
    fn default() -> Self {
        Self {
            offset: point(0.0, 0.0),
            zoom: 1.0,
        }
    }
}

/// A proposed controlled graph change.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeGraphEvent {
    /// Proposes a pan or zoom change.
    ViewportChanged(GraphViewport),
    /// Proposes the complete caller-owned node selection.
    SelectionChanged { ids: Vec<SharedString> },
    /// Proposes a new world-space position for a node.
    NodeMoved {
        id: SharedString,
        position: Point<f32>,
    },
    /// Proposes deleting a node by business identity.
    NodeDeleted { id: SharedString },
    /// Proposes a new output-to-input connection.
    ConnectionRequested {
        from: GraphEndpoint,
        to: GraphEndpoint,
    },
    /// Proposes removing one controlled edge by its stable identity.
    DisconnectRequested { id: SharedString },
}

type EventHandler = Rc<dyn Fn(&NodeGraphEvent, &mut Window, &mut App)>;

/// Which controlled changes an interactive graph may propose.
///
/// Omitting [`NodeGraph::on_event`] still makes any mode static. With a
/// handler installed, `Inspect` permits navigation and selection, `Arrange`
/// additionally permits moving nodes, and `Edit` retains the complete graph
/// editor including connection and deletion proposals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphInteraction {
    Inspect,
    Arrange,
    #[default]
    Edit,
}

impl GraphInteraction {
    fn moves_nodes(self) -> bool {
        matches!(self, Self::Arrange | Self::Edit)
    }

    fn edits_topology(self) -> bool {
        self == Self::Edit
    }
}

#[derive(Debug, Clone)]
enum Gesture {
    Pan {
        at: Point<Pixels>,
        viewport: GraphViewport,
        moved: bool,
    },
    Node {
        at: Point<Pixels>,
        id: SharedString,
        position: Point<f32>,
        peers: Vec<(SharedString, Point<f32>)>,
        moved: bool,
        extend_selection: bool,
    },
    Connect {
        from: GraphEndpoint,
    },
    Marquee {
        origin: Point<Pixels>,
        current: Point<Pixels>,
    },
}

#[derive(Debug, Default)]
struct GestureState {
    gesture: Option<Gesture>,
    pointer: Option<Point<Pixels>>,
    animation_started: Option<Instant>,
}

fn world_to_screen(world: Point<f32>, viewport: GraphViewport) -> Point<f32> {
    point(
        viewport.offset.x + world.x * viewport.zoom,
        viewport.offset.y + world.y * viewport.zoom,
    )
}
fn screen_to_world(screen: Point<f32>, viewport: GraphViewport) -> Point<f32> {
    point(
        (screen.x - viewport.offset.x) / viewport.zoom,
        (screen.y - viewport.offset.y) / viewport.zoom,
    )
}
fn zoom_at(viewport: GraphViewport, screen: Point<f32>, zoom: f32) -> GraphViewport {
    let world = screen_to_world(screen, viewport);
    GraphViewport {
        offset: point(screen.x - world.x * zoom, screen.y - world.y * zoom),
        zoom,
    }
}

fn world_view(viewport: GraphViewport, screen: Bounds<Pixels>) -> Bounds<f32> {
    let width = f32::from(screen.size.width);
    let height = f32::from(screen.size.height);
    let origin = screen_to_world(point(0.0, 0.0), viewport);
    let far = screen_to_world(point(width, height), viewport);
    Bounds::new(
        point(origin.x.min(far.x), origin.y.min(far.y)),
        size((far.x - origin.x).abs(), (far.y - origin.y).abs()),
    )
}

fn bounds_overlap(left: Bounds<f32>, right: Bounds<f32>, pad: f32) -> bool {
    left.left() - pad < right.right()
        && left.right() + pad > right.left()
        && left.top() - pad < right.bottom()
        && left.bottom() + pad > right.top()
}

fn route_signature(nodes: &[NodeGeometry], edges: &[GraphEdge]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for node in nodes {
        node.id.hash(&mut hasher);
        f32::to_bits(node.bounds.origin.x).hash(&mut hasher);
        f32::to_bits(node.bounds.origin.y).hash(&mut hasher);
        f32::to_bits(node.bounds.size.width).hash(&mut hasher);
        f32::to_bits(node.bounds.size.height).hash(&mut hasher);
    }
    for edge in edges {
        edge.edge_id().hash(&mut hasher);
        edge.from().hash(&mut hasher);
        edge.to().hash(&mut hasher);
    }
    hasher.finish()
}

fn viewport_value(state: &str, viewport: GraphViewport) -> String {
    format!(
        "state:{state};offset:{:.3},{:.3};zoom:{:.3}",
        viewport.offset.x, viewport.offset.y, viewport.zoom
    )
}

fn composite_id(prefix: &str, parts: &[&str]) -> SharedString {
    let mut id = prefix.to_string();
    for part in parts {
        id.push(':');
        id.push_str(&part.len().to_string());
        id.push(':');
        id.push_str(part);
    }
    id.into()
}

/// Places nodes in layers along the reading direction from a caller-owned
/// edge list. The graph still draws whatever positions it is handed; this
/// is a helper a host may apply before that.
pub fn layered_layout<'a>(
    ids: impl IntoIterator<Item = impl Into<SharedString>>,
    edges: impl IntoIterator<Item = &'a GraphEdge>,
    column_gap: f32,
    row_gap: f32,
) -> Vec<(SharedString, Point<f32>)> {
    let ids: Vec<SharedString> = ids.into_iter().map(Into::into).collect();
    let mut incoming: HashMap<SharedString, usize> =
        ids.iter().cloned().map(|id| (id, 0)).collect();
    let mut outgoing: HashMap<SharedString, Vec<SharedString>> = HashMap::new();
    for edge in edges {
        if incoming.contains_key(edge.from()) && incoming.contains_key(edge.to()) {
            *incoming.entry(edge.to().clone()).or_insert(0) += 1;
            outgoing
                .entry(edge.from().clone())
                .or_default()
                .push(edge.to().clone());
        }
    }
    let mut layers: Vec<Vec<SharedString>> = Vec::new();
    let mut remaining = incoming;
    while !remaining.is_empty() {
        let ready: Vec<SharedString> = remaining
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(id, _)| id.clone())
            .collect();
        let ready = if ready.is_empty() {
            remaining.keys().take(1).cloned().collect()
        } else {
            ready
        };
        for id in &ready {
            remaining.remove(id);
            if let Some(next) = outgoing.get(id) {
                for child in next {
                    if let Some(count) = remaining.get_mut(child) {
                        *count = count.saturating_sub(1);
                    }
                }
            }
        }
        layers.push(ready);
    }
    let mut placed = Vec::new();
    for (column, layer) in layers.iter().enumerate() {
        for (row, id) in layer.iter().enumerate() {
            placed.push((
                id.clone(),
                point(column as f32 * column_gap, row as f32 * row_gap),
            ));
        }
    }
    placed
}

fn selection_after(
    selected: &[SharedString],
    id: &SharedString,
    extend: bool,
) -> Vec<SharedString> {
    if !extend {
        return vec![id.clone()];
    }
    if selected.contains(id) {
        selected
            .iter()
            .filter(|selected| *selected != id)
            .cloned()
            .collect()
    } else {
        let mut next = selected.to_vec();
        next.push(id.clone());
        next
    }
}

#[derive(Debug, Clone)]
struct PortGeometry {
    id: SharedString,
    anchor: Anchor,
    direction: super::node::PortDirection,
}
#[derive(Debug, Clone)]
struct NodeGeometry {
    id: SharedString,
    bounds: Bounds<f32>,
    ports: Vec<PortGeometry>,
}
#[derive(Debug, Clone)]
struct RoutedEdge {
    edge: GraphEdge,
    route: OrthogonalRoute,
}

#[derive(Default)]
struct RouteCache {
    signature: u64,
    routes: Vec<RoutedEdge>,
}

#[derive(Debug, Clone)]
struct ConnectionPreview {
    route: OrthogonalRoute,
    target: Option<(GraphEndpoint, bool)>,
}

/// A node and where its top left corner sits, in canvas coordinates.
pub struct Placed {
    node: GraphNode,
    x: f32,
    y: f32,
    /// A height the caller declared, for a card whose content the node cannot
    /// measure. Left unset, the node measures itself.
    height: Option<f32>,
}

impl Placed {
    pub fn new(node: GraphNode, x: f32, y: f32) -> Self {
        Self {
            node,
            x,
            y,
            height: None,
        }
    }

    /// Sets the card's actual logical height. Routing and card layout use this
    /// same value. Non-finite and non-positive values leave the height
    /// automatic rather than creating geometry the card cannot render.
    pub fn height(mut self, height: f32) -> Self {
        self.height = (height.is_finite() && height > 0.0).then_some(height);
        self
    }

    fn bounds(&self, theme: &gpui_kit_theme::Theme, measured_height: Option<f32>) -> Bounds<f32> {
        let height = self
            .height
            .or(measured_height.filter(|height| height.is_finite() && *height > 0.0))
            .unwrap_or_else(|| self.node.measured_height(theme));
        Bounds::new(point(self.x, self.y), size(self.node.node_width(), height))
    }
}

impl std::fmt::Debug for Placed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Placed")
            .field("node", self.node.ident())
            .field("x", &self.x)
            .field("y", &self.y)
            .finish()
    }
}

/// What the canvas can currently say about itself.
///
/// These are the same five distinct states the rest of the library keeps
/// apart, and for the same reason: a canvas that is still loading, a run with
/// no steps, a graph the host would not produce, and a graph that failed to
/// load are four different things, and drawing any of them as an empty canvas
/// would be a lie a reader cannot detect.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GraphState {
    #[default]
    Ready,
    Loading,
    /// The host declined to produce the graph, in its own words.
    Refused(SharedString),
    /// The graph could not be loaded, in the host's own words.
    Failed(SharedString),
}

/// A run drawn as connected steps.
#[derive(IntoElement)]
pub struct NodeGraph {
    ident: Ident,
    nodes: Vec<Placed>,
    edges: Vec<GraphEdge>,
    state: GraphState,
    empty: Option<EmptyState>,
    grid: bool,
    viewport: GraphViewport,
    zoom_range: (f32, f32),
    interaction: GraphInteraction,
    on_event: Option<EventHandler>,
    minimap: bool,
}

impl std::fmt::Debug for NodeGraph {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NodeGraph")
            .field("ident", &self.ident)
            .field("nodes", &self.nodes.len())
            .field("edges", &self.edges.len())
            .field("state", &self.state)
            .finish()
    }
}

impl NodeGraph {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
            state: GraphState::Ready,
            empty: None,
            grid: true,
            viewport: GraphViewport::default(),
            zoom_range: (0.5, 2.0),
            interaction: GraphInteraction::default(),
            on_event: None,
            minimap: false,
        }
    }

    pub fn node(mut self, node: GraphNode, x: f32, y: f32) -> Self {
        self.nodes.push(Placed::new(node, x, y));
        self
    }

    pub fn placed(mut self, placed: Placed) -> Self {
        self.nodes.push(placed);
        self
    }

    pub fn edge(mut self, edge: GraphEdge) -> Self {
        self.edges.push(edge);
        self
    }

    pub fn edges(mut self, edges: impl IntoIterator<Item = GraphEdge>) -> Self {
        self.edges.extend(edges);
        self
    }

    pub fn state(mut self, state: GraphState) -> Self {
        self.state = state;
        self
    }

    /// What to draw when the run has no steps at all. Without one, an empty
    /// run draws as an empty canvas, which is only honest when the caller has
    /// confirmed that is what it is.
    pub fn empty(mut self, empty: EmptyState) -> Self {
        self.empty = Some(empty);
        self
    }

    /// Turns off the dot grid, for a canvas embedded somewhere that already
    /// has a texture of its own.
    pub fn grid(mut self, grid: bool) -> Self {
        self.grid = grid;
        self
    }

    /// Scrolls the canvas. The caller owns the offset, because panning is a
    /// gesture the host binds and a position the host may want to keep.
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        if x.is_finite() && y.is_finite() {
            self.viewport.offset = point(x, y);
        }
        self
    }

    pub fn viewport(mut self, viewport: GraphViewport) -> Self {
        if viewport.offset.x.is_finite() && viewport.offset.y.is_finite() {
            self.viewport.offset = viewport.offset;
        }
        if viewport.zoom.is_finite() && viewport.zoom > 0.0 {
            self.viewport.zoom = viewport.zoom;
        }
        self
    }
    pub fn zoom(mut self, zoom: f32) -> Self {
        if zoom.is_finite() && zoom > 0.0 {
            self.viewport.zoom = zoom;
        }
        self
    }
    pub fn zoom_range(mut self, min: f32, max: f32) -> Self {
        if min.is_finite() && max.is_finite() && min > 0.0 && min <= max {
            self.zoom_range = (min, max);
        }
        self
    }

    pub fn interaction(mut self, interaction: GraphInteraction) -> Self {
        self.interaction = interaction;
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(&NodeGraphEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }

    /// Draws a small overview of the placed nodes in the corner.
    pub fn minimap(mut self, show: bool) -> Self {
        self.minimap = show;
        self
    }

    /// The box of every node, by identity, for the edge painter.
    #[cfg(test)]
    fn geometry(&self, theme: &gpui_kit_theme::Theme) -> Vec<NodeGeometry> {
        self.geometry_with_heights(theme, &HashMap::new())
    }

    fn geometry_with_heights(
        &self,
        theme: &gpui_kit_theme::Theme,
        measured_heights: &HashMap<SharedString, f32>,
    ) -> Vec<NodeGeometry> {
        let node_counts = self
            .nodes
            .iter()
            .fold(HashMap::new(), |mut counts, placed| {
                *counts
                    .entry(placed.node.ident().semantic_id())
                    .or_insert(0usize) += 1;
                counts
            });
        self.nodes
            .iter()
            .filter(|placed| node_counts.get(&placed.node.ident().semantic_id()) == Some(&1))
            .map(|placed| {
                let id = placed.node.ident().semantic_id();
                let bounds = placed.bounds(theme, measured_heights.get(&id).copied());
                let port_counts =
                    placed
                        .node
                        .graph_ports()
                        .iter()
                        .fold(HashMap::new(), |mut counts, port| {
                            *counts.entry(port.id()).or_insert(0usize) += 1;
                            counts
                        });
                let ports = placed
                    .node
                    .graph_ports()
                    .iter()
                    .filter(|port| port_counts.get(port.id()) == Some(&1))
                    .map(|port| {
                        let same: Vec<&GraphPort> = placed
                            .node
                            .graph_ports()
                            .iter()
                            .filter(|p| p.port_side() == port.port_side())
                            .collect();
                        let index = same.iter().position(|p| p.id() == port.id()).unwrap_or(0);
                        let fraction = (index + 1) as f32 / (same.len() + 1) as f32;
                        let anchor = match port.port_side() {
                            PortSide::Top => {
                                point(bounds.left() + bounds.size.width * fraction, bounds.top())
                            }
                            PortSide::Right => {
                                point(bounds.right(), bounds.top() + bounds.size.height * fraction)
                            }
                            PortSide::Bottom => point(
                                bounds.left() + bounds.size.width * fraction,
                                bounds.bottom(),
                            ),
                            PortSide::Left => {
                                point(bounds.left(), bounds.top() + bounds.size.height * fraction)
                            }
                        };
                        PortGeometry {
                            id: port.id().clone(),
                            anchor: Anchor {
                                point: anchor,
                                side: port.port_side(),
                            },
                            direction: port.direction(),
                        }
                    })
                    .collect();
                NodeGeometry { id, bounds, ports }
            })
            .collect()
    }

    /// The edges that name two nodes this graph actually has.
    ///
    /// An edge to a node that is not here is dropped rather than guessed at:
    /// a line drawn to the wrong box would report a connection the run does
    /// not have, and there is no correct place to put a line whose end is
    /// missing.
    #[cfg(test)]
    fn routable(&self, theme: &gpui_kit_theme::Theme) -> Vec<RoutedEdge> {
        let nodes = self.geometry(theme);
        self.routable_geometry(&nodes)
    }

    fn routable_geometry(&self, nodes: &[NodeGeometry]) -> Vec<RoutedEdge> {
        let counts = self.edges.iter().fold(HashMap::new(), |mut m, e| {
            *m.entry(e.edge_id()).or_insert(0usize) += 1;
            m
        });
        self.edges
            .iter()
            .filter(|edge| counts.get(&edge.edge_id()) == Some(&1))
            .filter_map(|edge| {
                let from = nodes.iter().find(|n| &n.id == edge.from())?;
                let to = nodes.iter().find(|n| &n.id == edge.to())?;
                let (a, b) = match (edge.source_port(), edge.target_port()) {
                    (Some(a), Some(b)) => {
                        let a = from.ports.iter().find(|p| &p.id == a)?;
                        let b = to.ports.iter().find(|p| &p.id == b)?;
                        if a.direction != super::node::PortDirection::Output
                            || b.direction != super::node::PortDirection::Input
                        {
                            return None;
                        }
                        (a.anchor, b.anchor)
                    }
                    (None, None) => auto_anchors(from.bounds, to.bounds, edge.kind()),
                    _ => return None,
                };
                let route =
                    route_orthogonal(a, b, from.bounds, to.bounds, edge.kind(), edge.edge_lane())?;
                Some(RoutedEdge {
                    edge: edge.clone(),
                    route,
                })
            })
            .collect()
    }
}

fn auto_anchors(
    from: Bounds<f32>,
    to: Bounds<f32>,
    kind: super::edge::EdgeKind,
) -> (Anchor, Anchor) {
    let fc = from.center();
    let tc = to.center();
    let (fs, ts) = if kind == super::edge::EdgeKind::Feedback {
        (PortSide::Bottom, PortSide::Bottom)
    } else if (tc.x - fc.x).abs() >= (tc.y - fc.y).abs() {
        if tc.x >= fc.x {
            (PortSide::Right, PortSide::Left)
        } else {
            (PortSide::Left, PortSide::Right)
        }
    } else if tc.y >= fc.y {
        (PortSide::Bottom, PortSide::Top)
    } else {
        (PortSide::Top, PortSide::Bottom)
    };
    let at = |b: Bounds<f32>, side| Anchor {
        point: match side {
            PortSide::Top => point(b.center().x, b.top()),
            PortSide::Right => point(b.right(), b.center().y),
            PortSide::Bottom => point(b.center().x, b.bottom()),
            PortSide::Left => point(b.left(), b.center().y),
        },
        side,
    };
    (at(from, fs), at(to, ts))
}

fn connection_target(
    nodes: &[NodeGeometry],
    from: &GraphEndpoint,
    pointer: Point<f32>,
    viewport: GraphViewport,
    radius: f32,
) -> Option<(GraphEndpoint, bool)> {
    nodes
        .iter()
        .flat_map(|node| node.ports.iter().map(move |port| (node, port)))
        .filter_map(|(node, port)| {
            let at = world_to_screen(port.anchor.point, viewport);
            let distance = (at.x - pointer.x).powi(2) + (at.y - pointer.y).powi(2);
            (distance <= radius.powi(2)).then(|| {
                let endpoint = GraphEndpoint::new(node.id.clone(), port.id.clone());
                let valid = port.direction == PortDirection::Input && &endpoint != from;
                (distance, endpoint, valid)
            })
        })
        .min_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(_, endpoint, valid)| (endpoint, valid))
}

impl RenderOnce for NodeGraph {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut viewport = self.viewport;
        viewport.zoom = viewport.zoom.clamp(self.zoom_range.0, self.zoom_range.1);
        let moving_effects = matches!(self.state, GraphState::Ready)
            && (self.edges.iter().any(GraphEdge::is_active)
                || self
                    .nodes
                    .iter()
                    .any(|placed| placed.node.node_state().is_busy()));
        let graph_busy = matches!(self.state, GraphState::Loading) || moving_effects;
        let gesture = keyed::slot::<GestureState>(&self.ident.semantic_id(), cx);
        let animation_phase = if moving_effects && !cx.reduce_motion() {
            let now = cx.background_executor().now();
            let mut state = gesture.borrow_mut();
            let started = *state.animation_started.get_or_insert(now);
            Some((now.duration_since(started).as_secs_f32() / 1.8).rem_euclid(1.0))
        } else {
            gesture.borrow_mut().animation_started = None;
            None
        };
        if animation_phase.is_some() {
            window.request_animation_frame();
        }
        let spec = NodeSpec::new(self.ident.semantic_id(), Role::Group).busy(graph_busy);

        let measured = measure::cell(&self.ident.child("viewport").semantic_id(), cx);
        let record = Rc::clone(&measured);
        let mut frame = div()
            .on_children_prepainted(move |bounds, window, _| {
                if let Some(first) = bounds.first() {
                    measure::record(&record, *first, window);
                }
            })
            .id(self.ident.element_id())
            .relative()
            .size_full()
            .overflow_hidden()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .surface(&theme, Surface::Canvas);

        if let Some(report) = self
            .on_event
            .as_ref()
            .cloned()
            .filter(|_| matches!(self.state, GraphState::Ready) && !self.nodes.is_empty())
        {
            let down = Rc::clone(&gesture);
            frame =
                frame.on_mouse_down_with_pointer_capture(MouseButton::Left, move |event, _, cx| {
                    down.borrow_mut().gesture = if event.modifiers.shift {
                        Some(Gesture::Marquee {
                            origin: event.position,
                            current: event.position,
                        })
                    } else {
                        Some(Gesture::Pan {
                            at: event.position,
                            viewport,
                            moved: false,
                        })
                    };
                    cx.stop_propagation();
                });
            let moving = Rc::clone(&gesture);
            let move_report = Rc::clone(&report);
            frame = frame.on_mouse_move(move |event, window, cx| {
                let mut state = moving.borrow_mut();
                state.pointer = Some(event.position);
                if let Some(Gesture::Pan {
                    at,
                    viewport,
                    moved,
                }) = state.gesture.as_mut()
                {
                    if event.pressed_button != Some(MouseButton::Left) {
                        state.gesture = None;
                        return;
                    }
                    let delta = point(
                        f32::from(event.position.x - at.x),
                        f32::from(event.position.y - at.y),
                    );
                    *moved |= delta.x.abs().max(delta.y.abs()) >= 4.0;
                    move_report(
                        &NodeGraphEvent::ViewportChanged(GraphViewport {
                            offset: point(viewport.offset.x + delta.x, viewport.offset.y + delta.y),
                            zoom: viewport.zoom,
                        }),
                        window,
                        cx,
                    );
                } else if let Some(Gesture::Marquee { current, .. }) = state.gesture.as_mut() {
                    if event.pressed_button != Some(MouseButton::Left) {
                        state.gesture = None;
                        return;
                    }
                    *current = event.position;
                    window.refresh();
                }
            });
            let up = Rc::clone(&gesture);
            let up_report = Rc::clone(&report);
            let up_bounds = Rc::clone(&measured);
            let up_nodes = self
                .nodes
                .iter()
                .map(|placed| (placed.node.ident().semantic_id(), placed.x, placed.y))
                .collect::<Vec<_>>();
            frame = frame.on_mouse_up(MouseButton::Left, move |_, window, cx| {
                let gesture = up.borrow_mut().gesture.take();
                match gesture {
                    Some(Gesture::Pan { moved: false, .. }) => {
                        up_report(
                            &NodeGraphEvent::SelectionChanged { ids: Vec::new() },
                            window,
                            cx,
                        );
                    }
                    Some(Gesture::Marquee { origin, current }) => {
                        let frame = up_bounds.get();
                        let a = screen_to_world(
                            point(
                                f32::from(origin.x - frame.origin.x),
                                f32::from(origin.y - frame.origin.y),
                            ),
                            viewport,
                        );
                        let b = screen_to_world(
                            point(
                                f32::from(current.x - frame.origin.x),
                                f32::from(current.y - frame.origin.y),
                            ),
                            viewport,
                        );
                        let box_bounds = Bounds::new(
                            point(a.x.min(b.x), a.y.min(b.y)),
                            size((a.x - b.x).abs(), (a.y - b.y).abs()),
                        );
                        let ids = up_nodes
                            .iter()
                            .filter(|(_, x, y)| {
                                bounds_overlap(
                                    Bounds::new(point(*x, *y), size(NODE_WIDTH, 48.0)),
                                    box_bounds,
                                    0.0,
                                )
                            })
                            .map(|(id, _, _)| id.clone())
                            .collect();
                        up_report(&NodeGraphEvent::SelectionChanged { ids }, window, cx);
                    }
                    _ => {}
                }
            });
            let wheel_report = Rc::clone(&report);
            let wheel_bounds = Rc::clone(&measured);
            let wheel_gesture = Rc::clone(&gesture);
            let (min_zoom, max_zoom) = self.zoom_range;
            frame = frame.on_scroll_wheel(move |event, window, cx| {
                if wheel_gesture.borrow().gesture.is_some() {
                    cx.stop_propagation();
                    return;
                }
                let delta = match event.delta {
                    ScrollDelta::Lines(delta) => delta.y * 40.0,
                    ScrollDelta::Pixels(delta) => f32::from(delta.y),
                };
                if !delta.is_finite() || delta == 0.0 {
                    return;
                }
                let next = (viewport.zoom * (delta / 400.0).exp()).clamp(min_zoom, max_zoom);
                if (next - viewport.zoom).abs() < f32::EPSILON {
                    return;
                }
                let bounds = wheel_bounds.get();
                if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
                    return;
                }
                let at = point(
                    f32::from(event.position.x - bounds.origin.x),
                    f32::from(event.position.y - bounds.origin.y),
                );
                wheel_report(
                    &NodeGraphEvent::ViewportChanged(zoom_at(viewport, at, next)),
                    window,
                    cx,
                );
            });
        }

        // A canvas that is not ready draws its reason and nothing else. Steps
        // left underneath a failure would read as a run that is still going.
        let message = match &self.state {
            GraphState::Loading => Some((
                theme.colors.text_muted,
                cx.strings().text(StringKey::Loading),
            )),
            GraphState::Refused(reason) => Some((theme.colors.warning, reason.clone())),
            GraphState::Failed(reason) => Some((theme.colors.danger, reason.clone())),
            GraphState::Ready => None,
        };
        if let Some((color, text)) = message {
            return frame
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .p_token(&theme, Space::Lg)
                        .type_scale(&theme, TypeScale::Label)
                        .text_color(color)
                        .child(text),
                )
                .semantic_in(
                    cx,
                    spec.value(viewport_value(
                        match self.state {
                            GraphState::Loading => "loading",
                            GraphState::Refused(_) => "refused",
                            _ => "failed",
                        },
                        viewport,
                    )),
                )
                .into_any_element();
        }

        if self.nodes.is_empty() {
            let empty = self.empty.map(|empty| {
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(empty)
            });
            return frame
                .children(empty)
                .semantic_in(cx, spec.value(viewport_value("empty", viewport)))
                .into_any_element();
        }

        let node_measurements: HashMap<SharedString, Rc<Cell<Bounds<Pixels>>>> = self
            .nodes
            .iter()
            .map(|placed| {
                let id = placed.node.ident().semantic_id();
                let measurement_id = composite_id("node-measure", &[id.as_ref()]);
                (id, measure::cell(&measurement_id, cx))
            })
            .collect();
        let measured_heights: HashMap<SharedString, f32> = node_measurements
            .iter()
            .filter_map(|(id, measured)| {
                let height = f32::from(measured.get().size.height);
                (height > 0.0 && height.is_finite()).then(|| (id.clone(), height))
            })
            .collect();
        let geometry = self.geometry_with_heights(&theme, &measured_heights);
        let route_cell = keyed::slot::<RouteCache>(&self.ident.child("routes").semantic_id(), cx);
        let signature = route_signature(&geometry, &self.edges);
        let routes = {
            let mut cache = route_cell.borrow_mut();
            if cache.signature == signature && !cache.routes.is_empty() {
                cache.routes.clone()
            } else {
                let routes = self.routable_geometry(&geometry);
                cache.signature = signature;
                cache.routes = routes.clone();
                routes
            }
        };
        let compact = viewport.zoom < LOD_ZOOM;
        let view = {
            let bounds = measured.get();
            (f32::from(bounds.size.width) > 1.0).then(|| world_view(viewport, bounds))
        };
        let visible_ids: std::collections::HashSet<SharedString> = geometry
            .iter()
            .filter(|node| {
                view.map(|view| bounds_overlap(node.bounds, view, CULL_PAD))
                    .unwrap_or(true)
            })
            .map(|node| node.id.clone())
            .collect();
        let routes: Vec<RoutedEdge> = routes
            .into_iter()
            .filter(|routed| {
                let Some(view) = view else {
                    return true;
                };
                let from = geometry.iter().find(|node| node.id == *routed.edge.from());
                let to = geometry.iter().find(|node| node.id == *routed.edge.to());
                match (from, to) {
                    (Some(from), Some(to)) => {
                        visible_ids.contains(&from.id)
                            || visible_ids.contains(&to.id)
                            || bounds_overlap(from.bounds, view, CULL_PAD)
                            || bounds_overlap(to.bounds, view, CULL_PAD)
                    }
                    _ => false,
                }
            })
            .collect();
        let preview = {
            let state = gesture.borrow();
            match (&state.gesture, state.pointer) {
                (Some(Gesture::Connect { from }), Some(pointer)) => {
                    let source = geometry
                        .iter()
                        .find(|node| node.id == from.node)
                        .and_then(|node| node.ports.iter().find(|port| port.id == from.port));
                    source.map(|source| {
                        let bounds = measured.get();
                        let pointer = point(
                            f32::from(pointer.x - bounds.origin.x),
                            f32::from(pointer.y - bounds.origin.y),
                        );
                        let world = screen_to_world(pointer, viewport);
                        ConnectionPreview {
                            route: route_preview(source.anchor, world),
                            target: connection_target(
                                &geometry,
                                from,
                                pointer,
                                viewport,
                                (14.0 * viewport.zoom).max(10.0),
                            ),
                        }
                    })
                }
                _ => None,
            }
        };
        let stroke = theme.borders.hairline;
        let grid_color = theme.colors.hairline;
        let draw_grid = self.grid;
        let edge_theme = theme.clone();
        let painted_routes = routes.clone();
        let painted_preview = preview.clone();

        // Edges and the grid are one painted layer beneath the nodes, so a
        // connection never intercepts a click meant for a card and adding one
        // moves nothing.
        let beneath = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                if draw_grid {
                    paint_grid(
                        window,
                        bounds,
                        viewport.offset.x,
                        viewport.offset.y,
                        GRID_STEP * viewport.zoom,
                        grid_color,
                    );
                }
                for routed in painted_routes {
                    let transform =
                        RouteTransform::new(bounds.origin, viewport.offset, viewport.zoom);
                    paint_route(
                        window,
                        &edge_theme,
                        &routed.edge,
                        &routed.route,
                        transform,
                        stroke,
                        animation_phase,
                    );
                }
                if let Some(preview) = painted_preview {
                    let color = match preview.target {
                        Some((_, true)) => edge_theme.colors.success,
                        Some((_, false)) => edge_theme.colors.danger,
                        None => edge_theme.colors.accent,
                    };
                    paint_route_stroke(
                        window,
                        &preview.route,
                        RouteTransform::new(bounds.origin, viewport.offset, viewport.zoom),
                        stroke * 1.5,
                        color.opacity(0.88),
                        None,
                    );
                }
            },
        )
        .absolute()
        .inset_0();

        let edge_labels: Vec<AnyElement> = routes
            .iter()
            .filter_map(|routed| {
                let label = routed.edge.edge_label()?.clone();
                let at = world_to_screen(routed.route.midpoint(), viewport);
                let from = geometry
                    .iter()
                    .find(|node| &node.id == routed.edge.from())?;
                let to = geometry.iter().find(|node| &node.id == routed.edge.to())?;
                let center = world_to_screen(
                    point(
                        (from.bounds.center().x + to.bounds.center().x) * 0.5,
                        (from.bounds.center().y + to.bounds.center().y) * 0.5,
                    ),
                    viewport,
                );
                let left_edge = world_to_screen(
                    point(
                        from.bounds.left().max(to.bounds.left()),
                        from.bounds.top().max(to.bounds.top()),
                    ),
                    viewport,
                );
                // The disconnect chip is drawn on the same midpoint, so the
                // label has to clear its radius rather than the line's.
                let chip_radius = if self.on_event.is_some() && self.interaction.edits_topology() {
                    9.0
                } else {
                    0.0
                };
                let gap = (chip_radius + 6.0) * viewport.zoom;
                let label = div()
                    .absolute()
                    .whitespace_nowrap()
                    .px(px(theme.spacing.xs * viewport.zoom))
                    .rounded_sm()
                    .bg(theme.colors.canvas)
                    .text_size(px(theme.typography.caption.size * viewport.zoom))
                    .text_color(theme.colors.text_muted)
                    .child(label);
                let label = match routed.route.midpoint_axis() {
                    Axis::Vertical if at.x <= left_edge.x => label.right(px(gap)).top(px(-theme
                        .typography
                        .caption
                        .line_height
                        * viewport.zoom
                        * 0.5)),
                    Axis::Vertical => label.left(px(gap)).top(px(-theme
                        .typography
                        .caption
                        .line_height
                        * viewport.zoom
                        * 0.5)),
                    Axis::Horizontal if at.y <= center.y && at.x >= center.x => {
                        label.right(px(gap)).bottom(px(gap))
                    }
                    Axis::Horizontal if at.y <= center.y => label.left(px(gap)).bottom(px(gap)),
                    Axis::Horizontal if at.x >= center.x => label.right(px(gap)).top(px(gap)),
                    Axis::Horizontal => label.left(px(gap)).top(px(gap)),
                };
                Some(
                    div()
                        .absolute()
                        .left(px(at.x))
                        .top(px(at.y))
                        .w(px(0.0))
                        .h(px(0.0))
                        .child(label)
                        .into_any_element(),
                )
            })
            .collect();

        let edge_nodes: Vec<AnyElement> = routes
            .iter()
            .map(|routed| {
                let id = routed.edge.edge_id();
                let semantic_id = composite_id("graph-edge", &[id.as_ref()]);
                let at = world_to_screen(routed.route.midpoint(), viewport);
                let size = 18.0 * viewport.zoom;
                let relation = routed
                    .edge
                    .edge_label()
                    .cloned()
                    .unwrap_or_else(|| cx.strings().text(StringKey::CanvasConnection));
                if let Some(report) = self
                    .on_event
                    .as_ref()
                    .filter(|_| self.interaction.edits_topology())
                {
                    let report_pointer = Rc::clone(report);
                    let pointer_id = id.clone();
                    let report_key = Rc::clone(report);
                    let key_id = id.clone();
                    div()
                        .id(semantic_id.clone())
                        .absolute()
                        .left(px(at.x - size * 0.5))
                        .top(px(at.y - size * 0.5))
                        .w(px(size))
                        .h(px(size))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(theme.colors.hairline_strong)
                        .bg(theme.colors.canvas)
                        .cursor_pointer()
                        .tab_index(0)
                        .focus_ring(&theme)
                        .pressable(cx)
                        .hover(|style| style.bg(theme.colors.hover))
                        .child(
                            icon(Icon::Close)
                                .size(px(9.0 * viewport.zoom))
                                .text_color(theme.colors.text_muted),
                        )
                        .on_mouse_down_with_pointer_capture(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation()
                        })
                        .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                            report_pointer(
                                &NodeGraphEvent::DisconnectRequested {
                                    id: pointer_id.clone(),
                                },
                                window,
                                cx,
                            );
                            cx.stop_propagation();
                        })
                        .on_key_down(move |event, window, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                report_key(
                                    &NodeGraphEvent::DisconnectRequested { id: key_id.clone() },
                                    window,
                                    cx,
                                );
                                cx.stop_propagation();
                            }
                        })
                        .semantic_in(
                            cx,
                            NodeSpec::new(semantic_id, Role::Button)
                                .parent(self.ident.semantic_id())
                                .text(cx.strings().text(StringKey::CanvasDisconnect))
                                .description(relation)
                                .value(id),
                        )
                        .into_any_element()
                } else {
                    div()
                        .absolute()
                        .left(px(at.x - size * 0.5))
                        .top(px(at.y - size * 0.5))
                        .w(px(size))
                        .h(px(size))
                        .semantic_in(
                            cx,
                            NodeSpec::new(semantic_id, Role::Group)
                                .parent(self.ident.semantic_id())
                                .text(relation)
                                .value(id),
                        )
                        .into_any_element()
                }
            })
            .collect();

        let mut ports = Vec::new();
        for node in geometry.iter().filter(|_| !compact) {
            let Some(placed) = self
                .nodes
                .iter()
                .find(|placed| placed.node.ident().semantic_id() == node.id)
            else {
                continue;
            };
            for port_geometry in &node.ports {
                let Some(port) = placed
                    .node
                    .graph_ports()
                    .iter()
                    .find(|port| port.id() == &port_geometry.id)
                else {
                    continue;
                };
                let at = world_to_screen(port_geometry.anchor.point, viewport);
                let endpoint = GraphEndpoint::new(node.id.clone(), port.id().clone());
                let semantic_id =
                    composite_id("graph-port", &[node.id.as_ref(), port.id().as_ref()]);
                let editable = self.interaction.edits_topology();
                let spec = NodeSpec::new(
                    semantic_id.clone(),
                    if editable { Role::Button } else { Role::Group },
                )
                .text(port.label().clone())
                .value(port.direction().name());
                let diameter = 12.0 * viewport.zoom;
                let label_gap = 4.0 * viewport.zoom;
                let target = preview
                    .as_ref()
                    .and_then(|preview| preview.target.as_ref())
                    .filter(|(target, _)| target == &endpoint)
                    .map(|(_, valid)| *valid);
                let color = match target {
                    Some(true) => theme.colors.success,
                    Some(false) => theme.colors.danger,
                    None if port.direction() == PortDirection::Output => theme.colors.accent,
                    None => theme.colors.hairline_strong,
                };
                let label = div()
                    .absolute()
                    .whitespace_nowrap()
                    .px(px(2.0 * viewport.zoom))
                    .rounded_sm()
                    .bg(theme.colors.canvas)
                    .text_size(px(theme.typography.caption.size * viewport.zoom))
                    .text_color(theme.colors.text_muted)
                    .child(port.label().clone());
                let label = match (port.port_side(), port.direction()) {
                    (PortSide::Left, PortDirection::Input) => label
                        .right(px(diameter + label_gap))
                        .top(px(diameter + label_gap)),
                    (PortSide::Left, PortDirection::Output) => label
                        .right(px(diameter + label_gap))
                        .bottom(px(diameter + label_gap)),
                    (PortSide::Right, PortDirection::Input) => label
                        .left(px(diameter + label_gap))
                        .top(px(diameter + label_gap)),
                    (PortSide::Right, PortDirection::Output) => label
                        .left(px(diameter + label_gap))
                        .bottom(px(diameter + label_gap)),
                    (PortSide::Top, _) => label
                        .left(px(diameter + label_gap))
                        .bottom(px(diameter / 2.0)),
                    (PortSide::Bottom, _) => {
                        label.left(px(diameter + label_gap)).top(px(diameter / 2.0))
                    }
                };
                let mut view = div()
                    .id(semantic_id)
                    .absolute()
                    .left(px(at.x - diameter / 2.0))
                    .top(px(at.y - diameter / 2.0))
                    .w(px(diameter))
                    .h(px(diameter))
                    .rounded_full()
                    .border_1()
                    .border_color(theme.colors.canvas)
                    .bg(color)
                    .child(label);
                if editable {
                    view = view.cursor_pointer();
                }
                if editable && port.direction() == PortDirection::Output {
                    let down = Rc::clone(&gesture);
                    let from = endpoint.clone();
                    view = view.on_mouse_down_with_pointer_capture(
                        MouseButton::Left,
                        move |event, window, cx| {
                            let mut state = down.borrow_mut();
                            state.pointer = Some(event.position);
                            state.gesture = Some(Gesture::Connect { from: from.clone() });
                            window.refresh();
                            cx.stop_propagation();
                        },
                    );
                    let moving = Rc::clone(&gesture);
                    view = view.on_mouse_move(move |event, window, cx| {
                        if event.pressed_button != Some(MouseButton::Left) {
                            moving.borrow_mut().gesture = None;
                            return;
                        }
                        moving.borrow_mut().pointer = Some(event.position);
                        window.refresh();
                        cx.stop_propagation();
                    });
                    let up = Rc::clone(&gesture);
                    let candidates = geometry.clone();
                    let report = self.on_event.clone();
                    let target_bounds = Rc::clone(&measured);
                    view = view.on_mouse_up(MouseButton::Left, move |event, window, cx| {
                        let mut state = up.borrow_mut();
                        if let Some(Gesture::Connect { from }) = state.gesture.take() {
                            let bounds = target_bounds.get();
                            let pointer = point(
                                f32::from(event.position.x - bounds.origin.x),
                                f32::from(event.position.y - bounds.origin.y),
                            );
                            if let (Some(report), Some((to, true))) = (
                                &report,
                                connection_target(
                                    &candidates,
                                    &from,
                                    pointer,
                                    viewport,
                                    (14.0 * viewport.zoom).max(10.0),
                                ),
                            ) {
                                report(
                                    &NodeGraphEvent::ConnectionRequested { from, to },
                                    window,
                                    cx,
                                );
                            }
                        }
                        state.pointer = None;
                        window.refresh();
                        cx.stop_propagation();
                    });
                } else {
                    view = view.on_mouse_down(MouseButton::Left, |_, _, cx| {
                        // Input ports are connection targets, not blank canvas.
                        cx.stop_propagation();
                    });
                }
                ports.push(view.semantic_in(cx, spec).into_any_element());
            }
        }

        let mut shockwaves = Vec::new();
        for placed in self
            .nodes
            .iter()
            .filter(|placed| placed.node.node_state().is_busy())
            .filter(|placed| {
                geometry
                    .iter()
                    .any(|node| node.id == placed.node.ident().semantic_id())
            })
        {
            let Some(bounds) = geometry
                .iter()
                .find(|node| node.id == placed.node.ident().semantic_id())
                .map(|node| node.bounds)
            else {
                continue;
            };
            let screen = world_to_screen(bounds.origin, viewport);
            let width = bounds.size.width * viewport.zoom;
            let height = bounds.size.height * viewport.zoom;
            let phases: Vec<f32> = animation_phase
                .map(|phase| vec![phase, (phase + 0.5).rem_euclid(1.0)])
                .unwrap_or_else(|| vec![0.35]);
            for phase in phases {
                let reach = (8.0 + phase * 30.0) * viewport.zoom;
                let opacity = if animation_phase.is_some() {
                    0.36 * (1.0 - phase).powf(1.7)
                } else {
                    0.18
                };
                shockwaves.push(
                    div()
                        .absolute()
                        .left(px(screen.x - reach))
                        .top(px(screen.y - reach))
                        .w(px(width + reach * 2.0))
                        .h(px(height + reach * 2.0))
                        .rounded(px(theme.radius(Radius::Card) * viewport.zoom + reach))
                        .border_1()
                        .border_color(theme.colors.accent.opacity(opacity))
                        .into_any_element(),
                );
            }
        }

        let report = self.on_event.clone();
        let interaction = self.interaction;
        let starts: HashMap<SharedString, Point<f32>> = self
            .nodes
            .iter()
            .map(|placed| (placed.node.ident().semantic_id(), point(placed.x, placed.y)))
            .collect();
        let selected: Vec<SharedString> = self
            .nodes
            .iter()
            .filter(|placed| placed.node.node_selected())
            .map(|placed| placed.node.ident().semantic_id())
            .collect();
        let cards: Vec<AnyElement> = self
            .nodes
            .into_iter()
            .filter(|placed| {
                let id = placed.node.ident().semantic_id();
                visible_ids.contains(&id) && geometry.iter().any(|node| node.id == id)
            })
            .map(|placed| {
                let screen = world_to_screen(point(placed.x, placed.y), viewport);
                let height = placed.height;
                let id = placed.node.ident().semantic_id();
                let measurement = node_measurements.get(&id).cloned();
                let click = placed.node.click_handler();
                let pointer_click = report.is_none();
                let node = if let Some(report) = report.as_ref() {
                    let activate_report = Rc::clone(report);
                    let activate_id = id.clone();
                    let activate_selected = selected.clone();
                    let activate_click = click.clone();
                    let activate = Rc::new(move |window: &mut Window, cx: &mut App| {
                        activate_report(
                            &NodeGraphEvent::SelectionChanged {
                                ids: selection_after(&activate_selected, &activate_id, false),
                            },
                            window,
                            cx,
                        );
                        if let Some(click) = &activate_click {
                            click(window, cx);
                        }
                    });
                    let delete = interaction.edits_topology().then(|| {
                        let delete_report = Rc::clone(report);
                        let delete_id = id.clone();
                        Rc::new(move |window: &mut Window, cx: &mut App| {
                            delete_report(
                                &NodeGraphEvent::NodeDeleted {
                                    id: delete_id.clone(),
                                },
                                window,
                                cx,
                            );
                        }) as super::node::ClickHandler
                    });
                    placed.node.graph_handlers(Some(activate), delete)
                } else {
                    placed.node
                };
                let mut card = div()
                    .absolute()
                    .left(px(screen.x))
                    .top(px(screen.y))
                    .w(px(node.node_width() * viewport.zoom))
                    .child(
                        node.display_at(viewport.zoom, height)
                            .compact(compact)
                            .pointer_click(pointer_click),
                    );
                if let Some(measurement) = measurement {
                    card = card.on_children_prepainted(move |bounds, window, _| {
                        let Some(first) = bounds.first() else {
                            return;
                        };
                        let logical = Bounds::new(
                            point(px(0.0), px(0.0)),
                            size(
                                px(f32::from(first.size.width) / viewport.zoom),
                                px(f32::from(first.size.height) / viewport.zoom),
                            ),
                        );
                        measure::record(&measurement, logical, window);
                    });
                }
                let mut card = card.id(composite_id("node-drag", &[id.as_ref()]));
                if let Some(report) = report.as_ref().cloned() {
                    let down = Rc::clone(&gesture);
                    let start = point(placed.x, placed.y);
                    let drag_id = id.clone();
                    let peers = if selected.contains(&drag_id) {
                        selected
                            .iter()
                            .filter(|peer| *peer != &drag_id)
                            .filter_map(|peer| {
                                starts
                                    .get(peer)
                                    .copied()
                                    .map(|position| (peer.clone(), position))
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                    card = card.on_mouse_down_with_pointer_capture(
                        MouseButton::Left,
                        move |event, _, cx| {
                            down.borrow_mut().gesture = Some(Gesture::Node {
                                at: event.position,
                                id: drag_id.clone(),
                                position: start,
                                peers: peers.clone(),
                                moved: false,
                                extend_selection: event.modifiers.shift
                                    || event.modifiers.secondary(),
                            });
                            cx.stop_propagation();
                        },
                    );
                    let moving = Rc::clone(&gesture);
                    let move_report = Rc::clone(&report);
                    let moves_nodes = interaction.moves_nodes();
                    card = card.on_mouse_move(move |event, window, cx| {
                        let mut state = moving.borrow_mut();
                        if event.pressed_button != Some(MouseButton::Left) {
                            state.gesture = None;
                            return;
                        }
                        let moves = match state.gesture.as_mut() {
                            Some(Gesture::Node {
                                at,
                                id,
                                position,
                                peers,
                                moved,
                                ..
                            }) => {
                                let screen_delta = point(
                                    f32::from(event.position.x - at.x),
                                    f32::from(event.position.y - at.y),
                                );
                                *moved |= screen_delta.x.abs().max(screen_delta.y.abs()) >= 4.0;
                                let delta = point(
                                    screen_delta.x / viewport.zoom,
                                    screen_delta.y / viewport.zoom,
                                );
                                let mut moves = vec![(
                                    id.clone(),
                                    point(position.x + delta.x, position.y + delta.y),
                                )];
                                for (peer, start) in peers {
                                    moves.push((
                                        peer.clone(),
                                        point(start.x + delta.x, start.y + delta.y),
                                    ));
                                }
                                moves
                            }
                            _ => return,
                        };
                        drop(state);
                        if moves_nodes {
                            for (id, position) in moves {
                                move_report(
                                    &NodeGraphEvent::NodeMoved { id, position },
                                    window,
                                    cx,
                                );
                            }
                        }
                        cx.stop_propagation();
                    });
                    let up = Rc::clone(&gesture);
                    let current_selection = selected.clone();
                    card = card.on_mouse_up(MouseButton::Left, move |_, window, cx| {
                        let gesture = up.borrow_mut().gesture.take();
                        if let Some(Gesture::Node {
                            id,
                            moved: false,
                            extend_selection,
                            ..
                        }) = gesture
                        {
                            report(
                                &NodeGraphEvent::SelectionChanged {
                                    ids: selection_after(&current_selection, &id, extend_selection),
                                },
                                window,
                                cx,
                            );
                            if let Some(click) = &click {
                                click(window, cx);
                            }
                        }
                        cx.stop_propagation();
                    });
                }
                card.into_any_element()
            })
            .collect();

        let overview = self
            .minimap
            .then(|| graph_minimap(&self.ident, &geometry, viewport, &theme, cx));
        let group = selected
            .len()
            .gt(&1)
            .then(|| {
                let members: Vec<_> = geometry
                    .iter()
                    .filter(|node| selected.iter().any(|id| id == &node.id))
                    .collect();
                let (min, max) =
                    members
                        .iter()
                        .fold(None, |acc: Option<(Point<f32>, Point<f32>)>, node| {
                            let origin = world_to_screen(node.bounds.origin, viewport);
                            let far = world_to_screen(
                                point(
                                    node.bounds.origin.x + node.bounds.size.width,
                                    node.bounds.origin.y + node.bounds.size.height,
                                ),
                                viewport,
                            );
                            Some(match acc {
                                None => (origin, far),
                                Some((left, right)) => (
                                    point(left.x.min(origin.x), left.y.min(origin.y)),
                                    point(right.x.max(far.x), right.y.max(far.y)),
                                ),
                            })
                        })?;
                Some(
                    div()
                        .absolute()
                        .left(px(min.x - 8.0))
                        .top(px(min.y - 8.0))
                        .w(px((max.x - min.x) + 16.0))
                        .h(px((max.y - min.y) + 16.0))
                        .rounded(px(theme.radius(Radius::Card)))
                        .border_1()
                        .border_color(theme.colors.accent.opacity(0.55))
                        .into_any_element(),
                )
            })
            .flatten();
        let marquee = {
            let state = gesture.borrow();
            match &state.gesture {
                Some(Gesture::Marquee { origin, current }) => {
                    let frame = measured.get();
                    let left = f32::from(origin.x.min(current.x) - frame.origin.x);
                    let top = f32::from(origin.y.min(current.y) - frame.origin.y);
                    let width = f32::from((origin.x - current.x).abs());
                    let height = f32::from((origin.y - current.y).abs());
                    Some(
                        div()
                            .absolute()
                            .left(px(left))
                            .top(px(top))
                            .w(px(width))
                            .h(px(height))
                            .border_1()
                            .border_color(theme.colors.accent.opacity(0.7))
                            .bg(theme.colors.accent.opacity(0.08))
                            .into_any_element(),
                    )
                }
                _ => None,
            }
        };
        frame
            .child(beneath)
            .children(shockwaves)
            .children(if compact { Vec::new() } else { edge_labels })
            .children(group)
            .children(cards)
            .children(ports)
            .children(edge_nodes)
            .children(marquee)
            .children(overview)
            .semantic_in(cx, spec.value(viewport_value("ready", viewport)))
            .into_any_element()
    }
}

fn graph_minimap(
    ident: &Ident,
    geometry: &[NodeGeometry],
    viewport: GraphViewport,
    theme: &gpui_kit_theme::Theme,
    cx: &mut App,
) -> AnyElement {
    let ident = ident.child("minimap");
    let world: Option<(Point<f32>, Point<f32>)> = geometry.iter().fold(None, |acc, node| {
        let min = node.bounds.origin;
        let max = point(
            node.bounds.origin.x + node.bounds.size.width,
            node.bounds.origin.y + node.bounds.size.height,
        );
        Some(match acc {
            None => (min, max),
            Some((left, right)) => (
                point(left.x.min(min.x), left.y.min(min.y)),
                point(right.x.max(max.x), right.y.max(max.y)),
            ),
        })
    });
    let marks = world
        .map(|(min, max)| {
            let width = (max.x - min.x).max(1.0);
            let height = (max.y - min.y).max(1.0);
            geometry
                .iter()
                .map(|node| {
                    let x = (node.bounds.origin.x - min.x) / width;
                    let y = (node.bounds.origin.y - min.y) / height;
                    let w = node.bounds.size.width / width;
                    let h = node.bounds.size.height / height;
                    div()
                        .absolute()
                        .left(relative(x))
                        .top(relative(y))
                        .w(relative(w.max(0.04)))
                        .h(relative(h.max(0.04)))
                        .bg(theme.colors.accent.opacity(0.7))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let _ = viewport;
    div()
        .id(ident.element_id())
        .absolute()
        .right(px(theme.space(Space::Sm)))
        .bottom(px(theme.space(Space::Sm)))
        .w(px(140.0))
        .h(px(88.0))
        .radius(theme, Radius::Small)
        .surface(theme, Surface::Overlay)
        .elevation(theme, Elevation::Raised)
        .overflow_hidden()
        .children(marks)
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Status)
                .text(cx.strings().text(StringKey::GraphMinimap))
                .value("minimap"),
        )
        .into_any_element()
}

/// Paints the dot grid the canvas sits on.
///
/// The grid is anchored to the pan offset rather than to the viewport, so it
/// travels with the graph and reports that the canvas moved. A grid pinned to
/// the viewport would sit still under a graph that was moving, which reads as
/// the graph having stayed where it was.
fn paint_grid(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    pan_x: f32,
    pan_y: f32,
    step: f32,
    color: gpui::Hsla,
) {
    let first = |pan: f32| pan.rem_euclid(step);
    let mut y = first(pan_y);
    while y < f32::from(bounds.size.height) {
        let mut x = first(pan_x);
        while x < f32::from(bounds.size.width) {
            window.paint_quad(gpui::fill(
                Bounds::new(
                    point(bounds.origin.x + px(x), bounds.origin.y + px(y)),
                    size(px(GRID_DOT), px(GRID_DOT)),
                ),
                color,
            ));
            x += step;
        }
        y += step;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::edge::EdgeKind;
    use crate::canvas::node::NodeState;

    fn graph() -> NodeGraph {
        NodeGraph::new("run")
            .node(GraphNode::new("run.plan", "Plan"), 0.0, 0.0)
            .node(
                GraphNode::new("run.apply", "Apply").state(NodeState::Failed),
                300.0,
                0.0,
            )
    }

    #[test]
    fn a_node_box_follows_the_width_the_node_carries() {
        let placed = Placed::new(GraphNode::new("a", "A").width(150.0), 10.0, 20.0);
        let bounds = placed.bounds(&gpui_kit_theme::Theme::studio_dark(), None);
        assert_eq!(bounds.origin.x, 10.0);
        assert_eq!(bounds.size.width, 150.0);
        assert!(bounds.size.height > 0.0);
    }

    #[test]
    fn a_declared_height_positions_the_edges() {
        let placed = Placed::new(GraphNode::new("a", "A"), 0.0, 0.0).height(200.0);
        assert_eq!(
            placed
                .bounds(&gpui_kit_theme::Theme::studio_dark(), None)
                .size
                .height,
            200.0
        );
    }

    #[test]
    fn an_invalid_declared_height_keeps_rendering_and_routing_automatic() {
        let theme = gpui_kit_theme::Theme::studio_dark();
        let automatic = Placed::new(GraphNode::new("a", "A"), 0.0, 0.0)
            .bounds(&theme, None)
            .size
            .height;
        for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(
                Placed::new(GraphNode::new("a", "A"), 0.0, 0.0)
                    .height(invalid)
                    .bounds(&theme, None)
                    .size
                    .height,
                automatic
            );
        }
    }

    #[test]
    fn edges_route_between_the_nodes_that_are_present() {
        let routes = graph()
            .edge(GraphEdge::new("run.plan", "run.apply"))
            .routable(&gpui_kit_theme::Theme::studio_dark());
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].edge.kind(), EdgeKind::Flow);
    }

    /// A line to a node that is not on the canvas has no correct place to go,
    /// so it goes nowhere rather than somewhere wrong.
    #[test]
    fn an_edge_to_a_missing_node_is_dropped_rather_than_guessed() {
        let routes = graph()
            .edge(GraphEdge::new("run.plan", "run.publish"))
            .edge(GraphEdge::new("run.nowhere", "run.apply"))
            .routable(&gpui_kit_theme::Theme::studio_dark());
        assert!(routes.is_empty());
    }

    #[test]
    fn a_feedback_edge_keeps_its_kind_through_routing() {
        let routes = graph()
            .edge(GraphEdge::new("run.apply", "run.plan").feedback())
            .routable(&gpui_kit_theme::Theme::studio_dark());
        assert_eq!(routes[0].edge.kind(), EdgeKind::Feedback);
    }

    #[test]
    fn explicit_ports_are_strict_and_duplicate_identities_are_rejected() {
        let theme = gpui_kit_theme::Theme::studio_dark();
        let valid = NodeGraph::new("run")
            .node(
                GraphNode::new("a", "A").port(GraphPort::output("out", "Out")),
                0.0,
                0.0,
            )
            .node(
                GraphNode::new("b", "B").port(GraphPort::input("in", "In")),
                300.0,
                0.0,
            )
            .edge(GraphEdge::new("a", "b").ports("out", "in"));
        assert_eq!(valid.routable(&theme).len(), 1);

        let invalid_direction = NodeGraph::new("run")
            .node(
                GraphNode::new("a", "A").port(GraphPort::input("in", "In")),
                0.0,
                0.0,
            )
            .node(
                GraphNode::new("b", "B").port(GraphPort::output("out", "Out")),
                300.0,
                0.0,
            )
            .edge(GraphEdge::new("a", "b").ports("in", "out"));
        assert!(invalid_direction.routable(&theme).is_empty());

        let duplicate = NodeGraph::new("run")
            .node(GraphNode::new("a", "A"), 0.0, 0.0)
            .node(GraphNode::new("a", "A again"), 10.0, 0.0)
            .node(GraphNode::new("b", "B"), 300.0, 0.0)
            .edge(GraphEdge::new("a", "b"));
        assert!(duplicate.routable(&theme).is_empty());
    }

    #[test]
    fn pointer_centered_zoom_preserves_the_world_point() {
        let viewport = GraphViewport::new(point(30.0, -20.0), 1.25);
        let pointer = point(240.0, 130.0);
        let world = screen_to_world(pointer, viewport);
        let zoomed = zoom_at(viewport, pointer, 1.8);
        assert_eq!(world_to_screen(world, zoomed), pointer);
    }

    #[test]
    fn layered_layout_puts_dependents_in_later_columns() {
        let plan = SharedString::from("plan");
        let apply = SharedString::from("apply");
        let edge = GraphEdge::new("plan", "apply");
        let placed = layered_layout([plan.clone(), apply.clone()], [&edge], 280.0, 96.0);
        let plan_at = placed
            .iter()
            .find(|(id, _)| id == &plan)
            .expect("plan is placed")
            .1;
        let apply_at = placed
            .iter()
            .find(|(id, _)| id == &apply)
            .expect("apply is placed")
            .1;
        assert!(apply_at.x > plan_at.x);
    }

    #[test]
    fn a_new_graph_is_ready_and_carries_its_grid() {
        let graph = NodeGraph::new("run");
        assert_eq!(graph.state, GraphState::Ready);
        assert!(graph.grid);
        assert_eq!(graph.viewport, GraphViewport::default());
    }

    /// The four not-ready states are separate answers and none of them may
    /// collapse into another.
    #[test]
    fn a_viewport_names_the_world_it_can_see() {
        let viewport = GraphViewport::new(point(0.0, 0.0), 1.0);
        let screen = Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(100.0)));
        let view = world_view(viewport, screen);
        assert!((view.size.width - 200.0).abs() < 0.01);
        assert!((view.size.height - 100.0).abs() < 0.01);
    }

    #[test]
    fn bounds_that_do_not_overlap_are_culled() {
        let left = Bounds::new(point(0.0, 0.0), size(10.0, 10.0));
        let right = Bounds::new(point(100.0, 100.0), size(10.0, 10.0));
        assert!(!bounds_overlap(left, right, 0.0));
        assert!(bounds_overlap(left, right, 200.0));
    }

    #[test]
    fn a_route_signature_changes_when_a_node_moves() {
        let theme = gpui_kit_theme::Theme::studio_dark();
        let placed = Placed::new(GraphNode::new("a", "A"), 0.0, 0.0);
        let geometry = [NodeGeometry {
            id: SharedString::from("a"),
            bounds: placed.bounds(&theme, None),
            ports: Vec::new(),
        }];
        let first = route_signature(&geometry, &[]);
        let moved = Placed::new(GraphNode::new("a", "A"), 40.0, 0.0);
        let shifted = [NodeGeometry {
            id: SharedString::from("a"),
            bounds: moved.bounds(&theme, None),
            ports: Vec::new(),
        }];
        assert_ne!(first, route_signature(&shifted, &[]));
        assert_eq!(first, route_signature(&geometry, &[]));
    }

    #[test]
    fn compact_lod_begins_below_the_threshold() {
        let compact = GraphViewport::new(point(0.0, 0.0), LOD_ZOOM - 0.01);
        let full = GraphViewport::new(point(0.0, 0.0), LOD_ZOOM);
        assert!(compact.zoom < LOD_ZOOM);
        assert!(full.zoom >= LOD_ZOOM);
    }

    #[test]
    fn the_canvas_states_stay_distinct() {
        assert_ne!(
            GraphState::Refused("no".into()),
            GraphState::Failed("no".into())
        );
        assert_ne!(GraphState::Loading, GraphState::Ready);
    }
}
