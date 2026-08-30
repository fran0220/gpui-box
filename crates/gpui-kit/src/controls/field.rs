//! Field chrome.
//!
//! One frame carries the surface, focus and invalid treatment every editable
//! control wears. The editable surface itself arrives with `TextInput`.

use gpui::{Styled, div, prelude::FluentBuilder, px};
use gpui_kit_theme::{ControlSize, Radius, Space, Theme};

use crate::foundation::StyledExt;

/// What an editable surface currently reports about itself.
///
/// The chrome is drawn from this alone, so every field in the library says
/// the same thing the same way.
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

/// The size an adornment control takes when it sits *inside* a field.
///
/// A stepper built at the field's own size fills the field edge to edge, so it
/// reads as a block wedged into the field's rounded end rather than as a
/// control the field contains. One step down leaves the field's own corners
/// and borders visible around it.
pub fn nested_control_size(size: ControlSize) -> ControlSize {
    match size {
        ControlSize::Lg => ControlSize::Md,
        ControlSize::Md => ControlSize::Sm,
        ControlSize::Sm | ControlSize::Xs => ControlSize::Xs,
    }
}

/// The surface, focus and invalid treatment every editable control wears.
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
        .well(theme)
        // The recess is the whole resting treatment: a theme meeting the
        // canvas-to-sunken floor has already said where the field is, and a
        // line added on top of a step that works is the outline this library
        // spends everywhere else. Invalidity colours the space the well is
        // already holding, so becoming invalid reflows nothing, and focus
        // stays the ring every other focusable thing in the library wears.
        .when(state.invalid, |field| {
            field.border_color(theme.colors.danger)
        })
        .when(state.focused, |field| field.shadow(theme.focus_ring()))
        .text_size(px(metrics.font_size))
        .font_fallbacks(gpui_kit_assets::text_fallbacks())
        .text_color(if state.disabled {
            theme.colors.text_disabled
        } else {
            theme.colors.text
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit_theme::Theme;

    /// A stepper has to be shorter than the field around it, or the field's
    /// own corners and border are behind it rather than around it.
    #[test]
    fn an_adornment_is_shorter_than_the_field_that_holds_it() {
        let theme = Theme::default();
        for size in ControlSize::ALL {
            let nested = nested_control_size(size);
            assert!(
                theme.control.get(nested).height <= theme.control.get(size).height,
                "{size:?} holds {nested:?}"
            );
        }
        assert!(
            theme
                .control
                .get(nested_control_size(ControlSize::Md))
                .height
                < theme.control.get(ControlSize::Md).height
        );
    }

    /// The smallest control has nothing under it, so it holds its own size
    /// rather than resolving to one the token scale does not have.
    #[test]
    fn the_smallest_field_still_resolves_to_a_size() {
        assert_eq!(nested_control_size(ControlSize::Xs), ControlSize::Xs);
    }
}
