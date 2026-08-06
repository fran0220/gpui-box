//! Product-shaped layout that belongs to an application, not to the library.
//!
//! These helpers used to live in `gpui-kit` as `settings` and `card` row
//! fragments. They encode one product's page rhythm, so they stay here as a
//! recipe the gallery renders and applications can copy.

use gpui::{SharedString, div, prelude::*, px};
use gpui_kit::assets::{Icon, icon};
use gpui_kit_theme::Theme;

pub fn page(theme: &Theme) -> gpui::Div {
    div()
        .w_full()
        .max_w(px(768.0))
        .mx_auto()
        .px(px(theme.spacing.xl))
        .pt(px(theme.spacing.xxl))
        .pb(px(64.0))
        .flex()
        .flex_col()
}

pub fn page_header(
    theme: &Theme,
    title: impl Into<SharedString>,
    count: Option<usize>,
) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(px(10.0))
        .child(
            div()
                .text_size(px(theme.typography.title.size))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.colors.text)
                .child(title.into()),
        )
        .when_some(count, |element, count| {
            element.child(
                div()
                    .text_size(px(theme.typography.body.size))
                    .text_color(theme.colors.text_muted.opacity(0.7))
                    .child(SharedString::from(count.to_string())),
            )
        })
}

pub fn subtitle(theme: &Theme, text: impl Into<SharedString>) -> gpui::Div {
    div()
        .mt(px(theme.spacing.xs))
        .text_size(px(theme.typography.body.size))
        .line_height(px(theme.typography.body.line_height))
        .text_color(theme.colors.text_muted)
        .child(text.into())
}

pub fn section_title(theme: &Theme, text: impl Into<SharedString>) -> gpui::Div {
    div()
        .mt(px(theme.spacing.xl))
        .mb(px(theme.spacing.sm))
        .text_size(px(theme.typography.label.size))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.colors.text_muted)
        .child(text.into())
}

pub fn footnote(theme: &Theme, text: impl Into<SharedString>) -> gpui::Div {
    div()
        .mt(px(theme.spacing.sm))
        .text_size(px(theme.typography.caption.size))
        .line_height(px(theme.typography.label.line_height))
        .text_color(theme.colors.text_faint)
        .child(text.into())
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
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(px(13.5))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.colors.text)
                .child(title.into()),
        )
        .child(
            div()
                .mt(px(theme.spacing.xs))
                .min_w_0()
                .text_size(px(11.5))
                .text_color(theme.colors.text_muted.opacity(0.68))
                .child(detail.into()),
        )
}
