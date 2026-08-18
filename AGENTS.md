# GPUI Box contributor guidance

## Boundaries

This repository is product-neutral UI infrastructure. Components may depend on
GPUI, tokens, theme, assets, and semantic testability. They must not depend on
application hosts, databases, credentials, RPC transports, or product models.

Components:

1. read caller-owned data;
2. emit caller-owned actions;
3. hold only visual transient state such as hover, focus, open, selection, and
   animation.

## Framework infrastructure

Do not hide a missing GPUI primitive behind a component-specific workaround.
When a requirement is product-neutral, will be reused by more than one
component, or must coordinate rendering, layout, clipping, hit testing, input,
or platform behavior, first verify whether the GPUI Box framework
already provides it. If it does not, implement the smallest complete primitive
at the framework boundary. GPUI Box is the sole development authority; implement
framework and platform changes directly here and never restore a Zed Cargo Git
dependency or continuous source synchronization.

Keep product and component policy in this repository. Node routing, port
meaning, semantic ids, and caller-owned events belong to GPUI Box Kit; generic
subtree transforms, pointer capture, renderer behavior, and platform event
delivery belong to the framework. Do not add a partial framework API that works for one
primitive or platform while leaving its layout, clipping, accessibility bounds,
or hit testing inconsistent.

A framework infrastructure change must:

1. be product-neutral and documented at the primitive boundary;
2. carry focused GPUI tests, including the affected platform-independent input
   or rendering invariants;
3. preserve the frozen historical import receipt without treating it as an
   update lane;
4. keep the root and `tools/headless-visual` workspaces on the same local GPUI
   Box package authority, without Git sources or `[patch]` overrides;
5. update `PROVENANCE.md`, `THIRD_PARTY_NOTICES`, and compatibility documentation;
6. pass `cargo run -p xtask -- dependencies check` and the relevant Linux,
   macOS, and Windows validation.

Local geometry remains appropriate when it occurs once and does not create a
second implementation of a renderer or input primitive. Record any deliberately
deferred framework gap in coverage documentation rather than presenting a local
approximation as complete support.

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
public method, what each reports, and the scenes that review it. It is
generated from the source, so it is the answer when it and any prose disagree:

```bash
cargo run -p xtask -- api generate
cargo run -p xtask -- api check     # runs inside `gate`
```

A new component appears there by existing. Adding one and leaving the index
stale fails the gate, which is the point: a reader who is told a signature that
does not compile was failed by this file, not by the compiler.

## A component is reviewed by an exhibit

A rendering in `crates/gpui-kit/src/scenes/` declares what it is for, and the
declaration is checked:

- `Shows::Subjects` names the components the rendering is *about*. It lives in
  the file for those components' family, and it is where a reader is sent to
  review their states.
- `Shows::Composition` is an arrangement built the way a product would build
  one, kept because components interact in ways none of them shows alone. It
  is nobody's coverage.

Three things fail `api check`:

1. a public component with no exhibit anywhere;
2. an exhibit whose own source never builds a component it claims to review —
   a component drawn inside another component has been recognised, not
   reviewed, and the picture that would fail when its states change belongs to
   whatever mounted it;
3. a declaration naming something the rendering cannot reach at all.

A scene and the function that builds it are one thing, so `api check` also
requires the function to be named after the scene.

So a new component needs a scene that builds it, in the family file next to
the components it sits with — not a mention inside a shell.

`docs/llms.txt` is the entry point for a program reading this repository, and
`tools/mcp` serves the same catalog as MCP tools, including one that renders a
scene and returns the image. The stdio server and the hosted Worker both read
this tree's index, not crates.io `gpui-box-mcp`. Hosted `/mcp` is the last
deploy of the repository; stdio (`tools/mcp/run.sh`) is the working copy.

## Hosted MCP after every push

The public catalog at `https://gpui-box.origingame.dev/mcp` must track
`origin/main`, not crates.io. Do not publish a crates.io MCP bump as part of
ordinary Kit work.

After every `git push` to `main`:

1. `tools/site/deploy.sh` (set `CLOUDFLARE_ACCOUNT_ID` when more than one
   Cloudflare account is logged in);
2. confirm hosted `/api-index.json` has the same component and scene counts
   as `docs/api-index.json` on that commit;
3. confirm `POST /mcp` `search_components` with an empty query returns that
   same count.

A push that changes the catalog and leaves the Worker on the previous
deploy is unfinished.

## Validation

```bash
cargo run -p xtask -- gate                      # fmt, check, test, clippy, tokens
cargo run -p xtask -- gate full                 # the above, plus rustdoc and scene images
cargo run -p xtask -- gate only keymap-editor   # while iterating on one component
```

`gate only` takes scene names and answers the same questions about those:
`gpui-box-kit` builds and lints clean, the tests whose names mention them pass, the
generated artifacts are current, and those scenes match their baseline. It is a
shortcut while iterating, never what a commit runs — it says nothing about the
other workspace members, the doctests, or a scene the edit reached without
anybody predicting it.

The visual gate on each supported native platform is
`cargo run -p xtask -- headless check` (`headless capture` accepts). It renders
the catalog into offscreen textures at a size it names — Metal on macOS and
software adapters on Linux and Windows —
with reduced motion and simulated time, so no window, display, dock, or
compositor takes part and the same scene produces the same bytes on any
machine with that renderer. Active baselines live in
`snapshots/headless/{macos,windows}/scenes`, one set per renderer; the Linux
set is retired and no CI job compares it.

Never hold a baseline from a real window. A window negotiates its size with
the display it opens on, which is how `snapshots/macos` came to hold two
incompatible sets that no machine could pass; `docs/screenshot-testing.md`
records what that cost.

A comparison allows one step per channel. Exactness does not hold even within
one renderer: the sprite atlas has accumulated different state by the
ninetieth scene of a full run, which moves one antialiased pixel of `frost` by
one step. Scoped runs agree to the byte, so the tolerance is what makes a
scoped check mean the same thing as the full one. Anything a component changed
moves far further than one step.

While iterating on one component, check only what it touches:

```bash
cargo run -p xtask -- headless check list tree     # compare without rewriting
cargo run -p xtask -- headless capture list tree   # accept these scenes
```

The harness lives in `tools/headless-visual` as its own workspace with
renderer-specific dependencies and a separate lockfile. It and the root
workspace must resolve the same local GPUI Box package authority without Git
sources or patches; run `cargo run -p xtask -- dependencies check` after
changing it. See `docs/screenshot-testing.md`.

`cargo run -p xtask -- scenes render` opens the real gallery window and writes
to `target/scenes`. It holds no baseline and is not a gate; it is how motion
and the text caret get looked at, since both need a real window.

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
  cargo run -p gpui-box-gallery -- --scene data-grid-editing \
      --theme studio-light --capture /tmp/check.png
  ```
