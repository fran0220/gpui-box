//! Procedural micro-motion applied to any element.
//!
//! These are functions of time, not assets. A heartbeat is two damped pulses,
//! a bounce is a hop, a wobble is a short shake, a pop is an overshoot, and a
//! sparkle is an opacity shimmer. Reduced motion returns the element
//! unchanged. The layout box never moves: only paint-local offset and opacity
//! do, because GPUI carries subtree scale and rotation on `Svg` alone.

use gpui::{
    AnimationExt as _, AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, TypeScale};

use crate::foundation::{Ident, StyledExt};
use crate::strings::{ActiveStrings, StringKey};

use super::spec::MotionSpec;
use super::{Easing, reduce_motion};

/// A named procedural reaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Micro {
    Heartbeat,
    Bounce,
    Wobble,
    Pop,
    Sparkle,
}

impl Micro {
    pub fn name(self) -> &'static str {
        match self {
            Self::Heartbeat => "heartbeat",
            Self::Bounce => "bounce",
            Self::Wobble => "wobble",
            Self::Pop => "pop",
            Self::Sparkle => "sparkle",
        }
    }

    fn key(self) -> StringKey {
        match self {
            Self::Heartbeat => StringKey::MicroHeartbeat,
            Self::Bounce => StringKey::MicroBounce,
            Self::Wobble => StringKey::MicroWobble,
            Self::Pop => StringKey::MicroPop,
            Self::Sparkle => StringKey::MicroSparkle,
        }
    }

    fn repeating(self) -> bool {
        matches!(self, Self::Heartbeat | Self::Sparkle)
    }

    fn spec(self, theme: &gpui_kit_theme::Theme) -> MotionSpec {
        let duration = match self {
            Self::Heartbeat => theme.motion.pulse_ms,
            Self::Bounce => theme.motion.micro_bounce_ms,
            Self::Wobble => theme.motion.micro_wobble_ms,
            Self::Pop => theme.motion.micro_pop_ms,
            Self::Sparkle => theme.motion.shimmer_ms,
        };
        MotionSpec::new(duration, Easing::EaseInOut.curve(theme))
    }
}

/// Applies a named micro-motion. Honors reduced motion by doing nothing.
pub fn micro<E>(element: E, id: impl Into<ElementId>, kind: Micro, cx: &App) -> AnyElement
where
    E: Styled + IntoElement + 'static,
{
    let theme = cx.theme().clone();
    if reduce_motion(cx) {
        return element.into_any_element();
    }
    let spec = kind.spec(&theme);
    let animation = if kind.repeating() {
        spec.repeating()
    } else {
        spec.animation()
    };
    element
        .with_animation(id.into(), animation, move |element, delta| match kind {
            Micro::Heartbeat => {
                let pulse = heartbeat(delta);
                element.opacity(0.72 + 0.28 * pulse)
            }
            Micro::Bounce => {
                let lift = bounce(delta);
                element.mt(px(-8.0 * lift))
            }
            Micro::Wobble => element.ml(px(wobble(delta))),
            Micro::Pop => {
                let scale = pop(delta);
                element
                    .opacity((0.85 + (scale - 1.0) * 2.0).clamp(0.85, 1.0))
                    .mt(px(-6.0 * (scale - 1.0) / 0.22))
            }
            Micro::Sparkle => element.opacity(0.45 + 0.55 * sparkle(delta)),
        })
        .into_any_element()
}

/// Fluent entry point used as `.micro(id, Micro::Heartbeat, cx)`.
pub trait MicroMotion: Styled + IntoElement + 'static {
    fn micro(self, id: impl Into<ElementId>, kind: Micro, cx: &App) -> AnyElement {
        micro(self, id, kind, cx)
    }

    /// Alias matching the design language of a reaction on a glyph or emoji.
    fn reaction(self, id: impl Into<ElementId>, kind: Micro, cx: &App) -> AnyElement {
        micro(self, id, kind, cx)
    }
}

impl<E: Styled + IntoElement + 'static> MicroMotion for E {}

/// A labelled glyph that plays one named micro-motion.
#[derive(IntoElement)]
pub struct MicroMark {
    ident: Ident,
    kind: Micro,
    label: SharedString,
}

impl MicroMark {
    pub fn new(ident: impl Into<Ident>, kind: Micro, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            kind,
            label: label.into(),
        }
    }
}

impl RenderOnce for MicroMark {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let caption = cx.strings().text(self.kind.key());
        let mark = div()
            .type_scale(&theme, TypeScale::Title)
            .text_color(theme.colors.text)
            .child(self.label.clone())
            .micro(
                self.ident.child(self.kind.name()).element_id(),
                self.kind,
                cx,
            );
        div()
            .column()
            .items_center()
            .gap(px(theme.space(gpui_kit_theme::Space::Xs)))
            .child(mark)
            .child(
                div()
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.text_muted)
                    .child(caption.clone()),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Status)
                    .text(caption)
                    .value(self.kind.name()),
            )
    }
}

pub(crate) fn heartbeat(delta: f32) -> f32 {
    let t = delta.rem_euclid(1.0);
    if t < 0.18 {
        (t / 0.18 * std::f32::consts::PI).sin()
    } else if t < 0.36 {
        ((t - 0.18) / 0.18 * std::f32::consts::PI).sin() * 0.7
    } else {
        0.0
    }
}

pub(crate) fn bounce(delta: f32) -> f32 {
    let t = delta.clamp(0.0, 1.0);
    if t < 0.45 {
        (t / 0.45 * std::f32::consts::PI).sin()
    } else if t < 0.7 {
        ((t - 0.45) / 0.25 * std::f32::consts::PI).sin() * 0.35
    } else {
        0.0
    }
}

pub(crate) fn wobble(delta: f32) -> f32 {
    let t = delta.clamp(0.0, 1.0);
    (1.0 - t) * 4.0 * (t * std::f32::consts::TAU * 3.0).sin()
}

pub(crate) fn pop(delta: f32) -> f32 {
    let t = delta.clamp(0.0, 1.0);
    if t < 0.4 {
        1.0 + 0.22 * (t / 0.4 * std::f32::consts::PI).sin()
    } else {
        1.0
    }
}

pub(crate) fn sparkle(delta: f32) -> f32 {
    0.5 + 0.5 * (delta.rem_euclid(1.0) * std::f32::consts::TAU).sin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heartbeat_has_two_pulses_then_rests() {
        assert!(heartbeat(0.08) > 0.4);
        assert!(heartbeat(0.26) > 0.2);
        assert_eq!(heartbeat(0.8), 0.0);
    }

    #[test]
    fn a_bounce_leaves_the_ground_and_lands() {
        assert!(bounce(0.2) > 0.5);
        assert_eq!(bounce(1.0), 0.0);
    }

    #[test]
    fn a_pop_overshoots_then_settles() {
        assert!(pop(0.2) > 1.1);
        assert_eq!(pop(1.0), 1.0);
    }

    #[test]
    fn every_kind_has_a_stable_name() {
        assert_eq!(Micro::Heartbeat.name(), "heartbeat");
        assert_eq!(Micro::Sparkle.name(), "sparkle");
    }
}
