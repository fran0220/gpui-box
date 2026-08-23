# Components

Every component derives its GPUI element id and its semantic assertion id from
one caller-supplied `Ident`, reads the theme from the application context, and
publishes a semantic node during prepaint. Builders are `RenderOnce`; anything
that must survive a frame is a view.

No component holds a word a reader reads. Text this library authors — `Copy`,
`Try again`, `No rows`, `Next page` — is named by a
`gpui_kit::strings::StringKey` and read from the installed catalogue at render
time, the same way a colour is read from the theme. A host that installs
nothing gets the English compiled into the binary; a host that installs some
entries gets its own words for those and English for the rest, so a label is
never blank. Text the *caller* supplied is shown verbatim and outranks the
catalogue: a refusal's reason, a column header, a month's name, and an
explicit `handle_label` are the host's, not the library's.

`cargo run -p xtask -- strings check`, which runs inside `gate`, fails when a
component grows a literal a reader could read.

## Controls

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `Button` | builder | click | No handler is installed while disabled or loading |
| `TextInput` | view | change, submit, cancel, focus, blur | Grapheme-aware editing, input-method composition, masking, length limit |
| `PasswordInput` | view | change, submit, cancel, backspace at start, focus, blur | One sensitive `TextInput` editor plus a keyboard-operable reveal action. Reveal changes only the visual mask: semantic and AccessKit values/text runs, Debug, and clipboard copy/cut remain redacted |
| `OneTimeCodeInput` | view | change, submit | One sensitive `TextInput` editor rendered as 1–12 caller-chosen slots (six by default), not one field per slot. A slot accepts one Unicode grapheme; semantics and AccessKit publish only redaction plus the current/target length shape |
| `TextArea` | view | change, submit, cancel, paste, move up/down, focus, blur | Wrapped multi-line editing. `Enter::Opens` inserts a line and the primary modifier submits; `Enter::Submits` swaps the two, which is what a composer is. The vertical arrows can be claimed by a surface listing over the area. A paste that is not text reports what arrived. Motion follows visual rows with a preserved goal column, and the frame grows from `rows` to `max_rows` before it scrolls. `measured()` publishes what the last layout pass found, for a host whose frame changes shape around the text rather than growing by rows. `Frame::Host` hands the well, padding and type to a host that already drew a frame, so a composer's pill is one surface rather than two. `detached()` builds one where there is no window, and it starts watching focus at its first render |
| `Select` | view | selected, opened, closed | Owns only whether the menu is open. `name` supplies the caller-owned accessible label independently of the answer or placeholder |
| `Cascader`, `CascaderOption` | view | selected, expanded, retry, opened, closed | A hierarchical choice with stable caller-owned option identities. Only the open path is transient; the accepted value and hierarchy remain caller-owned. Child loading, empty, unavailable, error, and ready states stay distinct, and RTL swaps the arrows that enter and leave a branch |
| `Checkbox` | builder | next state | Supports a mixed state for a group that disagrees |
| `Radio` | builder | selection | The group is owned by the caller |
| `Switch` | builder | next state | For changes that take effect at once |
| `Slider` | builder | value on the step grid | Pointer and keyboard |
| `IconButton` | builder | click | A glyph-only action. The accessible name is a required argument, because a glyph nobody can name is a button nobody can reach |
| `ButtonGroup` | builder | — | Adjacent related actions sharing one frame. It reports nothing: every action inside still reports itself, and the group only decides where the corners are and forces one control size |
| `SplitButton` | view | click on the default action; the menu reports the alternatives | The action and the arrow are separate targets with separate ids. `default_disabled` refuses the usual thing while leaving the alternatives reachable |
| `FormField` | builder | — | Label, description, and error around a caller-supplied control. The label carries `labels` so a test that knows only the wording can reach the control |
| `NumberInput` | view | change, unparsable text, submit | Typing, arrow and page keys, and step buttons. It never clamps: a value outside the range is shown as it is and published `invalid` |
| `SegmentedControl` | builder | the segment that was picked | A single-choice strip. Left, right, home, and end move over refused segments and stop at the ends, because a strip has ends |
| `Toggle` | builder | the state pressing asks for | A button that stays in, published as a `Button` carrying a checked state where out is `false` rather than absent. Distinct from `Switch`: a switch is a setting that applies, a toggle changes what the next thing you do means |
| `ToggleGroup`, `ToggleItem` | builder | the whole set the group should hold next, and which toggle was acted on | `ToggleSelection::Any` takes several; `AtMostOne` takes one or **none**. It does not reimplement `SegmentedControl`, which covers the case where exactly one is required and there is no move that empties it. Every toggle is its own tab stop |
| `CopyButton` | view | copied, or failed with a reason | Copies caller-supplied text and confirms truthfully. It never publishes its payload, the confirmation times out and the refusal does not, and what it can and cannot know about the clipboard is stated below |
| `Combobox` | view | selected, custom, opened, closed | A `Select` you can type into. `name` labels both the combobox and its editable query target. Escape puts the query back to the current answer and reports nothing. A query nothing answers reports nothing unless `allow_custom` |
| `TagInput` | view | added, removed, duplicate, refused | Enter or comma commits a token. The first backspace in an empty field singles out the last tag and the second removes it. A duplicate and a full field are refusals shown where the typist is looking |
| `SettingsRow`, `SettingsSection` | builder | — | One setting per row: name and description on the left, the caller's control on the right. A row that is managed elsewhere, or that belongs to a section which does not apply here, never renders the control at all |
| `FilterBar` | builder | add, remove one condition, clear them all | The conditions are the caller's, and so is the result count. Counting, a known count, a count nobody established, and a count the host refused are four different things |
| `InlineEdit` | view | edit requested, commit, cancel | Text that becomes a field where it stands. The component never opens itself, never applies a commit, and a refused save keeps what was typed |
| `KeybindingRecorder` | view | recording started, a captured keystroke, cancelled | Captures the next keystroke instead of acting on it, and reports it in GPUI's own syntax so it goes straight into a keymap. A modifier alone is not a keystroke, escape ends recording rather than being captured, and a conflict is the reason the host found |
| `KeymapEditor` | view | add a captured binding, remove a binding, reset a command, recording cancelled | Coordinates command identity, label, context, defaults, effective bindings, caller-supplied search metadata, and one shared active recorder. Multiple bindings keep caller-owned identities. Conflict, provenance, and refusal are facts supplied by the caller; the editor judges, applies, persists, and executes nothing |
| `SearchField` | view | the query, next, previous, cancelled, and the two match rules | A find field over `TextInput` with a hit count beside it. Unsearched, counting, none, a known total, a count that stopped early, and a host that could not search are six different things, and a step with nowhere to go installs no handler |
| `FindReplace` | view | replace one, and replace all with the number it stated | `SearchField` with a replacement field under it. Replace all carries its count on the control before it is taken, and a count nobody established — too many, still counting, unavailable — leaves it refused with the reason beside it |
| `UploadList`, `Upload` | builder | a file to retry, one to stop, one to take off the list | Files on their way somewhere, over the `Dropzone` that took them. A refusal is not a failure and is offered no retry; overall progress is claimed only when every file still in flight declared an extent |
| `field_shell`, `FieldState` | helper | — | The one border, background, and focus treatment every editable control draws. A composed field — `NumberInput`, `Combobox`, `TagInput` — wraps a bare input in one of these rather than nesting two frames |

### Sensitive text remains one editor

`PasswordInput` and `OneTimeCodeInput` do not duplicate editing. Each owns one
`TextInput` entity, one focus handle, one selection, one caret, and one input
method composition. Password reveal is visual transient state. One-time-code
slots are visual policy over the same editor, with hit testing and input-method
bounds mapped across the complete segmented surface.

Both controls return the typed value only to their caller and emit caller-owned
change/submit events whose `Debug` output is redacted. Deterministic semantics,
GPUI's AccessKit values and text runs, clipboard copy/cut, and component Debug
carry no credential or code. `auth-sign-in` and `auth-verification` demonstrate
composition with `Card`, `FormField`, `Callout`, and generic caller-supplied
actions. They define no account, provider, network, or credential policy.

## Display

| Component | Kind | Notes |
|---|---|---|
| `Badge`, `StatusDot`, `StatusLine`, `Callout` | builder | Status vocabulary |
| `Card`, `CardHeader`, `ListRow` | builder | Grouping. See [The card is the container, and there is one of it](#the-card-is-the-container-and-there-is-one-of-it) |
| `ProgressBar` | builder | Reports a position only when the extent is known. In-flight fill is the working signature from `color.loader.gradient`, not accent |
| `AnimatedNumber` | builder | Counts to a new value, and publishes the target from the frame it changes: a number in flight is not a fact. A caller-supplied format function decides the text |
| `Tag` | builder | Removal exists only when removal is allowed. Accepts a caller-owned `tint` the way `Badge` does; the published tone name is unchanged |
| `Avatar` | builder | Initials fallback, blank when there is no name |
| `Divider` | builder | Optional caption |
| `EmptyState` | builder | Names which of empty, unstarted, unavailable, or failed holds |
| `PulseLoader`, `GradientSpinner`, `Skeleton` | builder | Publish a busy indeterminate node. All three paint the working signature |
| `ProgressCircle` | builder | The ring form of `ProgressBar`, over the same state and the same working signature. A position only when the extent is known; an unknown extent travels a short arc rather than tinting part of the ring |
| `DescriptionList` | builder | Term and value pairs for a detail page. Unknown, not applicable, and redacted are three different facts, and a redacted value carries only its shape |
| `Timeline` | builder | A chronological feed. Every time and every day heading is a string the caller already formatted, and an entry whose time nobody knows says so |
| `HighlightedText` | builder | Marks caller-given byte ranges in caller-given text. It searches nothing: the ranges are the caller's, the current one is drawn differently from the others rather than more strongly, and a range naming no real slice costs its mark and not the line |
| `Sparkline` | builder | A narrow accessible trend reading. The caller supplies points already normalized into the `0..=1` square and exact label/current/minimum/maximum text; invalid points are skipped, no scale or locale is inferred, and loading, empty, unavailable, error, stale, and ready are separate states |
| `LineChart` | builder | A cartesian reading over one or more host-owned series. `ChartPoint` carries stable business identity plus exact host-formatted label/value text. Keyed transitions and presence animate geometry while semantics update immediately; `area` supplies the renderer-backed gradient fill and `crosshair` supplies pointer/keyboard navigation with a business-id callback. Loading, empty, unavailable, error, stale-with-last-verified-data, and ready stay distinct |
| `BarChart` | builder | Categorized bars over one host-owned series. Bars enter, update, and exit by `ChartPoint` identity and expose the same truthful state contract as `LineChart` |
| `FailurePanel` | builder | A region the host could not produce, in the host's own words. Not an error boundary and deliberately not named one: GPUI has no fallible render and no catchable render panic, so this takes a failure the host is already holding, through `from_result`. It publishes `failed`, never empty |
| `Icon` | builder | A glyph from the bundled catalog, sized from the `control.*` glyph step and coloured from a semantic role rather than an `Hsla`. Emits nothing: a glyph that can be clicked is `IconButton`. Decorative by default and published only when named, so a glyph that repeats the label beside it is not announced twice |

### The card is the container, and there is one of it

A card is a surface that groups content, so it owns the whole vocabulary of
one: a `media` band flush to its edges, a `CardHeader` with a title, an
optional subtitle and an optional control, body content, a `footer`, and
`divided` to put a rule between adjacent regions and between body rows.
A caller reaching for a card gets all of it or none of it, and never a
rectangle it has to finish by hand.

That last sentence is the point of the component rather than a description of
it. Before it existed, `Card` offered a radius, a surface and a click, thirty
nine source files each drew the rest themselves, and no two of them agreed:
`agent/*` padded by `Space::Md` and cast a shadow, `game/*` drew a hairline
and cast none, and the component itself padded by `Space::Lg`. Three answers
to one question is not a style; it is the absence of one. A container that
cannot carry a title is a container every caller has to escape.

`variant` names how the card separates itself from what is behind it:

| Variant | Evidence | Reach for it when |
|---|---|---|
| `Elevated` | A shadow | One card, or a few, on the surface behind them |
| `Outlined` | A hairline, no shadow | A grid. Shadows are drawn per card and know nothing of each other, so a dozen at close range stack into a wash that reads as one smudged region instead of twelve things |
| `Ghost` | Neither | Structure, padding, identity and interaction without claiming to be a plane |

The colour step is doing the separating in all three cases: a theme that meets
the surface separation floor in `docs/token-model.md` has already made the
boundary legible, and a variant chooses the second piece of evidence on top of
it. This is why a card carries no line by default. The library reserves lines
for what they alone can say — focus, invalidity, a drop target — and `divided`
is the deliberate exception, because it draws a line *between two pieces of
content on one surface* rather than around the surface.

`selected` is a wash and a rail at the reading edge, both painted inside, so
choosing a card moves nothing around it.
`disabled` publishes the refusal, changes the text tone, and installs no
handler at all; it does not fade the card out from under the reader, because
unavailable is a fact to be read and not a thing to be hidden. A card that is
itself one action drops its header's control, since two targets inside one
target is a click whose outcome depends on which pixel it landed on.

`ListRow` carries `leading` and `trailing` slots rather than relying on child
order, so a list of rows lines its text up whether or not a given row has an
icon or a badge. A row takes the press response and not the hover lift: a row
that rose would leave the card it belongs to.

A component that is already a card but owns a richer semantic node than a
grouping — `ToolCallCard` is a tool invocation, `ApprovalRequest` is a form —
cannot be wrapped in one, so the shell itself is the shared piece:
`StyledExt::card_surface(theme, variant)` is the single definition of what a
card is made of, and `Card` is a caller-facing composition on top of it. That
is the whole of the fix: those components each drew the shell by hand before,
and a card that means the same thing and is drawn two ways is two components
wearing one name.

## Agent experience

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `AgentAvatar`, `AgentAppearance` | builder, data | — | Renders a caller-owned `AgentSnapshot` as identity, presence, execution halo, and a non-colour execution glyph. Optional image and role tint are separate appearance facts. Busy motion follows reduced-motion policy; a static status mark remains when motion is off |
| `AgentActivityLine` | builder | — | Maps every typed execution, activity, wait, refusal, and terminal state to localized visible wording and a non-colour mark |
| `AgentCard` | builder | selected agent | A complete agent identity, role, activity, and current caller-owned task. It installs no selection handler until the caller provides one |
| `AgentGroup` | builder | — | An RTL-aware overlapping identity group with a truthful `+N` overflow count |
| `AgentRoster` | builder | selected agent | A virtualized roster consuming `AgentSnapshot` and `AgentTaskSnapshot` directly. Selection remains caller-owned and row IDs are agent business identities |
| `SubagentTree` | builder | selected agent and requested disclosure state | Derives only parent-child structure from typed `Spawn` links. Expanded branches and selection remain caller-owned; non-spawn delegation, report, handoff, and dependency links do not silently reparent agents |
| `AgentRunIssues` | builder | — | Renders `AgentRunSnapshot::issues()` as one stable failure notice. Identity-indexed components use it instead of collapsing duplicate or dangling facts into a different roster or topology |
| `AgentRunCanvas`, `AgentRunLayout` | builder, policy | typed subject selection, viewport, and optional arranged subject positions | Projects one valid `AgentRunSnapshot` directly onto `NodeGraph`: agents, tasks, invocation placeholders, every execution state, and all seven relationship kinds keep their typed identity and localized wording. The built-in layered layout is deterministic and RTL-aware. Inspection never installs topology edits; malformed topology renders `AgentRunIssues` before any identity-indexed projection |
| `PersonaPortrait` | builder | — | Builds an expressive large portrait on `AgentAvatar` from caller-owned expression, resolved image, tint, execution, and optional normalized `VoiceSample`. It owns expression marks, voice bars, crop, RTL placement, reduced motion, and an optional policy-resolved `EffectPlan`; it fetches no art and observes no microphone |
| `VoiceReactive` | builder | — | Maps one finite normalized level/envelope and `VoiceState` to the standard accessible meter. Live rendering owns its timeline; reduced motion is static and symmetric; `sample_at` is exact and schedules no frame. Invalid or out-of-range samples are rejected rather than clamped into plausible facts |
| `PersonaDialogue` | builder | choice and Markdown events stamped with turn identity | Composes `PersonaPortrait`, localized agent activity, safe `Markdown`, streaming presentation, and caller-owned choices. Unavailable choices keep their host reason visible and install no action. Selection and dialogue progression remain caller-owned; host-resolved Markdown images and code spans retain the standalone renderer's security boundary |

The presentation layer is not an agent runtime. It consumes an observed
`AgentRunSnapshot`, displays waiting/refused/failed/cancelled as distinct facts,
and reports caller-owned actions. `AgentRunSnapshot::issues()` remains the
source of structural validation: components never repair duplicate identities
or dangling topology into a different run.

### Semantic visual effects

`AgentVisualEvent` reports facts such as `AgentSpawned`,
`DelegationStarted`, `HandoffCommitted`, `ResultAggregated`,
`AgentSucceeded`, `AgentRefused`, `AgentFailed`, and `RewardGranted`. It carries
stable event, surface, target, and optional origin identities; it cannot name a
particle system, colour, shader, duration, or animation recipe.

`EffectPlanner` normalizes those events together with generic `EffectEvent`s.
It chooses a semantic `EffectRecipe` and one of `Static`, `Animated`, or
replay-suppressed presentation. The installed `EffectPolicy` has four complete
quality tiers — `Off`, `Essential`, `Balanced`, and `Cinematic` — plus global
and per-surface frame budgets for events, emitters, particles, and animated
area. Reduced motion and exhausted budgets choose the static recipe; they do
not erase feedback. Stable event identity seeds deterministic rendering and a
bounded, per-surface replay history prevents reconnects or rebuilt trees from
replaying a celebration.

The planner is the policy boundary, not a particle renderer. Components and
agent runtimes provide semantic events. `EffectParticles` consumes the plan and
fills a caller-bounded layer with one deterministic CPU-sampled, atlas-backed
sprite batch. It owns cue topology, theme colors, RTL trace direction, elapsed
time, frame scheduling, and the smaller fixed constellation used for `Static`.
Particle tints stay semantic but are reinforced toward the active text tone,
so tiny marks retain a contrast floor across the standard light/dark surfaces;
an active reduced-motion preference also converts an already-running plan
immediately. `sample_at` exists only for deterministic replay, scrubbers, and
captures. Suppressed plans and zero-emitter recipes paint nothing. Because the
layer is decorative, the target status/control remains the semantic feedback
and the layer adds no hitbox or accessibility node. This keeps graceful
degradation and performance policy out of every downstream chatbot, persona,
or game surface.

`CinematicEffect` consumes the same plan and upgrades it through a semantic
`CinematicRecipe`: Box owns the product-neutral asset slot, duration, poster
sample, directional RTL rule, deterministic timeline, localized semantics,
and particle fallback. The host resolves a slot to `DotLottieAsset` bytes and
prepares a `DotLottieClip`; no public type accepts a URL, path, filename,
third-party runtime value, shader, expression, or arbitrary program. Playback
and state-machine integration use caller-owned `DotLottiePlayback` facts and
typed `DotLottieRequest` intents. The optional `dotlottie` feature provides
`RasterDotLottieAdapter`; core contracts and unavailable/invalid-asset
fallbacks compile without it. The adapter rejects archives outside bounded
encoded/expanded size, compression, entry, canvas, frame, duration, animation,
state-machine, embedded-image-count, source-dimension, or aggregate image-pixel
limits, sanitizes every admitted entry and inspects image headers before
decoding, and returns complete RGBA frames through `RenderImage`.

Persona presentation follows the same boundary. Audio capture, recognition,
playback, portrait download, model expression inference, and dialogue
progression are host capabilities. Box accepts only resolved assets and typed
facts, then owns the reusable visual mapping, deterministic sampling,
accessibility semantics, RTL order, and fallback treatment.

## Game experience

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `PartyRoster` | builder | selected member identity | Consumes `PartySnapshot` and the standard agent/persona model, then owns portrait, activity, gauge, selected, compact-wrap, and RTL presentation. Duplicate member or per-member gauge identities render one issue surface instead of a collapsed roster |
| `ObjectiveTracker` | builder | selected objective identity | Owns state marks, hierarchy indentation, progress, semantics, and RTL from a caller-owned `ObjectiveSnapshot`. Duplicate ids, dangling parents, and cycles refuse the complete identity-indexed projection rather than drawing a believable but invented hierarchy |
| `AbilityBar` | builder | activation request by ability identity | Maps ready, cooldown, disabled, and unavailable facts plus charges, cost, shortcut, and icon into one keyboard-ready control family. Only `Ready` installs an action; requesting activation consumes no charge, starts no cooldown, pays no cost, and claims no result |
| `RewardReveal` | builder | reveal or claim request by reward identity | Hidden, revealed, claimed, and unavailable remain caller-owned facts. Revealed items use business ids, optional resolved art, localized quantity grammar, staggered reduced-motion-aware arrival, and an optional policy-resolved `EffectPlan`. Duplicate item ids render an issue instead of one invented item |

`GameFraction` and `AbilityCharges` reject non-finite, out-of-range, zero-maximum,
and above-maximum facts rather than clamping them into plausible telemetry. The
game module deliberately owns no combat formula, inventory, quest progression,
reward outcome, save data, asset transport, input mapping, or game engine. A
host supplies those facts and handles the typed requests; the components own
the state-to-visual mapping, topology validation, semantic ids, responsive
wrapping, locale hooks, RTL, motion, and graceful effect degradation that a
downstream chatbot or game surface should not have to recreate.

## Navigation

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `Tabs`, `TabItem` | builder | the tab that was picked, and the tab that should be put away | Renders the strip only, never a panel, so no `TabPanel` node is published; the caller renders the body. Left, right, home, and end move between tabs, skipping disabled ones and stopping at the ends. A document tab carries `SaveState`: clean draws nothing, dirty, saving, and a save that failed are three marks. The close control is its own hit target and stops the click travelling, a middle click means the same thing, and a strip with no room does one of two things: `overflow_after` with an `overflow_menu` moves the rest into a menu, or `scrolling` scrolls the strip, fades the edge that has more behind it, and brings a newly chosen tab into view without hauling itself back while the reader is looking elsewhere. Either way the keyboard still reaches every tab |
| `AnchorList`, `Anchor` | builder | the document section that should be navigated to | A reading-order strip of caller-owned in-page anchors. The active anchor never moves itself. Declared overflow moves anchors into a caller-owned `Menu`, which reports `MenuEvent::Invoked` with the same anchor id; without that menu or while the list is disabled, every anchor remains inline rather than being dropped |
| `Accordion` | builder | a section id and the state it should take | A closed section does not render its body at all. `exclusive` changes only what is reported: opening a section also reports a close for every other open one |
| `Collapsible` | builder | the state activating the header asks for | The one-region case, built by handing a single section to an `Accordion` rather than by writing the disclosure again. The header lands at `{ident}.header` |
| `Breadcrumb` | builder | the crumb that was picked, and the ids an ellipsis hides | The last crumb is the current place: it publishes `Text` rather than `Link` and installs no handler. `max_visible` collapses the middle of a long trail and publishes the hidden count |
| `Sidebar` | builder | the place that was picked | Sections, badges, and one level of nesting. Collapsing narrows the drawing, never the substance: a glyph-only rail reaches each label through a `Tooltip` and every item still publishes its full name and its depth |
| `Wizard` | builder | a step to jump to, back, next, or finish | A step strip with the caller's body under it, horizontal or vertical. A step is complete, current, upcoming, blocked, or failed, and the last two say why |
| `UndoHistory`, `HistoryEntry` | builder | the history entry that should be restored | A caller-owned revision list, not an undo stack. Entry order, current identity, descriptions, already-formatted time/source labels, and restore refusals are rendered exactly as supplied. Arrow, Home, and End keys skip refused entries; reporting a jump changes nothing |
| `Pagination` | builder | the page that was asked for | First, previous, next, last, and a numbered range with an ellipsis that says how many pages it stands for. A step with nowhere to go installs no handler. With `PageTotal::Unknown` there is no last-page control, no numbers, and no total in the copy |

### The wizard moves nothing

`Wizard` reports `Step`, `Back`, `Next`, and `Finish`; which step is current
stays with the caller, exactly as `Tabs` never switches its own tab. Only
completed steps are revisitable by default, and a step nobody may jump to
installs no handler. `Blocked` and `Failed` carry the host's reason and publish
it as a child node, because a step that has gone grey for a reason nobody
states is a dead end.

### Undo history owns no undo stack

`UndoHistory` neither records changes nor restores them. The caller supplies
durable entry identities, their order, the current entry, and any refusal to
restore one; `on_jump` reports only the requested identity. Times are already
formatted by the caller because locale and time-zone policy do not belong in
a navigation component. An unavailable revision stays visible with its reason
and installs no action.

### An unknown page count is not a page count

`PageTotal::Known` and `PageTotal::Unknown { has_next }` are different facts. A
host that paginates a cursor knows only whether one more page exists, so that
is all the control claims: it offers next and previous, states "Page 9" with no
total, and publishes no `value` on the container. Rendering an invented last
page would be a number nobody counted.

## Layout

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `DesktopTitlebar` | builder | minimize, toggle-maximize, and close requests | A custom client titlebar with an optional subtitle and caller-owned left and right content. The strip is draggable, supplied content explicitly remains client input, and caption buttons retain native control identities. macOS leaves controls to the native traffic lights; browser builds publish no desktop controls |
| `SplitPane` | builder | the ratio a drag or a keystroke asked for, and the side a double-click would collapse | Minimum sizes become a travel range published on the divider, and a drag past a minimum reports the minimum rather than a value the caller would have to clamp. A pane at ratio 0 or 1 drops its content instead of drawing it at zero size |
| `AspectRatio` | builder | — | A frame that keeps a ratio. `AspectFit` names which dimension the parent decides and the ratio computes the other; when the parent constrains both, `fit` still wins and the overflow is visible rather than silently switched to a contain box |
| `ScrollArea` | builder | — | Scroll position is transient view state, held per identity like `List`. It can instead bind its gutter to any GPUI `ScrollTarget`, so a virtualized list keeps the only scroll position rather than being wrapped in a second one. A gutter is reserved for every enabled axis whether or not a thumb is drawn, so turning a scrollbar on never reflows the content that decided it was needed. The gutter paints nothing and the thumb rises to full weight under the pointer. A soft band fades in at the top once the content is off the top, read straight off the offset rather than animated |
| `ScrollFade` | builder | which edges fade, or `none` | Content fades towards the edges it is scrolled past, per painted primitive rather than under a gradient overlay, which is the only way to say "there is more" over a translucent or frosted surface. The edges are the caller's statement about overflow: a region that hides nothing fades at neither edge and publishes `none` |
| `Toolbar` | builder | — | Groups separated by rules, a spacer, and an overflow menu. Every action inside still reports itself |
| `SplitTree` | builder | the ratio a divider asked for, and the pane a double-click would collapse | However many nested splits the caller declares, as a `SplitLayout` the caller owns. Minimums propagate up the tree, so a divider stops where a leaf far below it would run out of room, and a collapsed leaf is drawn at its rail with no divider beside it |
| `Dock` | builder | a panel that was picked, a panel that was dragged somewhere, a region asked to collapse, and a region divider's share | Panels in a left, centre, right, and bottom region around one another. Region sizes go through `SplitTree` and panel headers are `Tabs` strips, so resizing and dragging are the same two systems used elsewhere. It moves nothing |
| `StatusBar` | builder | a click on an item that has an action | Text, a toned state dot, a progress ring, an action, or a caller-supplied element, in a start, centre, and end group. An item the host gave no state claims none |

### A titlebar asks; the window decides

`DesktopTitlebar` consumes the title and optional subtitle while reading the
window's current maximized state and available platform controls. Clicking a
caption control reports a `DesktopTitlebarEvent`; it does not mutate or remove
the window. A host that accepts `Close` calls `Window::request_close`, which
preserves the window's `on_window_should_close` refusal path. On Windows the
maximize button keeps `WindowControlArea::Max`, so the operating system still
owns Snap Layout; ordinary elements mounted in the titlebar are
`WindowControlArea::Client` and remain clickable instead of becoming drag
handles.

### A layout the host can write down

`SplitLayout` is data, not view state. This crate takes no serialization
dependency, so instead of a derived `Serialize` the layout converts losslessly
to and from a flat `Vec<SplitRecord>` of plain fields through `to_records` and
`from_records`, which a host persists with whatever format it already uses.
`from_records` reports why a set of records is not a tree — no root, two roots,
a duplicate id, a missing parent, a split without exactly two children, records
the root does not reach — rather than silently building something else.

A reported `SplitChange` is still only a request. `SplitLayout::applied` exists
for the host that accepts every change and wants one call to make; a host that
judges them applies the ones it accepts with `with_ratio` and `with_collapsed`.

### The dock moves nothing

Which panels a region holds, which one is on top, whether a region is
collapsed, and how much room it takes are all the caller's. `DockEvent`
names what the typist asked for and the arrangement on screen stays as it was,
so a host that refuses a move keeps showing the layout that still holds. A move
names the panel the dragged one should sit **in front of**, never an index: an
index stops meaning anything the moment the host applies the move.

What the dock deliberately cannot do: a region holding no panels is not drawn,
so it cannot be dropped onto; a panel is drawn in exactly one region, with no
split inside a region and no floating panel; and a collapsed region shows a
rail whose glyphs report both the selection and the request to expand without
applying either. A panel the host cannot show keeps its tab and states the
reason where its content would be, because a panel that vanished would read as
one the workspace never had.

### A status bar never invents reassurance

Every fact in the strip belongs to the host, so an item with no state carries
no state rather than a green dot nobody asked for.
`StatusItem::tracking` reads `AsyncValue` straight: a value whose refresh is in
flight or has failed while a value is still held is drawn with its last
verified text and the word `stale` beside it, and publishes `stale` as its
value. It is never drawn as current. A progress item with neither a fraction
nor a count is drawn as an unknown extent rather than as a ring that happens to
be part full.

### A recorder that cannot bind escape

`KeybindingRecorder` reports `gpui::Keystroke::unparse`, which is exactly what
`gpui::Keystroke::parse` reads and what `Kbd` splits, so a captured binding is
usable without translation. Escape ends recording without capturing: that is
how everything else in this library abandons something in flight, and a
recorder that swallowed it would leave the typist inside a field that eats
every key. The cost is stated rather than hidden — escape cannot be bound
unless the caller turns `allow_escape` on and provides its own way out. A
conflict is never the recorder's judgement: it has no keymap to consult, so it
renders the reason the host found and nothing else.

`KeymapEditor` is the coordination above that primitive rather than another
settings-row skin. It filters the command facts the caller supplied, keeps at
most one recorder active across all visible rows, and reports stable
command-and-binding identities with add, remove, and reset intents. Capturing
does not change the effective bindings on screen. If the host refuses an
intent, it leaves its supplied state unchanged; if it marks a command refused,
the row keeps its bindings and reason but installs no actions. Conflict and
provenance remain caller statements, never conclusions drawn by the component.

### A scrollbar that is absent means there is nothing more

A viewport can mean two different things: the content fits, or there is more
off screen. `ScrollArea` publishes a `Scrollbar` node **only** in the second
case, carrying how far the content reaches and how far it has been scrolled. A
test therefore tells the two apart from the tree, rather than guessing from
what happens to be visible.

Both the divider's travel and the scrollbar's reach are extents only layout
knows, so they are measured during prepaint and published by the following
frame. Tests deliver that frame themselves with `Harness::advance`.

### Toolbar overflow is declared, not measured

A truthful overflow would have to know how wide every item is before deciding
which ones fit, but GPUI measures after the element tree is built and a
toolbar child is an `AnyElement` that can be consumed exactly once — so a
builder cannot measure a child and then still move it into a menu. Guessing at
widths would produce a bar claiming to have dropped items it in fact drew.

So the caller declares the cut with `Toolbar::overflow_after`, and the toolbar
guarantees the part it can: an item past the cut is **moved**, never dropped.
It becomes a row in the overflow `Menu` keeping its identity, its label, and
its refusal, and the trigger publishes how many items went there. With no menu
to move them into, every item is drawn inline, because losing an action is
never the better failure.

## Data

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `List` | builder | the row that was picked | Virtualized over GPUI's uniform list by default. `flowing` switches to the measured variable-height list: rows grow to fit, unseen extent is estimated, and `anchored_to_end` rests short content at the bottom. The caller renders one index at a time and stamps each row with its own identity. Up, down, home, and end move the reported selection, skip refusals, and scroll the reported row into view |
| `Flow` | builder | nothing; the caller's rows report themselves | The virtualized surface `List`'s flowing mode is built from, with no chrome of its own: it draws the element the caller returns and nothing around it, which is what a conversation, a log, or a diff needs. Rows are as tall as what they hold, `keys` keeps a measurement against the row that earned it, and the measured state is registered under the caller's identity so `follow_end`, `scroll_to_row`, `reveal_row`, and `glide_to_row` reach it by name |
| `DiagnosticsList`, `Diagnostic` | builder | filter, selection, diagnostic action, and retry intents | Composes `FilterBar`, `List`, severity `Badge`s, and caller-owned action buttons. Diagnostic identity, location, message, selection, filters, and actions remain caller-owned; the list never opens a file or executes a fix. Loading, empty, unavailable, error, and a ready set with no filter matches remain distinct |
| `Table` | builder | the sort a header click implies, and the row that was picked | Sorting is caller-owned: the table reports `(key, next direction)` and renders whatever order it is handed. Columns are fixed or flex, and the header stays put while the body scrolls. Row rules default on; loading, empty, and a refresh failure stay distinct. Not virtualized — reach for `DataGrid` past a few hundred rows |
| `DataGrid` | builder | a sort, a column width, a column order, a selection change, a disclosure, and a finished edit | The heavyweight tabular surface: virtualized over `uniform_list`, resizable and reorderable columns, a left-pinned group, three selection modes with a truthful select-all, opened rows with a detail region, and cells that become fields. It applies none of it |
| `TreeGrid`, `TreeGridRow` | builder | caller-owned selection and expansion intents | A DataGrid-backed hierarchy over caller-flattened visible rows. Rows supply stable ids, levels, parent ids and branch state. Disclosure and indentation live in the first ordered column. Fixed/flex columns only; no horizontal scrolling or frozen columns |
| `BulkBar` | builder | the wider selection, and the dismissal that clears the selection | Appears over a selection through `Presence`, states the count it actually has, and offers "select all N" as a separate named action when more rows exist than the host has loaded |
| `Tree` | builder | a node id and the disclosure state it should take, and the node that was picked | A collapsed node renders none of its children. Up and down walk visible nodes, right opens a shut branch or descends into an open one, left shuts an open branch or ascends |

### Only rendered rows are published

A virtualized surface holds a viewport, not a data set. A row outside the
viewport is never laid out, has no bounds, and publishes no semantic node, so a
snapshot describes what is on screen and nothing else. The container node
carries the total in `value`: a test asserts that the list holds a thousand
items and drew twelve, rather than pretending the other nine hundred and
eighty-eight are addressable.

Virtualization needs a bounded viewport. `List`, `Table`, `Tree`, `TreeGrid` and
`DataGrid` each take a `visible_rows` bound and draw only the rows that fit;
without one they size themselves to their content and every row is laid out.
That is the right answer for a settings summary and the wrong one for a
hundred thousand log lines.

Two of the four have a second condition. `Table` virtualizes only when it is
given a row source — `Table::rows_from(count, closure)` — because
`Table::rows` hands it elements the caller has already built, and an element
can be laid out once while a `uniform_list` needs to build a row twice in a
frame: once to measure the height, once to draw. `rows_from` is offered
alongside `rows` rather than replacing it, so a table of six settings does not
have to be written as a closure over an index. `Tree` flattens the hierarchy
to the rows a reader could see and virtualizes that, so what it draws follows
what is open; flattening still walks the whole hierarchy each frame, which is
data rather than elements.

A `Tree` reports the number of rows it disclosed in `value`, which keeps three
absences apart: a node under a shut branch is not disclosed, a disclosed node
past the edge of the viewport is counted but not drawn, and a node that is not
in the data at all is neither. A bounded tree can draw a node whose parent has
scrolled off the top; the node still names the parent it has, so a walk down
from the tree will not reach it and a test should name it instead.

`List`, `Tree` and `DataGrid` move the selection with the keyboard over the
whole collection, not over the rows that happen to be drawn, and scroll what
they report into view. `Table` reports only what is clicked, so a caller that
moves the selection somewhere the viewport has never drawn brings it into view
itself with `data::reveal_row`, naming the table's body as
`<table ident>.body`.

### Table or DataGrid

Both are column-oriented and both report rather than apply. The difference is
what they are handed:

- **`Table` takes rows, or a source.** Handed rows, the caller builds every
  cell before the table sees it and the whole set is laid out. Handed a source
  through `rows_from`, it survives a large collection but gains nothing else:
  no selection over an incompletely loaded set, no opened rows, no editing, and
  no keyboard. Reach for it for a settings summary, a short run list, a preview
  of a result set.
- **`DataGrid` takes a closure.** It asks for one row at a time and only for
  the rows the viewport holds, which is what lets it carry twelve thousand rows
  and also what makes column resizing, reordering, selection over an
  incompletely loaded set, opened rows, and cell editing worth its weight.
  Reach for it for the administrative surface: the thing with a header, a
  selection, a bulk bar, and more rows than fit.

If a surface would work as either, pick `Table`. It is smaller, and a grid's
machinery costs something even when nothing uses it.

`Table` now keeps the same vacancy contract as `DataGrid`: a first load with
no rows is busy, a successful empty query is empty, and a refresh failure
keeps any rows that are still true and states the refusal above them. Row
rules default on for `Table` and stay off for `DataGrid`.

### What DataGrid does not do

**It does not scroll horizontally.** `uniform_list` owns its own scroll offset
and lays every row out at the width it is given. A frozen left group under a
horizontal scroll needs either two vertically-synchronised uniform lists —
and nothing keeps two `UniformListScrollHandle`s in step without one writing
the other every frame, which is a redraw loop — or a per-row
counter-translation that fights the list's own content mask. So
`GridColumn::pinned` means "this column holds the left edge whatever order the
caller declares, and may not be dragged out of it or dropped across", not
"this column stays while the rest scrolls away". Columns share the grid's
width the way a table's do.

**It does not measure a column to its content.** A double click on a resize
handle reports a fit request through `on_fit` and stops. The grid can only
measure the rows it drew, and a width fitted to fourteen of twelve thousand
rows is a guess wearing a measurement's clothes; the host owns the data and
can answer properly.

**An opened row declares where it sits.** A virtualized body reserves room by
counting fixed-height slots, so it has to know where an opened row is before it
has drawn it — hence `Expanded { id, index }`. The index is layout arithmetic
and never reaches an id.

**Tab moves within the row.** Tab commits the open cell and names the next
editable column in the same row. When a row's editable columns are exhausted
the edit simply commits: the row below may never have been drawn, and the grid
will not build a row nobody asked to see in order to guess where a caret goes.

### Select all is two different claims

A header checkbox over a virtualized grid can only speak for the rows the host
has handed over. `DataGrid` keeps the two apart:

- the box publishes both numbers, as `"<selected> of <loaded> loaded, <total>
  total"`, and reports `SelectionChange::Loaded` — never anything wider;
- `SelectionChange::Everything` is only ever reported by a control that says
  that is what it does, which is `BulkBar`'s "Select all N".

So a typist who selects everything on screen is told they selected forty rows
and offered twelve thousand as a separate, named step, rather than being
quietly credited with rows nobody has loaded.

Cells are quiet by default. A table of two hundred rows and six columns would
bury every other assertion target under twelve hundred nodes that repeat what
the row already says, so a cell publishes a `Cell` node only where the caller
marks it with `Cell::published`, under the id `<row id>.<column key>`. A
sortable header publishes a `Button` carrying its current direction in `value`;
a header that does not sort publishes a `Cell` and installs no handler.

## Date and time

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `Calendar` | view | the day that was picked, the month now shown, and the day under the pointer | A month grid over a host-supplied `DateAdapter`. Every weekday heading, month name, day label, block reason, and the notion of today comes from the adapter. Arrows, page up and page down, and a step off the edge of the grid all move the month through `shift_month`, so a host that refuses a month refuses every route into it |
| `DateInput` | view | a day the adapter read, text it would not read, opened, closed, submit | A field with that calendar in a popover. Text the adapter refuses stays exactly where the typist left it, the field publishes `invalid`, and the adapter's message is shown word for word |
| `RangePicker` | view | a day picked as a start, and a day picked as an end | Two ends over one calendar. Unset, incomplete, complete, and end-before-start are four states rather than three and an error, and a blocked day inside a range is named in the host's own words |
| `TimeInput` | view | the time as it now stands | Hour, minute, optionally second, and a meridiem only when the host's `Clock` has one. Segments step within the clock's bounds and stop there rather than rolling over |

### This crate owns no calendar

There is no calendar system, no time-zone database, no locale, and no notion of
today anywhere in these four components. `Day` and `MonthKey` are opaque
integers the adapter mints; the components carry them, compare them, and hand
them back, and nothing here ever adds a day to a date. An adapter that answers
`None` to `today`, to `shift_month`, or to `days_in` is answering, not failing,
and each of those has a rendered consequence rather than a guess: a calendar
with no month to show says so instead of opening on one it chose. The trait a
host implements, method by method, is in `docs/datetime.md`.

The reference calendar the scenes and tests run on is behind the `fixtures`
cargo feature, off by default, so a host cannot reach a half-correct calendar
from the component path.

## Content

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `Markdown` | builder | a link that was taken, an image it did not fetch, a code block that was copied, and the lines truncation left out | Read-only rendered Markdown: headings, prose, emphasis, code, quotes, nested and task lists, links, images, rules, and tables. It parses to an owned tree first and draws that, so what is rendered is what a test can read |
| `AgentDocument`, `AgentDocumentBlock` | builder | Markdown actions together with the stable block that produced them | A stream-friendly sequence of stable, revisioned blocks. Text and nested Markdown participate in one cross-block document selection without losing block reading order. Markdown remains prose while code, tool calls, diffs, artifacts, schemas, charts, images, notices, choices, and host elements retain their own component contracts. Duplicate ids are reported and never silently collapsed. A block holds a way to build its element rather than a built one, which is what lets `virtualized` lay out a screenful instead of a whole conversation; blocks are matched across frames by id, so a streaming reply re-measures itself alone |
| `MessageList` | builder | a failed message that should be tried again, and whatever a Markdown body reported | A conversation over the virtualized `List`. Uniform slots report the plain-text lines they left out; `grows_to_fit` instead measures variable-height rows and leaves the message whole. Five delivery states, a streaming mark keyed to the message rather than to its text, caller-declared grouping, and following that happens only while the reader is already at the bottom |
| `Outline`, `Mark` | builder | the mark that was chosen | A compact index of a long surface, named rather than owned: it reads the mapped list's scroll position and glides it. The footprint is fixed — past what fits, marks become even ranges over the whole surface and each says how many places it stands for, so a hundred-turn conversation is still a glance rather than a solid line. The mark whose section is on screen is the selected one; only the pointer changes a mark's size |
| `ImageViewer` | builder | the fit that was asked for, the image that was stepped to, and an image the host has not supplied | One image at a time, with contain, cover, 1:1, and zoom; the wheel zooms at the pointer and a drag pans, clamped so the picture cannot leave the frame. Loading, unavailable, failed, and ready are four renderings, and dimensions are a caller input |
| `CodeView`, `CodeLine` | builder | a copy of the whole text | Read-only code with a gutter. No grammar and no new dependency: spans are pre-classified by the caller, exactly as a Markdown fenced block. A long line scrolls rather than wrapping, because a column carries meaning in code and a wrap would break the gutter's claim that one line is one row. Line numbers are the file's, not the slice's, and only a marked line publishes a node |
| `LogStream`, `LogEntry` | builder | the stable entry selected and the selected entry the host should copy | Caller-owned entries over virtualized `List`, with fixed clipped rows and a bounded viewport. Timestamp, level, source and message strings and search-hit ranges all come from the caller. Following/paused is identity-keyed visual state; loading, empty, unavailable, error and stale remain distinct, and stale keeps the last verified entries visible |
| `DiffView`, `DiffFile`, `DiffHunk`, `DiffLine` | builder | file, hunk and line activation, an unfolded file, and a hunk to be widened, all by stable caller identity | Read-only, caller-computed diff rows. The caller supplies aligned old/new sides for replacements; unified and split arrange them with the same renderer. The caller's spans always win; under them a replacement gets word-level marks and everything else is coloured by `language` if one was named. Rows are fixed-height so a huge diff opens instantly, `wrapping` measures them instead when the change lives at the end of a long line, and `DiffFile::folded` puts a file behind a header carrying its `+n −m`. It computes no diff or alignment, applies nothing and reads no filesystem |
| `TransportBar` | builder | play, pause, a preview while scrubbing and one seek on release, volume, mute, speed, and a track step | Playback controls for media this crate does not play. A duration the host does not know is a state, buffered ranges are drawn apart from the played position, and every readout is a string the host wrote |

### Developer data stays caller-owned

`LogStream` and `DiffView` both flatten caller-owned data into the crate's one
fixed-height virtualized `List`. Only viewport rows are laid out and published;
the list node still reports the complete row count. That keeps a large
already-materialized log or diff viewport-cheap, but it is not lazy loading:
building a frame still walks the caller's entries or hierarchy. Fixed rows also
mean long content is clipped instead of wrapped or horizontally scrolled.

`LogStream` keeps only follow/paused and scroll position. Its mounted messages
participate in GPUI's pointer document selection; a selection crossing
virtualized rows reports itself incomplete, while the existing stable-entry
copy intent remains the route to a whole caller-owned entry. `DiffView` applies
the same rule to mounted code sides and reports file, hunk and line actions but
has no apply operation. Neither component opens files, starts a process,
searches text, computes a diff or classifies syntax.

### A document is drawn, never obeyed

`Markdown` renders text nobody in the application wrote, so it does nothing
that text asks for. Raw HTML is drawn as the literal characters somebody typed,
marked `unrendered html`, because interpreting it would let a document reach
outside its own text and dropping it would let a document hide its own contents
from the reader. A link states its destination in hover help and in its node's
`value` before it is taken, and taking it reports `LinkClicked`; this crate
opens nothing. An image is never fetched — the crate has no network — so it is
drawn as a placeholder naming its alt text and its source and reported once as
`ImageRequested`, and a host that holds the bytes supplies an element through
`Markdown::image`. A fenced block publishes its info string exactly as written,
`plain text` when there is none, and is coloured only from spans the host
computed. `docs/content.md` is the whole posture.

`max_lines` cuts to a line count and says how many lines it left out, offering
them by name rather than behind a fade: a gradient over the last line says
something was cut without saying how much.

### A failure stays on screen

`MessageList` keeps `Sending`, `Sent`, `Delivered`, `Read`, and
`Failed { reason }` apart as five renderings, because collapsing the middle
three into one tick says less than the host knows and folding the last into any
of them says something untrue. A failed message keeps its place and its full
text, states the host's reason word for word, and gains one control that
reports the retry. Nothing is resent and nothing is removed.

Whether consecutive messages from one author are one turn is
`group_consecutive`, declared by the caller for the same reason
`Toolbar::overflow_after` is. Following a new message happens only while the
reader is already at the bottom; when it does not follow it publishes the
count — `3 new messages` for arrivals, `3 more messages` for what has always
been below — which is `ScrollArea`'s "content continues past the view" rule on
a surface that grows downward. Times are strings the host already wrote, as in
`Timeline`, and an unrecorded author is `unknown` rather than blank.

### Nothing is fetched, and nothing is played

`ImageViewer` extends `Markdown`'s posture to a whole frame. The crate has no
network and no asset resolution, so an image arrives from `ImageViewer::image`
or not at all; a host that answers `None` gets a frame naming the source rather
than a grey rectangle, and one `ImageRequested` per image rather than one per
frame. Natural dimensions are a caller input, and an image nobody measured
reads `Size unknown` and refuses the fit and zoom controls, because a scale is
a ratio against a size and reporting the box the picture was drawn in would
invent the fact the host declined to give. Zooming happens at the pointer,
against the frame measured during prepaint, and a pan is clamped so the picture
cannot be dragged off its own edge. Stepping past the last image is refused and
the position is published — `2 of 2` — rather than wrapping silently.

`TransportBar` plays nothing. Every control reports: `PlayRequested`,
`PauseRequested`, `SeekPreview` on every move of a scrub and `SeekRequested`
once on release, `VolumeRequested`, `MuteToggled`, `SpeedRequested`, and
`Stepped`. The head is drawn where the caller says it is, so a refused seek
keeps the position that still holds. A duration nobody knows is
`TransportDuration::Unknown`, which is `PageTotal::Unknown` for a timeline: the
scrubber then shows elapsed, says the total is unknown, and draws no fraction
at all. Buffered ranges are the host's and are drawn as their own band; a host
that supplies none gets no band and no node. Elapsed and remaining are strings
the host wrote, the rule `Timeline` and `MessageList` keep, and buffering while
playing is a state of its own — a stalled transport says it is waiting, and
still offers the control that would stop it, because nothing has stopped.
`docs/content.md` states the whole posture.

## Media

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `AudioPlayer` | builder | what a control asked the transport for, and whether it was applied, refused, or unsupported | A player over `MediaTransport`, with `PlatformMediaTransport` available for native playback. Idle, loading, no backend, failed, and ready are five renderings, and a player with no transport is a sixth that draws no scrubber at all. A waveform is drawn only from peaks the host measured |
| `VideoPlayer` | builder | the same, over the same transport | The audio player plus a frame surface. A frame the host supplied publishes `frame`; a still standing in for one publishes `poster` with the reason no picture is arriving over it; neither is `none`. Frames are not asked for from a transport that has said it cannot open the media, and a player with no transport carries no controls |
| `ModelViewer` | builder | the orbit a drag asked for, and the shading a control asked for | A bounded glTF 2.0 viewer. The document is read by `ModelScene::parse` inside `ModelBounds`, drawn flat-shaded or as a wireframe with an orbit camera, and published as the counts the reader counted. A refusal names the limit or the defect; a viewer holding nothing publishes no counts |
| `MediaTransport`, `FixtureTransport` | type | — | The seam between a player and an operating-system backend, and the deterministic stand-in that decodes nothing. Every surface publishes `MediaOrigin`, so a fixture is never mistaken for a player |
| `PlatformMediaTransport` | type | native media change signals through `subscribe`; component controls still report `MediaEvent` | The product-neutral native implementation of `MediaTransport`: AVFoundation on macOS and Media Foundation on Windows. `audio()` owns a decoder, output and clock; `video()` additionally owns a platform view, returned by `frame()`. `load` accepts a local file or URL, and snapshot state includes duration, position, buffered ranges, volume, mute, rate, buffering, end and native failures. Linux and Web construct the same API in explicit no-backend state |

### Native playback lands behind the component seam

GPUI draws images, and on macOS composites a `CVPixelBuffer` through its
surface element. GPUI itself still has no decoder, audio device or frame pump,
so the player components remain written against `MediaTransport` — `origin`,
`snapshot`, and `apply`. `PlatformMediaTransport` implements that seam without
changing the components: AVFoundation supplies macOS audio/video and an
`AVPlayerLayer`-backed `NSView`; Media Foundation supplies Windows audio/video
and renders to a child `HWND`. The GPUI `platform_view` element hosts the video
view and retains its native player until delayed detach is complete.

Create the transport on the UI thread, retain it, call `load`, and rebuild the
owning GPUI view when `subscribe` signals a native change. Native callbacks may
arrive on a media thread, so the subscriber must marshal invalidation to the
GPUI foreground executor rather than call UI APIs inside the callback. A video
player supplies the transport's frame explicitly, preserving the component's
existing truthful rule:

```rust
# use gpui_kit::prelude::*;
let playback = PlatformMediaTransport::video();
playback.load(MediaSource::file("walkthrough.mp4"))?;
let frame = playback.clone();
let player = VideoPlayer::new("walkthrough")
    .transport(playback.shared())
    .frame(move |_, _| frame.frame());
# Ok::<_, NativeMediaError>(player)
```

The native service covers ordinary unprotected files and streams supported by
the installed operating system codecs. It does not claim DRM, custom network
policy, playlists, track/subtitle selection, output-device routing, capture,
or Linux/Web playback.

A control asks the transport and reports what came back: `MediaEvent::Applied`,
`Refused` with the backend's own sentence, or `Unsupported`. The next frame
draws the transport's snapshot, which is what makes a refused seek leave the
head where it was. `FixtureTransport` takes commands and advances no clock, so
a scene renders the same bytes on every run; it reports `MediaOrigin::Fixture`
and every surface publishes and draws that.

### A model is read inside a fence

`ModelScene::parse` accepts glTF 2.0 in both containers, buffers that are
inside the file — the GLB binary chunk and `data:` base64 URIs — triangle
primitives with a `VEC3` `FLOAT` `POSITION` accessor, unsigned byte, short or
int indices, and node hierarchy by matrix or by translation, rotation and
scale. Any other URI is refused, because resolving one is I/O and this crate
performs none. Materials, textures, animation, skins, morph targets, sparse
accessors and non-triangle primitives are refused rather than approximated, and
`ModelBounds` caps bytes, nodes, depth, primitives, vertices and triangles with
every cap checked while reading rather than after allocating. A refusal names
what the document asked for and what was allowed, so a caller raises a bound on
purpose instead of guessing.

## Interaction

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `Dropzone` | builder | the item that was dropped, and the paths a platform file drop carried | Distinguishes idle, accepting, and refusing, and never renders refusing as idle. It refuses by payload kind and says why; `state` pins one of the three for review. File paths reach the handler and never the semantic tree |

`List`, `Tree`, and `Tabs` also take part in drag and drop, through
`reorderable`, `accepts`, and `on_reorder` or `on_move`. The contract they all
share — what a drop reports, what a drag publishes, what the host has to do —
is in `docs/interaction.md`.

## Overlay

| Component | Kind | Notes |
|---|---|---|
| `Overlay` | builder | Placement, which trigger edge it hangs from, token-driven paint priority, scrim, dismissal |
| `Frost` | builder | Glass: the pixels behind are blurred and the surface colour is laid over them at `effect.glassAlpha`, so a popover, dialog or rail on a translucent window keeps its contrast. The whole subtree paints in one scene layer, which is what keeps the blur underneath the content instead of intermittently over it. Where the renderer has no backdrop blur, and where a theme sets `effect.glassAlpha` to 1, the tinted fill is drawn on its own and the surface is merely unblurred |
| `Dialog` | view | Composed modal: reports opened, confirmed, cancelled, dismissed, closed. A dialog that is not dismissable installs no escape or scrim handler |
| `Drawer` | view | The same surface arriving from an edge: same scrim, same focus trap, same escape and scrim dismissal. It slides out through `Presence`, and because an element cannot animate after it is dropped it stays in the tree until the exit finishes and only then reports `Closed` |
| `Popover` | view | The anchored surface `Menu` and `Select` are special cases of. Owns only whether it is open: the body is a per-frame callback, escape and a click outside dismiss it unless it is not dismissable, and closing gives the keyboard back to the trigger |
| `Positioner`, `Room`, `Side` | helper | Resolves an anchored surface's preferred or opposite side, width, and available height before that surface is built. An unmeasured trigger is reported as provisional rather than mistaken for a trigger at the origin |
| `Menu` | view | Commands, checkable rows, separators, section labels, and nested submenus, opened from a trigger. Up and down step over rules, labels, and refused rows; a letter jumps to the next row starting with it; right and left enter and leave a submenu; escape folds one submenu away before it closes the menu. Taking a row reports it once and closes the whole chain, and a refused row installs no handler |
| `ContextMenu` | view | The same list opened at the pointer over a wrapped region. Reports the target it was opened on and selects nothing, because opening a menu is not choosing anything. A surface that would leave the viewport flips to the other side of the pointer |
| `MenuItem` | builder | One row: `command`, `check`, `separator`, `section`, or `submenu`, with an optional shortcut hint and icon. A checkable row draws the state the host holds and reports the intent to change it |
| `CommandPalette`, `Command` | view, builder | A query field over a command list, filtered by `popover::match_rank` — prefix, then word start, then substring, then subsequence — with sections kept contiguous behind their best match. Nothing matching shows an `EmptyState` naming the query that answered nothing, and a command the host marked unavailable stays listed with its reason rather than being hidden |
| `Tooltip`, `Tooltipped` | builder, trait | Hover-delayed help on GPUI's hover machinery. Never actionable, and never the only copy of what is needed to act. `Tooltipped` attaches one to any element |
| `HoverCard` | view | opened, closed | A preview that opens on hover and holds content worth reaching, so it tracks the pointer over the trigger and over the card separately and keeps a grace period between them. Anchoring is `Popover`'s. The trigger is a tab stop, and escape closes and hands the keyboard back |
| `Menubar`, `MenubarMenu` | view, builder | opened, invoked with the menu it came from, closed | A row of `Menu` views. It adds only what the row implies: at most one open, hover switching once one is open and not before, and the reading-order arrows stepping between titles. A refused title has no menu behind it at all |
| `NotificationCenter`, `Notification` | view, builder | Where a notification goes after the toast that showed it has gone. One record and two surfaces: `show` files it here and pushes the toast built from it, sharing the id, the wording, and the severity. Dismissing one and clearing them all are separate reports, and a centre that has dropped records to stay bounded stops claiming an exact unread count |
| `ToastLayer`, `Toast` | view, builder | Transient notifications. The host mounts the layer in the window it wants them drawn in; `overlay::toast::push` reaches it from any call site and reports whether a layer was mounted to deliver to. One action at most, an optional dismiss control, entry and exit through `Presence` |
| `FocusTrap` | helper | Keeps the keyboard inside an open overlay and restores focus |
| `Kbd` | builder | Platform-specific keystroke caps |
| `popover` | helpers | Anchoring, menu rows, cursor movement, type-ahead, filtering, and key classification |

### What a copy button can honestly claim

`gpui::App::write_to_clipboard` returns `()`. There is no `Result`, no error,
and no callback, so a tick shown because that call returned would be a tick
shown because a function with no failure mode did not fail. The one piece of
evidence GPUI offers is `read_from_clipboard`, so `CopyButton` writes, reads
back, and compares; a read that comes back empty or holding something else is
reported as a failure with `invalid` set on a published `Status` node.

The gap that leaves is stated rather than papered over: a platform where the
write lands in a clipboard this process can read but no other application can
see would be indistinguishable from success. Nothing in GPUI's surface can tell
those apart. A host that knows better supplies its own `copier`, which returns
a `Result`, and whose failure text is shown verbatim.

The confirmation times out; the refusal does not, for the same reason a
`Toast` reporting a failure does not.

### A hover card the pointer can reach

A tooltip may vanish the instant the pointer leaves, because nobody was ever
going to point at it. A hover card holds a link, a button, or text to read, and
between the trigger and the card there is a gap the surface does not cover. So
the card tracks two facts rather than one — pointer over the trigger, pointer
over the card — and leaving *both* starts a countdown that entering *either*
cancels. Only a countdown that runs out closes it, which is what makes the
diagonal trip across the gap winnable.

Opening has its own countdown for the opposite reason: a pointer crossing a row
of triggers on its way somewhere else opens none of them. Both durations are
caller-settable and neither is a token, because they are reaction times rather
than paint.

### Failures do not time out

A notification that reports a failure — `Tone::Danger` or `Tone::Warning` —
stays until it is dismissed. A failure the typist never saw is a failure that
was never reported, so no timer is allowed to hide one. Every other tone times
out after `motion.durationMs.toast`, and a pointer resting on a toast pauses
its timer so nothing disappears mid-sentence.

The stack has a cap. When it overflows, the oldest toast that both times out
and can be dismissed leaves first; a persistent one is never evicted to make
room, and when nothing may be evicted the cap yields rather than swallow a
report.

### A field says what is wrong without taking back what it said

`FormField` shows the description and the error together. They answer different
questions — what the field is for, and what went wrong this time — and swapping
one for the other loses an answer the typist still needs. The exception is a
pair that says the same thing twice: when the error repeats the description
word for word, only the error is drawn.

`NumberInput` and `TagInput` extend the same rule to what the host holds. A
number outside the range stays on screen exactly as it is, published `invalid`;
a tag the field will not take leaves the typed text in place and says why.
Neither silently corrects the caller, because a value nobody chose is a value
nobody can trust.

### A settings row withholds the control, not just the colour

A setting decided by policy, and a setting that belongs to a section which
does not apply on this machine, are both shown with their value and with a
line saying so. Neither renders the control the caller passed: dimming a live
switch leaves something on screen that can be operated to no effect. The
section states the reason once above its rows rather than once per row.

### The timeline does not know what time it is

`Timeline` takes times and day headings as finished strings. Turning an instant
into words is calendar, time-zone and locale work, which this crate does not
do: the date components push the same work out to a `DateAdapter` rather than
guessing at it, and a timeline entry's wording is pushed out one step further,
to whoever already holds the clock. An entry with no known time is neither
floated to the top nor dropped to the bottom: it says its time is unknown and
publishes `time unknown` as its value.

## Agent run

A conversation is not the unit an agent application shows; a run made of steps
is. These three are that vocabulary, and every one of them exists because a
plainer component would have to collapse two facts into one.

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `ToolCallCard` | builder | a failed call that should be tried again | One invocation of one tool: what it was called with, which of its five states holds, its result or its error, and how long it took. Arguments and results publish their shape and never their text |
| `StepList` | builder | — | An ordered run of steps, each with its own state, each able to hold a `ToolCallCard`. A run whose length nobody knows gets an indeterminate summary rather than an invented fraction |
| `ThinkingBlock` | builder | the state the disclosure should take | Model reasoning, collapsed by default. Withheld, absent, and collapsed are three states and three presentations |
| `NodeGraph`, `GraphInteraction` | builder, policy | viewport and selection; Arrange adds node moves; Edit adds node deletion and connection/disconnection proposals | A controlled run canvas over caller-positioned `GraphNode`s and caller-owned `GraphEdge`s. `Inspect` never installs topology-edit actions, while the default `Edit` preserves the complete editor. A node thumbnail is a caller-rendered element slot; the graph fetches and decodes nothing |

`GraphNode` does not compress execution into a success flag. `Pending`, `Idle`,
`Queued`, `Starting`, `Running`, `Waiting`, `Blocked`, `Succeeded`, `Partial`,
`Failed`, `Refused`, `Cancelling`, `Cancelled`, `TimedOut`, and `Unavailable`
remain distinct semantic values. Related states may share a theme tone, but
wording, a static glyph, busy state, and invalid state preserve the distinction
when colour or motion is unavailable. Every routed edge remains a named
semantic group in Inspect and Arrange modes; Edit changes that same stable
target into the disconnect action rather than making topology disappear from
non-editable semantic trees.

### A refusal is not an absence and not an error

`ToolCallState` is five states, not a flag beside a result: `PendingApproval`,
`Running`, `Succeeded`, `Failed`, and `Refused`. The last two are the pair that
gets collapsed everywhere else, and they say different things. A failure blames
the tool for something it did and carries the host's error; a refusal is a
decision somebody made before anything ran, and carries the host's reason. A
third thing is neither: `ToolOutput::Silent` is a call that ran, succeeded, and
returned nothing. Each publishes its own name in `value` and renders its own
consequence, and a refused card publishes no elapsed time at all, because
nothing ran to take any.

An elapsed time is a string the caller already wrote — the rule `Timeline` and
`TransportBar` keep — and a duration nobody stated is `Elapsed::Unknown`, which
says so rather than reading as zero.

### A body publishes its shape, never its text

Arguments and results are somebody else's data and may be a credential, so a
`ToolBody` node carries only the measurement: `2 of 4 lines shown` when the
caller set `max_lines`, and `4 lines` when it did not. The same sentence is
drawn beside the block, so the cut is stated where it happens rather than
implied by a fade, and it is stated whether or not anything was cut, so
"there is more" is read off the same line every time.

### A run nobody counted has no progress

`RunLength::Known` and `RunLength::Unknown` are `PageTotal`'s distinction for a
run. With a known length the summary is `ProgressBar::count`, which publishes a
position; with an unknown one the bar is indeterminate, publishes no fraction,
and states only what has finished — `1 step done`. A bar crawling toward a
total nobody established would be a number this library invented.

A step's state is `Pending`, `Running`, `Done`, `Failed`, or `Skipped`, and the
last two carry the host's own words: a step that never ran and a step that ran
and failed are different sentences, published under different names.

### Three states, and no `Option` to lose one in

`Reasoning::Present`, `Reasoning::Withheld`, and `Reasoning::Absent`.
An `Option<String>` cannot hold this: its `None` would have to stand for both
"the provider withheld it" and "there was none", and a block that says nothing
was produced when in fact it was withheld states something nobody established.
So the type has three variants, no conversion from `Option`, and a required
reason on `Withheld` — whoever withheld it has to say so, and the words are
shown verbatim. Only `Present` can be opened: the other two install no toggle
handler at all and publish `Text` rather than `Button`, and an open block
renders its body while a closed one renders none, the rule `Accordion` keeps.
Reasoning that exists and is empty is still `Present`.

## Permission and cost

The two places in an agent application where a careless interface misleads
somebody about something that matters: what it is allowed to do, and what it is
spending.

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `ApprovalPrompt` | view | approved, with how far the approval reaches, and declined | One request for permission to do one specific thing. The keyboard lands on decline, return acts only on the control that holds it, escape declines, and a resolved prompt installs no handler at all |
| `PermissionMatrix` | builder | the state a cell would take next | Subjects against actions. Allowed, denied, ask every time, and not applicable are four states, and every cell that has a state says whether it was set here or inherited, in the host's words |
| `CostMeter` | builder | — | What a run has cost, line by line. Measured, estimated, and unavailable are three different readings, and a stale line keeps its last verified value and says when it was from |
| `ContextGauge` | builder | — | How much of a context window has been used. A proportion only when both the reading and the limit are known |

### The default is refusal

`ApprovalPrompt` is arranged so that nothing makes approving easier than
declining by accident. The keyboard lands on decline when the prompt appears,
the way `Dialog` opens a destructive confirmation on cancel; return acts on
whichever control holds the keyboard, so a return key pressed at rest declines
and approving with the keyboard costs a deliberate tab first; and escape
declines, which is both what escape does everywhere else here and the safe
direction. The request is stated specifically — the constructor takes what is
about to happen, and there is no way to describe it as a category — with the
exact path, command, or host beside it in a `DescriptionList`.

`Declined`, `Expired`, and `Superseded` are three states, not one. Nobody
answering in time is not a refusal, and a request a later one replaced is
neither; each publishes its own name and its own sentence, and a host that
supersedes a prompt says so through the component rather than by removing it,
so the reader finds out why the controls went away.

### An unscoped "always" does not exist

`AlwaysScope` has no variant meaning "always, everywhere". It is the session,
or one named tool, one named path, or one named host, and the wording on the
control is derived from the variant — so a control offering a standing
permission without saying what the permission covers is not something a caller
can construct.

### Not applicable is not denied

A `PermissionMatrix` cell is `Allowed`, `Denied`, `Ask`, or `NotApplicable`.
"This tool has no network to reach" and "this tool is refused the network" are
different sentences, and collapsing the first into the second invents a refusal
nobody made. `NotApplicable` has no next state, so it installs no handler even
in an editable matrix; a matrix given no `on_change` is read-only, publishes
`Cell` rather than `Button`, and installs nothing anywhere.

Provenance is carried, never derived. Deciding which rule won is policy
evaluation over a rule set the host owns — the same kind of fact the date
components take from a `DateAdapter` — so a cell renders the
`PermissionSource` it was handed, either naming the broader rule it came from
in the host's own words or saying it was set here, and computes nothing.

### An estimate says so wherever it appears

`Quantity` has no constructor that takes a bare number: `Quantity::measured`
and `Quantity::estimated` name the basis in the same call, so a number reaches
a screen with the fact that it was estimated attached or it does not reach a
screen. The label is in the drawn text, in the mark beside it, and in the
node's published value, because a reader who saw the number labelled on one
surface and bare on another would trust the wrong one.

`Reading::Unavailable` is a state rather than a quantity: nothing about it is
drawn as a number and no proportion is computed from it. `Limit::Unknown` is
the same refusal one level up — a proportion of an unknown total is invented,
so `ContextGauge` draws no fill and publishes no range, exactly as
`ProgressBar` refuses to claim a position for work whose extent is unknown. It
does not fall back to the indeterminate sweep either: the sweep means "in
flight", and a reading of what has been used so far is not in flight.

A refresh that failed keeps its value: `CostLine::stale` takes a
`LastVerified` rather than a flag, so a value cannot be marked stale without
saying when it was from.

Numbers are the caller's. Currency, token counts, grouping, and where a unit
sits are locale work this crate does not do, so a `Quantity` carries the
caller's already-formatted wording and, separately, the bare number — which is
used for one thing only, the proportion against a known limit. Nothing here
turns a number into text.

## Structured data

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `JsonView` | builder | a path and the disclosure state it should take, and the row that was picked | A structured value over a caller-supplied `JsonValue`. Virtualized, so only the rows the viewport holds are laid out or published. `null`, an empty container, and a key the document does not hold are three presentations, and a withheld subtree reads as withheld |
| `SchemaForm` | view | a field that changed, a file field whose picker was requested, and a submit | A form built from a caller-supplied `Schema` over the existing controls. Date shapes use the host's `DateAdapter`; `Files` uses a host `SchemaFilePolicy` without owning an OS picker; repeating `List` owns stable add/remove UI and nested values. A field it cannot draw states so where the control would have been and is still reported by `values` |
| `ServerList` | builder | a server that was picked, a failed one that should be tried again, and a server whose offerings should be shown | What is connected and what each connection offers. Five states, none of them a shade of another, and an empty answer that is not an unasked question |
| `OfferingCatalog` | builder | activation carrying `{server_id, offering_id}` | Searchable Tool, Skill, and Resource results aggregated across caller-owned servers. Search text and kind filters are caller supplied; duplicate names remain attributed, stale data remains visible, and the component performs no install, invocation, trust, permission, or network policy |

### This crate parses nothing

`JsonValue` and `Schema` are plain shapes a host converts into, for the same
reason `SplitLayout` converts to records instead of deriving `Serialize`:
product-neutral infrastructure must not decide which parsing crate an
application depends on. A number is carried as the text the document wrote,
because `f64` cannot hold every integer JSON can write and cannot tell `1.10`
from `1.1`, and this crate formats no numbers. An object is a list of pairs,
because JSON documents have an order and may repeat a key, and a map would
silently reorder the first and drop the second.

`JsonView` does not build on `Tree`. A tree node is one label; a JSON row is a
key and a typed value with different treatments, and there is no `Slot` here to
put a second column into one. It virtualizes over the same `uniform_list`
primitive `List` and `DataGrid` use, so it inherits the rule rather than the
component: only rendered rows publish nodes, and the container carries how many
rows are currently disclosed.

### Withheld, null, empty, and absent

Four facts, four renderings. A key the document does not hold produces no row.
`null` is a row reading `null`. An empty object is a row reading `{}` that
offers no disclosure, so it can never be mistaken for a branch that is merely
shut. A subtree the caller withheld is a row marked `withheld` beside a
description of its shape.

The secret never reaches the component: a caller replaces the subtree with
`JsonValue::Redacted`, which carries a shape and no content, so no rendering
path and no export can leak it. The published `value` is `withheld` and nothing
else — not even the shape, which is drawn and never recorded.

### A form that cannot draw a field says so

This is the rule `SchemaForm` exists to keep. A host converting a schema it does
not fully understand puts `SchemaKind::Unrenderable` in place of the field with
its own reason; the form refuses a few shapes itself, such as a choice among no
choices. Either way the field keeps its place, its label, and its required mark,
states the reason where the control would have been, and is still reported by
`SchemaForm::values` as `FieldValue::Unrenderable`. A required one publishes
`unrenderable, required` and makes `validate` answer no however much else is
filled in.

A form that quietly dropped an argument it did not understand would send an
invalid call and let the reader be blamed for it.

Errors come from two places and stay apart. `validate` marks required fields
nobody filled in, which is all the form can judge on its own; `set_error` shows
what the host returned, in the host's words, and outranks the form's own on the
same field. Both are drawn by `FormField`, next to the control they are about.

Files keep the same boundary. The form owns its drop target, selected rows,
maximum count, removal, and `FilesRequested` event. The installed
`SchemaFilePolicy` decides whether a complete candidate selection is admissible
and supplies display names; the host opens the platform picker and returns paths
through `set_files`. A repeating `List` needs no host policy: it creates nested
forms with monotonic visual identity, reports indexed values and errors, and
applies the same date/file adapter requirements recursively.

### Five connection states, and an answer that was empty

`ServerState` is `Connected`, `Connecting`, `Disconnected`, `Failed`, and
`Disabled`. The last two are the pair that gets collapsed elsewhere and they
say different things: something broke, against nobody wanted it. A failure keeps
the host's reason on screen and offers exactly one control, which reports a
retry and retries nothing. A connection the reader turned off is refused rather
than dimmed — nothing on its row installs a handler.

`Catalog::Offers` holding an empty list is an answer; `Catalog::Unasked` is the
absence of a question. Rendering the second as the first tells somebody their
server is useless when the truth is that the application has not asked yet.
`Asking` and `Unavailable` are the two remaining states, and each is drawn as
itself.

Nothing here names a protocol or a vendor. A connected thing is a *server*,
what it offers are *tools*, *skills* and *resources*, and an offering's id
carries the server that offers it, because two servers may offer the same name.

## What every component agrees on

These hold across the families above, so a habit learned on one component
transfers to the next.

**Identity.** A constructor takes `impl Into<Ident>` and derives every child id
from it with `Ident::child`. A part with no business identity — a loader cell,
a skeleton row — gets `indexed_element_id` and publishes nothing. Purely
decorative display components (`Badge`, `Card`, `ListRow`, `Divider`, `Avatar`,
`StatusLine`, `Callout`, `Kbd`) take no `Ident` and publish a node only when
the caller gives them one with `.id(..)`, so an ornamental badge does not bury
the assertion target next to it.

**Refusal.** Anything that can be refused implements `Disableable`. A refused
control installs **no** handler — not a handler that returns early — so it
cannot fire even if a host mis-routes an event, and it publishes
`disabled: true`. Dimming alone is not a refusal.

**Size.** Anything with a size implements `Sizable` and takes every metric —
height, horizontal padding, gap, font size, glyph size — from one step of
`control.*` in the token document. A `sm` button and a `sm` select are the same
height because they read the same row of the same table; nothing hard-codes a
height beside it.

**Selection.** Anything that can present itself as the current choice
implements `Selectable`.

**Reading direction.** `LayoutDirection` is a global a host sets with
`set_layout_direction`, and components read it during render through
`ActiveDirection` exactly as they read the theme. It defaults to left to right,
so a host that never sets one renders what it always did. Components spell
edges logically — `row_reading`, `ps`/`pe`, `ms`/`me`, `border_s`/`border_e`,
`text_start`/`text_end` — wherever the edge means "where reading begins", and
keep saying left and right where the edge is genuinely about the screen: the
gutter of a vertical scroll region, a dock region, a split pane's axis. A
horizontal arrow key that means previous or next swaps with the direction; one
that means "toward an edge", and every vertical arrow, does not. Whether a
glyph turns around is a property of the drawing, carried by
`Icon::mirroring` in the asset catalog, so a chevron flips and a checkmark
does not.

**Focus.** Every interactive element is reachable with tab and wears the same
ring, from `effect.focusRingWidth` and `effect.focusRingAlpha` in the focus
colour, applied through `FocusRing::focus_ring`. The ring is a shadow rather
than a border, so focus never reflows what is around it, and it is a different
treatment from selection on purpose: focus says where the next keystroke goes,
selection says which answer is current. Selection is a neutral wash and an
accent rail at the reading edge, so the two never wear one appearance.

## What a node's `value` means

`value` carries the one fact a node reports about itself that a reader would
otherwise have to measure off the pixels:

- a control that holds something publishes what it holds — the committed text
  of an input, the label of the chosen option, a divider's ratio, a
  scrollbar's position and reach, a progress position;
- a container publishes how much it holds, as a count — a list's total, a
  toolbar's item count, the number of pages, the number of rows an ellipsis
  stands for, the tags in a field against its limit;
- a state carrier publishes the **name** of the state rather than its colour —
  a toast's tone, a header's sort direction, which of empty, unstarted,
  unavailable, or failed an empty state names;
- a refused row publishes the host's reason for refusing it.

`value` never repeats a label, a role, or a position in a list; the node's
`text` carries the name, and `bounds` carry the geometry.

## What a component owns

A component holds hover, focus, open, and animation state. It never holds the
answer: a value, a selection, and a list all belong to the caller. A host that
refuses a change simply does not apply it, and the control keeps showing what
is still true. This is why `Select` reports the option that was picked instead
of moving its own checkmark.

## Validation

Every component appears in `gpui_kit::scenes`, which `xtask headless check`
renders offscreen in every bundled theme and
`crates/gpui-kit/tests/scenes.rs` audits headlessly. Behaviour is asserted
through simulated key and mouse input against the published semantic tree, in
`crates/gpui-kit/tests/`.

The catalog is in two tiers, and the difference is what "appears in" means.
Almost every rendering is an **exhibit**: it is about one component, it lives
in `crates/gpui-kit/src/scenes/<family>.rs` next to the other renderings of
that family, and it is where a reader is sent to review that component's
states. `xtask api check` fails when a public component has no exhibit, so a
component cannot be added and left unseen, and it fails when an exhibit claims
a component its own source never builds, so a component cannot be covered by a
picture of something else that happens to mount it.

The remaining three are **compositions** — `motion-flip`, `motion-state`, and
`reading-direction` — built the way a product would build them, because
components interact in ways none of them shows alone. A composition is nobody's
coverage: `Shows::Composition` says so, and the `scenes` list on a component in
`docs/api-index.json` therefore names only the exhibits, which is the honest
answer to "where do I go to look at this".
