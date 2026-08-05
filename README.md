# gpui-kit

`gpui-kit` is a design system, component library, semantic automation layer,
and visual test kit for native desktop applications built with
[GPUI](https://github.com/zed-industries/zed).

It extracts product-neutral lessons from a production Studio interface:

- one typed token authority instead of colors scattered through views;
- compact native-desktop components with complete interaction states;
- truthful async state that does not turn failures into empty or successful UI;
- host/view separation, where components read view models and emit actions;
- a per-frame semantic tree for native windows without a DOM;
- in-process window capture and deterministic visual fixtures.

The repository does **not** contain OriginGame, Forge, agent runtime, account,
project, thread, or workflow domain logic.

## Workspace

| Crate | Responsibility |
|---|---|
| `gpui-kit-tokens` | GPUI-independent token document, validation, and typed semantic access |
| `gpui-kit-theme` | The single Token → GPUI adapter and `Theme` global |
| `gpui-kit-assets` | Licensed Geist fonts and product-neutral SVG icons |
| `gpui-kit` | Components, motion, frost, edge fade, popovers, settings patterns, truthful state |
| `gpui-kit-semantics` | Per-frame semantic nodes measured during GPUI prepaint |
| `gpui-kit-testkit` | Semantic assertions, window capture, PNG output, and frame comparison |
| `gpui-kit-gallery` | Runnable component gallery and visual fixture |

## Quick start

```toml
[dependencies]
gpui-kit = { git = "https://github.com/fran0220/gpui-kit", rev = "<commit>" }
gpui-kit-semantics = { git = "https://github.com/fran0220/gpui-kit", rev = "<commit>" }
```

```rust
use gpui_kit::button::{action_button, ButtonSize, ButtonVariant};
use gpui_kit_theme::Theme;

// Once, during application boot.
gpui_kit::install(cx);

// In a view.
let theme = Theme::get(cx);
let save = action_button(
    "settings.save",
    theme,
    "Save",
    ButtonVariant::Primary,
    ButtonSize::Medium,
    saving,
    move |window, cx| {
        controller.update(cx, |controller, cx| controller.save(window, cx));
    },
);
```

`action_button` does not install its click handler while disabled. Disabled is
therefore behavior, not only opacity.

## Run the gallery

```bash
cargo run -p gpui-kit-gallery
```

Capture the gallery's own window on macOS:

```bash
cargo run -p gpui-kit-gallery -- \
  --capture snapshots/macos/gallery.png
```

This uses the owning process and window id. It does not capture the desktop.

## Tokens

`tokens/studio-dark.json` is the source of truth. Views consume semantic roles
through `gpui-kit-tokens` and `gpui-kit-theme`.

```bash
cargo run -p xtask -- tokens generate
cargo run -p xtask -- tokens check
```

The first command updates `docs/token-reference.md`; the second fails if that
generated reference has drifted.

## Validation

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- tokens check
```

## GPUI compatibility

The workspace pins the public `fran0220/zed` fork at
`cb2ce82766e42c55fc8563de3c5c98ac8e1c9fd9`. The fork contains the
EdgeFade, BackdropBlur, and native overlay patches required by the component
effects. See [`docs/compatibility.md`](docs/compatibility.md).

## Documentation

- [Design principles](docs/design-principles.md)
- [Token model](docs/token-model.md)
- [Component contracts](docs/component-contracts.md)
- [Truthful UI](docs/truthful-ui.md)
- [Host/view boundary](docs/host-view-boundary.md)
- [Semantic automation](docs/semantic-automation.md)
- [Screenshot testing](docs/screenshot-testing.md)
- [Accessibility](docs/accessibility.md)
- [GPUI recipes](docs/gpui-recipes.md)
- [Migration guide](docs/migration-guide.md)
- [Agent Skill](skills/building-gpui-product-ui/SKILL.md)

## License

The repository code is MIT licensed. Included and derived third-party material
retains its original license and attribution. See
[`THIRD_PARTY_NOTICES`](THIRD_PARTY_NOTICES) and
[`PROVENANCE.md`](PROVENANCE.md).
