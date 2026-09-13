//! Displayed graph-space boxes. Caller metadata and editor proposals never
//! pass through these transitions; cards and routes share their sampled boxes.

use std::collections::HashMap;

use gpui::{Bounds, Point, SharedString, Size};
use web_time::Instant;

use crate::motion::{MotionSpec, Transition};

struct MovingBox {
    origin: Transition<Point<f32>>,
    size: Transition<Size<f32>>,
    last: Instant,
    seen: u64,
}

#[derive(Default)]
pub(super) struct GeometryMotion {
    boxes: HashMap<SharedString, MovingBox>,
    generation: u64,
}

impl GeometryMotion {
    pub(super) fn begin(&mut self) {
        self.generation += 1;
    }

    pub(super) fn sample(
        &mut self,
        id: &SharedString,
        target: Bounds<f32>,
        snap: bool,
        now: Instant,
        spec: MotionSpec,
    ) -> (Bounds<f32>, bool) {
        let entry = self.boxes.entry(id.clone()).or_insert_with(|| MovingBox {
            origin: Transition::new(target.origin, spec),
            size: Transition::new(target.size, spec),
            last: now,
            seen: self.generation,
        });
        entry.seen = self.generation;
        let elapsed = now.saturating_duration_since(entry.last);
        entry.last = now;
        entry.origin = entry.origin.spec(spec);
        entry.size = entry.size.spec(spec);
        entry.origin.advance(elapsed);
        entry.size.advance(elapsed);
        if snap {
            entry.origin.snap(target.origin);
            entry.size.snap(target.size);
        } else {
            entry.origin.set(target.origin);
            entry.size.set(target.size);
        }
        let mut bounds = Bounds::new(entry.origin.value(), entry.size.value());
        // A spring may overshoot a drastic narrowing. A nonpositive extent
        // cannot define coherent card layout, socket anchors and hit bounds.
        if ![bounds.left(), bounds.top(), bounds.right(), bounds.bottom()]
            .into_iter()
            .all(f32::is_finite)
            || bounds.size.width <= 0.
            || bounds.size.height <= 0.
        {
            entry.origin.snap(target.origin);
            entry.size.snap(target.size);
            bounds = target;
        }
        (
            bounds,
            entry.origin.is_animating() || entry.size.is_animating(),
        )
    }

    pub(super) fn finish(&mut self) {
        self.boxes.retain(|_, entry| entry.seen == self.generation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, size};
    use std::time::Duration;

    #[test]
    fn retarget_keeps_displayed_box_then_direct_manipulation_snaps() {
        let spec = crate::motion::MotionPolicy::spec(
            crate::motion::MotionRole::Navigation,
            &gpui_kit_theme::Theme::studio_light(),
        );
        let now = Instant::now();
        let a = Bounds::new(point(17., 31.), size(210., 90.));
        let b = Bounds::new(point(417., 93.), size(120., 240.));
        let c = Bounds::new(point(-73., 227.), size(330., 113.));
        let mut motion = GeometryMotion::default();
        let id = SharedString::from("card");
        motion.begin();
        assert_eq!(motion.sample(&id, a, false, now, spec), (a, false));
        assert_eq!(motion.sample(&id, b, false, now, spec).0, a);
        let later = now + Duration::from_millis(80);
        let (middle, moving) = motion.sample(&id, b, false, later, spec);
        assert!(moving);
        assert!(middle.left() > a.left() && middle.left() < b.left());
        assert!(middle.size.width < a.size.width && middle.size.width > b.size.width);
        assert_eq!(motion.sample(&id, c, false, later, spec).0, middle);
        assert_eq!(motion.sample(&id, c, true, later, spec), (c, false));
        motion.finish();
        motion.begin();
        motion.finish();
        assert!(
            motion.boxes.is_empty(),
            "removed identity has no stale target"
        );
        motion.begin();
        assert_eq!(motion.sample(&id, a, false, later, spec), (a, false));
    }
}
