# Examples

## Gallery

`gallery` is a real GPUI window that renders the public components at a fixed
920×900 logical viewport.

```bash
cargo run -p gpui-kit-gallery
cargo run -p xtask -- scenes render
cargo run -p xtask -- scenes render button dialog
```

`scenes render` writes real-window images to `target/scenes` for human review.
The visual gate uses `cargo run -p xtask -- headless check` instead.

Its data is deterministic fixture content, explicitly not product-backed
evidence. The example demonstrates:

- action variants and disabled behavior;
- semantic badges and callouts;
- settings-page/card patterns;
- stale-data messaging;
- pulse, gradient, and skeleton loaders;
- menu/popover geometry;
- semantic registration.

Host-backed applications should keep their own host smoke tests outside this
repository and use the same components over real view models.

## Browser gallery

`browser-gallery` is a product-neutral host for the same
`gpui_kit::scenes::catalog()` used by the native gallery, audits, and visual
gates. It copies no component or scene. The host fixes a logical viewport,
enables reduced motion, installs `gpui-kit`, and applies the requested bundled
theme and scene direction. Unsupported scene or theme ids render an explicit
Unavailable state.

```bash
rustup target add wasm32-unknown-unknown
cargo run -p xtask -- web check  # core library; passes without WebPlatform
cargo run -p xtask -- web build  # stable WebPlatform browser host
cargo run -p xtask -- web smoke  # npm ci + pinned Playwright browser smoke
cargo run -p xtask -- web visual check button input dialog node-graph
```

There was no existing JavaScript package manager convention in this
repository, so the browser host uses a local `package.json` and
`package-lock.json`. `xtask web smoke` runs `npm ci` followed by `npm exec`; it
never relies on a globally installed or floating Playwright package. A small
Node HTTP server keeps the smoke command cross-platform.

The smoke uses WebPlatform's stable single-threaded default and does not require
the page to provide COOP/COEP headers. It covers forced WebGL2, forced WebGPU,
and Auto fallback to WebGL2 when WebGPU is disabled. A threaded WebPlatform host
needs separate deployment-header and worker evidence and is not claimed by
this smoke. The current GPUI compatibility boundary is recorded once in
[`docs/compatibility.md`](../docs/compatibility.md).
