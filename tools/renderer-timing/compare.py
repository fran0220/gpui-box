"""Same-host renderer evidence; CPU completion/readback never stand in for GPU time.

Renderer-specific authority: same-adapter raw median intervals, baseline CI hull
expanded by baseline median range, all candidate repeats must clear the envelope.
Never divide by version-dependent renderer work: that hides uniform regressions.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import random
import statistics
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[2]
SAMPLES = 30
WARMUP = 8
RESAMPLES = 100000
POLICY = "renderer-raw-envelope-v1"
METRICS = ("cpu_encode_submit", "submit_to_completion", "gpu_execution",
           "timestamp_readback", "separate_capture_total")
IDENTITY = ("schema", "session", "host", "adapter", "os", "arch", "compiler",
            "profile", "source_sha256", "workload_version", "warmup", "sample_count",
            "warmup_seconds", "renderer_class", "runner_sha256", "build_flags_sha256")


def positive(value):
    if isinstance(value, bool) or not isinstance(value, (float, int)) or not math.isfinite(value) or value <= 0:
        raise ValueError("expected positive finite timing evidence")
    return value


def metric_value(sample, name):
    value = sample[name]
    if name == "gpu_execution":
        if value["status"] == "unsupported":
            if set(value) != {"status", "reason"} or not isinstance(value["reason"], str) or not value["reason"]:
                raise ValueError("unsupported GPU evidence needs a reason, not a duration")
            return None
        if value["status"] != "measured" or set(value) != {"status", "seconds"}:
            raise ValueError("invalid GPU capability evidence")
        return positive(value["seconds"])
    if name == "timestamp_readback" and value is None:
        return None
    return positive(value)


def validate(evidence):
    if set(evidence) != {"baseline", "candidate"}:
        raise ValueError("baseline and candidate required")
    if any(not isinstance(runs, list) or not 3 <= len(runs) <= 10 for runs in evidence.values()):
        raise ValueError("three to ten independent repeats required")
    reference = evidence["baseline"][0]
    if any(not reference[key] for key in IDENTITY):
        raise ValueError("missing host/workload identity")
    seen = set()
    for side in ("baseline", "candidate"):
        runs = evidence[side]
        if not 3 <= len(runs) <= 10:
            raise ValueError("three to ten independent repeats required")
        revision = runs[0]["revision"]
        binary = runs[0]["binary_sha256"]
        lockfile = runs[0]["lockfile_sha256"]
        if not revision or not binary or not lockfile:
            raise ValueError("revision and executable identity required")
        for run in runs:
            if any(run[key] != reference[key] for key in IDENTITY):
                raise ValueError("mismatched same-host workload or build configuration")
            if (run["schema"] != 1 or run["warmup"] != WARMUP or run["sample_count"] != SAMPLES or run["warmup_seconds"] != .5
                    or run["debug_assertions"] is not False or run["profile"] != "release"
                    or run["dirty"] is not False or run["capture_complete"] is not True):
                raise ValueError("invalid benchmark configuration or dirty build")
            if run["revision"] != revision or run["binary_sha256"] != binary or run["lockfile_sha256"] != lockfile:
                raise ValueError("revision/binary changed within one side")
            if not run["run_id"] or run["run_id"] in seen:
                raise ValueError("reused or missing run identity")
            seen.add(run["run_id"])
            positive(run["renderer_initialization"])
            if set(run["workloads"]) != {"small_quads", "quads"}:
                raise ValueError("missing workload")
            previous = 0
            for workload, count in (("small_quads", 32), ("quads", 1024)):
                data = run["workloads"][workload]
                if data["primitive_count"] != count or len(data["samples"]) != SAMPLES:
                    raise ValueError("changed workload or inadequate samples")
                if type(data["warmup_samples"]) is not int or not WARMUP <= data["warmup_samples"] <= 4096 or positive(data["warmup_elapsed"]) < .5:
                    raise ValueError("inadequate or unbounded warmup")
                first = data["first_frame"]
                for index, sample in enumerate([first] + data["samples"]):
                    submission = sample["submission_id"]
                    if type(submission) is not int or submission <= previous:
                        raise ValueError("stale, duplicate or unordered submission evidence")
                    if index == 1 and submission != previous + data["warmup_samples"] + 1:
                        raise ValueError("warmup evidence does not match submission IDs")
                    if index > 1 and submission != previous + 1:
                        raise ValueError("missing/failed sample submission")
                    previous = submission
                    for metric in METRICS:
                        if index or metric != "separate_capture_total":
                            metric_value(sample, metric)
                    if sample["gpu_execution"]["status"] == "unsupported" and sample["timestamp_readback"] is not None:
                        raise ValueError("unsupported GPU clock has query readback evidence")
    return reference


def interval(samples):
    """Deterministic percentile bootstrap median CI, matching policy confidence .95."""
    rng = random.Random(0)
    draws = sorted(statistics.median(rng.choices(samples, k=len(samples))) for _ in range(RESAMPLES))
    return draws[int(RESAMPLES * .025)], statistics.median(samples), draws[int(RESAMPLES * .975)]


def envelope(baseline, candidate):
    spread = max(row[1] for row in baseline) - min(row[1] for row in baseline)
    low = max(0, min(row[0] for row in baseline) - spread)
    high = max(row[2] for row in baseline) + spread
    if not math.isfinite(high):
        raise ValueError("nonfinite noise envelope")
    cl, cu = min(row[0] for row in candidate), max(row[2] for row in candidate)
    verdict = ("regression" if cl > high else "improvement" if cu < low else
               "within_noise" if cl >= low and cu <= high else "inconclusive")
    return {"verdict": verdict, "baseline_envelope": [low, high],
            "baseline_median_range": spread, "baseline_intervals": baseline, "candidate_intervals": candidate}


def compare(evidence):
    validate(evidence)
    result = {}
    for workload in ("small_quads", "quads"):
        for metric in METRICS:
            intervals = {side: [] for side in evidence}
            unsupported = False
            for side, runs in evidence.items():
                for run in runs:
                    values = [metric_value(sample, metric) for sample in run["workloads"][workload]["samples"]]
                    if any(value is None for value in values):
                        unsupported = True
                        continue
                    intervals[side].append(interval(values))
            key = f"{workload}/{metric}"
            if unsupported:
                not_applicable = (metric == "timestamp_readback" and evidence["baseline"][0]["renderer_class"] == "native-metal"
                                  and not any(intervals.values()))
                result[key] = {"verdict": "not_applicable" if not_applicable else "unavailable",
                               "reason": "no query readback on native Metal" if not_applicable else "at least one run lacks this metric; no numeric comparison"}
            else:
                result[key] = envelope(intervals["baseline"], intervals["candidate"])
    return result


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def capture(output, session, repeats):
    if not session or not 3 <= repeats <= 10:
        raise ValueError("nonempty session and three to ten repeats required")
    if output.exists() or output.with_suffix(".log").exists():
        raise ValueError("capture output and log must be new")
    if command("git", "status", "--porcelain", "--untracked-files=normal"):
        raise ValueError("capture requires a clean checkout")
    revision = command("git", "rev-parse", "HEAD")
    subprocess.run(["cargo", "build", "--manifest-path", "tools/headless-visual/Cargo.toml",
                    "--example", "renderer_timing", "--release", "--locked"], cwd=ROOT, check=True)
    # cargo metadata respects CARGO_TARGET_DIR, including platform-specific paths.
    metadata = json.loads(command("cargo", "metadata", "--manifest-path", "tools/headless-visual/Cargo.toml", "--no-deps", "--format-version", "1"))
    executable = Path(metadata["target_directory"]) / "release" / "examples" / ("renderer_timing.exe" if os.name == "nt" else "renderer_timing")
    source = ROOT / "tools/headless-visual/examples/renderer_timing.rs"
    host = hashlib.sha256((platform.node() + platform.platform() + platform.processor()).encode()).hexdigest()
    flags = {key: value for key, value in os.environ.items()
             if key.startswith("CARGO_PROFILE_") or key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER")}
    common = {"revision": revision, "dirty": False, "host": host, "compiler": command("rustc", "-Vv"),
              "profile": "release", "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
              "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "build_flags_sha256": hashlib.sha256(json.dumps(flags, sort_keys=True).encode()).hexdigest(),
              "lockfile_sha256": hashlib.sha256((ROOT / "tools/headless-visual/Cargo.lock").read_bytes()).hexdigest(),
              "binary_sha256": hashlib.sha256(executable.read_bytes()).hexdigest()}
    runs = []
    with output.with_suffix(".log").open("x") as log:
        for _ in range(repeats):
            process = subprocess.run([str(executable)], cwd=ROOT, stderr=log, stdout=subprocess.PIPE,
                                     env={**os.environ, "GPUI_TIMING_SESSION": session}, timeout=180, text=True)
            process.check_returncode()
            run = json.loads(process.stdout)
            run.update(common, run_id=str(uuid.uuid4()), capture_complete=False)
            runs.append(run)
            # Preserve completed raw repeats even when a later process fails.
            output.write_text(json.dumps(runs, indent=2) + "\n")
    if revision != command("git", "rev-parse", "HEAD") or command("git", "status", "--porcelain", "--untracked-files=normal"):
        raise ValueError("checkout changed during capture")
    for run in runs:
        run["capture_complete"] = True
    output.write_text(json.dumps(runs, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    take = sub.add_parser("capture")
    take.add_argument("output", type=Path)
    take.add_argument("--session", required=True)
    take.add_argument("--repeats", type=int, default=3)
    diff = sub.add_parser("compare")
    diff.add_argument("baseline", type=Path)
    diff.add_argument("candidate", type=Path)
    args = parser.parse_args()
    if args.action == "capture":
        capture(args.output, args.session, args.repeats)
    else:
        results = compare({side: json.loads(getattr(args, side).read_text()) for side in ("baseline", "candidate")})
        print(json.dumps({"policy": POLICY, "confidence_level": .95, "resamples": RESAMPLES,
                          "baseline_sha256": hashlib.sha256(args.baseline.read_bytes()).hexdigest(),
                          "candidate_sha256": hashlib.sha256(args.candidate.read_bytes()).hexdigest(),
                          "results": results}, indent=2))
        # Like the shaping policy, inconclusive does not fail, but is not evidence
        # of equivalence. Missing requested metrics cannot pass as zero.
        if any(row["verdict"] in ("regression", "unavailable") for row in results.values()):
            raise SystemExit(1)


if __name__ == "__main__":
    main()
