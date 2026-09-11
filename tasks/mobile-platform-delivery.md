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

## Combined candidate (not yet accepted or published)

The candidate was assembled in `/home/user/workspace/mobile-integration` and
transferred into the main checkout after the Touch foundation deployment
completed. Both hosted domains verified that foundation's revision, catalog
counts, tool schemas and component identities. The combined candidate below
is not yet published.

- Applied shared input stages 1–3, native pinch terminal cleanup, test inset
  simulation, and native point-query declarations. Both affinity type
  prerequisites came from stage 3 and were not applied twice.
- Applied checked native operations, WGPU lifecycle and explicit destroy
  cleanup; native surface lifecycle remains unverified.
- Applied navigation, Overlay origin correction, sheets/refresh/swipe,
  constrained detents, forms/pickers and their scene registrations. Added
  central prelude exports and checkout MCP viewport schema/documentation.
- Applied native layout/buffer/history implementation and hard-newline
  correction, nearest-stop point geometry, Kit native adapters and bounded
  incident-paragraph caret queries. General range/point queries remain
  document-wide, and missing intra-ligature caret data remains unavailable.
- Applied Android owned source and platform dispatch integration, plus Web
  owned changes. Android is experimental source, not an accepted native target;
  SDK/NDK licensing and execution remain pending. The iOS adapter, explicit
  font initializer and completed-frame diagnostic are integrated separately
  from the direct Metal fixture; neither has native execution evidence yet.
- Parent checks: combined Kit all-feature check passed before the text/backend
  additions; 72 editable-text tests passed before the hard-newline correction;
  MCP tool schema test passes with optional viewport dimensions and unchanged
  remote/local tool counts. These are checkpoints, not a final combined gate.
- Parent inspected actual narrow masked inputs: labels, masked password and
  six-character OTP fit. Picker bottom anchoring and width fit, but last-row
  clipping required an actual scroll/selection check. Owner reports item 12
  becomes fully visible and selectable; parent inspected the scrolled frame
  and confirmed the final row and close control are fully visible.
- Parent found Sunday Touch cells can overflow the 361px date exhibit card.
  Corrected minimum 352px/full-bleed layout preserves every 48px day target;
  parent inspected the corrected 361px frame. Date placeholders now paint and
  fixtures carry explicit accessible names after the combined audit found one
  unnamed actionable date field. The original clipped frame is not acceptance.
- Foundation Platforms run 34609208735 has failing native lanes: macOS app-host
  tests hardcode Ctrl-Z instead of platform Undo; JS binding coverage omits
  Overlay::window_snap_margin; Windows unsandboxed handle probe gets access
  denied. These do not establish a Touch regression, but the run is not green.
  Regenerating the binding catalog for the new APIs also resolves that existing
  metadata drift. No native failure was suppressed or converted to success.

- Parent core all-feature checkpoint passed 602 tests. Initial combined Kit run
  passed 1019 integration tests and failed the date-name audit; it was not green.
  The first iOS host suite failed Linux linkage of the native example; corrected
  non-iOS entrypoint then passed Android 5/iOS 3 tests. Later expanded tests are
  included in the final gate rather than inferred from those earlier results.
- The initial full visual check found 24 new frames and two textarea focus
  differences. A mount-scoped scene focus correction passed a real touch/field
  switching regression and restored both textarea baselines. The scoped media
  check found six expected frames; date-time and textarea matched. Parent
  inspected all 30 new/changed frames and recaptured them; all 30 captured images
  were byte-identical to the inspected candidates.
- `gate full`, `web smoke` and `web gate` now execute the 13-test mobile browser
  suite, with its emulation limits intact. A dispatch-only iOS workflow retains
  separate protocol and GPUI-frame metadata/logs/pixels; it is not yet executed.

Final full gate and native execution remain outstanding. The shared reference
application and separate Android/iOS launch/checkpoint paths have been exported
for the next integration stage; they are not yet part of this candidate.

## Reference integration and final gate

The combined tree now includes the shared reference application and its separate
Android Activity and iOS app. Default reference-library features do not select a
desktop platform. Native example hosts own private version-1 JSON checkpoints;
Kit owns neither persistence nor platform back policy. The iOS dispatch-only
workflow includes separate protocol, GPUI smoke and reference evidence paths.

- The first combined full gate passed workspace checks/tests, strict Clippy,
  performance, wasm32 and rustdoc, then matched all 360 Linux images. It failed
  the final browser suite (7 passed, 6 failed), so it was not a passing gate.
- Five browser failures assumed old scene rows were mounted at the top of the
  virtualized catalog. Tests now scroll to the same named targets before
  exercising touch. First-size readiness is explicit, while exact CSS/backing
  dimension assertions remain unchanged. The integrated browser rerun passed
  all 13 tests in 4.7 minutes.
- Reference review found its ImageViewer ignored fit requests and used desktop
  controls. The fixture now accepts fit requests in transient caller state.
  Its new measured test then caught 46px Touch segments: the shared segmented
  control now preserves full 48px targets without enlarging desktop controls.
  A focused Kit integration regression passes for both sizes.
- Parent ran the reference capture program: 12 actual 390×844/390×520 logical
  window captures, with action/refusal/restoration checks, accepted fit changes
  and measured Touch bounds. Updated image and open-sheet frames were inspected.
  Both changed Linux ImageViewer exhibit frames were inspected before acceptance.
  The next full comparison matched 358 frames and identified the same Touch
  correction in AudioPlayer's rate segments. Both remaining frames were inspected
  and accepted; another full gate is required, not a claimed prior success.
- The portable iOS reference runner suite passes 6 tests, and workflow YAML and
  shell syntax were checked. These do not execute UIKit, Android, native
  persistence, IME, accessibility, predictive back or gesture timing.
- Parent iOS reference filesystem/version tests pass 3/3, and the default
  reference library passes wasm32 checking with warnings denied.
- Reference back ordering now closes its sheet/picker before routing. The
  public host entry is `request_back(window, cx)` and the Android host invokes
  it through the mounted window. The capture runner checks this policy directly
  without claiming native OS event delivery. Latest Android host-check, reference
  tests, relevant all-target/all-feature Clippy, reference wasm32/rustdoc and
  iOS checkpoint tests pass after this change.
- Native affinity adapters cover TextInput/TextArea and their masked/OTP
  wrappers. RichTextEditor's separate rich-document handler remains legacy and
  is refused by the Android selection bridge; compatibility records name this
  source gap explicitly instead of implying that all editors are native-ready.

The final merged `CARGO_INCREMENTAL=0 cargo run -p xtask -- gate full` passed:
360 Linux images match (541.00 seconds), the reference capture runner passes
all action checks and writes 12 frames, and all 13 mobile browser tests pass
(4.4 minutes). Workspace default/all-feature tests, strict workspace Clippy,
performance, wasm32, rustdoc and generated catalogs also pass. The latest
reference back-policy change additionally passed the focused checks above.
The complete log is `target/mobile-final-gate-full-accepted.log`.

Publication and native execution remain outstanding at this checkpoint.
Android SDK license acceptance and a native device/executor are not implied by
the source implementation or the browser/offscreen evidence above.

## Verification after integrating the published glass work

The mobile candidate was rebased over published glass source, native baselines,
and catalog corrections at `cc012a00f74a1fff05a8f19d8429ab324ccdb143`.
`CARGO_INCREMENTAL=0 cargo run -p xtask -- gate full` passed again:
362 Linux images match (544.31 seconds), all reference action checks and 12
captures pass, and all 13 mobile browser tests pass (4.4 minutes). Workspace
default/all-feature tests, strict Clippy, performance, wasm32, rustdoc and
generated catalogs pass. The log is `target/mobile-rebased-gate-full.log`.
The reference image is byte-identical to the inspected reference-final artifact.

Two app-host history tests now send Cmd-Z on macOS and Ctrl-Z elsewhere, matching
the component keymaps without changing their expected history assertions. This
gate verifies Linux only; the corrected macOS tests still require native execution.

Publication remains paused for the glass owner's final test-only platform cfg
commit and deployment/macOS verification. Earlier Metal/WARP and Windows native
successes belong to the glass revision, not to this integrated mobile candidate.
Windows runtime AppContainer error 5 remains unresolved. No Android licenses,
native mobile IME/accessibility, or native checkpoint roundtrip evidence is claimed.
