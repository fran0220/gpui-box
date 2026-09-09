# Native CHART adapter coverage

All 13 builders are actual native constructors using caller data. No fixture
data is imported by production code. Constructor arguments are descriptor id,
label and discriminated caller state, except ChartLegend(id, series).

| Component | Native options | Native events | Native slots |
|---|---|---|---|
| AreaChart | axes, polyline, crosshair, current | on_current → {seriesId,pointId} | empty/failed/loading |
| BarChart | axes | — | empty/failed/loading |
| CandlestickChart | body_width, rising_tint, falling_tint, current | on_current → candle id | — |
| ChartLegend | hidden | on_toggle → {id,hidden}, native bool unchanged | — |
| GaugeChart | constructor ChartState | — | empty/failed/loading |
| LineChart | axes, area, smooth, crosshair, current | on_current → {seriesId,pointId} | empty/failed/loading |
| PieChart | donut | — | empty/failed/loading |
| Plot | paint, current | on_current → mark id | — |
| RadarChart | constructor ChartState | — | empty/failed/loading |
| SankeyChart | current | on_current → native node.<id> or link.<id> | — |
| ScatterChart | axes, crosshair, current | on_current → {seriesId,pointId} | empty/failed/loading |
| Sparkline | tint, embedded, stale | — | empty/failed/loading |
| StackedBarChart | axes | — | empty/failed/loading |

There are no native entity commands. Two data queries dispatch real public
data APIs (not fabricated methods on the builders):

- Sparkline.published_points({}) builds caller SparklineReading and calls its
  native published_points(). It reports the native bounded-point count for
  ready/stale only; absent verified readings refuse rather than report zero.
- SankeyChart.layout({data,weights,nodeWidth,gap,alignment}) calls actual
  SankeyData.layout(); returns scale and bounded nodes/links. Cycles, invalid
  weight cardinality, nonpositive flow and invalid geometry return refusal.
  The native algorithm computes geometry; the adapter does not substitute a
  prearranged example. The query accepts caller data rather than native handles.

ChartLegend's native callback bool is the current hidden state before the
toggle, not the desired new hidden state. The wire preserves that native value;
the worker can apply its own controlled visibility policy.

ChartPoint/SparklinePoint, ChartSeries, ChartAxes, Candlestick, PlotMark,
SankeyNode/Link/Data and SparklineReading are converted directly into typed
native values. Loading, empty, unavailable, error, ready and stale remain
distinct. Stale contains last verified data plus refusal reason. IDs and
normalized geometry are caller-owned; invalid identities/ranges are refused.

Plot.paint is a bounded native painter adapter: rect, line and polygon data
are converted through PlotFrame's normalized coordinate transform into actual
native fill/path painting. It does not transport arbitrary native/JS closures,
images, text renderers, paths or URLs. This restriction is explicit, not a claim
of unrestricted painter coverage. Underlying native Plot mark traversal is
keyboard-driven, not an invented click action.

Native tests exercise all seven declared event routes (including Plot and
ChartLegend), both queries, asymmetric values/ranges/identity, all 13 builders
in both themes, and disabled legend refusal. JS tests exercise real factories,
typed callbacks, closed paint/state/layout schemas and exact membership.
Inspected Linux native captures include actual caller-data paint, OHLC, Sankey,
stale Sparkline, disabled legend and explicit error state in both themes.
Shared host lifecycle/query authority and central registration hooks are listed
in ../../display/fixture/COVERAGE.md; the combined gate remains parent-owned.
