use gpui::{IntoElement, ParentElement, Styled, div, px};
use gpui_kit_theme::Theme;

use crate::motion::{self, AnimationExt as _, MotionSpec};

const CELLS: usize = 5;
const MATRIX_SIDE: usize = 3;

pub fn pulse_loader(id: &'static str, theme: &Theme, cell_size: f32) -> impl IntoElement {
    let color = theme.colors.text;
    let period = MotionSpec::new(
        theme.motion.pulse_ms,
        motion::CubicBezier::new(0.25, 0.1, 0.25, 1.0),
    );
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(cell_size / 2.0))
        .children((0..CELLS).map(move |index| {
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
                        .with_animation((id, index), period.repeating(), move |element, delta| {
                            let phase = motion::staggered_phase(delta, index, 0.0625);
                            let wave = motion::pulse_wave(phase);
                            element
                                .opacity(0.08 + 0.92 * wave)
                                .size(px(cell_size * (0.9 + 0.1 * wave)))
                        }),
                )
        }))
}

pub fn gradient_spinner(id: &'static str, theme: &Theme, cell_size: f32) -> impl IntoElement {
    let colors = theme.colors.loader_gradient;
    let period = MotionSpec::new(750, motion::CubicBezier::new(0.25, 0.1, 0.25, 1.0));
    div()
        .flex()
        .flex_col()
        .gap(px(cell_size / 2.0))
        .children((0..MATRIX_SIDE).map(move |row| {
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
                        .with_animation((id, index), period.repeating(), move |element, delta| {
                            element.opacity(motion::gradient_opacity(delta + phase, 0.1))
                        })
                }))
        }))
}

pub fn skeleton_rows(id: &'static str, theme: &Theme, count: usize) -> impl IntoElement {
    let color = theme.colors.hover.opacity(0.28);
    let radius = theme.radii.control;
    let period = MotionSpec::new(
        theme.motion.pulse_ms,
        motion::CubicBezier::new(0.25, 0.1, 0.25, 1.0),
    );
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .children((0..count).map(move |index| {
            div()
                .h(px(28.0))
                .rounded(px(radius))
                .bg(color)
                .with_animation((id, index), period.repeating(), move |element, delta| {
                    let wave = motion::pulse_wave(motion::staggered_phase(delta, index, 0.08));
                    element.opacity(0.35 + 0.4 * wave)
                })
        }))
}
