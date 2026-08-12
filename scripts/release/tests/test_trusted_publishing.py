import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "trusted_publishing.py"
SPEC = importlib.util.spec_from_file_location("trusted_publishing", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class FakeRegistry:
    def __init__(self):
        self.configs = {}
        self.requests = []

    def request(
        self,
        method,
        path,
        *,
        payload=None,
        authenticated=True,
        expected=(200,),
    ):
        self.requests.append((method, path, payload, authenticated, expected))
        if path.endswith("/owners"):
            return 200, {
                "users": [{"login": MODULE.EXPECTED_OWNER, "kind": "user"}]
            }
        if path.startswith("/crates/") and method == "GET":
            return 200, {"version": {}}
        if path.startswith("/trusted_publishing/github_configs?crate="):
            name = path.rsplit("=", 1)[1]
            configs = self.configs.get(name, [])
            return 200, {
                "github_configs": configs,
                "meta": {"total": len(configs)},
            }
        if path == "/trusted_publishing/github_configs" and method == "POST":
            config = payload["github_config"]
            self.configs.setdefault(config["crate"], []).append(config)
            return 200, {"github_config": config}
        if path.startswith("/crates/") and method == "PATCH":
            return 200, {"crate": {"trustpub_only": True}}
        raise AssertionError((method, path))


class TrustedPublishingTests(unittest.TestCase):
    def test_authority_parser_selects_publishable_unique_packages(self):
        with tempfile.TemporaryDirectory() as directory:
            authority = Path(directory) / "authority.toml"
            authority.write_text(
                '\n'.join(
                    [
                        '[[package]]',
                        'name = "one"',
                        'version = "0.1.0"',
                        'publish = true',
                        '[[package]]',
                        'name = "private"',
                        'version = "0.1.0"',
                        'publish = false',
                    ]
                )
            )
            self.assertEqual(
                MODULE.packages_from_authority(authority),
                [MODULE.Package("one", "0.1.0")],
            )

    def test_configure_is_idempotent_without_duplicate_posts(self):
        package = MODULE.Package("one", "0.1.0")
        registry = FakeRegistry()
        MODULE.configure(registry, [package])
        MODULE.configure(registry, [package])
        posts = [request for request in registry.requests if request[0] == "POST"]
        self.assertEqual(len(posts), 1)
        self.assertEqual(registry.configs["one"], [MODULE.desired_for(package)])

    def test_configure_refuses_conflicting_configuration(self):
        packages = [MODULE.Package("one", "0.1.0"), MODULE.Package("two", "0.1.0")]
        registry = FakeRegistry()
        registry.configs["two"] = [
            {**MODULE.desired_for(packages[1]), "repository_name": "other"}
        ]
        with self.assertRaises(MODULE.RegistryError):
            MODULE.configure(registry, packages)
        self.assertFalse(any(request[0] == "POST" for request in registry.requests))

    def test_hardening_verifies_all_configs_before_mutation(self):
        packages = [MODULE.Package("one", "0.1.0"), MODULE.Package("two", "0.1.0")]
        registry = FakeRegistry()
        registry.configs = {
            package.name: [MODULE.desired_for(package)] for package in packages
        }
        MODULE.harden(registry, packages)
        mutations = [request[:2] for request in registry.requests if request[0] != "GET"]
        self.assertEqual(
            mutations,
            [
                ("PATCH", "/crates/one"),
                ("PATCH", "/crates/two"),
            ],
        )


if __name__ == "__main__":
    unittest.main()
