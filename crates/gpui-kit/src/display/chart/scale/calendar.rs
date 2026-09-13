//! Calendar boundaries, not fixed-duration approximations. The caller supplies
//! a Chrono timezone and formats labels; no system timezone or clock is read.
//! This opt-in Gregorian utility does not change the host-owned DateAdapter.

#![doc = include_str!("../../../../../../docs/visualization-data.md")]

use super::{NumericScale, ScaleKind};
use chrono::{
    DateTime, Datelike, Duration, LocalResult, Months, NaiveDate, NaiveDateTime, Offset, TimeZone,
    Utc,
};

/// Wall-clock boundary unit. Weeks start on Monday; months/quarters/years start
/// on their first day. Multiples align to 1970 (weeks to 1969-12-29).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CalendarInterval {
    pub unit: CalendarUnit,
    pub step: u32,
}

/// A unique instant. Repeated wall-clock labels during a fall-back retain both
/// instants with different offsets. Nonexistent local boundaries are omitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarTick {
    pub timestamp_ms: i64,
    pub local: NaiveDateTime,
    pub offset_seconds: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarTickError {
    NotTimeScale,
    InvalidInterval,
    OutOfRange,
    LimitExceeded,
}

impl CalendarInterval {
    fn seconds(self) -> Option<i64> {
        let unit = match self.unit {
            CalendarUnit::Second => 1,
            CalendarUnit::Minute => 60,
            CalendarUnit::Hour => 3600,
            CalendarUnit::Day => 86400,
            CalendarUnit::Week => 604800,
            _ => return None,
        };
        Some(unit * i64::from(self.step))
    }

    fn months(self) -> Option<u32> {
        self.step.checked_mul(match self.unit {
            CalendarUnit::Month => 1,
            CalendarUnit::Quarter => 3,
            CalendarUnit::Year => 12,
            _ => return None,
        })
    }

    fn floor(self, local: NaiveDateTime) -> Option<NaiveDateTime> {
        if let Some(seconds) = self.seconds() {
            let origin = if self.unit == CalendarUnit::Week {
                -259200
            } else {
                0
            };
            let value = local.and_utc().timestamp();
            let aligned = (value - origin)
                .div_euclid(seconds)
                .checked_mul(seconds)?
                .checked_add(origin)?;
            DateTime::from_timestamp(aligned, 0).map(|dt| dt.naive_utc())
        } else {
            let step = i64::from(self.months()?);
            let value = i64::from(local.year() - 1970) * 12 + i64::from(local.month0());
            let aligned = value
                .div_euclid(step)
                .checked_mul(step)?
                .checked_add(1970 * 12)?;
            NaiveDate::from_ymd_opt(
                i32::try_from(aligned.div_euclid(12)).ok()?,
                u32::try_from(aligned.rem_euclid(12) + 1).ok()?,
                1,
            )?
            .and_hms_opt(0, 0, 0)
        }
    }

    fn next(self, local: NaiveDateTime) -> Option<NaiveDateTime> {
        if let Some(seconds) = self.seconds() {
            local.checked_add_signed(Duration::seconds(seconds))
        } else {
            local.checked_add_months(Months::new(self.months()?))
        }
    }
}

/// Select a nominal interval near the requested density. Actual ticks must be
/// generated in the chosen timezone with `calendar_ticks`; month lengths and
/// DST are never computed from these nominal durations.
pub fn calendar_interval(
    scale: NumericScale,
    target: usize,
) -> Result<CalendarInterval, CalendarTickError> {
    if scale.kind() != ScaleKind::Time {
        return Err(CalendarTickError::NotTimeScale);
    }
    let [a, b] = scale.domain();
    let desired = (b - a).abs() / target.max(1) as f64 / 1000.;
    let choices = [
        (CalendarUnit::Second, 1, 1.),
        (CalendarUnit::Second, 5, 5.),
        (CalendarUnit::Second, 15, 15.),
        (CalendarUnit::Second, 30, 30.),
        (CalendarUnit::Minute, 1, 60.),
        (CalendarUnit::Minute, 5, 300.),
        (CalendarUnit::Minute, 15, 900.),
        (CalendarUnit::Minute, 30, 1800.),
        (CalendarUnit::Hour, 1, 3600.),
        (CalendarUnit::Hour, 3, 10800.),
        (CalendarUnit::Hour, 6, 21600.),
        (CalendarUnit::Hour, 12, 43200.),
        (CalendarUnit::Day, 1, 86400.),
        (CalendarUnit::Week, 1, 604800.),
        (CalendarUnit::Month, 1, 2629800.),
        (CalendarUnit::Quarter, 1, 7889400.),
        (CalendarUnit::Year, 1, 31557600.),
    ];
    if desired <= 31557600.
        && let Some(&(unit, step, _)) = choices.iter().min_by(|a, b| {
            (a.2 / desired.max(1.))
                .ln()
                .abs()
                .total_cmp(&(b.2 / desired.max(1.)).ln().abs())
        })
    {
        return Ok(CalendarInterval { unit, step });
    }
    let years = (desired / 31557600.).ceil();
    if !years.is_finite() || years > f64::from(u32::MAX / 12) {
        return Err(CalendarTickError::OutOfRange);
    }
    Ok(CalendarInterval {
        unit: CalendarUnit::Year,
        step: years.max(1.) as u32,
    })
}

/// Generate in-domain ticks in display order, including both repeated local
/// boundaries and skipping nonexistent ones. Fractional-ms endpoints are not
/// rounded into the domain. `limit` rejects excess output rather than truncating.
/// Work is also capped at one million candidate boundaries. The UTC envelope
/// includes all possible Chrono offsets (strictly less than 24h), so historical
/// date-line changes and domains contained inside a repeated hour are covered.
pub fn calendar_ticks<Tz: TimeZone>(
    scale: NumericScale,
    zone: &Tz,
    interval: CalendarInterval,
    limit: usize,
) -> Result<Vec<CalendarTick>, CalendarTickError> {
    if scale.kind() != ScaleKind::Time {
        return Err(CalendarTickError::NotTimeScale);
    }
    if interval.step == 0 || (interval.seconds().is_none() && interval.months().is_none()) {
        return Err(CalendarTickError::InvalidInterval);
    }
    let [a, b] = scale.domain();
    let low = a.min(b);
    let high = a.max(b);
    let instant = |value: f64| {
        if value.abs() > 9_007_199_254_740_991. {
            return None;
        }
        DateTime::<Utc>::from_timestamp_millis(value as i64).map(|dt| dt.naive_utc())
    };
    let start = instant(low)
        .ok_or(CalendarTickError::OutOfRange)?
        .checked_sub_signed(Duration::days(1))
        .ok_or(CalendarTickError::OutOfRange)?;
    let end = instant(high)
        .ok_or(CalendarTickError::OutOfRange)?
        .checked_add_signed(Duration::days(1))
        .ok_or(CalendarTickError::OutOfRange)?;
    let mut cursor = interval.floor(start).ok_or(CalendarTickError::OutOfRange)?;
    let mut ticks = Vec::new();
    let mut candidates = 0usize;
    while cursor <= end {
        candidates += 1;
        if candidates > 1_000_000 {
            return Err(CalendarTickError::LimitExceeded);
        }
        let resolved = match zone.from_local_datetime(&cursor) {
            LocalResult::None => [None, None],
            LocalResult::Single(value) => [Some(value), None],
            LocalResult::Ambiguous(first, second) => [Some(first), Some(second)],
        };
        for value in resolved.into_iter().flatten() {
            let timestamp_ms = value.timestamp_millis();
            if (timestamp_ms as f64) < low || (timestamp_ms as f64) > high {
                continue;
            }
            if ticks.len() == limit {
                return Err(CalendarTickError::LimitExceeded);
            }
            ticks.push(CalendarTick {
                timestamp_ms,
                local: cursor,
                offset_seconds: value.offset().fix().local_minus_utc(),
            });
        }
        cursor = interval.next(cursor).ok_or(CalendarTickError::OutOfRange)?;
    }
    ticks.sort_by_key(|tick| tick.timestamp_ms);
    ticks.dedup_by_key(|tick| tick.timestamp_ms);
    if a > b {
        ticks.reverse();
    }
    Ok(ticks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    fn scale(a: &str, b: &str) -> NumericScale {
        let parse = |s| {
            DateTime::parse_from_rfc3339(s)
                .expect("valid test instant")
                .timestamp_millis() as f64
        };
        NumericScale::new(ScaleKind::Time, [parse(a), parse(b)]).expect("valid test time scale")
    }

    #[test]
    fn repeated_hour_and_spring_gap_keep_real_instants() {
        let hour = CalendarInterval {
            unit: CalendarUnit::Hour,
            step: 1,
        };
        let autumn = calendar_ticks(
            scale("2024-11-03T00:30:00-04:00", "2024-11-03T02:30:00-05:00"),
            &chrono_tz::America::New_York,
            hour,
            10,
        )
        .expect("fall ticks");
        assert_eq!(
            autumn
                .iter()
                .map(|t| (t.local.hour(), t.offset_seconds))
                .collect::<Vec<_>>(),
            [(1, -14400), (1, -18000), (2, -18000)]
        );
        assert_eq!(autumn[1].timestamp_ms - autumn[0].timestamp_ms, 3_600_000);
        let spring = calendar_ticks(
            scale("2024-03-10T00:30:00-05:00", "2024-03-10T04:30:00-04:00"),
            &chrono_tz::America::New_York,
            hour,
            10,
        )
        .expect("spring ticks");
        assert_eq!(
            spring.iter().map(|t| t.local.hour()).collect::<Vec<_>>(),
            [1, 3, 4]
        );
        let inner = calendar_ticks(
            scale("2024-11-03T01:45:00-04:00", "2024-11-03T01:15:00-05:00"),
            &chrono_tz::America::New_York,
            hour,
            10,
        )
        .expect("domain crosses repeated boundary");
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0].offset_seconds, -18000);
    }

    #[test]
    fn calendar_days_months_and_date_line_are_not_fixed_durations() {
        let daily = calendar_ticks(
            scale("2024-03-09T00:00:00-05:00", "2024-03-12T00:00:00-04:00"),
            &chrono_tz::America::New_York,
            CalendarInterval {
                unit: CalendarUnit::Day,
                step: 1,
            },
            10,
        )
        .expect("daily ticks");
        assert_eq!(
            daily
                .windows(2)
                .map(|w| (w[1].timestamp_ms - w[0].timestamp_ms) / 3_600_000)
                .collect::<Vec<_>>(),
            [24, 23, 24]
        );
        let months = calendar_ticks(
            scale("2024-03-01T00:00:00Z", "2024-01-01T00:00:00Z"),
            &Utc,
            CalendarInterval {
                unit: CalendarUnit::Month,
                step: 1,
            },
            10,
        )
        .expect("reversed leap months");
        assert_eq!(
            months.iter().map(|t| t.local.month()).collect::<Vec<_>>(),
            [3, 2, 1]
        );
        assert_eq!(
            (months[0].timestamp_ms - months[1].timestamp_ms) / 86_400_000,
            29
        );
        let samoa = calendar_ticks(
            scale("2011-12-29T00:00:00-10:00", "2011-12-31T00:00:00+14:00"),
            &chrono_tz::Pacific::Apia,
            CalendarInterval {
                unit: CalendarUnit::Day,
                step: 1,
            },
            10,
        )
        .expect("date-line skip");
        assert_eq!(
            samoa.iter().map(|t| t.local.day()).collect::<Vec<_>>(),
            [29, 31]
        );
    }

    #[test]
    fn fractional_bounds_limits_and_non_hour_offsets_are_explicit() {
        let minute = CalendarInterval {
            unit: CalendarUnit::Minute,
            step: 1,
        };
        let domain =
            NumericScale::new(ScaleKind::Time, [0.1, 120_000.]).expect("fractional domain");
        let zone = chrono::FixedOffset::east_opt(5 * 3600 + 45 * 60).expect("quarter-hour zone");
        let ticks = calendar_ticks(domain, &zone, minute, 2).expect("two ticks");
        assert_eq!(
            ticks.iter().map(|t| t.timestamp_ms).collect::<Vec<_>>(),
            [60_000, 120_000]
        );
        assert_eq!(ticks[0].local.hour(), 5);
        assert_eq!(ticks[0].local.minute(), 46);
        assert_eq!(
            calendar_ticks(domain, &zone, minute, 1),
            Err(CalendarTickError::LimitExceeded)
        );
        assert_eq!(
            calendar_ticks(domain, &zone, CalendarInterval { step: 0, ..minute }, 10),
            Err(CalendarTickError::InvalidInterval)
        );
        let weeks = calendar_ticks(
            scale("2024-01-01T00:00:00Z", "2024-01-15T00:00:00Z"),
            &Utc,
            CalendarInterval {
                unit: CalendarUnit::Week,
                step: 1,
            },
            10,
        )
        .expect("Monday weeks");
        assert_eq!(
            weeks.iter().map(|t| t.local.day()).collect::<Vec<_>>(),
            [1, 8, 15]
        );
        assert_eq!(
            calendar_interval(scale("2024-01-01T00:00:00Z", "2025-01-01T00:00:00Z"), 4)
                .expect("automatic interval")
                .unit,
            CalendarUnit::Quarter
        );
    }
}
