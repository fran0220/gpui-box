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

The gallery baseline uses a 920×900 logical viewport.

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
`snapshots/<platform>/scenes/`. Each capture runs in its own process because a
GPUI application owns the window system for its lifetime.

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
