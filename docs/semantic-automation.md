# Semantic automation

GPUI renders native windows without a browser DOM. `gpui-box-kit-semantics`
provides a small, transport-independent semantic tree.

## Registration

```rust
use gpui_kit_semantics::{NodeSpec, Role, Semantic, SemanticCoordinator};

let coordinator = SemanticCoordinator::global(cx);
let diagnostics = coordinator.arm(); // retain for the consumer's lifetime
coordinator.begin_frame(window);
button.semantic_in(
    cx,
    NodeSpec::new("settings.account.sign-out", Role::Button)
        .text("Sign out")
        .disabled(saving),
)
```

The decorated element records its bounds and GPUI-resolved focus handle after
subtree prepaint while a diagnostic consumer is armed; diagnostics add no
layout or paint element and consume no input. Kit installation itself leaves
diagnostics dormant, so a release application with no harness, inspector, or
automation session installs no diagnostic callbacks and records no diagnostic
nodes. Dropping the `DiagnosticArm` guard disarms that consumer. Platform
accessibility is independent and remains active. A caller-owned handle declared
with `NodeSpec::focus` remains
authoritative. Otherwise a focusable element publishes the stable handle GPUI
created for `tab_index`; a non-focusable element remains unfocused. The same
call also projects the supported role, name, value, control state, focus,
range, and widget selection into GPUI's AccessKit tree. `NodeSpec::labels` and
`NodeSpec::describes` also become native labelled-by and described-by
relationships when both stable ids resolve uniquely in the active window,
including across deferred overlays. See [Accessibility](accessibility.md) for
the exact platform capability matrix and unsupported boundaries.

## Frame lifecycle

Call `SemanticCoordinator::global(cx).begin_frame(window)` once at the top of
each window root render, whether or not diagnostics are armed: the same clock
temporarily supports Kit's older transient state. While armed, the installed
coordinator owns a stable
`WindowSemanticContext` for every GPUI `WindowId`; nodes registered during that
window's frame form its next snapshot. A node not rendered in the next frame
disappears. Rendering or closing another window cannot clear this tree, advance
its generation, or collide with its local ids.

Each window alternates between two node buffers. Opening a frame clears the
next buffer instead of retaining over every node from the previous generation.
The coordinator and all per-window buffers are UI-thread state behind one
`RefCell`, so a registration does not traverse nested mutexes.

This prevents closed dialogs, hidden panels, and removed rows from remaining as
stale automation targets.

## Stable IDs

IDs use capability and business identity:

```text
settings.account.sign-out
project.<project-id>.open
model.<model-id>.select
```

Do not use:

```text
button-3
row-7
right-panel-child-2
```

Ordering and internal layout are implementation details.

## Node data

A node reports:

- id and role;
- optional parent and visible text;
- measured bounds;
- visible and focused;
- disabled and selected;
- hovered and pressed;
- optionally `checked`, `expanded`, `level`, `busy`, `invalid`, `required`, a
  numeric `value_min`/`value_max`/`value_now` range, and a `value`.
- optional non-topological `labels` and `describes` relationships.

`value` has one meaning across the library — what a control holds, how much a
container holds, the name of a state, or the reason a row was refused. The
cases are spelled out in `docs/components.md`; a component that publishes
`value` for anything else is a bug in that component, not a new case.

After retaining an arm guard, read a tree with `coordinator.snapshot(window_id)` and wait on
`coordinator.generation(window_id)`. The deterministic tree is not itself a
network server and is not the screen-reader transport. In this repository,
`headless-visual serve` is the debug-only session host: it serializes the
snapshot, injects input by semantic id, and captures the same offscreen frames
the visual gate uses. The stdio MCP `session_*` tools are a thin proxy over
that host. GPUI's AccessKit adapter owns the separate platform tree.
Applications may also read the snapshot in-process or in unit tests.

## Security

`snapshot()` redacts common API-key, bearer, JWT, password, and
secret-assignment shapes at the export boundary. `snapshot.redacted()` remains
available and idempotent for host-constructed snapshots.

Release applications should not expose input injection or automation servers
unless that is an explicit product feature with its own security review.

## Waiting

Automation waits for that window's `generation(window_id)` to advance after
input. Sleeping for a guessed delay does not prove the action produced a frame.
Animations and native composition may require an additional settle interval
before capture.
