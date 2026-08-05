use gpui::{AnyElement, SharedString, div, prelude::*, px};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_theme::Theme;

pub fn section_card(theme: &Theme) -> gpui::Div {
    div()
        .w_full()
        .rounded(px(theme.radii.card))
        .border_1()
        .border_color(theme.colors.hairline)
        .bg(theme.colors.panel)
        .overflow_hidden()
        .flex()
        .flex_col()
}

pub fn card_row(theme: &Theme, first: bool) -> gpui::Div {
    div()
        .w_full()
        .px(px(theme.spacing.lg + theme.spacing.xs))
        .py(px(theme.spacing.md + 2.0))
        .when(!first, |element| {
            element.border_t_1().border_color(theme.colors.hairline)
        })
        .hover(|style| style.bg(theme.colors.hover.opacity(0.3)))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme.spacing.md + 2.0))
}

pub fn identity_tile(theme: &Theme, glyph: Icon) -> gpui::Div {
    div()
        .flex_none()
        .size(px(36.0))
        .rounded(px(10.0))
        .border_1()
        .border_color(theme.colors.hairline)
        .bg(theme.colors.hover.opacity(0.25))
        .flex()
        .items_center()
        .justify_center()
        .child(
            icon(glyph)
                .size(px(16.0))
                .text_color(theme.colors.text_muted),
        )
}

pub fn row_title(theme: &Theme, title: impl Into<SharedString>) -> gpui::Div {
    div()
        .min_w_0()
        .truncate()
        .text_size(px(13.5))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.colors.text)
        .child(title.into())
}

pub fn row_detail(theme: &Theme, detail: impl Into<SharedString>) -> gpui::Div {
    div()
        .mt(px(theme.spacing.xs))
        .min_w_0()
        .text_size(px(11.5))
        .text_color(theme.colors.text_muted.opacity(0.68))
        .child(detail.into())
}

pub fn row_content(
    theme: &Theme,
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(row_title(theme, title))
        .child(row_detail(theme, detail))
}

pub fn row_with_trailing(
    theme: &Theme,
    first: bool,
    leading: AnyElement,
    content: AnyElement,
    trailing: AnyElement,
) -> gpui::Div {
    card_row(theme, first)
        .child(leading)
        .child(content)
        .child(trailing)
}
