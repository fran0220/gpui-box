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

## Tokens

Durations, the nine easing curves and the three spring presets live in
`tokens/*.json`. Component code names a role (`Easing::Standard`,
`SpringPreset::Snappy`) rather than control points.
