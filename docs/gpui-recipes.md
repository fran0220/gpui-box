# GPUI recipes

## Theme boot

```rust
let app = gpui_platform::application()
    .with_assets(gpui_kit::assets::Assets);

app.run(|cx| {
    gpui_kit::install(cx);
});
```

## Per-frame semantics

```rust
impl Render for View {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.semantics.begin_frame();
        // Render and attach NodeSpec values.
    }
}
```

## Disabled action

Use `action_button`; it omits the listener while disabled. If an application
builds its own interactive element, apply the same rule explicitly instead of
returning early inside an installed callback.

## Async refresh with stale data

```rust
let mut models = AsyncValue::ready(last_verified);
models.refresh();

// If the host refuses:
models.fail_refresh(error);
// models.value remains visible and status reports the error.
```

## Anchored menu

The trigger owns open state and conditionally adds:

```rust
trigger.child(popover::anchored_below(
    "model-menu",
    theme,
    popover::card(theme).children(rows).into_any_element(),
))
```

The owning view handles focus, keyboard navigation, outside-click dismissal,
Escape, and action dispatch.

## Capture after settle

In an `AsyncApp` task:

```rust
cx.background_executor().timer(settle).await;
let frame = cx.update(|cx| {
    handle.update(cx, |_, window, _| capture_window(window))
})??;
frame.write_png(path)?;
```

First wait for the semantic generation produced by the action. A timer by
itself is not proof that a frame occurred.
