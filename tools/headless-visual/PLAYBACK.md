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
