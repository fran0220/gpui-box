# Deterministic performance testing

GPUI Box treats performance shape as a compatibility contract. A large data
set must not make a bounded viewport mount, lay out, paint, or publish every
item. The authority check is:

```bash
cargo run -p xtask -- performance check
```

It writes a machine-readable report to
`target/performance/report.json`. The report covers 10,000-item fixtures for
List, DataGrid, TreeGrid, CodeView, LogStream, and AgentDocument. It also draws
a fully visible 64-node graph split between 32 zero-snapshot resting materials
and 32 promoted glass requests. That promoted set deliberately exceeds the
renderer admission limit, proving the bounded backdrop/fill-fallback path under
the same structural budget. The command additionally renders a deliberately
unbounded 10,000-child fixture and fails if the budget does not reject it.

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

For live diagnostics, `FrameTimingMonitor` holds a reference-counted frame
trace lease, filters observations to one `WindowId`, and retains a caller-sized
history. It observes only newly drawn frames that reached synchronous platform
submission. Its `FrameTimingSummary` derives rate from actual submission-end
timestamps, keeps mean and P95 draw duration, draw-budget overage,
invalidations, and dirty-to-draw latency, and adds submission-call,
dirty-to-submit, first-input-to-submit, and coalesced top-level-input
measurements. “Over budget” still describes time spent in `Window::draw`.
“Submitted” means the platform draw call returned; neither metric claims that
the compositor or display presented or dropped a frame.

The monitor never requests a redraw. A host decides when to poll it and whether
the application workload should produce another frame. Multiple monitors,
benchmarks, and a manual `set_frame_trace_enabled` owner can coexist: dropping
one lease cannot disable another owner or clear its active trace.
