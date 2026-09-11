# Mobile reference fixture

This example owns fixture data and routing policy. Kit owns components and
transient interaction state; native wrappers own lifecycle, persistence,
keyboard delivery, browser history/deep links and system back. Nothing here
contacts a server or reports a simulated native operation as successful.

## Run and verify

```sh
cargo run -p gpui-box-mobile-reference --features desktop --bin mobile-reference
cargo test -p gpui-box-mobile-reference --lib
cargo clippy -p gpui-box-mobile-reference --all-features --all-targets -- -D warnings
cargo run -p gpui-box-mobile-reference --features capture --example capture -- target/mobile-reference
```

The capture program opens actual **390 × 844** and **390 × 520 logical-pixel
windows**, checks action targets are visible, dispatches real pointer events,
asserts semantic results and saves 12 renders. The renderer may produce 2× image
pixels. The shorter window verifies resized layout, **not** the appearance of
a native keyboard. It uses reduced motion and does not verify gesture timing.
The optional desktop/capture dependencies never enter the default library.

## Native mount contract

Provide `gpui_kit::assets::Assets` to the platform application and call
`gpui_kit::install(cx)` once for bundled fonts, theme and semantics. A native
text system that requires its fonts at construction can use the Kit asset
crate's `font_bytes()` as the native fixtures do.

```rust,ignore
use gpui_box_mobile_reference::{mount, mount_checkpoint, ReferenceApp, state::Checkpoint};

// Platform owns the window and application context.
fn mount(window: &mut gpui::Window, cx: &mut gpui::App) -> gpui::Entity<ReferenceApp>;
fn mount_checkpoint(saved: Checkpoint, window: &mut gpui::Window, cx: &mut gpui::App)
    -> Result<gpui::Entity<ReferenceApp>, &'static str>;

// Invoke through the mounted entity or typed window handle.
fn prepare_background(&mut self, window: &mut gpui::Window,
    cx: &mut gpui::Context<Self>) -> Checkpoint;
fn request_back(&mut self, window: &mut gpui::Window,
    cx: &mut gpui::Context<Self>) -> bool;
```

Mount does **not** register platform back callbacks or change enabled state.
The native wrapper bridges those callbacks through the mounted window. Back
closes an open sheet or picker before attempting to leave the current route;
it does not accept a draft or alter the picker selection. `request_back == false` can mean
refused or root and never authorizes exiting by itself. The visible notice
states refusals; hosts may inspect `state.dirty`, `state.tab`, and
`state.history.can_pop()` to distinguish their policies.

`Checkpoint` is versioned (currently 1), `Clone + serde::Serialize +
serde::Deserialize`. Native wrappers persist its JSON bytes using their own
sandbox storage and call `mount_checkpoint` on recreation. Invalid JSON,
unknown versions, visit sequences or unresolved routes are errors; there is no
silent new-state fallback. Checkpoints retain Chinese/multiline drafts, dirty
state, current tab, visit history, verified rows and pin values. They exclude
focus, keyboard state, open pickers/sheets, gestures and refresh activity.
Recreation mounts fresh controls; refresh is not automatically replayed.
No private data should be placed in exported semantic snapshots.

## Fixture walkthrough

- **Library:** refresh deliberately fails while keeping verified rows. Row
  Actions buttons are keyboard/touch alternatives to framework-owned swipes.
  Reveal alone changes no data. Pin is an explicit in-memory fixture update;
  Remove is explicitly refused and Share is disabled.
- **Detail:** selecting a record creates a visit identity separate from the
  record ID. Back retains forward history.
- **Form:** Chinese name, multiline draft and retained bottom Select. Any edit
  marks the draft dirty. Back and tab changes are refused until **Accept
  fixture draft**, which is not a server save.
- **Checkpoint / Restore:** exercise the same checkpoint and recreation policy
  in memory. The buttons are not labelled native background/resume and write
  no storage.
- **Image:** Kit ImageViewer displays a caller-supplied fixture illustration
  with Touch-sized fit/zoom controls. The fixture accepts fit requests in
  transient host state; image zoom is not checkpointed. **Sheet** opens the existing keyboard-aware Kit
  surface with a real text input; only platform-provided insets are consumed.

Native IME composition, safe-area changes, keyboard occlusion, device swipes,
system/predictive back, and OS process restoration require the native fixture
lanes. Desktop/headless success is not evidence of those native behaviors.
