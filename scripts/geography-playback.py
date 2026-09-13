#!/usr/bin/env python3
"""Verify geography's exact offscreen camera/color playback in both themes.

Requires the checkout headless `motion`/`frame` protocol and ImageMagick.
Samples are simulated-time renderer evidence, not native display/FPS evidence.
"""
import json
import pathlib
import subprocess
import sys
import time

root = pathlib.Path(__file__).resolve().parents[1]
output = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "target/geography-playback").resolve()
output.mkdir(parents=True, exist_ok=True)
binary = root / "tools/headless-visual/target/debug/gpui-box-headless-visual"

for theme in ("studio-dark", "studio-light"):
    with subprocess.Popen([str(binary), "serve"], cwd=root, stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, text=True) as process:
        request = 0

        def call(method, **params):
            global request
            request += 1
            process.stdin.write(json.dumps(dict(id=request, method=method, params=params)) + "\n")
            process.stdin.flush()
            reply = json.loads(process.stdout.readline())
            if not reply["ok"]:
                raise RuntimeError(reply)
            return reply["result"]

        opened = call("open", scene="geography", theme=theme)
        session = opened["session"]
        scale = opened["viewport"]["scale_factor"]
        samples = []

        def click(target):
            call("act", session=session, type="click", id="scene.geography." + target)

        def sample(stage, ms=0):
            path = output / f"{theme}-{stage}.png"
            frame = call("frame", session=session, ms=ms, path=str(path))
            nodes = {node["id"]: node for node in frame["snapshot"]["nodes"]}
            shape = nodes.get("scene.geography.ready.geometry.lagoon", {}).get("bounds")
            probe = samples[0]["bounds"] if stage.startswith("geometry") else shape
            # Interior of the original synthetic lagoon, away from its hole/edges.
            x = round((probe["x"] + probe["width"] * 0.3) * scale)
            y = round((probe["y"] + probe["height"] * 0.8) * scale)
            rgb = list(subprocess.check_output([
                "magick", str(path), "-crop", f"1x1+{x}+{y}", "-depth", "8", "rgb:-"
            ]))
            result = dict(stage=stage, time_ms=frame["time_ms"],
                          reduced_motion=frame["reduced_motion"], bounds=shape, rgb=rgb,
                          geometry_ids=[key for key in nodes if ".ready.geometry." in key],
                          value=nodes["scene.geography.ready.feature.lagoon"]["value"],
                          camera=nodes["scene.geography.ready.map"]["value"], path=str(path))
            samples.append(result)
            return result

        start = sample("start")
        call("motion", session=session, reduced_motion=False)
        click("camera-target")
        middle = sample("camera-middle", 60)
        assert start["bounds"]["width"] < middle["bounds"]["width"] < start["bounds"]["width"] * 1.8
        click("camera-target")
        interrupted = sample("camera-interrupted")
        assert abs(interrupted["bounds"]["width"] - middle["bounds"]["width"]) < 0.1
        sample("camera-return", 60)
        settled = sample("camera-settled", 1000)
        assert settled["bounds"] == start["bounds"]

        click("value-target")
        color_start = sample("color-start")
        color_middle = sample("color-middle", 60)
        assert color_start["value"] == color_middle["value"] == "34 samples"
        assert color_middle["rgb"] != start["rgb"]
        click("value-target")
        color_interrupted = sample("color-interrupted")
        assert color_interrupted["rgb"] == color_middle["rgb"]
        assert color_interrupted["value"] == "12 samples"
        color_settled = sample("color-settled", 1000)
        assert color_settled["rgb"] == start["rgb"]

        call("motion", session=session, reduced_motion=True)
        click("camera-target")
        click("value-target")
        reduced = sample("reduced-motion")
        assert reduced["time_ms"] == color_settled["time_ms"]
        assert reduced["camera"] == "zoom 1.8; center 0.4, 0.48"
        assert reduced["value"] == "34 samples"
        assert reduced["rgb"] not in (start["rgb"], color_middle["rgb"])
        click("camera-target")
        click("value-target")
        click("ready.feature.lagoon-sensor")
        call("motion", session=session, reduced_motion=False)
        click("geometry-target")
        geometry_start = sample("geometry-start")
        assert geometry_start["bounds"] is None
        assert not any(key.endswith(".unobserved") for key in geometry_start["geometry_ids"])
        geometry_middle = sample("geometry-middle", 60)
        assert geometry_middle["bounds"] is not None
        assert geometry_middle["rgb"] != geometry_start["rgb"]
        click("geometry-target")
        geometry_interrupted = sample("geometry-interrupted")
        assert geometry_interrupted["rgb"] == geometry_middle["rgb"]
        geometry_settled = sample("geometry-settled", 1000)
        assert geometry_settled["rgb"] == start["rgb"]
        call("motion", session=session, reduced_motion=True)
        click("geometry-target")
        geometry_reduced = sample("geometry-reduced")
        assert geometry_reduced["bounds"] is not None
        assert any(key.endswith(".arriving") for key in geometry_reduced["geometry_ids"])
        call("close", session=session)
        session = call("open", scene="geography", theme=theme)["session"]

        def raw_frame(stage):
            path = output / f"{theme}-{stage}.png"
            frame = call("frame", session=session, ms=0, path=str(path))
            (output / f"{theme}-{stage}.json").write_text(json.dumps(frame["snapshot"], indent=2))
            return {node["id"]: node for node in frame["snapshot"]["nodes"]}

        def act(kind, **params):
            return call("act", session=session, type=kind, **params)

        nodes = raw_frame("raw-start")
        camera_id = "scene.geography.ready.map"
        # Preserve G4's injected gesture independently of old Div-envelope
        # quantization versus current fractional measured-leaf bounds.
        map_bounds = nodes[camera_id]["bounds"]
        x, y = map_bounds["x"] + 191.05, map_bounds["y"] + 154.6
        initial_camera = nodes[camera_id]["value"]
        act("pointer_move", x=x, y=y)
        hover = raw_frame("raw-hover")
        assert any("12 samples" in str(node) and ".hover" in key for key, node in hover.items())
        act("pointer_down", x=x, y=y, button="left")
        act("pointer_move", x=x + 25, y=y + 15, pressed_button="left")
        dragged = raw_frame("raw-drag")
        assert dragged[camera_id]["value"] != initial_camera
        act("pointer_move", x=-100, y=-100, pressed_button="left")
        raw_frame("raw-outside")
        act("pointer_cancel")
        cancelled = raw_frame("raw-cancel")
        assert cancelled[camera_id]["value"] == initial_camera
        act("touch", touch_id=1, phase="started", x=x, y=y)
        act("touch", touch_id=2, phase="started", x=x + 40, y=y)
        act("touch", touch_id=2, phase="moved", x=x + 80, y=y)
        pinched = raw_frame("raw-pinch")
        assert pinched[camera_id]["value"] != initial_camera
        act("touch_cancel_all")
        assert raw_frame("raw-pinch-cancel")[camera_id]["value"] == initial_camera
        call("close", session=session)
        session = call("open", scene="geography-scale", theme=theme)["session"]
        click("scale-density")
        started = time.perf_counter()
        dense = raw_frame("dense")
        elapsed = time.perf_counter() - started
        geometry = [key for key in dense if key.startswith("scene.geography.scale.geometry.")]
        assert len(geometry) == 10000
        print(f"{theme}: dense renderer frame+PNG+snapshot round trip {elapsed:.3f}s; {len(geometry)} exact visible geometry targets (not GPU-only timing)")
        call("close", session=session)
        process.stdin.close()
        process.wait(timeout=10)
        (output / f"{theme}-samples.json").write_text(json.dumps(samples, indent=2) + "\n")
        print(f"{theme}: {len(samples)} exact frames; camera/color/geometry interruption and reduced motion passed")
