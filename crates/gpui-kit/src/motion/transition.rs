//! Animating a value toward a target that can change mid-flight.

use std::time::Duration;

use gpui::{App, Window};
use web_time::Instant;

use super::{Interpolate, MotionSpec};

/// A value that animates toward whatever it is last told to be.
///
/// Retargeting starts from the value currently on screen rather than from the
/// previous target, so an interrupted transition does not jump backward.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition<T: Interpolate> {
    from: T,
    to: T,
    spec: MotionSpec,
    elapsed: Duration,
    last_frame: Option<Instant>,
}

impl<T: Interpolate> Transition<T> {
    /// Starts settled at `value`, so a first render does not animate in.
    pub fn new(value: T, spec: MotionSpec) -> Self {
        Self {
            from: value,
            to: value,
            spec,
            elapsed: spec.total(),
            last_frame: None,
        }
    }

    pub fn spec(mut self, spec: MotionSpec) -> Self {
        self.spec = spec;
        self
    }

    pub fn target(&self) -> T {
        self.to
    }

    pub fn value(&self) -> T {
        self.from.lerp(self.to, self.progress())
    }

    pub fn is_animating(&self) -> bool {
        self.elapsed < self.spec.total()
    }

    fn progress(&self) -> f32 {
        let total = self.spec.total().as_secs_f32();
        if total <= 0.0 {
            return 1.0;
        }
        self.spec
            .progress((self.elapsed.as_secs_f32() / total).clamp(0.0, 1.0))
    }

    /// Aims at a new target. Setting the current target again is a no-op, so a
    /// render that re-declares the same value does not restart the animation.
    pub fn set(&mut self, target: T)
    where
        T: PartialEq,
    {
        if target == self.to {
            return;
        }
        self.from = self.value();
        self.to = target;
        self.elapsed = Duration::ZERO;
    }

    /// Jumps to `target` without animating, for state changes the user did not
    /// cause, such as a theme switch.
    pub fn snap(&mut self, target: T) {
        self.from = target;
        self.to = target;
        self.elapsed = self.spec.total();
    }

    pub fn advance(&mut self, delta: Duration) {
        self.elapsed = (self.elapsed + delta).min(self.spec.total());
    }

    /// Advances by the time since the previous frame and schedules the next
    /// one while the transition is still running.
    ///
    /// Honors reduced motion by finishing immediately, so a caller gets the
    /// final value without any intermediate frames.
    pub fn animate(&mut self, window: &mut Window, cx: &mut App) -> T {
        if cx.reduce_motion() {
            self.elapsed = self.spec.total();
            self.last_frame = None;
            return self.value();
        }

        let now = cx.background_executor().now();
        if let Some(last) = self.last_frame {
            self.advance(now.saturating_duration_since(last));
        }
        if self.is_animating() {
            self.last_frame = Some(now);
            window.request_animation_frame();
        } else {
            self.last_frame = None;
        }
        self.value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{CubicBezier, MotionSpec};

    fn linear(duration_ms: u64) -> MotionSpec {
        MotionSpec::new(duration_ms, CubicBezier::new(0.0, 0.0, 1.0, 1.0))
    }

    #[test]
    fn a_new_transition_is_already_settled() {
        let transition = Transition::new(1.0_f32, linear(200));
        assert!(!transition.is_animating());
        assert_eq!(transition.value(), 1.0);
    }

    #[test]
    fn advancing_moves_the_value_and_finishes_exactly_on_target() {
        let mut transition = Transition::new(0.0_f32, linear(200));
        transition.set(10.0);
        transition.advance(Duration::from_millis(100));
        assert!((transition.value() - 5.0).abs() < 0.1);
        transition.advance(Duration::from_millis(100));
        assert_eq!(transition.value(), 10.0);
        assert!(!transition.is_animating());
    }

    #[test]
    fn retargeting_continues_from_the_value_on_screen() {
        let mut transition = Transition::new(0.0_f32, linear(200));
        transition.set(10.0);
        transition.advance(Duration::from_millis(100));
        let interrupted = transition.value();

        transition.set(0.0);
        assert_eq!(transition.value(), interrupted);
        transition.advance(Duration::from_millis(200));
        assert_eq!(transition.value(), 0.0);
    }

    #[test]
    fn setting_the_current_target_does_not_restart_the_animation() {
        let mut transition = Transition::new(0.0_f32, linear(200));
        transition.set(10.0);
        transition.advance(Duration::from_millis(100));
        let midpoint = transition.value();
        transition.set(10.0);
        assert_eq!(transition.value(), midpoint);
    }

    #[test]
    fn snapping_skips_the_animation_entirely() {
        let mut transition = Transition::new(0.0_f32, linear(200));
        transition.snap(10.0);
        assert_eq!(transition.value(), 10.0);
        assert!(!transition.is_animating());
    }

    #[test]
    fn advancing_past_the_end_never_overshoots_the_target() {
        let mut transition = Transition::new(0.0_f32, linear(100));
        transition.set(1.0);
        transition.advance(Duration::from_secs(5));
        assert_eq!(transition.value(), 1.0);
    }
}
