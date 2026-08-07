# Motion

Motion in this library is layered, and every layer is pure enough to test
without a window.

| Layer | Type | Purpose |
|---|---|---|
| Curve | `CubicBezier`, `Easing` | Names a shape; `Easing` resolves against the theme. |
| Physics | `Spring` | Closed-form damped spring from stiffness, damping and mass. |
| Specification | `MotionSpec` | A curve — or a spring, via `MotionSpec::sprung` — plus duration and delay. |
| Value | `Interpolate` | Moves `f32`, `Pixels`, `Rems`, `Hsla`, `Point` and `Size`, and measures how far apart two of them are. |
| Path | `Keyframes` | Takes a value through named stops rather than straight across. |
| State | `Transition` | Animates a value whose target can change mid-flight, carrying the speed it already had. |
| Lifecycle | `Presence` | Keeps an element alive long enough to animate out. |
| Group | `Stagger` | Spreads one specification across a list. |
| Position | `Flipping::flip` | Slides an element from where it was to where it is. |
| Rectangle | `Flipping::flip_size` | The same, and resizes it too — which, unlike the slide, is a real layout change. |
| Pointer | `Pressable`, `HoverLift` | The two responses a control gives a pointer. |
| Value | `AnimatedNumber` | Counts to a new number while publishing the target. |

## What moves, where

Every animation in the library is one of the layers above applied to one
component. Anything not listed here does not move.

| Component | What moves | Layer | Why |
|---|---|---|---|
| `Button`, `IconButton`, `SplitButton` | Sinks while held | `Pressable` | The control answers the pointer that is on it. |
| `Checkbox` | Check draws in, mixed bar and check cross over | `Transition<Point<f32>>` on `motion.quick` | Mixed and checked are tracked separately, so the box is never momentarily empty between them. |
| `Radio` | Dot scales in, border tints | `Transition<f32>` on `motion.quick` | The dot arrives rather than blinking on. |
| `Switch` | Knob slides, track crossfades | `Transition<f32>` on `motion.quick` | The knob is placed by margin, so the switch is the same size at every point of the slide. |
| `Slider` | Fill and handle follow the value | `Transition<f32>` on `spring.grab`, snapped while dragging | A value the pointer is holding must be exactly under the pointer; a value from anywhere else settles onto it. |
| `SegmentedControl` | Selection background slides | `Flipping::flip` | One background for the whole strip, so choosing moves it rather than redrawing it elsewhere. |
| `Select`, `Combobox`, `Menu`, `ContextMenu`, `CommandPalette` | Rows fade in as a wave | `Stagger::rows` + `motion.menu` | Opacity only: a rise is a layout input and would publish a moving box. |
| `Tabs` | Accent underline slides | `Flipping::flip` | The indicator is one element for the strip, not one per tab. |
| `Accordion` | Body height opens and closes | `Transition<f32>` on `motion.resize` + `layout::measure` | A settled section is laid out exactly as it was before there was motion here; only a section in flight is driven. |
| `Sidebar` | Icon slot slides on collapse | `Flipping::flip` | The glyph is what survives collapsing, so it travels rather than being redrawn narrow. |
| `ProgressBar` | Determinate fill moves | `Transition<f32>` on `motion.resize` | The published range is the caller's number from the frame it changes. |
| `Skeleton` | Highlight band sweeps | `with_animation`, `motion.shimmer` | A sweep reads as work moving through the list where a pulse reads as the list blinking. |
| `PulseLoader`, `GradientSpinner` | Cells breathe | `with_animation`, `motion.pulse` | Decorative, self-contained loops. |
| `EmptyState`, `Callout` | Content fades and rises | `motion::content_in` | The travel is inside the element that publishes the node, so the published box never moves. |
| `Card` | Rises on hover, sinks while held | `HoverLift`, `Pressable` | Only when the card is itself an action. |
| `ListRow`, `List`, `Table`, `Tree` rows | Sink while held | `Pressable` | Rows get no entrance: a row scrolled into a viewport is the same row that was always there. |
| `Dialog`, `Drawer` | Arrive on a spring, leave on a curve | `Presence` + `spring.smooth` | Arriving has weight; being dismissed is just gone. |
| `Toast` | Slot slides when the stack reflows | `Flipping::flip` | The slot slides, not the card, because the card is already carrying its own arrival. |
| `AnimatedNumber` | Glyphs count | `Transition<f32>` | The target is published from the frame it changes. |

Deliberately still: `Tooltip`, `Badge`, `Tag` (the body of it), `Breadcrumb`,
`Divider`, `Avatar`, `Kbd`, the split divider, and every scrim. A drag handle
in particular gets no press response, because a handle that sank under the
pointer would fight the drag it exists to serve.

### The stagger cap

`Stagger::rows` is 16ms a row across at most eight rows, so a wave never
lasts longer than `ROW_STAGGER_CAP` — 112ms — however many rows there are.
Past eight rows the step shrinks rather than the window growing, so a fifty-row
menu is fully drawn in about a sixth of a second instead of most of a second.

## Motion never changes what is published

This is the rule the rest of the page keeps. A slide, a press response and a
counting readout are all painted over a layout, a hit target and a semantic
tree that already report the settled value:

- a FLIP offset is applied after layout, so the element's box, its siblings and
  its published bounds are the ones it will have when the slide ends;
- a press or a hover response is a relative inset and a shadow, so no control
  changes size or pushes anything beside it;
- an `AnimatedNumber` publishes its target from the frame the target changes,
  and only the glyphs count up.

A test that reads the semantic tree therefore reads the settled truth, and
motion cannot make it flaky. Where a value in flight has to be observable —
watching a slide, for instance — it is exposed as an explicit accessor
(`Flip::offset`) rather than published as a fact about the interface.

The one exception is `Flipping::flip_size`, and it is an exception because
GPUI cannot make it anything else: an element really is a different size on
every frame of a size animation, so the box it occupies is the box in flight
and its siblings move with it. Nothing else in the library animates a size,
and no component uses `flip_size`. The section on FLIP below says what the
difference costs.

## Reduced motion

`gpui::App::reduce_motion` is authoritative. GPUI's `with_animation` already
renders a static state when it is set; `Transition::animate` and
`Presence::animate` honor the same preference by finishing immediately, so a
caller never has to branch on it.

Tests set `cx.set_reduce_motion(true)` when they need a deterministic frame.
`crates/gpui-kit/tests/motion.rs` carries one reduced-motion test per family —
choice controls, navigation, display, overlay — and each asserts the same
thing: what the tree publishes on the frame a change lands on is what it
publishes for good.

## Choosing a layer

Decorative, self-contained loops (spinners, pulses, skeletons) use
`with_animation` with a `MotionSpec`. Motion that follows application state
(a value that moves, a panel that opens) uses `Transition` or `Presence`,
because those survive interruption:

- retargeting a `Transition` continues from the value on screen instead of
  restarting from the old target;
- reversing a `Presence` mid-flight resumes from what is currently visible.

## Velocity handover

Continuing from the value on screen is not enough on its own: a value that was
travelling and is then aimed somewhere else would still leave from a standing
start, which reads as a stall for the first few frames.

A retarget therefore measures the speed the value already had, rescales it into
the new distance — that is what `Interpolate::distance` is for — and releases
the new motion with it. A sprung transition is then given
`Spring::settle_time_at` rather than its resting settle time, because a spring
that is already moving needs longer to come to rest.

Only a spring carries velocity. A cubic bezier is a shape read off a clock and
has no state to hand on, so a curve-based transition hands over nothing and
restarts its curve from zero.

The speed keeps its direction. `Interpolate::distance` is a length with no
sign, so the direction is recovered by stepping a little further along the path
the value is already on and asking whether that landed nearer the new target.
Reversing a target therefore throws the value on the way it was already going
before it comes back, which is what a moving thing does; turning it round on
the spot would be the same stall this exists to remove, wearing a different
shape. The same test reads a spring that has overshot correctly, where the
value is past its target and already travelling back toward it.

## Keyframes

`Keyframes` is a path through stops instead of a straight line between two
ends:

```rust
let path = Keyframes::new(theme, spec, [
    Keyframe::new(0.0, 0.0),
    Keyframe::new(0.5, 12.0).eased(Easing::EaseOut),
    Keyframe::new(1.0, 0.0),
])
.expect("a path needs at least one stop");
let value = path.sample(progress);
```

Stops may be given in any order and are sorted; offsets are clamped into
`0..=1`. A stop may name the `Easing` it is reached on, which is resolved
against the theme when the path is built, because `sample` has no theme to
resolve one against; stops without one use the specification's curve. If the
author wrote no stop at `0.0` or at `1.0`, the nearest stop extends to that
end. Sampling outside `0..=1` clamps: a keyframe list is an explicit path, and
what lies past the last place the author put a value is not something to guess.
An empty list is not a path, so the constructor returns `None`.

## Exit animations

An element cannot animate out after it has been dropped, so `Presence` owns the
decision:

```rust
let progress = self.presence.animate(window, cx);
if self.presence.is_rendered() {
    parent = parent.child(panel.opacity(progress));
}
```

`is_rendered` stays true through the whole exit and turns false only once the
element is gone, which is also what the semantic snapshot reports.

## Springs

`Spring` is evaluated analytically, so a value at any instant costs the same
regardless of frame rate and cannot drift. `Spring::settle_time` finds when the
motion is within one part in a thousand of its target, capped at four seconds so
an over-soft configuration cannot animate forever. `Spring::animation` adapts a
spring to GPUI's `Animation`, so it can drive `with_animation` like any curve.

`Spring::value_at` is the same solution released with a velocity already
carried into the motion, and returns the value with its own velocity so a
caller that retargets can hand the motion on. `Spring::settle_time_at` is the
matching settle time, bounded by the same four seconds.

Only an underdamped spring (damping ratio below one) overshoots. `bouncy` does,
`smooth` and `snappy` do not.

## FLIP

A row that changes place should arrive there, not appear there. `flip` reads
where the element was, lets layout put it where it now belongs, inverts the
difference into a visual offset, and plays that offset back to zero on the
`grab` spring:

```rust
let handle = flip("queue.publish", cx);
row.flip(&handle, window, cx)
```

The offset is applied during prepaint through `Window::with_element_offset`,
which runs after layout has already been computed. That is the whole reason it
cannot move a sibling: there is no box to push. The alternative — a relative
`inset` on the element — also leaves siblings alone, but it is a layout input,
so the measurement taken on the next frame includes the offset already applied
and has to be corrected back out. Offsetting during prepaint measures the pure
layout origin instead.

The origin an element is compared against has the ambient element offset
removed, so scrolling a list — which offsets every row at once — is not
mistaken for a reorder. A move that arrives mid-slide continues from the offset
on screen rather than restarting.

Per-element state lives in an application global keyed by semantic id, the same
arrangement `layout::measure` uses, because a `RenderOnce` builder cannot carry
anything across frames. An id that stops rendering is dropped within two
frames; the frame counter is the semantic registry's generation, which a host
bumps at the top of every root render.

Under reduced motion the offset is zero from the first frame: the element is
simply at its new place.

### Position is free, size is not

`flip` and `flip_size` are two different promises and choosing between them is
choosing which one you want:

| | `flip` | `flip_size` |
|---|---|---|
| What animates | Position | Position and size |
| Effect on siblings | None, ever | They move with it |
| Layout node | The wrapped element's own | One the wrapper owns |
| Cost per frame | An offset | An extra measurement |
| What the semantic tree publishes | The settled box | The box in flight |

The asymmetry is not a design choice. The pinned GPUI revision has no
transform for an element subtree — `TransformationMatrix` reaches sprites
alone — so a size change cannot be faked with a scale the way a browser does
it. An element that grows is genuinely laid out larger, and everything after
it in its container is genuinely pushed. `flip` is therefore the default and
`flip_size` is opt-in: a row that only changes place must not start owning a
layout node because something else in the library grew.

```rust
let handle = flip("card.7", cx);
card.flip_size(&handle, window, cx)
```

What `flip_size` animates is the box the element is given. An element that
sizes itself from that box — `size_full`, a percentage, a flex child — is
therefore drawn at the animated size. An element with a fixed size of its own
keeps it, and what animates is the space it sits in. There is no third option
without a transform.

Two consequences worth knowing before choosing it:

- The first frame an id is ever seen on is passed straight through to the
  parent, because the constraints the element will be measured against are not
  known until the parent has laid it out once. That frame is the correct one;
  animation starts from the second.
- A change that comes from the *container* rather than from the element takes
  a frame to be noticed, for the same reason: the new constraints arrive
  during layout, and the natural size is measured against them on the frame
  after. A frame is requested for it, so nothing waits on unrelated work.

`Flip::size` reports the size being painted and `Flip::target_size` the size
layout would give the element if nothing were animating. Under reduced motion
the element is at its new size from the first frame, siblings included.

### Shared elements

A row in a list and the detail panel it opens into are two elements in two
trees with one identity. Because flip state is keyed by semantic id rather
than by element, the panel inverts from the rectangle the row last recorded
and travels there instead of cutting:

```rust
let handle = shared_flip("item.7", cx);
panel.flip_size(&handle, window, cx)
```

`shared_flip` differs from `flip` in one respect only: patience. The
rectangle survives the frames in which neither tree renders the id — 30
frames, or 500ms of wall clock, whichever runs out first. Two bounds because
an idle window advances the clock without drawing and a busy one draws faster
than the clock moves; past either, the arriving element is simply already in
place, because flying in from where something stood a minute ago is worse than
not animating at all.

Both trees rendering the same id in one frame is a collision, and a collision
does not animate. Two elements sharing one slot would each read the other's
rectangle as its own previous one and throw the other across the window, every
frame, for as long as both were on screen. Instead neither moves, and the
refusal outlasts the collision by one frame, because the frame after is the
first that can record a rectangle nothing else is writing to. `Flip::is_contended`
reports it. The rule is deliberate rather than emergent: the alternative,
letting the last element rendered win, is the oscillation itself.

A shared element transition cannot be captured as a scene. A still frame of it
is either the list or the panel, and the arrangement that would show both — two
trees rendering one id at once — is exactly the collision case above. Run it,
or read the tests in `crates/gpui-kit/tests/motion.rs`.

## Pointer responses

```rust
div().id("card").hover_lift(cx).pressable(cx)
```

`pressable` sinks a control by `motion.pressOffsetPx` while it is held.
It is a downward shift and not a scale: the pinned GPUI revision offers a
transform on `svg` alone, and redrawing a control at a different size would be
a layout change wearing a costume. Every actionable `Button` wears it.

`hover_lift` raises a surface by `motion.hoverLiftPx` onto the raised elevation
shadow while the pointer is over it. The shadow does the work; the pixel of
travel only sells it.

Both are relative insets, so neither changes an element's size or moves what is
beside it, and both are nothing at all under reduced motion.

## Animated numbers

```rust
AnimatedNumber::new("run.total", 1204.0).format(grouped)
```

The readout counts to its new value on a `Transition<f32>` kept per id, and
publishes the target immediately — an assertion that the total is `1,204`
cannot race the count. The format function decides the text: the component
never invents a grouping separator or a precision, because how many decimals a
quantity carries belongs to whoever owns the quantity. `grouped` is provided
for the common case. Reduced motion shows the target at once.

## Tokens

Durations, the nine easing curves and the four spring presets live in
`tokens/*.json`. Component code names a role (`Easing::Standard`,
`SpringPreset::Snappy`) rather than control points.

`spring.grab` is the tight, quick-settling spring for direct manipulation: it
drives FLIP, the slider's follow, and anything else that has to feel attached
to the pointer rather than trailing it. `spring.smooth` is what a dialog and a
drawer arrive on, through `MotionSpec::sprung`, which takes its duration from
`Spring::settle_time` so a sprung specification runs anywhere a curved one
does. GPUI requires an eased delta inside `0..1`, so `MotionSpec::animation`
clamps; overshoot survives in `Transition` and `Presence`, which sample
`MotionSpec::progress` directly. `motion.pressOffsetPx` and `motion.hoverLiftPx` are the two
pointer responses, both validated to stay within a hairline so a response can
never be mistaken for a layout change.
