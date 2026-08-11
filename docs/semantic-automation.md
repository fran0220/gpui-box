# Semantic automation

GPUI renders native windows without a browser DOM. `gpui-box-kit-semantics`
provides a small, transport-independent semantic tree.

## Registration

```rust
use gpui_kit_semantics::{NodeSpec, Role, Semantic};

button.semantic(
    &registry,
    NodeSpec::new("settings.account.sign-out", Role::Button)
        .text("Sign out")
        .disabled(saving),
)
```

The probe stretches over its parent and records bounds during prepaint. It
paints nothing and does not consume input. The same call also projects the
supported role, name, value, control state, focus, range, and widget selection
into GPUI's AccessKit tree. See [Accessibility](accessibility.md) for the exact
platform capability matrix and unsupported boundaries.

## Frame lifecycle

Call `registry.begin_frame()` before rendering a product frame. Nodes
registered during that frame form the next snapshot. A node not rendered in
the next frame disappears.

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

`value` has one meaning across the library — what a control holds, how much a
container holds, the name of a state, or the reason a row was refused. The
cases are spelled out in `docs/components.md`; a component that publishes
`value` for anything else is a bug in that component, not a new case.

The deterministic tree does not define a network server and is not itself the
screen-reader transport. Applications may serialize the snapshot through a
debug-only transport, read it in-process, or use it directly in unit tests.
GPUI's AccessKit adapter owns the separate platform tree.

## Security

Call `snapshot.redacted()` before exporting diagnostic data. It removes common
API-key, bearer, JWT, password, and secret-assignment shapes.

Release applications should not expose input injection or automation servers
unless that is an explicit product feature with its own security review.

## Waiting

Automation waits for `generation()` to advance after input. Sleeping for a
guessed delay does not prove the action produced a frame. Animations and native
composition may require an additional settle interval before capture.
