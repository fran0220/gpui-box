# Migration guide

This guide moves an existing GPUI application onto the `gpui-box` framework
and `gpui-box-kit` component library without a big-bang shell rewrite.

```toml
[dependencies]
gpui = { package = "gpui-box", version = "0.1" }
gpui_kit = { package = "gpui-box-kit", version = "0.1" }
```

## 1. Inventory

Classify current code:

- raw palette and repeated metrics;
- product-neutral primitives;
- reusable interaction patterns;
- product view models and actions;
- host, persistence, process, and credential authority;
- automation and screenshot infrastructure.

Only the first three categories migrate to the kit.

## 2. Establish visual baselines

Capture fixed viewports for important states before changing dependencies.
Record hover, selected, disabled, focus, loading, empty, error, stale, popover,
and dialog states.

## 3. Map tokens

Add a theme document or map the existing design to `studio-dark`. Do not keep
an old theme global active beside `gpui_kit_theme::Theme`.

Replace:

```rust
const PANEL: u32 = 0x0d0d0d;
```

with:

```rust
let theme = Theme::get(cx);
theme.colors.panel
```

## 4. Install assets and theme

```rust
let app = gpui_platform::application()
    .with_assets(gpui_kit::assets::Assets);

app.run(|cx| {
    gpui_kit::install(cx);
    // Open application windows.
});
```

Applications with their own assets can compose or delegate an `AssetSource`.

## 5. Migrate low-coupling primitives

Move in this order:

1. badges and status;
2. buttons;
3. cards and settings scaffolding;
4. loaders and motion;
5. popovers and dialogs;
6. frost and edge fade.

After each move, delete the application copy. Do not retain two primitive sets.

## 6. Add semantic IDs

Create a `SemanticRegistry` per product window. Begin a frame before render and
attach `NodeSpec` to every action and assertion target. Keep application IDs in
the application; the library does not define product vocabulary.

## 7. Preserve the host boundary

Convert product models into view models before rendering. Components must not
gain direct host references merely to simplify migration.

## 8. Replace automation

Use semantic generation to wait for frames, target controls by stable id, and
capture the owned window. Keep any RPC server and input injection debug-only.

## 9. Remove duplicates

Search for:

- second Theme globals;
- copied RGB or alpha values;
- duplicate Loadable enums;
- duplicate popover geometry;
- multiple semantic registries;
- full-desktop screenshot code.

The migration is complete only after the old implementation is gone.

## Forge-specific note

Forge integration is intentionally not part of this repository's initial
implementation. A future Forge migration should happen through a reviewed
dependency update while preserving Forge's product and host authority.
