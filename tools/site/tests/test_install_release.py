"""Exercise the real installer with isolated filesystem and external commands."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import shutil
import socketserver
import tempfile
import threading
import time
import unittest

ROOT = Path(__file__).resolve().parents[3]
INSTALLER = ROOT / "tools/site/ops/install-release.sh"


class InstallRelease(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.base = self.root / "installed"
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.env = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}",
                        GPUI_BOX_INSTALL_BASE=str(self.base),
                        GPUI_BOX_INSTALL_LOCK=str(self.root / "lock"))
        for name, body in {
            "id": "echo 0",
            "file": "echo 'ELF 64-bit x86-64'",
            "readelf": "exit 0",
            "runuser": "exit 0",
            "chown": "exit 0",
            "systemctl": "exit 0",
            "nginx": "exit 0",
            "sleep": "exit 0",
            "curl": '''
case " $* " in *" --connect-timeout 1 --max-time 2 "*) ;; *) exit 99;; esac
if [[ "${FAULT:-}" == timeout ]]; then
  if [[ ! -f "$GPUI_BOX_INSTALL_BASE/probed" ]]; then
    touch "$GPUI_BOX_INSTALL_BASE/probed"
    # The real curl connects to a silent local TCP peer on the first attempt.
    # Subsequent retries model the same timeout without adding a minute.
    args=("$@")
    unset 'args[${#args[@]}-1]'
    exec "$REAL_CURL" "${args[@]}" "$HANG_URL"
  fi
  exit 28
fi
revision=$(cat "$GPUI_BOX_INSTALL_BASE/current/REVISION")
printf '{"status":"ok","revision":"%s","toolCount":10}' "$revision"
''',
        }.items():
            script = self.bin / name
            script.write_text("#!/usr/bin/env bash\nset -eu\n" + body + "\n")
            script.chmod(0o755)

    def bundle(self, revision, expected):
        parent = Path(tempfile.mkdtemp(dir=self.root))
        bundle = parent / f"gpui-box-linux-x64-{revision[:12]}"
        contents = {
            "REVISION": revision,
            "EXPECTED_REVISION": expected,
            "bin/gpui-box-mcp": "executable fixture",
            "public/build-info.json": json.dumps({"schema": 1, "revision": revision,
                "catalogSchema": 1, "counts": {"tools": 10, "components": 1, "symbols": 1, "scenes": 1}}),
        }
        for name in ["public/index.html", "public/mcp/index.html", "public/mcp/tools.json",
                     "public/api-index.json", "public/developer-index.json"]:
            contents[name] = "{}"
        sums = []
        for name, content in contents.items():
            path = bundle / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
            sums.append(f"{hashlib.sha256(content.encode()).hexdigest()}  {name}\n")
        (bundle / "SHA256SUMS").write_text("".join(sums))
        return bundle

    def install(self, revision, expected, fault="", success=True):
        result = subprocess.run(["bash", str(INSTALLER), str(self.bundle(revision, expected))],
                                env=dict(self.env, FAULT=fault), capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode == 0, success, result.stdout + result.stderr)
        # No completed attempt, even a failed health check, may keep the lock.
        subprocess.run(["flock", "-n", str(self.root / "lock"), "true"], check=True)
        return result

    def test_stale_activation_redeploy_and_explicit_recovery(self):
        a, b, c = (letter * 40 for letter in "abc")
        self.install(a, "none")
        self.install(b, a)
        refused = self.install(c, a, success=False)
        self.assertIn("stale activation", refused.stderr)
        self.assertEqual((self.base / "current/REVISION").read_text(), b)
        self.install(b, a)  # Idempotent recheck, even with the old expectation.
        self.install(a, b)  # Explicit recovery to a previously installed release.
        self.assertEqual((self.base / "current/REVISION").read_text(), a)

    def test_health_timeout_rolls_back_and_releases_lock(self):
        stopped = threading.Event()

        class SilentPeer(socketserver.BaseRequestHandler):
            def handle(self):
                stopped.wait(10)

        server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), SilentPeer)
        server.daemon_threads = True
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        self.addCleanup(stopped.set)
        self.env["REAL_CURL"] = shutil.which("curl")
        self.env["HANG_URL"] = f"http://127.0.0.1:{server.server_address[1]}/healthz"
        a, b = "a" * 40, "b" * 40
        self.install(a, "none")
        started = time.monotonic()
        failed = self.install(b, a, fault="timeout", success=False)
        self.assertGreaterEqual(time.monotonic() - started, 2)
        self.assertIn("rolling back", failed.stderr)
        self.assertEqual((self.base / "current/REVISION").read_text(), a)
        self.install(b, a)


if __name__ == "__main__":
    unittest.main()
