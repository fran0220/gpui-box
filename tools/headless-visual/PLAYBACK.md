# Exact local playback frames

This is an opt-in extension to the checkout's `serve` protocol, not the hosted
MCP contract. Ordinary `open`, `screenshot`, `capture`, and `check` retain their
reduced-motion and settling defaults. No framework changes are required.

Build with `cargo build --manifest-path tools/headless-visual/Cargo.toml`.
Start `tools/headless-visual/target/debug/gpui-box-headless-visual serve` and send
newline-delimited JSON requests. Use the session returned by `open`:

```json
{"id":1,"method":"open","params":{"scene":"motion-primitives","theme":"studio-light"}}
{"id":2,"method":"act","params":{"session":"s1","type":"click","id":"scene.motion.tabs.spring"}}
{"id":3,"method":"motion","params":{"session":"s1","reduced_motion":false}}
{"id":4,"method":"act","params":{"session":"s1","type":"click","id":"scene.motion.spring.timeline"}}
{"id":5,"method":"frame","params":{"session":"s1","ms":80,"path":"target/playback/middle.png"}}
{"id":6,"method":"motion","params":{"session":"s1","reduced_motion":true}}
{"id":7,"method":"frame","params":{"session":"s1","ms":0,"path":"target/playback/settled.png"}}
```

`frame` advances the application clock by exactly `ms`, schedules the next frame
using the existing renderer, draws, and captures **without the screenshot settling
loop**. Its response includes cumulative `time_ms` (including prior `advance`
calls), `reduced_motion`, semantic `generation`, a redacted `snapshot` for that
frame, and the existing PNG path/bytes/base64 fields. `ms:0` samples without
advancing time, including immediately after retargeting. Clock time is simulated,
not wall time or a guaranteed native-display presentation time. Draws do not
advance it. The existing `screenshot` still redraws to pixel stability at the
current time; use `frame` when intermediate state is the evidence.

Motion and the clock are application-global. Opt-in requires exactly one open
session and blocks additional opens. Restoring reduced motion or closing that
session restores normal operation. Use separate serve processes for parallel
playback. No opt-in persists across process restarts.

## Reproduce intermediate motion, interruption, and reduced-motion settling

```bash
python3 tools/headless-visual/examples/playback.py .amp/in/artifacts/spring-playback
ffmpeg -y -framerate 62.5 -i .amp/in/artifacts/spring-playback/frame-%03d.png \
  -c:v libx264 -pix_fmt yuv420p .amp/in/artifacts/spring-playback/playback.mp4
cargo test --manifest-path tools/headless-visual/Cargo.toml playback_samples -- --test-threads=1
```

The script records each exact sampled time and indicator bounds in `samples.json`.
It reverses a moving spring at 160ms and later restores reduced motion without
advancing time. The video is a viewing aid with uniform frame presentation;
`samples.json` is authoritative for the duplicate zero-duration samples.
The test independently checks default settling, intermediate semantic geometry
and rendered pixel changes, continuous retargeting, reduced-motion final geometry,
exact clock values, session exclusivity, and cleanup.

## Raw local input (v2)

`act` additionally accepts the following action `type` values. The session can
be alongside the action fields, as below, or outside a nested `action` object.
No action advances simulated time; follow it with `frame` to sample explicitly.

| Type | Fields | GPUI dispatch |
| --- | --- | --- |
| `pointer_move` | `x`, `y`, optional `pressed_button` | `MouseMoveEvent` |
| `pointer_down` | `x`, `y`, `button` | `MouseDownEvent`, click count 1 |
| `pointer_up` | `x`, `y`, `button` | `MouseUpEvent`, click count 1 |
| `pointer_cancel` | none | `MouseCancelEvent`, never release/click |
| `wheel` | `x`, `y`, `delta_x`, `delta_y` | `ScrollWheelEvent`, pixel delta, phase Moved |
| `touch` | `x`, `y`, `touch_id`, `phase` | `TouchEvent`, no force/prediction |
| `touch_cancel_all` | none | `Window::cancel_touch_input`, including pending recognition/momentum |

Coordinates are logical **window** pixels, not screenshot pixels (the default
screenshot scale is 2). Coordinate and raw wheel-delta fields must be finite and
within −16,384…16,384. Out-of-window positions are permitted and never clamped:
capture must deliver real outside moves/releases. Invalid raw fields are rejected
before dispatch. Button names are `left`, `right`, `middle`, `back`, `forward`.
On moves, omitted/null `pressed_button` means none. Supply held-button state on
every move; the harness does not infer it or manufacture missing events.

Pointer and wheel inputs optionally carry `modifiers`, an array containing any
of `shift`, `control`, `alt`, `platform`, `function`. Unknown names are errors.
`platform` means Command on macOS or Super/Windows elsewhere. Existing semantic
`scroll {id,pixels}` now also accepts modifiers; its downward-positive `pixels`
convention remains unchanged. Raw `wheel` deltas are passed through **without**
sign conversion. Existing click/keystrokes/text/default settling are unchanged.

```json
{"id":20,"method":"act","params":{"session":"s1","type":"pointer_down","x":137,"y":219,"button":"left","modifiers":["shift"]}}
{"id":21,"method":"act","params":{"session":"s1","type":"pointer_move","x":1200,"y":1100,"pressed_button":"left","modifiers":["shift"]}}
{"id":22,"method":"frame","params":{"session":"s1","ms":0,"path":"target/playback/brush.png"}}
{"id":23,"method":"act","params":{"session":"s1","type":"pointer_cancel"}}
```

Use actual semantic bounds from `snapshot` to choose a starting point; the
coordinates above only illustrate the protocol. Replace cancellation with
`pointer_up` at the outside coordinates to test commitment. Mouse cancellation
is the framework's platform cancellation event, not Escape or a synthesized
mouse-up, so it does not manufacture a click/drop.

Touch phases are `started`, `moved`, `ended`, `cancelled`; `touch_id` is a stable
unsigned 64-bit contact identity. The caller owns valid contact lifecycles and
sample timing. The harness sends only the supplied raw touch to GPUI's existing
recognizer, never parallel synthesized mouse/native events. Per-contact cancel
and system-wide `touch_cancel_all` are distinct. This is portable input injection,
not proof of a native touch device, pressure, prediction, or platform delivery.

`motion` now restores the selected session's scene/theme before drawing, including
after another differently themed session closes. Regression coverage compares
the actual rendered dark frame before/after this interleaving.
