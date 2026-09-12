"""Portable protocol tests. Synthetic RGB/PNG is never native or optical evidence."""
import copy
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
import zlib

import gpui_capture as g
from validate import ppm


def png(w, h):
    def chunk(k, v):
        return struct.pack(">I", len(v)) + k + v + struct.pack(">I", zlib.crc32(k+v))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress((b"\0" + b"\x19\x63\xdb"*w)*h)) + chunk(b"IEND", b""))


class Protocol(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.fixture = g.load(g.HERE / "fixture.json")
        self.reference = {"logical_size": [960, 640], "scale": 1, "frames": [
            {"appearance": "dark", "phase": "transition", "index": 7,
             "capture_start": 103.125, "capture_end": 103.375, "trigger_time": 103},
            {"appearance": "light", "phase": "static", "index": 0}]}

    def request(self):
        return g.plan(self.reference, self.fixture, {})

    def report(self):
        frames = []
        for i, p in enumerate(self.request()["frames"]):
            (self.root / f"{i}.rgb").write_bytes(b"\x19\x63\xdb"*(960*640))
            (self.root / f"{i}.png").write_bytes(png(960, 640))
            frames.append({**p, "raw_file": f"{i}.rgb", "file": f"{i}.png", "pixel_size": [960, 640],
                           "sample_time_after_trigger": p.get("requested_sample_time", 0),
                           "transition_status": "persistent-surface-resize" if i == 0 else "not-applicable",
                           "resize_geometry": {"identity": "menu", "bounds": [64, 448, 184, 73],
                               "sample_time_after_trigger": .25, "phase": "resizing",
                               "motion": "Animator linear 0.8s; independent compact-state replay"} if i == 0 else None})
        return {"schema": 1, "renderer": "wgpu-software-fallback",
                "clock": "GPUI TestDispatcher; measured executor now at draw completion", "frames": frames}

    def test_asymmetric_keys_and_times_not_indices(self):
        request = self.request()
        self.assertEqual(request["frames"][0]["requested_sample_time"], .25)
        self.assertEqual(request["frames"][0]["index"], 7)
        self.assertEqual(len(request["frames"]), 2)  # No hard-coded native frame count.
        self.reference["frames"][0].update(capture_start=104.5, capture_end=104.75)
        self.assertEqual(self.request()["frames"][0]["requested_sample_time"], 1.625)

    def test_asymmetric_rgb_survives_sidecar_and_hashes(self):
        frames = g.collect(self.root, self.request(), self.report())
        w, h, pixels = ppm(self.root / frames[0]["rgb_file"])
        self.assertEqual((w, h), (960, 640))
        self.assertEqual(pixels[:6], b"\x19\x63\xdb"*2)
        self.assertEqual(frames[0]["rgb_file_sha256"], g.digest(self.root / "0.ppm"))
        self.assertEqual(frames[0]["transition_status"], "persistent-surface-resize")
        self.assertEqual(frames[0]["resize_geometry"]["bounds"], [64, 448, 184, 73])

    def test_bad_intervals(self):
        for start, end, trigger in [(4, 3, 2), (3, 3, 2), (1, 2, 3),
                                    (float("nan"), 4, 2), (2, float("inf"), 1), (True, 4, 0)]:
            with self.subTest(start=start, end=end):
                self.reference["frames"][0].update(capture_start=start, capture_end=end, trigger_time=trigger)
                with self.assertRaises(ValueError): self.request()

    def test_parameters_are_explicit_bounded_and_default_empty(self):
        self.assertEqual(g.parameters({}), {})
        self.assertEqual(g.parameters({"blur": 3.25, "refractive_index": 1.7}),
                         {"blur": 3.25, "refractive_index": 1.7})
        for p in ({"blur": -1}, {"blur": 65}, {"blur": True}, {"blur": "12"},
                  {"thickness": float("nan")}, {"refractive_index": 0.9}, {"shader": 1}, []):
            with self.subTest(p=p), self.assertRaises(ValueError): g.parameters(p)

    def test_json_rejects_duplicate_and_nonfinite(self):
        for value in ('{"blur":1,"blur":2}', '{"blur":NaN}', '{"blur":Infinity}',
                      '{', '{"protect_text":}', '{"regular_gain":1,}'):
            path = self.root / "p.json"
            path.write_text(value)
            with self.assertRaises(ValueError): g.load(path)

    def test_trial_bounds_and_exact_numeric_protection(self):
        for key, (lo, hi) in g.LIMITS.items():
            for value in (lo, hi):
                self.assertEqual(g.parameters({key: value}), {key: value})
            for value in (lo - .01, hi + .01, None, "1", True, [], {}, float("inf")):
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    g.parameters({key: value})
        for value in (.5, .00001, .99999):
            with self.assertRaises(ValueError): g.parameters({"protect_text": value})
        for value in (0, 1, 0.0, 1.0):
            self.assertEqual(g.parameters({"protect_text": value}), {"protect_text": value})

    def test_unsupported_new_input_case_fails_closed(self):
        self.reference["frames"][0]["phase"] = "pointer-down"
        with self.assertRaisesRegex(ValueError, "unsupported phase"): self.request()

    def test_duplicate_keys_bad_dimensions_and_fixture(self):
        for change in (lambda: self.reference["frames"].append(self.reference["frames"][0]),
                       lambda: self.reference.update(logical_size=[640, 960]),
                       lambda: self.reference.update(scale=True),
                       lambda: self.fixture.update(version=99)):
            original = copy.deepcopy((self.reference, self.fixture))
            change()
            with self.assertRaises(ValueError): self.request()
            self.reference, self.fixture = original

    def test_returned_time_not_trusted(self):
        for actual in (.2, .5, float("nan"), True):
            report = self.report()
            report["frames"][0]["sample_time_after_trigger"] = actual
            with self.subTest(actual=actual), self.assertRaises(ValueError):
                g.collect(self.root, self.request(), report)

    def test_resize_requires_fresh_measured_geometry_and_honest_phase(self):
        for mutation in ({"sample_time_after_trigger": 0}, {"bounds": [64, 448, 144, 48]},
                         {"bounds": [64, 448, 185, 73]}, {"bounds": [64, 448, True, 73]},
                         {"phase": "settled"}, {"identity": "replacement"}, {"motion": "Apple equivalent"}):
            report = self.report()
            report["frames"][0]["resize_geometry"].update(mutation)
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                g.collect(self.root, self.request(), report)

    def test_static_pixels_cannot_claim_resize_even_if_other_region_changes(self):
        report = self.report()
        request = self.request()
        request["frames"][1]["appearance"] = "dark"
        report["frames"][1]["appearance"] = "dark"
        # Deliberately alter a pixel outside the resize ROI; full-frame hashes
        # would differ, but that is not evidence of menu motion.
        raw = self.root / "1.rgb"
        raw.write_bytes(b"\xff\xff\xff" + raw.read_bytes()[3:])
        with self.assertRaisesRegex(ValueError, "static resize pixels"):
            g.collect(self.root, request, report)

    def test_report_rejects_malformed_outputs(self):
        mutations = [lambda r: r["frames"].clear(),
                     lambda r: r["frames"][0].update(pixel_size=[640, 960]),
                     lambda r: r["frames"][0].update(raw_file="../escape.rgb"),
                     lambda r: r["frames"][0].update(transition_status="supported"),
                     lambda r: r.update(renderer="unknown"),
                     lambda r: r.update(clock="requested time copied"),
                     lambda r: (self.root / "0.rgb").write_bytes(b"short"),
                     lambda r: (self.root / "0.png").write_bytes(png(640, 960))]
        for mutate in mutations:
            report = self.report()
            mutate(report)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                g.collect(self.root, self.request(), report)

    def test_duplicate_and_missing_returned_keys(self):
        report = self.report()
        report["frames"] = [report["frames"][0], report["frames"][0]]
        with self.assertRaisesRegex(ValueError, "duplicate returned"):
            g.collect(self.root, self.request(), report)
        (self.root / "0.ppm").unlink()
        report["frames"] = report["frames"][:1]
        with self.assertRaisesRegex(ValueError, "missing returned"):
            g.collect(self.root, self.request(), report)

    def test_refusal_precedes_launch_and_output(self):
        reference = self.root / "reference"
        reference.mkdir()
        (reference / "manifest.json").write_text("{}")
        exe = self.root / "example"
        exe.write_bytes(b"not executed")
        with patch.object(g, "validate", side_effect=ValueError("native refused")) as validator, \
                patch.object(g.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "native refused"):
                g.capture(reference, self.root / "out", exe, "test-revision")
            validator.assert_called_once()
            run.assert_not_called()
            self.assertFalse((self.root / "out").exists())

    def test_smoke_cannot_overwrite_existing_directory(self):
        exe = self.root / "example"
        exe.write_bytes(b"not executed")
        with patch.object(g.subprocess, "run") as run:
            with self.assertRaises(FileExistsError): g.capture(None, self.root, exe, "test")
            run.assert_not_called()

    def test_candidate_handoff_pairs_all_keys_after_validation(self):
        # Mock ONLY acquisition/validation here; real compare.py checks the
        # resulting candidate, and the rejection test proves validation gates it.
        reference = self.root / "reference"
        reference.mkdir()
        (reference / "fixture.json").write_bytes((g.HERE / "fixture.json").read_bytes())
        self.reference["provenance"] = {"fixture_sha256": g.digest(reference / "fixture.json")}
        for i, f in enumerate(self.reference["frames"]):
            f.update(pixel_size=[960, 640], rgb_file=f"native-{i}.ppm")
        g.write(reference / "manifest.json", self.reference)
        exe = self.root / "example"
        exe.write_bytes(b"synthetic executable identity, never executed")
        output = self.root / "output"

        def render(command, **kwargs):
            self.assertEqual(kwargs["timeout"], 600)
            self.assertTrue(kwargs["check"])
            request = g.load(command[1])
            self.assertEqual(request["frames"][0]["requested_sample_time"], .25)
            report = self.report()
            for f in report["frames"]:
                for field in ("raw_file", "file"):
                    (output / f[field]).write_bytes((self.root / f[field]).read_bytes())
            g.write(output / "render.json", report)

        with patch.object(g, "validate", return_value={"status": "synthetic-test-only"}) as validator, \
                patch.object(g.subprocess, "run", side_effect=render):
            options = {"blur": 7.5, "regular_blur": 23, "regular_saturation": .63,
                       "regular_wash": .17, "regular_gain": 1.31, "regular_lift": .09,
                       "protect_text": 0}
            candidate = g.capture(reference, output, exe, "test-revision", options)
            validator.assert_called_once()
        self.assertEqual(len(candidate["pairing"]["pairs"]), 2)
        self.assertEqual(g.load(output / "parameters.json"), options)
        self.assertEqual(g.load(output / "request.json")["parameters"], options)
        sources = candidate["provenance"]["source_sha256"]
        for path in ("crates/gpui/src/scene.rs", "crates/gpui_macos/src/metal_renderer.rs",
                     "crates/gpui_windows/src/directx_renderer.rs", "crates/gpui_windows/src/shaders.hlsl",
                     "crates/gpui_wgpu/src/wgpu_renderer.rs"):
            self.assertEqual(sources[path], g.digest(g.REPO / path))
        self.assertEqual(candidate["parameters_sha256"], g.digest(output / "parameters.json"))
        self.assertEqual(candidate["provenance"]["reference_manifest_sha256"], g.digest(reference / "manifest.json"))
        self.assertIn("no Apple dynamics equivalence", candidate["coverage"]["transition"])
        self.assertEqual(g.load(output / "candidate.json"), candidate)
        self.assertFalse((output / "smoke.json").exists())

    def test_mask_metadata_separates_foreground(self):
        masks = g.masks(self.fixture)
        self.assertIn("not Apple", masks["typography"])
        self.assertEqual(masks["material_shapes"][2]["rect"], [64, 160, 208, 72])
        self.assertEqual(masks["foreground_exclusion_rects"][2], [64, 183, 208, 26])


if __name__ == "__main__":
    unittest.main()
