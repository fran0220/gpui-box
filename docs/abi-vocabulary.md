# ABI vocabulary feasibility

GPUI Box Kit can be the public visual vocabulary of a host-rendered plugin ABI.
It cannot become that vocabulary by serializing its current public API. The
components are renderer contracts: they take GPUI elements, entities, focus
handles, and callbacks, while the wire needs stable data and typed intent.

The viable boundary is a parallel, GPUI-free DTO vocabulary. A host validates
plugin JSON, converts the DTO into the current components, and converts their
events back into typed actions. The component implementations, theme, focus,
motion, and semantic bounds stay in the host.

Sophon is one possible consuming host. Surface registration and placement,
JSON-RPC, the command palette, and the WebView widget remain Sophon concerns.
Dock panels, status items, settings sections, dialogs, wizards, timeline cards,
and other slot contents may use this vocabulary without putting a slot system
or product model into GPUI Box Kit.

## Verdict

| Family | Feasibility | What prevents direct publication |
|---|---|---|
| Layout / dock | **Feasible with a bounded composition DTO** | `DockPanel` stores an `AnyElement`; `Dock` emits GPUI callbacks. Slot placement remains host-side. |
| Layout / status bar | **High; suitable for ABI v1** | The scalar variants are easy to mirror, but `StatusItem` stores private renderer state and a click closure. |
| Navigation / wizard | **Feasible with a composition DTO** | Step data and `WizardIntent` mirror cleanly; the step body is an `AnyElement`. |
| Structured forms | **High for the supported schema dialect** | `Schema` is already a product-neutral projection, but no form type derives serde and stateful controls contain `Entity` and `FocusHandle`. |
| Overlay dialogs | **Feasible with host-owned lifecycle** | `Dialog` owns focus/runtime state and rebuilds its body from a closure. |
| Table / DataGrid | **Feasible, but the hardest v1 family** | Cells are elements, virtual rows and details are closures, and reorder uses drag runtime types. A purpose-built row/cell DTO is required. |
| Content / document | **Feasible with bounded source and resource contracts** | Text models mirror well. Images, highlighting, and media are supplied through host closures or runtime elements. |
| Agent run | **Feasible from existing primitives; aggregate missing** | Run steps, approvals, permissions, tool calls, cost, and thinking exist, but there is no single serialized run/card composition contract. |
| Game experience | **High from typed snapshots and events** | Party, objective, ability, and reward models mirror cleanly. Portrait images and effect plans remain host/runtime resources rather than wire recipes. |
| Controls / display | **Broadly feasible** | Scalar state mirrors well. Interactive controls emit heterogeneous GPUI callbacks/events; file drops and clipboard work are runtime capabilities. |

This is a **yes** for a full vocabulary, not a claim that the current Rust
types are a wire ABI. Status, display, basic controls, schema forms, simple
dialogs, and Markdown are the shortest route to a useful v1. DataGrid,
arbitrary nested composition, file/media resources, and an aggregate agent run
need deliberate DTO contracts before publication.

## How this inventory reads the current API

- **Serializable** means the current Rust type derives both serde `Serialize`
  and `Deserialize`.
- **Mirrorable** means a host can copy the type's meaning into plain JSON
  values without carrying a GPUI object. `SharedString` becomes a JSON string;
  a closed scalar enum becomes an explicitly named wire value. Mirrorable does
  not mean the existing Rust representation is stable or already serialized.
- **DTO seam required** means the current contract contains `AnyElement`,
  `Entity`, `FocusHandle`, `Ident`, `Window`, `App`, a render closure, a GPUI
  geometry or platform payload, or another runtime-only value.

No component contract in `crates/gpui-kit` currently derives serde. The only
current serialized UI contract is the semantic output described below.

## Component inventory

### Layout / dock

**Host projection.** A dock slot needs a panel record with stable identity,
title, optional icon and badge, availability, and a declarative body. The body
needs bounded stack/group, text, separator, scroll, toolbar, display, control,
and family-specific nodes. Sophon owns which dock region receives the slot,
whether it is visible, and whether a plugin is allowed to mount it.

**Current Rust types.** `DockRegion`, `DockPanel`, `DockEvent`, and `Dock` are in
`crates/gpui-kit/src/layout/dock.rs`. `ToolbarItem` and `Toolbar` are in
`crates/gpui-kit/src/layout/toolbar.rs`. `SplitPaneSpec`, `SplitLayout`,
`SplitRecord`, `SplitChange`, and `SplitTree` are in
`crates/gpui-kit/src/layout/tree.rs`; `ScrollArea` and `ScrollAxis` are in
`crates/gpui-kit/src/layout/scroll.rs`.

`DockRegion`, `SplitRecord`, and the scalar split state are mirrorable. They
are not serde types. `DockPanel` stores its content as `AnyElement`; `ToolbarItem`
does the same, and `Toolbar` may hold `Entity<Menu>`. Those are hard DTO seams.
The ABI must not introduce an "arbitrary element" escape hatch to match them.

**Emitted actions.** `DockEvent` reports `PanelSelected`, `PanelMoved`,
`RegionCollapsed`, and `RegionResized`. The first two use panel business ids;
move uses `before: Option<SharedString>` rather than an index. These are good
intent shapes, but they are host slot actions in Sophon, not automatically
plugin actions. Toolbar item activation comes from the contained control or
from `MenuEvent::Invoked` in `crates/gpui-kit/src/overlay/menu.rs`.

**Verdict.** The dock shell exists. The missing ABI work is a safe declarative
panel body and a decision about which shell events remain host-local. Do not
serialize `DockPanel` or let a plugin select a `DockRegion` through the visual
vocabulary.

### Layout / status bar

**Host projection.** A status item needs a stable node, a presentation kind
(`text`, `state`, `progress`, or `action`), label, optional allowlisted icon,
state name, tone or progress, stale and disabled state, and an optional action
id. Group, ordering, overflow, and slot lifetime are host placement.

**Current Rust types.** `StatusGroup`, `StatusItem`, and `StatusBar` are in
`crates/gpui-kit/src/layout/status_bar.rs`. `Tone` is in
`crates/gpui-kit/src/display/badge.rs`. `AsyncStatus` and `AsyncValue`, used by
`StatusItem::tracking`, are in `crates/gpui-kit/src/state.rs`.

`StatusGroup`, `Tone`, progress/count values, state names, and truthful async
states are trivially mirrorable, but none is serialized here. `StatusItem` is
not: its presentation is a private renderer enum, `StatusItem::element` takes
an `AnyElement`, and `StatusItem::on_click` stores a callback over `Window` and
`App`. A wire status item should deliberately omit the arbitrary-element
variant.

**Emitted actions.** An action item invokes `StatusItem::on_click`. There is no
`StatusBar` event enum and non-action status items do not become clickable just
because a callback exists. The adapter should route an action item's opaque
action id and emit no handler while disabled.

**Verdict.** This is the simplest useful ABI family. Its DTO can be smaller and
more stable than the renderer builder.

### Navigation / wizard

**Host projection.** A wizard needs stable step ids, title and description,
`complete`/`current`/`upcoming`/`blocked`/`failed` state, reachability, layout,
the current step's declarative body, back target, finish state, advance
refusal, labels, and disabled state.

**Current Rust types.** `StepStatus`, `WizardStep`, `WizardLayout`,
`WizardIntent`, and `Wizard` are in
`crates/gpui-kit/src/navigation/wizard.rs`. Related navigation models include
`TabItem`, `SaveState`, and `Tabs` in `navigation/tabs.rs`; `AccordionSection`
and `Accordion` in `navigation/accordion.rs`; `SidebarItem`, `SidebarSection`,
and `Sidebar` in `navigation/sidebar.rs`; and `PageTotal` and `Pagination` in
`navigation/pagination.rs`.

The step, layout, status, and intent shapes are mirrorable, including refusal
reasons. They have no serde derives. `Wizard` itself requires a DTO seam because
its body is an `AnyElement` and navigation is an `Rc<dyn Fn(..., Window, App)>`.
The related navigation builders have the same callback/element boundary.

**Emitted actions.** `WizardIntent` is already the right semantic split:
`Step(id)`, `Back`, `Next`, and `Finish`. Tabs report select, close, and reorder;
accordions report `(section_id, expanded)`; pagination reports a page number.
The host adapter must attach the wizard node and action id and wait for the
plugin's next ViewModel before showing a new durable step.

**Verdict.** The flow contract is present. Only its body composition and wire
identity/action envelope are missing.

### Structured forms

**Host projection.** A form needs stable section, row, field, and option ids;
labels and descriptions; required, disabled, managed, and invalid state;
values and host error text; and a closed set of field kinds. Input drafts and
selection/caret state remain host-visual state. Credentials may be accepted by
a secret field, but their value must never enter a semantic snapshot or action
log.

**Current Rust types.** The strongest existing projection is `SchemaChoice`,
`NumberBounds`, `SchemaKind`, `SchemaField`, `Schema`, `UnrenderableField`,
`FieldValue`, `SchemaFormEvent`, and `SchemaForm` in
`crates/gpui-kit/src/structured/schema_form.rs`. Its module explicitly says
that the host converts its schema dialect into this product-neutral shape
because GPUI Box Kit takes no serialization dependency. Unsupported input stays
visible as `SchemaKind::Unrenderable` and `FieldValue::Unrenderable`.

The other structured surface is `JsonValue`, `ValueKind`, and `JsonView` in
`crates/gpui-kit/src/structured/json_view.rs`. `JsonValue` preserves number
text, object order, repeated keys, and explicit redaction rather than forcing a
lossy map or `f64`. It is a strong, mirrorable ViewModel source. `JsonView`
adds `Ident`, disclosure/selection callbacks, and viewport state, so the value
model belongs in a DTO while the view remains behind the adapter.

Settings composition is `SettingsRow` and `SettingsSection` in
`crates/gpui-kit/src/controls/settings_row.rs`. The available field controls
include:

- `TextInput` and `TextInputEvent` in `controls/input/mod.rs`;
- `TextArea` and `TextAreaEvent` in `controls/textarea/mod.rs`;
- `SelectOption`, `SelectEvent`, and `Select` in `controls/select.rs`;
- `ComboboxEvent` and `Combobox` in `controls/combobox.rs`;
- `NumberInputEvent` and `NumberInput` in `controls/number_input.rs`;
- `TagInputEvent` and `TagInput` in `controls/tag_input.rs`;
- `Checkbox`, `Radio`, and `Switch` in `controls/toggle.rs`;
- `InlineEdit` in `controls/inline_edit.rs`; and
- `SearchField`, `SearchFieldEvent`, `FindReplace`, and `FindReplaceEvent` in
  `controls/search.rs`.

Date and time fields are represented by `Day`, `MonthKey`, `MonthCell`,
`MonthGrid`, `Selectability`, `Clock`, `TimeOfDay`, and the `DateAdapter` trait
in `crates/gpui-kit/src/datetime/adapter.rs`; `DayMark`, `CalendarEvent`,
`DateInputEvent`, `DayRange`, `RangeState`, `BlockedDay`, `BlockedReport`,
`RangePickerEvent`, `TimeSegment`, and `TimeInputEvent` are exported from the
components under `crates/gpui-kit/src/datetime/`.

The schema records and most field value/event payloads are trivially
mirrorable. None derives serde. `SchemaForm`, every editable field, and the
date controls are runtime views containing `Entity`, `FocusHandle`,
subscriptions, or `SharedDateAdapter`. A plugin cannot provide a `DateAdapter`
trait object; an ABI date field must use a host calendar capability and portable
date tokens or remain outside the first field-kind set. `SettingsRow` and
`SettingsSection` also accept `AnyElement` controls/actions.

**Emitted actions.** `SchemaFormEvent` reports `Changed(path)` and `Submitted`,
with values read from `SchemaForm::values`. The concrete controls provide typed
payloads: text change/submit/cancel/focus, selected option ids, numeric change
or unparsable text, added/removed/refused tags, boolean change, search intent,
and date/time selections. `InlineEdit` reports edit, commit text, or cancel.
`JsonView` reports `(path, expanded)` and selected path. Settings containers
emit nothing; their nested control does.

**Verdict.** Schema forms are close to a DTO already and should anchor v1.
Mirror their meaning rather than derive serde on the stateful views. Preserve
`Unrenderable` as an explicit refusal during dialect conversion.

### Overlay dialogs

**Host projection.** A dialog needs a stable node, title, optional description,
declarative body, confirm/cancel labels, destructive and dismissable flags,
open/busy/refusal state, and stable action ids. The host owns modal stacking,
focus containment, Escape, scrim behavior, and restoring focus to the trigger.

**Current Rust types.** `DialogEvent` and `Dialog` are in
`crates/gpui-kit/src/overlay/dialog.rs`. Adjacent surfaces are `Drawer` and
`DrawerEvent` in `overlay/drawer.rs`, and `Popover` and `PopoverEvent` in
`overlay/popover.rs`. `MenuItem`, `MenuEvent`, and `Menu` in `overlay/menu.rs`
are useful inside bounded choices, but the Sophon command palette remains
host-side.

`DialogEvent` is trivially mirrorable. `Dialog` is not: it contains several
`FocusHandle`s and a `FocusTrap`, is constructed with `Context<Self>`, and
rebuilds its body through `Fn(&mut Window, &mut App) -> AnyElement`. The same
runtime seam applies to drawer and popover content.

**Emitted actions.** `DialogEvent` reports `Opened`, `Confirmed`, `Cancelled`,
`Dismissed`, and `Closed`; an outcome is followed by `Closed`. The wire adapter
should normally expose confirmed/cancelled/dismissed as plugin intent and keep
opened/closed as host lifecycle unless a capability says otherwise. A
non-dismissable dialog must not gain a wire dismissal path.

**Verdict.** Simple confirmation and form dialogs are feasible now. General
dialog bodies depend on the same declarative composition grammar as dock and
wizard content.

### Data table / DataGrid

**Host projection.** A table needs stable table, column, row, and published
cell ids; column labels, width and alignment; bounded, typed cell content;
authoritative sort and selection; loaded and total counts; loading, empty,
unavailable, failure, and stale states; and optional expansion/edit state.
Virtualized data also needs a bounded loaded page or update protocol. Business
ids, never viewport indexes, route every action.

**Current Rust types.** The materialized family is `SortDirection`,
`ColumnWidth`, `Align`, `Column`, `Cell`, `Row`, and `Table` in
`crates/gpui-kit/src/data/table.rs`. The virtualized family is `GridColumn`,
`GridRow`, `SelectionMode`, `SelectionChange`, `Expanded`, `EditOutcome`,
`EditIntent`, `EditingCell`, `DataGrid`, and `BulkBar` in
`crates/gpui-kit/src/data/grid.rs`. `scroll_to_row` and `reveal_row` are host
viewport operations in `crates/gpui-kit/src/data/viewport.rs`.

The same data module also exports `ListItem` and `List` from
`crates/gpui-kit/src/data/list.rs`, and `TreeNode` and `Tree` from
`crates/gpui-kit/src/data/tree.rs`. `TreeNode` is mostly mirrorable recursive
identity/label/icon/disabled data. `ListItem` contains `AnyElement`; `List`
takes a row closure. `Tree` adds disclosure, selection, viewport, and drag
callbacks. Both report selection by business id and normalize reorder/move
through a host adapter rather than serializing `DragItem` or `DropIntent`.

The scalar column, sort, selection, expansion, and edit shapes are mirrorable
and not serialized. `Cell` contains `AnyElement`. `Table::rows_from` and
`DataGrid` take row render closures; `DataGrid` may also take a detail render
closure. `DataGrid` stores GPUI state for focus, editing, drag, and its
virtualized scroll handle. These are hard DTO seams, not inconveniences a
derive can hide.

**Emitted actions.** `Table` reports sort `(column, direction)` and selected
row id. `DataGrid` reports sort, `SelectionChange`, column resize and fit,
column reorder, row expansion, edit request `(row, column)`, and `EditIntent`.
`BulkBar` reports select-all and dismiss. `List` reports select and reorder;
`Tree` reports select, `(node, expanded)`, and move. The wire should normalize
the current drag `DropIntent` into stable item/before/parent ids and preserve
the distinction between `SelectionChange::Loaded` and
`SelectionChange::Everything`.

**Verdict.** The renderer and interaction reducers exist. The ABI still needs
a purpose-built cell vocabulary and a bounded loaded-row contract. This is a
design task, not a component implementation gap.

### Content / document

**Host projection.** A content node may carry bounded plain text or Markdown,
code lines and preclassified spans, messages and attachments, image metadata,
or transport state. Links, referenced images, copy, retry, expansion, media
seek, and stepping are typed intents. Raw GPUI elements, executable HTML,
script, arbitrary CSS, and native views are never content node variants. A
WebView is a separate host surface.

**Current Rust types.** The family now contains:

- `Block`, `CellAlign`, `Document`, `Inline`, `ListEntry`, `CodeBlock`,
  `CodeSpan`, `ImageRequest`, `MarkdownEvent`, and `Markdown` in
  `crates/gpui-kit/src/content/markdown.rs` and `content/markdown/parse.rs`;
- `LineMark`, `CodeLine`, and `CodeView` in `content/code_view.rs`;
- `FitMode`, `ImageSize`, `ImageState`, `ImageFrame`, `ImageViewerEvent`, and
  `ImageViewer` in `content/image_viewer.rs`;
- `DeliveryState`, `MessageBody`, `Attachment`, `Reaction`, `Message`, and
  `MessageList` in `content/message_list.rs`; and
- `TransportDuration`, `TransportState`, `BufferedRange`, `TrackStep`,
  `TransportEvent`, and `TransportBar` in `content/transport.rs`.

The parsed Markdown tree, code lines, message records, image metadata, and
transport state are mirrorable but not serde types. For the wire, Markdown
source is a smaller contract than exposing the parser's `Document` tree. The
renderers remain GPUI-bound. `Markdown::image` and `ImageViewer::image` return
`AnyElement` through host closures; Markdown highlighting is a host closure;
message lists and code views virtualize through host state. An image reference
therefore needs an opaque, policy-checked resource id that the host resolves.

**Emitted actions.** `MarkdownEvent` reports link click, image request, code
copy, and more request. `MessageList` reports retry by message id and forwards
`MarkdownEvent` with the message id. `ImageViewerEvent` reports fit change,
step by image id, and image request. `TransportEvent` reports play, pause,
seek preview, seek request, volume, mute, speed, and track step. `CodeView`
performs its built-in whole-text copy locally and emits no public event.

**Verdict.** Plain text, Markdown, code, messages, and transport controls are
good v1 candidates. Host-resolved images and attachments need a resource
capability; arbitrary element suppliers are not wire vocabulary.

### Agent run

**Host projection.** A run needs stable run, step, tool-call, approval,
permission, cost, and timeline identities; truthful states and reasons;
bounded caller text; ordering and run-length knowledge; declarative detail;
and explicit retry, approval, permission, expansion, and selection actions.
The host may compose these records into a timeline card without exposing an
agent product model to GPUI Box Kit.

**Current Rust types.** Product-neutral agent primitives now include:

- `ToolBody`, `ToolOutput`, `ToolCallState`, `ToolFamily`, `Elapsed`, and
  `ToolCall` in `agent/tool_call.rs`;
- `Reasoning` and `ThinkingBlock` in `agent/thinking.rs`;
- `AlwaysScope`, `ApprovalDecision`, `ApprovalStatus`, `ApprovalEvent`, and
  `ApprovalPrompt` in `agent/approval.rs`;
- `PermissionState`, `PermissionSource`, `PermissionEntry`,
  `PermissionAction`, `PermissionSubject`, `PermissionChange`, and
  `PermissionMatrix` in `agent/permission.rs`;
- `ServerState`, `OfferingKind`, `Offering`, `Catalog`, `ServerEntry`, and
  `ServerList` in `agent/server_list.rs`;
- `PersonaExpression`, `VoiceState`, `VoiceSample`, `DialogueChoice`,
  `DialogueTurn`, `PersonaDialogueEvent`, `PersonaPortrait`, `VoiceReactive`,
  and `PersonaDialogue` in `agent/persona.rs`; and
- `Basis`, `Quantity`, `Reading`, `Limit`, `LastVerified`, `CostLine`,
  `CostMeter`, and `ContextGauge` in `agent/cost.rs`.

`EntryTime`, `TimelineEntry`, `TimelineGroup`, and `Timeline` in
`crates/gpui-kit/src/display/timeline.rs` provide the chronological shell.

Most data shapes are trivially mirrorable and deliberately preserve pending,
unavailable, failed, refused, expired, superseded, withheld, and unknown as
different facts. None derives serde. `Step` and `TimelineEntry` contain
`AnyElement` details. `ApprovalPrompt` is an entity view with focus handles;
the remaining interactive builders store callbacks over `Window` and `App`.

**Emitted actions.** `ToolCall` reports retry and the requested expanded state;
`ThinkingBlock` reports the requested expanded state; `ApprovalEvent` reports approved scope or declined;
`PermissionMatrix` reports `PermissionChange`; and `ServerList` reports select,
retry, and expanded state. `PersonaDialogue` reports choices and unchanged
Markdown events with turn identity. Cost views, portraits, voice
meters, and `Timeline` are display-only unless their composed details carry
controls.

**Verdict.** The vocabulary pieces are unusually strong and truthful. The gap
is an aggregate, bounded composition contract for one run or timeline card,
not the absence of agent components.

### Game experience

**Host projection.** A product-neutral game surface needs stable member,
gauge, objective, ability, reward, and reward-item identities; explicit state,
progress, charges, cost, shortcut, refusal, and availability facts; resolved
portrait/item resource ids; and select, activate, reveal, and claim actions.
Rules, progression, inventory, outcome, persistence, transport, and input maps
remain outside the visual vocabulary.

**Current Rust types.** `GameFraction`, `PartyGaugeState`, `PartyGauge`,
`PartyMember`, `PartySnapshot`, `ObjectiveState`, `Objective`,
`ObjectiveSnapshot`, `AbilityCharges`, `AbilityState`, `Ability`, `AbilitySet`,
`RewardItem`, `RewardState`, and `RewardSnapshot` are in
`crates/gpui-kit/src/game/model.rs`. `PartyRoster`, `ObjectiveTracker`,
`AbilityBar`, `RewardReveal`, and their event enums are in
`game/presentation.rs`.

The identity, snapshot, state, validation, and event shapes are mirrorable but
deliberately do not derive serde. Resolved image paths, `Icon`, `Hsla`, and
`EffectPlan` are renderer values: a wire contract should carry opaque
policy-checked resource ids and a semantic visual cue, then let the host select
the icon, tint, and bounded effect plan. `EffectPlan::cinematic_recipe` then
selects Box-owned asset slots, timing, poster, RTL, and fallback policy; the
host may resolve a slot to bytes and route typed `DotLottieRequest` intents,
but the wire vocabulary still must not expose arbitrary particle, shader,
duration, animation filename, state-machine program, or third-party runtime
types.

**Emitted actions.** `PartyRosterEvent` and `ObjectiveTrackerEvent` report
selection by business identity. `AbilityBarEvent::Activate` is intent only and
is installed only for `Ready`; it changes no cooldown, charge, cost, target, or
outcome. `RewardRevealEvent` reports reveal or claim requests; it does not
change the reward state or inventory. Duplicate and malformed identities
produce issue presentation and no identity-indexed action surface.

**Verdict.** This is a strong DTO candidate because the complete reusable
state-to-visual mapping now sits behind typed snapshots. The adapter should
mirror those records, preserve validation failures, map semantic cues through
host effect policy, and wait for the next authoritative snapshot after every
request.

### Controls / display

**Host projection.** Every control needs a stable node, accessible label,
committed state, disabled/refusal state, and a stable action id. Every display
node needs bounded text, a semantic tone, an allowlisted icon, and truthful
loading/empty/unavailable/error/stale state. Visual size and variant may be
portable hints; theme values and motion timing are not.

**Current Rust types.** The main controls are `Button`, `IconButton`, and
`ButtonGroup` in `controls/button.rs`; `Checkbox`, `Radio`, and `Switch` in
`controls/toggle.rs`; `Toggle`, `ToggleItem`, `ToggleSelection`, and
`ToggleGroup` in `controls/toggle_button.rs`; `Slider` in `controls/slider.rs`;
`Segment` and `SegmentedControl` in `controls/segmented.rs`; `SplitButton` in
`controls/split_button.rs`; the form controls listed above;
`KeybindingRecorderEvent` and `KeybindingRecorder` in
`controls/keybinding_recorder.rs`; `CopyState`, `CopyEvent`, and `CopyButton`
in `controls/copy_button.rs`; `FilterCondition`, `ResultCount`, and `FilterBar`
in `controls/filter_bar.rs`; `DropzoneState` and `Dropzone` in
`controls/dropzone.rs`; and `UploadState`, `Upload`, `OverallProgress`, and
`UploadList` in `controls/upload_list.rs`.

Display contracts include `Tone` and `Badge` in `display/badge.rs`;
`StatusDot`, `StatusLine`, and `Callout` in `display/status.rs`; `EmptyKind` and
`EmptyState` in `display/empty.rs`; `ProgressBar` and `ProgressCircle` in their
display modules; `PulseLoader`, `Spinner`, `Skeleton`, `BarLoader`,
`LoadMoreState`, and `LoadMore` in `display/loading.rs`; `DescriptionValue`, `DescriptionItem`, and
`DescriptionList` in `display/description_list.rs`; and `FailurePanel` in
`display/failure_panel.rs`. `AnimatedNumber`, `Avatar`, `Card`, `ListRow`,
`HighlightedText`, and `Tag` are in the corresponding modules under
`crates/gpui-kit/src/display/`.

The state enums and scalar payloads are mirrorable and not serialized. All
renderers use `Ident`, `SharedString`, theme, or GPUI elements. `EmptyState`
accepts an arbitrary action element; `Card` and `ListRow` accept arbitrary
children or callbacks. `AnimatedNumber` takes a formatter closure and host
motion. `Avatar::image` is a host-resolved path or URI. `CopyButton` owns
clipboard runtime and a copier closure. `KeybindingRecorder` contains focus
runtime and reports captures in GPUI keystroke syntax. `SplitButton` contains
`Entity<Menu>`. `Dropzone` reports `DragItem` or GPUI `ExternalPaths`, which
must never be sent to an out-of-process plugin as though they were ordinary
JSON values. The host should issue an opaque file capability after applying
its permission policy.

**Emitted actions.** Buttons activate; toggles and switches report the requested
boolean; toggle groups and segmented controls report ids; split buttons report
their primary action or `MenuEvent::Invoked`; sliders report a number;
keybinding recorders report started/cancelled/captured; filter bars report
add/remove/clear; upload lists report retry/cancel/remove by file id; inline
edit reports edit/commit/cancel; copy emits `CopyEvent`; and `FailurePanel`
reports retry. Display-only status, badge, loading, progress, and description
components emit nothing.

Motion is host rendering, not plugin state. `MotionSpec`, `Transition`,
`Presence`, and `Flipping` in `crates/gpui-kit/src/motion/` respect host theme
and reduced-motion state and must not become plugin-set timing or animation
objects.

**Verdict.** This is a broad renderer vocabulary. A small number of bounded
wire variants can cover most of it; runtime capabilities such as file drop,
clipboard, focus, and motion stay behind the host adapter.

## The current serializable boundary is semantic output

`Role`, `Rect`, `Node`, and `Snapshot` in
`crates/gpui-kit-semantics/src/lib.rs` derive `Serialize` and `Deserialize`.
`Role` already covers buttons, links, tabs, inputs, dialogs, status, checkbox,
radio, switch, slider, table, cell, tree, progress, toolbar, combobox, option,
form, field, image, and drag. `Node` uses `#[serde(default)]` and omits many
newer optional or false fields, which is a useful additive snapshot practice.

That contract describes what the host rendered. It is not a plugin input
schema. The installed `SemanticCoordinator` now owns a stable
`WindowSemanticContext` per GPUI `WindowId`; generations, duplicate evidence,
snapshots, and close cleanup cannot cross windows. `NodeSpec` contains
`SharedString` and optional `FocusHandle`; the `Semantic` trait records actual
bounds during GPUI prepaint. Those remain renderer-side. A GPUI-free
namespace/action envelope remains committed foundation work in
`docs/foundation-roadmap.md`.

`gpui-box-kit-testkit` now provides `present`, `visible`, `actionable`, and `text`
in `crates/gpui-kit-testkit/src/lib.rs`; `Finding`, `Problem`, and `audit` in
`gpui-kit-testkit/src/audit.rs`; and the headless `Harness` in
`gpui-kit-testkit/src/harness.rs`. `Harness` is available as
`gpui_kit_testkit::harness::Harness` only with the off-by-default
`test-support` feature. It can snapshot; click, scroll, and drag against
semantic ids; send keystrokes to the focused control; and advance frames. This
is enough to build plugin surface tests once mounting, window isolation, and
namespace rules are enforced.

## Derive a parallel DTO vocabulary

### Options

| Approach | Benefit | Cost and risk | Recommendation |
|---|---|---|---|
| Derive serde on existing types | Minimal duplication for a few scalar enums and records | Most component fields are private and GPUI-bound. Deriving would expose renderer layout, closures cannot serialize, and default enum JSON would become an accidental ABI. Renderer refactors would become wire breaks. | Use only for a future type designed as a DTO from birth; do not derive on current component views. |
| Parallel GPUI-free Rust DTOs, then generate JSON Schema | Gives one typed source for data, defaults, bounds, docs, actions, and schema artifacts while keeping the renderer free to evolve | Requires explicit adapter and compatibility tests; duplicates some scalar enums on purpose | **Recommended.** The duplication is the seam, not waste. |
| Schema-first code generation | Makes JSON the language-neutral source and can generate several SDKs | Generated Rust is awkward at the GPUI renderer boundary; semantic validation and host capability rules still need handwritten code; early schema churn spreads into every generated target | Revisit after the vocabulary is mature and multiple non-Rust SDKs need generated models. Do not begin here. |

The DTO crate should depend on serde and a JSON Schema generator, but not on
GPUI, theme, slots, JSON-RPC, host services, or product models. Check in a
deterministic schema for every published vocabulary minor. Put the vocabulary
version in the surface envelope or schema id, independently of the GPUI Box Kit
crate version and the host's plugin protocol version.

```text
┌─────────────┐    ┌──────────────────┐    ┌──────────────┐    ┌──────────┐
│ Plugin JSON │───▶│ versioned DTO +  │───▶│ host adapter │───▶│ GPUI Box Kit │
└──────┬──────┘    │ schema validation│    └──────┬───────┘    └────┬─────┘
       │           └──────────────────┘           │                 │
       │                                          │ typed callback  │
       │           ┌──────────────────┐           ◀─────────────────┘
       └───────────│ typed JSON action│◀──────────┘
                   └──────────────────┘
```

The host adapter owns four translations:

1. local plugin node keys into qualified semantic ids;
2. DTO scalar values into current renderer enums and builders;
3. declarative child nodes into `AnyElement` and row render closures; and
4. component events into one typed action envelope containing surface, node,
   action id, action kind, and bounded payload.

Unknown node and action kinds need an explicit unsupported path. A closed Rust
enum that fails deserialization before capability negotiation is not a
forward-compatible wire enum. Either dispatch on an open kind string and retain
an unknown payload, or guarantee through negotiated capabilities that an older
host is never sent a newer kind. Unsupported content is a refusal, not empty
content.

### Worked simple example: status action

This is an illustrative future JSON record, not a current Rust type:

```json
{
  "vocabulary": "gpui-box.ui/1.0",
  "kind": "status-item",
  "node": "sync.open",
  "presentation": {
    "kind": "action",
    "label": "Open sync status",
    "icon": "refresh"
  },
  "disabled": false,
  "action": "open-sync"
}
```

The host qualifies `sync.open` as, for example,
`plugin.example.sync.open`, maps the allowlisted icon, constructs
`StatusItem::action` from `crates/gpui-kit/src/layout/status_bar.rs`, and calls
`StatusItem::on_click` only when an action is present and enabled. Placement in
`StatusGroup` is host slot state and is not in this record. Activation becomes:

```json
{
  "vocabulary": "gpui-box.ui/1.0",
  "kind": "activate",
  "node": "sync.open",
  "action": "open-sync",
  "payload": null
}
```

The plugin returns a new ViewModel if activation changes anything. The host
does not optimistically rewrite a status item. A display-only state item uses
`StatusItem::state`; it carries no action and installs no callback.

### Worked hard example: DataGrid

The grid DTO must replace `AnyElement` cells and render closures with bounded
data. One possible fragment is:

```json
{
  "vocabulary": "gpui-box.ui/1.0",
  "kind": "data-grid",
  "node": "jobs",
  "loaded": 1,
  "total": 42,
  "columns": [
    {
      "id": "name",
      "label": "Job",
      "width": { "kind": "flex", "share": 1.0 },
      "sortable": true
    },
    {
      "id": "state",
      "label": "State",
      "width": { "kind": "fixed", "px": 112 },
      "sortable": true
    }
  ],
  "rows": [
    {
      "id": "job-42",
      "label": "Index workspace",
      "cells": {
        "name": { "kind": "text", "text": "Index workspace" },
        "state": { "kind": "badge", "label": "Running", "tone": "info" }
      }
    }
  ],
  "sort": { "column": "name", "direction": "ascending" },
  "selection": { "kind": "single", "rows": ["job-42"] },
  "loading": false
}
```

The adapter validates uniqueness and references, maps columns to `GridColumn`,
captures the validated row vector in the closure required by `DataGrid::new`,
and maps each bounded cell variant to a host-built `Cell`. It never accepts a
serialized GPUI element. Loaded count and total remain distinct; a select-all
over loaded rows must not claim every row that exists.

A sort callback becomes intent, not a local sort:

```json
{
  "vocabulary": "gpui-box.ui/1.0",
  "kind": "grid-sort-requested",
  "node": "jobs",
  "action": "sort-jobs",
  "payload": {
    "column": "state",
    "direction": "descending"
  }
}
```

The same rule covers selection, resize, fit, reorder, expansion, and edit. The
plugin returns authoritative state; a refusal leaves the old order, selection,
or value visible. The schema must bound columns, loaded rows, cell text,
nesting, and total payload bytes before GPUI sees them. Semantic ids derive
from `jobs`, `job-42`, and column ids, never row positions.

## Vocabulary evolution is additive within a major

Once published, every ViewModel field, node kind, enum value, action kind, and
action payload in a vocabulary major line is additive-only.

1. Do not remove or rename a published field, kind, value, or action.
2. Do not reuse or repurpose one. Meaning, unit, default, accepted domain,
   disabled behavior, authority owner, and action effect are part of the ABI.
3. Add fields as optional or defaultable. Missing and explicit default must
   have a documented, stable meaning. Older hosts must ignore unknown fields
   inside a known record, or negotiation must guarantee they are never sent
   those fields; a closed JSON Schema cannot silently decide otherwise.
4. Add kinds and enum values only with an unknown/unsupported strategy and
   capability negotiation. A new value is not additive if an old host cannot
   deserialize the enclosing record.
5. Do not tighten validation for data an earlier minor accepted. Add a new
   field or kind when a stricter contract is needed.
6. Deprecation is documentation, not removal. Preserve old spellings and
   behavior for the major line.
7. Removal, rename, repurposing, or incompatible validation requires a
   vocabulary major bump and migration guide.
8. Vocabulary version, renderer crate version, semantic snapshot version, and
   host plugin protocol version are independent.
9. Diff generated schemas in CI. Keep fixtures proving that the newest host
   accepts every prior minor, and that an older host either ignores permitted
   newer fields or receives an explicit negotiated unsupported result.

Current repository practice is safe for the published Rust crates but not yet
sufficient for this promise:

- The `0.1.x` cohort treats Rust API, token key, and semantic id removal or
  rename as documented breaking changes; it does not preserve them additively
  within a vocabulary major.
- `CONTRIBUTING.md` asks for migration impact and changelog entries. It permits
  a breaking migration where the vocabulary must retain the old contract until
  a major bump.
- Most component enums are exhaustive and have no wire unknown-value policy.
  `Role` is serde-serialized but is also a closed enum, so a future role can
  still fail in an older deserializer.
- Renderer builders expose required constructor arguments, callbacks, and
  arbitrary elements. Those APIs may evolve with Rust; they cannot be the
  additive wire schema.
- There is no generated component schema, vocabulary version, compatibility
  fixture set, or schema-diff gate.

These are pre-publication gaps, not violations of an existing ABI promise.

## Plugin-mounted semantic trees

The semantic tree is capable enough to assert plugin UI, but mounting needs a
namespace contract.

1. A plugin supplies a local stable node key. The host validates it and
   qualifies it as `plugin.<plugin-id>.<node>`, for example
   `plugin.example.settings.refresh`.
2. Plugin id and node segments use a restricted grammar. Empty segments,
   traversal, reserved host prefixes, user-generated text, and unbounded input
   are rejected. Qualification is a constructor, not string concatenation
   scattered through adapters.
3. A repeated mount adds a stable surface-instance segment beneath the plugin
   prefix. Two mounted nodes may not alias. Business identity, not list
   position, render order, or a random id, names repeated rows and cells.
4. Plugin subtrees participate in the host window's semantic frame. They
   neither install a competing coordinator nor call `begin_frame`. The host
   root opens the installed coordinator's per-window frame once. Identical
   local component ids in different windows therefore cannot collide, while
   repeated plugin mounts in one window still require the namespace contract
   below.
5. The host assigns the mount root. Parent ids stay under that root; a plugin
   cannot parent into another plugin or arbitrary host UI.
6. Every user-visible action and assertion target receives a semantic node.
   The local DTO node routes both semantics and actions, while action ids remain
   separate so labels or behavior can change without changing identity.
7. Bounded or source-backed surfaces publish only rows they render.
   Materialized `Table::rows`, and `MessageList` or `CodeView` without a
   viewport bound, may render all applicable rows; `CodeView` publishes row
   nodes only for marked lines. `DataGrid` publishes its loaded count at the
   root, not its optional total. Tests currently scroll a named semantic
   container by pixels or use host viewport helpers by row index; business ids
   identify assertions and actions, not the scrolling API.
8. Secret fields, copy payloads, tool arguments/results, raw paths, and
   unrestricted user text do not enter semantic text. Redaction is a second
   line of defence, not permission to publish the value first.

Current plugin-semantic gaps:

- There is no validated semantic-id or plugin-namespace type.
  `NodeSpec::new` accepts an unchecked string.
- Duplicate publications remain in registration order so `audit` can report
  every owner, but there is not yet a mount owner field in that diagnostic.
- `Snapshot::under` is raw prefix matching, not segment-aware namespace
  matching.
- `audit` catches empty ids, a narrow trailing-number positional pattern,
  duplicates present in a snapshot, unnamed actions, invalid ranges, likely
  secrets, and zero-sized visible nodes. It does not enforce namespace
  ownership, parent containment, or stable surface instance identity.
- The semantic protocol has no action id. End-to-end tests still need a host
  action recorder that proves clicking a semantic node emitted the expected
  plugin action and bounded payload.
- Probe-created nodes redact `text` and `value`, but `Snapshot::redacted`
  re-redacts only `text`, and `audit` checks likely secrets only in `text`.
  Directly constructed snapshots therefore have a value-field redaction and
  audit hole; adapters must prevent sensitive values at construction time.
- Plugin adapters need coverage tables proving that every exposed component
  action and assertion target publishes an appropriate `Role` and state.

Add tests for namespace parsing, reserved segments, collisions, mount
instances, parent containment, frame replacement, unmount teardown,
virtualized business ids, secret redaction, and action routing. Use
`gpui_kit_testkit::harness::Harness` for behavior and `audit` for tree
invariants; source text assertions are not evidence.

## Gaps ranked for plugin ABI v1

1. **No versioned, GPUI-free DTO crate or generated schema.** Nothing in the
   current component API can be accepted as plugin JSON without inventing an
   ABI in the host. This blocks every family.
2. **No bounded declarative composition grammar.** Dock, wizard, dialog,
   settings, toolbar, cards/list rows, list items, timeline detail, DataGrid
   cells, and agent steps all use `AnyElement` or render closures. Until those
   children are closed DTO variants, the host cannot render general plugin
   surfaces safely.
3. **No uniform typed action envelope.** Current components report through
   several event enums and callback signatures. The wire needs stable node and
   action ids, typed bounded payloads, disabled/refusal rules, and one adapter
   path back to the plugin.
4. **No enforced plugin semantic namespace or mount ownership.** The
   semantic/testkit foundation now isolates windows and preserves duplicate
   evidence, but unchecked ids and unowned same-window collisions prevent
   reliable end-to-end assertions for independently mounted plugins.
5. **No DataGrid wire projection and loaded-row protocol.** For a v1 that
   promises operational tables, arbitrary cells, row/detail closures, drag
   reorder intent, and loaded-versus-total selection need one bounded schema.
   A smaller v1 may defer this family explicitly.

The next gaps are a single agent-run/timeline-card aggregate, host-resolved
image/attachment/file capabilities, and a portable date capability over
`DateAdapter`. `DataGrid` now carries one shared horizontal viewport and a
frozen leading group, but still needs a bounded wire projection before plugins
can supply its arbitrary cells and loaded-row protocol. GPUI's read-only text
primitive covers one shaped value, while a drag spanning separately mounted or
virtualized document rows remains unsupported. Those are real component
limitations, but they need not block a first vocabulary if its published
capabilities say so.

Slot registration, placement, command palette integration, JSON-RPC, and the
WebView widget are host work, not missing GPUI Box Kit components.
