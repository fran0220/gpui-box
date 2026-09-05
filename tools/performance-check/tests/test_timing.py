import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("timing", Path(__file__).resolve().parents[1] / "timing.py")
timing = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(timing)


def estimate(point, width=0.01):
    return {"point_estimate": point, "confidence_interval": {
        "confidence_level": 0.95, "lower_bound": point * (1 - width),
        "upper_bound": point * (1 + width)}}


def evidence(speed=1, slowdown=1, width=0.01):
    result = {"config": timing.CONFIG.copy(), "session": "same-host-session",
              "corpus_sha256": "common-harness-and-fonts"}
    for side in ("baseline", "candidate"):
        runs = []
        for _ in range(3):
            scale = speed if side == "candidate" else 1
            work = 100 * scale * (slowdown if side == "candidate" else 1)
            measurements = {name: {"median": estimate(work, width), "samples": 30}
                            for name in timing.WORKLOADS}
            measurements[timing.CALIBRATION] = {"median": estimate(10 * scale, width), "samples": 30}
            runs.append({"session": result["session"], "corpus_sha256": result["corpus_sha256"],
                         "measurements": measurements})
        result[side] = {"runs": runs}
    return result


class Timing(unittest.TestCase):
    def verdicts(self, data):
        return {r["verdict"] for r in timing.compare(data).values()}

    def test_uniform_host_speed_normalizes_away(self):
        self.assertEqual(self.verdicts(evidence(speed=4)), {"within_noise"})

    def test_true_twofold_regression(self):
        self.assertEqual(self.verdicts(evidence(slowdown=2)), {"regression"})

    def test_unchanged(self):
        self.assertEqual(self.verdicts(evidence()), {"within_noise"})

    def test_noisy_overlap_is_inconclusive_not_regression(self):
        self.assertEqual(self.verdicts(evidence(slowdown=1.1, width=0.3)), {"inconclusive"})

    def test_each_repeat_must_clear_envelope(self):
        data = evidence(slowdown=2)
        data["candidate"]["runs"][0] = copy.deepcopy(data["baseline"]["runs"][0])
        self.assertEqual(self.verdicts(data), {"inconclusive"})

    def test_noise_margin_is_measured_from_baseline_only(self):
        data = evidence(slowdown=1.1)
        for name in timing.WORKLOADS:
            data["baseline"]["runs"][2]["measurements"][name]["median"] = estimate(120)
        for row in timing.compare(data).values():
            self.assertEqual(row["noise_margin"], 2)
            self.assertEqual(row["verdict"], "within_noise")

    def test_mismatched_corpus_and_host(self):
        for key in ("corpus_sha256", "session"):
            data = evidence()
            data["candidate"]["runs"][0][key] = "other"
            with self.assertRaises(ValueError):
                timing.compare(data)

    def test_missing_calibration_or_workload(self):
        for name in [timing.CALIBRATION] + timing.WORKLOADS:
            data = evidence()
            del data["baseline"]["runs"][0]["measurements"][name]
            with self.assertRaises(ValueError):
                timing.compare(data)

    def test_nonfinite_nonpositive_and_unordered_intervals(self):
        for value in (float("nan"), float("inf"), 0, -1, True, "100", 10000):
            data = evidence()
            data["baseline"]["runs"][0]["measurements"][timing.CALIBRATION]["median"]["point_estimate"] = value
            with self.assertRaises(ValueError):
                timing.compare(data)

    def test_inadequate_repeats_and_samples(self):
        for side in ("baseline", "candidate"):
            data = evidence()
            data[side]["runs"].pop()
            with self.assertRaises(ValueError):
                timing.compare(data)
        data = evidence()
        data["baseline"]["runs"][0]["measurements"][timing.CALIBRATION]["samples"] = 10
        with self.assertRaises(ValueError):
            timing.compare(data)

    def test_configuration_and_confidence_mismatch(self):
        data = evidence()
        data["config"]["samples"] = 20
        with self.assertRaises(ValueError):
            timing.compare(data)
        data = evidence()
        data["baseline"]["runs"][0]["measurements"][timing.CALIBRATION]["median"]["confidence_interval"]["confidence_level"] = 0.9
        with self.assertRaises(ValueError):
            timing.compare(data)

    def test_reads_criterion_artifacts_and_rejects_missing_or_bad_samples(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(FileNotFoundError):
                timing.read_measurements(root)
            for name in [timing.CALIBRATION] + timing.WORKLOADS:
                directory = root / name / "new"
                directory.mkdir(parents=True)
                (directory / "estimates.json").write_text(json.dumps({"median": estimate(10)}))
                (directory / "sample.json").write_text(json.dumps({"iters": [1] * 30, "times": [10] * 30}))
            self.assertEqual(len(timing.read_measurements(root)), 4)
            (root / timing.CALIBRATION / "new/sample.json").write_text('{"iters": [1], "times": [1, 2]}')
            with self.assertRaises(ValueError):
                timing.read_measurements(root)


if __name__ == "__main__":
    unittest.main()
