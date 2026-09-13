# Specialized visualization interaction and motion

`SpecializedData` validates raw measurements and builds normalized geometry.
`SpecializedChart` renders the geometry, exact-value key, selection, hierarchy
navigation, and optional visual transitions. `ContinuousHeatmap` maps an explicit
raw domain to colors. `Plot` owns measured frame/input/readout/label placement;
it does not infer a second scale system.

## New experiences animate by default; legacy presentation stays compatible

`SpecializedChart` and `ContinuousHeatmap` default to theme-aware data animation,
consistent with raw Cartesian experiences. `.animate(false)` directly snaps
updates; `.motion(bool)` is the same switch retained as a compatibility alias.
`.animation(MotionSpec)` overrides timing without bypassing reduced motion.
Changing timing starts from displayed geometry/color rather than reinterpreting
elapsed time under a new curve. Hover, exact readouts and controlled proposals
work independently of animation. Labels remain a separate density decision.

`MotionPolicy` supplies `Resize` timing for specialized geometry and Sankey,
and `StateChange` for heatmap color, entry/exit and keyed axis movement. The
normalized legacy `SankeyChart` retains static defaults: opt into `.animate(true)`
once for the layout experience. Discrete `Heatmap` levels 0–4 are unchanged.
The earlier frozen specialist milestone's opt-in default was a rollout stage,
not an enduring API requirement. Hosts need no extra switches for new charts.

## Downstream composition, lifecycle and budgets

For a normal bounded overview, mount one stable chart identity, supply complete
validated caller data, and pass controlled selection
plus proposal handlers. The hierarchy example below is the current API. Add
`.labels(true)` when measured label density is useful; neither this flag nor
an `on_hover` callback is required for geometric picking or native readouts.
For continuous heatmaps, provide explicit domain, stable cell/axis identities,
rows/columns; never animate numbers in a separate host loop.

- Keep a chart identity stable across accepted revisions. Replace it when the
  dataset represents a different business object, not to force a repaint.
- Publish only accepted data. Rejecting selection or drill proposals leaves
  the accepted state unchanged; any already-running accepted transition keeps
  progressing. A rejected update is not an animated excursion followed by a
  rollback. Use `Stale` with the last verified data and refusal reason when a
  refresh fails.
- Builders validate the supplied dataset, not a patch stream. Merge partial
  server updates into caller-owned data and validate that complete revision
  before publishing it. Invalid inputs are errors, not silently dropped marks.
  Partially accepting a revision is a host decision and must be represented by
  the actual accepted data and an honest status, not manufactured interpolation.
- For specialized shapes, an accepted revision retargets from the current
  display. Removed shapes immediately lose interaction/semantics while their
  decorative exit may finish. Pass a valid empty `SpecializedData` in `Ready`
  to retain those exits while reporting Empty; explicit `PlotState::Empty`,
  Loading, Error and Unavailable reset specialized animation state.
- Reduced motion settles endpoints and discards exits. `.animate(false)` snaps
  data presentation without disabling input. Unmounting is not a promise to
  complete an exit: keyed slots use the existing window-scoped two-generation
  grace and need normal root semantic frame publication for pruning. Remounting
  after pruning starts fresh; do not rely on visual state as stored data.

These are overview components, not virtualized large-data engines. There is no
benchmarked universal mark cap or automatic animation budget in this milestone.
Specialized animation stores transitions per displayed vertex, including exits;
painting and exact picking scan geometry. Measured label placement adds pairwise
collision checks over candidate rows. Heatmaps materialize the row × column grid,
not only cells with observations. Sankey refinement costs O(V E²).

Downstream must bound the visible hierarchy depth, bin/aggregate distributions
using explicit statistical semantics, and page/filter heatmap axes before
mounting an unbounded dataset. Disable labels first for density, and use
`.animate(false)` for high-churn views whose measured budget is exceeded. Neither
switch removes geometry construction or hit-testing cost. Coalesce accepted
revisions upstream rather than enqueueing every intermediate server packet.
Choose limits from profiling the actual viewport/data on target platforms;
the 32-frame regression proves correctness, not large-data throughput.

## Caller-owned state

Use `.selected(Some(id))` or `.selected(None)` for controlled selection and
`.on_current(...)` to receive proposals. Refusing a proposal does not change
painted or semantic selection. The legacy `.current(id)` retains local keyboard
selection behavior. `.on_hover(...)` reports geometric hover separately; it does
not accept selection. Hover emphasizes the shape and feeds a native floating
tooltip; the header and persistent key provide exact values without hovering.

Hierarchy navigation is explicit:

```rust,ignore
let data = SpecializedData::sunburst_at(&root, &focus_id)?;
SpecializedChart::new("allocation", "Allocation", PlotState::Ready(data))
    .selected(selection)
    .on_current(propose_selection)
    .on_navigate(propose_focus)
    .motion(true)
    .labels(true)
```

`treemap_at` and `sunburst_at` validate the **entire** tree, then lay out the
selected subtree. Ancestor controls navigate back; child controls drill down.
Their identities and labels come from the supplied nodes. The focused control
is disabled and has no handler. The host accepts navigation by rebuilding data
with the proposed identity; no hidden component focus path changes. Unknown
focus is an error, not an empty tree. Plain `treemap` and `sunburst` remain
available without navigation chrome.

## Exact geometry and semantic envelopes

`SpecializedData::geometry(id)` exposes actual normalized vertices.
`hit_test(point, pixel_size, stroke_width)` uses the same polygon fill and stroke
geometry as painting, including annular holes and reverse paint-order overlap.
Points outside the plot are rejected. Stroke tolerance is half the actual
pixel width, not a guessed normalized rectangle. The shared `Plot::hit_test`
callback also receives the measured pixel frame and can only return live mark
identities. Removed animated shapes cannot be returned by that callback.

Platform accessibility regions remain tight **axis-aligned envelopes**, measured
during prepaint. They are not claims of polygon-shaped accessibility support.
Visual labels and their leader lines are not shape hit regions. During motion,
semantic envelopes and pointer picking use the displayed geometry, while raw
semantic values already describe the latest verified input. Selection/hover
outlines are decorative emphasis, not expanded data hit regions.

## Visual motion never invents raw readings

New charts use identity-keyed geometry and enter/exit opacity using
the existing `Transition` and `MotionPolicy` contracts. Updates retarget from
the current display. A reinserted identity resumes its retained visual state;
removed shapes may finish fading but immediately leave keys, semantics, and
picking. Empty data publishes Empty while the last decorative exit finishes.
Error/loading/unavailable clears animation state rather than displaying an old
measurement as current. Reduced motion settles at the latest endpoint and
discards exits immediately.

Polygon tessellation-count changes interpolate from the displayed vertex
sequence. This is a visual morph, not a claim of interpolated statistical area
or conserved totals at intermediate times. Exact values never interpolate.

Heatmap motion interpolates colors and fades in changed exact value text. It
does **not** tween numbers. `None` remains missing, zero remains a verified
observation, out-of-domain values are errors, and the visible domain legend
always reports the declared current scale. Legacy discrete levels 0–4 remain
unchanged. Tokens and the active locale continue to own styling and Kit wording.

Heatmap row/column identity already belongs to the caller: `HeatAxis.id` is
independent of its label, and `ContinuousHeatCell.id` is independent of its
`row`/`column` coordinates. Reorder axis vectors without changing those ids.
When migrating positional data, introduce domain keys upstream; do not derive
ids from the new index after sorting. Existing `rows(["east", "west"])` treats
those strings as stable identities and labels, not positions.

Live headers, row labels and cells use measured shared FLIP movement.
Reordering paths can cross; later-painted live cells win exact pointer picking
and consume their action so an underlying cell cannot overwrite selection.
Flexible layout slots use current column allocation. Their children use
`flip_size` with assigned border-box dimensions, so column-count changes also
animate displayed widths and reflow text. The first natural layout measures
the matrix; subsequent width changes request the next measured frame. Pointer
and semantic bounds follow displayed children, not the settled slots.
Removed measured cells/axis labels fade as clipped decorations without ids,
tooltips, focus or actions. Reinsertion within the retained exit resumes opacity
and position; direct/reduced updates discard exits immediately. Labels and
values belong to the current revision even during visual interpolation.

## Measured labels and conserved flows

`.labels(true)` on a specialized chart enables measured on-plot labels and
leader lines, in addition to the persistent exact-value key. `Plot` shapes
each string through the text system, prioritizes the current readout, and tries
nonoverlapping slots within the frame. Labels that cannot fit are omitted;
their values remain accessible in the key/header/tooltip. This is a bounded
placement heuristic, not a global optimization or a polygon-inscription claim.

`SankeyOrder::Barycenter` starts from stable identity order, alternates weighted
sweeps, refines adjacent swaps, and retains the best weighted center-line
crossing score. Skip-layer links participate; zero flows and shared endpoints
do not manufacture crossings. Stable link order makes accumulation and tie
behavior reproducible. All layouts retain the single raw-weight-to-height
scale; crossing refinement changes ordering, never amounts or ribbon widths.
The crossing proxy does not guarantee minimum curved-ribbon intersections.
Refinement costs O(V E²): use it for overview-sized flows, not enormous DAGs.

Sankey pointer picking inverts the same cubic x curve used by painting and tests
the actual upper/lower ribbon edges. Nodes take precedence over ribbons. Its
semantic identities are `node.<id>` and `link.<id>`. `.labels(true)` preserves
the existing persistent key and also enables measured floating plot labels.

Animated Sankey nodes and ribbons are keyed separately. Ribbon attachment x
coordinates follow displayed node edges; vertical offsets and thickness tween
in plot units, avoiding the false taper produced by multiplying separately
interpolated node heights and flow fractions. Raw readings remain the exact
accepted endpoint values. A link whose endpoint identity changes crossfades
its old decorative connection and new live connection rather than traversing
unrelated nodes. Removed connections retain their old endpoint nodes only for
decorative retirement; neither is added back to the live picker or readout.

## Review surfaces

- `specialized-exploration`: live hierarchy/distribution updates, drill/back,
  refusal, removal/reinsertion, labels, and interrupted motion.
- `continuous-heatmap-transition`: negative → positive → missing color/text
  changes beside a stable zero reading.
- `heatmap-reordering`: row/column crossing, entry, exit, reinsertion and direct updates.
- `sankey-motion`: keyed node/ribbon updates, current readouts, retirement and selection.
- `specialized`, `specialized-distribution`, `continuous-heatmap`, and
  `sankey-layout`: baseline raw-value, state, and conservation presentations.

Use explicit simulated frame advancement for intermediate/retarget/exit review.
Static baseline capture deliberately uses reduced motion and is not evidence
of animation correctness. No network, persistence, product models, or financial
data authority belongs to these components.

The opt-in actual-renderer regression uses the local headless playback protocol:

```bash
cargo build --manifest-path tools/headless-visual/Cargo.toml
python3 crates/gpui-kit/tests/specialized_playback.py
python3 crates/gpui-kit/tests/specialized_layout_playback.py
```

It records 32 light/dark frames and redacted semantic snapshots under
`target/specialized-playback`, checking interrupted geometry continuity, exact
raw readings, absent removed targets, separated measured labels and changed
intermediate pixels. Supply an output directory to retain review artifacts.
These are simulated offscreen frames, not native-window timing or FPS evidence.
