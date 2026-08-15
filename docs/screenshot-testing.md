# Screenshot testing

## Two captures, two questions

The `gpui_kit_testkit::capture` module from Cargo package
`gpui-box-kit-testkit` answers two different questions with two functions.

`render_frame` (behind the `test-support` feature) re-renders the scene GPUI
drew last into an offscreen texture and reads the pixels straight back. The
window server, the window's position, its rounded corners, and whatever else
the compositor does never touch the result, which is why two captures of the
same scene agree to the byte. This is what the visual regression gate uses.

`capture_window` asks the macOS window server for the specific window owned by
the process — what was actually composited to the screen. It excludes framing
and returns RGBA8 pixels for the content area. Use it when the question is
about a real product window on a real display, not for regression baselines.

Do not use full-desktop capture for automated evidence:

- it may trigger OS consent prompts;
- it captures unrelated user content;
- it depends on z-order;
- semantic bounds no longer map directly to image pixels.

Non-macOS capture currently returns `Unsupported` from `capture_window`, and
`render_frame` needs a platform window that implements GPUI's
`render_to_image`, which today is macOS. The visual baseline does not go
through either of them: supported native platforms hold it through the
headless gate described below, which renders without any window at all. Image
writing, semantic assertions, and frame comparison remain portable.

## Settle before capture

After an action:

1. wait for the semantic generation to advance;
2. refresh the window if the fixture changed directly;
3. allow entrance, caret, and composition frames to settle;
4. capture the product-owned window.

Capturing immediately after input can return the previous frame with valid
bytes and no obvious error.

## Fixed fixture contract

Visual baselines state:

- viewport;
- theme;
- fixture identity;
- interaction state;
- platform and scale factor;
- reduced-motion setting.

Active baselines are 1840×2000 device pixels on each supported native platform:
920×1000 logical at a scale factor of 2. That is a constant, because the gate
asks the renderer for that size rather than opening a window and accepting
whatever the display grants.

It used to be otherwise, and the cost is worth recording. The gate opened a
real 920×1000 window and captured its drawable, whose size the platform clamps
to the available screen area. Two Macs with different menu bar and Dock
geometry produced 1840×1568 and 1842×1374, so `snapshots/macos` accumulated two
incompatible sets, a full-catalog check could not pass on either machine, and
every wave was reviewed with a scoped check instead. The baselines described
the machine, not the components.

## Frame comparison

`compare_frames` reports:

- changed pixel count;
- changed ratio;
- maximum channel delta;
- mean channel delta.

Use a small channel tolerance for rasterization noise, but inspect meaningful
changes. A single global percentage is not enough for tiny controls; semantic
bounds allow focused crops when needed.

## Visual and behavioral proof

Screenshot tests prove appearance. They do not prove:

- a callback fired;
- a file was written;
- a request was sent;
- the host accepted the action.

Behavioral tests assert those outcomes separately. A fixture screenshot is not
a host smoke test.

## Scenes

`gpui_kit::scenes::catalog()` is the single description of each component's
states. The gallery renders a scene with `--scene <name>`, and

```bash
cargo run -p xtask -- scenes list
cargo run -p xtask -- scenes render          # or: scenes render list tree
```

writes one image per scene per bundled theme into `target/scenes` for a person
to look at. One process renders the whole catalog on the window it launched
with, because a GPUI application owns the window system for its lifetime and
paying application startup per image cost over twenty minutes. A run takes an
exclusive lock: two galleries rendering at once take the foreground from each
other, and a window the platform has pushed to the background stops being
scheduled for draws, so both runs stall on stale frames.

`scenes render` is a viewing tool, not the gate. It is how motion and the text
caret get reviewed, because both need a real window. It holds no baseline.

## The gate

```bash
cargo run -p xtask -- headless check          # or: headless check list tree
cargo run -p xtask -- headless capture        # accept
```

`tools/headless-visual` renders each scene into an offscreen texture at a size
it names and reads the pixels straight back. No window, display, menu bar,
dock, or compositor takes part, so any machine with the same renderer produces
the same bytes. Baselines live in `snapshots/headless/{macos,windows}/scenes`,
one set per renderer, because Metal and WARP land antialiased edges
differently. Both sets are active platform-specific gates and must be
captured on the renderer they represent. A retired llvmpipe set remains under
`snapshots/headless/linux` from when CI compared it; no job runs it today.

Text is shaped by cosmic-text from the bundled fonts only. Loading the
machine's own fonts would shape text differently from one machine to the next,
which is also why `crates/gpui-kit-assets` bundles `KeySymbols.ttf`: without
it the macOS `⌘ ⌃ ⌥` glyphs came from whatever font the host happened to have,
and the gate rendered them as missing-glyph boxes. Noto Sans Arabic and Noto
Sans Hebrew are bundled and named in every Kit type style for the same reason;
the `reading-direction` scene exercises mixed RTL scripts, Latin, punctuation,
paths, and numbers without consulting machine fonts.

Gradient noise is deterministic for the same reason. Linear, radial, and conic
gradients on Metal, Direct3D, and WGPU derive their triangular dither from the
integer identity of the screen pixel. The former float `sin` hash was not a
reproducible contract: different Metal GPU families produced different random
fields even though the gradients and geometry were identical. The integer hash
fixes the field without raising the comparison tolerance or weakening the
component gate. It does not make color conversion identical across renderers,
so the three baseline sets remain separate.

A comparison allows one step per channel. Exactness was tried and does not
hold: capturing `frost` alone and capturing it as the ninetieth scene of a full
run differ by one pixel at one step, because the sprite atlas has accumulated
different state by then. Scoped runs agree with each other to the byte, so the
tolerance is what makes a scoped check mean the same thing as the full one.
Naming scenes checks or captures only those, which is what a change to one
component needs.

A failing run writes only its changed or new actual images to
`target/headless-scene-check`, which CI uploads as an artifact. Passing frames
are compared directly from memory instead of being PNG-encoded and decoded
again; a difference nobody can look at is still not a review.

The same catalog is rendered headlessly by `crates/gpui-kit/tests/scenes.rs`,
which audits every published tree, so a component cannot be reviewed visually
in one arrangement and tested in another.

## Downstream theme and gallery contract

An application owns its token documents and baselines; it does not copy the
token structs or maintain another schema. For each complete dark/light pair:

1. Parse or register both JSON documents with `TokenDocument::parse` or
   `ThemeRegistry::register_json`. Parsing rejects missing and unknown fields,
   invalid token values, and all required contrast failures before the
   registry changes. Assert that the documents declare one `Appearance::Dark`
   and one `Appearance::Light`; ids and display names remain application-owned.
2. Render `gpui_kit::scenes::catalog()` in its returned stable order. For each
   scene, render the dark and light ids next to each other. Set reduced motion,
   use the scene's `gpui_kit::scenes::direction`, park the pointer, and discard
   one warm-up frame before settling and capturing. Use a fixed logical
   viewport and bundled fonts only. These are the same determinism rules used
   by the repository gallery and headless harness.
3. Audit the semantic snapshot with `gpui_kit_testkit::audit_or_error`. Its one
   error contains every failing node id and invariant; do not replace this with
   source-text assertions or a hand-maintained list of expected ids.
4. Name each captured frame from stable fixture identity, for example
   `format!("{}-{}", scene.name, theme_id)`, and pass it to
   `VisualBaselines::check`. `VisualBaselines::capture` is the explicit accept
   operation. A mismatch reports the name, path, changed-pixel count and ratio,
   and maximum and mean channel deltas before a reviewer opens the PNG.

```rust,no_run
use gpui_kit_testkit::VisualBaselines;

# fn check(frame: &gpui_kit_testkit::capture::Frame) -> Result<(), Box<dyn std::error::Error>> {
let baselines = VisualBaselines::new(
    std::path::Path::new("snapshots")
        .join(std::env::consts::OS)
        .join("gpui-kit-gallery"),
);
baselines.check("button-application-dark", frame)?;
// Deliberate acceptance is separate:
// baselines.capture("button-application-dark", frame)?;
# Ok(())
# }
```

Keep native macOS and Windows baselines separate. `render_frame` reads a native
window where that platform implements GPUI's readback contract; the repository
uses the same public scene catalog with its offscreen software renderer for
Windows CI. A downstream product may supply another product-owned
`Frame`, but must preserve the fixed viewport, scale, fonts, motion, pointer,
fixture, theme, and platform in its baseline contract. A software-rendered
gallery baseline does not replace native interaction and accessibility evidence
for a product surface.

## Browser visual gate

The browser gate enumerates `gpui_kit::scenes::catalog()` and the
bundled themes at runtime; it carries no hand-maintained scene or image count.
Its fixed contract is the browser gallery's logical viewport, DPR 1, reduced
motion, bundled fonts, each scene's declared direction, a pointer parked
outside the canvas, and one discarded warm-up frame. Future captures remain
separate from native and headless baselines. Capture and check either the full
runtime catalog or a scoped list:

```bash
cargo run -p xtask -- web visual capture button input dialog node-graph
cargo run -p xtask -- web visual check button input dialog node-graph
cargo run -p xtask -- web visual check # every catalog scene in every bundled theme
```

The repository accepts a two-theme baseline for every scene published by the
runtime catalog. A second full run reproduced every image exactly, and the
complete set was visually inspected through labeled contact sheets with
full-resolution review of suspected anomalies. The separate Chromium smoke
verifies real pointer and keyboard paths plus the AccessKit DOM bridge; the
diagnostic semantic snapshot is used only for stable identity and bounds
correlation.

## The headless gate

`tools/headless-visual` renders the same catalog with no window system at all:
GPUI's renderer draws each scene into an offscreen texture and the pixels are
read straight back. Metal is used on macOS; llvmpipe and WARP provide software
adapters on Linux and Windows, so those gates run on headless CI machines with
no discrete GPU. Text is shaped from bundled fonts only, and time is simulated.
Repeated runs are stable within each renderer and compare against that
platform's baseline.

```bash
cargo run -p xtask -- headless check     # compare against the baseline
cargo run -p xtask -- headless capture   # accept what check reported
```

The active baselines live in `snapshots/headless/{macos,windows}/scenes`.
Metal and WARP land antialiased edges differently, so each supported
renderer verifies its own baseline.

The harness is its own Cargo workspace because its renderer dependencies and
lockfile are platform-specific. It resolves the same local GPUI Box package
family as the root workspace with path-plus-version declarations and no Git
source. Both workspaces apply the one audited crates.io patch for the vendored
`block` 0.1.6 future-compatibility fix; no GPUI Box package is patched, and no
other patch is allowed. `xtask dependencies check` fails if either graph,
authority declaration, patch receipt, or lockfile drifts. CI invokes this
workspace directly so a cold visual job does not first build the overlapping
root `xtask` graph. It also disables native window-system and media playback
features: the harness needs the platform's offscreen renderer, while media
scenes use the deterministic `FixtureTransport`. Normal applications keep both
native feature sets enabled by default.

Windows CI builds that harness once, uploads the executable, and assigns the
stable scene catalog round-robin to eight fresh WARP workers with
`check --shard INDEX/COUNT`. Every scene belongs to exactly one shard, each
worker still compares against the same committed Windows baseline, and an
aggregate `headless (windows-2025)` check fails if the build or any shard fails.
This parallelizes the software renderer rather than compiling the crate graph
eight times. Each scene reports its own elapsed time so a renderer-specific
straggler can be distinguished from runner startup and queueing. The ordinary
unsharded command remains the local and capture contract.

## Audit

`gpui_kit_testkit::audit` reports the properties that make a tree usable:

- ids that are non-empty, unique, and not derived from list position;
- an accessible name on every actionable role;
- a value inside the range the same node reports, with indeterminate waits
  exempt because they have no position to report;
- no text that survived redaction;
- no visible node that occupies no space.
