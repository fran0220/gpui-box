"""Measured pixel footprints, NOT private material geometry or a GPUI fit."""
import hashlib
import json
import math
from pathlib import Path
import sys
import importlib.util

# Explicit sibling-file import: independent of cwd and unittest's module cache.
_spec = importlib.util.spec_from_file_location(
    "liquid_glass_reference_validate", Path(__file__).resolve().parents[1] / "validate.py")
_helpers = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_helpers)
require = _helpers.require
png_size = _helpers.png_size


def finite(value):
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def safe_file(root, name):
    require(isinstance(name, str) and name not in ("", ".", "..")
            and Path(name).name == name and "\\" not in name, "unsafe path")
    path = root / name
    require(not path.is_symlink() and path.is_file(), "missing file or symlink: " + name)
    return path


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def ppm(path):
    magic, size, maximum, data = Path(path).read_bytes().split(b"\n", 3)
    w, h = map(int, size.split())
    require(magic == b"P6" and maximum == b"255" and w > 0 and h > 0
            and len(data) == w*h*3, "invalid PPM")
    return w, h, data


def footprint(data, background, w, scale, rect, threshold):
    x, y, width, height = [round(n*scale) for n in rect]
    left, top, right, bottom = w, 10**9, -1, -1
    for row in range(y, y+height):
        for col in range(x, x+width):
            k = (row*w+col)*3
            if max(abs(data[k+c]-background[k+c]) for c in range(3)) >= threshold:
                left, top, right, bottom = min(left,col), min(top,row), max(right,col), max(bottom,row)
    if right < left:
        return None
    return [left/scale, top/scale, (right-left+1)/scale, (bottom-top+1)/scale]


def interior(data, before, w, scale, cx, cy):
    # Two strips away from the 15pt label and away from the rounded edges.
    delta, values, count = [0,0,0], [0,0,0], 0
    for ya, yb in [(cy-20,cy-14),(cy+14,cy+20)]:
        for y in range(round(ya*scale),round(yb*scale)):
            for x in range(round((cx-40)*scale),round((cx+40)*scale)):
                k=(y*w+x)*3
                for c in range(3):
                    delta[c] += data[k+c]-before[k+c]
                    values[c] += data[k+c]
                count += 1
    return {"mean_rgb": [n/count for n in values], "signed_rgb_change_from_before": [n/count for n in delta]}


def analyze(root):
    """Reject malformed evidence even under python -O; never change its files."""
    try:
        return _analyze(root)
    except (KeyError, TypeError, IndexError, OverflowError) as error:
        raise ValueError("malformed evidence schema: " + str(error)) from error


def _analyze(root):
    root = Path(root)
    require(not any(p.is_symlink() for p in (root, *root.parents)), "symlink evidence root")
    m = json.loads(safe_file(root, "manifest.json").read_text())
    f = json.loads(safe_file(root, "fixture.json").read_text())
    require(digest(root / "fixture.json") == "e350d1b6530c4bbd386e01a209944953e1db4d4e1de1757a502e196fdae3bc39", "fixed fixture mismatch")
    require(m["schema"] == "native-button-styles-evidence-1", "unknown evidence schema")
    require(m["backend"] == "ScreenCaptureKit.SCScreenshotManager.desktopIndependentWindow", "unknown backend")
    require(m["logical_size"] == f["canvas"] == [800,520] and type(m["scale"]) in (int, float) and m["scale"] in [1,2], "invalid canvas or scale")
    require(finite(m["clock_origin_uptime"]), "invalid clock origin")
    require(m["input"] == "synthetic NSEvent via NSApplication.postEvent; no HID or global cursor movement", "unknown input")
    settings = {"differentiate_without_color", "increase_contrast", "invert_colors", "reduce_motion", "reduce_transparency"}
    require(set(m["settings"]) == settings and all(v is False for v in m["settings"].values()), "unsupported settings")
    layout = {"glass": [316,84,168,56], "glass-prominent": [316,228,168,56], "custom": [328,376,144,48]}
    require(m["layout_bounds"] == layout, "fixed layout mismatch")
    for file in ["Buttons.swift","fixture.json","run.py"]:
        require(digest(safe_file(root, file)) == m["provenance"][file+"_sha256"], "source hash mismatch")
    require(len(m["frames"]) == 86 and len(m["runs"]) == 6, "incomplete frames or runs")
    expected = {(a,"all","background",0) for a in ["light","dark"]}
    expected |= {(a,c["id"],p,i) for a in ["light","dark"] for c in f["cases"] for p,n in [("before",1),("held",4),("release",9)] for i in range(n)}
    ordered = []
    for a in ("light", "dark"):
        ordered.append((a,"all","background",0))
        ordered.extend((a,c["id"],p,i) for c in f["cases"] for p,n in (("before",1),("held",4),("release",9)) for i in range(n))
    seen = set(); images = {}; previous = 0
    scale = m["scale"]
    for position, frame in enumerate(m["frames"]):
        require(type(frame["index"]) is int, "invalid frame index")
        key = tuple(frame[k] for k in ["appearance","case","phase","index"])
        require(key in expected and key not in seen, "unexpected or duplicate frame"); seen.add(key)
        require(key == ordered[position], "frame sequence mismatch")
        for kind in ["png","ppm"]:
            require(frame[kind] == "-".join(map(str, key)) + "." + kind, "frame path/identity mismatch")
            require(digest(safe_file(root, frame[kind])) == frame[kind+"_sha256"], "frame hash mismatch")
        w,h,data = ppm(root/frame["ppm"])
        require(all(type(n) is int for n in frame["pixel_size"])
                and [w,h] == frame["pixel_size"] == [800*scale,520*scale], "PPM dimension mismatch")
        require(png_size((root/frame["png"]).read_bytes()) == (w,h), "PNG dimension mismatch")
        start,end = frame["capture_start"],frame["capture_end"]
        require(finite(start) and finite(end) and previous <= start < end and end-start < 1, "invalid capture time")
        previous = end
        for marker in f["fiducials"]:
            x,y,mw,mh = marker["rect"]; k=(round((y+mh/2)*scale)*w+round((x+mw/2)*scale))*3
            require(all(abs(data[k+c]-marker["rgb"][c])<=12 for c in range(3)), "fiducial mismatch")
        images[key] = data
    require(seen == expected, "missing frames")
    events = []
    run_keys = set()
    expected_runs = {(a,c["id"]) for a in ("light", "dark") for c in f["cases"]}
    for run in m["runs"]:
        identity = (run["appearance"], run["case"])
        require(identity in expected_runs and identity not in run_keys, "unexpected or duplicate run")
        run_keys.add(identity)
        require(run["layout_bounds"] == layout, "run layout mismatch")
        ds,rs,acts=run["dispatches"],run["receipts"],run["actions"]
        require([e["number"] for e in ds] == [e["number"] for e in rs] == [101,102]
                and all(type(e["number"]) is int for e in ds+rs), "wrong event numbers")
        require([e["type"] for e in ds] == [e["type"] for e in rs] == ["down","up"], "wrong event order")
        require(len(acts)==1 and acts[0]["id"]==run["case"], "wrong action")
        require(all(finite(e[k]) for records, keys in ((ds, ("event_uptime", "dispatch_uptime")), (rs, ("event_uptime", "received_uptime")), (acts, ("uptime",))) for e in records for k in keys), "invalid event time")
        require(m["clock_origin_uptime"] <= ds[0]["event_uptime"] <= ds[0]["dispatch_uptime"]<=rs[0]["received_uptime"]<ds[1]["event_uptime"]<=ds[1]["dispatch_uptime"]<=rs[1]["received_uptime"]<=acts[0]["uptime"], "event/action time order")
        cx,cy = next(c["center"] for c in f["cases"] if c["id"] == run["case"])
        for d,r in zip(ds,rs):
            require(abs(d["event_uptime"]-r["event_uptime"])<1e-6, "event timestamp mismatch")
            require(r["point_bottom_left"] == [cx,520-cy], "receipt point mismatch")
        subset=[v for v in m["frames"] if v["appearance"]==run["appearance"] and v["case"]==run["case"]]
        for v in subset:
            start,end=[v[k]+m["clock_origin_uptime"] for k in ["capture_start","capture_end"]]
            if v["phase"]=="before": require(end <= ds[0]["event_uptime"], "before capture after down")
            if v["phase"]=="held": require(rs[0]["received_uptime"]<=start<end<ds[1]["event_uptime"], "held capture outside press")
            if v["phase"]=="release": require(acts[0]["uptime"]<=start, "release capture before action")
        events.append({"appearance":run["appearance"],"case":run["case"],"action_after_release_ms":1000*(acts[0]["uptime"]-ds[1]["dispatch_uptime"])})
    require(run_keys == expected_runs, "missing runs")
    measurements = []
    for frame in m["frames"]:
        if frame["phase"]=="background": continue
        a,c,phase,index=[frame[k] for k in ["appearance","case","phase","index"]]
        cx,cy=next(case["center"] for case in f["cases"] if case["id"]==c)
        data=images[a,c,phase,index]; bg=images[a,"all","background",0]; before=images[a,c,"before",0]
        row={"appearance":a,"case":c,"phase":phase,"index":index,"layout_rect":m["layout_bounds"][c],
             "footprint_rect_by_max_channel_threshold":{str(t):footprint(data,bg,w,scale,[cx-110,cy-40,220,80],t) for t in [4,8,16]},
             "interior":interior(data,before,w,scale,cx,cy)}
        measurements.append(row)
    return {"schema":"native-button-styles-measurements-1","manifest_sha256":digest(root/"manifest.json"),
            "method":"Layout rect from SwiftUI onGeometryChange. Pixel footprint: bbox of max absolute sRGB-channel delta against same-appearance background in 220x80pt ROI, thresholds 4/8/16 of 255; includes optical outsets, not exact shape boundary. Pixel precision 1/scale pt. Interior: x center±40, y center[-20,-14] and [14,20], excludes label.",
            "checks":"Fixed fixture/layout; 86 unique ordered frames/source and frame hashes/PNG and PPM dimensions/fiducials/finite time brackets/safe paths; six unique acknowledged native down/up pairs; before captures before down; held captures between down receipt and up event; release captures after action",
            "events":events,"measurements":measurements}


if __name__ == "__main__":
    result=analyze(sys.argv[1])
    print(json.dumps(result,indent=2))
