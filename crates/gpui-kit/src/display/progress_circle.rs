//! The ring form of [`crate::display::progress::ProgressBar`].
//!
//! Same state, same rules, same node: a position is published only when the
//! extent of the work is known, and an unknown extent is drawn as an unknown
//! extent rather than as a ring that happens to be a quarter full.

use std::f32::consts::{FRAC_PI_2, TAU};

use gpui::{
    App, Hsla, IntoElement, ParentElement, PathBuilder, Pixels, Point, RenderOnce, SharedString,
    Styled, Window, canvas, div, point, px,
};
use gpui_kit_semantics::Semantic;
use gpui_kit_theme::{ActiveTheme, ControlSize, TypeScale};

use crate::display::progress::ProgressValue;
use crate::foundation::{Ident, Sizable, StyledExt};
use crate::motion;

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
        let track = theme.colors.hairline_strong;
        let accent = theme.colors.accent;

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

        let ring = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let centre = bounds.center();
                arc(window, centre, radius, stroke, 0.0, 1.0, track);
                match drawn {
                    Some(fraction) if fraction > 0.0 => {
                        arc(window, centre, radius, stroke, 0.0, fraction, accent)
                    }
                    // An unknown extent tints the whole ring faintly. A part
                    // of the ring, moving or still, would be read as a
                    // position, and there is none to read.
                    None => arc(
                        window,
                        centre,
                        radius,
                        stroke,
                        0.0,
                        1.0,
                        accent.opacity(theme.opacity.muted),
                    ),
                    _ => {}
                }
            },
        )
        .size(px(diameter));

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
            .semantic_in(cx, self.value.spec(self.ident.semantic_id(), self.label))
    }
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
