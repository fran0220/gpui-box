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
- Pinned integration revision: `bbcc88d1f0ae98d2bd8632ea2a4ccede51d9108d`
- Runtime topic head: `ed8643941bd01704e1e2c8d4c606c2dc737d4d9c`
- Offscreen WGPU topic head: `0bfd4852d83d52aeb8b2d7c4a4ef583a1576f40c`
- Headless software-adapter topic head: `4577f453643fcffcfc990658b839a822bad59ea0`
- License: Apache-2.0
- Copyright: Copyright 2022–2024 Zed Industries, Inc.

`gpui-kit` links one immutable integration revision. The runtime topic carries
EdgeFade, BackdropBlur, layered scenes, native focus/surface work, and the
native WebView example. The accessibility topic adds AccessKit control states
and deterministic adapter activation for platform-tree tests. The WGPU topic
is kept as a separate history because it is also the basis of upstream proposal
zed-industries/zed#62341. The integration commit combines those topics and adds
a consumer verification workflow; generally reusable changes remain candidates
for small upstream pull requests.

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
- Files: `crates/gpui-kit-assets/assets/fonts/*`

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

## What was deliberately not copied

- OriginGame and Forge product state or business actions.
- t3code behavior or source.
- provider logos and product trademarks.
- application credentials, fixtures, telemetry, or user-generated content.
- monorepo-relative token paths.
