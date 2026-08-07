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
    reversed: bool,
}

impl Stagger {
    pub fn new(step: Duration, max_items: usize) -> Self {
        Self {
            step,
            max_items: max_items.max(2),
            reversed: false,
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

    /// Runs the wave from the last item to the first.
    ///
    /// This is the order a list leaves on. A list that arrived from the top
    /// down should depart from the bottom up, so the row the user is looking
    /// at — the one they just acted on, at the top — is the last to go rather
    /// than the first, and the group empties away from them instead of out
    /// from under them.
    pub fn reversed(mut self) -> Self {
        self.reversed = true;
        self
    }

    pub fn is_reversed(&self) -> bool {
        self.reversed
    }

    /// How far apart two neighbours start, for a group of `count`.
    ///
    /// Past `max_items` the window is fixed, so the step shrinks to fit it.
    fn step_for(&self, count: usize) -> Duration {
        if count > self.max_items {
            self.step
                .mul_f32((self.max_items - 1) as f32 / (count - 1) as f32)
        } else {
            self.step
        }
    }

    pub fn delay(&self, index: usize, count: usize) -> Duration {
        if count <= 1 {
            return Duration::ZERO;
        }
        let last = count - 1;
        let place = index.min(last);
        let place = if self.reversed { last - place } else { place };
        self.step_for(count).mul_f32(place as f32)
    }

    /// The window the whole group occupies, including the last item's span.
    ///
    /// Reversing changes which item waits longest, not how long the group
    /// takes, so the window is measured from the furthest place rather than
    /// from the last index.
    pub fn total(&self, count: usize, spec: MotionSpec) -> Duration {
        let furthest = self.step_for(count).mul_f32(count.saturating_sub(1) as f32);
        furthest + spec.total()
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
    fn a_reversed_wave_starts_at_the_far_end() {
        let stagger = Stagger::from_millis(30).reversed();
        assert!(stagger.is_reversed());
        assert_eq!(stagger.delay(4, 5), Duration::ZERO);
        assert_eq!(stagger.delay(3, 5), Duration::from_millis(30));
        assert_eq!(stagger.delay(0, 5), Duration::from_millis(120));
    }

    #[test]
    fn reversing_does_not_change_how_long_the_group_takes() {
        let forward = Stagger::from_millis(30);
        for count in [1, 2, 5, 200] {
            assert_eq!(
                forward.total(count, spec()),
                forward.reversed().total(count, spec()),
                "{count} items"
            );
        }
    }

    #[test]
    fn a_reversed_wave_compresses_the_same_way_a_forward_one_does() {
        let stagger = Stagger::rows().reversed();
        for count in [2, 8, 50, 500] {
            let waited = stagger.delay(0, count);
            assert!(
                waited.as_millis() <= ROW_STAGGER_CAP.as_millis(),
                "{count} rows waited {waited:?}"
            );
        }
        assert_eq!(stagger.delay(7, 8), Duration::ZERO);
        assert_eq!(stagger.delay(0, 8), ROW_STAGGER_CAP);
    }

    #[test]
    fn a_staggered_phase_wraps_within_one_cycle() {
        for index in 0..6 {
            let phase = staggered_phase(0.2, index, 0.15);
            assert!((0.0..1.0).contains(&phase));
        }
    }
}
