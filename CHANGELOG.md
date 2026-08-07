# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Entries say what the library now does and what it refuses to do, because a
refusal is the part a caller has to plan around.

Nothing here has been released. There is no tag and no registry entry, and
what a consumer pins is a commit; see "Versioning and compatibility" in
`README.md` for what that means for a dependency. The `0.1.0` in `Cargo.toml`
is what Cargo requires a manifest to carry, not a version anybody can take.

## Unreleased

### Added

**Tokens and theme.** `tokens/studio-dark.json` and `tokens/studio-light.json`
are the source of truth, expressed as a palette plus references so retuning a
scale is one edit rather than one per role. Themes carry elevation, z-index and
density axes, spring presets and the six easing curves, and are built through
`Theme::from_tokens` at a density that scales spacing, control geometry and type
onto the pixel grid while leaving colour and radius alone. `ThemeRegistry` lets
an application register its own document and switch theme or density at runtime;
an unknown id returns false and leaves the active theme where it was rather than
falling back to a default nobody asked for. `xtask tokens check` verifies WCAG
contrast for every theme and fails if the generated reference has drifted.

**Controls.** `TextInput` and `TextArea` with grapheme-aware caret motion, word
motions, selection by keyboard and pointer, cut/copy/paste and input-method
composition; a secret renders as dots, stays out of the clipboard, and publishes
only that something was typed. `Checkbox` with a real mixed state, `Radio`,
`Switch`, `Slider`, `Select`, `Combobox`, `NumberInput`, `TagInput`,
`SegmentedControl`, `FormField`, `InlineEdit`, `SettingsRow` and
`SettingsSection`, `Button`, `IconButton`, `ButtonGroup`, `SplitButton`,
`KeybindingRecorder`, `FilterBar`, `Dropzone`. Every one of them reports an
intent and applies nothing: a value, a selection and a choice belong to the
host, so a refused change is visible as the control not moving. A disabled or
refused control installs no handler at all, which is why disabled is behaviour
rather than opacity.

**Overlays.** `Overlay` places a surface below, above, at a point or centered,
taking its paint order from the z-index tokens; `FocusTrap` cycles only the
stops the open frame registered and restores focus on close. `Dialog`, `Drawer`,
`Popover`, `Menu`, `ContextMenu`, `CommandPalette`, `Tooltip`, `Kbd`, and
`Toast` over a host-mounted `ToastLayer`. A dialog that cannot be dismissed
installs no dismissal, so escape and the scrim cannot close a decision the host
requires. A danger or warning toast never leaves on a timer, a pointer resting
on the stack pauses the countdown, and a push that went nowhere says so instead
of being dropped. A palette that nothing answered says so rather than showing an
empty list, and a refused command keeps its reason instead of disappearing.

**Data.** `List` over GPUI's uniform list, publishing its total so a test can
tell a thousand items from the twelve that were drawn; `Table`, whose header
click reports the sort it implies and sorts nothing; `Tree`, which renders none
of a collapsed node's children; and `DataGrid`, which virtualizes rows, reserves
slots beneath an opened row, and reports every sort, width, column order,
selection, expansion and edit without applying any of them. The grid keeps the
two select-all claims apart, because selecting the forty rows that are loaded
and selecting all twelve thousand are different promises. A fit-to-content
request asks the host, since a grid can only measure the rows it drew.

**Navigation and layout.** `Tabs`, `Accordion`, `Breadcrumb`, `Sidebar`,
`Pagination`, `Wizard`, `SplitPane`, `SplitTree`, `ScrollArea`, `Toolbar`,
`Dock` and `StatusBar`. `Pagination` can say there is another page without
inventing a total. A collapsed sidebar still publishes every item's full name.
An item past a toolbar's declared cut moves into the overflow keeping its
identity and its refusal, never dropped. `SplitTree` propagates minimums up the
tree so a divider stops where a leaf two branches below would starve, and
converts to and from plain records so a host can persist a layout without this
crate taking a serialization dependency. `Dock` builds on that same tree and the
same drag system, so there is one resize implementation and one drag
implementation rather than three.

**Date and time.** `Calendar`, `DateInput`, `RangePicker` and `TimeInput` over a
host-implemented `DateAdapter`. This crate owns no calendar, no time zone and no
locale, and holds no month or weekday name in any language: `Day` and `MonthKey`
are opaque integers the host mints, and moving a month is an adapter call rather
than an addition. An adapter is allowed to answer "I don't know", and each of
those has a rendered consequence instead of a guess — no today means no ring and
no guessed month, a refused `shift_month` stops navigation on every route into
the month beyond it, and a `days_in` of `None` makes a range say its days could
not be checked rather than that it is clear. The reference calendar is behind
the `fixtures` feature, off by default.

**Content.** `Markdown`, `MessageList`, `ImageViewer` and `TransportBar` — the
surfaces that draw text and media nobody in the application wrote, and therefore
the ones that act on none of it. Raw HTML is drawn as the characters somebody
typed and marked unrendered, because interpreting it lets a document reach
outside its own text and dropping it lets a document delete itself from the
reader's view; `pulldown-cmark` is compiled with its html feature off, so there
is no renderer to reach for. A link states where it goes and opens nothing, an
image is named rather than fetched, and a code fence is coloured only by spans
the host computed. `MessageList` keeps five delivery states, because collapsing
sent, delivered and read into one tick says less than the host knows and folding
a failure into any of them says something untrue. `ImageViewer` decodes nothing
and reports an unknown source size as unknown; `TransportBar` reports play,
pause, seek, volume, speed and a track step and applies none of them.

**Motion.** Springs solved in closed form with a bounded settle time,
`Interpolate` for `f32`, `Pixels`, `Rems`, `Hsla`, `Point` and `Size`,
`Transition` retargeting from the value on screen, `Presence` keeping an element
alive for its exit, `Stagger` over a fixed group window so a fifty-row menu
opens as fast as a five-row one, `Flip` and `flip_size` so a moved element
slides without disturbing its neighbours, `Keyframes` for a path that is not a
straight line between two ends, `AnimatedNumber` that publishes the total it is
counting to rather than the frame it is on, and the press and hover responses
every actionable control wears — withheld from everything that is not
actionable, so a response never promises an action that does not exist. Gesture
motion arrived with them: a velocity tracker measured over a trailing window and
sampled against the clock at the moment it is asked, so a drag the user parked
before releasing reports no velocity and flings nothing; `flick`, `rubber_band`,
and `ScrollLink`, which reads an offset as a progress, holds no state, and never
consults reduced motion on its own because only the caller knows whether the
motion answers the user's own hand. Everything honours `App::reduce_motion` by
settling in one frame.

**Drag and drop.** One payload and one vocabulary for where a drop lands: before
or after a named anchor, or into one that can contain it, never an index,
because an index stops meaning anything the moment the host applies the move.
The drag reports an intent and moves nothing; the row changes place when the
host hands back the new order. It publishes what is held and where it would land
while it is in flight, so a test reads a drag from an ordinary snapshot. An item
onto itself and a node into its own subtree are refused as impossible; every
other refusal belongs to the caller, and a zone that cannot take what is over it
never looks like a zone that is merely empty. `List`, `Tree`, `Tabs`,
`Dropzone`, `DataGrid` column headers and `Dock` panel headers all use it.

**Semantics and testing.** A per-frame semantic tree measured during prepaint,
with roles, state and bounds, and ids that come from business identity rather
than list position. `gpui_kit_testkit::audit` rejects positional, empty and
duplicate ids, unnamed actionable roles, values outside their own reported
range, text that survived redaction, and visible nodes with no size. A test
harness drives simulated keys, pointer, drags and frames against a simulated
clock. `gpui_kit::scenes` is one canonical rendering per component, shared by
the gallery, the capture task and the headless audit, so a component cannot be
reviewed visually in one arrangement and tested in another. In-process window
capture asks the window server for the process's own window and never the
desktop.

**Tooling.** `xtask tokens generate|check`, `xtask scenes list|capture|check`
and `xtask gate [full]`. `scenes check` is the visual regression gate; see
`docs/screenshot-testing.md` for where it can and cannot run.

### Changed

- Components are `RenderOnce` builders that read the theme from the application
  context and derive their element and semantic id from one `Ident`, replacing
  free functions that took a `&Theme` and positional flags. This was a breaking
  rewrite of every call site, made before anything depended on the old shape.
- A consistency pass settled the details a caller notices first: focus is drawn
  one way everywhere, from its own tokens, as an outset ring that cannot be
  mistaken for the inset selection ring and that never moves what it marks;
  `Slider`, `Accordion` and `Sidebar` honour the size they are given; bounded
  stepping has one implementation; and `FieldFrame` and `SearchFrame`, which the
  coverage table claimed and nothing used, were deleted.
- Capturing the scene catalog runs in one process on one window instead of one
  process per image, which took over twenty minutes. Captures became
  reproducible at the same time: reduced motion holds an animation at a defined
  frame, the tracked pointer is parked so a row is not captured hovered because
  of where the operator left the mouse, and the run waits for the window server
  to settle on a frame that is both stable and new instead of sleeping. Because
  the bytes are reproducible, `scenes check` can assert them.

### Fixed

- `scenes check` reported that 112 images matched after writing one of them: a
  run that failed part way still exited zero, and the check counted the images
  it had asked for rather than the ones that arrived. A gate that passes when it
  did not look is worse than no gate, so a capture now names every image it owed
  and fails on the ones that never arrived.
- A capture read the previous scene until it gave up, because a window nobody is
  compositing keeps handing back the frame it drew last. The application claims
  the foreground for the run, and reclaims it in the branch that sees an
  unchanged frame, since anything that takes the foreground part way through
  causes exactly that symptom and no other.
- `Presence` reversed a phase by scaling elapsed time, which is exact only when
  both curves are linear; on an ease-in-out an entrance cancelled at 60ms jumped
  from 0.187 to 0.071 and the element blinked. Progress is now inverted on the
  same clock, so a phase resumes from the opacity that was actually on screen.
- The semantic registry reported the previous frame when a frame published
  nothing, which let a test assert an element had disappeared against a stale
  snapshot.
- Semantic ids forced relative positioning, so any absolutely positioned element
  that also carried one silently collapsed to nothing.

### Not provided

`docs/coverage.md` states what this library refuses to build and why — the
calendar, time formatting, rich-text and code editing, charts, platform pickers,
window chrome — and, separately, what is missing but intended. Read it before
planning around a component that is not here.
