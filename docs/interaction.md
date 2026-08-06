# Drag and drop

A drag starts in one component and finishes in another, so the rules cannot
live inside either of them. They live in `gpui_kit::interaction::dnd`, and
every surface that can be dragged from or dropped on implements the same
contract: `List`, `Tree`, `Tabs`, and `Dropzone`.

## The contract

**The library never moves anything.** A drop reports where the item should go
and stops. The host applies the move to the data it owns and hands back a new
order, and the surface shows the reorder on the frame that new order arrives —
not before, and not at all if the host refuses.

This is the same rule the rest of the library follows for values, selections,
sorts, and expansions. It is what makes a drag truthful: a row that snapped
into its new place and then snapped back would have told a lie for the length
of a round trip, and a row that stayed put after a host refusal would look
broken rather than refused.

## Where a drop lands

```rust
pub enum DropPosition {
    Before(SharedString),
    After(SharedString),
    Into(SharedString),
}
```

A position is always expressed against something already on screen, named by
its business identity.

**"At index N" is deliberately absent.** An index is a position, and a
position stops meaning anything the moment the host applies the move — the
list it indexed no longer exists. `Before(beta)` still means the same thing
after the move, after a filter, and after a sort.

- A drop at the top of a list is `Before` the first item.
- A drop at the bottom is `After` the last one.
- A drop into a folder, or into a container that holds items without ordering
  them, is `Into` that container.

`Into` is offered only where it means something. A tree branch offers it; a
tree leaf, a list row, and a tab do not, and split in two so that every pixel
of them asks for one of the two slots beside them.

## What is carried

```rust
pub struct DragItem {
    pub source: SharedString,  // the surface the drag began in
    pub id: SharedString,      // business identity, never position
    pub label: SharedString,   // what the ghost shows
    pub kind: SharedString,    // `ROW_KIND`, `FILE_KIND`, or the host's own
    pub icon: Option<Icon>,
}
```

`source` is what lets a surface tell its own rows from somebody else's.
`kind` is what lets a target refuse a payload it does not handle.

## What a drag publishes

While a drag is in flight the semantic tree carries one extra node, id
`dnd.drag`, role `Drag`:

| Field | Value |
|---|---|
| `text` | the label of the item being carried |
| `value` | `"<item id> before:<anchor>"`, `"after:<anchor>"`, `"into:<anchor>"`, or `"<item id> none"` |
| `invalid` | set when the target under the pointer refuses the payload |

The node exists only while the drag does. A test reads it from an ordinary
snapshot and never has to sleep:

```rust
harness.drag_start("queue.gamma");
let over = harness.point_down("queue.alpha", 0.2);
harness.drag_to(over);
assert_eq!(
    harness.node(DRAG_NODE_ID).and_then(|node| node.value),
    Some("gamma before:alpha".into())
);
harness.drop_here();
```

## Refusals

A refusal is visible and reports nothing.

- The target under the pointer decides, through `accepts`, whether the payload
  may land there. A refused landing draws its indicator in the danger colour,
  the ghost takes a danger border, and the published node is `invalid`.
- Letting go over a refusing target calls no handler at all. Nothing is
  reported, and nothing moves.
- A `Dropzone` distinguishes **idle**, **accepting**, and **refusing**, and
  never renders refusing as idle. A zone that looked idle while refusing would
  tell a typist that letting go was going to work.

Two refusals are the library's own rather than the host's, because they are
structural rather than policy:

- An item offers no slot against itself. Its own row is neither accepting nor
  refusing; it simply asks for nothing.
- A tree node cannot be moved into, before, or after anything in its own
  subtree. Its descendants travel with it, so the destination would end up
  inside the thing being moved. This is judged before the caller's `accepts`
  is consulted.

Everything else is policy, and policy is the host's. Without an `accepts`, a
reorderable surface takes its own items and nothing else.

## Cancelling

Escape abandons a drag in flight. The ghost disappears, the indicator
disappears, the published node disappears, and **nothing is reported** — a
cancelled drag is not a drop on the item it happened to be over.

Escape is observed at the application rather than bound to an element, because
the pointer can be anywhere by the time a drag is abandoned and the element the
drag started on may no longer be under it.

## What the host has to do

1. Hold the order. The surface renders whatever the host currently says.
2. Take the reported intent — `on_reorder` for `List` and `Tabs`, `on_move`
   for `Tree`, `on_drop` for `Dropzone` — and apply it to that order.
3. Ask for a frame. The reorder appears because the data changed, not because
   the drop happened.
4. Refuse where refusing is right, through `accepts`, so the refusal is
   visible during the drag instead of silently discarded after it.

```rust
List::new("queue", steps.len(), render_step)
    .reorderable(true)
    .on_reorder(move |intent, _, cx| {
        let intent = intent.clone();
        host.update(cx, |host, cx| {
            host.apply_move(&intent);
            cx.notify();
        });
    })
```

The gallery's Interaction section does exactly this, and the reorder it shows
is a real one applied by the window.

## What the pointer sees

- **The ghost** is a rendering of the item — its label and its icon — not a
  grey rectangle. It is what the hand is holding, so it has to look like the
  thing that was picked up.
- **The indicator** is a line for `Before` and `After` and a highlight for
  `Into`, drawn over the target rather than between targets. A real gap would
  move the layout, and a virtualized list has no room between its slots.
- **Make-way** slides the rows at and after the insertion point aside, so the
  slot the drop would land in is visibly open.

The whole row, node, or tab is the handle. A row's ordinary action is a click,
and GPUI only calls a press a drag once it has travelled past its own
threshold, so both fit on the same row without a grip column every caller would
have to render.

## Reduced motion

The ghost is direct manipulation, not decoration: it is the thing the hand is
holding, so it **keeps following the pointer** under reduced motion. What it
loses is the spring — it tracks the pointer exactly instead of trailing it.

The make-way slides are decoration, so they settle instantly: the slot is
simply open from the first frame.

## Files from the platform

A file drag from outside the application never passes through the library's
own gesture. A `Dropzone` adopts it, so everything downstream sees the same
session an in-application drag produces, and reports the paths through
`on_files`.

The adopted item is labelled by **count** — "3 files" — and never by path. A
path is user-generated content, and the label is published in the semantic
tree.

## Staging, for captures

A still image cannot photograph a gesture, and a capture that waited for a real
drag would race the pointer and the spring. `dnd::stage` puts the system into
one fixed state — a carried item, a landing, an open slot, no pointer, no timer
— so the scenes `drag-list` and `drag-tree` render the same pixels every run.
`dnd::staged_ghost` returns the ghost for a scene to place itself.

Under staging the make-way slides settle instantly, for the same reason they do
under reduced motion.
