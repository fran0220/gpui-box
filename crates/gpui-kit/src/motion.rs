//! Reduced-motion-aware animation specs and pure easing/loader math.
//!
//! The CSS-compatible cubic-bezier evaluator is derived from Comet's MIT
//! licensed motion catalog at `fb22e269…`; see `PROVENANCE.md`.

use std::time::Duration;

use gpui::{Animation, AnimationElement, ElementId, IntoElement, Styled, px};
use gpui_kit_theme::Theme;

pub use gpui::AnimationExt;

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
        self.sample_y(t).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSpec {
    pub duration_ms: u64,
    pub delay_ms: u64,
    pub curve: CubicBezier,
}

impl MotionSpec {
    pub const fn new(duration_ms: u64, curve: CubicBezier) -> Self {
        Self {
            duration_ms,
            delay_ms: 0,
            curve,
        }
    }

    pub const fn with_delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    pub fn total(self) -> Duration {
        Duration::from_millis(self.duration_ms + self.delay_ms)
    }

    pub fn progress(self, raw: f32) -> f32 {
        let total = (self.duration_ms + self.delay_ms) as f32;
        if total == 0.0 || self.duration_ms == 0 {
            return 1.0;
        }
        let local = (raw.clamp(0.0, 1.0) * total - self.delay_ms as f32) / self.duration_ms as f32;
        self.curve.eval(local.clamp(0.0, 1.0))
    }

    pub fn animation(self) -> Animation {
        Animation::new(self.total()).with_easing(move |delta| self.progress(delta))
    }

    pub fn repeating(self) -> Animation {
        Animation::new(self.total()).repeat()
    }
}

pub fn entrance(theme: &Theme) -> MotionSpec {
    MotionSpec::new(
        theme.motion.entrance_ms,
        CubicBezier::new(
            theme.motion.settle[0],
            theme.motion.settle[1],
            theme.motion.settle[2],
            theme.motion.settle[3],
        ),
    )
}

pub fn menu(theme: &Theme) -> MotionSpec {
    MotionSpec::new(
        theme.motion.menu_ms,
        CubicBezier::new(
            theme.motion.standard[0],
            theme.motion.standard[1],
            theme.motion.standard[2],
            theme.motion.standard[3],
        ),
    )
}

pub fn dialog(theme: &Theme) -> MotionSpec {
    MotionSpec::new(
        theme.motion.dialog_ms,
        CubicBezier::new(
            theme.motion.standard[0],
            theme.motion.standard[1],
            theme.motion.standard[2],
            theme.motion.standard[3],
        ),
    )
}

pub fn fade_in<E>(id: impl Into<ElementId>, theme: &Theme, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, entrance(theme).animation(), |element, progress| {
        element
            .relative()
            .opacity(progress)
            .top(px(4.0 * (1.0 - progress)))
    })
}

pub fn menu_in<E>(id: impl Into<ElementId>, theme: &Theme, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, menu(theme).animation(), |element, progress| {
        element
            .relative()
            .opacity(0.3 + 0.7 * progress)
            .top(px(-2.0 * (1.0 - progress)))
    })
}

pub fn dialog_in<E>(id: impl Into<ElementId>, theme: &Theme, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, dialog(theme).animation(), |element, progress| {
        element
            .relative()
            .opacity(progress)
            .top(px(2.0 * (1.0 - progress)))
    })
}

pub fn pulse_wave(phase: f32) -> f32 {
    0.5 - 0.5 * (phase * std::f32::consts::TAU).cos()
}

pub fn staggered_phase(raw: f32, index: usize, stagger: f32) -> f32 {
    (raw - index as f32 * stagger).rem_euclid(1.0)
}

pub fn gradient_opacity(phase: f32, dim: f32) -> f32 {
    let phase = phase.rem_euclid(1.0);
    if phase < 0.45 {
        1.0 + (dim - 1.0) * (phase / 0.45)
    } else if phase < 0.92 {
        dim
    } else {
        dim + (1.0 - dim) * ((phase - 0.92) / 0.08)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_has_exact_endpoints_and_bounded_output() {
        let curve = CubicBezier::new(0.16, 1.0, 0.3, 1.0);
        assert_eq!(curve.eval(0.0), 0.0);
        assert_eq!(curve.eval(1.0), 1.0);
        for step in 0..=100 {
            assert!((0.0..=1.0).contains(&curve.eval(step as f32 / 100.0)));
        }
    }

    #[test]
    fn delayed_specs_hold_then_finish() {
        let spec = MotionSpec::new(500, CubicBezier::new(0.0, 0.0, 1.0, 1.0)).with_delay(500);
        assert_eq!(spec.progress(0.25), 0.0);
        assert_eq!(spec.progress(1.0), 1.0);
    }

    #[test]
    fn gradient_pulse_stays_in_range() {
        for step in 0..200 {
            let opacity = gradient_opacity(step as f32 / 100.0, 0.1);
            assert!((0.1..=1.0).contains(&opacity));
        }
    }
}
