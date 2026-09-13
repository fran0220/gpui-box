# Mature visualization experience delivery

Base: `d368a2180c4be1ee4e0ad5abbd4bbfaa3ac37955`.
Coordinator: https://ampcode.com/threads/T-01a0993e-08fb-7068-8b51-c9813aec87cf

The owner requests a complete downstream visualization system. The previous
delivery established raw-data geometry and controlled inputs; its passing gate
does not establish mature animation, exploration or large-data performance.
This round closes those gaps as one coherent experience, with reviewable
implementation milestones rather than relabeling each partial stage as complete.

## Contracts that every family must preserve

- Data, identities, locale/timezone policy, selection and viewport are caller
  owned. Requests may be accepted or refused without hidden state accumulating.
- Current semantic/readout values are never interpolated. Exiting pictures are
  presentation-only and lose hit/action/semantic authority immediately.
- Interrupted transitions start from displayed geometry. Reduced motion snaps;
  direct gestures track input without a trailing camera or geometry animation.
- Paint, clips, hit geometry and measured accessibility bounds agree throughout
  animation, resize, orientation changes and captured input.
- Legacy normalized APIs remain available. New public capabilities appear in
  generated catalogs with compilable examples and independently reachable exhibits.
- Generic rendering/input gaps are fixed at the framework authority, not hidden
  inside a component. Network/database/provider integration remains downstream.

## Acceptance matrix and ownership

| Stream | Required outcome | Owned boundary | Status |
| --- | --- | --- | --- |
| E1 Cartesian motion | Keyed enter/update/exit, visual style transitions, hide/show/reorder continuity, configurable timing, idle/active fast paths | Cartesian renderer, motion/layout/performance modules, legacy chart integration | Integrated; tested playback |
| E2 Cartesian exploration | Rich floating exact-value tooltip, emphasis, editable persistent range, overview navigation, linked caller state, reference/label layout | Same core owner as E1; one coherent implementation | Integrated; tested playback |
| E3 Specialized experience | Direct geometric picking, controlled hierarchy drilldown/back, layout/color lifecycle, useful labels/leader lines, improved conserved Sankey order | specialized, plot, heatmap and their scenes/tests | Integrated; tested playback |
| E4 Graph/trace | Animated node/routes, improved cycle/crossing/obstacle routing, bounded mounting, temporal range/navigation and incremental hierarchy | canvas graph/layout, trace and their scenes/tests | Integrated; safety snaps and work limits documented |
| E5 Geography | Captured drag/touch exploration, hover/fit, camera/style motion, GeoJSON interchange, explicit antimeridian policy, spatial culling/index/simplification | geography and its scenes/tests | Integrated; tested playback |
| E6 Data/time foundation | Explicit calendar/timezone ticks and pure source-traceable filtering/aggregation/bin/window transforms | chart scale/data companion modules; coordinator | Deployed foundation; batch transforms |
| E7 Dynamic/performance acceptance | Real playback evidence, reduced motion and refused updates; sparse/dense and sustained updates; Linux/native platform evidence | shared performance tooling and integrated review; coordinator | Playback, CPU and native evidence recorded below; no FPS guarantee |
| E8 Downstream delivery | Migration/composition guides, locale coverage, generated catalogs, full gate, main commits and hosted MCP verification | shared exports/strings/docs/release; coordinator | Deployed; full Linux gate and both hosted MCP catalogs verified |

Workers continue their existing isolated threads, updated to the exact base.
They own disjoint source families and return frozen incremental patches; messages
do not transfer local work. Core owns `cartesian*.rs`; coordinator owns `scale.rs`,
`data.rs`, their new companion modules and shared performance tooling. Shared
registry/strings/provenance/catalog changes are reconciled by the coordinator.

## Verification is part of the feature

For each family, exercise initial appearance, update, reorder, insertion,
removal, interruption, empty/stale/refused state, reduced motion, keyboard and
pointer input, narrow/wide layouts and both themes. Test asymmetric data and
boundary cases where a plausible incorrect implementation produces another
value or hit target. Inspect actual intermediate and final renders; a still
baseline alone is not motion acceptance. Capture real playback when timing is
under review and report unsupported native environments rather than invent proof.

Performance reports separate preprocessing, first mount, resting redraw, active
animation, append/retarget, hit lookup and semantic publication. Include dense
and sparse input plus worst-case overlap; visible paint counts are not full
work bounds. Keep GPU/frame timing distinct from test-platform CPU counters.
The previous 100k animated chart redraw and 10k graph mount figures are
regression evidence to address, not supported responsiveness guarantees.

Every integrated commit runs the repository full Linux gate. Platform dispatch
is used when required by framework/renderer changes and for native acceptance;
claims remain limited to executed checks. After each main push, deploy the exact
clean commit and verify both hosted catalogs and complete remote MCP schemas.

## Evidence ledger

- Implementation started from the clean deployed baseline. Prior worker work is
  preserved before resetting their isolated checkouts to the integrated base.
- No experience-stream capability in this document is accepted merely because
  its implementation has started. Exact results and unresolved gaps follow in
  frozen milestone reports and integrated acceptance entries.

### Shared foundation milestone (Linux integration passed)

- Calendar boundaries and source-lineage transforms are implemented with 39
  chart-family tests passing. DST repetition/gaps, skipped dates, fractional
  domains, missing readings, bin identity and finite extreme means are covered.
  The guide is included in rustdoc so its downstream example is compiled.
  Rolling reduction remains a prepared batch operation, not streaming support.
- Keyed window state now prunes once per semantic generation. Seven lifecycle
  tests pass, including 100k keys, old-grace-before-new-grace expiry, zero grace,
  lazy reads and immediate owner release. The examination counter proves linear
  generation work instead of repeated full-map scans on each insertion.
- Shared f64 interpolation preserves raw time precision and exact endpoints;
  all 188 motion tests pass. Scalar distance saturates within the existing f32
  engine contract. No renderer/platform primitive was changed.
- The API catalog now resolves actual public module/reexport bindings rather
  than guessing the first directory segment. All 81 xtask tests pass, including
  compiling generated imports in a downstream fixture. Public chart paths are
  checked against actual compiled types. Macro/cfg expansion and duplicate bare
  declaration names remain existing source-catalog limitations.
- Local headless playback can toggle reduced motion and capture exact simulated
  frame times without settling. The focused test passes. Coordinator generated
  and inspected a slowed spring playback: continuous reversal, visible overshoot
  and reduced-motion settle, no black frames. This proves the playback tool,
  not completed motion for the still-active chart/geography/graph workstreams.
- `cargo run -p xtask -- gate full` exited 0 with `gate passed`: workspace
  default/all-feature tests, Clippy, generated indexes/tokens, performance,
  wasm compile/release build and rustdoc passed; 392 Linux scene images match;
  17 headless-tool tests, 8 glass-reference tests and 13 browser mobile input
  tests passed. Mobile-reference refusal/restore checks also passed. The log is
  `/tmp/experience-foundation-gate.log`. Foundation commit
  [53ddf792](https://github.com/fran0220/gpui-box/commit/53ddf792b9ec641d8c5d3bcb47fb22c532f50a09)
  is deployed: both hosted domains verified the exact revision, 212 components,
  196 scenes and all 10 MCP tool schemas (`/tmp/experience-foundation-deploy.log`).
  Frozen family patches still await separate coordinator review and are not
  part of this shared-foundation acceptance. Native platform maturity remains
  unproven by the Linux/browser checks.

### Shared interaction foundation (Linux integration passed)

- Core owns a standalone `interaction/range.rs` state machine shared by Cartesian
  and trace. Family adapters own mapping, measured rendering and public events.
  Raw f64 caller state stays authoritative; cancellation clears drafts without
  rolling back already accepted changes. Delayed unrelated acceptance cancels
  rather than silently rebasing. Focused tests cover reversed/nonlinear mappings,
  exact unchanged endpoints and actual whole-component capture removal.
- Specialized owns configurable FLIP timing and enable/snap behavior. A new
  mounted test found that existing `flip_size` changes the wrapper but does not
  constrain an explicitly sized child: a 40px wrapper can publish an 80px child
  hit target. The coordinator confirmed no existing forced-root-size API and
  authorized a separate framework primitive with descendant reflow, restored
  authored styles and correct layout-cache invalidation. The combined tree now
  passes 625 framework tests, the configured-FLIP mounted test and seven raw-range
  tests. Playback asserts 26 actual frames in both themes with delayed tween,
  timing changes, spring reversal, displayed pointer picking and disabled/reduced
  snaps. New default baseline inspection caught a clipped label; it was shortened,
  both themes rerendered and inspected, then accepted. Full Linux gate exited 0
  with `gate passed`: 394 images match and 13 browser mobile tests pass, alongside
  workspace tests/Clippy, performance, wasm, rustdoc and tool checks. Evidence is
  `/tmp/interaction-foundation-gate.log`; native platform acceptance is pending.
- Frozen graph viewport culling and trace motion/readout patches await review
  after their layout/hierarchy dependencies. Fixed visible paint counts do not
  establish bounded source scans. Prior graph/trace/geography multi-case Harness
  timings were contaminated by still-open windows and are retracted. Corrected
  cases explicitly remove each window; geography also asserts empty window lists. Worker
  isolated debug measurements: edgeless 100k graph 1447ms mount/704ms redraw;
  sparse 100k/95k-edge graph 3789/1176ms; 100k trace 94.7/6.5ms. Geography full-world
  10k remains 4127/2289ms. These are CPU evidence, not dense/FPS acceptance.
  Cartesian timing cases already use a fresh App per case and were not affected.
- Coordinator's capture-unmount fix passes all 622 GPUI framework tests,
  including actual cached-subtree capture/remap, removal cancellation, reinsertion
  without click revival, and button-owned cancellation; the combined assigned-root
  tree passes 625. Trace and Cartesian owners independently reran their real
  arbitrary-unmount regressions successfully. Native acceptance remains pending;
  no per-component capture registry was added.

### Bounds-index and runtime-contract integration (Linux gate passed)

- Shared interaction foundation
  [480336c3](https://github.com/fran0220/gpui-box/commit/480336c3fd5ea04823c8e06fe063edc5a260dd9b)
  is deployed. Both domains verified 212 components, 197 scenes and all ten
  complete MCP tool schemas (`/tmp/interaction-foundation-deploy.log`).
- The balanced BoundsTree patch preserves assigned overlap ordering and removes
  degenerate insertion depth. Integrated focused tests pass: seven passed, one
  opt-in CPU benchmark ignored. Full Linux rendering passes; native index
  acceptance remains pending. Index-only speedups are not frame-rate claims.
- Native run [34759501968](https://github.com/fran0220/gpui-box/actions/runs/34759501968)
  passed both native jobs and Windows headless build/tests. Each renderer's
  scoped image check reports zero changed and two new FLIP images; all four new
  images were inspected for readable labels and contained geometry, then accepted.
  This run predates the BoundsTree integration and remains failed overall because
  its runtime jobs exposed the contract problem below. It is not evidence for
  native acceptance of the new bounds index.
- Both runtime jobs exposed stale JS binding coverage and family tests that
  incorrectly equated Rust catalog membership with a native JS adapter. The
  existing generator now explicitly includes the four unbound visualization
  components. Tests retain closed schemas and caller-data fixtures for supported
  entries and prove named unsupported entries reject root/nested wire nodes.
  Integrated JS family tests pass 10/10; catalog check reports 200/212 adapters.
  Full JS/runtime checks pass 209 tests with 17 platform skips. This does not
  implement new JS visualization adapters. Signature generation also now uses
  complete Rust signature parsing to avoid truncating arrays at their semicolon;
  generated TraceView/SpanTimeline viewport signatures and asymmetric nested-array
  and const-block fixtures pass in the 84-test xtask suite.
- Frozen paint integration remains separate. Coordinator review identified
  repeated Scene-prefix scans per recording and atlas-wide scans per sprite;
  the owner supplied a frozen follow-up using the active layer stack, reverse
  tile index and intersecting lease-range lookup. Its measured counters and
  unchanged frames await combined integration; graph recording is not yet accepted.
- Integrated exact-time FLIP playback passes 26 frames with real pointer picking,
  cancellation, timing/size retargets and reduced motion in both themes. The dark
  reverse-middle image was inspected: readable label, intact controls and no stale
  target-sized child. `CARGO_INCREMENTAL=0 cargo run -p xtask -- gate full` exited
  zero with `gate passed`: workspace tests/Clippy, generated artifacts, deterministic
  performance budgets, wasm compile/build, rustdoc, 394 matching Linux images,
  headless/mobile-reference tests and 13 browser mobile input tests all passed.
  Evidence: `/tmp/bounds-runtime-integration-gate-clean.log`. Two earlier attempts
  failed for disk exhaustion, not assertions; the successful attempt rebuilt a
  clean disposable debug target with incremental compilation disabled.

### Combined experience integration (full Linux gate passed)

- The coordinator has applied Cartesian lifecycle, controlled range/overview and
  emphasis; specialized layout/motion; Geography G6 measured-leaf integration;
  Trace hierarchy/range/localization; GraphSource, displayed node layout,
  card/route paint retirement, node entrance and global obstacle routing.
  Shared paint-recording follow-up and measured descriptive leaves use the
  existing framework authority. Generated catalogs are rebuilt centrally.
- Settings rows and custom section blocks now use horizontal `Space::Md` and
  vertical `Space::Sm`, with aligned divider insets. The screenshot report
  exposed the old 4px default. Twenty-five focused tests pass; all four Linux
  settings/settings-page images were rendered, inspected and checked.
- Graph source geometry reuse and incremental bounds-index refits, obstacle-safe
  route transitions and interrupted timing changes are integrated. Route samples
  share paint, hit, label and semantic geometry; incompatible corridors snap
  rather than crossing obstacles. The material allocation ratchet is 11868 of
  12105. Full metadata comparisons and arbitrary obstacle edits still scan or
  reroute globally; these CPU budgets do not establish native FPS guarantees.
- Trace expansion now moves surviving rows before publishing incoming rows.
  Waiting rows have no semantic or input authority. Eleven runtime tests pass;
  both themes have 125 actual 16ms playback frames checked for nonoverlapping
  published rows, and the recorded clips were inspected. Selection, scrolling,
  disabled and reduced motion snap without reviving old targets.
- Syntax-aware string and API scanners retain production items following
  test-only fields, including Unicode source spans and declared external test
  modules. Graph state descriptions are localized while wire values remain
  stable. Whole-tree accounting covers 479 literals, with no component exemption.
- Combined gate 7 passed workspace default/all-feature tests, Clippy, catalogs,
  performance, rustdoc and 408 Linux images, then exposed browser-release atlas
  lease callback bounds. The framework now requires Send + Sync on native
  release callbacks and preserves Wasm thread confinement without unsafe Send
  wrappers. Both atlas tests and the exact browser release build pass. Final
  gate 8 exited zero with `gate passed`: 1131 Kit unit tests, 1048 integration
  tests and 90 xtask tests passed, along with workspace tests/Clippy, current
  catalogs, performance, rustdoc, Wasm release, 408 matching Linux images and
  13 Chromium mobile-input tests. The log is
  `/tmp/visualization-combined-gate-8.log`. Browser touch emulation is not native
  mobile device certification.
- JS/runtime checks pass 209 tests with 17 platform skips. Catalog coverage
  remains an explicit 200 of 212 native adapters, not a claim that unsupported
  visualization bindings exist. Large active Cartesian revisions still scan
  and copy source geometry. Native acceptance remains separate from Linux and
  browser evidence.
- Foundation native run
  [34762664207](https://github.com/fran0220/gpui-box/actions/runs/34762664207)
  passed native and runtime tests but failed image checks: Metal reports 14
  changed/28 new frames; WARP reports 20 changed/28 new. Artifact inspection
  found coherent old-baseline differences shared with foundation Linux, plus
  six WARP frames with only 1–3 pixels exceeding tolerance by one channel step.
  This is not native acceptance of the uncommitted integration, and baselines
  have not been blindly accepted.

### Experience release and native follow-up

- Integration commit
  [41063868](https://github.com/fran0220/gpui-box/commit/41063868029baaae36b2e64a56732701d4cc3c6b)
  is deployed. Both hosted domains verify that exact revision, 212 components,
  204 scenes and all ten complete MCP tool schemas. Evidence is
  `/tmp/visualization-experience-deploy.log`.
- The deployed settings scene was opened in Chromium/WebGL2 at 2x. Its measured
  autosave row origin is (281, 196.5), and its label origin is (293, 204.5):
  exactly 12px horizontal and 8px vertical inset. The inspected screenshot is
  `.amp/in/artifacts/settings-live-deployed.png`; disabled content remains legible.
- Native run
  [34785409721](https://github.com/fran0220/gpui-box/actions/runs/34785409721)
  passed both runtime jobs and the Windows native job. macOS Kit tests and native
  menu smoke passed, but two new WGPU tests mistakenly requested software
  fallback adapters on Metal. Their platform conditions now match the existing
  Linux/WARP-only lifecycle tests; production renderer behavior is unchanged.
  The actual Metal frozen text/image recording playback test passed separately.
- All 60 Metal and 65 WARP changed/new images were inspected, including
  full-resolution checks of dense labels and comparisons with approved Linux
  geometry. No blocking visual defects were found. WARP CodeView and Performance
  HUD each differ at one pixel per theme by at most two channel steps; dark
  Agent Roster differs at three. The unchanged one-step tolerance was preserved.
  All 125 reviewed native frames were accepted from that exact run. All Windows
  headless and optical tests passed; its image jobs failed only for these
  reviewed baseline changes. Stills are not motion or FPS certification.
- The test-platform follow-up passed the complete Linux gate again, including
  408 matching images and 13 browser tests:
  `/tmp/visualization-native-followup-gate.log`. The macOS-only follow-up lane
  must verify the corrected WGPU test conditions and accepted Metal baselines;
  Windows production code and its executed test bodies are unchanged by the
  platform-condition correction.
