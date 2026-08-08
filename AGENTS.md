# gpui-kit contributor guidance

## Boundaries

This repository is product-neutral UI infrastructure. Components may depend on
GPUI, tokens, theme, assets, and semantic testability. They must not depend on
application hosts, databases, credentials, RPC transports, or product models.

Components:

1. read caller-owned data;
2. emit caller-owned actions;
3. hold only visual transient state such as hover, focus, open, selection, and
   animation.

## Token authority

`crates/gpui-kit-tokens/tokens/*.json` is the source of truth, and every theme carries the same key
set. Repeated semantic color,
spacing, radius, typography, motion, and effect values belong there. Local
geometry that occurs once may stay next to the component.

After token changes:

```bash
cargo run -p xtask -- tokens generate
cargo run -p xtask -- tokens check
```

Do not hand-edit `docs/token-reference.md`.

## Truthful UI

- Loading, Empty, Unavailable, Error, and Ready are distinct states.
- A refresh failure keeps the last verified value visible.
- A disabled control must not install its action handler.
- Host refusals are displayed as refusals, not converted to empty data.
- Fixtures and product-backed data must be explicitly distinguishable.

## Testability

Every user-visible action and assertion target needs a stable semantic id.
IDs derive from business identity, never list position. Bounds are measured
during prepaint. Never put credentials or unredacted user-generated content in
semantic snapshots.

Tests assert behavior and generated artifacts, never source text.

## Provenance

Source ports and translations must update `PROVENANCE.md` and
`THIRD_PARTY_NOTICES`. Preserve upstream copyright notices and exact revisions.
Do not add product or provider trademarks to the generic asset crate.

## The generated API index

`docs/api-index.json` carries every component, the exact signature of every
public method, what each reports, and the scenes that render it. It is
generated from the source, so it is the answer when it and any prose disagree:

```bash
cargo run -p xtask -- api generate
cargo run -p xtask -- api check     # runs inside `gate`
```

A new component appears there by existing. Adding one and leaving the index
stale fails the gate, which is the point: a reader who is told a signature that
does not compile was failed by this file, not by the compiler.

`docs/llms.txt` is the entry point for a program reading this repository, and
`tools/mcp` serves the same catalog as MCP tools, including one that renders a
scene and returns the image.

## Validation

```bash
cargo run -p xtask -- gate        # fmt, check, test, clippy, tokens
cargo run -p xtask -- gate full   # the above, plus rustdoc and scene images
```

Captures read the frame GPUI drew straight back from the GPU rather than
asking the window server what it composited, and the gallery renders with
reduced motion, parks the pointer, and discards one warm-up frame so stray
platform events cannot leak into the first image. The same scene therefore
produces the same bytes on every run and in any order — a scoped check
matches the full-catalog baseline — which is what makes `scenes check` a gate
rather than a suggestion. The comparison still allows one step per channel,
because another machine's GPU or OS may rasterize an antialiased edge one
step differently. Anything a component changed moves far further than that.

While iterating on one component, capture only what it touches:

```bash
cargo run -p xtask -- scenes capture list tree   # rewrite these scenes
cargo run -p xtask -- scenes check list tree     # compare without rewriting
```

On Linux and Windows the visual gate is `cargo run -p xtask -- headless check`
(`headless capture` accepts), which renders the same catalog into offscreen
textures through a software adapter — no window system or GPU — against its
own baseline in `snapshots/headless/scenes`. The harness lives in
`tools/headless-visual` as its own workspace with renderer-specific
dependencies and a separate lockfile. It and the root workspace must pin the
same immutable `fran0220/zed` integration revision; run `cargo run -p xtask --
dependencies check` after changing it. See `docs/screenshot-testing.md`.

UI changes additionally require visual inspection of the captured images. A
changed image is a claim about what the component now looks like, so look at
it before accepting it.

Two things a captured image does not show, both known:

- A repeating animation is held at its first frame and a one-shot at its last,
  because a still frame of a moving thing is not reproducible. Review motion by
  running the gallery, not by reading a snapshot.
- A text caret does not appear once another scene has been rendered in the same
  run, which is why `data-grid-editing` shows its editor without an insertion
  point. Rendering that one scene on its own restores it, and that is the way
  to check it:

  ```bash
  cargo run -p gpui-kit-gallery -- --scene data-grid-editing \
      --theme studio-light --capture /tmp/check.png
  ```
