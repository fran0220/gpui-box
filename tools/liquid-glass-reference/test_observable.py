import copy
import math
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import numpy as np
from PIL import Image
import observable as o


def cases():
    return [dict(id=f"dark-y-p32-m50-a12-{i}", group="dark-y-p32-m50-a12",
                 appearance="dark", axis="y", period=32, mean=.5, amplitude=.12,
                 index=i, phase=p, kind="primary", rest=None,
                 phase_role="fit" if i < 8 else "heldout",
                 pattern=f"dark-y-p32-m50-a12-{i}-pattern.png")
            for i, p in enumerate(o.PHASES)]


class ObservableTests(unittest.TestCase):
    def test_protocol_and_defaults(self):
        cs = cases()
        r = o.request(cs, {})
        self.assertEqual(r["parameters"], {})
        self.assertEqual(len(r["cases"]), 10)
        self.assertEqual(len(r["cases"][0]["codes"]), 800)
        self.assertNotEqual(o.contract({}), o.contract({"protect_text": 0}))
        self.assertIn("material-only-baseline", o.contract({"protect_text": 0}))
        for bad in (cs[:-1], cs + [cs[0]]):
            with self.assertRaises(ValueError): o.complete_cases(bad)
        bad = copy.deepcopy(cs)
        bad[8]["phase_role"] = "fit"
        with self.assertRaises(ValueError): o.complete_cases(bad)
        with self.assertRaises(ValueError): o.request(cs, {}, ["light-x-p64-m50-a06"])

    def test_encoded_axis_and_fiducial_boundaries(self):
        p = o.pattern(cases()[1])
        self.assertEqual(p[48, 80, 0], round(255*(.5+.12*math.cos(2*math.pi*24.25/32+math.pi/4))))
        np.testing.assert_array_equal(p[16, 16], [255, 0, 0])
        np.testing.assert_array_equal(p[47, 47], [255, 0, 0])
        self.assertEqual(p[48, 47, 0], p[48, 80, 0])
        self.assertNotEqual(p[48, 80, 0], p[80, 48, 0])

    def test_asymmetric_metrics_and_no_heldout_leakage(self):
        error = np.array([[[1, -2, 6], [99, 99, 99]], [[-4, 3, 2], [7, 7, 7]]])
        m = o.metric(error, np.array([[True, False], [True, False]]))
        self.assertEqual(m["sse_rgb_codes"], 70)
        self.assertAlmostEqual(m["mae_rgb_codes"], 3)
        self.assertAlmostEqual(m["rmse_rgb_codes"], math.sqrt(70/6))
        self.assertEqual(m["bias_rgb_codes"], [-1.5, .5, 4])
        records = [dict(m, phase_role="fit", region="interior"),
                   dict(m, phase_role="heldout", region="interior")]
        before = o.training_objective(records)
        records[1]["sse_rgb_codes"] = 10**12
        self.assertEqual(o.training_objective(records), before)
        self.assertGreater(o.split_loss(records, "heldout", "interior")["rmse_rgb_codes"], 1000)

    def test_masks_retain_full_rim_and_distinct_sizes(self):
        for name in o.CAPS:
            box, inward, masks = o.geometry(name)
            np.testing.assert_array_equal(masks["interior"] | masks["rim_all_0_3pt"], inward >= 0)
            self.assertFalse((masks["interior"] & masks["rim_all_0_3pt"]).any())
            self.assertTrue(all(mask.any() for mask in masks.values()))
            self.assertEqual(masks["interior"].shape, (box[3]-box[1], box[2]-box[0]))

    def test_missing_stale_partial_background_mismatch(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            r = o.request(cases(), {})
            r["cases"] = r["cases"][:1]
            o.write(root / "request.json", r)
            report = dict(schema="gpui-regular-observable-render-1", renderer="wgpu-software-fallback",
                          request_text=(root / "request.json").read_text(), frames=[])
            wrong_geometry = dict(r, logical_size=[960, 640])
            with self.assertRaisesRegex(ValueError, "request geometry/schema"):
                o.collect(root, wrong_geometry, report)
            with self.assertRaisesRegex(ValueError, "missing returned"): o.collect(root, r, report)
            bad = dict(report, request_text="stale")
            with self.assertRaisesRegex(ValueError, "stale"): o.collect(root, r, bad)
            for state in ("background", "glass", "repeat"):
                stem = "case-000-" + state
                pixels = np.zeros((800, 1280, 3), dtype=np.uint8)
                Image.fromarray(pixels).save(root / (stem + ".png"))
                (root / (stem + ".rgb")).write_bytes(pixels.tobytes())
                report["frames"].append(dict(id=r["cases"][0]["id"], state=state,
                    file=stem+".png", raw_file=stem+".rgb", pixel_size=[1280, 800]))
            with self.assertRaisesRegex(ValueError, "background mismatch"): o.collect(root, r, report)
            (root / "case-000-background.rgb").write_bytes(b"partial")
            with self.assertRaisesRegex(ValueError, "partial raw"): o.collect(root, r, report)

    def test_comparison_rejects_relabelled_renderer_and_incomplete_marker(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            o.write(root / "manifest.json", {})
            requested = o.request(cases(), {})
            o.write(root / "request.json", requested)
            o.write(root / "render.json", {"renderer": "wgpu-software-fallback"})
            candidate = dict(schema="gpui-regular-observable-candidate-1",
                status="complete; exact encoded backgrounds and repeats verified; uncalibrated",
                native_manifest_sha256=o.digest(root / "manifest.json"),
                request_sha256=o.digest(root / "request.json"),
                render_sha256=o.digest(root / "render.json"),
                material_contract=o.contract({}), renderer="native-metal")
            with mock.patch.object(o, "native", return_value=(cases(), {})):
                o.write(root / "candidate.json", candidate)
                with self.assertRaisesRegex(ValueError, "candidate renderer mismatch"):
                    o.compare(root, root, root / "output")
                candidate["renderer"] = "wgpu-software-fallback"
                candidate["status"] = "partial"
                o.write(root / "candidate.json", candidate)
                with self.assertRaisesRegex(ValueError, "incomplete candidate"):
                    o.compare(root, root, root / "output")
            self.assertFalse((root / "output").exists())


if __name__ == "__main__":
    unittest.main()
