//! Static edge data and orthogonal geometry for a node graph.

use gpui::{Bounds, Hsla, PathBuilder, Pixels, Point, SharedString, Window, point, px};
use gpui_kit_theme::Theme;

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
            Self::Flow => theme.colors.hairline_strong,
            Self::Feedback => theme.colors.danger,
        }
    }

    fn dashes(self) -> Option<[Pixels; 2]> {
        (self == Self::Feedback).then(|| [px(5.0), px(4.0)])
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
    active: bool,
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
            active: false,
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
        self.active = active;
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
        self.active
    }
    pub(crate) fn edge_lane(&self) -> i16 {
        self.lane
    }
    pub(crate) fn identity(&self) -> SharedString {
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
}

impl OrthogonalRoute {
    fn new(points: Vec<Point<f32>>) -> Self {
        let points = normalize(points);
        let mut cumulative = vec![0.0];
        for pair in points.windows(2) {
            cumulative.push(
                cumulative.last().copied().unwrap_or(0.0)
                    + (pair[1].x - pair[0].x).abs()
                    + (pair[1].y - pair[0].y).abs(),
            );
        }
        let total = cumulative.last().copied().unwrap_or(0.0);
        Self {
            points,
            cumulative,
            total,
        }
    }
    pub(crate) fn points(&self) -> &[Point<f32>] {
        &self.points
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

    pub(crate) fn midpoint_axis(&self) -> Axis {
        if self.points.len() < 2 {
            return Axis::Horizontal;
        }
        let target = self.total * 0.5;
        let index = self
            .cumulative
            .partition_point(|length| *length < target)
            .clamp(1, self.points.len() - 1);
        if self.points[index - 1].x == self.points[index].x {
            Axis::Vertical
        } else {
            Axis::Horizontal
        }
    }
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
    let a = from.outward_point(lead_distance(from, to_bounds, preferred_lead)?);
    let b = to.outward_point(lead_distance(to, from_bounds, preferred_lead)?);
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

pub(crate) fn paint_route(
    window: &mut Window,
    theme: &Theme,
    edge: &GraphEdge,
    route: &OrthogonalRoute,
    transform: RouteTransform,
    width: f32,
    phase: Option<f32>,
) {
    let active_color = match edge.kind {
        EdgeKind::Flow => theme.colors.accent,
        EdgeKind::Feedback => theme.colors.danger,
    };
    if edge.active {
        paint_route_stroke(
            window,
            route,
            transform,
            width * 5.0,
            active_color.opacity(0.14),
            edge.kind.dashes(),
        );
    }
    paint_route_stroke(
        window,
        route,
        transform,
        width,
        edge.kind.color(theme),
        edge.kind.dashes(),
    );
    if edge.active {
        paint_route_stroke(
            window,
            route,
            transform,
            width * 1.2,
            active_color.opacity(0.72),
            edge.kind.dashes(),
        );
        if let Some(phase) = phase {
            paint_comets(
                window,
                route,
                transform,
                width.max(1.0),
                phase,
                active_color,
            );
        }
    }
}

/// Three phase-shifted traffic trails. Each trail is made from short straight
/// samples, so it follows square corners without reintroducing a curve mode.
fn paint_comets(
    window: &mut Window,
    route: &OrthogonalRoute,
    transform: RouteTransform,
    width: f32,
    phase: f32,
    color: Hsla,
) {
    const COMETS: usize = 3;
    const TAIL_STEPS: usize = 7;
    const TAIL: f32 = 0.075;

    for comet in 0..COMETS {
        let head = (phase + comet as f32 / COMETS as f32).rem_euclid(1.0);
        for step in 0..TAIL_STEPS {
            let end = head - TAIL * step as f32 / TAIL_STEPS as f32;
            let start = head - TAIL * (step + 1) as f32 / TAIL_STEPS as f32;
            // A wrapped tail resumes at the start of the route on the next
            // frame instead of drawing one false segment across the graph.
            if start < 0.0 || end < 0.0 {
                continue;
            }
            let mut builder = PathBuilder::stroke(px(width * (1.9 - step as f32 * 0.1)));
            builder.move_to(transform.point(route.sample(start)));
            builder.line_to(transform.point(route.sample(end)));
            if let Ok(path) = builder.build() {
                let opacity = 0.82 * (1.0 - step as f32 / TAIL_STEPS as f32).powf(1.4);
                window.paint_path(path, color.opacity(opacity));
            }
        }
    }
}

pub(crate) fn paint_route_stroke(
    window: &mut Window,
    route: &OrthogonalRoute,
    transform: RouteTransform,
    width: f32,
    color: Hsla,
    dashes: Option<[Pixels; 2]>,
) {
    let Some(first) = route.points.first() else {
        return;
    };
    let mut builder = PathBuilder::stroke(px(width));
    if let Some(dashes) = dashes {
        builder = builder.dash_array(&dashes);
    }
    builder.move_to(transform.point(*first));
    for point in &route.points[1..] {
        builder.line_to(transform.point(*point));
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
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
    #[test]
    fn overlapping_cards_are_omitted_and_self_links_route() {
        let a = bounds(50.0, 20.0);
        let overlapping = bounds(55.0, 25.0);
        assert!(
            route_orthogonal(
                anchor(PortSide::Right, a),
                anchor(PortSide::Left, overlapping),
                a,
                overlapping,
                EdgeKind::Flow,
                0,
            )
            .is_none()
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
        assert_eq!(r.sample(2.0), point(10.0, 30.0));
    }
    #[test]
    fn zero_length_is_safe_and_finite() {
        let r = OrthogonalRoute::new(vec![point(2.0, 3.0), point(2.0, 3.0)]);
        assert_eq!(r.total_length(), 0.0);
        assert_eq!(r.sample(f32::NAN), point(2.0, 3.0));
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
            a.identity(),
            GraphEdge::new("one", "two")
                .ports("out", "in")
                .lane(3)
                .feedback()
                .identity()
        );
        assert_ne!(a.identity(), other.identity());
        assert_eq!(a.from(), "one");
        assert_eq!(a.to(), "two");
        assert_eq!(a.kind(), EdgeKind::Feedback);
        assert_eq!(a.source_port().expect("source port"), "out");
        assert_eq!(a.target_port().expect("target port"), "in");
        assert_eq!(a.edge_label().expect("edge label"), "work");
        assert!(a.is_active());
        assert_eq!(a.edge_lane(), 3);
        assert_eq!(
            a.clone().id("business").identity(),
            SharedString::from("business")
        );
    }
}
