//! Hour, minute, and optionally second, as separate segments.
//!
//! How far the hours run and what the two halves of a twelve-hour day are
//! called both come from [`Clock`], which the adapter supplies. The crate
//! contains no meridiem label in any language and never assumes a
//! twenty-four-hour day.

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TextTone, TypeScale};

use gpui_kit_assets::Icon;

use crate::controls::button::{ButtonJoin, IconButton};
use crate::controls::field::{FieldState, field_shell, nested_control_size};
use crate::datetime::adapter::{Clock, SharedDateAdapter, TimeOfDay};
use crate::foundation::direction::ActiveDirection;
use crate::foundation::stepping::bounded_step;
use crate::foundation::{Disableable, Ident, Sizable, StyledExt, text as foundation_text};
use crate::strings::{ActiveStrings, StringKey};

/// Which part of the time the keyboard is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Segment {
    Hour,
    Minute,
    Second,
    Meridiem,
}

impl Segment {
    fn key(self) -> &'static str {
        match self {
            Self::Hour => "hour",
            Self::Minute => "minute",
            Self::Second => "second",
            Self::Meridiem => "meridiem",
        }
    }

    fn label(self) -> StringKey {
        match self {
            Self::Hour => StringKey::TimeHour,
            Self::Minute => StringKey::TimeMinute,
            Self::Second => StringKey::TimeSecond,
            Self::Meridiem => StringKey::TimeMeridiem,
        }
    }
}

/// What a time field reports. The owner decides what any of it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInputEvent {
    Changed(TimeOfDay),
}

impl EventEmitter<TimeInputEvent> for TimeInput {}

/// A segmented time field over the host's own clock.
pub struct TimeInput {
    ident: Ident,
    focus_handle: FocusHandle,
    adapter: SharedDateAdapter,
    value: TimeOfDay,
    /// Whether a seconds segment is drawn at all.
    seconds: bool,
    active: Segment,
    /// The digits typed into the active segment since it was entered. Typing
    /// overwrites rather than inserts, so this restarts whenever the segment
    /// changes.
    typed: String,
    size: ControlSize,
    disabled: bool,
    /// Validation owned by a surrounding form.
    invalid: bool,
}

impl std::fmt::Debug for TimeInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TimeInput")
            .field("ident", &self.ident)
            .field("value", &self.value)
            .field("seconds", &self.seconds)
            .field("active", &self.active)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl TimeInput {
    pub fn new(
        ident: impl Into<Ident>,
        adapter: SharedDateAdapter,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let clock = adapter.clock();
        let value = TimeOfDay {
            hour: clock.hour_min,
            minute: 0,
            second: None,
            meridiem: clock.meridiem.as_ref().map(|_| 0),
        };
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            adapter,
            value,
            seconds: false,
            active: Segment::Hour,
            typed: String::new(),
            size: ControlSize::Md,
            disabled: false,
            invalid: false,
        }
    }

    /// Seeds the time the caller holds.
    pub fn value(mut self, value: TimeOfDay) -> Self {
        self.value = value;
        self.seconds = self.seconds || value.second.is_some();
        self
    }

    /// Whether the seconds segment is drawn.
    pub fn seconds(mut self, seconds: bool) -> Self {
        self.seconds = seconds;
        if seconds && self.value.second.is_none() {
            self.value.second = Some(0);
        }
        self
    }

    pub fn set_value(&mut self, value: TimeOfDay, cx: &mut Context<Self>) {
        self.value = value;
        self.typed.clear();
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn set_invalid(&mut self, invalid: bool, cx: &mut Context<Self>) {
        self.invalid = invalid;
        cx.notify();
    }

    pub fn current(&self) -> TimeOfDay {
        self.value
    }

    pub fn active_segment(&self) -> Segment {
        self.active
    }

    pub fn clock(&self) -> Clock {
        self.adapter.clock()
    }

    fn segments(&self) -> Vec<Segment> {
        let mut segments = vec![Segment::Hour, Segment::Minute];
        if self.seconds {
            segments.push(Segment::Second);
        }
        if self.adapter.clock().is_twelve_hour() {
            segments.push(Segment::Meridiem);
        }
        segments
    }

    /// The inclusive bounds of a segment, from the host's clock.
    fn bounds(&self, segment: Segment) -> (u32, u32) {
        let clock = self.adapter.clock();
        match segment {
            Segment::Hour => (clock.hour_min, clock.hour_max),
            Segment::Minute => (0, clock.minute_max),
            Segment::Second => (0, clock.second_max),
            Segment::Meridiem => (0, 1),
        }
    }

    fn raw(&self, segment: Segment) -> u32 {
        match segment {
            Segment::Hour => self.value.hour,
            Segment::Minute => self.value.minute,
            Segment::Second => self.value.second.unwrap_or(0),
            Segment::Meridiem => self.value.meridiem.unwrap_or(0) as u32,
        }
    }

    fn write(&mut self, segment: Segment, raw: u32, cx: &mut Context<Self>) {
        match segment {
            Segment::Hour => self.value.hour = raw,
            Segment::Minute => self.value.minute = raw,
            Segment::Second => self.value.second = Some(raw),
            Segment::Meridiem => self.value.meridiem = Some(raw as usize),
        }
        cx.emit(TimeInputEvent::Changed(self.value));
        cx.notify();
    }

    /// Steps the active segment, stopping at its bounds rather than rolling
    /// over into a neighbour the typist did not point at.
    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let segment = self.active;
        let (min, max) = self.bounds(segment);
        let Some(next) = step_within(min, max, self.raw(segment), delta) else {
            return;
        };
        self.typed.clear();
        self.write(segment, next, cx);
    }

    fn focus_segment(&mut self, segment: Segment, cx: &mut Context<Self>) {
        if self.active == segment {
            return;
        }
        self.active = segment;
        self.typed.clear();
        cx.notify();
    }

    fn move_segment(&mut self, delta: isize, cx: &mut Context<Self>) {
        let segments = self.segments();
        let from = segments.iter().position(|segment| *segment == self.active);
        let Some(next) = bounded_step(segments.len(), from, delta, |_| false) else {
            return;
        };
        self.focus_segment(segments[next], cx);
    }

    /// Overwrites the active segment with what is being typed.
    ///
    /// Two digits complete a segment and move on. A second digit that would
    /// leave the segment's bounds starts the segment over on that digit, so
    /// nothing lands on a value the clock does not have.
    fn type_digit(&mut self, digit: char, cx: &mut Context<Self>) {
        if self.disabled || self.active == Segment::Meridiem {
            return;
        }
        let segment = self.active;
        let (min, max) = self.bounds(segment);
        let mut candidate = format!("{}{digit}", self.typed);
        let mut value: u32 = candidate.parse().unwrap_or(0);
        if value > max || value < min && candidate.len() >= 2 {
            candidate = digit.to_string();
            value = candidate.parse().unwrap_or(min);
        }
        if value > max {
            return;
        }
        self.typed = candidate;
        self.write(segment, value, cx);
        if self.typed.len() >= 2 {
            self.typed.clear();
            self.move_segment(1, cx);
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let key = event.keystroke.key.as_str();
        // The segments are written in reading order, so moving between them
        // follows the reading direction. Up and down change the value, which
        // has nothing to do with which way the field reads.
        if let Some(step) = cx.layout_direction().arrow_step(key) {
            self.move_segment(step as isize, cx);
            cx.stop_propagation();
            return;
        }
        match key {
            "up" => self.step(1, cx),
            "down" => self.step(-1, cx),
            _ => {
                let mut characters = key.chars();
                match (characters.next(), characters.next()) {
                    (Some(digit), None) if digit.is_ascii_digit() => self.type_digit(digit, cx),
                    _ => return,
                }
            }
        }
        cx.stop_propagation();
    }

    fn segment_text(&self, segment: Segment) -> SharedString {
        match segment {
            Segment::Meridiem => self
                .adapter
                .clock()
                .meridiem_label(self.value.meridiem.unwrap_or(0))
                .unwrap_or_default(),
            other => SharedString::from(format!("{:02}", self.raw(other))),
        }
    }
}

impl Disableable for TimeInput {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for TimeInput {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for TimeInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TimeInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let focused = self.focus_handle.is_focused(window);
        let segments = self.segments();
        let last = segments.len().saturating_sub(1);
        // Every segment holds two characters, so no segment is narrower than
        // two. Sizing each one to its own glyphs is what put the colons of
        // one field at different places from the colons of the field beside
        // it; a caption wider than two digits then widens the column it
        // names, because a caption that does not occupy its own width prints
        // over the one next to it.
        let cell_width = px(metrics.font_size * 2.0);

        // A caption and the value it names are one column, so the column is
        // built once and both rows of it are laid out together. The mark
        // between two segments is a column of the same shape with nothing on
        // its caption line.
        let separator = |index: usize| {
            (index != last && segments[index] != Segment::Meridiem).then(|| {
                let mark = if segments[index + 1] == Segment::Meridiem {
                    SharedString::new_static(" ")
                } else {
                    SharedString::new_static(":")
                };
                div()
                    .flex_none()
                    .column()
                    .items_center()
                    .child(
                        foundation_text(&theme, TypeScale::Caption, SharedString::new_static(" "))
                            .text_color(gpui::transparent_black()),
                    )
                    .child(
                        foundation_text(&theme, TypeScale::Body, mark)
                            .text_color(theme.colors.text_faint),
                    )
                    .into_any_element()
            })
        };

        let mut cells: Vec<gpui::AnyElement> = Vec::new();
        for (index, segment) in segments.iter().copied().enumerate() {
            let ident = self.ident.child(segment.key());
            let text = self.segment_text(segment);
            let active = self.active == segment && focused;
            let (min, max) = self.bounds(segment);

            cells.push(
                div()
                    .id(ident.element_id())
                    .flex_none()
                    .min_w(cell_width)
                    .column()
                    .items_center()
                    .px_token(&theme, Space::Xs)
                    .radius(&theme, gpui_kit_theme::Radius::Small)
                    .when(active, |element| element.bg(theme.colors.selected))
                    .when(!self.disabled, |element| {
                        element.cursor_pointer().on_click(cx.listener(
                            move |input, _, window, cx| {
                                input.focus_handle.focus(window, cx);
                                input.focus_segment(segment, cx);
                            },
                        ))
                    })
                    .child(
                        foundation_text(
                            &theme,
                            TypeScale::Caption,
                            cx.strings().text(segment.label()),
                        )
                        .text_tone(
                            &theme,
                            match active {
                                true => TextTone::Primary,
                                false => TextTone::Faint,
                            },
                        ),
                    )
                    .child(
                        foundation_text(&theme, TypeScale::Body, text.clone()).text_tone(
                            &theme,
                            if self.disabled {
                                TextTone::Faint
                            } else {
                                TextTone::Primary
                            },
                        ),
                    )
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.semantic_id(), Role::Input)
                            .parent(self.ident.semantic_id())
                            .text(cx.strings().text(segment.label()))
                            .value(text)
                            .disabled(self.disabled)
                            .range(min as f32, max as f32, self.raw(segment) as f32)
                            .selected(active),
                    )
                    .into_any_element(),
            );
            cells.extend(separator(index));
        }

        // The two arrows step the segment the keyboard is on, which is the
        // same thing the up and down keys do. Without them the field says
        // nothing about being adjustable at all.
        let control = cx.entity().downgrade();
        let stepper_size = nested_control_size(self.size);
        let steppers = (!self.disabled).then(|| {
            let stepper = control.clone();
            let up = IconButton::new(
                self.ident.child("increment"),
                Icon::ArrowUp,
                cx.strings().text(StringKey::NumberIncrease),
            )
            .secondary()
            .join(ButtonJoin::Leading)
            .control_size(stepper_size)
            .semantic_parent(self.ident.semantic_id())
            .on_click(move |_window, cx| {
                stepper.update(cx, |input, cx| input.step(1, cx)).ok();
            });
            let stepper = control.clone();
            let down = IconButton::new(
                self.ident.child("decrement"),
                Icon::ArrowDown,
                cx.strings().text(StringKey::NumberDecrease),
            )
            .secondary()
            .join(ButtonJoin::Trailing)
            .control_size(stepper_size)
            .semantic_parent(self.ident.semantic_id())
            .on_click(move |_window, cx| {
                stepper.update(cx, |input, cx| input.step(-1, cx)).ok();
            });
            div().row().flex_none().child(up).child(down)
        });

        div()
            .id(self.ident.element_id())
            .column()
            .flex_none()
            .gap_token(&theme, Space::Xs)
            .track_focus(&self.focus_handle)
            .when(!self.disabled, |element| element.tab_index(0))
            .on_key_down(cx.listener(Self::on_key_down))
            .child(
                field_shell(
                    &theme,
                    self.size,
                    FieldState::default()
                        .focused(focused)
                        .invalid(self.invalid)
                        .disabled(self.disabled),
                )
                .children(cells)
                .child(div().flex_1())
                .children(steppers),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .focus(&self.focus_handle)
                    .disabled(self.disabled)
                    .invalid(self.invalid)
                    .value(self.adapter.format_time(self.value)),
            )
    }
}

/// The value `delta` steps away, inside the clock's own bounds.
///
/// A segment has ends: a minute past the last one is not the first one, it is
/// nowhere, so the step is refused rather than rolled over.
fn step_within(min: u32, max: u32, value: u32, delta: isize) -> Option<u32> {
    if max < min {
        return None;
    }
    let count = (max - min + 1) as usize;
    let from = value.checked_sub(min).map(|offset| offset as usize);
    bounded_step(count, from, delta, |_| false).map(|index| min + index as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_steps_inside_the_bounds_the_clock_gave_it() {
        assert_eq!(step_within(0, 59, 30, 1), Some(31));
        assert_eq!(step_within(0, 59, 30, -1), Some(29));
    }

    #[test]
    fn a_segment_has_ends_rather_than_rolling_over() {
        assert_eq!(step_within(0, 59, 59, 1), None);
        assert_eq!(step_within(0, 59, 0, -1), None);
    }

    #[test]
    fn a_twelve_hour_clock_starts_where_its_host_says_it_does() {
        assert_eq!(step_within(1, 12, 12, 1), None);
        assert_eq!(step_within(1, 12, 1, -1), None);
        assert_eq!(step_within(1, 12, 1, 1), Some(2));
    }
}
