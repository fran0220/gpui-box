import copy
import unittest
from unittest.mock import patch

import compare


def fixture():
    def sample(submission):
        return {"submission_id": submission, "cpu_encode_submit": .003,
                "submit_to_completion": .004, "gpu_execution": {"status": "measured", "seconds": .002},
                "timestamp_readback": .001, "separate_capture_total": .009}

    def run(index):
        result = {key: "fixture" for key in compare.IDENTITY}
        result.update(schema=1, warmup=8, warmup_seconds=.5, sample_count=30, debug_assertions=False,
                      profile="release", dirty=False, revision="revision", binary_sha256="binary",
                      lockfile_sha256="lockfile", capture_complete=True,
                      run_id=str(index), renderer_initialization=.1, workloads={})
        for name, count, first in (("small_quads", 32, 1), ("quads", 1024, 40)):
            result["workloads"][name] = {
                "primitive_count": count, "first_frame": sample(first), "warmup_samples": 8, "warmup_elapsed": .5,
                "samples": [sample(first + 9 + offset) for offset in range(30)]}
        return result

    return {"baseline": [run(i) for i in range(3)], "candidate": [run(i) for i in range(3, 6)]}


class RendererTimingTests(unittest.TestCase):
    def test_noise_envelope_requires_every_repeat_to_clear(self):
        baseline = [[8, 10, 12], [9, 11, 13], [7, 9, 11]]
        self.assertEqual(compare.envelope(baseline, [[16, 17, 18]] * 3)["verdict"], "regression")
        self.assertEqual(compare.envelope(baseline, [[1, 2, 4]] * 3)["verdict"], "improvement")
        self.assertEqual(compare.envelope(baseline, [[5, 10, 15]] * 3)["verdict"], "within_noise")
        self.assertEqual(compare.envelope(baseline, [[16, 17, 18], [14, 16, 17], [16, 17, 18]])["verdict"], "inconclusive")

    def test_real_bootstrap_is_deterministic_and_not_min_max(self):
        samples = list(range(1, 31))
        low, point, high = compare.interval(samples)
        self.assertEqual(point, 15.5)
        self.assertTrue(1 < low < point < high < 30)
        self.assertEqual(compare.interval(samples), (low, point, high))

    def test_invalid_missing_stale_and_cross_host_evidence_rejected(self):
        original = fixture()
        compare.validate(original)
        changes = [
            lambda e: e["candidate"].pop(),
            lambda e: e["candidate"][0].update(host="other host"),
            lambda e: e["candidate"][0].update(dirty=True),
            lambda e: e["candidate"][0].update(capture_complete=False),
            lambda e: e["candidate"][0].update(debug_assertions=True),
            lambda e: e["candidate"][0].update(run_id="0"),
            lambda e: e["candidate"][0].update(renderer_class="native-metal"),
            lambda e: e["candidate"][0]["workloads"].pop("small_quads"),
            lambda e: e["candidate"][0]["workloads"]["quads"].update(warmup_elapsed=.1),
            lambda e: e["candidate"][0]["workloads"]["quads"]["samples"].pop(),
            lambda e: e["candidate"][0]["workloads"]["quads"]["samples"][0].update(submission_id=40),
            lambda e: e["candidate"][0]["workloads"]["quads"]["samples"][0].pop("gpu_execution"),
            lambda e: e["candidate"][0]["workloads"]["quads"]["samples"][0].update(cpu_encode_submit=float("nan")),
            lambda e: e["candidate"][0]["workloads"]["quads"]["samples"][0].update(gpu_execution={"status": "measured", "seconds": 0}),
        ]
        for change in changes:
            evidence = copy.deepcopy(original)
            change(evidence)
            with self.assertRaises((ValueError, KeyError)):
                compare.validate(evidence)

    def test_unsupported_is_not_zero_or_a_successful_comparison(self):
        evidence = fixture()
        for run in evidence["candidate"]:
            for workload in run["workloads"].values():
                for sample in [workload["first_frame"]] + workload["samples"]:
                    sample.update(gpu_execution={"status": "unsupported", "reason": "no timestamp feature"}, timestamp_readback=None)
        # This test owns availability, not the independently tested bootstrap.
        with patch.object(compare, "interval", side_effect=lambda values: (min(values), values[0], max(values))):
            result = compare.compare(evidence)
        self.assertEqual(result["quads/gpu_execution"]["verdict"], "unavailable")
        self.assertEqual(result["quads/cpu_encode_submit"]["verdict"], "within_noise")
        sample = evidence["candidate"][0]["workloads"]["quads"]["samples"][0]
        sample["gpu_execution"]["seconds"] = 0
        with self.assertRaises(ValueError):
            compare.validate(evidence)

    def test_uniform_renderer_regression_cannot_calibrate_itself_away(self):
        evidence = fixture()
        for run in evidence["candidate"]:
            for workload in run["workloads"].values():
                for sample in workload["samples"]:
                    sample["gpu_execution"]["seconds"] *= 2
                    sample["cpu_encode_submit"] *= 9
        with patch.object(compare, "interval", side_effect=lambda values: (min(values), values[0], max(values))):
            result = compare.compare(evidence)
        for workload in ("small_quads", "quads"):
            self.assertEqual(result[f"{workload}/gpu_execution"]["verdict"], "regression")
            self.assertEqual(result[f"{workload}/cpu_encode_submit"]["verdict"], "regression")


if __name__ == "__main__":
    unittest.main()
