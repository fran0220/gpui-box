#!/usr/bin/env python3
"""Opt-in actual-renderer regression; requires headless PLAYBACK.md protocol.

Build tools/headless-visual, then run this file with an optional output directory.
This does not accept baselines or assert native presentation timing/FPS.
"""
import hashlib
import itertools
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
OUTPUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ROOT / "target/specialized-playback").resolve()
OUTPUT.mkdir(parents=True, exist_ok=True)
records = []
for theme in ["studio-light", "studio-dark"]:
    for scene, button in [
        ("specialized-exploration", "scene.specialized.advance"),
        ("continuous-heatmap-transition", "scene.heat.advance"),
    ]:
        with subprocess.Popen(
            [str(ROOT / "tools/headless-visual/target/debug/gpui-box-headless-visual"), "serve"],
            cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
        ) as process:
            sequence = 0

            def call(method, **params):
                global sequence
                sequence += 1
                process.stdin.write(json.dumps(dict(id=sequence, method=method, params=params)) + "\n")
                process.stdin.flush()
                reply = json.loads(process.stdout.readline())
                assert reply["ok"], reply
                return reply["result"]

            session = call("open", scene=scene, theme=theme)["session"]

            def frame(phase, ms):
                path = OUTPUT / f"{scene}-{theme}-{phase}.png"
                result = call("frame", session=session, ms=ms, path=str(path))
                records.append(dict(scene=scene, theme=theme, phase=phase,
                                    time=result["time_ms"], snapshot=result["snapshot"],
                                    sha256=hashlib.sha256(path.read_bytes()).hexdigest()))

            def click():
                call("act", session=session, type="click", id=button)

            frame("initial", 0)
            call("motion", session=session, reduced_motion=False)
            click(); frame("update-start", 0); frame("middle", 80)
            click(); frame("retarget", 0); frame("exit-middle", 80)
            click(); frame("reinsert", 0); frame("reinsert-middle", 80)
            call("motion", session=session, reduced_motion=True)
            frame("reduced", 0)
            call("close", session=session)
            process.stdin.close()
            process.wait(timeout=10)

(OUTPUT / "samples.json").write_text(json.dumps(records, indent=2) + "\n")
for sample in records:
    nodes = sample["snapshot"]["nodes"]
    labels = [n for n in nodes if ".plot.label." in n["id"]]
    for left, right in itertools.combinations(labels, 2):
        a, b = left["bounds"], right["bounds"]
        assert (a["x"] + a["width"] <= b["x"] or b["x"] + b["width"] <= a["x"]
                or a["y"] + a["height"] <= b["y"] or b["y"] + b["height"] <= a["y"]), sample
    if sample["scene"] == "specialized-exploration" and sample["phase"] in ("retarget", "exit-middle"):
        assert not any(".funnel.plot.mark." in n["id"] or ".funnel.key." in n["id"] for n in nodes)

for theme in ["studio-light", "studio-dark"]:
    for scene in ["specialized-exploration", "continuous-heatmap-transition"]:
        phases = {r["phase"]: r for r in records if r["theme"] == theme and r["scene"] == scene}

        def node(phase, identity):
            return next(n for n in phases[phase]["snapshot"]["nodes"] if n["id"] == identity)

        assert phases["middle"]["sha256"] != phases["update-start"]["sha256"]
        assert phases["retarget"]["time"] == phases["middle"]["time"] == 80
        assert phases["reduced"]["time"] == phases["reinsert-middle"]["time"] == 240
        if scene == "specialized-exploration":
            identity = "scene.specialized.live.tree.plot.mark.parse"
            assert node("initial", identity)["bounds"] == node("update-start", identity)["bounds"]
            assert node("middle", identity)["bounds"] == node("retarget", identity)["bounds"]
            assert node("middle", identity)["bounds"] != node("initial", identity)["bounds"]
            assert node("update-start", identity)["value"] == "47"
        else:
            assert node("update-start", "scene.heat.live.cell.changing")["value"] == "32"
            assert node("retarget", "scene.heat.live.cell.changing")["value"] == "Not observed"
            for phase in phases:
                assert node(phase, "scene.heat.live.cell.zero")["value"] == "0"
print("PASS: 32 actual frames; disjoint labels, noninteractive exits, exact values, continuous retarget geometry, changed intermediate pixels, both themes")
