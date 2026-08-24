//! The working mark: the one neutral colour work in progress paints with.
//!
//! Accent stays the compact action and focus colour, and waiting is not
//! information — so the mark carries no hue at all. Surfaces that report a
//! position or a wait paint from here so a progress bar, a ring, a transport
//! scrubber and a loader cannot invent four different "working" colours, and
//! every one of them stays caller-themable through `color.loader.*` rather
//! than through anything welded in.
//!
//! The mark never enters the semantic tree. A progress node still publishes
//! its fraction and its busy flag; a reader that asked what the surface
//! claimed is not told which colour it wore.

use gpui::{
    AnimationExt as _, AnyElement, App, Div, Hsla, IntoElement, Styled, div, linear_color_stop,
    linear_gradient_stops, relative,
};
use gpui_kit_theme::Theme;

use crate::motion::{self, Activity, MotionSpec};

/// How much of an unknown-extent track the travelling band covers.
const SWEEP_BAND: f32 = 0.42;

/// How much of a placeholder the sheen highlight covers. The band and the
/// travel that moves it share this one number, so the highlight cannot park
/// a bright sliver at either end of its run.
pub(crate) const SHIMMER_BAND: f32 = 0.38;

/// The colour of the moving part: a fill, an arc, a dot.
pub(crate) fn mark(theme: &Theme) -> Hsla {
    theme.colors.loader_mark
}

/// The groove the mark travels. Quieter than the mark by construction; the
/// token gate holds it visible and holds it down.
pub(crate) fn track(theme: &Theme) -> Hsla {
    theme.colors.loader_track
}

/// A determined fill: the mark clipped to `fraction` of the track.
pub(crate) fn determined(theme: &Theme, fraction: f32) -> Div {
    filled(mark(theme), fraction)
}

/// A determined fill in a caller-meant colour: a stalled bar's warning, a
/// paused bar's dimmed mark. The geometry stays this module's so every pace
/// draws the same shape.
pub(crate) fn filled(color: Hsla, fraction: f32) -> Div {
    div()
        .absolute()
        .left_0()
        .top_0()
        .bottom_0()
        .rounded_full()
        .w(relative(fraction.clamp(0.0, 1.0)))
        .bg(color)
}

/// A band of the mark sweeping a track whose extent is unknown.
///
/// The band fades in and out at its own edges so it reads as travel rather
/// than as a fill that stops at an arbitrary position. Under reduced motion
/// it sits still at the leading edge: a still sweep that covered the whole
/// track would read as finished work.
pub(crate) fn unknown(id: impl Into<gpui::ElementId>, theme: &Theme, cx: &App) -> AnyElement {
    let color = mark(theme);
    let band = div()
        .absolute()
        .top_0()
        .bottom_0()
        .w(relative(SWEEP_BAND))
        .bg(linear_gradient_stops(
            90.0,
            [
                linear_color_stop(color.opacity(0.0), 0.0),
                linear_color_stop(color, 0.3),
                linear_color_stop(color, 0.7),
                linear_color_stop(color.opacity(0.0), 1.0),
            ],
        ));

    if motion::reduce_motion(cx) {
        return band.left(relative(0.08)).into_any_element();
    }

    let period = MotionSpec::new(
        Activity::Advancing.period_ms(theme),
        Activity::Advancing.curve(theme),
    );
    band.with_animation(id.into(), period.repeating(), |element, progress| {
        element.left(relative(motion::shimmer_offset(progress, SWEEP_BAND)))
    })
    .into_any_element()
}

/// The sheen highlight that crosses a placeholder, for a skeleton row.
///
/// Painted in `color.loader.sheen`, which the token gate measures over the
/// placeholder it crosses rather than over the bare surface.
pub(crate) fn shimmer_band(theme: &Theme) -> Div {
    let sheen = theme.colors.loader_sheen;
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .w(relative(SHIMMER_BAND))
        .bg(linear_gradient_stops(
            90.0,
            [
                linear_color_stop(sheen.opacity(0.0), 0.0),
                linear_color_stop(sheen, 0.5),
                linear_color_stop(sheen.opacity(0.0), 1.0),
            ],
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit_theme::Theme;

    /// Waiting is not information, so no loader role may carry a hue that
    /// says anything: colour on a loading surface is the caller's meaning,
    /// and the caller sets it through the token document.
    ///
    /// Stated against the theme's own semantic colours rather than against a
    /// fixed saturation, because a neutral ramp is allowed the faint cast
    /// that makes it belong to its appearance — `studio-light` builds its
    /// greys on a cool near-black. What is ruled out is a loader role
    /// carrying enough colour to read as one of the meanings the semantic
    /// roles exist to carry.
    #[test]
    fn no_loader_role_carries_a_meaningful_hue() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            let quietest_meaning = [
                theme.colors.accent,
                theme.colors.danger,
                theme.colors.warning,
                theme.colors.success,
                theme.colors.info,
            ]
            .into_iter()
            .map(|color| color.s)
            .fold(f32::INFINITY, f32::min);
            for (name, color) in [
                ("mark", mark(&theme)),
                ("track", track(&theme)),
                ("placeholder", theme.colors.loader_placeholder),
                ("sheen", theme.colors.loader_sheen),
            ] {
                assert!(
                    color.s < quietest_meaning / 4.0,
                    "{}: loader {name} reads at {} against the quietest meaning at \
                     {quietest_meaning}",
                    theme.id,
                    color.s
                );
            }
        }
    }

    #[test]
    fn the_working_mark_is_not_the_accent() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            assert_ne!(
                mark(&theme),
                theme.colors.accent,
                "working colour collapsed onto accent in {}",
                theme.id
            );
        }
    }

    /// The groove must stay quieter than the mark that travels it, or the
    /// track reads as the progress and the fill as the remainder.
    #[test]
    fn the_groove_is_quieter_than_the_mark() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            let mark_alpha = mark(&theme).a;
            let track_alpha = track(&theme).a;
            assert!(
                track_alpha < mark_alpha,
                "{}: track at {track_alpha} outweighs mark at {mark_alpha}",
                theme.id
            );
        }
    }
}
