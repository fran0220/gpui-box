"""Bounded macOS build/capture; only validated runs receive manifest.json."""
import argparse
import json
import platform
import plistlib
from pathlib import Path
import shutil
import subprocess
import sys
import traceback
from validate import digest, validate

HERE = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True, help="new directory; existing outputs refused")
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    def command(argv, timeout=30):
        with (out / "commands.log").open("a") as log:
            log.write("$ " + " ".join(map(str, argv)) + "\n")
            log.flush()
            try:
                result = subprocess.run(list(map(str, argv)), stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
            except subprocess.TimeoutExpired as error:
                log.write((error.stdout or b"").decode(errors="replace"))
                raise
            text = result.stdout.decode(errors="replace")
            log.write(text)
            if result.returncode:
                raise RuntimeError(f"Command failed ({result.returncode}): {argv[0]}; see commands.log")
            return text.strip()
    try:
        if platform.system() != "Darwin":
            raise RuntimeError("Native evidence requires macOS 26 and Xcode 26+, not this platform")
        provenance = {"os_version": command(["sw_vers", "-productVersion"]), "os_build": command(["sw_vers", "-buildVersion"]), "xcode": command(["xcodebuild", "-version"]), "sdk": command(["xcrun", "--sdk", "macosx", "--show-sdk-version"]), "architecture": platform.machine()}
        if int(provenance["os_version"].split(".")[0]) < 26 or int(provenance["sdk"].split(".")[0]) < 26:
            raise RuntimeError("macOS 26 and macOS SDK 26+ required")
        for name in ("Reference.swift", "fixture.json"):
            shutil.copy2(HERE / name, out / name)
        provenance.update(source_sha256=digest(out / "Reference.swift"), fixture_sha256=digest(out / "fixture.json"))
        bundle = out / "GlassReference.app" / "Contents"
        (bundle / "MacOS").mkdir(parents=True)
        (bundle / "Info.plist").write_bytes(plistlib.dumps({"CFBundleIdentifier": "dev.gpui-box.liquid-glass-reference", "CFBundleName": "GlassReference", "CFBundleExecutable": "GlassReference", "CFBundlePackageType": "APPL", "LSMinimumSystemVersion": "26.0", "NSHighResolutionCapable": True}))
        executable = bundle / "MacOS" / "GlassReference"
        command(["xcrun", "swiftc", "-O", "-parse-as-library", "-swift-version", "5", "-target", platform.machine() + "-apple-macos26.0", "-sdk", command(["xcrun", "--sdk", "macosx", "--show-sdk-path"]), out / "Reference.swift", "-o", executable], timeout=180)
        command(["codesign", "--force", "--sign", "-", bundle.parent])
        provenance["executable_sha256"] = digest(executable)
        (out / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
        command([executable, out, out / "fixture.json"], timeout=90)
        manifest = json.loads((out / "native.json").read_text())
        manifest["provenance"] = provenance
        for frame in manifest["frames"]:
            for key in ("file", "rgb_file"):
                frame[key + "_sha256"] = digest(out / frame[key])
        (out / "candidate.json").write_text(json.dumps(manifest, indent=2) + "\n")
        report = validate(out, manifest)
        (out / "validation.json").write_text(json.dumps(report, indent=2) + "\n")
        (out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        pointer = json.loads((out / "pointer-native.json").read_text())
        pointer.update(provenance=provenance, capture_backend=manifest["capture_backend"],
                       logical_size=manifest["logical_size"], scale=manifest["scale"],
                       baseline_manifest_sha256=digest(out / "manifest.json"),
                       validation="native dispatch/action assertions passed; separate visual review required; not optical calibration")
        for frame in pointer["frames"]:
            for key in ("file", "rgb_file"):
                frame[key + "_sha256"] = digest(out / frame[key])
        (out / "pointer.json").write_text(json.dumps(pointer, indent=2) + "\n")
        print(json.dumps(report))
        return 0
    except Exception as error:
        (out / "failure.json").write_text(json.dumps({"status": "failed-not-reference", "error": str(error), "platform": platform.platform()}, indent=2) + "\n")
        (out / "diagnostic.log").write_text(traceback.format_exc())
        print(f"Reference capture failed; diagnostics: {out}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
