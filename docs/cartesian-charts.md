# Raw Cartesian charts

The normalized `LineChart`, `AreaChart`, `BarChart`, `ScatterChart`,
`StackedBarChart`, `PieChart`, `RadarChart` and `GaugeChart` APIs are unchanged.
Use `display::chart::cartesian::CartesianChart` when Kit should compute a shared
coordinate system from raw observations rather than receive normalized points.

```rust
use gpui_kit::display::chart::{cartesian::CartesianChart, data::*, scale::*};

fn regional_reading() -> Result<CartesianChart, ScaleError> {
let points = vec![
    RawPoint::new("west", ChartValue::Number(12.0), Some(-7.5))
        .text("West", "−7.5 units"),
    RawPoint::new("east", ChartValue::Number(31.0), Some(23.0))
        .text("East", "23 units"),
];
let values = NumericScale::extent(
    ScaleKind::Linear, points.iter().flat_map(RawPoint::values), true,
)?;
let chart = CartesianChart::new(
    "regional-reading", "Regional reading",
    ChartScale::Numeric(NumericScale::new(ScaleKind::Linear, [0.0, 40.0])?),
    [ValueAxis { id: "units".into(), label: "Units".into(), scale: values }],
).series([RawSeries::new("reading", "units", SeriesMark::Bar).points(points)]);
Ok(chart)
}
```

## Coordinate contracts

- Values remain f64 until paint projection. `NumericScale::map` and `invert`
  extrapolate; the plot clips to its rectangle. Caller code can explicitly
  clamp a fraction before inversion. The exact two endpoints are preserved.
- Reversed domains reverse orientation. Constants expand by 5% (one unit at
  zero), or by a representable neighbor at subnormal limits. Log constants
  expand multiplicatively. Finite limits constrain expansion; an unrepresentable
  transformed span returns `UnrepresentableDomain` rather than nonfinite pixels.
- Empty extent input returns `Empty`; NaN/infinity return `NonFinite`. Logarithmic
  scales reject nonpositive domains and values. `None` is the only missing value.
- Time means UTC Unix milliseconds. Ticks use fixed durations, not local-calendar
  months, daylight-saving rules or automatic locale formatting. `format_ticks`
  receives the axis ID (`"x"` for the independent axis) and exact numeric tick.
- Categories retain caller order and business identity. Category inversion uses
  half-open bands with an inclusive final edge. Duplicate categories are errors.
- `RawPoint::text` supplies exact accessible/readout wording. A missing point's
  default formatted string is empty; it is not a built-in English label.

## Composition and controlled input

Series share one x scale and select a `ValueAxis` by ID. Lines/areas support
linear, step-before, step-after and monotone Hermite interpolation. Missing
samples break paths and omit marks. Monotone interpolation falls back to linear
for repeated/unordered projected x. Bar and area baselines are zero; log
bar/area baselines are rejected. `Stack::Absolute` separates positive/negative
totals; `Stack::Percent` normalizes each sign independently to 100%. Stacks key
source x values, never projected pixels or array positions.

`SeriesMark::Range` draws from `RawPoint::baseline` to `y`, in either direction;
equal endpoints draw a one-pixel body. Ranges cannot stack. Per-point colors
apply to bars/ranges/scatter; line strokes use the series color. `error([low,
high])` always means absolute axis readings, including on stacks. `values()`
includes both range endpoints and both error endpoints for extent inference.

References name a value-axis ID and line/range, with an exact label; their screen
direction follows the chart orientation.
Their semantic values and bar/range/error bounds use the visible clipped extents.
Point/shared-axis readouts retain current caller text; shared matching uses the
source coordinate, not rounded screen position.

`hovered`, `selected`, `hidden` and the x scale are controlled. `on_event` reports
requests only. Wheel zoom is pointer-anchored. Dragging proposes a numeric pan
on movement; Shift-drag shows a transient brush and reports raw endpoints on
release. Capture retains the initial geometry/domain across accepted or refused
redraws. Cancellation abandons the gesture. Arrow keys select visible readings;
Home requests reset and Escape clears selection/cancels capture. Link charts by
feeding their requests to one caller-owned state, as `cartesian-linked` does.

`.state(impl HasPhase)` accepts the existing shared `Phase`/`AsyncValue` contract.
Loading, Empty, Unavailable and Error render through `StateView` without plot
handlers. Stale errors and `.stale(reason)` keep the supplied verified series.

`PieChart::from_raw` validates and normalizes nonnegative shares without total
overflow. All-zero shares remain valid; missing/negative shares are rejected.
`RadarChart::from_raw` requires exactly one in-domain reading per explicit axis,
and orders by axis identity. `GaugeChart::from_raw` maps an optional raw reading
through an explicit domain; it rejects nonfinite/out-of-domain readings.

## Orientation, caller ticks and custom marks

`orientation(ChartOrientation::Horizontal)` rotates the coordinate interpretation:
independent x increases downward and value-axis readings increase rightward.
Every mark, reference, error whisker, hit box, arrow key, wheel anchor and brush
uses this same mapping. Reversed scale domains still reverse their own direction.
`x_axis_side` and `axis_side` place lanes on either edge. `AxisSide::Leading`
means left for a vertical lane and top for a horizontal lane; `Trailing` means
right/bottom. Defaults are bottom x/left values in vertical charts, left x/bottom
values in horizontal charts. Lane widths and collision suppression use measured
labels and the actual plot size.

`x_ticks` and `axis_ticks` take `ChartTick { value, label }`, replacing automatic
ticks without changing domains. They reject wrong coordinate types, nonfinite,
outside-domain and duplicate raw values (including signed zero). Distinct raw
values that project to the same fraction remain distinct; labels may be hidden
by measured collision suppression. An empty list hides ticks. These lists allow
caller-owned calendar ticks; `ScaleKind::Time` itself remains fixed-duration UTC.

`custom_marks` replaces scatter glyphs using `CustomMark::new(width, height,
painter)`. Dimensions must be finite and positive. The painter receives physical
pixel bounds and color, and is clipped to both those bounds and the plot. Hit and
semantic bounds use that same clipped rectangle. Return `None` for the standard
glyph. Overlay a scatter series to compose custom annotations with other marks.

## Keyed geometry, style and presence

Raw geometry updates animate by series/point business identity, including reordered
inputs and interrupted updates. Only projected f64 geometry interpolates; raw
values and accessible/readout text switch atomically to current caller input.
Reduced motion settles immediately; `.animate(false)` disables transitions.
Viewport/domain and mark topology changes snap, preserving direct manipulation.
Grouped bar slots animate by series identity, including reordered input. New
points enter at their true readings with opacity, never a fabricated zero value.
Removed/hidden points immediately retire their input and semantic authority while
paint-only layers fade out. Reentry reverses an unfinished exit continuously.
Series and explicit point colors interpolate through existing motion primitives;
line strokes retain series-level color policy. Custom painters receive animated
color and must honor its alpha. Arbitrary custom painter internals are not morphed.

`.motion(CartesianMotion { enter, update, exit })` accepts existing `MotionSpec`
values. Defaults resolve Entrance, Resize and Exit theme roles. Changing the
coordinate system or timing policy clears obsolete exit geometry. Settled
redraws retain immutable projection, hit-index and style ownership; only active
transition clocks are sampled. Active geometry still clones the projected graph,
and revision reconciliation scans source keys: this is not full virtualization.

Point/shared-axis tooltip surfaces float through the generic anchored overlay,
bounded to the viewport. `.tooltip_content` receives exact current raw rows for
caller-built rich content. `.floating_tooltip(false)` hides only that surface;
the persistent accessible readout remains. Removed/hidden/offscreen anchors do
not retain a stale tooltip. A tooltip may cover nearby marks; it does not reserve
plot layout space or change source hit geometry.

`.emphasized(Some(series_id))` dims other series using theme opacity and the
configured update timing. Legend pointer entry/exit emits `Emphasis` proposals;
the host accepts them explicitly. Dimmed series retain their hit geometry,
selection, raw values and visibility. Unknown or hidden emphasis identities
restore normal styling. `ChartLegend::on_emphasis` exposes the same identity
proposal for standalone legends; existing click/keyboard hide/show is unchanged.

## Persistent selection and overview

`.range(CartesianRange::new(domain, value, label))` adds a dedicated horizontal
strip for numeric, logarithmic or Unix-ms selections. Its domain is independent
of the plot viewport and never follows viewport zoom implicitly. The strip
remains horizontal even for a horizontal main chart. A decorative linear trace
shows the same raw series' y readings; it is not a second interactive mark layer
or a miniature reproduction of bar/range/error glyphs. Set `overview: false` for
selection alone. Overview projection and normalized paths are retained by input
revision/domain/width; `PathSampling::MinMax` also reduces these linear traces.
Missing observations break overview paths. Raw inputs are never resampled.

`CartesianEvent::Range(RangeEvent)` exposes Update, Commit and Cancel plus intent
and logical handle. The caller accepts proposals into `CartesianRange::value`
and may derive one or more linked viewports, as `cartesian-linked` demonstrates.
No accepted input is inferred from an unchanged redraw. The strip displays a
localized transient preview until release; a refused release restores the last
caller value. Cancelling does not roll back earlier accepted updates. External
selection or overview-domain replacement cancels an active draft. Delayed older
proposals count as external replacements under the shared synchronous/latest
acceptance contract.

Drag outside the selection to create, Shift-drag to replace, drag inside to move,
or grab a handle to resize. Capture preserves initial measured bounds for outside
release. Logical Start/End remain ascending raw endpoints on descending domains;
crossing clamps without swapping identity. Move preserves projected width (not
raw width on logarithmic scales). Arrow keys edit a focused handle/window by 1%
of display span; Home/End reach a bound, Escape cancels. Unedited endpoints retain
their exact f64 bits. Invalid/out-of-domain caller ranges are not normalized:
editing is rejected. `enabled: false` installs no handlers, cancels capture and
marks endpoint semantics disabled; arbitrary unmount uses framework cancellation.
Caller tick formatting supplies endpoint text; exact raw values remain separate
in events and endpoint semantics. Empty, committed and preview readouts use the
shared language pack, not time-specific labels.

## Shared data, sampling and measured performance

`.shared_series(Rc<Vec<RawSeries>>)` retains caller-owned immutable input across
redraws. Use a replacement Rc or `Rc::make_mut` for updates. The original
`.series(iter)` remains supported. Projection caches both success and rejection
by that revision, x scale, complete value axes and hidden IDs; hidden data is
still validated. Motion runs after cached projection and cannot mutate it.
Readout text is formatted only when used, not allocated for every offscreen point.

`.sampling(PathSampling::MinMax)` reduces ordered linear paths to pixel-column
endpoints and extrema of both value and area baseline. Reversed ordered x works;
missing runs stay separate. Curves, unordered x and the default `Exact` mode
retain all segments. This is an explicit visual approximation, not data
aggregation: point glyphs, semantic targets, selection IDs and values remain
original observations. A column can retain up to six vertices per continuous
run, plus offscreen tail buckets. Many gaps can still produce many runs.

Pointer lookup uses an exact rectangle tree over final painted mark extents,
including custom sizes and animated geometry. Ties retain original source order.
Overlapping rectangles can still require a full scan. Keyboard navigation,
semantic publication, hit construction and enabled motion remain O(n); hiding
offscreen glyph painting is not full chart virtualization.

Run `cargo run -p gpui-box-performance -- --charts --output
target/performance/charts.json` for separate input/projection, mount, static
redraw, append, selection, hover and accepted viewport measurements. It covers
1k/10k/100k sparse inputs (24 visible marks), full-domain 1k, and motion on/off.
The 2026-09-13 Linux orb run reduced 100k sparse static redraw allocations from
3,001,309 to 1,917 with shared input and motion disabled (1,937 enabled). Advisory
CPU redraw times were 26.6 ms disabled versus 175.3 ms enabled. Dense 1k still
mounted 1,000 targets and took about 53–55 ms. These are test-platform CPU
measurements, not native GPU/FPS guarantees; elapsed time is not a test budget.

The extended workload explicitly disables reduced motion for 64ms value-update
and interruption samples and asserts intermediate geometry plus exact new raw
text. The 100k sparse motion-on value-only retarget fell from 523ms to 315ms by
avoiding redundant lifecycle hash reconciliation; the active frame still takes
about 34ms, while mount/revision work and hit rebuilding remain full-source.
Overview adds separate projection/path/paint costs. Fixture cases use isolated
TestAppContexts and explicitly remove their windows. These are advisory Linux
CPU observations, not bounded-work or native-FPS claims.

This is native raw-data composition, not Recharts/ECharts API parity. Automatic
calendar ticks are not supplied by the fixed-duration Time scale. Exact simulated playback
exercises the actual Cartesian renderer; native-window motion and macOS/Windows
evidence remain pending. The legacy normalized builders retain their APIs.

Exhibits: `cartesian` (mixed units, diverging/percentage stacks, gaps, stale
readings), `cartesian-linked` (wide/narrow shared time state and intervals),
`cartesian-layout` (opposite edges, horizontal ranges, custom glyphs, click to
update/reorder raw readings),
`cartesian-lifecycle` (configurable value/color, hide/show, removal/reentry,
grouped reorder and direct viewport transitions),
`cartesian-dense` (exact/sampled wide/narrow paths, spikes, gaps and raw selection),
`cartesian-states` (raw polar readings and distinct non-ready states).
