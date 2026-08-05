# Examples

## Gallery

`gallery` is a real GPUI window that renders the public components at a fixed
920×900 logical viewport.

```bash
cargo run -p gpui-kit-gallery
cargo run -p gpui-kit-gallery -- --capture snapshots/macos/gallery.png
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
