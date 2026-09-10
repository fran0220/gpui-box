//! Field chrome.
//!
//! One frame carries the surface, focus and invalid treatment every editable
//! control wears. The editable surface itself arrives with `TextInput`.

use gpui::{InteractiveElement, Styled, div, px};
use gpui_kit_theme::{ControlSize, Elevation, FieldFocus, Radius, SemanticWash, Space, Theme};

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
    field_chrome(div(), theme, state)
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme.space(Space::Sm)))
        .min_h(px(metrics.height))
        .px(px(metrics.padding_x))
        .radius(theme, Radius::Control)
        .text_size(px(metrics.font_size))
        .font_fallbacks(gpui_kit_assets::text_fallbacks())
        .text_color(if state.disabled {
            theme.colors.text_disabled
        } else {
            theme.colors.text
        })
}

/// Geometry-independent material shared by single-line shells and owned
/// multiline frames. Host-framed editors deliberately do not call this.
pub(crate) fn field_chrome<T: Styled + InteractiveElement>(
    field: T,
    theme: &Theme,
    state: FieldState,
) -> T {
    let fill = if state.invalid {
        theme
            .colors
            .control
            .blend(theme.color_wash(theme.colors.danger, SemanticWash::Faint))
    } else if state.focused && !state.disabled && theme.effects.field_focus == FieldFocus::Fill {
        theme.colors.control_hover
    } else {
        theme.colors.control
    };
    let mut shadows = theme.control_shadows(Elevation::Flat);
    if state.invalid {
        shadows.extend(theme.glow(theme.colors.danger));
    } else if state.focused && !state.disabled && theme.effects.field_focus == FieldFocus::Ring {
        shadows.extend(theme.focus_ring_on(fill));
    }
    let field = field
        .control_surface(theme, Elevation::Flat)
        .bg(fill)
        .shadow(shadows);
    if !state.disabled && !state.invalid {
        field.hover(|style| style.bg(theme.colors.control_hover))
    } else {
        field
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit_theme::Theme;

    #[test]
    fn field_focus_treatment_preserves_invalid_disabled_and_hairline() {
        for treatment in [FieldFocus::Ring, FieldFocus::Fill] {
            let mut tokens = gpui_kit_tokens::studio_light().clone();
            tokens.effect.field_focus = treatment;
            let theme = Theme::from_tokens(&tokens, Default::default());
            assert_eq!(theme.effects.field_focus, treatment);
            for focused in [false, true] {
                for invalid in [false, true] {
                    for disabled in [false, true] {
                        let state = FieldState {
                            focused,
                            invalid,
                            disabled,
                        };
                        let mut field = field_shell(&theme, ControlSize::Md, state);
                        let style = field.style();
                        let fill = if invalid {
                            theme
                                .colors
                                .control
                                .blend(theme.color_wash(theme.colors.danger, SemanticWash::Faint))
                        } else if focused && !disabled && treatment == FieldFocus::Fill {
                            theme.colors.control_hover
                        } else {
                            theme.colors.control
                        };
                        assert_eq!(
                            style.background,
                            Some(fill.into()),
                            "{treatment:?} {state:?}"
                        );
                        assert_eq!(style.border_color, Some(theme.colors.control_hairline));
                        let mut shadows = theme.control_shadows(Elevation::Flat);
                        if invalid {
                            shadows.extend(theme.glow(theme.colors.danger));
                        } else if focused && !disabled && treatment == FieldFocus::Ring {
                            shadows.extend(theme.focus_ring_on(fill));
                        }
                        assert_eq!(
                            style.box_shadow.as_deref(),
                            Some(shadows.as_slice()),
                            "{treatment:?} {state:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn interaction_halos_preserve_the_control_highlight() {
        let theme = Theme::studio_dark();
        for state in [
            FieldState::default(),
            FieldState::default().focused(true),
            FieldState::default().invalid(true),
            FieldState::default().focused(true).invalid(true),
        ] {
            let mut field = field_shell(&theme, ControlSize::Md, state);
            let shadows = field.style().box_shadow.as_ref().expect("control lighting");
            assert_eq!(
                shadows
                    .iter()
                    .filter(|s| s.style == gpui::ShadowStyle::Inset)
                    .count(),
                1
            );
            assert_eq!(shadows[0].color, theme.colors.control_highlight);
            assert_eq!(shadows.len() > 1, state.focused || state.invalid);
        }
    }

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
