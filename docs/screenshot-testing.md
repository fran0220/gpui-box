# Screenshot testing

## Two captures, two questions

`gpui-kit-testkit::capture` answers two different questions with two functions.

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
`render_to_image`, which today is macOS. Linux and Windows hold their own
visual baseline through the headless gate described below, which renders
without any window at all. Image writing, semantic assertions, and frame
comparison remain portable.

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

The gallery asks for a 920×1000 logical window. What is captured is the
window's drawable, whose size the platform clamps to the display, so the
committed baselines are 1842×1374 device pixels: 921×687 logical at a backing
scale factor of 2. Those numbers describe the machine the baselines were
captured on, not a constant. A display of another size or scale produces images
of another size, and a comparison treats a size difference as maximally
different — which is the first thing to check when every image is reported as
changed.

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
cargo run -p xtask -- scenes capture
```

writes one image per scene per bundled theme under
`snapshots/<platform>/scenes/`. One process renders the whole catalog on the
window it launched with, because a GPUI application owns the window system for
its lifetime and paying application startup per image cost over twenty minutes.
A run takes an exclusive lock: two galleries capturing at once take the
foreground from each other, and a window the platform has pushed to the
background stops being scheduled for draws, so both runs stall on stale
frames. Before anything is recorded, one settled frame is rendered and thrown
away, because the platform delivers mouse events of its own while the window
takes the foreground, and one arriving late would hover whatever sits under
the physical cursor into the first image.

`cargo run -p xtask -- scenes check` captures into `target/scene-check` and
compares. Naming scenes captures or checks only those, which is what a change to
one component needs.

## Where the gate can run

`scenes check` needs three things, and a machine that is missing any of them
cannot report a visual regression truthfully:

1. **A macOS graphical session with Metal.** Pixels are read back from the
   GPU rather than from the window server, but the gallery still opens a real
   window to host the renderer and drive redraws, so a session that can open
   one is required. The compositor's output is not consulted.
2. **The ability to be frontmost.** A window the platform has pushed to the
   background stops being scheduled for draws, so the run reads the previous
   scene until it gives up; frontmost is also what lets focus rings and
   carets render. The gallery claims the foreground for the run and reclaims
   it whenever a poll sees an unchanged frame.
3. **The display the baselines came from.** See the fixture contract above:
   size and backing scale factor are part of the baseline.

The gate therefore runs on a Mac someone can hand a display session to — a
development machine or a self-hosted runner — rather than on an arbitrary
hosted VM. Where no such machine is attached to CI, the gate is a step a
reviewer performs and records in the pull request; the template asks for the
output rather than for a claim.

Wherever it runs, a failing run's `target/scene-check` is uploaded as an
artifact, because a difference nobody can look at is not a review.

The same catalog is rendered headlessly by `crates/gpui-kit/tests/scenes.rs`,
which audits every published tree, so a component cannot be reviewed visually
in one arrangement and tested in another.

## The headless gate

`tools/headless-visual` renders the same catalog on Linux and Windows with no
window system at all: GPUI's wgpu renderer draws each scene into an offscreen
texture and the pixels are read straight back. A software adapter — llvmpipe
on Linux, WARP on Windows — is enough, so the gate runs on a headless VM or a
CI box with no GPU. Text is shaped by cosmic-text from the bundled Geist
fonts only, and time is simulated. Linux llvmpipe and Windows WARP can still
land an antialiased edge one 8-bit channel step apart, so the comparison
permits that smallest representable difference and nothing larger.

```bash
cargo run -p xtask -- headless check     # compare against the baseline
cargo run -p xtask -- headless capture   # accept what check reported
```

Its baseline lives in `snapshots/headless/scenes`, beside but distinct from
the macOS one. CoreText and Metal land antialiased edges differently from
cosmic-text and a software rasterizer, so the two baselines are two truthful
pictures of the same catalog, not one picture captured twice. The macOS
baseline stays authoritative for what users of native macOS builds see; the
headless baseline is what every other platform can verify.

The harness is its own Cargo workspace because its Linux/Windows renderer
dependencies and lockfile are platform-specific. It no longer patches one GPUI
source into another: the root workspace and harness directly pin the same
immutable `fran0220/zed` integration revision. That revision includes the
offscreen renderer from
[zed-industries/zed#62341](https://github.com/zed-industries/zed/pull/62341)
alongside the runtime primitives the scenes use, and `xtask dependencies check`
fails if either workspace or lockfile drifts.

## Audit

`gpui_kit_testkit::audit` reports the properties that make a tree usable:

- ids that are non-empty, unique, and not derived from list position;
- an accessible name on every actionable role;
- a value inside the range the same node reports, with indeterminate waits
  exempt because they have no position to report;
- no text that survived redaction;
- no visible node that occupies no space.
