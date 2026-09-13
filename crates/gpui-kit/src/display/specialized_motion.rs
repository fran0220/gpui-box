//! Visual-only keyed polygon motion; removed records never reenter live data.
use super::{Shape, SpecializedData};
use crate::motion::{MotionSpec, Transition};
use gpui::{Point, SharedString};
use std::collections::{BTreeMap, HashSet};
use std::time::Duration;

struct Animated {
    shape: Shape,
    points: Vec<Transition<Point<f32>>>,
    opacity: Transition<f32>,
}

#[derive(Default)]
pub(super) struct ShapeMotion {
    shapes: BTreeMap<SharedString, Animated>,
    last: Option<web_time::Instant>,
    spec: Option<MotionSpec>,
}

impl ShapeMotion {
    pub(super) fn animate(
        &mut self,
        data: &mut SpecializedData,
        timing: Option<MotionSpec>,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> Vec<(Shape, f32)> {
        let now = cx.background_executor().now();
        let delta = self
            .last
            .map(|last| now.saturating_duration_since(last))
            .unwrap_or_default();
        let spec = timing.unwrap_or_else(|| {
            crate::motion::MotionPolicy::resolve(crate::motion::MotionRole::Resize, cx).spec()
        });
        let result = self.update(data, spec, delta, cx.reduce_motion());
        if self
            .shapes
            .values()
            .any(|a| a.opacity.is_animating() || a.points.iter().any(Transition::is_animating))
        {
            self.last = Some(now);
            window.request_animation_frame();
        } else {
            self.last = None;
        }
        result
    }

    pub(super) fn update(
        &mut self,
        data: &mut SpecializedData,
        spec: MotionSpec,
        delta: Duration,
        reduced: bool,
    ) -> Vec<(Shape, f32)> {
        fn sample<T: crate::motion::Interpolate>(
            t: &mut Transition<T>,
            delta: Duration,
            reduced: bool,
        ) -> T {
            if reduced {
                t.snap(t.target());
            } else {
                t.advance(delta);
            }
            t.value()
        }
        let live: HashSet<_> = data.shapes.iter().map(|s| s.item.id.clone()).collect();
        if self.spec.is_some_and(|previous| previous != spec) {
            for shape in self.shapes.values_mut() {
                crate::display::plot::retime(&mut shape.opacity, spec);
                for point in &mut shape.points {
                    crate::display::plot::retime(point, spec);
                }
            }
        }
        self.spec = Some(spec);
        let mut exits = Vec::new();
        self.shapes.retain(|id, animated| {
            if live.contains(id) {
                return true;
            }
            animated.opacity = animated.opacity.spec(spec);
            animated.opacity.set(0.0);
            let alpha = sample(&mut animated.opacity, delta, reduced).clamp(0.0, 1.0);
            for (p, t) in animated.shape.points.iter_mut().zip(&mut animated.points) {
                *p = sample(t, delta, reduced);
            }
            if alpha > 0.0 {
                exits.push((animated.shape.clone(), alpha));
            }
            animated.opacity.is_animating()
        });
        for shape in &mut data.shapes {
            let animated = self
                .shapes
                .entry(shape.item.id.clone())
                .or_insert_with(|| Animated {
                    shape: shape.clone(),
                    points: shape
                        .points
                        .iter()
                        .map(|p| Transition::new(*p, spec))
                        .collect(),
                    opacity: Transition::new(0.0, spec),
                });
            // Topology changes (including annular tessellation counts) use the
            // displayed vertex sequence, rather than jumping to old targets.
            if animated.points.len() != shape.points.len() {
                let old: Vec<_> = animated.points.iter().map(Transition::value).collect();
                animated.points = (0..shape.points.len())
                    .map(|i| {
                        let at = i as f32 * (old.len() - 1) as f32
                            / (shape.points.len() - 1).max(1) as f32;
                        let left = at.floor() as usize;
                        let right = (left + 1).min(old.len() - 1);
                        let t = at.fract();
                        Transition::new(
                            gpui::point(
                                old[left].x * (1.0 - t) + old[right].x * t,
                                old[left].y * (1.0 - t) + old[right].y * t,
                            ),
                            spec,
                        )
                    })
                    .collect();
            }
            for (point, transition) in shape.points.iter_mut().zip(&mut animated.points) {
                *transition = transition.spec(spec);
                let target = *point;
                transition.set(target);
                *point = sample(transition, delta, reduced);
                // Spring overshoot cannot escape the plot clip or semantics.
                point.x = point.x.clamp(0.0, 1.0);
                point.y = point.y.clamp(0.0, 1.0);
                if *point != transition.value() {
                    transition.snap(*point);
                    transition.set(target);
                }
            }
            animated.opacity = animated.opacity.spec(spec);
            animated.opacity.set(1.0);
            let alpha = sample(&mut animated.opacity, delta, reduced).clamp(0.0, 1.0);
            animated.shape = shape.clone();
            exits.push((shape.clone(), alpha));
        }
        exits
    }
}

#[cfg(test)]
mod tests {
    use super::super::{WeightedValue, rectangle};
    use super::*;
    use crate::motion::CubicBezier;
    use gpui::{point, size};

    fn data(x: f32, id: &str, value: f64) -> SpecializedData {
        SpecializedData {
            shapes: vec![rectangle(
                WeightedValue::new(id.to_owned(), id.to_owned(), value),
                x,
                0.2,
                0.2,
                0.3,
            )],
            ..Default::default()
        }
    }

    #[test]
    fn retarget_reinsert_and_exit_keep_exact_values_and_live_picking_separate() {
        let spec = MotionSpec::new(1000, CubicBezier::new(0.0, 0.0, 1.0, 1.0));
        let mut motion = ShapeMotion::default();
        let mut initial = data(0.1, "a", 3.0);
        let entering = motion.update(&mut initial, spec, Duration::ZERO, false);
        assert_eq!(entering[0].1, 0.0);
        motion.update(
            &mut data(0.1, "a", 3.0),
            spec,
            Duration::from_secs(1),
            false,
        );
        let mut changed = data(0.7, "a", 27.0);
        motion.update(&mut changed, spec, Duration::from_millis(500), false);
        assert!((changed.shapes[0].points[0].x - 0.4).abs() < 0.001);
        assert_eq!(changed.shapes[0].item.value, 27.0);
        assert_eq!(
            changed
                .hit_test(point(0.5, 0.3), size(300.0, 200.0), 1.0)
                .as_deref(),
            Some("a")
        );
        assert_eq!(
            changed.hit_test(point(0.8, 0.3), size(300.0, 200.0), 1.0),
            None
        );
        let mut interrupted = data(0.2, "a", 11.0);
        motion.update(&mut interrupted, spec, Duration::ZERO, false);
        assert!((interrupted.shapes[0].points[0].x - 0.4).abs() < 0.001);
        let mut removed = SpecializedData::default();
        let exits = motion.update(&mut removed, spec, Duration::from_millis(250), false);
        assert_eq!(exits.len(), 1);
        assert!(exits[0].1 > 0.0 && exits[0].1 < 1.0);
        assert_eq!(
            removed.hit_test(point(0.4, 0.3), size(300.0, 200.0), 1.0),
            None
        );
        let mut reinserted = data(0.6, "a", 19.0);
        let shown = motion.update(&mut reinserted, spec, Duration::ZERO, false);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].1, exits[0].1);
        assert_eq!(shown[0].0.item.value, 19.0);
        let mut reduced = data(0.7, "a", 23.0);
        let shown = motion.update(&mut reduced, spec, Duration::ZERO, true);
        assert_eq!(shown[0].0.points[0].x, 0.7);
        assert_eq!(shown[0].1, 1.0);
        assert!(
            motion
                .update(&mut SpecializedData::default(), spec, Duration::ZERO, true)
                .is_empty()
        );
        assert!(motion.shapes.is_empty());
    }
}
