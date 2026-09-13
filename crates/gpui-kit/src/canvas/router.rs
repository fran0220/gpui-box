//! Global graph-space obstacle checks and bounded rectilinear detour search.
//!
//! The common path validates the existing lane without a search allocation.
//! Blocked paths discover obstacle coordinate planes lazily; only visited
//! (point, incoming heading) labels are stored, never a full Cartesian grid.
//! Failure describes this clearance/search model, not geometric impossibility.

use std::{
    cmp::Ordering,
    collections::{BTreeSet, BinaryHeap, HashMap},
};

use gpui::{Bounds, Point, point, size};

use super::{
    edge::{
        Anchor, OrthogonalRoute, PortSide, RouteMetrics, lead_distance, route_is_directional,
        segment_clear,
    },
    spatial::BoundsIndex,
};
use crate::strings::StringKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RouteStatus {
    Clear,
    Obstructed,
    SearchLimited,
}

impl RouteStatus {
    pub(super) fn warning(self) -> Option<(StringKey, &'static str)> {
        match self {
            Self::Clear => None,
            Self::Obstructed => Some((StringKey::GraphRouteObstructed, "obstructed")),
            Self::SearchLimited => Some((StringKey::GraphRouteSearchLimited, "search-limited")),
        }
    }
}

pub(super) struct Router {
    bounds: Vec<Bounds<f32>>,
    index: BoundsIndex,
    clearance: f32,
    corner: f32,
    remaining: usize,
}

// Coordinate construction and visited search labels both spend work. A dense
// diagram cannot multiply an unlimited per-edge search by its edge count.
const PER_ROUTE_WORK: usize = 32_768;
const BATCH_WORK: usize = 1_048_576;

fn envelope(a: Point<f32>, b: Point<f32>) -> Bounds<f32> {
    Bounds::new(
        point(a.x.min(b.x), a.y.min(b.y)),
        size((a.x - b.x).abs(), (a.y - b.y).abs()),
    )
}

fn inflate(bounds: Bounds<f32>, pad: f32) -> Bounds<f32> {
    Bounds::new(
        bounds.origin - point(pad, pad),
        bounds.size + size(2. * pad, 2. * pad),
    )
}

fn heading(side: PortSide) -> usize {
    match side {
        PortSide::Right => 0,
        PortSide::Bottom => 1,
        PortSide::Left => 2,
        PortSide::Top => 3,
    }
}

fn distance(a: Point<f32>, b: Point<f32>) -> f64 {
    (f64::from(a.x) - f64::from(b.x)).abs() + (f64::from(a.y) - f64::from(b.y)).abs()
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SearchState {
    cell: usize,
    heading: usize,
    loop_clearance: bool,
}

#[derive(Clone, Copy, PartialEq)]
struct Open {
    state: SearchState,
    cost: f64,
    estimate: f64,
}
impl Eq for Open {}
impl Ord for Open {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .estimate
            .total_cmp(&self.estimate)
            .then_with(|| other.state.cmp(&self.state))
            .then_with(|| other.cost.total_cmp(&self.cost))
    }
}
impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct Label {
    cost: f64,
    previous: Option<SearchState>,
}

impl Router {
    pub(super) fn new(
        bounds: impl IntoIterator<Item = Bounds<f32>>,
        corner: f32,
        stroke: f32,
    ) -> Self {
        let bounds: Vec<_> = bounds.into_iter().collect();
        let clearance = corner + stroke / 2.;
        Self {
            index: BoundsIndex::new(bounds.iter().map(|b| inflate(*b, clearance))),
            bounds,
            clearance,
            corner,
            remaining: BATCH_WORK,
        }
    }

    fn obstacle(&self, index: usize, owners: [usize; 2]) -> Bounds<f32> {
        if owners.contains(&index) {
            self.bounds[index]
        } else {
            inflate(self.bounds[index], self.clearance)
        }
    }

    fn blocker(&self, a: Point<f32>, b: Point<f32>, owners: [usize; 2]) -> Option<usize> {
        self.index.find(envelope(a, b), |index| {
            !segment_clear(a, b, self.obstacle(index, owners))
        })
    }

    fn corner_blocker(
        &self,
        a: Point<f32>,
        b: Point<f32>,
        c: Point<f32>,
        owners: [usize; 2],
        searching: bool,
    ) -> Option<usize> {
        if (a.x == b.x && b.x == c.x) || (a.y == b.y && b.y == c.y) {
            return None;
        }
        // Search neighbors may later merge into longer straight segments.
        // Reserve the full corner there; clipping it to one grid step would
        // allow the normalized, larger rendered turn to cut through a card.
        let radius = if searching {
            self.corner
        } else {
            f64::from(self.corner)
                .min(distance(a, b) / 2.)
                .min(distance(b, c) / 2.) as f32
        };
        let direction = |delta: f32| if delta == 0. { 0. } else { delta.signum() };
        let enter = b + point(direction(a.x - b.x) * radius, direction(a.y - b.y) * radius);
        let leave = b + point(direction(c.x - b.x) * radius, direction(c.y - b.y) * radius);
        let turn = envelope(enter, leave);
        self.index.find(turn, |index| {
            let obstacle = self.obstacle(index, owners);
            turn.left() < obstacle.right()
                && turn.right() > obstacle.left()
                && turn.top() < obstacle.bottom()
                && turn.bottom() > obstacle.top()
        })
    }

    fn lead(&self, anchor: Anchor, preferred: f32, owners: [usize; 2]) -> Option<Point<f32>> {
        let normal = anchor.side.outward();
        let end = anchor.point + point(normal.x * preferred, normal.y * preferred);
        let mut lead = preferred;
        for index in self.index.query(envelope(anchor.point, end), 0.) {
            lead = lead_distance(anchor, self.obstacle(index, owners), lead)?;
        }
        let end = anchor.point + point(normal.x * lead, normal.y * lead);
        (end != anchor.point && end.x.is_finite() && end.y.is_finite()).then_some(end)
    }

    /// The same clearance model validates both solved and displayed routes.
    pub(super) fn accepts(
        &self,
        route: &OrthogonalRoute,
        from: Anchor,
        to: Anchor,
        owners: [usize; 2],
    ) -> bool {
        route_is_directional(route, from, to)
            && route
                .points()
                .windows(2)
                .all(|p| self.blocker(p[0], p[1], owners).is_none())
            && route.points().windows(3).all(|p| {
                self.corner_blocker(p[0], p[1], p[2], owners, false)
                    .is_none()
            })
    }

    pub(super) fn resolve(
        &mut self,
        original: OrthogonalRoute,
        from: Anchor,
        to: Anchor,
        owners: [usize; 2],
        metrics: RouteMetrics,
    ) -> (OrthogonalRoute, RouteStatus) {
        if self.accepts(&original, from, to, owners) {
            return (original, RouteStatus::Clear);
        }
        let fallback = if route_is_directional(&original, from, to) {
            original
        } else {
            // A buried endpoint still keeps its hard outward stub. This is an
            // explicitly warned connection, not an obstacle-free solution.
            let a = from.point + from.side.outward() * metrics.lead;
            let b = to.point + to.side.outward() * metrics.lead;
            OrthogonalRoute::new(vec![from.point, a, point(b.x, a.y), b, to.point])
        };
        let (Some(a), Some(b)) = (
            self.lead(from, metrics.lead, owners),
            self.lead(to, metrics.lead, owners),
        ) else {
            return (fallback, RouteStatus::Obstructed);
        };
        if self.blocker(from.point, a, owners).is_some()
            || self.blocker(b, to.point, owners).is_some()
        {
            return (fallback, RouteStatus::Obstructed);
        }
        let mut work = self.remaining.min(PER_ROUTE_WORK);
        let before = work;
        let searched = self.search(from, to, owners, [a, b], metrics, &mut work);
        self.remaining -= before - work;
        match searched {
            Ok(points) => (OrthogonalRoute::new(points), RouteStatus::Clear),
            Err(status) => (fallback, status),
        }
    }

    fn search(
        &self,
        from: Anchor,
        to: Anchor,
        owners: [usize; 2],
        ends: [Point<f32>; 2],
        metrics: RouteMetrics,
        work: &mut usize,
    ) -> Result<Vec<Point<f32>>, RouteStatus> {
        let [a, b] = ends;
        let mut discovered = BTreeSet::from(owners);
        loop {
            let coordinate_work = discovered.len().saturating_mul(4).saturating_add(8);
            if *work < coordinate_work {
                return Err(RouteStatus::SearchLimited);
            }
            *work -= coordinate_work;
            let mut xs = vec![a.x, b.x];
            let mut ys = vec![a.y, b.y];
            for at in [a, b] {
                xs.extend([at.x - metrics.corridor, at.x + metrics.corridor]);
                ys.extend([at.y - metrics.corridor, at.y + metrics.corridor]);
            }
            for &index in &discovered {
                let bounds = self.obstacle(index, owners);
                xs.extend([
                    bounds.left() - metrics.corridor,
                    bounds.right() + metrics.corridor,
                ]);
                ys.extend([
                    bounds.top() - metrics.corridor,
                    bounds.bottom() + metrics.corridor,
                ]);
            }
            for axis in [&mut xs, &mut ys] {
                axis.retain(|value| value.is_finite());
                axis.sort_unstable_by(f32::total_cmp);
                axis.dedup();
            }
            let nx = xs.len();
            let locate = |at: Point<f32>| {
                // Both finite endpoints were inserted before sorting/deduplication.
                ys.partition_point(|y| *y < at.y) * nx + xs.partition_point(|x| *x < at.x)
            };
            let position = |cell: usize| point(xs[cell % nx], ys[cell / nx]);
            let start = SearchState {
                cell: locate(a),
                heading: heading(from.side),
                loop_clearance: from.point != to.point,
            };
            let goal = locate(b);
            let mut open = BinaryHeap::from([Open {
                state: start,
                cost: 0.,
                estimate: distance(a, b),
            }]);
            let mut labels = HashMap::from([(
                start,
                Label {
                    cost: 0.,
                    previous: None,
                },
            )]);
            let known = discovered.len();
            while let Some(candidate) = open.pop() {
                if *work == 0 {
                    return Err(RouteStatus::SearchLimited);
                }
                *work -= 1;
                let label = &labels[&candidate.state];
                if label.cost != candidate.cost {
                    continue;
                }
                let cell = candidate.state.cell;
                let incoming = candidate.state.heading;
                let at = position(cell);
                let previous = label
                    .previous
                    .map(|state| position(state.cell))
                    .unwrap_or(from.point);
                if cell == goal && candidate.state.loop_clearance && incoming != heading(to.side) {
                    if let Some(blocker) = self.corner_blocker(previous, at, to.point, owners, true)
                    {
                        discovered.insert(blocker);
                    } else {
                        let mut path = vec![to.point];
                        let mut cursor = Some(candidate.state);
                        while let Some(state) = cursor {
                            path.push(position(state.cell));
                            cursor = labels[&state].previous;
                        }
                        path.push(from.point);
                        path.reverse();
                        return Ok(path);
                    }
                }
                let x = cell % nx;
                let y = cell / nx;
                for direction in 0..4 {
                    if direction == (incoming + 2) % 4 {
                        continue;
                    }
                    let next = match direction {
                        0 if x + 1 < nx => cell + 1,
                        1 if y + 1 < ys.len() => cell + nx,
                        2 if x > 0 => cell - 1,
                        3 if y > 0 => cell - nx,
                        _ => continue,
                    };
                    let point = position(next);
                    if let Some(blocker) = self
                        .blocker(at, point, owners)
                        .or_else(|| self.corner_blocker(previous, at, point, owners, true))
                    {
                        discovered.insert(blocker);
                        continue;
                    }
                    let cost = candidate.cost
                        + distance(at, point)
                        + if direction == incoming {
                            0.
                        } else {
                            f64::from(RouteMetrics::BEND_PENALTY)
                        };
                    let normal = from.side.outward();
                    let delta = point - from.point;
                    let loop_clearance = delta.x * normal.x + delta.y * normal.y
                        >= metrics.lead + metrics.corridor
                        && (delta.x * normal.y - delta.y * normal.x).abs() >= metrics.corridor;
                    let state = SearchState {
                        cell: next,
                        heading: direction,
                        loop_clearance: candidate.state.loop_clearance || loop_clearance,
                    };
                    if labels.get(&state).is_some_and(|old| old.cost <= cost) {
                        continue;
                    }
                    labels.insert(
                        state,
                        Label {
                            cost,
                            previous: Some(candidate.state),
                        },
                    );
                    open.push(Open {
                        state,
                        cost,
                        estimate: cost + distance(point, b),
                    });
                }
            }
            if discovered.len() == known {
                return Err(RouteStatus::Obstructed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::edge::{EdgeKind, route_orthogonal};
    use super::*;

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Bounds<f32> {
        Bounds::new(point(x, y), size(width, height))
    }

    fn setup() -> (Vec<Bounds<f32>>, Anchor, Anchor, RouteMetrics) {
        (
            vec![
                rect(0., 20., 40., 30.),
                rect(300., 70., 40., 30.),
                rect(130., -40., 55., 210.),
            ],
            Anchor {
                point: point(40., 35.),
                side: PortSide::Right,
            },
            Anchor {
                point: point(300., 85.),
                side: PortSide::Left,
            },
            RouteMetrics {
                lead: 20.,
                corridor: 16.,
                lane: 12.,
            },
        )
    }

    fn original(
        bounds: &[Bounds<f32>],
        a: Anchor,
        b: Anchor,
        metrics: RouteMetrics,
    ) -> OrthogonalRoute {
        route_orthogonal(a, b, bounds[0], bounds[1], EdgeKind::Flow, 0, metrics)
            .expect("endpoint route")
    }

    // Independent slab intersection, not the production segment-clear helper.
    fn crosses(a: Point<f32>, b: Point<f32>, obstacle: Bounds<f32>) -> bool {
        let mut low = 0f64;
        let mut high = 1f64;
        for (start, end, min, max) in [
            (a.x, b.x, obstacle.left(), obstacle.right()),
            (a.y, b.y, obstacle.top(), obstacle.bottom()),
        ] {
            let delta = f64::from(end) - f64::from(start);
            if delta == 0. {
                if start <= min || start >= max {
                    return false;
                }
            } else {
                let first = (f64::from(min) - f64::from(start)) / delta;
                let last = (f64::from(max) - f64::from(start)) / delta;
                low = low.max(first.min(last));
                high = high.min(first.max(last));
            }
        }
        low < high
    }

    #[test]
    fn short_port_leads_reserve_unrelated_stroke_clearance() {
        let (mut bounds, from, to, metrics) = setup();
        bounds[2] = rect(50., 20., 10., 30.);
        let mut router = Router::new(bounds.iter().copied(), 1., 10.);
        assert_eq!(
            router.lead(from, metrics.lead, [0, 1]),
            Some(point(42., 35.))
        );
        let (route, status) = router.resolve(
            original(&bounds, from, to, metrics),
            from,
            to,
            [0, 1],
            metrics,
        );
        assert_eq!(status, RouteStatus::Clear);
        assert!(
            route
                .points()
                .windows(2)
                .all(|p| !crosses(p[0], p[1], rect(44., 14., 22., 42.)))
        );
    }

    #[test]
    fn detour_clears_unrelated_obstacles_and_keeps_exact_fixed_ports() {
        let (bounds, from, to, metrics) = setup();
        let before = original(&bounds, from, to, metrics);
        assert!(
            before
                .points()
                .windows(2)
                .any(|p| crosses(p[0], p[1], bounds[2]))
        );
        let mut router = Router::new(bounds.iter().copied(), 6., 2.);
        let (route, status) = router.resolve(before, from, to, [0, 1], metrics);
        assert_eq!(status, RouteStatus::Clear, "{:?}", route.points());
        assert_eq!(route.points().first(), Some(&from.point));
        assert_eq!(route.points().last(), Some(&to.point));
        assert!(route.points()[1].x > from.point.x);
        assert_eq!(route.points()[1].y, from.point.y);
        let before_end = route.points()[route.points().len() - 2];
        assert!(before_end.x < to.point.x);
        assert_eq!(before_end.y, to.point.y);
        for segment in route.points().windows(2) {
            assert!(segment[0].x == segment[1].x || segment[0].y == segment[1].y);
            for obstacle in &bounds {
                assert!(!crosses(segment[0], segment[1], *obstacle));
            }
        }
        assert!(route.points().iter().any(|p| p.y < -40. || p.y > 170.));
    }

    #[test]
    fn buried_ports_and_exhausted_search_have_distinct_warned_fallbacks() {
        let (mut bounds, from, to, metrics) = setup();
        let mut limited = Router::new(bounds.iter().copied(), 6., 2.);
        limited.remaining = 0;
        let (fallback, status) = limited.resolve(
            original(&bounds, from, to, metrics),
            from,
            to,
            [0, 1],
            metrics,
        );
        assert_eq!(status, RouteStatus::SearchLimited);
        assert!(route_is_directional(&fallback, from, to));
        bounds.push(rect(30., 25., 40., 20.));
        let mut buried = Router::new(bounds.iter().copied(), 6., 2.);
        buried.remaining = 0;
        let (fallback, status) = buried.resolve(
            original(&bounds, from, to, metrics),
            from,
            to,
            [0, 1],
            metrics,
        );
        assert_eq!(status, RouteStatus::Obstructed);
        assert!(route_is_directional(&fallback, from, to));
        use crate::strings::TranslationPack;
        let pack = TranslationPack::SimplifiedChinese;
        assert_eq!(
            pack.text(status.warning().expect("warning").0),
            "未找到避开障碍的路径"
        );
        assert_eq!(
            pack.text(RouteStatus::SearchLimited.warning().expect("warning").0),
            "路径搜索已达到上限"
        );
    }

    #[test]
    fn valid_lane_does_not_spend_search_budget_with_one_hundred_thousand_obstacles() {
        let (mut bounds, from, to, metrics) = setup();
        bounds.truncate(2);
        bounds.extend((0..100_000).map(|i| rect(i as f32 * 100., 500., 20., 30.)));
        let candidate = original(&bounds, from, to, metrics);
        let expected = candidate.points().to_vec();
        let mut router = Router::new(bounds, 6., 2.);
        router.remaining = 0;
        let (route, status) = router.resolve(candidate, from, to, [0, 1], metrics);
        assert_eq!(status, RouteStatus::Clear);
        assert_eq!(route.points(), expected);
        assert_eq!(router.remaining, 0);
    }

    #[test]
    fn all_port_orientations_and_rounded_detours_clear_the_whole_graph() {
        let (bounds, _, _, metrics) = setup();
        let at = |side, bounds: Bounds<f32>| Anchor {
            side,
            point: match side {
                PortSide::Left => point(bounds.left(), bounds.center().y),
                PortSide::Right => point(bounds.right(), bounds.center().y),
                PortSide::Top => point(bounds.center().x, bounds.top()),
                PortSide::Bottom => point(bounds.center().x, bounds.bottom()),
            },
        };
        for first in [
            PortSide::Left,
            PortSide::Right,
            PortSide::Top,
            PortSide::Bottom,
        ] {
            for last in [
                PortSide::Left,
                PortSide::Right,
                PortSide::Top,
                PortSide::Bottom,
            ] {
                let from = at(first, bounds[0]);
                let to = at(last, bounds[1]);
                let mut router = Router::new(bounds.iter().copied(), 6., 2.);
                let (route, status) = router.resolve(
                    original(&bounds, from, to, metrics),
                    from,
                    to,
                    [0, 1],
                    metrics,
                );
                assert_eq!(status, RouteStatus::Clear, "{first:?} → {last:?}");
                assert!(route_is_directional(&route, from, to));
                for p in route.points().windows(2) {
                    assert!(bounds.iter().all(|b| !crosses(p[0], p[1], *b)));
                }
                for p in route.points().windows(3) {
                    let incoming = p[1] - p[0];
                    let outgoing = p[2] - p[1];
                    let before = incoming.x.hypot(incoming.y);
                    let after = outgoing.x.hypot(outgoing.y);
                    let radius = 6f32.min(before / 2.).min(after / 2.);
                    let enter = p[1] - incoming * (radius / before);
                    let leave = p[1] + outgoing * (radius / after);
                    for step in 0..=100 {
                        let t = step as f32 / 100.;
                        let point = enter * (1. - t).powi(2)
                            + p[1] * (2. * t * (1. - t))
                            + leave * t.powi(2);
                        for obstacle in &bounds {
                            assert!(
                                !(point.x > obstacle.left()
                                    && point.x < obstacle.right()
                                    && point.y > obstacle.top()
                                    && point.y < obstacle.bottom()),
                                "rounded turn entered {obstacle:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_blocked_self_loop_keeps_a_visible_loop_instead_of_collapsing_to_a_stub() {
        let bounds = [rect(0., 20., 40., 30.), rect(70., 40., 30., 40.)];
        let from = Anchor {
            point: point(40., 35.),
            side: PortSide::Right,
        };
        let metrics = RouteMetrics {
            lead: 20.,
            corridor: 16.,
            lane: 12.,
        };
        let before = route_orthogonal(
            from,
            from,
            bounds[0],
            bounds[0],
            EdgeKind::Feedback,
            0,
            metrics,
        )
        .expect("self loop");
        assert!(
            before
                .points()
                .windows(2)
                .any(|p| crosses(p[0], p[1], bounds[1]))
        );
        let mut router = Router::new(bounds.iter().copied(), 6., 2.);
        let (route, status) = router.resolve(before, from, from, [0, 0], metrics);
        assert_eq!(status, RouteStatus::Clear);
        assert!(route_is_directional(&route, from, from));
        assert!(
            route
                .points()
                .iter()
                .any(|p| p.x >= 76. && (p.y - 35.).abs() >= 16.)
        );
        for p in route.points().windows(2) {
            assert!(bounds.iter().all(|b| !crosses(p[0], p[1], *b)));
        }
    }

    #[test]
    fn progressively_taller_walls_exhaust_the_real_budget_without_erasing_the_connection() {
        let count = 1_000;
        let mut bounds = vec![
            rect(0., 140., 120., 80.),
            rect(count as f32 * 200. + 300., 140., 120., 80.),
        ];
        bounds.extend((0..count).map(|i| {
            rect(
                240. + i as f32 * 200.,
                90. - i as f32 * 50.,
                120.,
                180. + i as f32 * 100.,
            )
        }));
        let from = Anchor {
            point: point(120., 180.),
            side: PortSide::Right,
        };
        let to = Anchor {
            point: point(bounds[1].left(), 180.),
            side: PortSide::Left,
        };
        let metrics = RouteMetrics {
            lead: 24.,
            corridor: 24.,
            lane: 12.,
        };
        let mut router = Router::new(bounds.iter().copied(), 6., 2.);
        let (route, status) = router.resolve(
            original(&bounds, from, to, metrics),
            from,
            to,
            [0, 1],
            metrics,
        );
        assert_eq!(status, RouteStatus::SearchLimited);
        assert_eq!(route.points().first(), Some(&from.point));
        assert_eq!(route.points().last(), Some(&to.point));
    }
}
