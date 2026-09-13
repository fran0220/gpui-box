# Measured descriptive leaves without per-target layout

`gpui_kit_semantics::MeasuredLeafBatch` is for descriptions of many caller-owned
geometries, not controls. It uses one layout element and a real native Group,
publishing native children with the existing `A11ySubtreeBuilder` authority.
Diagnostics go to the installed, window-scoped semantic coordinator. No second
registry, renderer, clip engine, input route or retained leaf cache is added.

## Supply current-frame window geometry

```rust
use gpui::{Bounds, Styled, point, px, size};
use gpui_kit_semantics::{MeasuredLeaf, MeasuredLeafBatch, MeasuredLeafRole};

let targets = MeasuredLeafBatch::new("plot.descriptions", |area, _, _| {
    let measured = Bounds::new(
        area.origin + point(px(3.5), px(4.5)),
        size(area.size.width / 4.0, px(7.5)),
    );
    vec![MeasuredLeaf::new("plot.mark.alpha", MeasuredLeafRole::Image, measured)
        .text("Alpha")
        .value("42 units")
        .selected(true)
        .read_only(true)]
})
.diagnostic_parent("plot") // host separately publishes this diagnostic parent
.absolute()
.size_full();
```

As with ordinary `semantic_in`, install semantics once and begin the semantic
frame at the window root. Diagnostics require an active `SemanticCoordinator`
arm; native accessibility works without an arm or semantics installation.
The measurement callback runs at most once in current prepaint after layout.
When both outputs are inactive it does not run, so painting, picking and required
state updates must not depend on callback side effects.

Bounds are **untransformed logical window coordinates**, not batch-relative
offsets. Translate local geometry by the callback's `area.origin`. Keep caller
projection precision until converting into `Pixels`. The callback owns visibility
selection and geometry/viewport intersection, including holes, offscreen data,
opacity and retiring geometry. The batch cannot infer these from rectangles.

`MeasuredLeafRole` accepts only Image and Text. Text, literal description, value,
read-only state and diagnostic selection are supported. Credential-shaped text,
description and value use the same redaction as ordinary semantics. Image/Text
native selected remains unset, matching `Semantic` conventions. There is no
general `NodeSpec` conversion or API accepting focus, actions, ranges, checks,
live regions or non-topological relationships; those belong on ordinary controls.

## Identity, ancestry and clipping are explicit

- Use stable business-derived leaf ids, never indices into the visible list.
  Duplicate ids inside a batch panic rather than silently dropping a native
  leaf. The caller must also keep diagnostic ids unique within its window.
- The batch's stable mounted id plus each leaf key derive its native identity.
  Reordering, changing properties and removing other leaves keep that identity.
  Moving the batch to another mounted ancestry changes the native identity.
- Native leaves are direct children of the batch's real Group, beneath the
  enclosing mounted native parent. `diagnostic_parent` only supplies diagnostic
  parentage; it neither adds a diagnostic owner nor reparents native nodes.
- Diagnostics preserve the ordinary transformed-logical **unclipped** convention.
  Native bounds additionally receive GPUI's physical scaling and conservative
  rectangular/rounded ancestor clip. Rounded corners are not exact native shapes.
- While its owner is published, a fully clipped native leaf remains as a
  zero-area description at the clip edge. A fully clipped nonempty owner omits
  its native subtree. Omitted leaves, empty output or unmount disappear in the
  next completed frame; nothing retains prior targets by id.
- Per-leaf Taffy layout is absent, including its device-pixel snapping. Caller
  fractional geometry can therefore differ from prior absolute Div bounds by
  layout quantization. Do not round the geometry to imitate the old layout.
  Compare exact properties/keys, independently calculated geometry and rendered
  pixels as separate assertions.
- The batch installs no hitboxes, focus or input handlers. Its optional Styled
  container paint is ordinary GPUI style paint; the leaves themselves paint no
  primitives and cannot intercept parent pointer capture.

The public native inspection JSON now includes optional `bounds` with physical
`x0`, `y0`, `x1`, `y1`. These are the committed AccessKit coordinates, not a second
calculation. This additive debug field changes no native publication behavior.

## Mounted verification and reproduction

```bash
cargo test -p gpui-box-kit-semantics
cargo test -p gpui-box --lib
cargo clippy -p gpui-box-kit-semantics -p gpui-box --lib --tests -- -D warnings
cargo run -p xtask -- api generate
cargo run -p xtask -- api check
cargo run -p xtask -- dependencies check
cargo test -p gpui-box-kit-semantics --lib measured_leaf_work_breakdown -- --ignored --nocapture --test-threads=1
cargo test --release -p gpui-box-kit-semantics --lib measured_leaf_work_breakdown -- --ignored --nocapture --test-threads=1
```

Mounted tests compare ordinary semantic Divs and batches on exactly representable
coordinates, then independently verify nonzero current-frame origin/resize,
fractional visual transforms, native physical scale, clipped leaves and rounded
corner conservatism. With visual scale 1.25 around (−10,−5), native scale 2 and
ancestor clip x=10..152.5, y=15..75 logical, one leaf's diagnostic rectangle is
(150,56.25,16.25,11.25), while its native physical rectangle is
(x0=300,y0=112.5,x1=305,y1=135). Another fully clipped leaf remains native
(20,150,20,150), and a zero-size point stays (142.5,112.5,142.5,112.5).
A leaf inside the conservative rectangle but outside its rounded corner remains
native (22.5,32.5,25,35), proving the intentionally conservative contract.

Additional tests cover business-key reorder/subsets, changed readout properties,
redaction, reattachment, empty output, unmount, duplicate-key refusal, independent
output activation and diagnostic disarming. A compile-fail test rejects a Button
role. A mounted parent still receives pointer capture through the descriptive
batch even when native accessibility is active without semantics installation.

For 0, 1 and 10,000 targets, every output combination has identical fixture
request-layout/prepaint/paint counts: **3/3/3** (entity/root plus one batch).
The ordinary 10k Div fixture uses **10,002/10,002/10,002**. Both-off measurement
invocations are zero; otherwise exactly the current count is measured. Native
and diagnostic leaf counts are independently asserted outside benchmark timing.

The CPU benchmark uses independent windows for every old/batch × native ×
diagnostic case, closes and verifies empty windows between cases, and records
five warm `refresh_windows`/`run_until_parked` redraws. Each timed iteration must
advance the frame index and exhibit its expected element count. JSON export,
snapshot/count assertions and input construction checks are outside the timer;
render-time key/property construction remains included. Geometry is a 10-row,
source-ordered dense fixture of 10×10 rectangles. No native GPU or serialization
occurs inside the timer. Release uses the repository's thin-LTO/codegen-units=1
profile, not an isolated substitute profile.

### CPU samples on Linux x86-64, 2026-09-13

Base: [480336c3](https://github.com/fran0220/gpui-box/commit/480336c3fd5ea04823c8e06fe063edc5a260dd9b),
Rust 1.97.1. Entries are median microseconds [minimum..maximum] of five warm
frames. These are paired fixtures in this orb, not a comparison against another
worker's elapsed times. Variation between modes is not additive overhead.

| Native | Diagnostics | Debug Div | Debug batch |
|---|---|---:|---:|
| Off | Off | 361824 [359370..384327] | 163 [132..194] |
| Off | On | 368151 [358662..371018] | 24762 [14374..24928] |
| On | Off | 475598 [465930..506144] | 94431 [77106..99336] |
| On | On | 475888 [474417..479283] | 78981 [78586..80799] |

| Native | Diagnostics | Release Div | Release batch |
|---|---|---:|---:|
| Off | Off | 44647 [43832..46087] | 4 [4..10] |
| Off | On | 48342 [47185..58904] | 3691 [3671..3810] |
| On | Off | 68024 [67001..71254] | 10866 [10742..15755] |
| On | On | 72346 [71015..72965] | 11676 [11631..12350] |

Both benchmark profiles passed all mode counts and every timed frame-index and
layout-count assertion. The off/off batch does not construct leaf geometry or
properties; active modes necessarily remain O(n) output work and O(n) temporary
storage, with one fixed layout element. The fixture has no paintable leaf
primitives, so it does not attribute all whole-visualization work to semantics.

Validation passed: 31 semantics library tests (one benchmark ignored), one
compile-fail doctest, all 626 GPUI library tests, library/test Clippy with
`-D warnings`, API generation/check, and dependency-authority check. Catalog
generation produced current API/developer indexes locally; generated index
overwrites are deliberately omitted from the integration patches.

Platform screen-reader acceptance and full renderer/headless integration belong
to the coordinator's native lanes and geography integration. This Linux
TestPlatform evidence establishes CPU work and committed accessibility data,
not device touch, native presentation or FPS. Generated catalogs must be
regenerated after integration rather than overwritten with another checkout's
generated files.
