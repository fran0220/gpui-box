---
name: building-gpui-product-ui
description: "Builds or reviews native GPUI product interfaces using gpui-kit tokens, components, truthful async state, semantic automation, and window-level visual validation. Use when implementing GPUI views, extracting GPUI components, adding native UI states, or reviewing GPUI design-system compliance."
---

# Building GPUI product UI

Use this workflow for implementation and review. Do not begin by drawing the
requested screenshot. Establish state authority and interaction contracts
first.

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

Use `gpui-kit-theme::Theme`. Do not add literal palette colors in views.
Promote a value to the token document only when it is repeated and semantic;
one-off geometry may remain local.

Accent stays on compact actions and focus chrome. Large layout regions use
surface roles.

## 4. Reuse components

Search `gpui-kit` before creating a primitive. Keep one Button, Card, Popover,
Dialog, Settings rhythm, loader family, and status language per product.

Application-specific combinations remain in the application until at least two
real consumers demonstrate a stable product-neutral contract.

## 5. Preserve host/view separation

Views read view models and emit actions. They do not open files, launch
processes, query databases, call transports, or read credentials.

A click is not success. Render completion only after the owning layer projects
completion. Preserve exact refusals when they matter.

## 6. Implement behavior before polish

- disabled controls do not install handlers;
- all operations are keyboard reachable;
- popovers/dialogs handle initial focus, Escape, outside click, and focus
  restoration;
- long content has wrap, truncate, or scroll boundaries;
- blocking work stays off the GPUI window thread;
- stale async results cannot overwrite newer attempts.

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

Run the component gallery at a fixed viewport. Cover default, hover, pressed,
selected, disabled, focus, loading, empty, error, stale, popover, and dialog.

Wait for a real semantic generation and settle animated/composited frames.
Capture the product-owned window, not the desktop.

## 10. Final review

Use `references/review-checklist.md`. Report static checks, behavior tests,
visual checks, unsupported platforms, and any fixture-only evidence separately.
