#!/usr/bin/env python3
"""Actual-renderer Trace hierarchy reflow; simulated time, not an FPS benchmark."""
import json
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parents[1]
output = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else root / "target/trace-reflow-playback").resolve()
output.mkdir(parents=True, exist_ok=True)
samples = []
for theme in ["studio-light", "studio-dark"]:
    with subprocess.Popen(
        [str(root / "tools/headless-visual/target/debug/gpui-box-headless-visual"), "serve"],
        cwd=root, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
    ) as process:
        serial = 0

        def call(method, **params):
            global serial
            serial += 1
            process.stdin.write(json.dumps(dict(id=serial, method=method, params=params)) + "\n")
            process.stdin.flush()
            reply = json.loads(process.stdout.readline())
            assert reply["ok"], reply
            return reply["result"]

        session = call("open", scene="trace-time", theme=theme)["session"]
        call("motion", session=session, reduced_motion=False)

        def frame(phase, ms=0):
            result = call("frame", session=session, ms=ms, path=str(output / f"{theme}-{phase}.png"))
            samples.append(dict(theme=theme, phase=phase, time_ms=result["time_ms"], snapshot=result["snapshot"]))
            rows = sorted((node for node in result["snapshot"]["nodes"]
                           if node["role"] == "tree-item" and node.get("visible")
                           and node.get("parent") == "scene.trace-time.tree"),
                          key=lambda node: node["bounds"]["y"])
            for first, second in zip(rows, rows[1:]):
                assert first["bounds"]["y"] + first["bounds"]["height"] <= second["bounds"]["y"] + 0.25, (phase, first, second)
            return {node["id"]: node for node in result["snapshot"]["nodes"]}

        def toggle():
            call("act", session=session, type="click", id="scene.trace-time.tree.request.atlas.toggle")
            call("act", session=session, type="pointer_move", x=-20, y=-20)

        sibling = "scene.trace-time.tree.request.birch"
        child = "scene.trace-time.tree.request.atlas.prepare"
        initial = frame("initial")
        before = initial[sibling]["bounds"]
        description = initial[sibling].get("description")
        toggle()
        start = frame("collapse-zero")
        assert start[sibling]["bounds"] == before
        assert child not in start
        middle = frame("collapse-middle", 32)
        assert middle[sibling]["bounds"]["y"] < before["y"]
        assert middle[sibling].get("description") == description
        toggle()
        interrupted = frame("reexpand-zero")
        assert interrupted[sibling]["bounds"] == middle[sibling]["bounds"]
        assert child not in interrupted
        frame("reexpand-middle", 32)
        completed = frame("reexpand-settled", 2000)
        assert child in completed
        assert completed[sibling]["bounds"] == before
        toggle()
        frame("second-collapse-zero")
        frame("second-collapse-middle", 32)
        published = frame("collapse-settled", 2000)
        assert "scene.trace-time.tree.request.cedar.prepare" in published
        toggle()
        frame("second-expansion-zero")
        frame("second-expansion-middle", 32)
        call("motion", session=session, reduced_motion=True)
        settled = frame("reduced-expanded")
        assert settled[sibling]["bounds"] == before
        toggle()
        collapsed = frame("reduced-collapsed")
        assert child not in collapsed
        assert collapsed[sibling]["bounds"]["y"] < middle[sibling]["bounds"]["y"]
        assert frame("reduced-stable", 100)[sibling]["bounds"] == collapsed[sibling]["bounds"]
        call("close", session=session)
        process.stdin.close()
        process.wait(timeout=10)

(output / "samples.json").write_text(json.dumps(samples, indent=2) + "\n")
print(f"PASS: {len(samples)} actual Trace frames; nonoverlapping rows, staged publication, accepted collapse, interrupted expansion, immediate semantic removal, current readout and reduced-motion settle in both themes")
