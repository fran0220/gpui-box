# Native iOS host validation — not full GPUI acceptance

This stage supplies a UIKit application host, `IosPlatform` / `PlatformWindow`
adapter, WGPU Metal surface lifecycle, GPUI text-input bridge, persistent
AccessKit projection, and three separately scoped applications. Native execution
has **not** been validated. `gpui_platform::application_with_ios_fonts` exposes
the explicit initializer; generic and headless initialization are refused.
Do not describe portable checks or a successful cross-compile as native support.

## Actual GPUI app (native build and execution pending)

The supported initialization contract is explicit and fallible:
`gpui_ios::application(fallback_family, Vec<Cow<'static, [u8]>>) -> Result<Application>`.
It requires the native main thread and refuses empty/invalid assets or a missing
fallback family. Font shaping and rasterization use exactly those assets through
CosmicTextSystem; no system-font discovery or headless UIKit fallback is claimed.
For diagnostics, retain an `Rc<IosPlatform>` and use `Application::with_platform`.

`bash examples/ios-native/build-gpui-simulator.sh` compiles the actual Rust
adapter and packages `target/ios-native/GpuiNative.app` with an ad-hoc signature.
It selects `aarch64-apple-ios-sim` on Apple silicon or `x86_64-apple-ios` on Intel.
After booting an available simulator as described below:

```sh
xcrun simctl install "$SIMULATOR_UDID" target/ios-native/GpuiNative.app
xcrun simctl launch --console "$SIMULATOR_UDID" dev.gpui-box.ios-native
```

`python3 -B examples/ios-native/run-simulator.py --gpui` automates this actual
GPUI app on a disposable available simulator. It requires both
`IOS_GPUI_WINDOW_OPENED` and `IOS_GPUI_FRAME_COMPLETED`, then captures a screenshot.
The second marker is emitted only after `WgpuRenderer::draw` returns true and an
immediate bounded GPU wait succeeds without pending validation/device errors.
An idle queue or window construction alone never produces it. The Rust build
has a 1,800-second timeout; other command/launch/cleanup bounds match the host
fixture. Always upload `target/ios-native/gpui-evidence/` (`run.json`, `build.log`,
`console.log`, and `host.png` when captured), separately from host fixture evidence.

Completion does not prove the pixels displayed. Inspect an actual screenshot for the GPUI-drawn title and subtitle,
then exercise background/foreground, rotation and surface recovery. This smoke
scene is not an editor and does not certify IME or accessibility interaction.
The example reuses the repository's existing Geist font asset and font license.
`cargo clippy -p gpui-box-ios --features platform-check --all-targets -- -D warnings`
type-checks the adapter and example on Linux without linking a substitute UIKit.

## Interactive mobile reference (separate launch and checkpoint storage)

`python3 -B examples/ios-native/run-simulator.py --reference` builds and launches
`GpuiReference.app` (`dev.gpui-box.ios-reference`), using the shared
`gpui-box-mobile-reference` mount and bundled Kit assets/fonts. It requires its
own `IOS_REFERENCE_MOUNTED` and `IOS_REFERENCE_FRAME_COMPLETED` markers; either
smoke fixture's markers are insufficient. Upload `target/ios-native/reference-evidence/`.
The automated run captures the initial frame and deletes its simulator. It does
not test editing, persistence, background behavior, or process recreation.

For interactive/recreation validation, keep an existing test simulator:

```sh
bash examples/ios-native/build-gpui-simulator.sh --reference
xcrun simctl install "$SIMULATOR_UDID" target/ios-native/GpuiReference.app
xcrun simctl launch --console "$SIMULATOR_UDID" dev.gpui-box.ios-reference
```

On the native Background lifecycle event the reference root runs
`prepare_background`; the host serializes its checkpoint to
`Application Support/mobile-reference.json` inside this app's sandbox using
temporary-file replacement. Relaunch reads it and calls `mount_checkpoint`.
Missing storage starts a fresh reference; malformed bytes, unsupported routes,
and I/O failures are explicit failures, never a claimed successful restore.
No checkpoint contents are logged or copied into runner artifacts.

Test real keyboard composition and edited drafts, background the app, terminate
and relaunch the same installed app, then inspect restored routes and values.
Termination without a Background event is not a save guarantee. No keyboard,
checkpoint persistence, or restore acceptance is claimed before that native run.

## Reproducible simulator path without a development team

Required access: a macOS machine with full Xcode selected by `xcode-select`,
accepted Xcode license, iPhoneSimulator SDK, an installed iOS 16+ Simulator
runtime, Metal-capable simulator, and an interactive session for keyboard,
touch, rotation and accessibility checks. `xcrun`, `simctl`, `clang`, `plutil`
and `codesign` come from Xcode/macOS. No Apple account, provisioning profile or
distribution certificate is needed for the simulator's local ad-hoc signature.

For an on-demand `macos-15` CI lane, run
`python3 -B examples/ios-native/run-simulator.py`. It builds the app, selects the
newest **installed and available** iOS 16+ runtime with an available iPhone
device configuration, creates a disposable device of that installed type,
boots/installs/launches, waits at most 60 seconds for the three native assertion
markers, captures a screenshot, then terminates and deletes its own simulator.
No specific iPhone model is assumed. A missing runtime fails, never skips.

Upload `target/ios-native/evidence/` even on failure: `run.json` records commit,
Xcode, SDK, architecture, runtime, device type and outcome; `console.log`
contains actual process output; `host.png` is the real simulator screenshot
when capture succeeded. `build.log` captures native compilation; pre-build and
runtime-selection failures still write failed `run.json` metadata. Commands
have 180-second bounds, the build 600 seconds, and cleanup commands 30 seconds.
Every cleanup operation is attempted even after another fails; nonzero cleanup
exit codes and stderr are recorded and prevent a successful run status.
The screenshot requires human/agent visual inspection;
the runner does not declare visual, real IME, VoiceOver or GPUI acceptance.
`python3 -B examples/ios-native/test_runner.py` tests runtime selection and
subprocess-failure cleanup/metadata behavior, not native acceptance.

For interactive checks instead:

```sh
bash examples/ios-native/check-portable.sh
bash examples/ios-native/build-simulator.sh
xcrun simctl list devices available
# Set SIMULATOR_UDID to an existing disposable test simulator from that list.
xcrun simctl boot "$SIMULATOR_UDID" # omit if already booted
xcrun simctl bootstatus "$SIMULATOR_UDID" -b
xcrun simctl install "$SIMULATOR_UDID" target/ios-native/GpuiHostFixture.app
xcrun simctl launch --console "$SIMULATOR_UDID" dev.gpui-box.ios-host-fixture
```

The process stays open for interactive checks. Native startup assertions must
produce all three markers, not merely a successful install or launch:

* `IOS_TEXT_PROTOCOL_PASS emoji-delete composition-replace utf16-range`
* `IOS_ACCESSIBILITY_BRIDGE_PASS identity action`
* `IOS_METAL_PRESENT_PASS frames=3`

These prove only the native fixture paths named in the markers. Direct
`UITextInput` calls are not evidence of an actual Chinese/Japanese keyboard
composition session. Direct `accessibilityActivate` is not VoiceOver evidence.
The solid Metal clear does not validate GPUI rendering, shaders or text.

Record Xcode version, SDK/runtime, simulator model/UDID, architecture, commit,
complete console log and inspected screenshots in the validation report.
Do not replace a real failed run with these expected marker strings.

## Interactive/native acceptance still required

1. Tap and drag with two contacts; inspect contact IDs and phases. Test
   interruption while touching and while portable momentum is running once
   GPUI is wired. No native pinch events may duplicate contacts.
2. Show/dismiss keyboard, rotate both ways, resize iPad viewport, and test
   docked/floating keyboards. Inspect every `IOS_GEOMETRY` transition for
   current logical viewport units, safe-area separation and no double-count.
3. Use real Chinese and Japanese keyboards, autocorrection, emoji/combining
   deletion, selected replacement, focus changes and hardware keyboard.
4. Home/background and foreground repeatedly. Cancellation must precede
   suspension; no Metal work may be submitted in background. Inspect recovery
   errors, not just whether the app remains alive.
5. Run VoiceOver/Accessibility Inspector: focus order, stable focus across
   updates, rotated bounds, activation, adjustable actions, disabled refusal.
6. Once the GPUI adapter exists, repeat all tests through real GPUI components
   and capture GPUI text, clipping, hit testing and renderer recovery evidence.

Capture review artifacts after a real run:

```sh
mkdir -p .amp/in/artifacts
xcrun simctl io "$SIMULATOR_UDID" screenshot .amp/in/artifacts/ios-host.png
```

## Implementation dependencies (not toolchain blockers)

`UITextPosition` retains offset and affinity; native selection atomically retains
anchor and head. Directional navigation, caret bounds, farthest position,
selection fragments and grapheme queries delegate to the shared GPUI handler.
Absent geometry returns unavailable. `firstRectForRange` selects the fragment
containing the logical start, never a union across unrelated bidi runs.
Paragraph direction mutation is refused by editors that do not implement it.
Hit testing delegates logical window points and optional UTF16 range restrictions
to `native_position_for_point`, preserving returned affinity. The core filters
actual visual stops before nearest-point selection; the UIKit adapter never
clamps a logical bidi endpoint. Editors must implement the shared native hooks
to expose editing geometry; unsupported hooks remain unavailable.

The host is a single legacy application window, not multi-scene restoration.
URL handling, string pasteboard, thermal/input-mode/appearance notifications and
checked app/window refusals are implemented. Hardware-key mapping, pointer hover,
document pickers, Keychain and multi-scene restoration remain unsupported.
Keyboard purpose, return action, secure input and autofill hints map to UIKit
traits; this is not evidence of working SMS autofill or actual IME composition.

The adapter consumes `UiKit` raw handles on the main thread, retains the host
longer than every surface, and calls fallible WGPU detach/drain before background
and destroy before releasing UIKit. Failure disables rendering and is retained
in `IosPlatform::last_error`. These lifetime/timing properties require actual
native execution. The direct-Metal fixture does not replace that validation.

Actual device validation additionally requires an arm64 device SDK build,
connected/trusted device with Developer Mode, a development team, device
registration and a matching development provisioning profile/signing identity.
No device signing material is requested or read by the simulator script.
Rust adapter compilation additionally requires the repository toolchain and
`aarch64-apple-ios-sim` (Apple silicon), `x86_64-apple-ios` (Intel simulator),
and `aarch64-apple-ios` (device). Linux has no supported native acceptance path.

## Provenance and integration receipt

`crates/gpui_ios` and this fixture are original GPUI Box work, Apache-2.0.
No Apple example source, macOS source port, new font asset or frozen historical
import receipt is included. API references inspected:

* https://developer.apple.com/documentation/uikit/uitextinput
* https://developer.apple.com/documentation/uikit/uikeyboardlayoutguide
* https://developer.apple.com/documentation/uikit/uiaccessibilityelement
* https://github.com/gfx-rs/wgpu/blob/v30.0.0/wgpu-hal/src/metal/mod.rs
* https://docs.rs/raw-window-metal/1.1.0/raw_window_metal/struct.Layer.html

Parent integration should record this original native-host stage in
`PROVENANCE.md` and `THIRD_PARTY_NOTICES`, preserving both frozen receipts, and
record **native execution pending / GPUI adapter acceptance incomplete** in compatibility
documentation. Workspace registration and the explicit font-supplied initializer
do not mark iOS as a validated platform. The dispatch-only `iOS native smoke`
workflow executes both simulator fixtures and retains their evidence separately.
