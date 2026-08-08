# GPUI compatibility

## Current pin

| gpui-kit | GPUI source | Revision |
|---|---|---|
| 0.1.x | `https://github.com/zed-industries/zed` | `a6a23c7b80a5cefa0487b7856335be89ace7e483` |

The workspace depends on upstream GPUI only. There is no patched fork, and
every capability the components use is a public upstream API. crates.io is not
usable yet: the published `gpui` crate stopped at 0.2.2 and `gpui_platform`
has never been published, so the pin is a Git revision until upstream resumes
publishing.

## Platform behavior

| Capability | macOS | Windows | Linux |
|---|---|---|---|
| Core components | Supported | Compile target | Compile target |
| In-process capture | Supported | Not implemented | Not implemented |

“Compile target” is not the same as a validated production journey. Platform
support claims should be upgraded only with native build and visual evidence.

Overlay surfaces are opaque on every platform. Backdrop frost and scroll-edge
fading existed only as fork patches and were removed with the fork; if
upstream GPUI grows a per-element backdrop blur or an alpha-mask primitive,
they can return as ordinary components with the same tokens.

## Upgrade process

1. Select the new upstream Zed revision.
2. Update the workspace Git revision.
3. Run all workspace gates.
4. Capture the gallery on each supported platform and inspect text layout:
   upstream wrapping fixes land continuously and move line breaks.
5. Update this matrix and `PROVENANCE.md`.

Do not use a floating branch dependency.
