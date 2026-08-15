//! A date field with a calendar hanging off it.
//!
//! Typing goes through [`DateAdapter::parse_day`](crate::datetime::DateAdapter::parse_day).
//! Text the adapter will not read stays exactly where the typist left it, the
//! field publishes itself invalid, and the adapter's own message is shown word
//! for word — the same rule `NumberInput` keeps, for the same reason: silently
//! rewriting what somebody typed hides the disagreement instead of reporting
//! it.

use gpui::{
    App, AppContext as _, Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, SharedString, Styled,
    Subscription, Window, div, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TypeScale};

use crate::controls::button::IconButton;
use crate::controls::field::{FieldState, field_shell};
use crate::controls::input::{TextInput, TextInputEvent};
use crate::datetime::adapter::{Day, SharedDateAdapter};
use crate::datetime::calendar::{Calendar, CalendarEvent};
use crate::foundation::{Disableable, Ident, Sizable, StyledExt, text as foundation_text};
use crate::overlay::popover;
use crate::strings::{ActiveStrings, StringKey};

/// What a date field reports. The owner decides what any of it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateInputEvent {
    /// A day the adapter read, by typing or from the calendar.
    Changed(Day),
    /// The adapter would not read what is in the field. Both the text as it
    /// was typed and the adapter's message travel with it.
    Unparsable {
        text: SharedString,
        message: SharedString,
    },
    Opened,
    Closed,
    Submit,
}

impl EventEmitter<DateInputEvent> for DateInput {}

/// A text field over a host-owned calendar, with that calendar in a popover.
pub struct DateInput {
    ident: Ident,
    focus_handle: FocusHandle,
    adapter: SharedDateAdapter,
    field: Entity<TextInput>,
    calendar: Entity<Calendar>,
    value: Option<Day>,
    /// The adapter's refusal to read the current text, held verbatim.
    message: Option<SharedString>,
    open: bool,
    size: ControlSize,
    disabled: bool,
    required: bool,
    /// Whether the seeded day has been put on screen. The text belongs to the
    /// typist afterwards, so it is written once.
    seeded: bool,
    /// Held so the field and calendar subscriptions live as long as this does.
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for DateInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DateInput")
            .field("ident", &self.ident)
            .field("value", &self.value)
            .field("open", &self.open)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl DateInput {
    pub fn new(
        ident: impl Into<Ident>,
        adapter: SharedDateAdapter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ident = ident.into();
        let field = cx.new(|cx| TextInput::new(ident.child("field"), window, cx).bare(true));
        let calendar =
            cx.new(|cx| Calendar::new(ident.child("calendar"), adapter.clone(), window, cx));
        let subscriptions = vec![
            cx.subscribe(&field, |input, _field, event, cx| match event {
                TextInputEvent::Change(text) => input.read_typed(text.clone(), cx),
                TextInputEvent::Submit => cx.emit(DateInputEvent::Submit),
                _ => {}
            }),
            cx.subscribe(&calendar, |input, _calendar, event, cx| {
                if let CalendarEvent::Picked(day) = event {
                    input.take(*day, cx);
                }
            }),
        ];

        Self {
            ident,
            focus_handle: cx.focus_handle(),
            adapter,
            field,
            calendar,
            value: None,
            message: None,
            open: false,
            size: ControlSize::Md,
            disabled: false,
            required: false,
            seeded: false,
            _subscriptions: subscriptions,
        }
    }

    /// Seeds the day the caller holds.
    pub fn value(mut self, day: Day) -> Self {
        self.value = Some(day);
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Replaces the day from the host side.
    ///
    /// The host already knows the day it just set, so this reports nothing.
    pub fn set_value(&mut self, day: Option<Day>, cx: &mut Context<Self>) {
        self.value = day;
        self.message = None;
        self.seeded = true;
        let text = day
            .map(|day| self.adapter.format_day(day))
            .unwrap_or_default();
        self.field
            .update(cx, |field, cx| field.set_text_quietly(text, cx));
        if let Some(day) = day {
            self.calendar
                .update(cx, |calendar, cx| calendar.set_selection(vec![day], cx));
        }
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.field
            .update(cx, |field, cx| field.set_disabled(disabled, cx));
        if disabled {
            self.open = false;
        }
        cx.notify();
    }

    pub fn field(&self) -> &Entity<TextInput> {
        &self.field
    }

    pub fn calendar(&self) -> &Entity<Calendar> {
        &self.calendar
    }

    pub fn current(&self) -> Option<Day> {
        self.value
    }

    /// The day the adapter last accepted from the typed text, if any.
    pub fn parsed_day(&self, cx: &App) -> Option<Day> {
        if let Some(day) = self.value {
            return Some(day);
        }
        let text = self.shown_text(cx);
        if text.trim().is_empty() {
            return None;
        }
        self.adapter.parse_day(text.as_ref()).ok()
    }

    /// The adapter's reason for refusing the current text, or `None`.
    pub fn message(&self) -> Option<&SharedString> {
        self.message.as_ref()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The text as the typist left it, which is not the host's value while an
    /// edit is in progress.
    pub fn shown_text(&self, cx: &App) -> SharedString {
        self.field.read(cx).value().clone()
    }

    pub fn is_invalid(&self) -> bool {
        self.message.is_some()
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        if self.disabled || self.open {
            return;
        }
        self.open = true;
        cx.emit(DateInputEvent::Opened);
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        cx.emit(DateInputEvent::Closed);
        cx.notify();
    }

    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.close(cx);
        } else {
            self.open(cx);
        }
    }

    /// Reads what was typed, and keeps it whatever the answer is.
    fn read_typed(&mut self, text: SharedString, cx: &mut Context<Self>) {
        if text.trim().is_empty() {
            self.message = None;
            cx.notify();
            return;
        }
        match self.adapter.parse_day(text.as_ref()) {
            Ok(day) => {
                self.message = None;
                self.calendar
                    .update(cx, |calendar, cx| calendar.set_selection(vec![day], cx));
                cx.emit(DateInputEvent::Changed(day));
            }
            Err(message) => {
                self.message = Some(message.clone());
                cx.emit(DateInputEvent::Unparsable { text, message });
            }
        }
        cx.notify();
    }

    /// Takes a day from the calendar, writing it into the field so the typist
    /// sees what they asked for while the host decides about it.
    fn take(&mut self, day: Day, cx: &mut Context<Self>) {
        let text = self.adapter.format_day(day);
        self.field
            .update(cx, |field, cx| field.set_text_quietly(text, cx));
        self.message = None;
        self.calendar
            .update(cx, |calendar, cx| calendar.set_selection(vec![day], cx));
        cx.emit(DateInputEvent::Changed(day));
        self.close(cx);
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        match event.keystroke.key.as_str() {
            "escape" if self.open => {
                self.close(cx);
                cx.stop_propagation();
            }
            "down" if !self.open => {
                self.open(cx);
                cx.stop_propagation();
            }
            _ => {}
        }
    }
}

impl Disableable for DateInput {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for DateInput {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for DateInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DateInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        if !self.seeded {
            self.seeded = true;
            if let Some(day) = self.value {
                let text = self.adapter.format_day(day);
                self.field
                    .update(cx, |field, cx| field.set_text_quietly(text, cx));
                self.calendar
                    .update(cx, |calendar, cx| calendar.set_selection(vec![day], cx));
            }
        }
        if self.disabled != self.field.read(cx).is_disabled() {
            let disabled = self.disabled;
            self.field
                .update(cx, |field, cx| field.set_disabled(disabled, cx));
        }

        let focused = self.field.read(cx).focus_handle(cx).is_focused(window);
        let invalid = self.is_invalid();
        let input = cx.entity().downgrade();

        let trigger = IconButton::new(
            self.ident.child("open"),
            Icon::Checklist,
            cx.strings().text(StringKey::DateInputOpen),
        )
        .ghost()
        .control_size(self.size)
        .semantic_parent(self.ident.semantic_id())
        .disabled(self.disabled)
        .on_click(move |_window, cx| {
            input.update(cx, |input, cx| input.toggle(cx)).ok();
        });

        let surface = self.open.then(|| {
            let card = popover::card(&theme)
                .child(self.calendar.clone())
                .into_any_element();
            popover::anchored_below(
                ElementId::from(self.ident.child("surface").semantic_id()),
                &theme,
                card,
            )
        });

        let message = self.message.clone().map(|message| {
            foundation_text(&theme, TypeScale::Caption, message.clone())
                .text_color(theme.colors.danger)
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("message").semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .text(message),
                )
        });

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap(px(theme.space(Space::Xs)))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .child(
                field_shell(
                    &theme,
                    self.size,
                    FieldState::default()
                        .focused(focused)
                        .invalid(invalid)
                        .disabled(self.disabled),
                )
                .child(div().flex_1().child(self.field.clone()))
                .child(trigger),
            )
            .child(div().relative().children(surface))
            .children(message)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Input)
                    .focus(&self.field.read(cx).focus_handle(cx))
                    .disabled(self.disabled)
                    .required(self.required)
                    .invalid(invalid)
                    .expanded(self.open)
                    .value(self.field.read(cx).value().clone())
                    .placeholder(cx.strings().text(StringKey::DateInputPlaceholder)),
            )
    }
}
