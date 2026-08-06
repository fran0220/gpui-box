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
| Action | `Button`, `IconButton`, `ButtonGroup`, `SplitButton` |
| Text entry | `TextInput`, `TextArea`, `NumberInput`, `TagInput`, `InlineEdit` |
| Choice | `Select`, `Combobox`, `Checkbox`, `Radio`, `Switch`, `Slider`, `SegmentedControl` |
| Form | `FormField`, `SettingsRow`, `SettingsSection` |
| Navigation | `Tabs`, `Accordion`, `Breadcrumb`, `Sidebar`, `Pagination`, `Wizard` |
| Data | `List` (virtualized), `Table`, `DataGrid` (virtualized), `BulkBar`, `Tree` |
| Display | `Badge`, `Tag`, `Avatar`, `Card`, `ListRow`, `Divider`, `ProgressBar`, `EmptyState`, `StatusDot`, `StatusLine`, `Callout`, `PulseLoader`, `GradientSpinner`, `Skeleton`, `ProgressCircle`, `DescriptionList`, `Timeline` |
| Overlay | `Overlay`, `Dialog`, `Drawer`, `Popover`, `Menu`, `ContextMenu`, `CommandPalette`, `Tooltip`, `Toast`, `ToastLayer`, `Kbd` |
| Layout | `SplitPane`, `SplitTree`, `ScrollArea`, `Toolbar` |
| Shell | `Dock`, `StatusBar` |
| Keymap | `KeybindingRecorder` |
| Interaction | `Dropzone` |
| Filtering | `FilterBar` |

`Tooltipped` is an extension trait rather than a component: it attaches a
`Tooltip` to any element, and is covered wherever that `Tooltip` is.

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

## Helpers, which the four-part rule does not reach

`field_shell` and `FieldState` draw the one border, background, and focus
treatment every editable control wears. `FocusTrap` keeps the keyboard inside
an open overlay. Neither renders on its own, so neither has a scene of its
own; both are exercised through every control and overlay that uses them.

## Out of scope, and why

- **Time formatting.** `Timeline` displays times and day headings, and
  formats neither: it takes strings the host has already put into words. The
  same reasoning that keeps calendars out of this crate keeps
  “two minutes ago” out of it, and the seam is the adapter above the
  component.
- **Date, time, and calendar pickers.** Correctness here is calendar,
  time-zone and locale work, not UI work. A library that ships a half-correct
  calendar is worse than one that ships none, and the honest form of it belongs
  next to a date library this crate does not have.
- **Judging a keybinding.** `KeybindingRecorder` captures a keystroke and
  reports it. Whether it clashes with something, and what to do about it, needs
  the keymap, which the host owns; the recorder renders the conflict the host
  found rather than inventing one.
- **Persisting a layout.** `SplitLayout` converts to and from plain records so
  a host can write it out, and this crate takes no serialization dependency to
  do it for them.
- **Rich text and code editing.** `TextArea` edits plain text. Syntax
  highlighting, folding, and multi-caret editing are an editor, not a control.
- **Charts.** A chart is a data-visualisation library with its own scales,
  axes and accessibility model.
- **Colour picker, file picker, print dialog.** Platform surfaces; a host
  should reach the operating system rather than a reimplementation.
- **Menu bar and window chrome.** Owned by the platform window, not by a
  component tree.
- **Carousel, rating, and other marketing patterns.** Not desktop-application
  vocabulary.

## Rules every covered component follows

1. The answer belongs to the caller. A component holds hover, focus, open, and
   animation state; a value, a selection, a sort, and an expansion belong to
   the host, which is why every one of them reports an intent instead of
   applying it.
2. A refused or disabled control installs no handler at all.
3. Loading, empty, unstarted, unavailable, and failed are distinct, and a
   refusal is never rendered as an absence of data.
4. Ids come from business identity, never from list position.
5. Anything visible comes from tokens.
6. A component that can carry a credential publishes its shape, never its text.
