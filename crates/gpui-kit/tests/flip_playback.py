#!/usr/bin/env python3
"""Opt-in actual-renderer FLIP test; requires headless playback v2."""
import hashlib
import json
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parents[3]
output = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else root / "target/flip-playback").resolve()
output.mkdir(parents=True, exist_ok=True)
samples = []
for theme in ["studio-light", "studio-dark"]:
    with subprocess.Popen([str(root / "tools/headless-visual/target/debug/gpui-box-headless-visual"), "serve"], cwd=root, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True) as process:
        sequence = 0
        def call(method, **params):
            global sequence
            sequence += 1
            process.stdin.write(json.dumps(dict(id=sequence, method=method, params=params)) + "\n")
            process.stdin.flush()
            reply = json.loads(process.stdout.readline())
            assert reply["ok"], reply
            return reply["result"]
        session = call("open", scene="flip-configuration", theme=theme)["session"]
        def frame(phase, ms):
            path = output / f"{theme}-{phase}.png"
            result = call("frame", session=session, ms=ms, path=str(path))
            nodes = {n["id"]: n for n in result["snapshot"]["nodes"]}
            sample = dict(theme=theme, phase=phase, time=result["time_ms"], bounds=nodes["scene.flip.config-hit"]["bounds"], hits=nodes["scene.flip.config.hits"]["value"], sha256=hashlib.sha256(path.read_bytes()).hexdigest())
            samples.append(sample)
            return sample
        def click(name):
            call("act", session=session, type="click", id="scene.flip.config." + name)
        first = frame("initial", 0)
        call("motion", session=session, reduced_motion=False)
        click("change")
        assert frame("start", 0)["bounds"] == first["bounds"]
        assert frame("delay", 100)["bounds"] == first["bounds"]
        middle = frame("middle", 300)
        assert abs(middle["bounds"]["x"] - first["bounds"]["x"] - 90) <= 1
        assert abs(middle["bounds"]["width"] - 200) <= 1
        assert abs(middle["bounds"]["height"] - 70) <= 1
        assert first["sha256"] != middle["sha256"]
        x, y = middle["bounds"]["x"] + 5, middle["bounds"]["y"] + 5
        call("act", session=session, type="pointer_move", x=x, y=y)
        call("act", session=session, type="pointer_down", x=x, y=y, button="left")
        call("act", session=session, type="pointer_up", x=x, y=y, button="left")
        assert frame("picked-middle", 0)["hits"] == "1"
        call("act", session=session, type="pointer_down", x=-20, y=-20, button="left")
        call("act", session=session, type="pointer_cancel")
        assert frame("outside-cancel", 0)["hits"] == "1"
        click("timing")
        assert frame("retimed", 0)["bounds"] == middle["bounds"]
        frame("spring", 30)
        click("direct")
        direct = frame("direct", 0)
        assert direct["bounds"]["width"] == 270
        assert direct["bounds"]["height"] == 100
        click("direct")
        assert frame("reenabled", 0)["bounds"] == direct["bounds"]
        click("change")
        assert frame("reversed", 0)["bounds"] == direct["bounds"]
        frame("reverse-middle", 30)
        call("motion", session=session, reduced_motion=True)
        assert frame("reduced", 0)["bounds"] == first["bounds"]
        call("close", session=session)
        process.stdin.close()
        process.wait(timeout=10)
(output / "samples.json").write_text(json.dumps(samples, indent=2) + "\n")
print("PASS: 26 actual FLIP frames, delayed tween/size/position, timing continuity, displayed pointer picking, outside cancellation, spring retarget, disable/re-enable and reduced motion in both themes")
