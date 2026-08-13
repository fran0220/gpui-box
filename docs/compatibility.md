# GPUI Box compatibility

## 0.1.x matrix

GPUI Box owns its framework and package compatibility. There is one package and
type universe: applications use Cargo package `gpui-box` as Rust crate `gpui`,
and `gpui-box-kit` as `gpui_kit`. Zed crates must not be mixed into that graph.

| GPUI Box cohort | Framework origin | Rust | Status |
|---|---|---|---|
| `0.1.x` | filtered bootstrap `fran0220/zed@0b9c8dc932b65cba2dc87464148984e93f60ae18`; official baseline `a6a23c7b80a5cefa0487b7856335be89ace7e483`; PlatformView fork overlay through `b46bf740a55c53612b14120f5dfbb7ceec463261` | 1.97, edition 2024 | `0.1.2` public release |

The SHAs identify imported source provenance, not Cargo Git dependencies.
`scripts/sync-zed/state.json` records the deterministic filtered bootstrap tip,
the official baseline cursor, and its exact integration merge. It separately
records the exact bootstrap-rooted fork overlay that supplies native
PlatformView hosting. Offline release verification checks both retained vendor
refs, every recorded source trailer and parent, their unique bootstrap merge
base, and both integration merges against the release commit's first-parent
history. It does not contact either historical source repository.

## Platform evidence and limits

| Platform | Current repository contract | Validation command/evidence | Limits |
|---|---|---|---|
| macOS | Framework, kit, Metal, clipped and paint-ordered native views, native AVFoundation audio/video playback, and deterministic headless catalog | CI runs native media load/control/replacement/teardown tests, the all-feature native check, and `cargo run -p xtask -- headless check`; real-window review remains separate | Playback supports operating-system codecs and unprotected sources; no DRM, track selection, capture, or application network policy; real-window accessibility needs a logged-in host |
| Windows | Framework/kit, clipped and paint-ordered native views, native Media Foundation audio/video playback, and deterministic WGPU/WARP headless catalog | CI runs native media replacement/teardown/COM/MF-lifetime and truthful no-backend tests on its audio-less hosted runner, the all-feature native check, and `cargo run -p xtask -- headless check`; release acceptance additionally runs load/control/end/restart on an audio-equipped Windows host; Windows baselines exist | Playback supports operating-system codecs and unprotected sources; native frame capture is not implemented |
| Linux | Wayland/X11 framework code and deterministic llvmpipe headless catalog | CI runs the all-feature native check and `cargo run -p xtask -- headless check`; Linux baselines exist | Native media service reports no-backend; AT-SPI and native behavior claims remain capability-scoped |
| Browser/WASM | Stable, single-threaded browser gallery, hosted compose surface, and lazy catalog embeds using the same Rust scenes | CI runs `web check` and the real Chromium gallery/site smoke; the visual command remains available for scoped review | Native media service reports no-backend; no threaded COOP/COEP claim and no screen-reader announcement coverage |

All four rows are mandatory CI surfaces. A release may claim only results
recorded for its commit; the commands do not erase the explicit limitations in
the final column. Accessibility capability details remain in
[`accessibility.md`](accessibility.md), and visual mechanics in
[`screenshot-testing.md`](screenshot-testing.md).

Linear-gradient dithering follows one rendering contract on Metal, Direct3D,
and WGPU: a screen-pixel-anchored unsigned integer hash produces the two
triangular samples, so GPU families and shader compilers cannot choose different
transcendental approximations for the noise. Color conversion and edge
rasterization remain renderer-specific, which is why each renderer retains its
own baseline rather than claiming identical bytes across platforms.

Browser checks are:

```bash
rustup target add wasm32-unknown-unknown
cargo run -p xtask -- web check
cargo run -p xtask -- web build
cargo run -p xtask -- web smoke
cargo run -p xtask -- web visual check button input dialog node-graph
```

The browser host is not a DOM rewrite. The catalog home remains selectable,
indexable HTML and retains committed captures as fallbacks; the live GPUI scene
is a lazy enhancement shown in both themes, while `/compose/` is the complete
interactive surface. The pinned Playwright smoke covers forced WebGL2, forced
WebGPU, automatic fallback, catalog embedding, static deep links, and the
compose route. Its AccessKit adapter mirrors roles, focus, actions, values, and
canvas-scaled bounds into semantic DOM, but the JSON semantic snapshot is only
a testing/debug surface.

## Framework development contract

GPUI Box is the sole framework and platform development authority. Changes are
implemented, tested, documented, and released directly from this repository;
Zed is neither a dependency, synchronization source, nor future compatibility
target. `scripts/sync-zed` is an offline verifier for immutable historical
attribution only. Its source list, filtered refs, and receipts are never
advanced or rewritten. A deliberate future source port must receive a new,
independent provenance record rather than reopening the retired import lane.
