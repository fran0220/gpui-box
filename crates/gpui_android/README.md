# Android Activity backend — experimental, native validation pending

This is an Android implementation, not a declaration that Android is a validated
GPUI Box platform. The initial supported **development scope is Android 13+
(API 33), arm64-v8a, Vulkan, one Activity and one opaque GPUI window**. A licensed
Android SDK/NDK and a real device or externally hosted emulator are required to
complete its acceptance. No emulator is installed or run in the Linux orb.

## Host contract

Include `java/dev/gpui/box/*.java` in the Android application. Subclass
`GpuiActivity`, load the application's Rust cdylib and invoke
`gpui_android::initialize` from `createNative` through JNI. Return the
`ApplicationHandle` produced by `Application::run_embedded`; this host retains
that handle until Activity destruction. Do not use `Application::run`, whose
stack no longer retains application state after the Android callback returns.

The Android UI thread owns all GPUI state. `Handler` wakes drain foreground
tasks even while frames are suspended; background jobs and timers do not invoke
GPUI UI callbacks directly. `Choreographer` requests frames only while resumed
and a surface is available. `SurfaceHolder.surfaceDestroyed` synchronously
drains WGPU and drops its Surface before returning. The NDK window reference is
released after the renderer detaches. Surface replacement preserves the GPU
device, sprite atlas and caller-owned entities. Device-loss recovery uses the
renderer recovery path and does not claim preserved GPU contents after loss.

The manifest keeps the Activity across rotation, density, keyboard and other
listed configuration changes. Background/foreground and surface loss preserve
the in-process model. **Android process death does not preserve Rust entities**;
durable product state and saved-state policy belong to the caller. A fresh
process creates a fresh fixture in this example. The backend does not silently
persist arbitrary application data or secrets.

Input uses stable `MotionEvent` pointer ids, not pointer indexes. Coordinates
are divided by density once into logical window pixels. Every pointer is
forwarded as `PlatformInput::Touch`; no duplicate mouse or native pinch stream
is synthesized. `ACTION_CANCEL` cancels every tracked contact. Pause, focus
loss and surface detach also deliver inactive status so GPUI cancels gestures,
momentum and pending long-press timers.

The view is edge-to-edge with `adjustNothing`. `WindowInsets` system bars,
cutouts and IME occlusion are reported separately in logical pixels, including
`WindowInsetsAnimation` progress. There is no simultaneous keyboard-driven
viewport resize to double-count. Hosts changing this window policy must also
change their viewport-relative inset conversion.

`InputConnection` reads and edits the authoritative GPUI input handler in
UTF-16 units. It supports selection, commit/composing text, composing regions,
finish composition, surrounding text and code-point deletion. Focus-generation
tokens reject old connections after focus moves. Focus/options notifications
post to Android's Handler before querying GPUI, avoiding a nested App borrow.
Selection snapshots preserve native anchor/head order; Android selection writes
use the atomic `NativeTextSelection` hook with downstream affinity because
Android supplies no affinity. Editors must implement the native selection hooks;
unsupported handlers are refused rather than silently losing direction.
Kit's TextInput and TextArea implement these hooks, including their masked/OTP
wrappers. RichTextEditor still has only its legacy input handler and is not
accepted by this native selection bridge.
Keyboard purpose, multiline, secure and return-key hints map to `EditorInfo`;
return actions are accepted only when the editor accepts them. Password values
are not exported to accessibility, and secure keyboard sessions disable
personalized learning. These are OS hints, not a guarantee about a third-party
keyboard's behavior.

System back registers an Android `OnBackInvokedCallback` only while
`Window::set_back_enabled(true)` is in effect; the application supplies its
handler. Otherwise Android owns back. This stage handles back invocation, not
predictive-back progress animations.

Accessibility uses an `AccessibilityNodeProvider` with stable virtual ids from
AccessKit identities, child relationships, physical/clipped bounds, focus,
click/set-value actions, subtree change events and touch exploration. GPUI
already scales AccessKit bounds; Android adds only the view's screen origin.
The adapter is a first native bridge, **not a complete AccessKit Android adapter**.

Appearance reads the Activity's actual `Configuration.uiMode` at initialization
and on configuration changes. A changed night mode notifies GPUI's window
appearance callback to invalidate rendering. Caller themes still own colors.

## Explicit limits

- No desktop windows, popups, titlebars, native application/dock menus, external
  drag, screen capture service, filesystem pickers or credential storage.
  Fallible APIs return unsupported errors; legacy void requests fail explicitly.
  Registration of GPUI's automatic desktop-menu callbacks is inert because no
  menu can be installed. There are no hover/moved/chrome callbacks on the
  touch-only Activity window.
- Multi-window display enumeration, native view
  portals, hardware mouse/stylus semantics and full hardware keyboard/dead-key
  layout mapping are not implemented. The current keyboard mapper is explicitly
  the GPUI dummy mapper; do not advertise physical shortcut-layout support.
- No rich-content IME attachments, extracted-text monitoring, batched document
  edits, handwriting or OS autofill/SMS retrieval integration. Unsupported
  InputConnection capabilities return false/null.
- Accessibility does not yet map the whole AccessKit role/action vocabulary,
  text navigation, ranges, collection metadata, live-region announcements or
  complex transformed synthetic subtrees. TalkBack and switch-access acceptance
  remains required; the tests below are not a substitute.
- Native GPU/Activity/IME behavior and Java compilation have not been verified
  in the initial orb stage because no licensed SDK/NDK or native device was
  available. Do not turn host-check success into a mobile compatibility claim.

## Validation and SDK approval

Host-feasible checks:

```sh
cargo test -p gpui-box-android
cargo check -p gpui-box-android-example --features host-check
cargo clippy -p gpui-box-android -p gpui-box-android-example --features host-check -- -D warnings
bash -n examples/android/build-apk.sh
```

`host-check` checks Rust/JNI types against the real traits on Linux. It does not
emulate Android, link an APK or test Java/native ABI delivery.

SDK license review: <https://developer.android.com/studio#terms-and-conditions>
and <https://developer.android.com/ndk/downloads>. **Do not accept SDK licenses on
the user's behalf.** After explicit authorization or on an existing licensed
SDK installation, the user can review each interactive license prompt with:

```sh
"$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" --licenses
"$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" \
  'platforms;android-35' 'build-tools;35.0.0' 'ndk;28.2.13676358' 'platform-tools'
rustup target add aarch64-linux-android
bash examples/android/build-apk.sh
```

The build requires JDK 17+, Bash 4+, zip and the repository Rust toolchain. It
uses only pinned SDK tools and the local Cargo authority, produces an ordinary
debug-signed APK and checks APK signing and 16-KiB native library alignment.
This is a repeatable development recipe, not a bit-identical release artifact:
the development keystore and archive timestamps are local. Never ship this
debuggable APK or its test instrumentation as a production release.

See [the native acceptance procedure](../../examples/android/README.md) for
device commands and the remaining manual acceptance cases.

## Provenance and parent integration

The Android Rust/JNI/Java bridge and fixture are original GPUI Box work using
the platform contracts and Android public APIs. They are not a copied Android
adapter from another repository. The example reuses the repository's IBM Plex
Sans Regular font in `gpui_web/assets/fonts`, under its existing OFL notice;
no new font binary or trademark asset is introduced. Android SDK components
are external build prerequisites, not vendored code.

Authoritative references consulted:

- <https://developer.android.com/reference/android/view/SurfaceHolder.Callback>
- <https://developer.android.com/reference/android/view/inputmethod/BaseInputConnection>
- <https://developer.android.com/reference/android/view/accessibility/AccessibilityNodeProvider>
- <https://developer.android.com/reference/android/view/WindowInsetsAnimation.Callback>
- <https://developer.android.com/guide/navigation/custom-back/predictive-back-gesture>

The parent integration owns root/shared metadata. Add workspace members
`crates/gpui_android` and `examples/android`, plus workspace dependency
`gpui_android = { package = "gpui-box-android", path = "crates/gpui_android", version = "0.1.2" }`.
Add that optional dependency under `cfg(target_os = "android")` in
`gpui_platform`, include it in `native-platform`, and dispatch Android with
`AndroidPlatform::current().expect("initialize the Android Activity first")`.
Android headless creation must fail explicitly; it cannot create an Activity.
The example uses `initialize` directly and does not need convenience dispatch.
Update `Cargo.lock`, dependency authority/catalog generation, `PROVENANCE.md`,
`THIRD_PARTY_NOTICES` and `compatibility.toml` with **experimental / native
validation pending**, not a passing Android lane. Shared input stage1/stage2/stage3,
editor adapters implementing atomic native selection,
the fallible mobile operation contract, and WGPU's fallible detach/destroy and
two-argument surface replacement are required integration dependencies.
