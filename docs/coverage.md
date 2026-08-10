# Coverage

What a general-purpose desktop UI library is expected to provide, what this
one provides, and what is deliberately out of scope. `docs/components.md`
describes the components themselves; this file exists so a gap is a recorded
decision rather than an oversight.

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
| Navigation | `Tabs`, `Accordion`, `Collapsible`, `Breadcrumb`, `Sidebar`, `Pagination`, `Wizard` |
| Data | `List` (virtualized), `Table`, `DataGrid` (virtualized), `BulkBar`, `Tree` |
| Date and time | `Calendar`, `DateInput`, `RangePicker`, `TimeInput` |
| Content | `Markdown`, `MessageList`, `ImageViewer`, `TransportBar`, `BrowserPanel` (shell only), `LogStream`, `DiffView` |
| Display | `Badge`, `Tag`, `Avatar`, `Card`, `ListRow`, `Divider`, `ProgressBar`, `EmptyState`, `StatusDot`, `StatusLine`, `Callout`, `PulseLoader`, `GradientSpinner`, `Skeleton`, `ProgressCircle`, `DescriptionList`, `Timeline`, `Sparkline` |
| Overlay | `Overlay`, `Dialog`, `Drawer`, `Popover`, `Menu`, `ContextMenu`, `Menubar`, `CommandPalette`, `Tooltip`, `HoverCard`, `Toast`, `ToastLayer`, `Kbd` |
| Layout | `SplitPane`, `SplitTree`, `ScrollArea`, `Toolbar`, `AspectRatio` |
| Shell | `Dock`, `StatusBar` |
| Keymap | `KeybindingRecorder` |
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
and a layout algorithm here would make that claim for every host at once.

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
- **Judging a keybinding.** `KeybindingRecorder` captures a keystroke and
  reports it. Whether it clashes with something, and what to do about it, needs
  the keymap, which the host owns; the recorder renders the conflict the host
  found rather than inventing one.
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
  `VideoPlayer` go one step further and ask a `MediaTransport`, which this
  crate declares and does not implement: decoding, an output device, a clock
  that advances by itself, and where the live edge of a stream is belong to a
  backend, and there is none in this crate and none in GPUI at the pinned
  revision. That is why a duration nobody stated is a state rather than a zero,
  and why a surface with no transport says so instead of drawing controls.
- **Reading a 3D model that is not glTF.** `ModelViewer` reads the subset of
  glTF 2.0 stated in `docs/components.md` and refuses everything else, without
  a scene-graph dependency, a material system, or a texture pipeline. Other
  formats, materials, animation and skinning are not gaps to be filled here:
  a document that needs them is one an application converts before it arrives.
- **Charts.** A chart is a data-visualisation library with its own scales,
  axes and accessibility model. `Sparkline` is the deliberate narrow exception:
  an accessible reading over caller-normalized points, with no scale, axes,
  locale, interaction or chart model of its own.
- **Colour picker, file picker, print dialog.** Platform surfaces; a host
  should reach the operating system rather than a reimplementation.
- **Menu bar and window chrome.** Owned by the platform window, not by a
  component tree.
- **Carousel, rating, and other marketing patterns.** Not desktop-application
  vocabulary.

## Known gaps, in the order they are being closed

Out of scope above means "will not be built, and here is why". This section is
the opposite: things a general-purpose library is expected to have that this
one does not have yet. They are recorded here so that the difference between a
decision and a debt stays visible.

### Motion, which is where the largest gap is

The primitives in `docs/motion.md` cover a value moving from one state to
another, motion that is interrupted, composed or driven by a gesture, and
springs described as a duration and a bounce. Two things are left.

| Gap | Why it matters |
|---|---|
| Shape in `flip` | `flip_size` records a rectangle, so position and size are continuous, but a radius, a border and a colour are not: a pill that becomes a card changes shape on the frame it changes kind. |
| Overscroll | `motion::rubber_band` damps a pull past a boundary, but nothing in the library overscrolls: a `ScrollArea` stops dead at its end, so the band is available to a caller and used by no component here. |

### A platform media backend

`AudioPlayer` and `VideoPlayer` are complete as components: their states, their
controls, their semantics and their refusals are all here, and the fixture
transport proves them. What is not here is anything that decodes: no
implementation of `MediaTransport` reaches an operating-system player, so on
every platform today the only transports in the tree are the host's own and the
fixture. `VideoPlayer` will render a real frame the moment a host hands one back
— GPUI's `surface` element composites a `CVPixelBuffer` on macOS, and a decoded
frame can be uploaded as a `RenderImage` anywhere — so the deferred work is the
backend behind the trait and not the surface above it. This is recorded as a gap
rather than presented as support: a component that reports `no-backend` on every
machine is truthful, and it is not playback.

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

Then: mentions in a text field, search within settings, a keymap editor showing
conflicts, and undo history.

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
| Text selection | GPUI offers no pointer text-selection primitive. Components such as `CodeView`, `Markdown` and `LogStream` can offer explicit whole-value copy actions or intents, but arbitrary rendered ranges cannot be selected. |
| Text range highlighting | `HighlightedText`, `LogStream`, `CodeView` and `DiffView` render caller-supplied ranges while constructing their text. GPUI still has no API that marks a substring of an arbitrary already-rendered text element, which blocks a generic find-in-page overlay. |
| Writing direction | Nothing reads right-to-left. Every inset is left and right rather than start and end, so this is whole-library work that gets more expensive every batch. |
| Number, date, and quantity formatting | Every word is now host-replaceable, but every *number* beside one is still formatted by Rust. See "Numbers a catalogue cannot fix" below. |
| Assistive technology gaps | Basic semantics, grapheme-based editable text runs and selection actions, and explicit live-region properties now reach GPUI's AccessKit platform tree. Character geometry/native caret tracking, verified screen-reader announcement timing, native-child handoff, and native Windows/Linux session verification remain absent; see `docs/accessibility.md`. |
| Validation vocabulary | `FormField` shows an error it is handed. When to validate, field against form, and validation still in flight have no shared shape. |
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
calendar and ask a host-supplied `DateAdapter` for every date they show. A
number reader of the same shape — asked for a count, a plural category, a
percentage, a duration — would close this without pulling a formatting library
into product-neutral infrastructure. That is a batch of its own, and none of it
is started.

### Delivery

`CHANGELOG.md` and the versioning policy in `README.md` now say what a consumer
pins and what breaks them, including the two breaks the compiler cannot see: a
token key and a semantic id. What is still missing is a performance budget, so
nothing fails when a virtualized list gets slower, and published documentation;
the gallery is local. Publishing to a registry is impossible while GPUI is a git
dependency.

The visual regression is a gate everywhere except CI. `scenes check` needs a
composited, frontmost window and the display the baselines came from, which a
hosted runner does not give, so it runs on a self-hosted runner if one is
configured and is otherwise a step a reviewer performs and records.
`docs/screenshot-testing.md` states the three requirements; a job that ran it
where it cannot look would report agreement it never established, which has
happened here once already.

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
