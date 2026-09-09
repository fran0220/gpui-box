# Opt-in native capture review font

NotoColorEmoji.ttf is the unmodified full upstream v2.042 font, not the orb's
Debian rebuild and not a subset. Keeping upstream bytes preserves font naming,
color bitmap tables, and license metadata without a transformation toolchain.
Its approximately 10 MiB cost is confined to consumers opting into the helper.

- Upstream: https://github.com/googlefonts/noto-emoji
- Release: v2.042
- Exact tag commit: d79d23e6822e0f6e5731b114cbfb26b2a4e380da
- Source: fonts/NotoColorEmoji.ttf and fonts/LICENSE at that commit
- Font version: 2.042;GOOG;noto-emoji:20231129:7f49a00d523ae5f94e52fd9f9a39bac9cf65f958
- Copyright 2022 Google Inc. (embedded font name table)
- License: SIL Open Font License 1.1; verbatim upstream text in OFL.txt
- Font SHA256: c2f19f6a404baa7da7a710b018c2892d7b51386983ddca146811f76aea0b6861
- OFL.txt SHA256: 6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2

The independent helper is tools/app-host/src/review_fonts.rs. The capture owner
must explicitly declare the module in the native capture feature scope and call:

```rust,ignore
let text_system = gpui_platform::test_text_system("Geist");
review_fonts::register_emoji_review_font(text_system.as_ref())?;
// Pass this same Arc into HeadlessAppContext, then install Kit and shape text.
```

No capture wiring, default renderer behavior, default test fonts, Kit assets,
or system font discovery changes here. Register once per fresh text system.
Linux software offscreen review is not evidence of hardware, live X11, or
cross-platform parity. The runtime owner reruns the actual JS click fixture.
