//! Rendering a keyboard shortcut the way the platform writes it.

use gpui::{App, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px};

use crate::foundation::{ActiveTheme, StyledExt};

/// A keyboard shortcut, written the way the current platform writes it.
///
/// macOS composes modifiers into one glyph run, while other platforms spell
/// them out and join with `+`, matching what users read elsewhere in their
/// system.
#[derive(Debug, Clone, IntoElement)]
pub struct Kbd {
    keystroke: SharedString,
}

impl Kbd {
    /// Takes a GPUI keystroke such as `cmd-shift-p`.
    pub fn new(keystroke: impl Into<SharedString>) -> Self {
        Self {
            keystroke: keystroke.into(),
        }
    }

    pub fn caps(&self) -> Vec<SharedString> {
        caps(self.keystroke.as_ref(), cfg!(target_os = "macos"))
    }
}

impl RenderOnce for Kbd {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .row()
            .gap(px(theme.spacing.xs / 2.0))
            .children(self.caps().into_iter().map(|cap| {
                div()
                    .h(px(theme
                        .control
                        .get(gpui_kit_theme::ControlSize::Sm)
                        .height))
                    .min_w(px(theme
                        .control
                        .get(gpui_kit_theme::ControlSize::Sm)
                        .height))
                    .px(px(theme.spacing.xs))
                    .flex()
                    .items_center()
                    .justify_center()
                    .radius(&theme, gpui_kit_theme::Radius::Small)
                    .bg(theme.colors.hover)
                    .font_family(theme.typography.mono.clone())
                    .text_size(px(theme.typography.caption.size))
                    .text_color(theme.colors.text_muted)
                    .child(cap)
            }))
    }
}

/// Splits a keystroke into the caps to draw.
///
/// Written as a free function so the platform choice can be tested on any host.
pub fn caps(keystroke: &str, macos: bool) -> Vec<SharedString> {
    let mut modifiers = String::new();
    let mut caps: Vec<SharedString> = Vec::new();
    let parts: Vec<&str> = keystroke
        .split('-')
        .filter(|part| !part.is_empty())
        .collect();
    let Some((key, modifier_parts)) = parts.split_last() else {
        return Vec::new();
    };

    for modifier in modifier_parts {
        let label = modifier_label(modifier, macos);
        if macos {
            modifiers.push_str(&label);
        } else {
            caps.push(label.into());
        }
    }
    let key = key_label(key, macos);
    if macos {
        modifiers.push_str(&key);
        vec![modifiers.into()]
    } else {
        caps.push(key.into());
        caps
    }
}

fn modifier_label(modifier: &str, macos: bool) -> String {
    match (modifier, macos) {
        ("cmd" | "super" | "win", true) => "⌘".into(),
        ("cmd" | "super" | "win", false) => "Win".into(),
        ("ctrl" | "control", true) => "⌃".into(),
        ("ctrl" | "control", false) => "Ctrl".into(),
        ("alt" | "option", true) => "⌥".into(),
        ("alt" | "option", false) => "Alt".into(),
        ("shift", true) => "⇧".into(),
        ("shift", false) => "Shift".into(),
        (other, _) => capitalize(other),
    }
}

fn key_label(key: &str, macos: bool) -> String {
    match (key, macos) {
        ("enter", true) => "↩".into(),
        ("escape", true) => "esc".into(),
        ("backspace", true) => "⌫".into(),
        ("delete", true) => "⌦".into(),
        ("tab", true) => "⇥".into(),
        ("up", _) => "↑".into(),
        ("down", _) => "↓".into(),
        ("left", _) => "←".into(),
        ("right", _) => "→".into(),
        ("space", _) => "␣".into(),
        (other, _) if other.chars().count() == 1 => other.to_uppercase(),
        (other, _) => capitalize(other),
    }
}

fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_composes_modifiers_into_one_cap() {
        assert_eq!(caps("cmd-shift-p", true), vec![SharedString::from("⌘⇧P")]);
    }

    #[test]
    fn other_platforms_spell_each_modifier_out() {
        assert_eq!(
            caps("ctrl-shift-p", false),
            vec![
                SharedString::from("Ctrl"),
                SharedString::from("Shift"),
                SharedString::from("P")
            ]
        );
    }

    #[test]
    fn named_keys_use_their_symbols_where_the_platform_expects_them() {
        assert_eq!(caps("enter", true), vec![SharedString::from("↩")]);
        assert_eq!(caps("enter", false), vec![SharedString::from("Enter")]);
        assert_eq!(caps("up", false), vec![SharedString::from("↑")]);
    }

    #[test]
    fn an_empty_keystroke_draws_nothing() {
        assert!(caps("", true).is_empty());
    }
}
