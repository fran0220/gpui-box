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
that identifies the action. `Select` and editable `Combobox` names are explicit
caller-owned inputs: neither a selected answer nor a placeholder is treated as
the control's name. A combobox propagates that name to its nested editable query
target so the shared focus path contains no unnamed competing control.

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

GPUI can forward that AccessKit tree to NSAccessibility on macOS, UI Automation
on Windows, AT-SPI on Linux, and an invisible semantic DOM mirror in browsers.
Linux compatibility is currently deferred: the adapter path is recorded below
for future roadmap work, but is not tested, supported, or release-gating. The
rows below describe the maintained fork's adapter paths. Deterministic tests
exercise the AccessKit tree, and the browser smoke exercises the DOM mirror's
roles, focus, actions, and canvas-scaled bounds. The macOS smoke check
queries each running gallery by PID with `AXUIElement` after confirming that the
invoking terminal has Accessibility permission. A platform adapter existing is
not evidence that a particular screen reader announces every property correctly.

| Capability | macOS AX | Windows UIA | Linux AT-SPI (deferred) | Browser semantic DOM |
|---|---|---|---|---|
| Role and accessible name | Native AX smoke verified | Bridged; native session unverified | Adapter exists; non-gating and unverified | Button/dialog roles and names browser-smoke verified |
| String value and placeholder | Bridged; deterministic tree verified | Bridged; native session unverified | Bridged; native session unverified | Mirrored; text editing browser-smoke verified |
| Disabled, invalid, required, busy | Disabled native AX smoke verified; other states deterministic only | Bridged; native session unverified | Bridged; native session unverified | Mirrored; screen-reader announcement unverified |
| Checked, expanded, widget selection | Checked native AX smoke verified; expanded/selection deterministic only | Bridged; native session unverified | Bridged; native session unverified | Mirrored; screen-reader announcement unverified |
| Focus | GPUI focus is projected and assistive-technology-requested focus has a native AX smoke check | Native UIA `SetFocus` verified: keyboard focus, global focused element, and one focus-changed event agree; Tab navigation is not claimed | GPUI focus action is routed to its owning handle; native AT-SPI session unverified | DOM focus action and ownership browser-smoke verified |
| Numeric min, max, current value | Bridged; deterministic tree verified | Bridged; native session unverified | Bridged; native session unverified | Mirrored; AT interaction unverified |
| Editable text | Native AX name/value/enabled/focus/editing smoke verified; grapheme-based TextRun structure verified deterministically | Same AccessKit structure; native session unverified | Same AccessKit structure; native session unverified | Keyboard editing browser-smoke verified |
| Text selection and caret | Selection/caret structure and actions verified deterministically; native AX interaction unverified | Same AccessKit structure; native UIA interaction unverified | Same AccessKit structure; native AT-SPI interaction unverified | Mirrored; browser AT mutation unverified |
| Live-region updates | Explicit polite/assertive atomic Toast create/update/removal verified deterministically; native `AXApplicationStatus` identity/action/removal verified, but VoiceOver speech/timing unverified; static Status is non-live | Same AccessKit structure; UIA announcement unverified | Same AccessKit structure; AT-SPI announcement unverified | ARIA live attributes mirrored; announcement unverified |
| GPUI overlays | Dialog/Menu/Tooltip/Status roles and lifetime native-smoke verified, including exact `AXDialog`, `AXMenu`/`AXMenuItem`, `AXUserInterfaceTooltip`, and `AXApplicationStatus` subroles where applicable; screen-reader ordering, navigation, and announcement remain unverified | Same AccessKit structure; native UIA revalidation pending | Same AccessKit structure; native AT-SPI session unverified | Dialog role and dismiss action browser-smoke verified |
| Bounds | Native adapter | Native adapter | Native adapter | Canvas-scaled DOM bounds browser-smoke verified |
| Native-child handoff | Not implemented | Not implemented | Not implemented | Not applicable |

`hovered` and pointer `pressed` remain diagnostic-only transient state. Semantic
`parent` records actual diagnostic tree parentage; `labels` and `describes`
record non-topological diagnostic relationships for tests. The native tree
follows GPUI's rendered element nesting, so no cross-tree labelled-by,
described-by, or reparenting claim is made. Literal descriptions can be
published on a native role-bearing node, but GPUI does not yet expose a native
cross-tree described-by relationship. Tooltip triggers therefore opt into the
same literal description as their visible tooltip instead of claiming that
native relationship.

Editable controls publish UTF-8 character lengths, generation-scoped text runs,
selection/caret positions, and `SetValue` and
`SetTextSelection` actions. AccessKit characters use the same Unicode grapheme
boundaries as editing, including combining sequences and emoji ZWJ sequences.
TextArea run boundaries come from its shaped visual rows, hard line breaks
remain in the preceding row, and line links never cross a shaped wrap or hard
break. Resolved Unicode bidi levels split mixed-direction text into truthful
logical-order runs; no whole-chunk first-strong direction is inferred.
Kit type styles bundle and explicitly select Arabic and Hebrew fallback faces,
so those logical runs remain visible even in deterministic renderers with no
system font database. A final
line break publishes a distinct empty-line position. IME offsets are converted
between UTF-8 and UTF-16, including surrogate pairs. Accessibility selection
requests carry revision-bearing run ids and the exact stored source revision,
so a request from a stale tree cannot be applied to changed text. Disabled fields clear
focus and composition/drag transients, install no input handler, and advertise
no native focus, mutation, or selection actions; read-only fields remain
focusable/selectable but non-mutating. Password fields use the native password
role but publish neither plaintext nor text runs. Read-only selectable
`StyledText` uses the same grapheme and bidi run contract and additionally
publishes word starts plus shaped per-grapheme positions, widths, and run
bounds; `SetTextSelection` is accepted only against the exact published text
and layout revision. Live regions are explicit
opt-in: static Status nodes are not live, ordinary toasts are polite, and danger
toasts are assertive, with the whole toast marked atomic. Announcement speech
and timing, editable per-grapheme geometry and native caret geometry, and native
child nodes remain separate platform work.

Open dialogs publish a modal native Dialog node, and open menus and visible
tooltips publish separate native Menu and Tooltip nodes; each role-bearing node
leaves the AccessKit tree when its deferred overlay closes. Focus containment
and restoration are component behavior, while platform screen-reader ordering
around deferred overlays remains unverified. Deferred hover-card and popover
surfaces are non-modal Groups and become siblings of their trigger group under
the native Window root; no unsupported cross-deferred relationship is claimed.
Menu keyboard focus remains on the Menu container, while GPUI's active-descendant
bridge reports the current stable MenuItem as AccessKit focus only while that
container owns keyboard focus. Moving the menu cursor updates the reported
descendant without inventing selection state.

The deterministic smoke test activates GPUI's test accessibility adapter,
renders a real element tree, and asserts role, name, value, range, control
states, selection, focus, and an inherited click action in
`gpui-box-kit-semantics`. On macOS, run:

```bash
cargo run -p xtask -- accessibility check
```

That command opens the button, input, dialog, menu, tooltip, and toast scenes and
queries their PID-scoped native trees with `AXUIElement`. It verifies roles,
names, enabled/disabled and checked state, AX-requested focus and editing, a
unique named `AXDialog` with its initial focused action, active `AXMenuItem`
movement and `AXPress`, exact Tooltip/Status subroles, the trigger's literal
`AXHelp`, and transient dismissal/hide lifetime. It fails honestly when
Accessibility permission or an interactive user session is unavailable. The
dialog scene starts open and has no previously focused native trigger, so native
focus return is covered by the deterministic action tree rather than claimed by
this smoke check. Native text-selection mutation remains unverified, while the
real AccessKit action path is covered deterministically. VoiceOver speech,
navigation order, value/range adjustment, selection, and announcement timing
remain manual, unverified boundaries.
