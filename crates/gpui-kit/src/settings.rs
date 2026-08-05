use gpui::{SharedString, div, prelude::*, px};
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
