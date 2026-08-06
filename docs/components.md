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

## Navigation

| Component | Kind | Reports | Notes |
|---|---|---|---|
| `Tabs` | builder | the tab that was picked | Renders the strip only, never a panel, so no `TabPanel` node is published; the caller renders the body. Left, right, home, and end move between tabs, skipping disabled ones and stopping at the ends |
| `Accordion` | builder | a section id and the state it should take | A closed section does not render its body at all. `exclusive` changes only what is reported: opening a section also reports a close for every other open one |
| `Breadcrumb` | builder | the crumb that was picked, and the ids an ellipsis hides | The last crumb is the current place: it publishes `Text` rather than `Link` and installs no handler. `max_visible` collapses the middle of a long trail and publishes the hidden count |
| `Sidebar` | builder | the place that was picked | Sections, badges, and one level of nesting. Collapsing narrows the drawing, never the substance: a glyph-only rail reaches each label through a `Tooltip` and every item still publishes its full name and its depth |
| `Pagination` | builder | the page that was asked for | First, previous, next, last, and a numbered range with an ellipsis that says how many pages it stands for. A step with nowhere to go installs no handler. With `PageTotal::Unknown` there is no last-page control, no numbers, and no total in the copy |

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
| `ScrollArea` | builder | — | Scroll position is transient view state, held per identity like `List`. A gutter is reserved for every enabled axis whether or not a thumb is drawn, so turning a scrollbar on never reflows the content that decided it was needed |
| `Toolbar` | builder | — | Groups separated by rules, a spacer, and an overflow menu. Every action inside still reports itself |

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
| `Table` | builder | the sort a header click implies, and the row that was picked | Sorting is caller-owned: the table reports `(key, next direction)` and renders whatever order it is handed. Columns are fixed or flex, and the header stays put while the body scrolls |
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

Cells are quiet by default. A table of two hundred rows and six columns would
bury every other assertion target under twelve hundred nodes that repeat what
the row already says, so a cell publishes a `Cell` node only where the caller
marks it with `Cell::published`, under the id `<row id>.<column key>`. A
sortable header publishes a `Button` carrying its current direction in `value`;
a header that does not sort publishes a `Cell` and installs no handler.

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
