"""Bounded actual-render fitting. Freeze selection before fresh held-out validation.

Usage: observable_fit.py NATIVE SOURCE OUTPUT --executable EXE --revision SHA
No acquisition, production edits, phase inversion, or held-out candidate scoring.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

import numpy as np
import observable as o

# Fixed before rendering: baseline and a small cross-appearance wash/rim search.
TRIALS = [{"protect_text": 0}] + [dict(protect_text=0, regular_wash=w,
    regular_hairline=.25, regular_specular=.03) for w in (.25, .35, .45)]
PRIOR_GROUPS = {"light-x-p32-m50-a06", "light-y-p64-m50-a12",
                "dark-x-p128-m50-a12", "dark-y-p64-m50-a06"}
POLICY = dict(interior_rmse_max=3., rim_rmse_max=5., worst_group_interior_max=5.,
              worst_group_rim_max=8., minimum_improvement=.20,
              max_group_regression=1.)


def validate_trials(trials):
    o.require(2 <= len(trials) <= 8, "bounded 2..8 candidates required")
    seen = set()
    for p in trials:
        o.parameters(p)
        o.require(p.get("protect_text") == 0 and all(
            k == "protect_text" or k.startswith("regular_") for k in p), "material-only regular contract required")
        key = json.dumps(p, sort_keys=True)
        o.require(key not in seen, "duplicate trial")
        seen.add(key)
    o.require(trials[0] == {"protect_text": 0}, "baseline must be unmodified material-only")


def loss(rows, appearance, split="fit", region="interior"):
    # Equal group/size/phase weighting: large capsules must not swamp small ones.
    values = [r["rmse_rgb_codes"] for r in rows if r["appearance"] == appearance
              and r["phase_role"] == split and r["region"] == region]
    o.require(values, "empty scoring cell")
    return float(np.mean(values))


def select(training):
    o.require(all(r["phase_role"] in ("fit", "flat") for rows in training for r in rows),
              "held-out leakage in selection input")
    return {a: min(range(len(training)), key=lambda i:
            loss(training[i], a) + .25 * loss(training[i], a, region="rim_all_0_3pt"))
            for a in ("light", "dark")}


def check_coverage(rows, cases, splits):
    expected = {(c["id"], size, region) for c in cases if c["phase_role"] in splits
                for size in o.CAPS for region in o.geometry(size)[2]}
    keys = [(r["id"], r["size"], r["region"]) for r in rows]
    o.require(len(keys) == len(set(keys)) and set(keys) == expected, "missing/duplicate scoring cell")
    o.require(all(np.isfinite(r["rmse_rgb_codes"]) and r["rmse_rgb_codes"] >= 0 for r in rows),
              "invalid score")


def verdict(rows, baseline):
    decisions = {}
    for a in ("light", "dark"):
        selected = [r for r in rows if r["appearance"] == a and r["phase_role"] == "heldout"
                    and r["group"] not in PRIOR_GROUPS]
        old = [r for r in baseline if r["appearance"] == a and r["phase_role"] == "heldout"
               and r["group"] not in PRIOR_GROUPS]
        regions = {}
        for region, mean_limit, worst_limit in (
                ("interior", POLICY["interior_rmse_max"], POLICY["worst_group_interior_max"]),
                ("rim_all_0_3pt", POLICY["rim_rmse_max"], POLICY["worst_group_rim_max"])):
            keys = sorted({(r["group"], r["size"]) for r in selected})
            means, regressions = [], []
            for group, size in keys:
                def cell(data):
                    values = [r["rmse_rgb_codes"] for r in data if r["group"] == group
                              and r["size"] == size and r["region"] == region]
                    o.require(len(values) == 2, "incomplete held-out cell")
                    return float(np.mean(values))
                means.append(cell(selected))
                regressions.append(cell(selected) - cell(old))
            current, previous = loss(selected, a, "heldout", region), loss(old, a, "heldout", region)
            regions[region] = dict(mean_rmse=current, baseline_rmse=previous,
                worst_group_size_rmse=max(means), worst_regression=max(regressions),
                pass_policy=current <= mean_limit and max(means) <= worst_limit
                and current <= previous * (1 - POLICY["minimum_improvement"])
                and max(regressions) <= POLICY["max_group_regression"])
        decisions[a] = dict(regions=regions, adopt=all(v["pass_policy"] for v in regions.values()))
    return decisions


def score(directory, cases, native_root, native_frames, splits, raw_root):
    """Never decode native held-out glass during candidate selection."""
    report = o.load(directory / "render.json")
    lookup = {(f["id"], f["state"]): f for f in report["frames"]}
    rows = []
    raw_root.mkdir(parents=True)
    for c in cases:
        if c["phase_role"] not in splits:
            continue
        actual = o.image(directory / lookup[c["id"], "glass"]["file"])
        reference = o.image(native_root / native_frames[c["id"], "glass"]["ppm"])
        for name in o.CAPS:
            (x0, y0, x1, y1), inward, masks = o.geometry(name)
            a, b = actual[y0:y1, x0:x1], reference[y0:y1, x0:x1]
            error = a.astype(np.int16) - b.astype(np.int16)
            file = raw_root / f'{c["id"]}-{name}.npz'
            np.savez_compressed(file, native_rgb=b, gpui_rgb=a, error_rgb_codes=error,
                                inward_points=inward, **masks)
            for region, mask in masks.items():
                rows.append({**{k: v for k, v in c.items() if k != "pattern"}, "size": name,
                    "region": region, "raw_file": file.name, "raw_sha256": o.digest(file),
                    "previously_compared_group": c["group"] in PRIOR_GROUPS,
                    **o.metric(error, mask)})
        if c["index"] == 8 and c["group"] not in PRIOR_GROUPS:
            o.review(actual, reference, c["id"], raw_root / "comparison.png")
    return rows


def render_job(label, group, options, cases, native_root, native_frames, executable, output, splits):
    requested = o.request(cases, options, [group])
    # Flats captured once per appearance per pass rather than once per group.
    appearance = group.split("-")[0]
    first = min(c["group"] for c in cases if c["kind"] != "flat" and c["appearance"] == appearance)
    requested["cases"] = [c for c in requested["cases"] if c["kind"] != "flat"
                          or (group == first and c["appearance"] == appearance)]
    directory = output / label / group
    directory.mkdir(parents=True)
    o.write(directory / "request.json", requested)
    command = [str(executable), "--observable", str(directory / "request.json"), str(directory)]
    env = dict(os.environ, LP_NUM_THREADS="2")
    with (directory / "renderer.log").open("w") as log:
        subprocess.run(command, check=True, timeout=1200, env=env, stdout=log, stderr=subprocess.STDOUT)
    frames = o.collect(directory, requested, o.load(directory / "render.json"))
    selected = [c for c in cases if c["id"] in {item["id"] for item in requested["cases"]}]
    rows = score(directory, selected, native_root, native_frames, splits, output / (label + "-errors") / group)
    o.write(directory / "candidate.json", dict(parameters=options, material_contract=o.contract(options),
        request_sha256=o.digest(directory / "request.json"), render_sha256=o.digest(directory / "render.json"),
        command=command, environment={"LP_NUM_THREADS": "2"}, frames=frames,
        status="complete; exact backgrounds and repeats verified"))
    archive = directory.with_suffix(".tar.gz")
    with tarfile.open(archive, "w:gz") as tar:
        tar.add(directory, arcname=group)
    shutil.rmtree(directory)  # Only this job's disposable, now archived output.
    print("verified", label, group, len(frames), "frames", flush=True)
    return rows, dict(group=group, archive=str(archive.relative_to(output)), sha256=o.digest(archive))


def run(native_root, source, output, executable, revision, workers):
    native_root, source, output, executable = [Path(p).resolve() for p in (native_root, source, output, executable)]
    validate_trials(TRIALS)
    cases, frames = o.native(native_root, source)
    groups = sorted({c["group"] for c in cases if c["kind"] != "flat"})
    o.require(len(groups) == 32 and 1 <= workers <= 8, "full operating coverage/workers required")
    output.mkdir(parents=True, exist_ok=False)
    before, binary = o.sources(), o.digest(executable)
    inputs = {str(p): o.digest(p) for p in [native_root / n for n in
        ("manifest.json", "provenance.json", "fixture.json", "cases.json")] +
        [source / n for n in ("README.md", "METHODS.md", "fixture.json", "cases.json", "Phase.swift", "run.py")]}
    plan = dict(schema="observable-fit-plan-1", revision=revision, trials=TRIALS, groups=groups,
        objective="equal group/size/phase mean RGB RMSE interior + 0.25 all-rim RMSE; eight fit phases only",
        policy=POLICY, policy_meaning="predeclared engineering screening limits, not perceptual equivalence",
        known_prior_heldout_groups=sorted(PRIOR_GROUPS), validation="fresh render after frozen selection; independent verdict excludes previously compared four groups",
        workers=workers, executable_sha256=binary, source_sha256=before, input_sha256=inputs,
        native_frames="source evidence retains all PNG/PPM hashes and active brackets; audited at start/end")
    o.write(output / "plan.json", plan)
    archives, training = [], []
    def batch(label, options, splits):
        with ThreadPoolExecutor(max_workers=workers) as pool:
            futures = [pool.submit(render_job, label, g, options(g), cases, native_root, frames,
                                   executable, output, splits) for g in groups]
            rows = []
            for f in futures:
                result, archive = f.result()
                rows.extend(result)
                archives.append(archive)
        check_coverage(rows, cases, splits)
        return rows
    for i, p in enumerate(TRIALS):
        rows = batch(f"trial-{i}", lambda g: p, {"fit"})
        training.append(rows)
        o.write(output / f"training-{i}.json", rows)
        print("training", i, {a: loss(rows, a) for a in ("light", "dark")}, flush=True)
    selection = select(training)
    frozen = dict(trials=selection, parameters={a: TRIALS[i] for a, i in selection.items()},
                  plan_sha256=o.digest(output / "plan.json"), training_sha256={str(i):
                  o.digest(output / f"training-{i}.json") for i in range(len(TRIALS))})
    o.write(output / "selection.json", frozen)
    selection_hash = o.digest(output / "selection.json")
    # No held-out errors are computed until selection is durably frozen.
    validation = batch("validation", lambda g: TRIALS[selection[g.split("-")[0]]], {"fit", "heldout"})
    baseline = []
    for g in groups:
        with tempfile.TemporaryDirectory() as d:
            with tarfile.open(output / "trial-0" / (g + ".tar.gz")) as tar:
                tar.extractall(d, filter="data")
            baseline.extend(score(Path(d) / g, [c for c in cases if c["group"] == g], native_root,
                frames, {"heldout"}, output / "baseline-heldout-errors" / g))
    o.require(o.digest(output / "selection.json") == selection_hash, "selection changed during validation")
    o.require(o.sources() == before and o.digest(executable) == binary
              and all(o.digest(Path(p)) == h for p, h in inputs.items()), "stale source/executable/input")
    o.native(native_root, source)
    o.write(output / "validation.json", validation)
    o.write(output / "baseline-heldout.json", baseline)
    result = dict(schema="observable-fit-result-1", selection=frozen, selection_sha256=selection_hash,
        decisions=verdict(validation, baseline), archives=archives,
        validation_sha256=o.digest(output / "validation.json"), baseline_sha256=o.digest(output / "baseline-heldout.json"),
        adoption="screening only; no production changes; native Metal verification still required",
        rows=dict(training=sum(map(len, training)), validation=len(validation), baseline_heldout=len(baseline)))
    o.write(output / "result.json", result)
    print(json.dumps(result["decisions"], indent=2), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("native", "source", "output"):
        parser.add_argument(name)
    parser.add_argument("--executable", required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--workers", type=int, default=4)
    args = parser.parse_args()
    run(args.native, args.source, args.output, args.executable, args.revision, args.workers)
