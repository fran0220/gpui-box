//! What work in progress looks like.
//!
//! Every surface in this library that can be waiting had been deciding for
//! itself whether to say so, and most of them decided not to: a running tool
//! call drew a rotation glyph that did not rotate, an indeterminate ring drew
//! a full circle that did not move, and a thinking block drew nothing at all.
//! A still picture of a rotation arrow does not read as "working", it reads as
//! "stuck", which is the one thing it was there to rule out.
//!
//! So the choice is made once, here, and the three answers are three claims
//! about the work rather than three decorations:
//!
//! - [`Activity::Advancing`] — the extent is known and the work is moving
//!   through it. A band sweeps across, in the direction the work is going.
//! - [`Activity::Working`] — it is definitely running and nobody can say how
//!   much is left. A mark turns, because a turn has no end to imply.
//! - [`Activity::Deliberating`] — something is being weighed and there is no
//!   progress to report at all. It breathes, because a breath claims even
//!   less than a turn does.
//!
//! Picking the wrong one is not a style mistake, it is a false statement: a
//! sweep on work of unknown extent draws a finish line that does not exist.
//!
//! # Reduced motion
//!
//! Every helper here checks [`reduce_motion`] and returns the element
//! unanimated when it is set. That is deliberately not the same as letting the
//! repeating animation hold frame zero: frame zero of a turn is a rotation
//! glyph sitting still, which is exactly the "stuck" reading this module
//! exists to remove. Under reduced motion the state is carried by the colour
//! and the published `busy` flag, which were carrying it anyway.

use gpui::{
    AnimationExt as _, AnyElement, App, ElementId, IntoElement, ParentElement, Styled, Svg,
    Transformation, div, percentage, px, relative,
};
use gpui_kit_theme::Theme;

use super::spec::{MotionSpec, pulse_wave, shimmer_offset};
use super::{CubicBezier, Easing, reduce_motion};

/// How faint a breathing element gets at the bottom of its breath.
///
/// It never reaches nothing: an element that vanished would read as having
/// been removed rather than as still being there and still working.
const BREATH_FLOOR: f32 = 0.45;
/// How much of the swept element the travelling band covers.
const SWEEP_BAND: f32 = 0.35;

/// The shape of a piece of work that is currently under way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    /// The extent is known and the work is moving through it.
    Advancing,
    /// It is running, and how much is left is not known.
    Working,
    /// Something is being weighed, with no progress to report.
    Deliberating,
}

impl Activity {
    /// How long one repetition takes.
    pub fn period_ms(self, theme: &Theme) -> u64 {
        match self {
            Self::Advancing => theme.motion.shimmer_ms,
            Self::Working => theme.motion.spin_ms,
            Self::Deliberating => theme.motion.pulse_ms,
        }
    }

    /// The curve the repetition runs on.
    ///
    /// A turn is linear because any easing on a loop puts a stall at the seam,
    /// and a mark that hesitates once per revolution reads as a mark that is
    /// catching on something. The other two ease, because both of them have a
    /// natural turning point where slowing down is the honest shape.
    pub fn curve(self, theme: &Theme) -> CubicBezier {
        match self {
            Self::Working => Easing::Linear.curve(theme),
            Self::Advancing | Self::Deliberating => Easing::EaseInOut.curve(theme),
        }
    }

    fn spec(self, theme: &Theme) -> MotionSpec {
        MotionSpec::new(self.period_ms(theme), self.curve(theme))
    }
}

/// Turns a glyph, for work whose remaining extent is unknown.
///
/// Takes an [`Svg`] rather than any element because rotation is a transform
/// and GPUI carries transforms on `Svg` alone. [`crate::display::icon::paint`]
/// is the supported way to get one that already honours reading direction.
pub fn spin(icon: Svg, id: impl Into<ElementId>, theme: &Theme, cx: &App) -> AnyElement {
    if reduce_motion(cx) {
        return icon.into_any_element();
    }
    let activity = Activity::Working;
    icon.with_animation(
        id.into(),
        activity.spec(theme).repeating(),
        |element, progress| {
            element.with_transformation(Transformation::rotate(percentage(progress)))
        },
    )
    .into_any_element()
}

/// Breathes an element, for work that has nothing to report yet.
///
/// Opacity only. A breath that also moved or resized would shift the text
/// beside it on every cycle, and the whole point of this one is that it is the
/// quietest of the three.
pub fn breathe<E>(element: E, id: impl Into<ElementId>, theme: &Theme, cx: &App) -> AnyElement
where
    E: Styled + IntoElement + 'static,
{
    if reduce_motion(cx) {
        return element.into_any_element();
    }
    let activity = Activity::Deliberating;
    element
        .with_animation(
            id.into(),
            activity.spec(theme).repeating(),
            |element, progress| {
                element.opacity(BREATH_FLOOR + (1.0 - BREATH_FLOOR) * pulse_wave(progress))
            },
        )
        .into_any_element()
}

/// A band travelling across whatever it is placed in, for work of known extent.
///
/// Returns an absolutely positioned overlay, so the caller adds it as a child
/// of a `relative().overflow_hidden()` element and nothing it already draws
/// moves. It paints in `color`, which callers set to the accent or to the
/// track they are sweeping over.
pub fn sweep(
    id: impl Into<ElementId>,
    theme: &Theme,
    color: gpui::Hsla,
    cx: &App,
) -> Option<AnyElement> {
    if reduce_motion(cx) {
        return None;
    }
    let activity = Activity::Advancing;
    // Two halves rather than one block: a band needs three stops — up, held,
    // and back down — and this renderer takes two per gradient.
    let band = div()
        .absolute()
        .top_0()
        .bottom_0()
        .flex()
        .flex_row()
        .w(relative(SWEEP_BAND))
        .child(div().h_full().w(relative(0.5)).bg(gpui::linear_gradient(
            90.0,
            gpui::linear_color_stop(color.opacity(0.0), 0.0),
            gpui::linear_color_stop(color, 1.0),
        )))
        .child(div().h_full().w(relative(0.5)).bg(gpui::linear_gradient(
            90.0,
            gpui::linear_color_stop(color, 0.0),
            gpui::linear_color_stop(color.opacity(0.0), 1.0),
        )))
        .with_animation(
            id.into(),
            activity.spec(theme).repeating(),
            |element, progress| element.left(relative(shimmer_offset(progress, SWEEP_BAND))),
        );
    Some(band.into_any_element())
}

/// A dot that breathes, for a place with room for a mark but not a glyph.
///
/// The size is the caller's, because this sits inside layouts that have
/// already decided how much room the mark gets.
pub fn breathing_dot(
    id: impl Into<ElementId>,
    theme: &Theme,
    color: gpui::Hsla,
    size: f32,
    cx: &App,
) -> AnyElement {
    breathe(div().size(px(size)).rounded_full().bg(color), id, theme, cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme::studio_dark()
    }

    /// The three answers are three different claims, so no two of them may
    /// run on the same period: a reader who learns the rhythm of one is
    /// entitled to read a different rhythm as a different statement.
    #[test]
    fn the_three_activities_run_at_three_different_rates() {
        let theme = theme();
        let periods = [
            Activity::Advancing.period_ms(&theme),
            Activity::Working.period_ms(&theme),
            Activity::Deliberating.period_ms(&theme),
        ];
        for (index, period) in periods.iter().enumerate() {
            for other in &periods[index + 1..] {
                assert_ne!(period, other);
            }
        }
    }

    /// A loop that eases has to stall somewhere, and on a turn that stall
    /// lands at the seam and reads as the mark catching on something.
    #[test]
    fn a_turn_runs_linear_and_the_others_do_not() {
        let theme = theme();
        let linear = Easing::Linear.curve(&theme);
        assert_eq!(Activity::Working.curve(&theme), linear);
        assert_ne!(Activity::Deliberating.curve(&theme), linear);
        assert_ne!(Activity::Advancing.curve(&theme), linear);
    }

    /// Every period comes from the theme, so a host that retunes motion
    /// retunes these with it rather than finding three numbers welded in.
    #[test]
    fn every_period_is_a_token_the_theme_carries() {
        let theme = theme();
        assert_eq!(
            Activity::Advancing.period_ms(&theme),
            theme.motion.shimmer_ms
        );
        assert_eq!(Activity::Working.period_ms(&theme), theme.motion.spin_ms);
        assert_eq!(
            Activity::Deliberating.period_ms(&theme),
            theme.motion.pulse_ms
        );
    }

    /// A breath must not reach nothing: an element that vanished would read
    /// as having been removed rather than as still working.
    #[test]
    fn a_breath_never_fades_to_nothing() {
        const { assert!(BREATH_FLOOR > 0.0) };
        for step in 0..=8 {
            let opacity = BREATH_FLOOR + (1.0 - BREATH_FLOOR) * pulse_wave(step as f32 / 8.0);
            assert!(opacity >= BREATH_FLOOR, "{opacity}");
            assert!(opacity <= 1.0, "{opacity}");
        }
    }
}
