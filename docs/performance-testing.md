# Deterministic performance testing

GPUI Box treats performance shape as a compatibility contract. A large data
set must not make a bounded viewport mount, lay out, paint, or publish every
item. The authority check is:

```bash
cargo run -p xtask -- performance check
```

It writes a machine-readable report to
`target/performance/report.json`. Each rendering fixture is warmed for two
explicit redraws and measured on the third. The report covers 1,000- and 10,000-item fixtures with identical viewport budgets for
List, DataGrid, TreeGrid, CodeView, LogStream, and AgentDocument. It also draws
a fully visible 64-node graph split between 32 zero-snapshot resting materials
and 32 promoted glass requests. That promoted set deliberately exceeds the
renderer admission limit, proving the bounded backdrop/fill-fallback path under
the same structural budget. A static theme-and-semantics fixture publishes 128
Buttons and Badges on palette-backed presentation tiers so render-path theme
resolution and diagnostic semantics have a heap-allocation ratchet. An idle
fixture renders twice, then only drains already-scheduled work; it fails if the
window's frame index advances without an invalidation. The command additionally renders a deliberately
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

The performance-check binary alone installs a counting global allocator. It
counts allocation and reallocation calls strictly around the measured redraw;
the libraries and shipping applications keep their normal allocator. Every
fixture carries its own allocation ceiling in the serialized report so later
work can lower the relevant ratchet when it removes render-path allocation.
The initial ceilings are the repeatable third-frame value plus 10%, rounded up;
zero remains a strict zero.

`gpui-box-kit-testkit` adds `PerformanceBudget`, named typed limits, and a
serializable `PerformanceReport`. Component fixtures also report mounted rows
and caller-visible builder calls. An unavailable requested metric is a failure,
not a zero.

Schema 3 also reports `caller_input_conversions` and
`has_row_builder_callback`. CodeView, LogStream and AgentDocument take eager
owned collections, not lazy row callbacks: their builder count is genuinely
zero, **not a proxy for mounted rows**. Fixture strings are prepared before
measurement; each clone/conversion needed to feed those APIs is counted
separately and remains dataset-sized. Their current APIs cannot make total
input preparation viewport-bounded. The fixed mount/layout/prepaint/paint and
semantic ceilings are enforced at both dataset sizes; heap ceilings still
include collection conversion and component work. This proves bounded visible
work at the tested sizes, not constant-time ingestion of arbitrary datasets.

The counters are per window. Drawing another window cannot reset or contribute
to them. Shipping builds pay fixed integer increments only; retained allocator
growth is compiled only for tests and explicit test-support builds.

## Timing

Structural counts are the authority wherever they describe the cost. They are
portable across Metal, WARP, Linux software rendering, and browser builds and
do not become looser because a CI machine is slower.

Text shaping, rasterization, and end-to-end renderer submission still need
timing measurements. Acceptance requires Criterion warmup and medians, a same-process
calibration workload, and a documented noise margin. A raw millisecond
threshold from one runner is never accepted as a cross-platform budget. This
renderer-calibration lane is reported separately from the structural gate so a
timing regression cannot be hidden by changing a structural limit, or vice
versa.

`.github/workflows/performance-timing.yml` runs the existing
`gpui-box-wgpu` **CPU shaping** workloads on manual dispatch, with required
`baseline_ref` input. It now blocks that dispatched job on a confident
calibrated regression or invalid/missing evidence. It runs on no push or
schedule. Structural and heap-allocation budgets remain a separate root gate;
the root gate also tests the timing comparator, but does not run noisy timing.

### Calibrated shaping acceptance

From a clean, committed, non-shallow checkout with dependencies cached:

```bash
python3 -B tools/performance-check/timing.py compare \
  --baseline-ref <reviewed-commit> --output target/timing/review
python3 -B -m unittest discover -s tools/performance-check/tests -v
```

The output directory must be new. There is no implicit previous baseline and
no checked-in machine-speed reference. The operator chooses a reviewed source
revision; the report resolves it and the candidate (default `HEAD`) to exact
commits. Both are measured on the same host in this execution. To exercise the
unchanged control, explicitly pass `--baseline-ref HEAD`.

The runner extracts both revisions with `git archive` into temporary source
directories, then installs the **candidate's identical benchmark and embedded
Lilex/IBM Plex font bytes into both archives**. Library code and lockfiles stay
revision-specific; the workload is the common candidate harness, not the old
revision's benchmark definition. An incompatible baseline API/build or
Criterion package is refused, never silently replaced with the candidate.
Both builds use the current toolchain and the same optimized bench profile.
Sources are removed even on failure; separate reusable build caches remain in
`target/timing-build/{baseline,candidate}`. Builds are locked and offline: CI
explicitly fetches candidate dependencies first; baseline-only dependencies
must already be cached or the run fails closed. The runner never fetches refs
or modifies the checkout. Fetch/unshallow and dependency preparation are
explicit operator actions before measuring.

Each fresh benchmark process runs deterministic black-box integer arithmetic
independent of GPUI, followed by the three existing CosmicText `layout_line`
workloads: no fallback, ASCII with fallback, and mixed-direction paragraphs.
No system fonts are loaded. Each workload and calibration receives 0.5 seconds
warmup, 1 second target measurement, 30 Criterion samples, and a bootstrapped
95% median confidence interval (100,000 resamples). Both builds complete before
sampling. Three unchanged baseline processes run first, then three candidate
processes; `--repeats N` may increase, but never decrease, that minimum.

For workload median interval `[L, U]`, median `M`, and same-process calibration
`[cL, cU]`, median `cM`, the normalized interval is `[L/cU, U/cL]` and point is
`M/cM`. Uniform CPU speed changes normalize away. This is conservative interval
arithmetic, **not a claim of joint 95% coverage** or calibration of GPU work.
For each workload, let `d` be the maximum minus minimum normalized baseline
median across unchanged repeats. Its acceptance envelope is the hull of all
baseline normalized intervals expanded by `d` on each side (lower bound clamped
to zero). Thus the margin is measured baseline noise, not an arbitrary percent
or absolute-millisecond threshold; candidate noise cannot widen the baseline.

- `regression`: every candidate interval is wholly above the envelope; fails.
- `improvement`: every candidate interval is wholly below the envelope.
- `within_noise`: all candidate intervals fit inside the envelope.
- `inconclusive`: all other cases, including overlaps and inconsistent repeats.

The last three do not fail timing acceptance, but **neither within-noise nor
inconclusive proves equivalence**. Three baseline repeats are a small empirical
noise estimate, not a bound on future drift; a noisy host can hide real changes.
Review intervals and repeat on a quiet host when inconclusive. Sequential CPU
calibration does not cancel workload-specific scheduling, thermal or frequency
effects. No sensitivity to small regressions is promised. Missing calibration,
workloads, nonfinite/nonpositive values, inconsistent corpus/session/config,
malformed intervals, or inadequate samples/repeats fail rather than pass.

`report.json` records source commits, exact benchmark and font SHA-256 hashes,
runner hash, lockfile hashes, toolchain/profile, host CPU/OS and CPU-only backend,
raw median confidence intervals, normalized intervals, measured noise margins,
envelopes and verdicts. Raw Criterion samples and process logs accompany it;
CI uploads evidence even after failure. Synthetic tests prove uniform-speed
normalization, true 2x slowdown rejection, noisy overlap handling and invalid
data rejection; they are not measured baselines.

**Unsupported timing coverage:** rasterization, renderer submission, GPU
execution, Metal/WARP timing, browser frame latency and compositor/display
presentation. These need their own workload and calibration designs. This CPU
shaping lane does not close those renderer-latency gaps or replace native visual
and structural validation.

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
