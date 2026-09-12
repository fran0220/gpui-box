//! Sensitive text controls for product-neutral authentication composition.
//!
//! Both controls reuse [`TextInput`] as their only editor. They add visual
//! transient state and presentation, never account models, credential policy,
//! provider policy, or transport.

use std::ops::Range;

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Window, div,
    prelude::FluentBuilder as _, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TypeScale};
use unicode_segmentation::UnicodeSegmentation;

use crate::controls::button::Button;
use crate::controls::field::{FieldState, field_shell};
use crate::controls::input::{TextInput, TextInputEvent};
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{Disableable, Ident, Sizable, StyledExt, text as foundation_text};
use crate::reactive::Signal;
use crate::strings::{ActiveStrings, StringKey};

const DEFAULT_CODE_SLOTS: usize = 6;
const MIN_CODE_SLOTS: usize = 1;
const MAX_CODE_SLOTS: usize = 12;

/// What a password field reports to its owner.
#[derive(Clone, PartialEq, Eq)]
pub enum PasswordInputEvent {
    Change(SharedString),
    Submit,
    Cancel,
    BackspaceAtStart,
    Focus,
    Blur,
}

impl std::fmt::Debug for PasswordInputEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Change(_) => formatter
                .debug_tuple("Change")
                .field(&"[REDACTED]")
                .finish(),
            Self::Submit => formatter.write_str("Submit"),
            Self::Cancel => formatter.write_str("Cancel"),
            Self::BackspaceAtStart => formatter.write_str("BackspaceAtStart"),
            Self::Focus => formatter.write_str("Focus"),
            Self::Blur => formatter.write_str("Blur"),
        }
    }
}

impl EventEmitter<PasswordInputEvent> for PasswordInput {}
impl EventEmitter<gpui::TextInputAction> for PasswordInput {}

/// One sensitive password editor with a visual reveal action.
///
/// Revealing changes only the pixels. The value remains a secret for
/// deterministic semantics, AccessKit text runs and values, Debug, and
/// clipboard copy/cut.
pub struct PasswordInput {
    ident: Ident,
    field: Entity<TextInput>,
    reveal_focus: FocusHandle,
    placeholder: Option<SharedString>,
    name: Option<SharedString>,
    initial: Option<SharedString>,
    size: ControlSize,
    input_options: gpui::TextInputOptions,
    disabled: bool,
    invalid: bool,
    required: bool,
    read_only: bool,
    revealed: bool,
    seeded: bool,
    configured: bool,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for PasswordInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PasswordInput")
            .field("ident", &self.ident)
            .field("size", &self.size)
            .field("disabled", &self.disabled)
            .field("invalid", &self.invalid)
            .field("required", &self.required)
            .field("read_only", &self.read_only)
            .field("revealed", &self.revealed)
            .finish()
    }
}

impl PasswordInput {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let field = cx.new(|cx| {
            TextInput::new(ident.clone(), window, cx)
                .secret(true)
                .bare(true)
        });
        let subscription = cx.subscribe(&field, |_password, _field, event, cx| {
            let event = match event {
                TextInputEvent::Change(value) => PasswordInputEvent::Change(value.clone()),
                TextInputEvent::Submit => PasswordInputEvent::Submit,
                TextInputEvent::Cancel => PasswordInputEvent::Cancel,
                TextInputEvent::BackspaceAtStart => PasswordInputEvent::BackspaceAtStart,
                TextInputEvent::Focus => PasswordInputEvent::Focus,
                TextInputEvent::Blur => PasswordInputEvent::Blur,
            };
            cx.emit(event);
        });
        let actions = cx.subscribe(&field, |_, _, action: &gpui::TextInputAction, cx| {
            cx.emit(*action)
        });
        Self {
            ident,
            field,
            reveal_focus: cx.focus_handle(),
            placeholder: None,
            name: None,
            initial: None,
            size: ControlSize::Md,
            input_options: gpui::TextInputOptions::default(),
            disabled: false,
            invalid: false,
            required: false,
            read_only: false,
            revealed: false,
            seeded: false,
            configured: false,
            _subscriptions: vec![subscription, actions],
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Caller-selected keyboard and autofill hints; reveal never removes the
    /// editor's secure flag. Next/previous emit [`gpui::TextInputAction`].
    pub fn input_options(mut self, options: gpui::TextInputOptions) -> Self {
        self.input_options = options;
        self
    }

    /// Updates native hints while retaining the sensitive editing session.
    pub fn set_input_options(&mut self, options: gpui::TextInputOptions, cx: &mut Context<Self>) {
        self.input_options = options;
        self.field
            .update(cx, |field, cx| field.set_input_options(options, cx));
    }

    /// Names the one native password input when its visible label is outside
    /// this view.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Seeds the sensitive text without reporting a caller edit.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.initial = Some(text.into());
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn value(&self, cx: &App) -> SharedString {
        self.field.read(cx).value().clone()
    }

    pub fn is_revealed(&self) -> bool {
        self.revealed
    }

    pub fn selected_range(&self, cx: &App) -> Range<usize> {
        self.field.read(cx).selected_range()
    }

    /// `None` removes the name without replacing the sensitive editor.
    pub fn set_name(&mut self, name: Option<SharedString>, cx: &mut Context<Self>) {
        self.name = name.clone();
        self.field
            .update(cx, |field, cx| field.set_name(name.unwrap_or_default(), cx));
        cx.notify();
    }

    /// `None` restores the native empty placeholder.
    pub fn set_placeholder(&mut self, placeholder: Option<SharedString>, cx: &mut Context<Self>) {
        self.placeholder = placeholder.clone();
        self.field.update(cx, |field, cx| {
            field.set_placeholder(placeholder.unwrap_or_default(), cx)
        });
        cx.notify();
    }

    pub fn set_required(&mut self, required: bool, cx: &mut Context<Self>) {
        self.required = required;
        self.field
            .update(cx, |field, cx| field.set_required(required, cx));
        cx.notify();
    }

    pub fn set_control_size(&mut self, size: ControlSize, cx: &mut Context<Self>) {
        self.size = size;
        self.field
            .update(cx, |field, cx| field.set_control_size(size, cx));
        cx.notify();
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.seeded = true;
        self.field
            .update(cx, |field, cx| field.set_value(value, cx));
    }

    /// Keeps a password field and a caller-owned [`Signal`] holding the same
    /// text.
    ///
    /// The signal is the caller's storage for a credential, and it stays the
    /// caller's: nothing here writes it anywhere else, and a [`Signal`] does
    /// not print what it holds. Neither direction fires when the two already
    /// agree.
    ///
    /// The subscriptions are the binding: the caller holds them for as long
    /// as the field and the signal should stay together.
    #[must_use]
    pub fn bind(field: &Entity<Self>, signal: &Signal<String>, cx: &mut App) -> Vec<Subscription> {
        let seed = signal.get(cx);
        field.update(cx, |field, cx| field.set_value(seed, cx));

        let to_signal = {
            let signal = signal.clone();
            cx.subscribe(field, move |_field, event, cx| {
                if let PasswordInputEvent::Change(text) = event {
                    signal.set(cx, text.to_string());
                }
            })
        };
        let to_field = {
            let field = field.clone();
            cx.observe(signal.entity(), move |value, cx| {
                let text = value.read(cx).clone();
                field.update(cx, |field, cx| {
                    if field.value(cx).as_ref() != text.as_str() {
                        field.set_value(text, cx);
                    }
                });
            })
        };
        vec![to_signal, to_field]
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.field
            .update(cx, |field, cx| field.set_disabled(disabled, cx));
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        self.field
            .update(cx, |field, cx| field.set_read_only(read_only, cx));
        cx.notify();
    }

    pub fn set_invalid(&mut self, invalid: bool, cx: &mut Context<Self>) {
        self.invalid = invalid;
        self.field
            .update(cx, |field, cx| field.set_invalid(invalid, cx));
        cx.notify();
    }

    fn configure(&mut self, cx: &mut Context<Self>) {
        if self.configured {
            return;
        }
        self.configured = true;
        let placeholder = self.placeholder.take();
        let name = self.name.take();
        let initial = self.initial.take().filter(|_| !self.seeded);
        let input_options = self.input_options;
        self.seeded = true;
        let (disabled, invalid, required, read_only, size) = (
            self.disabled,
            self.invalid,
            self.required,
            self.read_only,
            self.size,
        );
        self.field.update(cx, move |field, cx| {
            field.set_input_options(input_options, cx);
            if let Some(placeholder) = placeholder {
                field.set_placeholder(placeholder, cx);
            }
            if let Some(name) = name {
                field.set_name(name, cx);
            }
            if let Some(initial) = initial {
                field.set_text_quietly(initial, cx);
            }
            field.set_disabled(disabled, cx);
            field.set_invalid(invalid, cx);
            field.set_required(required, cx);
            field.set_read_only(read_only, cx);
            field.set_control_size(size, cx);
        });
    }

    fn toggle_reveal(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.revealed = !self.revealed;
        let masked = !self.revealed;
        self.field
            .update(cx, |field, cx| field.set_visually_masked(masked, cx));
        cx.notify();
    }
}

impl Disableable for PasswordInput {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for PasswordInput {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for PasswordInput {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.read(cx).focus_handle(cx)
    }
}

impl Render for PasswordInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.configure(cx);
        let theme = cx.theme().clone();
        let focused = self.field.read(cx).focus_handle(cx).is_focused(window);
        let name = cx.strings().text(if self.revealed {
            StringKey::PasswordConceal
        } else {
            StringKey::PasswordReveal
        });
        let control = cx.entity().downgrade();
        let mut reveal = Button::new(self.ident.child("reveal"))
            .ghost()
            .when(self.size == ControlSize::Touch, |button| {
                button.label(name.clone())
            })
            .when(self.size != ControlSize::Touch, |button| {
                button.icon_only(Icon::Key, name)
            })
            .checked_state(self.revealed)
            .control_size(self.size)
            .semantic_parent(self.ident.semantic_id())
            .disabled(self.disabled);
        if !self.disabled {
            reveal = reveal
                .track_focus(&self.reveal_focus)
                .on_click(move |_, cx| {
                    control
                        .update(cx, |password, cx| password.toggle_reveal(cx))
                        .ok();
                });
        }

        field_shell(
            &theme,
            self.size,
            FieldState::default()
                .focused(focused)
                .invalid(self.invalid)
                .disabled(self.disabled),
        )
        .child(div().flex_1().min_w_0().child(self.field.clone()))
        .child(reveal)
    }
}

/// What a one-time code field reports to its owner.
#[derive(Clone, PartialEq, Eq)]
pub enum OneTimeCodeInputEvent {
    Change(SharedString),
    Submit,
}

impl std::fmt::Debug for OneTimeCodeInputEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Change(_) => formatter
                .debug_tuple("Change")
                .field(&"[REDACTED]")
                .finish(),
            Self::Submit => formatter.write_str("Submit"),
        }
    }
}

impl EventEmitter<OneTimeCodeInputEvent> for OneTimeCodeInput {}
impl EventEmitter<gpui::TextInputAction> for OneTimeCodeInput {}

/// One sensitive editor presented as a bounded run of visual slots.
///
/// A slot accepts one Unicode grapheme. The slots are not fields: one
/// `TextInput` owns the focus, selection, composition, paste, and native text
/// actions for the entire control.
pub struct OneTimeCodeInput {
    ident: Ident,
    field: Entity<TextInput>,
    name: Option<SharedString>,
    initial: Option<SharedString>,
    slots: usize,
    size: ControlSize,
    input_options: gpui::TextInputOptions,
    disabled: bool,
    invalid: bool,
    required: bool,
    read_only: bool,
    seeded: bool,
    configured: bool,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for OneTimeCodeInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OneTimeCodeInput")
            .field("ident", &self.ident)
            .field("slots", &self.slots)
            .field("disabled", &self.disabled)
            .field("invalid", &self.invalid)
            .field("required", &self.required)
            .field("read_only", &self.read_only)
            .finish()
    }
}

impl OneTimeCodeInput {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let field = cx.new(|cx| {
            TextInput::new(ident.clone(), window, cx)
                .secret(true)
                .bare(true)
        });
        let subscription = cx.subscribe(&field, |_code, _field, event, cx| match event {
            TextInputEvent::Change(value) => {
                cx.emit(OneTimeCodeInputEvent::Change(value.clone()));
            }
            TextInputEvent::Submit => cx.emit(OneTimeCodeInputEvent::Submit),
            _ => {}
        });
        let actions = cx.subscribe(&field, |_, _, action: &gpui::TextInputAction, cx| {
            cx.emit(*action)
        });
        Self {
            ident,
            field,
            name: None,
            initial: None,
            slots: DEFAULT_CODE_SLOTS,
            size: ControlSize::Md,
            input_options: gpui::TextInputOptions::default(),
            disabled: false,
            invalid: false,
            required: false,
            read_only: false,
            seeded: false,
            configured: false,
            _subscriptions: vec![subscription, actions],
        }
    }

    /// Requests keyboard/autofill hints. `OneTimeCode` is only an operating
    /// system hint: clipboard paste is not SMS autofill, and this control
    /// neither reads SMS nor guarantees suggestions. Alphanumeric codes remain
    /// valid; callers choose whether a numeric keyboard suits their code.
    pub fn input_options(mut self, options: gpui::TextInputOptions) -> Self {
        self.input_options = options;
        self
    }

    /// Updates hints without resetting code slots or the sensitive editor.
    pub fn set_input_options(&mut self, options: gpui::TextInputOptions, cx: &mut Context<Self>) {
        self.input_options = options;
        self.field
            .update(cx, |field, cx| field.set_input_options(options, cx));
    }

    /// Names the one native sensitive input when its visible label is outside
    /// this view.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Seeds the sensitive text without reporting a caller edit.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.initial = Some(text.into());
        self
    }

    /// Chooses the visual/code length. Values outside 1 through 12 use the
    /// nearest bound so the control remains legible and finite.
    pub fn slots(mut self, slots: usize) -> Self {
        self.slots = slots.clamp(MIN_CODE_SLOTS, MAX_CODE_SLOTS);
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn value(&self, cx: &App) -> SharedString {
        self.field.read(cx).value().clone()
    }

    pub fn len(&self, cx: &App) -> usize {
        self.field.read(cx).value().graphemes(true).count()
    }

    pub fn is_empty(&self, cx: &App) -> bool {
        self.field.read(cx).is_empty()
    }

    pub fn is_complete(&self, cx: &App) -> bool {
        self.len(cx) == self.slots
    }

    pub fn slot_count(&self) -> usize {
        self.slots
    }

    /// `None` removes the accessible name without replacing the sensitive editor.
    pub fn set_name(&mut self, name: Option<SharedString>, cx: &mut Context<Self>) {
        self.name = name.clone();
        self.field
            .update(cx, |field, cx| field.set_name(name.unwrap_or_default(), cx));
        cx.notify();
    }

    /// Changes the visual slots and future input limit. Existing text is retained.
    pub fn set_slots(&mut self, slots: usize, cx: &mut Context<Self>) {
        self.slots = slots.clamp(MIN_CODE_SLOTS, MAX_CODE_SLOTS);
        self.field
            .update(cx, |field, cx| field.set_sensitive_slots(self.slots, cx));
        cx.notify();
    }

    pub fn set_required(&mut self, required: bool, cx: &mut Context<Self>) {
        self.required = required;
        self.field
            .update(cx, |field, cx| field.set_required(required, cx));
        cx.notify();
    }

    pub fn set_control_size(&mut self, size: ControlSize, cx: &mut Context<Self>) {
        self.size = size;
        self.field
            .update(cx, |field, cx| field.set_control_size(size, cx));
        cx.notify();
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.seeded = true;
        let value = value.into();
        let value = value.graphemes(true).take(self.slots).collect::<String>();
        self.field
            .update(cx, |field, cx| field.set_value(value, cx));
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.field
            .update(cx, |field, cx| field.set_disabled(disabled, cx));
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        self.field
            .update(cx, |field, cx| field.set_read_only(read_only, cx));
        cx.notify();
    }

    pub fn set_invalid(&mut self, invalid: bool, cx: &mut Context<Self>) {
        self.invalid = invalid;
        self.field
            .update(cx, |field, cx| field.set_invalid(invalid, cx));
        cx.notify();
    }

    fn configure(&mut self, cx: &mut Context<Self>) {
        if self.configured {
            return;
        }
        self.configured = true;
        let name = self.name.take();
        let initial = self
            .initial
            .take()
            .filter(|_| !self.seeded)
            .map(|value| value.graphemes(true).take(self.slots).collect::<String>());
        let input_options = self.input_options;
        self.seeded = true;
        let (slots, disabled, invalid, required, read_only, size) = (
            self.slots,
            self.disabled,
            self.invalid,
            self.required,
            self.read_only,
            self.size,
        );
        self.field.update(cx, move |field, cx| {
            field.set_sensitive_slots(slots, cx);
            field.set_input_options(input_options, cx);
            if let Some(name) = name {
                field.set_name(name, cx);
            }
            if let Some(initial) = initial {
                field.set_text_quietly(initial, cx);
            }
            field.set_disabled(disabled, cx);
            field.set_invalid(invalid, cx);
            field.set_required(required, cx);
            field.set_read_only(read_only, cx);
            field.set_control_size(size, cx);
        });
    }
}

impl Disableable for OneTimeCodeInput {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for OneTimeCodeInput {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for OneTimeCodeInput {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.read(cx).focus_handle(cx)
    }
}

impl Render for OneTimeCodeInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.configure(cx);
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let (focused, value, selection, cursor) = {
            let field = self.field.read(cx);
            (
                field.focus_handle(cx).is_focused(window),
                field.value().clone(),
                field.selected_range(),
                field.cursor_offset(),
            )
        };

        let length = value.graphemes(true).count();
        let selected_start = value[..selection.start].graphemes(true).count();
        let selected_end = value[..selection.end].graphemes(true).count();
        let cursor = value[..cursor].graphemes(true).count();
        let direction = cx.layout_direction();
        self.field
            .update(cx, |field, _| field.reset_slot_bounds(self.slots));
        // A code is read as a run of separate places, so each place is drawn
        // as one: its own well, its own boundary, its own gap. Hairlines
        // inside a single bar say only that the bar has been divided, and
        // leave a typed slot looking exactly like an empty one.
        let slots = (0..self.slots).map(|index| {
            let field = self.field.clone();
            let selected = selected_start <= index && index < selected_end;
            let active = focused && selection.is_empty() && cursor == index;
            let filled = index < length;
            // A slot is a field of the same family as the ones above it in a
            // form, so it is built from the same chrome rather than from a
            // second description of what a field looks like.
            field_shell(
                &theme,
                self.size,
                FieldState::default()
                    .focused(active)
                    .invalid(self.invalid)
                    .disabled(self.disabled),
            )
            .relative()
            .w_auto()
            .flex_1()
            .px_0()
            .justify_center()
            // Each narrow slot is its own tonal well. The active one receives
            // the shared focus halo from `field_shell`, so the next character
            // has one soft location marker rather than an outlined box.
            .when(selected, |slot| slot.bg(theme.colors.selected))
            .child(
                gpui::canvas(
                    move |bounds, window, _| (bounds, window.visual_transform()),
                    move |_, (bounds, transform), _, cx| {
                        field.update(cx, |field, _| {
                            field.set_slot_bounds(index, bounds, transform)
                        });
                    },
                )
                .absolute()
                .inset_0(),
            )
            .child(
                foundation_text(&theme, TypeScale::Label, if filled { "•" } else { "" })
                    .text_size(px(metrics.font_size))
                    .text_color(theme.colors.text),
            )
        });

        div()
            .relative()
            .w_full()
            .child(
                div()
                    .row_reading(direction)
                    .w_full()
                    .gap_token(&theme, Space::Xs)
                    .children(slots),
            )
            // The one editor occupies exactly the segmented surface. It paints
            // nothing in slot mode, but owns input, hit testing, IME bounds,
            // and the one semantic/native node for the control.
            .child(div().absolute().inset_0().child(self.field.clone()))
    }
}

#[cfg(test)]
mod native_geometry_tests {
    use super::*;
    use gpui::{EntityInputHandler, NativeTextPosition, TextAffinity, TextNavigationDirection};

    #[gpui::test]
    fn auth_visual_transform_uses_each_slots_own_snapshot(cx: &mut gpui::TestAppContext) {
        use crate::controls::input::TestVisualScale;
        use std::{
            cell::{Cell, RefCell},
            rc::Rc,
        };
        let scaled = Rc::new(Cell::new(false));
        let scale = scaled.clone();
        let slot = Rc::new(RefCell::new(None));
        let build = slot.clone();
        let mut harness =
            gpui_kit_testkit::harness::Harness::new(cx, crate::install, move |window, cx| {
                let code = build
                    .borrow_mut()
                    .get_or_insert_with(|| {
                        cx.new(|cx| {
                            OneTimeCodeInput::new("scaled.code", window, cx)
                                .slots(4)
                                .text("a🦀e\u{301}z")
                        })
                    })
                    .clone();
                TestVisualScale {
                    enabled: scale.get(),
                    child: div()
                        .p(px(40.0))
                        .w(px(320.0))
                        .child(code)
                        .into_any_element(),
                }
                .into_any_element()
            });
        harness.frame();
        let code = slot.borrow().clone().expect("code mounted");
        let field = harness.update(|_, cx| code.read(cx).field.clone());
        let logical = harness.update(|window, cx| {
            field.update(cx, |field, cx| {
                field.selection_rects_for_range(0..6, window, cx)
            })
        });
        assert_eq!(logical.len(), 4);
        for enabled in [true, false] {
            scaled.set(enabled);
            harness.frame();
            harness.update(|window, cx| {
                field.update(cx, |field, cx| {
                    let rects = field.selection_rects_for_range(0..6, window, cx);
                    for (actual, original) in rects.iter().zip(&logical) {
                        let expected = if enabled {
                            gpui::Bounds::new(
                                gpui::point(
                                    original.bounds.left() * 1.5 + px(7.5),
                                    original.bounds.top() * 1.5 - px(20.5),
                                ),
                                original.bounds.size.map(|value| value * 1.5),
                            )
                        } else {
                            original.bounds
                        };
                        assert_eq!(actual.bounds, expected);
                    }
                    assert_eq!(
                        field.character_index_for_point(rects[2].bounds.origin, window, cx),
                        Some(3)
                    );
                    assert_eq!(
                        field
                            .native_position_for_point(
                                rects[2].bounds.origin,
                                Some(3..5),
                                window,
                                cx
                            )
                            .expect("slot point")
                            .utf16_offset,
                        3
                    );
                    let position = NativeTextPosition {
                        utf16_offset: 3,
                        ..Default::default()
                    };
                    assert_eq!(
                        field
                            .native_position_bounds(position, window, cx)
                            .expect("slot caret")
                            .origin,
                        rects[2].bounds.origin
                    );
                    // A well can have a different scale/origin than the hidden
                    // editor. Publishing that snapshot must not use the editor T.
                    let bounds = logical[2].bounds;
                    field.set_slot_bounds(
                        2,
                        bounds,
                        gpui::VisualTransform::scale_about(0.5, gpui::point(px(31.0), px(-19.0))),
                    );
                    let expected = gpui::Bounds::new(
                        gpui::point(bounds.left() * 0.5 + px(15.5), bounds.top() * 0.5 - px(9.5)),
                        bounds.size.map(|value| value * 0.5),
                    );
                    assert_eq!(
                        field.selection_rects_for_range(3..5, window, cx)[0].bounds,
                        expected
                    );
                    let caret = field
                        .native_position_bounds(position, window, cx)
                        .expect("independent slot caret");
                    assert_eq!(caret.origin, expected.origin);
                    assert_eq!(caret.size.height, expected.size.height);
                    assert_eq!(caret.size.width, px(cx.theme().measures.caret_width) * 0.5);
                    assert_eq!(
                        field
                            .native_position_for_point(expected.origin, Some(3..5), window, cx)
                            .expect("independent slot point")
                            .utf16_offset,
                        3
                    );
                })
            });
        }
    }

    #[gpui::test]
    fn native_otp_fragments_follow_measured_wells_not_uniform_editor_slices(
        cx: &mut gpui::TestAppContext,
    ) {
        let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
        let build = slot.clone();
        let mut harness =
            gpui_kit_testkit::harness::Harness::new(cx, crate::install, move |window, cx| {
                let code = build
                    .borrow_mut()
                    .get_or_insert_with(|| {
                        cx.new(|cx| {
                            OneTimeCodeInput::new("native.code", window, cx)
                                .slots(4)
                                .text("a🦀e\u{301}z")
                                .control_size(ControlSize::Touch)
                        })
                    })
                    .clone();
                div().w(px(300.0)).child(code).into_any_element()
            });
        harness.frame();
        let code = slot.borrow().clone().expect("mounted OTP");
        harness.update(|window, cx| {
            let field = code.read(cx).field.clone();
            field.update(cx, |field, cx| {
                let rects = field.selection_rects_for_range(0..6, window, cx);
                assert_eq!(rects.len(), 4);
                for pair in rects.windows(2) {
                    assert!(
                        pair[0].bounds.right() < pair[1].bounds.left(),
                        "selection fragments exclude the real gaps"
                    );
                }
                assert!(rects[0].bounds.size.width >= px(48.0));
                assert!(
                    rects[0].bounds.size.width < px(75.0),
                    "not one quarter of the300px editor"
                );
                let before = NativeTextPosition {
                    utf16_offset: 3,
                    affinity: TextAffinity::Upstream,
                };
                let after = NativeTextPosition {
                    affinity: TextAffinity::Downstream,
                    ..before
                };
                assert_eq!(
                    field
                        .native_position_bounds(before, window, cx)
                        .expect("trailing slot caret")
                        .left(),
                    rects[1].bounds.right()
                );
                assert_eq!(
                    field
                        .native_position_bounds(after, window, cx)
                        .expect("leading slot caret")
                        .left(),
                    rects[2].bounds.left()
                );
                assert_eq!(
                    field
                        .native_position_in_direction(
                            after,
                            TextNavigationDirection::Right,
                            1,
                            window,
                            cx
                        )
                        .expect("next slot")
                        .utf16_offset,
                    5
                );
                let selected = field
                    .native_position_for_point(
                        rects[3].bounds.bottom_right(),
                        Some(1..3),
                        window,
                        cx,
                    )
                    .expect("constrained slot position");
                assert_eq!(selected.utf16_offset, 3);
                assert!(
                    field
                        .native_position_for_point(rects[0].bounds.origin, Some(2..3), window, cx)
                        .is_none()
                );
            });
        });
    }
}
