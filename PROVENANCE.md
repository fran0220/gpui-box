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

- Source: <https://github.com/zed-industries/zed>
- Public fork: <https://github.com/fran0220/zed>
- Baseline: `a6a23c7b80a5cefa0487b7856335be89ace7e483`
- Pinned fork revision: `cb2ce82766e42c55fc8563de3c5c98ac8e1c9fd9`
- License: Apache-2.0
- Copyright: Copyright 2022–2024 Zed Industries, Inc.

The fork revision adds line-wrap corrections, EdgeFade, BackdropBlur,
three-plane native-surface composition, native focus handoff, and Windows
native-surface portals. `gpui-kit` links these crates by public Git revision.

The Apache-2.0 text is stored at `licenses/ZED-APACHE-2.0.txt`.

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
