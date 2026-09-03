# Motion

Motion in this library is layered, and every layer is pure enough to test
without a window.

| Layer | Type | Purpose |
|---|---|---|
| Curve | `CubicBezier`, `Easing` | Names a shape; `Easing` resolves against the theme. |
| Physics | `Spring` | Closed-form damped spring from stiffness, damping and mass. |
| Specification | `MotionSpec` | A curve — or a spring, via `MotionSpec::sprung` — plus duration and delay. |
| Policy | `MotionRole`, `MotionPolicy`, `ResolvedMotion` | Names why UI moves, resolves one theme-backed specification, and chooses the reduced-motion disposition. |
| Value | `Interpolate` | Moves `f32`, `Pixels`, `Rems`, `Hsla`, `Point` and `Size`, and measures how far apart two of them are. |
| Path | `Keyframes` | Takes a value through named stops rather than straight across. |
| State | `Transition` | Animates a value whose target can change mid-flight, carrying the speed it already had. |
| Gesture | `VelocityTracker` | Measures how fast a gesture is moving, so `flick`, `rubber_band` and `Transition::release` have a speed to work from. |
| Offset | `ScrollLink` | Reads a scroll offset as a progress, with no clock and no frames. |
| Lifecycle | `Presence` | Keeps an element alive long enough to animate out, and plays a cancelled phase backwards. |
| Group | `Stagger` | Spreads one specification across a list, forwards or in reverse. |
| Order | `Sequence` | Runs specifications one after another and reports how long they take together. |
| Position | `Flipping::flip` | Slides an element from where it was to where it is. |
| Rectangle | `Flipping::flip_size` | The same, and resizes it too — which, unlike the slide, is a real layout change. |
| Pointer | `Pressable`, `HoverLift` | The two responses a control gives a pointer. |
| Value | `AnimatedNumber` | Counts to a new number while publishing the target. |

## Components choose roles, not timings

`MotionRole` is the component boundary. A component may say that a value is a
`StateChange`, that the reader is making a `Navigation`, or that a mark reports
`Activity::Working`; it may not choose milliseconds, a bezier, or a spring.
`MotionPolicy` is the only mapping from those reasons to the theme. This keeps
motion coherent when a theme changes and keeps reduced motion from becoming a
different local `if` in every component.

| Role | Used for | Theme-backed shape |
|---|---|---|
| `Entrance` | Content joining the current surface | `durationMs.entrance` + settle curve |
| `MenuEnter` | Menus, popovers, and other direct pointer answers | `durationMs.menu` + standard curve |
| `ModalEnter` | A modal panel taking over a region | `spring.smooth` |
| `ModalTransition` | Content changing inside that modal | `durationMs.dialog` + standard curve |
| `Exit` | Dismissed content leaving | `durationMs.quick` + exit curve |
| `StateChange` | A local control response | `durationMs.quick` + standard curve |
| `Resize` | A measured value or extent changing | `durationMs.resize` + standard curve |
| `Tracking` | A value following a pointer or moving target | `spring.grab` |
| `Navigation` | Travel between locations, pages, or months | `durationMs.entrance` + ease-in-out curve |
| `Streaming` | Newly arrived text settling in place | cadence bounded by menu/entrance roles |
| `Feedback` | A one-shot outcome, transfer, or handoff | `durationMs.feedback` + settle curve |
| `Celebration` | A deliberately prominent reward | `durationMs.celebration` + emphasized curve |
| `Activity(Activity)` | Advancing, working, or deliberating loops | the activity's shimmer/spin/pulse token |
| `Micro(Micro)` | Heartbeat, bounce, wobble, pop, or sparkle | the named micro token |

### The product contract

A product asking "what should move here" starts from the event, not from a
component. One event, one role, one helper; anything not on this list holds
still.

| The product event | Role | The one call |
|---|---|---|
| A page's content replaces what was there | `Entrance` | `animate_in(Entrance::Rise)` / `content_in` |
| Real content replaces a skeleton or empty stand-in | `StateChange` | `surface_in` (opacity only — the box was already reserved) |
| A menu or popover answers a click | `MenuEnter` | `animate_in(Entrance::Menu)`; rows via `Stagger::rows` |
| A dialog takes the page over | `ModalEnter` | `Dialog` carries it; elsewhere `animate_in(Entrance::Dialog)` / `dialog_in` |
| A list arrives with its surface | `MenuEnter` | `List::arriving` / `animate_in_staggered` |
| A control answers the pointer on it | `StateChange` | `Pressable`, `animate_change` |
| A selection or indicator moves between peers | `Tracking` | `Flipping::flip` |
| A measured value or extent changes | `Resize` | `Transition` |
| Dismissed content leaves | `Exit` | `Presence` |
| Work is in flight | `Activity(..)` | the loader family; never a hand-rolled loop |

Downstream components use the same boundary. `Transition` and `Presence`
already finish immediately for reduced motion, so they take the resolved spec:

```rust
let motion = MotionPolicy::resolve(MotionRole::StateChange, cx);
let transition = Transition::new(current, motion.spec());

let presence = Presence::hidden(
    MotionPolicy::spec(MotionRole::MenuEnter, theme),
    MotionPolicy::spec(MotionRole::Exit, theme),
);
```

For a repeating or decorative timeline, inspect `motion.disposition()` or the
short form `motion.animates()` before scheduling frames. `MotionSpec::new`
remains available for motion tooling, authored keyframes, and tests; it is not
a component styling API. The older `entrance`, `menu`, `dialog`, `resize`,
`state_change`, and `tracking` helpers remain source-compatible wrappers and
delegate to `MotionPolicy`.

## What moves, where

Every animation in the library is one of the layers above applied to one
component. Anything not listed here does not move.

| Component | What moves | Role + mechanism | Why |
|---|---|---|---|
| `Button`, `IconButton`, `SplitButton` | Sinks while held | `StateChange` + `Pressable` | The control answers the pointer that is on it. |
| `Checkbox` | Check draws in, mixed bar and check cross over | `StateChange` + `Transition<Point<f32>>` | Mixed and checked are tracked separately, so the box is never momentarily empty between them. |
| `Radio` | Dot scales in, border tints | `StateChange` + `Transition<f32>` | The dot arrives rather than blinking on. |
| `Switch` | Knob slides, track crossfades | `StateChange` + `Transition<f32>` | The knob is placed by margin, so the switch is the same size at every point of the slide. |
| `Slider`, media scrubbers | Fill and handle follow the value | `Tracking` + `Transition<f32>`, snapped while dragging | A value the pointer is holding must be exactly under the pointer; a value from anywhere else settles onto it. |
| `SegmentedControl`, `Tabs`, `Sidebar` | Indicator or surviving glyph changes slots | `Tracking` + `Flipping::flip` | One visual identity travels rather than being redrawn elsewhere. |
| `Select`, `Combobox`, `Menu`, `ContextMenu`, `CommandPalette` | Rows fade in as a wave | `MenuEnter` + `Stagger::rows` | Opacity only: a rise is a layout input and would publish a moving box. |
| `Accordion`, `Collapsible` | Body height opens and closes | `Resize` + tracked progress + `gpui::reveal` | Kit chooses the semantic motion role; the framework measures the natural subtree and keeps layout, clipping, hit testing, and accessibility bounds on the same revealed extent. |
| `ProgressBar`, `ProgressCircle`, `AnimatedNumber` | Determinate value moves | `Resize` + `Transition` | The published value is the caller's number from the frame it changes. |
| `Skeleton`, unknown progress, graph activity | Highlight or trace sweeps | `Activity::Advancing` + repeating timeline | A sweep reads as work moving through known direction without inventing a percentage. |
| `PulseLoader`, listening voice | Dots or bars breathe | `Activity::Deliberating` + repeating timeline | The quietest claim there is: something is being waited on. Reduced motion leaves a static meaningful mark. |
| `Spinner`, speaking voice | Arc or bars cycle | `Activity::Working` + repeating timeline | A turn has no endpoint to imply; reduced motion suppresses the timeline. |
| `EmptyState`, `Callout` | Content fades and rises | `Entrance` + `content_in` | The travel is inside the element that publishes the node, so the published box never moves. |
| `Card` | Rises on hover, sinks while held | `StateChange` + `HoverLift`/`Pressable` | Only when the card is itself an action. |
| `ListRow`, `List`, `Table`, `Tree` rows | Sink while held | `StateChange` + `Pressable` | Rows get no entrance by default: a row scrolled into a viewport is the same row that was always there. `List::arriving` opts a fully laid-out list into the menu wave for the moment the whole list is what arrived; a virtualized window ignores it. |
| `Tooltip` | Arrives as a menu does | `MenuEnter` + `animate_in` | Help answering a hover that has already happened. |
| `Dialog`, `Drawer` | Arrive on a spring, leave on a curve | `ModalEnter`/`Exit` + `Presence` | Arriving has weight; being dismissed is just gone. |
| `Toast` | Arrives, exits, and changes stack slot | `MenuEnter`/`Exit` + `Presence`; `Tracking` + FLIP | The slot slides, not the card, because the card already carries its own arrival. |
| `Carousel`, `Calendar`, virtualized glide | Page, month, or viewport travels | `Navigation` + `Transition`/`Glide` | Travel preserves the reader's direction and place. |
| `Wizard` | Current/completed marker fills | `StateChange` + `Transition<f32>` | Status changes in place without changing the published step. |
| Charts | Marks enter, move, leave, and crosshair follows | `Entrance`/`Resize`/`Exit`/`Tracking` | Data identity and target values are settled facts; only paint travels. |
| Streaming `Markdown` | Newly appended text fades to settled opacity | `Streaming` + cadence policy | Text is laid out immediately; only the newest glyphs soften their arrival. |
| `Glass` | Press refraction depth follows state | `StateChange` + `Transition<f32>` | Material response uses the same control rhythm as the pointer action. |
| `EffectParticles`, `CinematicEffect` | Semantic outcomes and rewards play once | `Feedback`/`Celebration` | Outcomes share global timing; reduced motion receives a policy-owned poster. |
| Drag ghost and make-way slots | Ghost/slot follows a moving target | `Tracking` + `spring.grab` | Pointer-owned movement remains attached and settles coherently. |
| `ScrollArea` | Top shadow fades in once the content is off the top | `ScrollLink` | A function of the offset rather than of a clock, so it never animates on its own and asks for no frames. |

Deliberately still: `Badge`, `Tag` (the body of it), `Breadcrumb`,
`Divider`, `Avatar`, `Kbd`, the split divider, and every scrim. A drag handle
in particular gets no press response, because a handle that sank under the
pointer would fight the drag it exists to serve.

### The stagger cap

`Stagger::rows(theme)` reads both its neighbour delay and maximum item window
from `motion.durationMs.staggerStep` and `motion.staggerMaxItems`. In the bundled
themes that is 16ms a row across at most eight rows, so `row_stagger_cap(theme)`
is 112ms. Past eight rows the step shrinks rather than the window growing, so a
fifty-row menu is fully drawn in about a sixth of a second instead of most of a
second. Custom themes can change either part without replacing motion code.

### Which end a wave starts from

`Stagger::reversed` runs the same wave from the last item to the first. A list
that arrived from the top down should leave from the bottom up: the row the
user is looking at, the one they just acted on, should be the last to go rather
than the first, so the group empties away from them rather than out from under
them. Reversing changes which item waits longest and nothing else — the step,
the cap and `Stagger::total` are the same either way, which is what lets a
caller hold the group on screen for one duration whichever direction it runs
in.

## And then

`MotionSpec::after` moves a specification to start when another has finished,
keeping its own delay as the gap between the two:

```rust
let panel = MotionPolicy::spec(MotionRole::ModalEnter, theme);
let content = MotionPolicy::spec(MotionRole::Entrance, theme)
    .with_delay(40)
    .after(panel);
```

`Sequence` is the same composition for more than two, and exists for what a
chain of `after` cannot answer: it keeps the steps, so `Sequence::step(i)`
hands one of them to anything that runs a single specification and
`Sequence::progress(i, raw)` drives every step from one clock over the whole
run; and it reports `Sequence::total`, which is what a caller holding an
element on screen — `Presence` included — needs and cannot otherwise get
without adding the durations up by hand.

```rust
let run = Sequence::new([header]).then(body.with_delay(40)).then(footer);
let total = run.total();
```

A step that has not started reports 0 and a step that is over reports 1, so
painting all of them from one progress leaves the finished ones where they
landed.

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

`gpui::App::reduce_motion` is authoritative, but not every kind of motion has
the same truthful static answer. `MotionPolicy` resolves the answer with the
role instead of leaving each component to improvise a preference branch:

| Role under reduced motion | Disposition | Static answer |
|---|---|---|
| Finite transitions (`Entrance` through `Navigation`) | `Settle` | Publish and paint the endpoint immediately. |
| `Activity`, `Streaming`, repeating `Micro` | `Suppress` | Start no timeline; leave the meaningful static state in place. |
| `Feedback`, `Celebration` | `Poster` | Paint the policy-owned representative frame once. |
| Finite `Micro` | `Settle` | Paint its endpoint without playing the reaction. |

GPUI's `with_animation`, `Transition::animate`, and `Presence::animate` already
settle finite motion. Repeating and procedural renderers check the resolved
disposition before asking for frames, and effect renderers select their poster
when the policy says `Poster`. A downstream component should therefore resolve
one role rather than read `cx.reduce_motion()` directly.

Tests set `cx.set_reduce_motion(true)` when they need a deterministic frame.
`crates/gpui-kit/tests/motion.rs` carries one reduced-motion test per family —
choice controls, navigation, display, overlay — and each asserts the same
thing: what the tree publishes on the frame a change lands on is what it
publishes for good.

## Choosing a layer

Decorative, self-contained loops (spinners, pulses, skeletons) resolve an
`Activity` role, check its disposition, then use `with_animation` with the
resolved specification. Motion that follows application state (a value that
moves, a panel that opens) resolves its finite role and uses `Transition` or
`Presence`, because those survive interruption:

- retargeting a `Transition` continues from the value on screen instead of
  restarting from the old target;
- reversing a `Presence` mid-flight resumes from what is currently visible.

## Describing a run, and driving it by hand

A run with more than one property is written as a description rather than as
a chain of setters. `motion!` is the grammar for one motion and `sequence!`
places motions on one clock; both expand to the existing `MotionSpec` model,
so no second easing or time vocabulary exists:

```rust
let arrive = motion! {
    duration: 420;
    ease: overshoot;
    opacity: 0.0 => 1.0;
    y: 12.0 => 0.0;
};
let opening = sequence![menu_spec, +80 arrive.spec(&theme)];
```

`Motion::sample(&theme, t)` is pure — keyframed tracks in, a `MotionSample`
out, nothing to tick — which is why the whole description layer unit-tests
without a window, and why a non-finite value can be caught there and fall
back to the property's neutral instead of reaching layout.

`Animator` is the clock when the caller owns the playhead: play, pause,
reverse, scrub, and re-speed over one motion or a whole sequence. It stores
an anchor rather than ticking, so a paused animator costs nothing and seeking
is one assignment; `finish()` is the reduced-motion answer. See the
`micro` scene's third strip for a described motion sampled along its run.

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

## Gesture velocity

A gesture reports where the pointer is. How fast it was going is a
measurement, and `VelocityTracker` is that measurement:

```rust
tracker.sample(pointer, cx.background_executor().now());
let velocity = tracker.velocity_at(cx.background_executor().now());
```

Two decisions in it matter more than the arithmetic.

The speed is measured over a short trailing window — `VELOCITY_WINDOW`, 100ms
— rather than over the last two events. Platforms deliver moves at whatever
rate they please, and the last pair can be a millisecond apart, which divides
two pixels by almost nothing and reports a speed no hand ever moved at. A span
shorter than 8ms is therefore not believed at all and reports nothing, because
"not measurable" is the honest answer and a made-up number would be flung.

Samples older than the window are discarded against the clock passed to
`velocity_at`, not against the last sample, because a pointer that has stopped
sends nothing at all. **A drag that stops before release has no velocity.**
That is the whole reason the window exists: a tracker that reported the speed
from before the pause would fling away the thing the user deliberately parked.

A drop carries it. `DropIntent::velocity` is the speed the pointer was moving
at when it let go, and `ActiveDrag::velocity` is the speed it is moving at now.
A staged drag has no pointer, so it reports `Velocity::ZERO`.

Three effects are built on it:

- `flick(travel, velocity, theme)` answers whether a gesture was a flick and
  which way. It takes both the distance and the speed, because speed alone
  calls a twitch a flick and distance alone calls a slow deliberate drag one,
  and those are exactly the two gestures a dismissal has to tell apart. The
  travel has to agree with the direction of the speed, so a gesture that was
  already coming back was not flicked out. The threshold is
  `motion.flickVelocityPxPerSec`.
- Inertia is `Transition::release(target, velocity)`: the same handover a
  retarget performs, with the speed coming from the hand instead of from the
  motion being interrupted. A flicked value carries on and settles under its
  spring rather than stopping dead the instant the finger leaves it. Only a
  sprung specification can carry it, for the reason above: a curve has no
  momentum.
- `rubber_band(overscroll, extent, tension)` maps a pull past a boundary to
  the distance actually shown. `motion.rubberBandTension` is the fraction of
  the first pixel that shows; every pixel after it shows less, so the band
  tightens smoothly rather than at a point the hand can feel. The result
  approaches `extent` and never reaches it, so a boundary can be stretched but
  not crossed. It is a pure function of the pull — no clock, no state, no
  frame — because the band is where the hand is holding it.

Nothing in this library overscrolls, so `rubber_band` is provided for a caller
and used by no component here. That is recorded in `docs/coverage.md`.

## Scroll-linked values

`ScrollLink` maps a range of scroll offsets onto progress from 0 to 1:

```rust
let header = ScrollLink::new(px(0.0), px(64.0));
let height = header.sample(
    scroll_offset("inbox", window, cx).y,
    px(96.0),
    px(40.0),
);
```

It has no duration, no start and no end, it never requests an animation frame,
and there is no such thing as interrupting it. Scrolling back up runs it
backwards because the offset went backwards. That is why it is a plain value
with no `animate` and no `Window`: there is nothing to drive. Anything that
takes a progress can be driven from it, `Keyframes::sample` included.

`layout::scroll_offset(ident, window, cx)` reads how far a `ScrollArea` has been
scrolled, which is the input side of the same pair.

`ScrollArea` uses it for the second of the two motivating cases: a hairline
shadow at the top of the viewport, faded in over the first `effect.edgeFadeBand`
pixels of scrolling. It says there is content above the fold, it is not drawn
at all while the content is at the top, and it moves only because the user
moved the content.

### Reduced motion, and whose decision it is

A link makes no decision of its own, deliberately. A header that collapses as
the content scrolls under it, or a shadow that appears once there is something
above the fold, is not gratuitous motion: it is a one-to-one response to a
movement the user is making with their own hand, and suppressing it would
remove information rather than calm — the header would jump between two heights
and the shadow would blink. A decorative parallax, a background drifting at a
different rate to say nothing at all, is the opposite.

Only the caller knows which of those it is building, so the caller says so:

```rust
ScrollLink::over(px(300.0)).decorative(motion::reduce_motion(cx))
```

A decorative link under reduced motion reports 0 at every offset, which is the
resting end of the effect: the layer simply sits where it belongs.

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

### A phase that is cancelled plays backwards

An entrance cancelled at 30% leaves from 30%. It does not restart a full exit
from a state the element never reached, and it does not jump to a different
opacity on the frame it is cancelled.

The two phases are separate specifications with separate durations and separate
curves, so "where it had got to" is a position, not a time. The visible
progress is looked up in the other specification — `MotionSpec::time_at`, the
inverse of `MotionSpec::progress` — and the reversal starts at the point that
produces it. The element therefore leaves through the exit's own curve, using
the part of the exit's time that is left once the rest of it is already behind:
roughly 30% of it for a 30% entrance, and exactly that when both curves are
linear. Reading the position back rather than scaling the elapsed time is what
makes it true for a curve that is not linear and for a sprung phase.

`time_at` is sampled at a millisecond rather than solved. A cubic bezier has no
closed-form inverse, and a spring is not even monotonic — an underdamped one
passes its target and comes back — so the earliest time the value was there is
the only well-defined answer, and a millisecond is the granularity a
specification is written in anyway.

This is deliberately not the velocity handover a `Transition` retarget
performs. A value aimed somewhere new is still going the way it was going; a
phase that is cancelled has been told to go back. Carrying the speed across
would mean an element on its way in overshooting past being present, and that
is not a state a lifecycle has.

## Springs

`Spring` is evaluated analytically, so a value at any instant costs the same
regardless of frame rate and cannot drift. `Spring::settle_time` finds when the
motion is within one part in a thousand of its target, capped at four seconds so
an over-soft configuration cannot animate forever. `Spring::animation` adapts a
spring to GPUI's `Animation`, so it can drive `with_animation` like any curve.

The scalar solver is GPUI's `SpringConfig`: it advances a `SpringState` toward
a fixed target with `step`, toward a steadily moving target with `step_ramp`,
and can derive a conservative settle time without integrating frame by frame.
`AnimationExt::with_spring` is the stateful element path. It keeps velocity
across retargets under a stable element id and supports pause, stop, complete,
cancel, an explicit initial value, and reduced motion. `SpringTarget` projects
the scalar coordinate to pixels, rems, phases, booleans, or a caller-defined
path. Kit's `Spring` remains the policy adapter for theme tokens, perceptual
duration/bounce, visual thresholds, transitions, presence, and FLIP; its
scalar values come from the same framework solver rather than a second copy.

`Spring::value_at` is the same solution released with a velocity already
carried into the motion, and returns the value with its own velocity so a
caller that retargets can hand the motion on. `Spring::settle_time_at` is the
matching settle time, bounded by the same four seconds.

Only an underdamped spring (damping ratio below one) overshoots. `bouncy` does,
`smooth` and `snappy` do not.

### Duration and bounce

Stiffness, damping and mass are three numbers for two decisions, and neither
decision is any of the three. `Spring::perceptual(duration, bounce)` is the way
in for a design decision, and it is a change of variables rather than an
approximation — the same parameterisation as SwiftUI's
`Spring(duration:bounce:)`:

- mass is fixed at 1, because a spring depends on stiffness and damping only
  through `k/m` and `c/m`;
- `duration` is the period of the undamped oscillation, `omega = 2π/duration`,
  so `stiffness = omega² · mass`;
- `bounce` is the damping ratio turned inside out so that 0 is critical damping
  from either side: `zeta = 1 - bounce` for a positive bounce, reaching the
  undamped `zeta = 0` at 1, and `zeta = 1/(1 + bounce)` for a negative one,
  growing without bound toward -1. Damping follows:
  `damping = 2·zeta·sqrt(stiffness·mass)`, which is `4π·zeta·mass/duration`.

So a bounce of 0 settles without passing its target, a positive bounce
overshoots and comes back, and a negative one crawls in. The bounce is held
inside `-0.99..=0.99`, because both ends of the mapping describe a spring that
never arrives.

`Spring::perceptual_duration` and `Spring::bounce` read the same two numbers
back off a spring built any other way. The duration is not the settle time: at
a bounce of 0 a spring is about 99% of the way there when its perceptual
duration is up, and a bouncier one is still visibly moving. `settle_time` is
the honest end of the motion.

`Spring::new` is unchanged, and the token presets still come through it.

## FLIP

A row that changes place should arrive there, not appear there. `flip` reads
where the element was, lets layout put it where it now belongs, inverts the
difference into a visual offset, and plays that offset back to zero on the
`grab` spring:

```rust
let handle = flip("queue.publish", window, cx);
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
let handle = flip("card.7", window, cx);
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
let handle = shared_flip("item.7", window, cx);
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
`crates/gpui-kit-tokens/tokens/*.json`. Component code names a role (`Easing::Standard`,
`SpringPreset::Snappy`) rather than control points.

`spring.grab` is the tight, quick-settling spring for direct manipulation: it
drives FLIP, the slider's follow, and anything else that has to feel attached
to the pointer rather than trailing it. `spring.smooth` is what a dialog and a
drawer arrive on, through `MotionSpec::sprung`, which takes its duration from
`Spring::settle_time` so a sprung specification runs anywhere a curved one
does. GPUI accepts any finite eased value, and `with_spring`, `Transition`, and
`Presence` preserve an underdamped spring's overshoot. `MotionSpec::animation`
remains clamped because the established Kit helpers feed its fraction to
bounded properties such as opacity. `motion.pressOffsetPx` and
`motion.hoverLiftPx` are the two
pointer responses, both validated to stay within a hairline so a response can
never be mistaken for a layout change.

`motion.flickVelocityPxPerSec` and `motion.rubberBandTension` are the two
gesture decisions. A flick threshold is a judgement about intent and a band
tension is a judgement about how much a boundary should give, so neither is a
number a component gets to invent.
