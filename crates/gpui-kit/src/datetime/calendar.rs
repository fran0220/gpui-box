//! A month grid over a host-owned calendar.
//!
//! Every fact on screen — the weekday headings, the month name, which cells
//! hold days, what a day is called, whether it may be picked, and what day it
//! is — comes from the [`DateAdapter`](crate::datetime::DateAdapter). The
//! calendar owns the month it is
//! looking at, where the keyboard is, and what the pointer is over.

use std::rc::Rc;

use gpui::{
    AnimationElement, AnimationExt, AnyElement, App, Context, ElementId, EventEmitter, FocusHandle,
    Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, TextTone, Theme, TypeScale};

use crate::strings::{ActiveStrings, StringKey};

use crate::controls::button::IconButton;
use crate::datetime::adapter::{Day, MonthCell, MonthGrid, MonthKey, SharedDateAdapter};
use crate::datetime::range::DayRange;
use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::direction::{ActiveDirection, DirectionalExt, LayoutDirection};
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{
    Disableable, FocusRing, Ident, Sizable, StyledExt, text as foundation_text,
};
use crate::motion;
use crate::overlay::Tooltipped;

/// How wide one day cell is, and how tall. This geometry occurs once, so it
/// stays beside the component rather than in the token document.
const CELL_SIZE: f32 = 34.0;

/// How far a month slides as it arrives.
const MONTH_TRAVEL: f32 = 12.0;

/// A mark the host puts on a day, such as "three runs finished here".
///
/// The calendar draws the dot and publishes the wording; what it means is the
/// host's business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayMark {
    pub label: SharedString,
    pub tone: Tone,
}

impl DayMark {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            tone: Tone::Accent,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

type Overlay = Rc<dyn Fn(Day) -> Option<DayMark>>;

/// What a calendar reports. The owner decides what any of it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarEvent {
    /// A day was asked for. The selection stays the caller's.
    Picked(Day),
    /// The month on screen changed, always through
    /// [`DateAdapter::shift_month`](crate::datetime::DateAdapter::shift_month).
    MonthShown(MonthKey),
    /// The pointer moved onto or off a day, which is what a range preview
    /// follows.
    Hovered(Option<Day>),
}

impl EventEmitter<CalendarEvent> for Calendar {}

/// A month of days.
pub struct Calendar {
    ident: Ident,
    focus_handle: FocusHandle,
    adapter: SharedDateAdapter,
    selection: Vec<Day>,
    multi: bool,
    /// The month the caller asked to open on, if any.
    requested_month: Option<MonthKey>,
    /// The month navigation has moved to. Until something navigates, the
    /// month is resolved from the selection, then from today, and if neither
    /// is known the calendar says so instead of picking one.
    month: Option<MonthKey>,
    cursor: Option<Day>,
    hovered: Option<Day>,
    range: Option<DayRange>,
    overlay: Option<Overlay>,
    disabled: bool,
    /// Which way the last navigation travelled, and how many have happened.
    /// The count keys the arrival animation; zero means the first frame, which
    /// arrives without motion so a capture of a settled calendar is settled.
    travel: i32,
    navigations: usize,
    slots: Slots,
}

impl std::fmt::Debug for Calendar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Calendar")
            .field("ident", &self.ident)
            .field("selection", &self.selection)
            .field("multi", &self.multi)
            .field("month", &self.month)
            .field("cursor", &self.cursor)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl Calendar {
    pub fn new(
        ident: impl Into<Ident>,
        adapter: SharedDateAdapter,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            adapter,
            selection: Vec::new(),
            multi: false,
            requested_month: None,
            month: None,
            cursor: None,
            hovered: None,
            range: None,
            overlay: None,
            disabled: false,
            travel: 0,
            navigations: 0,
            slots: Slots::default(),
        }
    }

    /// Seeds the days the caller says are chosen.
    pub fn selected(mut self, days: impl IntoIterator<Item = Day>) -> Self {
        self.selection = days.into_iter().collect();
        self
    }

    /// Whether more than one day may be chosen. The calendar reports either
    /// way; this only decides what it draws and publishes.
    pub fn multi(mut self, multi: bool) -> Self {
        self.multi = multi;
        self
    }

    /// The month to open on, for a calendar whose host knows where it wants
    /// to start but has nothing selected and no today.
    pub fn month(mut self, month: MonthKey) -> Self {
        self.requested_month = Some(month);
        self
    }

    /// Marks days with a dot the host supplies.
    pub fn overlay(mut self, overlay: impl Fn(Day) -> Option<DayMark> + 'static) -> Self {
        self.overlay = Some(Rc::new(overlay));
        self
    }

    /// Draws a range across the grid, with its endpoints picked out.
    pub fn range(mut self, range: DayRange) -> Self {
        self.range = Some(range);
        self
    }

    pub fn set_selection(&mut self, days: Vec<Day>, cx: &mut Context<Self>) {
        if self.selection == days {
            return;
        }
        self.selection = days;
        cx.notify();
    }

    pub fn set_overlay(
        &mut self,
        overlay: impl Fn(Day) -> Option<DayMark> + 'static,
        cx: &mut Context<Self>,
    ) {
        self.overlay = Some(Rc::new(overlay));
        cx.notify();
    }

    /// A `RangePicker` pushes its caller's range down on every frame, so this
    /// has to be quiet when nothing moved or the two of them redraw each other
    /// forever.
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
        cx.notify();
    }

    /// Pins what the pointer is over, so a range preview can be staged for
    /// review without a pointer in the window.
    pub fn set_hovered_day(&mut self, day: Option<Day>, cx: &mut Context<Self>) {
        self.hover(day, cx);
    }

    pub fn adapter(&self) -> &SharedDateAdapter {
        &self.adapter
    }

    pub fn selection(&self) -> &[Day] {
        &self.selection
    }

    pub fn cursor(&self) -> Option<Day> {
        self.cursor
    }

    pub fn hovered_day(&self) -> Option<Day> {
        self.hovered
    }

    /// The month on screen, or `None` when nothing has established one.
    ///
    /// Navigation, then the caller's month, then the first selected day, then
    /// today. A calendar with none of those does not invent one.
    pub fn shown_month(&self) -> Option<MonthKey> {
        if let Some(month) = self.month {
            return Some(month);
        }
        if let Some(month) = self.requested_month {
            return Some(month);
        }
        if let Some(day) = self.selection.first() {
            return Some(self.adapter.month_of(*day));
        }
        self.adapter
            .today()
            .map(|today| self.adapter.month_of(today))
    }

    /// Moves the month through the adapter. Nothing here adds or subtracts.
    pub fn shift(&mut self, delta: i32, cx: &mut Context<Self>) {
        let Some(current) = self.shown_month() else {
            return;
        };
        let Some(next) = self.adapter.shift_month(current, delta) else {
            return;
        };
        self.month = Some(next);
        self.travel = delta.signum();
        self.navigations += 1;
        cx.emit(CalendarEvent::MonthShown(next));
        cx.notify();
    }

    pub fn show_month(&mut self, month: MonthKey, cx: &mut Context<Self>) {
        self.month = Some(month);
        self.navigations += 1;
        cx.emit(CalendarEvent::MonthShown(month));
        cx.notify();
    }

    fn grid(&self) -> Option<MonthGrid> {
        self.shown_month()
            .map(|month| self.adapter.month_grid(month))
    }

    fn pick(&mut self, day: Day, cx: &mut Context<Self>) {
        if self.disabled || !self.adapter.is_selectable(day).is_selectable() {
            return;
        }
        self.cursor = Some(day);
        cx.emit(CalendarEvent::Picked(day));
        cx.notify();
    }

    fn hover(&mut self, day: Option<Day>, cx: &mut Context<Self>) {
        if self.hovered == day {
            return;
        }
        self.hovered = day;
        cx.emit(CalendarEvent::Hovered(day));
        cx.notify();
    }

    /// Where the keyboard is, or where it would enter.
    fn anchor(&self, grid: &MonthGrid) -> Option<Day> {
        if let Some(cursor) = self.cursor
            && grid.position_of(cursor).is_some()
        {
            return Some(cursor);
        }
        self.selection
            .iter()
            .copied()
            .find(|day| grid.position_of(*day).is_some())
            .or_else(|| {
                self.adapter
                    .today()
                    .filter(|today| grid.position_of(*today).is_some())
            })
            .or_else(|| first_day(grid))
    }

    /// Moves the cursor one day, one week, or to the end of a week.
    ///
    /// Stepping off the grid moves to the neighbouring month through the
    /// adapter and lands on the cell the travel would have reached.
    fn move_cursor(&mut self, motion: Motion, cx: &mut Context<Self>) {
        let Some(grid) = self.grid() else {
            return;
        };
        let Some(from) = self.anchor(&grid) else {
            return;
        };
        // The first movement on a calendar the keyboard has not been in yet
        // places the cursor rather than moving it, so nothing is skipped over.
        if self.cursor.is_none() {
            self.cursor = Some(from);
            cx.notify();
            return;
        }
        self.cursor = Some(from);
        match step(&grid, from, motion) {
            Step::To(day) => {
                self.cursor = Some(day);
                cx.notify();
            }
            Step::OffGrid { delta, landing } => {
                let Some(current) = self.shown_month() else {
                    return;
                };
                let Some(next) = self.adapter.shift_month(current, delta) else {
                    return;
                };
                let grid = self.adapter.month_grid(next);
                self.month = Some(next);
                self.travel = delta.signum();
                self.navigations += 1;
                self.cursor = landing.resolve(&grid);
                cx.emit(CalendarEvent::MonthShown(next));
                cx.notify();
            }
            Step::Nowhere => {}
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        // A month grid is laid out in reading order, so the horizontal arrows
        // step through the calendar, not across the screen: the arrow that
        // reaches yesterday is the one pointing back the way the row was
        // written.
        let direction = cx.layout_direction();
        let key = event.keystroke.key.as_str();
        let motion = match direction.arrow_step(key) {
            Some(1) => Motion::NextDay,
            Some(_) => Motion::PreviousDay,
            None => match key {
                "up" => Motion::PreviousWeek,
                "down" => Motion::NextWeek,
                "home" => Motion::WeekStart,
                "end" => Motion::WeekEnd,
                "pageup" => {
                    self.shift(-1, cx);
                    cx.stop_propagation();
                    return;
                }
                "pagedown" => {
                    self.shift(1, cx);
                    cx.stop_propagation();
                    return;
                }
                "enter" | "space" => {
                    if let Some(day) = self.cursor {
                        self.pick(day, cx);
                    } else if let Some(grid) = self.grid()
                        && let Some(day) = self.anchor(&grid)
                    {
                        self.cursor = Some(day);
                        cx.notify();
                    }
                    cx.stop_propagation();
                    return;
                }
                _ => return,
            },
        };
        self.move_cursor(motion, cx);
        cx.stop_propagation();
    }

    fn header(&self, month: Option<MonthKey>, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let label = month
            .map(|month| self.adapter.month_label(month))
            .unwrap_or_else(|| cx.strings().text(StringKey::CalendarNoMonth));
        let can_move = month.is_some() && !self.disabled;
        let calendar = cx.entity().downgrade();
        let backward = calendar.clone();

        div()
            .row_reading(direction)
            .w_full()
            .gap_token(&theme, Space::Sm)
            .child(
                IconButton::new(
                    self.ident.child("previous"),
                    Icon::AltArrowLeft,
                    cx.strings().text(StringKey::CalendarPreviousMonth),
                )
                .ghost()
                .small()
                .semantic_parent(self.ident.semantic_id())
                .disabled(!can_move)
                .on_click(move |_window, cx| {
                    backward
                        .update(cx, |calendar, cx| calendar.shift(-1, cx))
                        .ok();
                }),
            )
            .child(
                foundation_text(&theme, TypeScale::Label, label)
                    .flex_1()
                    .text_align(gpui::TextAlign::Center),
            )
            .child(
                IconButton::new(
                    self.ident.child("next"),
                    Icon::AltArrowRight,
                    cx.strings().text(StringKey::CalendarNextMonth),
                )
                .ghost()
                .small()
                .semantic_parent(self.ident.semantic_id())
                .disabled(!can_move)
                .on_click(move |_window, cx| {
                    calendar
                        .update(cx, |calendar, cx| calendar.shift(1, cx))
                        .ok();
                }),
            )
            .into_any_element()
    }

    fn weekday_header(&self, theme: &Theme, direction: LayoutDirection) -> AnyElement {
        div()
            .row_reading(direction)
            .children(self.adapter.weekday_labels().into_iter().map(|label| {
                foundation_text(theme, TypeScale::Caption, label)
                    .w(px(CELL_SIZE))
                    .flex_none()
                    .text_align(gpui::TextAlign::Center)
                    .text_tone(theme, TextTone::Faint)
            }))
            .into_any_element()
    }

    fn cell(&self, cell: MonthCell, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(day) = cell.day() else {
            return div().size(px(CELL_SIZE)).flex_none().into_any_element();
        };

        let day_id = format!("day-{}", day.0);
        let ident = self.ident.child(day_id);
        let selectability = self.adapter.is_selectable(day);
        let blocked = selectability.reason().cloned();
        let selectable = selectability.is_selectable() && !self.disabled;
        let selected = self.selection.contains(&day);
        let is_today = self.adapter.today() == Some(day);
        let cursored = self.cursor == Some(day);
        let mark = self.overlay.as_ref().and_then(|overlay| overlay(day));
        let banded = self.band(day);
        let endpoint = self.is_endpoint(day);
        let label = self.adapter.day_label(day);

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Option)
            .parent(self.ident.child("grid").semantic_id())
            .text(label.clone())
            .checked(selected || endpoint)
            .disabled(!selectable)
            .hovered(self.hovered == Some(day));
        if let Some(reason) = blocked.clone() {
            spec = spec.value(reason);
        } else if let Some(mark) = &mark {
            spec = spec.value(mark.label.clone());
        }

        let background = if selected || endpoint {
            Some(theme.colors.accent)
        } else if banded {
            Some(theme.colors.selected)
        } else {
            None
        };

        let cell = div()
            .id(ident.element_id())
            .size(px(CELL_SIZE))
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(2.0))
            .radius(&theme, Radius::Control)
            .when_some(background, |element, color| element.bg(color))
            .when(is_today && !selected && !endpoint, |element| {
                element
                    .border(px(theme.borders.thick))
                    .border_color(theme.colors.accent)
            })
            .when(cursored, |element| element.shadow(theme.focus_ring()))
            .when(selectable, |element| {
                element
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.colors.hover))
                    .on_click(cx.listener(move |calendar, _, _window, cx| {
                        calendar.pick(day, cx);
                    }))
                    .on_hover(cx.listener(move |calendar, over: &bool, _window, cx| {
                        calendar.hover(over.then_some(day), cx);
                    }))
            })
            .when_some(blocked.clone(), |element, reason| {
                element
                    .opacity(theme.opacity.disabled)
                    .tip(ident.clone(), reason)
            })
            .child(if selected || endpoint {
                foundation_text(&theme, TypeScale::Label, label)
                    .text_color(theme.colors.text_on_accent)
            } else if !selectable {
                foundation_text(&theme, TypeScale::Label, label).text_tone(&theme, TextTone::Faint)
            } else if cell.is_adjacent() {
                foundation_text(&theme, TypeScale::Label, label).text_tone(&theme, TextTone::Muted)
            } else {
                foundation_text(&theme, TypeScale::Label, label)
            })
            .children(mark.as_ref().map(|mark| {
                div()
                    .size(px(4.0))
                    .rounded_full()
                    .bg(if selected || endpoint {
                        theme.colors.text_on_accent
                    } else {
                        mark.tone.color(&theme)
                    })
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.child("mark").semantic_id(), Role::Status)
                            .parent(ident.semantic_id())
                            .text(mark.label.clone()),
                    )
            }))
            .semantic_in(cx, spec);

        cell.into_any_element()
    }

    /// Whether the day falls inside the drawn range, including the length a
    /// hover is currently previewing.
    fn band(&self, day: Day) -> bool {
        let Some(range) = &self.range else {
            return false;
        };
        let end = range.end.or(self.hovered);
        let Some(end) = end else {
            return false;
        };
        let (low, high) = if range.start <= end {
            (range.start, end)
        } else {
            (end, range.start)
        };
        low <= day && day <= high
    }

    fn is_endpoint(&self, day: Day) -> bool {
        self.range
            .as_ref()
            .is_some_and(|range| range.start == day || range.end == Some(day))
    }

    fn body(&self, grid: &MonthGrid, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let grid_ident = self.ident.child("grid");
        let weeks: Vec<AnyElement> = grid
            .weeks
            .iter()
            .map(|week| {
                div()
                    .row_reading(direction)
                    .children(
                        week.iter()
                            .map(|cell| self.cell(*cell, cx))
                            .collect::<Vec<_>>(),
                    )
                    .into_any_element()
            })
            .collect();

        div()
            .column()
            .child(self.weekday_header(&theme, direction))
            .child(div().column().children(weeks))
            .semantic_in(
                cx,
                NodeSpec::new(grid_ident.semantic_id(), Role::Group)
                    .parent(self.ident.semantic_id()),
            )
            .into_any_element()
    }
}

impl Disableable for Calendar {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Focusable for Calendar {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Slotted for Calendar {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl Render for Calendar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let month = self.shown_month();
        let today = self.adapter.today();
        let header = self.header(month, cx);

        let body = match (month, self.grid()) {
            (Some(month), Some(grid)) => {
                let body = div().column().child(self.body(&grid, cx));
                if self.navigations == 0 {
                    body.into_any_element()
                } else {
                    let animation_id = format!("month.{}.{}", month.0, self.navigations);
                    month_in(
                        self.ident.child(animation_id).element_id(),
                        &theme,
                        self.travel * if direction.is_rtl() { -1 } else { 1 },
                        body,
                    )
                    .into_any_element()
                }
            }
            _ => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(
                    self.ident.child("unknown-month"),
                    cx.strings().text(StringKey::CalendarUnknownMonth),
                )
                .kind(EmptyKind::Unavailable)
                .detail(cx.strings().text(StringKey::CalendarUnknownMonthDetail))
                .into_any_element()
            }),
        };

        let today_marker = today
            .filter(|today| {
                self.grid()
                    .is_some_and(|grid| grid.position_of(*today).is_some())
            })
            .map(|today| {
                foundation_text(&theme, TypeScale::Caption, self.adapter.format_day(today))
                    .text_tone(&theme, TextTone::Muted)
                    .semantic_in(
                        cx,
                        NodeSpec::new(self.ident.child("today").semantic_id(), Role::Status)
                            .parent(self.ident.semantic_id())
                            .text(self.adapter.format_day(today)),
                    )
            });

        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Group)
            .focus(&self.focus_handle)
            .disabled(self.disabled);
        match month {
            Some(month) => spec = spec.text(self.adapter.month_label(month)),
            None => spec = spec.value("month unknown"),
        }

        div()
            .id(self.ident.element_id())
            .column()
            .flex_none()
            .gap_token(&theme, Space::Sm)
            .p_token(&theme, Space::Sm)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .track_focus(&self.focus_handle)
            .when(!self.disabled, |element| {
                element.tab_index(0).focus_ring(&theme)
            })
            .on_key_down(cx.listener(Self::on_key_down))
            .child(header)
            .child(body)
            .children(today_marker)
            .semantic_in(cx, spec)
    }
}

/// The arrival a month makes when navigation replaces it.
///
/// Only a month that was travelled to gets this. The first month drawn is
/// rendered bare rather than animated to a standstill, so a calendar nobody
/// has touched photographs the same way twice.
fn month_in<E>(
    id: impl Into<ElementId>,
    theme: &Theme,
    direction: i32,
    element: E,
) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    let offset = MONTH_TRAVEL * direction as f32;
    element.with_animation(
        id,
        motion::menu(theme).animation(),
        move |element, progress| {
            element
                .relative()
                .opacity(0.4 + 0.6 * progress)
                .left(px(offset * (1.0 - progress)))
        },
    )
}

/// Which way a keystroke moves the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Motion {
    PreviousDay,
    NextDay,
    PreviousWeek,
    NextWeek,
    WeekStart,
    WeekEnd,
}

/// Where a step that left the grid should land in the neighbouring month.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Landing {
    First,
    Last,
    /// The same position in the first or last week of the new month.
    Column {
        column: usize,
        from_start: bool,
    },
}

impl Landing {
    fn resolve(self, grid: &MonthGrid) -> Option<Day> {
        match self {
            Self::First => first_day(grid),
            Self::Last => last_day(grid),
            Self::Column { column, from_start } => {
                let week = if from_start {
                    grid.weeks.first()
                } else {
                    grid.weeks.last()
                };
                week.and_then(|week| week.get(column).and_then(|cell| cell.day()))
                    .or_else(|| {
                        if from_start {
                            first_day(grid)
                        } else {
                            last_day(grid)
                        }
                    })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    To(Day),
    OffGrid { delta: i32, landing: Landing },
    Nowhere,
}

pub(crate) fn first_day(grid: &MonthGrid) -> Option<Day> {
    grid.weeks
        .iter()
        .flat_map(|week| week.iter())
        .find_map(|cell| cell.day())
}

pub(crate) fn last_day(grid: &MonthGrid) -> Option<Day> {
    grid.weeks
        .iter()
        .rev()
        .flat_map(|week| week.iter().rev())
        .find_map(|cell| cell.day())
}

/// Where one keystroke takes the cursor, using only the grid the adapter
/// produced. Nothing here adds a day to a date.
pub(crate) fn step(grid: &MonthGrid, from: Day, motion: Motion) -> Step {
    let Some((week, column)) = grid.position_of(from) else {
        return Step::Nowhere;
    };
    match motion {
        Motion::WeekStart => grid.weeks[week]
            .iter()
            .find_map(|cell| cell.day())
            .map_or(Step::Nowhere, Step::To),
        Motion::WeekEnd => grid.weeks[week]
            .iter()
            .rev()
            .find_map(|cell| cell.day())
            .map_or(Step::Nowhere, Step::To),
        Motion::PreviousDay | Motion::NextDay => {
            let forward = motion == Motion::NextDay;
            match neighbour(grid, week, column, forward) {
                Some(day) => Step::To(day),
                None => Step::OffGrid {
                    delta: if forward { 1 } else { -1 },
                    landing: if forward {
                        Landing::First
                    } else {
                        Landing::Last
                    },
                },
            }
        }
        Motion::PreviousWeek | Motion::NextWeek => {
            let forward = motion == Motion::NextWeek;
            let target = if forward {
                week.checked_add(1)
            } else {
                week.checked_sub(1)
            };
            match target
                .and_then(|week| grid.weeks.get(week))
                .and_then(|week| week.get(column))
                .and_then(|cell| cell.day())
            {
                Some(day) => Step::To(day),
                None => Step::OffGrid {
                    delta: if forward { 1 } else { -1 },
                    landing: Landing::Column {
                        column,
                        from_start: forward,
                    },
                },
            }
        }
    }
}

/// The next cell holding a day, walking the grid in reading order.
fn neighbour(grid: &MonthGrid, week: usize, column: usize, forward: bool) -> Option<Day> {
    let flat: Vec<Option<Day>> = grid
        .weeks
        .iter()
        .flat_map(|week| week.iter().map(|cell| cell.day()))
        .collect();
    let width = grid.weeks.first().map_or(0, |week| week.len());
    if width == 0 {
        return None;
    }
    let index = week * width + column;
    if forward {
        flat.get(index + 1..)?.iter().flatten().copied().next()
    } else {
        flat.get(..index)?.iter().rev().flatten().copied().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> MonthGrid {
        MonthGrid::new([
            vec![
                MonthCell::Empty,
                MonthCell::Day(Day(1)),
                MonthCell::Day(Day(2)),
            ],
            vec![
                MonthCell::Day(Day(3)),
                MonthCell::Day(Day(4)),
                MonthCell::Day(Day(5)),
            ],
        ])
    }

    #[test]
    fn a_day_step_walks_the_grid_in_reading_order() {
        assert_eq!(step(&grid(), Day(2), Motion::NextDay), Step::To(Day(3)));
        assert_eq!(step(&grid(), Day(3), Motion::PreviousDay), Step::To(Day(2)));
        assert_eq!(
            step(&grid(), Day(1), Motion::PreviousDay),
            Step::OffGrid {
                delta: -1,
                landing: Landing::Last
            }
        );
    }

    #[test]
    fn home_and_end_reach_the_ends_of_the_week_that_holds_the_cursor() {
        assert_eq!(step(&grid(), Day(2), Motion::WeekStart), Step::To(Day(1)));
        assert_eq!(step(&grid(), Day(1), Motion::WeekEnd), Step::To(Day(2)));
        assert_eq!(step(&grid(), Day(4), Motion::WeekStart), Step::To(Day(3)));
    }

    #[test]
    fn a_week_step_keeps_the_column() {
        assert_eq!(step(&grid(), Day(1), Motion::NextWeek), Step::To(Day(4)));
        assert_eq!(
            step(&grid(), Day(5), Motion::PreviousWeek),
            Step::To(Day(2))
        );
    }

    #[test]
    fn stepping_off_the_grid_asks_for_the_neighbouring_month() {
        assert_eq!(
            step(&grid(), Day(5), Motion::NextDay),
            Step::OffGrid {
                delta: 1,
                landing: Landing::First
            }
        );
        assert_eq!(
            step(&grid(), Day(4), Motion::NextWeek),
            Step::OffGrid {
                delta: 1,
                landing: Landing::Column {
                    column: 1,
                    from_start: true
                }
            }
        );
    }

    #[test]
    fn the_ends_of_a_grid_are_found_by_content_rather_than_by_slot() {
        assert_eq!(first_day(&grid()), Some(Day(1)));
        assert_eq!(last_day(&grid()), Some(Day(5)));
    }
}
