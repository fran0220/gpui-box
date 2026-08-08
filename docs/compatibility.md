# GPUI compatibility

## Current pin

| gpui-kit | GPUI source | Revision |
|---|---|---|
| 0.1.x | `https://github.com/fran0220/zed` | `b9e434908126d3215c22443b0102093113f8b292` |

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
gpui-kit. The detailed, evidence-scoped platform matrix lives in
[`accessibility.md`](accessibility.md).

## Platform behavior

| Capability | macOS | Windows | Linux |
|---|---|---|---|
| Core components | Supported | Compile + headless visual gate | Compile + headless visual gate |
| Native frame capture | Supported | Not implemented | Not implemented |
| Offscreen WGPU capture | Not used | WARP | llvmpipe |
| Edge fade | Supported | Supported | Supported |
| Backdrop blur | Metal | Translucent fallback | Translucent fallback |
| Native child surfaces | Supported | Supported | Not implemented |

Headless rendering verifies deterministic component output but is not a native
desktop journey. Windows and Linux support claims should be upgraded only with
native input, accessibility, windowing, and visual evidence.

Backdrop blur is deliberately renderer-specific: Metal snapshots and blurs the
scene below it, while other renderers preserve the translucent fill as a
truthful unblurred fallback. Edge fade is encoded into ordinary painted
primitives and is exercised by the WGPU integration test.

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
