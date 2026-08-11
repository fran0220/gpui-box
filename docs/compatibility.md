# GPUI Box compatibility

## 0.1.x matrix

GPUI Box owns its framework and package compatibility. There is one package and
type universe: applications use Cargo package `gpui-box` as Rust crate `gpui`,
and `gpui-box-kit` as `gpui_kit`. Zed crates must not be mixed into that graph.

| GPUI Box cohort | Framework origin | Rust | Status |
|---|---|---|---|
| `0.1.x` | filtered bootstrap `fran0220/zed@0b9c8dc932b65cba2dc87464148984e93f60ae18`; official baseline `a6a23c7b80a5cefa0487b7856335be89ace7e483` | 1.97, edition 2024 | unreleased release candidate |

The SHAs identify imported source provenance, not Cargo Git dependencies.
`scripts/sync-zed/state.json` currently has null vendor tip, cursor, and
integration commit: filtered-sync history has not been bootstrapped. Do not
describe the baseline as a completed sync cursor until that receipt changes.

## Platform evidence and limits

| Platform | Current repository contract | Validation command/evidence | Limits |
|---|---|---|---|
| macOS | Framework, kit, Metal, and deterministic headless catalog | CI runs the all-feature native check and `cargo run -p xtask -- headless check`; real-window review remains separate | A local result is not evidence for another OS; real-window accessibility needs a logged-in host |
| Windows | Framework/kit and deterministic WGPU/WARP headless catalog | CI runs the all-feature native check and `cargo run -p xtask -- headless check`; Windows baselines exist | Native frame capture is not implemented; detailed UIA limits are in `accessibility.md` |
| Linux | Wayland/X11 framework code and deterministic llvmpipe headless catalog | CI runs the all-feature native check and `cargo run -p xtask -- headless check`; Linux baselines exist | AT-SPI and native behavior claims remain capability-scoped |
| Browser/WASM | Stable, single-threaded browser gallery using the same Rust scenes | CI runs `web check` and the real Chromium `web smoke`; the visual command remains available for scoped review | No threaded COOP/COEP claim and no screen-reader announcement coverage |

All four rows are mandatory CI surfaces. A release may claim only results
recorded for its commit; the commands do not erase the explicit limitations in
the final column. Accessibility capability details remain in
[`accessibility.md`](accessibility.md), and visual mechanics in
[`screenshot-testing.md`](screenshot-testing.md).

Browser checks are:

```bash
rustup target add wasm32-unknown-unknown
cargo run -p xtask -- web check
cargo run -p xtask -- web build
cargo run -p xtask -- web smoke
cargo run -p xtask -- web visual check button input dialog node-graph
```

The browser host is not a DOM rewrite. It covers forced WebGL2, forced WebGPU,
and automatic fallback in the pinned Playwright smoke. Its AccessKit adapter
mirrors roles, focus, actions, values, and canvas-scaled bounds into semantic
DOM, but the JSON semantic snapshot is only a testing/debug surface.

## Upstream update contract

Zed is upstream input and a compatibility-validation object, not a dependency.
To update it: verify `scripts/sync-zed/config.json`; bootstrap history first if
the receipt remains null; sync an official full SHA; resolve conflicts
manually; update the receipt and provenance; run `dependencies check`, package
check, all claimed platform gates, and inspect changed images. Generally useful
changes may still be proposed to Zed independently. GPUI Box remains an
independent derivative and is not endorsed by Zed Industries.
