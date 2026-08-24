# Deterministic performance testing

GPUI Box treats performance shape as a compatibility contract. A large data
set must not make a bounded viewport mount, lay out, paint, or publish every
item. The authority check is:

```bash
cargo run -p xtask -- performance check
```

It writes a machine-readable report to
`target/performance/report.json`. The report currently covers 10,000-item
fixtures for List, DataGrid, TreeGrid, CodeView, LogStream, and AgentDocument.
The command also renders a deliberately unbounded 10,000-child fixture and
fails if the budget does not reject it.

## What is counted

`Window::frame_stats()` returns the most recently completed `FrameStats` for
that window:

- reactive entity renders;
- element request-layout, prepaint, and paint calls;
- invalidations coalesced into the frame;
- product semantic nodes;
- native platform-view placements; and
- retained element-arena capacity growth when test allocator accounting is
  available.

`gpui-box-kit-testkit` adds `PerformanceBudget`, named typed limits, and a
serializable `PerformanceReport`. Component fixtures also report mounted rows
and caller-visible builder calls. An unavailable requested metric is a failure,
not a zero.

The counters are per window. Drawing another window cannot reset or contribute
to them. Shipping builds pay fixed integer increments only; retained allocator
growth is compiled only for tests and explicit test-support builds.

## Timing

Structural counts are the authority wherever they describe the cost. They are
portable across Metal, WARP, Linux software rendering, and browser builds and
do not become looser because a CI machine is slower.

Text shaping, rasterization, and end-to-end renderer submission still need
timing measurements. Those use Criterion warmup and medians, a same-process
calibration workload, and a documented noise margin. A raw millisecond
threshold from one runner is never accepted as a cross-platform budget. This
renderer-calibration lane is reported separately from the structural gate so a
timing regression cannot be hidden by changing a structural limit, or vice
versa.
