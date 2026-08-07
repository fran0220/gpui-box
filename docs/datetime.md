# Date and time

This crate has no calendar system, no time-zone database, no locale, and no
notion of what day it is. `Calendar`, `DateInput`, `RangePicker`, and
`TimeInput` draw dates and read them, and every fact they draw arrives through
one trait the host implements: `gpui_kit::datetime::DateAdapter`.

## Why the seam is here

A date picker that owns its own calendar owns all of it. It has to know how
long February is in a year nobody has told it about, which day a week starts
on where the typist lives, what the month is called in their language, whether
the hour that just went missing to daylight saving is selectable, and what
"today" means for somebody whose device clock is wrong. None of those are UI
questions, and every one of them is answered correctly only by the date
library, locale data, and clock the application already has.

So the components refuse to answer any of them. `Day` and `MonthKey` are
opaque integers minted by the adapter; the components carry them, compare
them, and hand them back. Nothing in this crate ever adds a day to a date or a
month to a month — moving to the next month is `shift_month`, an adapter call,
not an addition. A `Day` from one adapter means nothing to another.

`docs/coverage.md` used to list date pickers as out of scope for exactly this
reason. What changed is not the reasoning; it is that the reasoning has been
turned into a seam, so the crate can ship the picker without shipping the
calendar.

## What a host implements

```rust
pub trait DateAdapter {
    fn today(&self) -> Option<Day>;
    fn month_of(&self, day: Day) -> MonthKey;
    fn month_grid(&self, month: MonthKey) -> MonthGrid;
    fn month_label(&self, month: MonthKey) -> SharedString;
    fn weekday_labels(&self) -> Vec<SharedString>;
    fn format_day(&self, day: Day) -> SharedString;
    fn day_label(&self, day: Day) -> SharedString;
    fn parse_day(&self, text: &str) -> Result<Day, SharedString>;
    fn shift_month(&self, month: MonthKey, delta: i32) -> Option<MonthKey>;
    fn is_selectable(&self, day: Day) -> Selectability;
    fn days_in(&self, start: Day, end: Day) -> Option<Vec<Day>>;
    fn clock(&self) -> Clock;
    fn format_time(&self, time: TimeOfDay) -> SharedString;
}

pub type SharedDateAdapter = Rc<dyn DateAdapter>;
```

Method by method, and what each return value is taken to mean:

| Method | Returning this means |
|---|---|
| `today` | `Some(day)` — that day gets the today ring, and a calendar with nothing else to go on opens on its month. `None` — **the host has not established a today**. No ring is drawn, no month is inferred from it, and no clock is consulted behind the host's back. |
| `month_of` | The month that holds a day. Used to open a calendar on the month of the current selection. |
| `month_grid` | The month laid out in weeks. Every week must hold as many cells as `weekday_labels` returns, in the same order, because the header and the body are drawn as one table. |
| `month_label` | The month's name, already in the host's language. The crate carries no month name in any language. |
| `weekday_labels` | The weekday headings, already in the host's first-day-of-week order. Whether a week starts on Monday, Sunday, or Saturday is decided entirely by the order of this vector and of the cells in `month_grid`. |
| `format_day` | The whole day as a field would show it. This is what `DateInput` writes into its text field and what `RangePicker` puts in its summary. |
| `day_label` | What the grid draws inside the day's own cell, usually its number. |
| `parse_day` | `Ok(day)` — the text was read. `Err(message)` — the text was not read, and `message` is the sentence shown to the typist **word for word**. The components never author one. |
| `shift_month` | `Some(month)` — that is the month `delta` months away. `None` — **the host will not go there**, and navigation stops rather than travelling somewhere the host does not cover. |
| `is_selectable` | `Selectable`, or `Blocked { reason }` where the reason is the host's own wording, shown verbatim in a tooltip and published as the cell's `value`. |
| `days_in` | `Some(days)` — every day from `start` to `end` inclusive. `None` — **the host cannot enumerate them**, so a range picker says it could not check rather than claiming a range is clear because it never looked. |
| `clock` | How far the hours, minutes, and seconds run, and the two meridiem labels when the host's clock has them. `Clock::meridiem` is `None` for a twenty-four-hour clock. |
| `format_time` | The time as the host would write it. This is what `TimeInput` publishes as its `value`. |

`Day` is ordered, and the ordering is part of the contract: an adapter must
number days so that an earlier day compares less than a later one. A range
picker has to be able to say that an end comes before its start.

`MonthCell` is `Empty`, `Day(day)`, or `Adjacent(day)`. An adapter that pads
the first week with blanks uses `Empty`; one that shows the neighbouring
month's days greyed uses `Adjacent`. Both render; only `Day` and `Adjacent`
carry a day the keyboard can reach.

`Selectability::Blocked` is a refusal, not a disabled style. A blocked cell
installs no click handler at all, and it states the host's reason.

## What each component does when the adapter knows nothing

The adapter is allowed to answer "I don't know" to three questions, and each
one has a rendered consequence rather than a guess.

**No `today`.** `Calendar` resolves the month it shows in this order:
navigation, then the caller's `.month(..)`, then the first selected day, then
today. With none of those, it draws an `EmptyState` of kind `Unavailable`
saying it does not know which month to show and why — nothing selected, no
today, no month given — instead of opening on a month it picked itself. It
draws no today ring, and its container node publishes `month unknown` as its
`value`. `DateInput` and `RangePicker` inherit this, since both are built on
`Calendar`.

**A refused `shift_month`.** Navigation stops. The month on screen does not
change, no `MonthShown` event is emitted, and the keyboard's cursor stays where
it was. This applies equally to the header arrows, page up and page down, and a
day step that walks off the edge of the grid: all four go through
`shift_month`, so a host that bounds its calendar bounds every route into the
neighbouring month at once. The arrows are not separately dimmed for this —
they are dimmed only when there is no month at all to move from — so a bounded
calendar refuses by not moving rather than by pretending the control was never
there.

**A `parse_day` error.** The typed text stays exactly where the typist left it.
`DateInput` publishes itself `invalid`, shows the adapter's message verbatim
under the field as a `Status` node, and emits `DateInputEvent::Unparsable`
carrying both the text and the message. It does not clear the field, does not
revert to the last good day, and does not correct what was typed. This is the
same rule `NumberInput` keeps, for the same reason: silently rewriting what
somebody typed hides the disagreement instead of reporting it. Emptying the
field clears the message and reports nothing — an empty field is not a wrong
answer.

**A `days_in` of `None`.** `RangePicker` publishes `BlockedReport::Unchecked`
and says on screen that the host cannot list the days in this range, so none of
them were checked. `Unchecked` and `Clear` are separate answers, and the picker
never renders the first as the second.

## What the components own

The same rule as everywhere else in this library: the answer belongs to the
caller.

- `Calendar` owns the month it is looking at, where the keyboard cursor is, and
  what the pointer is over. It reports `Picked`, `MonthShown`, and `Hovered`.
  The selection is the caller's; picking a day moves nothing.
- `DateInput` owns whether its popover is open and what is in its text field
  mid-edit. The day is the caller's.
- `RangePicker` owns nothing beyond the calendar underneath it. `Unset`,
  `Incomplete`, `Complete`, and `Inverted` are four states, not three states
  and an error: an end before a start is reported as `end before start` and
  drawn as given, never quietly swapped, because swapping decides on the
  host's behalf that the typist made a mistake rather than meant it.
- `TimeInput` owns which segment the keyboard is on and the digits typed into
  it since it was entered. A segment steps within the clock's bounds and stops
  there rather than rolling over into a neighbour the typist did not point at.

## The fixture calendar

`gpui_kit::datetime::fixture::FixtureDateAdapter` is a proleptic Gregorian
calendar with English month and weekday names, weeks starting on Monday, and a
pinned today.

It is behind the `fixtures` cargo feature, which is **off by default**, and
that is the point. A host that reached for it would be shipping exactly the
half-correct calendar this crate refuses to own — right for one locale, one
week convention, and one calendar system, and quietly wrong everywhere else.
The feature exists so that:

- the scenes `calendar`, `date-range`, and `date-time` photograph the same
  pixels on every run, because their today is pinned rather than read from the
  machine's clock;
- `crates/gpui-kit/tests/datetime.rs` can assert behaviour against a calendar
  that never moves;
- the gallery can show the components at all.

The gallery, `xtask`, and the crate's own dev-dependencies turn the feature on.
Nothing on the default component path can reach it. If you are writing a host,
implement `DateAdapter` over the date library you already depend on; the
fixture is a test artifact, not a default to lean on.

## Semantics

| Node | Role | `value` |
|---|---|---|
| the calendar | `Group` | `month unknown` when no month resolved; otherwise the month label is the node's `text` |
| `<calendar>.grid` | `Group` | — |
| `<calendar>.day-<day>` | `Option` | the block reason when blocked, else the host's mark label when marked |
| `<calendar>.today` | `Status` | — (`text` is the formatted day) |
| the date field | `Input` | the text as it stands, which mid-edit is not the host's value |
| `<field>.message` | `Status` | — (`text` is the adapter's refusal) |
| the range picker | `Group` | `unset`, `incomplete`, `complete`, or `end before start` |
| `<picker>.blocked` | `Status` | `unchecked` |
| `<picker>.blocked-<day>` | `Status` | the host's reason for that day |
| the time field | `Group` | `format_time` of the current value |
| `<time>.hour`, `.minute`, `.second`, `.meridiem` | `Input` | the segment's text, with the clock's bounds as its range |

Day cell ids are `day-<Day.0>` — the adapter's own number for the day, which is
business identity, not a position in the grid. The same day keeps the same id
after the month scrolls, after a filter, and after a re-layout.
