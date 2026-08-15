//! Loading indicators.
//!
//! Every animation here runs through GPUI's `with_animation`, which holds a
//! single static frame when the platform asks for reduced motion.

use gpui::{
    App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::ActiveTheme;

use crate::display::signature;
use crate::foundation::Ident;
use crate::motion::{self, AnimationExt as _, MotionSpec};

const PULSE_CELLS: usize = 5;
const MATRIX_SIDE: usize = 3;
/// How much of a placeholder row the moving highlight covers, and how far one
/// row's sweep trails the row above it.
const SHIMMER_BAND: f32 = 0.35;
const SHIMMER_ROW_OFFSET: f32 = 0.08;

/// A row of pulsing cells, used while a request is in flight.
#[derive(Debug, IntoElement)]
pub struct PulseLoader {
    ident: Ident,
    cell_size: f32,
    label: Option<gpui::SharedString>,
}

impl PulseLoader {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            cell_size: 8.0,
            label: None,
        }
    }

    /// What the wait is for. Announced with the busy state.
    pub fn label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn cell_size(mut self, cell_size: f32) -> Self {
        self.cell_size = cell_size;
        self
    }
}

impl RenderOnce for PulseLoader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = signature::stops(theme);
        let cell_size = self.cell_size;
        let period = MotionSpec::new(
            theme.motion.pulse_ms,
            motion::CubicBezier::new(0.25, 0.1, 0.25, 1.0),
        );
        let ident = self.ident.clone();
        let spec = busy_spec(&self.ident, self.label.clone());
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(cell_size / 2.0))
            .semantic_in(cx, spec)
            .children((0..PULSE_CELLS).map(move |index| {
                let color = colors[index % colors.len()];
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
    label: Option<gpui::SharedString>,
}

impl GradientSpinner {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            cell_size: 5.0,
            label: None,
        }
    }

    pub fn label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.label = Some(label.into());
        self
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
        let spec = busy_spec(&self.ident, self.label.clone());
        div()
            .flex()
            .flex_col()
            .gap(px(cell_size / 2.0))
            .semantic_in(cx, spec)
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
    label: Option<gpui::SharedString>,
}

impl Skeleton {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            rows: 3,
            row_height: 28.0,
            label: None,
        }
    }

    pub fn label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.label = Some(label.into());
        self
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
            theme.motion.shimmer_ms,
            motion::CubicBezier::new(0.25, 0.1, 0.25, 1.0),
        );
        let ident = self.ident.clone();
        let spec = busy_spec(&self.ident, self.label.clone());
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .semantic_in(cx, spec)
            .children((0..self.rows).map(move |index| {
                div()
                    .h(px(row_height))
                    .rounded(px(radius))
                    .bg(color)
                    .relative()
                    .overflow_hidden()
                    // A band of the working signature sweeping the row,
                    // rather than the whole row breathing: a sweep reads as
                    // work moving through the list. Under reduced motion the
                    // repeating animation holds delta zero, which parks the
                    // band off the leading edge and leaves a plain
                    // placeholder behind.
                    .child(
                        signature::shimmer_band(theme).with_animation(
                            ident.indexed_element_id(index),
                            period.repeating(),
                            move |element, delta| {
                                let phase =
                                    motion::staggered_phase(delta, index, SHIMMER_ROW_OFFSET);
                                element.left(relative(motion::shimmer_offset(
                                    phase,
                                    SHIMMER_BAND,
                                )))
                            },
                        ),
                    )
            }))
    }
}

/// Loading is a distinct state, so every indicator publishes it rather than
/// leaving a test to infer a wait from an absence of content.
fn busy_spec(ident: &Ident, label: Option<gpui::SharedString>) -> NodeSpec {
    let mut spec = NodeSpec::new(ident.semantic_id(), Role::Progress).busy(true);
    if let Some(label) = label {
        spec = spec.text(label);
    }
    spec
}
