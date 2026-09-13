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

NodeGraph continues to consume caller positions and propose editor actions.
Routing uses measured node heights and measured socket rows. Cache comparison
includes socket anchors/directions/colors, routing mode, complete edge data,
resolved named colors and route metrics; an empty routing result is cacheable.
Endpoint lookup, route visibility and node-to-port lookup avoid repeated full
node scans. Cache hits compare borrowed slices without allocating new graph
keys; owned copies are retained only on a cache miss. The router still avoids the endpoint cards, not every unrelated
card in the graph. Routes with both endpoints outside the padded viewport can
still be culled even if an intermediate segment crosses the viewport. These
are existing limitations, not solved global obstacle/crossing support.

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

Hierarchy is the caller's depth-first preorder and `depth`, with collapsed
identities supplied by `.collapsed`. Disclosures and left/right keys propose
expanded state through `.on_toggle`; up/down/home/end propose selection and
scroll the target row into view. Focus moves to the persistent container when
navigation unmounts a formerly focused row. Hosts retain all data authority.

The default height is 12 rows; `.visible_rows` changes it. GPUI's uniform list
mounts visible rows plus measurement work, rather than every span.
`.shared_spans(Rc<Vec<TraceSpan>>)` avoids cloning input on redraw. Hierarchy
flattening and duration-column discovery still scan the input; this is bounded
paint work, not constant-time preprocessing or streaming trace ingestion.

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

Those costs exclude mounting and painting. The separate **edgeless** grid
mount workload measured 605 ms mount / 28 ms static redraw at 1,000 nodes and
10.40 s mount / 1.10 s redraw at 10,000 nodes. Settled paint and prepaint counts
were 271 in both cases; initial unmeasured geometry can mount the whole graph.
This is evidence **against** claiming a responsive 10k/100k-node editor.

The trace workload published eight rows and exactly 123 paint calls at each of
1k, 10k and 100k spans. One run measured static redraw at 5.0, 11.2 and 18.4 ms,
respectively. CPU input construction, hierarchy processing, mount and redraw
are reported separately by the tests. No GPU submission, frame-rate guarantee,
dense graph benchmark or native macOS/Windows timing is claimed.

The repository performance ratchet also passes: `node-graph-material` uses
11,383 heap allocations against its unchanged 12,105 limit, with 1,004
paint/prepaint calls. This measures the existing 64-node material fixture,
not large-graph latency.

`node-graph-layout` reviews unequal dimensions and disconnected components.
`trace-time` reviews raw time, clipped intervals, virtual rows and shared
controlled windows. `trace` preserves normalized usage. New code is original;
no third-party source or framework primitive is imported by this stage.
