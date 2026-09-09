# Full platform delivery

This is the execution ledger for the user-authorized performance, component,
document, browser, runtime, and plugin expansion. An assigned stream is not a
completed capability. Integration, tests, platform evidence, and deployment are
tracked separately from implementation.

Coordinator: https://ampcode.com/threads/T-01a0862b-202e-71a0-b491-fbb2fe9d5357

## Ownership

| Stream | Owner thread | Initial state |
| --- | --- | --- |
| Framework scrolling, Flow, MessageList | https://ampcode.com/threads/T-01a0865a-4044-734a-9c17-f5dafceefcbb | Implementation assigned |
| Markdown and AgentDocument | https://ampcode.com/threads/T-01a0865a-4951-714b-ae8b-3a63380c8167 | Implementation assigned |
| Large text and editor | https://ampcode.com/threads/T-01a0865a-51b7-7360-ad3f-eae8b6b87fe1 | Implementation assigned |
| Two-axis DataGrid and performance checks | https://ampcode.com/threads/T-01a0865a-59cd-74ac-b288-cf4a2c85d6f1 | Implementation assigned |
| Navigation, attachments, Dock, charts, settings, localization, assets | https://ampcode.com/threads/T-01a0865a-ee10-75da-81ff-beb1052435f2 | Implementation assigned |
| Cross-platform WebView | https://ampcode.com/threads/T-01a0865a-f730-713e-9dcc-9dd492532c3e | Implementation assigned |
| JS/TS runtime, application host, plugins | https://ampcode.com/threads/T-01a0865a-fe20-726a-918a-b1d6e9c4c744 | Implementation assigned |
| Renderer timestamp evidence | https://ampcode.com/threads/T-01a08675-02d3-73df-bddf-2d47f1980e19 | Linux delivery integrated; native evidence pending |
| Retained native JS Kit bindings | https://ampcode.com/threads/T-01a08696-7195-7158-9960-138cdbcc3e2a | Seven-control stage in progress; full catalog incomplete |

Workers use isolated checkouts and local staged commits. The coordinator
transfers commits/files, resolves shared manifests and generated catalogs,
validates the combined tree, and pushes directly to main with the required
hosted deployment. Existing source changes are preserved. Shared source edits
must be reconciled, not overwritten with another checkout's final file.

## Architecture contracts

- Generic scrolling, text storage/layout, clipping, hit testing, input and
  platform views belong to the local GPUI framework authority.
- Kit components read caller data and emit caller actions. Document revisions
  invalidate measurements without changing stable semantic/business IDs.
- Application runtime, transport, filesystem, process, credentials and plugin
  policy live in separate host packages depending on Kit, never vice versa.
- Browser engines supply browser-grade HTML/CSS. Native rich text has an
  explicit supported subset; no browser compatibility is inferred from it.
- A permission whitelist alone is not an untrusted-code sandbox. Security
  claims require enforced resource and process/OS boundaries and negative tests.

## Acceptance matrix

### Reading, scrolling and streaming

- Reading history while tokens arrive preserves the reading anchor and does
  not resume tail follow. User navigation to the tail can resume following.
- Prepending history is distinct from appending new messages and updating an
  existing message; unread counts and follow decisions reflect that distinction.
- Identity survives insertion/reorder; anchor removal has a documented fallback.
- Content revisions, asynchronous images, expanded code and width/font changes
  invalidate affected heights without resetting the whole conversation.
- Nested code/table scroll containers consume the appropriate axes and hand
  off unconsumed scrolling consistently at boundaries.
- Static scrolling does not repeatedly parse/replan the whole document.
- Streaming fences, lists, references and replacements preserve parser
  correctness; stale asynchronous parse results cannot replace newer content.
- Selection, copy, search navigation and semantic targeting remain consistent
  across virtualized blocks. Unsupported cross-unmounted selection is not
  silently described as complete.

### Large text and grids

- Local editing does not require copying or shaping the entire document.
  Unicode, grapheme, UTF-16/IME, undo/redo and selection invariants are tested.
- Editor capabilities are integrated interactions, not service traits alone:
  multiple selections, rectangular selection, folding, syntax and language
  service results must be exercised with revision/cancellation handling.
- Grid cell building is bounded by visible rows and columns plus pinned and
  explicit overdraw regions; eager input preparation remains separately counted.
- Horizontal scrolling, RTL, resizing, pinned regions, editing, keyboard focus,
  selection and accessibility share one column geometry.
- Performance fixtures include wide grids, streaming documents and large-text
  edits, not only narrow grids and warmed visible-row counts.

### Components and resources

- NavStack includes history, transition lifecycle and focus behavior.
- Attachments expose media/content/action slots and truthful processing states.
- Dock supports floating tiles and validated caller-owned restoration contracts.
- Sankey accepts caller data with a tested layout algorithm, not only normalized
  precomputed geometry. Settings, locale packs and icon selection are integrated.
- Every new public component has real state exhibits, semantic targets and
  generated API documentation. Images are inspected before baseline acceptance.

### Browser, runtime and plugins

- WebView examples load actual content and exercise navigation, focus, IPC,
  lifecycle, failure, popup/download/permission handling and host integration.
- Platform clipping, stacking and capture restrictions are explicit and tested
  independently; unsupported platforms do not return fake successful operations.
- JS views render and dispatch real events; state, module/async lifecycle,
  hot reload, cancellation, errors and typed bindings work end to end.
- Plugin discovery, contribution registration, permission refusal, enable/disable,
  dependency/version validation, installation/update/rollback and cleanup have
  executable tests, including malicious paths/manifests and crashed workers.
- CLI/templates and application packaging demonstrate an executable application.
  External signing credentials and release authorization are recorded separately.

## Integrated validation and release

For each integrated stage, record commits, actual commands, decisive results,
reviewed artifacts, remaining gaps and platform coverage. Run the repository's
full Linux gate before shipping. Framework/platform work also needs dependency
authority and provenance checks plus relevant macOS/Windows validation. Timing
claims require matched workloads and measured evidence; structural budgets are
not GPU or presentation-latency measurements.

After every main push, deploy with `tools/site/deploy-main.sh` and verify the
hosted revision and catalog. No PRs and no ordinary crates.io MCP publication.

## Evidence log

- Initial coordinator checkout was clean and six commits behind origin/main;
  it was fast-forwarded before integration. No unpushed source was distributed.
- Seven high-mode implementation threads launched. No stream is yet accepted
  as complete by this ledger.
- Baseline `cargo run -p xtask -- dependencies check` passed: package
  identities, dependency graphs, compatibility and provenance records agree.
  Baseline `cargo run -p xtask -- gate full` passed, including 316 matching
  Linux headless images. This certifies the pre-integration baseline, not any
  of the independently implemented streams.
- Scroll/document owners agreed on stable row keys plus separate
  `revisions(Vec<u64>)`, targeted remeasurement, and stable-ID anchor remapping.
  This is an implementation contract, not a verified delivered API yet.
- The grid owner found eager cell construction before filtering and is adding
  a lazy keyed provider while preserving the eager API's compatibility.
- The editor owner is migrating to persistent Ropey storage and indexed
  snapshots, including an exact-byte undo regression for combining marks.
- The runtime owner selected supervised Node/V8 processes. Runtime version,
  application packaging, complete binding coverage, and malicious-plugin OS
  isolation remain acceptance requirements; Node permissions alone do not
  satisfy the sandbox contract.
- WebView inspection found no active Linux framework platform-view attachment.
  Wry Xlib child integration requires GTK/GLib event pumping; direct embedding
  in GPUI Wayland is not established. Linux implementation and platform-specific
  constraints remain open, not accepted as a working cross-platform host.

### First combined source checkpoint

- Integrated both scrolling stages, two-axis grid and wide performance fixtures,
  two retained-document stages, persistent/no-wrap editor plus optional syntax,
  Sankey including the subnormal-scale fix, NavStack, AttachmentTile, three
  WebView stages, and renderer timestamp instrumentation/comparison.
- Shared generated catalogs were regenerated from the combined source. Provenance
  and compatibility append conflicts retain every stream's records. Borrowed
  prerequisite commits were excluded from worker deliveries.
- The first combined gate stopped on disk exhaustion; after generated-cache
  cleanup, its retry passed workspace tests but found two framework regression
  test `unwrap()` lint failures missed by the worker's Kit-only Clippy. These
  assertions were corrected; framework all-target/all-feature Clippy passed.
- Renderer comparator tests are now part of the root gate: five tests pass,
  including rejection of a uniform slowdown in both workloads. No measured
  speedup is claimed. Worker same-revision software Vulkan measurements are
  controls, not hardware GPU budgets or Metal/WARP evidence.
- Combined API check and two setup runs pass. The first combined full run then
  caught six missing syntax diagnostic/panic allowlist entries. After reviewing
  and regenerating them, the retry passed `CARGO_INCREMENTAL=0 cargo run -p xtask
  -- gate full`: 322 images match, 422.94 seconds for the visual comparison.
  Source remained frozen throughout this successful run, including editor wheel
  browsing and the SettingsList page-chrome stage.
- Combined native X11 WebView smoke passed CSS/viewport, IPC/script, HTTP
  navigation/history/reload, policy refusal and a real connection-refused error.
  NavStack push, attachment states and settings default/no-match/reset were
  exercised in the combined headless host; audits returned no findings and
  screenshots were inspected. No baselines were accepted by the coordinator.
- Still open: soft-wrap/event/accessibility costs,
  multicursor/folding/services; background document coalescing/stale rejection;
  remaining component/resource scope; complete runtime bindings, native OS
  sandbox parity, packaging and native browser/renderer execution. Further
  cross-stream document/editor performance fixtures must use their real APIs.
- The Windows sandbox candidate has a separate implementation owner:
  https://ampcode.com/threads/T-01a086b0-d0e2-7491-b7bb-70b73c3825dc.
  The framework scrolling owner now owns inherited effect/clipboard context;
  native clipboard gestures must not silently bypass an owner's capability
  denial. Existing verified CopyButton success still requires readback.

### Second combined source checkpoint

- Integrated global accessibility text indexing, background Markdown parsing,
  retained segmented document plans, cross-stream performance fixtures,
  selective icon resources, floating Dock stacks, and the Settings overlap fix.
- The exact combined source passed `CARGO_INCREMENTAL=0 cargo run -p xtask --
  gate full`: 324 Linux images match in 443.91 seconds; workspace tests,
  Clippy, generated catalogs, wasm32 compilation, rustdoc and all 49 performance
  reports passed. The eager 10,000-item negative detector still refuses its
  unbounded fixture; no performance budgets were relaxed.
- At both 1,000 and 10,000 history blocks, Markdown static redraw constructs
  zero new plans, a new 21-byte message constructs one, and the next 21-byte
  tail append constructs two. Caller metadata iteration and full accessibility
  publication still scale with document size: these are not bounded-total-work
  claims. Retained accessibility publication remains active editor work.
- Settings had a real overlap despite the first checkpoint's inspected image:
  a 240px Sidebar was allocated only 160px. The corrected exhibit allocates
  its intrinsic width; Sidebar spans x16..256 and content starts x272.
  Default and dark no-match images were inspected again with no overlap.
  The floating Dock image was also inspected; it deliberately shows one
  floating inspector over the main workspace, not detached OS windows.
- First native Platforms run 34370021007 passed native macOS but failed image
  comparisons and the Windows UIA menu watchdog. Old Settings Metal/WARP
  images must not be accepted. Windows baseline drift is under independent
  artifact/history review. A separate UIA capture fix preserves timeout
  diagnostics; it does not establish that native menu behavior is fixed.
- The first hosted deployment failed before activation because publication
  depended on stale macOS images. Publication now selects the Linux daily
  authority consistently for copying and fingerprinting, publishes explicit
  image-source metadata, and rejects missing scene/theme images. Browser
  inspection passed in both site themes: the NavStack live surface and static
  preview fit, and both Linux-derived images loaded. Native Chromium 2x was
  required because CDP-only DPR emulation reported contradictory device-pixel
  sizes; CSS width 920, backing width 1840 and DPR 2 then agreed. Successful
  deployment remains separate release evidence.
- Effect-owner, clipboard, multicursor and runtime candidates remain outside
  this frozen checkpoint. Full native JS bindings, OS sandbox execution and
  the runtime owner's live-window black client area remain unresolved.

### Third published checkpoint and fourth-stage integration

- Published and deployed source `f0f1a1ee1d965587de681f659bc24a5f0c7d2d39`
  passed the combined Linux full gate: 330 images across 165 scenes. Hosted
  verification matched 32 packages, 8,126 symbols, 200 components and 10 tools.
- The fourth stage integrates DnD, native-menu session lifetimes, incremental
  editor accessibility, platform sandbox/WebView candidates, and the frozen
  51-component partial JS binding catalog. Native references, typed/deferred
  construction, full family coverage and safe untrusted Cinematic decoding
  are not complete. A production dotLottie decoder exists; its missing
  pre-parse and per-frame work/cancellation budgets prevent untrusted use.
- The merged JavaScript suite passed 86 tests with 15 explicit platform skips.
  Generated binding coverage was then refreshed and its check passed. The
  first combined full gate passed both workspace test modes but rejected 23
  new native-menu test `unwrap()` calls in workspace Clippy. They now use
  diagnostic `expect()` calls without removing assertions or relaxing lints;
  the repeated strict workspace Clippy passed. The frozen full-gate retry
  passed, including 332 matching Linux images in 443.52 seconds, wasm32,
  rustdoc and all unchanged performance ratchets. The real isolated-worker
  Host click also reported `Move right refused by fixture host; sets unchanged`;
  its inspected image retained the source/target sets and disabled controls.
- Native image acceptance is exclusively from Platforms run 34385663795 at
  the published source above: 24 Metal and 181 WARP changed/new frames. The
  renderer owner inspected all 205 candidates; the coordinator additionally
  inspected Settings pages on both platforms, Metal editor-services and the
  Windows grid. The acceptance script copied exactly these 205 paths, and
  every SHA256 matched the audit manifest. Old rejected Settings frames were
  not reused. This does not validate subsequent DnD or native-menu changes.
- That native run passed macOS workspace tests; Windows workspace tests also
  passed, then WebView authored-HTML navigation failed before UIA could run.
  The imported navigation fix still needs native execution. Windows sandbox
  probes failed at CreateProcessW with error 2 before payload execution;
  per-instance profile provisioning and detached-process launch are candidate
  fixes, not a proven containment result. The independent session teardown
  fix addresses the replacement worker's leaked cwd and bounds shutdown.
- Runtime received a validation-only parent bundle containing the 51 partial
  bindings and both Windows/session candidates. Its full reference, resource,
  typed-slot and lifecycle integration must be tested on the resulting merged
  tree. No fourth-stage deployment or cross-platform success is claimed here.

### Folding and native-menu verification checkpoint

- Integrated revision-tagged nested editor folding and its source-row
  projection, then fixed the API generator to discover declarations before
  attaching cross-file implementations. The regenerated index includes
  `Editor::set_folds` and `set_service_result`; missing signatures are no
  longer hidden by source-file ordering.
- The new combined `CARGO_INCREMENTAL=0 cargo run -p xtask -- gate full`
  passed: 334 matching Linux images in 447.26 seconds, workspace tests,
  strict Clippy, generated checks, wasm32, rustdoc and performance ratchets.
  The combined runtime/plugin/app-host JavaScript check passed 86 tests with
  15 platform skips and current catalog metadata. Both folding themes were
  inspected; the exhibit intentionally has one editor, not a second preview.
- Both native lanes now execute the real native-menu tracking smoke even if
  a preceding browser/accessibility check fails (unless the job is cancelled).
  It must exit successfully and emit its complete PASS marker. Its OS-thread
  watchdog distinguishes a hung modal loop from successful cancellation;
  simulated menus do not report real native tracking. Native execution is
  still required before claiming platform lifecycle support.
- Fresh 49-case measurements on the earlier 51-adapter validation checkpoint
  retain significant source-sized costs: Editor static-frame requested bytes
  grow from 827,047 at 1,000 lines to 4,401,384 at 10,000 lines. Bounded
  accessibility leaf publication is not bounded total traversal or allocation.
  This measurement does not include folding or soft-wrap.
- AccessKit 0.24.1 additionally deep-copies whole-node properties and performs
  full-text comparisons/conversions in native adapters. TextRun-backed
  selection alone does not require a parent Value, but removing that Value
  with stock AccessKit loses macOS AXValue and Windows ValuePattern behavior.
  An Arc-only change would not remove adapter work or text-edit materialization.
  No local AccessKit authority/fork is approved by this checkpoint; accessible
  content and platform behavior must remain intact during independent GPUI
  cache and soft-wrap optimization.

### Sixth-stage integration (not published)

- Central native registration now consumes each family's component list and
  routes rendering, retained-state reconciliation, cross-field validation and
  data dispatch. Typed ButtonGroup/SettingsSection/SettingsList children are
  built before type erasure, retaining their owner and mixed-content ordering.
  Shared Menu/Focus references use central argument/result validation and
  instance-aware native anchors; Drawer focus stops are revalidated on render.
- Before the latest InlineEdit/editor/platform imports, the combined host
  suite passed 135 tests; the explicitly executed real isolated-worker resource
  registration/reload test also passed. The JS suite passed 178 tests with 15
  OS skips, including strict declaration checking without skipLibCheck.
  An actual native Settings click reported `Nested row action retained`; the
  no-match state retained its page footer. Both resulting images were inspected.
  These results are not evidence for the later imports or the whole full gate.
- The latest local imports add InlineEdit (185 registered partial constructors,
  15 still unbound), retained editor options, complete static native Value
  retention and row/representability caches. Worker 49-case measurements reduce
  10,000-line static requested bytes to 1,566,344 for TextArea and 1,756,914 for
  Editor, but total work still scales with source size. Soft-wrap is still an
  independently owned follow-up; no AccessKit fork or missing native Value.
- Source-bound Platforms run 34396298822 at published source
  `02468534f3863e1f627559808f55128eb8bc8c4d` rendered 334 Metal and 334 WARP
  frames. The independent reviewer inspected all 17 changed/new candidates;
  acceptance copied exactly 4 Metal and 13 WARP files and every SHA256 matched
  the receipt. The nine existing WARP changes total 54 pixels over tolerance,
  maximum two channel steps; tolerance remains unchanged. This does not prove
  interaction, timing, sandbox, menu or later source correctness.
- That run passed the full macOS WebView smoke. Windows WebView reached fixture
  IPC and navigation refusals, then failed at its 200x200 controller viewport.
  Both actual native-menu smokes timed out. macOS runtime failed only its old
  socket-allocation network probe; Windows payload launch failed with error203.
  Imported candidates add WebView2 full physical allocation, NSRunLoop menu
  scheduling/Win32 notifications, live-endpoint network probes and private
  AppContainer bootstrap paths. All require a new real native lane.
- Windows debug-helper packaging is integrated with relocation and missing-file
  refusal tests, not Windows ACL/execution proof. Live native deferred-drop to
  worker predicate resolution/cancellation, packaged asset activation and
  Windows evaluator cancellation remain runtime-owner work. No unused-field
  lint suppression or fake deferred controller is used to declare completion.
  The unused `deferred: None` construction field was removed from this partial
  stage; the actual controller and its consumers must be delivered together.
- The expanded full gate passed workspace check, then ran out of orb disk
  during workspace test compilation. Generated incremental caches were cleared;
  this failed run is not a successful gate and must be repeated.
- The retry found three visible English strings outside StringKey and two Debug
  names outside the allowlist. The visible Avatar/JSON refusal messages now use
  English and Chinese translation keys; only the Debug names were allowlisted.
  A real accessibility regression also showed EmptyState omitted its rendered
  detail from the native tree. It now publishes that detail as description.
  Focused native tests verify translated JSON title/description and the revoked
  Avatar's translated accessible label; both pass. Expanded JS is 181 passing
  tests plus 15 OS skips, with 185 constructors and current method authority.
- The next full run reached rustdoc and rejected an unquoted `Entity<Menu>` in
  the shared helper documentation. Quoting the type fixed the warning without
  a lint exemption. The subsequent combined `CARGO_INCREMENTAL=0 cargo run -p
  xtask -- gate full` passed, including 336 matching Linux images (426.36 seconds
  rendering/comparison), both workspace test modes, strict Clippy, wasm32,
  rustdoc, dependency checks, generated catalogs and performance ratchets.
  The all-feature framework suite passed 574 tests; app-host passed 136 with
  four explicit ignores. Later soft-wrap and KeybindingRecorder deliveries
  are not part of this source or this result. Native candidates still require
  actual macOS/Windows execution; no publication is implied by this local gate.

### Sixth-stage publication and seventh-stage verification

- The sixth stage was pushed and deployed as
  `8939172a6c3ae1c18f0cb22c8989d14d280413f3`. Both hosted domains verified
  that revision, 32 packages, 8,332 symbols, 200 components, 168 scenes and
  all ten MCP tools with matching schemas and structured component identities.
- Source-bound Platforms run 34406540756 passed actual browser smoke on both
  native platforms: a 900x650 DOM CSS viewport, IPC/evaluation, navigation,
  history/reload, prohibited-navigation refusal and offline error. Those are
  not physical-pixel, DPI, clipping, IME or accessibility proofs. Both actual
  OS menu tracking smokes passed cancellation, replacement, stale-owner,
  teardown and surviving/new-owner phases. Windows native UIA still timed out
  separately; the last log precedes both target lookup and pattern acquisition,
  so it does not establish a provider wait cycle or an Invoke failure.
- macOS native sandbox probes passed, but four resource-transport fixtures
  hardcoded Linux and failed that runtime job. Windows now executes real
  payloads but its runtime job still failed six probes. The next owned Windows
  candidate tightens profile paths and corrects launch/probe handling without
  relaxing access-denied assertions; it needs another actual native run.
- That run rendered 336 Metal and 336 WARP images. Ten audited candidates
  comprise two Metal editor-options frames and eight WARP frames; six existing
  WARP differences total 49 pixels over tolerance, at most two channel steps.
  The four new editor-options frames retain known default-font emoji tofu.
  Acceptance uses only this run's hash-bound actuals; tolerance is unchanged.
- Seventh-stage code integrates exact incremental paragraph wrapping, same-
  prepaint native row topology, release-time List/Tabs/Tree worker predicates,
  KeybindingRecorder, PasswordInput and OneTimeCodeInput, for 188 partial
  adapters. SearchField/FindReplace and their shared reference follow-up are
  later deliveries, not part of this result. Packaged asset activation and
  Windows evaluator cancellation remain open.
- `CARGO_INCREMENTAL=0 cargo run -p xtask -- gate full` passed with 336 matching
  Linux images in 428.59 seconds, 576 all-feature framework tests, 138 host
  tests with five explicit ignores, strict Clippy, wasm32, rustdoc, generated
  checks and unchanged performance budgets. The combined JS suite passed
  186 tests with 15 OS skips. Explicit isolated native pointer E2E passed all
  nine accept/refuse/timeout/revision/removal scenarios; accepted and revoked
  captures were inspected. Actual recorder click reported `Native recording
  started`; recording, masked-input and actual reveal captures were inspected.
- Worker unchanged 63-phase measurements show wrapped 10k edit plus settle
  requested bytes fall from 108,499,972 to 12,104,718. Cold plus settle falls
  from 171,817,986 to 126,832,351, but the first cold draw increases. Warm
  static bytes remain 1,841,017; metadata indexes still scale with source.
  Full native Value/AXValue and folded source remain; total work is not bounded.

### Eighth-stage packaged resources and retained input verification

- Seventh-stage source `8c7c58bf8de41b52403d8aeda22baa9b4aab7f3d` was pushed
  and deployed; both domains verified 8,355 symbols and the same complete
  component/scene/tool identities. The next stage registers SearchField,
  FindReplace, Combobox, MultiSelect, TagInput and Cascader, including actual
  nested reference contracts, for 194 partial adapters.
- CLI-built asset receipts activate only after mounted permission consent and
  native registration acknowledgements. Explicit packaged-runner/isolated-worker
  E2E passed: deleting the raw asset after build still renders receipt pixels;
  deny registers nothing; revoke removes pixels; retry registers again; corrupt
  receipt reload retains the verified old image; successful reload requires new
  consent. Ready and revoked captures were inspected. The packaged host payload
  in this test is the test executable, not a signed installer or native OS
  package-execution claim. Resource fixtures now select the actual native
  sandbox backend instead of hardcoding Linux.
- Actual native button checks passed the three retained editor draft queries,
  FindReplace → SearchField → TextInput query, and Cascader open/query. Their
  affected rendered states were inspected, including the localized MultiSelect
  no-match popup and all six Cascader branch rows. The fixed-size search capture
  cuts off the bottom host plugin hint; affected controls remain readable.
- UIA smoke searches now start at the activated target HWND rather than desktop
  descendants, with distinct lookup/pattern/Invoke/dismissal diagnostics. This
  is a client correction, not proof of a provider deadlock fix. Real Windows
  execution remains required; action assertions and timeout are unchanged.
- Combined `CARGO_INCREMENTAL=0 cargo run -p xtask -- gate full` passed with
  336 matching Linux images in 405.16 seconds, 576 all-feature framework tests,
  143 host tests and six explicit ignores, strict Clippy, wasm32, rustdoc,
  generated checks and performance budgets. Combined JS passed 192 with
  15 OS skips; packaged native E2E was explicitly run and passed separately.
  Later debug cancellation and bare DragItem deliveries are not included in
  this result. External dropped-file/pasted-image authority remains refused,
  not represented by fake readable references.

### Complete constructor catalog and remaining native acceptance

- Ninth-stage source `101cffd3134a0a8d41b9b5445897dfe6567b9f47` was pushed and
  deployed with matching hosted catalogs. Its frozen full gate passed 336 Linux
  images (429.69 seconds), 576 all-feature framework tests, 147 host tests and
  199 JS tests with 15 OS skips. Actual Editor → TextArea → FocusHandle and
  MentionInput child-reference clicks and inspected renders passed.
- Combined feature validation caught optional syntax event variants absent
  from single-package checks. App-host now explicitly compiles existing Kit
  syntax support and ignores the two parser events outside its closed schema;
  no grammar is installed or loading authority granted. Native harness modules
  consistently require capture, and still run in all-feature workspace tests.
  TextArea focus and rich session methods name their exact native authorities
  in the generated binding catalog rather than inventing component methods.
- All 200 catalog components now have declared adapters, including all 34
  controls. Status remains partial or fallback-only where appropriate; this
  is not full method parity, untrusted Cinematic playback, grammar loading,
  external dropped-file/image access, or bounded total native text work.
  UploadList constructs its typed Dropzone with the child's effect scope and
  parent disabled guard. RichTextEditor uses a real session, complete styled
  documents and a non-recycling ID allocator; exposed session references have
  six read-only queries, with mutations routed through editor policy.
- Final combined `CARGO_INCREMENTAL=0 cargo run -p xtask -- gate full` passed:
  336 Linux images in 409.44 seconds, 576 all-feature framework tests, 151 host
  tests with six explicit ignores, strict Clippy, wasm32, rustdoc, generated
  checks and performance budgets. Combined JS passed 202 tests with 15 OS
  skips and catalog 200/200. Actual buttons verified indeterminate upload
  progress and complete native rich bold styling; their images were inspected.
- Run 34413155692 at source 0703 passed both full native jobs, macOS runtime
  and all Metal/WARP lanes (336 images each, no changed/new artifacts). Windows
  UIA now reached actual Invoke and dismissal, with editable/form checks also
  passing. That success does not identify which old desktop traversal stalled.
- Run 34415773540 at source 101cff still failed two Windows runtime probes.
  Exact private environment paths and all three ACCESS_DENIED writes passed;
  file/ACL and normal/breakaway spawn refusals passed. TCP returned timeout,
  not permission denial, so network remains unaccepted; later independent
  assertions were not reached. Host-death killed processes but left the profile.
  macOS runtime passed 200 tests with 14 skips. Debug helper was not built and
  its suite never ran after the Windows sandbox abort, so no native debug
  cancellation success is inferred.
- The next candidate keeps the worker's atomic isolation Job while detaching
  only its trusted cleanup supervisor from Node's host-death Job. Session
  preserves stdio/reap ownership. Native positive network controls and exact
  SID/PID/port diagnostics precede strict network assertions; handle checks run
  earlier. Workflow builds/tests the Windows debug helper independently and
  captures WFP drop evidence without changing rules, capabilities or exemptions.
  These changes need fresh Windows execution. Timeout remains a failing probe,
  not an accepted network-isolation result. Debug cancel/timeout retires and
  reaps the isolated session; explicit Reload recovers, without claiming to
  undo arbitrary evaluated JavaScript in a surviving worker.
