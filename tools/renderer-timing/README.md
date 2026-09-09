# Offscreen renderer timing authority

This is a separate, opt-in timing lane, not the structural performance gate or
the CPU shaping lane. Its scope is two deterministic quad workloads rendered
at 640 × 480 through Linux software Vulkan, Windows wgpu/WARP, or native Metal.
It does not establish complete renderer/component coverage, native Windows
Direct3D timing, application startup, input-to-display or presentation latency.

## Boundaries and capability

`PlatformHeadlessRenderer::measure_scene` waits only when explicitly called.
Normal `render_scene` remains asynchronous. Each attempt owns fresh wgpu query
storage or a fresh Metal command buffer. Errors return no sample; failed
attempts consume an ID and cannot return a previous result.

The raw record separates:

- renderer initialization (device, atlas and renderer construction, not process
  or application startup);
- each workload's first frame, outside steady-state samples;
- CPU target preparation, encoding and submission return;
- CPU wall time from submission return until completion is observed, including
  poll/scheduling overhead (Metal polls at 100 µs, not a precision GPU clock);
- GPU execution: wgpu encoder timestamp writes bracket render commands, before
  query resolve/copy; Metal's completed command buffer supplies GPUStartTime and
  GPUEndTime. This excludes CPU scene construction and wgpu uploads queued before
  the encoder. It is not an entire application's GPU utilization;
- wgpu query resolve/copy/map/extraction after render completion; native Metal
  reads command-buffer properties, so query readback is not applicable;
- a **separate capture's total** render/wait/pixel-transfer/CPU-copy duration.
  The existing image API cannot isolate pixel readback alone. No field presents
  that combined wall timer as GPU execution or pixel-readback-only time.

The measured paths retain their existing target policy: wgpu allocates a new
offscreen target each frame; Metal reuses one at the same size. CPU preparation
includes that cost. Cross-backend comparison would not isolate the same work
and is rejected, rather than treating these values as portable frame budgets.

GPU timestamps require encoder timestamp features on wgpu, or usable Metal
command-buffer GPU clock properties. Unsupported evidence carries a reason,
never a numeric zero. Unordered/nonfinite/zero-length timestamp intervals,
wgpu mapping/device/validation errors and failed Metal command buffers fail the
attempt. The timed path bounds completion waits to 30 seconds; each captured
process additionally has a 180-second limit. Headless completion is not OS
presentation evidence. The existing profiler's input-to-submission name and
meaning remain unchanged.

## Renderer-specific statistical policy

Policy `renderer-raw-envelope-v1` is the authority for these offscreen workloads.
It deliberately differs from the Criterion CPU shaping calibration policy:
GPU work cannot be calibrated by CPU arithmetic, and dividing 1024-quad time by
32-quad time would hide a uniform regression in the renderer under test. **No
normalization is performed.** Both workloads are independently compared against
the same workload in the reviewed revision, on the exact same host, renderer
class, adapter, driver, compiler and profile, with identical harness bytes.
Software WARP/llvmpipe evidence cannot be compared to native Metal/hardware or
to a different adapter. There is no stored millisecond budget.

Each workload has at least 0.5 seconds and eight completed frames of warmup
(maximum 4096); actual warmup counts/time are retained. Each process records 30
single-frame samples, each owning its completed submission. Unlike Criterion's
adaptive iteration batches, this fixed sample unit preserves per-submission
ownership and keeps total rendering bounded on slow software adapters. Samples
are not called Criterion output; there is no 1-second adaptive measurement
target. A deterministic percentile bootstrap computes the median's 95% interval
using 100,000 resamples. Three to ten fresh processes per revision supply the
independent repeats. Initialization and first-frame observations are retained
for diagnosis, not given an underpowered startup acceptance verdict.

For each workload and metric, the baseline envelope is the hull of its baseline
median intervals, expanded on each side by the range of baseline medians (lower
bound clamped to zero). This reuses the existing measured-noise rule without its
CPU calibration ratio. Candidate noise never widens the baseline envelope:

- all candidate intervals above it: regression;
- all below it: improvement;
- all inside it: within_noise;
- otherwise: inconclusive.

Confident regressions and unavailable requested metrics fail the command.
Malformed/missing evidence also fails. As in the CPU shaping policy, inconclusive
does not fail, but neither it nor within_noise proves equivalence. Three repeats
give a small empirical noise estimate, not a bound on future drift. Frequency,
thermal and scheduling changes do **not** normalize away; repeat inconclusive
or suspect results on a quiet host. Never run baseline/candidate concurrently or
while the full visual gate is loading the same GPU. This lane detects a uniform
2× regression in both quad workloads instead of cancelling it in a ratio; the
synthetic comparator test explicitly covers that competing interpretation.

## Reproduce

Build both reviewed revisions with the identical example before sampling, in
clean worktrees with dependencies cached. Both revisions must implement the
measurement API; an older baseline must first receive the instrumentation-only
commit, never substituted candidate renderer code. The capture command records
the exact commit, executable hash, harness hash, toolchain, hashed host identity,
renderer/driver identity, session and distinct process IDs. It refuses dirty
checkouts, missing capabilities and incompatible/missing evidence.

From each clean worktree, using the same session identifier on the same machine:

```bash
cargo build --manifest-path tools/headless-visual/Cargo.toml \
  --example renderer_timing --release --locked
python3 -B tools/renderer-timing/compare.py capture /tmp/baseline.json --session review-42
# Run the same command from the candidate worktree, writing /tmp/candidate.json.
python3 -B tools/renderer-timing/compare.py compare /tmp/baseline.json /tmp/candidate.json \
  > /tmp/renderer-comparison.json
python3 -B -m unittest discover -s tools/renderer-timing -v
```

An unchanged control uses two distinct captures from the same revision, not the
same file passed twice. Preserve both raw JSON files and the comparison report.
The command never pushes, deploys, fetches refs or changes an existing checkout.

Native validation commands (must run on their named host):

```bash
# Linux / Windows fallback adapter; no skipped-adapter success:
cargo test -p gpui-box-wgpu --features test-support renderer_timing_is_owned --locked
# macOS native Metal:
cargo test -p gpui-box-macos --features test-support renderer_timing_is_owned --locked
# Each host also runs capture/compare and its ordinary headless visual gate.
```

Linux compilation of target-gated macOS/Windows packages is not native validation.
No visual appearance changes are intended by this instrumentation.
