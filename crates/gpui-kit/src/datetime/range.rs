//! Two ends of a range over a host-owned calendar.
//!
//! The picker reports and never judges. An incomplete range is a state of its
//! own rather than an error; an end before its start is said out loud instead
//! of quietly swapped; and a blocked day inside a range is named, leaving the
//! host to decide whether that makes the range unusable.

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled, Subscription,
    Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TextTone, TypeScale};

use crate::datetime::adapter::{Day, SharedDateAdapter};
use crate::datetime::calendar::{Calendar, CalendarEvent, DayMark};
use crate::display::badge::Tone;
use crate::display::status::StatusLine;
use crate::foundation::{Disableable, Ident, StyledExt, text as foundation_text};
use crate::strings::{ActiveStrings, StringKey};

/// A range as the caller holds it: a start, and an end once there is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayRange {
    pub start: Day,
    pub end: Option<Day>,
}

impl DayRange {
    pub fn starting(start: Day) -> Self {
        Self { start, end: None }
    }

    pub fn new(start: Day, end: Day) -> Self {
        Self {
            start,
            end: Some(end),
        }
    }
}

/// What the picker currently holds.
///
/// Incomplete and inverted are facts, not failures. Naming them separately is
/// what stops a half-finished range being drawn as a broken one, and an end
/// before a start being drawn as a range nobody asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RangeState {
    #[default]
    Unset,
    Incomplete {
        start: Day,
    },
    Complete {
        start: Day,
        end: Day,
    },
    /// The end sits before the start, exactly as it was given.
    Inverted {
        start: Day,
        end: Day,
    },
}

impl RangeState {
    /// The word the picker publishes for this state.
    pub fn name(self) -> &'static str {
        match self {
            Self::Unset => "unset",
            Self::Incomplete { .. } => "incomplete",
            Self::Complete { .. } => "complete",
            Self::Inverted { .. } => "end before start",
        }
    }

    pub fn from_range(range: Option<DayRange>) -> Self {
        match range {
            None => Self::Unset,
            Some(DayRange { start, end: None }) => Self::Incomplete { start },
            Some(DayRange {
                start,
                end: Some(end),
            }) if end < start => Self::Inverted { start, end },
            Some(DayRange {
                start,
                end: Some(end),
            }) => Self::Complete { start, end },
        }
    }
}

/// One day inside a range the adapter refuses, in the host's own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedDay {
    pub day: Day,
    pub reason: SharedString,
}

/// What the picker can say about the days between the two ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockedReport {
    /// There is no complete range to look at yet.
    NotApplicable,
    /// The host cannot enumerate the days in the range, so nothing was
    /// checked. This is not the same as finding nothing.
    Unchecked,
    Clear,
    Blocked(Vec<BlockedDay>),
}

/// What a range picker reports. The owner decides what any of it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangePickerEvent {
    /// A day was asked for as the start of a new range.
    StartPicked(Day),
    /// A day was asked for as the end of the range in progress.
    EndPicked(Day),
}

impl EventEmitter<RangePickerEvent> for RangePicker {}

/// A calendar with two ends and a running account of what they add up to.
pub struct RangePicker {
    ident: Ident,
    focus_handle: FocusHandle,
    adapter: SharedDateAdapter,
    calendar: Entity<Calendar>,
    range: Option<DayRange>,
    disabled: bool,
    /// Held so the calendar subscription lives as long as the picker does.
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for RangePicker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RangePicker")
            .field("ident", &self.ident)
            .field("range", &self.range)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl RangePicker {
    pub fn new(
        ident: impl Into<Ident>,
        adapter: SharedDateAdapter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ident = ident.into();
        let calendar =
            cx.new(|cx| Calendar::new(ident.child("calendar"), adapter.clone(), window, cx));
        let subscription = cx.subscribe(&calendar, |picker, _calendar, event, cx| match event {
            CalendarEvent::Picked(day) => picker.take(*day, cx),
            CalendarEvent::Hovered(_) => cx.notify(),
            CalendarEvent::MonthShown(_) => {}
        });

        Self {
            ident,
            focus_handle: cx.focus_handle(),
            adapter,
            calendar,
            range: None,
            disabled: false,
            _subscriptions: vec![subscription],
        }
    }

    /// Seeds the range the caller holds.
    pub fn range(mut self, range: DayRange) -> Self {
        self.range = Some(range);
        self
    }

    /// Marks days with a dot the host supplies, on the calendar underneath.
    pub fn set_overlay(
        &mut self,
        overlay: impl Fn(Day) -> Option<DayMark> + 'static,
        cx: &mut Context<Self>,
    ) {
        self.calendar
            .update(cx, |calendar, cx| calendar.set_overlay(overlay, cx));
    }

    pub fn set_range(&mut self, range: Option<DayRange>, cx: &mut Context<Self>) {
        if self.range == range {
            return;
        }
        self.range = range;
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled == disabled {
            return;
        }
        self.disabled = disabled;
        self.calendar
            .update(cx, |calendar, cx| calendar.set_disabled(disabled, cx));
        cx.notify();
    }

    pub fn calendar(&self) -> &Entity<Calendar> {
        &self.calendar
    }

    pub fn state(&self) -> RangeState {
        RangeState::from_range(self.range)
    }

    /// The days the adapter refuses inside the range, or the reason the
    /// question could not be answered.
    pub fn blocked(&self) -> BlockedReport {
        let (start, end) = match self.state() {
            RangeState::Complete { start, end } => (start, end),
            RangeState::Inverted { start, end } => (end, start),
            _ => return BlockedReport::NotApplicable,
        };
        let Some(days) = self.adapter.days_in(start, end) else {
            return BlockedReport::Unchecked;
        };
        let blocked: Vec<BlockedDay> = days
            .into_iter()
            .filter_map(|day| {
                self.adapter
                    .is_selectable(day)
                    .reason()
                    .cloned()
                    .map(|reason| BlockedDay { day, reason })
            })
            .collect();
        if blocked.is_empty() {
            BlockedReport::Clear
        } else {
            BlockedReport::Blocked(blocked)
        }
    }

    /// Which end the next pick fills. A range that is complete, inverted, or
    /// unset starts a new one.
    fn take(&mut self, day: Day, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        match self.state() {
            RangeState::Incomplete { .. } => cx.emit(RangePickerEvent::EndPicked(day)),
            _ => cx.emit(RangePickerEvent::StartPicked(day)),
        }
    }

    fn summary(&self, cx: &App) -> (SharedString, Tone) {
        let strings = cx.strings();
        match self.state() {
            RangeState::Unset => (strings.text(StringKey::RangeUnset), Tone::Neutral),
            RangeState::Incomplete { start } => (
                strings.format(
                    StringKey::RangeIncomplete,
                    &[self.adapter.format_day(start).as_ref()],
                ),
                Tone::Info,
            ),
            RangeState::Complete { start, end } => (
                strings.format(
                    StringKey::RangeComplete,
                    &[
                        self.adapter.format_day(start).as_ref(),
                        self.adapter.format_day(end).as_ref(),
                    ],
                ),
                Tone::Success,
            ),
            RangeState::Inverted { start, end } => (
                strings.format(
                    StringKey::RangeInverted,
                    &[
                        self.adapter.format_day(end).as_ref(),
                        self.adapter.format_day(start).as_ref(),
                    ],
                ),
                Tone::Warning,
            ),
        }
    }
}

impl Disableable for RangePicker {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Focusable for RangePicker {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RangePicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let range = self.range;
        self.calendar
            .update(cx, |calendar, cx| calendar.set_range(range, cx));
        let (summary, tone) = self.summary(cx);
        let state = self.state();
        let blocked = self.blocked();

        let blocked_line = match &blocked {
            BlockedReport::Unchecked => Some(
                foundation_text(
                    &theme,
                    TypeScale::Caption,
                    cx.strings().text(StringKey::RangeUncheckable),
                )
                .text_tone(&theme, TextTone::Muted)
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("blocked").semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .value("unchecked")
                        .text(cx.strings().text(StringKey::RangeUncheckable)),
                ),
            ),
            _ => None,
        };

        let named: Vec<gpui::AnyElement> = match &blocked {
            BlockedReport::Blocked(days) => days
                .iter()
                .map(|blocked| {
                    let blocked_id = format!("blocked-{}", blocked.day.0);
                    let ident = self.ident.child(blocked_id);
                    let text = cx.strings().format(
                        StringKey::RangeBlockedDay,
                        &[
                            self.adapter.format_day(blocked.day).as_ref(),
                            blocked.reason.as_ref(),
                        ],
                    );
                    foundation_text(&theme, TypeScale::Caption, text.clone())
                        .text_color(theme.colors.warning)
                        .semantic_in(
                            cx,
                            NodeSpec::new(ident.semantic_id(), Role::Status)
                                .parent(self.ident.semantic_id())
                                .text(text)
                                .value(blocked.reason.clone()),
                        )
                        .into_any_element()
                })
                .collect(),
            _ => Vec::new(),
        };

        div()
            .id(self.ident.element_id())
            .column()
            .flex_none()
            .gap_token(&theme, Space::Sm)
            .track_focus(&self.focus_handle)
            .child(self.calendar.clone())
            .child(
                div()
                    .column()
                    .gap(px(theme.space(Space::Xs)))
                    .child(StatusLine::new(summary.clone(), tone).id(self.ident.child("summary")))
                    .children(blocked_line)
                    .children(named),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .disabled(self.disabled)
                    .value(state.name()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_incomplete_range_is_its_own_state_rather_than_an_error() {
        assert_eq!(
            RangeState::from_range(Some(DayRange::starting(Day(4)))),
            RangeState::Incomplete { start: Day(4) }
        );
        assert_eq!(RangeState::from_range(None), RangeState::Unset);
    }

    #[test]
    fn an_end_before_the_start_is_reported_rather_than_swapped() {
        let state = RangeState::from_range(Some(DayRange::new(Day(9), Day(4))));
        assert_eq!(
            state,
            RangeState::Inverted {
                start: Day(9),
                end: Day(4)
            }
        );
        assert_eq!(state.name(), "end before start");
    }

    #[test]
    fn a_range_that_runs_forwards_is_complete() {
        assert_eq!(
            RangeState::from_range(Some(DayRange::new(Day(4), Day(9)))),
            RangeState::Complete {
                start: Day(4),
                end: Day(9)
            }
        );
    }
}
