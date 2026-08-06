//! Orchestration for motion that runs across a group of elements.

use std::time::Duration;

use super::MotionSpec;

/// How far apart two neighbouring rows start, and how many rows the wave is
/// allowed to span before it compresses instead of growing.
const ROW_STEP_MS: u64 = 16;
const ROW_WINDOW: usize = 8;

/// The longest a row wave can last, whatever the row count.
pub const ROW_STAGGER_CAP: Duration = Duration::from_millis(ROW_STEP_MS * (ROW_WINDOW as u64 - 1));

/// Delays each item in a list so a group animates as a wave.
///
/// The total is capped so a long list stays responsive: past `max_items` the
/// per-item delay shrinks instead of the sequence growing without bound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stagger {
    step: Duration,
    max_items: usize,
}

impl Stagger {
    pub fn new(step: Duration, max_items: usize) -> Self {
        Self {
            step,
            max_items: max_items.max(2),
        }
    }

    pub fn from_millis(step_ms: u64) -> Self {
        Self::new(Duration::from_millis(step_ms), 12)
    }

    /// The wave a list of menu-shaped rows arrives on.
    ///
    /// Sixteen milliseconds a row across at most eight rows, so the last row
    /// in a fifty-row menu starts 112ms after the first rather than a second
    /// later: past eight rows the step shrinks to keep the window fixed.
    pub fn rows() -> Self {
        Self::new(Duration::from_millis(ROW_STEP_MS), ROW_WINDOW)
    }

    pub fn max_items(mut self, max_items: usize) -> Self {
        self.max_items = max_items.max(2);
        self
    }

    pub fn delay(&self, index: usize, count: usize) -> Duration {
        if count <= 1 {
            return Duration::ZERO;
        }
        // Past max_items the window is fixed, so the step shrinks to fit it.
        let step = if count > self.max_items {
            self.step
                .mul_f32((self.max_items - 1) as f32 / (count - 1) as f32)
        } else {
            self.step
        };
        step.mul_f32(index.min(count.saturating_sub(1)) as f32)
    }

    /// The window the whole group occupies, including the last item's span.
    pub fn total(&self, count: usize, spec: MotionSpec) -> Duration {
        self.delay(count.saturating_sub(1), count) + spec.total()
    }

    /// Applies the delay for one item to a spec.
    pub fn spec(&self, index: usize, count: usize, spec: MotionSpec) -> MotionSpec {
        spec.with_delay(spec.delay_ms + self.delay(index, count).as_millis() as u64)
    }
}

/// The repeating phase for item `index` in a looping group animation, such as
/// a chase or wave loader.
pub fn staggered_phase(raw: f32, index: usize, stagger: f32) -> f32 {
    (raw - index as f32 * stagger).rem_euclid(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::CubicBezier;

    fn spec() -> MotionSpec {
        MotionSpec::new(100, CubicBezier::new(0.0, 0.0, 1.0, 1.0))
    }

    #[test]
    fn the_first_item_never_waits() {
        let stagger = Stagger::from_millis(30);
        assert_eq!(stagger.delay(0, 5), Duration::ZERO);
        assert_eq!(stagger.delay(0, 1), Duration::ZERO);
    }

    #[test]
    fn delays_increase_with_position() {
        let stagger = Stagger::from_millis(30);
        assert_eq!(stagger.delay(1, 5), Duration::from_millis(30));
        assert_eq!(stagger.delay(4, 5), Duration::from_millis(120));
    }

    #[test]
    fn a_long_list_compresses_instead_of_growing_without_bound() {
        let stagger = Stagger::from_millis(30).max_items(10);
        let short = stagger.total(10, spec());
        for count in [200, 2000] {
            let long = stagger.total(count, spec());
            assert!(
                long.abs_diff(short) < Duration::from_millis(1),
                "{count} items took {long:?} against {short:?} for ten"
            );
        }
    }

    #[test]
    fn the_group_window_covers_the_last_items_span() {
        let stagger = Stagger::from_millis(30);
        assert_eq!(stagger.total(3, spec()), Duration::from_millis(160));
    }

    #[test]
    fn a_row_wave_never_outlasts_its_cap() {
        let stagger = Stagger::rows();
        assert_eq!(stagger.delay(0, 50), Duration::ZERO);
        for count in [2, 8, 50, 500] {
            // Compared in whole milliseconds, which is the granularity the cap
            // is stated in; the compressed step is a float division and lands
            // a few tens of nanoseconds either side of it.
            let waited = stagger.delay(count - 1, count);
            assert!(
                waited.as_millis() <= ROW_STAGGER_CAP.as_millis(),
                "{count} rows waited {waited:?}"
            );
        }
        assert_eq!(stagger.delay(7, 8), ROW_STAGGER_CAP);
    }

    #[test]
    fn a_staggered_phase_wraps_within_one_cycle() {
        for index in 0..6 {
            let phase = staggered_phase(0.2, index, 0.15);
            assert!((0.0..1.0).contains(&phase));
        }
    }
}
