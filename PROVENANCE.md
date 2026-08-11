# GPUI Box source provenance

GPUI Box is an independent derivative project, not an official Zed project.
Provenance applies to imported and translated source as well as linked assets.
The machine-readable release record is `provenance.toml`.

## P01: GPUI / Zed filtered framework import

- Official upstream: <https://github.com/zed-industries/zed>
- Bootstrap source: <https://github.com/fran0220/zed>
- Bootstrap revision: `0b9c8dc932b65cba2dc87464148984e93f60ae18`
- Official baseline: `a6a23c7b80a5cefa0487b7856335be89ace7e483`
- Filter definition: `scripts/sync-zed/config.json`
- Sync receipt: `scripts/sync-zed/state.json`
- History algorithm: `first-parent-v1`, deterministically replayed at release
- License: Apache-2.0
- Copyright: Copyright 2022–2024 Zed Industries, Inc.

Framework/support subtrees listed in the filter were imported into GPUI Box and
renamed into the `gpui-box-*` package cohort. They are repository source, not a
Git-linked Cargo dependency. The bootstrap includes native surfaces, offscreen
WGPU, browser WebGPU/WebGL2 rendering and input, AccessKit projection, bounded
backdrop blur, pointer exit/capture, and first-prepaint tracked scrolling.
`wgpu` and `gpu-allocator` resolve from crates.io, not integration forks.
The workspace retains the pinned bootstrap's complete `windows` feature
authority. Its crates.io `zed-scap` substitute constrains `windows-capture` to
the compatible 1.4.4 API instead of restoring Zed's Cargo Git patch.

The committed receipt records bootstrap/vendor tip
`c036e5bcb472b7c557c231a66d69e646285d1942`, official cursor
`a6a23c7b80a5cefa0487b7856335be89ace7e483`, and integration merge
`82fdda6a265e556afc65b9ff1eb200f7bda8d3fc`. Release verification reconstructs
the filtered commits from remote source objects and proves that merge on the
current first-parent history. Future source movement follows
`scripts/sync-zed/README.md`. License text: `licenses/ZED-APACHE-2.0.txt`.

Kit text-input code under `crates/gpui-kit/src/controls/input/` follows GPUI's
`EntityInputHandler`, UTF-8/UTF-16 conversion, shaping, caret, and selection
architecture. The kit editing model, bindings, masking, limits, semantics, and
theme policy are GPUI Box work under MIT.

## P02: Comet presentation system

- Source: <https://github.com/zeronsh/comet>
- Revision: `fb22e269ac57331ee7aa4a9673530acf3299a886`
- License: MIT; copyright (c) 2026 Wing
- Scope: source-derived theme geometry, motion, loader math, popover/dialog and
  settings scaffolding, frost, edge fade, font registration, and generic assets
- Excluded: product engine, RPC, transport, accounts, provider brands, runtime
  state, and product authority

License text: `licenses/COMET-MIT.txt`.

## P03: Geist and Geist Mono

- Source: <https://vercel.com/font>; transported through P02
- License: SIL Open Font License 1.1
- Copyright: Copyright 2023 Vercel Inc.
- Files: `crates/gpui-kit-assets/assets/fonts/Geist*.ttf`
- License text: `licenses/GEIST-OFL-1.1.txt`

## P04: Solar Icons

- Source: Solar Icons, Linear weight, by 480 Design
- URL: <https://www.figma.com/community/file/1166831539721848736>
- License: Creative Commons Attribution 4.0 International
- Attribution: “Solar Icons by 480 Design.”
- Files: most SVGs under `crates/gpui-kit-assets/assets/icons/`
- License text: `crates/gpui-kit-assets/licenses/CC-BY-4.0.txt`

Comet-authored generic glyphs in that directory remain under MIT and are
identified by the adjacent `assets/SOURCE.md`.

## P05: Noto Sans Symbols families

- Source: <https://github.com/notofonts/notofonts.github.io>
- Revision: `c16b117609abbe4e60b3f2bd4433bdb3d0accb2e`
- Versions: Noto Sans Symbols 2.003; Noto Sans Symbols 2 2.008
- License: SIL Open Font License 1.1, no Reserved Font Name
- Copyright: Copyright 2022 The Noto Project Authors
- File: `crates/gpui-kit-assets/assets/fonts/KeySymbols.ttf`

This is a seven-glyph subset (`⌘ ⌃ ⌥ ⏎ ⌦ ⌫ ␣`), renamed `GPUI Kit Key
Symbols` to avoid shadowing an installed Noto family. The exact recipe is in
`crates/gpui-kit-assets/assets/SOURCE.md`; license text is
`licenses/NOTO-OFL-1.1.txt`.

## P06: Framework test and fallback fonts

- IBM Plex Sans source: <https://github.com/IBM/plex>
- Lilex source: <https://github.com/mishamyrt/Lilex>
- License: SIL Open Font License 1.1
- Files: `crates/gpui/assets/fonts/`, `crates/gpui_web/assets/fonts/`, and
  `crates/gpui_wgpu/assets/fonts/`

These fonts arrived with the filtered GPUI framework source. Each public crate
that embeds them carries `assets/fonts/SOURCES.md` and the applicable license
text beside the font files, so its `.crate` archive is independently
redistributable.

## P07: Hash Function Prospector `lowbias32`

- Source: <https://github.com/skeeto/hash-prospector>
- Revision: `396dbe235c94dfc2e9b559fc965bcfda8b6a122c`
- Author/discoverer: Christopher Wellons (`skeeto`)
- Source location: `README.md`, `lowbias32`
- License: public-domain dedication under the Unlicense
- Scope: the `lowbias32` finalizer translated to Metal, HLSL, and WGSL for
  deterministic gradient dithering

GPUI Box adds the two-dimensional screen-pixel fold, domain-separation salts,
and triangular sample mapping. The exact upstream license is preserved in
`licenses/HASH-PROSPECTOR-UNLICENSE.txt` and beside each public renderer crate
that contains the translation.

No product state, credentials, telemetry, user content, provider logos, or Zed
editor/workspace/product source is included by the framework filter.
