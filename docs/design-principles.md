# Design principles

## Compact native desktop density

This is application chrome, not a marketing page. Controls are compact,
information remains scannable, and vertical rhythm favors work over spectacle.

## Surfaces establish hierarchy

Use canvas, panel, raised, and overlay surfaces before adding outlines.
Hairlines separate adjacent planes. Cards may use both a raised surface and a
low-alpha hairline when the boundary must remain legible.

“Borderless” does not mean “no hierarchy.” It means hierarchy should first come
from surface, spacing, grouping, and elevation.

## Accent has limited area

Accent marks primary actions, keyboard focus, links, and compact selection
chrome. It must not wash large application regions. Success, warning, danger,
and info are semantic states, not decorative alternatives.

## Geometry is semantic

- 5px: key caps and tiny controls.
- 8px: normal controls and menu rows.
- 12px: cards and popovers.
- 16px: dialogs and message bubbles.
- pill: badges and status dots.

Repeated semantic geometry is tokenized. One-off geometry may remain local.

## State is complete

Interactive components define default, hover, pressed, selected, disabled, and
focus behavior. Disabled means the action handler is absent, not merely faded.

Loading, empty, unavailable, error, and stale are visually and semantically
different.

## Motion supports continuity

Motion communicates where content came from and what changed. It never blocks
input or moves surrounding layout unexpectedly. Repeating animation uses fixed
slots so opacity and scale remain paint-local. GPUI reduced-motion behavior is
honored rather than reimplemented per component.

## Effects preserve structure

Frost and edge fades are structural paint effects:

- frost paints blur before the complete floating subtree in one layer;
- edge fade applies to primitives by distance to the scroll boundary;
- selected rings are inset shadows and consume no layout space;
- non-macOS platforms use an opaque fallback instead of exposing the desktop
  through unsupported transparency.

## Truth over optimistic appearance

The interface presents facts from the owning application layer. A click is not
success. A request failure is not empty data. The last verified value may remain
visible while refresh fails, but it must be marked stale.

## Testability is part of the component

A native window has no DOM. Semantic identity, role, measured bounds, focus,
disabled state, and selection are part of the user-facing interface. A control
that cannot be named and reached by the semantic tree is incomplete.
