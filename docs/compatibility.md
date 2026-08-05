# GPUI compatibility

## Current pin

| gpui-kit | GPUI source | Revision |
|---|---|---|
| 0.1.x | `https://github.com/fran0220/zed` | `cb2ce82766e42c55fc8563de3c5c98ac8e1c9fd9` |

Baseline Zed revision:
`a6a23c7b80a5cefa0487b7856335be89ace7e483`.

## Required fork capabilities

The component layer uses:

- `EdgeFade` and `Window::with_edge_fade`;
- `Window::paint_backdrop_blur`;
- stable single-layer paint ordering for frosted subtrees.

The fork additionally carries native-surface composition and focus handoff
used by desktop products that place WKWebView or WebView2 between GPUI's base
and overlay scenes. `gpui-kit` itself does not ship a WebView component.

## Platform behavior

| Capability | macOS | Windows | Linux |
|---|---|---|---|
| Core components | Supported | Compile target | Compile target |
| Backdrop frost | Enabled | Opaque fallback | Opaque fallback |
| Edge fade | Patched GPUI | Patched GPUI | Patched GPUI |
| In-process capture | Supported | Not implemented | Not implemented |
| Native surface portal | Fork capability | Fork capability | Not claimed |

“Compile target” is not the same as a validated production journey. Platform
support claims should be upgraded only with native build and visual evidence.

## Upgrade process

1. Rebase the fork branch onto the selected Zed revision.
2. Keep each patch attributable and reviewable.
3. Run the GPUI patch tests and native overlay example.
4. Update the workspace Git revision.
5. Run all workspace gates.
6. Capture the gallery on each supported platform.
7. Update this matrix and `PROVENANCE.md`.

Do not use a floating branch dependency.
