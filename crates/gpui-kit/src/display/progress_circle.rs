//! The ring form of [`crate::display::progress::ProgressBar`].
//!
//! Same state, same rules, same node: a position is published only when the
//! extent of the work is known, and an unknown extent is drawn as an unknown
//! extent rather than as a ring that happens to be a quarter full.

use std::f32::consts::{FRAC_PI_2, TAU};

use gpui::{
    AnimationExt as _, AnyElement, App, Hsla, IntoElement, ParentElement, PathBuilder, Pixels,
    Point, RenderOnce, SharedString, Styled, Window, canvas, div, point, px,
};
use gpui_kit_semantics::Semantic;
use gpui_kit_theme::{ActiveTheme, ControlSize, TypeScale};

use crate::display::progress::ProgressValue;
use crate::display::signature;
use crate::foundation::{Ident, Sizable, StyledExt};
use crate::motion::{self, MotionSpec};

/// How much larger the ring is than the control step it is sized from.
const RING_SCALE: f32 = 1.4;

/// A ring for work in a place too tight for a bar.
#[derive(Debug, IntoElement)]
pub struct ProgressCircle {
    ident: Ident,
    label: Option<SharedString>,
    /// What to show in the middle of the ring, when anything belongs there.
    centre: Option<SharedString>,
    value: ProgressValue,
    size: ControlSize,
}

impl ProgressCircle {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            centre: None,
            value: ProgressValue::default(),
            size: ControlSize::Md,
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
        let stroke = theme.borders.thick;
        let radius = (diameter - stroke) / 2.0;
        let track = theme.colors.track;
        let signature = signature::stops(&theme);
        let muted = signature[1].opacity(theme.opacity.muted);

        // The published position is the caller's number from the frame it
        // changes; only the arc takes its time getting there.
        let drawn = self.value.fraction.map(|fraction| {
            motion::tracked(
                &self.ident.semantic_id(),
                fraction,
                motion::resize(&theme),
                window,
                cx,
            )
        });

        // An unknown extent turns a short arc around the ring. A *still* part
        // of a ring would be read as a position, and there is none to read —
        // but a part that travels at a constant rate is the one shape nobody
        // reads as a position, because a position does not lap itself. Under
        // reduced motion there is no travel to rely on, so it falls back to
        // tinting the whole ring, which claims nothing either.
        let still = motion::reduce_motion(cx);
        let ring: AnyElement = if drawn.is_none() && !still {
            let period = MotionSpec::new(
                motion::Activity::Working.period_ms(&theme),
                motion::Activity::Working.curve(&theme),
            );
            div()
                .size(px(diameter))
                .with_animation(
                    self.ident.child("turn").element_id(),
                    period.repeating(),
                    move |element, phase| {
                        element.child(ring_canvas(
                            diameter,
                            radius,
                            stroke,
                            track,
                            signature,
                            muted,
                            None,
                            Some(phase),
                        ))
                    },
                )
                .into_any_element()
        } else {
            ring_canvas(
                diameter, radius, stroke, track, signature, muted, drawn, None,
            )
            .into_any_element()
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

        div()
            .flex_none()
            .relative()
            .size(px(diameter))
            .child(ring)
            .children(centre)
            .semantic_in(
                cx,
                self.value.spec(self.ident.semantic_id(), self.label, cx),
            )
    }
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
    signature: [Hsla; 3],
    muted: Hsla,
    drawn: Option<f32>,
    // Where the travelling arc has got to, or `None` for the still ring that
    // reduced motion falls back to.
    phase: Option<f32>,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let centre = bounds.center();
            arc(window, centre, radius, stroke, 0.0, 1.0, track);
            match (drawn, phase) {
                (Some(fraction), _) if fraction > 0.0 => {
                    signature_arc(window, centre, radius, stroke, 0.0, fraction, signature)
                }
                (Some(_), _) => {}
                (None, Some(phase)) => signature_arc(
                    window,
                    centre,
                    radius,
                    stroke,
                    phase,
                    phase + TRAVELLING_ARC,
                    signature,
                ),
                (None, None) => arc(window, centre, radius, stroke, 0.0, 1.0, muted),
            }
        },
    )
    .size(px(diameter))
}

/// Strokes a working arc in the three signature colours, so a ring and a bar
/// name the same work.
fn signature_arc(
    window: &mut Window,
    centre: Point<Pixels>,
    radius: f32,
    width: f32,
    from: f32,
    to: f32,
    signature: [Hsla; 3],
) {
    if to <= from {
        return;
    }
    let span = to - from;
    // Three segments keep the ring on the same three stops the bar uses.
    arc(
        window,
        centre,
        radius,
        width,
        from,
        from + span / 3.0,
        signature[0],
    );
    arc(
        window,
        centre,
        radius,
        width,
        from + span / 3.0,
        from + span * 2.0 / 3.0,
        signature[1],
    );
    arc(
        window,
        centre,
        radius,
        width,
        from + span * 2.0 / 3.0,
        to,
        signature[2],
    );
}

/// Strokes the part of a circle between two turns, clockwise from the top.
///
/// The arc is sampled rather than swept with an elliptical segment so a
/// partial ring and a full one are the same shape built the same way.
fn arc(
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
