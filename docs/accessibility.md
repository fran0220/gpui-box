# Accessibility

## Keyboard

Every pointer action has a keyboard path. Menus support Up, Down, Enter, and
Escape. Modified Enter is modeled separately when an application assigns it a
distinct action.

## Focus

Interactive elements use a stable focus handle owned by the view. Dialogs and
popovers:

1. move focus to the first meaningful control;
2. contain navigation while modal;
3. close on Escape;
4. restore focus to the trigger;
5. hand focus predictably to and from native child views.

Focus is both painted and reported in the semantic tree.

## Roles and names

Every action and assertion target has a role and stable id. Visible labels
provide names wherever possible. Icon-only actions need nearby or semantic text
that identifies the action.

## Contrast

Primary text, muted text, faint text, status colors, and text-on-accent are
separate roles. Do not use faint text for required instructions. Accent fill
uses `text_on_accent`, not ordinary body text.

## Motion

GPUI's reduced-motion flag controls `with_animation`. Do not create wall-clock
animation loops that ignore it. Essential state remains understandable when
entrance and repeating animations snap to rest states.

## Content boundaries

Long paths and external content must not cover controls or escape the viewport.
Use wrapping, truncation, scrolling, and accessible full-value affordances
according to the content's purpose.

## Platform claims

Semantic automation is not a screen reader implementation. Applications must
also use GPUI's native accessibility APIs for user-facing assistive technology.
The semantic tree exists for testing and diagnostics and should align with,
not replace, the product accessibility tree.
