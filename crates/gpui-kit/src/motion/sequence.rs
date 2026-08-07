//! Motion that runs one step after another, rather than all at once.

use std::time::Duration;

use super::MotionSpec;

/// A chain of specifications, each starting when the one before it has
/// finished.
///
/// [`MotionSpec::after`] already composes two, and a chain of them is a
/// perfectly good way to write "and then". This exists for what that shape
/// cannot answer: it keeps the steps, so a caller can ask where step three
/// starts or drive the whole chain from one clock, and it reports the total —
/// which [`Presence`](super::Presence), a caller holding an element on screen,
/// or anything else waiting for the group to be over cannot otherwise know
/// without adding the durations up by hand.
///
/// Each step keeps its own delay, counted from the end of the previous step,
/// so a gap between two motions is written where the gap is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sequence {
    /// Steps as given, with their delays still relative to the step before.
    steps: Vec<MotionSpec>,
}

impl Sequence {
    pub fn new(steps: impl IntoIterator<Item = MotionSpec>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }

    /// Adds a step that begins when everything already in the sequence has
    /// finished.
    pub fn then(mut self, spec: MotionSpec) -> Self {
        self.steps.push(spec);
        self
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// When step `index` starts, measured from the start of the sequence.
    /// Its own delay is part of the step, not part of the wait.
    pub fn start(&self, index: usize) -> Duration {
        self.steps
            .iter()
            .take(index.min(self.steps.len()))
            .map(|spec| spec.total())
            .sum()
    }

    /// Step `index` with its delay measured from the start of the sequence,
    /// so it can be handed to anything that runs a single specification.
    pub fn step(&self, index: usize) -> Option<MotionSpec> {
        let spec = *self.steps.get(index)?;
        Some(spec.with_delay(spec.delay_ms + self.start(index).as_millis() as u64))
    }

    pub fn steps(&self) -> impl Iterator<Item = MotionSpec> + '_ {
        (0..self.steps.len()).filter_map(|index| self.step(index))
    }

    /// How long the whole sequence lasts.
    pub fn total(&self) -> Duration {
        self.steps.iter().map(|spec| spec.total()).sum()
    }

    /// Where step `index` has got to when the sequence as a whole is `raw`
    /// through, so one clock over [`Sequence::total`] drives every step.
    ///
    /// A step that has not started reports 0 and a step that has finished
    /// reports 1, which is what a caller painting all of them at once needs:
    /// the steps that are over stay where they landed.
    pub fn progress(&self, index: usize, raw: f32) -> f32 {
        let Some(spec) = self.steps.get(index).copied() else {
            return 1.0;
        };
        let elapsed = self.total().mul_f32(raw.clamp(0.0, 1.0));
        let local = elapsed.saturating_sub(self.start(index));
        let span = spec.total();
        if span.is_zero() {
            return 1.0;
        }
        spec.progress((local.as_secs_f32() / span.as_secs_f32()).min(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::CubicBezier;

    fn linear(ms: u64) -> MotionSpec {
        MotionSpec::new(ms, CubicBezier::new(0.0, 0.0, 1.0, 1.0))
    }

    fn sequence() -> Sequence {
        Sequence::new([linear(200)]).then(linear(100).with_delay(50))
    }

    #[test]
    fn a_step_starts_when_the_one_before_it_ends() {
        let sequence = sequence();
        assert_eq!(sequence.start(0), Duration::ZERO);
        assert_eq!(sequence.start(1), Duration::from_millis(200));
        assert_eq!(sequence.step(1).expect("two steps").delay_ms, 250);
    }

    #[test]
    fn the_total_is_the_sum_of_the_steps() {
        assert_eq!(sequence().total(), Duration::from_millis(350));
        assert_eq!(Sequence::default().total(), Duration::ZERO);
    }

    #[test]
    fn an_empty_sequence_has_no_steps_to_report() {
        let empty = Sequence::default();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert!(empty.step(0).is_none());
    }

    #[test]
    fn a_step_holds_at_both_ends_of_its_own_span() {
        let sequence = sequence();
        // The second step has not begun while the first is still running.
        assert_eq!(sequence.progress(1, 0.5), 0.0);
        // And the first stays where it landed once it is over.
        assert_eq!(sequence.progress(0, 1.0), 1.0);
        assert!((sequence.progress(0, 200.0 / 350.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn the_steps_carry_the_offsets_the_sequence_gave_them() {
        let delays: Vec<u64> = sequence().steps().map(|spec| spec.delay_ms).collect();
        assert_eq!(delays, vec![0, 250]);
    }
}
