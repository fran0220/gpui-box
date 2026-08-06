//! Named curves and the CSS-compatible evaluator behind them.
//!
//! The cubic-bezier evaluator is derived from Comet's MIT licensed motion
//! catalog at `fb22e269…`; see `PROVENANCE.md`.

use gpui_kit_theme::Theme;
use gpui_kit_tokens::MotionEasing;

/// A CSS-compatible cubic-bezier curve, evaluated in pure Rust so a curve can
/// be asserted without a window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl CubicBezier {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    pub const fn from_points(points: [f32; 4]) -> Self {
        Self::new(points[0], points[1], points[2], points[3])
    }

    fn coefficients(a: f32, b: f32) -> (f32, f32, f32) {
        let c = 3.0 * a;
        let second = 3.0 * (b - a) - c;
        (1.0 - c - second, second, c)
    }

    fn sample_x(self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.x1, self.x2);
        ((a * t + b) * t + c) * t
    }

    fn sample_y(self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.y1, self.y2);
        ((a * t + b) * t + c) * t
    }

    fn derivative(self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.x1, self.x2);
        (3.0 * a * t + 2.0 * b) * t + c
    }

    /// Evaluates the curve at `input`.
    ///
    /// The result is not clamped to 0..1, because overshoot curves are
    /// expected to exceed one before settling.
    pub fn eval(self, input: f32) -> f32 {
        if input <= 0.0 {
            return 0.0;
        }
        if input >= 1.0 {
            return 1.0;
        }
        let mut t = input;
        let mut solved = false;
        for _ in 0..8 {
            let error = self.sample_x(t) - input;
            if error.abs() < 1e-6 {
                solved = true;
                break;
            }
            let derivative = self.derivative(t);
            if derivative.abs() < 1e-6 {
                break;
            }
            t -= error / derivative;
        }
        if !solved {
            let (mut low, mut high) = (0.0, 1.0);
            for _ in 0..32 {
                let middle = (low + high) / 2.0;
                if self.sample_x(middle) < input {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            t = (low + high) / 2.0;
        }
        self.sample_y(t)
    }
}

/// A curve named by role rather than by control points.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Easing {
    Linear,
    /// The default for state changes that start and end on screen.
    #[default]
    Standard,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// Fast start with a long settle, for entrances that need attention.
    Emphasized,
    /// Passes its target and returns; the only curve that leaves 0..1.
    Overshoot,
    Exit,
    Settle,
    Custom(CubicBezier),
}

impl Easing {
    pub fn curve(self, theme: &Theme) -> CubicBezier {
        let named = |easing: MotionEasing| CubicBezier::from_points(theme.easing(easing));
        match self {
            Self::Linear => named(MotionEasing::Linear),
            Self::Standard => named(MotionEasing::Standard),
            Self::EaseIn => named(MotionEasing::EaseIn),
            Self::EaseOut => named(MotionEasing::EaseOut),
            Self::EaseInOut => named(MotionEasing::EaseInOut),
            Self::Emphasized => named(MotionEasing::Emphasized),
            Self::Overshoot => named(MotionEasing::Overshoot),
            Self::Exit => named(MotionEasing::Exit),
            Self::Settle => named(MotionEasing::Settle),
            Self::Custom(curve) => curve,
        }
    }
}

impl From<CubicBezier> for Easing {
    fn from(curve: CubicBezier) -> Self {
        Self::Custom(curve)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_has_exact_endpoints() {
        let curve = CubicBezier::new(0.16, 1.0, 0.3, 1.0);
        assert_eq!(curve.eval(0.0), 0.0);
        assert_eq!(curve.eval(1.0), 1.0);
    }

    #[test]
    fn a_linear_curve_is_the_identity() {
        let curve = CubicBezier::new(0.0, 0.0, 1.0, 1.0);
        for step in 0..=20 {
            let input = step as f32 / 20.0;
            assert!((curve.eval(input) - input).abs() < 1e-3);
        }
    }

    #[test]
    fn an_overshoot_curve_passes_its_target_before_returning() {
        let curve = CubicBezier::new(0.34, 1.56, 0.64, 1.0);
        let peak = (0..=100)
            .map(|step| curve.eval(step as f32 / 100.0))
            .fold(f32::MIN, f32::max);
        assert!(
            peak > 1.0,
            "overshoot must exceed its target, peaked at {peak}"
        );
        assert_eq!(curve.eval(1.0), 1.0);
    }

    #[test]
    fn every_named_curve_resolves_from_the_theme() {
        let theme = Theme::studio_dark();
        for easing in [
            Easing::Linear,
            Easing::Standard,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::Emphasized,
            Easing::Overshoot,
            Easing::Exit,
            Easing::Settle,
        ] {
            let curve = easing.curve(&theme);
            assert_eq!(curve.eval(0.0), 0.0);
            assert_eq!(curve.eval(1.0), 1.0);
        }
    }
}
