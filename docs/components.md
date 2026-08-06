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

## Overlay

| Component | Kind | Notes |
|---|---|---|
| `Overlay` | builder | Placement, token-driven paint priority, scrim, dismissal |
| `Dialog` | view | Composed modal: reports opened, confirmed, cancelled, dismissed, closed. A dialog that is not dismissable installs no escape or scrim handler |
| `Tooltip` | builder | Hover-delayed help on GPUI's hover machinery. Never actionable, and never the only copy of what is needed to act |
| `FocusTrap` | helper | Keeps the keyboard inside an open overlay and restores focus |
| `Kbd` | builder | Platform-specific keystroke caps |
| `popover` | helpers | Anchoring, menu rows, filtering, and key classification |

## What a component owns

A component holds hover, focus, open, and animation state. It never holds the
answer: a value, a selection, and a list all belong to the caller. A host that
refuses a change simply does not apply it, and the control keeps showing what
is still true. This is why `Select` reports the option that was picked instead
of moving its own checkmark.

## Still missing

These are known gaps rather than deliberate omissions:

- `TextArea`: multi-line editing with wrapped shaping and vertical motion.
- `Table`, `Tree`, virtualized `List`: large data surfaces.
- `Toast`: transient notifications with a stack and a timer.

## Validation

Every component appears in `gpui_kit::scenes`, which the gallery renders, the
`xtask scenes capture` task photographs in every bundled theme, and
`crates/gpui-kit/tests/scenes.rs` audits headlessly. Behaviour is asserted
through simulated key and mouse input against the published semantic tree, in
`crates/gpui-kit/tests/`.
