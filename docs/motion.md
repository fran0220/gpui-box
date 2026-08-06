# Motion

Motion in this library is layered, and every layer is pure enough to test
without a window.

| Layer | Type | Purpose |
|---|---|---|
| Curve | `CubicBezier`, `Easing` | Names a shape; `Easing` resolves against the theme. |
| Physics | `Spring` | Closed-form damped spring from stiffness, damping and mass. |
| Specification | `MotionSpec` | A curve plus duration and delay. |
| Value | `Interpolate` | Moves `f32`, `Pixels`, `Rems`, `Hsla`, `Point` and `Size`. |
| State | `Transition` | Animates a value whose target can change mid-flight. |
| Lifecycle | `Presence` | Keeps an element alive long enough to animate out. |
| Group | `Stagger` | Spreads one specification across a list. |
| Position | `Flipping::flip` | Slides an element from where it was to where it is. |
| Pointer | `Pressable`, `HoverLift` | The two responses a control gives a pointer. |
| Value | `AnimatedNumber` | Counts to a new number while publishing the target. |

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

## Reduced motion

`gpui::App::reduce_motion` is authoritative. GPUI's `with_animation` already
renders a static state when it is set; `Transition::animate` and
`Presence::animate` honor the same preference by finishing immediately, so a
caller never has to branch on it.

Tests set `cx.set_reduce_motion(true)` when they need a deterministic frame.

## Choosing a layer

Decorative, self-contained loops (spinners, pulses, skeletons) use
`with_animation` with a `MotionSpec`. Motion that follows application state
(a value that moves, a panel that opens) uses `Transition` or `Presence`,
because those survive interruption:

- retargeting a `Transition` continues from the value on screen instead of
  restarting from the old target;
- reversing a `Presence` mid-flight resumes from what is currently visible.

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
drives FLIP, and anything else that has to feel attached to the pointer rather
than trailing it. `motion.pressOffsetPx` and `motion.hoverLiftPx` are the two
pointer responses, both validated to stay within a hairline so a response can
never be mistaken for a layout change.
