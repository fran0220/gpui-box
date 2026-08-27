# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Entries say what the library now does and what it refuses to do, because a
refusal is the part a caller has to plan around.

See `docs/releasing.md` for the protected publication and verification runbook.

## [Unreleased]

### Changed

**Focus-visible follows where focus came from, not what was typed last.**
`focus_visible` asked whether the window's last input event was a key, which
answers a different question than the one it is styling: a dialog that moves
focus to its own primary button after the person clicked *Open* had that
button's ring suppressed, because a mouse was the last thing touched. GPUI now
records whether a pointer press is what placed the current focus —
`Window::focus_from_pointer` alongside `Window::focus`, read back through
`Window::focus_is_visible` — and `focus_visible` matches unless that press put
it there. Focus a tab stop, an action, a focus trap, or the application moved
is visible; focus somebody clicked onto is not.

**The focus ring answers the keyboard, and only the keyboard.** `FocusRing`
draws through `focus_visible` rather than `focus`. It always said "the
keyboard is here" and now it only says it then: a mouse-focused strip, tab,
sidebar row or slider no longer wears an outline the borderless catalogue does
not otherwise draw, while a dialog's initial focus keeps its ring. Editable
controls are unaffected and still show a click: a field's ring comes from its
own focused state through `field`, never through this trait.

**Windows overlays keep retained text visible.** GPUI still rasterizes text
directly into a transparent native scene as grayscale, while cached views now
include their base-versus-overlay plane in the reuse key. Direct3D also treats
RGB subpixel atlas tiles that nevertheless reach the transparent split as
coverage masks and writes real alpha for DirectComposition instead of using
ClearType's deliberately alpha-less blend state. Moving or replaying a dialog,
menu, prompt, drag preview, or tooltip across the native-view boundary can no
longer retain its shapes while dropping its glyphs. Direct3D path and backdrop
passes also restore the scene plane that invoked them instead of unconditionally
returning later batches to the primary swap chain.

**Text-area completions have one product-neutral seam.** `TextArea` exposes
current-frame range and caret geometry, explicit selection, and a one-step
undoable range replacement. Completion claim, accept, dismiss, selection, and
geometry events let a caller coordinate a popup without replacing the editor's
input engine. `MentionInput` builds on that seam with `@` trigger detection,
locale-aware candidate matching, stable candidate identity, caret-anchored
presentation, keyboard and pointer acceptance, and distinct loading,
refreshing, empty, no-match, unavailable, error, and stale-error states. Hosts
still own candidate retrieval, exact replacement text, and the semantic link
between inserted plain text and an accepted identity. GPUI's cross-tree active
descendant relationship keeps keyboard focus on that editor while projecting
the active option from its deferred popup to AccessKit.

**Waiting is grey, and the theme owns it.** The three-stop loader gradient is
gone. `color.loader` now carries four neutral roles — `mark` (the moving part:
a bar's fill, a spinner's arc, a breathing dot), `track` (the groove it
travels), `placeholder` (the shape of content that is not there yet), and
`sheen` (the highlight that crosses it) — and every loading, progress and
scrubbing surface in the library paints from them. Colour on a loading surface
is now a caller's `tint` with a meaning attached, never the library's
decoration.

Two token rules came with it, because the failures they catch were both
shipping. `color.loader.mark` is held to the 3:1 an active identity gets, on
every surface. `color.loader.placeholder` is held inside a *band*: visible, and
quieter than content. A skeleton had been filled with
`color.interactive.track` — a value tuned to clear the 3:1 a control boundary
needs — which made the absence of content the brightest thing on a dark page in
nine scenes. Too loud is a defect exactly as real as invisible, and the
validator now says so.

**A state glow stays out of its neighbours' pixels.** `effect.glowSpread` is
the bloom budget: how far a state glow is pulled in before it is blurred.
Without it a failed panel put its full alpha on its own edge and reached its
whole blur past it, which is how a red card tinted the card beside it and a
running node was cut flat at the edge of its canvas.

**The accent stopped being decoration.** Colour in the library now marks
selection, focus, status, or data, and nothing else. A chosen segment or tab
rises to the brightest neutral the interactive scale has and spends the accent
on one rail along its edge, so the current answer is the lightest thing in a
run rather than the darkest; a context gauge, a vote, a tag and a rating all
draw "selected" the same way; and every border width and corner radius that
means anything now comes from a token. What is left is drawing: the hue ramp
in the colour picker, and the marks inside a persona's face.

**A refused control looks refused.** Disabling used to remove the handlers and
leave the drawing alone, which left a rating, a permission cell, a dropzone and
a primary button looking exactly like the ones that still worked. A refused
action now gives up its variant's fill entirely, and a refusal on a surface
that has one — a dropzone, a consent prompt — is a sentence in the refusal
tone rather than an unlabelled red box.

**A region says which edge hides something.** `ScrollArea` fades its own
content at the edges it is scrolled past instead of laying a backdrop-tinted
wash over it, which is how a scrolled panel in the dark theme came to show a
hard cut through its last row of glyphs. The fade a component draws inside
itself publishes no node, so a caller who wraps that component in a
`ScrollFade` of its own still owns the only region under that identity.

**Rows are ruled, states are separated, and content is not clipped.** Lists,
diagnostics, offering rows, kanban cards and grid cells took separators,
shared baselines, a hover fill quieter than selection, and bottom padding
where a panel had been cutting letterforms. A consent prompt draws its two
answers at equal weight, because a primary grant beside a secondary decline is
pressure a consent prompt must not apply.

**A bundled fallback face is reachable, and every keycap modifier draws.**
`⌘`, `⌃` and `⌥` were blank boxes on every keystroke in the library. The asset
crate bundles a face covering the seven keyboard symbols no Geist face draws,
and `Kbd` names it in its fallback list, but both text systems threw that face
away before the shaper could reach it: a font is dropped if it has no `m`,
because `em_width` measures with `m` and a caller asks a family for its em
before it draws. A face named in a *fallback list* is never measured with — it
is only reached for the characters the primary does not cover — so requiring
`m` of it rejected exactly the symbol-only faces a fallback list exists to
name, and the cosmic text system additionally removed it from the database,
which took it away from everything else too. Family lookup now knows which of
the two roles it is filling. On macOS, an embedded font is also registered with
CoreText: a cascade list entry is resolved from a family name, and a family
CoreText was never told about resolves to nothing, so the bundled face was
unreachable there for a second and independent reason.

Which glyph appeared used to depend on what the host machine had installed,
which is the one thing a bundled face is for. `gpui-box-kit-assets` now checks
the bundle against its own bytes: that the face publishes the family the
fallback list names, that it covers all seven symbols, and that it has no `m`
— the property that made it a fallback rather than a family.

**A control the form is complaining about draws itself refused.** `SchemaForm`
showed a red sentence under a field wearing its ordinary border, which says the
field is fine and something else is wrong. Text, choice, open-choice and list
controls are now told when the form is showing an error about them, and
`Select`, `Combobox` and `TagInput` gained the `set_invalid` that `TextInput`
already had, for an owner that learns the answer is wrong after building the
control. A number still works this out for itself.

**Validation has a state before failure.** `ValidationState` gives fields and
forms one caller-owned `Pending`, `Validating`, `Invalid { reason }`, and
`Valid` vocabulary. `FormField` publishes validating as busy and shows it in a
quiet caption rather than red invalid chrome. `SchemaForm` tracks field and
whole-form validation separately, routes repeated-field paths to their stable
child forms, blocks submission while an explicitly managed check is pending or
in flight, rejects stale paths instead of creating invisible blockers, and
shows a form refusal once instead of painting every field red.
Number, date, time, range, switch, and file-drop controls now accept host
invalidity in addition to their own parser/range/drag state, so every schema
field can agree with the reason beside it.

**A hidden schema field has one explicit submission answer.** `FieldVisibility`
records the result of a caller-owned condition without teaching `SchemaForm`
product rules. Hidden fields and subtrees do not render or field-validate;
`HiddenSubmission::Omit` leaves them out of `submission_values`, while
`Include` preserves the complete held subtree. `values` remains the lossless
inventory. Object and repeated-list parents govern their descendants, and a
repeated child's visibility follows stable item identity across reorders.

**Windows UIA now has a real editable and form proof.** The native smoke edits
an input through ValuePattern, requests focus, reads distinct TextPattern
character rectangles and the logical end caret, verifies relationship-derived
field names and complete descriptions, and invokes the uniquely focused menu
item before observing its menu close. A small native COM query bridge reads the
modern FullDescription property that PowerShell's legacy managed identifier
table cannot represent. Narrator speech, selection mutation, remaining overlay
lifetime, and announcement events stay explicitly unverified.

**A settings page no longer invents its own filter.** `SettingsList` searches
the words `SettingsRow` already displays plus caller-authored aliases and
opaque-control vocabulary through the installed locale matcher. It filters
whole sections without reordering familiar settings, publishes the exact
result count through `NumberAdapter`, and distinguishes an unpopulated page
from a non-empty page whose query matched nothing.

**An accordion body turns with the reading order.** The header honoured RTL and
the body did not, so a disclosed section read as belonging to the one on the
other side. Turning the text alone was not enough: a shrink-to-fit child is
placed by the flex axis, which `text-align` does not reach.

**Three components that drew their shape but not their content.** A `Trace`
waterfall was unlabelled bars on an unmarked field: rows now carry the span
they belong to, the field carries gridlines and time ticks, and each span
states its own duration. A `Wizard` drew its steps as loose marks with nothing
between them; they are joined now, in both orientations, and the current step's
body sits in a container instead of floating beside the rail. An `UndoHistory`
broke its own timeline at the selected revision — the rail runs through it now,
and the selected card is inset clear of the rail rather than sitting on it.

**Fields and pickers say what they are.** A segmented code input draws each
place as its own slot, a time field gives every segment the same width so the
colons line up, a calendar draws a range as one shape with corners only where
the run ends, and today is a neutral ring rather than a second chosen day. A
count in progress in a filter bar wears the same loading mark as everything
else that is waiting.

### Added

**Labels and help now reach the native accessibility relationship graph.**
Role-bearing GPUI elements can declare labelled-by and described-by in either
direction. The per-window resolver runs after deferred prepaint, aggregates
multiple descriptions, rejects ambiguous ids, and removes a relationship in
the same frame either endpoint disappears. Kit semantic labels, form help and
errors, search labels, and deferred tooltips use the native AccessKit relation
without changing their deterministic diagnostic vocabulary.
Adapters do not consume those references uniformly, so the same resolver also
derives an absent scalar name or description from the related node text while
preserving the references and explicit-scalar precedence. The macOS native
smoke verifies complete form names and help/error descriptions.

**Rich text now has one storage-neutral edit vocabulary.**
`RichTextDocument` carries stable block identity, hard block boundaries, soft
line breaks, complete inline style and link coverage, logical alignment, and
ordered or unordered list metadata without choosing Markdown, HTML, or a
durable format. Caller-owned `RichTextEditSession` applies typed selection,
replacement, formatting, link, list, split, merge, IME composition, undo, and
redo intents with grapheme-safe boundaries. External document replacement and
secret-history refusal cannot resurrect superseded text. `RichTextEditor`
renders that caller-owned session with styled wrapped blocks, logical
alignment, list markers, selections, diagnostics, clipboard, keyboard and IME
input, multiline accessibility text, and a quiet token-backed formatting
toolbar. Hosts still own persistence, collaboration, grammar facts, URL policy,
and stable ids for newly split blocks.

**Editable text has one framework authority.** `gpui::EditBuffer` now owns the
grapheme-safe selection, replacement, marked-composition, grouped undo/redo,
secret-history refusal, input limits, and UTF-8/UTF-16 arithmetic consumed by
both `TextInput` and `TextArea`. `EditableTextLayout` now owns the wrapped and
bidirectional offset/point mapping, visual rows, range fragments, caret bounds,
platform range envelope, alignment-aware hit testing, and minimal reveal scroll
used by both controls.
`EditableStyleRuns<S>` supplies grapheme-safe, normalized style coverage that
follows source replacement without choosing a rich-text format. The controls
no longer carry private edit or character-geometry engines; product document
policy remains separate.

**Editable native ranges use the geometry that was painted.** GPUI exposes its
generic per-grapheme AccessKit publisher at the framework boundary. `TextInput`,
`TextArea`, and `RichTextEditor` capture current-frame shaped cells during
prepaint, including horizontal/vertical scroll, wrapping, alignment, bidi, and
segmented input direction, then publish positions, widths, and run bounds
without reshaping. Deterministic tests cover all three controls and the macOS
native smoke queries distinct character rectangles, the selected range, and
the end-caret rectangle.

**The loading family says six different things.** `PulseLoader` breathes,
`Spinner` turns an open arc whose gap is what says the ring is not a position,
`Skeleton` is the shape of absent content, `BarLoader` is the strip form for a
region filling in, `LoadMore` is a list tail whose idle, loading and exhausted
states are three different pictures, and `RefreshVeil` covers last-verified
content without erasing it. `PulseLoader` and `Spinner` take a `ControlSize`
and a caller `tint`; `GradientSpinner` is removed.

Each of them now handles reduced motion itself rather than letting a repeating
animation hold frame zero. Frame zero of the old pulse was opacity 0.08 — a
row of marks nobody could see — and frame zero of a rotating refresh glyph is a
picture of being stuck, which is the one reading these exist to rule out.

**`AvatarGroup` stacks identities that will not fit as a row.** Each member
carries a cut-out ring so the one beside it cannot eat its edge or its presence
dot, and the rest are counted.

**A tooltip says what it came out of.** `overlay::tail` paints the mark that
hangs off the side facing the trigger, because a rectangle of panel colour on a
page of panel colour says only that it is there. It is a painted path rather
than a rotated square: GPUI rotates sprites and not layout boxes, and a
staircase of one-pixel rows is a triangle only until somebody looks at it.

**Thirteen glyphs the components were drawing as characters.** `calendar`,
`checkbox-empty`, `checkbox-checked`, `play`, `pause`, `sound-wave`,
`drag-handle`, `filter`, `image`, `video`, `minus`, `double-arrow-left` and
`double-arrow-right` are original drawings on the same grid as the set around
them. A markdown task list had been rendering its boxes as tofu, a date field
had borrowed the checklist glyph, and a transport had no play or pause of its
own. `stop` is redrawn: at eight units of a ten-unit box it read as a solid
block among stroked glyphs rather than as a mark.

**Media capability and failure vocabulary survives the platform seam.**
`MediaCapabilities` reports audio/video, seek, volume, rate, native-track, and
output-selection support at runtime. Player controls omit or disable operations
the selected backend cannot honor. `MediaErrorKind` keeps no-backend,
invalid-source, open, playback, and refusal categories machine-readable while
preserving the backend's diagnostic text.

**A shared `Phase` projects every per-surface state enum.** `HasPhase` reports
which of ten phases a surface is in, the host's reason, and whether a failed
refresh left a verified value on screen. Construction APIs are unchanged.
`StateView` renders the phase; `RefreshVeil` covers last-verified content
during a refresh.

**Loading and status vocabulary for downstream waits.** `Spinner` is the inline
turn. `Skeleton` takes a sequence of shapes. `ProgressBar` and `ProgressCircle`
distinguish stalled and paused work and can report cancel. `StageProgress` is
a host-owned multi-stage run. `Banner` is the page-level callout. `OutcomePanel`
is the counterpart to `FailurePanel`. `StaleMark` names a stale verified value.

### Changed

**Agent transcript evidence no longer competes with the answer.**
`ToolCall` replaces the card-shaped `ToolCallCard` with a caption-sized mono
row, a required `ToolFamily`, a display-safe key-argument summary, a state dot,
inline failures/refusals, and controlled expansion. Expanded arguments and
results are unlabelled, borderless mono washes, bounded to four lines by
default with an explicit remaining-line count. `ThinkingBlock` uses the same
quiet row language and expands to muted italic text. `AgentDocument` groups
adjacent evidence more tightly and gives its labels the caption/faint tier.
Markdown's existing built-in scanner now includes YAML/YML alongside JSON,
Rust, TypeScript/JavaScript, shell, TOML, Python, Go, and Markdown; a host
highlighter still overrides it.

**`EmptyKind` gained `Unauthorized`.** An authorization refusal is no longer
folded into `Unavailable`. Exhaustive matches on `EmptyKind` will not compile
until they name the new variant.

**The public site is three destinations, not one catalog dump.** `/` is the
Box: thesis, a live specimen in both themes, the six surfaces the library
actually is, and selected plates. `/components/` groups the catalog by kit
module and each component has its own page, with signatures and scene
examples folded closed. `/docs/` groups the contracts and puts MCP first;
`POST /mcp` is unchanged. Old `/#components`, `/?component=`, `/?scene=`,
and `/scenes/` addresses redirect.

**A decorative line is no longer held to a control boundary's contrast.**
`interactive.hairline` and `interactive.divider` leave the 3:1 non-text gate
and enter a separation floor of 1.5 L\* against each of the six surfaces:
nobody aims a pointer at a rule between two menu groups, so the question it
has to answer is whether it can be seen, not whether it can be hit.
`interactive.track` and `interactive.hairlineStrong` — slider rails, switch
edges, scrollbar gutters, resize seams — keep 3:1, because those are aimed
at. `TokenError::Line` reports a line that composites back into its own
surface, and `contrast::line_report` returns the whole table. Holding all
four to 3:1 was what forced both Studio themes to 35–55% alphas, which drew
an outline around every card, table, menu, and toolbar in the catalog; both
themes are retuned to match, so every scene image moves.

**A table and a data grid draw no row rules by default.**
`GridLines::default()` is now `None` for both `Table` and `DataGrid`, which
were `Rows`. Row height, hover, selection, and the optional zebra striping
separate rows; a rule between every pair of them is a grid drawn over data
that already has a shape. Callers who want the old appearance ask for it:
`.lines(GridLines::Rows)`. This is a breaking change to the rendered default,
not to the API.

**Selection is one statement everywhere: a neutral wash and an accent rail.**
Collections use `SelectedRow::selected_row`, which pairs
`interactive.selected` with an accent bar `effect.selectionRailWidth` wide at
the reading edge. Neither consumes layout, so arriving on a row still moves
nothing. The inset neutral ring that `Card`, `Tag`, menu rows, and canvas
nodes drew is gone: it read as a border the thing had always had. Where a row
is not a row, selection is the accent in the foreground — a menu row's label,
a segment's label, a sidebar place's glyph — or, for a node floating on a
canvas, the accent all the way round it.

**A button variant no longer carries an outline it does not need.**
`Secondary` is a tonal fill, `Danger` is a danger-coloured tint with
danger-coloured text rather than a solid red block with white text, and the
transparent placeholder border every variant carried is gone. `ButtonGroup`
abuts its children with a seam rule instead of overlapping their borders with
a negative margin. A selected button is an accent tint with accent text; it
previously took a neutral wash over a raised fill, which read as disabled.

**A split handle draws a rule and a grip rather than a filled lane.**
The lane painted `panel` between every pair of panes whether or not it could
be moved. It now carries a `divider` rule along its length, and, only when the
split is actionable, a short grip that turns accent under the pointer.

**A scrollbar has no gutter and a scroll shadow has no line.**
The bar's track paints nothing until the pointer is over the region, and the
thumb rises from 55% to full. The 1px `hairlineStrong` line under a scrolled
edge is a gradient band of `backdrop`: content continuing past an edge is a
soft fact, and a hard rule there reads as a boundary that has been reached.

**An elevation step is now an ordered set of layers.**
`elevation.flat`, `raised`, `overlay` and `modal` are arrays. `flat` is
empty rather than a transparent layer. The theme adapter already handed
components a `Vec<BoxShadow>`, so a document that names two layers at
`raised` reaches the renderer without a component change. Steps must
strictly increase in reach (`y + blur` of the farthest layer). This is a
breaking change to the portable schema and to `ElevationTokens`.

**The catalog tools follow this repository, not the crates.io cohort.**
`search_components` and `component` now answer supporting types from
`docs/api-index.json` (`CardHeader`, `CardVariant`, `AsyncValue`, and the
rest) as well as mountable components. The hosted Worker states that it
serves the current deploy of this tree rather than a published crate.
`tools/mcp/run.sh` starts the checkout stdio server. The product-UI skill
names current Kit types instead of gallery-only helpers.

**A scene declares what it is for, and the catalog is grouped by family.**
`gpui_kit::scenes` was one 8,400-line file registering 108 flat scenes; it is
now `scenes/` with one file per component family and `Shows` on every
registration. `Shows::Subjects` names the components a rendering is the review
of; `Shows::Composition` marks the three arrangements (`motion-flip`,
`motion-state`, `reading-direction`) that are built the way a product would
build them and are nobody's coverage. The `scenes` list on a component in
`docs/api-index.json` therefore answers "where do I go to look at this"
instead of "what code path touches this type": it used to be inferred by
following every helper a scene called, so `hover-card`, `menubar`, and
`copy-button` all reported the same seven components because they share one
fixture. `api check` now holds each declaration to what the source can reach
and fails when a public component has no exhibit.

**A component is reviewed where it is built, not where it is glimpsed.**
An exhibit's own source must now build every component it says it reviews.
Reaching one through a component it mounts still counts for a `Shows`
declaration — that is how a tooltip's view is nameable — but no longer counts
as a review. Two components were relying on it: `AgentAvatar` was a picture of
an avatar inside a card inside a roster, and `AgentRunIssues` draws nothing at
all unless a snapshot is malformed, so the scene that claimed to cover it had
never rendered a pixel of it. `api check` also requires a scene's build
function to carry the scene's name; `anchor_navigation` and
`diagnostics_surface` are now `anchor_list` and `diagnostics_list`.

**A component page and the MCP catalog distinguish review from appearance.**
`/components/<Name>` says "Reviewed in" and separately lists the compositions
that draw the component without reviewing it. `component(name)` says the same
over MCP, and `scene(name)` reports whether the scene is an exhibit or a
composition instead of an undifferentiated `uses` list.

### Added

**Five exhibits where `content` used to be.**
`progress-bar`, `divider`, `tag`, `avatar`, and `empty-state`. One scene named
after nothing was reviewing five unrelated components, so a change to `Tag`
moved the image that also stood for `Avatar`. Each now shows more states than
the slice it replaced: every `EmptyKind`, every `Tag` tone, `Avatar` at three
sizes and with no name at all.

**`agent-avatar` and `agent-run-issues`.**
The presence marks side by side, the execution sentence in all five outcomes,
and a deliberately malformed run snapshot beside a well-formed one — which is
the only way to see that `AgentRunIssues` draws nothing when nothing is wrong.

**A scene for `Icon` and one for `Responsive`.**
Both existed only inside other components' scenes, so the whole glyph catalog,
the nine icon tones, the direction rule, and a container that arranges itself
from its own measured width were recognisable everywhere and reviewed nowhere.
The coverage gate is what found them.

**One recipe for each visual statement the library repeats.**
`foundation::rule` and `rule_vertical` are the line that divides content
sharing a surface — child elements rather than borders, so they can be inset
and so a component spending its border on focus can still draw one.
`foundation::selection_rail` and the `SelectedRow` and `Hoverable` traits are
how a collection says which row it is on and which row the pointer is over. A
statement drawn a dozen ways is a dozen different statements wearing one name,
which is what these exist to stop.

**`effect.selectionRailWidth`.**
The width of the accent bar at the reading edge of a selected row, in both
bundled themes and required by the schema. It must be positive.

**A backdrop surface below the page.**
`Surface::Backdrop` is the substrate a card can sit on, darker than
`canvas`. It is compared to `canvas` and `panel` at the same 3 L\* floor as
the rest of the ramp, and not to `sunken`: a well never sits on the
substrate. Both Studio themes ship one (`#050505` / `#dcdce2`) that the
existing five surfaces already clear, so the gallery and the headless
baselines do not move.

**A sidebar place can carry a caller-owned image in the glyph slot.**
`SidebarItem::image(path)` takes the same asset-source path `Avatar::image`
uses. It shares the leading slot with `icon()`, so the last call wins, and
the type stays `Clone + PartialEq + Eq`. A collapsed rail still draws the
image. This is for product marks a catalog glyph cannot name; it is not a
generic leading element.

**A surface separation and tone distinction contract in the token layer.**
`TokenDocument::validate` now measures both in CIE L\*, because the WCAG ratio
compresses near black and near white and a theme can pass it while looking
like one flat plane. Six nestings — canvas to panel, panel to card, card to
raised, and the overlay against what it covers — must differ by at least three
L\* in the declared direction, and the `muted`, `faint`, `placeholder` and
`disabled` rungs must each differ by three L\* measured as distance from the
canvas, so one rule holds in both appearances. Both bundled Studio themes were
retuned to pass: dark had been drawing three of those four tones as the same
grey, and light had `placeholder` stronger than `faint`. `xtask tokens
generate` prints both tables, the contrast gate reports both sets of failures,
and `docs/token-model.md` states the contract. Every macOS baseline was
re-rendered; the Windows set still has to be accepted on Windows.

**A card component family, and one definition of the card shell.** `Card`
carries `Elevated`, `Outlined` and `Ghost` variants, a `CardHeader` with
title, subtitle and action, and media, body, footer and `divided` regions
whose padding collapses against an adjacent region rather than doubling. It is
selectable and disableable, a disabled card installs no handler, and an
actionable card without a `name` falls back to its header title so it cannot
reach the audit unnamed. `StyledExt::card_surface` is the same shell for the
twelve components that own a richer semantic node than a grouping and so
cannot be wrapped in a `Card`; the agent, game and notification surfaces that
each hand-rolled their own now use it.

**Slots, container-size response, a theme escape hatch, and shape in flip.**
`Slotted::SLOTS` lets a caller replace a component's `EMPTY`, `FAILED` or
`LOADING` region by name, and a name the component does not declare panics
instead of silently rendering the default. `Responsive` reports a
`ContainerSize` that is either `Measured` or honestly `Unmeasured` — never a
guess — and `Toolbar` now computes its overflow cut from the widths it
measured last frame, so a moved item cannot make room for itself and
oscillate. `ThemeOverlay` adjusts the theme for one subtree across
request_layout, prepaint and paint, so a host can restyle a region without any
component learning about it. `Flip::shape` interpolates radius, border width,
border colour and background over the spring that already carries position and
size, and snaps under reduced motion.

**Semantic cinematic effects and optional dotLottie playback.**
`CinematicEffect` maps an existing `EffectPlan` to policy-owned asset slots,
durations, poster frames, RTL mirroring, localized semantics, and the built-in
particle fallback, so chat, persona, agent-run, and game surfaces choose an
event rather than a filename or timeline. Hosts supply resolved bytes and own
typed play, pause, stop, seek, and bounded state-machine-input requests. The
optional `dotlottie` feature adds a pure-Rust `rasterlottie` adapter with
encoded, archive, expansion-ratio, entry, dimension, pixel, frame-rate, frame,
duration, animation-count, state-machine-count, embedded-image-count, source
dimension, and aggregate target-pixel ceilings. Validation fully expands each
admitted entry into a bounded sanitized archive and inspects image headers
before the decoder sees it. Without the feature, after any rejection, and under
reduced motion, the same core API renders an explicit particle fallback or
deterministic poster rather than a blank effect.

**Renderer-backed radial and conic gradients.** `Background` now carries
linear, elliptical radial, and clockwise conic geometry through the same
bounded two-to-eight-stop scene ABI. Quads and filled paths share sRGB/Oklab
interpolation, deterministic screen-pixel dithering, clipping, and opacity on
Metal, Direct3D, WGPU, and browser WebGL. Callers choose normalized geometry
and semantic colors; they no longer approximate glows, wheels, or area washes
with rings of adjacent elements.

**Composited sprite batches.** `Window::paint_sprite_batch` reuses one atlas
upload for explicit source rectangles with center-relative translation,
rotation and scale, transformed rounded/source-alpha masks, tint and opacity,
and normal, additive, or screen hardware compositing. Metal, Direct3D, WGPU,
and WebGL share the scene ABI and split batches only where texture, paint order,
or blend state requires it. Invalid batches report an error before painting any
instance. `RenderImage::from_rgba` also converts procedural RGBA8 pixels to the
private atlas layout, so a host does not hand-swap channels. The primitive is
paint-only and does not pretend that alpha holes are hitboxes or accessibility
nodes; subtree-wide offscreen composition remains deliberately unsupported.

**Deterministic batched particles.** `ParticleEmitter` samples bounded CPU
trajectories from a stable seed and absolute elapsed time, then
`Window::paint_particle_batch` submits every live instance through one
atlas-backed sprite batch. Birth order, spawn area, velocity, acceleration,
size, rotation, fades, tint, masks, and hardware compositing remain identical
across dropped-frame histories; invalid or over-4,096-slot batches fail before
painting. Kit's `EffectParticles` consumes an `EffectPlan` directly and owns
the recipe topology, procedural atlas, theme palette, RTL mirroring, timing,
frame requests, and fixed reduced-motion fallback. Downstream chat and game
surfaces therefore report semantic effects instead of building one element per
particle or selecting shaders, particle counts, colors, and degradation rules.

**Measured path-stroke effects.** `PathBuilder::stroke_trim` reveals any
ordered normalized interval before stroke tessellation, while `dash_offset`
advances and wraps a validated dash pattern against the same path measurement.
Curves, transforms, joins, caps, clipping, and gradient paint stay intact;
empty or all-zero patterns stay safely solid. Node-graph traffic now uses the
shared trim primitive instead of rebuilding tails from short sampled lines.

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

**Complete number and plural adapters.** Library-authored numeric facts now go
through `NumberAdapter` the way dates go through `DateAdapter`: grouped counts
and decimals, editable decimal parsing, count-of-total, percentages,
quantities and affixes, dimensions, ordinals, signed deltas, lower bounds, and
playback multipliers. `NumberInput` parses through the same adapter that writes
its value. `Strings` accepts `zero`, `one`, `two`, `few`, `many`, and `other`
phrase overrides, so components no longer choose English plurals themselves.
The built-in English adapter is a complete fallback, not locale discovery;
caller-authored clock, currency, source, identifier, and diagnostic strings
remain verbatim.

**Schema shapes a settings page can name.** `SchemaKind` now includes
`Date`, `Time`, `DateRange`, `Files`, and repeating `List`. Until a host
supplies a date adapter or a file policy those fields stay visible as
unrenderable rather than disappearing.

**Charts.** `ChartPoint` gives every sample caller-owned business identity and
exact display text. `LineChart` and `BarChart` now run keyed enter/update/exit
motion without delaying semantic values; reduced motion settles immediately.
Line charts can add renderer-backed multi-stop area fills and an animated
crosshair whose pointer and keyboard callbacks report business ids while its
tooltip swaps exact strings atomically. A failed refresh can retain the last
verified series as an explicit stale state. Axis wording, domains, queries,
aggregation, and locale policy remain host-owned.

**Schema date fields.** `SchemaKind::Date`, `Time`, and `DateRange` now
construct the existing date controls when the host has installed a
`DateAdapter` through `set_date_adapter`. Without one they stay
unrenderable. `Files` uses the installed `SchemaFilePolicy` for admissibility
and display names while the host still owns the OS picker; repeating `List`
owns add/remove UI and stable item identity without requiring another policy.

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
