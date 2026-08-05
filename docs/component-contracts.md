# Component contracts

## General contract

Every component:

1. consumes `gpui_kit_theme::Theme`;
2. accepts display data from the caller;
3. emits an application-owned callback;
4. owns only visual transient state;
5. remains independent of host services and product models;
6. exposes a stable semantic target when it is actionable or assertable.

## Buttons

`action_button` supports Primary, Ghost, and Danger variants and Small or
Medium density. When disabled it does not register the click listener.

Primary is reserved for the main action in a local decision area. Danger is
only for destructive or irreversible intent.

## Badge and status

Badges are compact labels. Status dots and callouts carry semantic state.
Neutral is the default; Accent, Success, Warning, Danger, and Info must match
the actual state.

Do not use Success to make an idle state look healthy.

## Cards and settings

`section_card` groups related rows. `card_row` supplies spacing, separators, and
the quiet hover wash. Identity tiles are optional. Row content must preserve
the title when detail text is long.

Settings pages use:

- a centered 768px maximum column;
- 24px horizontal inset;
- 32px top rhythm;
- title, subtitle, section title, card, and optional footnote.

Product-specific settings models stay in the application.

## Popovers and dialogs

Floating layers:

- are deferred above normal content;
- occlude hit testing beneath them;
- snap to the viewport with margin;
- use one frost layer around the complete subtree;
- choose above or below anchoring based on trigger location;
- restore focus and close on Escape in the owning view.

The library supplies geometry and reducers. The owning view supplies open
state, focus handles, keyboard dispatch, dismissal, and actions.

## Loading

Skeleton and spinner slots have fixed layout dimensions. Repeating animation
changes opacity or paint-local size rather than reflowing siblings.

Use:

- skeletons when the eventual content has known row geometry;
- pulse loader for a neutral wait;
- gradient spinner for compact active work.

## Effects

`frosted` and `edge_faded` depend on the pinned public GPUI fork. They must wrap
the complete affected subtree. Applying blur separately to each row creates
unstable paint ordering and is not supported.

## Long content

External paths, messages, command descriptions, and user content require an
explicit boundary:

- truncate for identity lines;
- wrap for explanatory copy;
- scroll for large output;
- never allow long content to move the primary action outside the viewport.
