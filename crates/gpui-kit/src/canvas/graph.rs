//! The canvas a run is drawn on.
//!
//! The graph owns no layout algorithm. Where a node sits is a product
//! question — a plan graph, a dependency graph and a retry graph want
//! different answers, and none of them belong in a component library — so the
//! caller places every node and this draws what it was given. What the graph
//! does own is the part that is the same every time: the backdrop, the
//! stacking of edges beneath nodes, and the five states a canvas can be in.

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use gpui::{
    AnyElement, App, Bounds, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, Point, RenderOnce, ScrollDelta, SharedString, StatefulInteractiveElement, Styled,
    Window, canvas, div, linear_color_stop, linear_gradient_stops, point, prelude::FluentBuilder,
    px, relative, size,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, Elevation, Radius, SemanticWash, Space, Surface, TypeScale, Variant,
};
use web_time::Instant;

use crate::display::empty::EmptyState;
use crate::display::state_view::StateView;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{FocusRing, Ident, Pressable, StyledExt};
use crate::layout::measure;
use crate::motion::{Activity, MotionPolicy, MotionRole, MotionSpec, keyed};
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

use super::band::GraphBand;
use super::edge::{
    Anchor, Axis, EdgeColors, EdgePaint, EdgeState, GraphEdge, GraphEndpoint, GraphRouting,
    OrthogonalRoute, PortSide, RouteTransform, paint_route, paint_route_stroke, route_curved,
    route_curved_preview, route_orthogonal, route_preview,
};
use super::node::{GraphNode, GraphPort, PortDirection};
use super::toolbar::CanvasToolbar;

/// The spacing of the dot grid behind the canvas, in pixels.
const GRID_STEP: f32 = 24.0;
/// Below this zoom a node draws only its title, and ports stay off.
const LOD_ZOOM: f32 = 0.4;
/// Extra world space kept around the viewport so a node entering does not pop.
///
/// Wide enough to cover a whole card, because culling reads a node's box and
/// that box is an estimate until the card has been measured once. A pad
/// narrower than a card lets a node that is really on screen be dropped for
/// the frame in which its own height is still being guessed.
const CULL_PAD: f32 = 240.0;

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
    /// Reports a press that landed on the canvas itself rather than on any
    /// node, with where it landed in world space.
    ///
    /// A caller that puts a new node where the reader pointed needs this, and
    /// only the canvas can answer it: it holds every card's measured box,
    /// while a caller testing the point itself would be re-deriving those
    /// boxes from the positions it handed over and would miss whichever card
    /// came out taller than it assumed.
    SurfacePressed {
        position: Point<f32>,
        button: MouseButton,
        click_count: usize,
    },
    /// Proposes a new output-to-input connection.
    ConnectionRequested {
        from: GraphEndpoint,
        to: GraphEndpoint,
    },
    /// A connection gesture ended on open canvas rather than another port.
    /// The position uses the same canvas coordinates as placed nodes.
    ConnectionDropped { from: GraphEndpoint, at: Point<f32> },
    /// Proposes removing one controlled edge by its stable identity.
    DisconnectRequested { id: SharedString },
}

type EventHandler = Rc<dyn Fn(&NodeGraphEvent, &mut Window, &mut App)>;
type ConnectionValidator = Rc<dyn Fn(&GraphEndpoint, &GraphEndpoint) -> bool>;

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
        direction: PortDirection,
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
    /// The visible colour crossover for each caller-owned edge.
    edge_transitions: HashMap<SharedString, EdgeTransition>,
    /// The fit token this canvas last framed itself for. Kept beside the
    /// gesture because it is the same kind of fact: what this one canvas has
    /// been through, not what the caller asked for.
    framed: Option<u64>,
    /// When each connection this canvas is drawing was first seen, so a new
    /// one can arrive rather than appear.
    ///
    /// The same kind of fact as the two above: what this canvas has been
    /// through. The caller owns which edges exist; only the canvas knows
    /// which of them it has already drawn, and a caller asked to say so would
    /// be keeping a record of the component's own paint history.
    ///
    /// Empty on the first frame, which is deliberate: a canvas opening onto a
    /// graph draws it, and does not animate in every connection at once.
    arrived: HashMap<SharedString, Instant>,
    /// Whether the first frame has been drawn, which is what makes the
    /// distinction above possible.
    opened: bool,
    /// Which sockets have already been seen connected and the short visual
    /// settle for a newly landed connection.
    port_settles: PortSettles,
    /// Where the canvas is looking, while that is somewhere other than where
    /// the caller has put it.
    travel: Travel,
    /// The viewport this canvas last proposed from a direct manipulation.
    ///
    /// A drag and a wheel are the reader moving the canvas with their own
    /// hand, so the canvas must be exactly where they left it on the next
    /// frame; anything else lags the pointer. A frame, a zoom-to, or a
    /// caller restoring a saved position is a jump, and a jump is travelled.
    /// Recording what was proposed is how the two are told apart exactly,
    /// rather than by guessing from how far the viewport moved.
    direct: Option<GraphViewport>,
}

type PortKey = (SharedString, SharedString);

/// Paint history for the one-shot contraction when a connection lands.
///
/// The caller owns the wired set. This state remembers only what this canvas
/// has already shown, just as `arrived` above remembers which edges have
/// already entered. Opening onto existing wiring never replays old work.
#[derive(Debug, Default)]
struct PortSettles {
    previous: HashSet<PortKey>,
    started: HashMap<PortKey, Instant>,
    opened: bool,
}

impl PortSettles {
    /// Returns the remaining expansion for every socket still contracting,
    /// where one is the first frame and zero is settled.
    fn show(
        &mut self,
        wired: &HashSet<PortKey>,
        now: Instant,
        spec: MotionSpec,
        animates: bool,
    ) -> (HashMap<PortKey, f32>, bool) {
        if !self.opened || !animates {
            self.previous = wired.clone();
            self.started.clear();
            self.opened = true;
            return (HashMap::new(), false);
        }

        for key in wired.difference(&self.previous) {
            self.started.insert(key.clone(), now);
        }
        self.previous = wired.clone();
        let span = spec.total().as_secs_f32().max(f32::EPSILON);
        let mut shown = HashMap::new();
        self.started.retain(|key, started| {
            if !wired.contains(key) {
                return false;
            }
            let raw = (now.duration_since(*started).as_secs_f32() / span).clamp(0.0, 1.0);
            if raw >= 1.0 {
                return false;
            }
            shown.insert(key.clone(), 1.0 - spec.progress(raw));
            true
        });
        let animating = !shown.is_empty();
        (shown, animating)
    }
}

#[derive(Debug, Clone, Copy)]
struct EdgeTransition {
    state: EdgeState,
    from: EdgeColors,
    to: EdgeColors,
    started: Option<Instant>,
}

impl EdgeTransition {
    fn settled(state: EdgeState, colors: EdgeColors) -> Self {
        Self {
            state,
            from: colors,
            to: colors,
            started: None,
        }
    }

    fn at(&self, now: Instant, spec: MotionSpec, theme: &gpui_kit_theme::Theme) -> EdgeColors {
        let Some(started) = self.started else {
            return self.to;
        };
        let span = spec.total().as_secs_f32().max(f32::EPSILON);
        let raw = (now.duration_since(started).as_secs_f32() / span).clamp(0.0, 1.0);
        if raw <= 0.0 {
            return self.from;
        }
        if raw >= 1.0 {
            return self.to;
        }
        let progress = spec.progress(raw);
        EdgeColors::new(
            theme.mix(self.from.from, self.to.from, progress),
            theme.mix(self.from.to, self.to.to, progress),
        )
    }

    /// Retargets from the paint visible now rather than an earlier semantic
    /// state, so two rapid changes cannot jump backwards between colours.
    fn show(
        &mut self,
        state: EdgeState,
        target: EdgeColors,
        now: Instant,
        spec: MotionSpec,
        animates: bool,
        theme: &gpui_kit_theme::Theme,
    ) -> (EdgeColors, bool) {
        if !animates {
            *self = Self::settled(state, target);
            return (target, false);
        }
        if self.state != state {
            let visible = self.at(now, spec, theme);
            *self = Self {
                state,
                from: visible,
                to: target,
                started: Some(now),
            };
        } else if self.to != target {
            // A theme change is not an edge-state event. It adopts the new
            // theme directly rather than replaying a traffic transition.
            *self = Self::settled(state, target);
        }
        let visible = self.at(now, spec, theme);
        let animating = self.started.is_some_and(|started| {
            now.duration_since(started).as_secs_f32() < spec.total().as_secs_f32().max(f32::EPSILON)
        });
        if !animating {
            self.from = self.to;
            self.started = None;
        }
        (visible, animating)
    }
}

/// A canvas moving from where it was looking to where it has been asked to
/// look.
#[derive(Debug, Clone, Copy, Default)]
struct Travel {
    from: Option<GraphViewport>,
    to: Option<GraphViewport>,
    started: Option<Instant>,
}

impl Travel {
    /// Where the canvas is looking this frame.
    ///
    /// `snap` is the reader's own hand on the canvas, and arrives instantly.
    /// Everything else is a jump the reader did not make, and a jump that is
    /// not travelled leaves them to work out afterwards which part of the
    /// graph they are now looking at.
    fn shown(
        &mut self,
        asked: GraphViewport,
        snap: bool,
        now: Instant,
        spec: MotionSpec,
    ) -> (GraphViewport, bool) {
        let showing = self.at(now, spec);
        if snap || self.to.is_none() {
            *self = Self {
                from: Some(asked),
                to: Some(asked),
                started: None,
            };
            return (asked, false);
        }
        if self.to != Some(asked) {
            *self = Self {
                from: Some(showing),
                to: Some(asked),
                started: Some(now),
            };
        }
        let showing = self.at(now, spec);
        (showing, showing != asked)
    }

    fn at(&self, now: Instant, spec: MotionSpec) -> GraphViewport {
        let (Some(from), Some(to)) = (self.from, self.to) else {
            return GraphViewport::default();
        };
        let Some(started) = self.started else {
            return to;
        };
        let span = spec.total().as_secs_f32().max(f32::EPSILON);
        let progress = (now.duration_since(started).as_secs_f32() / span).clamp(0.0, 1.0);
        if progress >= 1.0 {
            return to;
        }
        interpolate_viewport(from, to, spec.progress(progress))
    }
}

/// Two viewports blended at `t`.
///
/// The offset travels straight and the scale travels geometrically, because
/// scale is a ratio: halfway between 0.5 and 2.0 is 1.0, and a straight blend
/// would put it at 1.25 and spend most of the journey zoomed further in than
/// either end. That is the difference between a canvas that pulls back to show
/// the reader where they are going and one that lurches.
fn interpolate_viewport(from: GraphViewport, to: GraphViewport, t: f32) -> GraphViewport {
    let blend = |a: f32, b: f32| a + (b - a) * t;
    GraphViewport {
        offset: point(
            blend(from.offset.x, to.offset.x),
            blend(from.offset.y, to.offset.y),
        ),
        zoom: (blend(
            from.zoom.max(f32::EPSILON).ln(),
            to.zoom.max(f32::EPSILON).ln(),
        ))
        .exp(),
    }
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

/// How much of the surface is kept clear when framing, so a card at the edge
/// of the graph does not end up against the edge of the panel.
const FIT_MARGIN: f32 = 48.0;
const GRAPH_MINIMAP_WIDTH: f32 = 140.0;
const GRAPH_MINIMAP_HEIGHT: f32 = 88.0;

#[derive(Debug, Clone, Copy)]
enum FitCorner {
    TopLeft,
    BottomRight,
}

/// One piece of canvas-owned chrome that fitted world content must not sit
/// beneath. The extent includes the chrome's offset from its two edges.
#[derive(Debug, Clone, Copy)]
struct FitObstacle {
    corner: FitCorner,
    width: f32,
    height: f32,
}

impl FitObstacle {
    fn new(corner: FitCorner, width: f32, height: f32) -> Self {
        Self {
            corner,
            width: width.max(0.0),
            height: height.max(0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct FitInsets {
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
}

impl FitInsets {
    /// Excludes the obstacle with a full-height strip on its horizontal side.
    fn beside(mut self, obstacle: FitObstacle) -> Self {
        match obstacle.corner {
            FitCorner::TopLeft => self.left = self.left.max(obstacle.width),
            FitCorner::BottomRight => self.right = self.right.max(obstacle.width),
        }
        self
    }

    /// Excludes the obstacle with a full-width strip on its vertical side.
    fn beyond(mut self, obstacle: FitObstacle) -> Self {
        match obstacle.corner {
            FitCorner::TopLeft => self.top = self.top.max(obstacle.height),
            FitCorner::BottomRight => self.bottom = self.bottom.max(obstacle.height),
        }
        self
    }
}

/// Every rectangular content area that can avoid the corner obstacles. A
/// corner can be cleared along either adjacent edge; trying both lets a wide,
/// shallow toolbar consume height while a tall rail consumes width, without a
/// host choosing insets or the graph hard-coding that policy per component.
fn fit_inset_candidates(obstacles: &[FitObstacle]) -> Vec<FitInsets> {
    obstacles
        .iter()
        .fold(vec![FitInsets::default()], |areas, obstacle| {
            areas
                .into_iter()
                .flat_map(|area| [area.beside(*obstacle), area.beyond(*obstacle)])
                .collect()
        })
}

/// The token to frame for, or `None` when this canvas has already framed for
/// the one the caller is holding. Separated from the drawing so the rule —
/// framing happens once per token, and a reader's own panning never earns
/// another one — can be read and tested without a window.
fn wants_frame(fit: GraphFit, framed: Option<u64>) -> Option<u64> {
    match fit {
        GraphFit::Never => None,
        GraphFit::Whole(token) => (framed != Some(token)).then_some(token),
    }
}

/// The viewport that holds every card, or `None` when there is nothing to hold
/// or nowhere to hold it yet.
fn frame_all(
    nodes: &[NodeGeometry],
    bands: &[GraphBand],
    surface: Bounds<Pixels>,
    zoom_range: (f32, f32),
    obstacles: &[FitObstacle],
) -> Option<GraphViewport> {
    let width = f32::from(surface.size.width);
    let height = f32::from(surface.size.height);
    if width <= FIT_MARGIN || height <= FIT_MARGIN {
        return None;
    }
    let content = nodes
        .iter()
        .map(|node| node.bounds)
        .chain(bands.iter().map(GraphBand::bounds));
    let (min, max) = content.fold(None::<(Point<f32>, Point<f32>)>, |acc, bounds| {
        let low = bounds.origin;
        let high = point(
            bounds.origin.x + bounds.size.width,
            bounds.origin.y + bounds.size.height,
        );
        Some(match acc {
            None => (low, high),
            Some((left, right)) => (
                point(left.x.min(low.x), left.y.min(low.y)),
                point(right.x.max(high.x), right.y.max(high.y)),
            ),
        })
    })?;
    fit_inset_candidates(obstacles)
        .into_iter()
        .filter_map(|insets| {
            let available_width = width - insets.left - insets.right - FIT_MARGIN;
            let available_height = height - insets.top - insets.bottom - FIT_MARGIN;
            if available_width <= 0.0 || available_height <= 0.0 {
                return None;
            }
            let zoom = (available_width / (max.x - min.x).max(1.0))
                .min(available_height / (max.y - min.y).max(1.0))
                .clamp(zoom_range.0, zoom_range.1)
                // Never magnify. A graph of three cards blown up to fill a
                // panel reads as a mistake, and the reader can still zoom in
                // themselves.
                .min(1.0);
            let available_center = point(
                insets.left + (width - insets.left - insets.right) / 2.0,
                insets.top + (height - insets.top - insets.bottom) / 2.0,
            );
            Some(GraphViewport::new(
                point(
                    available_center.x - (min.x + max.x) * zoom / 2.0,
                    available_center.y - (min.y + max.y) * zoom / 2.0,
                ),
                zoom,
            ))
        })
        .max_by(|left, right| {
            left.zoom
                .partial_cmp(&right.zoom)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
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
    /// The one colour that stands for this node where the card is not drawn.
    tint: Hsla,
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
    from: GraphEndpoint,
    direction: PortDirection,
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

impl GraphState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Refused(_) => "refused",
            Self::Failed(_) => "failed",
        }
    }
}

impl HasPhase for GraphState {
    fn phase(&self) -> Phase {
        match self {
            Self::Ready => Phase::Ready,
            Self::Loading => Phase::Loading,
            Self::Refused(_) => Phase::Unavailable,
            Self::Failed(_) => Phase::Error,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Refused(reason) | Self::Failed(reason) => Some(reason.as_ref()),
            _ => None,
        }
    }
}

/// A run drawn as connected steps.
#[derive(IntoElement)]
pub struct NodeGraph {
    ident: Ident,
    nodes: Vec<Placed>,
    edges: Vec<GraphEdge>,
    bands: Vec<GraphBand>,
    state: GraphState,
    empty: Option<EmptyState>,
    slots: Slots,
    grid: bool,
    viewport: GraphViewport,
    zoom_range: (f32, f32),
    interaction: GraphInteraction,
    on_event: Option<EventHandler>,
    can_connect: Option<ConnectionValidator>,
    minimap: bool,
    toolbar: Option<CanvasToolbar>,
    fit: GraphFit,
    routing: GraphRouting,
}

/// Whether a canvas frames its own content before the reader touches it.
///
/// A canvas whose caller computes every position — from a dependency depth, a
/// git lineage, a ledger's layers — is routinely wider than the surface
/// holding it, and the default viewport opens on its top-left corner. The
/// reader's first sight of the graph is then two cards and an edge leaving the
/// frame, with nothing to say that the rest exists.
///
/// The canvas is the only thing that can answer this honestly: it holds every
/// card's measured box and its own surface size, while a caller knows only the
/// positions it handed over and would have to guess how tall a card came out.
/// What stays with the caller is whether framing is wanted at all, because on
/// a canvas the reader arranges, the opening view is theirs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphFit {
    /// The viewport is the caller's from the first frame.
    #[default]
    Never,
    /// Frame every node and world-space band as soon as the cards and
    /// canvas-owned chrome have been laid out, and again each time this value
    /// changes — which is what a caller's own Fit control bumps, since the
    /// caller cannot compute the frame itself. The fitted content clears the
    /// graph's minimap and toolbar rather than remaining technically inside
    /// the viewport underneath them. Reported as an ordinary
    /// [`NodeGraphEvent::ViewportChanged`], so a caller that already stores
    /// its viewport needs nothing else. Between framings the viewport is the
    /// reader's and the canvas never moves it on its own, so a caller that
    /// holds one value is framed exactly once, when the canvas first opens.
    Whole(u64),
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

impl Slotted for NodeGraph {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl NodeGraph {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
            bands: Vec::new(),
            state: GraphState::Ready,
            empty: None,
            slots: Slots::default(),
            grid: true,
            viewport: GraphViewport::default(),
            zoom_range: (0.5, 2.0),
            interaction: GraphInteraction::default(),
            on_event: None,
            can_connect: None,
            minimap: false,
            toolbar: None,
            fit: GraphFit::Never,
            routing: GraphRouting::Lanes,
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

    /// Names a region of the canvas, in the same world coordinates the nodes
    /// are placed in.
    ///
    /// Bands are drawn above the grid and below every connection and card, in
    /// the order they were added, and never intercept a pointer. See
    /// [`GraphBand`].
    pub fn band(mut self, band: GraphBand) -> Self {
        self.bands.push(band);
        self
    }

    pub fn bands(mut self, bands: impl IntoIterator<Item = GraphBand>) -> Self {
        self.bands.extend(bands);
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

    /// Supplies the caller's connection rules so the graph can preview legal
    /// and illegal targets before proposing a connection. Endpoints are
    /// always passed as output then input, including gestures begun at input.
    pub fn can_connect(
        mut self,
        validator: impl Fn(&GraphEndpoint, &GraphEndpoint) -> bool + 'static,
    ) -> Self {
        self.can_connect = Some(Rc::new(validator));
        self
    }

    /// Draws a small overview of the placed nodes in the corner.
    pub fn minimap(mut self, show: bool) -> Self {
        self.minimap = show;
        self
    }

    /// Seats canvas chrome in the graph's top-left overlay layer.
    ///
    /// The graph measures the finished toolbar rather than duplicating its
    /// control geometry. [`GraphFit::Whole`] then keeps world content out from
    /// under it, just as it does for the graph's own minimap. This is the
    /// complete overlay path: paint order and fit geometry cannot disagree,
    /// and the host supplies no pixel inset.
    pub fn toolbar(mut self, toolbar: CanvasToolbar) -> Self {
        self.toolbar = Some(toolbar);
        self
    }

    /// Whether this canvas frames its own content before the reader touches
    /// it. See [`GraphFit`].
    pub fn fit(mut self, fit: GraphFit) -> Self {
        self.fit = fit;
        self
    }

    /// How this canvas draws its connections. See [`GraphRouting`].
    pub fn routing(mut self, routing: GraphRouting) -> Self {
        self.routing = routing;
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
                let tint = placed.node.node_tint(theme);
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
                NodeGeometry {
                    id,
                    bounds,
                    tint,
                    ports,
                }
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
                let route = match self.routing {
                    GraphRouting::Lanes => route_orthogonal(
                        a,
                        b,
                        from.bounds,
                        to.bounds,
                        edge.kind(),
                        edge.edge_lane(),
                    )?,
                    GraphRouting::Curves => route_curved(a, b),
                };
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
    from_direction: PortDirection,
    pointer: Point<f32>,
    viewport: GraphViewport,
    radius: f32,
    can_connect: Option<&ConnectionValidator>,
) -> Option<(GraphEndpoint, bool)> {
    nodes
        .iter()
        .flat_map(|node| node.ports.iter().map(move |port| (node, port)))
        .filter_map(|(node, port)| {
            let at = world_to_screen(port.anchor.point, viewport);
            let distance = (at.x - pointer.x).powi(2) + (at.y - pointer.y).powi(2);
            (distance <= radius.powi(2)).then(|| {
                let endpoint = GraphEndpoint::new(node.id.clone(), port.id.clone());
                let valid_direction = port.direction != from_direction && &endpoint != from;
                let valid = valid_direction
                    && can_connect.is_none_or(|validator| {
                        let (output, input) =
                            normalized_connection(from.clone(), from_direction, endpoint.clone());
                        validator(&output, &input)
                    });
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

fn normalized_connection(
    from: GraphEndpoint,
    from_direction: PortDirection,
    target: GraphEndpoint,
) -> (GraphEndpoint, GraphEndpoint) {
    match from_direction {
        PortDirection::Output => (from, target),
        PortDirection::Input => (target, from),
    }
}

impl RenderOnce for NodeGraph {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut viewport = self.viewport;
        viewport.zoom = viewport.zoom.clamp(self.zoom_range.0, self.zoom_range.1);
        let active_edges =
            matches!(self.state, GraphState::Ready) && self.edges.iter().any(GraphEdge::is_active);
        let graph_busy = matches!(self.state, GraphState::Loading)
            || active_edges
            || (matches!(self.state, GraphState::Ready)
                && self
                    .nodes
                    .iter()
                    .any(|placed| placed.node.node_state().is_busy()));
        let gesture = keyed::slot::<GestureState>(
            &self.ident.semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        // Where the canvas is looking. The caller owns where it has been asked
        // to look; what the canvas owns is that it does not arrive there in
        // one frame when the reader did not move it themselves. A frame or a
        // zoom-to that cut would leave the reader to work out afterwards which
        // part of the graph they are now in front of.
        //
        // Everything downstream reads this rather than the caller's value, so
        // a click during the travel lands on what is under the pointer and a
        // pan started during it continues from where the canvas actually is.
        // What the canvas publishes stays the settled value, whatever it is
        // painting on the way there: motion never changes what a surface
        // reports.
        let asked = viewport;
        let travelling = {
            let travel = MotionPolicy::resolve(MotionRole::Navigation, cx);
            let now = cx.background_executor().now();
            let mut state = gesture.borrow_mut();
            let direct = state.direct.take();
            let snap = state.gesture.is_some() || !travel.animates() || direct == Some(viewport);
            let (shown, travelling) = state.travel.shown(viewport, snap, now, travel.spec());
            viewport = shown;
            travelling
        };
        if travelling {
            window.request_animation_frame();
        }
        let activity = MotionPolicy::resolve(MotionRole::Activity(Activity::Transmitting), cx);
        let edge_flow_phase = if active_edges && activity.animates() {
            let now = cx.background_executor().now();
            let mut state = gesture.borrow_mut();
            let started = *state.animation_started.get_or_insert(now);
            Some(
                (now.duration_since(started).as_secs_f32() / activity.spec().total().as_secs_f32())
                    .rem_euclid(1.0),
            )
        } else {
            gesture.borrow_mut().animation_started = None;
            None
        };
        if edge_flow_phase.is_some() {
            window.request_animation_frame();
        }
        let spec = NodeSpec::new(self.ident.semantic_id(), Role::Group).busy(graph_busy);

        let measured = measure::cell(&self.ident.child("viewport").semantic_id(), window, cx);
        let toolbar_measured = self
            .toolbar
            .as_ref()
            .map(|_| measure::cell(&self.ident.child("toolbar-seat").semantic_id(), window, cx));
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
            // The graph is the floor beneath window chrome, not another pane
            // on the same plane. Sunken is the existing half-step down in the
            // surface scale; top light below gives it depth without a vignette.
            .surface(&theme, Surface::Sunken);

        // Route hover is visual transient state and exists even on an inspect-
        // only graph. The pointer is recorded here and resolved against the
        // exact routes below, after their measured geometry is available.
        let hover_motion = Rc::clone(&gesture);
        frame = frame.on_mouse_move(move |event, window, _| {
            hover_motion.borrow_mut().pointer = Some(event.position);
            window.refresh();
        });
        let leave_motion = Rc::clone(&gesture);
        frame = frame.on_hover(move |hovered, window, _| {
            if !hovered {
                leave_motion.borrow_mut().pointer = None;
                window.refresh();
            }
        });

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
                    let dragged = GraphViewport {
                        offset: point(viewport.offset.x + delta.x, viewport.offset.y + delta.y),
                        zoom: viewport.zoom,
                    };
                    // The reader's own hand: whatever the caller writes back
                    // for this, the canvas is already there.
                    state.direct = Some(dragged);
                    drop(state);
                    move_report(&NodeGraphEvent::ViewportChanged(dragged), window, cx);
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
            // Each card's own box, not one shared guess: a marquee that
            // selected by a fixed rectangle would miss a tall card the reader
            // dragged across and catch a short one they went around.
            let up_nodes = self
                .nodes
                .iter()
                .map(|placed| {
                    (
                        placed.node.ident().semantic_id(),
                        placed.bounds(&theme, None),
                    )
                })
                .collect::<Vec<_>>();
            frame = frame.on_mouse_up(MouseButton::Left, move |event, window, cx| {
                let gesture = up.borrow_mut().gesture.take();
                match gesture {
                    Some(Gesture::Pan { moved: false, .. }) => {
                        up_report(
                            &NodeGraphEvent::SelectionChanged { ids: Vec::new() },
                            window,
                            cx,
                        );
                        // A press that reached this branch never touched a
                        // card, so where it landed is a place on the canvas
                        // and the caller may want to put something there.
                        let frame = up_bounds.get();
                        let at = screen_to_world(
                            point(
                                f32::from(event.position.x - frame.origin.x),
                                f32::from(event.position.y - frame.origin.y),
                            ),
                            viewport,
                        );
                        up_report(
                            &NodeGraphEvent::SurfacePressed {
                                position: at,
                                button: MouseButton::Left,
                                click_count: event.click_count,
                            },
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
                            .filter(|(_, bounds)| bounds_overlap(*bounds, box_bounds, 0.0))
                            .map(|(id, _)| id.clone())
                            .collect();
                        up_report(&NodeGraphEvent::SelectionChanged { ids }, window, cx);
                    }
                    _ => {}
                }
            });
            // The secondary button is not a gesture and is left to travel, so
            // a caller keeps whatever menu it already opens. What the canvas
            // adds is the one fact the caller cannot work out: whether the
            // press was on a card or on the canvas behind it.
            let context_report = Rc::clone(&report);
            let context_bounds = Rc::clone(&measured);
            let context_nodes = self
                .nodes
                .iter()
                .map(|placed| placed.bounds(&theme, None))
                .collect::<Vec<_>>();
            frame = frame.on_mouse_down(MouseButton::Right, move |event, window, cx| {
                let frame = context_bounds.get();
                let at = screen_to_world(
                    point(
                        f32::from(event.position.x - frame.origin.x),
                        f32::from(event.position.y - frame.origin.y),
                    ),
                    viewport,
                );
                if context_nodes.iter().any(|bounds| bounds.contains(&at)) {
                    return;
                }
                context_report(
                    &NodeGraphEvent::SurfacePressed {
                        position: at,
                        button: MouseButton::Right,
                        click_count: event.click_count,
                    },
                    window,
                    cx,
                );
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
                let zoomed = zoom_at(viewport, at, next);
                // The reader turning the wheel is moving the canvas
                // themselves, so it is already where they have put it.
                wheel_gesture.borrow_mut().direct = Some(zoomed);
                wheel_report(&NodeGraphEvent::ViewportChanged(zoomed), window, cx);
            });
        }

        // A canvas that is not ready uses the same complete state surface as
        // every other region. Steps left underneath a failure would read as a
        // run that is still going, so this replaces the canvas body.
        let state_slot = match &self.state {
            GraphState::Loading => Some(slot::LOADING),
            GraphState::Refused(_) => Some(slot::EMPTY),
            GraphState::Failed(_) => Some(slot::FAILED),
            GraphState::Ready => None,
        };
        if let Some(state_slot) = state_slot {
            let state = self.slots.or_else(state_slot, window, cx, |_, _| {
                StateView::new(self.ident.child("state"), self.state.clone()).into_any_element()
            });
            return frame
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .p_token(&theme, Space::Lg)
                        .child(state),
                )
                .semantic_in(cx, spec.value(viewport_value(self.state.name(), asked)))
                .into_any_element();
        }

        if self.nodes.is_empty() {
            let empty = self.slots.or_else(slot::EMPTY, window, cx, |_, _| {
                self.empty
                    .map(IntoElement::into_any_element)
                    .unwrap_or_else(|| {
                        StateView::new(self.ident.child("state"), Phase::Empty).into_any_element()
                    })
            });
            let empty = div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(empty);
            return frame
                .child(empty)
                .semantic_in(cx, spec.value(viewport_value("empty", asked)))
                .into_any_element();
        }

        let node_measurements: HashMap<SharedString, Rc<Cell<Bounds<Pixels>>>> = self
            .nodes
            .iter()
            .map(|placed| {
                let id = placed.node.ident().semantic_id();
                let measurement_id = composite_id("node-measure", &[id.as_ref()]);
                (id, measure::cell(&measurement_id, window, cx))
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
        let route_cell = keyed::slot::<RouteCache>(
            &self.ident.child("routes").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
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
        // Read out rather than borrowed in place: a borrow held in an `if let`
        // scrutinee lives as long as the block, and the block takes the same
        // cell mutably to record what it framed.
        let already_framed = gesture.borrow().framed;
        let pending_frame = wants_frame(self.fit, already_framed);
        // Waiting for real heights is the whole point: framing from the
        // estimate would leave the tallest card half outside the frame it was
        // supposed to guarantee. Every card has to be measured, not just one —
        // the ones a frame is most likely to leave out are exactly the far ones
        // the opening viewport never drew.
        let every_card_measured = geometry
            .iter()
            .all(|node| measured_heights.contains_key(&node.id));
        let overlay_offset = theme.space(Space::Sm);
        let every_overlay_measured = toolbar_measured.as_ref().is_none_or(|measured| {
            let size = measured.get().size;
            size.width > px(0.0) && size.height > px(0.0)
        });
        let mut fit_obstacles = Vec::new();
        if let Some(measured) = &toolbar_measured {
            let size = measured.get().size;
            fit_obstacles.push(FitObstacle::new(
                FitCorner::TopLeft,
                f32::from(size.width) + overlay_offset,
                f32::from(size.height) + overlay_offset,
            ));
        }
        if self.minimap {
            fit_obstacles.push(FitObstacle::new(
                FitCorner::BottomRight,
                GRAPH_MINIMAP_WIDTH + overlay_offset,
                GRAPH_MINIMAP_HEIGHT + overlay_offset,
            ));
        }
        if let Some(token) = pending_frame
            && every_card_measured
            && every_overlay_measured
            && let Some(framed) = frame_all(
                &geometry,
                &self.bands,
                measured.get(),
                self.zoom_range,
                &fit_obstacles,
            )
            && let Some(report) = self.on_event.as_ref().cloned()
        {
            gesture.borrow_mut().framed = Some(token);
            // Reported rather than applied: the viewport belongs to the caller
            // and this is the same proposal a pan or a wheel makes. Deferred
            // because a caller answers it by writing its own state, which it
            // cannot do in the middle of being drawn.
            window.defer(cx, move |window, cx| {
                report(&NodeGraphEvent::ViewportChanged(framed), window, cx);
            });
        }
        let visible_ids: std::collections::HashSet<SharedString> = geometry
            .iter()
            .filter(|node| {
                // A card outside the opening view is never drawn, so it is
                // never measured, so a frame waiting on measurements would
                // wait forever for the very cards it exists to bring in. While
                // a frame is owed, everything is drawn once.
                pending_frame.is_some()
                    || view
                        .map(|view| bounds_overlap(node.bounds, view, CULL_PAD))
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
        let hovered_edge = {
            let state = gesture.borrow();
            match (&state.gesture, state.pointer) {
                (Some(Gesture::Connect { .. }), _) | (_, None) => None,
                (_, Some(pointer)) => {
                    let bounds = measured.get();
                    let local = point(
                        f32::from(pointer.x - bounds.origin.x),
                        f32::from(pointer.y - bounds.origin.y),
                    );
                    let world = screen_to_world(local, viewport);
                    let threshold = 7.0 / viewport.zoom.max(f32::EPSILON);
                    routes
                        .iter()
                        .map(|routed| (routed.route.distance_to(world), routed.edge.edge_id()))
                        .filter(|(distance, _)| *distance <= threshold)
                        .min_by(|left, right| {
                            left.0
                                .partial_cmp(&right.0)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(_, id)| id)
                }
            }
        };
        let state_change = MotionPolicy::resolve(MotionRole::StateChange, cx);
        let edge_colors: HashMap<SharedString, EdgeColors> = {
            let now = cx.background_executor().now();
            let live: HashSet<SharedString> = self.edges.iter().map(GraphEdge::edge_id).collect();
            let mut gesture = gesture.borrow_mut();
            gesture.edge_transitions.retain(|id, _| live.contains(id));
            let mut animating = false;
            let colors = self
                .edges
                .iter()
                .map(|edge| {
                    let id = edge.edge_id();
                    let target = edge.edge_state().colors(edge.kind(), &theme);
                    let transition = gesture
                        .edge_transitions
                        .entry(id.clone())
                        .or_insert_with(|| EdgeTransition::settled(edge.edge_state(), target));
                    let (colors, crossing) = transition.show(
                        edge.edge_state(),
                        target,
                        now,
                        state_change.spec(),
                        state_change.animates(),
                        &theme,
                    );
                    animating |= crossing;
                    (id, colors)
                })
                .collect();
            drop(gesture);
            if animating {
                window.request_animation_frame();
            }
            colors
        };
        let preview = {
            let state = gesture.borrow();
            match (&state.gesture, state.pointer) {
                (Some(Gesture::Connect { from, direction }), Some(pointer)) => {
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
                            route: match self.routing {
                                GraphRouting::Lanes => route_preview(source.anchor, world),
                                GraphRouting::Curves => route_curved_preview(source.anchor, world),
                            },
                            from: from.clone(),
                            direction: *direction,
                            target: connection_target(
                                &geometry,
                                from,
                                *direction,
                                pointer,
                                viewport,
                                (14.0 * viewport.zoom).max(10.0),
                                self.can_connect.as_ref(),
                            ),
                        }
                    })
                }
                _ => None,
            }
        };
        let stroke = theme.borders.hairline;
        let grid_paint = GridPaint {
            minor: theme.colors.node.grid,
            major: theme.colors.node.grid_strong,
            axis: theme.colors.node.grid_axis,
            dot: theme.borders.hairline,
        };
        let draw_grid = self.grid;
        let edge_theme = theme.clone();
        // How far each connection has got into arriving. A connection the
        // canvas has drawn before is simply there; one it has not is drawn
        // from the port it leaves to the port it reaches, over the same
        // entrance the rest of the library arrives on.
        let arrival = MotionPolicy::resolve(MotionRole::Entrance, cx);
        let reveals: HashMap<SharedString, f32> = {
            let now = cx.background_executor().now();
            let mut state = gesture.borrow_mut();
            let opened = state.opened;
            state.opened = true;
            // Every edge the caller declared, not the ones that survived
            // culling: a connection panned off screen and back has already
            // been drawn, and treating it as new would animate it in every
            // time it returned.
            let live: HashSet<SharedString> = self.edges.iter().map(GraphEdge::edge_id).collect();
            state.arrived.retain(|id, _| live.contains(id));
            let span = arrival.spec().total().as_secs_f32().max(f32::EPSILON);
            live.into_iter()
                .map(|id| {
                    let born = *state.arrived.entry(id.clone()).or_insert(now);
                    // A canvas opening onto a graph draws it rather than
                    // animating every connection in at once, and reduced
                    // motion settles the same way.
                    let reveal = if !opened || !arrival.animates() {
                        1.0
                    } else {
                        (now.duration_since(born).as_secs_f32() / span).clamp(0.0, 1.0)
                    };
                    (id, reveal)
                })
                .collect()
        };
        let arriving = reveals.values().any(|reveal| *reveal < 1.0);
        if arriving {
            window.request_animation_frame();
        }
        let painted_routes: Vec<(RoutedEdge, f32)> = routes
            .iter()
            .map(|routed| {
                let reveal = reveals.get(&routed.edge.edge_id()).copied().unwrap_or(1.0);
                (routed.clone(), reveal)
            })
            .collect();
        let painted_preview = preview.clone();
        let ground_light = linear_gradient_stops(
            180.0,
            [
                linear_color_stop(
                    theme.colors.white_fill.opacity(theme.effects.sheen_alpha),
                    0.0,
                ),
                linear_color_stop(gpui::transparent_black(), 0.38),
                linear_color_stop(gpui::transparent_black(), 1.0),
            ],
        );

        // The ground the canvas stands on, under everything the caller put
        // there. The only lighting is a top-origin material cast: no corner or
        // edge is darkened, because that would imply depth the graph does not
        // contain. The grid remains a painted child and intercepts nothing.
        let ground = div().absolute().inset_0().bg(ground_light).child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    if draw_grid {
                        paint_grid(window, bounds, viewport, grid_paint);
                    }
                },
            )
            .absolute()
            .inset_0(),
        );

        // Edges are their own painted layer above the regions and below the
        // cards: a connection crosses a region it does not belong to, and a
        // region drawn over its own connections would hide them.
        let beneath = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let transform = RouteTransform::new(bounds.origin, viewport.offset, viewport.zoom);
                for (routed, reveal) in painted_routes {
                    let id = routed.edge.edge_id();
                    let colors = edge_colors.get(&id).copied().unwrap_or_else(|| {
                        routed
                            .edge
                            .edge_state()
                            .colors(routed.edge.kind(), &edge_theme)
                    });
                    paint_route(
                        window,
                        &edge_theme,
                        &routed.edge,
                        &routed.route,
                        transform,
                        EdgePaint::new(stroke, colors)
                            .reveal(reveal)
                            .phase(routed.edge.is_active().then_some(edge_flow_phase).flatten())
                            .hovered(hovered_edge.as_ref() == Some(&id)),
                    );
                }
                if let Some(preview) = painted_preview {
                    // A proposal that has found a port is drawn as the
                    // connection it would become; one still crossing open
                    // canvas is drawn as the provisional thing it is. The
                    // dashes are the same vocabulary a return path uses, for
                    // the same reason: this line is not an ordinary flow.
                    let (color, dashes) = match preview.target {
                        Some((_, true)) => (edge_theme.colors.success, None),
                        Some((_, false)) => (edge_theme.colors.danger, None),
                        None => (
                            edge_theme.colors.accent,
                            Some([px(PREVIEW_DASH), px(PREVIEW_GAP)]),
                        ),
                    };
                    let connecting = preview.target.is_some_and(|(_, legal)| legal);
                    paint_route_stroke(
                        window,
                        &preview.route,
                        transform,
                        stroke * if connecting { 2.0 } else { 1.5 },
                        color.opacity(if connecting {
                            edge_theme.effects.node_active_stroke_alpha
                        } else {
                            edge_theme.effects.node_preview_alpha
                        }),
                        dashes,
                        preview.route.corner(&edge_theme),
                    );
                    // The head of the proposal, so the reader's own gesture
                    // has a mark on the canvas rather than only a line
                    // trailing off the pointer.
                    let head = preview.route.sample(1.0);
                    let head = point(
                        bounds.origin.x + px(head.x * viewport.zoom + viewport.offset.x),
                        bounds.origin.y + px(head.y * viewport.zoom + viewport.offset.y),
                    );
                    let radius = px(PREVIEW_HEAD * viewport.zoom.clamp(0.5, 1.5));
                    for step in (1..=3).rev() {
                        let halo = radius * (1.0 + step as f32 * 0.38);
                        window.paint_quad(gpui::fill(
                            Bounds::new(
                                point(head.x - halo, head.y - halo),
                                size(halo * 2.0, halo * 2.0),
                            ),
                            color.opacity(
                                edge_theme.effects.node_active_wash_alpha / (step as f32 + 1.5),
                            ),
                        ));
                    }
                    window.paint_quad(gpui::fill(
                        Bounds::new(
                            point(head.x - radius, head.y - radius),
                            size(radius * 2.0, radius * 2.0),
                        ),
                        color.opacity(edge_theme.effects.node_active_stroke_alpha),
                    ));
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
                    .rounded(px(theme.radius(Radius::Small) * viewport.zoom))
                    .bg(theme.colors.node.label_wash)
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
                let description: SharedString =
                    format!("{relation}; state {}", routed.edge.edge_state().value()).into();
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
                        // At rest this control is the quietest mark on the
                        // canvas: the glyph alone, at the faintest tone. It
                        // stays present rather than appearing on hover,
                        // because a control nobody can see until they are
                        // already on it is a control that cannot be found or
                        // tabbed to — but the ring and the fill it used to
                        // wear at rest made a filled circle the loudest thing
                        // at the midpoint of every edge, and a graph of ten
                        // connections was read as ten buttons. The chip
                        // assembles itself under the pointer, where it is
                        // about to be used.
                        .cursor_pointer()
                        .tab_index(0)
                        .focus_ring(&theme)
                        .pressable(cx)
                        .hover(|style| {
                            style
                                .bg(theme.colors.hover)
                                .shadow(theme.glow(theme.colors.accent))
                        })
                        .child(
                            icon(Icon::Close)
                                .size(px(9.0 * viewport.zoom))
                                .text_color(theme.colors.text_faint),
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
                                .description(description)
                                .selected(routed.edge.is_selected())
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
                                .description(description)
                                .selected(routed.edge.is_selected())
                                .value(id),
                        )
                        .into_any_element()
                }
            })
            .collect();

        // Which ports are wired is something the graph already knows and used
        // to throw away. A port that is named by an edge and a port that is
        // waiting for one are the two states a reader is looking for when they
        // open a graph editor at all.
        let wired: HashSet<(SharedString, SharedString)> = self
            .edges
            .iter()
            .flat_map(|edge| {
                [
                    edge.source_port()
                        .map(|port| (edge.from().clone(), port.clone())),
                    edge.target_port()
                        .map(|port| (edge.to().clone(), port.clone())),
                ]
            })
            .flatten()
            .collect();
        let feedback = MotionPolicy::resolve(MotionRole::Feedback, cx);
        let (port_settles, ports_animating) = gesture.borrow_mut().port_settles.show(
            &wired,
            cx.background_executor().now(),
            feedback.spec(),
            feedback.animates(),
        );
        if ports_animating {
            window.request_animation_frame();
        }

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
                let candidate = preview.as_ref().and_then(|preview| {
                    (endpoint != preview.from).then(|| {
                        port.direction() != preview.direction
                            && self.can_connect.as_ref().is_none_or(|validator| {
                                let (output, input) = normalized_connection(
                                    preview.from.clone(),
                                    preview.direction,
                                    endpoint.clone(),
                                );
                                validator(&output, &input)
                            })
                    })
                });
                let connected = wired.contains(&(node.id.clone(), port_geometry.id.clone()));
                let contraction = port_settles
                    .get(&(node.id.clone(), port_geometry.id.clone()))
                    .copied()
                    .unwrap_or(0.0);
                // A wired port wears the connection's own colour and a free
                // one stays neutral, so "what is already joined up" is read
                // off the ports without tracing a single wire. The outer wash
                // and inner bead differ in area as well as hue, keeping them
                // apart without drawing an outline round either one.
                let color = match target {
                    Some(true) => theme.colors.success,
                    Some(false) => theme.colors.danger,
                    None if candidate == Some(true) => theme.colors.success,
                    None if connected => theme.colors.node.port_connected,
                    None => theme.colors.node.port_idle,
                };
                let emphatic = target.is_some() || candidate == Some(true) || connected;
                let outer = theme.color_wash(
                    color,
                    if emphatic {
                        SemanticWash::Standard
                    } else {
                        SemanticWash::Faint
                    },
                );
                let inner_diameter = diameter * if emphatic { 0.48 } else { 0.34 };
                let settle = (contraction > 0.0).then(|| {
                    let size = diameter * (1.0 + contraction * 0.9);
                    let offset = (size - diameter) * 0.5;
                    div()
                        .absolute()
                        .left(px(-offset))
                        .top(px(-offset))
                        .size(px(size))
                        .rounded_full()
                        .bg(color.opacity(theme.effects.semantic_wash_faint_alpha * contraction))
                });
                // A port's name answers "what would I be joining to", which is
                // a question asked while reaching for it and at no other time.
                // Held open, one name per port turns the space between cards
                // into a field of words with no card to belong to — and the
                // ports a reader is not reaching for outnumber the one they
                // are by every other port on the board. So the name comes back
                // on the node the reader has picked, while a wire is being
                // dragged anywhere, or under the pointer.
                let named = placed.node.node_selected() || preview.is_some();
                let port_group = SharedString::from(format!("{semantic_id}-name"));
                // The chip is what keeps a port name off the wire that runs
                // under it: without clearance either side the stroke touches
                // the letterforms and the two read as one mark.
                let wash = theme.colors.node.label_wash;
                let ink = theme.colors.text_muted;
                let label = div()
                    .map(|element| {
                        if named {
                            element.bg(wash).text_color(ink)
                        } else {
                            // Withheld by colour rather than by leaving the
                            // chip out, because a name that only exists once
                            // the pointer is on the port cannot be laid out in
                            // time to be under it.
                            element
                                .bg(gpui::transparent_black())
                                .text_color(gpui::transparent_black())
                                .group_hover(port_group.clone(), move |style| {
                                    style.bg(wash).text_color(ink)
                                })
                        }
                    })
                    .absolute()
                    .whitespace_nowrap()
                    .px(px(theme.spacing.xs * viewport.zoom))
                    .rounded(px(theme.radius(Radius::Small) * viewport.zoom))
                    .text_size(px(theme.typography.caption.size * viewport.zoom))
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
                    .group(port_group)
                    .absolute()
                    .left(px(at.x - diameter / 2.0))
                    .top(px(at.y - diameter / 2.0))
                    .w(px(diameter))
                    .h(px(diameter))
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(outer)
                    .when(emphatic, |element| element.shadow(theme.glow(color)))
                    .when(candidate == Some(false) && target.is_none(), |element| {
                        element.opacity(theme.opacity.disabled)
                    })
                    // Hover strengthens the same material hierarchy rather
                    // than drawing a third outline language around it.
                    .hover(|style| {
                        style
                            .bg(theme
                                .color_wash(theme.colors.node.port_hover, SemanticWash::Strong))
                            .shadow(theme.glow(theme.colors.node.port_hover))
                    })
                    .children(settle)
                    .child(div().size(px(inner_diameter)).rounded_full().bg(color))
                    .child(label);
                if editable {
                    view = view.cursor_pointer();
                }
                if editable {
                    let down = Rc::clone(&gesture);
                    let from = endpoint.clone();
                    let from_direction = port.direction();
                    view = view.on_mouse_down_with_pointer_capture(
                        MouseButton::Left,
                        move |event, window, cx| {
                            let mut state = down.borrow_mut();
                            state.pointer = Some(event.position);
                            state.gesture = Some(Gesture::Connect {
                                from: from.clone(),
                                direction: from_direction,
                            });
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
                    let can_connect = self.can_connect.clone();
                    let target_bounds = Rc::clone(&measured);
                    view = view.on_mouse_up(MouseButton::Left, move |event, window, cx| {
                        let mut state = up.borrow_mut();
                        if let Some(Gesture::Connect { from, direction }) = state.gesture.take() {
                            let bounds = target_bounds.get();
                            let pointer = point(
                                f32::from(event.position.x - bounds.origin.x),
                                f32::from(event.position.y - bounds.origin.y),
                            );
                            let target = connection_target(
                                &candidates,
                                &from,
                                direction,
                                pointer,
                                viewport,
                                (14.0 * viewport.zoom).max(10.0),
                                can_connect.as_ref(),
                            );
                            if let Some(report) = &report {
                                let opposite = target.as_ref().is_some_and(|(target, _)| {
                                    candidates
                                        .iter()
                                        .find(|node| node.id == target.node)
                                        .and_then(|node| {
                                            node.ports.iter().find(|port| port.id == target.port)
                                        })
                                        .is_some_and(|port| port.direction != direction)
                                });
                                match (target, opposite) {
                                    (Some((to, _)), true) => {
                                        let (from, to) = normalized_connection(from, direction, to);
                                        report(
                                            &NodeGraphEvent::ConnectionRequested { from, to },
                                            window,
                                            cx,
                                        );
                                    }
                                    (None, _) => report(
                                        &NodeGraphEvent::ConnectionDropped {
                                            from,
                                            at: screen_to_world(pointer, viewport),
                                        },
                                        window,
                                        cx,
                                    ),
                                    _ => {}
                                }
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
            .then(|| graph_minimap(&self.ident, &geometry, view, &theme, cx));
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
                        .bg(theme.color_wash(theme.colors.accent, SemanticWash::Faint))
                        .shadow(theme.glow(theme.colors.accent))
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
                            .rounded(px(theme.radius(Radius::Small)))
                            .bg(theme.color_wash(theme.colors.accent, SemanticWash::Faint))
                            .shadow(theme.glow(theme.colors.accent))
                            .into_any_element(),
                    )
                }
                _ => None,
            }
        };
        // Region bands, in the order the caller declared them. Each is a
        // world rectangle, so it pans and zooms with the cards it encloses;
        // its name is not, because a caption that shrank with the zoom would
        // stop being readable exactly when the reader zoomed out to take in
        // the regions. Nothing here is interactive: every gesture crossing a
        // band reaches the canvas underneath.
        let bands: Vec<AnyElement> = self
            .bands
            .iter()
            .filter(|band| {
                view.map(|view| bounds_overlap(band.bounds(), view, CULL_PAD))
                    .unwrap_or(true)
            })
            .map(|band| {
                let colors = band
                    .color
                    .as_ref()
                    .map(|color| theme.variant_colors(Variant::Light, color));
                let origin = world_to_screen(band.bounds().origin, viewport);
                // A region is the plane the cards stand on, not a card. The
                // wash is the strength a chart fills an area at, so a card
                // inside a region still reads as the loudest thing in it.
                let wash = colors.map_or(theme.colors.sunken, |colors| {
                    colors.background.opacity(theme.effects.area_wash_alpha)
                });
                let wash = if band.selected {
                    theme.color_wash(theme.colors.accent, SemanticWash::Standard)
                } else {
                    wash
                };
                div()
                    .absolute()
                    .left(px(origin.x))
                    .top(px(origin.y))
                    .w(px(band.bounds().size.width * viewport.zoom))
                    .h(px(band.bounds().size.height * viewport.zoom))
                    .rounded(px(theme.radius(Radius::Card)))
                    .bg(wash)
                    .when(band.selected, |element| {
                        element.shadow(theme.glow(theme.colors.accent))
                    })
                    .child(
                        div()
                            .absolute()
                            .left(px(theme.space(Space::Xs)))
                            .top(px(theme.space(Space::Xs)))
                            .px_token(&theme, Space::Xs)
                            .radius(&theme, Radius::Small)
                            .bg(theme.colors.node.label_wash)
                            .type_scale(&theme, TypeScale::Caption)
                            .text_color(if band.selected {
                                theme.colors.text
                            } else {
                                theme.colors.text_muted
                            })
                            .child(band.label.clone()),
                    )
                    .semantic_in(
                        cx,
                        NodeSpec::new(band.ident.semantic_id(), Role::Group)
                            .text(band.label.clone())
                            .selected(band.selected),
                    )
                    .into_any_element()
            })
            .collect();

        let toolbar = self.toolbar.map(|toolbar| {
            let measured = toolbar_measured.expect("a toolbar seat has a measurement");
            div()
                .on_children_prepainted(move |bounds, window, _| {
                    if let Some(first) = bounds.first() {
                        measure::record(&measured, *first, window);
                    }
                })
                .absolute()
                .top(px(overlay_offset))
                .left(px(overlay_offset))
                .child(toolbar)
                .into_any_element()
        });

        frame
            .child(ground)
            .children(if compact { Vec::new() } else { bands })
            .child(beneath)
            .children(if compact { Vec::new() } else { edge_labels })
            .children(group)
            .children(cards)
            .children(ports)
            .children(edge_nodes)
            .children(marquee)
            .children(overview)
            .children(toolbar)
            .semantic_in(cx, spec.value(viewport_value("ready", asked)))
            .into_any_element()
    }
}

fn graph_minimap(
    ident: &Ident,
    geometry: &[NodeGeometry],
    view: Option<Bounds<f32>>,
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
                        .rounded(px(theme.radius(Radius::Small) * 0.5))
                        // A mark carries the node's own colour, so the
                        // overview is the same graph seen small rather than a
                        // second diagram a reader has to map back by position.
                        .bg(node.tint.opacity(theme.effects.node_minimap_alpha))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // Without this rectangle the overview says where the nodes are but not
    // where the reader is, which is the one question a minimap exists to
    // answer.
    let indicator = world.zip(view).map(|((min, max), view)| {
        let width = (max.x - min.x).max(1.0);
        let height = (max.y - min.y).max(1.0);
        let x = ((view.origin.x - min.x) / width).clamp(0.0, 1.0);
        let y = ((view.origin.y - min.y) / height).clamp(0.0, 1.0);
        div()
            .absolute()
            .left(relative(x))
            .top(relative(y))
            .w(relative((view.size.width / width).clamp(0.04, 1.0 - x)))
            .h(relative((view.size.height / height).clamp(0.04, 1.0 - y)))
            .radius(theme, Radius::Small)
            .bg(theme
                .colors
                .accent
                .opacity(theme.effects.semantic_wash_alpha))
    });
    div()
        .id(ident.element_id())
        .absolute()
        .right(px(theme.space(Space::Sm)))
        .bottom(px(theme.space(Space::Sm)))
        .w(px(GRAPH_MINIMAP_WIDTH))
        .h(px(GRAPH_MINIMAP_HEIGHT))
        .radius(theme, Radius::Small)
        .surface(theme, Surface::Overlay)
        .elevation(theme, Elevation::Raised)
        .overflow_hidden()
        .children(marks)
        .children(indicator)
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Status)
                .text(cx.strings().text(StringKey::GraphMinimap))
                .value("minimap"),
        )
        .into_any_element()
}

/// The paints one canvas rules itself in.
#[derive(Debug, Clone, Copy)]
struct GridPaint {
    minor: Hsla,
    major: Hsla,
    axis: Hsla,
    dot: f32,
}

/// Every dot the same weight is a texture, not a grid: it says the canvas has
/// a surface but not how far anything has been dragged. Marking one interval
/// in a heavier dot gives the pan a ruler to move against.
const MAJOR: i32 = 5;
/// The closest two dots may sit before the grid stops being a ruler and
/// becomes a fill.
const GRID_MIN_SPACING: f32 = 15.0;
/// How far above that floor the finest level is fully drawn, as a share of the
/// floor.
///
/// The fade exists to take a level out before it is replaced, so it belongs at
/// the bottom of the band rather than across it. Spread over the whole band —
/// which is five times as wide — the level a canvas actually sits at is only a
/// fifth drawn, and the grid a reader sees at rest is the major interval on
/// its own.
const GRID_FADE_SPAN: f32 = 0.6;
/// How wide the axis rules are drawn, in device pixels.
const AXIS_WIDTH: f32 = 1.0;
/// The dash and gap of a connection proposal that has not found a port.
const PREVIEW_DASH: f32 = 6.0;
const PREVIEW_GAP: f32 = 5.0;
/// The radius of the mark at the head of a connection proposal.
const PREVIEW_HEAD: f32 = 4.0;

/// The world step the grid rules itself at, and how far the finest level has
/// faded in.
///
/// A grid of one fixed world step is a grid for one zoom. Multiplied by the
/// zoom, as this was, its dots close to a smear on the way out and spread to
/// nothing on the way in — and both are the same failure, which is that the
/// interval stopped being something a reader can count. The step climbs by
/// the major interval instead, so whatever the zoom the dots are a countable
/// distance apart and the heavier dot is still five of them.
///
/// The level changes at a zoom, and a level that appeared would pop, so the
/// finest one fades across the band it lives in and is gone by the moment it
/// would have been replaced. The heavier interval never fades: it is the one
/// that becomes the next level's dots.
fn grid_level(zoom: f32) -> (f32, f32) {
    let mut world = GRID_STEP;
    let major = MAJOR as f32;
    while world * zoom < GRID_MIN_SPACING {
        world *= major;
    }
    while world * zoom >= GRID_MIN_SPACING * major {
        world /= major;
    }
    let spacing = world * zoom;
    let fade = ((spacing - GRID_MIN_SPACING) / (GRID_MIN_SPACING * GRID_FADE_SPAN)).clamp(0.0, 1.0);
    (world, fade)
}

/// Paints the dot grid the canvas sits on, and the two rules through its
/// origin.
///
/// The grid is anchored to the pan offset rather than to the viewport, so it
/// travels with the graph and reports that the canvas moved. A grid pinned to
/// the viewport would sit still under a graph that was moving, which reads as
/// the graph having stayed where it was.
///
/// The axes are where the origin is. Every interval of a grid looks like every
/// other one, so a grid alone says how far the canvas has been dragged and
/// never where the reader has arrived; the axes are the one place on the
/// canvas that is somewhere in particular.
fn paint_grid(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    viewport: GraphViewport,
    paint: GridPaint,
) {
    let (world_step, fade) = grid_level(viewport.zoom);
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let screen = |world: f32, offset: f32| world * viewport.zoom + offset;
    // The first ruled world coordinate at or before the visible edge, so a
    // dot half off screen is still drawn where it belongs.
    let first = |offset: f32| ((-offset / viewport.zoom) / world_step).floor() * world_step;

    let minor = paint.minor.opacity(fade);
    // As the finest level fades out, the heavier one is about to become the
    // finest, so it settles onto the finest level's own weight on the way. A
    // major dot that stayed heavy right up to the switch and then became a
    // minor dot at the same place would change weight in one frame, which is
    // the pop the fade exists to avoid.
    let major = gpui::Hsla {
        a: paint.minor.a + (paint.major.a - paint.minor.a) * fade,
        ..paint.major
    };
    let dot = paint.dot;
    let major_dot = dot * (1.0 + 0.6 * fade);
    let mut world_y = first(viewport.offset.y);
    while screen(world_y, viewport.offset.y) < height {
        let y = screen(world_y, viewport.offset.y);
        let row_major = ((world_y / world_step).round() as i32).rem_euclid(MAJOR) == 0;
        let mut world_x = first(viewport.offset.x);
        while screen(world_x, viewport.offset.x) < width {
            let x = screen(world_x, viewport.offset.x);
            let column_major = ((world_x / world_step).round() as i32).rem_euclid(MAJOR) == 0;
            let heavy = row_major && column_major;
            if y >= 0.0 && x >= 0.0 && (heavy || fade > 0.0) {
                let size_px = if heavy { major_dot } else { dot };
                window.paint_quad(gpui::fill(
                    Bounds::new(
                        point(bounds.origin.x + px(x), bounds.origin.y + px(y)),
                        size(px(size_px), px(size_px)),
                    ),
                    if heavy { major } else { minor },
                ));
            }
            world_x += world_step;
        }
        world_y += world_step;
    }

    let origin_x = viewport.offset.x;
    let origin_y = viewport.offset.y;
    if (0.0..width).contains(&origin_x) {
        window.paint_quad(gpui::fill(
            Bounds::new(
                point(bounds.origin.x + px(origin_x), bounds.origin.y),
                size(px(AXIS_WIDTH), bounds.size.height),
            ),
            paint.axis,
        ));
    }
    if (0.0..height).contains(&origin_y) {
        window.paint_quad(gpui::fill(
            Bounds::new(
                point(bounds.origin.x, bounds.origin.y + px(origin_y)),
                size(bounds.size.width, px(AXIS_WIDTH)),
            ),
            paint.axis,
        ));
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

    fn geometry_at(boxes: &[(f32, f32, f32, f32)]) -> Vec<NodeGeometry> {
        boxes
            .iter()
            .enumerate()
            .map(|(index, (x, y, width, height))| NodeGeometry {
                id: format!("node.{index}").into(),
                bounds: Bounds::new(point(*x, *y), size(*width, *height)),
                ports: Vec::new(),
                tint: gpui_kit_theme::Theme::studio_dark().colors.accent,
            })
            .collect()
    }

    fn surface(width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(height)))
    }

    fn travel_spec() -> MotionSpec {
        MotionPolicy::spec(
            MotionRole::Navigation,
            &gpui_kit_theme::Theme::studio_dark(),
        )
    }

    /// Edge state is caller-owned and may change more quickly than its visual
    /// crossover. A second change must leave from the paint actually visible,
    /// while reduced motion must make the latest state true immediately.
    #[test]
    fn edge_state_changes_retarget_without_jumping_and_reduce_to_the_truth() {
        let theme = gpui_kit_theme::Theme::studio_dark();
        let spec = MotionPolicy::spec(MotionRole::StateChange, &theme);
        let start = Instant::now();
        let idle = EdgeState::Idle.colors(EdgeKind::Flow, &theme);
        let active = EdgeState::Active.colors(EdgeKind::Flow, &theme);
        let failed = EdgeState::Failed.colors(EdgeKind::Flow, &theme);
        let mut transition = EdgeTransition::settled(EdgeState::Idle, idle);

        let (paint, animating) =
            transition.show(EdgeState::Active, active, start, spec, true, &theme);
        assert_eq!(paint, idle);
        assert!(animating);

        let interrupted = start + spec.total() / 2;
        let visible = transition.at(interrupted, spec, &theme);
        assert_ne!(visible, idle);
        assert_ne!(visible, active);
        let (paint, animating) =
            transition.show(EdgeState::Failed, failed, interrupted, spec, true, &theme);
        assert_eq!(paint, visible);
        assert_eq!(transition.from, visible);
        assert!(animating);

        let (paint, animating) =
            transition.show(EdgeState::Active, active, interrupted, spec, false, &theme);
        assert_eq!(paint, active);
        assert!(!animating);
        assert!(transition.started.is_none());
    }

    #[test]
    fn a_new_wire_contracts_once_but_existing_wiring_does_not_replay() {
        let theme = gpui_kit_theme::Theme::studio_dark();
        let spec = MotionPolicy::spec(MotionRole::Feedback, &theme);
        let start = Instant::now();
        let key: PortKey = ("node".into(), "port".into());
        let empty = HashSet::new();
        let wired = HashSet::from([key.clone()]);
        let mut settles = PortSettles::default();

        // Opening on existing wiring draws the current graph and does not
        // pretend that a historical connection just landed.
        let (shown, animating) = settles.show(&wired, start, spec, true);
        assert!(shown.is_empty());
        assert!(!animating);

        // Once removed and added again, the same business endpoint is a new
        // landing and starts from its expanded feedback frame.
        settles.show(&empty, start, spec, true);
        let landed = start + spec.total();
        let (shown, animating) = settles.show(&wired, landed, spec, true);
        assert_eq!(shown.get(&key), Some(&1.0));
        assert!(animating);

        let (shown, animating) = settles.show(&wired, landed + spec.total() / 2, spec, true);
        let contraction = shown.get(&key).copied().expect("mid-settle port");
        assert!(contraction > 0.0 && contraction < 1.0);
        assert!(animating);

        let (shown, animating) = settles.show(&wired, landed + spec.total(), spec, true);
        assert!(shown.is_empty());
        assert!(!animating);
    }

    #[test]
    fn reduced_motion_lands_a_wire_without_a_timeline() {
        let spec = MotionPolicy::spec(MotionRole::Feedback, &gpui_kit_theme::Theme::studio_dark());
        let start = Instant::now();
        let mut settles = PortSettles::default();
        settles.show(&HashSet::new(), start, spec, true);
        let wired = HashSet::from([("node".into(), "port".into())]);
        let (shown, animating) = settles.show(&wired, start, spec, false);
        assert!(shown.is_empty());
        assert!(!animating);
        assert!(settles.started.is_empty());
    }

    /// A frame is a jump the reader did not make, and a jump that is not
    /// travelled leaves them to work out afterwards which part of the graph
    /// they are now in front of.
    #[test]
    fn a_canvas_travels_to_a_frame_and_snaps_to_the_readers_own_hand() {
        let spec = travel_spec();
        let start = Instant::now();
        let here = GraphViewport::new(point(0.0, 0.0), 1.0);
        let there = GraphViewport::new(point(-400.0, -260.0), 0.6);
        let mut travel = Travel::default();

        // The first frame is where the canvas opens, not somewhere it
        // travelled from.
        let (shown, travelling) = travel.shown(here, false, start, spec);
        assert_eq!(shown, here);
        assert!(!travelling);

        let (shown, travelling) = travel.shown(there, false, start, spec);
        assert!(travelling);
        assert_eq!(shown, here, "the travel began somewhere other than here");

        let midway = start + spec.total() / 2;
        let (shown, travelling) = travel.shown(there, false, midway, spec);
        assert!(travelling);
        assert!(shown.offset.x < here.offset.x && shown.offset.x > there.offset.x);
        assert!(shown.zoom < here.zoom && shown.zoom > there.zoom);

        let (shown, travelling) = travel.shown(there, false, start + spec.total(), spec);
        assert_eq!(shown, there);
        assert!(!travelling);

        // A drag or a wheel is the reader moving the canvas with their own
        // hand, and a canvas that eased after the pointer would lag it.
        let dragged = GraphViewport::new(point(-380.0, -260.0), 0.6);
        let (shown, travelling) = travel.shown(dragged, true, start + spec.total(), spec);
        assert_eq!(shown, dragged);
        assert!(!travelling);
    }

    /// Scale is a ratio. Halfway between 0.5 and 2.0 is 1.0, and a straight
    /// blend puts it at 1.25 — most of the journey spent further in than
    /// either end, which is a lurch rather than a pull-back.
    #[test]
    fn a_travel_changes_scale_geometrically() {
        let from = GraphViewport::new(point(0.0, 0.0), 0.5);
        let to = GraphViewport::new(point(100.0, 40.0), 2.0);
        let midway = interpolate_viewport(from, to, 0.5);
        assert!((midway.zoom - 1.0).abs() < 0.001, "{}", midway.zoom);
        assert_eq!(midway.offset, point(50.0, 20.0));
        assert_eq!(interpolate_viewport(from, to, 0.0).zoom, from.zoom);
        assert!((interpolate_viewport(from, to, 1.0).zoom - to.zoom).abs() < 0.001);
    }

    /// A grid of one fixed world step is a grid for one zoom: scaled straight
    /// by the zoom it closes to a smear on the way out and spreads to nothing
    /// on the way in, and both are the interval ceasing to be countable.
    #[test]
    fn the_grid_stays_countable_at_every_zoom() {
        for zoom in [
            0.05, 0.1, 0.2, 0.35, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 8.0,
        ] {
            let (world, fade) = grid_level(zoom);
            let spacing = world * zoom;
            assert!(
                (GRID_MIN_SPACING..GRID_MIN_SPACING * MAJOR as f32).contains(&spacing),
                "at zoom {zoom} the dots sit {spacing} apart"
            );
            assert!((0.0..=1.0).contains(&fade));
            // The step is always a whole number of the authored interval, up
            // or down by the major one, so distance keeps its meaning: a
            // heavier dot is five dots at every level.
            let ratio = (world / GRID_STEP).max(GRID_STEP / world);
            assert!(
                (ratio.log(MAJOR as f32) - ratio.log(MAJOR as f32).round()).abs() < 1.0e-4,
                "at zoom {zoom} the world step {world} is not a major multiple of {GRID_STEP}"
            );
        }
    }

    /// The level changes at a zoom, and a level that appeared would pop. The
    /// finest one is gone by the moment it would have been replaced.
    #[test]
    fn the_finest_grid_level_fades_out_before_it_is_replaced() {
        // Walk the zoom down through a level change and watch the fade fall
        // to nothing and then come back on the level below.
        let mut zoom = 1.0f32;
        let mut faded_out = false;
        for _ in 0..400 {
            zoom *= 0.99;
            let (_, fade) = grid_level(zoom);
            if fade < 0.02 {
                faded_out = true;
            }
        }
        assert!(faded_out, "the level changed without the finest one fading");
        // Right at the floor of the band there is nothing left of it.
        let (world, _) = grid_level(1.0);
        let (_, fade) = grid_level(GRID_MIN_SPACING / world);
        assert!(fade < 1.0e-4);
    }

    #[test]
    fn a_frame_holds_every_card_it_was_given() {
        // Deliberately wider than the surface and starting far from the
        // origin, which is the case a default viewport shows nothing of.
        let nodes = geometry_at(&[(400.0, 300.0, 200.0, 160.0), (1600.0, 900.0, 200.0, 400.0)]);
        let surface = surface(640.0, 480.0);
        let framed = frame_all(&nodes, &[], surface, (0.2, 2.0), &[]).expect("a frame");
        let view = world_view(framed, surface);
        for node in &nodes {
            assert!(
                node.bounds.origin.x >= view.origin.x
                    && node.bounds.origin.y >= view.origin.y
                    && node.bounds.origin.x + node.bounds.size.width
                        <= view.origin.x + view.size.width
                    && node.bounds.origin.y + node.bounds.size.height
                        <= view.origin.y + view.size.height,
                "{:?} fell outside {view:?}",
                node.bounds
            );
        }
    }

    #[test]
    fn a_frame_never_magnifies_and_never_leaves_its_zoom_range() {
        let one = geometry_at(&[(0.0, 0.0, 40.0, 40.0)]);
        let framed =
            frame_all(&one, &[], surface(1200.0, 900.0), (0.2, 2.0), &[]).expect("a frame");
        assert_eq!(
            framed.zoom, 1.0,
            "a small graph is shown at its own size, not blown up"
        );

        let wide = geometry_at(&[(0.0, 0.0, 100_000.0, 100.0)]);
        let floored =
            frame_all(&wide, &[], surface(640.0, 480.0), (0.4, 2.0), &[]).expect("a frame");
        assert_eq!(
            floored.zoom, 0.4,
            "a graph too wide to fit is held at the caller's floor rather than \
             shrunk past it"
        );
    }

    #[test]
    fn there_is_no_frame_without_something_to_frame_or_somewhere_to_put_it() {
        assert!(frame_all(&[], &[], surface(640.0, 480.0), (0.2, 2.0), &[]).is_none());
        assert!(
            frame_all(
                &geometry_at(&[(0.0, 0.0, 40.0, 40.0)]),
                &[],
                surface(4.0, 4.0),
                (0.2, 2.0),
                &[],
            )
            .is_none(),
            "a canvas that has not been laid out yet is not a frame of zero size"
        );
    }

    #[test]
    fn a_frame_holds_world_bands_as_well_as_cards() {
        let nodes = geometry_at(&[(120.0, 80.0, 160.0, 120.0)]);
        let bands = [GraphBand::new(
            "evaluation.scope",
            "Evaluation scope",
            -240.0,
            -60.0,
            1_200.0,
            420.0,
        )];
        let surface = surface(720.0, 480.0);
        let framed = frame_all(&nodes, &bands, surface, (0.2, 2.0), &[]).expect("a frame");
        let view = world_view(framed, surface);
        let band = bands[0].bounds();
        assert!(
            band.left() >= view.left()
                && band.top() >= view.top()
                && band.right() <= view.right()
                && band.bottom() <= view.bottom(),
            "the declared world region {band:?} fell outside {view:?}"
        );
    }

    #[test]
    fn fitted_content_clears_canvas_owned_corner_chrome() {
        let nodes = geometry_at(&[(0.0, 0.0, 1_000.0, 520.0)]);
        let surface = surface(800.0, 560.0);
        let obstacles = [
            FitObstacle::new(FitCorner::TopLeft, 320.0, 56.0),
            FitObstacle::new(FitCorner::BottomRight, 152.0, 100.0),
        ];
        let framed = frame_all(&nodes, &[], surface, (0.2, 2.0), &obstacles).expect("a frame");
        let node = nodes[0].bounds;
        let origin = world_to_screen(node.origin, framed);
        let content = Bounds::new(
            origin,
            size(
                node.size.width * framed.zoom,
                node.size.height * framed.zoom,
            ),
        );
        let toolbar = Bounds::new(point(0.0, 0.0), size(320.0, 56.0));
        let minimap = Bounds::new(point(800.0 - 152.0, 560.0 - 100.0), size(152.0, 100.0));
        assert!(
            !bounds_overlap(content, toolbar, 0.0),
            "fitted content {content:?} remained underneath the toolbar {toolbar:?}"
        );
        assert!(
            !bounds_overlap(content, minimap, 0.0),
            "fitted content {content:?} remained underneath the minimap {minimap:?}"
        );
    }

    #[test]
    fn a_canvas_frames_once_per_token_and_never_takes_the_view_back() {
        assert_eq!(
            wants_frame(GraphFit::Never, None),
            None,
            "a canvas the caller never asked to frame stays where the caller put it"
        );

        let opening = wants_frame(GraphFit::Whole(0), None).expect("the opening frame");
        assert_eq!(opening, 0);
        assert_eq!(
            wants_frame(GraphFit::Whole(0), Some(opening)),
            None,
            "once framed, the view is the reader's: panning away does not earn \
             it back"
        );
        assert_eq!(
            wants_frame(GraphFit::Whole(1), Some(0)),
            Some(1),
            "a caller's own Fit control bumps the token, which is how it asks \
             for a frame it cannot compute itself"
        );
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
    fn connection_targets_follow_direction_and_caller_rules() {
        let nodes = vec![
            NodeGeometry {
                id: "source".into(),
                bounds: Bounds::new(point(0.0, 0.0), size(10.0, 10.0)),
                tint: gpui_kit_theme::Theme::studio_dark().colors.text_faint,
                ports: vec![PortGeometry {
                    id: "out".into(),
                    anchor: Anchor {
                        point: point(0.0, 0.0),
                        side: PortSide::Right,
                    },
                    direction: PortDirection::Output,
                }],
            },
            NodeGeometry {
                id: "target".into(),
                bounds: Bounds::new(point(100.0, 0.0), size(10.0, 10.0)),
                tint: gpui_kit_theme::Theme::studio_dark().colors.text_faint,
                ports: vec![PortGeometry {
                    id: "in".into(),
                    anchor: Anchor {
                        point: point(100.0, 0.0),
                        side: PortSide::Left,
                    },
                    direction: PortDirection::Input,
                }],
            },
        ];
        let source = GraphEndpoint::new("source", "out");
        let target = GraphEndpoint::new("target", "in");
        let viewport = GraphViewport::default();

        assert_eq!(
            connection_target(
                &nodes,
                &source,
                PortDirection::Output,
                point(100.0, 0.0),
                viewport,
                10.0,
                None,
            ),
            Some((target.clone(), true))
        );
        assert_eq!(
            connection_target(
                &nodes,
                &target,
                PortDirection::Input,
                point(0.0, 0.0),
                viewport,
                10.0,
                None,
            ),
            Some((source.clone(), true))
        );

        let rejects_all: ConnectionValidator = Rc::new(|_, _| false);
        assert_eq!(
            connection_target(
                &nodes,
                &source,
                PortDirection::Output,
                point(100.0, 0.0),
                viewport,
                10.0,
                Some(&rejects_all),
            ),
            Some((target, false))
        );
        assert_eq!(
            normalized_connection(
                GraphEndpoint::new("target", "in"),
                PortDirection::Input,
                GraphEndpoint::new("source", "out"),
            ),
            (source, GraphEndpoint::new("target", "in"))
        );
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
            tint: theme.colors.text_faint,
            ports: Vec::new(),
        }];
        let first = route_signature(&geometry, &[]);
        let moved = Placed::new(GraphNode::new("a", "A"), 40.0, 0.0);
        let shifted = [NodeGeometry {
            id: SharedString::from("a"),
            bounds: moved.bounds(&theme, None),
            tint: theme.colors.text_faint,
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

#[cfg(test)]
mod graph_phase_tests {
    use super::*;

    #[test]
    fn a_refusal_is_unavailable_and_a_load_failure_is_error() {
        let refused = GraphState::Refused("policy".into());
        assert_eq!(refused.phase(), Phase::Unavailable);
        assert_eq!(refused.name(), "refused");
        assert_eq!(GraphState::Failed("offline".into()).phase(), Phase::Error);
    }
}
