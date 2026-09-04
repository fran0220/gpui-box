//! A field that captures the next keystroke instead of acting on it.
//!
//! # The syntax it produces
//!
//! The recorder reports GPUI's own keystroke syntax, not a private one: it
//! hands back [`gpui::Keystroke::unparse`], which is exactly what
//! [`gpui::Keystroke::parse`] reads. A captured binding therefore goes
//! straight into a keymap, and straight into [`Kbd`], which splits the same
//! string. The shape is `[fn-][ctrl-][alt-][cmd-|super-|win-][shift-]key`,
//! where `key` is the lowercase name of the key — `cmd-shift-p`,
//! `ctrl-alt-delete`, `f5`.
//!
//! # Escape
//!
//! Escape ends recording without capturing. That is the honest limit: escape
//! is how every other surface in this library abandons something in flight,
//! and a recorder that swallowed it would leave the typist with no way out of
//! a field that eats every key. The cost is that **escape cannot be bound**
//! through the default recorder. A caller that genuinely needs it turns
//! [`KeybindingRecorder::allow_escape`] on and provides its own way out.
//!
//! # What it does not do
//!
//! - A modifier on its own is not a keystroke. The recorder keeps waiting
//!   rather than reporting `shift` as a binding.
//! - A captured binding is **reported**, never applied. The caller owns the
//!   keymap, as it owns every other value in this library.
//! - A conflict is the host's judgement. The recorder has no keymap to consult
//!   and never guesses; [`KeybindingRecorder::conflict`] renders the reason the
//!   host found, and nothing else.

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, TypeScale};

use crate::foundation::{
    Disableable, FocusRing, Ident, Sizable, StyledExt, text as foundation_text,
};
use crate::motion;
use crate::overlay::Kbd;
use crate::strings::{ActiveStrings, StringKey};

/// The key context the recorder claims while it is capturing, so a host can
/// see in the inspector which element is eating its keys.
const KEY_CONTEXT: &str = "KeybindingRecorder";

/// What a [`KeybindingRecorder`] reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeybindingRecorderEvent {
    /// Recording began. Every keystroke now goes to the recorder.
    Started,
    /// A keystroke was captured, in GPUI's syntax. Nothing has been bound.
    Captured(SharedString),
    /// Recording ended without a keystroke.
    Cancelled,
}

impl EventEmitter<KeybindingRecorderEvent> for KeybindingRecorder {}

/// A field that records one keystroke.
///
/// Whether recording is in flight is transient view state, so this is a view
/// rather than a builder. The binding itself is not: the recorder reports the
/// keystroke that was pressed and renders whatever the caller says is bound,
/// so a host that refuses a binding keeps showing the one that still holds.
pub struct KeybindingRecorder {
    ident: Ident,
    focus_handle: FocusHandle,
    label: Option<SharedString>,
    placeholder: Option<SharedString>,
    binding: Option<SharedString>,
    conflict: Option<SharedString>,
    allow_escape: bool,
    size: ControlSize,
    disabled: bool,
    recording: bool,
}

impl std::fmt::Debug for KeybindingRecorder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KeybindingRecorder")
            .field("ident", &self.ident)
            .field("binding", &self.binding)
            .field("recording", &self.recording)
            .field("conflict", &self.conflict)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl KeybindingRecorder {
    pub fn new(ident: impl Into<Ident>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            label: None,
            placeholder: None,
            binding: None,
            conflict: None,
            allow_escape: false,
            size: ControlSize::Md,
            disabled: false,
            recording: false,
        }
    }

    /// What the binding is for, for a reader that has only the tree.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// What to show where a binding would be, when there is none.
    /// The placeholder the host gave, or the built-in default word for a
    /// value that is not there.
    fn resolved_placeholder(&self, cx: &App) -> SharedString {
        self.placeholder
            .clone()
            .unwrap_or_else(|| cx.strings().text(StringKey::KeybindingUnbound))
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// The binding the caller says is current, in GPUI's syntax.
    pub fn binding(mut self, binding: impl Into<SharedString>) -> Self {
        self.binding = Some(binding.into());
        self
    }

    pub fn set_binding(&mut self, binding: Option<SharedString>, cx: &mut Context<Self>) {
        self.binding = binding;
        cx.notify();
    }

    /// The conflict the **host** found, in its own words.
    ///
    /// The recorder has no keymap and never decides that two bindings clash.
    pub fn conflict(mut self, reason: Option<impl Into<SharedString>>) -> Self {
        self.conflict = reason.map(Into::into);
        self
    }

    pub fn set_conflict(&mut self, reason: Option<SharedString>, cx: &mut Context<Self>) {
        self.conflict = reason;
        cx.notify();
    }

    /// Lets escape be captured as a binding rather than ending recording.
    ///
    /// A caller that turns this on owes the typist another way out, because
    /// the usual one is now a keystroke the recorder eats.
    pub fn allow_escape(mut self, allow: bool) -> Self {
        self.allow_escape = allow;
        self
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn current_binding(&self) -> Option<&SharedString> {
        self.binding.as_ref()
    }

    /// Begins recording and takes the keyboard.
    pub fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.recording {
            return;
        }
        self.recording = true;
        window.focus(&self.focus_handle, cx);
        cx.emit(KeybindingRecorderEvent::Started);
        cx.notify();
    }

    /// Ends recording without capturing anything.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if !self.recording {
            return;
        }
        self.recording = false;
        cx.emit(KeybindingRecorderEvent::Cancelled);
        cx.notify();
    }

    fn capture(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if !self.recording {
            return false;
        }
        let key = event.keystroke.key.as_str();
        if key == "escape" && !self.allow_escape {
            self.recording = false;
            cx.emit(KeybindingRecorderEvent::Cancelled);
            cx.notify();
            return true;
        }
        // A modifier on its own is a hand resting on the keyboard, not a
        // binding, so the recorder keeps waiting for the key it modifies.
        if is_modifier(key) {
            return true;
        }
        self.recording = false;
        cx.emit(KeybindingRecorderEvent::Captured(SharedString::from(
            event.keystroke.unparse(),
        )));
        cx.notify();
        true
    }
}

impl Disableable for KeybindingRecorder {
    /// Refuses the recorder. A refused recorder installs no handler at all.
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for KeybindingRecorder {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for KeybindingRecorder {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Whether a key name is a modifier rather than a key.
///
/// GPUI names a bare modifier `shift`, `control`, `alt`, `platform`, or
/// `function` when it parses one; platforms deliver the more familiar spelling
/// of the same keys, so both are refused.
pub fn is_modifier(key: &str) -> bool {
    matches!(
        key,
        "shift"
            | "control"
            | "ctrl"
            | "alt"
            | "option"
            | "cmd"
            | "command"
            | "super"
            | "win"
            | "platform"
            | "function"
            | "fn"
    )
}

impl Render for KeybindingRecorder {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let recording = self.recording && !self.disabled;
        let conflicted = self.conflict.is_some();
        let actionable = !self.disabled;

        // Recording has to be unmistakable: a recorder that looks like a text
        // field invites someone to type into it and lose whatever they typed.
        let body = if recording {
            let caret = div()
                .flex_none()
                .w(px(theme.measures.caret_width))
                .h(px(metrics.font_size))
                .bg(theme.colors.accent);
            div()
                .row()
                .gap_token(&theme, Space::Xs)
                .text_color(theme.colors.accent)
                .child(
                    icon(Icon::Keyboard)
                        .size(px(metrics.icon_size))
                        .text_color(theme.colors.accent),
                )
                .child(motion::breathe(
                    caret,
                    self.ident.child("recording.caret").element_id(),
                    &theme,
                    cx,
                ))
                .into_any_element()
        } else {
            match self.binding.clone() {
                Some(binding) => div()
                    .row()
                    .gap_token(&theme, Space::Xs)
                    .child(Kbd::new(binding).id(self.ident.child("keys")))
                    .into_any_element(),
                // Nothing bound is an absence, so it takes the tone this
                // library spends on absences rather than the one it spends on
                // values.
                None => div()
                    .row()
                    .gap_token(&theme, Space::Xs)
                    .child(
                        icon(Icon::Keyboard)
                            .size(px(metrics.icon_size))
                            .text_color(theme.colors.text_faint),
                    )
                    .child(
                        foundation_text(&theme, TypeScale::Label, self.resolved_placeholder(cx))
                            .text_size(px(metrics.font_size))
                            .text_tone(&theme, gpui_kit_theme::TextTone::Placeholder),
                    )
                    .into_any_element(),
            }
        };

        let mut field = div()
            .id(self.ident.element_id())
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .row()
            .flex_none()
            .h(px(metrics.height))
            // Wide enough that a chord has room and no wider: a field sized to
            // whatever column it landed in leaves a one-key binding adrift in
            // an empty box.
            .min_w(px(metrics.height * 5.0))
            .px(px(metrics.padding_x))
            .gap(px(metrics.gap))
            .items_center()
            // What the field holds is one short thing whatever state it is
            // in, and the field is wider than any of them. Left at the
            // leading edge, a two-key chord sat against one end of an
            // otherwise empty box while the words of the other states filled
            // it, so the four states of one control did not line up.
            .justify_center()
            .radius(&theme, Radius::Control)
            .surface(&theme, gpui_kit_theme::Surface::Sunken)
            .when(recording, |field| {
                field
                    .bg(theme.color_wash(theme.colors.accent, gpui_kit_theme::SemanticWash::Faint))
                    .shadow(theme.glow(theme.colors.accent))
            })
            .when(conflicted, |field| {
                field
                    .bg(theme.color_wash(theme.colors.danger, gpui_kit_theme::SemanticWash::Faint))
                    .shadow(theme.glow(theme.colors.danger))
            })
            .text_size(px(metrics.font_size))
            .text_color(theme.colors.text)
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .hover(|style| style.bg(theme.colors.hover))
            })
            // The accent wash and glow are already the recording mark; adding
            // a focus halo would repeat the same state and muddy the surface.
            .when(actionable && !recording, |element| {
                element.focus_ring(&theme)
            })
            .child(body);

        if actionable {
            field = field
                .on_click(cx.listener(|recorder, _, window, cx| recorder.start(window, cx)))
                .on_key_down(cx.listener(|recorder, event: &KeyDownEvent, _, cx| {
                    // While recording, the keystroke belongs to the recorder
                    // and to nothing else on the way up.
                    if recorder.capture(event, cx) {
                        cx.stop_propagation();
                        return;
                    }
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        recorder.recording = true;
                        cx.emit(KeybindingRecorderEvent::Started);
                        cx.notify();
                        cx.stop_propagation();
                    }
                }));
        }

        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Input)
            .focus(&self.focus_handle)
            .disabled(self.disabled)
            .busy(recording)
            .invalid(conflicted)
            .placeholder(self.resolved_placeholder(cx));
        if let Some(label) = self.label.clone() {
            spec = spec.text(label);
        }
        // A recorder in flight says so where its value would be: the value is
        // what the caller bound, and nothing is bound while it is listening.
        if recording {
            spec = spec
                .value("recording")
                .description(cx.strings().text(StringKey::KeybindingPrompt));
        } else if let Some(binding) = self.binding.clone() {
            spec = spec.value(binding);
        }
        let published = field.semantic_in(cx, spec);

        let conflict = self.conflict.clone().map(|reason| {
            let ident = self.ident.child("conflict");
            div()
                .row()
                .gap_token(&theme, Space::Xs)
                .child(
                    icon(Icon::Danger)
                        .size(px(theme.control.get(ControlSize::Xs).icon_size))
                        .text_color(theme.colors.danger),
                )
                .child(
                    foundation_text(&theme, TypeScale::Caption, reason.clone())
                        .text_color(theme.colors.danger),
                )
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .invalid(true)
                        .text(reason),
                )
        });

        div()
            .column()
            .gap_token(&theme, Space::Xs)
            .child(published)
            .children(conflict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Keystroke;

    /// The one property that matters: whatever the recorder reports, GPUI
    /// reads back as the keystroke that produced it.
    fn round_trips(source: &str) {
        let keystroke = Keystroke::parse(source).expect("gpui parses its own syntax");
        let reported = keystroke.unparse();
        let read_back = Keystroke::parse(&reported).expect("gpui reads what the recorder reports");
        assert_eq!(read_back.modifiers, keystroke.modifiers, "for {source}");
        assert_eq!(read_back.key, keystroke.key, "for {source}");
    }

    #[test]
    fn what_the_recorder_reports_is_what_gpui_parses() {
        for source in [
            "cmd-shift-p",
            "ctrl-alt-delete",
            "f5",
            "shift-tab",
            "alt-enter",
            "ctrl-,",
            "P",
        ] {
            round_trips(source);
        }
    }

    #[test]
    fn a_capital_letter_is_reported_as_shift_and_a_lowercase_key() {
        let keystroke = Keystroke::parse("P").expect("parses");
        assert_eq!(keystroke.key, "p");
        assert!(keystroke.modifiers.shift);
        assert_eq!(keystroke.unparse(), "shift-p");
    }

    #[test]
    fn what_the_recorder_reports_is_what_kbd_draws() {
        let keystroke = Keystroke::parse("cmd-shift-p").expect("parses");
        let caps =
            crate::overlay::caps(&keystroke.unparse(), true, &crate::strings::Strings::new());
        assert_eq!(caps, vec![SharedString::from("⌘⇧P")]);
    }

    #[test]
    fn a_bare_modifier_is_not_a_keystroke() {
        for key in ["shift", "control", "alt", "platform", "function"] {
            assert!(is_modifier(key), "{key} is a modifier");
            // GPUI itself parses a lone modifier into exactly these names.
            assert_eq!(Keystroke::parse(key).expect("parses").key, key);
        }
        assert!(!is_modifier("p"));
        assert!(!is_modifier("escape"));
    }
}
