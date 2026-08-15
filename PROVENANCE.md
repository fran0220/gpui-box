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
- Frozen import receipt: `scripts/sync-zed/state.json`
- Historical import algorithm: `first-parent-v1`
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
`82fdda6a265e556afc65b9ff1eb200f7bda8d3fc`. The offline verifier checks the
frozen vendor ref, commit source trailer, filtered destinations, exact merge,
and its ancestry through the current first-parent history. The receipt is not a
future source movement mechanism. License text: `licenses/ZED-APACHE-2.0.txt`.

Post-bootstrap native PlatformView support is tracked as an independent fork
overlay, not as official-Zed replay. Its exact linear source chain is
`1755444d8efd9c7b34d8f2fbe36a327b85ca4e9b`,
`f212b120ede8c5ffcc5c60ebe1ac92d64fab9db7`,
`7bcda540a22cf9e8bbd946f954c8f28f266e452b`, and
`b46bf740a55c53612b14120f5dfbb7ceec463261`, each fetched from the same fork
and rooted directly at the bootstrap revision. The last commit supplies the
complete Windows redirection-backed popup-host implementation; the fork's later
main merge and its sibling layered-child implementation are deliberately not
part of this source lane. `provenance.toml [historical_overlay]` records the
shared filter digest, deterministic overlay tip, and exact integration merge.
Offline release verification checks every retained overlay commit, source
trailer, parent, vendor ref, and integration marker and proves that both lanes
meet only at the filtered bootstrap.

The two-repository development model ended after this import. GPUI Box is now
the sole development authority for GPUI, its platform implementations, media,
and Kit. The Zed repositories and recorded SHAs remain source attribution and
license evidence only; they are not synchronization remotes or compatibility
targets for future framework work.

The `PlatformViewHandle::keep_alive` lifetime attachment is subsequent GPUI
Box work, not part of either imported lane. It changes no native hosting or
renderer behavior; it retains caller-owned native controller state until the
existing host has detached and released its final handle clone.

The renderer-backed linear-gradient primitive with up to eight ordered color
stops is also subsequent GPUI Box work. Its shared inline scene representation
and Metal, Direct3D, WGPU, and WebGL shader implementations were authored here;
they import no additional framework or shader source.

The read-only selectable-text primitive, wrapped and bidirectional range
geometry, pointer-capture interaction, clipboard behavior, and AccessKit text
run publication are subsequent GPUI Box work. Rounded backgrounds for shaped
text fragments were added at the same framework boundary so selection does not
force Kit highlights to recompose text into separate layout elements. They use
the workspace's existing Unicode segmentation and bidirectional libraries and
import no editor or product source.

The explicit `Styled::font_fallbacks` refinement, grapheme-aware fallback run
selection, and DirectWrite lookup across registered and system collections are
subsequent GPUI Box work. They make caller-declared font chains a consistent
framework contract across native and offscreen renderers and import no source.

The full-frame platform-view clipping and macOS restacking implementation is
also subsequent GPUI Box work. It extends the product-neutral host contract on
both native platforms and imports no platform-view source from Zed or another
project.

The `gpui-box-media` AVFoundation and Media Foundation playback services are
also subsequent GPUI Box work. They call operating-system frameworks supplied
by macOS and Windows and import no third-party player source or media assets.

## P08: `block` 0.1.6 compatibility fork

- Source: <https://github.com/SSheldon/rust-block>
- Revision/tag: `47178790cfc9d4a8b092051d8b413b78bd31254a`
  (`0.1.6`)
- Historical parent license source:
  <https://github.com/SSheldon/rust-objc/blob/master/LICENSE.txt>
- License: MIT; copyright (c) Steven Sheldon
- Local source: `vendor/block`

The macOS Cocoa, Core Video, and Metal crates still resolve `block` 0.1.6,
whose final upstream release declares `_NSConcreteStackBlock` with an empty,
uninhabited enum. Rust warns that such an extern static will become a hard
error. The vendored source changes only that private opaque marker to an
inhabited zero-sized `#[repr(C)]` struct and spells the previously implicit C
ABI explicitly on function pointers. Consumers still pass the address of the
same external symbol through the same pointer-sized `isa` field, so no symbol,
calling convention, block layout, or public API changes. Both Cargo workspaces
pin this exact local source, and the dependency gate rejects any additional
patch. Its audited `src/lib.rs` SHA-256 is
`51e54353cee1cc853e567d140d35b4a74e27d5cbdbcbe68e979269f39209906a`.

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

## P05: Noto Sans Symbols and script fallback families

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

The complete Noto Sans Arabic 2.012 and Noto Sans Hebrew 3.001 variable faces
come from <https://github.com/google/fonts> revision
`352f6b7d9d6cc4fa9e242b931291d31b21a6dc84`, paths
`ofl/notosansarabic/NotoSansArabic[wdth,wght].ttf` and
`ofl/notosanshebrew/NotoSansHebrew[wdth,wght].ttf`. They are unmodified,
licensed under OFL 1.1 with no Reserved Font Name, and live at
`crates/gpui-kit-assets/assets/fonts/NotoSansArabic.ttf` and
`NotoSansHebrew.ttf`. Exact SHA-256 checksums are recorded in the asset crate's
`assets/SOURCE.md`.

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
