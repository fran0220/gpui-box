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

Window-control hit testing that gives a later-painted nested caption control or
explicit client area precedence over its enclosing drag area, vetoable
programmatic close requests, current-request-position native caption hit tests,
corrected Windows horizontal/vertical resize-frame metrics for transparent
title bars, host-reserved Toast edges, grayscale glyph rasterization on
transparent native scene-overlay planes, and Direct3D offscreen-pass target
restoration are also subsequent GPUI Box work. Deferring a native frame request
that synchronously re-enters while the application is already borrowed, while
preserving its forced-render and presentation intents, belongs to this same
framework-owned window lane. Discarding accessibility action registrations
while the per-frame AccessKit state is inactive is a GPUI Box correction in
that window lane as well. They use the existing operating-system and GPUI
layout APIs and import no additional source.

The renderer-backed linear, elliptical radial, and conic gradient primitives
with up to eight ordered color stops are also subsequent GPUI Box work. Their
shared inline scene representation and Metal, Direct3D, WGPU, and WebGL shader
implementations were authored here; they import no additional framework or
shader source.

The composited sprite-batch scene primitive, explicit atlas source rectangles,
center-relative transforms, rounded/source-alpha masks, tint modes, and normal,
additive, and screen pipeline states are subsequent GPUI Box work across Metal,
Direct3D, WGPU, and WebGL. The procedural RGBA8 image constructor and canonical
sprite atlas are authored here and import no source, shader, or visual asset.

The deterministic CPU particle sampler, bounded emitter contract, absolute-time
birth and trajectory math, built-in alpha-mask atlas, and policy-owned Kit
recipes are subsequent GPUI Box work. They reuse that sprite-batch renderer and
import no simulation source, shader, or visual asset.

Normalized path-stroke trimming and wrapping dash offsets are subsequent GPUI
Box work at the pre-tessellation path boundary. They use the existing Lyon
measurement and tessellation dependency and import no source or visual assets.

The read-only selectable-text primitive, wrapped and bidirectional range
geometry, pointer-capture interaction, clipboard behavior, and AccessKit text
run publication are subsequent GPUI Box work. Rounded backgrounds for shaped
text fragments were added at the same framework boundary so selection does not
force Kit highlights to recompose text into separate layout elements. They use
the workspace's existing Unicode segmentation and bidirectional libraries and
import no editor or product source.

The product-neutral editable-text buffer, grapheme-safe selection and
replacement transactions, marked-composition lifecycle, grouped undo/redo,
length policy, and UTF-8/UTF-16 conversion are also subsequent GPUI Box work.
The wrapped and bidirectional editable layout, byte/point and visual-row
mapping, alignment-aware hit testing, selection/caret geometry, reveal-scroll
projection, and generic normalized style runs are part of the same subsequent
work. Current-frame editable AccessKit positions, widths, and run bounds are
captured from that same shaped layout rather than a second accessibility
layout. They move the shared authority used by Kit's plain text controls to the
framework boundary and use the existing shaping and Unicode segmentation
dependencies; no editor, document-format, grammar, language-server, or product
source is imported.

The per-window labelled-by, described-by, and cross-tree active-descendant
resolver, including inverse label/description declarations, deferred-subtree
resolution, focused-owner validation, ambiguous-target refusal, stale-endpoint
removal, and absent-scalar text fallback is subsequent GPUI Box work. It fills
the existing AccessKit relationship and focus projection from role-bearing GPUI
element identities while supplying native adapters the same resolved name/help
text when they do not consume those properties. It imports no accessibility
adapter, product, or UI source.

The window-owned document-selection coordinator, caller-declared reading order,
overlay scope isolation, truthful virtualized-copy coverage, and aggregate
clipboard path are subsequent GPUI Box work above that selectable-text
primitive. The generic `ScrollTarget` contract over overflowing containers,
uniform lists, and measured variable-height lists is GPUI Box work as well.
Both additions coordinate existing framework layout and input state and import
no editor, product, or third-party source.

The inline sticky-subtree transform, deferred-prepaint content-mask retention,
and inset-aware focus reveal contract are subsequent GPUI Box work at the
shared layout/input boundary. They keep layout, clipping, hit testing,
accessibility geometry, and paint on one translated subtree and import no
browser sticky implementation, grid source, product policy, or third-party
code.

The bounded native external-data drag bridge for encoded images, MIME-tagged
text, URLs, and promised or virtual files on macOS and Windows is subsequent
GPUI Box work. It uses operating-system drag and pasteboard APIs and imports no
third-party or product source.

Per-window structural frame counters, retained element-arena growth
accounting, deterministic Kit performance budgets, and the 10,000-item
component fixtures are subsequent GPUI Box work. They instrument existing
framework draw boundaries and import no profiler, allocator, benchmark, or
product source.

Reference-counted frame-trace leases and the bounded per-window timing monitor
are subsequent GPUI Box work over those same draw boundaries. They derive
summaries from observed draw timestamps without scheduling frames and import no
profiler HUD, product, or third-party source.

The explicit `Styled::font_fallbacks` refinement, grapheme-aware fallback run
selection, and DirectWrite lookup across registered and system collections are
subsequent GPUI Box work. They make caller-declared font chains a consistent
framework contract across native and offscreen renderers and import no source.
Role-aware face acceptance — a face reached only as a fallback is no longer
required to carry the `m` that em measurement uses, and is no longer removed
from the shared database for lacking it — and CoreText process-scope
registration of embedded fonts on macOS are the same lane of work. Both make an
embedded symbol-only face reachable by the name a cascade list gives it.

The full-frame platform-view clipping and macOS restacking implementation is
also subsequent GPUI Box work. It extends the product-neutral host contract on
both native platforms and imports no platform-view source from Zed or another
project.

The `gpui-box-media` AVFoundation and Media Foundation playback services are
also subsequent GPUI Box work. They call operating-system frameworks supplied
by macOS and Windows and import no third-party player source or media assets.

## P09: optional dotLottie raster backend

- Backend: `rasterlottie` 0.2.2
- Source: <https://github.com/neodyland/rasterlottie>
- License: MIT OR Apache-2.0
- Copyright: Copyright (c) 2026 neodyland contributors
- Archive reader: `zip` 8.6.0, <https://github.com/zip-rs/zip2>, MIT,
  Copyright (c) 2014 Mathijs van de Nes
- Relationship: optional crates.io dependencies; no Cargo Git source

GPUI Box's public dotLottie contracts, semantic recipes, poster behavior, and
particle fallbacks are authored here and expose no backend type. The optional
`gpui-box-kit/dotlottie` feature uses `rasterlottie` only after Box validates
and fully reads a host-provided archive under hard and host-tightenable limits,
inspects embedded image dimensions and aggregate target pixels, then rebuilds a
bounded stored archive for decoding. The official `dotlottie-rs` crate is not
linked: its crates.io alpha depends on native Conan/bindgen/ThorVG artifacts
while current releases are Git-only, which conflicts with the repository's
crates.io/local package authority. No animation, visual asset, URL, path, or
product model is imported.

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

The `star` and `star-filled` glyphs are original repository drawings and are
covered by the repository license rather than the Solar Icons attribution.

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

## P10: bezel terminal and markdown document model

- Source: <https://github.com/crabtalk/bezel>
- Revision: `86b8997c0601ebcd416632ebde33d78f82b05917`
- License: MIT
- Copyright: Copyright (c) 2026 clearloop
- Source locations: `crates/terminal/src/{emulator.rs,view.rs}` and
  `crates/markdown/src/{doc.rs,parse.rs,serialize.rs,edit.rs}`, with the
  matching `tests/`
- Destinations: `crates/gpui-kit/src/content/terminal/` and
  `crates/gpui-kit/src/content/markdown/doc/`

A source-level port, not a dependency: bezel pins `gpui` to a Zed Git
revision, and this repository is the sole authority for its own GPUI
packages. License text is `licenses/BEZEL-MIT.txt`.

The port is not a transplant. The terminal's per-appearance ANSI tables became
`color.terminal.*` in the token documents, because a palette compiled into a
component is the second colour authority the token rule exists to prevent, and
the contrast report now covers all sixteen slots against the terminal
background. The emulator's public API was rewritten so no `alacritty_terminal`
type appears in it. The markdown document model arrived with its round-trip
fixed-point tests, which are the property the model is worth having for.

### `alacritty_terminal`

- Source: <https://github.com/alacritty/alacritty>
- Version: 0.26.0 (crates.io)
- License: Apache-2.0
- Scope: the escape-sequence state machine and grid behind
  `content::terminal::Emulator`

Behind the `terminal` feature, which is on by default so the gate, the gallery
and the visual baselines all cover the component. Only `Term`, the ANSI
`Processor`, `Selection` and the grid types are used. The crate's `tty` and
`event_loop` modules compile and are never referenced: they have no feature to
turn off upstream, and this repository opens no pty and spawns no process,
which stays the host's job. Recorded here rather than left implicit, because a
UI crate that links a process spawner should have to say so.

## P12: clear liquid-glass material and dual-source renderer

- Material-policy source: <https://github.com/crabtalk/bezel>
- Bezel revision: `2cfff23c96c6d33177a65d523f1827b0941b2eac`
- Bezel license: MIT
- Renderer source: <https://github.com/crabtalk/zed>
- Renderer revision: `ddd1c7d2cd98e1109f5bc4e21488c7ec8aefe198`
- Renderer parent: `756cafe25ddfa4a702c39db70f4b16d6276c02a3`
- Renderer license: Apache-2.0
- Source locations: Bezel `crates/ui/src/material.rs`; crabtalk/zed
  `crates/gpui/src/{scene.rs,window.rs}` and
  `crates/gpui_apple/src/{metal_renderer.rs,shaders.metal}`
- Destinations: GPUI Box `crates/gpui/src/{scene.rs,window.rs}`,
  `crates/gpui_{macos,wgpu,windows}/src/`, and
  `crates/gpui-kit/src/overlay/glass.rs`

This is a source-level, product-neutral adaptation, not a Cargo dependency and
not a reopening of the frozen Zed import lane. Bezel supplies the measured
material policy: clear glass defaults to no blur, bevel depth is 0.225 of the
short edge, magnification is 0.34, dispersion is 0.005, white additive lift is
0.075, transmission gain is 1.042, and the hairline is one logical pixel. The
crabtalk/zed commit supplies the dual-source rendering model and spherical
profile: preserve sharp and blurred snapshots, blend the sharp source toward
the rim by `(1 - depth)²`, and cap displacement at 0.45 of the bevel.

GPUI Box generalized the Metal-only source across its existing multi-lobe SDF,
paint-order, clipping, probe, WGPU, and Direct3D infrastructure. It retained
the existing directional specular interaction as an independent optional axis,
made blur part of the framework material rather than a call-site gate, and
added bounded-pass fallback and cross-renderer tests. The separate 16-surface
per-frame admission budget, replay-aware ordinary-fill fallback, and rejection
of over-budget probe work are subsequent GPUI Box framework work rather than
adapted source. Bezel's license text is `licenses/BEZEL-MIT.txt`; the
Apache-2.0 text is `licenses/ZED-APACHE-2.0.txt`.

## P11: bundled theme preset palettes

- Destinations: `crates/gpui-kit-tokens/tokens/{catppuccin-mocha,
  catppuccin-latte,nord,tokyo-night,gruvbox-dark,dracula,solarized-dark,
  solarized-light}.json`
- Scope: hexadecimal palette values transcribed from each upstream colour
  scheme into this repository's own token document shape

| Preset | Upstream | License | Copyright |
| --- | --- | --- | --- |
| `catppuccin-mocha`, `catppuccin-latte` | <https://github.com/catppuccin/catppuccin> | MIT | Copyright (c) 2021 Catppuccin |
| `nord` | <https://github.com/nordtheme/nord> | MIT | Copyright (c) 2016-present Sven Greb |
| `tokyo-night` | <https://github.com/enkia/tokyo-night-vscode-theme> | MIT | Copyright (c) 2019 enkia |
| `gruvbox-dark` | <https://github.com/morhetz/gruvbox> | MIT | Copyright (c) 2018 Pavel Pertsev |
| `dracula` | <https://github.com/dracula/dracula-theme> | MIT | Copyright (c) 2016 Dracula Theme |
| `solarized-dark`, `solarized-light` | <https://github.com/altercation/solarized> | MIT | Copyright (c) 2011 Ethan Schoonover |

No upstream file is vendored: each preset is a token document written here,
carrying the same key set as `studio-dark.json`, whose colour values are the
published hexadecimal palettes above. Nothing outside the palette — spacing,
motion, elevation, control metrics — comes from upstream.

Where a published value could not clear this repository's contrast, surface
separation, tone distinction, line visibility or placeholder loudness gates, it
was moved along its own lightness ramp by the smallest step that passes and the
hue was kept. Those are the presets' own values and are not attributed to
upstream:

- `nord`: `nord11` `#bf616a` lightened to `#cf7c85` for `danger` and the diff's
  removed sign, and the Aurora tints lightened for the five agent families,
  which carry the body text floor rather than the identity floor. The ANSI
  table keeps `#bf616a` unchanged.
- `solarized-dark` and `solarized-light`: the `base01`/`base00`/`base0`/`base1`
  ladder is the scheme's defining low-contrast feature and cannot seat five
  distinguishable text tones over six surfaces, so both presets carry their own
  five-rung grey ladder in Solarized's hue with the upstream steps kept in
  `palette.neutral.300` and `.500` and across the ANSI table. `red`, `yellow`,
  `green`, `cyan` and `blue` are moved a step where a surface required it.
- `catppuccin-latte`: `yellow`, `green`, `teal` and `magenta` darkened for the
  page they sit on, and `mauve` darkened where it paints code.
- `gruvbox-dark`: bright `purple` `#d3869b` lightened to `#d992a5` for the
  agent family tint only.

Two non-colour tokens are also theme-owned rather than upstream:
`effect.focusRingAlpha` is 1 in `catppuccin-latte`, `solarized-light` and
`solarized-dark`, and `opacity.disabled` is raised in the two light presets,
because both schemes' text sits closer to their page than the studio pair's
does.

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

## P13: GPUI Component plot behavior reference

- Reference: <https://github.com/longbridge/gpui-component>
- Revision: `6761b4ec9ca90cf2c37f8ba01deaa9ffcf0d0da7`
- License: Apache-2.0
- Reference locations: `crates/ui/src/plot/mod.rs`,
  `crates/ui/src/chart/candlestick_chart.rs`, and
  `crates/ui/src/chart/sankey_chart.rs`
- Destination: `crates/gpui-kit/src/display/plot.rs`

The reference established the product-neutral separation between a measured
plot frame, caller data, and styled chart wrappers. GPUI Box's implementation
is original and deliberately narrower: callers supply normalized mark, OHLC,
node, and ribbon geometry; Kit supplies stable semantic ids, measured bounds,
keyboard traversal, truthful states, and theme presentation. No upstream Rust
source was copied. In particular, GPUI Box does not include or translate the
referenced `d3-sankey` topology/layout algorithm, and carries no d3 dependency
or financial fixture policy.

## P14: GPUI Component FPS behavior reference

- Reference: <https://github.com/longbridge/gpui-component>
- Revision: `6761b4ec9ca90cf2c37f8ba01deaa9ffcf0d0da7`
- License: Apache-2.0
- Reference locations: `crates/fps/src/lib.rs`,
  `crates/fps/src/monitor.rs`, `crates/fps/src/overlay.rs`,
  `crates/fps/src/sampler.rs`, and `crates/fps/src/style.rs`
- Destination: `crates/gpui/src/profiler.rs` and
  `crates/gpui-kit/src/display/performance_hud.rs`

The reference established the usefulness of a bounded, per-window live frame
reading. GPUI Box's implementation is original and separates responsibilities
more strictly: the framework monitor derives FPS from recorded draw-start
timestamps and never requests a frame, while the controlled Kit view owns no
monitor, clock, history, resource sampler, overlay placement, or refresh loop.
Draw-budget overage is not called a dropped frame, and all styling, strings,
numbers, states, and semantic ids use existing Box authorities. No upstream
Rust source, resource-profiling code, or dependency was copied.
