# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Entries say what the library now does and what it refuses to do, because a
refusal is the part a caller has to plan around.

See `docs/releasing.md` for the protected publication and verification runbook.

## [Unreleased]

### Added

**Application paint the role vocabulary does not model.** `Theme` now carries
the active document's palette and reads an entry by `"group.step"` through
`Theme::palette_color`. An application whose product has colour the shared
roles have no slot for — a colour per person, a syntax class, a diff sign —
keeps it in the same token document, validated by the same parse and retinted
by the same registry, instead of as literals in views. Components never reach
past the typed roles themselves. An entry the active document does not declare
resolves to `None` rather than a guessed colour: a theme that has not named a
scale has not agreed to paint it.

**Identity colour on the mark surfaces.** `Avatar`, `Badge`, `StatusDot`,
`StatusLine` and `Tag` accept a caller-owned `tint`. A tint answers whose the
mark is while the tone still answers how it is going, and painting cannot edit
the claim: `Badge`, `StatusLine` and `Tag` now publish the tone by name, so a
mark wearing a colour no tone maps to can still be asked what severity it
reported. A tinted mark keeps the tone language's own treatment — a carried
wash, not a filled shape — so a colour cannot turn one mark into a second mark
shape. `StatusLine` additionally reaches `StatusDot`'s breathing state, so a
running row does not have to be rebuilt out of parts to move. Tints are never
derived from a name; an application that wants a stable colour per identity
derives it and passes it, because the library cannot know which colours that
application has spent. `Callout` refuses a tint: a refusal is a severity, not
an identity.

**Working signature on in-flight tracks.** `color.loader.gradient` now
resolves to the three `palette.loader` stops rather than a pair of indigo
accents, and the same wash paints every surface that means "work is
happening": `ProgressBar`, `ProgressCircle`, a playing `TransportBar`
scrubber, `PulseLoader`, `GradientSpinner`, and the `Skeleton` shimmer.
Idle chrome — a paused transport, a slider, a tab indicator, a focus ring —
stays on accent. The signature never enters the semantic tree: a progress
node still publishes its fraction and its busy flag. Under reduced motion an
unknown-extent bar parks a still band at the leading edge rather than
filling the track.

**Box substrate, not a Kit-era subset.** Coverage now treats charts and the
remaining form shapes as missing application primitives rather than another
library's job. Box still refuses to invent calendar arithmetic, locale
wording, transports, and OS window chrome; it no longer refuses the
surfaces a downstream desktop app has to put on screen.

**Number adapter.** Counts go through `NumberAdapter` the way dates go
through `DateAdapter`. `ProgressBar` asks it for `3 of 12` instead of
formatting Rust digits. The English adapter is a fallback, not a locale.

**Schema shapes a settings page can name.** `SchemaKind` now includes
`Date`, `Time`, `DateRange`, `Files`, and repeating `List`. Until a host
supplies a date adapter or a file policy those fields stay visible as
unrenderable rather than disappearing.

**Line chart.** `LineChart` paints one or more host-owned series inside a
frame whose axis wording the host already wrote. It invents no domain and
keeps Loading / Empty / Unavailable / Error / Ready distinct.

## [0.1.2] - 2026-08-13

### Added

**Native media playback.** `media::PlatformMediaTransport` now implements the
existing player seam with AVFoundation on macOS and Media Foundation on
Windows. Audio and video load local files or platform-supported URLs and expose
play, pause, seek, volume, mute and rate commands plus non-blocking snapshots of
position, duration, buffered ranges, buffering, end and native errors. Video
uses a retained `NSView`/child `HWND` through GPUI's platform-view host. Linux
and Web keep an explicit no-backend state; playlists, DRM, media-track policy,
output-device routing, custom network policy and capture remain outside the
service. Source replacements invalidate callbacks from the prior decoder;
failed replacements and terminal errors remain failures, and seek or restart
state changes only after the native operation succeeds. AVFoundation creation
and teardown are main-thread confined, while Media Foundation and caller-owned
COM apartment lifetimes remain balanced across concurrent players. Native CI
executes load, playback, seek, end, restart, audio setting, replacement, and
teardown behavior on macOS and audio-equipped Windows hosts. The audio-less
hosted Windows runner additionally proves that an unavailable native sink is a
truthful no-backend state rather than a bad-source failure.

### Changed

**Truthful native-view clipping.** A partially clipped `platform_view` now
keeps its full layout frame while the platform host crops it to GPUI's visible
rectangle. macOS uses a masking container and reapplies GPUI paint order to
existing views; Windows keeps full child `HWND` geometry and clips the popup
host region. Scrolling no longer resizes native video or web content.

**Catalog site.** The public site now keeps compose, scenes, and components on
the home page. Scenes list the components they build. The only standalone pages
are Docs and MCP. The former playground is `/compose/` and is embedded on the
home page in both themes. Dark navigation uses primary text so the links stay
readable. Old `/components/`, `/scenes/`, and `/playground/` URLs redirect.

**Faster validation.** CI now runs once per pull-request commit, cancels
superseded runs, omits debugger data, and caches both Cargo workspaces used by
the headless renderer. Platform jobs no longer repeat the Linux authority
gate, and the release workflow composes the same independent proofs instead of
rerunning the full gate on macOS. `xtask web gate` builds and prepares Chromium
once for the browser checks. Passing headless frames are compared in memory;
only changed or new images are PNG-encoded for review. Routine local builds
retain source-line backtraces while omitting the variable-level debug data that
made accumulated build artifacts unnecessarily large. The registry-only
package gate now materializes the patched `block` crate from its vendored source
instead of accidentally depending on a warm developer Cargo cache, and removes
multi-gigabyte temporary consumers after a successful proof rather than feeding
them into the CI cache. Headless CI now enters its independent workspace
directly instead of compiling the overlapping root `xtask` graph first, and
its deterministic build omits native window-system and media playback backends
that no captured scene constructs. Normal framework and Kit builds retain
those native backends by default. The Windows cold gate now builds that reduced
harness once and renders four disjoint scene shards in parallel instead of
driving all 198 WARP images serially. Package validation invalidates historical
transient proof caches and patches only each archive's actual local dependency
closure, eliminating restore diagnostics and unused-patch warnings.

**Single-repository framework authority.** GPUI Box now develops GPUI, native
platforms, media, and Kit directly in this repository. The former Zed sync and
fork-overlay mutation commands have been removed. `scripts/sync-zed` retains a
read-only, offline verifier for the exact historical mappings, vendor refs,
source trailers, integration merges, package identities, and licenses; releases
no longer contact either historical source repository.

## [0.1.1] - 2026-08-12

### Added

**Native platform views.** `PlatformViewHandle` and `platform_view` now make
GPUI the layout, clipping, stacking, visibility, and teardown authority for a
caller-owned native view. macOS retains and hosts `NSView` instances between
GPUI's base and deferred-overlay planes. Windows reparents caller-owned child
`HWND`s into a redirection-backed popup host, follows owner geometry, DPI and
visibility, clips the host to the painted view union, preserves paint order,
and restores parent, non-visibility styles, extended style, and window region
on detach while deliberately leaving the child hidden for its owner to place.
Linux and Web retain an explicit inert layout-only contract rather than
pretending a native view was attached. This unblocks WKWebView and WebView2
consumers. The migration guide also distinguishes GPUI's inherent
`Window::window_handle()` from `raw_window_handle::HasWindowHandle` and shows
the fully qualified raw-handle call required by Rust method resolution.

**Reproducible fork overlays.** `scripts/sync-zed` now keeps post-bootstrap fork
work in an exact-SHA, bootstrap-rooted vendor lane distinct from official Zed
first-parent replay. Release verification reconstructs both lanes from remote
objects, validates the shared filter digest and exact source parent chain,
requires both canonical refs and integration merges, and proves that the lanes
meet only at the deterministic filtered bootstrap.

## [0.1.0] - 2026-08-11

### Added

**GPUI Box umbrella distribution.** Filtered GPUI framework/support source is
now imported from `fran0220/zed@0b9c8dc932b65cba2dc87464148984e93f60ae18`
against official baseline `a6a23c7b80a5cefa0487b7856335be89ace7e483`,
rather than linked as a Cargo Git dependency. The public identities are
`gpui-box` (Rust `gpui`), the `gpui-box-*` framework cohort,
`gpui-box-kit`/`gpui-box-kit-*`, and `gpui-box-mcp`, with one GPUI type
universe. A local-registry package gate validates registry-only framework and
framework-plus-kit consumers, while `scripts/sync-zed` records the reproducible
filtered update boundary. Its committed bootstrap receipt is reproducible from
the pinned fork and official source objects and is enforced by the release gate.

**Undo history.** `navigation::UndoHistory` renders caller-owned revision
entries and reports only the durable identity a typist asked to restore. The
caller owns order, current identity, already-formatted time and source labels,
and restoration refusals; an unavailable entry stays visible with its reason,
and the component keeps no undo stack and mutates no document state.

**Media.** `media::AudioPlayer`, `media::VideoPlayer` and `media::ModelViewer`
are the three surfaces a desktop application cannot assemble out of a button
and a slider. The two players run over `media::MediaTransport` — `origin`,
`snapshot`, `apply` — which this crate declares and does not implement, because
there is no decoder, no output device and no frame pump in GPUI at the imported
revision. A control asks the transport and reports `Applied`, `Refused` with the
backend's own sentence, or `Unsupported`; the next frame draws the transport's
snapshot, so a refused seek leaves the head where it was. Idle, loading, no
backend, failed and ready are five renderings and a surface with no transport is
a sixth that carries no controls at all, because a bar with a head at zero says
playback exists and has not started. `VideoPlayer` publishes `frame` for a
picture the host supplied and `poster` for a still standing in for one, with the
reason no picture is arriving drawn over it, and asks for no frames at all from
a transport that has said it cannot open the media. `AudioPlayer` draws a
waveform only from peaks somebody measured. `media::FixtureTransport` decodes
nothing and advances no clock, and every surface publishes `MediaOrigin`, so a
fixture is never mistaken for a player.

`ModelViewer` reads glTF 2.0 through `media::ModelScene::parse`, which accepts a
stated subset — both containers, buffers inside the file, triangle primitives
with a `VEC3` `FLOAT` `POSITION` accessor, and node hierarchy — and refuses
everything else, including any URI it would have to fetch. `media::ModelBounds`
caps bytes, nodes, depth, primitives, vertices and triangles, and every cap is
checked while reading rather than after allocating, so a document declaring ten
million vertices is refused at the accessor that says so. The model is drawn
flat-shaded from face normals or as a wireframe, with an orbit camera the caller
owns; a refusal names the limit or the defect, and a viewer holding nothing
publishes no counts rather than three zeroes.

**Glass and edge fades.** `overlay::Frost` paints a frosted surface: the pixels
behind it are blurred and the surface colour is laid over them at the new
`effect.glassAlpha`, with `effect.glassBlur` deciding how far the blur reaches.
The whole subtree paints inside one scene layer, so the blur cannot be reordered
over the content it sits behind. It fakes nothing where blur does not exist —
a renderer without a backdrop blur, or a theme that sets `effect.glassAlpha` to
1, gets the tinted fill on its own and a surface that is merely unblurred.
`layout::ScrollFade` fades content towards the edges it is scrolled past by
multiplying each painted primitive's opacity, which is what says "there is more"
over a surface no gradient can match; the edges are the caller's statement about
hidden content, and a region that hides nothing fades at neither edge and
publishes `none` rather than implying overflow that is not there.

**Platform accessibility.** Semantic nodes now project supported roles, names,
values, control states, focus, numeric ranges, and widget selection into GPUI's
AccessKit tree while retaining the deterministic test registry. The maintained
fork forwards that tree to macOS AX, Windows UIA, and Linux AT-SPI and exposes a
deterministic adapter smoke path. Editable caret/selection, live regions, and
native-child handoff remain explicitly unsupported.

**Tokens and theme.** `crates/gpui-kit-tokens/tokens/studio-dark.json` and `crates/gpui-kit-tokens/tokens/studio-light.json`
are the source of truth, expressed as a palette plus references so retuning a
scale is one edit rather than one per role. Themes carry elevation, z-index and
density axes, spring presets and the six easing curves, and are built through
`Theme::from_tokens` at a density that scales spacing, control geometry and type
onto the pixel grid while leaving colour and radius alone. `ThemeRegistry` lets
an application register its own document and switch theme or density at runtime;
an unknown id returns false and leaves the active theme where it was rather than
falling back to a default nobody asked for. `xtask tokens check` verifies WCAG
contrast for every theme and fails if the generated reference has drifted.

**Controls.** `TextInput` and `TextArea` with grapheme-aware caret motion, word
motions, selection by keyboard and pointer, cut/copy/paste and input-method
composition; a secret renders as dots, stays out of the clipboard, and publishes
only that something was typed. `Checkbox` with a real mixed state, `Radio`,
`Switch`, `Slider`, `Select`, `Combobox`, `NumberInput`, `TagInput`,
`SegmentedControl`, `FormField`, `InlineEdit`, `SettingsRow` and
`SettingsSection`, `Button`, `IconButton`, `ButtonGroup`, `SplitButton`,
`KeybindingRecorder`, `FilterBar`, `Dropzone`. Every one of them reports an
intent and applies nothing: a value, a selection and a choice belong to the
host, so a refused change is visible as the control not moving. A disabled or
refused control installs no handler at all, which is why disabled is behaviour
rather than opacity.

**Overlays.** `Overlay` places a surface below, above, at a point or centered,
taking its paint order from the z-index tokens; `FocusTrap` cycles only the
stops the open frame registered and restores focus on close. `Dialog`, `Drawer`,
`Popover`, `Menu`, `ContextMenu`, `CommandPalette`, `Tooltip`, `Kbd`, and
`Toast` over a host-mounted `ToastLayer`. A dialog that cannot be dismissed
installs no dismissal, so escape and the scrim cannot close a decision the host
requires. A danger or warning toast never leaves on a timer, a pointer resting
on the stack pauses the countdown, and a push that went nowhere says so instead
of being dropped. A palette that nothing answered says so rather than showing an
empty list, and a refused command keeps its reason instead of disappearing.

**Data.** `List` over GPUI's uniform list, publishing its total so a test can
tell a thousand items from the twelve that were drawn; `Table`, whose header
click reports the sort it implies and sorts nothing; `Tree`, which renders none
of a collapsed node's children; and `DataGrid`, which virtualizes rows, reserves
slots beneath an opened row, and reports every sort, width, column order,
selection, expansion and edit without applying any of them. The grid keeps the
two select-all claims apart, because selecting the forty rows that are loaded
and selecting all twelve thousand are different promises. A fit-to-content
request asks the host, since a grid can only measure the rows it drew.

**Navigation and layout.** `Tabs`, `Accordion`, `Breadcrumb`, `Sidebar`,
`Pagination`, `Wizard`, `SplitPane`, `SplitTree`, `ScrollArea`, `Toolbar`,
`Dock` and `StatusBar`. `Pagination` can say there is another page without
inventing a total. A collapsed sidebar still publishes every item's full name.
An item past a toolbar's declared cut moves into the overflow keeping its
identity and its refusal, never dropped. `SplitTree` propagates minimums up the
tree so a divider stops where a leaf two branches below would starve, and
converts to and from plain records so a host can persist a layout without this
crate taking a serialization dependency. `Dock` builds on that same tree and the
same drag system, so there is one resize implementation and one drag
implementation rather than three.

**Date and time.** `Calendar`, `DateInput`, `RangePicker` and `TimeInput` over a
host-implemented `DateAdapter`. This crate owns no calendar, no time zone and no
locale, and holds no month or weekday name in any language: `Day` and `MonthKey`
are opaque integers the host mints, and moving a month is an adapter call rather
than an addition. An adapter is allowed to answer "I don't know", and each of
those has a rendered consequence instead of a guess — no today means no ring and
no guessed month, a refused `shift_month` stops navigation on every route into
the month beyond it, and a `days_in` of `None` makes a range say its days could
not be checked rather than that it is clear. The reference calendar is behind
the `fixtures` feature, off by default.

**Content.** `Markdown`, `MessageList`, `ImageViewer` and `TransportBar` — the
surfaces that draw text and media nobody in the application wrote, and therefore
the ones that act on none of it. Raw HTML is drawn as the characters somebody
typed and marked unrendered, because interpreting it lets a document reach
outside its own text and dropping it lets a document delete itself from the
reader's view; `pulldown-cmark` is compiled with its html feature off, so there
is no renderer to reach for. A link states where it goes and opens nothing, an
image is named rather than fetched, and a code fence is coloured only by spans
the host computed. `MessageList` keeps five delivery states, because collapsing
sent, delivered and read into one tick says less than the host knows and folding
a failure into any of them says something untrue. `ImageViewer` decodes nothing
and reports an unknown source size as unknown; `TransportBar` reports play,
pause, seek, volume, speed and a track step and applies none of them.

**Motion.** Springs solved in closed form with a bounded settle time,
`Interpolate` for `f32`, `Pixels`, `Rems`, `Hsla`, `Point` and `Size`,
`Transition` retargeting from the value on screen, `Presence` keeping an element
alive for its exit, `Stagger` over a fixed group window so a fifty-row menu
opens as fast as a five-row one, `Flip` and `flip_size` so a moved element
slides without disturbing its neighbours, `Keyframes` for a path that is not a
straight line between two ends, `AnimatedNumber` that publishes the total it is
counting to rather than the frame it is on, and the press and hover responses
every actionable control wears — withheld from everything that is not
actionable, so a response never promises an action that does not exist. Gesture
motion arrived with them: a velocity tracker measured over a trailing window and
sampled against the clock at the moment it is asked, so a drag the user parked
before releasing reports no velocity and flings nothing; `flick`, `rubber_band`,
and `ScrollLink`, which reads an offset as a progress, holds no state, and never
consults reduced motion on its own because only the caller knows whether the
motion answers the user's own hand. Everything honours `App::reduce_motion` by
settling in one frame.

**Drag and drop.** One payload and one vocabulary for where a drop lands: before
or after a named anchor, or into one that can contain it, never an index,
because an index stops meaning anything the moment the host applies the move.
The drag reports an intent and moves nothing; the row changes place when the
host hands back the new order. It publishes what is held and where it would land
while it is in flight, so a test reads a drag from an ordinary snapshot. An item
onto itself and a node into its own subtree are refused as impossible; every
other refusal belongs to the caller, and a zone that cannot take what is over it
never looks like a zone that is merely empty. `List`, `Tree`, `Tabs`,
`Dropzone`, `DataGrid` column headers and `Dock` panel headers all use it.

**Semantics and testing.** A per-frame semantic tree measured during prepaint,
with roles, state and bounds, and ids that come from business identity rather
than list position. `gpui_kit_testkit::audit` rejects positional, empty and
duplicate ids, unnamed actionable roles, values outside their own reported
range, text that survived redaction, and visible nodes with no size. A test
harness drives simulated keys, pointer, drags and frames against a simulated
clock. `gpui_kit::scenes` is one canonical rendering per component, shared by
the gallery, the capture task and the headless audit, so a component cannot be
reviewed visually in one arrangement and tested in another. In-process window
capture asks the window server for the process's own window and never the
desktop.

**Node graph.** `NodeGraph`, `GraphNode` and `GraphEdge` draw a run as connected
steps rather than as a list, which is the shape a run takes once anything is
retried. A node reports what it is, what it is doing now, how it ended and what
it cost, and the five endings stay five: pending, running, succeeded, failed and
refused are separate colours, separate glyphs and separate published values, so
a host that declined to run a step is never drawn as a step that broke. Forward
work is a solid path and a retry is a dashed one in the danger colour, dipping
below the nodes it returns under, because a loop drawn like a flow would report
a run that went cleanly in a circle. There is no layout algorithm: the caller
places every node, since where a step belongs is a claim about the run. An edge
naming a node the canvas does not have is dropped rather than pointed at the
nearest box.

**Web view shell.** `BrowserPanel` is the chrome around an embedded web view and
draws no web pages, because rendering one means an engine and no component
library should charge every host that wanted a button for a browser. The host
owns the engine and hands the panel a `ViewportState`. The default state is
`Unavailable`, not `Ready`: a build with no engine says so instead of showing a
blank page, and "the site served nothing" and "this build cannot ask" are the
two failures a reader most needs told apart. Refused and failed stay separate
for the same reason, and a history control with nowhere to go is disabled and
installs no handler.

**Tooling.** `xtask tokens generate|check`, `xtask scenes list|render`,
`xtask headless capture|check`, and `xtask gate [full]`. The headless check is
the renderer-specific visual regression gate; see `docs/screenshot-testing.md`
for where it can and cannot run.

### Changed

**Supported platforms.** macOS, Linux, Windows, and single-threaded
Browser/WASM are active CI surfaces. The three native platforms compile every
feature and gate renderer-specific headless baselines; Browser/WASM gates the
shared gallery and real Chromium smoke within its documented accessibility and
threading limits.

- Studio Dark now keeps faint text above 4.8:1 even on the raised surface,
  strengthens hairlines that previously disappeared into dark panels, and
  leaves disabled content visibly present while still subdued. Both Studio
  themes now use the library's indigo accent for loader gradients instead of
  a separate blue/orange/pink palette. These are token changes, not
  component-local colour exceptions; the library still refuses to infer
  semantic status colours or to make unavailable content look enabled.
- `TypeScale` gained `Strong` and `Subtitle`. The scale ran caption 10.5, label
  12, body 13, and then jumped to title 16 at weight 600, so anything between a
  field label and a component's own name had nowhere to land and reached for
  `Title`. `Strong` is body's size and line height at weight 600, so a run can
  be emphasised without changing the line box it sits in; `Subtitle` is 14/20 at
  600 for a heading inside a component. Both themes carry both steps. No
  component uses them yet, so no baseline moved.
- Visible strings now have a product-neutral `foundation::text` entry point
  that applies one complete `TypeScale` step and the primary `TextTone`; callers
  can override the tone or logical start/end alignment without separating size,
  line height, and weight. `xtask typography check` rejects direct string and
  `SharedString` children. It deliberately does not set a root font: a component
  embedded below a host-owned text style must render identically to the same
  component in the gallery, so inheritance is the defect rather than its fix.
  The pre-change review of all 196 Linux theme/scene images is recorded in
  `docs/coverage.md` before the component-family sweep changes any baseline.
- The visual gate is `xtask headless check` on every platform, and the macOS
  baseline moved from `snapshots/macos/scenes` to
  `snapshots/headless/macos/scenes`. It used to open a real 920×1000 window and
  capture the drawable, whose size the platform clamps to the available screen
  area, so two Macs produced 1840×1568 and 1842×1374 and `snapshots/macos`
  accumulated two incompatible sets that no machine could pass in full. Every
  wave was therefore reviewed with a scoped check against a baseline the
  reviewer could not reproduce. The harness now asks GPUI's Metal headless
  renderer for an exact size, as the Linux and Windows harnesses already asked
  their software adapters, so every baseline is 1840×2000 and a scoped check
  means the same thing as the full one. Comparison allows one step per channel,
  which exactness could not: the sprite atlas has accumulated different state by
  the ninetieth scene of a full run, and that moves one antialiased pixel of
  `frost`.
- `xtask gate only <scene>...` runs the part of the gate one component can
  invalidate: `gpui-box-kit` clippy, the tests whose names mention those scenes, the
  generated-artifact checks, and those scenes' baselines. The full gate compiles
  and tests every workspace member and renders the whole catalog, which is
  minutes of waiting for an edit to one file. It is a shortcut while iterating
  and not what a commit runs.
- `xtask scenes capture` and `xtask scenes check` are replaced by
  `xtask scenes render`, which writes to `target/scenes` and holds no baseline.
  A real window is still how motion and the text caret get reviewed; it is no
  longer how a baseline is recorded.
- GPUI framework source is now a filtered import from the exact recorded Zed
  bootstrap revision rather than a Cargo Git dependency. The root workspace and
  standalone headless harness consume the same local GPUI Box package family
  through path-plus-version declarations with no `[patch]` override. `xtask
  dependencies check` treats `package-authority.toml` as authority and rejects
  manifest, graph, lockfile, compatibility, provenance, or sync-boundary drift.

- The visual regression gate stopped photographing windows and started
  reading frames back from the GPU. `scenes capture` used to ask the macOS
  window server for what it had composited, which meant every image carried
  the window's rounded corners, the compositor's colour handling, and
  whatever settled latency the machine felt like that day; re-capturing an
  unchanged catalog could disagree with itself on half the images. Captures
  now re-render the scene GPUI drew into an offscreen texture and read the
  pixels straight back (`gpui_kit_testkit::capture::render_frame`, built on
  GPUI's `render_to_image` under its `test-support` feature). One warm-up
  frame is rendered and discarded so stray platform mouse events cannot
  hover a row in the first image. Two runs of the same catalog now agree to
  the byte, a scoped check matches the full-catalog baseline in any order,
  and a full check answers in under two minutes instead of six.
  `capture_window`, the window-server grab, remains for photographing real
  composited product windows; the gate no longer depends on it.

- Linux and Windows gained their own visual gate. `tools/headless-visual`
  renders the scene catalog with no window system at all — GPUI's wgpu
  renderer draws into offscreen textures on a software adapter (llvmpipe,
  WARP), text is shaped by cosmic-text from the bundled Geist fonts only, and
  time is simulated. Repeated output from either adapter is byte-stable, while
  their antialiased edges differ, so Linux and Windows hold separate exact
  baselines in `snapshots/headless/{linux,windows}/scenes`.
  `cargo run -p xtask -- headless check` runs it. The harness is its own
  workspace with renderer-specific dependencies and a separate lockfile, but
  resolves the same local GPUI Box authority as the root without a Git source
  or patch. The offscreen WGPU origin remains recorded in the provenance files.

- The design system stopped drawing hairlines to say what a colour could say.
  Borders were doing the work of grouping because the surface ramp was too flat
  to do it — canvas, panel and raised sat within about three percent lightness
  of each other — so a line was the only thing separating a card from what it
  sat on. The ramp was widened first and the lines came out afterwards:
  structural borders became `frame()`, which is a surface plus the elevation
  shadow that belongs to it; dividers between rows, between a header and its
  body, and inside menus and grids were deleted, because adjacent surfaces
  already report the boundary; and input borders became `well()`, a sunken
  surface that reads as somewhere to put something. Lines that carry meaning
  stayed: the column resize handle, the pinned-column edge, and the red border
  an invalid field wears. `DataGrid` row lines are now an option that defaults
  to off. Selection, focus and tone are carried by fill and shadow rather than
  outline, so a badge, a tag and a callout are colour blocks.
- New tokens back that change rather than hard-coded values: `color.surface.
  sunken` for wells, `effect.glowAlpha` and `effect.glowBlur` for the state
  bleed a node or a failure panel uses instead of a coloured border, and
  `motion.durationMs.slow` and `motion.staggerStep` for entrances. A test
  asserts the surface ramp keeps at least two percent between steps, so the
  flatness that made the borders load-bearing cannot come back unnoticed.
- Motion became something a component opts into in one line. The `Animated`
  trait and its four `Entrance` presets — fade, rise, menu, dialog — sit on top
  of the existing spring and easing machinery, which was already complete and
  barely used: `Presence` appeared in three files out of a hundred and eight.
  Reduced motion still resolves to the settled frame, so an entrance is an
  entrance and never a requirement.
- Work in progress has one vocabulary instead of one improvisation per
  component. `Activity` splits it by what the component actually knows:
  `Advancing` sweeps when there is measured progress, `Working` spins when
  something is running but its end is unknown, and `Deliberating` breathes when
  a model is thinking. `Icon::spinning`, `Icon::breathing` and `StatusDot::busy`
  are the one-line spellings, and `motion.durationMs.spin` backs the period.
  Six places that had been reporting work with a still glyph or with colour
  alone now move: `ToolCallCard`, `StepList`, `ThinkingBlock`, `UploadList`,
  `MessageList` and `ProgressCircle`, whose indeterminate ring now travels an
  arc instead of tinting the whole circle and standing still.
  Motion is never the only carrier: a thinking block says "Thinking" where a
  settled one says "Reasoning", so reduced motion loses the movement and keeps
  the state.

- The catalog is readable by a program. `docs/api-index.json` carries all 122
  components, the exact signature of every public method sorted by what the
  caller has to hold, the events each one reports, and the scenes that render
  it — generated from the source by `xtask api generate` and checked by `gate`,
  so a signature it states is one a compiler agreed to. Each of the 99 scenes
  carries its own source as an example, which is worth more than a written one
  because the gate compiles it and `headless check` renders it. `docs/llms.txt`
  is the entry point, and `tools/mcp` serves the same catalog as Model Context
  Protocol tools, one of which renders a scene and returns the image so a
  caller can look at a component rather than read a description of it.
- The catalog is published. `xtask site generate` builds a static site out of
  the same index, the same scene sources and the same captured images the gate
  checks, styled from the token document the components read, and one
  Cloudflare Worker serves it alongside an MCP endpoint at `/mcp`. It needs no
  host: `render_scene` can only ever draw catalog scenes, the catalog is 99
  scenes in two themes, and captures here are deterministic, so the bytes a
  hosted renderer would produce are the bytes already committed. The hosted
  server therefore serves the published revision and says so, while the local
  stdio server renders the working tree and can show a component being changed.
- The token documents moved to `crates/gpui-kit-tokens/tokens/`. A package may
  only carry files under its own directory, so `include_str!` reaching up to a
  repository-root `tokens/` meant the one crate in this workspace that does not
  depend on GPUI — and could therefore be published — could not even be
  packaged. It packages now. `docs/releasing.md` records what a release is
  here: one protected, verified, registry-only-tested cohort from an immutable
  tag, rather than an isolated package upload.

- The component-level `effects::frosted` and `effects::edge_faded` wrappers
  remain removed. The integration fork now carries the underlying
  BackdropBlur and EdgeFade primitives, but a public component does not return
  until its renderer fallback, tokens, semantics, and scenes make the same
  truthful promise. Anchored overlays and modals therefore still draw the
  opaque overlay surface. `effect.edgeFadeBand` remains because scroll-linked
  toolbar motion uses it as a distance.
- Components are `RenderOnce` builders that read the theme from the application
  context and derive their element and semantic id from one `Ident`, replacing
  free functions that took a `&Theme` and positional flags. This was a breaking
  rewrite of every call site, made before anything depended on the old shape.
- A consistency pass settled the details a caller notices first: focus is drawn
  one way everywhere, from its own tokens, as an outset ring that cannot be
  mistaken for the inset selection ring and that never moves what it marks;
  `Slider`, `Accordion` and `Sidebar` honour the size they are given; bounded
  stepping has one implementation; and `FieldFrame` and `SearchFrame`, which the
  coverage table claimed and nothing used, were deleted.
- Capturing the scene catalog runs in one process on one window instead of one
  process per image, which took over twenty minutes. Captures became
  reproducible at the same time: reduced motion holds an animation at a defined
  frame, the tracked pointer is parked so a row is not captured hovered because
  of where the operator left the mouse, and the run waits for the window server
  to settle on a frame that is both stable and new instead of sleeping. Because
  the bytes are reproducible, `scenes check` can assert them.

### Fixed

- Linear-gradient dithering used a transcendental float hash whose result was
  not required to agree between GPU families, so identical Metal scenes could
  differ by up to five channel steps across Macs. Metal, Direct3D, and WGPU now
  derive the same triangular noise from an integer screen-pixel hash; renderer-
  specific color conversion still keeps their baseline sets separate.
- `overlay::Kbd` drew `⌘ ⌃ ⌥ ⏎ ⌦ ⌫ ␣` only where the host machine happened to
  own a font covering them. No Geist face does, and the component relied on the
  platform's own fallback, so the library could not draw its own keyboard
  shortcuts unaided and no baseline could record them — the first offscreen
  macOS run rendered them as missing-glyph boxes. `gpui-box-kit-assets` now bundles
  `KeySymbols.ttf`, a seven-glyph subset of Noto Sans Symbols and Noto Sans
  Symbols 2 renamed so it cannot shadow a full Noto family on the host.
- `cargo doc` rejected three redundant explicit link targets in `content::DiffView`,
  `content::LogStream`, and `media::VideoPlayer`, so `gate full` could not pass.
- `scenes check` reported that 112 images matched after writing one of them: a
  run that failed part way still exited zero, and the check counted the images
  it had asked for rather than the ones that arrived. A gate that passes when it
  did not look is worse than no gate, so a capture now names every image it owed
  and fails on the ones that never arrived.
- A capture read the previous scene until it gave up, because a window nobody is
  compositing keeps handing back the frame it drew last. The application claims
  the foreground for the run, and reclaims it in the branch that sees an
  unchanged frame, since anything that takes the foreground part way through
  causes exactly that symptom and no other.
- `Presence` reversed a phase by scaling elapsed time, which is exact only when
  both curves are linear; on an ease-in-out an entrance cancelled at 60ms jumped
  from 0.187 to 0.071 and the element blinked. Progress is now inverted on the
  same clock, so a phase resumes from the opacity that was actually on screen.
- The semantic registry reported the previous frame when a frame published
  nothing, which let a test assert an element had disappeared against a stale
  snapshot.
- Semantic ids forced relative positioning, so any absolutely positioned element
  that also carried one silently collapsed to nothing.

### Not provided

`docs/coverage.md` states what this library refuses to invent — calendar
arithmetic, time wording, grammars, transports, platform window chrome —
and, separately, the application surfaces that are still missing. Charts
and the remaining form shapes are gaps, not refusals. Read it before
planning around a component that is not here.

[Unreleased]: https://github.com/fran0220/gpui-box/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/fran0220/gpui-box/releases/tag/v0.1.2
[0.1.1]: https://github.com/fran0220/gpui-box/releases/tag/v0.1.1
[0.1.0]: https://github.com/fran0220/gpui-box/releases/tag/v0.1.0
