//! The canvas a run is drawn on.
//!
//! The graph owns no layout algorithm. Where a node sits is a product
//! question — a plan graph, a dependency graph and a retry graph want
//! different answers, and none of them belong in a component library — so the
//! caller places every node and this draws what it was given. What the graph
//! does own is the part that is the same every time: the backdrop, the
//! stacking of edges beneath nodes, and the five states a canvas can be in.

use gpui::{
    AnyElement, App, Bounds, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
    SharedString, Styled, Window, canvas, div, point, px, size,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, Surface, TypeScale};

use crate::display::empty::EmptyState;
use crate::foundation::{Ident, StyledExt};
use crate::strings::{ActiveStrings, StringKey};

use super::edge::{EdgeKind, GraphEdge, paint_edge};
use super::node::GraphNode;

/// The spacing of the dot grid behind the canvas, in pixels.
const GRID_STEP: f32 = 24.0;
const GRID_DOT: f32 = 1.0;

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

    /// Overrides the height the node measured for itself. This positions the
    /// edges, not the card: the card is still laid out by its own content.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    fn bounds(&self, theme: &gpui_kit_theme::Theme) -> Bounds<Pixels> {
        let height = self
            .height
            .unwrap_or_else(|| self.node.measured_height(theme));
        Bounds::new(
            point(px(self.x), px(self.y)),
            size(px(self.node.node_width()), px(height)),
        )
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
    /// How far the whole canvas is scrolled, in canvas coordinates.
    offset: (f32, f32),
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
            offset: (0.0, 0.0),
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
        self.offset = (x, y);
        self
    }

    /// The box of every node, by identity, for the edge painter.
    fn boxes(&self, theme: &gpui_kit_theme::Theme) -> Vec<(SharedString, Bounds<Pixels>)> {
        self.nodes
            .iter()
            .map(|placed| (placed.node.ident().semantic_id(), placed.bounds(theme)))
            .collect()
    }

    /// The edges that name two nodes this graph actually has.
    ///
    /// An edge to a node that is not here is dropped rather than guessed at:
    /// a line drawn to the wrong box would report a connection the run does
    /// not have, and there is no correct place to put a line whose end is
    /// missing.
    fn routable(
        &self,
        theme: &gpui_kit_theme::Theme,
    ) -> Vec<(EdgeKind, Bounds<Pixels>, Bounds<Pixels>)> {
        let boxes = self.boxes(theme);
        let find = |id: &SharedString| {
            boxes
                .iter()
                .find(|(other, _)| other == id)
                .map(|(_, bounds)| *bounds)
        };
        self.edges
            .iter()
            .filter_map(|edge| Some((edge.kind, find(&edge.from)?, find(&edge.to)?)))
            .collect()
    }
}

impl RenderOnce for NodeGraph {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let spec = NodeSpec::new(self.ident.semantic_id(), Role::Group)
            .busy(matches!(self.state, GraphState::Loading));

        let frame = div()
            .id(self.ident.element_id())
            .relative()
            .size_full()
            .overflow_hidden()
            .surface(&theme, Surface::Canvas);

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
                    spec.value(match self.state {
                        GraphState::Loading => "loading",
                        GraphState::Refused(_) => "refused",
                        _ => "failed",
                    }),
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
                .semantic_in(cx, spec.value("empty"))
                .into_any_element();
        }

        let (pan_x, pan_y) = self.offset;
        let routes = self.routable(&theme);
        let stroke = theme.borders.hairline;
        let grid_color = theme.colors.hairline;
        let draw_grid = self.grid;
        let edge_theme = theme.clone();

        // Edges and the grid are one painted layer beneath the nodes, so a
        // connection never intercepts a click meant for a card and adding one
        // moves nothing.
        let beneath = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                if draw_grid {
                    paint_grid(window, bounds, pan_x, pan_y, grid_color);
                }
                for (kind, from, to) in routes {
                    let shift = |mut rect: Bounds<Pixels>| {
                        rect.origin.x += bounds.origin.x + px(pan_x);
                        rect.origin.y += bounds.origin.y + px(pan_y);
                        rect
                    };
                    paint_edge(window, &edge_theme, kind, shift(from), shift(to), stroke);
                }
            },
        )
        .absolute()
        .inset_0();

        let cards: Vec<AnyElement> = self
            .nodes
            .into_iter()
            .map(|placed| {
                div()
                    .absolute()
                    .left(px(placed.x + pan_x))
                    .top(px(placed.y + pan_y))
                    .w(px(placed.node.node_width()))
                    .child(placed.node)
                    .into_any_element()
            })
            .collect();

        frame
            .child(beneath)
            .children(cards)
            .semantic_in(cx, spec.value("ready"))
            .into_any_element()
    }
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
    color: gpui::Hsla,
) {
    let first = |pan: f32| -(pan.rem_euclid(GRID_STEP));
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
            x += GRID_STEP;
        }
        y += GRID_STEP;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let bounds = placed.bounds(&gpui_kit_theme::Theme::studio_dark());
        assert_eq!(bounds.origin.x, px(10.0));
        assert_eq!(bounds.size.width, px(150.0));
        assert!(bounds.size.height > px(0.0));
    }

    #[test]
    fn a_declared_height_positions_the_edges() {
        let placed = Placed::new(GraphNode::new("a", "A"), 0.0, 0.0).height(200.0);
        assert_eq!(
            placed
                .bounds(&gpui_kit_theme::Theme::studio_dark())
                .size
                .height,
            px(200.0)
        );
    }

    #[test]
    fn edges_route_between_the_nodes_that_are_present() {
        let routes = graph()
            .edge(GraphEdge::new("run.plan", "run.apply"))
            .routable(&gpui_kit_theme::Theme::studio_dark());
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].0, EdgeKind::Flow);
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
        assert_eq!(routes[0].0, EdgeKind::Feedback);
    }

    #[test]
    fn a_new_graph_is_ready_and_carries_its_grid() {
        let graph = NodeGraph::new("run");
        assert_eq!(graph.state, GraphState::Ready);
        assert!(graph.grid);
        assert_eq!(graph.offset, (0.0, 0.0));
    }

    /// The four not-ready states are separate answers and none of them may
    /// collapse into another.
    #[test]
    fn the_canvas_states_stay_distinct() {
        assert_ne!(
            GraphState::Refused("no".into()),
            GraphState::Failed("no".into())
        );
        assert_ne!(GraphState::Loading, GraphState::Ready);
    }
}
