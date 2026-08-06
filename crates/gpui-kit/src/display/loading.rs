//! Loading indicators.
//!
//! Every animation here runs through GPUI's `with_animation`, which holds a
//! single static frame when the platform asks for reduced motion.

use gpui::{App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};
use gpui_kit_theme::ActiveTheme;

use crate::foundation::Ident;
use crate::motion::{self, AnimationExt as _, MotionSpec};

const PULSE_CELLS: usize = 5;
const MATRIX_SIDE: usize = 3;

/// A row of pulsing cells, used while a request is in flight.
#[derive(Debug, IntoElement)]
pub struct PulseLoader {
    ident: Ident,
    cell_size: f32,
}

impl PulseLoader {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            cell_size: 8.0,
        }
    }

    pub fn cell_size(mut self, cell_size: f32) -> Self {
        self.cell_size = cell_size;
        self
    }
}

impl RenderOnce for PulseLoader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = theme.colors.text;
        let cell_size = self.cell_size;
        let period = MotionSpec::new(
            theme.motion.pulse_ms,
            motion::CubicBezier::new(0.25, 0.1, 0.25, 1.0),
        );
        let ident = self.ident.clone();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(cell_size / 2.0))
            .children((0..PULSE_CELLS).map(move |index| {
                div()
                    .size(px(cell_size))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(px(cell_size))
                            .rounded(px(cell_size / 4.0))
                            .bg(color)
                            .with_animation(
                                ident.indexed_element_id(index),
                                period.repeating(),
                                move |element, delta| {
                                    let phase = motion::staggered_phase(delta, index, 0.0625);
                                    let wave = motion::pulse_wave(phase);
                                    element
                                        .opacity(0.08 + 0.92 * wave)
                                        .size(px(cell_size * (0.9 + 0.1 * wave)))
                                },
                            ),
                    )
            }))
    }
}

/// A three-by-three gradient matrix, used for longer indeterminate work.
#[derive(Debug, IntoElement)]
pub struct GradientSpinner {
    ident: Ident,
    cell_size: f32,
}

impl GradientSpinner {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            cell_size: 5.0,
        }
    }

    pub fn cell_size(mut self, cell_size: f32) -> Self {
        self.cell_size = cell_size;
        self
    }
}

impl RenderOnce for GradientSpinner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors.loader_gradient;
        let cell_size = self.cell_size;
        let period = MotionSpec::new(750, motion::CubicBezier::new(0.25, 0.1, 0.25, 1.0));
        let ident = self.ident.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(cell_size / 2.0))
            .children((0..MATRIX_SIDE).map(move |row| {
                let ident = ident.clone();
                div()
                    .flex()
                    .flex_row()
                    .gap(px(cell_size / 2.0))
                    .children((0..MATRIX_SIDE).map(move |column| {
                        let index = row * MATRIX_SIDE + column;
                        let center = (MATRIX_SIDE as f32 - 1.0) / 2.0;
                        let distance =
                            MATRIX_SIDE as f32 - 1.0 - row as f32 + (column as f32 - center).abs();
                        let phase = distance / (MATRIX_SIDE as f32 + center);
                        div()
                            .size(px(cell_size))
                            .rounded_full()
                            .bg(colors[row])
                            .with_animation(
                                ident.indexed_element_id(index),
                                period.repeating(),
                                move |element, delta| {
                                    element.opacity(motion::gradient_opacity(delta + phase, 0.1))
                                },
                            )
                    }))
            }))
    }
}

/// Placeholder rows shown while a list's real shape is unknown.
#[derive(Debug, IntoElement)]
pub struct Skeleton {
    ident: Ident,
    rows: usize,
    row_height: f32,
}

impl Skeleton {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            rows: 3,
            row_height: 28.0,
        }
    }

    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows;
        self
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = row_height;
        self
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = theme.colors.hover.opacity(0.28);
        let radius = theme.radii.control;
        let row_height = self.row_height;
        let period = MotionSpec::new(
            theme.motion.pulse_ms,
            motion::CubicBezier::new(0.25, 0.1, 0.25, 1.0),
        );
        let ident = self.ident.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .children((0..self.rows).map(move |index| {
                div()
                    .h(px(row_height))
                    .rounded(px(radius))
                    .bg(color)
                    .with_animation(
                        ident.indexed_element_id(index),
                        period.repeating(),
                        move |element, delta| {
                            let wave =
                                motion::pulse_wave(motion::staggered_phase(delta, index, 0.08));
                            element.opacity(0.35 + 0.4 * wave)
                        },
                    )
            }))
    }
}
