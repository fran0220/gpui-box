//! Field chrome.
//!
//! These frames carry the border, background and focus treatment shared by
//! editable controls. The editable surface itself arrives with `TextInput`.

use gpui::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space};

use crate::foundation::{Sizable, StyledExt};

/// A bordered container for an editable control.
#[derive(IntoElement)]
pub struct FieldFrame {
    content: AnyElement,
    size: ControlSize,
    invalid: bool,
}

impl std::fmt::Debug for FieldFrame {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FieldFrame")
            .field("size", &self.size)
            .field("invalid", &self.invalid)
            .finish()
    }
}

impl FieldFrame {
    pub fn new(content: impl IntoElement) -> Self {
        Self {
            content: content.into_any_element(),
            size: ControlSize::Md,
            invalid: false,
        }
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }
}

impl Sizable for FieldFrame {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for FieldFrame {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        div()
            .w_full()
            .px(px(metrics.padding_x))
            .py_token(&theme, Space::Sm)
            .radius(&theme, Radius::Control)
            .border(px(theme.borders.hairline))
            .border_color(if self.invalid {
                theme.colors.danger
            } else {
                theme.colors.hairline
            })
            .bg(theme.colors.hover.opacity(0.25))
            .text_size(px(metrics.font_size))
            .text_color(theme.colors.text)
            .child(self.content)
    }
}

/// The flatter frame used for filter and search inputs inside popovers.
#[derive(IntoElement)]
pub struct SearchFrame {
    content: AnyElement,
}

impl std::fmt::Debug for SearchFrame {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("SearchFrame").finish()
    }
}

impl SearchFrame {
    pub fn new(content: impl IntoElement) -> Self {
        Self {
            content: content.into_any_element(),
        }
    }
}

impl RenderOnce for SearchFrame {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .w_full()
            .mb(px(theme.spacing.xs))
            .px(px(10.0))
            .py(px(6.0))
            .radius(&theme, Radius::Control)
            .bg(theme.colors.hover.opacity(0.28))
            .text_size(px(theme.typography.body.size))
            .text_color(theme.colors.text)
            .child(self.content)
    }
}
