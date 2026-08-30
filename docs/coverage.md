# Coverage

What a general-purpose desktop UI library is expected to provide, what this
one provides, and what is deliberately out of scope. GPUI Box is the
application substrate: if a downstream desktop or browser-hosted product
needs a surface to exist, the surface belongs here unless it is a host
fact, a locale fact, a transport, or a platform chrome the OS already
owns. `docs/components.md` describes the components themselves; this file
exists so a gap is a recorded decision rather than an oversight.

A component counts as covered only when it has all four of: a public builder or
view, a scene in `gpui_kit::scenes`, behaviour tests driven through simulated
input, and an entry in `docs/components.md`.

## Covered

| Family | Components |
|---|---|
| Action | `Button`, `IconButton`, `ButtonGroup`, `SplitButton`, `Toggle`, `ToggleGroup`, `CopyButton` |
| Text entry | `TextInput`, `PasswordInput`, `OneTimeCodeInput`, `TextArea` (including bounded autosize), `MentionInput`, `RichTextEditor`, `NumberInput`, `TagInput`, `InlineEdit`, `SearchField`, `FindReplace`, `UploadList` |
| Choice | `Select`, `MultiSelect`, `TransferList`, `Cascader`, `Combobox`, `Checkbox`, `Radio`, `Switch`, `Slider` (horizontal and vertical), `SegmentedControl`, `ColorPicker`, `ColorSwatch` |
| Form | `FormField`, `SettingsRow`, `SettingsSection` |
| Navigation | `Tabs`, `Accordion`, `Collapsible`, `Breadcrumb`, `Sidebar`, `AnchorList`, `Pagination`, `Wizard`, `UndoHistory`, `Carousel` |
| Data | `List` (virtualized), `Flow` (virtualized), `Table`, `DataGrid` (virtualized), `TreeGrid` (virtualized), `BulkBar`, `Tree`, `KanbanBoard`, `DiagnosticsList`, `ImageList`, `Masonry` |
| Date and time | `Calendar`, `DateInput`, `RangePicker`, `TimeInput` |
| Content | `Markdown`, `AgentDocument`, `MessageList`, `ImageViewer`, `CodeView`, `TransportBar`, `BrowserPanel` (shell only), `LogStream`, `DiffView`, `ArtifactPreview`, `Terminal` |
| Display | `Icon`, `Badge`, `Tag`, `Avatar`, `AvatarGroup`, `Card`, `ListRow`, `Divider`, `ProgressBar`, `EmptyState`, `FailurePanel`, `StatusDot`, `StatusLine`, `Callout`, `Banner`, `StaleMark`, `PulseLoader`, `Skeleton`, `Spinner`, `BarLoader`, `LoadMore`, `RefreshVeil`, `ProgressCircle`, `StageProgress`, `StateView`, `OutcomePanel`, `DescriptionList`, `Timeline`, `HighlightedText`, `AnimatedNumber`, `MetricCard`, `Sparkline`, `PerformanceHud`, `MicroMark`, `Rating`, `Bubble`, `Plot`, `CandlestickChart`, `SankeyChart`, `LineChart`, `BarChart`, `AreaChart`, `ScatterChart`, `PieChart`, `StackedBarChart`, `RadarChart`, `GaugeChart`, `ChartLegend`, `Heatmap` |
| Overlay | `Overlay`, `Frost`, `Glass`, `Dialog`, `Drawer`, `Popover`, `Menu`, `ContextMenu`, `Menubar`, `CommandPalette`, `Tooltip`, `HoverCard`, `Toast`, `ToastLayer`, `NotificationCenter`, `Kbd` |
| Layout | `DesktopTitlebar`, `SplitPane`, `SplitTree`, `ScrollArea`, `ScrollFade`, `Toolbar`, `AspectRatio`, `Responsive`, `Grid`, `Container` |
| Shell | `Dock`, `StatusBar` |
| Keymap | `KeybindingRecorder`, `KeymapEditor` |
| Interaction | `Dropzone` |
| Filtering | `FilterBar` |
| Agent run and persona | `AgentAvatar`, `AgentActivityLine`, `AgentCard`, `AgentGroup`, `AgentRunIssues`, `ToolCall`, `StepList`, `ThinkingBlock`, `NodeGraph`, `GraphNode`, `NodeGroup`, `CanvasToolbar`, `Minimap`, `TraceView`, `SpanTimeline`, `AgentRoster`, `SubagentTree`, `AgentRunCanvas`, `PersonaPortrait`, `VoiceReactive`, `PersonaDialogue`, `FeedbackRating`, `PromptBuilder` |
| Permission and cost | `ApprovalPrompt`, `PermissionMatrix`, `CostMeter`, `ContextGauge` |
| Game experience | `PartyRoster`, `ObjectiveTracker`, `AbilityBar`, `RewardReveal` |
| Visual effects | `EffectParticles`, `CinematicEffect` |
| Structured data | `JsonView`, `Outline`, `SchemaForm` |
| Connections | `ServerList`, `OfferingCatalog` |
| Media | `AudioPlayer`, `AudioWaveform`, `VideoPlayer`, `ModelViewer` |

`Tooltipped` is an extension trait rather than a component: it attaches a
`Tooltip` to any element, and is covered wherever that `Tooltip` is.

`PasswordInput` and `OneTimeCodeInput` cover only product-neutral sensitive
entry. The `auth-sign-in` and `auth-verification` scenes show password,
one-time verification, passkey, organization sign-on, and recovery actions as
composition from generic primitives. Account models, provider policy,
credential storage, validation, networking, RPC, and authentication outcomes
remain with the caller.

`BrowserPanel` is listed with a qualification because it is one. It is the
chrome and the states around an embedded web view and it renders no web
content: that needs an engine, and a component library that pulled one into
every binary would be charging every host for a feature almost none of them
use. The host supplies the engine and the surface. What the panel does own is
the part hosts otherwise each get wrong — a build with no engine says so
instead of drawing a blank page, and Loading, Empty, Unavailable, Error, and
Ready remain five distinct answers.

`NodeGraph` places nothing. The caller positions every node, because where a
step belongs is a claim about the run rather than a fact about the component,
and a layout algorithm here would make that claim for every host at once. A
node may carry a caller-rendered thumbnail, whose pixels the graph neither
fetches nor decodes. `GraphInteraction::Inspect` permits only pan, zoom, and
selection proposals; `Arrange` additionally permits movement; `Edit` adds
deletion, connection, and disconnection. The caller remains authoritative for
the selection, topology, positions, and viewport shown on the next frame.

`CinematicEffect` is covered without making an animation runtime mandatory.
The `cinematic-effects` scene stages a resolved deterministic sample, explicit
runtime and invalid-archive fallbacks, and a reduced-motion poster. Component
tests drive exact sampling, frame ownership, RTL mirroring, typed semantics,
and diagnostic redaction. The optional pure-Rust adapter is separately tested
against archive and animation limits under `--all-features`; a default build
retains identical recipes and always-available particle fallback behavior.

## Systems, which span more than one component

A system is not a component. It has no builder of its own to place on a
screen; it is a contract several families implement, so it is covered when the
contract is documented, staged in a scene, and driven through simulated input
against every surface that implements it.

| System | Contract | Implemented by |
|---|---|---|
| Drag and drop (`gpui_kit::interaction::dnd`) | `docs/interaction.md` | `List`, `Tree`, `Tabs`, `Dropzone`, `DataGrid` (column headers), `Dock`/`DockTree` (panel headers, groups, split edges) |

Drag and drop is covered: the contract is written down, the scenes `drag-list`,
`drag-tree`, and `dropzone` stage it, and `crates/gpui-kit/tests/dnd.rs` drives
a simulated pointer through every surface above. `DataGrid` reorders its column
headers through the same system, driven in `crates/gpui-kit/tests/grid.rs`, and
`Dock` moves panels between regions through it, driven in
`crates/gpui-kit/tests/shell.rs`; `DockTree` additionally reports centre merges
into preserved empty stacks and four-way recursive split placement, driven in
`crates/gpui-kit/tests/dock_tree.rs`.

## One resize implementation

`SplitPane` is two panes and a divider; `SplitTree` is however many of those the
caller nests; `Dock` builds a `SplitLayout` from the regions that hold panels
and hands it to a `SplitTree`; `DockTree` projects the caller's recursive
`DockTopology` to that same layout. So every dock divider is the same divider a
plain split gives, with the same minimums and published travel range, and there
is one place where dragging a divider is implemented. Every dock stack header
is a `Tabs` strip for the same reason: dragging a panel is the drag system, not
a second one.

## Table or DataGrid

Both are covered and neither replaces the other. `Table` takes materialized
rows and lays all of them out; `DataGrid` takes a render closure and lays out
only the rows the viewport holds, which is what buys it column resizing and
reordering, a pinned group, selection over an incompletely loaded set, opened
rows, and cell editing. `docs/components.md` has the guidance on which to
reach for. A wide `DataGrid` uses one horizontal viewport for its header,
virtualized body, and summary, while a pinned leading group remains frozen at
the reading edge. Its remaining fit-to-content limit is stated rather than
faked: a double click on a column edge reports the request and lets the host
answer.

`TreeGrid` reuses that virtualized machinery for a caller-flattened hierarchy.
Its `tree-grid` scene and `crates/gpui-kit/tests/tree_grid.rs` cover bounded
materialization, TreeGrid/Row/GridCell semantics, row hierarchy metadata, and
logical disclosure/parent keyboard intents. It deliberately adds no second
flattening or cross-tree layout system; horizontal scrolling and a frozen
hierarchy column are inherited from `DataGrid` rather than reimplemented.

## Helpers, which the four-part rule does not reach

`field_shell` and `FieldState` draw the one border, background, and focus
treatment every editable control wears. `FocusTrap` keeps the keyboard inside
an open overlay. Neither renders on its own, so neither has a scene of its
own; both are exercised through every control and overlay that uses them.

## Out of scope, and why

- **The calendar itself.** `Calendar`, `DateInput`, `RangePicker`, and
  `TimeInput` are covered; the calendar system, the time-zone database, the
  locale, and the notion of today underneath them are not, and never will be.
  Correctness there is calendar, time-zone and locale work, not UI work, and a
  library that shipped a half-correct calendar would be worse than one that
  shipped none. So the components own no date arithmetic at all and read every
  fact from a host-implemented `DateAdapter`; `docs/datetime.md` is the
  contract. The reference calendar the scenes and tests run on is behind the
  `fixtures` feature, off by default, so it cannot be mistaken for a default.
- **Time formatting.** `Timeline` displays times and day headings, and
  formats neither: it takes strings the host has already put into words. The
  same reasoning that keeps the calendar out of this crate keeps
  “two minutes ago” out of it, and the seam is the adapter above the
  component.
- **Judging or applying a keybinding.** `KeybindingRecorder` captures a
  keystroke and `KeymapEditor` coordinates it with caller-owned command and
  binding identities. Whether it clashes with something, what provenance it
  has, and whether add, remove, or reset should be accepted needs the keymap,
  which the host owns. Both components render the caller's answer rather than
  inventing or persisting one.
- **Persisting a layout.** `SplitLayout` converts to and from plain records so
  a host can write it out, and this crate takes no serialization dependency to
  do it for them.
- **Language intelligence and document policy.** `Editor` fixes the shared
  `TextArea` buffer and geometry to a no-wrap source projection, then accepts
  revision-tagged caller highlights and one synchronous caller-owned
  indentation replacement. `RichTextEditor` projects the same editing
  invariants through styled blocks, alignment, lists, diagnostics, semantics,
  and a formatting toolbar. Grammar/LSP facts, folding and multi-caret policy,
  persistence, collaboration, URL policy, and conversion to a product
  document format remain host work. Syntax colouring still stops at four
  built-in classes on the eight languages `content::highlight` knows. A host
  that has a grammar installs its own spans, and those facts win.
- **Doing what a document says.** `Markdown` draws HTML as the characters
  somebody wrote, reports a link rather than opening it, and names an image
  rather than fetching it. There is no HTML renderer here, no URL policy, and
  no network; `docs/content.md` states why each of those is a refusal rather
  than a gap, and what a host has to supply instead.
- **Delivering a message.** `MessageList` renders five delivery states and
  reports a retry. Sending anything, deciding what a resend means, and knowing
  whether a message was really read are the transport's, and this crate has no
  transport.
- **Capturing a voice or advancing dialogue.** `VoiceReactive` maps a finite
  normalized host sample to a complete meter and `PersonaDialogue` owns the
  portrait, safe Markdown, streaming, and choice composition. Microphone
  access, recognition, synthesis, playback, expression inference, and choosing
  or applying the next turn remain host facts and capabilities.
- **Running a game or deciding an outcome.** `PartyRoster`, `ObjectiveTracker`,
  `AbilityBar`, and `RewardReveal` own reusable character/game presentation,
  malformed-topology refusal, typed intents, RTL, reduced motion, semantics,
  and policy-resolved effects. Combat formulas, cooldown clocks, input maps,
  objective progression, reward eligibility, inventory mutation, persistence,
  networking, and asset fetching remain authoritative host systems. The UI
  never converts a click into a successful action or an item into an owned one.
- **Fetching or decoding an image.** `ImageViewer` frames, zooms and pans an
  element the host hands it, and names the source when the host hands it
  nothing. There is no network here and no decoder, so the pixel size of a
  source is a caller input like every other fact this library cannot hold;
  a viewer given none says the size is unknown rather than reporting the box
  it drew.
- **Media policy.** `TransportBar` reports play, pause, seek, volume, mute,
  speed and a track step, and applies none of them. `AudioPlayer` and
  `VideoPlayer` ask a `MediaTransport`; `PlatformMediaTransport` currently
  implements it with AVFoundation on macOS and Media Foundation on Windows.
  Linux GStreamer and Web HTML media adapters are planned work in
  `docs/foundation-roadmap.md`, not permanent no-backend policy. Playlist and
  queue ownership, URL/auth policy, DRM, subtitle/track policy, output-device
  policy, custom cache/retry and capture remain host responsibilities.
- **Reading a 3D model that is not glTF.** `ModelViewer` reads the subset of
  glTF 2.0 stated in `docs/components.md` and refuses everything else, without
  a scene-graph dependency, a material system, or a texture pipeline. Other
  formats, materials, animation and skinning are not gaps to be filled here:
  a document that needs them is one an application converts before it arrives.
- **Inventing a scale, a locale, or a series policy.** A chart still does not
  own data. Axes, ticks, domains, stacking, aggregation, and "2 minutes ago"
  are facts the host already has or can compute; Box paints them. Line and bar
  geometry now enters, updates, and exits by caller business id; area fill,
  exact-text crosshair tooltips, keyboard traversal, and stale-data retention
  are component behavior rather than downstream drawing work. The old Kit-era
  refusal of charts themselves is lifted: line, bar, area, and distribution
  surfaces are in scope as application primitives. A
  business-intelligence toolkit — live query, crossfilter, annotation
  layers, financial overlays — is still a product, not a substrate.
- **Owning a platform picker.** Colour, file, and print dialogs that replace
  the operating system stay out. In-window colour wells, dropzones, and
  print-preview chrome that report a choice are in scope; they do not
  become the system dialog.
- **Menu bar and window chrome.** Owned by the platform window, not by a
  component tree.
- **Product-specific marketing policy.** `Carousel`, `Rating`, and `Bubble`
  are now neutral, caller-owned primitives. Product-specific autoplay,
  recommendation, delivery, and conversation policy remains outside the Kit.

## Framework and platform limitations

The component coverage review above has no remaining MUI or shadcn component
gap. The limitations below are intentionally kept separate: they are
framework, renderer, platform, or host-policy boundaries, not missing public
components. Out of scope above means "will not be built, and here is why";
these entries record capabilities that need a framework or platform owner
without pretending that a component-local workaround is complete support.

### Shared presentation tiers and semantic token authority, 2026-08-27

`Theme::variant_colors` resolves the seven shared tiers (`Filled`, `Light`,
`Outline`, `Subtle`, `Default`, `Transparent`, `White`) against a palette
group, a semantic role, or an explicit paint, and `Button`, `IconButton`,
`Badge`, and `Tag` accept `.variant(..)` / `.color(..)` on top of their
existing vocabularies without moving a pixel of the defaults. Other coloured
surfaces — `Callout`, `Banner`, `StatusDot`, `StatusLine`, `ProgressBar`, and
`ProgressCircle` — use the same semantic theme roles through their own
state-oriented APIs. They intentionally do not expose arbitrary per-instance
variant colours: tone is a fact, while variant tiers are a reusable choice for
surfaces such as buttons, badges, and tags. This keeps all actual paint values
in the theme token authority without creating a second override vocabulary.

### A product backdrop that the bundled ramp still cannot 1:1, 2026-08-18

`Surface::Backdrop` and multi-layer elevation close the two kit-side holes a
downstream prototype hit: there is now a plane darker than `canvas` for a
card to sit on, and an elevation step can carry both a contact shadow and a
wide one. The bundled Studio themes take the new roles without retuning the
five existing surfaces, so the gallery does not move.

What is still not 1:1 is the light appearance. The prototype paints the page
`#c9c9d1`, the well `#dcdce2`, and the card `#ffffff`. A white `panel`
leaves `raised` nowhere to go — it would need L\* ≥ 103, which is outside
the colour space — and dropping the page to `#c9c9d1` forces `sunken`
darker still, at which point the light text and semantic rungs (tuned for a
shallow well) fail the contrast floor in eleven pairs. Retuning that light
foreground and semantic scale is a separate change; this one does not do
it. The library still groups with colour rather than with a line, and a
card still does not draw a border: lines stay reserved for focus, invalid,
and drop.

### Typography and visual polish audit, 2026-08-11

Before changing typography, eight independent reviewers inspected all 98 Linux
headless scenes in both Studio themes: 196 images, each 1840×2000. Every image
was checked for (1) type hierarchy and accidental GPUI-default text, (2) line
height and icon/chip baselines, (3) token spacing rhythm, (4) dark-theme
contrast, (5) visibly distinct Loading, Empty, Unavailable, Error, and Ready
states, and (6) clipping, overflow, and unconstrained full-width content. This
is the named pre-change inventory; `clean` means no defect was visible under
that rubric, not that every possible interaction was exercised.

| Scenes | Pre-change finding |
|---|---|
| `accordion` | Medium: disabled title and detail are too faint in both themes. |
| `actions` | High: dark toolbar icons, shortcuts, menu outline, and separators nearly disappear. |
| `agent-roster` | Clean. |
| `agent-run-canvas` | Clean. |
| `anchor-list` | Medium: dark explanatory copy is nearly invisible. |
| `animated-number`, `aspect-ratio` | Clean. |
| `approval` | Medium: dark secondary action is weak; the focus ring and labelled divider need clearance. |
| `audio-player` | High: media-control baselines diverge and inactive rails/separators disappear, especially in dark. |
| `auth-sign-in`, `auth-verification` | Medium: dark banner, placeholder, link, and verification-cell boundaries are too weak. |
| `badge` | Medium: several dark semantic foreground/background pairs are hard to distinguish. |
| `breadcrumb` | Low: the light-theme slash sits above the text baseline. |
| `browser-panel` | Medium: the five states are distinct, but long Ready content is hard-clipped. |
| `button` | Medium: control sizes rely on boxes more than a coherent type ladder; dark disabled labels are weak. |
| `calendar` | Medium: dark adjacent/disabled dates and navigation are too faint; fixture explanation uses default-sized text. |
| `card` | Medium: a two-line card stretches across nearly the full canvas. |
| `cascader` | High: an unconstrained trigger and narrow popup disagree; dark disabled rows disappear. |
| `choice` | High: dark unchecked and disabled controls are almost invisible. |
| `code-view` | Medium: an intentionally long line clips without an equally visible horizontal-scroll affordance in both themes. |
| `collapsible` | High: the dark managed section is nearly invisible. |
| `command-palette` | Medium: dark section labels, secondary commands, and keycaps are too faint. |
| `content` | High: the states are truthful, but dark pinned/filter/empty affordances and progress rails disappear. |
| `context-menu` | Medium: fixture copy falls back to large default text; dark metadata is weak. |
| `conversation` | High: duplicated long message groups and an unconstrained reading column obscure the intended scroll states. |
| `copy-button` | Low: success is repeated as adjacent `Copied` labels. |
| `cost-meter` | High: dark labels, refusal reasons, and unknown values are nearly invisible. |
| `data-grid` | Medium: chip baselines and compact type are inconsistent. Dark row hover/selected are now separate washes; managed rows no longer fade the whole row. |
| `data-grid-editing` | Medium: editor and cell baselines differ and the focus edge collides with a grid line. |
| `date-range` | Medium: disabled/range dates lack contrast and adjacent-month dates lack hierarchy. |
| `date-time` | Medium: light separators sit low and error copy touches the field. |
| `detail` | High: labels, unknown/not-applicable values, and dark metadata are too faint. |
| `diagnostics-list` | High: Hint and explanatory text nearly disappear; chip icon baselines diverge. |
| `dialog` | Low: light is clean; only masked background copy is extremely faint in dark. |
| `diff-view` | Low: top clearance and the dark split divider are weak. |
| `document-tabs` | Medium: light helper copy and dark inactive close controls are too faint. |
| `drag-list` | Clean. |
| `drag-tree` | Medium: light is clean; dark status copy nearly disappears and preview content sits high. |
| `drawer` | Medium: light is clean; the dark unchecked box is difficult to find. |
| `dropzone` | Medium: helper copy is too faint and states move vertically when a detail line is absent. |
| `failure-panel` | Medium: top copy clips in light; secondary failure detail is weak and icon/text centring differs. |
| `filter-bar` | Low: light Add-filter/chip baselines differ; dark is clean. |
| `find-replace` | High: dark explanatory copy disappears, controls are over-wide, and count/arrows are crowded. |
| `form` | Medium: the Region popup covers field help; dark unselected text is still faint. Errors now clear the field. |
| `frost` | High: dark frost lacks enough edge/surface evidence; light needs bottom clearance and consistent stripes. The edge evidence a frosted surface is missing is what `glass` carries; `Frost` stays frosted deliberately, being the material every renderer can produce. |
| `glass` | Medium: Metal, WGPU (native and web), and DirectX implement the same dual-source material. Every surface snapshots the framebuffer at its exact paint order; a positive blur derives a frosted source, while the original sharp snapshot remains available to the refracted rim. `Liquid` and `Lens` therefore default to clear (`blur = 0`) without losing optics or burying them under an ordinary source-over fill, and `Frosted` alone defaults to `effect.glassFrostBlur`. The spherical profile, 45%-of-bevel displacement cap, subtle channel dispersion, transmission gain, additive lift and one-pixel hairline agree across the three shaders; bevel depth resolves from each control's short edge, and a fused group uses its constituent panes rather than the union width. `track_pointer`, `pressable`, `adaptive`, and eight-lobe `GlassGroup` remain supported; adaptive tint is an explicit readability policy rather than part of the clear material. Probes read the actual source used for the interior (sharp for clear, blurred for frost) one frame later. WGPU and DirectX both retain sharp optics and the probe when the per-frame gaussian budget cannot fund the requested blur, removing their previous over-budget divergence. The scene now carries one-pixel rules behind Frosted, Lens and Liquid so sharp-rim versus scattered-interior behavior is reviewable. macOS and Windows baselines are authoritative; the Linux baseline set remains retired. |
| `hover-card` | Medium: helper and body copy are too faint, especially in dark. |
| `ide-shell` | High: dark shell height leaves a large void and Empty/Unavailable claims conflict. |
| `image-viewer` | High: metadata and disabled controls are too faint; the third viewer breaks the first two viewers' grid/container rhythm. |
| `inline-edit` | High: dark helper text disappears, errors touch fields, and the short value gets an over-wide editor. |
| `input` | High: read-only/disabled values are made unreadable rather than distinctly unavailable. |
| `json-view` | Low: comment columns and selected-row extent are inconsistent; dark null/comment values are weak. |
| `kbd` | Clean. |
| `keybinding` | Medium: unbound/help/error text is faint and keycap/icon/row baselines vary. |
| `keymap-editor` | High: `Defaults` and host-managed copy use GPUI's large default; chips, conflict/source text, and actions do not share a row grid. |
| `list` | Light is clean; low in dark for title/list clearance and missing continuation affordance. |
| `loading` | Clean. Every indicator paints from the neutral `color.loader.*` roles, the placeholder is held inside a loudness band by the token gate, and each of the six is a different picture rather than a different arrangement of the same one. |
| `log-stream` | Medium: level/inline chips sit off baseline and small status copy is faint. |
| `markdown` | Medium: the dark unfetched state disappears; truncated repetition and embedded-block widths are unclear. |
| `menu` | Low: shortcut gaps/insets vary and the dark section label is weak. |
| `menubar` | Medium: shortcuts, anchors, item gaps, and dividers do not share one baseline/rhythm. |
| `model-viewer` | High: a dark labelled divider crosses its label; metadata and disabled controls are too faint. |
| `motion-flip` | Low: light caption contrast/button padding and dark badge centring need correction. |
| `motion-state` | Medium: progress tracks disappear and the dark segmented strip stretches beyond its content. |
| `node-graph` | High: edges cross labels, loose labels lack anchors, and dark node metadata is too faint. |
| `notification-center` | High: unread markers move between action/close slots and rows without detail collapse. |
| `offering-catalog` | Medium: type badges and source columns are inconsistent; source/banner copy is faint. |
| `overlay` | Low: dark secondary-button separation is weak. |
| `pagination` | High: light previous/first controls look disabled away from the first page. |
| `permission-matrix` | Low: wrapped policy detail breaks row rhythm and dark secondary copy is weak. |
| `popover` | Low: light unchecked-box contrast and optical centring are weak. |
| `progress-circle`, `reading-direction` | Clean. |
| `schema-form` | Medium: dark fields and placeholders disappear; required markers drift off baseline. |
| `scroll-area` | Medium: the final line is cut in half and the light thumb is faint. |
| `scroll-fade` | Clean. |
| `scroll-shadow` | Medium: the first line is cut in half and the dark bottom shadow is weak. |
| `search-field` | Low: light hit-chip outlines are too faint. |
| `server-list` | Low: expanded error spacing is inconsistent and light helper copy is weak. |
| `settings` | Medium: unavailable explanation is made unreadable, especially in dark. |
| `sidebar` | Medium: dark managed and workspace metadata nearly disappear. |
| `sparkline` | Medium: dark minimum/maximum values are too faint. |
| `split-pane`, `split-tree` | High: dark dividers and drag handles nearly disappear. |
| `status` | Medium: two short status messages stretch nearly the full canvas. |
| `step-list` | Medium: dark status/detail text is weak and the status column is too distant. |
| `table` | Low: the scene now shows ready, a stale refresh, and empty. Managed rows keep their badges; hover and selected are separate washes. |
| `tabs` | Medium: scene body falls back to large default text and dark disabled tab is too faint. |
| `textarea` | High: dark placeholders, disabled text, and field surfaces disappear. |
| `thinking` | Medium: dark secondary/empty copy is weak and short rows stretch full-width. |
| `toast` | Low: bottom clearance and dark close-control contrast are marginal. |
| `toggle` | High: dark disabled and unselected states nearly disappear. |
| `tool-call` | Clean. |
| `toolbar` | Medium: scene body falls back to default text; dark controls/boundaries and group spacing are weak. |
| `tooltip` | Medium: trigger and tooltip have nearly the same visual role and no strong pointing relationship. |
| `transport` | High: dark rails, speeds, status copy, and unknown duration disappear; control baselines and mute forms diverge. |
| `tree` | Medium: dark ignored state is too faint and selected-row extent is ambiguous. |
| `tree-grid` | Medium: dark structure disappears and disclosure/icon columns do not reserve consistent slots. |
| `upload-list` | High: dark queued/refusal explanation disappears; progress/action columns and icon sizes vary. |
| `video-player` | Medium: volume controls do not share a baseline; dark rails/help disappear and the player lacks a balanced width strategy. |
| `wizard` | High: dark step details and Back boundary disappear; horizontal steps lack a shared title baseline. |

This pass does not accept a root-level inherited font as a fix. A component can
be embedded beneath any host text style, so each element that emits text must
choose its own complete `TypeScale` step. The typography gate records that
contract. It also does not turn disabled content invisible, treat a refusal as
empty data, invent a decoder/browser/graph layout, or add application policy to
make a fixture look fuller.

#### What closed the table

The table above stays as the pre-change inventory. What closed most of it was
not per-scene patching but two machine-checked rules in the token layer, each
of which had been failing silently in every theme:

- **Surface separation.** Every nesting step — canvas to panel, panel to card,
  card to raised, and the overlay against what it covers — now has to differ by
  at least three CIE L\* points, in the right direction, or
  `TokenDocument::validate` refuses the theme. The dark neutral ramp was
  respaced and the light one retuned to pass it. That is what made the card,
  table, dock and popup boundaries in dozens of the `High` rows visible;
  `contrast.rs` and `docs/token-model.md` carry the contract.
- **Tone distinction.** `muted`, `faint`, `placeholder` and `disabled` were
  three different facts wearing one grey in dark, and an inverted ladder in
  light, while every foreground/background contrast pair passed. Each rung now
  has to differ by three L\* measured as distance from the canvas, so one rule
  holds in both appearances. That is what made the disabled control, the
  placeholder, the divider and the explanatory line in the remaining `High`
  and `Medium` rows tell themselves apart.

`Card`, `CardHeader` and `StyledExt::card_surface` replaced three incompatible
hand-rolled card shells, which is what closed the surface-rhythm findings in
the agent, game and notification rows.

After recapturing all 216 macOS baselines against the retuned themes, the rows
that still showed a defect were fixed individually:

| Scene | What was left, and what was done |
|---|---|
| `keymap-editor` | `Defaults` was a step larger than its peer `Current bindings`; both are now `Label`. |
| `ide-shell` | The `Problems` panel carried a badge counting three problems and one string that claimed Unavailable and Empty at once. It now makes one claim and counts nothing it cannot list. The shell also fills the scene rather than leaving the lower third empty. |
| `notification-center` | The unread marker sat in the trailing flow, so it landed beside the action on one row and beside the close control on the next. It is now a fixed slot beside the title. |
| `node-graph` | An edge label was drawn six pixels from the midpoint that also carries the disconnect chip, so the two overlapped. The label now clears the chip's radius. |
| `cascader` | The trigger stretched to the scene while its popup kept its own width. The scene now gives the trigger the width of the surface it opens. |

This re-review was one reviewer over the 216 recaptured macOS images, not a
repeat of the eight-reviewer pass, and it read the rows the table had flagged
most closely. Three findings were looked at and deliberately left: a
conversation and a diff still fill the width they are given, because the
reading measure is the caller's; the transport's mute control keeps a different
form when muted, because that difference is the state; and a scrolled popup
still shows a partial row at its clip, because that is what being scrolled
looks like.

### Motion framework boundaries

The primitives in `docs/motion.md` cover a value moving from one state to
another, motion that is interrupted, composed or driven by a gesture, and
springs described as a duration and a bounce. The remaining entries are
renderer/framework boundaries, not missing catalog components.

| Framework boundary | Why it matters |
|---|---|
| Shape in `flip` | `Flip::shape` interpolates radius, border width, border colour and background over the same spring that carries position and size, and `Shaping::shaped` applies the result, so a row becoming a card travels between the two forms. The caller states both forms and applies what comes back, because `Flipped` wraps an element it did not build and cannot reach the style inside it. What is still not interpolated is anything with no numeric path between the two forms — a shadow set, a gradient, or a change of element kind. |
| Overscroll | `motion::rubber_band` damps a pull past a boundary, but nothing in the library overscrolls: a `ScrollArea` stops dead at its end, so the band is available to a caller and used by no component here. |

### Media capabilities beyond native playback

`PlatformMediaTransport` closes the ordinary macOS/Windows decoder, output,
clock and native-view gap. `MediaCapabilities` now makes audio/video, seek,
volume, rates, native-track and output-selection support explicit at runtime;
unsupported controls are absent or inert. `MediaErrorKind` preserves no-backend,
invalid-source, open, playback and refusal categories through the Kit seam, so
downstream code never parses a platform diagnostic. The remaining media work is additive capability,
not basic playback: playlist/queue ownership; audio, subtitle and accessibility
description track selection; audio output-device switching; DRM; application
cache/retry and authentication policy; picture-in-picture and fullscreen;
recording/capture; frame-accurate extraction plus waveform/thumbnail generation;
and Linux/Web backends. A platform codec refusal remains `no-backend`, while an
unreadable source remains `failed`; neither is converted to empty media.

### Components

The internal inventory is not enough to discover a family that does not exist,
so the component catalog is also compared against a mature external baseline.
The 2026-08-27 primary baseline is the official
[Material UI component overview](https://mui.com/material-ui/all-components/).
That page lists 59 entries: Inputs (13), Data display (10), Feedback (6),
Surface (4), Navigation (9), Layout (5), Lab (2), and Utils (10). The current
[MUI component index](https://mui.com/components/) also exposes newer entries
such as `Number Field` and `InitColorSchemeScript`; because the two official
indexes are not yet identical, this document records the dated overview rather
than pretending that a component count is a stable quality score.

[MUI X](https://mui.com/x/introduction/) is tracked separately: its current
stable advanced families are Data Grid, Date and Time Pickers, Charts, and Tree
View. Scheduler is listed as Preview, and advanced Pro/Premium features are
not silently counted as free Core coverage. The mapping compares
product-neutral behavior, not React, DOM, CSS, or API spelling:

The second coverage baseline is [shadcn/ui](https://ui.shadcn.com/docs), whose
official description is a set of accessible components **and a code
distribution platform**. Its current [component catalog](https://ui.shadcn.com/docs/components)
lists 64 first-party entries. That is a valid open-source component coverage
baseline alongside MUI: the source-distribution model changes ownership and
customisation, not whether an input, overlay, data, or layout capability needs
to exist. Community registries remain outside this dated first-party count.
GPUI Box already follows the important part of that model — callers own data
and actions, while the Kit owns reusable visual and interaction contracts — but
those contracts must still be complete and machine-tested like a maintained
library. The count is not added to MUI's count because names and granularity
differ; every entry is instead mapped by behavior below.

| shadcn family | Entries | Strong equivalent | Partial / foundation | Unimplemented |
|---|---:|---|---|---|
| Interaction and forms | 20 | `Button`, `Button Group`, `Checkbox`, `Combobox`, `Calendar`, `Date Picker`, `Field`, `Input`, `Input OTP`, `Radio Group`, `Select`, `Switch`, `Textarea`, `Toggle`, `Toggle Group`, `Label`, `Slider` | `Input Group`, `Native Select`, `Questionnaire` | — |
| Feedback and surfaces | 20 | `Accordion`, `Alert`, `Aspect Ratio`, `Avatar`, `Badge`, `Bubble`, `Card`, `Carousel`, `Collapsible`, `Dialog`, `Empty`, `Hover Card`, `Message Scroller`, `Progress`, `Sheet`, `Skeleton`, `Toast` | `Alert Dialog`, `Attachment`, `Message` | — |
| Navigation and overlays | 13 | `Breadcrumb`, `Command`, `Context Menu`, `Drawer`, `Dropdown Menu`, `Menubar`, `Pagination`, `Popover`, `Sidebar`, `Tabs`, `Tooltip`, `Kbd` | `Navigation Menu` | — |
| Data, layout, and foundation | 11 | `Chart`, `Data Table`, `Direction`, `Item`, `Separator`, `Table`, `Typography`, `Resizable`, `Scroll Area`, `Spinner` | `Marker` | — |
| **Total** | **64** | **56** | **8** | **—** |

This normalized shadcn mapping is intentionally stricter than a name search.
For example, `Message Scroller` maps to the virtualized `MessageList`,
`Resizable` maps to the shared `SplitPane` divider, and `Direction` maps to the
layout-direction foundation. `Native Select` is only partial because GPUI's
`Select` is a themed popup control. `Input Group` and `Questionnaire` remain
composition patterns whose product-specific policy is intentionally
caller-owned. `Slider` and multiple selection are full behavior matches
through the orientation-aware `Slider` and controlled `MultiSelect` contracts.
`MultiSelect` is an intentional GPUI extension for the multiple-selection use
case; it is listed in the library coverage above and in the completed
capabilities below, but it is not counted as an additional shadcn entry.

| MUI family | Entries | GPUI Box mapping | Verdict |
|---|---:|---|---|
| Inputs | 13 | `Autocomplete` → `Combobox` (single-answer); `Button`/`Button Group` → `Button`/`ButtonGroup`; `Checkbox` → `Checkbox`; `Radio Group` → caller-composed `Radio`; `Select` → `Select` (single-answer); multiple selection → `MultiSelect`; `Slider` → orientation-aware `Slider`; `Switch` → `Switch`; `Text Field` → `TextInput`/`NumberInput`/`TextArea`; `Toggle Button` → `Toggle`/`ToggleGroup` | Broad coverage; Floating Action Button remains a deliberate desktop composition. |
| Data display | 10 | `Avatar`, `Badge`, `Tag`, `Divider`, `Icon`, `List`, `Table`, `ImageList`, `Tooltip`, foundation text/type scale | Strong coverage; typography and icon families are correctly foundation/catalog concerns rather than duplicate leaf components. |
| Feedback | 6 | `Callout`/`Banner`/`StateView`, `Overlay`, `Dialog`, `ProgressBar`/`ProgressCircle`, `Rating`, `Skeleton`, `Toast`/`ToastLayer` | Strong product-neutral coverage, with feedback states retained as caller-owned facts. |
| Surface | 4 | `Accordion`, `DesktopTitlebar`/`Toolbar`, `Card`, theme surface recipes plus `Frost`/`Glass` | Strong coverage by composition and complete surface recipes; no need for a second `Paper` shell. |
| Navigation | 9 | `Tabs`, `Breadcrumb`, `Drawer`, `Menu`, `Pagination`, `Wizard`/`StepList`, `Sidebar` | Strong desktop coverage; Bottom Navigation and Speed Dial are mobile-oriented patterns, not missing desktop primitives. |
| Layout | 5 | `Responsive`, `SplitPane`, `SplitTree`, `AspectRatio`, `Grid`/`Container` | Strong coverage; measured breakpoints preserve source order and caller-owned content. |
| Lab | 2 | `Timeline`, `Masonry` | Strong coverage with caller-measured tile heights and a documented non-virtualized boundary. |
| Utils | 10 | `Popover`, `Overlay`, positioner/focus/portal internals, `TextArea`, motion system, `Responsive` | Foundation coverage is present; these are not all user-facing components and should not inflate the public component count. |
| MUI X | 4 stable families | `DataGrid`, `Calendar`/`DateInput`/`RangePicker`/`TimeInput`, chart family, `Tree`/`TreeGrid` | Strong advanced coverage, with the documented non-virtualized `Table`/`Tree` limitation. |

Several MUI names intentionally resolve to existing primitives instead of new
public types. `Paper` is a surface recipe, `Typography` is the typed text
foundation, `Stack` and `Box` are ordinary GPUI composition, `Radio Group` is
the caller's group of `Radio` controls, and Modal/Popper/Portal/Transitions
are overlay and motion infrastructure. This is not a gap: duplicating them as
thin wrappers would create another style system. Conversely, a component is
not marked covered merely because a similarly named primitive can be composed;
the state, input, semantic, and caller-owned event contracts must also line up.

The MUI- and shadcn-derived component review is complete as of 2026-08-27.
Every component below has a public contract, a scene exhibit, simulated
behaviour coverage, semantic ids, and a documentation entry:

| Completed capability | Implementation boundary |
|---|---|
| Multiple selection / `MultiSelect` | Searchable controlled listbox with stable option identities, removable chips, disabled options, keyboard toggling, and caller-owned selected values. |
| Scalar `Rating` | Controlled whole/half precision rating with pointer and keyboard input, clearable/unrated and disabled states, and accessible value semantics. |
| `TransferList` | Controlled source/target assignment panes with filtering, truthful counts, individual selection, disabled items, move intents, and no component-owned mutation. |
| Horizontal and vertical `Slider` | One orientation-aware contract keeps track geometry, pointer mapping, keyboard direction, marks, range fill, and AccessKit orientation aligned. |
| Declarative `Grid` / `Container` | Token-backed measured breakpoints, columns, spans, gaps, readable widths, and source-order semantics. |
| `ImageList` | Stable selectable media tiles with measured responsive columns and token-backed layout; media phase remains caller-owned. |
| `Masonry` | Stable variable-height tiles placed into the shortest measured column with responsive token-backed columns and gaps. |
| Auto-growing `TextArea` | Bounded autosize measures shaped visual rows, preserves editor state, and scrolls after the maximum row bound. |
| Source `Editor` | One no-wrap `TextArea` projection adds hard-line numbers/geometry, revision-safe caller highlights, and caller-owned indentation without duplicating buffer, caret, IME, history, paint, or hit testing. |
| `Bubble` | Neutral caller-owned message surface with placement, grouping, max width, safe content, and optional actions. |
| `Carousel` | Controlled stable-item track with previous/next/direct selection, keyboard navigation, clipping, reduced motion, and truthful state phases. |

Ordinary `MultiSelect`, `TransferList`, `ImageList`, and `Masonry` instances
are intentionally non-virtualized components. Their option/tile sets are
caller-owned and should be kept to a bounded presentation size; very large
datasets must use `List`, `DataGrid`, or another caller-owned virtualized
surface. This is a documented performance boundary, not an unimplemented
component API.

These are distinct from cross-library or product patterns such as a
confirmation popover, product tour, sticky/affixed content, QR code, or
watermark. They may be useful additions, but they are not MUI Core entries and
must not be reported as MUI coverage gaps. Their priority should be decided by
desktop product demand and by whether GPUI has the required framework primitive,
not by inflating a benchmark score.

Document tabs, `SearchField`, `FindReplace`, `NotificationCenter`, `CodeView`,
and `UploadList` are covered above. `FailurePanel` presents an ordinary
caller-owned failure; a render error boundary is not implementable while GPUI
rendering is infallible and its draw arenas are not unwind-safe.

`Toggle`, `ToggleGroup`, `Collapsible`, `HoverCard`, `Menubar`, `CopyButton`
and `AspectRatio` are covered above. Three of them were built on top of what
was already here rather than beside it, which is the whole reason they are
small: `Collapsible` is an `Accordion` with one section, `Menubar` is a row of
`Menu` views with the row's own three behaviours added, and `Toggle` is a
`Button` that publishes a checked state. Two of them state a limit rather than
inventing an answer, and `docs/components.md` carries both — what `CopyButton`
can and cannot know about the clipboard, and what a hover card's grace period
is for. `Cascader`, `AnchorList`, and `DiagnosticsList` are also covered above;
they compose the existing popover/menu, navigation, list, filter, badge, and
status vocabulary instead of creating parallel application infrastructure.

Mentions in a text field and settings search now share Kit contracts.
`MentionInput` owns trigger detection, querying, stable candidate focus,
caret-anchored presentation, and completion insertion while callers own
candidate retrieval, stable identity, exact replacement text, and the semantic
attachment of accepted identity to plain text. `SettingsList` filters sections
and rows through the installed locale matcher, includes visible row copy and
explicit caller-authored aliases/control vocabulary, preserves the settings
page's familiar order, counts the result, and presents a distinct no-match
state. `UndoHistory` covers the caller-owned revision list and reports restore
intents without keeping or mutating an undo stack. The non-rendering
`reactive::History<T>` is the bounded undo/redo record store beneath callers
that need one: ignored records are refused by the model, a divergent push
clears redo, and the caller remains responsible for applying every record.
`PerformanceHud` covers live diagnostics without turning the observer into the
workload: framework `FrameTimingMonitor` filters and bounds existing per-window
draws, while the controlled Kit view presents the caller's latest summary and
never schedules another frame.

Application forms now cover date, time, range, files, and repeating sections.
Date facts come from `DateAdapter`; file admissibility and display names come
from `SchemaFilePolicy`, while the host still opens the OS picker. Repeating
sections keep stable visual identity and nested values without owning product
data. Host-declared conditional rules resolve into `FieldVisibility`: visible,
or hidden with `Omit` / `Include` submission policy. Hidden fields and subtrees
cannot become invisible validation blockers. `values()` remains the complete
held-value inventory while `submission_values()` applies that explicit policy;
the form never owns the condition or removes caller data.

`LineChart` and `BarChart` now cover the cartesian presentation gap with keyed
motion, area fills, pointer and keyboard crosshairs, exact host-formatted text,
and stale-data retention. Domains, ticks, aggregation, and queries remain host
facts rather than drawing work. `Plot` supplies the lower generic measured
frame and semantic mark traversal. `CandlestickChart` and `SankeyChart` render
caller-normalized OHLC and flow geometry through that boundary; neither owns a
market scale, topology algorithm, value transform, or financial vocabulary.

Agent and game applications now have product-neutral run, persona, party,
objective, ability, and reward families rather than one-off downstream cards.
`ApprovalPrompt`, `PermissionMatrix`, `CostMeter`, and the context gauge keep
approval, scope, and estimated costs explicit. `OfferingCatalog` covers Tool,
Skill, and Resource results together rather than creating separate
`ToolCatalog` and `SkillCard` APIs; per-server attribution is part of every
result because two servers may offer the same name. `LogStream`, `DiffView`,
`JsonView`, `SchemaForm`, `ServerList`, `OfferingCatalog`, and `NodeGraph` are
covered above.

`NodeGraph` remains the lower-level graph whose caller supplies coordinates.
`AgentRunCanvas` is the high-level run composition for a caller that has only
typed topology: `AgentRunLayout` supplies its deterministic layered,
RTL-aware placement. A product-specific force layout or manually persisted
coordinates can still use `NodeGraph` directly without creating a second run
presentation contract downstream.

### Capabilities that are not components

| Gap | Why it matters |
|---|---|
| Grayscale glyph compositing | Metal now applies the same DirectWrite-derived contrast/gamma alpha correction as Direct3D and WGPU when tinting atlas coverage. Glyphs remain reusable grayscale masks, so destination-aware Core Text / AppKit smoothing is still not available. macOS stays grayscale-only. Linear atlas sampling can still soften 12–13px geometric faces; changing that filter would also affect monochrome SVG icons that share the same sprite path. |
| Complete copies across unmounted text | GPUI now coordinates one grapheme-safe selection across separately mounted `StyledText` participants in caller-declared reading order, with pointer capture, keyboard copy/select-all, AccessKit text runs, and overlay scope isolation. `AgentDocument`, `CodeView`, `Markdown`, `LogStream`, `DiffView`, and `HighlightedText` participate. A selection crossing virtualized rows copies the mounted text and reports that it is incomplete; whole-value component copy actions remain the path to content GPUI never laid out. |
| Text range highlighting | `HighlightedText`, `LogStream`, `CodeView` and `DiffView` render caller-supplied ranges while constructing their text. GPUI still has no API that marks a substring of an arbitrary already-rendered text element, which blocks a generic find-in-page overlay. |
| Writing direction | `LayoutDirection` supplies logical row order, start/end spacing and borders, text alignment, directional glyph mirroring, and reading-order keyboard traversal across controls, navigation, menus, calendars, trees, structured views, and schema forms. Unicode bidi shaping keeps mixed Arabic/Hebrew, Latin, punctuation, and numbers in logical order. Host-owned localized copy, locale formatting, and a larger language-specific bidi corpus remain integration work rather than component geometry. |
| Number, date, and quantity formatting | `NumberAdapter` owns every library-authored numeric shape — grouped counts and decimals, editable parsing, plural category, count-of-total, percent, multiplier, dimensions, ordinals, signed deltas, lower bounds, and affix placement. `Strings` owns every phrase and its zero/one/two/few/many/other variants. Dates remain the parallel `DateAdapter` contract. See "Numbers a catalogue cannot fix alone" below. |
| Assistive technology gaps | Basic semantics, grapheme-based editable and read-only text runs, shared shaped character/caret geometry, selection actions, explicit live-region properties, same-window labelled-by/described-by relationships, and deferred-overlay active descendants now reach GPUI's AccessKit platform tree. macOS and Windows natively verify relationship-derived field name/help and editable character/caret geometry; Windows additionally verifies ValuePattern editing and MenuItem focus/invocation/lifetime. Cross-tree completion focus is deterministic only. Native-child handoff, platform live-event verification, remaining Windows overlay/event sessions, and Linux AT-SPI validation are active foundation work; see `docs/accessibility.md` and `docs/foundation-roadmap.md`. |
| Validation vocabulary | `ValidationState` is the caller-owned `Pending` / `Validating` / `Invalid { reason }` / `Valid` ladder. `FormField` presents it without painting in-flight work as failure; `SchemaForm` keeps field and whole-form validation separate and blocks submission while an explicitly managed check is pending or validating. Rules and timing remain host-owned. |
| Schema field participation | `FieldVisibility` records the result of a host-owned condition without evaluating it. Hidden fields are absent from rendering and field validation; `HiddenSubmission::Omit` removes the subtree only from `submission_values`, while `Include` preserves its complete held subtree. `values` stays lossless, and a hidden object or repeated-list parent governs every descendant. |
| Settings search | `SettingsList` takes the query a host commonly receives from `SearchField` and owns matching, filtering, result counting, and the no-match state for complete `SettingsSection` builders. Label, description, badge, displayed value, management reason, section context, and `SettingsRow::search_terms` all use the installed `SearchMatcher`; text hidden inside an arbitrary caller control must be named explicitly. Matches retain section and row order rather than turning preferences into a ranked command palette. |
| Composition | `Slotted` lets a caller replace a node a component authored rather than only configure it. A component publishes only positions its public state model can actually reach as `SLOTS`, and a name outside that list panics rather than silently rendering nothing. Surfaces with loading and failure phases offer those distinct slots; an empty-only collection offers only `empty`. No component yet slots a node that is not a whole-region state. |
| Size response | `Responsive` builds its content from its own measured width, so a component laid out in a sidebar and in a full-width page arranges itself differently without either of them consulting the window. `ContainerSize` reports `Unmeasured` for the one frame before there is a width rather than guessing at one. `Grid` and `Container` now provide the shared token-backed breakpoint vocabulary; `Toolbar` measures its cut from the widths it recorded last frame; `overflow_after` remains for a caller who already knows. The remaining framework boundary is response to a size a component cannot itself be given, such as the width of a sibling. |
| Style escape hatch | `ThemeOverlay` installs a caller-adjusted `Theme` for one subtree and pops it afterwards, in every element phase, so an override cannot reach a sibling. What comes back is a whole `Theme`, so the subtree still reads a complete token set and a component inside it cannot tell it was overridden. There is still no way to override *one property of one instance* without constructing a theme for it, which is deliberate: a per-instance colour is how a library stops being one. |
| Non-virtualized `Table` and `Tree` | `List`, `DataGrid`, and `MessageList` virtualize. These two lay out every row. |

### Numbers a catalogue cannot fix alone

`gpui_kit::strings` closes both halves of localization without making a
component infer either one. `Strings` places facts inside phrases and selects
host-installed `zero`, `one`, `two`, `few`, `many`, or `other` variants.
`NumberAdapter` shapes the facts themselves: grouped counts and decimals,
editable decimal parsing, count-of-total, percentages, quantities and affixes,
image dimensions, ordered-list markers, signed deltas, lower bounds, and
playback multipliers. `NumberInput` writes and reads through the same adapter,
so a localized value never becomes unparsable merely because the control drew
it. Pagination, search, grids, document views, media, navigation, forms,
canvas, game, and agent surfaces all use that boundary for the numeric facts
they author.

That does not make the library the owner of every number it receives.
Caller-authored labels, clock readouts, currency and cost strings, source text,
terminal output, paths, identifiers, and diagnostics remain verbatim because
re-parsing them would change caller-owned meaning. Numeric geometry used only
for layout, rendering, hit testing, stable ids, and debug output is not reader
copy and is not localized. Date facts remain on the parallel `DateAdapter`;
the host still owns its calendar, locale, and time zone. The built-in English
adapter is a complete fallback, not a claim to discover the host locale.

### Delivery

`CHANGELOG.md` and the versioning policy in `README.md` now say what a consumer
pins and what breaks them, including the two breaks the compiler cannot see: a
token key and a semantic id. The publishable crates are a crates.io cohort;
GPUI Box is no longer a git dependency of itself. An enforceable structural
and calibrated timing budget is active foundation work in
`docs/foundation-roadmap.md`; until that phase lands, virtualization behavior
tests still do not fail on every class of slowdown.
The hosted catalog at gpui-box.origingame.dev is the published documentation;
it is deployed from a checkout and is not itself a crates.io release.

The visual regression gate is `headless check` on macOS (Metal) and Windows
(WARP). It renders offscreen at a fixed device-pixel size, so it does not
depend on a composited, frontmost window or the host display. Linux is a
compile-and-capability surface: CI builds the native graph there, but does
not compare llvmpipe pixels. Prior images under
`snapshots/headless/linux` are retired and non-gating.
`docs/screenshot-testing.md` describes the gate and review workflow.

## Rules every covered component follows

1. The answer belongs to the caller. A component holds hover, focus, open, and
   animation state; a value, a selection, a sort, and an expansion belong to
   the host, which is why every one of them reports an intent instead of
   applying it. Where a component needs a fact it cannot hold either — what
   day it is, what a month is called, whether a range's days can be listed —
   it takes an injected reader for that fact rather than deriving one, and a
   reader that answers "I don't know" is answering.
2. A refused or disabled control installs no handler at all.
3. Loading, empty, unstarted, unavailable, and failed are distinct, and a
   refusal is never rendered as an absence of data. A question nobody could
   answer is distinct again: a calendar with no month to show renders
   unavailable rather than blank, and a range whose days could not be
   enumerated reports unchecked rather than clear.
4. Ids come from business identity, never from list position.
5. Anything visible comes from tokens. Wording that belongs to the host — a
   refusal's reason, a month's name, a message saying why text could not be
   read — is shown verbatim and never authored by a component. Wording the
   library does author comes from `gpui_kit::strings`, so a host replaces it
   without forking the component; `cargo run -p xtask -- strings check` fails
   the build if a component grows a literal a reader could read.
6. A component that can carry a credential publishes its shape, never its text.
