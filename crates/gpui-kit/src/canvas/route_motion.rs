//! Compatible lane corridors interpolate; current ports and global clearance
//! always win. A changed obstacle-side topology cannot safely crossfade through
//! an obstacle. Invalid samples adopt the solved target without a ghost route.

use std::collections::HashMap;

use gpui::{Point, SharedString};
use web_time::Instant;

use super::edge::{GraphEdge, OrthogonalRoute};
use crate::motion::MotionSpec;

struct MovingRoute {
    edge: GraphEdge,
    from: Vec<Point<f32>>,
    target: Vec<Point<f32>>,
    started: Option<Instant>,
    spec: MotionSpec,
    seen: u64,
}

impl MovingRoute {
    fn at(&self, now: Instant) -> Vec<Point<f32>> {
        let Some(started) = self.started else {
            return self.target.clone();
        };
        let t = now.saturating_duration_since(started).as_secs_f32()
            / self.spec.total().as_secs_f32().max(f32::EPSILON);
        if t >= 1. {
            return self.target.clone();
        }
        let t = self.spec.progress(t.clamp(0., 1.));
        self.from
            .iter()
            .zip(&self.target)
            .map(|(a, b)| *a + (*b - *a) * t)
            .collect()
    }
}

#[derive(Default)]
pub(super) struct RouteMotion {
    routes: HashMap<SharedString, MovingRoute>,
    generation: u64,
}

fn compatible(a: &[Point<f32>], b: &[Point<f32>]) -> bool {
    a.len() == b.len()
        && a.len() >= 2
        && a.windows(2).zip(b.windows(2)).all(|(a, b)| {
            let horizontal = a[0].y == a[1].y && b[0].y == b[1].y;
            let vertical = a[0].x == a[1].x && b[0].x == b[1].x;
            horizontal || vertical
        })
}

/// Keep the first/last segment on the current port axis, without moving the
/// current ports back towards a previous caller geometry publication.
fn pin(points: &mut [Point<f32>], target: &[Point<f32>]) {
    let last = points.len() - 1;
    points[0] = target[0];
    points[last] = target[last];
    if last > 1 {
        if target[0].y == target[1].y {
            points[1].y = target[0].y;
        } else {
            points[1].x = target[0].x;
        }
        if target[last].y == target[last - 1].y {
            points[last - 1].y = target[last].y;
        } else {
            points[last - 1].x = target[last].x;
        }
    }
}

impl RouteMotion {
    pub(super) fn begin(&mut self) {
        self.generation += 1;
    }

    pub(super) fn sample(
        &mut self,
        edge: &GraphEdge,
        target: &OrthogonalRoute,
        now: Instant,
        spec: MotionSpec,
        accepts: impl FnOnce(&OrthogonalRoute) -> bool,
    ) -> (OrthogonalRoute, bool) {
        let entry = self
            .routes
            .entry(edge.edge_id())
            .or_insert_with(|| MovingRoute {
                edge: edge.clone(),
                from: target.points().to_vec(),
                target: target.points().to_vec(),
                started: None,
                spec,
                seen: self.generation,
            });
        entry.seen = self.generation;
        let same_endpoints = entry.edge.from() == edge.from()
            && entry.edge.to() == edge.to()
            && entry.edge.source_port() == edge.source_port()
            && entry.edge.target_port() == edge.target_port();
        if !same_endpoints || !compatible(&entry.target, target.points()) {
            entry.from = target.points().to_vec();
            entry.target = target.points().to_vec();
            entry.started = None;
        } else if entry.target != target.points() || (entry.spec != spec && entry.started.is_some())
        {
            // Sample the old timing before adopting the new timing, including
            // a same-frame target change. Settled routes need no new travel.
            entry.from = entry.at(now);
            pin(&mut entry.from, target.points());
            entry.target = target.points().to_vec();
            entry.started = Some(now);
        }
        entry.spec = spec;
        entry.edge = edge.clone();
        if entry.started.is_none() {
            return (target.clone(), false);
        }
        let mut points = entry.at(now);
        pin(&mut points, target.points());
        let finite = points.iter().all(|p| p.x.is_finite() && p.y.is_finite());
        let candidate = OrthogonalRoute::new(points);
        if !finite || !compatible(candidate.points(), target.points()) || !accepts(&candidate) {
            entry.started = None;
            entry.from.clone_from(&entry.target);
            return (target.clone(), false);
        }
        let moving = candidate.points() != target.points();
        if !moving {
            entry.started = None;
        }
        (candidate, moving)
    }

    pub(super) fn finish(&mut self) {
        self.routes.retain(|_, route| route.seen == self.generation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{
        edge::{Anchor, PortSide},
        router::Router,
    };
    use gpui::{Bounds, point, size};
    use std::time::Duration;

    fn lane(y: f32) -> OrthogonalRoute {
        OrthogonalRoute::new(vec![
            point(0., 0.),
            point(30., 0.),
            point(30., y),
            point(270., y),
            point(270., 0.),
            point(300., 0.),
        ])
    }
    fn spec() -> MotionSpec {
        MotionSpec::new(100, crate::motion::CubicBezier::new(0., 0., 1., 1.))
    }

    #[test]
    fn timing_changes_preserve_same_time_sample_with_or_without_retarget() {
        // x(t) = t, y(t) = t²: half the new duration travels one quarter
        // of the remaining distance, independently of MotionSpec::progress.
        let quadratic = crate::motion::CubicBezier::new(1. / 3., 0., 2. / 3., 1. / 3.);
        for (new_spec, fraction) in [
            (MotionSpec::new(200, spec().curve), 0.5),
            (MotionSpec::new(100, quadratic), 0.25),
            (MotionSpec::new(200, quadratic), 0.25),
        ] {
            for target in [100., 180.] {
                let edge = GraphEdge::new("a", "b").id("stable-wire");
                let mut motion = RouteMotion::default();
                let now = Instant::now();
                motion.sample(&edge, &lane(20.), now, spec(), |_| true);
                motion.sample(&edge, &lane(100.), now, spec(), |_| true);
                let later = now + Duration::from_millis(25);
                let (before, active) = motion.sample(&edge, &lane(100.), later, spec(), |_| true);
                assert!(active);
                assert!((before.points()[2].y - 40.).abs() < 0.01);
                let (after, active) =
                    motion.sample(&edge, &lane(target), later, new_spec, |_| true);
                assert!(active);
                assert_eq!(
                    after.points(),
                    before.points(),
                    "timing change for target {target}: {new_spec:?}"
                );
                // Repeated renders at the same instant must not restart motion.
                assert_eq!(
                    motion
                        .sample(&edge, &lane(target), later, new_spec, |_| true)
                        .0
                        .points(),
                    before.points()
                );
                let half = later + Duration::from_millis(new_spec.duration_ms / 2);
                let (middle, active) =
                    motion.sample(&edge, &lane(target), half, new_spec, |_| true);
                assert!(active);
                assert!((middle.points()[2].y - (40. + (target - 40.) * fraction)).abs() < 0.01);
                let (end, active) = motion.sample(
                    &edge,
                    &lane(target),
                    later + new_spec.total(),
                    new_spec,
                    |_| true,
                );
                assert_eq!(end.points(), lane(target).points());
                assert!(!active);
            }
        }
    }

    #[test]
    fn intermediate_retarget_pins_ports_and_removal_drops_old_corridor() {
        let mut motion = RouteMotion::default();
        let edge = GraphEdge::new("a", "b").id("business-edge");
        let now = Instant::now();
        motion.begin();
        assert!(!motion.sample(&edge, &lane(20.), now, spec(), |_| true).1);
        assert_eq!(
            motion
                .sample(&edge, &lane(100.), now, spec(), |_| true)
                .0
                .points(),
            lane(20.).points()
        );
        let later = now + Duration::from_millis(25);
        let (middle, active) = motion.sample(&edge, &lane(100.), later, spec(), |_| true);
        assert!(active);
        assert!((middle.points()[2].y - 40.).abs() < 0.01);
        assert_eq!(
            motion
                .sample(&edge, &lane(180.), later, spec(), |_| true)
                .0
                .points(),
            middle.points()
        );
        let mut shifted = lane(180.).points().to_vec();
        shifted[0].y = 13.;
        shifted[1].y = 13.;
        let shifted = OrthogonalRoute::new(shifted);
        let (pinned, _) = motion.sample(&edge, &shifted, later, spec(), |_| true);
        assert_eq!(pinned.points()[0], point(0., 13.));
        assert_eq!(pinned.points()[1].y, 13.);
        motion.finish();
        motion.begin();
        motion.finish();
        assert!(motion.routes.is_empty());
        assert!(!motion.sample(&edge, &lane(77.), later, spec(), |_| true).1);
    }

    #[test]
    fn real_obstacle_validation_snaps_without_painting_a_crossing_sample() {
        let router = Router::new(
            [
                Bounds::new(point(-80., -20.), size(80., 40.)),
                Bounds::new(point(300., -20.), size(80., 40.)),
                Bounds::new(point(100., 40.), size(80., 30.)),
            ],
            4.,
            2.,
        );
        let accepts = |route: &OrthogonalRoute| {
            router.accepts(
                route,
                Anchor {
                    point: point(0., 0.),
                    side: PortSide::Right,
                },
                Anchor {
                    point: point(300., 0.),
                    side: PortSide::Left,
                },
                [0, 1],
            )
        };
        assert!(accepts(&lane(20.)) && accepts(&lane(100.)));
        assert!(!accepts(&lane(60.)));
        let edge = GraphEdge::new("a", "b");
        let mut motion = RouteMotion::default();
        let now = Instant::now();
        motion.sample(&edge, &lane(20.), now, spec(), accepts);
        motion.sample(&edge, &lane(100.), now, spec(), accepts);
        let (shown, active) = motion.sample(
            &edge,
            &lane(100.),
            now + Duration::from_millis(50),
            spec(),
            accepts,
        );
        assert_eq!(shown.points(), lane(100.).points());
        assert!(!active);
    }

    #[test]
    fn incompatible_topology_or_reused_business_id_with_new_ports_snaps() {
        let edge = GraphEdge::new("a", "b").id("wire").ports("out", "in");
        let mut motion = RouteMotion::default();
        let now = Instant::now();
        motion.sample(&edge, &lane(20.), now, spec(), |_| true);
        let replacement = edge.clone().ports("other", "in");
        let (shown, active) = motion.sample(&replacement, &lane(100.), now, spec(), |_| true);
        assert_eq!(shown.points(), lane(100.).points());
        assert!(!active);
        let straight = OrthogonalRoute::new(vec![point(0., 0.), point(300., 0.)]);
        let (shown, active) = motion.sample(&replacement, &straight, now, spec(), |_| true);
        assert_eq!(shown.points(), straight.points());
        assert!(!active);
    }
}
