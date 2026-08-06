//! Duration, delay and curve bundled as one reusable specification.

use std::time::Duration;

use gpui::{Animation, AnimationElement, AnimationExt, ElementId, IntoElement, Styled, px};
use gpui_kit_theme::Theme;

use super::easing::{CubicBezier, Easing};

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
    MotionSpec::new(theme.motion.entrance_ms, Easing::Settle.curve(theme))
}

pub fn menu(theme: &Theme) -> MotionSpec {
    MotionSpec::new(theme.motion.menu_ms, Easing::Standard.curve(theme))
}

pub fn dialog(theme: &Theme) -> MotionSpec {
    MotionSpec::new(theme.motion.dialog_ms, Easing::Standard.curve(theme))
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
    fn delayed_specs_hold_then_finish() {
        let spec = MotionSpec::new(500, CubicBezier::new(0.0, 0.0, 1.0, 1.0)).with_delay(500);
        assert_eq!(spec.progress(0.25), 0.0);
        assert_eq!(spec.progress(1.0), 1.0);
    }

    #[test]
    fn a_zero_duration_spec_is_already_complete() {
        let spec = MotionSpec::new(0, CubicBezier::new(0.0, 0.0, 1.0, 1.0));
        assert_eq!(spec.progress(0.0), 1.0);
    }

    #[test]
    fn theme_presets_carry_their_token_durations() {
        let theme = Theme::studio_dark();
        assert_eq!(menu(&theme).duration_ms, theme.motion.menu_ms);
        assert_eq!(dialog(&theme).duration_ms, theme.motion.dialog_ms);
        assert_eq!(entrance(&theme).duration_ms, theme.motion.entrance_ms);
    }

    #[test]
    fn gradient_pulse_stays_in_range() {
        for step in 0..200 {
            let opacity = gradient_opacity(step as f32 / 100.0, 0.1);
            assert!((0.1..=1.0).contains(&opacity));
        }
    }
}
