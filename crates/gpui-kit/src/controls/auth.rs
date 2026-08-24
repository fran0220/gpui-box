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
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Surface, TypeScale};
use unicode_segmentation::UnicodeSegmentation;

use crate::controls::button::Button;
use crate::controls::field::{FieldState, field_shell};
use crate::controls::input::{TextInput, TextInputEvent};
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{Disableable, Ident, Sizable, StyledExt, text as foundation_text};
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
        Self {
            ident,
            field,
            reveal_focus: cx.focus_handle(),
            placeholder: None,
            name: None,
            initial: None,
            size: ControlSize::Md,
            disabled: false,
            invalid: false,
            required: false,
            read_only: false,
            revealed: false,
            seeded: false,
            configured: false,
            _subscriptions: vec![subscription],
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
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

    pub fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.seeded = true;
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
        let placeholder = self.placeholder.take();
        let name = self.name.take();
        let initial = self.initial.take().filter(|_| !self.seeded);
        self.seeded = true;
        let (disabled, invalid, required, read_only, size) = (
            self.disabled,
            self.invalid,
            self.required,
            self.read_only,
            self.size,
        );
        self.field.update(cx, move |field, cx| {
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
            .icon_only(Icon::Key, name)
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
        Self {
            ident,
            field,
            name: None,
            initial: None,
            slots: DEFAULT_CODE_SLOTS,
            size: ControlSize::Md,
            disabled: false,
            invalid: false,
            required: false,
            read_only: false,
            seeded: false,
            configured: false,
            _subscriptions: vec![subscription],
        }
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
        // A code is read as a run of separate places, so each place is drawn
        // as one: its own well, its own boundary, its own gap. Hairlines
        // inside a single bar say only that the bar has been divided, and
        // leave a typed slot looking exactly like an empty one.
        let slots = (0..self.slots).map(|index| {
            let selected = selected_start <= index && index < selected_end;
            let active = focused && selection.is_empty() && cursor == index;
            let filled = index < length;
            div()
                .flex_1()
                .h(px(metrics.height))
                .flex()
                .items_center()
                .justify_center()
                .radius(&theme, Radius::Control)
                .surface(&theme, Surface::Sunken)
                .border(px(theme.borders.hairline))
                .border_color(if self.invalid {
                    theme.colors.danger
                } else if active {
                    theme.colors.focus
                } else if filled {
                    theme.colors.hairline_strong
                } else {
                    theme.colors.hairline
                })
                .when(selected, |slot| slot.bg(theme.colors.selected))
                .when(active, |slot| slot.shadow(theme.focus_ring()))
                .when(self.disabled, |slot| slot.opacity(theme.opacity.disabled))
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
