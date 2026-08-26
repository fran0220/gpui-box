//! A playhead over a described motion: play, pause, reverse, scrub.
//!
//! Everything else in this module runs a motion from beginning to end and is
//! finished with it. An [`Animator`] is the motion held open: something the
//! user is in the middle of and can be moved back and forth over — a preview
//! being scrubbed, a walkthrough being stepped through, an editor showing what
//! a curve does at 40% of the way in.
//!
//! It holds an anchor rather than a position it has to keep up to date. A
//! playing animator is "the head was here at that instant, moving at this
//! rate", so a paused one costs nothing at all, seeking is one assignment, and
//! the head at any moment is arithmetic on two numbers rather than the sum of
//! however many frames have been delivered. Nothing here reads a clock by
//! itself: the caller passes the instant it is painting for, which is what
//! makes the whole playhead testable without a window.

use std::time::{Duration, Instant};

use gpui_kit_theme::Theme;

use super::description::{Motion, MotionSample};
use super::sequence::Sequence;

/// A scrubbable playhead over a run of a known length.
///
/// The head is a fraction of the whole run, which is the same clock
/// [`Motion::sample`] and [`Sequence::progress`] read, so one animator drives
/// either.
///
/// A caller holds one in an entity — `cx.new(|_| Animator::new(total))` — and
/// asks it where the head is while painting. A playing animator needs the
/// frame after this one, so the view that owns it requests one; an animator
/// that is paused or has reached an end has nothing to ask for, which is what
/// [`Animator::running`] answers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Animator {
    duration: Duration,
    /// Where the head was at `anchor`, or where it simply is when paused.
    head: f32,
    /// The instant `head` was true, set only while playing.
    anchor: Option<Instant>,
    speed: f32,
    reversed: bool,
}

impl Animator {
    /// A paused playhead at the start of a run of `duration`.
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            head: 0.0,
            anchor: None,
            speed: 1.0,
            reversed: false,
        }
    }

    /// A playhead over a described motion, including its delay.
    pub fn over(theme: &Theme, motion: &Motion) -> Self {
        Self::new(motion.spec(theme).total())
    }

    /// A playhead over every step of a sequence.
    pub fn across(sequence: &Sequence) -> Self {
        Self::new(sequence.total())
    }

    pub fn duration(self) -> Duration {
        self.duration
    }

    pub fn speed(self) -> f32 {
        self.speed
    }

    pub fn is_reversed(self) -> bool {
        self.reversed
    }

    pub fn is_playing(self) -> bool {
        self.anchor.is_some()
    }

    /// Where the head is at `now`, between 0 and 1.
    ///
    /// A pure function of the anchor, so the same animator and the same
    /// instant always report the same place, and a test can hand it an
    /// instant that has not happened.
    pub fn head(self, now: Instant) -> f32 {
        let Some(anchor) = self.anchor else {
            return self.head;
        };
        let span = self.duration.as_secs_f32();
        if span <= 0.0 {
            return if self.reversed { 0.0 } else { 1.0 };
        }
        let elapsed = now.saturating_duration_since(anchor).as_secs_f32();
        let travelled = elapsed * self.rate() / span;
        clamp_head(self.head + travelled)
    }

    /// Whether the head is still moving at `now`: playing, and not already
    /// parked against the end it is travelling towards.
    ///
    /// This is what a view asks before requesting another frame.
    pub fn running(self, now: Instant) -> bool {
        if !self.is_playing() || self.rate() == 0.0 {
            return false;
        }
        let head = self.head(now);
        if self.reversed {
            head > 0.0
        } else {
            head < 1.0
        }
    }

    /// Whether the head has reached the end it is travelling towards.
    pub fn finished(self, now: Instant) -> bool {
        let head = self.head(now);
        if self.reversed {
            head <= 0.0
        } else {
            head >= 1.0
        }
    }

    /// How far into the run the head is at `now`, as a time rather than a
    /// fraction, which is what a caller showing a position reads.
    pub fn elapsed(self, now: Instant) -> Duration {
        self.duration.mul_f32(self.head(now))
    }

    /// Starts the head moving from wherever it is.
    pub fn play(&mut self, now: Instant) {
        self.head = self.head(now);
        self.anchor = Some(now);
    }

    /// Stops the head where it is at `now`.
    ///
    /// A paused animator has no clock in it: it reports the same place
    /// however long it is left.
    pub fn pause(&mut self, now: Instant) {
        self.head = self.head(now);
        self.anchor = None;
    }

    pub fn toggle(&mut self, now: Instant) {
        if self.is_playing() {
            self.pause(now);
        } else {
            self.play(now);
        }
    }

    /// Turns the head around without moving it, so it retraces exactly the
    /// path it came along.
    pub fn reverse(&mut self, now: Instant) {
        self.head = self.head(now);
        self.reversed = !self.reversed;
        if self.anchor.is_some() {
            self.anchor = Some(now);
        }
    }

    /// Moves the head to `head` without changing whether it is playing.
    ///
    /// A head that is not a number is not a place, so it is refused rather
    /// than propagated: a NaN here would reach layout as a NaN offset.
    pub fn scrub(&mut self, now: Instant, head: f32) {
        self.head = clamp_head(head);
        if self.anchor.is_some() {
            self.anchor = Some(now);
        }
    }

    /// Scales how fast the head travels, without changing its direction.
    ///
    /// Zero holds the head where it is while leaving the animator playing,
    /// which is a different state from paused: it resumes at whatever speed
    /// it is next given.
    pub fn set_speed(&mut self, now: Instant, speed: f32) {
        self.head = self.head(now);
        if self.anchor.is_some() {
            self.anchor = Some(now);
        }
        self.speed = if speed.is_finite() {
            speed.max(0.0)
        } else {
            1.0
        };
    }

    /// Parks the head at the end it is travelling towards and stops.
    ///
    /// This is what [`reduce_motion`](super::reduce_motion) asks for: the run
    /// is over, and it is over where it was going to end.
    pub fn finish(&mut self) {
        self.head = if self.reversed { 0.0 } else { 1.0 };
        self.anchor = None;
    }

    /// Puts the head back at the start of the run, stopped and forwards.
    pub fn rewind(&mut self) {
        self.head = 0.0;
        self.anchor = None;
        self.reversed = false;
    }

    /// What `motion` looks like where the head is at `now`.
    pub fn sample(self, theme: &Theme, motion: &Motion, now: Instant) -> MotionSample {
        motion.sample(theme, self.head(now))
    }

    /// The signed rate the head travels at, per run length per second.
    fn rate(self) -> f32 {
        if self.reversed {
            -self.speed
        } else {
            self.speed
        }
    }
}

fn clamp_head(head: f32) -> f32 {
    if head.is_finite() {
        head.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::motion::{MotionProperty, MotionSpec};
    use crate::{motion, sequence};

    fn animator() -> (Animator, Instant) {
        (Animator::new(Duration::from_millis(400)), Instant::now())
    }

    fn at(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn a_new_playhead_is_stopped_at_the_start() {
        let (animator, now) = animator();
        assert!(!animator.is_playing());
        assert_eq!(animator.head(now), 0.0);
        assert_eq!(animator.head(at(now, 10_000)), 0.0);
    }

    #[test]
    fn a_playing_head_travels_the_run_in_the_time_the_run_takes() {
        let (mut animator, now) = animator();
        animator.play(now);
        assert!((animator.head(at(now, 100)) - 0.25).abs() < 1e-4);
        assert!((animator.head(at(now, 200)) - 0.5).abs() < 1e-4);
        assert_eq!(animator.head(at(now, 400)), 1.0);
        assert_eq!(animator.head(at(now, 9_000)), 1.0);
    }

    #[test]
    fn a_paused_head_holds_however_long_it_is_left() {
        let (mut animator, now) = animator();
        animator.play(now);
        animator.pause(at(now, 200));
        let held = animator.head(at(now, 200));
        assert!((held - 0.5).abs() < 1e-4);
        assert_eq!(animator.head(at(now, 5_000)), held);
        animator.play(at(now, 5_000));
        assert!((animator.head(at(now, 5_100)) - 0.75).abs() < 1e-4);
    }

    #[test]
    fn reversing_retraces_the_path_the_head_came_along() {
        let (mut animator, now) = animator();
        animator.play(now);
        animator.reverse(at(now, 300));
        assert!(animator.is_reversed());
        assert!((animator.head(at(now, 300)) - 0.75).abs() < 1e-4);
        assert!((animator.head(at(now, 400)) - 0.5).abs() < 1e-4);
        assert_eq!(animator.head(at(now, 700)), 0.0);
        // And it stays there rather than travelling past the start.
        assert_eq!(animator.head(at(now, 5_000)), 0.0);
    }

    #[test]
    fn scrubbing_seeks_without_stopping_the_head() {
        let (mut animator, now) = animator();
        animator.play(now);
        animator.scrub(at(now, 100), 0.9);
        assert!(animator.is_playing());
        assert_eq!(animator.head(at(now, 100)), 0.9);
        assert!((animator.head(at(now, 140)) - 1.0).abs() < 1e-4);

        let mut paused = Animator::new(Duration::from_millis(400));
        paused.scrub(now, 0.3);
        assert!(!paused.is_playing());
        assert_eq!(paused.head(at(now, 1_000)), 0.3);
    }

    #[test]
    fn a_head_is_never_outside_the_run_it_scrubs_over() {
        let (mut animator, now) = animator();
        animator.scrub(now, 4.0);
        assert_eq!(animator.head(now), 1.0);
        animator.scrub(now, -2.0);
        assert_eq!(animator.head(now), 0.0);
        animator.scrub(now, f32::NAN);
        assert_eq!(animator.head(now), 0.0);
    }

    #[test]
    fn speed_scales_the_travel_and_keeps_the_direction() {
        let (mut animator, now) = animator();
        animator.play(now);
        animator.set_speed(now, 2.0);
        assert!((animator.head(at(now, 100)) - 0.5).abs() < 1e-4);
        animator.set_speed(at(now, 100), 0.5);
        assert!((animator.head(at(now, 300)) - 0.75).abs() < 1e-4);
        assert_eq!(animator.speed(), 0.5);
    }

    #[test]
    fn a_speed_that_is_not_a_speed_falls_back_to_the_ordinary_one() {
        let (mut animator, now) = animator();
        animator.set_speed(now, f32::NAN);
        assert_eq!(animator.speed(), 1.0);
        animator.set_speed(now, -3.0);
        assert_eq!(animator.speed(), 0.0, "direction is reverse, not a sign");
    }

    #[test]
    fn a_held_head_is_playing_but_not_running() {
        let (mut animator, now) = animator();
        animator.play(now);
        animator.set_speed(now, 0.0);
        assert!(animator.is_playing());
        assert!(!animator.running(at(now, 1_000)));
        assert_eq!(animator.head(at(now, 1_000)), 0.0);
    }

    #[test]
    fn a_view_stops_asking_for_frames_when_the_head_has_arrived() {
        let (mut animator, now) = animator();
        assert!(
            !animator.running(now),
            "a paused head has nothing to ask for"
        );
        animator.play(now);
        assert!(animator.running(at(now, 200)));
        assert!(!animator.running(at(now, 400)));
        assert!(animator.finished(at(now, 400)));
    }

    #[test]
    fn finishing_parks_the_head_where_the_run_was_going_to_end() {
        let (mut animator, now) = animator();
        animator.play(now);
        animator.finish();
        assert_eq!(animator.head(at(now, 10)), 1.0);
        assert!(!animator.is_playing());

        animator.rewind();
        animator.reverse(now);
        animator.finish();
        assert_eq!(animator.head(now), 0.0);
    }

    #[test]
    fn a_run_of_no_length_is_over_as_soon_as_it_is_played() {
        let mut animator = Animator::new(Duration::ZERO);
        let now = Instant::now();
        animator.play(now);
        assert_eq!(animator.head(now), 1.0);
        assert!(animator.finished(now));
    }

    #[test]
    fn elapsed_is_the_head_measured_in_the_time_the_run_takes() {
        let (mut animator, now) = animator();
        animator.play(now);
        assert_eq!(animator.elapsed(at(now, 100)), Duration::from_millis(100));
        animator.pause(at(now, 100));
        assert_eq!(animator.elapsed(at(now, 9_000)), Duration::from_millis(100));
    }

    #[test]
    fn a_playhead_samples_the_motion_it_was_built_over() {
        let theme = Theme::studio_dark();
        let described = motion! {
            duration: 200;
            delay: 200;
            ease: linear;
            opacity: 0.0 => 1.0;
            y: 10.0 => 0.0;
        };
        let mut animator = Animator::over(&theme, &described);
        assert_eq!(animator.duration(), Duration::from_millis(400));
        let now = Instant::now();
        animator.play(now);
        // The first half of the run is the delay, so nothing has moved yet.
        let waiting = animator.sample(&theme, &described, at(now, 150));
        assert_eq!(waiting.get(MotionProperty::Y), 10.0);
        let arrived = animator.sample(&theme, &described, at(now, 400));
        assert_eq!(arrived.opacity, 1.0);
        assert_eq!(arrived.y, 0.0);
    }

    #[test]
    fn a_playhead_across_a_sequence_drives_every_step_of_it() {
        let step = |ms| MotionSpec::new(ms, crate::motion::CubicBezier::new(0.0, 0.0, 1.0, 1.0));
        let sequenced = sequence![step(200), +100 step(200)];
        let mut animator = Animator::across(&sequenced);
        assert_eq!(animator.duration(), Duration::from_millis(500));
        let now = Instant::now();
        animator.play(now);
        let head = animator.head(at(now, 250));
        assert_eq!(sequenced.progress(0, head), 1.0);
        assert_eq!(
            sequenced.progress(1, head),
            0.0,
            "the second step is still inside its gap"
        );
        assert_eq!(sequenced.progress(1, animator.head(at(now, 500))), 1.0);
    }
}
