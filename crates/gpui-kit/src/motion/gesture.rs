//! Motion measured off the hand rather than off a clock.
//!
//! A gesture reports where the pointer is. Everything that makes a gesture
//! feel physical — inertia after a release, a boundary that resists, a flick
//! that means "get rid of this" — needs how fast it was going as well, and
//! that is a measurement rather than a fact the platform hands over.
//!
//! [`VelocityTracker`] is that measurement. The effects built on it are
//! [`flick`], [`rubber_band`], and, for inertia,
//! [`Transition::release`](super::Transition::release), which hands a released
//! speed to the spring that already knows how to carry one.

use std::collections::VecDeque;
use std::time::Duration;

use gpui::{Pixels, Point, px};
use gpui_kit_theme::Theme;
use web_time::Instant;

/// How far back a velocity is measured by default.
///
/// Short enough that the answer is the speed at release rather than the
/// average of the whole gesture, long enough to span several events at any
/// frame rate a platform delivers.
pub const VELOCITY_WINDOW: Duration = Duration::from_millis(100);

/// The shortest span two samples can be apart and still be believed.
///
/// Two events a fraction of a millisecond apart divide a pixel or two by
/// almost nothing, which reports a speed no hand ever moved at. A gesture
/// measured over less than this is not measured at all.
const MIN_SPAN: Duration = Duration::from_millis(8);

/// How fast a gesture is moving, in pixels a second on each axis.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

impl Velocity {
    /// A gesture that is not moving.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// The speed, with the direction thrown away.
    pub fn speed(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Whether the gesture is, for practical purposes, standing still.
    pub fn is_still(self) -> bool {
        self.speed() < 1.0
    }

    /// The speed along the axis the gesture is mostly travelling on, signed.
    fn dominant(self) -> (Axis, f32) {
        if self.x.abs() >= self.y.abs() {
            (Axis::Horizontal, self.x)
        } else {
            (Axis::Vertical, self.y)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Horizontal,
    Vertical,
}

/// The speed and direction of a pointer, measured over a short trailing
/// window.
///
/// The window rather than the last two events is the whole point. Platforms
/// deliver moves at whatever rate they please, so the last pair can be a
/// millisecond apart and report an impossible speed, and — more importantly —
/// a gesture that stopped before release still has old fast samples behind it.
/// Samples older than the window are discarded, so a drag the user parked
/// reports a stop rather than the speed it had before the pause. A tracker
/// that reported one would fling away the thing the user deliberately put
/// down.
#[derive(Debug, Clone)]
pub struct VelocityTracker {
    window: Duration,
    samples: VecDeque<(Instant, Point<Pixels>)>,
}

impl Default for VelocityTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl VelocityTracker {
    pub fn new() -> Self {
        Self::with_window(VELOCITY_WINDOW)
    }

    pub fn with_window(window: Duration) -> Self {
        Self {
            window,
            samples: VecDeque::new(),
        }
    }

    /// Records where the pointer was at `at`.
    ///
    /// A sample older than one already recorded is ignored: the tracker
    /// measures a gesture forward in time, and reordering events would let a
    /// late delivery invent a direction.
    pub fn sample(&mut self, position: Point<Pixels>, at: Instant) {
        if self.samples.back().is_some_and(|(last, _)| at < *last) {
            return;
        }
        self.samples.push_back((at, position));
        self.prune(at);
    }

    /// The speed the pointer is moving at as of `now`.
    ///
    /// `now` rather than the last sample, because a pointer that has stopped
    /// sends nothing at all: the pause is visible only against a clock.
    pub fn velocity_at(&self, now: Instant) -> Velocity {
        let mut live = self
            .samples
            .iter()
            .filter(|(at, _)| now.saturating_duration_since(*at) <= self.window);
        let Some((first_at, first)) = live.next() else {
            return Velocity::ZERO;
        };
        let Some((last_at, last)) = live.next_back() else {
            return Velocity::ZERO;
        };
        let span = last_at.saturating_duration_since(*first_at);
        if span < MIN_SPAN {
            return Velocity::ZERO;
        }
        let seconds = span.as_secs_f32();
        Velocity::new(
            f32::from(last.x - first.x) / seconds,
            f32::from(last.y - first.y) / seconds,
        )
    }

    /// Forgets the gesture, for a drag that was cancelled rather than dropped.
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    fn prune(&mut self, now: Instant) {
        while self
            .samples
            .front()
            .is_some_and(|(at, _)| now.saturating_duration_since(*at) > self.window)
        {
            self.samples.pop_front();
        }
    }
}

/// Which way a flick went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flick {
    Left,
    Right,
    Up,
    Down,
}

impl Flick {
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
        }
    }
}

/// Whether a gesture that travelled `travel` and let go at `velocity` was a
/// flick, and which way it went.
///
/// A flick is a claim about intent, so it takes both numbers. Speed alone
/// would call a twitch a flick; distance alone would call a slow deliberate
/// drag one, and those are the two gestures a dismissal has to tell apart.
/// The threshold is `motion.flickVelocityPxPerSec`.
///
/// The direction comes from the speed and the travel has to agree with it: a
/// gesture that went out and was coming back when it was released was not
/// flicked out.
pub fn flick(travel: Point<Pixels>, velocity: Velocity, theme: &Theme) -> Option<Flick> {
    let (axis, speed) = velocity.dominant();
    if speed.abs() < theme.motion.flick_velocity {
        return None;
    }
    let travelled = match axis {
        Axis::Horizontal => f32::from(travel.x),
        Axis::Vertical => f32::from(travel.y),
    };
    if travelled == 0.0 || travelled.signum() != speed.signum() {
        return None;
    }
    Some(match (axis, speed < 0.0) {
        (Axis::Horizontal, true) => Flick::Left,
        (Axis::Horizontal, false) => Flick::Right,
        (Axis::Vertical, true) => Flick::Up,
        (Axis::Vertical, false) => Flick::Down,
    })
}

/// The distance actually shown when a gesture pulls `overscroll` past a
/// boundary.
///
/// Resistance grows with the pull: `tension` is the fraction of the first
/// pixel that shows, and every pixel after it shows less, so the band tightens
/// smoothly rather than at a point the hand can feel. The result approaches
/// `extent` and never reaches it, however hard the pull, so a boundary can be
/// stretched but not crossed.
///
/// This is a function of the pull and nothing else — no clock, no state, no
/// frame — because the band is where the hand is holding it.
/// How far a pull past a scroll boundary should paint, using the theme band.
///
/// The scroll itself still stops at the edge: GPUI's viewport does not travel
/// past its content. This is the visual remainder of that pull, so a host
/// that measures an attempted overscroll can show resistance rather than a
/// hard stop with no evidence.
pub fn overscroll(pull: Pixels, theme: &Theme) -> Pixels {
    rubber_band(
        pull,
        px(theme.effects.edge_fade_band),
        theme.motion.rubber_band_tension,
    )
}

pub fn rubber_band(overscroll: Pixels, extent: Pixels, tension: f32) -> Pixels {
    let extent = f32::from(extent);
    let tension = tension.max(f32::EPSILON);
    if extent <= 0.0 {
        return px(0.0);
    }
    let pull = f32::from(overscroll);
    let damped = (1.0 - 1.0 / (pull.abs() * tension / extent + 1.0)) * extent;
    px(damped.copysign(pull))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::point;

    fn theme() -> Theme {
        Theme::studio_dark()
    }

    fn steady(pixels_per_second: f32, samples: usize) -> (VelocityTracker, Instant) {
        let step = Duration::from_millis(10);
        let mut tracker = VelocityTracker::new();
        let start = Instant::now();
        for index in 0..samples {
            let elapsed = step.mul_f32(index as f32);
            tracker.sample(
                point(px(0.0), px(pixels_per_second * elapsed.as_secs_f32())),
                start + elapsed,
            );
        }
        (tracker, start + step.mul_f32((samples - 1) as f32))
    }

    #[test]
    fn a_steady_drag_reports_the_speed_it_was_moving_at() {
        let (tracker, now) = steady(600.0, 8);
        let velocity = tracker.velocity_at(now);
        assert!(
            (velocity.y - 600.0).abs() < 1.0,
            "measured {} instead of 600",
            velocity.y
        );
        assert_eq!(velocity.x, 0.0);
    }

    #[test]
    fn a_gesture_that_stopped_before_release_has_no_velocity() {
        let (tracker, moving) = steady(600.0, 8);
        assert!(!tracker.velocity_at(moving).is_still());
        let paused = moving + VELOCITY_WINDOW + Duration::from_millis(50);
        assert_eq!(
            tracker.velocity_at(paused),
            Velocity::ZERO,
            "a drag the user parked must not be flung"
        );
    }

    #[test]
    fn two_samples_a_fraction_of_a_millisecond_apart_report_nothing() {
        let mut tracker = VelocityTracker::new();
        let start = Instant::now();
        tracker.sample(point(px(0.0), px(0.0)), start);
        let next = start + Duration::from_micros(200);
        tracker.sample(point(px(0.0), px(3.0)), next);
        assert_eq!(tracker.velocity_at(next), Velocity::ZERO);
    }

    #[test]
    fn a_sample_that_arrives_out_of_order_is_ignored() {
        let (mut tracker, now) = steady(600.0, 8);
        let before = tracker.velocity_at(now);
        tracker.sample(point(px(0.0), px(-400.0)), now - Duration::from_millis(30));
        assert_eq!(tracker.velocity_at(now), before);
    }

    #[test]
    fn a_flick_and_a_slow_drag_of_the_same_distance_are_different_gestures() {
        let travel = point(px(120.0), px(0.0));
        let quick = Velocity::new(theme().motion.flick_velocity * 2.0, 0.0);
        let slow = Velocity::new(theme().motion.flick_velocity / 4.0, 0.0);
        assert_eq!(flick(travel, quick, &theme()), Some(Flick::Right));
        assert_eq!(flick(travel, slow, &theme()), None);
    }

    #[test]
    fn a_flick_takes_its_direction_from_the_axis_it_travelled_on() {
        let fast = theme().motion.flick_velocity * 2.0;
        assert_eq!(
            flick(
                point(px(0.0), px(-90.0)),
                Velocity::new(0.0, -fast),
                &theme()
            ),
            Some(Flick::Up)
        );
        assert_eq!(
            flick(
                point(px(-90.0), px(0.0)),
                Velocity::new(-fast, 0.0),
                &theme()
            ),
            Some(Flick::Left)
        );
    }

    #[test]
    fn a_gesture_already_on_its_way_back_was_not_flicked_out() {
        let fast = theme().motion.flick_velocity * 2.0;
        assert_eq!(
            flick(
                point(px(120.0), px(0.0)),
                Velocity::new(-fast, 0.0),
                &theme()
            ),
            None
        );
    }

    #[test]
    fn a_band_resists_more_the_further_it_is_pulled() {
        let extent = px(300.0);
        let tension = theme().motion.rubber_band_tension;
        let short = rubber_band(px(40.0), extent, tension);
        let long = rubber_band(px(200.0), extent, tension);
        assert!(short < long);
        assert!(short < px(40.0) && long < px(200.0));
        assert!(
            f32::from(long) / 200.0 < f32::from(short) / 40.0,
            "resistance did not grow with the pull"
        );
    }

    #[test]
    fn a_band_never_reaches_its_bound() {
        let extent = px(300.0);
        let tension = theme().motion.rubber_band_tension;
        for pull in [10.0, 500.0, 5_000.0, 100_000.0] {
            assert!(rubber_band(px(pull), extent, tension) < extent, "at {pull}");
        }
        assert_eq!(rubber_band(px(0.0), extent, tension), px(0.0));
    }

    #[test]
    fn a_band_pulled_the_other_way_stretches_the_other_way() {
        let extent = px(300.0);
        let tension = theme().motion.rubber_band_tension;
        assert_eq!(
            rubber_band(px(-80.0), extent, tension),
            -rubber_band(px(80.0), extent, tension)
        );
    }

    #[test]
    fn a_boundary_with_no_room_behind_it_does_not_stretch() {
        assert_eq!(rubber_band(px(50.0), px(0.0), 0.55), px(0.0));
    }

    #[test]
    fn an_overscroll_paint_uses_the_theme_band_and_never_reaches_it() {
        let theme = theme();
        let painted = overscroll(px(80.0), &theme);
        assert!(painted > px(0.0));
        assert!(painted < px(theme.effects.edge_fade_band));
        assert_eq!(overscroll(px(0.0), &theme), px(0.0));
    }
}
