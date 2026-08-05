use gpui::{SharedString, div, prelude::*, px};
use gpui_kit_theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeTone {
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
    Info,
}

pub fn badge(theme: &Theme, label: impl Into<SharedString>, tone: BadgeTone) -> gpui::Div {
    let (foreground, background, border) = match tone {
        BadgeTone::Neutral => (
            theme.colors.text_muted,
            theme.colors.raised,
            theme.colors.hairline,
        ),
        BadgeTone::Accent => (
            theme.colors.accent,
            theme.colors.accent.opacity(0.12),
            theme.colors.accent.opacity(0.2),
        ),
        BadgeTone::Success => (
            theme.colors.success,
            theme.colors.success.opacity(0.12),
            theme.colors.success.opacity(0.2),
        ),
        BadgeTone::Warning => (
            theme.colors.warning,
            theme.colors.warning.opacity(0.12),
            theme.colors.warning.opacity(0.2),
        ),
        BadgeTone::Danger => (
            theme.colors.danger,
            theme.colors.danger.opacity(0.12),
            theme.colors.danger.opacity(0.2),
        ),
        BadgeTone::Info => (
            theme.colors.info,
            theme.colors.info.opacity(0.12),
            theme.colors.info.opacity(0.2),
        ),
    };

    div()
        .flex_none()
        .px(px(theme.spacing.sm))
        .py(px(2.0))
        .rounded_full()
        .border_1()
        .border_color(border)
        .bg(background)
        .text_size(px(theme.typography.caption.size))
        .text_color(foreground)
        .child(label.into())
}
