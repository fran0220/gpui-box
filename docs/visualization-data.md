# Calendar ticks and source-traceable transforms

These pure utilities prepare caller-owned data. They do not fetch, query,
guess a timezone, translate labels, or modify a chart's selection. Keep their
results with the immutable data revision rather than recomputing during paint.

## Calendar boundaries use an explicit timezone

`display::chart::scale::calendar` adds Gregorian calendar ticks without changing
`NumericScale::ticks`, whose Time mode still uses fixed durations. Pass a Chrono
`TimeZone` explicitly: UTC, a fixed offset, or a host-owned named timezone such
as one from chrono-tz. Kit does not ship or consult a production timezone
database and never reads the machine's clock or default timezone. Date pickers
continue using the independent host-owned `DateAdapter` contract.

`calendar_interval(scale, target)` selects a nominal second/minute/hour/day/week/
month/quarter/year density; `calendar_ticks(scale, zone, interval, limit)` resolves
actual wall-clock boundaries. Weeks begin Monday. Multiples align to 1970,
with weeks anchored to 1969-12-29. Explicit intervals avoid automatic selection.

Returned ticks carry integer UTC milliseconds, local civil time and offset
seconds. Use UTC milliseconds for `ChartTick::value` and format the local time
with caller wording. Include the offset when repeated local labels need to be
distinguishable. Both occurrences of an autumn repeated hour are retained;
nonexistent spring boundaries and skipped dates are omitted. Calendar days may
last 23 or 25 hours and months use their actual lengths. Reversed domains return
reversed instants. No endpoint tick is manufactured if it is not a boundary.

Invalid interval, non-Time scale, unrepresentable date range, excess output and
excess candidate work return typed errors, never a truncated successful list.
The bounded search examines at most one million local boundaries, including a
24-hour margin on each side for offset/date-line transitions. The supported
range is inside Chrono's date limits with room for that margin; exact sub-ms
endpoints are honored, but generated ticks have millisecond identity.

```rust
use chrono::FixedOffset;
use gpui_kit::display::chart::{
    cartesian::ChartTick,
    data::ChartValue,
    scale::{NumericScale, ScaleKind, calendar::*},
};

let scale = NumericScale::new(ScaleKind::Time, [0.0, 86_400_000.0]).unwrap();
let zone = FixedOffset::east_opt(5 * 3600 + 45 * 60).unwrap();
let ticks = calendar_ticks(scale, &zone,
    CalendarInterval { unit: CalendarUnit::Hour, step: 6 }, 16).unwrap();
let labels: Vec<ChartTick> = ticks.into_iter().map(|tick| ChartTick {
    value: ChartValue::Number(tick.timestamp_ms as f64),
    label: tick.local.format("%H:%M").to_string().into(),
}).collect();
// Pass labels to chart.x_ticks(labels). Selection still uses exact raw x.
assert!(!labels.is_empty());
```

## Aggregation is distinct from visual sampling

`display::chart::data::transform` supplies:

- `aggregate_by`: caller-provided group ID/coordinate; first-encounter ordering;
  inconsistent coordinates for one identity are rejected.
- `bin_points`: explicit finite increasing numeric-x edges; half-open bins with
  an inclusive last edge. All bins are emitted. Bin identities derive from the
  exact edges, so inserting an earlier bin does not rename existing bins.
- `rolling_points`: trailing observation-count windows in caller order. Each
  output retains the current source ID/x; missing current readings stay gaps.
  `min_observed` makes undersized windows missing rather than inventing zeroes.

Reducers include observed Count, Sum, Mean, Min, Max and R-7 Quantile. Missing
policy is explicit: Skip excludes absent readings, while Propagate makes any
group containing missing data missing. Empty/all-missing groups are missing
under Skip except Count, which reports zero observed readings. A bin counts y
observations, not rows with missing y. Invalid/nonfinite input and duplicate
source IDs are refused before grouping. Finite arithmetic overflow is an error;
sum/mean use scaled compensated floating arithmetic, not arbitrary precision.

`TransformedData` retains the original `Rc<Vec<RawPoint>>`. Every `DerivedPoint`
carries exact membership (`SourceRows`) and observed/missing counts, allowing
downstream drill-through to original IDs, values and labels after source reorder
or replacement. Generated readings do not inherit source error bars, baselines,
colors or formatted value text: those would falsely describe the aggregate.
The caller may format the derived value after computation.

Use ordinary iterator filtering/sorting before these operations. No wrapper is
needed around Rust predicates, and identity must remain the source business ID.
Path sampling remains a separate rendering approximation: it does not change
data, whereas aggregation intentionally creates new readings.

Group/bin membership is linear in input rows; rolling lineage uses compact
ranges rather than storing every overlapping membership. Current rolling
reduction is O(rows × width), with additional sorting for quantiles; it is not
an incremental streaming reducer. This cost remains explicit in the maturity
acceptance matrix rather than being hidden by bounded paint counts.

## Executed focused evidence

Tests cover DST gaps/repetition (including an interval whose local endpoints
appear reversed), leap February, Monday weeks, a skipped Samoa date, quarter-hour
offsets, fractional domain edges, limits, missing groups, bin boundaries and
stable bin identity, retained input revisions, finite extreme means/cancelling
sums, R-7 quartiles and rolling gaps. Integrated gate and dynamic chart acceptance
are recorded separately in `tasks/visualization-experience.md`.
