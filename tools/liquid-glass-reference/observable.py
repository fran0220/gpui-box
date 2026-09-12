"""Forward native/GPUI observable comparison, never phase-coordinate inference.

capture NATIVE_RUN NATIVE_SOURCE OUTPUT --executable EXE --revision ID
        [--group GROUP ...] [--parameters JSON]
compare NATIVE_RUN CANDIDATE OUTPUT

Groups are always complete (eight fit phases plus two held-outs). Every run
also includes the four flat before/after controls. Without --group, all groups
are captured. OUTPUT must be fresh. Only compare writes baseline error reports.
Requires NumPy and Pillow. No native acquisition or material fitting occurs.
"""
import argparse
from collections import defaultdict
import math
from pathlib import Path
import subprocess
import uuid

import numpy as np
from PIL import Image, ImageDraw, __version__ as pillow_version

from gpui_capture import HERE, REPO, load, parameters, write
from validate import digest, require

CAPS = {"small": [148, 100, 144, 40], "large": [296, 239, 208, 72]}
PHASES = [i * math.pi / 4 for i in range(8)] + [math.pi / 8, 9 * math.pi / 8]
FIELDS = {"id", "group", "appearance", "axis", "period", "mean", "amplitude",
          "index", "phase", "kind", "rest", "phase_role", "pattern"}
ACTIVE = dict(window_is_key=True, window_is_main=True, app_is_active=True)
COMPLETE = "complete; exact encoded backgrounds and repeats verified; uncalibrated"


def safe_file(root, name):
    require(isinstance(name, str) and name not in ("", ".", "..")
            and Path(name).name == name and "\\" not in name, "unsafe filename")
    path = root / name
    require(path.is_file() and not path.is_symlink(), "missing/symlink file: " + name)
    return path


def image(path):
    with Image.open(path) as im:
        require(im.size == (1280, 800) and im.mode in ("RGB", "RGBA"), "invalid image extent/mode")
        return np.asarray(im.convert("RGB"))


def validate_case(c):
    require(isinstance(c, dict) and set(c) == FIELDS, "unknown/missing native case field")
    require(c["appearance"] in ("light", "dark") and c["axis"] in ("x", "y"), "invalid appearance/axis")
    require(type(c["period"]) is int and c["period"] in (32, 64, 128), "invalid period")
    require(type(c["index"]) is int, "invalid index")
    require(all(type(c[k]) in (int, float) and math.isfinite(c[k]) for k in ("mean", "amplitude", "phase")), "invalid scalar")
    group = f'{c["appearance"]}-{c["axis"]}-p{c["period"]}-m{round(c["mean"]*100):02}-a{round(c["amplitude"]*100):02}'
    require(c["group"] == group, "group mismatch")
    if c["kind"] == "flat":
        require((c["axis"], c["period"], c["mean"], c["amplitude"], c["index"], c["phase"], c["phase_role"])
                == ("x", 64, .5, 0, 0, 0, "flat") and c["rest"] in ("before", "after"), "invalid flat")
        expected_id = f'{c["appearance"]}-flat-{c["rest"]}'
    else:
        require(0 <= c["index"] < 10 and c["rest"] is None, "invalid phase index")
        require(abs(c["phase"] - PHASES[c["index"]]) < 1e-14
                and c["phase_role"] == ("fit" if c["index"] < 8 else "heldout"), "phase/split mismatch")
        require((c["kind"] == "primary" and c["mean"] == .5 and c["amplitude"] in (.06, .12))
                or (c["kind"] == "mean-control" and c["mean"] in (.35, .65)
                    and c["amplitude"] == .06 and c["period"] == 64), "invalid operating point")
        expected_id = f'{group}-{c["index"]}'
    require(c["id"] == expected_id and c["pattern"] == expected_id + "-pattern.png", "case filename mismatch")


def codes(c):
    length = 1280 if c["axis"] == "x" else 800
    return [round(255 * (c["mean"] + c["amplitude"] * math.cos(
        2 * math.pi * ((i + .5) / 2) / c["period"] + c["phase"]))) for i in range(length)]


def pattern(c):
    stripe = np.asarray(codes(c), dtype=np.uint8)
    gray = np.broadcast_to(stripe[None, :] if c["axis"] == "x" else stripe[:, None], (800, 1280))
    rgb = np.repeat(gray[:, :, None], 3, axis=2)
    for (x, y), color in zip([(16, 16), (1232, 16), (16, 752)], [(255, 0, 0), (0, 255, 0), (0, 0, 255)]):
        rgb[y:y+32, x:x+32] = color
    return rgb


def complete_cases(cases):
    require(0 < len(cases) <= 324, "invalid case count")
    ids, groups = set(), defaultdict(set)
    for c in cases:
        validate_case(c)
        require(c["id"] not in ids, "duplicate case")
        ids.add(c["id"])
        if c["kind"] != "flat":
            groups[c["group"]].add(c["index"])
    require(all(indices == set(range(10)) for indices in groups.values()), "partial phase group")


def native(root, source=None):
    """Validate all native identities/brackets; do not trust an earlier audit."""
    root = Path(root)
    manifest = load(root / "manifest.json")
    fixture = load(root / "fixture.json")
    require(manifest["schema"] == "native-regular-low-amplitude-evidence-3"
            and fixture["schema"] == "native-regular-low-amplitude-fixture-3", "wrong native schema")
    require(manifest["logical_size"] == fixture["size"] == [640, 400]
            and manifest["scale"] == fixture["scale"] == 2
            and manifest["layout"] == fixture["capsules"] == CAPS, "wrong native geometry")
    require(fixture["phases_fit"] == PHASES[:8] and fixture["phases_heldout"] == PHASES[8:], "wrong phase split")
    require(manifest["backend"] == "ScreenCaptureKit.SCScreenshotManager.desktopIndependentWindow"
            and manifest["settings"] == dict(increase_contrast=False, invert_colors=False, reduce_motion=False, reduce_transparency=False), "unsupported native capture settings")
    cases = load(root / "cases.json")
    complete_cases(cases)
    require(len(cases) == 324 and len(manifest["frames"]) == 972, "partial native run")
    if source:
        require(load(Path(source) / "fixture.json") == fixture and load(Path(source) / "cases.json") == cases, "source/evidence input mismatch")
    identities = load(root / "provenance.json")["identities"]
    for name, expected in identities.items():
        relative = Path(name)
        require(not relative.is_absolute() and ".." not in relative.parts, "unsafe provenance path")
        file = root / relative
        require(file.is_file() and not file.is_symlink() and digest(file) == expected, "native source/input identity mismatch: " + name)
    expected_cases = {c["id"]: c for c in cases}
    frames, files, previous = {}, set(), -1
    for f in manifest["frames"]:
        c = f["case"]
        require(c == expected_cases.get(c["id"]), "native frame case mismatch")
        key = (c["id"], f["state"])
        require(key not in frames and f["state"] in ("background", "glass", "repeat"), "duplicate/unknown native frame")
        require(f["activity_before"] == f["activity_after"] == ACTIVE, "inactive native bracket")
        require(f["capture_end_uptime"] >= f["capture_start_uptime"] >= f["state_change_uptime"]
                and f["capture_start_uptime"] >= previous, "unordered native capture")
        previous = f["capture_end_uptime"]
        # Native retains the last measured capsule layout while hidden.
        require(f["layout"] in ({}, CAPS) if f["state"] == "background"
                else f["layout"] == CAPS, "native layout mismatch")
        for ext in ("png", "ppm"):
            require(f[ext] not in files, "duplicate native file")
            files.add(f[ext])
            require(digest(safe_file(root, f[ext])) == f[ext + "_sha256"], "native frame hash mismatch")
        frames[key] = f
    require(set(frames) == {(c["id"], state) for c in cases for state in ("background", "glass", "repeat")}, "missing native frame")
    for c in cases:
        expected = pattern(c)
        require(np.array_equal(image(root / c["pattern"]), expected)
                and np.array_equal(image(root / frames[c["id"], "background"]["ppm"]), expected), "native background mismatch")
        require(np.array_equal(image(root / frames[c["id"], "glass"]["ppm"]), image(root / frames[c["id"], "repeat"]["ppm"])), "native repeat mismatch")
    return cases, frames


def request(cases, options, groups=None):
    complete_cases(cases)
    available = {c["group"] for c in cases if c["kind"] != "flat"}
    chosen = set(groups) if groups else available
    require(chosen <= available and chosen, "unknown/empty groups")
    selected = [c for c in cases if c["kind"] == "flat" or c["group"] in chosen]
    complete_cases(selected)
    return dict(schema="gpui-regular-observable-request-1", run_id=uuid.uuid4().hex,
                logical_size=[640, 400], scale=2, capsules=CAPS, parameters=parameters(options),
                cases=[{**{k: v for k, v in c.items() if k != "pattern"}, "codes": codes(c)} for c in selected])


def contract(options):
    if not options:
        return "unmodified-production-defaults; protect_text=1"
    if options == {"protect_text": 0}:
        return "material-only-baseline; protect_text=0; all optical defaults unchanged"
    return "material-only-trial" if options.get("protect_text", 1) == 0 else "text-protected-trial"


def collect(root, requested, report):
    require(requested.get("schema") == "gpui-regular-observable-request-1"
            and requested.get("logical_size") == [640, 400]
            and requested.get("scale") == 2 and requested.get("capsules") == CAPS,
            "wrong request geometry/schema")
    require(report.get("schema") == "gpui-regular-observable-render-1"
            and report.get("renderer") in ("native-metal", "wgpu-software-fallback"), "wrong renderer schema")
    require(report.get("request_text") == (root / "request.json").read_text()
            and load(root / "request.json") == requested, "stale renderer request")
    wanted = {(c["id"], state) for c in requested["cases"] for state in ("background", "glass", "repeat")}
    seen, names, frames = set(), set(), []
    for f in report["frames"]:
        require(set(f) == {"id", "state", "file", "raw_file", "pixel_size"}, "unexpected renderer field")
        key = f["id"], f["state"]
        require(key in wanted and key not in seen and f["pixel_size"] == [1280, 800], "duplicate/unexpected frame")
        seen.add(key)
        for key in ("file", "raw_file"):
            require(f[key] not in names, "duplicate output filename")
            names.add(f[key])
            safe_file(root, f[key])
        raw = np.frombuffer((root / f["raw_file"]).read_bytes(), dtype=np.uint8)
        require(raw.size == 1280 * 800 * 3, "partial raw output")
        require(np.array_equal(raw.reshape(800, 1280, 3), image(root / f["file"])), "PNG/raw mismatch")
        frames.append({**f, "file_sha256": digest(root / f["file"]), "raw_file_sha256": digest(root / f["raw_file"])})
    require(seen == wanted, "missing returned frame")
    require({p.name for p in root.glob("case-*")} == names, "stale/extra frame outputs")
    lookup = {(f["id"], f["state"]): f for f in frames}
    for c in requested["cases"]:
        require(np.array_equal(image(root / lookup[c["id"], "background"]["file"]), pattern(c)), "GPUI background mismatch")
        require(lookup[c["id"], "glass"]["raw_file_sha256"] == lookup[c["id"], "repeat"]["raw_file_sha256"], "GPUI repeat mismatch")
    return frames


def sources():
    paths = [p for base in (REPO / "crates", REPO / "tools/headless-visual/examples", HERE)
             for p in base.rglob("*") if p.is_file() and p.suffix in (".rs", ".wgsl", ".metal", ".hlsl", ".json", ".py", ".toml")]
    paths += [REPO / p for p in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "tools/headless-visual/Cargo.toml", "tools/headless-visual/Cargo.lock", "tools/liquid-glass-reference/requirements.txt")]
    return {str(p.relative_to(REPO)): digest(p) for p in sorted(set(paths))}


def capture(native_root, source, output, executable, revision, options, groups):
    require(revision and revision != "unknown", "explicit source revision/archive required")
    cases, _ = native(native_root, source)
    requested = request(cases, options, groups)
    output = Path(output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    write(output / "request.json", requested)
    executable = Path(executable).resolve()
    before = sources()
    binary = digest(executable)
    inputs = {str(Path(source) / name): digest(Path(source) / name) for name in ("README.md", "METHODS.md", "fixture.json", "cases.json", "Phase.swift", "run.py")}
    inputs.update({str(Path(native_root) / name): digest(Path(native_root) / name) for name in ("manifest.json", "provenance.json", "cases.json", "fixture.json")})
    command = [str(executable), "--observable", str(output / "request.json"), str(output)]
    with (output / "renderer.log").open("w") as log:
        subprocess.run(command, check=True, timeout=1200, stdout=log, stderr=subprocess.STDOUT)
    require(sources() == before and digest(executable) == binary, "sources/executable changed during capture")
    require(all(digest(Path(p)) == h for p, h in inputs.items()), "native inputs changed during capture")
    report = load(output / "render.json")
    frames = collect(output, requested, report)
    candidate = dict(schema="gpui-regular-observable-candidate-1", revision=revision,
        renderer=report["renderer"], parameters=requested["parameters"], baseline=not requested["parameters"],
        material_contract=contract(requested["parameters"]),
        request_sha256=digest(output / "request.json"), render_sha256=digest(output / "render.json"), frames=frames,
        native_manifest_sha256=digest(Path(native_root) / "manifest.json"),
        provenance=dict(command=command, executable_sha256=binary, source_sha256=before, input_sha256=inputs,
            python=__import__("sys").version, numpy=np.__version__, pillow=pillow_version, reduce_motion=True,
            material="production Liquid/Regular preset; no labels, no tint, no interaction; omitted trial setters unchanged"),
        status=COMPLETE)
    write(output / "candidate.json", candidate)  # Completion marker is written last.
    return candidate


def geometry(name):
    x, y, w, h = CAPS[name]
    box = (2*(x-8), 2*(y-8), 2*(x+w+8), 2*(y+h+8))
    xx = (np.arange(box[0], box[2]) + .5) / 2 - (x + w/2)
    yy = (np.arange(box[1], box[3]) + .5) / 2 - (y + h/2)
    inward = h/2 - np.hypot(np.maximum(np.abs(xx)[None, :] - (w-h)/2, 0), yy[:, None])
    rim = (inward >= 0) & (inward < 3)
    masks = dict(interior=inward >= 3, rim_all_0_3pt=rim,
        rim_top_0_3pt=rim & (yy[:, None] < 0) & (np.abs(xx)[None, :] < 16),
        rim_bottom_0_3pt=rim & (yy[:, None] > 0) & (np.abs(xx)[None, :] < 16),
        rim_left_0_3pt=rim & (xx[None, :] < 0) & (np.abs(yy)[:, None] < 2),
        rim_right_0_3pt=rim & (xx[None, :] > 0) & (np.abs(yy)[:, None] < 2))
    return box, inward, masks


def metric(error, mask):
    require(error.ndim == 3 and error.shape[2] == 3 and error.shape[:2] == mask.shape
            and mask.dtype == bool and mask.any() and np.isfinite(error).all(), "invalid metric inputs")
    values = error[mask].astype(np.float64)
    gray = values.mean(axis=1)
    return dict(pixels=len(values), sse_rgb_codes=float(np.square(values).sum()),
        mae_rgb_codes=float(np.abs(values).mean()), rmse_rgb_codes=float(np.sqrt(np.square(values).mean())),
        bias_rgb_codes=values.mean(axis=0).tolist(), max_abs_rgb_codes=float(np.abs(values).max()),
        gray_rmse_codes=float(np.sqrt(np.square(gray).mean())), gray_mae_codes=float(np.abs(gray).mean()),
        gray_abs_p95_codes=float(np.percentile(np.abs(gray), 95)))


def split_loss(records, split, region):
    chosen = [r for r in records if r["phase_role"] == split and r["region"] == region]
    require(chosen, "empty split/region")
    count = sum(r["pixels"] * 3 for r in chosen)
    return dict(frames=len(chosen), channel_samples=count,
                rmse_rgb_codes=math.sqrt(sum(r["sse_rgb_codes"] for r in chosen) / count))


def training_objective(records, region="interior"):
    """Only the eight designated fit phases can contribute to this objective."""
    return split_loss(records, "fit", region)


def compare(native_root, candidate_root, output):
    native_root, candidate_root, output = map(Path, (native_root, candidate_root, output))
    cases, native_frames = native(native_root)
    candidate = load(candidate_root / "candidate.json")
    require(candidate["schema"] == "gpui-regular-observable-candidate-1"
            and candidate.get("status") == COMPLETE, "incomplete candidate")
    require(digest(native_root / "manifest.json") == candidate["native_manifest_sha256"], "stale native pairing")
    require(digest(candidate_root / "request.json") == candidate["request_sha256"]
            and digest(candidate_root / "render.json") == candidate["render_sha256"], "stale request/report")
    requested = load(candidate_root / "request.json")
    require(candidate["material_contract"] == contract(requested["parameters"]), "material contract mismatch")
    selected = [{**{k: v for k, v in c.items() if k != "codes"}, "pattern": c["id"] + "-pattern.png"}
                for c in requested["cases"]]
    complete_cases(selected)
    render = load(candidate_root / "render.json")
    require(candidate["renderer"] == render["renderer"], "candidate renderer mismatch")
    frames = collect(candidate_root, requested, render)
    require(frames == candidate["frames"] and candidate["parameters"] == requested["parameters"]
            and candidate["baseline"] == (not requested["parameters"]), "candidate output identity mismatch")
    known = {c["id"]: c for c in cases}
    for c in requested["cases"]:
        require(c == {**{k: v for k, v in known[c["id"]].items() if k != "pattern"}, "codes": codes(known[c["id"]])}, "candidate/native case mismatch")
    output.mkdir(parents=True, exist_ok=False)
    lookup = {(f["id"], f["state"]): f for f in frames}
    records, raw = [], {}
    for name in CAPS:
        box, inward, masks = geometry(name)
        np.savez_compressed(output / f"{name}-masks.npz", box=box, inward_points=inward, **masks)
        raw[f"{name}-masks.npz"] = digest(output / f"{name}-masks.npz")
    for index, c in enumerate(requested["cases"]):
        actual = image(candidate_root / lookup[c["id"], "glass"]["file"])
        reference = image(native_root / native_frames[c["id"], "glass"]["ppm"])
        for name in CAPS:
            box, _, masks = geometry(name)
            x0, y0, x1, y1 = box
            a, b = actual[y0:y1, x0:x1], reference[y0:y1, x0:x1]
            error = a.astype(np.int16) - b.astype(np.int16)
            file = f'{c["id"]}-{name}.npz'
            np.savez_compressed(output / file, native_rgb=b, gpui_rgb=a, error_rgb_codes=error)
            raw[file] = digest(output / file)
            for region, mask in masks.items():
                records.append({**{k: v for k, v in c.items() if k != "codes"}, "size": name,
                                "region": region, **metric(error, mask)})
        # Deterministic representative held-out; no cherry-picked error mask.
        if index == next(i for i, item in enumerate(requested["cases"]) if item["phase_role"] == "heldout"):
            review(actual, reference, c["id"], output / "comparison.png")
    groups = defaultdict(list)
    for r in records:
        groups[r["group"], r["size"], r["region"], r["phase_role"]].append(r)
    summary = [{"group": group, "size": size, "region": region, "split": split,
                **split_loss(rows, split, region)} for (group, size, region, split), rows in sorted(groups.items())]
    report = dict(schema="gpui-regular-observable-comparison-1", baseline=candidate["baseline"],
        material_contract=candidate["material_contract"],
        parameters=candidate["parameters"], renderer=candidate["renderer"], case_count=len(requested["cases"]),
        coverage="explicit complete groups only; no claim for omitted groups or platforms",
        interpretation="encoded 8-bit sRGB codes; signed error GPUI minus native; no fitting, ray-map inference, region rejection, thresholds or equivalence verdict",
        training_objective=training_objective(records), heldout=split_loss(records, "heldout", "interior"),
        summary=summary, cases=records, raw_sha256=raw,
        provenance=dict(candidate_sha256=digest(candidate_root / "candidate.json"), native_manifest_sha256=digest(native_root / "manifest.json"),
                        comparison_source_sha256=digest(Path(__file__)), comparison_image_sha256=digest(output / "comparison.png")))
    write(output / "report.json", report)
    return report


def review(actual, reference, case_id, path):
    rows = []
    for name in CAPS:
        (x0, y0, x1, y1), _, masks = geometry(name)
        a, b = actual[y0:y1, x0:x1], reference[y0:y1, x0:x1]
        # Fixed display-only gain, never used in numerical errors.
        error = np.abs(a.astype(np.int16) - b.astype(np.int16)).max(axis=2)
        heat = np.zeros_like(a)
        heat[:, :, 0] = np.minimum(error * 4, 255).astype(np.uint8)
        heat[:, :, 2] = np.where(masks["rim_all_0_3pt"], 80, 0)
        rows.append((name, b, a, heat))
    width = max(r[1].shape[1] for r in rows)
    height = sum(r[1].shape[0] + 40 for r in rows) + 50
    canvas = Image.new("RGB", (width * 3, height), "#eeeeee")
    draw = ImageDraw.Draw(canvas)
    draw.text((8, 8), case_id + " | native / GPUI defaults or trial / abs RGB max error x4 (blue rim mask)", fill="black")
    y = 50
    for name, b, a, heat in rows:
        for col, pixels in enumerate((b, a, heat)):
            canvas.paste(Image.fromarray(pixels), (col * width, y + 20))
        draw.text((8, y), name, fill="black")
        y += b.shape[0] + 40
    canvas.save(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subs = parser.add_subparsers(dest="command", required=True)
    cap = subs.add_parser("capture")
    for key in ("native", "source", "output"): cap.add_argument(key, type=Path)
    cap.add_argument("--executable", required=True, type=Path)
    cap.add_argument("--revision", required=True)
    cap.add_argument("--parameters", type=Path)
    cap.add_argument("--group", action="append")
    cmp = subs.add_parser("compare")
    for key in ("native", "candidate", "output"): cmp.add_argument(key, type=Path)
    args = parser.parse_args()
    if args.command == "capture":
        capture(args.native, args.source, args.output, args.executable, args.revision, load(args.parameters) if args.parameters else {}, args.group)
    else:
        report = compare(args.native, args.candidate, args.output)
        print({k: report[k] for k in ("case_count", "baseline", "training_objective", "heldout")})


if __name__ == "__main__":
    main()
