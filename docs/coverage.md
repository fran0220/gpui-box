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
| Text entry | `TextInput`, `PasswordInput`, `OneTimeCodeInput`, `TextArea`, `NumberInput`, `TagInput`, `InlineEdit` |
| Choice | `Select`, `Combobox`, `Checkbox`, `Radio`, `Switch`, `Slider`, `SegmentedControl` |
| Form | `FormField`, `SettingsRow`, `SettingsSection` |
| Navigation | `Tabs`, `Accordion`, `Collapsible`, `Breadcrumb`, `Sidebar`, `Pagination`, `Wizard`, `UndoHistory` |
| Data | `List` (virtualized), `Table`, `DataGrid` (virtualized), `BulkBar`, `Tree` |
| Date and time | `Calendar`, `DateInput`, `RangePicker`, `TimeInput` |
| Content | `Markdown`, `MessageList`, `ImageViewer`, `TransportBar`, `BrowserPanel` (shell only), `LogStream`, `DiffView` |
| Display | `Badge`, `Tag`, `Avatar`, `Card`, `ListRow`, `Divider`, `ProgressBar`, `EmptyState`, `StatusDot`, `StatusLine`, `Callout`, `PulseLoader`, `GradientSpinner`, `Skeleton`, `ProgressCircle`, `DescriptionList`, `Timeline`, `Sparkline`, `LineChart`, `BarChart` |
| Overlay | `Overlay`, `Dialog`, `Drawer`, `Popover`, `Menu`, `ContextMenu`, `Menubar`, `CommandPalette`, `Tooltip`, `HoverCard`, `Toast`, `ToastLayer`, `Kbd` |
| Layout | `SplitPane`, `SplitTree`, `ScrollArea`, `Toolbar`, `AspectRatio` |
| Shell | `Dock`, `StatusBar` |
| Keymap | `KeybindingRecorder`, `KeymapEditor` |
| Interaction | `Dropzone` |
| Filtering | `FilterBar` |
| Agent run | `ToolCallCard`, `StepList`, `ThinkingBlock`, `NodeGraph` |
| Structured data | `JsonView`, `SchemaForm` |
| Connections | `ServerList`, `OfferingCatalog` |

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
fetches nor decodes. Selection, movement, deletion, connection, disconnection,
pan, and zoom are all controlled proposals: the caller remains authoritative
for the selection, topology, positions, and viewport shown on the next frame.

## Systems, which span more than one component

A system is not a component. It has no builder of its own to place on a
screen; it is a contract several families implement, so it is covered when the
contract is documented, staged in a scene, and driven through simulated input
against every surface that implements it.

| System | Contract | Implemented by |
|---|---|---|
| Drag and drop (`gpui_kit::interaction::dnd`) | `docs/interaction.md` | `List`, `Tree`, `Tabs`, `Dropzone`, `DataGrid` (column headers), `Dock` (panel headers) |

Drag and drop is covered: the contract is written down, the scenes `drag-list`,
`drag-tree`, and `dropzone` stage it, and `crates/gpui-kit/tests/dnd.rs` drives
a simulated pointer through every surface above. `DataGrid` reorders its column
headers through the same system, driven in `crates/gpui-kit/tests/grid.rs`, and
`Dock` moves panels between regions through it, driven in
`crates/gpui-kit/tests/shell.rs`.

## One resize implementation

`SplitPane` is two panes and a divider; `SplitTree` is however many of those the
caller nests; `Dock` builds a `SplitLayout` from the regions that hold panels
and hands it to a `SplitTree`. So a divider between two dock regions is the
same divider a plain split gives, with the same minimums and the same published
travel range, and there is one place where dragging a divider is implemented.
A dock region's header is a `Tabs` strip for the same reason: dragging a panel
is the drag system, not a second one.

## Table or DataGrid

Both are covered and neither replaces the other. `Table` takes materialized
rows and lays all of them out; `DataGrid` takes a render closure and lays out
only the rows the viewport holds, which is what buys it column resizing and
reordering, a pinned group, selection over an incompletely loaded set, opened
rows, and cell editing. `docs/components.md` has the guidance on which to
reach for, and the limits `DataGrid` states rather than fakes: no horizontal
scroll, so a pinned column holds the left edge rather than freezing under one;
and no fit-to-content measurement, so a double click on a column edge reports
the request and lets the host answer.

`TreeGrid` reuses that virtualized machinery for a caller-flattened hierarchy.
Its `tree-grid` scene and `crates/gpui-kit/tests/tree_grid.rs` cover bounded
materialization, TreeGrid/Row/GridCell semantics, row hierarchy metadata, and
logical disclosure/parent keyboard intents. It deliberately adds no second
flattening, horizontal-scroll, frozen-column, or cross-tree layout system.

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
- **Rich text and code editing.** `TextArea` edits plain text and `Markdown`
  renders it read-only; neither edits rich text. Syntax highlighting, folding,
  and multi-caret editing are an editor, not a control. `Markdown` will colour
  a code block from spans a host computed and will not compute them: deciding
  that a word is a keyword needs a grammar, which is the same kind of fact the
  calendar is — answered correctly only by the library the application already
  depends on.
- **Doing what a document says.** `Markdown` draws HTML as the characters
  somebody wrote, reports a link rather than opening it, and names an image
  rather than fetching it. There is no HTML renderer here, no URL policy, and
  no network; `docs/content.md` states why each of those is a refusal rather
  than a gap, and what a host has to supply instead.
- **Delivering a message.** `MessageList` renders five delivery states and
  reports a retry. Sending anything, deciding what a resend means, and knowing
  whether a message was really read are the transport's, and this crate has no
  transport.
- **Fetching or decoding an image.** `ImageViewer` frames, zooms and pans an
  element the host hands it, and names the source when the host hands it
  nothing. There is no network here and no decoder, so the pixel size of a
  source is a caller input like every other fact this library cannot hold;
  a viewer given none says the size is unknown rather than reporting the box
  it drew.
- **Playing media.** `TransportBar` reports play, pause, seek, volume, mute,
  speed and a track step, and applies none of them. `AudioPlayer` and
  `VideoPlayer` ask a `MediaTransport`; `PlatformMediaTransport` implements it
  with AVFoundation on macOS and Media Foundation on Windows, including the
  native decoder, output, clock and video view. Linux and Web remain explicit
  no-backend targets. Playlist policy, DRM, subtitle/track selection, output
  device routing, custom network retry/cache policy and capture remain host
  responsibilities rather than states the components invent.
- **Reading a 3D model that is not glTF.** `ModelViewer` reads the subset of
  glTF 2.0 stated in `docs/components.md` and refuses everything else, without
  a scene-graph dependency, a material system, or a texture pipeline. Other
  formats, materials, animation and skinning are not gaps to be filled here:
  a document that needs them is one an application converts before it arrives.
- **Inventing a scale, a locale, or a series policy.** A chart still does not
  own data. Axes, ticks, domains, stacking, aggregation, and "2 minutes ago"
  are facts the host already has or can compute; Box paints them. The old
  Kit-era refusal of charts themselves is lifted: line, bar, area, and
  distribution surfaces are in scope as application primitives. A
  business-intelligence toolkit — live query, crossfilter, annotation
  layers, financial overlays — is still a product, not a substrate.
- **Owning a platform picker.** Colour, file, and print dialogs that replace
  the operating system stay out. In-window colour wells, dropzones, and
  print-preview chrome that report a choice are in scope; they do not
  become the system dialog.
- **Menu bar and window chrome.** Owned by the platform window, not by a
  component tree.
- **Carousel, rating, and other marketing patterns.** Not desktop-application
  vocabulary.

## Known gaps, in the order they are being closed

Out of scope above means "will not be built, and here is why". This section is
the opposite: things a general-purpose library is expected to have that this
one does not have yet. They are recorded here so that the difference between a
decision and a debt stays visible.

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
| `data-grid` | High: muted table structure and managed state are unreadable in dark; chip baselines and compact type are inconsistent. |
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
| `form` | High: the Region popup covers field help; dark unselected text is too faint and errors touch fields. |
| `frost` | High: dark frost lacks enough edge/surface evidence; light needs bottom clearance and consistent stripes. The edge evidence a frosted surface is missing is what `glass` carries; `Frost` stays frosted deliberately, being the material every renderer can produce. |
| `glass` | Medium: refraction, dispersion and the specular rim are rendered by every renderer the repository ships — Metal, WGPU and DirectX — and the DirectX output was compared against WGPU on the same machine, agreeing to one step outside the text. The renderer on the web target draws no backdrop at all, so a surface there is its tint alone. The component now drives all four interactive capabilities: `track_pointer` moves the highlight with the pointer, `pressable` deepens the refraction by `effect.glassPressDepth` on a spring, `adaptive` claims a luminance probe slot and deepens the tint against an opposing backdrop with the flip band as hysteresis, and `GlassGroup` fuses up to eight panes into one body. The scene shows fusion and the settled adaptive flip; a pointer-lit or pressed surface cannot appear in a still, so those are reviewed in the gallery. Two accepted limits: a probe reading arrives one frame after the backdrop moved, so a backdrop change whose window then draws no further frame keeps the old tint until the next paint (the renderer does not yet request that frame itself), and on the web target `Window::backdrop_luminance` returns `None`, so an adaptive surface honestly keeps its thin tint. The scene holds macOS and Windows baselines; CI no longer runs the Linux headless comparison, so the Linux set under `snapshots/headless/linux` is retired rather than kept current. The two renderers that convolve their own blur hold a per-frame render-pass budget (256, enough for a full complement of sixteen probed surfaces at the themes' standard blur), and their behaviour past it diverges deliberately: DirectX keeps a surface's optics and probe and drops only its blur, while WGPU skips the surface whole — a scene dense enough to hit that edge renders differently per renderer, which is a recorded gap rather than supported behaviour. |
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
| `loading` | High: skeletons lack hierarchy/max-width and contrast; loaders crowd the top-left. The three indicators now share `color.loader.gradient` rather than mixing accent with a second palette. |
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
| `table` | Medium: the dark managed row is nearly invisible. |
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

### Motion, which is where the largest gap is

The primitives in `docs/motion.md` cover a value moving from one state to
another, motion that is interrupted, composed or driven by a gesture, and
springs described as a duration and a bounce. Two things are left.

| Gap | Why it matters |
|---|---|
| Shape in `flip` | `flip_size` records a rectangle, so position and size are continuous, but a radius, a border and a colour are not: a pill that becomes a card changes shape on the frame it changes kind. |
| Overscroll | `motion::rubber_band` damps a pull past a boundary, but nothing in the library overscrolls: a `ScrollArea` stops dead at its end, so the band is available to a caller and used by no component here. |

### Media capabilities beyond native playback

`PlatformMediaTransport` closes the ordinary macOS/Windows decoder, output,
clock and native-view gap. The remaining media work is additive capability,
not basic playback: playlist/queue ownership; audio, subtitle and accessibility
description track selection; audio output-device switching; DRM; application
cache/retry and authentication policy; picture-in-picture and fullscreen;
recording/capture; frame-accurate extraction plus waveform/thumbnail generation;
and Linux/Web backends. A platform codec refusal remains `no-backend`, while an
unreadable source remains `failed`; neither is converted to empty media.

### Components

Ordinary applications, roughly in the order they are wanted: document-tab
vocabulary on `Tabs` (dirty, close, overflow), a search field with in-place hit
highlighting, find and replace, a notification centre holding what `Toast`
showed, an error boundary for a panel that failed to render, a read-only code
view, and an upload list over `Dropzone`.

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

Then: mentions in a text field and search within settings. `UndoHistory` now
covers the caller-owned revision list and reports restore intents without
keeping or mutating an undo stack.

Application forms still need the schema shapes a settings page and a tool
card actually use: date, time, range, files, repeating sections, and
host-declared visibility. `SchemaForm` already keeps unrenderable fields
visible; it does not yet express those shapes. Charts are the same kind of
gap: `Sparkline`, `ContextGauge`, and `CostMeter` are readings, not the
cartesian surfaces an application dashboard needs.

Agent applications need a family this library is still filling in. A
conversation is not the unit; a run made of steps is, and `ToolCallCard`,
`StepList` and `ThinkingBlock` are covered above. Still wanted:
`ApprovalPrompt` and `PermissionMatrix` where the default is
refusal and the scope of "always" is stated, `CostMeter` and a context gauge
that says when a number is an estimate. `OfferingCatalog` covers Tool, Skill,
and Resource results together rather than creating separate `ToolCatalog` and
`SkillCard` APIs; per-server attribution is part of every result because two
servers may offer the same name. `LogStream`, `DiffView`, `JsonView`,
`SchemaForm`, `ServerList`, `OfferingCatalog` and `NodeGraph` are covered above.

`NodeGraph` covers drawing a run that branched or was retried; it does not
cover arranging one. A caller with a topology and no coordinates needs a layout
pass, and a layered or force-directed one is a plausible future addition here
precisely because it is separable from the drawing.

### Capabilities that are not components

| Gap | Why it matters |
|---|---|
| Cross-element text selection | GPUI now supplies grapheme-safe read-only selection for one shaped `StyledText`, including wrapping, bidi geometry, pointer capture, keyboard copy/select-all, and AccessKit text runs. `CodeView`, `Markdown`, `LogStream`, `DiffView`, and `HighlightedText` use it. A single drag spanning separately mounted elements or virtualized rows remains a document-selection coordinator gap; whole-value copy actions include unmounted content. |
| Text range highlighting | `HighlightedText`, `LogStream`, `CodeView` and `DiffView` render caller-supplied ranges while constructing their text. GPUI still has no API that marks a substring of an arbitrary already-rendered text element, which blocks a generic find-in-page overlay. |
| Writing direction | `LayoutDirection` supplies logical row order, start/end spacing and borders, text alignment, directional glyph mirroring, and reading-order keyboard traversal across controls, navigation, menus, calendars, trees, structured views, and schema forms. Unicode bidi shaping keeps mixed Arabic/Hebrew, Latin, punctuation, and numbers in logical order. Host-owned localized copy, locale formatting, and a larger language-specific bidi corpus remain integration work rather than component geometry. |
| Number, date, and quantity formatting | Every word is now host-replaceable, but every *number* beside one is still formatted by Rust. See "Numbers a catalogue cannot fix" below. |
| Assistive technology gaps | Basic semantics, grapheme-based editable and read-only text runs, read-only character geometry, selection actions, and explicit live-region properties now reach GPUI's AccessKit platform tree. Editable character geometry/native caret tracking, verified screen-reader announcement timing, native-child handoff, and native Windows session verification remain absent. Linux compatibility, including native AT-SPI validation, is deferred; see `docs/accessibility.md`. |
| Validation vocabulary | `FormField` shows an error it is handed. When to validate, field against form, and validation still in flight have no shared shape. |
| Cartesian readings | `Sparkline` is a normalized trend. Line, bar, area, and share surfaces with host-supplied domains and labels are not built yet. |
| Composition | There is no `Slot`: a caller cannot replace a node inside a component, only configure it. |
| Size response | No breakpoint or container query. `Toolbar` overflow is declared rather than measured, which is the same gap seen from one component. |
| Style escape hatch | Beyond tokens there is no supported way to override one instance. |
| Non-virtualized `Table` and `Tree` | `List`, `DataGrid`, and `MessageList` virtualize. These two lay out every row. |

### Numbers a catalogue cannot fix

`gpui_kit::strings` closed the wording gap: no component holds a word a reader
reads, and a host replaces any of them through `StringKey`. It did not close
the formatting gap, and the two are not the same problem. A catalogue can move
a value inside a sentence, because a template numbers its placeholders. It
cannot change what the value looks like, which is decided by
`usize::to_string` and by `format!` in the component.

What is affected, found while converting the components:

| Where | What is formatted | What a locale would change |
|---|---|---|
| `Pagination`, `ImageViewer`, `ProgressBar`, `ProgressCircle`, `TagInput`, `DataGrid` | counts and positions such as `2 of 9` | digits, grouping, and the ordinal form |
| `Breadcrumb`, `MessageList`, `Markdown`, `DataGrid`, `Dropzone` | plurals, chosen by `if count == 1` at the call site | languages with zero, dual, few, and many forms need more than two keys |
| `FilterBar`, `NumberInput`, `BulkBar` | `{count} {noun}` and `{number} {unit}` | the order of a number and its noun, and whether a space belongs between them |
| `ImageViewer` | `1920 × 1080 · 150%` | the multiplication sign, the percent sign, and which side it takes |
| `TransportBar` | `1.5×` and clock readouts | the speed mark, and a duration's own shape |
| `DescriptionList` | `51 characters`, from `redacted_from` | the count, and the noun's agreement with it |
| `Calendar`, `DateInput`, `RangePicker`, `TimeInput` | nothing: dates already come from the host `DateAdapter` | already correct, and the model the rest should follow |

The shape of the answer is visible in the date components, which own no
calendar and ask a host-supplied `DateAdapter` for every date they show.
`NumberAdapter` is that reader: a host supplies digits, plural category,
`count of total`, and percent marks. `ProgressBar` already asks it.
Pagination, Transport, ImageViewer, and the remaining count sites still
format Rust digits and are the rest of this batch.

### Delivery

`CHANGELOG.md` and the versioning policy in `README.md` now say what a consumer
pins and what breaks them, including the two breaks the compiler cannot see: a
token key and a semantic id. The publishable crates are a crates.io cohort;
GPUI Box is no longer a git dependency of itself. What is still missing is a
performance budget, so nothing fails when a virtualized list gets slower.
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
