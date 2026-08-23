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
| Windows | Framework/kit, paint-ordered custom caption controls and native drag/resize behavior, clipped and paint-ordered native views, native Media Foundation audio/video playback, and deterministic WGPU/WARP headless catalog | CI runs framework input tests, native media replacement/teardown/COM/MF-lifetime and truthful no-backend tests on its audio-less hosted runner, the all-feature native check, and `cargo run -p xtask -- headless check`; release acceptance additionally exercises real-window caption controls and borders plus media load/control/end/restart on an equipped Windows host; Windows baselines exist | Playback supports operating-system codecs and unprotected sources; native frame capture is not implemented |
| Linux | Wayland/X11 framework code and an all-feature native check | CI runs the all-feature native check. Headless pixel comparison is not a Linux gate; `snapshots/headless/linux` is retired | Native media service reports no-backend; AT-SPI and native behavior claims remain capability-scoped |
| Browser/WASM | Stable, single-threaded browser gallery, hosted compose surface, and lazy catalog embeds using the same Rust scenes | CI runs `web check` and the real Chromium gallery/site smoke; the visual command remains available for scoped review | Native media service reports no-backend; no threaded COOP/COEP claim and no screen-reader announcement coverage |

All four rows are mandatory CI surfaces. A release may claim only results
recorded for its commit; the commands do not erase the explicit limitations in
the final column. Accessibility capability details remain in
[`accessibility.md`](accessibility.md), and visual mechanics in
[`screenshot-testing.md`](screenshot-testing.md).

Renderer-backed linear, elliptical radial, and conic gradients accept two
through eight ordered color stops for both quads and filled paths. Radial
centres and radii are normalized to the painted bounds; conic gradients start
at the caller's CSS-oriented angle and proceed clockwise. Metal, Direct3D,
WGPU, and browser WebGL derive the same geometry and select the same adjacent
stop interval before applying sRGB or Oklab interpolation. Their dithering also
follows one contract: a screen-pixel-anchored unsigned integer hash produces
the two triangular samples, so GPU families and shader compilers cannot choose
different transcendental approximations for the noise. Color conversion and
edge rasterization remain renderer-specific, which is why each renderer
retains its own baseline rather than claiming identical bytes across platforms.

Read-only `StyledText` selection is a framework primitive rather than a Kit
gesture. A window-owned coordinator joins separately mounted participants in
caller-declared reading order, while stable business keys keep a selection on
the same content across reordering and re-virtualization. Selection scopes
isolate overlays from the page behind them. A copy crossing virtualized rows
contains only mounted text and reports `complete: false`; GPUI never invents
content it did not lay out. Pointer capture, grapheme-safe reverse drags, Copy,
Select All, and AccessKit selection continue to use the shaped text geometry of
each participant. A host that needs a complete copy of unmounted content still
uses the component's whole-value copy intent.

`ScrollTarget` gives overflowing containers, uniform lists, and measured
variable-height lists one offset, extent, viewport, and mutation contract.
Kit scrollbars bind to that target instead of wrapping a virtualized list in a
second scrolling container. A measured list freezes its reported extent while
the thumb is dragged, so newly measured rows cannot move the thumb away from
the pointer.

`Window::paint_sprite_batch` samples half-open physical-pixel rectangles from
one `RenderImage` frame and retains one atlas upload for all instances. Each
instance carries logical destination bounds, a center-relative transform,
rounded and source-alpha masking, tint/opacity, and normal, additive, or screen
compositing. Metal, Direct3D, WGPU, and WebGL apply the same transformed clip
and scene-culling contract; hardware blend state is selected per contiguous
paint-ordered batch. Invalid frame, source, destination, transform, or opacity
facts reject the whole call before any instance is painted. The primitive adds
no hit testing or accessibility nodes, and it does not claim subtree-wide
offscreen masks or blends. `RenderImage::from_rgba` is the public procedural
pixel boundary and hides the renderer's internal channel order.

`Window::paint_particle_batch` is a deterministic CPU sampler above that same
sprite ABI. One or more `ParticleEmitter`s use integer seed lanes and absolute
elapsed time, so sampling order and dropped frames do not alter birth times,
positions, dimensions, rotation, or opacity. A call admits at most 4,096
declared slots and validates every emitter and atlas source before scene
insertion. It performs no compute-shader simulation and creates no per-particle
element. Kit's `EffectParticles` is the policy-owned adapter: it maps
`EffectPlan` recipes to emitter topology, semantic theme colors and a built-in
procedural alpha atlas; reinforces tiny semantic marks toward the active text
tone for standard-surface contrast; mirrors directional traces under RTL;
schedules only while an animated recipe is live; and uses a fixed smaller
constellation for quality, budget, or reduced-motion fallbacks. Platform
renderers require no new particle pipeline because the final submission is one
ordinary sprite batch.

`CinematicEffect` is platform-neutral and available in every Kit build. It
maps the same semantic plans to Box-owned cinematic slots, timelines, poster
samples, directional RTL behavior, and particle fallback. The optional
`gpui-box-kit/dotlottie` feature links the crates.io `rasterlottie` 0.2 adapter
and accepts only host-resolved archive bytes. Before decoding, Box enforces hard
and host-tightenable limits for encoded size, entries, per-entry and total
expansion, compression ratio, canvas area, frame rate/count/duration, animation
count, state-machine count, embedded-image count, source dimensions, and
aggregate image target pixels; rejects traversal, duplicate paths, symlinks,
and encryption; inspects embedded image headers before full decoding; and
rebuilds a bounded stored archive from fully read entries. The adapter returns
deterministic RGBA samples and exposes no third-party type. A build without the
feature and any preparation/rendering failure remain truthful
runtime-unavailable or typed-error states and render the policy-owned fallback.
Reduced motion chooses the recipe's deterministic poster and owns no frame
timeline.

`PathBuilder::stroke_trim` keeps an ordered normalized interval of the measured
source path before Lyon expands the stroke. `dash_offset` advances and wraps a
validated dash pattern against that same measurement, so trim, phase, joins,
caps, transforms, clipping, and gradient paint remain one path contract. Empty
and all-zero dash arrays are solid strokes; negative or non-finite lengths and
invalid trim intervals are rejected at construction. Motion systems sample the
two numeric parameters and rebuild one path rather than approximating a trace
with one element per segment.

Read-only `StyledText` selection is a framework primitive rather than a Kit
gesture. Its stable element id retains transient anchor/focus state; pointer
capture continues reverse drags outside the element; grapheme-safe Copy and
Select All share the focused dispatch path; and AccessKit receives bidi-split
text runs, word starts, per-grapheme bounds, and stale-revision-safe selection
actions. Selection is scoped to one shaped value. A host that needs one drag to
span independently mounted or virtualized values still needs a future
document-selection coordinator rather than inferring bytes from row indexes.
`HighlightStyle::background_radius` paints each line or wrap fragment as its
own rounded quad, preserving range-highlight geometry without changing shaping,
wrapping, hit testing, or accessibility bounds.

Font fallback is also an explicit framework contract. `Styled::font_fallbacks`
inherits an ordered family chain; offscreen Cosmic shaping splits grapheme-safe
runs by registered-font coverage, macOS resolves registered family names, and
DirectWrite searches the application collection before the system collection.
Kit registers Geist plus Noto Sans Arabic and Noto Sans Hebrew and applies the
two Noto families to its type styles. Mixed RTL script output therefore does
not depend on fonts installed by a downstream host. Locale-specific copy,
number/date formatting, and language policy remain host-owned.

Grayscale glyph compositing shares one shader contract on Metal, Direct3D,
and WGPU: a tintable coverage mask in the sprite atlas, reshaped by
`apply_contrast_and_gamma_correction` before the hardware blend. The
parameters are per platform. DirectWrite and Cosmic coverage takes the
DirectWrite / Windows Terminal curve (default γ = 1.8, grayscale contrast
1.0). Core Text coverage takes identity parameters
(`TextGammaParams::identity`): Core Graphics bakes its gamma handling into
the mask and the macOS text system already dilates strokes per foreground
luminance to match AppleFontSmoothing, so a second reshape thickens and
smudges glyphs. The mask remains reusable across colors, so this is not
destination-aware Core Text or AppKit font smoothing. macOS windows still
report no subpixel support; subpixel mode stays a Windows / WGPU path. The
macOS swapchain layer is tagged `kCGColorSpaceSRGB`, so the window server
color-matches it like AppKit content instead of scanning untagged pixels out
in the display's native gamut.

Browser checks are:

```bash
rustup target add wasm32-unknown-unknown
cargo run -p xtask -- web check
cargo run -p xtask -- web build
cargo run -p xtask -- web smoke
cargo run -p xtask -- web visual check button input dialog node-graph
```

The browser host is not a DOM rewrite. The marketing home, the component
catalog, and the docs remain selectable, indexable HTML and retain committed
captures as fallbacks; the live GPUI scene is a lazy enhancement shown in both
themes on the home specimen, while `/compose/` is the complete interactive
surface. The pinned Playwright smoke covers forced WebGL2, forced WebGPU,
automatic fallback, catalog embedding, static deep links, and the compose
route. Its AccessKit adapter mirrors roles, focus, actions, values, and
canvas-scaled bounds into semantic DOM, but the JSON semantic snapshot is only
a testing/debug surface.

## Native external data drag and drop

macOS and Windows preserve the existing `ExternalPaths` contract for real
filesystem paths and additionally expose pathless native data as
`ExternalDrop`. Both platforms identify encoded images, UTF-8 text, URLs, and
promised or virtual files during hover without reading item content. Content is
read only after drop through a caller-supplied per-item byte limit; virtual
names are sanitized, and URLs are never fetched by GPUI. macOS uses dragging
pasteboards and file-promise receivers. Windows uses OLE `IDataObject`,
including PNG/DIB, Unicode text, URL, and
`FileGroupDescriptorW`/`FileContents` (`IStream` or `HGLOBAL`). A source that
also exposes a real path remains a path drop to avoid duplicate delivery.

Linux keeps its existing real-path drag support and does not yet publish the
new pathless payload. Web has no native desktop drag bridge in this cohort.

## Framework development contract

GPUI Box is the sole framework and platform development authority. Changes are
implemented, tested, documented, and released directly from this repository;
Zed is neither a dependency, synchronization source, nor future compatibility
target. `scripts/sync-zed` is an offline verifier for immutable historical
attribution only. Its source list, filtered refs, and receipts are never
advanced or rewritten. A deliberate future source port must receive a new,
independent provenance record rather than reopening the retired import lane.
