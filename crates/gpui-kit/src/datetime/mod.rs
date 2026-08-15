//! Date and time selection over a calendar this crate does not own.
//!
//! There is no calendar system, no time zone database, no locale, and no
//! notion of today anywhere in this module. All of it arrives through
//! [`DateAdapter`], which the host implements over whatever date library it
//! already depends on. `docs/datetime.md` says what a host has to supply and
//! why the seam is here rather than lower down.
//!
//! - [`Calendar`] — a month grid, keyboard navigation, blocked days with the
//!   host's reasons, and a today ring only when there is a today.
//! - [`DateInput`] — a field with that calendar in a popover. Text the adapter
//!   will not read stays in the field and publishes invalid.
//! - [`RangePicker`] — two ends, where incomplete is a state and an end before
//!   a start is a report rather than a swap.
//! - [`TimeInput`] — hour, minute, and optionally second, over the host's own
//!   clock.

pub mod adapter;
pub mod calendar;
pub mod date_input;
#[cfg(feature = "fixtures")]
pub mod fixture;
pub mod range;
pub mod time_input;

pub use adapter::{
    Clock, DateAdapter, Day, MonthCell, MonthGrid, MonthKey, Selectability, SharedDateAdapter,
    TimeOfDay, installed_adapter, reset_date_adapter, set_date_adapter,
};
pub use calendar::{Calendar, CalendarEvent, DayMark};
pub use date_input::{DateInput, DateInputEvent};
pub use range::{BlockedDay, BlockedReport, DayRange, RangePicker, RangePickerEvent, RangeState};
pub use time_input::{Segment as TimeSegment, TimeInput, TimeInputEvent};
