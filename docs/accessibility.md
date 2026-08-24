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

Charts keep semantic values separate from animated geometry. Pointer movement
and Left/Right navigation move the crosshair by stable caller-owned series and
point ids, publish the current point immediately, and report exact
caller-formatted label and value text; neither coordinates nor displayed text
are interpolated into invented facts. Reduced motion settles both series and
crosshair geometry immediately. A failed refresh retains the last verified
series as stale data and exposes the caller's failure reason.

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
semantic id and GPUI element id are required to match. Installing the
diagnostic coordinator is not a condition of platform accessibility.

GPUI can forward that AccessKit tree to NSAccessibility on macOS, UI Automation
on Windows, AT-SPI on Linux, and an invisible semantic DOM mirror in browsers.
Linux compatibility and the remaining native adapter proofs are active work in
`docs/foundation-roadmap.md`; they are not complete or release-gating yet. The
rows below describe the current adapter paths. Deterministic tests
exercise the AccessKit tree, and the browser smoke exercises the DOM mirror's
roles, focus, actions, and canvas-scaled bounds. The macOS smoke check
queries each running gallery by PID with `AXUIElement` after confirming that the
invoking terminal has Accessibility permission. A platform adapter existing is
not evidence that a particular screen reader announces every property correctly.

| Capability | macOS AX | Windows UIA | Linux AT-SPI (planned) | Browser semantic DOM |
|---|---|---|---|---|
| Role and accessible name | Native AX smoke verified, including relationship-derived form names | Native Edit, Menu, and MenuItem role/name smoke verified, including relationship-derived form names | Adapter exists; non-gating and unverified | Button/dialog roles and names browser-smoke verified |
| Labelled-by and described-by | Same-window AccessKit relationships, including deferred overlays, verified deterministically; native name/help fallback verified | Same AccessKit structure and scalar fallback; native relationship-derived form name and complete description verified | Same AccessKit structure and scalar fallback; native AT-SPI session unverified | AccessKit structure exists; semantic DOM relationship projection unverified |
| String value and placeholder | Bridged; deterministic tree verified | Native ValuePattern read/write verified; placeholder remains deterministic only | Bridged; native session unverified | Mirrored; text editing browser-smoke verified |
| Disabled, invalid, required, busy | Disabled native AX smoke verified; other states deterministic only | Bridged; native session unverified | Bridged; native session unverified | Mirrored; screen-reader announcement unverified |
| Checked, expanded, widget selection | Checked native AX smoke verified; expanded/selection deterministic only | Bridged; native session unverified | Bridged; native session unverified | Mirrored; screen-reader announcement unverified |
| Focus | GPUI focus is projected and assistive-technology-requested focus has a native AX smoke check | Native UIA `SetFocus` verified: keyboard focus, global focused element, and one focus-changed event agree; Tab navigation is not claimed | GPUI focus action is routed to its owning handle; native AT-SPI session unverified | DOM focus action and ownership browser-smoke verified |
| Numeric min, max, current value | Bridged; deterministic tree verified | Bridged; native session unverified | Bridged; native session unverified | Mirrored; AT interaction unverified |
| Editable text | Native AX name/value/enabled/focus/editing plus range-dependent character geometry verified; grapheme-based TextRun structure verified deterministically | Native ValuePattern editing and distinct non-empty TextPattern character rectangles verified | Same AccessKit structure; native session unverified | Keyboard editing browser-smoke verified |
| Text selection and caret | Selection/caret structure and actions verified deterministically; native AX selected range and caret bounds verified, selection mutation unverified | Native TextPattern logical end caret verified; selection mutation remains unverified | Same AccessKit structure; native AT-SPI interaction unverified | Mirrored; browser AT mutation unverified |
| Live-region updates | Explicit polite/assertive atomic Toast create/update/removal verified deterministically; native `AXApplicationStatus` identity/action/removal verified, but VoiceOver speech/timing unverified; static Status is non-live | Same AccessKit structure; UIA announcement unverified | Same AccessKit structure; AT-SPI announcement unverified | ARIA live attributes mirrored; announcement unverified |
| GPUI overlays | Dialog/Menu/Tooltip/Status roles and lifetime native-smoke verified, including exact `AXDialog`, `AXMenu`/`AXMenuItem`, `AXUserInterfaceTooltip`, and `AXApplicationStatus` subroles where applicable; screen-reader ordering, navigation, and announcement remain unverified | Native Menu/MenuItem focus, Invoke action, and close lifetime verified; Dialog/Tooltip/Status sessions remain unverified | Same AccessKit structure; native AT-SPI session unverified | Dialog role and dismiss action browser-smoke verified |
| Bounds | Control and editable character/caret bounds native-smoke verified | Native editable character bounds verified; general control bounds remain deterministic only | Native adapter; editable range session unverified | Canvas-scaled DOM bounds browser-smoke verified |
| Native-child handoff | Not implemented | Not implemented | Not implemented | Not applicable |

`hovered` and pointer `pressed` remain diagnostic-only transient state. Semantic
`parent` records actual diagnostic tree parentage; `labels` and `describes`
record non-topological diagnostic relationships for tests and project to
AccessKit labelled-by and described-by when both role-bearing element ids
resolve uniquely in the active window. Resolution runs after ordinary and
deferred prepaint, supports either declaration direction and multiple
descriptions, and omits missing, duplicate, self-referential, or removed
endpoints rather than retaining stale node ids. The native tree still follows
GPUI's rendered element nesting; relationships do not reparent nodes. Literal
descriptions remain available and may coexist with a relationship. Form labels,
help/error text, hidden search labels, and visible deferred tooltips use the
relationship path.

Native adapters do not expose AccessKit references uniformly: macOS did not
derive an `AXTitle` or `AXHelp` from them, Windows derives a name from
`labelled_by` but does not map `described_by` as a UIA relation. After resolving
the references, GPUI therefore also fills an absent scalar label or description
from the related nodes' text. Explicit scalar values always win, the original
references remain present, multiple descriptions retain declaration order, and
missing or stale endpoints contribute nothing. The form native smoke verifies
the resulting complete field names and help/error descriptions on macOS.

Editable controls publish UTF-8 character lengths, generation-scoped text runs,
selection/caret positions, and `SetValue` and
`SetTextSelection` actions. AccessKit characters use the same Unicode grapheme
boundaries as editing, including combining sequences and emoji ZWJ sequences.
TextArea run boundaries come from its shaped visual rows, hard line breaks
remain in the preceding row, and line links never cross a shaped wrap or hard
break. `RichTextEditor` publishes the same contract over its flat platform
projection: one separator between caller-identified blocks, styled visual rows
from the geometry that painted them, and alignment-aware caret, range, and IME
bounds. Resolved Unicode bidi levels split mixed-direction text into truthful
logical-order runs; no whole-chunk first-strong direction is inferred.
`TextInput`, `TextArea`, and `RichTextEditor` capture the exact current-frame
shaped cells during child prepaint and publish physical per-grapheme positions,
widths, and run bounds through their semantic owner. They do not clone or
independently reshape text for accessibility; horizontal and vertical scroll,
wrapping, alignment, and segmented-input direction are the same geometry that
was painted. A stale source falls back to logical runs without stale bounds.
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
and layout revision. A document selection reuses those per-participant runs,
joins them only inside one declared selection scope, and never publishes text
from an unmounted or sensitive participant. Live regions are explicit opt-in:
static Status nodes are not live, ordinary toasts are polite, and danger toasts
are assertive, with the whole toast marked atomic. Announcement speech
and timing, native selection mutation, Linux editable-range verification, and
native child nodes remain separate platform work.

Open dialogs publish a modal native Dialog node, and open menus and visible
tooltips publish separate native Menu and Tooltip nodes; each role-bearing node
leaves the AccessKit tree when its deferred overlay closes. Focus containment
and restoration are component behavior, while platform screen-reader ordering
around deferred overlays remains unverified. Deferred hover-card and popover
surfaces are non-modal Groups and become siblings of their trigger group under
the native Window root. A tooltip may describe its trigger across that sibling
boundary without changing either node's parentage.
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

That command opens the button, input, form, dialog, menu, tooltip, and toast scenes and
queries their PID-scoped native trees with `AXUIElement`. It verifies roles,
names, enabled/disabled and checked state, AX-requested focus and editing, a
field's relationship-derived name and complete help/error description,
range-dependent editable character and caret bounds, selected range, a unique
named `AXDialog` with its initial focused action, active `AXMenuItem` movement
and `AXPress`, exact Tooltip/Status subroles, the trigger's literal `AXHelp`,
and transient dismissal/hide lifetime. It fails honestly when
Accessibility permission or an interactive user session is unavailable. The
dialog scene starts open and has no previously focused native trigger, so native
focus return is covered by the deterministic action tree rather than claimed by
this smoke check. Native text-selection mutation remains unverified, while the
real AccessKit action path is covered deterministically. VoiceOver speech,
navigation order, value/range adjustment, selection, and announcement timing
remain manual, unverified boundaries. The automated macOS, Windows, Linux, and
Web completion plan is in `docs/foundation-roadmap.md`.

On Windows, the same command runs in the repository's interactive
`windows-2025` job. It uses PID-scoped UI Automation to set and read an Edit
value, request focus, compare distinct character rectangles, verify the logical
end caret, read relationship-derived field names and complete descriptions,
and invoke the uniquely focused MenuItem before confirming that its Menu left
the native tree. It deliberately does not claim Narrator speech, selection
mutation, Tab order, Dialog/Tooltip/Status lifetime, or UIA announcement
events; those remain separate proofs.
