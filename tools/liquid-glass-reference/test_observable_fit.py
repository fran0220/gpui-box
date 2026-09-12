import copy
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import observable_fit as f


def rows(light, dark, split="fit"):
    return [dict(appearance=a, group=a+"-x-p64-m35-a06", size=s, index=i,
                 phase_role=split, region=r, rmse_rgb_codes=v, pixels=100 if s=="small" else 900)
            for a, v in [("light", light), ("dark", dark)] for s in ("small", "large")
            for i in ([0, 1] if split=="fit" else [8, 9]) for r in ("interior", "rim_all_0_3pt")]


class FitTests(unittest.TestCase):
    def test_explicit_bounded_material_contract(self):
        f.validate_trials(f.TRIALS)
        for bad in ([{}], [{"protect_text":0}, {"protect_text":1}],
                    [{"protect_text":0}]*2, [{"protect_text":0}, {"protect_text":0,"blur":12}]):
            with self.assertRaises(ValueError): f.validate_trials(bad)

    def test_asymmetric_selection_and_no_heldout_inputs(self):
        training = [rows(7, 2), rows(3, 8)]
        self.assertEqual(f.select(training), {"light":1,"dark":0})
        for r in training[0]: r["pixels"] *= 100000
        self.assertEqual(f.select(training), {"light":1,"dark":0})
        training[1].append(dict(training[1][0], phase_role="heldout", rmse_rgb_codes=0))
        with self.assertRaisesRegex(ValueError, "leakage"): f.select(training)

    def test_independent_policy_excludes_prior_groups_and_rejects_residual(self):
        baseline, selected = rows(10, 10, "heldout"), rows(2, 4, "heldout")
        result = f.verdict(selected, baseline)
        self.assertTrue(result["light"]["adopt"])
        self.assertFalse(result["dark"]["adopt"])
        for data in (baseline, selected):
            data.extend([dict(data[0], group="light-x-p32-m50-a06", rmse_rgb_codes=10**9)])
        self.assertEqual(f.verdict(selected, baseline), result)
        with self.assertRaisesRegex(ValueError, "incomplete"):
            f.verdict(selected[1:], baseline)

    def test_missing_and_duplicate_coverage(self):
        cases = [dict(id="asymmetric", phase_role="fit")]
        valid = [dict(id="asymmetric", size=size, region=region, rmse_rgb_codes=1)
                 for size in f.o.CAPS for region in f.o.geometry(size)[2]]
        f.check_coverage(valid, cases, {"fit"})
        for bad in (valid[:-1], valid+[valid[0]]):
            with self.assertRaisesRegex(ValueError, "scoring cell"): f.check_coverage(bad,cases,{"fit"})

    def test_score_never_opens_heldout_for_training(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d)
            f.o.write(path/"render.json", {"frames":[]})
            with patch.object(f.o, "image", side_effect=AssertionError("heldout image read")):
                result = f.score(path, [{"phase_role":"heldout"}], path, {}, {"fit"}, path/"raw")
            self.assertEqual(result, [])


if __name__ == "__main__":
    unittest.main()
