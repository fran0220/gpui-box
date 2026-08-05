use std::sync::Arc;

use gpui::{ElementId, FontWeight, SharedString, Stateful, Window, div, prelude::*, px};
use gpui_kit_theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Ghost,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSize {
    Small,
    Medium,
}

pub fn action_button(
    id: impl Into<ElementId>,
    theme: &Theme,
    label: impl Into<SharedString>,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: bool,
    on_activate: impl Fn(&mut Window, &mut gpui::App) + 'static,
) -> Stateful<gpui::Div> {
    let mut button = button_frame(theme, variant, size, disabled)
        .id(id)
        .role(gpui::Role::Button)
        .when(!disabled, |element| {
            element.tab_index(0).focus(|style| {
                style
                    .border_1()
                    .border_color(theme.colors.focus)
                    .shadow(theme.selected_ring())
            })
        })
        .child(label.into());
    if !disabled {
        let on_activate = Arc::new(on_activate);
        let on_click = Arc::clone(&on_activate);
        button
            .interactivity()
            .on_click(move |_, window, cx| on_click(window, cx));
        button
            .interactivity()
            .on_key_down(move |event, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    on_activate(window, cx);
                    cx.stop_propagation();
                }
            });
    }
    button
}

pub fn button_frame(
    theme: &Theme,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: bool,
) -> gpui::Div {
    let (horizontal, vertical, text_size) = match size {
        ButtonSize::Small => (theme.spacing.sm, theme.spacing.xs, 12.0),
        ButtonSize::Medium => (theme.spacing.md, 6.0, 13.0),
    };
    let base = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .px(px(horizontal))
        .py(px(vertical))
        .rounded(px(theme.radii.control))
        .text_size(px(text_size))
        .font_weight(FontWeight::MEDIUM)
        .when(!disabled, |element| element.cursor_pointer())
        .when(disabled, |element| element.opacity(0.45));

    match variant {
        ButtonVariant::Primary => base
            .bg(theme.colors.text)
            .text_color(theme.colors.text_on_accent)
            .when(!disabled, |element| {
                element.hover(|style| style.opacity(0.9))
            }),
        ButtonVariant::Ghost => {
            base.text_color(theme.colors.text_muted)
                .when(!disabled, |element| {
                    element
                        .hover(|style| style.bg(theme.colors.hover).text_color(theme.colors.text))
                })
        }
        ButtonVariant::Danger => base
            .bg(theme.colors.danger.opacity(0.8))
            .text_color(gpui::white())
            .when(!disabled, |element| {
                element.hover(|style| style.opacity(0.9))
            }),
    }
}
