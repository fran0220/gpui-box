use gpui::{AnyElement, div, prelude::*, px};
use gpui_kit_theme::Theme;

pub fn field(theme: &Theme, input: AnyElement) -> gpui::Div {
    div()
        .w_full()
        .px(px(theme.spacing.md))
        .py(px(theme.spacing.sm))
        .rounded(px(theme.radii.control))
        .border_1()
        .border_color(theme.colors.hairline)
        .bg(theme.colors.hover.opacity(0.25))
        .text_size(px(theme.typography.body.size))
        .text_color(theme.colors.text)
        .child(input)
}

pub fn search_frame(theme: &Theme, input: AnyElement) -> gpui::Div {
    div()
        .w_full()
        .mb(px(theme.spacing.xs))
        .px(px(10.0))
        .py(px(6.0))
        .rounded(px(theme.radii.control))
        .bg(theme.colors.hover.opacity(0.28))
        .text_size(px(theme.typography.body.size))
        .text_color(theme.colors.text)
        .child(input)
}
