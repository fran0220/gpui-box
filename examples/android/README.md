# Run Android native acceptance on an actual device

Prerequisites: an authorized ADB connection to a disposable arm64 Android 13+
device (or an emulator on another machine), Vulkan support, an unlocked screen,
a licensed SDK/NDK and JDK. The Linux orb has no KVM and must not host an emulator.
See the [backend contract and SDK license approval](../../crates/gpui_android/README.md).

Build and install the **fixture-only, debug** APK:

```sh
bash examples/android/build-apk.sh
adb devices -l                         # confirm the intended authorized device
export ANDROID_SERIAL='<device serial>'
adb install -r target/android-apk/gpui-box-debug.apk
adb shell am instrument -w dev.gpui.box.example/dev.gpui.box.NativeSmoke
```

The instrumentation must print `PASS:` and `INSTRUMENTATION_CODE: -1`.
Any `FAIL:`, Java/native exception, timeout, missing input handler, empty hit
bounds or missing GPU presentation is a failure. A successful install or
Activity launch alone is not native validation.

The test checks real injected touch→counter changes, system back, accessibility hit bounds,
UTF-16 emoji replacement, combining text, CJK composition and unmarking,
code-point deletion, closed input connection rejection, an actual SurfaceHolder detach/recreate with later GPU
presentation, and landscape resize preserving model state. It writes a native
screen capture only after those assertions.

Retrieve and **inspect** the frame, rather than treating its existence as proof:

```sh
mkdir -p .amp/in/artifacts
adb exec-out run-as dev.gpui.box.example cat files/native-smoke.png \
  > .amp/in/artifacts/android-native-smoke.png
adb shell getprop ro.build.fingerprint
adb shell getprop ro.product.cpu.abi
adb shell getprop ro.build.version.sdk
adb shell getconf PAGE_SIZE
```

Record those device facts with the commit, instrument output, renderer identity
and inspected screenshot. Do not collect unrelated user/device logs.

The instrumentation directly exercises Android's real `InputConnection`; it
does **not** prove integration with a third-party IME or screen reader. Native
acceptance additionally requires:

1. Launch `adb shell am start -W -n dev.gpui.box.example/.MainActivity`. Tap the
   editable field; use an actual CJK IME to compose/replace/cancel text. Test
   emoji deletion, selection replacement, cursor movement and keyboard hide/show.
   Check the caret/candidate position and that content avoids the keyboard once.
2. Enable TalkBack through device settings with the device owner's approval.
   Explore the button and text input, activate the button, edit text, navigate
   focus, and verify labels/bounds after rotation. Do not automate accessibility
   service permission grants or claim full role/action coverage.
3. Background and foreground during input, during an active drag and immediately
   after lifting a finger. No ghost tap, long press, momentum or stale keyboard
   event may occur. Count and text must survive in-process restoration.
4. Exercise system back with the keyboard visible, with the application's back
   handler enabled and disabled. Test gesture navigation and three-button back.
5. Test portrait/landscape, cutouts, navigation modes, display density changes,
   multi-touch cancellation and a 16-KiB-page device. Verify no stale frame is
   presented during surface destruction. GPU loss remains a separate fault-
   injection acceptance case, not covered by rotating a healthy device.
6. On a disposable fixture instance, let Android reclaim the process and reopen.
   A new fixture is expected; generic platform code cannot restore caller data.

Until the APK build, instrumentation, visual inspection and these native cases
have actually run, report Android support as **experimental and unverified**.

## Separately launch the shared mobile reference app

The same development APK contains two independently selectable Activities. The
original `MainActivity` and `NativeSmoke` instrumentation remain the protocol
fixture. `ReferenceActivity` mounts the actual `gpui_box_mobile_reference`
library, not a copy of its UI. It runs in the `:reference` process because this
Android backend owns one Activity/GPUI application per UI thread.

System back calls the shared app's `request_back`. This fixture intentionally
stays open on root/refusal so the app's notice remains visible; use Home to
leave. It never converts a refused dirty-form navigation into Activity exit.

```sh
bash examples/android/build-apk.sh
export ANDROID_SERIAL='<authorized device serial>'
adb install -r target/android-apk/gpui-box-debug.apk
bash examples/android/launch.sh smoke
bash examples/android/launch.sh reference
```

The reference Activity calls `prepare_background` before stopping and stores
its JSON checkpoint in this debug application's private SharedPreferences.
Recreation uses `mount_checkpoint`; malformed JSON, refused routes and storage
failures are errors, never silent fresh-state restoration. Stopping before the
first Surface leaves the previous checkpoint unchanged. This example-host
policy saves fixture data only: it is neither a Kit persistence API nor a
production durability/security policy. Android killing the process without
`onStop` can lose changes since the last checkpoint.

For native acceptance, navigate several reference routes, edit the Chinese
name and multiline draft, background, then terminate only the already
backgrounded fixture with `adb shell am kill dev.gpui.box.example` and relaunch
the reference Activity. Check restored routes/drafts and absence of stale
keyboard, modal, gesture and refresh state. Separately toggle system dark mode
and inspect the reference UI. These are pending native checks, not assertions
proved by the protocol smoke test or Linux host compilation.

To intentionally reset only the reference checkpoint in this disposable debug
app (the `-S` option force-stops both fixture processes before the fresh launch):

```sh
bash examples/android/launch.sh reference -S --ez reset_checkpoint true
```
