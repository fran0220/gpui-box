//! Duration, delay and curve bundled as one reusable specification.

use std::time::Duration;

use gpui::{Animation, AnimationElement, AnimationExt, ElementId, IntoElement, Styled, px};
use gpui_kit_theme::{SpringPreset, Theme};

use super::Spring;
use super::easing::{CubicBezier, Easing};

/// A curve with a duration and a delay: everything one animation needs, taken
/// from the token document rather than written beside the element.
///
/// A specification can carry a spring instead of a curve. Its duration is then
/// the spring's settle time, so everything that already knows how to run a
/// specification — [`Transition`](super::Transition),
/// [`Presence`](super::Presence), [`Stagger`](super::Stagger) and GPUI's
/// `with_animation` — runs a spring without knowing it is one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSpec {
    pub duration_ms: u64,
    pub delay_ms: u64,
    pub curve: CubicBezier,
    /// Set when the specification is sprung; the curve is then unused.
    spring: Option<Spring>,
}

impl MotionSpec {
    pub const fn new(duration_ms: u64, curve: CubicBezier) -> Self {
        Self {
            duration_ms,
            delay_ms: 0,
            curve,
            spring: None,
        }
    }

    /// A specification that arrives on a spring rather than along a curve.
    ///
    /// The duration is the spring's settle time, which is where the weight
    /// comes from: a spring is still moving after a curve of the same nominal
    /// length has stopped.
    pub fn sprung(spring: Spring) -> Self {
        let settle = spring.settle_time().as_millis() as u64;
        Self {
            duration_ms: settle.max(1),
            delay_ms: 0,
            curve: CubicBezier::new(0.0, 0.0, 1.0, 1.0),
            spring: Some(spring),
        }
    }

    pub const fn with_delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    pub fn is_sprung(self) -> bool {
        self.spring.is_some()
    }

    /// The spring behind a sprung specification, for callers that need more of
    /// it than an eased fraction — velocity across a retarget, for one.
    pub fn spring(self) -> Option<Spring> {
        self.spring
    }

    pub fn total(self) -> Duration {
        Duration::from_millis(self.duration_ms + self.delay_ms)
    }

    pub fn progress(self, raw: f32) -> f32 {
        let total = (self.duration_ms + self.delay_ms) as f32;
        if total == 0.0 || self.duration_ms == 0 {
            return 1.0;
        }
        let local = ((raw.clamp(0.0, 1.0) * total - self.delay_ms as f32)
            / self.duration_ms as f32)
            .clamp(0.0, 1.0);
        match self.spring {
            Some(spring) => {
                if local >= 1.0 {
                    return 1.0;
                }
                spring.value(Duration::from_secs_f32(
                    local * self.duration_ms as f32 / 1000.0,
                ))
            }
            None => self.curve.eval(local),
        }
    }

    /// Adapts the specification to GPUI's animation driver.
    ///
    /// GPUI requires an eased delta inside 0..1, so an overshooting curve or
    /// an underdamped spring is clamped here. Overshoot survives in
    /// [`Transition`](super::Transition) and [`Presence`](super::Presence),
    /// which sample [`MotionSpec::progress`] directly.
    pub fn animation(self) -> Animation {
        Animation::new(self.total()).with_easing(move |delta| self.progress(delta).clamp(0.0, 1.0))
    }

    pub fn repeating(self) -> Animation {
        Animation::new(self.total()).repeat()
    }
}

/// How a surface that is part of the page arrives.
pub fn entrance(theme: &Theme) -> MotionSpec {
    MotionSpec::new(theme.motion.entrance_ms, Easing::Settle.curve(theme))
}

/// How a menu opens: short, because it is answering a click.
pub fn menu(theme: &Theme) -> MotionSpec {
    MotionSpec::new(theme.motion.menu_ms, Easing::Standard.curve(theme))
}

/// How a modal arrives: slower, because it is taking the page over.
pub fn dialog(theme: &Theme) -> MotionSpec {
    MotionSpec::new(theme.motion.dialog_ms, Easing::Standard.curve(theme))
}

/// How a modal arrives when it should arrive with weight: on the smooth
/// spring, which keeps moving after a curve of the same length has stopped.
pub fn dialog_arrival(theme: &Theme) -> MotionSpec {
    MotionSpec::sprung(Spring::preset(theme, SpringPreset::Smooth))
}

/// How one control moves to a new state: short, because it is answering a
/// pointer that is still on it.
pub fn state_change(theme: &Theme) -> MotionSpec {
    MotionSpec::new(theme.motion.quick_ms, Easing::Standard.curve(theme))
}

/// How a value that has a position moves to a new one, such as a progress
/// fill or an opening section.
pub fn resize(theme: &Theme) -> MotionSpec {
    MotionSpec::new(theme.motion.resize_ms, Easing::Standard.curve(theme))
}

/// How a control that is being manipulated follows the value: the tight
/// spring, so it feels attached rather than trailing.
pub fn tracking(theme: &Theme) -> MotionSpec {
    MotionSpec::sprung(Spring::preset(theme, SpringPreset::Grab))
}

/// Fades `element` in over the entrance specification.
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

/// The opening a menu makes: a fade with a small rise.
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

/// The arrival a modal makes: it rises onto the page on a spring, so it
/// arrives with weight rather than simply appearing at full strength.
pub fn dialog_in<E>(id: impl Into<ElementId>, theme: &Theme, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    let spec = dialog_arrival(theme);
    element.with_animation(id, spec.animation(), |element, progress| {
        element
            .relative()
            .opacity(progress)
            .top(px(8.0 * (1.0 - progress)))
    })
}

/// The arrival one row of a menu-shaped list makes.
///
/// Opacity only, and deliberately so: a rise is a layout input, so a row that
/// slid into place would publish a moving box while it travelled. The wave is
/// [`Stagger::rows`](super::Stagger::rows), whose window is capped however
/// long the list is.
pub fn row_in<E>(
    id: impl Into<ElementId>,
    theme: &Theme,
    index: usize,
    count: usize,
    element: E,
) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    let spec = super::Stagger::rows().spec(index, count, menu(theme));
    element.with_animation(id, spec.animation(), |element, progress| {
        element.opacity(progress)
    })
}

/// The arrival a block of content makes when it replaces what was there.
///
/// The rise belongs on a wrapper inside the element that publishes the node,
/// so the published box is the settled one and only the pixels travel.
pub fn content_in<E>(id: impl Into<ElementId>, theme: &Theme, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, entrance(theme).animation(), |element, progress| {
        element
            .relative()
            .opacity(progress)
            .top(px(6.0 * (1.0 - progress)))
    })
}

/// The sweep a loading placeholder's highlight travels on, as a fraction of
/// the placeholder's own width.
///
/// The band starts fully off the leading edge and finishes fully off the
/// trailing one, so a row is never left with a bright edge parked on it.
pub fn shimmer_offset(phase: f32, band: f32) -> f32 {
    phase.rem_euclid(1.0) * (1.0 + band) - band
}

/// The repeating wave a loading placeholder breathes on.
pub fn pulse_wave(phase: f32) -> f32 {
    0.5 - 0.5 * (phase * std::f32::consts::TAU).cos()
}

/// The repeating opacity a gradient spinner turns on.
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
