//! A reference calendar, for scenes and tests only.
//!
//! This is deliberately behind the `fixtures` feature, which is off by
//! default: a host that reached for it would be shipping a proleptic
//! Gregorian calendar with English labels and a pinned notion of today, which
//! is exactly the half-correct calendar this crate refuses to own. It exists
//! so the scenes photograph the same pixels twice and the tests can assert
//! behaviour against a calendar that never moves.

use gpui::SharedString;

use crate::datetime::adapter::{
    Clock, DateAdapter, Day, MonthCell, MonthGrid, MonthKey, Selectability, TimeOfDay,
};

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

const WEEKDAY_NAMES: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// Days from 0001-01-01 to 1970-01-01, which is where [`Day`] counts from.
const EPOCH_OFFSET: i64 = 719_162;

/// A fixture calendar: proleptic Gregorian, English labels, weeks starting on
/// Monday, and whatever "today" the test pinned.
#[derive(Debug, Clone)]
pub struct FixtureDateAdapter {
    today: Option<Day>,
    blocked: Vec<(Day, SharedString)>,
    twelve_hour: bool,
    /// The furthest the calendar will travel from January 1970, so a test can
    /// prove that a host may refuse a month.
    month_bounds: Option<(MonthKey, MonthKey)>,
}

impl Default for FixtureDateAdapter {
    fn default() -> Self {
        Self::without_today()
    }
}

impl FixtureDateAdapter {
    /// A calendar whose today is pinned, so nothing on screen depends on the
    /// machine's clock.
    pub fn pinned(year: i32, month: u32, day: u32) -> Self {
        Self {
            today: Some(day_of(year, month, day)),
            blocked: Vec::new(),
            twelve_hour: false,
            month_bounds: None,
        }
    }

    /// A calendar whose host has not established what day it is.
    pub fn without_today() -> Self {
        Self {
            today: None,
            blocked: Vec::new(),
            twelve_hour: false,
            month_bounds: None,
        }
    }

    pub fn blocking(mut self, year: i32, month: u32, day: u32, reason: &'static str) -> Self {
        self.blocked
            .push((day_of(year, month, day), SharedString::new_static(reason)));
        self
    }

    pub fn twelve_hour(mut self, twelve_hour: bool) -> Self {
        self.twelve_hour = twelve_hour;
        self
    }

    /// Refuses any month outside these bounds, inclusive.
    pub fn month_bounds(mut self, first: MonthKey, last: MonthKey) -> Self {
        self.month_bounds = Some((first, last));
        self
    }

    pub fn day(&self, year: i32, month: u32, day: u32) -> Day {
        day_of(year, month, day)
    }

    pub fn month(&self, year: i32, month: u32) -> MonthKey {
        month_key(year, month)
    }
}

impl DateAdapter for FixtureDateAdapter {
    fn today(&self) -> Option<Day> {
        self.today
    }

    fn month_of(&self, day: Day) -> MonthKey {
        let (year, month, _) = civil(day);
        month_key(year, month)
    }

    fn month_grid(&self, month: MonthKey) -> MonthGrid {
        let (year, index) = year_month(month);
        let first = day_of(year, index, 1);
        let length = month_length(year, index);
        // Monday is column zero; day zero of the epoch was a Thursday.
        let lead = (first.0 + 3).rem_euclid(7) as usize;

        let mut cells = Vec::new();
        for offset in 0..lead {
            cells.push(MonthCell::Adjacent(Day(first.0 - (lead - offset) as i64)));
        }
        for offset in 0..length {
            cells.push(MonthCell::Day(Day(first.0 + offset as i64)));
        }
        while cells.len() % 7 != 0 {
            let next = first.0 + cells.len() as i64 - lead as i64;
            cells.push(MonthCell::Adjacent(Day(next)));
        }

        MonthGrid::new(cells.chunks(7).map(<[MonthCell]>::to_vec))
    }

    fn month_label(&self, month: MonthKey) -> SharedString {
        let (year, index) = year_month(month);
        SharedString::from(format!("{} {year}", MONTH_NAMES[index as usize - 1]))
    }

    fn weekday_labels(&self) -> Vec<SharedString> {
        WEEKDAY_NAMES
            .iter()
            .map(|name| SharedString::new_static(name))
            .collect()
    }

    fn format_day(&self, day: Day) -> SharedString {
        let (year, month, date) = civil(day);
        SharedString::from(format!("{year:04}-{month:02}-{date:02}"))
    }

    fn day_label(&self, day: Day) -> SharedString {
        let (_, _, date) = civil(day);
        SharedString::from(format!("{date}"))
    }

    fn parse_day(&self, text: &str) -> Result<Day, SharedString> {
        let refusal = || {
            SharedString::new_static(
                "The fixture calendar reads dates as 2024-03-05 and nothing else.",
            )
        };
        let mut parts = text.trim().split('-');
        let (Some(year), Some(month), Some(date), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(refusal());
        };
        let year: i32 = year.parse().map_err(|_| refusal())?;
        let month: u32 = month.parse().map_err(|_| refusal())?;
        let date: u32 = date.parse().map_err(|_| refusal())?;
        if !(1..=12).contains(&month) || date < 1 || date > month_length(year, month) {
            return Err(refusal());
        }
        Ok(day_of(year, month, date))
    }

    fn shift_month(&self, month: MonthKey, delta: i32) -> Option<MonthKey> {
        let next = MonthKey(month.0 + delta as i64);
        match self.month_bounds {
            Some((first, last)) if next < first || next > last => None,
            _ => Some(next),
        }
    }

    fn is_selectable(&self, day: Day) -> Selectability {
        self.blocked
            .iter()
            .find(|(blocked, _)| *blocked == day)
            .map_or(Selectability::Selectable, |(_, reason)| {
                Selectability::Blocked {
                    reason: reason.clone(),
                }
            })
    }

    fn days_in(&self, start: Day, end: Day) -> Option<Vec<Day>> {
        if end < start {
            return None;
        }
        Some((start.0..=end.0).map(Day).collect())
    }

    fn clock(&self) -> Clock {
        Clock {
            hour_min: if self.twelve_hour { 1 } else { 0 },
            hour_max: if self.twelve_hour { 12 } else { 23 },
            minute_max: 59,
            second_max: 59,
            meridiem: self.twelve_hour.then(|| {
                (
                    SharedString::new_static("AM"),
                    SharedString::new_static("PM"),
                )
            }),
        }
    }

    fn format_time(&self, time: TimeOfDay) -> SharedString {
        let mut text = format!("{:02}:{:02}", time.hour, time.minute);
        if let Some(second) = time.second {
            text.push_str(&format!(":{second:02}"));
        }
        if let Some(meridiem) = time.meridiem
            && let Some(label) = self.clock().meridiem_label(meridiem)
        {
            text.push(' ');
            text.push_str(label.as_ref());
        }
        SharedString::from(text)
    }
}

fn month_key(year: i32, month: u32) -> MonthKey {
    MonthKey(year as i64 * 12 + month as i64 - 1)
}

fn year_month(month: MonthKey) -> (i32, u32) {
    (
        month.0.div_euclid(12) as i32,
        month.0.rem_euclid(12) as u32 + 1,
    )
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn month_length(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days from 0001-01-01 to the first of `year`.
fn days_before_year(year: i32) -> i64 {
    let previous = (year - 1) as i64;
    365 * previous + previous.div_euclid(4) - previous.div_euclid(100) + previous.div_euclid(400)
}

fn day_of_year(year: i32, month: u32, date: u32) -> i64 {
    (1..month)
        .map(|m| month_length(year, m) as i64)
        .sum::<i64>()
        + date as i64
}

fn day_of(year: i32, month: u32, date: u32) -> Day {
    Day(days_before_year(year) + day_of_year(year, month, date) - 1 - EPOCH_OFFSET)
}

/// The year, month, and date a [`Day`] stands for.
fn civil(day: Day) -> (i32, u32, u32) {
    let absolute = day.0 + EPOCH_OFFSET;
    let mut year = ((absolute as f64) / 365.2425).floor() as i32 + 1;
    while days_before_year(year) > absolute {
        year -= 1;
    }
    while days_before_year(year + 1) <= absolute {
        year += 1;
    }
    let mut remaining = absolute - days_before_year(year);
    let mut month = 1;
    while remaining >= month_length(year, month) as i64 {
        remaining -= month_length(year, month) as i64;
        month += 1;
    }
    (year, month, remaining as u32 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_is_day_zero_and_a_thursday() {
        assert_eq!(day_of(1970, 1, 1), Day(0));
        assert_eq!(civil(Day(0)), (1970, 1, 1));
        // Monday is column zero, and the epoch fell three columns later.
        assert_eq!((Day(0).0 + 3).rem_euclid(7), 3);
    }

    #[test]
    fn a_day_survives_a_round_trip_through_the_calendar() {
        for (year, month, date) in [
            (1970, 1, 1),
            (1999, 12, 31),
            (2000, 2, 29),
            (2024, 3, 5),
            (2100, 3, 1),
        ] {
            assert_eq!(civil(day_of(year, month, date)), (year, month, date));
        }
    }

    #[test]
    fn a_month_grid_is_whole_weeks_that_start_on_the_first() {
        let adapter = FixtureDateAdapter::pinned(2024, 3, 5);
        let grid = adapter.month_grid(month_key(2024, 3));
        assert!(grid.weeks.iter().all(|week| week.len() == 7));
        let first = adapter.day(2024, 3, 1);
        // 2024-03-01 was a Friday, so it sits in column four.
        assert_eq!(grid.position_of(first), Some((0, 4)));
    }

    #[test]
    fn an_unreadable_date_carries_the_hosts_own_message() {
        let adapter = FixtureDateAdapter::pinned(2024, 3, 5);
        assert_eq!(adapter.parse_day("2024-03-05"), Ok(adapter.day(2024, 3, 5)));
        assert!(adapter.parse_day("the fifth").is_err());
        assert!(adapter.parse_day("2024-02-30").is_err());
    }

    #[test]
    fn a_host_may_refuse_a_month() {
        let adapter = FixtureDateAdapter::pinned(2024, 3, 5)
            .month_bounds(month_key(2024, 3), month_key(2024, 4));
        assert_eq!(
            adapter.shift_month(month_key(2024, 3), 1),
            Some(month_key(2024, 4))
        );
        assert_eq!(adapter.shift_month(month_key(2024, 3), -1), None);
    }
}
