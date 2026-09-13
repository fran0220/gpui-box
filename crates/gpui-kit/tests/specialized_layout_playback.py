#!/usr/bin/env python3
"""Real offscreen keyed flow/matrix lifecycle and raw pointer regression."""
import hashlib
import json
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parents[3]
output = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else root / "target/specialized-layout-playback").resolve()
output.mkdir(parents=True, exist_ok=True)
samples = []
for theme in ["studio-light", "studio-dark"]:
    for scene, prefix, identity in [
        ("heatmap-reordering", "scene.heat.reorder", "scene.heat.reorder.cell.east-a"),
        ("sankey-motion", "scene.sankey.motion", "scene.sankey.motion.plot.mark.node.a"),
    ]:
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
            session = call("open", scene=scene, theme=theme)["session"]
            def frame(phase, ms):
                path = output / f"{scene}-{theme}-{phase}.png"
                result = call("frame", session=session, ms=ms, path=str(path))
                sample = dict(scene=scene, theme=theme, phase=phase, time=result["time_ms"], snapshot=result["snapshot"], sha256=hashlib.sha256(path.read_bytes()).hexdigest())
                samples.append(sample)
                (output / "samples.json").write_text(json.dumps(samples, indent=2) + "\n")
                return {n["id"]: n for n in result["snapshot"]["nodes"]}
            def click(suffix):
                call("act", session=session, type="click", id=prefix + "." + suffix)
            def same_box(a, b):
                # Flex rounding can redistribute one physical pixel (2× capture).
                return all(abs(a[k] - b[k]) <= .5 for k in ["x", "y", "width", "height"])
            first = frame("initial", 0)
            assert identity in first, list(first)
            call("motion", session=session, reduced_motion=False)
            click("advance")
            start = frame("start", 0)
            assert same_box(start[identity]["bounds"], first[identity]["bounds"])
            if scene == "heatmap-reordering":
                assert "scene.heat.reorder.cell.east-d" in start
            middle = frame("middle", 80)
            assert middle[identity]["bounds"] != start[identity]["bounds"]
            if scene == "heatmap-reordering":
                assert middle[identity]["bounds"]["width"] < first[identity]["bounds"]["width"]
            b = middle[identity]["bounds"]
            x, y = b["x"] + b["width"] * .5, b["y"] + b["height"] * .5
            call("act", session=session, type="pointer_move", x=x, y=y)
            frame("hover", 0)
            call("act", session=session, type="pointer_down", x=x, y=y, button="left")
            call("act", session=session, type="pointer_up", x=x, y=y, button="left")
            picked = frame("picked", 0)
            assert picked[identity].get("selected"), picked[identity]
            click("advance")
            retarget = frame("retarget", 0)
            assert same_box(retarget[identity]["bounds"], middle[identity]["bounds"])
            assert not any((".cell.west-" in id or ".mark.node.b" in id or ".mark.link.bc" in id) for id in retarget)
            if scene == "heatmap-reordering":
                assert "scene.heat.reorder.cell.east-d" not in retarget
                assert "scene.heat.reorder.cell.east-c" not in retarget
            frame("exit-middle", 80)
            click("advance")
            frame("reinsert", 0)
            frame("reinsert-middle", 80)
            click("direct")
            direct = frame("direct", 0)
            click("direct")
            assert frame("reenabled", 0)[identity]["bounds"] == direct[identity]["bounds"]
            click("advance")
            frame("next-start", 0)
            call("motion", session=session, reduced_motion=True)
            frame("reduced", 0)
            call("close", session=session)
            process.stdin.close()
            process.wait(timeout=10)
(output / "samples.json").write_text(json.dumps(samples, indent=2) + "\n")
print(f"PASS: {len(samples)} actual keyed flow/matrix frames; current pointer selection, continuous retarget, noninteractive exits, direct/reduced motion in both themes")
