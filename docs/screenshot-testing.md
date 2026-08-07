# Screenshot testing

## Capture the owned window

`gpui-kit-testkit::capture::capture_window` asks the macOS window server for the
specific window owned by the process. It excludes framing and returns RGBA8
pixels for the content area.

Do not use full-desktop capture for automated evidence:

- it may trigger OS consent prompts;
- it captures unrelated user content;
- it depends on z-order;
- semantic bounds no longer map directly to image pixels.

Non-macOS capture currently returns `Unsupported`. Image writing, semantic
assertions, and frame comparison remain portable.

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

The gallery asks for a 920×1000 logical window. What is captured is the content
area the window server gave it, which the platform clamps to the display, so the
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
foreground from each other, and a window nobody is compositing hands back the
frame it drew last, so both runs read each other's scenes.

`cargo run -p xtask -- scenes check` captures into `target/scene-check` and
compares. Naming scenes captures or checks only those, which is what a change to
one component needs.

## Where the gate can run

`scenes check` needs three things, and a machine that is missing any of them
cannot report a visual regression truthfully:

1. **A graphical session that composites windows.** The capture asks the window
   server for the process's own window by id. There is no offscreen path.
2. **The ability to be frontmost.** A window nobody is compositing keeps handing
   back the frame it drew last, so the run reads the previous scene until it
   gives up. The gallery claims the foreground for the run and reclaims it
   whenever a poll sees an unchanged frame.
3. **The display the baselines came from.** See the fixture contract above: size
   and backing scale factor are part of the baseline.

A GitHub-hosted runner has not been shown to provide the first two, and does not
provide the third. So CI runs the gate only on a self-hosted macOS runner named
by the `VISUAL_RUNNER` repository variable, and says in the run summary when it
ran nowhere. Where there is no such runner, the gate is a step a reviewer
performs and records in the pull request; the template asks for the output
rather than for a claim.

Wherever it runs, a failing run's `target/scene-check` is uploaded as an
artifact, because a difference nobody can look at is not a review.

The same catalog is rendered headlessly by `crates/gpui-kit/tests/scenes.rs`,
which audits every published tree, so a component cannot be reviewed visually
in one arrangement and tested in another.

## Audit

`gpui_kit_testkit::audit` reports the properties that make a tree usable:

- ids that are non-empty, unique, and not derived from list position;
- an accessible name on every actionable role;
- a value inside the range the same node reports, with indeterminate waits
  exempt because they have no position to report;
- no text that survived redaction;
- no visible node that occupies no space.
