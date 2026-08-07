//! Animating a value toward a target that can change mid-flight.

use std::time::Duration;

use gpui::{App, SharedString, Window};
use web_time::Instant;

use super::{Interpolate, MotionSpec, keyed};

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
    /// Progress velocity carried in from the motion this one interrupted, in
    /// units of the current distance per second.
    carried: f32,
    /// How long the run takes, delay excluded. A spring that was already
    /// moving needs longer than its resting settle time.
    duration: Duration,
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
            carried: 0.0,
            duration: Self::run_time(spec, 0.0),
            last_frame: None,
        }
    }

    pub fn spec(mut self, spec: MotionSpec) -> Self {
        self.spec = spec;
        self.duration = Self::run_time(spec, self.carried);
        self
    }

    fn run_time(spec: MotionSpec, carried: f32) -> Duration {
        match spec.spring() {
            Some(spring) if carried != 0.0 => spring.settle_time_at(carried),
            _ => Duration::from_millis(spec.duration_ms),
        }
    }

    fn delay(&self) -> Duration {
        Duration::from_millis(self.spec.delay_ms)
    }

    fn total(&self) -> Duration {
        self.delay() + self.duration
    }

    pub fn target(&self) -> T {
        self.to
    }

    pub fn value(&self) -> T {
        self.from.lerp(self.to, self.progress())
    }

    pub fn is_animating(&self) -> bool {
        self.elapsed < self.total()
    }

    fn progress(&self) -> f32 {
        let local = self.elapsed.saturating_sub(self.delay());
        if self.duration.is_zero() || local >= self.duration {
            return 1.0;
        }
        match self.spec.spring() {
            Some(spring) => spring.value_at(local, self.carried).0,
            None => self
                .spec
                .curve
                .eval(local.as_secs_f32() / self.duration.as_secs_f32()),
        }
    }

    /// How fast progress is moving right now, in progress per second.
    ///
    /// A curve reports nothing: a cubic bezier is a shape read off a clock,
    /// with no state to hand on, so pretending it has momentum would be an
    /// invention rather than a continuation.
    fn progress_velocity(&self) -> f32 {
        let Some(spring) = self.spec.spring() else {
            return 0.0;
        };
        let local = self.elapsed.saturating_sub(self.delay());
        if self.duration.is_zero() || local >= self.duration {
            return 0.0;
        }
        spring.value_at(local, self.carried).1
    }

    /// Whether travel along the current path also closes on `target`.
    ///
    /// A carried speed needs a direction, and [`Interpolate::distance`] is a
    /// length with no sign. Stepping a little further along the path the value
    /// is already on and asking whether that landed nearer `target` recovers
    /// one, without asking every interpolable value to define an axis. It also
    /// reads a spring that has overshot correctly, where the value is past its
    /// target and travelling back.
    fn heads_toward(&self, target: T) -> bool {
        const PROBE: f32 = 1e-3;
        let ahead = self.from.lerp(self.to, self.progress() + PROBE);
        ahead.distance(target) < self.value().distance(target)
    }

    /// Aims at a new target. Setting the current target again is a no-op, so a
    /// render that re-declares the same value does not restart the animation.
    ///
    /// A retarget hands the motion on rather than restarting it: the speed the
    /// value already had is measured, converted into the new distance, and
    /// released into the new motion. Without that, a target changed mid-flight
    /// stalls the value for the first few frames of the new run.
    ///
    /// The speed keeps its direction, so reversing a target throws the value
    /// on the way it was already going before it comes back. Turning it round
    /// on the spot would be the stall this exists to remove, wearing a
    /// different shape.
    pub fn set(&mut self, target: T)
    where
        T: PartialEq,
    {
        if target == self.to {
            return;
        }
        let current = self.value();
        let speed = self.progress_velocity() * self.from.distance(self.to);
        let forward = self.heads_toward(target);
        self.from = current;
        self.to = target;
        self.elapsed = Duration::ZERO;
        let distance = self.from.distance(self.to);
        self.carried = if distance > 0.0 {
            let along = if forward { speed } else { -speed };
            along / distance
        } else {
            0.0
        };
        self.duration = Self::run_time(self.spec, self.carried);
    }

    /// Jumps to `target` without animating, for state changes the user did not
    /// cause, such as a theme switch.
    pub fn snap(&mut self, target: T) {
        self.from = target;
        self.to = target;
        self.carried = 0.0;
        self.duration = Self::run_time(self.spec, 0.0);
        self.elapsed = self.total();
    }

    pub fn advance(&mut self, delta: Duration) {
        self.elapsed = (self.elapsed + delta).min(self.total());
    }

    /// Advances by the time since the previous frame and schedules the next
    /// one while the transition is still running.
    ///
    /// Honors reduced motion by finishing immediately, so a caller gets the
    /// final value without any intermediate frames.
    pub fn animate(&mut self, window: &mut Window, cx: &mut App) -> T {
        if cx.reduce_motion() {
            self.elapsed = self.total();
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

/// One transition kept per semantic id, for a `RenderOnce` builder that is
/// rebuilt every frame and cannot carry state of its own.
///
/// `Default` is what the keyed global needs, and `None` is the honest default:
/// the transition can only be created once the caller's first target is known,
/// so it starts settled there rather than animating in from nothing.
struct Tracked<T: Interpolate>(Option<Transition<T>>);

impl<T: Interpolate> Default for Tracked<T> {
    fn default() -> Self {
        Self(None)
    }
}

/// Moves the value kept for `id` toward `target` and returns what to draw.
///
/// The first frame for an id is already settled, so a control that appears
/// with a value does not animate up to it from zero.
pub(crate) fn tracked<T>(
    id: &SharedString,
    target: T,
    spec: MotionSpec,
    window: &mut Window,
    cx: &mut App,
) -> T
where
    T: Interpolate + PartialEq + 'static,
{
    tracked_or_snap(id, target, spec, false, window, cx)
}

/// The same, except that `snap` jumps straight to the target.
///
/// A control the pointer is holding must be exactly where the pointer is: a
/// spring that trails the finger by even a frame reads as the control being
/// broken rather than as motion.
pub(crate) fn tracked_or_snap<T>(
    id: &SharedString,
    target: T,
    spec: MotionSpec,
    snap: bool,
    window: &mut Window,
    cx: &mut App,
) -> T
where
    T: Interpolate + PartialEq + 'static,
{
    let cell = keyed::slot::<Tracked<T>>(id, cx);
    let mut tracked = cell.borrow_mut();
    let mut transition = tracked
        .0
        .unwrap_or_else(|| Transition::new(target, spec))
        .spec(spec);
    if snap {
        transition.snap(target);
    } else {
        transition.set(target);
    }
    let shown = transition.animate(window, cx);
    tracked.0 = Some(transition);
    shown
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{CubicBezier, MotionSpec, Spring};

    fn linear(duration_ms: u64) -> MotionSpec {
        MotionSpec::new(duration_ms, CubicBezier::new(0.0, 0.0, 1.0, 1.0))
    }

    /// An underdamped spring, so overshoot is available to assert on.
    fn sprung() -> MotionSpec {
        MotionSpec::sprung(Spring::new(400.0, 28.0, 1.0))
    }

    /// A transition caught while it is travelling upward at speed.
    fn in_flight() -> Transition<f32> {
        let mut transition = Transition::new(0.0_f32, sprung());
        transition.set(10.0);
        transition.advance(Duration::from_millis(40));
        transition
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
    fn a_retargeted_spring_keeps_moving_instead_of_starting_again() {
        let mut carried = in_flight();
        let interrupted = carried.value();
        carried.set(20.0);

        let mut from_rest = Transition::new(interrupted, sprung());
        from_rest.set(20.0);

        for _ in 0..2 {
            carried.advance(Duration::from_millis(16));
            from_rest.advance(Duration::from_millis(16));
        }
        assert!(
            carried.value() > interrupted,
            "the value stalled at {interrupted}"
        );
        assert!(
            carried.value() > from_rest.value(),
            "a retarget must not throw away the speed the value had: {} against {}",
            carried.value(),
            from_rest.value()
        );
    }

    #[test]
    fn a_retarget_rescales_the_speed_it_carries_into_the_new_distance() {
        let mut transition = in_flight();
        let speed = transition.progress_velocity() * transition.from.distance(transition.to);
        transition.set(10.2);
        let released = transition.progress_velocity() * transition.from.distance(transition.to);
        assert!(
            (released - speed).abs() < 1e-2,
            "a shorter distance changed the speed of the value: {released} against {speed}"
        );
    }

    #[test]
    fn a_spring_that_was_moving_the_other_way_is_given_longer_to_settle() {
        let mut transition = Transition::new(0.0_f32, sprung());
        transition.set(10.0);
        // Past the first overshoot, where the value is on its way back down.
        transition.advance(Duration::from_millis(300));
        assert!(transition.progress_velocity() < 0.0);

        // A short hop away from a value moving the wrong way at speed: the
        // spring has to turn the motion around before it can land.
        transition.set(transition.value() + 0.1);
        assert!(transition.total() > sprung().total());
    }

    #[test]
    fn reversing_mid_flight_carries_on_before_it_turns_round() {
        let mut transition = in_flight();
        let interrupted = transition.value();
        assert!(transition.progress_velocity() > 0.0);

        transition.set(0.0);
        let mut highest = f32::MIN;
        let mut lowest = f32::MAX;
        while transition.is_animating() {
            transition.advance(Duration::from_millis(8));
            highest = highest.max(transition.value());
            lowest = lowest.min(transition.value());
        }
        assert!(
            highest > interrupted,
            "a value moving away from its new target has to travel before it \
             can come back: it turned round on the spot at {interrupted}"
        );
        assert!(
            lowest < 0.0,
            "an underdamped reversal passes its target, lowest was {lowest}"
        );
        assert_eq!(transition.value(), 0.0);
    }

    #[test]
    fn a_reversal_and_a_continuation_carry_the_speed_opposite_ways() {
        let mut onward = in_flight();
        let mut back = in_flight();
        assert_eq!(onward.value(), back.value());

        onward.set(20.0);
        back.set(0.0);
        assert!(
            onward.carried > 0.0 && back.carried < 0.0,
            "the same motion was released {} one way and {} the other",
            onward.carried,
            back.carried
        );
    }

    #[test]
    fn a_spring_on_its_way_back_is_read_as_closing_on_a_target_behind_it() {
        let mut transition = Transition::new(0.0_f32, sprung());
        transition.set(10.0);
        // Past the first overshoot: the value is above its target and falling.
        transition.advance(Duration::from_millis(300));
        assert!(transition.value() > 10.0);
        assert!(transition.progress_velocity() < 0.0);

        transition.set(5.0);
        assert!(
            transition.carried > 0.0,
            "a value already falling toward a lower target is closing on it, \
             but it was released at {}",
            transition.carried
        );
    }

    #[test]
    fn a_curve_carries_no_speed_across_a_retarget() {
        let mut transition = Transition::new(0.0_f32, linear(200));
        transition.set(10.0);
        transition.advance(Duration::from_millis(100));
        assert_eq!(transition.value(), 5.0);

        transition.set(0.0);
        assert_eq!(transition.total(), linear(200).total());
        transition.advance(Duration::from_millis(100));
        assert_eq!(
            transition.value(),
            2.5,
            "a bezier has no momentum, so half the remaining distance is exactly half"
        );
    }

    #[test]
    fn advancing_past_the_end_never_overshoots_the_target() {
        let mut transition = Transition::new(0.0_f32, linear(100));
        transition.set(1.0);
        transition.advance(Duration::from_secs(5));
        assert_eq!(transition.value(), 1.0);
    }
}
