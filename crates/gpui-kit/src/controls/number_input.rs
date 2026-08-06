//! A numeric field with a step control.
//!
//! The number belongs to the caller. The control reports the number that was
//! asked for and renders whatever it is handed, so a value the host will not
//! accept is visible as the value not moving. A value outside the range the
//! control was given is shown as it is and published as invalid: hiding it
//! behind a silent clamp would report a number nobody entered.

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, SharedString, Styled,
    Subscription, Window, div, prelude::FluentBuilder,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, TypeScale};

use crate::controls::button::IconButton;
use crate::controls::field::{FieldState, field_shell};
use crate::controls::input::{TextInput, TextInputEvent};
use crate::foundation::{Disableable, Ident, Sizable, StyledExt};

/// How much larger a page step is than a single step.
const PAGE_FACTOR: f64 = 10.0;

/// What a number field reports. The owner decides what any of it means.
#[derive(Debug, Clone, PartialEq)]
pub enum NumberInputEvent {
    /// The number that was asked for, by typing or by a step. It is reported
    /// exactly as it was asked for, including outside the range, so the host
    /// decides what to do about it.
    Changed(f64),
    /// What is in the field is not a number at all.
    Unparsable(SharedString),
    Submit,
}

impl EventEmitter<NumberInputEvent> for NumberInput {}

pub struct NumberInput {
    ident: Ident,
    focus_handle: FocusHandle,
    field: Entity<TextInput>,
    value: f64,
    min: Option<f64>,
    max: Option<f64>,
    step: f64,
    page_step: Option<f64>,
    precision: usize,
    unit: Option<SharedString>,
    size: ControlSize,
    disabled: bool,
    required: bool,
    /// Whether the seeded number has been put on screen. The text belongs to
    /// the typist afterwards, so it is written once.
    seeded: bool,
    /// Held so the field subscription lives as long as the control does.
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for NumberInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NumberInput")
            .field("ident", &self.ident)
            .field("value", &self.value)
            .field("range", &(self.min, self.max))
            .field("step", &self.step)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl NumberInput {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let field = cx.new(|cx| TextInput::new(ident.child("field"), window, cx).bare(true));
        let subscription = cx.subscribe(&field, |number, _field, event, cx| match event {
            TextInputEvent::Change(text) => {
                number.report_typed(text.clone(), cx);
            }
            TextInputEvent::Submit => cx.emit(NumberInputEvent::Submit),
            _ => {}
        });

        Self {
            ident,
            focus_handle: cx.focus_handle(),
            field,
            value: 0.0,
            min: None,
            max: None,
            step: 1.0,
            page_step: None,
            precision: 0,
            unit: None,
            size: ControlSize::Md,
            disabled: false,
            required: false,
            seeded: false,
            _subscriptions: vec![subscription],
        }
    }

    /// Seeds the number the control draws. The caller keeps owning it.
    pub fn value(mut self, value: f64) -> Self {
        self.value = value;
        self
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    pub fn min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    pub fn max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    /// How far one arrow key or one step button moves.
    pub fn step(mut self, step: f64) -> Self {
        if step > 0.0 {
            self.step = step;
        }
        self
    }

    /// How far page-up and page-down move. Ten steps unless told otherwise.
    pub fn page_step(mut self, page_step: f64) -> Self {
        if page_step > 0.0 {
            self.page_step = Some(page_step);
        }
        self
    }

    /// How many decimals the field draws and reports.
    pub fn precision(mut self, precision: usize) -> Self {
        self.precision = precision;
        self
    }

    /// What the number counts, such as `"ms"`. Shown after the field and
    /// carried in the published value.
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Replaces the number from the host side.
    ///
    /// The host already knows the number it just set, so this reports
    /// nothing; only what a typist asks for is reported.
    pub fn set_value(&mut self, value: f64, cx: &mut Context<Self>) {
        self.value = value;
        self.seeded = true;
        self.write(value, cx);
        cx.notify();
    }

    fn write(&mut self, value: f64, cx: &mut Context<Self>) {
        let text = self.formatted(value);
        self.field
            .update(cx, |field, cx| field.set_text_quietly(text, cx));
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.field
            .update(cx, |field, cx| field.set_disabled(disabled, cx));
        cx.notify();
    }

    pub fn current(&self) -> f64 {
        self.value
    }

    pub fn field(&self) -> &Entity<TextInput> {
        &self.field
    }

    /// The number in the field, which is the typist's text rather than the
    /// host's value while an edit is in progress.
    pub fn shown(&self, cx: &App) -> Option<f64> {
        let text = self.field.read(cx).value();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        trimmed.parse::<f64>().ok()
    }

    fn is_empty(&self, cx: &App) -> bool {
        self.field.read(cx).value().trim().is_empty()
    }

    /// Whether what the field holds is something the control was told to
    /// accept. An empty field holds no number and says nothing about one.
    pub fn is_invalid(&self, cx: &App) -> bool {
        if self.is_empty(cx) {
            return false;
        }
        match self.shown(cx) {
            Some(value) => self.out_of_range(value),
            None => true,
        }
    }

    fn out_of_range(&self, value: f64) -> bool {
        self.min.is_some_and(|min| value < min) || self.max.is_some_and(|max| value > max)
    }

    fn formatted(&self, value: f64) -> SharedString {
        SharedString::from(format!("{value:.*}", self.precision))
    }

    /// What the control publishes as its value: the number and what it counts.
    fn display(&self, cx: &App) -> SharedString {
        let number = self.field.read(cx).value().clone();
        match &self.unit {
            Some(unit) if !number.is_empty() => SharedString::from(format!("{number} {unit}")),
            _ => number,
        }
    }

    /// Whether a step in this direction has anywhere to go.
    ///
    /// At a boundary there is no next number, so nothing is reported and the
    /// button that would report it is refused rather than inert-looking.
    pub fn can_step(&self, delta: f64, cx: &App) -> bool {
        if self.disabled {
            return false;
        }
        let from = self.shown(cx).unwrap_or(self.value);
        if delta > 0.0 {
            self.max.is_none_or(|max| from < max)
        } else {
            self.min.is_none_or(|min| from > min)
        }
    }

    fn stepped(&self, amount: f64, cx: &App) -> Option<f64> {
        if !self.can_step(amount, cx) {
            return None;
        }
        let from = self.shown(cx).unwrap_or(self.value);
        let mut next = from + amount;
        if let Some(max) = self.max {
            next = next.min(max);
        }
        if let Some(min) = self.min {
            next = next.max(min);
        }
        Some(round_to(next, self.precision))
    }

    /// Reports a step, and writes it into the field so the typist sees the
    /// number they asked for while the host decides about it.
    fn take_step(&mut self, amount: f64, cx: &mut Context<Self>) {
        let Some(next) = self.stepped(amount, cx) else {
            return;
        };
        self.write(next, cx);
        cx.emit(NumberInputEvent::Changed(next));
        cx.notify();
    }

    fn report_typed(&mut self, text: SharedString, cx: &mut Context<Self>) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            cx.notify();
            return;
        }
        match trimmed.parse::<f64>() {
            Ok(value) => cx.emit(NumberInputEvent::Changed(value)),
            Err(_) => cx.emit(NumberInputEvent::Unparsable(text)),
        }
        cx.notify();
    }

    fn page(&self) -> f64 {
        self.page_step.unwrap_or(self.step * PAGE_FACTOR)
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let amount = match event.keystroke.key.as_str() {
            "up" => self.step,
            "down" => -self.step,
            "pageup" => self.page(),
            "pagedown" => -self.page(),
            _ => return,
        };
        self.take_step(amount, cx);
        cx.stop_propagation();
    }
}

impl Disableable for NumberInput {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for NumberInput {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for NumberInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for NumberInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        if !self.seeded {
            self.seeded = true;
            let value = self.value;
            self.write(value, cx);
        }
        let focused = self.field.read(cx).focus_handle(cx).is_focused(window);
        let invalid = self.is_invalid(cx);
        let step = self.step;
        let can_increment = self.can_step(step, cx);
        let can_decrement = self.can_step(-step, cx);

        if self.disabled != self.field.read(cx).is_disabled() {
            let disabled = self.disabled;
            self.field
                .update(cx, |field, cx| field.set_disabled(disabled, cx));
        }

        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Input)
            .disabled(self.disabled)
            .invalid(invalid)
            .required(self.required)
            .focus(&self.field.read(cx).focus_handle(cx))
            .value(self.display(cx));
        if let (Some(min), Some(max)) = (self.min, self.max) {
            spec = spec.range(min as f32, max as f32, self.value as f32);
        }

        let control = cx.entity().downgrade();
        let decrement = IconButton::new(
            self.ident.child("decrement"),
            Icon::ArrowDown,
            SharedString::from("Decrease"),
        )
        .control_size(self.size)
        .semantic_parent(self.ident.semantic_id())
        .disabled(!can_decrement)
        .on_click({
            let control = control.clone();
            move |_window, cx| {
                control
                    .update(cx, |number, cx| number.take_step(-step, cx))
                    .ok();
            }
        });

        let increment = IconButton::new(
            self.ident.child("increment"),
            Icon::ArrowUp,
            SharedString::from("Increase"),
        )
        .control_size(self.size)
        .semantic_parent(self.ident.semantic_id())
        .disabled(!can_increment)
        .on_click(move |_window, cx| {
            control
                .update(cx, |number, cx| number.take_step(step, cx))
                .ok();
        });

        div()
            .id(self.ident.element_id())
            .row()
            .w_full()
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
                .when_some(self.unit.clone(), |element, unit| {
                    element.child(
                        div()
                            .flex_none()
                            .type_scale(&theme, TypeScale::Caption)
                            .text_color(theme.colors.text_muted)
                            .child(unit),
                    )
                })
                .child(div().flex_none().row().child(decrement).child(increment)),
            )
            .semantic_in(cx, spec)
    }
}

/// Rounds onto the grid the field draws, so a reported number is one the
/// field could show without changing it.
fn round_to(value: f64, precision: usize) -> f64 {
    let factor = 10f64.powi(precision as i32);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::round_to;

    #[test]
    fn stepping_lands_on_the_grid_the_field_draws() {
        assert_eq!(round_to(0.30000000000000004, 2), 0.3);
        assert_eq!(round_to(1.5, 0), 2.0);
    }
}
