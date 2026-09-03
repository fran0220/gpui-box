//! A closed-form damped-spring solver.
//!
//! Springs are described the way designers reason about them (stiffness,
//! damping, mass) and evaluated analytically, so a value at any instant costs
//! the same regardless of frame rate and never accumulates integration drift.

use std::time::Duration;

use gpui::{Animation, SpringConfig, SpringState};
use gpui_kit_theme::{SpringPreset, SpringTokens, Theme};

/// The fraction of the remaining distance treated as arrived.
const SETTLE_EPSILON: f32 = 0.001;
/// A displacement smaller than this is not worth another paint.
const PIXEL_SETTLE: f32 = 0.1;
/// An opacity change smaller than this is not worth another paint.
const OPACITY_SETTLE: f32 = 0.001;
/// A spring that has not settled by this point is treated as settled anyway,
/// so an over-soft configuration cannot animate forever.
const MAX_SETTLE: Duration = Duration::from_secs(4);
/// How far a bounce may be pushed either way. The mapping below runs to a
/// damping ratio of zero at 1 and to infinity at -1, neither of which is a
/// spring that ever arrives, so the ends are held just short of both.
const BOUNCE_LIMIT: f32 = 0.99;

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

    /// A spring described the way a design decision is: how long it takes and
    /// how much it overshoots.
    ///
    /// Stiffness, damping and mass are three numbers for two decisions, and
    /// neither decision is either of the three. This is the same
    /// parameterisation as SwiftUI's `Spring(duration:bounce:)`, and it is a
    /// change of variables rather than an approximation:
    ///
    /// - mass is fixed at 1. A spring's behaviour depends on stiffness and
    ///   damping only through `k/m` and `c/m`, so nothing is lost by it;
    /// - `duration` is the period of the undamped oscillation, which is what
    ///   sets the pace whatever the damping does:
    ///   `omega = 2 * PI / duration`, and `stiffness = omega^2 * mass`;
    /// - `bounce` is the damping ratio turned inside out, so that 0 is
    ///   critically damped whichever side of it the value is on:
    ///   `zeta = 1 - bounce` for a positive bounce, which reaches the
    ///   undamped `zeta = 0` at 1, and `zeta = 1 / (1 + bounce)` for a
    ///   negative one, which grows without bound toward -1. Damping follows
    ///   from the ratio: `damping = 2 * zeta * sqrt(stiffness * mass)`, which
    ///   with the frequency above is `4 * PI * zeta * mass / duration`.
    ///
    /// So a bounce of 0 settles without passing its target, a positive bounce
    /// overshoots and comes back, and a negative one crawls in. The bounce is
    /// held inside `-0.99..=0.99`.
    ///
    /// [`Spring::new`] is unchanged and remains the way in for a spring whose
    /// three constants are already known, a token preset included.
    pub fn perceptual(duration: Duration, bounce: f32) -> Self {
        let mass = 1.0;
        let seconds = duration.as_secs_f32().max(f32::EPSILON);
        let omega = std::f32::consts::TAU / seconds;
        let bounce = bounce.clamp(-BOUNCE_LIMIT, BOUNCE_LIMIT);
        let zeta = if bounce >= 0.0 {
            1.0 - bounce
        } else {
            1.0 / (1.0 + bounce)
        };
        Self::new(omega * omega * mass, 2.0 * zeta * omega * mass, mass)
    }

    pub fn preset(theme: &Theme, preset: SpringPreset) -> Self {
        Self::from(theme.spring(preset))
    }

    fn config(self) -> SpringConfig {
        SpringConfig::new(self.stiffness, self.damping, self.mass)
    }

    /// Undamped angular frequency.
    fn omega(self) -> f32 {
        self.config().canonical().0
    }

    /// Damping ratio: below one oscillates, one settles fastest, above one crawls.
    pub fn damping_ratio(self) -> f32 {
        self.config().canonical().1
    }

    /// The duration half of [`Spring::perceptual`], for a spring built any
    /// other way: the period of the undamped oscillation.
    ///
    /// It is not the settle time. A spring is still perceptibly arriving after
    /// its perceptual duration — at a bounce of 0 it is roughly 99% of the way
    /// there — and [`Spring::settle_time`] is the honest end of the motion.
    pub fn perceptual_duration(self) -> Duration {
        Duration::from_secs_f32(std::f32::consts::TAU / self.omega())
    }

    /// The bounce half, inverted from the damping ratio.
    pub fn bounce(self) -> f32 {
        let zeta = self.damping_ratio();
        if zeta <= 1.0 {
            1.0 - zeta
        } else {
            1.0 / zeta - 1.0
        }
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
        let state = self.config().step(
            SpringState {
                position: 0.0,
                velocity,
            },
            1.0,
            elapsed.as_secs_f32(),
        );
        (state.position, state.velocity)
    }

    /// How long until the spring stays within one part in a thousand of its target.
    pub fn settle_time(self) -> Duration {
        self.settle_time_at(0.0)
    }

    /// The same, for a spring released with `velocity` already carried into
    /// the motion: one that is travelling fast needs longer to come to rest.
    pub fn settle_time_at(self, velocity: f32) -> Duration {
        self.settle_time_at_for(velocity, 1.0)
    }

    /// How long until the remaining change is smaller than a pixel or an
    /// opacity step, for a motion whose full span is `span`.
    ///
    /// A ten-pixel slide can stop when 0.1px remains. A fade whose span is
    /// `1.0` still uses the opacity floor. Both also require the velocity to
    /// have dropped below the same floor, so a spring that is still racing
    /// through the epsilon band is not marked settled.
    pub fn settle_time_for(self, span: f32) -> Duration {
        self.settle_time_at_for(0.0, span)
    }

    /// The same, for a spring released with `velocity` already carried in.
    pub fn settle_time_at_for(self, velocity: f32, span: f32) -> Duration {
        let step = Duration::from_millis(4);
        let mut elapsed = step;
        while elapsed < MAX_SETTLE {
            if self.is_visually_settled(elapsed, velocity, span)
                && self.is_visually_settled(elapsed + step, velocity, span)
            {
                return elapsed;
            }
            elapsed += step;
        }
        MAX_SETTLE
    }

    /// Whether the remaining visual change is below a pixel or an opacity step.
    pub fn is_visually_settled(self, elapsed: Duration, velocity: f32, span: f32) -> bool {
        let (value, rate) = self.value_at(elapsed, velocity);
        let remaining = (1.0 - value).abs() * span.max(0.0);
        let speed = rate.abs() * span.max(0.0);
        remaining < Self::visual_epsilon(span) && speed < Self::visual_epsilon(span)
    }

    fn visual_epsilon(span: f32) -> f32 {
        if span <= 1.0 {
            OPACITY_SETTLE.max(SETTLE_EPSILON * span.max(f32::EPSILON))
        } else {
            (PIXEL_SETTLE / span).max(SETTLE_EPSILON)
        }
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

    fn peak(spring: Spring) -> f32 {
        let settle = spring.settle_time();
        (0..=400)
            .map(|step| spring.value(settle.mul_f32(step as f32 / 400.0)))
            .fold(f32::MIN, f32::max)
    }

    #[test]
    fn a_bounce_of_zero_is_critical_damping() {
        let spring = Spring::perceptual(Duration::from_millis(400), 0.0);
        assert!((spring.damping_ratio() - 1.0).abs() < 1e-4);
        assert!(peak(spring) <= 1.0 + SETTLE_EPSILON, "it passed its target");
    }

    #[test]
    fn only_a_positive_bounce_overshoots() {
        let duration = Duration::from_millis(400);
        let bouncy = Spring::perceptual(duration, 0.4);
        let sluggish = Spring::perceptual(duration, -0.4);
        assert!((bouncy.damping_ratio() - 0.6).abs() < 1e-4);
        assert!((sluggish.damping_ratio() - 1.0 / 0.6).abs() < 1e-4);
        assert!(peak(bouncy) > 1.0, "a positive bounce passes its target");
        assert!(
            peak(sluggish) <= 1.0 + SETTLE_EPSILON,
            "a negative bounce must only ever approach it"
        );
    }

    #[test]
    fn a_perceptual_spring_is_nearly_arrived_at_the_duration_it_was_given() {
        for ms in [150, 400, 900] {
            let duration = Duration::from_millis(ms);
            for bounce in [0.0, 0.3, 0.6] {
                let spring = Spring::perceptual(duration, bounce);
                let arrived = spring.value(duration);
                // A bouncier spring is still visibly moving at its duration —
                // that is what the bounce bought — so the tolerance covers the
                // oscillation left at one period rather than the arrival.
                assert!(
                    (arrived - 1.0).abs() < 0.1,
                    "{ms}ms at bounce {bounce} was {arrived} of the way there"
                );
            }
        }
    }

    #[test]
    fn a_negative_bounce_buys_its_calm_with_time() {
        // An overdamped spring keeps the pace it was asked for but no longer
        // lands on it: the duration is the frequency, not the arrival.
        let duration = Duration::from_millis(400);
        let arrived = |bounce| Spring::perceptual(duration, bounce).value(duration);
        assert!(arrived(-0.2) < arrived(0.0));
        assert!(arrived(-0.5) < arrived(-0.2));
        assert!(arrived(-0.5) > 0.7, "it is still most of the way there");
    }

    #[test]
    fn duration_and_bounce_survive_the_round_trip() {
        for ms in [120, 350, 1000] {
            for bounce in [-0.6, -0.2, 0.0, 0.25, 0.75] {
                let asked = Duration::from_millis(ms);
                let spring = Spring::perceptual(asked, bounce);
                let read = spring.perceptual_duration();
                assert!(
                    read.abs_diff(asked) < Duration::from_millis(1),
                    "{asked:?} at bounce {bounce} came back as {read:?}"
                );
                assert!(
                    (spring.bounce() - bounce).abs() < 1e-3,
                    "bounce {bounce} came back as {}",
                    spring.bounce()
                );
            }
        }
    }

    #[test]
    fn a_longer_perceptual_duration_is_a_proportionally_longer_spring() {
        let short = Spring::perceptual(Duration::from_millis(200), 0.2);
        let long = Spring::perceptual(Duration::from_millis(400), 0.2);
        let ratio = long.settle_time().as_secs_f32() / short.settle_time().as_secs_f32();
        assert!(
            (ratio - 2.0).abs() < 0.05,
            "twice the duration settled in {ratio} times the time"
        );
    }

    #[test]
    fn a_bounce_past_the_limit_is_held_at_it() {
        let duration = Duration::from_millis(300);
        for bounce in [-4.0, 4.0] {
            let spring = Spring::perceptual(duration, bounce);
            assert!(
                (spring.bounce().abs() - BOUNCE_LIMIT).abs() < 1e-3,
                "bounce {bounce} became {}",
                spring.bounce()
            );
            assert!(
                spring.damping_ratio() > 0.0,
                "bounce {bounce} lost its damping"
            );
        }
        // Both ends are still solvable, whatever they do to the settle time.
        assert!(
            Spring::perceptual(duration, 4.0)
                .value(duration)
                .is_finite()
        );
        assert!(
            Spring::perceptual(duration, -4.0)
                .value(duration)
                .is_finite()
        );
    }

    #[test]
    fn the_token_presets_are_untouched_by_the_perceptual_way_in() {
        let tokens = Theme::studio_dark().spring(SpringPreset::Smooth);
        let preset = spring(SpringPreset::Smooth);
        assert_eq!(
            preset,
            Spring::new(tokens.stiffness, tokens.damping, tokens.mass)
        );
        assert_eq!(preset.stiffness, 180.0);
        assert_eq!(preset.damping, 26.0);
        assert_eq!(preset.mass, 1.0);
    }

    #[test]
    fn a_spring_thrown_the_wrong_way_takes_longer_to_come_to_rest() {
        let spring = spring(SpringPreset::Smooth);
        assert!(spring.settle_time_at(-6.0) > spring.settle_time());
    }

    #[test]
    fn a_short_pixel_span_settles_sooner_than_the_normalized_clock() {
        let spring = spring(SpringPreset::Smooth);
        assert!(spring.settle_time_for(8.0) <= spring.settle_time());
        assert!(spring.is_visually_settled(spring.settle_time_for(8.0), 0.0, 8.0));
    }
}
