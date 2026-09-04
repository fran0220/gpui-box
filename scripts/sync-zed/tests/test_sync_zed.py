import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

MODULE = Path(__file__).parents[1] / "sync_zed.py"
spec = importlib.util.spec_from_file_location("sync_zed", MODULE)
sync_zed = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sync_zed)


class UnitTests(unittest.TestCase):
    def config(self):
        return json.loads((MODULE.parent / "config.json").read_text())

    def state(self):
        return json.loads((MODULE.parent / "state.json").read_text())

    def init_repo(self, path):
        subprocess.run(["git", "init", "-q", "-b", "main", str(path)], check=True)
        subprocess.run(
            ["git", "-C", str(path), "config", "user.name", "Test"], check=True
        )
        subprocess.run(
            ["git", "-C", str(path), "config", "user.email", "test@example.com"],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(path), "config", "commit.gpgSign", "false"],
            check=True,
        )

    def commit(self, repo, message):
        subprocess.run(
            [
                "git",
                "-C",
                str(repo),
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                message,
            ],
            check=True,
        )
        return subprocess.check_output(
            ["git", "-C", str(repo), "rev-parse", "HEAD"], text=True
        ).strip()

    def test_committed_config_and_state_are_frozen_and_complete(self):
        config = self.config()
        state = self.state()
        sync_zed.validate_config(config)
        self.assertEqual(sync_zed.validate_state(config, state), [])
        self.assertEqual(sync_zed.provenance_errors(config, state), [])

    def test_config_rejects_mutating_mode(self):
        config = self.config()
        config["mode"] = "continuous-sync"
        with self.assertRaisesRegex(
            sync_zed.VerificationError, "freeze the historical import"
        ):
            sync_zed.validate_config(config)

    def test_config_rejects_short_source_sha(self):
        config = self.config()
        config["official_baseline"] = "abc"
        with self.assertRaisesRegex(sync_zed.VerificationError, "full lowercase SHA"):
            sync_zed.validate_config(config)

    def test_config_rejects_stale_filter_digest(self):
        config = self.config()
        config["filter_digest_sha256"] = "0" * 64
        with self.assertRaisesRegex(sync_zed.VerificationError, "frozen mapping list"):
            sync_zed.validate_config(config)

    def test_config_rejects_duplicate_overlay_revisions(self):
        config = self.config()
        revisions = config["fork_overlay"]["source_revisions"]
        revisions[1] = revisions[0]
        with self.assertRaisesRegex(sync_zed.VerificationError, "duplicates"):
            sync_zed.validate_config(config)

    def test_nested_remapping_uses_the_most_specific_source(self):
        mappings = [
            {"source": "crates/refineable", "destination": "vendor/refineable"},
            {
                "source": "crates/refineable/derive_refineable",
                "destination": "crates/derive",
            },
        ]
        self.assertEqual(
            sync_zed.remap(
                "crates/refineable/derive_refineable/src/lib.rs", mappings
            ),
            "crates/derive/src/lib.rs",
        )
        self.assertIsNone(sync_zed.remap("crates/unmapped/src/lib.rs", mappings))

    def test_filtering_and_package_renaming_use_only_mapped_files(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            self.init_repo(repo)
            (repo / "source/gpui/src").mkdir(parents=True)
            (repo / "source/shared/src").mkdir(parents=True)
            (repo / "source/ignored").mkdir(parents=True)
            (repo / "source/gpui/Cargo.toml").write_text(
                '[package]\nname = "gpui"\n\n[dependencies]\n'
                'shared = { package = "gpui_shared_string", version = "1" }\n'
            )
            (repo / "source/gpui/src/lib.rs").write_text("pub struct App;\n")
            (repo / "source/shared/Cargo.toml").write_text(
                '[package]\nname = "gpui_shared_string"\n'
            )
            (repo / "source/shared/src/lib.rs").write_text("pub struct Shared;\n")
            (repo / "source/ignored/product.rs").write_text("product_only!();\n")
            subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
            commit = self.commit(repo, "source")

            mappings = [
                {"source": "source/gpui", "destination": "crates/gpui"},
                {"source": "source/shared", "destination": "crates/shared"},
            ]
            entries = sync_zed.source_entries(repo, commit, mappings)
            self.assertEqual(
                set(entries),
                {
                    "crates/gpui/Cargo.toml",
                    "crates/gpui/src/lib.rs",
                    "crates/shared/Cargo.toml",
                    "crates/shared/src/lib.rs",
                },
            )
            entries, stats = sync_zed.rewrite_filtered_manifests(
                repo,
                entries,
                {
                    "crates/gpui/Cargo.toml": "gpui-box",
                    "crates/shared/Cargo.toml": "gpui-box-shared-string",
                },
            )
            tree = sync_zed.write_filtered_tree(repo, entries)
            manifest = subprocess.check_output(
                [
                    "git",
                    "-C",
                    str(repo),
                    "show",
                    f"{tree}:crates/gpui/Cargo.toml",
                ],
                text=True,
            )
            self.assertIn('name = "gpui-box"\n', manifest)
            self.assertIn('package = "gpui-box-shared-string"', manifest)
            self.assertEqual(stats["renamed"], 2)

    def test_conflict_subsystem_classification(self):
        examples = {
            "crates/gpui/src/window.rs": "window",
            "crates/gpui/src/scene.rs": "scene",
            "crates/gpui_macos/src/metal_renderer.rs": "renderer_macos",
            "crates/gpui_windows/src/directx_renderer.rs": "renderer_windows",
            "crates/gpui_wgpu/src/lib.rs": "renderer_wgpu",
            "crates/gpui_web/src/web_renderer.rs": "renderer_web",
            "crates/gpui_linux/src/vulkan_renderer.rs": "renderer_linux",
            "crates/gpui/src/renderer.rs": "renderer_other",
            "crates/gpui/src/elements/canvas.rs": "elements",
            "crates/gpui/src/elements/text.rs": "text",
            "crates/gpui/src/platform_view.rs": "platform_view",
            "crates/gpui/src/a11y.rs": "a11y",
            "crates/collections/src/lib.rs": "other_kit_unrelated",
        }
        self.assertEqual(
            {path: sync_zed.classify_subsystem(path) for path in examples},
            examples,
        )

    def test_state_rejects_missing_historical_coordinates(self):
        config = self.config()
        state = self.state()
        state["vendor_tip"] = None
        self.assertIn(
            "frozen import receipt requires vendor_tip",
            sync_zed.validate_state(config, state),
        )

    def test_state_rejects_changed_overlay_source_list(self):
        config = self.config()
        state = self.state()
        state["fork_overlay"]["source_revisions"] = state["fork_overlay"][
            "source_revisions"
        ][:-1]
        self.assertIn(
            "state/config disagree on fork_overlay.source_revisions",
            sync_zed.validate_state(config, state),
        )

    def test_parser_exposes_only_read_only_commands(self):
        choices = sync_zed.parser()._subparsers._group_actions[0].choices
        self.assertEqual(set(choices), {"verify", "status", "assess"})

    def test_marker_parser_rejects_partial_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            self.init_repo(directory)
            commit = self.commit(
                directory,
                f"partial\n\nzed-overlay-algorithm: {sync_zed.OVERLAY_ALGORITHM}",
            )
            with self.assertRaisesRegex(
                sync_zed.VerificationError, "incomplete integration markers"
            ):
                sync_zed.overlay_integration_markers(directory, commit)

    def test_repository_history_matches_the_frozen_receipt(self):
        config = self.config()
        state = self.state()
        head = subprocess.check_output(
            ["git", "-C", str(sync_zed.ROOT), "rev-parse", "HEAD"], text=True
        ).strip()
        self.assertEqual(sync_zed.historical_git_errors(config, state, head), [])

    def test_history_rejects_a_changed_overlay_source_revision(self):
        config = self.config()
        state = self.state()
        state["fork_overlay"]["source_revisions"] = copy.copy(
            state["fork_overlay"]["source_revisions"]
        )
        state["fork_overlay"]["source_revisions"][0] = "f" * 40
        head = subprocess.check_output(
            ["git", "-C", str(sync_zed.ROOT), "rev-parse", "HEAD"], text=True
        ).strip()
        errors = sync_zed.historical_git_errors(config, state, head)
        self.assertTrue(any("wrong source trailer" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
