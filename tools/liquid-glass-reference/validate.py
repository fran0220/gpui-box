"""Fail-closed evidence prerequisites, not a fitted glass model or visual approval."""
import hashlib
import json
import math
from pathlib import Path
import struct
import sys
import zlib

BACKEND = "ScreenCaptureKit.SCScreenshotManager.desktopIndependentWindow"


def require(value, message):
    if not value:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def ppm(path):
    magic, size, maximum, pixels = Path(path).read_bytes().split(b"\n", 3)
    width, height = map(int, size.split())
    require(magic == b"P6" and maximum == b"255", "unsupported RGB sidecar")
    require(len(pixels) == width * height * 3, "truncated RGB sidecar")
    return width, height, pixels


def png_size(data):
    require(data[:8] == b"\x89PNG\r\n\x1a\n", "invalid PNG")
    offset, chunks = 8, []
    while offset < len(data):
        require(offset + 12 <= len(data), "truncated PNG")
        size = struct.unpack(">I", data[offset:offset+4])[0]
        kind = data[offset+4:offset+8]
        payload = data[offset+8:offset+8+size]
        require(offset+12+size <= len(data), "truncated PNG chunk")
        crc = struct.unpack(">I", data[offset+8+size:offset+12+size])[0]
        require(zlib.crc32(kind+payload) == crc, "PNG CRC mismatch")
        chunks.append((kind, payload))
        offset += size+12
    require(chunks and chunks[0][0] == b"IHDR" and len(chunks[0][1]) == 13 and chunks[-1] == (b"IEND", b""), "incomplete PNG")
    require(any(k == b"IDAT" for k, _ in chunks), "PNG missing pixels")
    require(bool(zlib.decompress(b"".join(v for k, v in chunks if k == b"IDAT"))), "PNG empty pixels")
    return struct.unpack(">II", chunks[0][1][:8])


def validate(root, manifest):
    root = Path(root)
    m = manifest
    require(m["schema"] == 1 and m["capture_backend"] == BACKEND, "unknown capture provenance")
    p = m["provenance"]
    for key in ("os_build", "os_version", "xcode", "sdk", "architecture", "source_sha256", "fixture_sha256"):
        require(isinstance(p.get(key), str) and p[key].strip() not in ("", "unknown"), "unknown provenance: " + key)
    require(int(p["os_version"].split(".")[0]) >= 26, "unsupported OS")
    for key, file in (("source_sha256", "Reference.swift"), ("fixture_sha256", "fixture.json")):
        require(digest(root / file) == p[key], "source identity mismatch")
    fixture = json.loads((root / "fixture.json").read_text())
    require(m["logical_size"] == fixture["canvas"] == [960, 640], "logical size mismatch")
    require(m["scale"] in (1, 2), "unsupported capture scale")
    require(m["interaction"] == "programmatic-state-change; no pointer dispatched", "unknown interaction provenance")
    expected_access = {"reduce_transparency", "reduce_motion", "increase_contrast", "differentiate_without_color", "invert_colors"}
    require(set(m["accessibility"]) == expected_access and all(v is False for v in m["accessibility"].values()), "unknown or unsupported accessibility settings")
    expected = {(a, phase, i) for a in ("light", "dark") for phase, count in (("background", 1), ("static", 1), ("transition", 16)) for i in range(count)}
    seen, files, images = set(), set(), {}
    previous = -1
    for f in m["frames"]:
        key = (f["appearance"], f["phase"], f["index"])
        require(key in expected and key not in seen, "unexpected or duplicate frame")
        seen.add(key)
        start, end = f["capture_start"], f["capture_end"]
        require(all(isinstance(t, (float, int)) and math.isfinite(t) for t in (start, end)), "invalid timestamp")
        require(previous <= start < end and end - start < 1, "timestamp mismatch or capture too slow")
        previous = end
        require(bool(f["wall_end"]), "missing wall timestamp")
        if f["phase"] == "transition":
            require(math.isfinite(f["trigger_time"]) and f["trigger_time"] <= start, "transition timestamp mismatch")
        for field in ("file", "rgb_file"):
            name = f[field]
            require(Path(name).name == name and name not in files, "unsafe or duplicate frame path")
            files.add(name)
            require((root / name).is_file(), "missing frame: " + name)
            require(digest(root / name) == f[field + "_sha256"], "frame hash mismatch")
        w, h = png_size((root / f["file"]).read_bytes())
        require([w, h] == f["pixel_size"] == [int(v*m["scale"]) for v in m["logical_size"]], "dimension mismatch")
        rw, rh, pixels = ppm(root / f["rgb_file"])
        require((rw, rh) == (w, h), "sidecar dimension mismatch")
        def pixel(x, y):
            offset = (int(y*m["scale"])*w + int(x*m["scale"]))*3
            return pixels[offset:offset+3]
        for marker in fixture["fiducials"]:
            x, y, mw, mh = marker["rect"]
            require(all(abs(a-b) <= 12 for a, b in zip(pixel(x+mw/2, y+mh/2), marker["rgb"])), "capture content/fiducial mismatch")
        images[key] = pixels
    require(seen == expected, "missing frames")
    # Non-text strip inside regular-large: reject absent glass, not merely absent labels.
    w = int(960*m["scale"])
    def roi(pixels, rect):
        x, y, width, height = [int(v*m["scale"]) for v in rect]
        return b"".join(pixels[(row*w+x)*3:(row*w+x+width)*3] for row in range(y, y+height))
    for appearance in ("light", "dark"):
        background = roi(images[appearance, "background", 0], [100, 166, 120, 8])
        glass = roi(images[appearance, "static", 0], [100, 166, 120, 8])
        require(sum(abs(a-b) for a, b in zip(background, glass))/len(glass) > 2, "glass content absent")
        series = [f for f in m["frames"] if f["appearance"] == appearance and f["phase"] == "transition"]
        trigger = series[0]["trigger_time"]
        require(all(f["trigger_time"] == trigger for f in series), "inconsistent trigger timestamps")
        require(sum(f["capture_end"]-trigger < 0.8 for f in series) >= 3, "insufficient in-transition samples")
        require(series[-1]["capture_start"]-trigger >= 0.8, "missing settled transition frame")
        moving = {roi(images[appearance, "transition", i], [60, 440, 284, 144]) for i in range(16)}
        require(len(moving) >= 3, "no observable transition")
    return {"status": "capture-prerequisites-passed", "frames": len(seen), "visual_review": "required", "calibration": "not performed"}


if __name__ == "__main__":
    directory = Path(sys.argv[1])
    print(json.dumps(validate(directory, json.loads((directory / "manifest.json").read_text())), indent=2))
