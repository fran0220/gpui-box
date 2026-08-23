# Product foundation roadmap

This roadmap closes five substrate gaps that otherwise force every host to
invent a private UI framework beside GPUI Box: basic rich-text editing,
Linux/Web media, real platform accessibility, enforceable performance budgets,
and window-safe plugin mounting. They are in scope. The work is staged because
they share framework primitives and must not become five unrelated component
implementations.

## Fixed boundaries

The same ownership rule applies to every phase:

| Layer | Owns | Does not own |
|---|---|---|
| GPUI framework | shaped runs, edit/selection/composition geometry, rendering and hit testing, per-window identity, accessibility tree relations, native-child handoff, platform-view hosting, frame measurements | theme, product document policy, plugin protocol policy |
| GPUI Box Kit | token-backed rich-text and media presentation, truthful states, semantic ids, standard commands and scenes | grammar/LSP facts, persistence, collaboration, URLs, credentials, DRM, queues, cache/network policy |
| Platform adapter | IME/caret/AT delivery, decoder/output/view integration, capability detection | a platform-specific visual language or invented fallback data |
| Host | authoritative document and validation policy, grammar/LSP/diagnostics, durable storage, collaboration, media sources and policy, plugin placement and permissions | private copies of GPUI layout, text geometry, media controls, semantic registries, or performance instrumentation |

Every visible state continues to use Kit tokens and the same components in all
themes and on all targets. A Linux player is not a Linux skin, a browser editor
is not a DOM-themed editor, and accessibility does not get a second hidden
component tree with different state.

## Dependency order

```text
window semantic context ──┬── accessibility window trees
                          └── plugin mount/action isolation

editable shaped layout ───┬── rich-text editor
                          └── editable character/caret geometry

frame counters ─────────────── performance gate for later phases

platform view + native child ─┬── Web HTMLMediaElement
                              └── accessible native media handoff
```

Work lands as independently reviewable commits in the order below. A later
phase does not paper over an unmet earlier primitive.

## Phase 0: truthful inventory and baseline

1. Keep the state ladder as the one loading/empty/unavailable/failed/stale
   vocabulary and correct coverage entries that lag the catalog.
2. Reclassify the five capabilities in this document from permanent refusal to
   known work, while preserving the host-owned boundaries above.
3. Hold macOS and Windows scene baselines for every new visible component.
   Linux remains a compile/behavior target rather than a pixel authority.

Acceptance: generated API and site catalogs agree with source; `xtask gate`,
macOS headless checks, and Windows WARP checks pass on the same commit.

## Phase 1: one runtime context per window

Replace the application-global frame with a window-owned context without
changing 400 component call sites into plugin-aware code.

- The installed semantic coordinator maps GPUI `WindowId` to one
  `WindowSemanticContext`. A probe selects its context from the window during
  prepaint, where the true window is known.
- `begin_frame`, generation, snapshot, duplicate diagnostics, action routing,
  and unmount cleanup are per window. Rendering one window cannot clear or
  collide with another.
- Kit state retained by `RenderOnce` builders — measurements, scroll and list
  handles, edit memory, modal stacks, follow state, motion caches, effect
  replay/budgets, and toast routing — is keyed by the same `WindowId` and is
  removed when that window closes. A local component id is never treated as
  application-global identity.
- Add validated segment types for semantic ids. Plugin ids, mount ids, and
  local business keys are qualified by constructors, never scattered string
  concatenation. Prefix checks are segment-aware and plugin parents must stay
  under the host-assigned mount root.
- Preserve duplicate publications in the diagnostic snapshot so an audit can
  explain both owners instead of silently replacing one.
- Keep components dependent only on a scoped semantic sink. Plugin/runtime
  types remain outside Kit components.

Acceptance: two simultaneously rendered test windows may publish identical
local component ids without sharing generations, nodes, transient visual
state, modal order, effect replay, or toast destinations; two mounts of one
plugin remain distinct; invalid segments, parent escapes, duplicate nodes, and
actions after unmount are refused and tested.

## Phase 2: deterministic performance budgets

Add low-overhead counters at framework boundaries before optimizing individual
components.

`FrameStats` records, per window and frame, entity renders, element
request-layout/prepaint/paint calls, invalidations coalesced, semantic nodes,
platform-view placements, and allocator delta when the test allocator supports
it. Testkit exposes a `PerformanceBudget` assertion with named limits. Shipping
builds pay only fixed counters, with expensive allocation accounting behind a
test/perf feature.

The first required budgets cover 10,000-row `List`, `DataGrid`, `TreeGrid`,
`CodeView`, `LogStream`, and a long `AgentDocument`. They gate bounded work,
not machine speed: mounted rows, builder calls, semantic nodes, invalidations,
and retained allocations must remain proportional to the viewport rather than
the dataset.

Timing remains appropriate only where structure cannot stand in for cost:
shaping, rasterization, and end-to-end draw. Those checks use Criterion,
same-process calibration, warmup, a median and a noise margin; raw wall-clock
milliseconds from one CI runner never become a cross-platform constant.

Acceptance: `cargo run -p xtask -- performance check` is part of the authority
gate, emits a machine-readable report, and a fixture that deliberately renders
all 10,000 rows demonstrably fails it. macOS, Linux, Windows, and Web compile
the counters; calibrated timing jobs report by renderer and fail only outside
their documented noise envelope.

## Phase 3: editable shaped text and basic rich text

### Framework primitive

Promote the duplicated `TextInput`/`TextArea` edit arithmetic into a reusable
UTF-8 editable document and shaped editable layout:

- grapheme-safe selection, reversed selection, marked composition, replacement
  transactions, undo/redo grouping, UTF-8/UTF-16 conversion, and secret-history
  refusal;
- normalized non-overlapping style runs that follow insert, delete, replace,
  undo, redo, and composition without splitting a grapheme;
- wrapped visual rows, byte-offset-to-position, position-to-byte-offset,
  range rectangles, caret rectangle, scrolling reveal, bidi, and IME bounds
  from the same shaped layout that paints;
- one API consumed by plain fields, rich text, find highlighting, and
  accessibility. No component keeps a second character-geometry algorithm.

Migrate `TextInput` and `TextArea` to the primitive first with behavior held
constant. Their tests prove the extraction before rich behavior is added.

### Kit component

Add `RichTextDocument`, inline marks, paragraph alignment/list metadata,
`RichTextEditor`, and typed edit/selection/format/link intents. The basic
vocabulary includes paragraphs, hard/soft breaks, bold, italic, underline,
strike, inline code, links, ordered/unordered lists, undo/redo, clipboard,
keyboard commands, IME, and caller-supplied diagnostic ranges. The standard
toolbar composes existing Kit controls and tokens; a compact toolbar may be
omitted, but downstream does not need to invent formatting behavior or visual
states.

The foundation deliberately does not parse a storage format, run a grammar or
LSP, fetch links/images, persist, collaborate, or accept arbitrary embedded
GPUI elements. Hosts convert their document format to the product-neutral
model and remain authoritative after each typed intent.

Acceptance: model property tests cover edits at every grapheme/style boundary;
IME, bidi, wrapped selection, undo/redo, clipboard, disabled/read-only and
secret behavior are driven through simulated input; the rich-editor scene is
visually inspected in both themes and checked on Metal and WARP.

## Phase 4: platform accessibility completion

Build on the same shaped editable layout and per-window context.

1. Publish editable per-grapheme positions/widths, visual rows, selection, and
   native caret geometry from the layout that painted the text.
2. Add GPUI relationships for labelled-by/described-by that resolve across
   deferred subtrees in one window and disappear when either endpoint leaves.
3. Add platform-view/native-child handoff so an embedded media or browser view
   participates in one accessibility hierarchy rather than becoming a second
   unnamed root.
4. Make live-region creation/update/removal observable at the platform adapter
   boundary and test announcement events without asserting synthesized speech.
5. Keep secrets out of text runs, values, relationships, debug trees, and
   action payloads.

Validation is layered:

| Target | Required proof |
|---|---|
| All | deterministic AccessKit tree/actions, editable geometry, stale action refusal, relationships and live events |
| macOS | `AXUIElement` role/name/value, selected range, range bounds/caret, described help, dialog/menu lifetime and live notification |
| Windows | UI Automation process-scoped role/name/value, text range/caret, actions, dialog/menu lifetime and notification events on a Windows runner |
| Linux | AccessKit Unix plus AT-SPI role/name/state/text/action smoke in an isolated D-Bus session |
| Web | browser DOM mirror role/name/state/text selection/action/live-region tests through the existing web gate |

Manual VoiceOver, Narrator, Orca, and browser screen-reader sessions remain a
release checklist for speech order and wording; automated adapter evidence is
not misreported as proof of what a person heard.

## Phase 5: Linux and Web media backends

Keep `MediaPlayer`, `MediaSnapshot`, `MediaTransport`, `AudioPlayer`,
`VideoPlayer`, `TransportBar`, and `StateView` as the sole public behavior and
visual contract.

- Linux uses a GStreamer adapter. Runtime/plugin/codec absence is
  `NoBackend`; a rejected source is `Failed`. Audio uses the platform sink.
  Video uses a bounded latest-frame appsink path into a GPUI image surface so
  decoding cannot grow an unbounded frame queue or require a private GTK tree.
- Web uses `HTMLAudioElement`/`HTMLVideoElement`. GPUI Web gains real DOM
  platform-view placement, clipping, visibility, stacking, resize, detach, and
  accessibility handoff instead of an inert handle.
- Add an explicit `MediaCapabilities` snapshot (audio/video, rates, seek,
  native tracks, output selection) so a control is absent or disabled when the
  backend cannot honor it. Capability refusal is never idle media.
- The shared transport and native service already carry that capability
  snapshot, and `MediaErrorKind` preserves no-backend, invalid-source, open,
  playback and command-refusal categories through the Kit boundary. Linux and
  Web adapters extend these types rather than introducing target-specific
  state or string parsing.
- Feature and build policy is target-specific but the default `native-playback`
  feature selects the normal backend on macOS, Windows, Linux, and Web. Linux
  distribution builds install GStreamer development/runtime packages; a build
  that intentionally omits the feature retains the truthful no-backend API.

URL/auth headers, cookies, DRM, queue/playlist ownership, track policy, output
device policy, caching/retry, capture, and frame extraction remain host policy.

Acceptance: common contract tests run against deterministic fake backends and
all real adapters; Linux exercises a generated local audio/video fixture under
GStreamer, Web exercises the same fixture in the browser gate, and macOS and
Windows lifecycle tests remain green. The existing player scenes do not fork
by platform and their Metal/WARP pixels remain the visual authority.

## Phase 6: plugin mount and action isolation

Publish the reusable protocol substrate after per-window semantics prove the
lifecycle:

- a small GPUI-free, versioned DTO crate for `PluginId`, `MountId`, local node
  and action ids, mount envelopes, bounded typed action envelopes, capability
  negotiation, and generated JSON Schema;
- a host adapter that qualifies
  `plugin.<plugin>.<mount>.<local-business-key>`, validates parent containment,
  associates every action with its window and live mount generation, and
  rejects late/stale actions after unmount;
- bounded payload size/depth and redaction before diagnostics; no raw path,
  credential, arbitrary GPUI element, or native handle crosses the protocol;
- compatibility fixtures proving additive minor evolution and explicit
  unsupported results for unknown kinds.

Surface placement, process isolation, transport/JSON-RPC, permissions, and
product slot policy remain host concerns. The shared DTO prevents each host
from inventing namespace, lifecycle, and action-routing safety; it does not
turn the current GPUI-bound component builders into a wire ABI.

Acceptance: two windows, two plugins, repeated mounts, reorder, unmount/remount,
duplicate local ids, stale action delivery, payload limits, and schema backward
compatibility all have end-to-end tests through `gpui-kit-testkit`.

## Release and cross-platform gate

Each phase updates compatibility and provenance documentation when it changes
framework/platform authority, runs `xtask dependencies check`, and carries
focused framework tests in addition to Kit tests. A visible change requires
scene capture and inspection; an invisible primitive must not add a scene just
to claim coverage.

The completion matrix is:

```text
                 macOS       Windows       Linux          Web
compile/clippy   required    required      required       required
behavior         required    required      required       required
native adapter   AX/media    UIA/media     AT-SPI/media   DOM/media
visual pixels    Metal       WARP          retired        browser smoke
performance      structure + calibrated renderer reports on each target
```

Windows validation uses the repository's `windows-2025` jobs for reproducible
compile, UIA and WARP checks. An interactive runner or SSH host is used only to
diagnose/accept a renderer or native-session difference; a baseline is never
fabricated on macOS or copied from another renderer.
