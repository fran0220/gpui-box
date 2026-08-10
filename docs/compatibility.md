# GPUI compatibility

## Current pin

| gpui-kit | GPUI source | Revision |
|---|---|---|
| 0.1.x | `https://github.com/fran0220/zed` | `0d6f50bbac07b76688e5471b932b12412d256a9a` |

The workspace and the standalone headless harness depend on one immutable
revision of the integration fork. That revision merges three independently
reviewable topic stacks on upstream baseline
`a6a23c7b80a5cefa0487b7856335be89ace7e483`: runtime primitives/native
surfaces, the offscreen WGPU renderer, and deterministic software-adapter
selection confined to headless rendering. crates.io is not usable yet: the
published `gpui` crate stopped at 0.2.2 and `gpui_platform` has never been
published, so the pin remains a Git revision.

The current integration descendant also carries the product-neutral control
state projection and deterministic accessibility-adapter activation used by
gpui-kit, plus whole-window pointer-exit delivery needed to retire transient
help from the Windows native accessibility tree. It exposes hitbox pointer
capture through fluent interactive elements so a bounded drag continues to
receive move and release events after leaving its original bounds, including
when a live preview redraws the window mid-gesture. The detailed, evidence-scoped
platform matrix lives in [`accessibility.md`](accessibility.md).

The renderer graph uses crates.io `wgpu` 29.0.4 and `gpu-allocator` 0.28.0;
there are no `fran0220/wgpu` or `fran0220/gpu-allocator` sources. GPUI's direct
Windows bindings remain on the compatible 0.61 line, while the independent
wgpu and allocator edges resolve their registry Windows 0.62 dependencies.

## Platform behavior

| Capability | macOS | Windows | Linux | Browser/WASM |
|---|---|---|---|---|
| Core components | Supported; native visual gate | Supported; headless visual gate | Supported; headless visual gate | Core stable wasm check passes; browser host blocked |
| Native frame capture | Supported | Not implemented | Not implemented | Not applicable |
| Offscreen WGPU capture | Not used | WARP | llvmpipe | No accepted baseline |
| Edge fade | Supported | Supported | Supported | Unverified |
| Backdrop blur | Metal | Translucent fallback | Translucent fallback | Unverified |
| Native child surfaces | Supported | Supported | Not implemented | Not applicable |

Core component rendering is supported and visually regression-tested on all
three platforms. macOS uses native window capture; Windows and Linux render the
same catalog through their deterministic offscreen WGPU backends. Native frame
capture and some accessibility or native-child capabilities remain
platform-specific as recorded in this table and in
[`accessibility.md`](accessibility.md).

Backdrop blur is deliberately renderer-specific: Metal snapshots and blurs the
scene below it, while other renderers preserve the translucent fill as a
truthful unblurred fallback. Edge fade is encoded into ordinary painted
primitives and is exercised by the WGPU integration test.

`cargo run -p xtask -- web check` checks the core `gpui-kit` library with
fixtures for `wasm32-unknown-unknown` on the repository's stable Rust
toolchain. It does not link `gpui_platform` or claim that a browser host runs.
At the current pin, linking WebPlatform reaches a dependency that requires an
unstable wasm feature, so browser rendering and interaction remain blocked.
Browser accessibility and a deterministic browser visual gate are also
unverified. Web support remains unclaimed until rendering, interaction,
accessibility, and visual regression coverage pass together.

The deferred host uses the single-threaded WebPlatform constructor for its
basic no-header smoke. Threaded browser execution requires separate COOP/COEP
and worker validation. The current web backend also does not implement
`PlatformWindow::a11y_init` or `a11y_tree_update`, so the host's diagnostic
semantic snapshot must not be presented as browser accessibility support.

## Upgrade process

1. Rebase each fork topic stack onto the selected upstream Zed revision.
2. Merge the verified topic heads into a new integration commit without
   rewriting the topic branches.
3. Update the root and headless manifests to that exact integration SHA.
4. Run `cargo run -p xtask -- dependencies check`, both workspace gates, and
   the fork's `gpui-kit consumer` workflow.
5. Capture the gallery on each supported native platform and inspect text layout:
   upstream wrapping fixes land continuously and move line breaks.
6. Update this matrix, `PROVENANCE.md`, and `THIRD_PARTY_NOTICES`.

Do not use a floating branch dependency.
