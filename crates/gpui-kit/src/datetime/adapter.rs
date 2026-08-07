//! The seam between the date components and whoever owns the calendar.
//!
//! This crate has no calendar system, no time zone, no locale, and no notion
//! of what day it is. Every one of those facts arrives through
//! [`DateAdapter`], which the host implements over whatever date library it
//! already depends on. A [`Day`] is an opaque token: the components carry it,
//! compare it, and hand it back, and only the adapter that produced it knows
//! what it means.

use std::rc::Rc;

use gpui::SharedString;

/// One day, meaningful only to the adapter that produced it.
///
/// The ordering is part of the contract: an adapter must number days so that
/// an earlier day compares less than a later one, because a range picker has
/// to be able to say that an end comes before its start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(pub i64);

/// One month, meaningful only to the adapter that produced it.
///
/// The components never take it apart and never do arithmetic on it. Moving to
/// the next month is [`DateAdapter::shift_month`], because how many months a
/// year has, and whether the answer is the same every year, is calendar work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonthKey(pub i64);

/// One slot in a month grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonthCell {
    /// Nothing at all sits here. A grid that pads its first week with blanks
    /// rather than with the previous month's days uses this.
    Empty,
    /// A day of the month the grid was asked for.
    Day(Day),
    /// A day of the month before or after, drawn so the grid is rectangular.
    Adjacent(Day),
}

impl MonthCell {
    pub fn day(self) -> Option<Day> {
        match self {
            Self::Empty => None,
            Self::Day(day) | Self::Adjacent(day) => Some(day),
        }
    }

    pub fn is_adjacent(self) -> bool {
        matches!(self, Self::Adjacent(_))
    }
}

/// A month laid out in weeks.
///
/// Every week must hold as many cells as [`DateAdapter::weekday_labels`]
/// returns, in the same order, because the header and the body are drawn as
/// one table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MonthGrid {
    pub weeks: Vec<Vec<MonthCell>>,
}

impl MonthGrid {
    pub fn new(weeks: impl IntoIterator<Item = Vec<MonthCell>>) -> Self {
        Self {
            weeks: weeks.into_iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.weeks
            .iter()
            .all(|week| week.iter().all(|cell| matches!(cell, MonthCell::Empty)))
    }

    /// Where `day` sits, as a week and a position inside it.
    pub fn position_of(&self, day: Day) -> Option<(usize, usize)> {
        self.weeks.iter().enumerate().find_map(|(week, cells)| {
            cells
                .iter()
                .position(|cell| cell.day() == Some(day))
                .map(|column| (week, column))
        })
    }
}

/// Whether a day may be picked, and if not, why not.
///
/// The reason is the host's own wording and is shown verbatim. The components
/// never author one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selectability {
    Selectable,
    Blocked { reason: SharedString },
}

impl Selectability {
    pub fn blocked(reason: impl Into<SharedString>) -> Self {
        Self::Blocked {
            reason: reason.into(),
        }
    }

    pub fn is_selectable(&self) -> bool {
        matches!(self, Self::Selectable)
    }

    pub fn reason(&self) -> Option<&SharedString> {
        match self {
            Self::Selectable => None,
            Self::Blocked { reason } => Some(reason),
        }
    }
}

/// The clock the host keeps: how far its hours run, and what it calls the two
/// halves of a twelve-hour day.
///
/// A crate that hard-coded `AM` and `PM` would be hard-coding a language, so
/// the labels arrive here or the clock has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clock {
    pub hour_min: u32,
    pub hour_max: u32,
    pub minute_max: u32,
    pub second_max: u32,
    /// The two meridiem labels in order, when the host's clock has them.
    pub meridiem: Option<(SharedString, SharedString)>,
}

impl Clock {
    pub fn is_twelve_hour(&self) -> bool {
        self.meridiem.is_some()
    }

    pub fn meridiem_label(&self, index: usize) -> Option<SharedString> {
        let (first, second) = self.meridiem.as_ref()?;
        match index {
            0 => Some(first.clone()),
            1 => Some(second.clone()),
            _ => None,
        }
    }
}

/// A time of day, in the host's own clock.
///
/// `meridiem` indexes [`Clock::meridiem`] rather than naming a half of the
/// day, because the naming belongs to the adapter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimeOfDay {
    pub hour: u32,
    pub minute: u32,
    pub second: Option<u32>,
    pub meridiem: Option<usize>,
}

impl TimeOfDay {
    pub fn new(hour: u32, minute: u32) -> Self {
        Self {
            hour,
            minute,
            second: None,
            meridiem: None,
        }
    }

    pub fn with_second(mut self, second: u32) -> Self {
        self.second = Some(second);
        self
    }

    pub fn with_meridiem(mut self, index: usize) -> Self {
        self.meridiem = Some(index);
        self
    }
}

/// Everything the date components are not allowed to know.
///
/// One implementation of this is the whole seam. The components read facts
/// from it, hand [`Day`] values back to it, and never derive a date fact
/// themselves — including which month a keystroke moved to, which is
/// [`shift_month`](DateAdapter::shift_month) rather than an addition.
pub trait DateAdapter {
    /// What day it is, or `None` when the host has not established one.
    ///
    /// `None` is a real answer and is rendered as one: no today ring is drawn
    /// and no month is guessed from it.
    fn today(&self) -> Option<Day>;

    fn month_of(&self, day: Day) -> MonthKey;

    fn month_grid(&self, month: MonthKey) -> MonthGrid;

    /// What the month is called, already in the host's language.
    fn month_label(&self, month: MonthKey) -> SharedString;

    /// The weekday headings, already in the host's first-day-of-week order.
    fn weekday_labels(&self) -> Vec<SharedString>;

    /// The whole day, as a field would show it.
    fn format_day(&self, day: Day) -> SharedString;

    /// What the grid draws in the day's own cell, usually its number.
    fn day_label(&self, day: Day) -> SharedString;

    /// Reads a typed date. The error is the message shown to the typist, word
    /// for word; the components never write one of their own.
    fn parse_day(&self, text: &str) -> Result<Day, SharedString>;

    /// The month `delta` months away, or `None` when the host will not go
    /// there.
    fn shift_month(&self, month: MonthKey, delta: i32) -> Option<MonthKey>;

    fn is_selectable(&self, day: Day) -> Selectability;

    /// Every day from `start` to `end` inclusive, or `None` when the host
    /// cannot enumerate them.
    ///
    /// A range picker uses this to name the blocked days inside a range. With
    /// `None` it says it could not check, rather than claiming a range is
    /// clear because it never looked.
    fn days_in(&self, start: Day, end: Day) -> Option<Vec<Day>>;

    fn clock(&self) -> Clock;

    /// The time as the host would write it.
    fn format_time(&self, time: TimeOfDay) -> SharedString;
}

/// The adapter as the components hold it.
pub type SharedDateAdapter = Rc<dyn DateAdapter>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_reports_the_day_it_carries_whichever_month_owns_it() {
        assert_eq!(MonthCell::Empty.day(), None);
        assert_eq!(MonthCell::Day(Day(3)).day(), Some(Day(3)));
        assert_eq!(MonthCell::Adjacent(Day(4)).day(), Some(Day(4)));
        assert!(MonthCell::Adjacent(Day(4)).is_adjacent());
        assert!(!MonthCell::Day(Day(4)).is_adjacent());
    }

    #[test]
    fn a_day_is_found_by_identity_rather_than_by_position() {
        let grid = MonthGrid::new([
            vec![MonthCell::Empty, MonthCell::Day(Day(1))],
            vec![MonthCell::Day(Day(2)), MonthCell::Adjacent(Day(3))],
        ]);
        assert_eq!(grid.position_of(Day(1)), Some((0, 1)));
        assert_eq!(grid.position_of(Day(3)), Some((1, 1)));
        assert_eq!(grid.position_of(Day(9)), None);
    }

    #[test]
    fn a_grid_of_nothing_but_blanks_is_empty() {
        assert!(MonthGrid::new([vec![MonthCell::Empty; 7]]).is_empty());
        assert!(!MonthGrid::new([vec![MonthCell::Day(Day(1))]]).is_empty());
    }
}
