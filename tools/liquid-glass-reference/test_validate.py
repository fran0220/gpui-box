"""Synthetic test images are contract tests, NEVER native reference evidence."""
import copy
import json
from pathlib import Path
import shutil
import struct
import tempfile
import unittest
import zlib
from compare import pair
from validate import BACKEND, digest, validate

HERE = Path(__file__).resolve().parent


def png(pixels):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind+data))
    scanlines = b"".join(b"\0" + pixels[y*2880:(y+1)*2880] for y in range(640))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 960, 640, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(scanlines)) + chunk(b"IEND", b"")


class EvidenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls.temp.name)
        for name in ("Reference.swift", "fixture.json"):
            shutil.copy2(HERE / name, cls.root / name)
        fixture = json.loads((cls.root / "fixture.json").read_text())
        m = {"schema": 1, "capture_backend": BACKEND, "logical_size": [960, 640], "scale": 1, "interaction": "programmatic-state-change; no pointer dispatched", "accessibility": dict.fromkeys(["reduce_transparency", "reduce_motion", "increase_contrast", "differentiate_without_color", "invert_colors"], False), "provenance": {"os_build": "synthetic-test-only", "os_version": "26.0", "xcode": "synthetic", "sdk": "26.0", "architecture": "synthetic", "source_sha256": digest(cls.root / "Reference.swift"), "fixture_sha256": digest(cls.root / "fixture.json")}, "frames": []}
        for ai, appearance in enumerate(("light", "dark")):
            for phase, count in (("background", 1), ("static", 1), ("transition", 16)):
                for index in range(count):
                    pixels = bytearray([80 if phase == "background" else 120+index] * (960*640*3))
                    for marker in fixture["fiducials"]:
                        x, y, w, h = marker["rect"]
                        for row in range(y, y+h):
                            pixels[(row*960+x)*3:(row*960+x+w)*3] = bytes(marker["rgb"])*w
                    name = f"{appearance}-{phase}-{index}"
                    (cls.root / (name + ".png")).write_bytes(png(pixels))
                    (cls.root / (name + ".ppm")).write_bytes(b"P6\n960 640\n255\n" + pixels)
                    start = ai*10 + {"background": 0, "static": 1, "transition": 2}[phase] + index*0.1
                    frame = {"file": name+".png", "rgb_file": name+".ppm", "appearance": appearance, "phase": phase, "index": index, "pixel_size": [960, 640], "capture_start": start, "capture_end": start+0.01, "wall_end": "2026-09-11T00:00:00Z"}
                    if phase == "transition":
                        frame["trigger_time"] = ai*10+2
                    for key in ("file", "rgb_file"):
                        frame[key+"_sha256"] = digest(cls.root / frame[key])
                    m["frames"].append(frame)
        cls.original = m

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def setUp(self):
        self.m = copy.deepcopy(self.original)

    def reject(self, message):
        with self.assertRaisesRegex(ValueError, message):
            validate(self.root, self.m)

    def test_complete_contract(self):
        self.assertEqual(validate(self.root, self.m)["frames"], 36)

    def test_missing_frame_record(self):
        self.m["frames"].pop()
        self.reject("missing frames")

    def test_missing_frame_file(self):
        self.m["frames"][0]["file"] = "missing.png"
        self.reject("missing frame")

    def test_dimensions(self):
        self.m["frames"][0]["pixel_size"] = [1920, 1280]
        self.reject("dimension mismatch")

    def test_timestamps(self):
        self.m["frames"][1]["capture_start"] = 0
        self.reject("timestamp mismatch")

    def test_nonfinite_timestamp(self):
        self.m["frames"][0]["capture_start"] = float("nan")
        self.reject("invalid timestamp")

    def test_unknown_provenance(self):
        self.m["provenance"]["os_build"] = "unknown"
        self.reject("unknown provenance")

    def test_bitmap_substitute(self):
        self.m["capture_backend"] = "ImageRenderer"
        self.reject("unknown capture provenance")

    def test_source_identity(self):
        self.m["provenance"]["source_sha256"] = "0"*64
        self.reject("source identity")

    def test_hash_mismatch(self):
        self.m["frames"][0]["file_sha256"] = "0"*64
        self.reject("frame hash mismatch")

    def test_trigger_mismatch(self):
        self.m["frames"][3]["trigger_time"] = 2.01
        self.reject("inconsistent trigger")

    def test_accessibility_unknown(self):
        self.m["accessibility"].pop("reduce_motion")
        self.reject("accessibility")

    def test_content_validation(self):
        frame = self.m["frames"][0]
        bad = self.root / "bad.ppm"
        bad.write_bytes(b"P6\n960 640\n255\n" + bytes(960*640*3))
        try:
            frame["rgb_file"] = bad.name
            frame["rgb_file_sha256"] = digest(bad)
            self.reject("fiducial mismatch")
        finally:
            bad.unlink()

    def candidate(self):
        c = {"schema": 1, "producer": "gpui", "revision": "synthetic-test-only", "renderer": "synthetic", "parameters_sha256": "0"*64, "fixture_sha256": self.m["provenance"]["fixture_sha256"], "logical_size": [960, 640], "scale": 1, "frames": copy.deepcopy(self.m["frames"])}
        for f in c["frames"]:
            if f["phase"] == "transition":
                f["sample_time_after_trigger"] = (f["capture_start"]+f["capture_end"])/2-f["trigger_time"]
        return c

    def test_comparison_pairing(self):
        self.assertEqual(len(pair(self.m, self.candidate(), self.root)["pairs"]), 36)

    def test_comparison_refusals(self):
        for field, value, message in (("producer", "unknown", "provenance"), ("fixture_sha256", "bad", "fixture mismatch"), ("scale", 2, "dimension mismatch")):
            with self.subTest(field=field):
                c = self.candidate()
                c[field] = value
                with self.assertRaisesRegex(ValueError, message):
                    pair(self.m, c, self.root)

    def test_comparison_missing_frame(self):
        c = self.candidate()
        c["frames"].pop()
        with self.assertRaisesRegex(ValueError, "missing candidate frames"):
            pair(self.m, c, self.root)

    def test_comparison_timestamp(self):
        c = self.candidate()
        c["frames"][2]["sample_time_after_trigger"] = 100
        with self.assertRaisesRegex(ValueError, "timestamp mismatch"):
            pair(self.m, c, self.root)


if __name__ == "__main__":
    unittest.main()
