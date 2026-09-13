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
| E1 Cartesian motion | Keyed enter/update/exit, visual style transitions, hide/show/reorder continuity, configurable timing, idle/active fast paths | Cartesian renderer, motion/layout/performance modules, legacy chart integration | Implementing |
| E2 Cartesian exploration | Rich floating exact-value tooltip, emphasis, editable persistent range, overview navigation, linked caller state, reference/label layout | Same core owner as E1; one coherent implementation | Implementing |
| E3 Specialized experience | Direct geometric picking, controlled hierarchy drilldown/back, layout/color lifecycle, useful labels/leader lines, improved conserved Sankey order | specialized, plot, heatmap and their scenes/tests | Implementing |
| E4 Graph/trace | Animated node/routes, improved cycle/crossing/obstacle routing, bounded mounting, temporal range/navigation and incremental hierarchy | canvas graph/layout, trace and their scenes/tests | Implementing |
| E5 Geography | Captured drag/touch exploration, hover/fit, camera/style motion, GeoJSON interchange, explicit antimeridian policy, spatial culling/index/simplification | geography and its scenes/tests | Implementing |
| E6 Data/time foundation | Explicit calendar/timezone ticks and pure source-traceable filtering/aggregation/bin/window transforms | chart scale/data companion modules; coordinator | Implementing |
| E7 Dynamic/performance acceptance | Real playback evidence, reduced motion and refused updates; sparse/dense and sustained updates; Linux/native platform evidence | shared performance tooling and integrated review; coordinator | Pending integration |
| E8 Downstream delivery | Migration/composition guides, locale coverage, generated catalogs, full gate, main commits and hosted MCP verification | shared exports/strings/docs/release; coordinator | Pending integration |

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
  `/tmp/experience-foundation-gate.log`; hosted verification follows the push.
  Frozen family patches still await separate coordinator review and are not
  part of this shared-foundation acceptance. Native platform maturity remains
  unproven by the Linux/browser checks.

### Shared follow-up contracts (not yet integrated)

- Core owns a standalone `interaction/range.rs` state machine shared by Cartesian
  and trace. Family adapters own mapping, measured rendering and public events.
  Raw f64 caller state stays authoritative; cancellation clears drafts without
  rolling back already accepted changes. Delayed unrelated acceptance cancels
  rather than silently rebasing. Reversed/nonlinear mappings and exact unchanged
  endpoints require focused tests before either family depends on it.
- Specialized owns configurable FLIP timing and enable/snap behavior. A new
  mounted test found that existing `flip_size` changes the wrapper but does not
  constrain an explicitly sized child: a 40px wrapper can publish an 80px child
  hit target. The coordinator confirmed no existing forced-root-size API and
  authorized a separate framework primitive with descendant reflow, restored
  authored styles and correct layout-cache invalidation. Framework provenance,
  compatibility and platform acceptance are required; none is claimed yet.
- Frozen graph viewport culling and trace motion/readout patches await review
  after their layout/hierarchy dependencies. Fixed visible paint counts do not
  establish bounded source scans: the worker's explicit-size edgeless 100k graph
  still takes about 1.5s to mount and 792ms for static redraw in debug Linux.
