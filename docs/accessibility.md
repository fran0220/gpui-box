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

The host owns focus transfer to and from native child views. The component
system does not claim that handoff until a platform bridge can identify and
verify the native child in the same accessibility hierarchy.

Focus is both painted and reported in the semantic tree. It is painted the same
way everywhere: one ring in `color.interactive.focus`, sized by
`effect.focusRingWidth` and `effect.focusRingAlpha`, applied through
`FocusRing::focus_ring`. It is drawn as a shadow, so showing it never moves
anything, and it is deliberately unlike the selected ring — focus says where
the next keystroke lands, selection says which answer is current.

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

`Semantic::semantic` and `Semantic::semantic_in` project the supported part of
each `NodeSpec` into GPUI's AccessKit tree as well as the deterministic semantic
registry. A stateful GPUI host keeps its existing click and accessibility action
handlers. A `Div` without an id becomes the stateful role-bearing host itself,
so its rendered semantic descendants remain its native descendants. The
semantic id and GPUI element id are required to match. Installing the test
registry is not a condition of platform accessibility.

GPUI forwards that AccessKit tree to NSAccessibility on macOS, UI Automation on
Windows, and AT-SPI on Linux. The macOS, Windows, and Linux rows below describe
the maintained fork's adapter path; only the deterministic AccessKit tree has
been exercised by this repository's deterministic test. The macOS smoke check
also queries the running gallery through System Events when the invoking
terminal has Accessibility permission. A platform adapter existing is not
evidence that a particular screen reader announces every property correctly.

| Capability | macOS AX | Windows UIA | Linux AT-SPI |
|---|---|---|---|
| Role and accessible name | Native AX smoke verified | Bridged; native session unverified | Bridged; native session unverified |
| String value and placeholder | Bridged; deterministic tree verified | Bridged; native session unverified | Bridged; native session unverified |
| Disabled, invalid, required, busy | Disabled native AX smoke verified; other states deterministic only | Bridged; native session unverified | Bridged; native session unverified |
| Checked, expanded, widget selection | Checked native AX smoke verified; expanded/selection deterministic only | Bridged; native session unverified | Bridged; native session unverified |
| Focus | GPUI focus is projected and assistive-technology-requested focus has a native AX smoke check | GPUI focus action is routed to its owning handle; native UIA re-verification pending | GPUI focus action is routed to its owning handle; native AT-SPI session unverified |
| Numeric min, max, current value | Bridged; deterministic tree verified | Bridged; native session unverified | Bridged; native session unverified |
| Editable text | Native AX name/value/enabled/focus/editing smoke verified; grapheme-based TextRun structure verified deterministically | Same AccessKit structure; native session unverified | Same AccessKit structure; native session unverified |
| Text selection and caret | Selection/caret structure and actions verified deterministically; native AX interaction unverified | Same AccessKit structure; native UIA interaction unverified | Same AccessKit structure; native AT-SPI interaction unverified |
| Live-region updates | Explicit polite/assertive atomic Toast create/update/removal verified deterministically; static Status is non-live; VoiceOver speech/timing unverified | Same AccessKit structure; UIA announcement unverified | Same AccessKit structure; AT-SPI announcement unverified |
| GPUI overlays | Nodes rendered in the overlay enter the GPUI tree; native announcement and ordering unverified | Same boundary; native session unverified | Same boundary; native session unverified |
| Native-child handoff | Not implemented | Not implemented | Not implemented |

`hovered` and pointer `pressed` remain diagnostic-only transient state. Semantic
`parent` and `labels` associations remain available to tests, while the native
tree follows GPUI's rendered element nesting; no cross-tree labelled-by or
reparenting claim is made. Editable controls publish UTF-8 character lengths,
stable text runs, selection/caret positions, and `SetValue` and
`SetTextSelection` actions. AccessKit characters use the same Unicode grapheme
boundaries as editing, including combining sequences and emoji ZWJ sequences.
Hard line breaks remain in the preceding run, links never cross a hard line,
and each run reports its first strong bidirectional direction with the current
layout direction as fallback. IME offsets are converted between UTF-8 and
UTF-16, including surrogate pairs. Disabled fields advertise no native focus,
mutation, or selection actions; read-only fields remain focusable/selectable
but non-mutating. Password fields use the native password role but publish
neither plaintext nor text runs. Live regions are explicit opt-in: static
Status nodes are not live, ordinary toasts are polite, and danger toasts are
assertive, with the whole toast marked atomic. Announcement speech and timing,
shaped per-grapheme positions/widths and native caret geometry, and native child
nodes remain separate platform work. AccessKit defines character geometry as
optional; gpui-kit omits it rather than inventing measurements that are not yet
available to the accessibility subtree callback.

The deterministic smoke test activates GPUI's test accessibility adapter,
renders a real element tree, and asserts role, name, value, range, control
states, selection, focus, and an inherited click action in
`gpui-kit-semantics`. On macOS, run:

```bash
cargo run -p xtask -- accessibility check
```

That command opens the button and input scenes and asks System Events for AX
roles, names, enabled/disabled state, checked state, AX-requested focus, and an
editable value change. It fails honestly when Accessibility permission or an
interactive user session is unavailable. Native text-selection mutation is not
claimed: System Events did not provide a reliable selected-range round trip on
this runner, while the real AccessKit action path is covered deterministically.
VoiceOver speech, focus navigation order, value/range adjustment, selection,
and announcement timing remain manual, unverified boundaries.
