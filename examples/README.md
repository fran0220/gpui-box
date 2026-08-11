# GPUI Box examples

## Native gallery

The unpublished `gpui-box-gallery` package renders the public GPUI Box Kit
components in a 920×900 logical window using deterministic fixture data.

```bash
cargo run -p gpui-box-gallery
cargo run -p xtask -- scenes render
cargo run -p xtask -- scenes render button dialog
```

Real-window captures go to `target/scenes` for review; the visual gate is
`cargo run -p xtask -- headless check`. Fixtures are explicitly not
product-backed evidence. Hosts keep transports and product smoke tests outside
the component library.

## Browser gallery

`gpui-box-browser-gallery` hosts the same `gpui_kit::scenes::catalog()` as the
native gallery. It copies no component or scene and renders an explicit
Unavailable state for unknown scene/theme ids.

```bash
rustup target add wasm32-unknown-unknown
cargo run -p xtask -- web check
cargo run -p xtask -- web build
cargo run -p xtask -- web smoke
cargo run -p xtask -- web visual check button input dialog node-graph
```

The checked-in npm lockfile pins Playwright. The smoke covers WebGL2, WebGPU,
and fallback on the stable single-threaded path. It does not claim threaded
COOP/COEP operation or screen-reader announcements; see
[`docs/compatibility.md`](../docs/compatibility.md).
