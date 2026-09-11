"""Runtime selection tests; no simulator or subprocess is mocked as acceptance."""
import importlib.util
import pathlib
import json
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location("runner", pathlib.Path(__file__).with_name("run-simulator.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class SelectionTests(unittest.TestCase):
    def test_available_iphone_on_newest_available_ios_without_model_assumption(self):
        old = "com.apple.CoreSimulator.SimRuntime.iOS-17-5"
        new = "com.apple.CoreSimulator.SimRuntime.iOS-18-2"
        future = "com.apple.CoreSimulator.SimRuntime.iOS-99-0"
        phone = {"isAvailable": True, "deviceTypeIdentifier": "com.apple.CoreSimulator.SimDeviceType.iPhone-ArbitraryFutureModel"}
        runtime, device = runner.select_device({
            "runtimes": [
                {"identifier": old, "version": "17.5", "isAvailable": True},
                {"identifier": new, "version": "18.2", "isAvailable": True},
                {"identifier": future, "version": "99.0", "isAvailable": False},
            ],
            "devices": {old: [phone], new: [phone], future: [phone]},
        })
        self.assertEqual(runtime["identifier"], new)
        self.assertEqual(device, phone)

    def test_missing_runtime_fails_instead_of_claiming_a_skipped_success(self):
        with self.assertRaises(RuntimeError):
            runner.select_device({"runtimes": [], "devices": {}})


class CleanupTests(unittest.TestCase):
    def test_gpui_runner_rejects_host_fixture_markers_as_rendering_evidence(self):
        self.assert_rejects_other_fixture_markers(reference=False)

    def test_reference_runner_rejects_gpui_smoke_markers(self):
        self.assert_rejects_other_fixture_markers(reference=True)

    def assert_rejects_other_fixture_markers(self, reference):
        with tempfile.TemporaryDirectory() as directory:
            def command(*args):
                if args[0] == "git":
                    return directory if args[-1] == "--show-toplevel" else "fixture-commit"
                return "owned" if "create" in args else "{}"

            def launch(*args, **kwargs):
                kwargs["stdout"].write("IOS_TEXT_PROTOCOL_PASS emoji-delete composition-replace utf16-range\n"
                                       "IOS_ACCESSIBILITY_BRIDGE_PASS identity action\nIOS_METAL_PRESENT_PASS frames=3\n"
                                       "IOS_GPUI_WINDOW_OPENED\n")
                if reference:
                    kwargs["stdout"].write("IOS_GPUI_FRAME_COMPLETED\n")
                kwargs["stdout"].flush()
                return Mock(poll=Mock(return_value=None))

            with patch.object(runner, "run", side_effect=command), \
                 patch.object(runner.platform, "system", return_value="Darwin"), \
                 patch.object(runner, "select_device", return_value=({"identifier": "runtime"}, {"deviceTypeIdentifier": "type"})), \
                 patch.object(runner.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)), \
                 patch.object(runner.subprocess, "Popen", side_effect=launch), \
                 patch.object(runner.time, "monotonic", side_effect=[0, 61]):
                with self.assertRaisesRegex(RuntimeError, "assertions did not finish"):
                    runner.main(gpui=True, reference=reference)
            evidence = "reference-evidence" if reference else "gpui-evidence"
            metadata = json.loads((pathlib.Path(directory) / "target/ios-native" / evidence / "run.json").read_text())
            self.assertEqual(metadata["status"], "failed")
            self.assertEqual(metadata["application"], "mobile-reference" if reference else "gpui-smoke")
            self.assertNotIn("observed_markers", metadata)

    def test_all_cleanup_attempts_survive_exit_codes_and_launcher_failure(self):
        launcher = Mock()
        launcher.wait.side_effect = OSError("launcher lost")
        with patch.object(runner.subprocess, "run", side_effect=[
            subprocess.CompletedProcess([], 1, stderr="not running"),
            OSError("shutdown transport"),
            subprocess.CompletedProcess([], 7, stderr="delete refused"),
        ]) as command:
            errors = runner.cleanup_simulator("owned", "fixture", launcher)
        self.assertEqual([call.args[0][2] for call in command.call_args_list], ["terminate", "shutdown", "delete"])
        self.assertEqual(len(errors), 4)
        self.assertIn("exit 7: delete refused", errors[-1])
        self.assertIn("launcher lost", errors[1])

    def test_failure_after_create_records_metadata_and_still_attempts_delete(self):
        with tempfile.TemporaryDirectory() as directory:
            def command(*args):
                if args[0] == "git":
                    return directory if args[-1] == "--show-toplevel" else "fixture-commit"
                if "create" in args:
                    return "owned"
                if "boot" in args:
                    raise RuntimeError("boot failed")
                return "{}"

            with patch.object(runner, "run", side_effect=command), \
                 patch.object(runner.platform, "system", return_value="Darwin"), \
                 patch.object(runner, "select_device", return_value=({"identifier": "runtime"}, {"deviceTypeIdentifier": "type"})), \
                 patch.object(runner.subprocess, "run", return_value=subprocess.CompletedProcess([], 3, stderr="cleanup refused")) as cleanup:
                with self.assertRaisesRegex(RuntimeError, "boot failed"):
                    runner.main()
            metadata = json.loads((pathlib.Path(directory) / "target/ios-native/evidence/run.json").read_text())
            self.assertEqual(metadata["status"], "failed")
            self.assertEqual(metadata["error"], "boot failed")
            self.assertEqual(len(metadata["cleanup_errors"]), 3)
            self.assertEqual(cleanup.call_args_list[-1].args[0][2], "delete")


if __name__ == "__main__":
    unittest.main()
