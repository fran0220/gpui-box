# Visualization system delivery

Coordinator: https://ampcode.com/threads/T-01a0993e-08fb-7068-8b51-c9813aec87cf

## Outcome and compatibility

Deliver a product-neutral, native visualization system, rather than a list of
normalized shape renderers. Recharts is the reference for Cartesian chart
composition and interaction, ECharts for visualization breadth and data
mapping, and React Flow for controlled graph editing. This is a capability
target, not a promise of API compatibility or every upstream feature.

Preserve existing normalized APIs. Caller-owned data, business identities,
formatters, locale/calendar policy, selection, viewport and application actions
remain caller-owned. Pure scales, ticks, layouts, sampling and visual encodings
are reusable Kit infrastructure, not application policy. Queries, persistence,
credentials, remote data fetching and financial trading logic stay outside Kit.
Generic clipping, transforms, capture and platform input remain framework work.

An assigned or implemented item is not delivered until integrated and verified.
The coordinator owns shared exports, registry/catalog reconciliation, coverage
documentation, full-tree validation, staged main pushes and hosted verification.
Workers own disjoint source families and return transferable patches and evidence.

## Delivery stages

| Stage | Scope | Status |
| --- | --- | --- |
| V1 | Typed data, scales, ticks and coordinate systems | Integrated, including caller ticks and measured axis lanes |
| V2 | Composable Cartesian series and reading layout | Integrated, including orientation and bounded custom marks |
| V3 | Controlled chart exploration and synchronization | Integrated, including keyed raw motion and current-value semantics |
| V4 | Distribution, hierarchy, flow and continuous heatmaps | Integrated; EN/zh-Hans localization verified |
| V5 | Graph layout and temporal exploration | Integrated; disclosure rendering corrected and inspected; Linux gate passed |
| V6 | Geographic visualization | Integrated; combined Linux gate passed |
| V7 | Performance, accessibility and platform acceptance | Sampling/index/shared-data integrated; Linux gate passed, native motion/macOS/Windows unclaimed |
| V8 | Documentation, migration, catalog and deployment | Implementation catalogs regenerated and gate passed; this delivery commit is the deployment candidate |

### Active ownership

| Stream | Owner | Primary write boundary |
| --- | --- | --- |
| V1–V3 | https://ampcode.com/threads/T-01a09975-360b-745a-9903-dc67e79e7b09 | chart.rs, new Cartesian modules and family scenes/tests |
| V4 | https://ampcode.com/threads/T-01a09975-a3dd-7229-b80a-338e8d1abec0 | plot.rs, heatmap.rs, new specialized modules and family scenes/tests |
| V5 | https://ampcode.com/threads/T-01a09975-fd52-77dd-a1fe-af7b019ad5f3 | canvas, trace.rs, graph/temporal scenes/tests |
| V6 | https://ampcode.com/threads/T-01a09976-c7c0-70d8-9ba1-7e8bf6694435 | new geographic modules and family scenes/tests |
| V7–V8 | Coordinator | integrated performance checks, shared catalogs, documentation and delivery |

Workers start from origin/main at the assessment revision. This plan was
transferred explicitly to their isolated workspaces. Shared module exports and
scene registry edits are reconciled by the coordinator; worker final files must
not overwrite another stream's exports. Candlestick/raw-time integration waits
for the V1 contract rather than introducing competing scale engines.

### V1: Data and coordinate foundation

- Finite numeric domains, continuous linear/logarithmic scales, categorical
  bands and time coordinates; forward/inverse mapping and explicit clamp policy.
- Defined empty, singleton, constant, reversed and invalid domain behavior;
  numeric precision remains f64 until projection to paint coordinates.
- Optional automatic domain/nice ticks with caller overrides. Time formatting
  and calendar/timezone policy are explicit, not guessed from numeric values.
- Raw sample identity is independent of index. Missing readings are not zero;
  broken lines and omitted marks have explicit semantics.
- Shared axis layout supports real ticks, units, long labels, measured margins,
  resize and non-default orientations without changing the legacy contract.
- Test asymmetric/reversed/extreme domains, log rejection, round trips, category
  identity, missing values and tick bounds using independent expected values.

### V2: Composable charts

- Shared plot coordinates and clipping for line, area, bar and scatter layers;
  mixed charts and independently assigned axes.
- Grouped bars, diverging positive/negative stacks, percentage stacks, explicit
  zero baseline and range/interval marks. Never silently truncate excess values.
- Linear, step and monotone interpolation with shape-preservation tests.
- Reference lines/regions, error bars, legends, continuous/discrete color keys,
  caller formatting and extensible marks participate in measured layout.
- Pie/donut normalize validated raw values; radar and gauge accept explicit
  domains. Distinguish unsupported input from valid zero/empty observations.
- Exhibits cover mixed units, negative stacks, gaps, dense/long labels, dark and
  light themes, narrow/wide containers and retained stale readings.

### V3: Exploration and synchronization

- Stable point and shared-axis tooltips, hover versus selection distinction,
  controlled legend visibility, keyboard navigation and accessible values.
- Brush/range selection, pointer-anchored zoom and pan, reset, and coordinated
  charts using caller-owned domain/selection state rather than a global bus.
- Input tests exercise capture, outside release, cancellation, redraw during
  gestures, host refusal and target removal. Bounds match actual painted marks.
- Semantic reads expose current raw/formatted data during animated geometry;
  reduced motion settles transitions without changing selection behavior.

### V4: Specialized visualization families

- Candlesticks on the shared time/value model, with independently composable
  volume and caller-supplied overlays; no market data or trading policy.
- Continuous and diverging heatmaps with an explicit domain/color legend while
  retaining the existing discrete contribution heatmap and missing-value state.
- Histogram binning, box plots and waterfall/range presentation with documented
  statistical conventions and independently derived boundary test cases.
- Treemap, sunburst and funnel: raw-value layout, stable identities, readable
  labels, selection and malformed/zero input handling, not painter stubs.
- Sankey layout preserves flow scale, improves deterministic crossing/order
  behavior, and supplies useful visible node/link labels and quantities.
- Every new component has its own reachable family exhibit and generated API.

### V5: Graph and temporal visualization

- Stable layered layout, explicit cyclic graph policy, real node dimensions,
  component separation and deterministic placement. Preserve the old helper.
- Keep routing consistent with measured node/port geometry; evaluate obstacle
  avoidance, crossing behavior and invalidation costs independently of layout.
- NodeGraph remains a controlled editor. Test existing gestures, groups,
  minimap and fit after layout changes; do not infer React Flow parity by count.
- Shared time viewport for TraceView/SpanTimeline, readable ticks, zoom/pan,
  hierarchy navigation and large-trace visible-work bounds.
- AudioWaveform remains an envelope viewer, ModelViewer a bounded model viewer;
  do not turn either into a decoder, CAD tool or general 3D renderer.

### V6: Geographic visualization

- Local caller-supplied geographic features, validated geometry, a documented
  projection subset, choropleth encodings and point overlays.
- Shared projected hit testing, selection, pan/zoom, legends and semantic IDs.
- Explicit antimeridian, holes, out-of-domain and unsupported geometry behavior.
  Tile services, geocoding, network access and provider assets stay outside Kit.
- Inspect exhibits with asymmetric geometry, polygon holes and narrow views;
  use original synthetic fixtures or record source provenance exactly.

### V7: Evidence, not renderer assumptions

- Chart-specific 1k/10k/100k input fixtures measure preprocessing separately
  from static redraw, append, selection, hover and viewport changes.
- Sampling preserves extrema and missing-data boundaries; hit-test acceleration
  returns source identities and values, not synthetic sample identities.
- Graph/trace stress cases report layout/routing versus mounted/painted work.
  Record the supported envelope; do not invent universal frame-rate guarantees.
- Tests assert output geometry, values and behavior, never source strings.
- Run targeted tests while iterating, inspect rendered affected states before
  accepting baselines, then run `cargo run -p xtask -- gate full` at integration.
- Framework/token/font/renderer changes additionally require dependency checks,
  provenance/compatibility updates and the repository's native platform lanes.
  Unavailable macOS/Windows evidence remains explicitly pending.

### V8: Delivery

- Update coverage boundaries to distinguish data ownership from generic math.
- Document raw-data and legacy normalized entry points, accepted subsets,
  missing/invalid data behavior, interaction ownership and performance limits.
- Regenerate API/developer indexes and provide compilable composition examples.
- Commit in verified, reviewable stages and push directly to main; never create
  PRs. After each push run `tools/site/deploy-main.sh` and verify hosted revision
  and catalog counts. A local commit or screenshot alone is not deployment.

## Evidence ledger

Each completed stage must record its source changes, exact checks and decisive
results, inspected visuals where relevant, unresolved limits, integration revision
and deployment. Worker results below are not integrated-main acceptance.

- V1–V3 frozen stage 1 is integrated, including six inspected Cartesian frames.
  Worker all-feature tests: 991 unit, 1023 integration, 17 doctests passed;
  all-target Clippy passed. Exact interval/wick centers, source-key stacks,
  controlled pan/cancel/refusal and raw-value semantics are covered. Stage 2
  remains active; stage 1 is not complete V1–V3 acceptance.
- V4 final initial stage is integrated: raw hierarchy/statistical geometry,
  continuous heatmaps, conserved Sankey ordering and shared-Range OHLC.
  Worker tests: 1002 unit, 1023 existing integration, four specialized and
  17 doctests passed; all-target Clippy passed. Ten new frames inspected.
  Integrated strings audit exposed user-visible English; localization is
  being corrected rather than absorbed into the diagnostic allowlist.
- V5 final source and baselines are integrated: measured layout, borrowed exact
  route-cache input comparison and virtual raw-time traces. Worker tests:
  989 unit, 1026 integration and 17 doctests passed; Clippy and performance
  check passed. Coordinator replaced missing-glyph disclosure triangles with
  bundled icons, rendered `headless capture trace-time`, and inspected both
  themes: expanded/collapsed chevrons, state dots and labels do not overlap.
- V6 final source is integrated. Worker geography tests: 11 passed; all-target
  Clippy, strings/API checks and two headless frames passed. Projected holes,
  controlled input/refusal and clipped measured geometry are covered. This
  remains the documented projected-edge subset, not a GeoJSON/GIS engine.
- V7 real Harness workloads run via `cargo run -p gpui-box-performance --
  --charts --output target/performance/charts-v7.json`: 44 phase reports over
  sparse 1k/10k/100k input and full-domain 1k, with actual selection, hover and
  accepted viewport assertions. Initial stage-1 sparse redraws mount 24 marks
  and record 70 layout/paint calls at all sizes, but allocations grow from
  31,282 to 3,001,309. Full preprocessing/cloning is not bounded by the viewport.
  These are CPU/test-platform counters and advisory times, not GPU/FPS results.
  Separate sampling/index tests pass four cases including exact brute-force
  hit comparisons at 100k; renderer integration remains pending.
- First combined `cargo run -p xtask -- gate full` stopped at strings audit:
  65 unlisted literals, including real localization gaps and a path-based
  test-module false positive. Test fixture exemption follows the existing
  explicit-file convention. A passing worker gate does not resolve this
  combined failure; rerun after fixes. Implementation is not yet committed,
  pushed or deployed.
- Coordinator ran `cargo run -p xtask -- gate full` before implementation
  integration: exit 0, `gate passed`, including the headless catalog and all
  13 mobile Chromium/WebGL2 tests. This validates the baseline and planning
  documentation, not worker implementations. The integrated source must run
  the gate again. `git diff --check` also passes.

### Integrated follow-up evidence

- Stage 2 and V4 localization are now integrated. Chart errors/stale status,
  heatmap missing/refusal/legend and generated statistical labels use EN/zh-Hans
  keys. `strings check`: 455 literals accounted for; only reviewed invariant
  diagnostics added to the allowlist. Both generated API catalogs are current.
- The first combined all-feature Kit suite passed 1024 unit, 1035 integration,
  four specialized and 18 doctests. After stage 2/V7, focused chart tests pass
  32 cases, including shared-data revision invalidation, indexed hits, orientation,
  capture/refusal, motion/current values and actual sampled-path selection.
  Localized specialized integration tests pass six cases.
- Combined scoped headless check matched 34 images before stage 2, followed by
  six localized V4 frames. Coordinator inspected updated trace and heatmap
  light/dark captures and Chinese statistics. New `cartesian-dense` light/dark
  images show the gap and both spikes in exact/sampled wide/narrow layouts;
  the same selected original reading is 73.125. Final combined gate follows.
- V7 now uses immutable caller-shared input and an exact projection cache, lazy
  readout formatting, a final-geometry rectangle index and opt-in linear-path
  extrema sampling. Gaps and source identities are preserved. The benchmark
  now records 88 phase reports with animation both enabled and disabled.
  At 100k sparse input, measured static allocations fell to 1917/1937
  (motion off/on), from 3,001,309. The sparse allocation budget is 2500;
  semantic publication, hit construction and motion still have linear work.
  See `docs/cartesian-charts.md` for advisory timings and dense-input limits.
- `cargo clippy -p gpui-box-kit -p gpui-box-performance --all-features
  --all-targets -- -D warnings` passes. No framework/token/font/package
  authority changes were needed. Native motion capture failed in the worker's
  black Xvfb window; simulated-frame tests are not claimed as native visual
  verification. macOS/Windows and a universal interactive-100k claim remain
  explicitly unverified rather than inferred from Linux headless pictures.

### Final integrated Linux acceptance

- `cargo run -p xtask -- gate full` exited 0 with `gate passed`. Kit default
  tests: 1026 unit and 1035 integration passed; all-feature tests: 1032 unit
  and 1035 integration passed. Six specialized tests and 18 doctests passed;
  existing ignored tests remain ignored. The gate also passed workspace
  checks, all-target Clippy, generated catalogs/strings/tokens, performance
  budgets, warning-free wasm compilation and rustdoc.
- The full Linux headless catalog matched all 392 images. The first full
  comparison exposed two old OHLC axis-label baselines after the shared axis
  layout change. Both themes were inspected: only tick-text alignment moved,
  with centered bodies/wicks, independent volume and exact raw readout intact.
  After accepting these frames, the complete gate was rerun successfully.
- Mobile reference capture passed retained-data/refusal/navigation/form-restore
  assertions. All 13 Chromium/WebGL2 tests passed. These do not replace native
  macOS/Windows or native animation evidence.
- Source, documentation, generated catalogs and reviewed Linux baselines are
  delivered together in this commit. The required post-push deployment must
  verify this commit on both hosted domains, including complete MCP tool
  schemas and component enumeration; successful deployment is reported by the
  coordinator after executing that check, not inferred from this ledger.
