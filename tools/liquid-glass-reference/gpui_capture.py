"""Real GPUI candidate producer; no Apple images are read by the renderer.

Build once:
  cargo build --locked --manifest-path tools/headless-visual/Cargo.toml --example glass_reference
Capture:
  python3 -B tools/liquid-glass-reference/gpui_capture.py REFERENCE OUTPUT \
    --executable tools/headless-visual/target/debug/examples/glass_reference \
    --revision CHECKOUT_ID [--parameters OPTIONS.json]

OPTIONS is a bounded JSON object of optional production Glass/GlassGroup setters:
blur [0,64], thickness [0,64], refractive_index [1,2.5], backdrop_depth [0,128].
Trial-only Liquid theme overrides: regular_blur [0,64], regular_saturation [0,3],
regular_wash [0,1], regular_gain [0,2], regular_lift [0,1]; never affect Clear.
regular_hairline [0,4] and regular_specular [0,1] isolate Regular's rim response.
regular_refraction [0,1], regular_thickness [0,64], regular_backdrop_depth [0,128]
and regular_refractive_index [1,2.5] isolate its dielectric geometry.
blur takes precedence over regular_blur. protect_text is numeric 0 or 1 (default 1).
Omitted fields retain production theme/preset values. No shader fitting occurs.
--smoke instead of REFERENCE captures four static/control frames without native
evidence; it writes smoke.json, NEVER candidate.json. Transition frames replay
one persistent menu surface's linear resize using the GPUI simulated clock.
Native capture brackets are not presentation timestamps or dynamics equivalence.
"""
import argparse
import json
import math
from pathlib import Path
import subprocess

from compare import pair
from validate import digest, png_size, require, validate

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
LIMITS = {"blur": (0, 64), "thickness": (0, 64),
          "refractive_index": (1, 2.5), "backdrop_depth": (0, 128),
          "regular_blur": (0, 64), "regular_saturation": (0, 3),
          "regular_wash": (0, 1), "regular_gain": (0, 2),
          "regular_lift": (0, 1), "regular_hairline": (0, 4),
          "regular_specular": (0, 1), "protect_text": (0, 1),
          "regular_refraction": (0, 1), "regular_thickness": (0, 64),
          "regular_backdrop_depth": (0, 128), "regular_refractive_index": (1, 2.5)}


def load(path):
    def pairs(items):
        result = {}
        for k, v in items:
            require(k not in result, "duplicate JSON key")
            result[k] = v
        return result
    return json.loads(Path(path).read_text(), object_pairs_hook=pairs,
                      parse_constant=lambda _: require(False, "nonfinite JSON"))


def number(v):
    return type(v) in (int, float) and math.isfinite(v)


def parameters(value):
    require(isinstance(value, dict) and set(value) <= set(LIMITS), "unknown material option")
    for k, v in value.items():
        lo, hi = LIMITS[k]
        require(number(v) and lo <= v <= hi, "material option out of bounds: " + k)
        require(k != "protect_text" or v in (0, 1), "protect_text must be numeric 0 or 1")
    return value


def key(f):
    require(f.get("appearance") in ("light", "dark"), "unsupported appearance")
    require(f.get("phase") in ("background", "static", "transition"), "unsupported phase/input case")
    require(type(f.get("index")) is int and f["index"] >= 0, "invalid frame index")
    return f["appearance"], f["phase"], f["index"]


def plan(reference, fixture, options):
    require(reference["logical_size"] == [960, 640] and type(reference["scale"]) is int
            and reference["scale"] in (1, 2), "unsupported dimensions")
    require(fixture == load(HERE / "fixture.json"), "unsupported fixture")
    frames, seen = [], set()
    for f in reference["frames"]:
        k = key(f)
        require(k not in seen, "duplicate reference key")
        seen.add(k)
        frame = dict(zip(("appearance", "phase", "index"), k))
        if f["phase"] == "transition":
            require(all(number(f.get(n)) for n in ("capture_start", "capture_end", "trigger_time")), "invalid capture interval")
            lo, hi = (f[n] - f["trigger_time"] for n in ("capture_start", "capture_end"))
            require(0 <= lo < hi <= 120, "invalid capture interval")
            frame["requested_sample_time"] = lo + (hi-lo)/2
            frame["reference_interval"] = [lo, hi]
        frames.append(frame)
    require(0 < len(frames) <= 512, "invalid frame count")
    return {"schema": 1, "scale": reference["scale"], "fixture": fixture,
            "parameters": parameters(options), "frames": frames}


def output_file(root, name, seen):
    require(isinstance(name, str) and name not in ("", ".", "..")
            and Path(name).name == name and "\\" not in name and name not in seen,
            "unsafe or duplicate output name")
    seen.add(name)
    path = root / name
    require(path.is_file() and not path.is_symlink(), "missing or symlink output")
    return path


def collect(root, request, report):
    require(report.get("schema") == 1 and report.get("renderer") in
            ("native-metal", "wgpu-software-fallback"), "unknown renderer")
    require(report.get("clock") == "GPUI TestDispatcher; measured executor now at draw completion", "unknown clock")
    wanted = {key(f): f for f in request["frames"]}
    seen, names, result = set(), set(), []
    resize_pixels = []
    w, h = [v*request["scale"] for v in (960, 640)]
    for f in report["frames"]:
        k = key(f)
        require(k in wanted and k not in seen, "unexpected or duplicate returned frame")
        seen.add(k)
        require(f["pixel_size"] == [w, h], "renderer dimension mismatch")
        actual = f.get("sample_time_after_trigger")
        require(number(actual) and actual >= 0, "invalid actual sample time")
        if k[1] == "transition":
            p = wanted[k]
            lo, hi = p["reference_interval"]
            require(lo <= actual <= hi and abs(actual-p["requested_sample_time"]) <= 1e-8,
                    "actual simulation timestamp mismatch")
            require(f.get("transition_status") == "persistent-surface-resize", "unverified transition support claim")
            geometry = f.get("resize_geometry", {})
            require(isinstance(geometry, dict) and geometry.get("identity") == "menu"
                    and geometry.get("motion") == "Animator linear 0.8s; independent compact-state replay",
                    "unknown resize motion/identity")
            measured_time = geometry.get("sample_time_after_trigger")
            require(number(measured_time) and abs(measured_time-actual) <= 1e-8,
                    "stale geometry timestamp")
            bounds = geometry.get("bounds")
            progress = min(actual / .8, 1)
            expected = [64, 448, 144 + 128*progress, 48 + 80*progress]
            # Prepaint reports layout rounded to physical pixels, not the
            # unrounded interpolated request (e.g. 156.8pt becomes 157 at 1x).
            require(isinstance(bounds, list) and len(bounds) == 4
                    and all(number(v) and abs(v-e) <= .51/request["scale"] for v, e in zip(bounds, expected)),
                    "measured resize bounds mismatch")
            require(geometry.get("phase") == ("settled" if actual >= .8 else "resizing"),
                    "resize phase mismatch")
        else:
            require(actual == 0 and f.get("transition_status") == "not-applicable", "unexpected static clock/state")
        raw = output_file(root, f["raw_file"], names)
        png = output_file(root, f["file"], names)
        pixels = raw.read_bytes()
        require(len(pixels) == w*h*3, "truncated RGB output")
        require(png_size(png.read_bytes()) == (w, h), "PNG dimension mismatch")
        if k[1] in ("static", "transition"):
            # Compare only the resizing region, so an unrelated animated pixel
            # cannot turn an unchanged menu into evidence of completion.
            scale = request["scale"]
            roi = b"".join(pixels[(y*w+64*scale)*3:(y*w+336*scale)*3]
                           for y in range(448*scale, 576*scale))
            bounds = f["resize_geometry"]["bounds"] if k[1] == "transition" else [64, 448, 144, 48]
            for appearance, other_bounds, other_roi in resize_pixels:
                if appearance == k[0] and max(abs(a-b) for a, b in zip(bounds, other_bounds)) > .5:
                    require(roi != other_roi, "static resize pixels despite changed geometry")
            resize_pixels.append((k[0], bounds, roi))
        ppm = root / (raw.stem + ".ppm")
        require(not ppm.exists() and ppm.name not in names, "duplicate sidecar")
        names.add(ppm.name)
        ppm.write_bytes(f"P6\n{w} {h}\n255\n".encode() + pixels)
        result.append({**{n: f[n] for n in ("appearance", "phase", "index", "pixel_size",
                       "sample_time_after_trigger", "transition_status")},
                       "resize_geometry": f.get("resize_geometry"),
                       "file": png.name, "file_sha256": digest(png),
                       "raw_file": raw.name, "raw_file_sha256": digest(raw),
                       "rgb_file": ppm.name, "rgb_file_sha256": digest(ppm)})
    require(seen == set(wanted), "missing returned frames")
    return result


def masks(fixture):
    """Analytic intended bounds, NOT inferred optical contours or glyph masks."""
    shapes, exclusions = [], []
    for p in fixture["pills"]:
        x, y, w, h = p["rect"]
        shapes.append({"id": p["id"], "rect": p["rect"], "radius": h/2})
        exclusions.append([x, y+h/2-13, w, 26])
    for name in ("near", "far"):
        for i, r in enumerate(fixture["fusion"][name]):
            x, y, w, h = r
            shapes.append({"id": f"{name}-{i}", "rect": r, "radius": 20})
            exclusions.append([x, y+h/2-14, w, 28])
    shapes.append({"id": "menu-initial", "rect": fixture["transition"]["button"], "radius": 24})
    exclusions.extend([[64, 458, 144, 28], [0, 594, 960, 28]])
    return {"coordinates": "logical points, top-left", "material_shapes": shapes,
            "foreground_exclusion_rects": exclusions,
            "policy": "Conservative horizontal text bands; erode material bounds before interior fitting. Edge/outset and fusion-bridge evaluation is separate. These are not rasterized optical masks.",
            "typography": "bundled Geist / Geist Mono, not Apple system font; no typography equality claimed"}


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n")


def capture(reference_dir, output, executable, revision, options=None, smoke_scale=1):
    require(isinstance(revision, str) and revision.strip() not in ("", "unknown"), "explicit revision required")
    executable = Path(executable).resolve()
    require(executable.is_file(), "build the Rust example first")
    options = parameters(options if options is not None else {})
    fixture = load(HERE / "fixture.json")
    if reference_dir is not None:
        reference_dir = Path(reference_dir)
        reference = load(reference_dir / "manifest.json")
        validate(reference_dir, reference)  # No bypass for unvalidated native evidence.
        require(digest(reference_dir / "fixture.json") == digest(HERE / "fixture.json"), "fixture hash mismatch")
    else:
        reference = {"logical_size": [960, 640], "scale": smoke_scale,
                     "frames": [{"appearance": a, "phase": p, "index": 0}
                                for a in ("light", "dark") for p in ("background", "static")]}
    request = plan(reference, fixture, options)
    output = Path(output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    write(output / "parameters.json", options)
    write(output / "request.json", request)
    command = [str(executable), str(output / "request.json"), str(output)]
    with (output / "renderer.log").open("w") as log:
        subprocess.run(command, check=True, timeout=600, stdout=log, stderr=subprocess.STDOUT)
    report = load(output / "render.json")
    frames = collect(output, request, report)
    sources = [HERE / "gpui_capture.py", REPO / "tools/headless-visual/examples/glass_reference.rs",
               REPO / "tools/headless-visual/Cargo.lock", REPO / "crates/gpui-kit/src/overlay/glass.rs",
               REPO / "crates/gpui-kit/src/motion/animator.rs"]
    sources += sorted((REPO / "crates").glob("gpui*/src/**/*.wgsl"))
    sources += sorted((REPO / "crates").glob("gpui*/src/**/*.metal"))
    sources += sorted((REPO / "crates").glob("gpui*/src/**/*.hlsl"))
    sources += [REPO / p for p in (
        "crates/gpui/src/scene.rs", "crates/gpui_macos/src/metal_renderer.rs",
        "crates/gpui_windows/src/directx_renderer.rs", "crates/gpui_wgpu/src/wgpu_renderer.rs")]
    sources += sorted((REPO / "crates/gpui-kit-tokens/tokens").glob("*.json"))
    sources += sorted((HERE / "color-resolution").glob("*"))
    candidate = {"schema": 1, "producer": "gpui", "revision": revision,
                 "renderer": report["renderer"], "parameters_sha256": digest(output / "parameters.json"),
                 "fixture_sha256": digest(HERE / "fixture.json"), "logical_size": [960, 640],
                 "scale": request["scale"], "frames": frames,
                 "provenance": {"executable_sha256": digest(executable), "command": command,
                    "source_sha256": {str(p.relative_to(REPO)): digest(p) for p in sources},
                    "request_sha256": digest(output / "request.json"), "render_report_sha256": digest(output / "render.json"),
                    "reference_manifest_sha256": digest(reference_dir / "manifest.json") if reference_dir else None,
                    "revision_authority": "caller supplied; executable and current sources independently hashed, no Git actions",
                    "color_space": "renderer sRGB RGB readback; PPM P6/255, no CPU glass, no image resampling",
                    "themes": ["studio-light", "studio-dark"], "reduce_motion": False},
                 "coverage": {"static": "production Glass and GlassGroup; uncalibrated",
                    "transition": "Persistent menu surface resize, GPUI Animator linear 0.8s; actual prepaint bounds and simulated draw time. Each sample replays from compact state. Native capture brackets are not presentation timestamps; no Apple dynamics equivalence or cross-view matched geometry claim.",
                    "pointer": "not dispatched; no interactive-response claim",
                    "fusion": "32pt production merge smoothing is not proven equivalent to Apple's container spacing",
                    "tint": "Color.orange input resolved publicly on macOS 27.0/26A5416b: light #FF8D28, dark #FF9230; separate default-environment experiment, not in-window introspection or a universal colour promise",
                    "foreground_and_material_masks": masks(fixture)},
                 "calibration": "not performed", "visual_review": "required"}
    if reference_dir:
        candidate["pairing"] = pair(reference, candidate, output)
        write(output / "candidate.json", candidate)
    else:
        candidate["status"] = "smoke-only; no native reference validated"
        write(output / "smoke.json", candidate)
    return candidate


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("reference", nargs="?", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--smoke", action="store_true")
    parser.add_argument("--scale", type=int, choices=(1, 2), default=1, help="smoke only; reference owns scale otherwise")
    parser.add_argument("--executable", required=True, type=Path)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--parameters", type=Path)
    args = parser.parse_args()
    require(args.smoke == (args.reference is None), "supply REFERENCE or --smoke, not both")
    capture(args.reference, args.output, args.executable, args.revision,
            load(args.parameters) if args.parameters else {}, args.scale)


if __name__ == "__main__":
    main()
