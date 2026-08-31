//! Static edge data and orthogonal geometry for a node graph.

use gpui::{Bounds, Hsla, PathBuilder, Pixels, Point, SharedString, Window, point, px, size};
use gpui_kit_theme::{Radius, Theme};

/// The routing treatment of an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeKind {
    #[default]
    Flow,
    Feedback,
}

impl EdgeKind {
    pub fn color(self, theme: &Theme) -> Hsla {
        match self {
            Self::Flow => theme.colors.node.edge,
            Self::Feedback => theme.colors.node.edge_feedback,
        }
    }

    /// The paint this kind takes when it carries traffic or sits under the
    /// pointer.
    pub fn active_color(self, theme: &Theme) -> Hsla {
        match self {
            Self::Flow => theme.colors.node.edge_active,
            Self::Feedback => theme.colors.node.edge_feedback_active,
        }
    }

    fn dashes(self) -> Option<[Pixels; 2]> {
        (self == Self::Feedback).then(|| [px(5.0), px(4.0)])
    }
}

/// The caller-owned state of one connection.
///
/// Only [`EdgeState::Active`] carries continuous traffic. A succeeded or
/// failed route keeps the outcome colour but stops moving, because motion on
/// either would claim work is still crossing it. [`GraphEdge::active`] remains
/// the compatibility builder for callers that only distinguish idle from
/// active; [`GraphEdge::state`] is the complete vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeState {
    #[default]
    Idle,
    Active,
    Succeeded,
    Failed,
}

/// The terminal mark a connection places at its destination.
///
/// The mark carries direction only when the host asks for it. Dense lane
/// diagrams generally need the whole corridor more than another glyph, while
/// a sparse curved graph can use a cap or arrow to make its reading direction
/// immediate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeMarker {
    /// The route ends cleanly at the destination port.
    #[default]
    None,
    /// A filled terminal bead, useful when destination is the emphasis but an
    /// arrow would overstate movement.
    Dot,
    /// A filled arrow following the route's exact terminal tangent.
    Arrow,
}

impl EdgeState {
    pub fn value(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Active => "active",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn colors(self, kind: EdgeKind, theme: &Theme) -> EdgeColors {
        match (self, kind) {
            (Self::Idle, EdgeKind::Flow) => {
                EdgeColors::new(theme.colors.node.edge, theme.colors.node.edge_target)
            }
            (Self::Active, EdgeKind::Flow) => EdgeColors::new(
                theme.colors.node.edge_active,
                theme.colors.node.edge_flow_highlight,
            ),
            (Self::Idle, EdgeKind::Feedback) => EdgeColors::new(
                theme.colors.node.edge_feedback,
                theme.colors.node.edge_feedback_active,
            ),
            (Self::Active, EdgeKind::Feedback) => EdgeColors::new(
                theme.colors.node.edge_feedback_active,
                theme.colors.node.aura_attention,
            ),
            (Self::Succeeded, _) => {
                EdgeColors::new(kind.color(theme), theme.colors.node.aura_success)
            }
            (Self::Failed, _) => EdgeColors::new(kind.color(theme), theme.colors.node.aura_danger),
        }
    }
}

/// Source and destination paints for one route frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EdgeColors {
    pub(crate) from: Hsla,
    pub(crate) to: Hsla,
}

impl EdgeColors {
    pub(crate) const fn new(from: Hsla, to: Hsla) -> Self {
        Self { from, to }
    }

    fn opacity(self, alpha: f32) -> Self {
        Self::new(self.from.opacity(alpha), self.to.opacity(alpha))
    }
}

/// The side of a node on which a port is placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PortSide {
    Top,
    Right,
    Bottom,
    #[default]
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Axis {
    Horizontal,
    Vertical,
}

impl PortSide {
    pub(crate) fn outward(self) -> Point<f32> {
        match self {
            Self::Top => point(0.0, -1.0),
            Self::Right => point(1.0, 0.0),
            Self::Bottom => point(0.0, 1.0),
            Self::Left => point(-1.0, 0.0),
        }
    }

    pub(crate) fn axis(self) -> Axis {
        match self {
            Self::Left | Self::Right => Axis::Horizontal,
            Self::Top | Self::Bottom => Axis::Vertical,
        }
    }

    /// The side something arriving from here would face.
    pub(crate) fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Right => Self::Left,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
        }
    }
}

/// A caller-owned node and port identity used by connection proposals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEndpoint {
    pub node: SharedString,
    pub port: SharedString,
}

impl GraphEndpoint {
    /// Creates an endpoint from business identities, not display labels.
    pub fn new(node: impl Into<SharedString>, port: impl Into<SharedString>) -> Self {
        Self {
            node: node.into(),
            port: port.into(),
        }
    }
}

/// A controlled connection between two graph nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    from: SharedString,
    to: SharedString,
    kind: EdgeKind,
    id: Option<SharedString>,
    from_port: Option<SharedString>,
    to_port: Option<SharedString>,
    label: Option<SharedString>,
    state: EdgeState,
    marker: EdgeMarker,
    selected: bool,
    lane: i16,
}

impl GraphEdge {
    pub fn new(from: impl Into<SharedString>, to: impl Into<SharedString>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            kind: EdgeKind::Flow,
            id: None,
            from_port: None,
            to_port: None,
            label: None,
            state: EdgeState::Idle,
            marker: EdgeMarker::None,
            selected: false,
            lane: 0,
        }
    }

    pub fn from(&self) -> &SharedString {
        &self.from
    }
    pub fn to(&self) -> &SharedString {
        &self.to
    }
    pub fn kind(&self) -> EdgeKind {
        self.kind
    }
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn ports(mut self, from: impl Into<SharedString>, to: impl Into<SharedString>) -> Self {
        self.from_port = Some(from.into());
        self.to_port = Some(to.into());
        self
    }
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
    pub fn active(mut self, active: bool) -> Self {
        self.state = if active {
            EdgeState::Active
        } else {
            EdgeState::Idle
        };
        self
    }
    pub fn state(mut self, state: EdgeState) -> Self {
        self.state = state;
        self
    }
    /// Places an optional direction mark at the destination port.
    pub fn marker(mut self, marker: EdgeMarker) -> Self {
        self.marker = marker;
        self
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    pub fn lane(mut self, lane: i16) -> Self {
        self.lane = lane;
        self
    }
    pub fn feedback(mut self) -> Self {
        self.kind = EdgeKind::Feedback;
        self
    }

    pub(crate) fn source_port(&self) -> Option<&SharedString> {
        self.from_port.as_ref()
    }
    pub(crate) fn target_port(&self) -> Option<&SharedString> {
        self.to_port.as_ref()
    }
    pub(crate) fn edge_label(&self) -> Option<&SharedString> {
        self.label.as_ref()
    }
    pub(crate) fn is_active(&self) -> bool {
        self.state == EdgeState::Active
    }
    pub(crate) fn edge_state(&self) -> EdgeState {
        self.state
    }
    pub(crate) fn edge_marker(&self) -> EdgeMarker {
        self.marker
    }
    pub(crate) fn is_selected(&self) -> bool {
        self.selected
    }
    pub(crate) fn edge_lane(&self) -> i16 {
        self.lane
    }
    /// The stable identity used for interaction and duplicate rejection.
    ///
    /// An explicit identity supplied with [`GraphEdge::id`] is returned as-is.
    /// Otherwise the endpoint, port, kind, and lane identities form an
    /// unambiguous compatibility identity.
    pub fn edge_id(&self) -> SharedString {
        if let Some(id) = &self.id {
            return id.clone();
        }
        // Length prefixes make the compatibility identity unambiguous even if ids contain separators.
        let kind = match self.kind {
            EdgeKind::Flow => "flow",
            EdgeKind::Feedback => "feedback",
        };
        format!(
            "{}:{}|{}:{}|{}:{}|{}:{}|{}|{}",
            self.from.len(),
            self.from,
            self.to.len(),
            self.to,
            self.from_port.as_ref().map_or(0, |v| v.len()),
            self.from_port.as_deref().unwrap_or(""),
            self.to_port.as_ref().map_or(0, |v| v.len()),
            self.to_port.as_deref().unwrap_or(""),
            kind,
            self.lane
        )
        .into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Anchor {
    pub(crate) point: Point<f32>,
    pub(crate) side: PortSide,
}

#[derive(Debug, Clone)]
pub(crate) struct OrthogonalRoute {
    points: Vec<Point<f32>>,
    cumulative: Vec<f32>,
    total: f32,
    /// Whether these points trace a curve rather than turn corners. A curve is
    /// carried as a fine polyline so that measuring, sampling, trimming and
    /// painting stay one implementation; what the flag decides is that its
    /// hundred tiny turns must not be rounded off as if they were corners.
    curved: bool,
}

impl OrthogonalRoute {
    fn new(points: Vec<Point<f32>>) -> Self {
        let points = normalize(points);
        let mut cumulative = vec![0.0];
        for pair in points.windows(2) {
            cumulative.push(
                cumulative.last().copied().unwrap_or(0.0)
                    + (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y),
            );
        }
        let total = cumulative.last().copied().unwrap_or(0.0);
        Self {
            points,
            cumulative,
            total,
            curved: false,
        }
    }

    fn into_curve(mut self) -> Self {
        self.curved = true;
        self
    }

    /// How far this route's turns are rounded when painted.
    pub(crate) fn corner(&self, theme: &Theme) -> f32 {
        if self.curved {
            0.0
        } else {
            theme.radius(Radius::Small)
        }
    }
    pub(crate) fn points(&self) -> &[Point<f32>] {
        &self.points
    }
    fn terminal_tangent(&self) -> Point<f32> {
        let Some(pair) = self.points.windows(2).next_back() else {
            return point(1.0, 0.0);
        };
        let run = point(pair[1].x - pair[0].x, pair[1].y - pair[0].y);
        let length = run.x.hypot(run.y);
        if length <= f32::EPSILON {
            point(1.0, 0.0)
        } else {
            point(run.x / length, run.y / length)
        }
    }

    /// Sparse curves grow gently towards their destination, carrying flow
    /// direction without changing the width of orthogonal lane diagrams.
    fn width_scale(&self, progress: f32) -> f32 {
        if !self.curved {
            return 1.0;
        }
        let progress = progress.clamp(0.0, 1.0);
        let eased = progress * progress * (3.0 - 2.0 * progress);
        0.78 + 0.52 * eased
    }
    #[cfg(test)]
    pub(crate) fn total_length(&self) -> f32 {
        self.total
    }
    pub(crate) fn sample(&self, progress: f32) -> Point<f32> {
        let Some(&first) = self.points.first() else {
            return point(0.0, 0.0);
        };
        if self.total == 0.0 {
            return first;
        }
        let target = progress.clamp(0.0, 1.0) * self.total;
        let index = self
            .cumulative
            .partition_point(|&length| length < target)
            .clamp(1, self.points.len() - 1);
        let start_length = self.cumulative[index - 1];
        let segment = self.cumulative[index] - start_length;
        let t = if segment == 0.0 {
            0.0
        } else {
            (target - start_length) / segment
        };
        point(
            self.points[index - 1].x + (self.points[index].x - self.points[index - 1].x) * t,
            self.points[index - 1].y + (self.points[index].y - self.points[index - 1].y) * t,
        )
    }
    pub(crate) fn midpoint(&self) -> Point<f32> {
        self.sample(0.5)
    }

    /// Shortest world-space distance from a point to this route.
    ///
    /// Curves use the same measured polyline that paint, sampling and trim
    /// use, so hover cannot disagree with the connection the reader sees.
    pub(crate) fn distance_to(&self, point: Point<f32>) -> f32 {
        self.points
            .windows(2)
            .map(|segment| distance_to_segment(point, segment[0], segment[1]))
            .fold(f32::INFINITY, f32::min)
    }

    pub(crate) fn axis_at(&self, progress: f32) -> Axis {
        if self.points.len() < 2 {
            return Axis::Horizontal;
        }
        let target = progress.clamp(0.0, 1.0) * self.total;
        let index = self
            .cumulative
            .partition_point(|length| *length < target)
            .clamp(1, self.points.len() - 1);
        let run = point(
            self.points[index].x - self.points[index - 1].x,
            self.points[index].y - self.points[index - 1].y,
        );
        if run.x.abs() >= run.y.abs() {
            Axis::Horizontal
        } else {
            Axis::Vertical
        }
    }
}

fn distance_to_segment(at: Point<f32>, from: Point<f32>, to: Point<f32>) -> f32 {
    let run = point(to.x - from.x, to.y - from.y);
    let length_squared = run.x * run.x + run.y * run.y;
    if length_squared <= f32::EPSILON {
        return (at.x - from.x).hypot(at.y - from.y);
    }
    let progress =
        (((at.x - from.x) * run.x + (at.y - from.y) * run.y) / length_squared).clamp(0.0, 1.0);
    let nearest = point(from.x + run.x * progress, from.y + run.y * progress);
    (at.x - nearest.x).hypot(at.y - nearest.y)
}

const LEAD: f32 = 24.0;
const CORRIDOR: f32 = 36.0;
const LANE_SPACING: f32 = 12.0;
const MIN_LEAD: f32 = 4.0;

pub(crate) fn route_orthogonal(
    from: Anchor,
    to: Anchor,
    from_bounds: Bounds<f32>,
    to_bounds: Bounds<f32>,
    kind: EdgeKind,
    lane: i16,
) -> Option<OrthogonalRoute> {
    if from.point == to.point {
        return Some(self_route(from, from_bounds, lane));
    }
    let lane_offset = lane as f32 * LANE_SPACING;
    // Separate lanes at the ports as well as in their middle corridor. Without
    // this, opposite routes between two same-side port groups can share their
    // first or last horizontal segment even though their trunks are distinct.
    let preferred_lead = (LEAD + lane_offset).max(MIN_LEAD);
    // Two cards packed until they overlap leave a port sitting inside the
    // other's box, and no lead clears both. The connection is still a fact,
    // so the route degrades to the straight line between the two ports: an
    // edge that disappeared would say these cards are not connected, which is
    // the one thing a reader would believe without checking.
    let (Some(from_lead), Some(to_lead)) = (
        lead_distance(from, to_bounds, preferred_lead),
        lead_distance(to, from_bounds, preferred_lead),
    ) else {
        return Some(OrthogonalRoute::new(vec![from.point, to.point]));
    };
    let a = from.outward_point(from_lead);
    let b = to.outward_point(to_lead);
    let left = from_bounds.left().min(to_bounds.left()) - CORRIDOR;
    let right = from_bounds.right().max(to_bounds.right()) + CORRIDOR;
    let top = from_bounds.top().min(to_bounds.top()) - CORRIDOR;
    let bottom = from_bounds.bottom().max(to_bounds.bottom()) + CORRIDOR;

    let finish = |middle: Vec<Point<f32>>| {
        let mut points = Vec::with_capacity(middle.len() + 2);
        points.push(from.point);
        points.extend(middle);
        points.push(to.point);
        let route = OrthogonalRoute::new(points);
        let clear = route.points().windows(2).all(|pair| {
            segment_clear(pair[0], pair[1], from_bounds)
                && segment_clear(pair[0], pair[1], to_bounds)
        });
        (clear && route_is_directional(&route, from, to)).then_some(route)
    };

    // A feedback path is a return lane, so its first choice remains the
    // corridor below both endpoint cards. Explicit side choices that make
    // that route cross a card fall through to the general router.
    if kind == EdgeKind::Feedback {
        let y = bottom + lane_offset;
        let middle = vec![a, point(a.x, y), point(b.x, y), b];
        if let Some(route) = finish(middle) {
            return Some(route);
        }
    }

    // A non-zero lane deliberately takes a parallel corridor. The first
    // candidate stays near the direct route; if that would cross an endpoint,
    // the sign of the lane selects the corresponding outside corridor.
    if lane != 0 {
        let candidates = match from.side.axis() {
            Axis::Horizontal => {
                let near = (a.y + b.y) / 2.0 + lane_offset;
                let outside = if lane > 0 {
                    bottom + lane_offset.abs()
                } else {
                    top - lane_offset.abs()
                };
                vec![
                    vec![a, point(a.x, near), point(b.x, near), b],
                    vec![a, point(a.x, outside), point(b.x, outside), b],
                ]
            }
            Axis::Vertical => {
                let near = (a.x + b.x) / 2.0 + lane_offset;
                let outside = if lane > 0 {
                    right + lane_offset.abs()
                } else {
                    left - lane_offset.abs()
                };
                vec![
                    vec![a, point(near, a.y), point(near, b.y), b],
                    vec![a, point(outside, a.y), point(outside, b.y), b],
                ]
            }
        };
        if let Some(route) = candidates.into_iter().find_map(&finish) {
            return Some(route);
        }
    }

    let mut candidates = vec![Vec::new()];
    if a.x == b.x || a.y == b.y {
        candidates.push(vec![a, b]);
    }
    candidates.push(vec![a, point(b.x, a.y), b]);
    candidates.push(vec![a, point(a.x, b.y), b]);

    let middle_x = (a.x + b.x) / 2.0;
    let middle_y = (a.y + b.y) / 2.0;
    for x in [middle_x, left, right] {
        candidates.push(vec![a, point(x, a.y), point(x, b.y), b]);
    }
    for y in [middle_y, top, bottom] {
        candidates.push(vec![a, point(a.x, y), point(b.x, y), b]);
    }

    candidates
        .into_iter()
        .filter_map(finish)
        .min_by(|left, right| {
            path_cost(left.points())
                .partial_cmp(&path_cost(right.points()))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        // Every candidate crossed a card, which happens once cards are packed
        // tightly enough. The plain elbow is drawn anyway, for the same reason
        // as above: a connection that exists is drawn even when it cannot be
        // drawn cleanly.
        .or_else(|| {
            Some(OrthogonalRoute::new(vec![
                from.point,
                a,
                point(b.x, a.y),
                b,
                to.point,
            ]))
        })
}

fn lead_distance(anchor: Anchor, obstacle: Bounds<f32>, preferred: f32) -> Option<f32> {
    const EPSILON: f32 = 0.001;
    let point = anchor.point;
    if point.x > obstacle.left() + EPSILON
        && point.x < obstacle.right() - EPSILON
        && point.y > obstacle.top() + EPSILON
        && point.y < obstacle.bottom() - EPSILON
    {
        return None;
    }
    let crosses_vertical_span =
        point.y > obstacle.top() + EPSILON && point.y < obstacle.bottom() - EPSILON;
    let crosses_horizontal_span =
        point.x > obstacle.left() + EPSILON && point.x < obstacle.right() - EPSILON;
    let clearance = match anchor.side {
        PortSide::Right if crosses_vertical_span && obstacle.left() >= point.x => {
            Some(obstacle.left() - point.x)
        }
        PortSide::Left if crosses_vertical_span && obstacle.right() <= point.x => {
            Some(point.x - obstacle.right())
        }
        PortSide::Bottom if crosses_horizontal_span && obstacle.top() >= point.y => {
            Some(obstacle.top() - point.y)
        }
        PortSide::Top if crosses_horizontal_span && obstacle.bottom() <= point.y => {
            Some(point.y - obstacle.bottom())
        }
        _ => None,
    };
    match clearance {
        Some(clearance) if clearance <= EPSILON => None,
        Some(clearance) => Some(preferred.min(clearance * 0.5)),
        None => Some(preferred),
    }
}

fn route_is_directional(route: &OrthogonalRoute, from: Anchor, to: Anchor) -> bool {
    let Some(first) = route.points().get(1) else {
        return false;
    };
    let Some(before) = route.points().get(route.points().len().saturating_sub(2)) else {
        return false;
    };
    let from_normal = from.side.outward();
    let to_normal = to.side.outward();
    (first.x - from.point.x) * from_normal.x + (first.y - from.point.y) * from_normal.y > 0.0
        && (before.x - to.point.x) * to_normal.x + (before.y - to.point.y) * to_normal.y > 0.0
}

/// Routes a connection gesture from a real port to the pointer without
/// inventing a target node. The preview leaves the source in its declared
/// direction and then takes one square corner to the pointer.
pub(crate) fn route_preview(from: Anchor, to: Point<f32>) -> OrthogonalRoute {
    let lead = from.outward_point(LEAD);
    let elbow = match from.side.axis() {
        Axis::Horizontal => point(to.x, lead.y),
        Axis::Vertical => point(lead.x, to.y),
    };
    OrthogonalRoute::new(vec![from.point, lead, elbow, to])
}

/// How the connections on one graph are drawn.
///
/// This is a statement about the graph rather than a taste about the drawing.
/// Lanes exist to keep many connections legible where they run together, and
/// [`GraphEdge::lane`] is how a caller separates them; a curve has no lane to
/// be in, so asking for curves on a graph that needs lanes trades a fact for a
/// look. What decides it is whether a reader has to follow one connection
/// through a crowd, or take in the shape of a sparse one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphRouting {
    /// Right-angled runs in separated lanes, which is how a dense pipeline
    /// stays traceable.
    #[default]
    Lanes,
    /// One curve from port to port, which is how a sparse board reads as a
    /// gesture rather than as wiring.
    Curves,
}

/// How far a curve leaves its port before it starts turning, at the least.
const CURVE_LEAD: f32 = 32.0;
/// Long routes stop increasing their handles here, avoiding the broad loops
/// that make two nearby endpoint tangents look unrelated.
const CURVE_MAX_LEAD: f32 = 180.0;
/// How finely a curve is cut into the polyline everything else measures.
const CURVE_STEPS: usize = 32;

/// A single curve between two ports, leaving and arriving along the sides they
/// face.
///
/// It ignores the cards it passes, which is the trade a curve makes: it cannot
/// be routed around an obstacle without ceasing to be one curve. That is why
/// it is the caller's choice and not the default.
pub(crate) fn route_curved(from: Anchor, to: Anchor) -> OrthogonalRoute {
    let span = (to.point.x - from.point.x).hypot(to.point.y - from.point.y);
    // A cubic already guarantees the declared endpoint tangents. Bounded
    // handles make those tangents relax into the body of the curve rather than
    // crossing on short hops or producing loops on very long ones.
    let lead = (span * 0.42).clamp(CURVE_LEAD.min(span * 0.5), CURVE_MAX_LEAD);
    let first = from.outward_point(lead);
    let last = to.outward_point(lead);
    let points = (0..=CURVE_STEPS)
        .map(|step| {
            let t = step as f32 / CURVE_STEPS as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            point(
                a * from.point.x + b * first.x + c * last.x + d * to.point.x,
                a * from.point.y + b * first.y + c * last.y + d * to.point.y,
            )
        })
        .collect();
    OrthogonalRoute::new(points).into_curve()
}

/// The same curve, from a real port to wherever the pointer is.
pub(crate) fn route_curved_preview(from: Anchor, to: Point<f32>) -> OrthogonalRoute {
    route_curved(
        from,
        Anchor {
            point: to,
            side: from.side.opposite(),
        },
    )
}

impl Anchor {
    fn outward_point(self, distance: f32) -> Point<f32> {
        let normal = self.side.outward();
        point(
            self.point.x + normal.x * distance,
            self.point.y + normal.y * distance,
        )
    }
}

fn self_route(anchor: Anchor, bounds: Bounds<f32>, lane: i16) -> OrthogonalRoute {
    let lead = anchor.outward_point(LEAD);
    let reach = CORRIDOR + lane.unsigned_abs() as f32 * LANE_SPACING;
    let normal = anchor.side.outward();
    let perpendicular = point(-normal.y, normal.x);
    let far = point(lead.x + normal.x * reach, lead.y + normal.y * reach);
    let corner = |origin: Point<f32>, direction: f32| {
        point(
            origin.x + perpendicular.x * reach * direction,
            origin.y + perpendicular.y * reach * direction,
        )
    };
    let direction = if lane < 0 { -1.0 } else { 1.0 };
    let route = OrthogonalRoute::new(vec![
        anchor.point,
        lead,
        corner(lead, direction),
        corner(far, direction),
        far,
        lead,
        anchor.point,
    ]);
    debug_assert!(route.points().iter().all(|point| {
        point.x.is_finite()
            && point.y.is_finite()
            && (point.x <= bounds.left()
                || point.x >= bounds.right()
                || point.y <= bounds.top()
                || point.y >= bounds.bottom())
    }));
    route
}

fn segment_clear(from: Point<f32>, to: Point<f32>, bounds: Bounds<f32>) -> bool {
    const EPSILON: f32 = 0.001;
    if from.x == to.x {
        let low = from.y.min(to.y);
        let high = from.y.max(to.y);
        !(from.x > bounds.left() + EPSILON
            && from.x < bounds.right() - EPSILON
            && high > bounds.top() + EPSILON
            && low < bounds.bottom() - EPSILON)
    } else if from.y == to.y {
        let low = from.x.min(to.x);
        let high = from.x.max(to.x);
        !(from.y > bounds.top() + EPSILON
            && from.y < bounds.bottom() - EPSILON
            && high > bounds.left() + EPSILON
            && low < bounds.right() - EPSILON)
    } else {
        false
    }
}

fn path_cost(points: &[Point<f32>]) -> f32 {
    let distance: f32 = points
        .windows(2)
        .map(|pair| (pair[1].x - pair[0].x).abs() + (pair[1].y - pair[0].y).abs())
        .sum();
    distance + points.len().saturating_sub(2) as f32 * 4.0
}

fn normalize(points: Vec<Point<f32>>) -> Vec<Point<f32>> {
    let mut out: Vec<Point<f32>> = Vec::new();
    for point in points
        .into_iter()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
    {
        if out.last() == Some(&point) {
            continue;
        }
        while out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            let same_axis = (a.x == b.x && b.x == point.x) || (a.y == b.y && b.y == point.y);
            let same_direction =
                (b.x - a.x) * (point.x - b.x) >= 0.0 && (b.y - a.y) * (point.y - b.y) >= 0.0;
            if same_axis && same_direction {
                out.pop();
            } else {
                break;
            }
        }
        out.push(point);
    }
    out
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RouteTransform {
    origin: Point<Pixels>,
    offset: Point<f32>,
    zoom: f32,
}

impl RouteTransform {
    pub(crate) fn new(origin: Point<Pixels>, offset: Point<f32>, zoom: f32) -> Self {
        Self {
            origin,
            offset,
            zoom,
        }
    }

    fn point(self, world: Point<f32>) -> Point<Pixels> {
        point(
            self.origin.x + px(world.x * self.zoom + self.offset.x),
            self.origin.y + px(world.y * self.zoom + self.offset.y),
        )
    }
}

/// How much of a connection is drawn, and what is travelling along it.
///
/// A struct rather than four positional arguments because the three facts are
/// independent — a connection can be arriving, carrying traffic, both, or
/// neither — and a caller reading `paint_route(.., 1.0, None, 1.0)` cannot see
/// which is which.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EdgePaint {
    /// The resting stroke width, before any state widens it.
    pub(crate) width: f32,
    /// How much of the route is drawn, from its source end, `0..=1`.
    ///
    /// A connection that has just been made draws itself from the port it
    /// leaves to the port it arrives at, which is the direction it means. Below
    /// 1 the connection is still arriving, so it carries no traffic: a comet
    /// running down a wire that does not reach anywhere yet is traffic to a
    /// place the graph has not said exists.
    pub(crate) reveal: f32,
    /// Where the traffic trails are along the route, if it carries any.
    pub(crate) phase: Option<f32>,
    /// The perceptually interpolated state paints for this frame.
    pub(crate) colors: EdgeColors,
    /// Pointer emphasis is transient and never implies traffic.
    pub(crate) hovered: bool,
}

impl EdgePaint {
    pub(crate) fn new(width: f32, colors: EdgeColors) -> Self {
        Self {
            width,
            reveal: 1.0,
            phase: None,
            colors,
            hovered: false,
        }
    }

    pub(crate) fn reveal(mut self, reveal: f32) -> Self {
        self.reveal = reveal.clamp(0.0, 1.0);
        self
    }

    pub(crate) fn phase(mut self, phase: Option<f32>) -> Self {
        self.phase = phase;
        self
    }

    pub(crate) fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }
}

pub(crate) fn paint_route(
    window: &mut Window,
    theme: &Theme,
    edge: &GraphEdge,
    route: &OrthogonalRoute,
    transform: RouteTransform,
    paint: EdgePaint,
) {
    if paint.reveal <= 0.0 {
        return;
    }
    let selected = edge.is_selected();
    let emphasis = if selected {
        theme.effects.node_edge_selected_width_scale
    } else if paint.hovered {
        theme.effects.node_edge_hover_width_scale
    } else {
        1.0
    };
    let width = paint.width * emphasis;
    let drawn = Stroke {
        width,
        color: paint.colors.from,
        dashes: edge.kind.dashes(),
        corner: route.corner(theme),
        trim: Some((0.0, paint.reveal)),
    };
    if selected || paint.hovered {
        paint_gradient(
            window,
            theme,
            route,
            transform,
            Stroke {
                width: paint.width * theme.effects.node_edge_glow_width_scale * emphasis,
                ..drawn
            },
            paint.colors.opacity(theme.effects.node_active_wash_alpha),
            paint.reveal,
        );
    }
    paint_gradient(
        window,
        theme,
        route,
        transform,
        drawn,
        paint.colors,
        paint.reveal,
    );
    let trail = Stroke {
        width: width.max(1.0),
        color: theme
            .colors
            .node
            .edge_flow_highlight
            .opacity(theme.effects.node_traffic_alpha),
        dashes: None,
        ..drawn
    };
    if paint.reveal < 1.0 {
        // The leading end of an arriving connection, so the reveal reads as
        // something travelling to the far port rather than as a line being
        // stretched. It is the same mark the traffic trails use, which is why
        // an edge that arrives and then goes live does not change vocabulary.
        paint_comet(window, route, transform, trail, paint.reveal);
        return;
    }
    if edge.is_active()
        && let Some(phase) = paint.phase
    {
        for comet in 0..COMETS {
            let head = (phase + comet as f32 / COMETS as f32).rem_euclid(1.0);
            paint_comet(window, route, transform, trail, head);
        }
    }
    if paint.reveal >= 1.0 {
        paint_end_marker(window, theme, edge, route, transform, paint, width);
    }
}

/// A source-to-destination colour gradient cut from the same route path used
/// by reveal and traffic. GPUI gradients are spatial backgrounds rather than
/// distance-along-path paints, so bounded route-progress slices are the local
/// geometry needed to keep a bent route directional. Every slice retains the
/// full path's trim and dash phase, avoiding seams in feedback routes.
fn paint_gradient(
    window: &mut Window,
    theme: &Theme,
    route: &OrthogonalRoute,
    transform: RouteTransform,
    stroke: Stroke,
    colors: EdgeColors,
    reveal: f32,
) {
    const SLICES: usize = 24;
    let reveal = reveal.clamp(0.0, 1.0);
    for slice in 0..SLICES {
        let from = slice as f32 / SLICES as f32;
        if from >= reveal {
            break;
        }
        let to = ((slice + 1) as f32 / SLICES as f32).min(reveal);
        let middle = (from + to) * 0.5;
        paint_trimmed_stroke(
            window,
            route,
            transform,
            Stroke {
                color: theme.mix(colors.from, colors.to, middle),
                width: stroke.width * route.width_scale(middle),
                trim: Some((from, to)),
                ..stroke
            },
        );
    }
}

/// Draws a filled cap on the destination tangent. It never uses an outline:
/// selection and hover add a low-alpha halo underneath the information mark.
fn paint_end_marker(
    window: &mut Window,
    theme: &Theme,
    edge: &GraphEdge,
    route: &OrthogonalRoute,
    transform: RouteTransform,
    paint: EdgePaint,
    stroke: f32,
) {
    let marker = edge.edge_marker();
    if marker == EdgeMarker::None {
        return;
    }
    let Some(&world_tip) = route.points().last() else {
        return;
    };
    let tip = transform.point(world_tip);
    let unit = theme.measures.status_mark.max(stroke * 2.0);
    let color = paint.colors.to;
    if edge.is_selected() || paint.hovered {
        for step in (1..=3).rev() {
            let radius = px(unit * (0.72 + step as f32 * 0.36));
            window.paint_quad(gpui::fill(
                Bounds::new(
                    point(tip.x - radius, tip.y - radius),
                    size(radius * 2.0, radius * 2.0),
                ),
                color.opacity(theme.effects.node_active_wash_alpha / (step as f32 + 1.5)),
            ));
        }
    }
    match marker {
        EdgeMarker::None => {}
        EdgeMarker::Dot => {
            let radius = px(unit * 0.42);
            window.paint_quad(gpui::fill(
                Bounds::new(
                    point(tip.x - radius, tip.y - radius),
                    size(radius * 2.0, radius * 2.0),
                ),
                color,
            ));
        }
        EdgeMarker::Arrow => {
            let tangent = route.terminal_tangent();
            let length = unit * 1.75;
            let half_width = unit * 0.72;
            let base = point(
                tip.x - px(tangent.x * length),
                tip.y - px(tangent.y * length),
            );
            let perpendicular = point(-tangent.y, tangent.x);
            let mut builder = PathBuilder::fill();
            builder.move_to(tip);
            builder.line_to(point(
                base.x + px(perpendicular.x * half_width),
                base.y + px(perpendicular.y * half_width),
            ));
            builder.line_to(point(
                base.x - px(perpendicular.x * half_width),
                base.y - px(perpendicular.y * half_width),
            ));
            builder.close();
            if let Ok(path) = builder.build() {
                window.paint_path(path, color);
            }
        }
    }
}

/// How many traffic trails share one live connection.
const COMETS: usize = 3;
/// How much of the route one trail covers.
const TAIL: f32 = 0.09;
/// How many nested strokes the trail is built from.
///
/// Each is shorter, wider and stronger than the one under it, so the paint
/// accumulates into an opaque head that narrows and fades behind. One flat
/// trim, which is what this was, is a dash: it has a hard back edge and no
/// direction, so a reader cannot tell which way the traffic is going.
const TAPER: usize = 4;

/// One traffic trail, its head at `head` along the route.
fn paint_comet(
    window: &mut Window,
    route: &OrthogonalRoute,
    transform: RouteTransform,
    trail: Stroke,
    head: f32,
) {
    for step in 0..TAPER {
        let remaining = (TAPER - step) as f32 / TAPER as f32;
        let extent = TAIL * remaining;
        // Widest and strongest at the head, which is the step with the least
        // extent left to cover.
        let step = Stroke {
            width: trail.width * (1.15 + 1.05 * (1.0 - remaining)),
            color: trail.color.opacity(0.3 + 0.7 * (1.0 - remaining).powf(1.5)),
            ..trail
        };
        let tail = head - extent;
        if tail >= 0.0 {
            paint_trimmed_stroke(window, route, transform, step.cut(tail, head));
        } else {
            // The trail is crossing the route's start, so it is drawn as the
            // two pieces of one interval rather than skipped: a comet that
            // vanished at the seam would report a gap in the traffic.
            paint_trimmed_stroke(window, route, transform, step.cut(0.0, head));
            paint_trimmed_stroke(window, route, transform, step.cut(1.0 + tail, 1.0));
        }
    }
}

/// Feeds a route into a builder with its right angles rounded.
///
/// Both the stroke and the traffic tails trace through here, because a comet
/// built from the raw vertices would cut the corner the stroke goes round and
/// the two would separate at every turn.
///
/// A corner never takes more than half of either run it joins, so a short jog
/// between two turns rounds to its own midpoint instead of overshooting into
/// the segment beyond and folding the path back on itself.
fn trace_route(
    builder: &mut PathBuilder,
    route: &OrthogonalRoute,
    transform: RouteTransform,
    corner: f32,
) {
    let points = &route.points;
    let Some(first) = points.first() else {
        return;
    };
    builder.move_to(transform.point(*first));
    if !corner.is_finite() || corner <= 0.0 || points.len() < 3 {
        for point in &points[1..] {
            builder.line_to(transform.point(*point));
        }
        return;
    }
    for index in 1..points.len() - 1 {
        let previous = points[index - 1];
        let turn = points[index];
        let next = points[index + 1];
        let radius = corner_radius(previous, turn, next, corner);
        if radius <= 0.0 {
            builder.line_to(transform.point(turn));
            continue;
        }
        builder.line_to(transform.point(step_towards(turn, previous, radius)));
        builder.curve_to(
            transform.point(step_towards(turn, next, radius)),
            transform.point(turn),
        );
    }
    if let Some(last) = points.last() {
        builder.line_to(transform.point(*last));
    }
}

/// How far back from one turn the bend may start.
///
/// Half of the shorter adjoining run is the ceiling, so two turns sharing a
/// short jog each stop at its midpoint. Letting a bend take the whole run
/// would put the leaving point of one corner beyond the entering point of the
/// next, and the path would double back through itself.
fn corner_radius(previous: Point<f32>, turn: Point<f32>, next: Point<f32>, corner: f32) -> f32 {
    let arriving = (turn.x - previous.x).abs() + (turn.y - previous.y).abs();
    let leaving = (next.x - turn.x).abs() + (next.y - turn.y).abs();
    corner.min(arriving / 2.0).min(leaving / 2.0).max(0.0)
}

/// Moves `from` towards `towards` by `distance` along whichever axis
/// separates them. A route is orthogonal, so exactly one axis ever differs.
fn step_towards(from: Point<f32>, towards: Point<f32>, distance: f32) -> Point<f32> {
    if from.x == towards.x {
        point(from.x, from.y + (towards.y - from.y).signum() * distance)
    } else {
        point(from.x + (towards.x - from.x).signum() * distance, from.y)
    }
}

pub(crate) fn paint_route_stroke(
    window: &mut Window,
    route: &OrthogonalRoute,
    transform: RouteTransform,
    width: f32,
    color: Hsla,
    dashes: Option<[Pixels; 2]>,
    corner: f32,
) {
    paint_trimmed_stroke(
        window,
        route,
        transform,
        Stroke {
            width,
            color,
            dashes,
            corner,
            trim: None,
        },
    );
}

/// One stroke of a route: how wide, in what paint, dashed or not, and cut to
/// which interval of the route's measured length.
#[derive(Debug, Clone, Copy)]
struct Stroke {
    width: f32,
    color: Hsla,
    dashes: Option<[Pixels; 2]>,
    corner: f32,
    /// The part of the route to draw, as fractions of its length. `None`
    /// draws all of it.
    trim: Option<(f32, f32)>,
}

impl Stroke {
    fn cut(self, from: f32, to: f32) -> Self {
        Self {
            trim: Some((from, to)),
            ..self
        }
    }
}

/// Draws one stroke of a route.
///
/// Everything that draws part of a connection goes through here — the reveal
/// of a new edge and every segment of a traffic trail — so a partial stroke
/// rounds the same corners the whole one does. A trim taken from the raw
/// vertices would cut the corner the stroke goes round, and the two would
/// separate at every turn.
fn paint_trimmed_stroke(
    window: &mut Window,
    route: &OrthogonalRoute,
    transform: RouteTransform,
    stroke: Stroke,
) {
    let mut builder = PathBuilder::stroke(px(stroke.width));
    if let Some(dashes) = stroke.dashes {
        builder = builder.dash_array(&dashes);
    }
    if let Some((from, to)) = stroke.trim {
        if to <= from {
            return;
        }
        builder = builder.stroke_trim(from, to);
    }
    trace_route(&mut builder, route, transform, stroke.corner);
    if let Ok(path) = builder.build() {
        window.paint_path(path, stroke.color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    fn bounds(x: f32, y: f32) -> Bounds<f32> {
        Bounds::new(point(x, y), size(40.0, 30.0))
    }
    fn anchor(side: PortSide, b: Bounds<f32>) -> Anchor {
        let p = match side {
            PortSide::Top => point(b.center().x, b.top()),
            PortSide::Right => point(b.right(), b.center().y),
            PortSide::Bottom => point(b.center().x, b.bottom()),
            PortSide::Left => point(b.left(), b.center().y),
        };
        Anchor { point: p, side }
    }
    fn assert_valid(route: &OrthogonalRoute, from: Anchor, to: Anchor) {
        assert_eq!(route.points()[0], from.point);
        assert_eq!(*route.points().last().expect("route endpoint"), to.point);
        for pair in route.points().windows(2) {
            assert!(pair.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
            assert_ne!(pair[0], pair[1]);
            assert!(pair[0].x == pair[1].x || pair[0].y == pair[1].y);
        }
        if route.points().len() > 1 {
            let n = from.side.outward();
            let first = route.points()[1];
            assert!((first.x - from.point.x) * n.x + (first.y - from.point.y) * n.y > 0.0);
            let n = to.side.outward();
            let before = route.points()[route.points().len() - 2];
            assert!((before.x - to.point.x) * n.x + (before.y - to.point.y) * n.y > 0.0);
        }
    }

    #[test]
    fn all_side_pairs_are_finite_orthogonal_and_directional() {
        let sides = [
            PortSide::Top,
            PortSide::Right,
            PortSide::Bottom,
            PortSide::Left,
        ];
        let a = bounds(0.0, 0.0);
        let b = bounds(100.0, 80.0);
        for from_side in sides {
            for to_side in sides {
                let from = anchor(from_side, a);
                let to = anchor(to_side, b);
                assert_valid(
                    &route_orthogonal(from, to, a, b, EdgeKind::Flow, 0)
                        .expect("separated cards route"),
                    from,
                    to,
                );
            }
        }
    }
    /// A connection is a fact about the graph, and cards packed until they
    /// overlap leave no clean corridor between them. The route degrades to the
    /// straight line rather than to nothing: an edge that disappeared would
    /// say these two cards are unconnected, which a reader would believe
    /// without checking.
    #[test]
    fn overlapping_cards_still_draw_their_connection_and_self_links_route() {
        let a = bounds(50.0, 20.0);
        let overlapping = bounds(55.0, 25.0);
        let from_port = anchor(PortSide::Right, a);
        let to_port = anchor(PortSide::Left, overlapping);
        let crossed = route_orthogonal(from_port, to_port, a, overlapping, EdgeKind::Flow, 0)
            .expect("an overlapping pair still draws its edge");
        assert_eq!(crossed.points()[0], from_port.point);
        assert_eq!(
            *crossed.points().last().expect("route endpoint"),
            to_port.point
        );
        let from = anchor(PortSide::Bottom, a);
        let to = anchor(PortSide::Top, a);
        let route = route_orthogonal(from, to, a, a, EdgeKind::Feedback, 0)
            .expect("one card can route around itself");
        assert_valid(&route, from, to);
    }
    #[test]
    fn feedback_passes_below_the_deeper_box() {
        let a = bounds(0.0, 0.0);
        let b = Bounds::new(point(100.0, 10.0), size(40.0, 100.0));
        let route = route_orthogonal(
            anchor(PortSide::Bottom, a),
            anchor(PortSide::Bottom, b),
            a,
            b,
            EdgeKind::Feedback,
            0,
        )
        .expect("feedback route");
        assert!(route.points().iter().any(|p| p.y > b.bottom()));
    }
    #[test]
    fn lanes_keep_anchors_but_distinguish_corridors() {
        let a = bounds(0.0, 0.0);
        let b = bounds(100.0, 50.0);
        let from = anchor(PortSide::Right, a);
        let to = anchor(PortSide::Left, b);
        let x = route_orthogonal(from, to, a, b, EdgeKind::Flow, 0).expect("direct lane");
        let y = route_orthogonal(from, to, a, b, EdgeKind::Flow, 2).expect("offset lane");
        assert_eq!(
            (x.points()[0], x.points().last()),
            (y.points()[0], y.points().last())
        );
        assert_ne!(x.points(), y.points());
    }
    #[test]
    fn opposite_lanes_do_not_share_terminal_segments() {
        let upper = Bounds::new(point(0.0, 0.0), size(100.0, 60.0));
        let lower = Bounds::new(point(20.0, 200.0), size(100.0, 60.0));
        let flow_from = Anchor {
            point: point(70.0, upper.bottom()),
            side: PortSide::Bottom,
        };
        let flow_to = Anchor {
            point: point(50.0, lower.top()),
            side: PortSide::Top,
        };
        let retry_from = Anchor {
            point: point(100.0, lower.top()),
            side: PortSide::Top,
        };
        let retry_to = Anchor {
            point: point(30.0, upper.bottom()),
            side: PortSide::Bottom,
        };
        let flow = route_orthogonal(flow_from, flow_to, upper, lower, EdgeKind::Flow, -1)
            .expect("forward lane");
        let retry = route_orthogonal(retry_from, retry_to, lower, upper, EdgeKind::Feedback, 1)
            .expect("return lane");

        let overlaps = |a: &[Point<f32>], b: &[Point<f32>]| {
            a.windows(2).any(|left| {
                b.windows(2).any(|right| {
                    if left[0].y == left[1].y && right[0].y == right[1].y && left[0].y == right[0].y
                    {
                        left[0].x.max(left[1].x).min(right[0].x.max(right[1].x))
                            > left[0].x.min(left[1].x).max(right[0].x.min(right[1].x))
                    } else if left[0].x == left[1].x
                        && right[0].x == right[1].x
                        && left[0].x == right[0].x
                    {
                        left[0].y.max(left[1].y).min(right[0].y.max(right[1].y))
                            > left[0].y.min(left[1].y).max(right[0].y.min(right[1].y))
                    } else {
                        false
                    }
                })
            })
        };
        assert!(!overlaps(flow.points(), retry.points()));
    }
    #[test]
    fn close_facing_cards_clamp_their_leads_without_crossing_either_card() {
        let a = bounds(0.0, 0.0);
        let b = bounds(50.0, 0.0);
        let from = anchor(PortSide::Right, a);
        let to = anchor(PortSide::Left, b);
        let route = route_orthogonal(from, to, a, b, EdgeKind::Flow, 0)
            .expect("the ten-unit corridor is routable");
        assert_valid(&route, from, to);
        for segment in route.points().windows(2) {
            assert!(segment_clear(segment[0], segment[1], a));
            assert!(segment_clear(segment[0], segment[1], b));
        }
    }
    #[test]
    fn sampling_uses_arc_length() {
        let r = OrthogonalRoute::new(vec![point(0.0, 0.0), point(10.0, 0.0), point(10.0, 30.0)]);
        assert_eq!(r.total_length(), 40.0);
        assert_eq!(r.midpoint(), point(10.0, 10.0));
        assert_eq!(r.axis_at(0.2), Axis::Horizontal);
        assert_eq!(r.axis_at(0.5), Axis::Vertical);
        assert_eq!(r.axis_at(0.9), Axis::Vertical);
        assert_eq!(r.sample(2.0), point(10.0, 30.0));
        assert_eq!(r.distance_to(point(4.0, 3.0)), 3.0);
        assert_eq!(r.distance_to(point(13.0, 20.0)), 3.0);
    }

    #[test]
    fn a_curve_leaves_and_arrives_along_the_sides_its_ports_face() {
        let from = anchor(PortSide::Right, bounds(0.0, 0.0));
        let to = anchor(PortSide::Left, bounds(400.0, 200.0));
        let route = route_curved(from, to);

        // It still starts and ends on the ports, so trimming, labelling and
        // sampling read it the same as any other route.
        assert_eq!(route.points().first().copied(), Some(from.point));
        assert_eq!(route.points().last().copied(), Some(to.point));

        // The first and last steps run outward along the port's own side,
        // which is what keeps a connection off the card it leaves.
        assert!(route.points()[1].x > from.point.x);
        assert!(route.points()[route.points().len() - 2].x < to.point.x);

        // Its many small turns are not corners to be rounded off.
        assert_eq!(route.corner(&Theme::studio_dark()), 0.0);

        // Length is measured along the curve, so a comet or a label at half
        // the length lands halfway along what the eye follows.
        let straight = (to.point.x - from.point.x).hypot(to.point.y - from.point.y);
        assert!(route.total_length() > straight);

        // The same eased taper every curved paint slice uses stays gentle at
        // both ends and grows towards the destination.
        assert!((route.width_scale(0.0) - 0.78).abs() < f32::EPSILON);
        assert!(route.width_scale(0.5) > route.width_scale(0.0));
        assert!((route.width_scale(1.0) - 1.30).abs() < f32::EPSILON);
        let tangent = route.terminal_tangent();
        assert!(tangent.x > 0.0);
        assert!(tangent.y.abs() < 0.05);
    }

    #[test]
    fn lane_routes_keep_one_width_from_source_to_destination() {
        let route = OrthogonalRoute::new(vec![point(0.0, 0.0), point(40.0, 0.0)]);
        for progress in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert_eq!(route.width_scale(progress), 1.0);
        }
    }

    #[test]
    fn a_short_facing_hop_relaxes_without_looping_past_either_port() {
        // Two facing ports on one axis have no second fact for a decorative
        // bend to carry. The curve may settle to a line, but it may not cross
        // its handles and hook backwards on the short run.
        let from = anchor(PortSide::Right, bounds(0.0, 0.0));
        let to = anchor(PortSide::Left, bounds(60.0, 0.0));
        let route = route_curved(from, to);
        assert!(route.points()[1].x - from.point.x > 0.0);
        assert!(route.points().windows(2).all(|pair| pair[0].x <= pair[1].x));
        assert!(route.points().iter().all(|point| {
            point.x >= from.point.x && point.x <= to.point.x && point.y == from.point.y
        }));
    }

    #[test]
    fn a_bend_never_takes_more_than_half_the_run_it_leaves() {
        // A long approach into a short jog: the jog, not the requested
        // corner, decides how far the bend may reach.
        let radius = corner_radius(point(0.0, 0.0), point(100.0, 0.0), point(100.0, 6.0), 8.0);
        assert_eq!(radius, 3.0);

        // With room on both sides the requested corner is what is used.
        let radius = corner_radius(point(0.0, 0.0), point(100.0, 0.0), point(100.0, 90.0), 8.0);
        assert_eq!(radius, 8.0);
    }

    #[test]
    fn a_bend_at_a_turn_with_no_run_is_left_square() {
        // Two turns on top of each other would otherwise ask for a negative
        // reach and fold the path back through itself.
        let radius = corner_radius(point(10.0, 10.0), point(10.0, 10.0), point(10.0, 40.0), 8.0);
        assert_eq!(radius, 0.0);
    }

    #[test]
    fn stepping_back_from_a_turn_stays_on_the_run() {
        // Orthogonal by construction: exactly one axis moves, and it moves
        // towards the neighbour rather than away from it.
        assert_eq!(
            step_towards(point(100.0, 40.0), point(100.0, 90.0), 8.0),
            point(100.0, 48.0)
        );
        assert_eq!(
            step_towards(point(100.0, 40.0), point(20.0, 40.0), 8.0),
            point(92.0, 40.0)
        );
    }

    #[test]
    fn zero_length_is_safe_and_finite() {
        let r = OrthogonalRoute::new(vec![point(2.0, 3.0), point(2.0, 3.0)]);
        assert_eq!(r.total_length(), 0.0);
        assert_eq!(r.sample(f32::NAN), point(2.0, 3.0));
    }
    /// A return is a fact about control flow, not a failure. Drawn in the
    /// danger paint, as it was, a run that retried once and then succeeded
    /// has a red line through it forever.
    #[test]
    fn a_return_path_is_not_drawn_as_a_failure() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            assert_ne!(EdgeKind::Feedback.color(&theme), theme.colors.danger);
            assert_ne!(EdgeKind::Feedback.active_color(&theme), theme.colors.danger);
            // And it is still not the flow it returns from, in either state.
            assert_ne!(
                EdgeKind::Feedback.color(&theme),
                EdgeKind::Flow.color(&theme)
            );
            assert_ne!(
                EdgeKind::Feedback.active_color(&theme),
                EdgeKind::Flow.active_color(&theme)
            );
            // Traffic is louder than rest, whichever kind is carrying it.
            for kind in [EdgeKind::Flow, EdgeKind::Feedback] {
                assert_ne!(kind.color(&theme), kind.active_color(&theme));
            }
        }
    }

    /// A connection that is still arriving carries no traffic: a comet running
    /// down a wire that does not reach anywhere yet is traffic to a place the
    /// graph has not said exists.
    #[test]
    fn an_arriving_connection_is_drawn_from_the_port_it_leaves() {
        let colors = EdgeState::Idle.colors(EdgeKind::Flow, &Theme::studio_dark());
        let paint = EdgePaint::new(1.0, colors).reveal(0.4).phase(Some(0.7));
        assert_eq!(paint.reveal, 0.4);
        assert_eq!(paint.width, 1.0);
        // Out-of-range reveals are clamped rather than trusted, because the
        // clock they come from can overrun a frame.
        assert_eq!(EdgePaint::new(1.0, colors).reveal(1.4).reveal, 1.0);
        assert_eq!(EdgePaint::new(1.0, colors).reveal(-0.2).reveal, 0.0);
        // A settled edge is the default, so a caller that never mentions
        // arrival draws the whole connection.
        assert_eq!(EdgePaint::new(1.0, colors).reveal, 1.0);
        assert_eq!(EdgePaint::new(1.0, colors).phase, None);
    }

    #[test]
    fn identity_and_builders_are_stable() {
        let a = GraphEdge::new("one", "two")
            .ports("out", "in")
            .label("work")
            .active(true)
            .lane(3)
            .feedback();
        let other = GraphEdge::new("x", "y");
        assert_eq!(
            a.edge_id(),
            GraphEdge::new("one", "two")
                .ports("out", "in")
                .lane(3)
                .feedback()
                .edge_id()
        );
        assert_ne!(a.edge_id(), other.edge_id());
        assert_eq!(a.from(), "one");
        assert_eq!(a.to(), "two");
        assert_eq!(a.kind(), EdgeKind::Feedback);
        assert_eq!(a.source_port().expect("source port"), "out");
        assert_eq!(a.target_port().expect("target port"), "in");
        assert_eq!(a.edge_label().expect("edge label"), "work");
        assert!(a.is_active());
        assert_eq!(a.edge_state(), EdgeState::Active);
        assert_eq!(a.edge_marker(), EdgeMarker::None);
        assert_eq!(
            a.clone().marker(EdgeMarker::Arrow).edge_marker(),
            EdgeMarker::Arrow
        );
        assert!(!a.is_selected());
        assert!(a.clone().selected(true).is_selected());
        assert_eq!(
            a.clone().state(EdgeState::Failed).edge_state(),
            EdgeState::Failed
        );
        assert_eq!(a.edge_lane(), 3);
        assert_eq!(
            a.clone().id("business").edge_id(),
            SharedString::from("business")
        );
    }
}
