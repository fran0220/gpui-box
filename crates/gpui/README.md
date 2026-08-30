# GPUI Box

GPUI Box is an independent distribution of GPUI: a hybrid immediate- and
retained-mode, GPU-accelerated UI framework for Rust. It is derived from GPUI
but maintained and released independently of Zed. GPUI Box is not an official
Zed project.

The framework is pre-1.0 and may make breaking changes between releases. Version
0.1.2 requires Rust 1.97 or newer.

## Getting Started

Use package aliases so application code keeps the familiar `gpui` and
`gpui_platform` crate names:

```toml
[dependencies]
gpui = { package = "gpui-box", version = "0.1.2" }
gpui_platform = { package = "gpui-box-platform", version = "0.1.2", features = ["font-kit", "wayland", "x11"] }
```

`gpui-box` and the `gpui-box-*` platform crates are released as one compatible
framework. Do not mix them with another GPUI distribution: Rust treats types
from separate GPUI packages as distinct, even when their names and source APIs
look alike. Using this package family throughout your dependency graph keeps a
single GPUI type universe and avoids incompatible `App`, `Window`, and element
types. Applications do not need a Zed checkout or a Git dependency.

Everything in a standalone GPUI Box application starts with an `Application`.
`gpui_platform::application()` selects the windowing and text backends for the
host OS. Pass a callback to `Application::run()`, then open a window with
`App::open_window()` and register a root view.

```rust,no_run
use gpui::*;

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        // Open your first window here.
    });
}
```

### Platform features

The `gpui-box-platform` features are platform-specific. The feature set above
is a safe default for a project that targets all supported desktop platforms.
Single-platform applications can trim it:

- **macOS** — rendering uses Metal. Enable `font-kit` for glyph rasterization;
  without it, the fallback text system lays text out but renders no glyphs.

  ```toml
  gpui_platform = { package = "gpui-box-platform", version = "0.1.2", features = ["font-kit"] }
  ```

- **Linux / FreeBSD** — enable `wayland`, `x11`, or both. These features also
  enable the renderer and text system.

  ```toml
  gpui_platform = { package = "gpui-box-platform", version = "0.1.2", features = ["wayland", "x11"] }
  ```

- **Windows** — no `gpui-box-platform` features are required. Windowing uses
  Win32 and text uses DirectWrite; `font-kit` has no effect.

## System dependencies

### macOS

GPUI Box uses Metal. Install Xcode from the
[Mac App Store](https://apps.apple.com/us/app/xcode/id497799835?mt=12) or the
[Apple Developer site](https://developer.apple.com/download/all/), launch it
once to install the macOS components, and install its command-line tools:

```sh
xcode-select --install
sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
```

## The Big Picture

GPUI provides three levels of API:

- **Entities** manage application state and communication. GPUI owns entities,
  which are accessed through owned smart pointers and framework contexts.
- **Views** provide high-level declarative UI. A view is an `Entity` that
  implements `Render`; each frame it builds and styles an element tree.
- **Elements** provide low-level imperative layout and painting for custom or
  performance-sensitive UI such as large virtualized lists.

The framework also provides user-defined actions and key bindings, platform
services, an async executor integrated with the event loop, and `#[gpui::test]`
with `TestAppContext` for UI tests.

### Measured subtree reveals

`reveal` is the framework boundary for disclosure geometry. It measures a
subtree at its natural primary-axis extent, contributes a caller-supplied
fraction of that extent to ordinary layout, and applies the same clipping to
paint, hit testing, and accessibility bounds. It owns no timer or easing; a
component or application supplies progress from its own motion policy. The id
retains the natural measurement across frames.

```rust,no_run
# use gpui::{div, reveal, ParentElement};
# let progress = 0.5;
let details = reveal("details-reveal", progress, div().child("Details"));
```

Vertical reveals open from the top by default. `Reveal::axis` selects the
horizontal axis and `Reveal::from_end` anchors a partial reveal to the physical
bottom or right edge. At zero, the subtree is measured but not prepainted, hit
testable, or addressable.

## Documentation and support

- [GPUI Box repository](https://github.com/fran0220/gpui-box)
- [Umbrella project README](https://github.com/fran0220/gpui-box/blob/main/README.md)
- [Compatibility notes](https://github.com/fran0220/gpui-box/blob/main/docs/compatibility.md)
- [Source provenance](https://github.com/fran0220/gpui-box/blob/main/PROVENANCE.md)
- [Examples](https://github.com/fran0220/gpui-box/tree/main/crates/gpui/examples)
