# Components

Every component derives its GPUI element id and its semantic assertion id from
one caller-supplied `Ident`, reads the theme from the application context, and
publishes a semantic node during prepaint. Builders are `RenderOnce`; anything
that must survive a frame is a view.

## Controls

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `Button` | builder | click | No handler is installed while disabled or loading |
| `TextInput` | view | change, submit, cancel, focus, blur | Grapheme-aware editing, input-method composition, masking, length limit |
| `TextArea` | view | change, submit, cancel, focus, blur | Wrapped multi-line editing. Enter inserts a line and the primary modifier plus enter submits. Motion follows visual rows with a preserved goal column, and the frame grows from `rows` to `max_rows` before it scrolls |
| `Select` | view | selected, opened, closed | Owns only whether the menu is open |
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
| `Combobox` | view | selected, custom, opened, closed | A `Select` you can type into. Escape puts the query back to the current answer and reports nothing. A query nothing answers reports nothing unless `allow_custom` |
| `TagInput` | view | added, removed, duplicate, refused | Enter or comma commits a token. The first backspace in an empty field singles out the last tag and the second removes it. A duplicate and a full field are refusals shown where the typist is looking |
| `SettingsRow`, `SettingsSection` | builder | — | One setting per row: name and description on the left, the caller's control on the right. A row that is managed elsewhere, or that belongs to a section which does not apply here, never renders the control at all |
| `FilterBar` | builder | add, remove one condition, clear them all | The conditions are the caller's, and so is the result count. Counting, a known count, a count nobody established, and a count the host refused are four different things |
| `InlineEdit` | view | edit requested, commit, cancel | Text that becomes a field where it stands. The component never opens itself, never applies a commit, and a refused save keeps what was typed |
| `KeybindingRecorder` | view | recording started, a captured keystroke, cancelled | Captures the next keystroke instead of acting on it, and reports it in GPUI's own syntax so it goes straight into a keymap. A modifier alone is not a keystroke, escape ends recording rather than being captured, and a conflict is the reason the host found |
| `field_shell`, `FieldState` | helper | — | The one border, background, and focus treatment every editable control draws. A composed field — `NumberInput`, `Combobox`, `TagInput` — wraps a bare input in one of these rather than nesting two frames |

## Display

| Component | Kind | Notes |
|---|---|---|
| `Badge`, `StatusDot`, `StatusLine`, `Callout` | builder | Status vocabulary |
| `Card`, `ListRow` | builder | Grouping |
| `ProgressBar` | builder | Reports a position only when the extent is known |
| `AnimatedNumber` | builder | Counts to a new value, and publishes the target from the frame it changes: a number in flight is not a fact. A caller-supplied format function decides the text |
| `Tag` | builder | Removal exists only when removal is allowed |
| `Avatar` | builder | Initials fallback, blank when there is no name |
| `Divider` | builder | Optional caption |
| `EmptyState` | builder | Names which of empty, unstarted, unavailable, or failed holds |
| `PulseLoader`, `GradientSpinner`, `Skeleton` | builder | Publish a busy indeterminate node |
| `ProgressCircle` | builder | The ring form of `ProgressBar`, over the same state. A position only when the extent is known; an unknown extent tints the whole ring rather than part of it |
| `DescriptionList` | builder | Term and value pairs for a detail page. Unknown, not applicable, and redacted are three different facts, and a redacted value carries only its shape |
| `Timeline` | builder | A chronological feed. Every time and every day heading is a string the caller already formatted, and an entry whose time nobody knows says so |

## Navigation

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `Tabs` | builder | the tab that was picked | Renders the strip only, never a panel, so no `TabPanel` node is published; the caller renders the body. Left, right, home, and end move between tabs, skipping disabled ones and stopping at the ends |
| `Accordion` | builder | a section id and the state it should take | A closed section does not render its body at all. `exclusive` changes only what is reported: opening a section also reports a close for every other open one |
| `Breadcrumb` | builder | the crumb that was picked, and the ids an ellipsis hides | The last crumb is the current place: it publishes `Text` rather than `Link` and installs no handler. `max_visible` collapses the middle of a long trail and publishes the hidden count |
| `Sidebar` | builder | the place that was picked | Sections, badges, and one level of nesting. Collapsing narrows the drawing, never the substance: a glyph-only rail reaches each label through a `Tooltip` and every item still publishes its full name and its depth |
| `Wizard` | builder | a step to jump to, back, next, or finish | A step strip with the caller's body under it, horizontal or vertical. A step is complete, current, upcoming, blocked, or failed, and the last two say why |
| `Pagination` | builder | the page that was asked for | First, previous, next, last, and a numbered range with an ellipsis that says how many pages it stands for. A step with nowhere to go installs no handler. With `PageTotal::Unknown` there is no last-page control, no numbers, and no total in the copy |

### The wizard moves nothing

`Wizard` reports `Step`, `Back`, `Next`, and `Finish`; which step is current
stays with the caller, exactly as `Tabs` never switches its own tab. Only
completed steps are revisitable by default, and a step nobody may jump to
installs no handler. `Blocked` and `Failed` carry the host's reason and publish
it as a child node, because a step that has gone grey for a reason nobody
states is a dead end.

### An unknown page count is not a page count

`PageTotal::Known` and `PageTotal::Unknown { has_next }` are different facts. A
host that paginates a cursor knows only whether one more page exists, so that
is all the control claims: it offers next and previous, states "Page 9" with no
total, and publishes no `value` on the container. Rendering an invented last
page would be a number nobody counted.

## Layout

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `SplitPane` | builder | the ratio a drag or a keystroke asked for, and the side a double-click would collapse | Minimum sizes become a travel range published on the divider, and a drag past a minimum reports the minimum rather than a value the caller would have to clamp. A pane at ratio 0 or 1 drops its content instead of drawing it at zero size |
| `ScrollArea` | builder | — | Scroll position is transient view state, held per identity like `List`. A gutter is reserved for every enabled axis whether or not a thumb is drawn, so turning a scrollbar on never reflows the content that decided it was needed. A hairline shadow fades in at the top once the content is off the top, read straight off the offset rather than animated |
| `Toolbar` | builder | — | Groups separated by rules, a spacer, and an overflow menu. Every action inside still reports itself |
| `SplitTree` | builder | the ratio a divider asked for, and the pane a double-click would collapse | However many nested splits the caller declares, as a `SplitLayout` the caller owns. Minimums propagate up the tree, so a divider stops where a leaf far below it would run out of room, and a collapsed leaf is drawn at its rail with no divider beside it |
| `Dock` | builder | a panel that was picked, a panel that was dragged somewhere, a region asked to collapse, and a region divider's share | Panels in a left, centre, right, and bottom region around one another. Region sizes go through `SplitTree` and panel headers are `Tabs` strips, so resizing and dragging are the same two systems used elsewhere. It moves nothing |
| `StatusBar` | builder | a click on an item that has an action | Text, a toned state dot, a progress ring, an action, or a caller-supplied element, in a start, centre, and end group. An item the host gave no state claims none |

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
| `List` | builder | the row that was picked | Virtualized over GPUI's `uniform_list`. The caller renders one index at a time and stamps each row with its own identity. Up, down, home, and end move the reported selection, skip refusals, and scroll the reported row into view |
| `Table` | builder | the sort a header click implies, and the row that was picked | Sorting is caller-owned: the table reports `(key, next direction)` and renders whatever order it is handed. Columns are fixed or flex, and the header stays put while the body scrolls. Not virtualized — reach for `DataGrid` past a few hundred rows |
| `DataGrid` | builder | a sort, a column width, a column order, a selection change, a disclosure, and a finished edit | The heavyweight tabular surface: virtualized over `uniform_list`, resizable and reorderable columns, a left-pinned group, three selection modes with a truthful select-all, opened rows with a detail region, and cells that become fields. It applies none of it |
| `BulkBar` | builder | the wider selection, and the dismissal that clears the selection | Appears over a selection through `Presence`, states the count it actually has, and offers "select all N" as a separate named action when more rows exist than the host has loaded |
| `Tree` | builder | a node id and the disclosure state it should take, and the node that was picked | A collapsed node renders none of its children. Up and down walk visible nodes, right opens a shut branch or descends into an open one, left shuts an open branch or ascends |

### Only rendered rows are published

A virtualized surface holds a viewport, not a data set. A row outside the
viewport is never laid out, has no bounds, and publishes no semantic node, so a
snapshot describes what is on screen and nothing else. The container node
carries the total in `value`: a test asserts that the list holds a thousand
items and drew twelve, rather than pretending the other nine hundred and
eighty-eight are addressable.

Virtualization needs a bounded viewport. With `List::visible_rows` the list
draws only the rows that fit; without it the list sizes itself to its content
and every row is laid out.

`Table` is the exception, and deliberately: it takes materialized rows, so
every cell element already exists by the time the table sees it and every row
it is given is rendered. Reach for `List` when the data set is large.

### Table or DataGrid

Both are column-oriented and both report rather than apply. The difference is
what they are handed:

- **`Table` takes rows.** The caller builds every cell before the table sees
  it, which is why the whole set is laid out and why a table stays readable at
  a few dozen rows and stops being sensible at a few thousand. Reach for it for
  a settings summary, a short run list, a preview of a result set.
- **`DataGrid` takes a closure.** It asks for one row at a time and only for
  the rows the viewport holds, which is what lets it carry twelve thousand rows
  and also what makes column resizing, reordering, selection over an
  incompletely loaded set, opened rows, and cell editing worth its weight.
  Reach for it for the administrative surface: the thing with a header, a
  selection, a bulk bar, and more rows than fit.

If a surface would work as either, pick `Table`. It is smaller, and a grid's
machinery costs something even when nothing uses it.

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
| `MessageList` | builder | a failed message that should be tried again, and whatever a Markdown body reported | A conversation over the virtualized `List`. Five delivery states, a streaming mark keyed to the message rather than to its text, caller-declared grouping, and following that happens only while the reader is already at the bottom |
| `ImageViewer` | builder | the fit that was asked for, the image that was stepped to, and an image the host has not supplied | One image at a time, with contain, cover, 1:1, and zoom; the wheel zooms at the pointer and a drag pans, clamped so the picture cannot leave the frame. Loading, unavailable, failed, and ready are four renderings, and dimensions are a caller input |
| `TransportBar` | builder | play, pause, a preview while scrubbing and one seek on release, volume, mute, speed, and a track step | Playback controls for media this crate does not play. A duration the host does not know is a state, buffered ranges are drawn apart from the played position, and every readout is a string the host wrote |

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
| `Overlay` | builder | Placement, token-driven paint priority, scrim, dismissal |
| `Dialog` | view | Composed modal: reports opened, confirmed, cancelled, dismissed, closed. A dialog that is not dismissable installs no escape or scrim handler |
| `Drawer` | view | The same surface arriving from an edge: same scrim, same focus trap, same escape and scrim dismissal. It slides out through `Presence`, and because an element cannot animate after it is dropped it stays in the tree until the exit finishes and only then reports `Closed` |
| `Popover` | view | The anchored surface `Menu` and `Select` are special cases of. Owns only whether it is open: the body is a per-frame callback, escape and a click outside dismiss it unless it is not dismissable, and closing gives the keyboard back to the trigger |
| `Menu` | view | Commands, checkable rows, separators, section labels, and nested submenus, opened from a trigger. Up and down step over rules, labels, and refused rows; a letter jumps to the next row starting with it; right and left enter and leave a submenu; escape folds one submenu away before it closes the menu. Taking a row reports it once and closes the whole chain, and a refused row installs no handler |
| `ContextMenu` | view | The same list opened at the pointer over a wrapped region. Reports the target it was opened on and selects nothing, because opening a menu is not choosing anything. A surface that would leave the viewport flips to the other side of the pointer |
| `MenuItem` | builder | One row: `command`, `check`, `separator`, `section`, or `submenu`, with an optional shortcut hint and icon. A checkable row draws the state the host holds and reports the intent to change it |
| `CommandPalette`, `Command` | view, builder | A query field over a command list, filtered by `popover::match_rank` — prefix, then word start, then substring, then subsequence — with sections kept contiguous behind their best match. Nothing matching shows an `EmptyState` naming the query that answered nothing, and a command the host marked unavailable stays listed with its reason rather than being hidden |
| `Tooltip`, `Tooltipped` | builder, trait | Hover-delayed help on GPUI's hover machinery. Never actionable, and never the only copy of what is needed to act. `Tooltipped` attaches one to any element |
| `ToastLayer`, `Toast` | view, builder | Transient notifications. The host mounts the layer in the window it wants them drawn in; `overlay::toast::push` reaches it from any call site and reports whether a layer was mounted to deliver to. One action at most, an optional dismiss control, entry and exit through `Presence` |
| `FocusTrap` | helper | Keeps the keyboard inside an open overlay and restores focus |
| `Kbd` | builder | Platform-specific keystroke caps |
| `popover` | helpers | Anchoring, menu rows, cursor movement, type-ahead, filtering, and key classification |

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

**Focus.** Every interactive element is reachable with tab and wears the same
ring, from `effect.focusRingWidth` and `effect.focusRingAlpha` in the focus
colour, applied through `FocusRing::focus_ring`. The ring is a shadow rather
than a border, so focus never reflows what is around it, and it is a different
treatment from the selected ring on purpose: focus says where the next
keystroke goes, selection says which answer is current.

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

Every component appears in `gpui_kit::scenes`, which the gallery renders, the
`xtask scenes capture` task photographs in every bundled theme, and
`crates/gpui-kit/tests/scenes.rs` audits headlessly. Behaviour is asserted
through simulated key and mouse input against the published semantic tree, in
`crates/gpui-kit/tests/`.
