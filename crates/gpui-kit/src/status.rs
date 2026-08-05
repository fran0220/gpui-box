use gpui::{SharedString, div, prelude::*, px};
use gpui_kit_theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusTone {
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
    Info,
}

fn color(theme: &Theme, tone: StatusTone) -> gpui::Hsla {
    match tone {
        StatusTone::Neutral => theme.colors.text_faint,
        StatusTone::Accent => theme.colors.accent,
        StatusTone::Success => theme.colors.success,
        StatusTone::Warning => theme.colors.warning,
        StatusTone::Danger => theme.colors.danger,
        StatusTone::Info => theme.colors.info,
    }
}

pub fn status_dot(theme: &Theme, tone: StatusTone) -> gpui::Div {
    div().size(px(7.0)).rounded_full().bg(color(theme, tone))
}

pub fn status_line(theme: &Theme, label: impl Into<SharedString>, tone: StatusTone) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme.spacing.sm))
        .text_size(px(theme.typography.label.size))
        .text_color(theme.colors.text_muted)
        .child(status_dot(theme, tone))
        .child(label.into())
}

pub fn callout(theme: &Theme, message: impl Into<SharedString>, tone: StatusTone) -> gpui::Div {
    let color = color(theme, tone);
    div()
        .w_full()
        .px(px(theme.spacing.lg))
        .py(px(theme.spacing.md))
        .rounded(px(theme.radii.card))
        .border_1()
        .border_color(color.opacity(0.2))
        .bg(color.opacity(0.06))
        .text_size(px(theme.typography.label.size))
        .line_height(px(theme.typography.body.line_height))
        .text_color(color.opacity(0.92))
        .flex()
        .flex_row()
        .items_start()
        .gap(px(theme.spacing.sm))
        .child(div().mt(px(5.0)).child(status_dot(theme, tone)))
        .child(div().min_w_0().child(message.into()))
}
