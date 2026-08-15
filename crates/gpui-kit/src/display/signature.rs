//! The working signature: three colours that appear only while work is in
//! flight.
//!
//! Accent stays the compact action and focus colour. These three stops are
//! something else: they name that work is happening, and they stay off idle
//! chrome. Surfaces that report a position or a wait paint from here so a
//! progress bar, a ring, a transport scrubber and a loader cannot invent
//! four different "working" colours.
//!
//! The signature never enters the semantic tree. A progress node still
//! publishes its fraction and its busy flag; a reader that asked what the
//! surface claimed is not told which colours it wore.

use gpui::{
    AnimationExt as _, AnyElement, App, Background, Div, Hsla, IntoElement, ParentElement, Styled,
    div, linear_color_stop, linear_gradient, relative,
};
use gpui_kit_theme::Theme;

use crate::motion::{self, Activity, MotionSpec};

#[cfg(test)]
use crate::motion::Interpolate;

/// How much of an unknown-extent track the travelling band covers.
const SWEEP_BAND: f32 = 0.42;

/// The three stops a working surface paints, in reading order.
pub(crate) fn stops(theme: &Theme) -> [Hsla; 3] {
    theme.colors.loader_gradient
}

/// A left-to-right wash across the three stops.
///
/// The renderer takes two colour stops per gradient, so the wash is two
/// halves rather than one three-stop block. Together they still read as
/// one signature.
pub(crate) fn wash(theme: &Theme) -> impl IntoElement {
    let [start, mid, end] = stops(theme);
    div()
        .absolute()
        .inset_0()
        .flex()
        .flex_row()
        .child(
            div()
                .h_full()
                .w(relative(0.5))
                .bg(linear_gradient(
                    90.0,
                    linear_color_stop(start, 0.0),
                    linear_color_stop(mid, 1.0),
                )),
        )
        .child(
            div()
                .h_full()
                .w(relative(0.5))
                .bg(linear_gradient(
                    90.0,
                    linear_color_stop(mid, 0.0),
                    linear_color_stop(end, 1.0),
                )),
        )
}

/// A colour along the signature, for a canvas that cannot take children.
#[cfg(test)]
pub(crate) fn sample(theme: &Theme, t: f32) -> Hsla {
    let [start, mid, end] = stops(theme);
    let t = t.clamp(0.0, 1.0);
    if t <= 0.5 {
        start.lerp(mid, t / 0.5)
    } else {
        mid.lerp(end, (t - 0.5) / 0.5)
    }
}

/// A determined fill: the signature clipped to `fraction` of the track.
pub(crate) fn determined(theme: &Theme, fraction: f32) -> Div {
    div()
        .absolute()
        .left_0()
        .top_0()
        .bottom_0()
        .rounded_full()
        .overflow_hidden()
        .w(relative(fraction.clamp(0.0, 1.0)))
        .child(wash(theme))
}

/// A band of the signature sweeping a track whose extent is unknown.
///
/// Under reduced motion the band sits still at the leading edge rather than
/// pretending to travel. A still sweep that covered the whole track would
/// read as finished work.
pub(crate) fn unknown(id: impl Into<gpui::ElementId>, theme: &Theme, cx: &App) -> AnyElement {
    let [start, mid, end] = stops(theme);
    let band = div()
        .absolute()
        .top_0()
        .bottom_0()
        .flex()
        .flex_row()
        .w(relative(SWEEP_BAND))
        .child(div().h_full().w(relative(1.0 / 3.0)).bg(linear_gradient(
            90.0,
            linear_color_stop(start.opacity(0.0), 0.0),
            linear_color_stop(start, 1.0),
        )))
        .child(
            div()
                .h_full()
                .w(relative(1.0 / 3.0))
                .bg(linear_gradient(
                    90.0,
                    linear_color_stop(start, 0.0),
                    linear_color_stop(mid, 1.0),
                )),
        )
        .child(div().h_full().w(relative(1.0 / 3.0)).bg(linear_gradient(
            90.0,
            linear_color_stop(end, 0.0),
            linear_color_stop(end.opacity(0.0), 1.0),
        )));

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

/// A shimmer highlight built from the signature, for a skeleton row.
pub(crate) fn shimmer_band(theme: &Theme) -> Div {
    let [start, mid, end] = stops(theme);
    let start = start.opacity(0.55);
    let mid = mid.opacity(0.7);
    let end = end.opacity(0.55);
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .flex()
        .flex_row()
        .w(relative(0.38))
        .child(div().h_full().w(relative(0.5)).bg(linear_gradient(
            90.0,
            linear_color_stop(start.opacity(0.0), 0.0),
            linear_color_stop(mid, 1.0),
        )))
        .child(div().h_full().w(relative(0.5)).bg(linear_gradient(
            90.0,
            linear_color_stop(mid, 0.0),
            linear_color_stop(end.opacity(0.0), 1.0),
        )))
}

/// Unused helper kept so a caller that needs the raw gradient pair can
/// build its own geometry without re-deriving the stops.
#[allow(dead_code)]
pub(crate) fn halves(theme: &Theme) -> [Background; 2] {
    let [start, mid, end] = stops(theme);
    [
        linear_gradient(
            90.0,
            linear_color_stop(start, 0.0),
            linear_color_stop(mid, 1.0),
        ),
        linear_gradient(
            90.0,
            linear_color_stop(mid, 0.0),
            linear_color_stop(end, 1.0),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit_theme::Theme;

    #[test]
    fn the_signature_is_three_distinct_stops() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            let [a, b, c] = stops(&theme);
            assert_ne!(a, b, "{}", theme.id);
            assert_ne!(b, c, "{}", theme.id);
            assert_eq!(sample(&theme, 0.0), a);
            assert_eq!(sample(&theme, 1.0), c);
            let middle = sample(&theme, 0.5);
            assert!((middle.h - b.h).abs() < 0.001);
            assert!((middle.s - b.s).abs() < 0.001);
            assert!((middle.l - b.l).abs() < 0.001);
        }
    }

    #[test]
    fn the_signature_is_not_the_accent() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            assert!(
                !stops(&theme).contains(&theme.colors.accent),
                "working colour collapsed onto accent in {}",
                theme.id
            );
        }
    }
}
