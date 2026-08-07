//! A closed-form damped-spring solver.
//!
//! Springs are described the way designers reason about them (stiffness,
//! damping, mass) and evaluated analytically, so a value at any instant costs
//! the same regardless of frame rate and never accumulates integration drift.

use std::time::Duration;

use gpui::Animation;
use gpui_kit_theme::{SpringPreset, SpringTokens, Theme};

/// The fraction of the remaining distance treated as arrived.
const SETTLE_EPSILON: f32 = 0.001;
/// A spring that has not settled by this point is treated as settled anyway,
/// so an over-soft configuration cannot animate forever.
const MAX_SETTLE: Duration = Duration::from_secs(4);

/// A damped spring, solved in closed form so a position is a function of time
/// rather than of how many frames happened to be delivered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
}

impl Spring {
    pub fn new(stiffness: f32, damping: f32, mass: f32) -> Self {
        Self {
            stiffness: stiffness.max(f32::EPSILON),
            damping: damping.max(0.0),
            mass: mass.max(f32::EPSILON),
        }
    }

    pub fn preset(theme: &Theme, preset: SpringPreset) -> Self {
        Self::from(theme.spring(preset))
    }

    /// Undamped angular frequency.
    fn omega(self) -> f32 {
        (self.stiffness / self.mass).sqrt()
    }

    /// Damping ratio: below one oscillates, one settles fastest, above one crawls.
    pub fn damping_ratio(self) -> f32 {
        self.damping / (2.0 * (self.stiffness * self.mass).sqrt())
    }

    /// Normalized step response: 0 at rest, approaching 1 as it settles.
    pub fn value(self, elapsed: Duration) -> f32 {
        self.value_at(elapsed, 0.0).0
    }

    /// Normalized step response for a spring that was already moving when it
    /// was aimed here.
    ///
    /// `velocity` is the speed carried into the motion, in units of the full
    /// distance per second and positive toward the target. The pair is the
    /// value and its own velocity, so a caller that retargets again can hand
    /// the motion on rather than restarting it.
    pub fn value_at(self, elapsed: Duration, velocity: f32) -> (f32, f32) {
        // Distance still to travel starts at the whole of it, and closing that
        // distance is what a positive carried velocity does.
        let (error, error_rate) = self.error(elapsed, 1.0, -velocity);
        (1.0 - error, -error_rate)
    }

    /// The remaining distance and its rate of change at `elapsed`, for a
    /// spring released with error `initial` changing at `initial_rate`.
    fn error(self, elapsed: Duration, initial: f32, initial_rate: f32) -> (f32, f32) {
        let t = elapsed.as_secs_f32();
        if t <= 0.0 {
            return (initial, initial_rate);
        }
        let omega = self.omega();
        let zeta = self.damping_ratio();
        if zeta < 1.0 {
            let damped = omega * (1.0 - zeta * zeta).sqrt();
            let a = initial;
            let b = (initial_rate + zeta * omega * initial) / damped;
            let decay = (-zeta * omega * t).exp();
            let (sin, cos) = (damped * t).sin_cos();
            (
                decay * (a * cos + b * sin),
                decay
                    * ((-zeta * omega * a + damped * b) * cos
                        + (-zeta * omega * b - damped * a) * sin),
            )
        } else if (zeta - 1.0).abs() < f32::EPSILON {
            let slope = initial_rate + omega * initial;
            let decay = (-omega * t).exp();
            let error = initial + slope * t;
            (decay * error, decay * (slope - omega * error))
        } else {
            let root = omega * (zeta * zeta - 1.0).sqrt();
            let first = -zeta * omega + root;
            let second = -zeta * omega - root;
            let c1 = (initial_rate - second * initial) / (first - second);
            let c2 = initial - c1;
            let (a, b) = (c1 * (first * t).exp(), c2 * (second * t).exp());
            (a + b, a * first + b * second)
        }
    }

    /// How long until the spring stays within one part in a thousand of its target.
    pub fn settle_time(self) -> Duration {
        self.settle_time_at(0.0)
    }

    /// The same, for a spring released with `velocity` already carried into
    /// the motion: one that is travelling fast needs longer to come to rest.
    pub fn settle_time_at(self, velocity: f32) -> Duration {
        let step = Duration::from_millis(4);
        let settled = |elapsed| (1.0 - self.value_at(elapsed, velocity).0).abs() < SETTLE_EPSILON;
        let mut elapsed = step;
        while elapsed < MAX_SETTLE {
            if settled(elapsed) && settled(elapsed + step) {
                return elapsed;
            }
            elapsed += step;
        }
        MAX_SETTLE
    }

    /// Expresses the spring as a GPUI animation, so it can drive
    /// `with_animation` alongside curve-based motion.
    pub fn animation(self) -> Animation {
        let settle = self.settle_time();
        Animation::new(settle).with_easing(move |delta| self.value(settle.mul_f32(delta)))
    }
}

impl From<SpringTokens> for Spring {
    fn from(tokens: SpringTokens) -> Self {
        Self::new(tokens.stiffness, tokens.damping, tokens.mass)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spring(preset: SpringPreset) -> Spring {
        Spring::preset(&Theme::studio_dark(), preset)
    }

    #[test]
    fn a_spring_starts_at_rest_and_reaches_its_target() {
        for preset in [
            SpringPreset::Snappy,
            SpringPreset::Smooth,
            SpringPreset::Bouncy,
        ] {
            let spring = spring(preset);
            assert_eq!(spring.value(Duration::ZERO), 0.0);
            let settled = spring.value(spring.settle_time());
            assert!(
                (settled - 1.0).abs() < 0.01,
                "{preset:?} settled at {settled}"
            );
        }
    }

    #[test]
    fn only_an_underdamped_spring_overshoots() {
        let bouncy = spring(SpringPreset::Bouncy);
        let smooth = spring(SpringPreset::Smooth);
        assert!(bouncy.damping_ratio() < 1.0);

        let peak = |spring: Spring| {
            (0..400)
                .map(|step| spring.value(Duration::from_millis(step * 5)))
                .fold(f32::MIN, f32::max)
        };
        assert!(peak(bouncy) > 1.0, "a bouncy spring passes its target");
        assert!(peak(smooth) <= 1.001, "a smooth spring approaches it");
    }

    #[test]
    fn a_stiffer_spring_settles_sooner() {
        let stiff = Spring::new(600.0, 30.0, 1.0);
        let soft = Spring::new(120.0, 30.0, 1.0);
        assert!(stiff.settle_time() < soft.settle_time());
    }

    #[test]
    fn an_overdamped_spring_still_converges() {
        let spring = Spring::new(200.0, 80.0, 1.0);
        assert!(spring.damping_ratio() > 1.0);
        assert!((spring.value(spring.settle_time()) - 1.0).abs() < 0.01);
    }

    #[test]
    fn settle_time_is_bounded_for_a_nearly_static_spring() {
        assert_eq!(Spring::new(1.0, 1000.0, 50.0).settle_time(), MAX_SETTLE);
    }

    /// The three damping regimes, so a claim about the solver is a claim about
    /// all of it.
    fn regimes() -> [Spring; 3] {
        [
            spring(SpringPreset::Bouncy),
            Spring::new(400.0, 40.0, 1.0),
            Spring::new(200.0, 80.0, 1.0),
        ]
    }

    #[test]
    fn released_from_rest_the_general_solution_is_the_step_response() {
        for spring in regimes() {
            for step in 0..200 {
                let elapsed = Duration::from_millis(step * 5);
                let (value, _) = spring.value_at(elapsed, 0.0);
                assert!(
                    (value - spring.value(elapsed)).abs() < 1e-4,
                    "{spring:?} diverged at {elapsed:?}: {value}"
                );
            }
        }
    }

    #[test]
    fn a_carried_velocity_is_the_starting_velocity() {
        for spring in regimes() {
            let (value, velocity) = spring.value_at(Duration::ZERO, 3.0);
            assert_eq!(value, 0.0);
            assert!((velocity - 3.0).abs() < 1e-5, "{spring:?} lost its speed");
        }
    }

    #[test]
    fn a_spring_released_with_speed_is_further_along_at_once() {
        for spring in regimes() {
            let early = Duration::from_millis(10);
            assert!(
                spring.value_at(early, 4.0).0 > spring.value(early),
                "{spring:?} did not carry its velocity"
            );
        }
    }

    #[test]
    fn every_regime_still_settles_from_a_carried_velocity() {
        for spring in regimes() {
            for velocity in [-4.0, 0.0, 6.0] {
                let settle = spring.settle_time_at(velocity);
                assert!(settle <= MAX_SETTLE);
                let settled = spring.value_at(settle, velocity).0;
                assert!(
                    (settled - 1.0).abs() < 0.01,
                    "{spring:?} at {velocity} settled on {settled}"
                );
            }
        }
    }

    #[test]
    fn a_spring_thrown_the_wrong_way_takes_longer_to_come_to_rest() {
        let spring = spring(SpringPreset::Smooth);
        assert!(spring.settle_time_at(-6.0) > spring.settle_time());
    }
}
