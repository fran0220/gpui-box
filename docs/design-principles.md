# Design principles

## Compact native desktop density

This is application chrome, not a marketing page. Controls are compact,
information remains scannable, and vertical rhythm favors work over spectacle.

## Surfaces establish hierarchy

Use backdrop, canvas, panel, raised, and overlay surfaces before adding outlines.
A card, a popover, a dialog, and a menu are told apart from what is behind them
by a colour step and an elevation, not by a line drawn around them.

“Borderless” does not mean “no hierarchy.” It means hierarchy should first come
from surface, spacing, grouping, and elevation.

## A line says something a surface cannot

Every line in the library is one of three things, and a component that draws a
fourth is drawing decoration:

- a **rule**, which divides content sharing one surface. It is
  [`foundation::rule`](../crates/gpui-kit/src/foundation/styled_ext.rs), a child
  element rather than a border, so it can be inset and so a component that
  spends its border on focus can still draw one. It is painted in
  `interactive.divider`;
- a **boundary a pointer acts on** — a slider rail, a switch edge, a scrollbar
  gutter, a resize seam. These are `interactive.track` and
  `interactive.hairlineStrong`, and they carry the 3:1 non-text contrast the
  guidelines ask of a control boundary;
- a **report** — focus, invalidity, a drop target, a refusal. These are borders,
  in the colour of the thing being reported, and they are the reason a resting
  control keeps a transparent border of the same width: becoming invalid must
  not reflow the row.

A rule and a divider are decorative and deliberately do **not** carry 3:1. A
theme whose hairline clears 3:1 against every surface has drawn an outline
around every card, table, and menu in the library. They instead carry a
documented floor: composited over each surface, a line must move it by at least
1.5 L\*, which is what `contrast::line_report` checks and the token gate
enforces.

## Selection is a wash and a rail

Every collection in the library says which row it is on the same way: a neutral
wash from `interactive.selected` carries the row, and an accent rail
`effect.selectionRailWidth` wide sits at the reading edge. Neither consumes
layout, so arriving on a row moves nothing. The recipes are
`SelectedRow::selected_row` and `selected_column`.

The wash alone cannot be pushed hard enough in a light theme to read as *chosen*
without also reading as *inactive*, and a whole row of accent would spend on one
line the area this library reserves for the decision a surface is asking for.
Where a row is not a row — a node on a canvas, a chip, a segment of a strip —
selection is the accent in the foreground or all the way round the object, never
a neutral outline a reader has to hunt for.

## Accent has limited area

Accent marks primary actions, keyboard focus, links, and compact selection
chrome. It must not wash large application regions. Success, warning, danger,
and info are semantic states, not decorative alternatives.

## Geometry is semantic

- Window planes, edge-attached regions, and rows inside a collection stay
  square. They are part of the surface behind them rather than detached
  entities sitting on it.
- 5px: key caps and tiny controls.
- 8px: normal controls and menu rows.
- 12px: cards and popovers.
- 16px: dialogs and message bubbles.
- pill: badges and status dots.

Repeated semantic geometry is tokenized, and the outer entity consumes the
role rather than reconstructing it from a raw value. One-off geometry may
remain local. Flat and rounded therefore describe attachment, not two visual
styles: a sidebar is flat because it is a window plane; a popover is rounded
because it is a detached entity.

## Production style has one authority

A component does not invent anonymous spacing, corner radius, type size,
weight, theme-colour alpha, or reusable measure. It consumes the theme's typed
tokens or a complete shared recipe. This applies to small values too: 2px is a
spacing step when it separates content, not an exception because it is small;
360px is a measure when several explanatory surfaces share it, not four local
widths that happen to agree.

Local values remain valid where tokenization would misstate ownership: chart
topology, normalized data encoding, asset proportions, hit testing, algorithmic
physics, and one component's layout geometry. Such values are named beside the
algorithm they explain. Mathematical zero and one remain local endpoints.
This boundary keeps themes fully retunable without turning the token document
into a collection of arbitrary coordinates.

The token gate parses production Rust and enforces this distinction. It skips
scenes and test fixtures, so examples can describe their own canvas while
shipping components cannot quietly create a second style system.

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
- a scroll shadow is a gradient band of `backdrop`, not a line: content that
  continues past an edge is a soft fact, and a hard rule there reads as a
  boundary that has been reached;
- selection washes and rails are painted inside and consume no layout space;
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
