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
        let t = elapsed.as_secs_f32();
        if t <= 0.0 {
            return 0.0;
        }
        let omega = self.omega();
        let zeta = self.damping_ratio();
        let displacement = if zeta < 1.0 {
            let damped = omega * (1.0 - zeta * zeta).sqrt();
            (-zeta * omega * t).exp()
                * ((damped * t).cos() + (zeta * omega / damped) * (damped * t).sin())
        } else if (zeta - 1.0).abs() < f32::EPSILON {
            (-omega * t).exp() * (1.0 + omega * t)
        } else {
            let root = omega * (zeta * zeta - 1.0).sqrt();
            let first = -zeta * omega + root;
            let second = -zeta * omega - root;
            (second * (first * t).exp() - first * (second * t).exp()) / (second - first)
        };
        1.0 - displacement
    }

    /// How long until the spring stays within one part in a thousand of its target.
    pub fn settle_time(self) -> Duration {
        let step = Duration::from_millis(4);
        let mut elapsed = step;
        while elapsed < MAX_SETTLE {
            if (1.0 - self.value(elapsed)).abs() < SETTLE_EPSILON
                && (1.0 - self.value(elapsed + step)).abs() < SETTLE_EPSILON
            {
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
}
