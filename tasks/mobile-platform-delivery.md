# Mobile and full-platform delivery

Coordinator: https://ampcode.com/threads/T-01a090a2-5dd6-7055-ac27-2e106aa37cb4

The user authorized implementation of the full-platform plan, parallel threads,
environment preparation, and native verification. Assignment is not delivery.
This ledger is separate from the earlier full-platform runtime/editor work.

## Ownership

| Stream | Owner | Write boundary |
| --- | --- | --- |
| Shared input and gestures | https://ampcode.com/threads/T-01a090ad-37b1-71a9-99f9-8a8e0f8585f4 | GPUI input contracts, gestures and narrow window dispatch |
| Mobile Web | https://ampcode.com/threads/T-01a090ad-7ba5-7244-97f4-bfb2db0d3d1d | gpui_web and browser-gallery validation |
| iOS | https://ampcode.com/threads/T-01a090ad-ca7c-76ad-bd99-1c1af0b0c30a | gpui_ios and iOS build/example resources |
| Android | https://ampcode.com/threads/T-01a090ae-1bc8-70ed-ac7d-081b58446c01 | gpui_android and Android build/example resources |
| Page structure and navigation | https://ampcode.com/threads/T-01a090ae-6b16-76cd-a31d-ac48a2e77ef0 | Kit layout/navigation and their exhibits |
| Sheets and list gestures | https://ampcode.com/threads/T-01a090af-16bc-73e8-bd9f-64f64a2b5231 | Kit overlay/interaction and their exhibits |
| Forms and pickers | https://ampcode.com/threads/T-01a090af-63a6-70ff-83a8-1ea29a4c40c0 | Kit controls/datetime and their exhibits |
| Mobile render surfaces and target support | https://ampcode.com/threads/T-01a090b1-4cef-73dd-9ac9-a048ea61e931 | gpui_wgpu and coordinated non-input core cfg/capabilities |
| Image viewer and media interaction | https://ampcode.com/threads/T-01a090bd-aa25-766d-b2dd-cb8c33df23ab | Existing content/media components and their exhibits |
| Native editable text geometry | https://ampcode.com/threads/T-01a090bf-b393-7000-8b98-3f9833a30719 | Core shaped/editable layout; input owner integrates trait declarations |

Workers develop in isolated checkouts and send locally committed changes to the
coordinator. The coordinator owns combined manifests, package authority,
generated catalogs, token reconciliation, provenance/compatibility integration,
the reference application, downstream packaging integration, and publication. Source
and commits must be transferred explicitly; messages alone do not transfer code.

## Implementation milestones

1. Establish explicit platform capabilities and a reference application covering
   bottom navigation, list/detail history, long lists, refresh, swipe actions,
   Chinese input, long forms, pickers, sheets with keyboard, image viewing,
   failures, refused navigation and background restoration.
2. Wire actual platform lifecycle, render surfaces, touch, IME, insets and
   accessibility. Shared tests do not establish native platform delivery.
3. Deliver page bars, bottom navigation, safe-area/keyboard layout, cancellable
   navigation and sheets with nested scrolling and explicit detents.
4. Adapt existing controls rather than duplicate mobile controls. Deliver
   refresh/swipe behavior, adaptive selection, feedback and media interactions.
5. Deliver system capability boundaries and reproducible development packages,
   templates and installation procedures. Signing identities remain external;
   never commit credentials or imply unsigned builds are store releases.
6. Establish platform-bound native execution, visual review, accessibility and
   performance evidence for the integrated revision, not only worker revisions.

## Acceptance invariants

- Caller-owned data, history, requests and permission decisions remain outside
  Kit. Components emit intents, including cancellation/refusal, not fake success.
- Pointer ownership is stable across movement, extra contacts and cancellation.
  A claimed drag cannot also tap or fling. Multi-touch must not produce a phantom
  click when one finger remains. Directional arbitration must preserve existing
  handler lifecycle contracts.
- Sheet drag, row swipe and nested scrolling compete through framework input
  authority, not separate component recognizers.
- Safe-area and keyboard data come from the platform; floating keyboard
  occlusion must not masquerade as a bottom edge inset.
- Browser history, OS back, open modal and focused input have an explicit host
  ordering. A refused/cancelled back action preserves accepted application state.
- Disabled controls install no action handlers. Loading, empty, unavailable,
  error and retained verified data remain distinct.
- Every public component has real family exhibits and stable semantic targets.
  Inspect affected rendered states before accepting visual baselines.
- Device accessibility, composition, keyboard avoidance, lifecycle and native
  system operations require executed checks, not screenshots alone.

## Validation and environment evidence

- Initial coordinator checkout is clean on main at tracked origin/main.
- Linux orb has native Linux and wasm32 Rust targets. It has no detected adb
  or xcodebuild. Android SDK/NDK provisioning is being investigated by its owner.
- Android SDK license acceptance is pending user approval or provision of an
  already licensed SDK. iOS needs a macOS Xcode executor; physical-device
  installation additionally needs the user's development signing setup.
- Existing Platforms workflow provides macOS and Windows native lanes, not
  iOS or Android coverage. Native mobile validation remains unestablished.
- Live runners reported studio-win and xiaomao-box. No runner was selected:
  the user must identify which machine to use before accessing it. Their names
  and working directories do not prove SDK, emulator or physical-device access.
- The orb has no KVM. Do not start Android emulators here. Browser touch
  emulation, cross-compilation, native simulators and physical devices are
  separate evidence classes; record each accurately.
- Source gate: Linux gate full plus dependencies check. Native desktop lanes
  and mobile native tests must run against the integrated source. No baseline
  acceptance from an unrelated revision and no claim that a compile is a run.
- Main publication follows the repository's direct-push and hosted deployment
  workflow. No pull requests or store publication are part of this assignment.

## Remaining work

All implementation streams are assigned, not accepted. The reference application,
downstream packaging integration, device access and integrated
native validation remain coordinator work. Record stage commits, commands,
decisive outputs, inspected artifacts and gaps here as results arrive.

## First foundation checkpoint (Linux verified; native acceptance pending)

- The coordinator added explicit ControlSize::Touch and required control.touch
  metrics across all ten themes. Compact density does not shrink Touch;
  explicit subtree scaling still scales it. Existing desktop defaults remain.
- `cargo test -p gpui-box-kit-tokens`: 84 passed. `cargo test -p
  gpui-box-kit-theme`: 38 passed, including density/scaling discrimination.
  `tokens generate` and `tokens check` pass. A first exact-name filtered theme
  command selected zero tests; it was replaced by the complete 38-test run.
- Baseline `dependencies check` passed. The first full gate passed workspace
  tests but failed strict Clippy on four pre-existing unwrap calls in the
  field-focus token test. Diagnostic expect calls preserve the assertions.
  Strict workspace all-target/all-feature Clippy then passed, followed by
  `CARGO_INCREMENTAL=0 cargo run -p xtask -- gate full`: 336 Linux images match
  in 471.20 seconds and the gate passed. No baseline images were changed.
  This proves only the frozen touch-token foundation, not worker candidates.
- Shared input stage 1, iOS host fixture stage 1 and WGPU lifecycle stage 1
  have been received for review, not integrated. The iOS fixture is not a
  GPUI Platform implementation. Parent caught invalid workspace homepage
  inheritance in its manifest; the owner is correcting it.
- Parent review found moving pinch centroids could route phases to different
  controls. The input owner is adding stable pinch ownership and cross-boundary
  tests. Start-edge sheet arbitration is only a first stage; same-contact
  residual scroll handoff remains required, with no duplicate consumed motion.
- Token prerequisites were transferred to component owners for isolated checks.
  Worker test and image reports are not integrated or native-device acceptance.
- Parent independently ran iOS fixture portable checks in an isolated review
  directory: six C occlusion assertions, one Rust UTF-16 ABI test and two Python
  simulator-selection tests pass. None executes UIKit, Metal or a simulator.
- Parent inspected the isolated touch-inputs light image: no input clipping or
  overlap; disabled/error states are visible. OTP labeling and reveal affordance
  were sent back for review. True narrow-window capture is being added to serve;
  a narrow child inside a desktop window is not a phone viewport test.
