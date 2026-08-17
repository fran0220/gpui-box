---
name: building-gpui-product-ui
description: Builds or reviews native GPUI product interfaces from the current GPUI Box checkout catalog, not the published crates.io cohort. Use when implementing GPUI views, extracting components, adding native UI states, reviewing GPUI Box compliance, or when Kit tokens, scenes, semantic ids, or gpui-box MCP are mentioned.
---

# Building GPUI product UI

Use this workflow for implementation and review. Do not begin by drawing the
requested screenshot. Establish state authority and interaction contracts
first.

Cargo package `gpui-box` imports as `gpui`; `gpui-box-kit` imports as
`gpui_kit`. Never mix Zed GPUI packages into the graph.

## 0. Read this repository's catalog

The catalog is this tree, not crates.io `gpui-box-mcp` `0.1.1`. Use the MCP
tools — stdio against the checkout (`tools/mcp/run.sh`) for work in progress,
or the hosted endpoint after it is deployed from this tree.

Before writing or reviewing a component, call these tools — do not recall
signatures:

1. `rules` — gate conventions from this tree
2. `search_components` — components and supporting types (`CardHeader`,
   `CardVariant`, `AsyncValue`). `kind=type` lists only types
3. `component` — exact constructors, options, commands, queries, reports,
   variants
4. `scene` — compiling example from this tree
5. `render_scene` — stdio draws the gallery now; hosted returns the last
   committed capture

Do not treat a crates.io install as the API.

## 1. Read the local boundary

Read the nearest repository instructions and identify:

- the crate that owns product facts;
- the application action boundary;
- the current theme and component source;
- the validation commands;
- source and asset provenance requirements.

Do not move product authority into a component crate.

## 2. Enumerate real states

Before rendering, list:

- initial/idle;
- loading;
- ready;
- successful empty;
- unavailable;
- refusal/error;
- refresh with existing data;
- disabled reason;
- selected/focused/hovered/pressed;
- long-content and narrow-window behavior.

Use `Loadable` or `AsyncValue` rather than booleans that collapse states.

## 3. Select semantic tokens

Use `gpui_kit_theme::Theme` from Cargo package `gpui-box-kit-theme`. Do not add
literal palette colors in views.
Promote a value to the token document only when it is repeated and semantic;
one-off geometry may remain local.

Accent stays on compact actions and focus chrome. Large layout regions use
surface roles. Theme documents must keep canvas/panel/card/raised/overlay
separable in CIE L\*, and keep `muted` / `faint` / `placeholder` / `disabled`
distinct from the canvas.

## 4. Reuse components

Search the checkout catalog before creating a primitive. Keep one Button, Card,
Popover, Dialog, Settings rhythm, loader family, and status language per
product.

See [references/component-selection.md](references/component-selection.md).
Application-specific combinations remain in the application until at least two
real consumers demonstrate a stable product-neutral contract.

Library-authored visible text uses `gpui_kit::strings::StringKey`. Caller text
stays caller-owned.

## 5. Preserve host/view separation

Views read view models and emit actions. They do not open files, launch
processes, query databases, call transports, or read credentials.

A click is not success. Render completion only after the owning layer projects
completion. Preserve exact refusals when they matter.

## 6. Implement behavior before polish

- disabled or loading controls do not install their action handler;
- all operations are keyboard reachable;
- popovers/dialogs handle initial focus, Escape, outside click, and focus
  restoration;
- long content has wrap, truncate, or scroll boundaries;
- blocking work stays off the GPUI window thread;
- stale async results cannot overwrite newer attempts;
- `Slotted` replacements use a name from `SLOTS` — an unknown name panics;
- `Responsive` reports `Measured` or honestly `Unmeasured`, never a guess.

## 7. Register semantics

Attach stable semantic IDs to every action and assertion target. IDs derive
from capability and business identity, never list position.

Report role, text, measured bounds, visibility, focus, disabled, selected,
hovered, and pressed where applicable. Redact snapshots before export.

## 8. Test behavior

Test pure reducers and state transitions directly. Test actions by observing
the request sent, value returned, or artifact written. Never grep source text
as a behavioral assertion.

Break a new test's implementation once and confirm the test fails before
trusting it.

## 9. Validate visuals

Iterate with the scenes the component actually touches:

```bash
cargo run -p xtask -- gate only <scene>
cargo run -p xtask -- headless check <scene>
```

Cover default, hover, pressed, selected, disabled, focus, loading, empty,
error, stale, popover, and dialog. Inspect every changed PNG before
`headless capture`. A hosted or crates.io snapshot is not this tree.

A repeating animation is held at its first frame and a one-shot at its last.
Review motion in the gallery window, not in a still.

## 10. Final review

Use [references/review-checklist.md](references/review-checklist.md). Report
static checks, behavior tests, visual checks, unsupported platforms, and any
fixture-only evidence separately.

## Additional resources

- [references/component-selection.md](references/component-selection.md)
- [references/truthful-ui.md](references/truthful-ui.md)
- [references/review-checklist.md](references/review-checklist.md)
