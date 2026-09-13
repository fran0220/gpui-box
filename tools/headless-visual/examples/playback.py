#!/usr/bin/env python3
"""Render exact 16ms samples of the existing spring exhibit, including interruption."""

import json
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parents[3]
output = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "target/playback").resolve()
output.mkdir(parents=True, exist_ok=True)
binary = root / "tools/headless-visual/target/debug/gpui-box-headless-visual"
with subprocess.Popen(
    [str(binary), "serve"], cwd=root, stdin=subprocess.PIPE,
    stdout=subprocess.PIPE, text=True,
) as process:
    request_id = 0

    def call(method, **params):
        global request_id
        request_id += 1
        process.stdin.write(json.dumps({"id": request_id, "method": method, "params": params}) + "\n")
        process.stdin.flush()
        reply = json.loads(process.stdout.readline())
        if not reply["ok"]:
            raise RuntimeError(reply)
        return reply["result"]

    session = call("open", scene="motion-primitives", theme="studio-light")["session"]

    def click(identity):
        call("act", session=session, type="click", id=identity)

    click("scene.motion.tabs.spring")
    click("scene.motion.spring.timeline")  # Default: already at the endpoint.
    call("motion", session=session, reduced_motion=False)
    click("scene.motion.spring.queue")
    samples = []
    for index in range(46):
        if index == 11:  # Last captured time is 160ms. Reverse while moving.
            click("scene.motion.spring.timeline")
        if index == 36:  # Last captured time is 544ms. Settle without advancing.
            call("motion", session=session, reduced_motion=True)
        frame = call("frame", session=session, ms=0 if index in (0, 11, 36) else 16,
                     path=str(output / f"frame-{index:03}.png"))
        indicator = next(node for node in frame["snapshot"]["nodes"]
                         if node["id"] == "scene.motion.spring.indicator")
        samples.append({"frame": index, "time_ms": frame["time_ms"],
                        "reduced_motion": frame["reduced_motion"],
                        "indicator": indicator["bounds"]})
    call("close", session=session)
    process.stdin.close()
    process.wait(timeout=10)
    (output / "samples.json").write_text(json.dumps(samples, indent=2) + "\n")
    print(json.dumps(samples, indent=2))
