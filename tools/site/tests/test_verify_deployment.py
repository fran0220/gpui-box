"""Equal counts must not hide schema or identity drift on either hostname."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]


class VerifyDeployment(unittest.TestCase):
    def test_exact_catalog_and_equal_count_corruption(self):
        with tempfile.TemporaryDirectory() as directory:
            curl = Path(directory) / "curl"
            curl.write_text('''#!/usr/bin/env python3
import json, os, pathlib, sys
root = pathlib.Path(os.environ["FIXTURE_ROOT"])
developer = json.loads((root / "docs/developer-index.json").read_text())
tools = json.loads((root / "tools/mcp/tools.json").read_text())
url = sys.argv[-1]
mode = os.environ["FAULT"] if "gpui-kit.origingame.dev" in url else ""
if url.endswith("build-info.json"):
    counts = {key: len(developer[key]) for key in ["packages", "symbols", "components", "types", "themes", "guides", "recipes", "scenes"]}
    counts["tools"] = len(tools)
    print(json.dumps({"revision": "a" * 40, "counts": counts}))
else:
    request = json.loads(sys.argv[sys.argv.index("--data") + 1])
    if request["method"] == "tools/list":
        if mode == "schema": tools[0]["inputSchema"]["description"] = "changed"
        if mode == "name": tools[0]["name"] = "replaced"
        result = {"tools": tools}
    else:
        ids = ["component:" + item["name"] for item in developer["components"]]
        if mode == "identity": ids[0] = ids[1]
        result = {"structuredContent": {"matches": [{"id": id} for id in ids], "nextCursor": None}}
    print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": result}))
''')
            curl.chmod(0o755)
            for fault in ["", "name", "schema", "identity"]:
                with self.subTest(fault=fault):
                    env = dict(os.environ, PATH=f"{directory}:{os.environ['PATH']}",
                               FIXTURE_ROOT=str(ROOT), FAULT=fault)
                    result = subprocess.run(["bash", str(ROOT / "tools/site/verify-deployment.sh"), "a" * 40],
                                            env=env, capture_output=True, text=True, timeout=10)
                    self.assertEqual(result.returncode == 0, not fault, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
