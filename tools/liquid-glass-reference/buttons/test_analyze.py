import unittest
import copy
import json
from pathlib import Path
import shutil
import struct
import tempfile
from unittest.mock import patch
import zlib
from analyze import analyze, digest, footprint, interior


class ValidationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        here = Path(__file__).parent
        for name in ("Buttons.swift", "fixture.json", "run.py"):
            shutil.copyfile(here/name, self.root/name)
        fixture = json.loads((here/"fixture.json").read_text())
        layout = {"glass": [316,84,168,56], "glass-prominent": [316,228,168,56], "custom": [328,376,144,48]}
        pixels = bytearray(800*520*3)
        for marker in fixture["fiducials"]:
            x,y,w,h = marker["rect"]
            k = ((y+h//2)*800+x+w//2)*3
            pixels[k:k+3] = bytes(marker["rgb"])
        self.rgb = b"P6\n800 520\n255\n" + pixels
        self.png = self.make_png(800,520)
        self.m = {"schema": "native-button-styles-evidence-1", "backend": "ScreenCaptureKit.SCScreenshotManager.desktopIndependentWindow",
                  "logical_size": [800,520], "scale": 1, "clock_origin_uptime": 100,
                  "input": "synthetic NSEvent via NSApplication.postEvent; no HID or global cursor movement",
                  "settings": dict.fromkeys(("differentiate_without_color", "increase_contrast", "invert_colors", "reduce_motion", "reduce_transparency"), False),
                  "layout_bounds": layout, "provenance": {n+"_sha256": digest(self.root/n) for n in ("Buttons.swift", "fixture.json", "run.py")}, "frames": [], "runs": []}
        time = 1
        for appearance in ("light", "dark"):
            self.frame(appearance,"all","background",0,time); time += 1
            for case in fixture["cases"]:
                c = case["id"]
                self.frame(appearance,c,"before",0,time)
                down = 100+time+0.5
                for i in range(4):
                    time += 1; self.frame(appearance,c,"held",i,time)
                up = 100+time+0.5
                for i in range(9):
                    time += 1; self.frame(appearance,c,"release",i,time)
                self.m["runs"].append({"appearance": appearance, "case": c, "layout_bounds": layout,
                    "dispatches": [{"number": n, "type": t, "event_uptime": v, "dispatch_uptime": v+0.01} for n,t,v in ((101,"down",down),(102,"up",up))],
                    "receipts": [{"number": n, "type": t, "event_uptime": v, "received_uptime": v+0.02, "point_bottom_left": [400,520-case["center"][1]]} for n,t,v in ((101,"down",down),(102,"up",up))],
                    "actions": [{"id": c, "uptime": up+0.03}]})
                time += 1

    @staticmethod
    def make_png(w,h):
        def chunk(k,v):
            return struct.pack(">I",len(v))+k+v+struct.pack(">I",zlib.crc32(k+v))
        return b"\x89PNG\r\n\x1a\n"+chunk(b"IHDR",struct.pack(">IIBBBBB",w,h,8,2,0,0,0))+chunk(b"IDAT",zlib.compress(bytes((w*3+1)*h)))+chunk(b"IEND",b"")

    def frame(self,a,c,p,i,t):
        row = dict(appearance=a, case=c, phase=p, index=i, capture_start=t, capture_end=t+0.1, pixel_size=[800,520])
        for kind,data in (("png",self.png),("ppm",self.rgb)):
            name = f"{a}-{c}-{p}-{i}.{kind}"
            (self.root/name).write_bytes(data)
            row[kind] = name; row[kind+"_sha256"] = digest(self.root/name)
        self.m["frames"].append(row)

    def check(self):
        (self.root/"manifest.json").write_text(json.dumps(self.m))
        # Validation reads real files; only expensive measurement loops are stubbed.
        with patch("analyze.footprint", return_value=None), patch("analyze.interior", return_value={}):
            return analyze(self.root)

    def test_valid(self):
        self.assertEqual(len(self.check()["measurements"]),84)

    def test_malformed_manifests(self):
        baseline = copy.deepcopy(self.m)
        mutations = [
            ("duplicate run", lambda m: m["runs"].__setitem__(1,copy.deepcopy(m["runs"][0]))),
            ("incomplete", lambda m: m["runs"].pop()),
            ("event/action", lambda m: m["runs"][0]["actions"][0].__setitem__("uptime",100)),
            ("wrong event", lambda m: m["runs"][0]["receipts"].reverse()),
            ("before capture", lambda m: (m["runs"][0]["dispatches"][0].update(event_uptime=102.05,dispatch_uptime=102.06), m["runs"][0]["receipts"][0].update(event_uptime=102.05))),
            ("frame index", lambda m: m["frames"][0].__setitem__("index",False)),
            ("frame index", lambda m: m["frames"][0].__setitem__("index",0.0)),
            ("layout", lambda m: m["layout_bounds"]["custom"].__setitem__(2,145)),
            ("path", lambda m: m["frames"][0].__setitem__("png","../escape.png")),
            ("hash", lambda m: m["frames"][0].__setitem__("png_sha256","0"*64)),
        ]
        for value in (float("nan"),float("inf"),-1,True):
            mutations.extend([
                ("capture time", lambda m,v=value: m["frames"][0].__setitem__("capture_start",v)),
                ("event time", lambda m,v=value: m["runs"][0]["actions"][0].__setitem__("uptime",v)),
                ("clock origin", lambda m,v=value: m.__setitem__("clock_origin_uptime",v)),
            ])
        for message, mutate in mutations:
            with self.subTest(message=message):
                self.m = copy.deepcopy(baseline); mutate(self.m)
                with self.assertRaisesRegex(ValueError,message): self.check()

    def test_changed_fixture_even_with_updated_hash(self):
        f = self.root/"fixture.json"
        data = json.loads(f.read_text()); data["cases"].reverse()
        f.write_text(json.dumps(data)); self.m["provenance"]["fixture.json_sha256"] = digest(f)
        with self.assertRaisesRegex(ValueError,"fixture mismatch"): self.check()

    def test_mismatched_dimensions_even_with_updated_hash(self):
        for kind,data in (("png",self.make_png(400,520)), ("ppm",b"P6\n400 520\n255\n"+bytes(400*520*3))):
            with self.subTest(kind=kind):
                row = self.m["frames"][0]; path = self.root/row[kind]
                original = path.read_bytes(); path.write_bytes(data)
                row[kind+"_sha256"] = digest(path)
                with self.assertRaisesRegex(ValueError,"dimension mismatch"): self.check()
                path.write_bytes(original); row[kind+"_sha256"] = digest(path)

    def test_symlink(self):
        row = self.m["frames"][0]; path = self.root/row["png"]
        target = self.root/"other.png"; path.rename(target); path.symlink_to(target)
        with self.assertRaisesRegex(ValueError,"symlink"): self.check()


class MeasurementTests(unittest.TestCase):
    def test_asymmetric_scaled_footprint_threshold(self):
        bg = bytes(10*8*3)
        data = bytearray(bg)
        data[(2*10+3)*3+1] = 8
        data[(4*10+7)*3+2] = 9
        data[(6*10+8)*3] = 7  # below threshold must not enlarge bounds
        self.assertEqual(footprint(data,bg,10,2,[1,0,4,4],8),[1.5,1,2.5,1.5])
        self.assertEqual(footprint(data,bg,10,2,[1,0,4,4],9),[3.5,2,0.5,0.5])
        self.assertIsNone(footprint(data,bg,10,2,[1,0,4,4],10))

    def test_roi_excludes_outside_pixel(self):
        bg = bytes(10*8*3)
        data = bytearray(bg); data[0] = 255
        self.assertIsNone(footprint(data,bg,10,1,[2,2,5,4],4))

    def test_interior_excludes_label_and_retains_signed_channels(self):
        before = bytes([10,35,42])*(120*80)
        data = bytearray(bytes([20,30,40])*(120*80))
        for y in range(30,50):
            for x in range(30,90):
                k=(y*120+x)*3; data[k:k+3]=b'\xff\xff\xff'
        self.assertEqual(interior(data,before,120,1,60,40),{
            "mean_rgb":[20,30,40],"signed_rgb_change_from_before":[10,-5,-2]})


if __name__ == "__main__":
    unittest.main()
