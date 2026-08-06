//! Field chrome.
//!
//! One frame carries the border, background and focus treatment every editable
//! control wears. The editable surface itself arrives with `TextInput`.

use gpui::{Styled, div, px};
use gpui_kit_theme::{ControlSize, Radius, Space, Theme};

use crate::foundation::StyledExt;

/// What an editable surface currently reports about itself.
///
/// The chrome is drawn from this alone, so every field in the library says
/// the same thing with the same border.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FieldState {
    pub focused: bool,
    pub invalid: bool,
    pub disabled: bool,
}

impl FieldState {
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// The border, background and focus treatment every editable control wears.
///
/// `TextInput` renders inside it, and the composed fields — `NumberInput`,
/// `Combobox`, `TagInput` — wrap a bare input in one of these so a composed
/// control is not two nested frames.
pub fn field_shell(theme: &Theme, size: ControlSize, state: FieldState) -> gpui::Div {
    let metrics = theme.control.get(size);
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme.space(Space::Sm)))
        .min_h(px(metrics.height))
        .px(px(metrics.padding_x))
        .radius(theme, Radius::Control)
        .border(px(if state.focused {
            theme.borders.thick
        } else {
            theme.borders.hairline
        }))
        .border_color(if state.invalid {
            theme.colors.danger
        } else if state.focused {
            theme.colors.focus
        } else {
            theme.colors.hairline
        })
        .bg(theme
            .colors
            .hover
            .opacity(if state.disabled { 0.12 } else { 0.25 }))
        .text_size(px(metrics.font_size))
        .text_color(if state.disabled {
            theme.colors.text_faint
        } else {
            theme.colors.text
        })
}
