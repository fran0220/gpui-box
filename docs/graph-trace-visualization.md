# Graph layout and temporal viewport contracts

`canvas::layered_layout_sized` is an opt-in caller-side layout. It consumes
stable identities, positive finite measured sizes and edges, returning bounds
sorted by identity. Lexical ordering is independent of node/edge input order.
Weak components occupy separate bands; column widths and row heights use the
largest/actual dimensions rather than fixed center-to-center spacing.

`GraphCyclePolicy::Reject` returns no partial result on a cycle.
`BreakIncoming` chooses the lexically first remaining node when no source
exists, ignoring its incoming dependencies. Backward edges are expected in
that mode; it does not claim a DAG ordering, SCC decomposition, crossing
minimization or a Sugiyama implementation. Duplicate edges do not alter layout;
unknown endpoints, duplicate identities, malformed dimensions/gaps and
unrepresentable accumulated geometry are errors. The legacy `layered_layout`
helper is unchanged.

`GraphCyclePolicy::Condense` uses two iterative DFS passes to identify strongly
connected components, stacks each component's members by identity using their
actual sizes, and layers the condensation DAG. Thus every edge between SCCs
points forward, while internal cyclic edges need not. Eight alternating
barycenter sweeps reduce adjacent-layer crossings; the best strict inversion
count is retained, never worse than the initial lexical ordering. Shared edge
endpoints are not counted as crossings. This is a bounded heuristic, not an
optimal crossing guarantee: long edges are not split into dummy vertices and
the crossing score excludes them. Legacy cycle policies retain their ordering.

NodeGraph continues to consume caller positions and propose editor actions.
Routing uses measured node heights and measured socket rows. Cache comparison
includes socket anchors/directions/colors, routing mode, complete edge data,
resolved named colors and route metrics; an empty routing result is cacheable.
Endpoint lookup, route visibility and node-to-port lookup avoid repeated full
node scans. Cache hits compare borrowed slices without allocating new graph
keys and share retained route storage; only visible routes are cloned for
rendering. Owned input copies are retained only on a cache miss. Orthogonal
lanes validate against all node rectangles, including unrelated cards, before
searching for a detour. A graph-space bounds index narrows obstacle checks;
heading-aware search discovers obstacle coordinate planes lazily and stores
only visited labels rather than a complete Cartesian grid. Exact cardinal
port stubs remain fixed. Rounded turns reserve conservative clearance.

Failed routing retains the caller connection and its caller-owned `EdgeState`.
A separately identified, localized status warns either that no obstacle-free
route was found in this routing model (`obstructed`) or that the search budget
was exhausted (`search-limited`). The former is not a geometric impossibility
proof. Coordinate construction and visited labels share limits of 32,768 work
units per route and 1,048,576 per batch; these are not elapsed-time bounds.
Curved routing retains its existing behavior and does not promise all-obstacle
avoidance. Corner radius and stroke width participate in cache invalidation.
The `node-graph-routing` exhibit includes a global detour, a caller-moved buried
port and an adversarial search-budget fixture.

Segment/viewport
intersection retains routes that cross the viewport even when both endpoint
cards are outside it. GPUI's same-frame container query bounds card mounting
from the first frame and after resize, without waiting for a cached size.
Whole-graph fit still measures offscreen content; input building, geometry and
cache comparison still scan the dataset. The isolated CPU workloads below
do not establish dense-graph or native FPS support, or viewport-bounded total work.

`NodeGraph::source(GraphSource)` is an exclusive reusable input path. Cloned
handles share identity and revision; accepted mutation advances the revision,
but the host must request redraw. Canonical `GraphNode` metadata requires
explicit finite positive dimensions. Opaque one-use children are rejected;
`set_content` and `set_thumbnail` register factories built only for visible
cards, after releasing the metadata borrow. Legacy consumed nodes remain
supported. A later legacy node/edge builder switches back to that input path.
Upsert preserves insertion order and factories; failed validation changes
nothing. Removal drops factories, preserves caller edges (including dangling
ones), and reinsertion appends without resurrecting removed content.
Declared heights avoid offscreen mounting even for Fit; offscreen ports have
no mounted controls. Explicit-height cards do not allocate height-measurement
registry entries. A source keeps immutable canonical geometry snapshots keyed
by source identity, accepted revision, resolved theme identity and measured port
rows. Changed node revisions and changed port rows recompute only their own
geometry; factory-only publications reuse the snapshot. Animation copies the
snapshot only when displayed bounds differ, preserving current interaction
geometry without mutating retained caller snapshots.

Node visibility queries use the displayed-geometry bounds index; visible source
identities resolve directly to insertion ordinals before lazy construction.
Stable-cardinality node/route index updates refit changed leaves and ancestors;
after a node-count's worth of changed leaves (at least 64), a repartition limits
accumulated spatial degradation. Cardinality changes rebuild. Exact inclusive
query semantics and source painter order survive updates/removal/reinsertion.
Other metadata/port-measurement and routing comparisons still scan the model,
and a route-cache miss still performs global routing. These changes do not
claim viewport-bounded total work or dense 100k interactive performance. The
`node-graph-layout` exhibit publishes source retarget/removal/reinsertion.

Explicitly sized cards animate caller-applied bounds using the Navigation
policy; `.animate_layout(false)` disables this. Retargeting starts from the
displayed box. First mount, reduced motion and direct editor proposals snap.
Cards keep current caller content, and sockets, route construction, culling,
marquee and context-menu tests use displayed geometry. Automatic-height legacy
cards retain instant measured layout rather than animating an estimated size.
Changing motion preference settles node-state paint and does not restart
already settled edge entrances. Route topology still recalculates during
movement. Compatible lane corridors (same segment count and axis sequence)
interpolate from their displayed sample using Navigation timing. Retargeting
preserves the current corridor; endpoints and their first/last segment axes are
pinned to current displayed ports. Timing changes sample the old specification
first, then restart from that position with the new full duration and delay;
settled routes do not restart. Each sample passes the router's global
segment and rounded-corner clearance checks before paint, label placement,
culling and semantic targets consume it. They never use a target-only hit box.
Changed endpoint identity, incompatible topology, blocked/search-limited routes,
and invalid intermediate geometry snap to the solved route. In particular, a
route switching obstacle sides cannot crossfade through that obstacle. Curves
continue following sampled node geometry with their existing no-obstacle-routing
contract. Initial mount, source replacement, reduced/disabled motion and direct
manipulation do not delay routes. `node-graph-motion` exposes a lane-only change
to inspect intermediate motion and interrupted retargeting with fixed nodes.

Newly published or reinserted visible identities fade in with the Entrance
policy. Metadata/factory updates keep their current opacity without delaying
caller content. Initial mounting, viewport reentry, direct node manipulation,
reduced motion and disabled layout motion snap. Removing an entering card
freezes its last displayed paint, including the partial opacity, before exit.

Removed visible cards replay their last supported paint with the Exit policy.
The retained object is a framework paint recording, never an element or lazy
factory: text/button/image pixels may remain while handlers, focus targets and
semantics retire immediately. The picture follows the current graph viewport.
Optical glass, native surfaces, hosted views, deferred frames and any other
recording/replay refusal retire immediately, with no substitute shell. The
empty-graph branch also supports retirement of the last card. Reinsertion
discards the old picture; completion, reduced motion, disabled layout motion
and non-ready graph states release recordings. Panning offscreen drops cached
pictures rather than treating the node as removed or retaining visited content
forever. Source identity/revision avoids rebuilding the current-id set on hits;
this does not change the full-model geometry and routing work described above.

Removed visible connections also retain only their recorded stroke/marker
paint, including the current entrance reveal, colour crossover and traffic
phase. Their labels, warnings, disconnect target and captured input retire
immediately. The picture follows the current viewport; it does not continue
claiming live traffic. Membership follows routable connections, so removing an
endpoint retires the picture even when the caller retains a dangling edge.
Reinsertion, offscreen visits, capture/replay refusal, completed exit, disabled
layout motion and reduced motion release old recordings. The empty-graph path
supports the last connection. This visual retirement does not add route-path
morphing or change the caller's edge model.

## Raw time and normalized compatibility

`TraceSpan::new` retains normalized `start`/`end` coordinates and the default
viewport is [0, 1]. Raw callers set `.time(start_ms, end_ms)` on each span and
`.time_viewport([start_ms, end_ms])` on TraceView or SpanTimeline. Both use
`display::chart::scale::NumericScale` with `ScaleKind::Time`: UTC Unix
milliseconds, f64 mapping, fixed-duration ticks, no calendar or timezone
guessing. Reversed viewport domains retain their orientation. Nonfinite or
backward span intervals retain their row/identity but have no bar. Bars are
clipped to the viewport and instantaneous intervals use a one-pixel mark.

`.format_time` supplies host wording for automatically generated ticks.
Density follows the measured track width; labels occupy bounded 64-pixel
seats, thin when those seats overlap, and truncate rather than collide.
Legacy `.axis` and `.ticks` remain available for normalized host labels.
Durations and status never derive from the clock or tick formatter.

Ctrl-wheel proposes pointer-anchored zoom; shift-wheel proposes time pan.
Unmodified wheel scrolls the row list. Without `.on_viewport` no time gesture
handler is installed. Proposals never accumulate behind a refusing host.

Caller-applied time windows animate through shared `Transition<f64>` and the
Navigation motion policy. `.animate_viewport(false)` disables this; reduced
motion and orientation reversal settle directly. Both bar geometry and tick
mapping use the displayed window, while input proposals use the latest caller
window. Retargeting starts at the displayed extent, not the previous target.
The sampled extent must retain the target orientation: collapsed, nonfinite
or overshooting inverted endpoints snap both transitions to the caller target.
Built-in Navigation uses a tween; tests additionally force a crossing spring.
The row tooltip and accessibility description report caller-formatted times
(exact UTC milliseconds by default, or legacy normalized coordinates), the
caller duration, status and optional detail. `.format_time` owns human-facing
range/span readouts as well as ticks, including timezone wording and precision.
Range semantic endpoint values and events retain exact raw f64 values regardless
of that wording. `interval` semantic children
publish measured bar geometry for intermediate-frame assertions.

Opt-in headless playback verified both themes at exact simulated times: one
fixture bar had width 128px initially, 130.5px at 80ms, stayed 130.5px when
retargeted at that same time, then measured 130px at 160ms. Reduced motion
returned it to 128px without advancing time. Rendered default, intermediate
and collapsed states were inspected; this is renderer evidence, not native FPS.
Raw pointer hover followed by 1000ms simulated time displayed the decode
interval and caller duration 235ms in an unclipped floating tooltip in both
themes. The temporal fixture now formats its times relative to its epoch.

`.selected_time(Option<[f64; 2]>)` retains an ascending raw-time range;
`.on_time_selection` receives shared `RangeEvent` update/commit/cancel events
with create, move or raw start/end resize intent. The shared f64
`interaction::range` state machine owns arithmetic, acceptance and refusal;
Trace supplies measured controls and the existing NumericScale adapter.
Drag empty track to create, drag inside the range to move, shift-drag to
replace, or drag a handle to resize. Arrow keys step one percent;
home/end act on the focused endpoint or whole range. Reversed time
viewports retain raw start/end identity. Captured outside release clamps to
the time window; cancellation never rolls back caller-accepted values.
Disabled/read-only controls install no edit handlers. Off-domain selected
values remain visible in the exact readout with disabled handles, not silently
rewritten. Unaccepted edits are explicitly labelled as previews.
All built-in labels use the strings catalog. A Chinese/custom-format input
test checks selected, preview and empty labels, asymmetric fractional epoch
endpoints, outside release and refusal without changing the raw semantic data.

The displayed viewport and measured track freeze during capture, so animation
cannot move the time coordinates beneath a held pointer. A caller viewport
replacement cancels; normal release resumes navigation. Actual component
removal cancels through framework capture retirement, without adapter pre-cancel
or a second lifetime registry. Remount does not resurrect a draft. Raw pointer
playback exercised acceptance, refusal, outside release, cancellation, clear,
creation and intermediate/reduced-motion zoom in both themes (22 captures).
The linked read-only timeline kept caller values during a refused preview.

Hierarchy is the caller's depth-first preorder and `depth`, with collapsed
identities supplied by `.collapsed`. Disclosures and left/right keys propose
expanded state through `.on_toggle`; right on an expanded branch selects its
first child, and left on a leaf or collapsed branch selects its parent (even
when caller depths skip levels). Up/down/home/end propose selection and
scroll the target row into view. Focus moves to the persistent container when
navigation unmounts a formerly focused row. Hosts retain all data authority.

Surviving mounted rows animate accepted hierarchy changes through shared FLIP.
`animate_layout(false)` disables this independently of time-viewport animation;
`layout_animation` overrides the theme's Tracking timing. New rows wait until
survivors settle, then mount in their current slots. They install no handlers or
semantic targets while waiting, and removed rows disappear immediately without
retained paint. Retargeting starts at displayed positions. Caller selection
changes, scrolling and reduced motion settle immediately; initial mounting and
ordinary scrolling never wait. The retained publication set covers mounted rows,
not the full source. Both-theme timed renderer checks assert pairwise row bounds
do not overlap through collapse, interrupted expansion and final publication.

The default height is 12 rows; `.visible_rows` changes it. GPUI's uniform list
mounts visible rows plus measurement work, rather than every span.
`.shared_spans(Rc<Vec<TraceSpan>>)` avoids cloning input on redraw. Hierarchy
subtree boundaries, parent links, identity lookup and duration-column discovery
are built once per shared input identity. A retained range-cover tree supplies
kth-visible rows and selection rank in O(log N), without a flattened copy.
Each changed collapse identity updates its descendant interval in O(log N);
nested collapse remains effective when an ancestor expands. Comparing the
caller-owned collapse set is O(C), where C is its size. Replacing the Rc input
rebuilds the O(N) index and releases the prior source; this is not incremental
streaming ingestion. The index belongs to the existing window/owner registry,
not a separate global lifetime.

Focused index tests compare all 256 collapse subsets of an asymmetric nested
preorder in both transition directions against a direct filtering oracle. A
100,000-span branch collapse visits at most 70 segment-tree nodes; a retained
same-source update visits none. A local debug workload measured index build /
retained update at 1.24ms / 0.59µs (1k), 7.74ms / 1.38µs (10k), and 85.11ms /
1.52µs (100k). These measurements exclude mounting and painting. The integration
harness separately verified accepted nested collapse/parent navigation and
refused viewport/collapse proposals, with eight published rows and 123 paint
calls at all three sizes. Those structural counts are not a frame-rate claim.

## Focused workload evidence

Reproduce with an unoptimized all-feature build on the Linux test platform:

```sh
cargo test -p gpui-box-kit --all-features graph_workloads -- --ignored --nocapture
cargo test -p gpui-box-kit --all-features trace_workloads -- --ignored --nocapture
cargo test -p gpui-box-kit --all-features graph_mount_workload -- --ignored --nocapture
cargo test -p gpui-box-kit --all-features it::trace -- --nocapture
cargo run -p xtask -- performance check
```

One local run of a forest of 16-node chains (asymmetric dimensions) measured:

| Nodes | Edges | Layout | Routing | Borrowed cache-hit comparison |
|---:|---:|---:|---:|---:|
| 1,000 | 937 | 7.17 ms | 15.27 ms | 0.13 ms |
| 10,000 | 9,375 | 88.85 ms | 145.49 ms | 1.26 ms |
| 100,000 | 93,750 | 947.97 ms | 1,581.13 ms | 13.57 ms |

Those costs exclude mounting and painting. Separate explicit-size grid
workloads close each window before the next case: `Harness::frame` refreshes
all open windows, so dropping only the harness contaminated earlier reports.
These isolated results supersede the earlier multi-window timings:

| Nodes | Edges | Mount | Static redraw | Paint/prepaint calls |
|---:|---:|---:|---:|---:|
| 1,000 | 0 | 49 ms | 18 ms | 272 |
| 10,000 | 0 | 153 ms | 69 ms | 272 |
| 100,000 | 0 | 1,447 ms | 704 ms | 272 |
| 1,000 | 950 | 66 ms | 23 ms | 292 |
| 10,000 | 9,500 | 383 ms | 108 ms | 292 |
| 100,000 | 95,000 | 3,789 ms | 1,176 ms | 292 |

Connected cases are row chains, not dense graphs. These numbers remain
evidence **against** claiming a responsive 100k-node editor.

The trace workload published eight rows and exactly 123 paint calls at each of
1k, 10k and 100k spans. An isolated-window run measured mount at 25.4, 19.7 and
94.7 ms, and static redraw at 5.8, 5.8 and 6.5 ms, respectively. CPU input
construction, hierarchy processing, mount and redraw
are reported separately by the tests. No GPU submission, frame-rate guarantee,
dense graph benchmark or native macOS/Windows timing is claimed.

The repository performance ratchet also passes: `node-graph-material` uses
11,384 heap allocations against its unchanged 12,105 limit, with 1,005
paint/prepaint calls. This measures the existing 64-node material fixture,
not large-graph latency.

`node-graph-layout` reviews unequal dimensions and disconnected components.
`trace-time` reviews raw time, clipped intervals, virtual rows and shared
controlled windows. `trace` preserves normalized usage. New code is original;
no third-party source or framework primitive is imported by this stage.
