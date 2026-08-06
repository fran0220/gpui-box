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
| `FieldFrame`, `SearchFrame` | builder | — | Chrome for a host-supplied editable surface |

## Display

| Component | Kind | Notes |
|---|---|---|
| `Badge`, `StatusDot`, `StatusLine`, `Callout` | builder | Status vocabulary |
| `Card`, `ListRow` | builder | Grouping |
| `ProgressBar` | builder | Reports a position only when the extent is known |
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
| `Popover` | view | The anchored surface `Menu` and `Select` are special cases of. Owns only whether it is open: the body is a per-frame callback, escape and a click outside dismiss it unless it is not dismissable, and closing gives the keyboard back to the trigger |
| `Menu` | view | Commands, checkable rows, separators, section labels, and nested submenus, opened from a trigger. Up and down step over rules, labels, and refused rows; a letter jumps to the next row starting with it; right and left enter and leave a submenu; escape folds one submenu away before it closes the menu. Taking a row reports it once and closes the whole chain, and a refused row installs no handler |
| `ContextMenu` | view | The same list opened at the pointer over a wrapped region. Reports the target it was opened on and selects nothing, because opening a menu is not choosing anything. A surface that would leave the viewport flips to the other side of the pointer |
| `MenuItem` | builder | One row: `command`, `check`, `separator`, `section`, or `submenu`, with an optional shortcut hint and icon. A checkable row draws the state the host holds and reports the intent to change it |
| `CommandPalette`, `Command` | view, builder | A query field over a command list, filtered by `popover::match_rank` — prefix, then word start, then substring, then subsequence — with sections kept contiguous behind their best match. Nothing matching shows an `EmptyState` naming the query that answered nothing, and a command the host marked unavailable stays listed with its reason rather than being hidden |
| `Tooltip` | builder | Hover-delayed help on GPUI's hover machinery. Never actionable, and never the only copy of what is needed to act |
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
