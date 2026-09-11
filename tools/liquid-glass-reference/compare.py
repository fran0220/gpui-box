"""Check GPUI/reference pairing only. Does not fit parameters or assert equivalence."""
import json
import math
from pathlib import Path
import sys
from validate import digest, ppm, require, validate


def pair(reference, candidate, root):
    """Reference must already pass validate(); candidate sidecars are sRGB P6."""
    require(candidate["schema"] == 1 and candidate["producer"] == "gpui", "unknown implementation provenance")
    for key in ("revision", "renderer", "parameters_sha256"):
        require(isinstance(candidate.get(key), str) and candidate[key].strip() not in ("", "unknown"), "unknown implementation provenance")
    require(candidate["fixture_sha256"] == reference["provenance"]["fixture_sha256"], "fixture mismatch")
    require(candidate["logical_size"] == reference["logical_size"] and candidate["scale"] == reference["scale"], "dimension mismatch")
    refs = {(f["appearance"], f["phase"], f["index"]): f for f in reference["frames"]}
    seen, result, names = set(), [], set()
    for frame in candidate["frames"]:
        key = frame["appearance"], frame["phase"], frame["index"]
        require(key in refs and key not in seen, "unexpected or duplicate candidate frame")
        seen.add(key)
        ref = refs[key]
        name = frame["rgb_file"]
        require(Path(name).name == name and name not in names, "unsafe or duplicate candidate path")
        names.add(name)
        path = Path(root) / name
        require(path.is_file(), "missing candidate frame")
        require(digest(path) == frame["rgb_file_sha256"], "candidate hash mismatch")
        w, h, _ = ppm(path)
        require([w, h] == ref["pixel_size"], "dimension mismatch")
        if frame["phase"] == "transition":
            time = frame["sample_time_after_trigger"]
            low, high = (ref[k]-ref["trigger_time"] for k in ("capture_start", "capture_end"))
            require(math.isfinite(time) and low <= time <= high, "timestamp mismatch: sample outside reference capture bracket")
        result.append({"reference": ref["rgb_file"], "candidate": name})
    require(seen == set(refs), "missing candidate frames")
    return {"status": "comparison-prerequisites-passed", "pairs": result, "calibration": "not performed"}


if __name__ == "__main__":
    ref_dir, candidate_dir = map(Path, sys.argv[1:3])
    reference = json.loads((ref_dir / "manifest.json").read_text())
    validate(ref_dir, reference)
    candidate = json.loads((candidate_dir / "candidate.json").read_text())
    print(json.dumps(pair(reference, candidate, candidate_dir), indent=2))
