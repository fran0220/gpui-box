# Examples

## Gallery

`gallery` is a real GPUI window that renders the public components at a fixed
920×900 logical viewport.

```bash
cargo run -p gpui-kit-gallery
cargo run -p gpui-kit-gallery -- \
  --density=compact --capture snapshots/macos/gallery.png
cargo run -p gpui-kit-gallery -- \
  --scene=lower --capture snapshots/macos/gallery-patterns.png
```

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

## Deferred browser gallery

`browser-gallery` is a product-neutral host for the same
`gpui_kit::scenes::catalog()` used by the native gallery, audits, and visual
gates. It copies no component or scene. The host fixes a logical viewport,
enables reduced motion, installs `gpui-kit`, and applies the requested bundled
theme and scene direction. Unsupported scene or theme ids render an explicit
Unavailable state.

```bash
rustup target add wasm32-unknown-unknown
cargo run -p xtask -- web check  # core library; passes without WebPlatform
cargo run -p xtask -- web build  # deferred browser host
cargo run -p xtask -- web smoke  # npm ci + pinned Playwright browser smoke
```

There was no existing JavaScript package manager convention in this
repository, so the deferred host uses a local `package.json` and
`package-lock.json`. `xtask web smoke` runs `npm ci` followed by `npm exec`; it
never relies on a globally installed or floating Playwright package. A small
Node HTTP server keeps the smoke command cross-platform.

The basic smoke deliberately uses `gpui_platform::single_threaded_web`, which
does not require the page to provide COOP/COEP headers. A threaded WebPlatform
host needs separate deployment-header and worker evidence and is not claimed
by this smoke. The current GPUI compatibility boundary is recorded once in
[`docs/compatibility.md`](../docs/compatibility.md).
