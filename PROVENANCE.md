# Source provenance

Source provenance is part of the public API. A language or framework port is
still derivative work and retains the upstream attribution.

## P01: Comet presentation system

- Source: <https://github.com/zeronsh/comet>
- Revision: `fb22e269ac57331ee7aa4a9673530acf3299a886`
- License: MIT
- Copyright: Copyright (c) 2026 Wing
- Adoption: source-derived theme geometry, motion catalog, loader math,
  popover/dialog structure, settings-page scaffolding, frost, edge fade,
  font registration, and generic assets.
- Excluded: product engine, RPC, transport, accounts, provider brands, runtime
  state, and product authority.

Source-derived modules carry a short provenance comment. The full upstream MIT
license is stored at `licenses/COMET-MIT.txt`.

## P02: GPUI / Zed

- Upstream source: <https://github.com/zed-industries/zed>
- Integration source: <https://github.com/fran0220/zed>
- Upstream baseline: `a6a23c7b80a5cefa0487b7856335be89ace7e483`
- Pinned integration revision: `ce35a66556c39f948ada12b60d2aaef579a8aa68`
- Browser platform topic range: `058e203` through `b6755ec`
- WGPU backdrop-blur topic head: `ce35a66556c39f948ada12b60d2aaef579a8aa68`
- Windows pointer-exit topic head: `f9e7bd617940af337f34f0c5d90e4a83846b6ebe`
- Fluent pointer-capture topic head: `a754da44757a0aa5179224fe37a024c7517c7a92`
- Windows UIA focus topic head: `5139c7a398679251d6703ebbe10066260cbfbc13`
- Runtime topic head: `ed8643941bd01704e1e2c8d4c606c2dc737d4d9c`
- Offscreen WGPU topic head: `0bfd4852d83d52aeb8b2d7c4a4ef583a1576f40c`
- Headless software-adapter topic head: `4577f453643fcffcfc990658b839a822bad59ea0`
- License: Apache-2.0
- Copyright: Copyright 2022–2024 Zed Industries, Inc.

`gpui-kit` links one immutable integration revision. The runtime topic carries
EdgeFade, BackdropBlur, layered scenes, native focus/surface work, and the
native WebView example. The accessibility topic adds AccessKit control states
and deterministic adapter activation for platform-tree tests. The pointer-exit
topic translates a real Windows whole-window leave into GPUI input and retires
active tooltip state without confusing client/non-client transitions. The WGPU
topic is kept as a separate history because it is also the basis of upstream
proposal zed-industries/zed#62341. The integration commit combines those topics
and adds a consumer verification workflow. The pointer-capture topic exposes
the existing hitbox capture primitive through fluent interactive elements, so
identified drag surfaces continue receiving move and release events outside
their bounds and across redraws during the gesture. The pinned revision applies
that complete pointer-capture tree directly to the pointer-exit head without
forking `wgpu` or `gpu-allocator`; those dependencies resolve from crates.io.
The browser platform topics stabilize the single-threaded lifecycle, repair
WebGPU primitive rendering, add explicit WebGPU/WebGL2 selection with automatic
fallback, deliver pointer and keyboard events, and mirror AccessKit roles,
focus, actions, values, and canvas-scaled bounds into browser semantic DOM. The
WGPU backdrop-blur topic adds ordered, content-masked, rounded separable blur
passes for native offscreen and browser rendering, with bounded work and an
unblurred fallback for invalid or over-budget regions.
Generally reusable changes remain candidates for small upstream pull requests.

The Apache-2.0 text is stored at `licenses/ZED-APACHE-2.0.txt`.

### P02a: Text input structure

`crates/gpui-kit/src/controls/input/` follows the editing architecture shown in
GPUI's own `crates/gpui/examples/input.rs` at the pinned revision above: the
`EntityInputHandler` implementation, the UTF-8 to UTF-16 offset conversion an
input method requires, and a custom element that shapes one line and paints the
caret and selection around it. That structure is dictated by the GPUI API. The
editing model, key bindings, masking, length limits, scrolling, semantics, and
theming are this repository's own work, under the same Apache-2.0 terms.

## P03: Geist

- Source: <https://vercel.com/font>
- Transport source: Comet revision P01
- License: SIL Open Font License 1.1
- Copyright: Copyright 2023 Vercel Inc.
- Files: `crates/gpui-kit-assets/assets/fonts/Geist*.ttf`

The variable fonts and static Medium, SemiBold, and Bold faces are embedded so
font weight remains correct on GPUI backends that do not apply variable-font
axes.

The OFL text is stored at `licenses/GEIST-OFL-1.1.txt`.

## P04: Solar Icons

- Source: Solar Icons, Linear weight, by 480 Design
- Source page: <https://www.figma.com/community/file/1166831539721848736>
- Transport source: Comet revision P01
- License: CC BY 4.0
- Attribution: “Solar Icons by 480 Design.”
- Files: most files under `crates/gpui-kit-assets/assets/icons/`

Several product-neutral hand-drawn glyphs are Comet MIT material rather than
Solar Icons; these are identified in the adjacent `assets/SOURCE.md`.

## P05: Noto Sans Symbols, Noto Sans Symbols 2

- Source: <https://github.com/notofonts/notofonts.github.io>
- Revision: `c16b117609abbe4e60b3f2bd4433bdb3d0accb2e`
- Versions: Noto Sans Symbols 2.003, Noto Sans Symbols 2 2.008
- License: SIL Open Font License 1.1, no Reserved Font Name
- Copyright: Copyright 2022 The Noto Project Authors
- Files: `crates/gpui-kit-assets/assets/fonts/KeySymbols.ttf`

A seven-glyph subset merged from both families, renamed to the family
`GPUI Kit Key Symbols`, supplying the keyboard symbols `⌘ ⌃ ⌥ ⏎ ⌦ ⌫ ␣` that no
Geist face draws. Before it existed those glyphs came from whatever font the
host machine had installed, so the library could not draw its own keyboard
shortcuts unaided and no visual baseline could record them.

The rename is not required by the license, which declares no Reserved Font
Name; it exists so the subset cannot shadow a full Noto family on a host that
has one. The `hhea` and `OS/2` vertical metrics are copied from `Geist.ttf` so
that a fallback glyph cannot change the line box of the run it appears in.
`crates/gpui-kit-assets/assets/SOURCE.md` records the exact subset recipe.

The OFL text is stored at `licenses/NOTO-OFL-1.1.txt`.

## What was deliberately not copied

- OriginGame and Forge product state or business actions.
- t3code behavior or source.
- provider logos and product trademarks.
- application credentials, fixtures, telemetry, or user-generated content.
- monorepo-relative token paths.
