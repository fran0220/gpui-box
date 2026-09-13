# BoundsTree balance and CPU evidence

This is index-only Linux/x86-64 orb evidence from 2026-09-13, against the
pre-change [base](https://github.com/fran0220/gpui-box/commit/53ddf792b9ec641d8c5d3bcb47fb22c532f50a09).
It measures neither native GPU performance nor FPS. Scene recording, renderers,
resource leases, geography components and presentation are outside this change.

## Contract and algorithm

Insertion assigns exactly one plus the maximum order of intersecting prior
bounds; no intersecting bounds means order one. Search calls the existing
`Bounds::intersects`: touching edges do not intersect, but a zero-size point
strictly inside a nonempty rectangle does. The global-max shortcut is valid
even with ties because any intersecting global maximum proves the answer.
A miss must search for other leaves, including tied maxima.

All leaves now have the same depth. A full internal node splits its 13 children
into groups of 6 and 7, sorting a fixed scratch array by origin along its longest
bounding-box axis. A sibling propagates to the parent, recursively in concept
but iteratively in code; an overflowing root creates a new root. Other internal
nodes have 6–12 children; the root has 2–12. Descent minimizes half-perimeter
enlargement, then union half-perimeter, with stable ties. Metadata is recomputed
from children; a maximum-order child is placed last for stack traversal.

For n leaves and height h measured in root-to-leaf edges, a non-leaf root requires
at least 2 × 6^(h−1) leaves. Descent and propagation are O(log n); each level does
bounded work on at most 13 children. A split uses fixed-size insertion sort,
O(13²), without requiring `Ord`, `Copy`, coordinate conversion or new arithmetic
traits on units. Search remains O(n) worst-case when subtree bounds overlap;
balanced height does not promise logarithmic spatial queries. Storage is O(n).
The nodes vector retains leaf indices across growth and splits. Search uses
indices rather than raw pointers. Both traversal stacks and node allocation
capacity are retained by `clear`; there is no recursive traversal or rebuild.

## Reproduce the isolated comparison

```bash
cargo test -p gpui-box --lib bounds_tree
cargo clippy -p gpui-box --lib --tests -- -D warnings
cargo run -p xtask -- dependencies check
bash crates/gpui/src/bounds_tree/benchmark.sh
```

The script compiles the actual old/new index source against the same GPUI
`Bounds`/geometry implementation. Only test instrumentation is inserted in the
old source: one counter at insertion-descent loop entry and one at search stack
pop. Current source has matching `cfg(test)` counters. These are not all CPU
operations: they exclude child scans, metadata unions, splitting and the single
global-max check. Height, ownership and metadata are inspected outside timing.
Input generation and order-oracle checks are also outside timing. Every case
constructs a separate tree and includes its insertion allocations in timing.

`release` in this script means index and generic geometry monomorphizations
compiled with `rustc -C opt-level=3 -C debug-assertions=no`; `debug` means
opt-level=0 with debug assertions. Linked GPUI dependencies use the test profile.
This isolates the index without a whole-workspace thin-LTO build and must not be
confused with an application's `cargo --release` timing. Timings below are one
sample, not statistical performance guarantees. Compiler: Rust 1.97.1.

Logs and complete little-endian u32 order vectors are written under
`target/bounds-tree-bench/`. The script compares every order byte-for-byte
between old and new in each mode; it does not infer equality from a checksum.

## Structural evidence exposes the old degeneration

| Workload | n | Old leaf depth | New leaf depth | Old insertion descents | New insertion descents | Old search pops | New search pops |
|---|---:|---:|---:|---:|---:|---:|---:|
| Sorted horizontal | 1,000 | 1–91 | 4 | 45,773 | 3,431 | 999 | 999 |
| Sorted horizontal | 10,000 | 1–909 | 5 | 4,548,636 | 46,580 | 9,999 | 9,999 |
| Sorted horizontal | 100,000 | 1–9,091 | 6 | 454,577,273 | 579,473 | 99,999 | 99,999 |
| Fully overlapping | 1,000 | 1–7 | 3 | 5,546 | 2,840 | 0 | 0 |
| Fully overlapping | 10,000 | 1–10 | 4 | 87,824 | 38,253 | 0 | 0 |
| Fully overlapping | 100,000 | 1–14 | 5 | 1,203,544 | 480,940 | 0 | 0 |
| Dense geographic fixture | 10,000 | 2–92 | 5 | 461,918 | 46,172 | 8,172,580 | 401,659 |

Horizontal bounds are (3i, −17, 2, 7), so every order is independently known
to be one. Fully overlapping bounds are (−3, 7, 11, 19), so order i is i+1.
The fully overlapping fast path explains its zero search pops; it does not
eliminate insertion descent. At 100k, horizontal node count grows from 109,091
to 119,994 for minimum split occupancy; fully overlapping node count falls from
116,383 to 109,978. This is a storage/time tradeoff, not free balancing.

The dense fixture inserts i=0..9,999 in source order with longitude
−179+(i mod 1000)×0.358 and latitude −80+floor(i/1000)×1.6. Projection is
x=(longitude+180)/360, y=0.5−latitude/360. Screen centers are
(320,140)+(projected−projected[5000])×280; destinations are 10×10 bounds centered
on them. Every assigned order is additionally checked with an independent
quadratic strict-overlap oracle. No component batching or paint-order shortcut
is involved.

## Optimized CPU samples

| Workload | n | Old ms | New ms |
|---|---:|---:|---:|
| Sorted horizontal | 1,000 | 2.078 | 0.495 |
| Sorted horizontal | 10,000 | 218.474 | 4.549 |
| Sorted horizontal | 100,000 | 33,572.595 | 55.683 |
| Fully overlapping | 1,000 | 0.237 | 0.332 |
| Fully overlapping | 10,000 | 4.206 | 3.190 |
| Fully overlapping | 100,000 | 85.537 | 46.027 |
| Dense geographic fixture | 10,000 | 70.981 | 5.264 |

Small fully overlapping submissions can be slower because splitting and exact
metadata maintenance cost more despite fewer descents. The result establishes
bounded insertion height, not a speedup for every input.

## Debug CPU samples and completed checks

| Workload | n | Old ms | New ms |
|---|---:|---:|---:|
| Sorted horizontal | 1,000 | 34.732 | 3.605 |
| Sorted horizontal | 10,000 | 3,569.945 | 49.675 |
| Sorted horizontal | 100,000 | 364,596.215 | 643.978 |
| Fully overlapping | 1,000 | 4.332 | 2.576 |
| Fully overlapping | 10,000 | 70.050 | 35.482 |
| Fully overlapping | 100,000 | 1,225.405 | 469.096 |
| Dense geographic fixture | 10,000 | 1,062.208 | 70.033 |

Both modes produced the same structural counters above. All seven old/new
order-vector comparisons passed byte-for-byte in both modes, including all
100k cases. `cargo test -p gpui-box --lib` passed 626 tests, with only the
isolated CPU benchmark ignored. Focused BoundsTree tests also passed with
`--no-default-features`. Default and no-default-feature library/test Clippy
passed with `-D warnings`. `dependencies check` reported “package identities,
dependency graphs, compatibility, and provenance records agree”. Rustfmt,
benchmark shell syntax and `git diff --check` passed.

## Correctness and remaining integration checks

Focused tests independently derive orders for asymmetric horizontal, vertical,
reversed, row-major grid, fully overlapping, nested, disconnected, touching and
zero-size inputs. Seeded random tests compare both insertion and later queries
against a naive oracle and verify deterministic node construction. Existing
1,000-seed fractional-coordinate coverage is retained. Structural checks cover
root splits, occupancy, unique ownership, reachability, metadata and equal leaf
depth, with six 100k workloads. Tied maxima, retained clear/reuse capacity and
non-`Copy`, partially ordered units have direct tests.

The renderer/headless catalog and the full workspace gate must be checked by
the coordinator after integration with independently owned Scene work. Native
macOS/Windows lanes have not run in this Linux worker. These CPU tests make no
new native-platform or rendered-frame claim and do not attribute all geography
paint or semantic-bound costs to this tree.
