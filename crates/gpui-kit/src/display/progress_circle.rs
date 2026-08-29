//! The ring form of [`crate::display::progress::ProgressBar`].
//!
//! Same state, same rules, same node: a position is published only when the
//! extent of the work is known, and an unknown extent is drawn as an unknown
//! extent rather than as a ring that happens to be a quarter full.

use std::f32::consts::{FRAC_PI_2, TAU};
use std::rc::Rc;

use gpui::{
    AnimationExt as _, AnyElement, App, Hsla, IntoElement, ParentElement, PathBuilder, Pixels,
    Point, RenderOnce, SharedString, Styled, Window, canvas, div, point, px,
};
use gpui_kit_semantics::Semantic;
use gpui_kit_theme::{ActiveTheme, ControlSize, TypeScale};

use crate::controls::button::Button;
use crate::display::progress::{ProgressPace, ProgressValue};
use crate::display::signature;
use crate::foundation::{Ident, Sizable, StyledExt};
use crate::motion::{self, MotionPolicy, MotionRole};
use crate::strings::{ActiveStrings, StringKey};

type CancelHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// How much larger the ring is than the control step it is sized from.
const RING_SCALE: f32 = 1.4;

/// How thick the ring is, as a share of its diameter.
///
/// A border width does not scale: at `border.thick` the largest ring in the
/// size ramp was the same hairline as the smallest, and an arc one twelfth of
/// the way across a hairline groove cannot be told from the groove.
const RING_STROKE: f32 = 0.11;

/// A ring for work in a place too tight for a bar.
#[derive(IntoElement)]
pub struct ProgressCircle {
    ident: Ident,
    label: Option<SharedString>,
    /// What to show in the middle of the ring, when anything belongs there.
    centre: Option<SharedString>,
    value: ProgressValue,
    size: ControlSize,
    on_cancel: Option<CancelHandler>,
}

impl std::fmt::Debug for ProgressCircle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProgressCircle")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("value", &self.value)
            .finish()
    }
}

impl ProgressCircle {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            centre: None,
            value: ProgressValue::default(),
            size: ControlSize::Md,
            on_cancel: None,
        }
    }

    /// What the work is, for a reader who has only the tree to go on.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// How much of the work is done, between zero and one.
    pub fn fraction(mut self, fraction: f32) -> Self {
        self.value.set_fraction(fraction);
        self
    }

    /// Reports `done` out of `total`, and stays indeterminate when the total
    /// is zero, because no fraction exists to report.
    pub fn count(mut self, done: usize, total: usize) -> Self {
        self.value.set_count(done, total);
        self
    }

    /// What the node publishes as its value, such as `"3 of 12"`.
    pub fn display(mut self, display: impl Into<SharedString>) -> Self {
        self.value.display = Some(display.into());
        self
    }

    /// A short reading inside the ring. It is the caller's words: the circle
    /// invents no percentage of its own.
    pub fn centre(mut self, centre: impl Into<SharedString>) -> Self {
        self.centre = Some(centre.into());
        self
    }

    pub fn stalled(mut self, stalled: bool) -> Self {
        if stalled {
            self.value.pace = ProgressPace::Stalled;
        }
        self
    }

    pub fn paused(mut self, paused: bool) -> Self {
        if paused {
            self.value.pace = ProgressPace::Paused;
        }
        self
    }

    pub fn on_cancel(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }
}

impl Sizable for ProgressCircle {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for ProgressCircle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let diameter = (metrics.height * RING_SCALE).round();
        let stroke = ring_stroke(&theme, diameter);
        let radius = (diameter - stroke) / 2.0;
        let track = signature::track(&theme);
        // The pace is part of the picture, not only of the caption: a
        // stalled ring wears the warning colour and a paused one dims, so
        // neither can pass for healthy running work at a glance.
        let mark = match self.value.pace {
            ProgressPace::Running => signature::mark(&theme),
            ProgressPace::Stalled => theme.colors.warning,
            ProgressPace::Paused => signature::mark(&theme).opacity(theme.opacity.muted),
        };
        // The published position is the caller's number from the frame it
        // changes; only the arc takes its time getting there.
        let drawn = self.value.fraction.map(|fraction| {
            motion::tracked(
                &self.ident.semantic_id(),
                fraction,
                MotionPolicy::spec(MotionRole::Resize, &theme),
                window,
                cx,
            )
        });

        // An unknown extent turns a short arc around the ring. A *still* part
        // of a ring would be read as a position, and there is none to read —
        // but a part that travels at a constant rate is the one shape nobody
        // reads as a position, because a position does not lap itself. Under
        // reduced motion there is no travel to rely on, so it falls back to
        // the same short arc parked at the top. A fully tinted ring looked
        // like work at ninety-something percent, which invented the position
        // this branch exists to avoid.
        let activity = MotionPolicy::resolve(MotionRole::Activity(motion::Activity::Working), cx);
        let still = !activity.animates() || !self.value.is_moving();
        let ring: AnyElement = if drawn.is_none() && !still {
            div()
                .size(px(diameter))
                .with_animation(
                    self.ident.child("turn").element_id(),
                    activity.spec().repeating(),
                    move |element, phase| {
                        element.child(ring_canvas(
                            diameter,
                            radius,
                            stroke,
                            track,
                            mark,
                            None,
                            Some(phase),
                        ))
                    },
                )
                .into_any_element()
        } else {
            ring_canvas(diameter, radius, stroke, track, mark, drawn, None).into_any_element()
        };

        let centre = self.centre.clone().map(|reading| {
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .type_scale(&theme, TypeScale::Caption)
                .text_color(theme.colors.text_muted)
                .child(reading)
        });
        let cancel = self.on_cancel.map(|handler| {
            Button::new(self.ident.child("cancel"))
                .label(cx.strings().text(StringKey::ProgressCancel))
                .ghost()
                .control_size(ControlSize::Xs)
                .semantic_parent(self.ident.semantic_id())
                .on_click(move |window, cx| handler(window, cx))
        });

        div()
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .flex_none()
                    .relative()
                    .size(px(diameter))
                    .child(ring)
                    .children(centre),
            )
            .children(cancel)
            .semantic_in(
                cx,
                self.value.spec(self.ident.semantic_id(), self.label, cx),
            )
    }
}

/// How thick a ring of this diameter is drawn, never thinner than a border.
fn ring_stroke(theme: &gpui_kit_theme::Theme, diameter: f32) -> f32 {
    (diameter * RING_STROKE).round().max(theme.borders.thick)
}

/// How much of the ring the travelling arc covers when the extent is unknown.
///
/// Short enough that the gap is unmistakable — a nearly closed ring would read
/// as work nearly done — and long enough to be seen moving.
const TRAVELLING_ARC: f32 = 0.25;

/// The ring itself, at one phase of its travel.
///
/// Built per frame rather than once, because the arc's position is what
/// carries "still going" and this renderer's transforms do not reach a canvas.
#[allow(clippy::too_many_arguments)]
fn ring_canvas(
    diameter: f32,
    radius: f32,
    stroke: f32,
    track: Hsla,
    mark: Hsla,
    drawn: Option<f32>,
    // Where the travelling arc has got to, or `None` for its reduced-motion
    // poster at the top of the ring.
    phase: Option<f32>,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let centre = bounds.center();
            arc(window, centre, radius, stroke, 0.0, 1.0, track);
            match (drawn, phase) {
                (Some(fraction), _) if fraction > 0.0 => {
                    arc(window, centre, radius, stroke, 0.0, fraction, mark)
                }
                (Some(_), _) => {}
                (None, Some(phase)) => arc(
                    window,
                    centre,
                    radius,
                    stroke,
                    phase,
                    phase + TRAVELLING_ARC,
                    mark,
                ),
                (None, None) => arc(
                    window,
                    centre,
                    radius,
                    stroke,
                    -TRAVELLING_ARC / 2.0,
                    TRAVELLING_ARC / 2.0,
                    mark,
                ),
            }
        },
    )
    .size(px(diameter))
}

/// Strokes the part of a circle between two turns, clockwise from the top.
///
/// The arc is sampled rather than swept with an elliptical segment so a
/// partial ring and a full one are the same shape built the same way.
/// Shared with the loading family, so a spinner's ring and a progress ring
/// are one shape at two sizes.
pub(crate) fn arc(
    window: &mut Window,
    centre: Point<Pixels>,
    radius: f32,
    width: f32,
    from: f32,
    to: f32,
    color: Hsla,
) {
    if radius <= 0.0 || to <= from {
        return;
    }
    let steps = (((to - from) * 96.0).ceil() as usize).max(2);
    let at = |turn: f32| {
        let angle = turn * TAU - FRAC_PI_2;
        point(
            centre.x + px(radius * angle.cos()),
            centre.y + px(radius * angle.sin()),
        )
    };

    let mut builder = PathBuilder::stroke(px(width));
    builder.move_to(at(from));
    for step in 1..=steps {
        builder.line_to(at(from + (to - from) * step as f32 / steps as f32));
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit_theme::Theme;

    /// A ring reports its position by the difference between the arc and the
    /// groove behind it. At a fixed border width the whole size ramp drew the
    /// same hairline, and a quarter turn of a hairline is not a reading.
    #[test]
    fn the_ring_thickens_with_the_size_it_is_drawn_at() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            let mut previous = 0.0_f32;
            for size in [
                ControlSize::Xs,
                ControlSize::Sm,
                ControlSize::Md,
                ControlSize::Lg,
            ] {
                let diameter = (theme.control.get(size).height * RING_SCALE).round();
                let stroke = ring_stroke(&theme, diameter);
                assert!(
                    stroke >= theme.borders.thick,
                    "{}: a {size:?} ring strokes at {stroke}, under the border at {}",
                    theme.id,
                    theme.borders.thick
                );
                assert!(
                    stroke >= previous,
                    "{}: a {size:?} ring at {stroke} is thinner than the step below it \
                     at {previous}",
                    theme.id
                );
                previous = stroke;
            }
            assert!(
                previous > theme.borders.thick,
                "{}: the largest ring never grew past a border at {previous}",
                theme.id
            );
        }
    }
}
