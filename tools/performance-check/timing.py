"""Same-host CPU shaping acceptance; no display, raster or GPU timing claims.

Only the standard library is used. See docs/performance-testing.md for the
empirical noise policy and the limits of interval normalization.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import tarfile
import tempfile
import tomllib
import uuid

ROOT = Path(__file__).resolve().parents[2]
BENCH = "crates/gpui_wgpu/benches/layout_line.rs"
COMMON = [BENCH, "crates/gpui_wgpu/assets/fonts/lilex/Lilex-Regular.ttf",
          "crates/gpui_wgpu/assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"]
CALIBRATION = "calibration/arithmetic"
WORKLOADS = ["layout_line/no_fallback", "layout_line/with_fallback_ascii",
             "layout_line/mixed_direction_paragraphs"]
CONFIG = {"warmup_seconds": 0.5, "measurement_seconds": 1.0,
          "samples": 30, "confidence_level": 0.95, "resamples": 100000}
POLICY = "baseline CI hull expanded on each side by baseline normalized median range"


def positive(value):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError("expected a finite positive number")
    if not math.isfinite(value) or value <= 0:
        raise ValueError("expected a finite positive number")
    return value


def interval(estimate):
    ci = estimate["confidence_interval"]
    if ci["confidence_level"] != CONFIG["confidence_level"]:
        raise ValueError("unexpected confidence level")
    low, point, high = map(positive, (ci["lower_bound"], estimate["point_estimate"],
                                    ci["upper_bound"]))
    if not low <= point <= high:
        raise ValueError("unordered confidence interval")
    return low, point, high


def compare(evidence):
    """Reject invalid evidence; return per-workload normalized intervals/verdicts.

    Interval division is conservative arithmetic, not a joint 95% confidence
    claim. All candidate repeats must clear the envelope to regress.
    """
    if evidence["config"] != CONFIG:
        raise ValueError("incompatible benchmark configuration")
    normalized = {}
    for side in ("baseline", "candidate"):
        runs = evidence[side]["runs"]
        if len(runs) < 3:
            raise ValueError("at least three independent repeats per revision required")
        normalized[side] = {name: [] for name in WORKLOADS}
        for run in runs:
            if (run["corpus_sha256"] != evidence["corpus_sha256"] or
                    run["session"] != evidence["session"]):
                raise ValueError("mismatched corpus or host session")
            measurements = run["measurements"]
            if set(measurements) != set(WORKLOADS + [CALIBRATION]):
                raise ValueError("missing or unexpected workload/calibration")
            for measurement in measurements.values():
                if measurement["samples"] != CONFIG["samples"]:
                    raise ValueError("inadequate sample count")
                interval(measurement["median"])
            cl, cp, cu = interval(measurements[CALIBRATION]["median"])
            for name in WORKLOADS:
                low, point, high = interval(measurements[name]["median"])
                ratio = [low / cu, point / cp, high / cl]
                for value in ratio:
                    positive(value)
                normalized[side][name].append(ratio)
    results = {}
    for name in WORKLOADS:
        baseline = normalized["baseline"][name]
        candidate = normalized["candidate"][name]
        spread = max(row[1] for row in baseline) - min(row[1] for row in baseline)
        low = max(0, min(row[0] for row in baseline) - spread)
        high = max(row[2] for row in baseline) + spread
        if not math.isfinite(high):
            raise ValueError("nonfinite noise envelope")
        cl, cu = min(row[0] for row in candidate), max(row[2] for row in candidate)
        if cl > high:
            verdict = "regression"
        elif cu < low:
            verdict = "improvement"
        elif cl >= low and cu <= high:
            verdict = "within_noise"
        else:
            verdict = "inconclusive"
        results[name] = {"baseline_normalized_intervals": baseline,
                         "candidate_normalized_intervals": candidate,
                         "noise_margin": spread, "baseline_envelope": [low, high],
                         "verdict": verdict}
    return results


def command(args, cwd=ROOT, **kwargs):
    return subprocess.check_output(args, cwd=cwd, text=True, **kwargs).strip()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read_measurements(directory):
    measurements = {}
    for name in [CALIBRATION] + WORKLOADS:
        path = directory / name / "new"
        estimate = json.loads((path / "estimates.json").read_text())["median"]
        sample = json.loads((path / "sample.json").read_text())
        if len(sample["iters"]) != len(sample["times"]):
            raise ValueError("mismatched iteration/time sample lengths")
        for value in sample["iters"] + sample["times"]:
            positive(value)
        interval(estimate)
        measurements[name] = {"median": estimate, "samples": len(sample["times"])}
    return measurements


def measure(args, evidence):
    if command(["git", "status", "--porcelain"]):
        raise ValueError("commit changes first: measurements require a clean checkout")
    if command(["git", "rev-parse", "--is-shallow-repository"]) == "true":
        raise ValueError("fetch --unshallow origin before resolving baseline history")
    revisions = {side: command(["git", "rev-parse", "--verify", "--end-of-options",
                               ref + "^{commit}"])
                 for side, ref in (("baseline", args.baseline_ref),
                                   ("candidate", args.candidate_ref))}
    common = {path: subprocess.check_output(["git", "show", f"{revisions['candidate']}:{path}"],
                                           cwd=ROOT) for path in COMMON}
    hashes = {path: digest(data) for path, data in common.items()}
    evidence.update(schema=1, session=str(uuid.uuid4()), config=CONFIG, policy=POLICY,
                    common_files_sha256=hashes, bench_sha256=hashes[BENCH],
                    corpus_sha256=digest(json.dumps(hashes, sort_keys=True).encode()),
                    runner_sha256=digest(Path(__file__).read_bytes()),
                    host={"platform": platform.platform(), "machine": platform.machine(),
                          "cpu": command(["lscpu"]), "rustc": command(["rustc", "-Vv"]),
                          "cargo": command(["cargo", "-V"]),
                          "backend": "CosmicText CPU shaping; embedded fonts; no GPU"})
    # Pin the toolchain and profile for both sources, regardless of old manifests.
    env = dict(os.environ, RUSTUP_TOOLCHAIN=command(["rustup", "show", "active-toolchain"]).split()[0],
               CARGO_PROFILE_BENCH_DEBUG="0", CARGO_PROFILE_BENCH_LTO="thin",
               CARGO_PROFILE_BENCH_CODEGEN_UNITS="1", CARGO_PROFILE_BENCH_OPT_LEVEL="3")
    evidence["build_environment"] = {key: value for key, value in env.items()
                                     if key.startswith("CARGO_PROFILE_") or
                                     key in ("RUSTUP_TOOLCHAIN", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")}
    with tempfile.TemporaryDirectory(prefix="gpui-shaping-") as temporary:
        temporary = Path(temporary)
        binaries = {}
        criterion_versions = []
        for side, revision in revisions.items():
            source = temporary / side
            source.mkdir()
            archive = temporary / f"{side}.tar"
            subprocess.run(["git", "archive", "--format=tar", "-o", str(archive), revision],
                           cwd=ROOT, check=True)
            with tarfile.open(archive) as bundle:
                bundle.extractall(source, filter="data")
            archive.unlink()
            for path, data in common.items():
                destination = source / path
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(data)
            lock = (source / "Cargo.lock").read_bytes()
            criterion_versions.append([p for p in tomllib.loads(lock.decode())["package"]
                                       if p["name"] == "criterion"])
            if not criterion_versions[-1] or criterion_versions[-1] != criterion_versions[0]:
                raise ValueError("baseline and candidate must use identical Criterion packages")
            evidence[side] = {"revision": revision, "cargo_lock_sha256": digest(lock), "runs": []}
            target = ROOT / "target" / "timing-build" / side
            build_env = dict(env, CARGO_TARGET_DIR=str(target))
            print(f"Building {side} {revision} with common harness", flush=True)
            output = command(["cargo", "bench", "--offline", "--locked", "-p", "gpui-box-wgpu",
                              "--bench", "layout_line", "--no-run", "--message-format=json"],
                             cwd=source, env=build_env)
            executables = [item["executable"] for line in output.splitlines()
                           if (item := json.loads(line)).get("reason") == "compiler-artifact"
                           and item.get("executable") and item["target"]["name"] == "layout_line"]
            if len(executables) != 1:
                raise ValueError("expected exactly one layout_line benchmark executable")
            binaries[side] = (source, executables[0])
        # Both builds finish before sampling; no compilation between repeats.
        for side in ("baseline", "candidate"):
            source, binary = binaries[side]
            for repeat in range(args.repeats):
                directory = args.output / side / str(repeat)
                directory.mkdir(parents=True)
                with (directory / "stdout.log").open("w") as log:
                    subprocess.run([binary, "--bench", "--noplot", "--warm-up-time", "0.5",
                                    "--measurement-time", "1", "--sample-size", "30",
                                    "--confidence-level", "0.95", "--nresamples", "100000"],
                                   cwd=source, env=dict(env, CRITERION_HOME=str(directory)),
                                   stdout=log, stderr=subprocess.STDOUT, check=True)
                evidence[side]["runs"].append({"corpus_sha256": evidence["corpus_sha256"],
                                               "session": evidence["session"],
                                               "measurements": read_measurements(directory)})
                print(f"Measured {side} repeat {repeat + 1}", flush=True)
    evidence["results"] = compare(evidence)
    evidence["verdict"] = ("regression" if any(r["verdict"] == "regression"
                           for r in evidence["results"].values()) else "no_confident_regression")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["compare"])
    parser.add_argument("--baseline-ref", required=True, help="explicit reviewed source revision")
    parser.add_argument("--candidate-ref", default="HEAD")
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--output", type=Path, required=True, help="new evidence directory")
    args = parser.parse_args()
    if args.repeats < 3:
        parser.error("at least three repeats required")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    evidence = {}
    try:
        measure(args, evidence)
    except (ValueError, KeyError, TypeError, OSError, subprocess.SubprocessError) as error:
        evidence.update(verdict="invalid", error=str(error))
    (args.output / "report.json").write_text(json.dumps(evidence, indent=2, allow_nan=False) + "\n")
    print(json.dumps({"verdict": evidence["verdict"], "results": evidence.get("results"),
                      "error": evidence.get("error")}, indent=2))
    return 0 if evidence["verdict"] == "no_confident_regression" else 1


if __name__ == "__main__":
    raise SystemExit(main())
