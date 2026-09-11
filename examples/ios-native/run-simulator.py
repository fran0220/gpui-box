#!/usr/bin/env python3
"""Finite native HOST smoke run on an ephemeral, dynamically selected simulator."""
import json
import pathlib
import platform
import subprocess
import sys
import time


def run(*args):
    return subprocess.check_output(args, text=True, timeout=180).strip()


def cleanup_simulator(udid, app_id, launcher):
    errors = []

    def command(*args):
        try:
            result = subprocess.run(args, check=False, timeout=30, capture_output=True, text=True)
            if result.returncode:
                errors.append(f"{args}: exit {result.returncode}: {result.stderr.strip()}")
        except Exception as error:
            errors.append(f"{args}: {error}")

    if udid is not None:
        command("xcrun", "simctl", "terminate", udid, app_id)
    if launcher is not None:
        try:
            try:
                launcher.wait(timeout=10)
            except subprocess.TimeoutExpired:
                launcher.terminate()
                launcher.wait(timeout=10)
        except Exception as error:
            errors.append(f"launcher cleanup: {error}")
    if udid is not None:
        command("xcrun", "simctl", "shutdown", udid)
        command("xcrun", "simctl", "delete", udid)
    return errors


def select_device(catalog):
    runtimes = {
        r["identifier"]: r
        for r in catalog["runtimes"]
        if r.get("isAvailable")
        and r["identifier"].startswith("com.apple.CoreSimulator.SimRuntime.iOS-")
        and int(r["version"].split(".")[0]) >= 16
    }
    candidates = [
        (runtimes[runtime], device)
        for runtime, devices in catalog["devices"].items()
        if runtime in runtimes
        for device in devices
        if device.get("isAvailable")
        and ".iPhone-" in device.get("deviceTypeIdentifier", "")
    ]
    if not candidates:
        raise RuntimeError("No available iOS 16+ iPhone simulator configuration; install a runtime in Xcode")
    return max(candidates, key=lambda item: tuple(map(int, item[0]["version"].split("."))))


def main(gpui=False, reference=False):
    gpui = gpui or reference
    root = pathlib.Path(run("git", "-C", str(pathlib.Path(__file__).parent), "rev-parse", "--show-toplevel"))
    evidence = root / "target" / "ios-native" / ("reference-evidence" if reference else "gpui-evidence" if gpui else "evidence")
    evidence.mkdir(parents=True, exist_ok=True)
    metadata = {
        "scope": ("GPUI frame submission/completion; pixels require screenshot review; not real IME, VoiceOver, or device acceptance"
                  if gpui else "UIKit/Metal host fixture; not GPUI adapter, real IME, VoiceOver, or device acceptance"),
        "architecture": platform.machine(),
        "status": "failed",
        "application": "mobile-reference" if reference else "gpui-smoke" if gpui else "host-fixture",
    }
    app_id = "dev.gpui-box.ios-reference" if reference else "dev.gpui-box.ios-native" if gpui else "dev.gpui-box.ios-host-fixture"
    launcher = None
    udid = None
    cleanup_errors = []
    try:
        if platform.system() != "Darwin":
            raise RuntimeError("Native simulator execution requires macOS and full Xcode")
        metadata["commit"] = run("git", "-C", str(root), "rev-parse", "HEAD")
        metadata["xcode"] = run("xcodebuild", "-version")
        metadata["sdk"] = run("xcrun", "--sdk", "iphonesimulator", "--show-sdk-version")
        with (evidence / "build.log").open("w") as build_log:
            subprocess.run(
                ["bash", str(root / "examples/ios-native" / ("build-gpui-simulator.sh" if gpui else "build-simulator.sh"))] + (["--reference"] if reference else []),
                check=True, stdout=build_log, stderr=subprocess.STDOUT, timeout=1800 if gpui else 600,
            )
        runtime, template = select_device(json.loads(run("xcrun", "simctl", "list", "--json")))
        metadata["runtime"] = runtime
        metadata["device_type"] = template["deviceTypeIdentifier"]
        udid = run("xcrun", "simctl", "create", "GPUI native host smoke", template["deviceTypeIdentifier"], runtime["identifier"])
        metadata["udid"] = udid
        run("xcrun", "simctl", "boot", udid)
        run("xcrun", "simctl", "bootstatus", udid, "-b")
        run("xcrun", "simctl", "install", udid, str(root / "target/ios-native" / ("GpuiReference.app" if reference else "GpuiNative.app" if gpui else "GpuiHostFixture.app")))
        log_path = evidence / "console.log"
        with log_path.open("w") as log:
            launcher = subprocess.Popen(
                ["xcrun", "simctl", "launch", "--console", udid, app_id],
                stdout=log, stderr=subprocess.STDOUT, text=True,
            )
            required = ["IOS_REFERENCE_MOUNTED", "IOS_REFERENCE_FRAME_COMPLETED"] if reference else ["IOS_GPUI_WINDOW_OPENED", "IOS_GPUI_FRAME_COMPLETED"] if gpui else [
                "IOS_TEXT_PROTOCOL_PASS emoji-delete composition-replace utf16-range",
                "IOS_ACCESSIBILITY_BRIDGE_PASS identity action",
                "IOS_METAL_PRESENT_PASS frames=3",
            ]
            deadline = time.monotonic() + 60
            while True:
                output = log_path.read_text()
                if launcher.poll() is not None:
                    raise RuntimeError(f"Fixture exited before capture: {launcher.returncode}\n{output}")
                if all(marker in output for marker in required):
                    break
                if time.monotonic() >= deadline:
                    raise RuntimeError(f"Native fixture assertions did not finish within 60 seconds\n{output}")
                time.sleep(0.25)
            run("xcrun", "simctl", "io", udid, "screenshot", str(evidence / "host.png"))
            metadata["status"] = "native-reference-frame-completed" if reference else "native-gpui-frame-completed" if gpui else "native-host-smoke-passed"
            metadata["observed_markers"] = required
    except Exception as error:
        metadata["error"] = str(error)
        raise
    finally:
        cleanup_errors = cleanup_simulator(udid, app_id, launcher)
        if cleanup_errors:
            metadata["cleanup_errors"] = cleanup_errors
            metadata["status"] = "failed"
        (evidence / "run.json").write_text(json.dumps(metadata, indent=2) + "\n")
    if cleanup_errors:
        raise RuntimeError("Native host smoke cleanup failed; inspect run.json")
    print(f"Native host evidence: {evidence}. Screenshot still requires visual inspection.")


if __name__ == "__main__":
    try:
        if sys.argv[1:] not in ([], ["--gpui"], ["--reference"]):
            raise RuntimeError("Usage: run-simulator.py [--gpui|--reference]")
        main(gpui=sys.argv[1:] == ["--gpui"], reference=sys.argv[1:] == ["--reference"])
    except Exception as error:
        print(error, file=sys.stderr)
        sys.exit(1)
