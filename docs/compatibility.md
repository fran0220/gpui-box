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

Inline deferred prepaint (including sticky content) preserves logical AccessKit
ancestors, source child order, synthetic children, and ancestor-based active
descendant focus independently of paint priority. Frame-local child
reservations attach to finalized ancestors without duplicate platform nodes.
Generic deferred overlays retain window-root accessibility parentage; inline
callers opt in with `Deferred::preserve_accessibility` or
`Window::defer_draw_with_accessibility`. Popover and hover-card surfaces remain
window siblings of their triggers.
While accessibility is active, visual view caches are rebuilt each frame:
their prepaint/paint ranges do not contain replayable accessibility trees.
Linux builder and sticky/nested-deferred integration tests cover these shared
invariants; browser smoke checks TreeGrid ancestry on WebGL2 and WebGPU. Native
macOS and Windows adapter validation remains the on-demand Platforms lane;
the shared change does not alter clipping, transforms, hit testing, or paint.

Windows native-view clips are independent child-local regions as well as a
host union; clipping never changes full layout dimensions. Empty frames hide
and empty the host, including subsequent move/resize synchronization. Saved
caller regions remain owned copies throughout restoration retries. If the OS
refuses teardown, GPUI hides and disowns the popup rather than destroying a
caller child. Explicit teardown can retry; final destruction deliberately
retains stranded controllers and the hidden popup (an exceptional resource
leak, not permission to destroy caller-owned windows).
Native regression command: `cargo test -p gpui-box-windows platform_view --lib`.
These HWND tests require Windows; Linux headless images do not prove them.

Pointer cancellation is a distinct `PlatformInput::MouseCancelled` event, not
a synthetic mouse-up: it cannot click or drop. Window broadcasts it regardless
of propagation, resets framework text/press/drag state, and releases capture.
Custom gesture listeners must discard pending state on `MouseCancelEvent`.
The legacy `Window::capture_pointer` retains its any-up policy; button-specific
`capture_pointer_for_button` and `on_mouse_down_with_pointer_capture` release
only on the matching button or cancellation. Framework text selection uses the
button-specific API and framework drag/drop remains left-button-owned.
Web reconciles the complete `buttons` snapshot on intermediate pointer moves,
and cancels only the active pointer on pointercancel, lostpointercapture, or
window blur. Normal final up followed by capture loss is idempotent. Primary
pen pointer compatibility is retained. Touch contacts use the distinct portable
multi-contact path described below rather than duplicate mouse events.
Windows retains OS capture until no mouse buttons remain, and delivers capture
loss separately after normal up dispatch. Shared regressions run with
`cargo test -p gpui-box --lib pointer_`; real browser cancellation and native
Windows device chords still require platform execution evidence.

Linux `gate` executes both `cargo test --workspace` and
`cargo test --workspace --all-features`, not only an all-feature compile.
Both Windows and macOS Platforms/Release native lanes execute
`cargo test --workspace --all-features --locked --exclude gpui-box-media`;
media lifecycle tests remain a separate invocation with their existing host
restrictions. These commands are required evidence, not a claim that an
unrun platform has passed.

| Platform | Current repository contract | Validation command/evidence | Limits |
|---|---|---|---|
| macOS | Framework, kit, Metal, native action context menus, clipped and paint-ordered native views, native AVFoundation audio/video playback, and deterministic headless catalog | On demand: the dispatched `Platforms` workflow runs native media load/control/replacement/teardown tests, the remaining all-feature platform tests and clippy, `performance check`, and `cargo run -p xtask -- headless check` on Metal; real-window review remains separate | Playback supports operating-system codecs and unprotected sources; no DRM, track selection, capture, or application network policy; real-window accessibility and context-menu presentation need a logged-in host |
| Windows | Framework/kit, native action context menus, paint-ordered custom caption controls and native drag/resize behavior, re-entrant native frame-request deferral, clipped and paint-ordered native views, native Media Foundation audio/video playback, and deterministic WGPU/WARP headless catalog | On demand: the dispatched `Platforms` workflow runs native UIA editing/focus/character/caret/form-description/menu-action smoke, native media replacement/teardown/COM/MF-lifetime and truthful no-backend tests on its audio-less hosted runner, the all-feature clippy, `performance check`, the Direct3D release-shader compile, and `cargo run -p xtask -- headless check` on WARP; release acceptance additionally exercises real-window caption controls and borders plus media load/control/end/restart on an equipped Windows host; Windows baselines exist | Playback supports operating-system codecs and unprotected sources; native frame capture is not implemented |
| Linux | Wayland/X11 framework code, the all-feature native check, and the deterministic llvmpipe headless catalog | Every commit: `cargo run -p xtask -- gate full` on the orb runs the all-feature tests and clippy, rustdoc, the structural performance budgets, and `headless check` against `snapshots/headless/linux` | Application context menus use Kit's in-window fallback; native media service reports no-backend; AT-SPI and native behavior claims remain capability-scoped |
| Browser/WASM | Stable, single-threaded browser gallery, hosted compose surface, and lazy catalog embeds using the same Rust scenes | Every commit: `gate` runs `web check` (wasm32 compile, warnings denied); `web smoke` drives the real Chromium gallery/site session when the browser gallery changes, and the visual command remains available for scoped review | Application context menus use Kit's in-window fallback; native media service reports no-backend; no threaded COOP/COEP claim and no screen-reader announcement coverage |

All four rows are mandatory surfaces, in two lanes recorded by
`compatibility.toml`'s `gate_lane`: `authority` (Linux and browser) runs in
`xtask gate` at every commit on the orb, `on-demand` (macOS and Windows) is
the dispatched `Platforms` workflow. No GitHub workflow builds on push. A
release may claim only results recorded for its commit, which for the
on-demand lane means a `Platforms` run of that commit; the commands do not
erase the explicit limitations in the final column. Accessibility capability details remain in
[`accessibility.md`](accessibility.md), and visual mechanics in
[`screenshot-testing.md`](screenshot-testing.md).

Native mobile source is **experimental and not an additional validated row**.
`gpui-box-android` provides an API33+ arm64 Vulkan Activity/JNI host, and
`gpui-box-ios` provides a UIKit/WGPU host with caller-supplied fonts. Portable
host checks exercise Rust types and local contracts, not Java/APK compilation,
Xcode linking, native frame presentation, keyboard behavior or accessibility.
See the [Android backend contract](../crates/gpui_android/README.md) and
[iOS native procedure](../examples/ios-native/README.md) for explicit limits,
SDK/device requirements and executable acceptance procedures.

Mobile browser tests run with
`npm --prefix examples/browser-gallery run mobile` after `xtask web build`.
`gate full`, `web smoke` and `web gate` build the gallery and run this suite.
They exercise Chromium touch delivery, keyless input/composition, lifecycle,
CSS/backing-size consistency and residual inset fixtures. They do not establish
Android Chrome or iOS Safari keyboard activation, autofill, native selection,
screen-reader or device lifecycle behavior. The exact boundary is documented in
[the browser mobile contract](../crates/gpui_web/MOBILE.md).

Checked `App::try_*` and `Window::try_*` operations preserve unsupported,
unavailable and refused outcomes before native dispatch. An accepted request
does not prove an OS transition completed. Mobile backends default to denial
unless they implement the operation. WGPU explicit detach/destroy drains native
resources before handle invalidation; device loss reports unconfirmed completion
while still releasing resources. Native teardown and recreation must be tested
on each platform before claiming lifecycle acceptance.

Accessibility action listeners are frame-local and registered only while the
window's AccessKit adapter is active for that frame. Adapter activation forces
a redraw before actions can be delivered, so inactive rendering does not add
unreachable handlers and active action behavior is unchanged.

Every native row also runs `cargo run -p xtask -- performance check`. The
command enforces per-window structural budgets over 10,000-item List,
DataGrid, TreeGrid, CodeView, LogStream, and AgentDocument fixtures and writes
`target/performance/report.json`. The browser build compiles the same fixed counters;
its renderer-specific timing lane remains separate from the structural gate.
See [`performance-testing.md`](performance-testing.md).

Live frame timing has the same per-window boundary. `FrameTiming` retains draw
work for benchmark consumers; `FrameSubmissionTiming` pairs the latest newly
drawn frame with the synchronous platform submission call, the first observed
invalidating input, and the number of coalesced top-level inputs.
`FrameTimingMonitor` retains a bounded history of those submitted frames and
does not create a refresh loop. Reference-counted trace leases keep concurrent
diagnostic and benchmark consumers independent. Submission completion means
the platform draw call returned, not that a compositor or display reported a
presented or dropped frame. The budget metric remains draw-only.

Client-drawn desktop titlebars share one framework contract. Nested `Client`
areas override an enclosing `Drag` strip, caption controls preserve their
platform identities, and `Window::request_close` follows the same vetoable
close path as native chrome. A native hit-test request carries its own current
position into the rendered-frame lookup instead of borrowing the previous
pointer event. Windows therefore maps maximize to `HTMAXBUTTON` on entry for
Snap Layout; macOS keeps native traffic lights; Linux reports the same requests
to its compositor-backed window implementation; browser builds expose no
desktop caption controls.

Transient notifications remain viewport-pinned, but a host can declare edges
already occupied by its own chrome through `ToastLayer::reserved_edges`.
`StatusBar::height` exposes the matching shared strip height, so applications
do not copy framework geometry to keep persistent notifications clear.

Inline sticky subtrees retain ordinary layout while GPUI translates their
prepaint, hit testing, accessibility bounds, and paint together against the
active content mask. Overflow containers can reveal a focused descendant with
physical edge insets reserved for those overlays. `DataGrid` and `TreeGrid`
use that contract for a single horizontal header/body/summary viewport and a
direction-aware frozen leading group; their virtualized vertical handle remains
independent.

Measured `Reveal` subtrees retain a stable natural primary-axis extent while
contributing caller-supplied progress to ordinary layout. The same content mask
governs paint and pointer delivery, and role-bearing bounds are intersected
with the active mask before AccessKit publication; a fully shut subtree is not
prepainted or published. `Reveal` owns no timer, easing, or component policy.
Kit's Accordion and Collapsible resolve semantic `Resize` motion and pass only
the resulting progress into this framework primitive.

Stateful springs also have one framework authority. `SpringConfig` analytically
advances position and velocity for fixed and steadily moving targets, and
`AnimationExt::with_spring` keeps that state under a stable element id so a
retarget does not restart from rest. A first mount starts at the target unless
the builder supplies `from`; pause retains velocity, stop discards it, complete
lands at the target, cancel returns to the initial value, and reduced motion
lands immediately. Finite eased values may exceed 0–1, so an underdamped
spring's overshoot is preserved. Kit retains the token, semantic-role, visual
settle, transition, presence, and FLIP policy above this scalar primitive.

Raw touch has one portable framework path. Every contact carries a stable
`TouchId`; native pointer indices are not identities. Single contact tap,
axis-locked scrolling/fling and claimed drag/long press remain compatible.
The first two unclaimed contacts promote to pinch, cancelling an existing
scroll. Pinch uses raw centroid and incremental span ratios, never predictions.
Coincident contacts retain the last nonzero scale baseline. Additional contacts
drain without becoming taps after the owner ends.

During paint, `Window::on_touch_pan` registers a stable window-unique owner ID.
An initial `Started` offer carries direction after slop. Later unconsumed live
scroll can offer `Started` with `is_scroll_handoff`; rejected offers have no
terminal event. Inspect `touch_start_position` for hit testing and use
`position - start_position` for travel: handoff rebases the latter to residual
movement only. `prevent_default` acquires and stops bubbling. Only the owner
receives later phases. A captured manipulation retains reversal and never
silently transfers back to scrolling. Frames with arbitration listeners use
raw scrolling positions so consumed plus residual equals actual travel.

`Window::on_touch_pinch` similarly captures an owner, even when the centroid
crosses into another view. Registering pinch and pan with the same ID permits
that owner's pan to cancel and promote to pinch; other claimed manipulations
retain ownership. Keep captured listeners registered until their terminal
phase, or call `cancel_touch_input` before removal. Inactive platform status
automatically cancels contacts, pending timers and already-released momentum.
This is exclusive arbitration for these primitives, not a rotation recognizer
or an arbitrary simultaneous-gesture graph. Shared tests are not native
device integration evidence. Platform-provided trackpad pinch remains separate.

`Window::insets` reports safe-area and IME edge avoidance in logical pixels
relative to the **current** content viewport. A resized viewport must not also
subtract the keyboard's full height; only residual overlap is reported.
`effective()` takes a per-edge maximum. Floating/split keyboard geometry is not
invented as full-width padding. Backend inset changes invalidate the window.
Text input hints and fallible actions flow through `EntityInputHandler`,
`InputHandler` and `PlatformInputHandler`; native text positions/ranges use
UTF-16 offsets and unsupported geometry is unavailable, not approximated.
Keyboard focus/options notifications follow the installed accepting handler.
Backends defer input-handler queries until these notifications return to avoid
reentering a borrowed core window. Autofill hints do not promise SMS access,
validation, or successful autofill.

Application-provided native context menus use the framework's existing
`Menu`/`MenuItem` action tree rather than a platform-specific component model.
macOS maps it to `NSMenu`; Windows maps it to `HMENU` and
`TrackPopupMenuEx`. Both run their blocking tracking loop only after GPUI's
current borrow has yielded, then dispatch the selected action through the
focus context captured at open time. The explicit unsupported result keeps
Linux, browser, headless, and other platforms on Kit's accessible in-window
`ContextMenu`; `ContextMenuPresentation::InWindow` also lets a host force that
portable rendering on a native-capable platform.

`Window::show_context_menu` returns an awaitable `NativeMenuSession`, not a
bare task. Retain `id()` for `Window::cancel_context_menu(id)` and revision
checks, and retain `effect_owner()` for asynchronous component callbacks.
Re-enter the captured owner with `App::with_effect_owner`; focus is not effect
authority. Framework command dispatch checks the latest application revision
and enters the captured owner independently.

Cancellation immediately invalidates commands, including already queued
selection, but `Ok(true)` only acknowledges the request. Completion confirms
tracking has exited. `Cancelled`, `Dismissed`, and `Unavailable` are distinct;
receiver loss/window destruction is unavailable, never a fake dismissal.
`NativeMenuError::NotSupported` is the only automatic fallback case.
`CancellationFailed` preserves the invalidated session for an explicit retry;
components must report refusal rather than pretending the native UI closed.
Cancel before changing a presented action snapshot. Each replacement gets a
fresh identity, and stale identities cannot cancel another owner's menu.

macOS cancels the tracked `NSMenu` with `cancelTracking`; AppKit supplies no
refusal return value, so loop completion remains the closure evidence.
Tracking is scheduled on the main NSRunLoop in common modes after the GCD
foreground task returns; entering the blocking menu from the serial main
dispatch queue would starve the very continuations needed to cancel it.
Windows guards thread-wide `EndMenu` by session and HWND. Both backends wait
for the previous tracking loop before starting a replacement, including queued
replacement chains and cross-window presentations, and invalidate on teardown.
Windows preserves menu-loop notifications so its existing `WM_ENTERMENULOOP`
timer drains foreground tasks inside `TrackPopupMenuEx`. `TPM_RETURNCMD` still
keeps command dispatch under the revision session; `TPM_NONOTIFY` would suppress
the entry notification needed to install the modal timer.
The shared deterministic lifecycle tests run on Linux. Actual native tracking,
replacement and cancellation need macOS/Windows execution; a Linux test pass
does not establish that native behavior. No renderer or baseline changes are
part of this primitive.

On a logged-in macOS/Windows desktop, run:

```sh
cargo run --locked -p gpui-box-platform --features test-support --example native_menu_smoke
```

This smoke opens actual OS menus and observes the backend inside
`NSMenu`/`TrackPopupMenuEx` before cancelling. It checks completed cancellation,
same-window and cross-window replacement, stale/wrong-owner cancellation,
destruction of a stale owner without closing the new menu, destruction during
active tracking, and successful presentation after teardown. The observation
API is test-support-only; simulated/queued menus cannot supply that evidence.
Every replacement must really enter native tracking, and every cancelled
session must complete with the specified outcome. An independent 45-second
watchdog fails on a blocked modal loop or shutdown, reporting the last phase.
Unsupported capability is a failure, not a skipped test or Kit fallback.
Capture both output streams as `target/native-menu-smoke.log` in each native
Platforms lane. Exit zero plus the final `native-menu-smoke: PASS` line is
required. Linux compilation/shared tests do not validate this smoke; native
execution must be recorded separately. This lifecycle smoke does not inject
OS selection/escape input or establish native command-selection accessibility;
queued stale-command and effect-owner invariants retain their shared tests.

Actual [Platforms run 34396298822](https://github.com/fran0220/gpui-box/actions/runs/34396298822)
at [02468534](https://github.com/fran0220/gpui-box/commit/02468534f3863e1f627559808f55128eb8bc8c4d)
failed this smoke on both macOS and Windows: each watchdog expired at
`cancel active native menu`, before observing tracking. The scheduling and
notification corrections above are candidates awaiting a new native run,
not established native parity. The smoke also exercises queued replacement
cancellation before invocation to guard the new run-loop scheduling gap;
all actual-tracking, cancellation, stale-owner and teardown assertions remain.

Native child views sit between GPUI's base and deferred-overlay scene planes.
Text on the opaque base plane retains platform subpixel rendering; text in the
transparent overlay plane uses grayscale antialiasing because RGB subpixel
coverage cannot carry meaningful alpha through native composition. Dialogs,
menus, prompts, drag previews, and tooltips therefore keep visible glyphs above
native surfaces without weakening ordinary document text. Cache reuse includes
the owning plane, and Direct3D converts any retained RGB coverage that still
crosses the split into one alpha-writing mask as the renderer's final invariant.
Direct3D path and backdrop passes restore the plane that invoked them after
using their scratch targets, so subsequent overlay batches cannot leak into the
opaque base scene.

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

A rounded sprite's antialiasing ramp is a property of its own geometry rather
than of its neighbours. Metal, Direct3D, WGPU, and WebGL all size the polychrome
sprite's corner mask from the analytic gradient of the same signed distance
field the mask uses, mapped to device pixels through the sprite's own
transform. That is the width a conforming `fwidth` reports for the smooth parts
of the field, and it is defined everywhere the field is — including along the
shared edge of the two triangles a sprite rectangle is drawn as, where a
screen-space derivative depends on how a backend reconstructs a helper
invocation and llvmpipe returns hundreds of pixels. Monochrome sprites, glyphs,
SVG masks, shadows, and quads never consumed that derivative; their coverage is
unchanged.

Scroll-edge fading uses those same painted bounds rather than treating every
primitive as one atomic mark. Solid quads and filled paths crossing one active
edge carry a per-pixel linear alpha ramp; a path's stops are normalized to the
clipped bounds consumed by every path shader. Shadows, monochrome SVGs,
polychrome images, and sprite instances expose uniform alpha, so primitives
larger than the band sample at the center of the portion inside the fade
region. Atomic primitives no larger than the band, including glyphs, retain
nearest-edge fading so they disappear before clipping can slice them.

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

`EditBuffer` is the shared editable-value authority for `TextInput`,
`TextArea`, and later rich editors. It owns grapheme-safe selection,
replacement and marked-composition transactions, grouped undo/redo,
single-/multi-line normalization, byte and grapheme limits, and UTF-8/UTF-16
conversion. Secret controls permanently refuse history. `EditableTextLayout`
is the corresponding shaped-geometry authority: the same wrapped and
bidirectional lines that paint answer byte/point hit tests, visual-row ranges,
selection fragments, aligned caret and hit geometry, platform range bounds,
and reveal scrolling.
`TextInput` and `TextArea` both consume it. `EditableStyleRuns<S>` keeps
grapheme-safe, complete, normalized caller-owned style coverage across source
replacement. Theme and document policy remain outside these framework
primitives. Kit's storage-neutral `RichTextDocument` now supplies stable block
identity, composite inline marks and links, paragraph/list metadata, and typed
edit intents. A caller-owned `RichTextEditSession` applies those intents and
owns composition and undo transactions without parsing or persisting a file
format. `RichTextEditor` projects the session through the same shaped geometry,
including selection, caret, IME, clipboard, diagnostics, links, lists,
alignment, and a token-backed toolbar.
All three editors capture the exact shaped cells during prepaint and publish
AccessKit per-grapheme positions, widths, and run bounds from that current-frame
geometry; they do not run a second accessibility layout.

Role-bearing elements can declare `aria_labelled_by`, `aria_described_by`, or
their inverse `aria_labels` and `aria_describes` forms. GPUI resolves the
referenced element id after ordinary and deferred prepaint in the active
window. A missing, duplicate, or removed endpoint produces no relationship for
that frame; no stale AccessKit node id is retained. Kit's `NodeSpec::labels`
and `NodeSpec::describes` project through this native relation path as well as
remaining visible in deterministic semantic snapshots. Because native adapters
do not consume the references uniformly, resolution also derives an absent
scalar label or description from the related node text. Explicit scalar values
still win and the references remain present.

`InteractiveElement::on_focus_resolved` runs after the element's subtree has
prepainted and receives the exact handle GPUI resolved during layout. An
explicit `track_focus` handle remains authoritative; `tab_index` elements use
their stable framework-generated handle; and non-focusable elements receive no
handle. Kit semantic diagnostics read focus through this observer, while
AccessKit continues to use the same existing handle and node projection.

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

## Focus visibility

`focus_visible` styling is placed by focus provenance, not by input history.
`Window::focus_from_pointer` is the move a pointer press makes and is the only
one that suppresses the styling; `Window::focus`, tab stops, focus traps, and
accessibility focus actions all leave it visible, so a dialog that moves focus
to its own action still says so. `Window::focus_is_visible` reads the current
answer. `Window::last_input_was_keyboard` is unchanged and still governs hover
suppression, which is about the pointer's position rather than focus.

Behaviour is platform-independent: it is decided in `Window` from dispatched
events, so macOS, Windows, Linux, and the browser host agree without any
platform reporting a focus modality of its own.

Backdrop glass is one material contract across Metal, Direct3D, and WGPU.
`GlassMaterial::blur_radius` controls scattering only: zero performs no
gaussian passes but still snapshots and composites clear refraction. A positive
radius derives a blurred source for both the interior and refracted rim; the
sharp paint-order snapshot is retained only for explicit edge-mask restoration.
`GlassMaterial::clear()` replaces the historical
zero-argument `frosted()` constructor; `GlassMaterial::frosted(radius)` names an
actual frost. The material also carries saturation, a straight-alpha colour wash,
transmission gain, additive optical lift, and hairline width. All renderers
apply Rec. 709 saturation to the sampled interior and refracted rim, clamp
negative channels, multiply transmission gain, source-over the material colour
wash, then add optical lift and edge light. Clear/frosted constructors default
to saturation 1 and transparent wash. Metal generates its packed material
layout from Rust; Direct3D and WGPU map it to aligned uniform registers.
The browser shares WGPU, not a separate glass implementation. Native Windows
and Linux validation of this extension is still required in their lanes.

Wash sanitization preserves RGB rather than collapsing it to a black/white
pole; each channel is clamped independently and non-finite channels become
zero. This reuses the existing three renderer colour paths without an ABI
change. Kit composes `Glass::tint` and `GlassGroup::tint` into that wash before
the optical rim, including fused bridges. Frosted keeps its existing fill.
Untinted Regular protects body-text contrast by default;
`protect_text_contrast(false)` preserves the configured material and makes
foreground legibility caller-owned. It is an explicit policy, not evidence
that an unprotected native-like control meets a body-text contrast target.

The independently authored `tools/liquid-glass-reference` harness records
public SwiftUI compositor output and validates source, image and timing
identities separately from GPUI candidate renders. Static trial selection
reports training, held-out size, cross-appearance and Clear-invariance scores;
neither a score nor native capture certifies GPUI equivalence to Apple's
private renderer. Native surface resizing and synthetic AppKit event receipt
are distinct from cross-view morphs and physical input.

Glass uses a shared elliptical height field: `thickness` (zero follows the
bounded bevel) times the signed `refraction` multiplier determines height and
its derivative. Positive refraction makes a convex cap; negative makes a
depression of the same maximum depth. `BackdropGlass::optical_bevel` bounds the
profile width by positive corner radii and half-extents without changing the
silhouette. Analytic rounded-rect and polynomial smooth-min derivatives retain
their magnitude through the union; only the final 3D normal is normalized.
Medial-axis ties choose an incident face rather than inventing a bisector.

Metal, HLSL and WGSL use Snell refraction with `refractive_index` in 1..=2.5.
RGB indices are `1 + (index - 1) * (1 - dispersion, 1, 1 + dispersion)`.
Index 1 is exactly undisplaced and has no Fresnel reflection. Rays intersect
an effective optical background plane at local height plus `backdrop_depth`;
there is no 45%-of-bevel displacement cap. The CPU sampling bound follows
`(maximum height + backdrop depth) * sqrt(maximum channel index² - 1)`, plus
one bilinear texel and Gaussian support. Content masks restrict output, not
the source information needed by a visible refracted pixel.

The same normal reflects the incident view ray into an analytic directional
environment. Schlick Fresnel blends reflection with transmission; `specular`
is its strength, no longer an independent additive highlight. Wash, gain,
lift and hairline remain explicit artistic controls, so the entire material
is not an energy-conserving physical volume. This is single-interface,
screen-space optics: `backdrop_depth` is distance in the refracted medium,
**not** a second-interface air gap. There is no scene-depth recovery, hidden
geometry, real external environment reflection, multiple internal reflection,
or caustic transport. Blur remains a spatially uniform scattering model.
It is not Apple's private Liquid Glass renderer.

The `glass-optics` exhibit isolates index, thickness, plane distance, dispersion,
Fresnel and scattering over a ruled fixture and includes a fused height field.
Existing `Glass`/`GlassGroup` press response changes this height and normal
together; pointer tracking changes the analytic light direction. Foreground
layout stays logical, while pressable content scales around its surface center
using the framework's paired prepaint/paint transform. Descendant hitboxes,
IME geometry and accessibility bounds follow the displayed transform. The
surface outline remains fixed. Reduced motion suppresses foreground scale.
Metal pixel regressions check the inner diagonal against adjacent face pixels,
retain the real arc highlight, and bound text-stroke variance at a blurred rim
against the flat interior. Blur-zero Clear remains sharp. These tests do not
substitute for the Linux/WGPU and Windows/HLSL native validation lanes.

Metal uses its platform gaussian when scattering is nonzero. Direct3D and WGPU
split wide gaussians into bounded passes and both degrade an over-budget blur to
the sharp source without dropping refraction or a requested luminance probe.
The browser uses the same WGPU path, including clear optics and bounded
scattering. Every backend retains full-size scratch textures and unchanged
viewport coordinates, but snapshot, blur, and composite work is clipped to the
integral visible surface plus three standard deviations of support for each
Gaussian pass and the maximum refracted and dispersed sampling reach. WGPU
evaluates each Gaussian weight once into a GPU-resident 65-entry texture and
reuses bind groups whose texture roles and uniform slot are unchanged; blur
fragments no longer evaluate an exponential for every tap. Metal publishes
luminance probes from command-buffer completion handlers, so a query returns
the most recently completed frame without waiting for the GPU and may remain
one additional frame behind when completion is late.

`Window::backdrop_statistics` exposes encoded RGB mean and encoded Rec.709
luminance mean, minimum, maximum and population variance from the same five
optical-source samples. These are not exhaustive backdrop extrema or linear
light measurements. Alpha is ignored, not used to unpremultiply or weight RGB.
Statistics share the lease and completion freshness of the scalar reading;
existing WGPU polling and Direct3D staging mapping can wait for completion.
Kit retains the last completed reading while a newer one is unavailable. Ring
shadow strength responds to mean darkness or twice the sampled standard
deviation, whichever is larger; sampled busyness cannot describe unsampled
content. Appearance hysteresis still uses the mean and only eligible compact
non-Clear controls may flip.

`Window::with_rounded_content_mask` explicitly scopes descendants in both
prepaint and paint using logical window coordinates. Rounded chains constrain
pointer input and primitive writes, including retained and deferred rendering,
without narrowing the rectangular optical source capture. AccessKit receives
only the conservative enclosing rectangle intersection. This does not change
Style overflow behavior or add rounded clipping to external native child views.
Linux tests exercise storage and integer-texture clip transport on a software
GPU; the latter is not a browser GL runtime test. The portable rounded-clip
pixel test, four glass-reference tests and seven Metal renderer tests also
passed on an exact source snapshot on macOS 27 build 26A5416b, M4 Pro, SDK26.2;
`compatibility.toml` records its SHA-256 identity. This does not exercise
CVPixelBuffer pixels or native Direct3D, certify a native full catalog, or
validate subsequent visual-scale changes.

`Window::without_content_masks` clears inherited rectangular and rounded masks,
restoring them after its scope without resetting transforms or logical ownership.
`Deferred::unclipped` opts in; default deferred clipping stays inherited. Kit
window overlays, popover helpers and toast layers use this explicit escape.
Their inner masks still apply. Later exact-source scale/frame tests and the
current release's platform evidence are recorded separately in
`compatibility.toml`; earlier clipping evidence is not reused as scale proof.

Independently, the scene admits at most 16 backdrop-glass surfaces per frame.
Valid surfaces past that paint their caller-supplied ordinary-fill fallback
and stay in cached paint ranges so a later paint-order change can admit them.
Rejected surfaces never issue luminance probes. Ordinary Liquid and Lens paint
no source-over fill while admitted; Kit supplies `effect.glassAlpha` as their
over-budget fallback. Frosted and the explicitly adaptive readability policy
already carry that fill. Adaptive surfaces begin with the safe tint and release
it after the first non-opposing probe reading, so first paint and renderers
without probe delivery do not expose content over an unknown backdrop.

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

## Editable layout ownership and lazy source geometry

`EditableTextLayout::painted_lines()` now yields `(Arc<WrappedLine>, Pixels)`
instead of `(&WrappedLine, Pixels)`. Ordinary `.paint(...)` calls continue to
work through dereferencing; callers storing a borrowed line must instead
retain the returned Arc. This is a source-level API change, recorded in the
generated developer index. Dense and lazy layouts share the same iterator.

`EditableTextLayout::unwrapped` retains a persistent document and shapes only
painted or explicitly queried hard lines. `bounds_for_range` still computes
exact logical geometry, including offscreen ranges. Selection rendering must
use `painted_bounds_for_range` to avoid shaping an entire selected document.
`text_width` is the maximum measured line width, not a measurement of unseen
lines. `shaping_work` counts actual submitted UTF-8 bytes and hard lines.

TextArea's new `document()` returns a persistent indexed snapshot;
`snapshot()` and `value()` retain their contiguous compatibility contracts.
This preserves caller behavior but does not eliminate their document-wide
cost. See `docs/coverage.md` for the remaining large-file paths.

The optional Kit `syntax` feature adds `Editor::syntax(EditorSyntax::json())`
or `EditorSyntax::new(language, query)` for caller-selected grammars compatible
with Tree-sitter 0.25.10. Consecutive `TextAreaEdit` events incrementally update
the retained tree using UTF-8 byte columns; skipped revisions reparse. Explicit
`EditorHighlights` for the current revision override parser colors. Parsing
is synchronous, with no process or filesystem access; error nodes are reported
through `EditorEvent::Parsed`, not silently converted to language diagnostics.
`EditorParseWork` counts borrowed input bytes offered and requests, including
repeated reads, and records whether a prior tree was reused. It does not
measure the parser's internal allocations or claim constant-time parsing.
The editor scene includes a JSON fixture when this feature is enabled.

## Native browser host evidence

The independent `gpui-box-webview` package uses Wry 0.57.0 with the installed
native browser engines. Linux X11/XWayland uses WebKitGTK 4.1 and GTK3; fresh
orbs and the release Linux gate install `libwebkit2gtk-4.1-dev`. Root and
headless workspaces continue to use the same local GPUI framework packages;
Kit/headless do not acquire a browser-engine dependency.

Linux native smoke has exercised full CSS layout, actual browser viewport,
script IPC/evaluation, HTTP navigation, back/forward/reload and native
connection-refused errors. X11 attachment tests separately cover clipping,
native pointer bounds, stacking and lifecycle. Windows GNU cross-compilation
passes; macOS compilation and both platforms' interactive browser validation
remain required. This is not native Wayland parity. See [webview.md](webview.md)
for the XWayland product route, unsupported composition operations, permission
policy constraints and exact reproduction commands.

## Editable viewport browsing

TextArea/Editor wheel and touchpad input can leave the caret offscreen.
Only editing and explicit selection/navigation reveal it again. Read-only
areas remain scrollable; disabled areas do not install a wheel handler.
Consumption uses `Window::consume_scroll_delta`, including independent axes,
partial-edge remainders and native line units, without swallowing overshoot.
The no-wrap horizontal extent retains the maximum width measured since the
last `set_value`, rather than shrinking when a shorter row enters the viewport.
This deliberately permits trailing blank space after shortening the widest
line; it avoids a horizontal jump while browsing and never pre-shapes unseen
lines to discover their widths.

Accessible text publication segments words once globally and indexes their
grapheme positions once, retaining word starts across visual, bidi, and
255-grapheme run boundaries. Bidi direction scans stop at the run limit.
Publication still builds a full-document accessibility tree and copies its
source snapshot: this removes quadratic segmentation, not all linear costs.
The platform-independent tests include mixed Unicode and 1,000/10,000-row
JSON fixtures with a byte-visit budget. Native accessibility adapter behavior
is unchanged; macOS and Windows execution remains a separate validation lane.

TextArea and Editor expose primary-first `selections`/`set_selections`, painted
`select_rectangle`, and atomic original-document range replacements. Overlaps
and duplicate carets merge; invalid batch boundaries are refused before edits.
Alt-click toggles a caret; Alt-Shift-drag selects painted columns and clamps
short rows to their ends. Navigation and typing operate on every selection;
one multicursor edit is one undo step. IME uses only the primary selection,
clears secondary carets during composition, and restores the previous set on
undo. Accessibility selection continues to describe the primary native caret.
Final-glyph hit testing now chooses the nearest start/end caret, including
multibyte single-glyph lines, instead of always choosing end-of-line.

`A11ySubtreeBuilder::retain_child` retains unchanged synthetic leaves only
while they remain connected across active frames. A removed or inactive-frame
leaf must be fully published before reuse; unchanged ids do not imply a live
platform node. `AccessibleTextCache` uses this contract for TextArea logical
runs. It rebuilds text segmentation on revision, row, or direction changes;
scrolling refreshes current and former viewport geometry, retaining offscreen
text payloads. Debug accessibility dumps accumulate incremental updates into
the complete tree rather than exposing only the latest delta.

This bounds text-run payload publication on scroll, not every frame operation:
parent child-id lists and live-id bookkeeping still scale with run count, and
a text revision still invalidates the logical cache. Compatibility whole-value
events and semantic values remain separate allocation work. Platform-independent
tests cover complete 1,000/10,000-row content, distant select-all endpoints,
bounded viewport cell queries, removal/reattachment and mid-frame deactivation.

Source migration: `TextAreaEvent::Change` and `EditorEvent::Changed` now carry
`gpui::EditSnapshot`, not `SharedString`. Read slices or indexed line/UTF-16
positions without a full copy; call `snapshot.text()` explicitly for the old
contiguous value. String signal bindings and MentionInput's existing string
event retain their compatibility conversion at the subscribing consumer.
Internal Editor forwarding does not flatten the snapshot.

`EditSnapshot::difference_from` computes a scalar-aligned single replacement
using borrowed chunks. Its `compared_bytes` and `shared_bytes` report actual
comparison and identity-skipped work; `inserted.len()` is the copied payload.
The regression edits a 100,000-row persistent snapshot, compares under 8 KiB,
and leaves both contiguous caches empty. Independent equal snapshots can
still require full comparison, and separated multicursor changes can span
unchanged text in the single-delta compatibility representation. This is not
a claim that rendering or new-revision accessibility publication is bounded.

Editor language services are opt-in via `language_services(true)`. Ctrl-Space,
F12, Ctrl-K Ctrl-I and Ctrl-. emit completion, definition, hover and code-action
requests carrying persistent snapshots and exact UTF-8 byte coordinates.
Callers publish `AsyncValue` replies against the request id; stale revisions,
duplicate identities, split-grapheme edits and overlapping batches are refused.
Completion and local code-action replacements use one shared undo transaction.
Definition targets and external code actions emit events; Kit never opens a
file, launches a server or executes a host command. Refresh errors retain the
last verified visible result but do not permit its acceptance. Navigation,
blur and text revision changes invalidate outstanding responses; an open
completion requests a new revision after typing. Diagnostics and semantic
tokens are revision-paired, with semantic colors overriding only covered
parser spans and severity underlines layered over both. The editor-services
exhibit uses explicit caller fixtures for ready, loading, refusal and hover.

`AccessibleTextCache::publish_document` accepts persistent snapshots and
resegments changed LF-delimited paragraphs, preserving unchanged offscreen
TextRun ids and payloads across byte/line shifts. It verifies unchanged row
mappings and falls back to a full segmentation when direction or wrapping
outside the edit changes. Native position lookup indexes the run first and
uses at most one run's selectable units instead of recounting the document
prefix. Old published snapshots still refuse a different current revision.
`TextArea::accessibility_work` exposes the last publication's segmented bytes,
compared bytes, published run/value bytes and retained run count. These are
not whole-frame allocation counters: complete source compatibility strings,
run metadata, parent child-id lists and native parent values remain linear.
The parent value is deliberately preserved: upstream AccessKit 0.24.1 omits
Windows Value-pattern support and macOS AXValue when a multiline input omits
its own value, even when all TextRun descendants remain available. Linux
platform-independent tests cover incremental native trees and Unicode
equivalence; macOS/Windows execution remains the separate platform lane.

TextArea keeps current logical text children connected on the first edit
relayout frame. Stale layout withholds painted geometry, not document content;
soft wrapping temporarily publishes complete hard rows until its new measured
rows are available. The mounted first-draw regression checks native value and
all 1,003 text runs before any settling frame, plus the combined former/current
viewport publication budget. This continuity is required for retained native
ids to remain reusable across real edits.

### Source folding preserves document coordinates

`EditableLineProjection` stores visible line spans rather than one entry per
source line. `EditableTextLayout::unwrapped_projected` shares the projection
across paint, clipping geometry, hit testing, selection and vertical navigation.
`AccessibleTextCache::publish_document_regions` keeps disjoint painted ranges
separate while retaining complete logical text and native Value/AXValue.
This does not make complete metadata or native parent values sublinear.

`Editor::set_folds` accepts revision-tagged, caller-identified hard-line ranges.
The header remains visible. Nested ranges are accepted, crossing ranges refused.
Gutter controls and Ctrl-Alt-F toggle transient state; a source edit invalidates
the ranges, and navigation into hidden text expands its ancestors. Callers
publish new ranges after parsing the edited revision. Disabled controls have
no toggle handler; read-only controls still allow browsing. The additive
`EditorEvent::FoldChanged` variant requires exhaustive consumers to add a case.
Line geometry reports original source line numbers, not projected row numbers.
The `editor-folding` exhibit reviews nested collapsed and expanded headers.
Linux framework/input checks cover projection and retained Unicode text;
macOS/Windows rendering and native adapter checks remain their dispatch lanes.

### Retained editor options preserve the editing session

TextArea, Editor and RichTextEditor expose mutable option setters; hosts do
not need to replace an entity when layout or service policy changes. The
`editor-options` exhibit applies these options after construction. Text,
selection, IME composition and history remain attached to the original session.
Wrapping/size changes invalidate measured geometry without resetting text.
Disabling keeps the existing refusal/cancellation semantics; re-enabling does
not steal keyboard focus.

`TextArea::set_max_length(None)` removes the future-input byte limit. Lowering
the limit never truncates existing text or history. Optional row caps on both
text editors accept `None` to keep fixed minimum-row height, including after
later minimum-row changes; TextArea's optional autosize pair can also be removed.
Editor owns its retained rows/read-only/disabled state and updates its child
immediately, so rendering does not overwrite host transitions. Optional
indentation and syntax providers accept `None`; disabling language services
removes outstanding popups and releases claimed completion/navigation keys.
Native full-document values remain complete; these setters make no additional
large-document performance claim.

### Explicit touch sizing is not mobile platform certification

`ControlSize::Touch` selects the new required `control.touch` token step. Custom
theme JSON must add that step; exhaustive Rust matches on `ControlSize` must
handle the new variant. Bundled themes use a 48 logical-pixel target height,
16px text and 20px icons. Compact density leaves touch metrics unchanged, while
explicit `Theme::scaled` scales them like other subtree geometry.

Desktop size defaults are unchanged. Components opting into Touch must retain
usable nested actions and allocate actual target width, hit bounds and accessible
bounds consistently. The token does not expand an arbitrary element's hit area
or establish mobile keyboard, screen-reader or device support. Native platform
and mobile component validation remains separately required by the delivery
ledger in `tasks/mobile-platform-delivery.md`.
