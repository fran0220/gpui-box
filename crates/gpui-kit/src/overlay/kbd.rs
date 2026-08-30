//! Rendering a keyboard shortcut the way the platform writes it.

use gpui::{App, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};

use crate::foundation::{ActiveTheme, Ident, StyledExt};
use crate::strings::{ActiveStrings, StringKey, Strings};

/// A keyboard shortcut, written the way the current platform writes it.
///
/// macOS composes modifiers into one glyph run, while other platforms spell
/// them out and join with `+`, matching what users read elsewhere in their
/// system.
#[derive(Debug, Clone, IntoElement)]
pub struct Kbd {
    keystroke: SharedString,
    ident: Option<Ident>,
}

impl Kbd {
    /// Takes a GPUI keystroke such as `cmd-shift-p`.
    pub fn new(keystroke: impl Into<SharedString>) -> Self {
        Self {
            keystroke: keystroke.into(),
            ident: None,
        }
    }

    /// Publishes the shortcut, for hints a test needs to assert. A shortcut
    /// shown next to the action it belongs to is decorative and needs no id.
    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    pub fn caps(&self, cx: &App) -> Vec<SharedString> {
        caps(
            self.keystroke.as_ref(),
            cfg!(target_os = "macos"),
            cx.strings(),
        )
    }
}

impl RenderOnce for Kbd {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let published = self.ident.as_ref().map(|ident| {
            NodeSpec::new(ident.semantic_id(), Role::Text).text(self.keystroke.clone())
        });
        // A cap is sized from the same control step its chip is, rather than
        // from the caption scale, and a cap the symbol face has to draw is
        // sized a step above that. Those glyphs are drawn well inside their
        // em, so at caption size the mark a reader has to recognise came out
        // at around seven pixels and ⌫, ⌦ and ⇥ stopped being separable from
        // each other.
        let metrics = theme.control.get(gpui_kit_theme::ControlSize::Sm);
        let element =
            div()
                .row()
                .gap(px(theme.spacing.xs / 2.0))
                .children(self.caps(cx).into_iter().map(|cap| {
                    let size = if drawn_by_symbol_face(cap.as_ref()) {
                        theme.typography.subtitle.size
                    } else {
                        metrics.font_size
                    };
                    div()
                        .h(px(metrics.height))
                        .min_w(px(metrics.height))
                        .px(px(theme.spacing.xs))
                        .flex()
                        .items_center()
                        .justify_center()
                        .radius(&theme, gpui_kit_theme::Radius::Small)
                        .bg(theme.colors.hover)
                        .mono(&theme)
                        .font_fallbacks(gpui_kit_assets::key_fallbacks())
                        .text_size(px(size))
                        .text_color(theme.colors.text_muted)
                        .child(cap)
                }));
        match published {
            Some(spec) => element.semantic_in(cx, spec).into_any_element(),
            None => element.into_any_element(),
        }
    }
}

/// Whether a cap contains a glyph the bundled fallback face has to draw.
///
/// The arrows are not among them: the mono face draws those itself, at the
/// same size as the letters beside them.
fn drawn_by_symbol_face(cap: &str) -> bool {
    cap.chars()
        .any(|glyph| matches!(glyph, '⌘' | '⌃' | '⌥' | '⇧' | '⏎' | '⌫' | '⌦' | '⇥' | '␣'))
}

/// Splits a keystroke into the caps to draw.
///
/// Written as a free function so the platform choice can be tested on any host.
pub fn caps(keystroke: &str, macos: bool, strings: &Strings) -> Vec<SharedString> {
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
        let label = modifier_label(modifier, macos, strings);
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

/// The symbol forms are only ever reached under macOS, where they are what a
/// keyboard shortcut is expected to look like.
///
/// The Geist faces draw `⇧` and `⇥` but none of the others, so the asset crate
/// bundles a small fallback face for the remainder. Leaving that to whatever
/// font the host machine happened to install made this component's output
/// depend on the machine rather than on the caller's data.
fn modifier_label(modifier: &str, macos: bool, strings: &Strings) -> String {
    match (modifier, macos) {
        ("cmd" | "super" | "win", true) => "⌘".into(),
        ("cmd" | "super" | "win", false) => strings.text(StringKey::KbdSuper).to_string(),
        ("ctrl" | "control", true) => "⌃".into(),
        ("ctrl" | "control", false) => strings.text(StringKey::KbdControl).to_string(),
        ("alt" | "option", true) => "⌥".into(),
        ("alt" | "option", false) => strings.text(StringKey::KbdAlt).to_string(),
        ("shift", true) => "⇧".into(),
        ("shift", false) => strings.text(StringKey::KbdShift).to_string(),
        (other, _) => capitalize(other),
    }
}

fn key_label(key: &str, macos: bool) -> String {
    match (key, macos) {
        // U+23CE, not U+21A9: the bundled mono face draws the hooked arrow as
        // a shape that reads as something other than a return key.
        ("enter", true) => "⏎".into(),
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
        assert_eq!(
            caps("cmd-shift-p", true, &Strings::new()),
            vec![SharedString::from("⌘⇧P")]
        );
    }

    #[test]
    fn other_platforms_spell_each_modifier_out() {
        assert_eq!(
            caps("ctrl-shift-p", false, &Strings::new()),
            vec![
                SharedString::from("Ctrl"),
                SharedString::from("Shift"),
                SharedString::from("P")
            ]
        );
    }

    #[test]
    fn named_keys_use_their_symbols_where_the_platform_expects_them() {
        assert_eq!(
            caps("enter", true, &Strings::new()),
            vec![SharedString::from("⏎")]
        );
        assert_eq!(
            caps("enter", false, &Strings::new()),
            vec![SharedString::from("Enter")]
        );
        assert_eq!(
            caps("up", false, &Strings::new()),
            vec![SharedString::from("↑")]
        );
    }

    #[test]
    fn the_caps_that_need_the_symbol_face_are_the_ones_it_draws() {
        assert!(drawn_by_symbol_face("⌘⇧P"));
        assert!(drawn_by_symbol_face("⌫"));
        assert!(!drawn_by_symbol_face("esc"));
        assert!(
            !drawn_by_symbol_face("↑"),
            "the mono face draws the arrows itself"
        );
    }

    #[test]
    fn an_empty_keystroke_draws_nothing() {
        assert!(caps("", true, &Strings::new()).is_empty());
    }
}
