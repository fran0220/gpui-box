"""Isolated native style experiment; new output directories only."""
import argparse
import hashlib
import json
import platform
import plistlib
from pathlib import Path
import shutil
import subprocess
import traceback

HERE = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    out = parser.parse_args().output.resolve()
    out.mkdir(parents=True, exist_ok=False)

    def command(args, timeout=30):
        with (out / "commands.log").open("a") as log:
            log.write("$ " + " ".join(map(str, args)) + "\n"); log.flush()
            result = subprocess.run(list(map(str, args)), stdout=log, stderr=subprocess.STDOUT, timeout=timeout)
            if result.returncode:
                raise RuntimeError(f"Command failed: {args[0]}")

    try:
        provenance = {key: subprocess.check_output(args, text=True, timeout=30).strip() for key, args in {
            "os": ["sw_vers", "-productVersion"], "build": ["sw_vers", "-buildVersion"],
            "xcode": ["xcodebuild", "-version"], "sdk": ["xcrun", "--sdk", "macosx", "--show-sdk-version"]}.items()}
        provenance["architecture"] = platform.machine()
        if int(provenance["os"].split(".")[0]) < 26 or int(provenance["sdk"].split(".")[0]) < 26:
            raise RuntimeError("macOS26+ and SDK26+ required")
        for name in ("Buttons.swift", "fixture.json", "run.py"):
            shutil.copy2(HERE / name, out / name)
            provenance[name + "_sha256"] = digest(out / name)
        bundle = out / "NativeButtons.app" / "Contents"
        (bundle / "MacOS").mkdir(parents=True)
        (bundle / "Info.plist").write_bytes(plistlib.dumps({"CFBundleIdentifier": "dev.gpui-box.native-button-styles", "CFBundleExecutable": "NativeButtons", "CFBundleName": "NativeButtons", "CFBundlePackageType": "APPL", "LSMinimumSystemVersion": "26.0", "NSHighResolutionCapable": True}))
        executable = bundle / "MacOS" / "NativeButtons"
        command(["xcrun", "swiftc", "-O", "-parse-as-library", "-swift-version", "5", "-target", "arm64-apple-macos26.0", out / "Buttons.swift", "-o", executable], 180)
        command(["codesign", "--force", "--sign", "-", bundle.parent])
        provenance["executable_sha256"] = digest(executable)
        (out / "provenance.json").write_text(json.dumps(provenance, indent=2))
        command([executable, out], 120)
        m = json.loads((out / "native.json").read_text())
        m["provenance"] = provenance
        for frame in m["frames"]:
            for key in ("png", "ppm"):
                frame[key + "_sha256"] = digest(out / frame[key])
        if len(m["frames"]) != 86 or len(m["runs"]) != 6:
            raise RuntimeError("Incomplete capture")
        (out / "manifest.json").write_text(json.dumps(m, indent=2))
        print(f"Captured {len(m['frames'])} frames; content inspection still required: {out}")
    except Exception:
        (out / "failure.txt").write_text(traceback.format_exc())
        raise


if __name__ == "__main__":
    main()
