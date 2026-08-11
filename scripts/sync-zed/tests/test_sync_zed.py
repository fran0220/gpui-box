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
    def unbootstrapped_state(self):
        state = json.loads((MODULE.parent / "state.json").read_text())
        for key in sync_zed.RECEIPT_KEYS:
            state[key] = None
        return state

    def git(self, repo, *args):
        return subprocess.check_output(
            ["git", "-C", str(repo), *args], text=True
        ).strip()

    def init_repo(self, path):
        subprocess.run(["git", "init", "-q", "-b", "main", str(path)], check=True)
        subprocess.run(["git", "-C", str(path), "config", "user.name", "Test"], check=True)
        subprocess.run(["git", "-C", str(path), "config", "user.email", "test@example.com"], check=True)

    def commit_file(self, repo, value, message):
        source = Path(repo) / "src"
        source.mkdir(exist_ok=True)
        (source / "value").write_text(value)
        subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
        subprocess.run(
            ["git", "-C", str(repo), "commit", "-q", "-m", message], check=True
        )
        return self.git(repo, "rev-parse", "HEAD")

    def replay_fixture(self, directory):
        bootstrap = Path(directory) / "bootstrap"
        official = Path(directory) / "official"
        self.init_repo(bootstrap)
        self.init_repo(official)
        bootstrap_revision = self.commit_file(bootstrap, "bootstrap", "bootstrap")
        baseline = self.commit_file(official, "baseline", "baseline")
        cursor = self.commit_file(official, "official", "official change")
        subprocess.run(
            ["git", "-C", str(official), "checkout", "-q", "--orphan", "unrelated"],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(official), "rm", "-q", "-rf", "."], check=True
        )
        unrelated = self.commit_file(official, "unrelated", "unrelated")
        config = {
            "official_baseline": baseline,
            "mappings": [{"source": "src", "destination": "vendor/src"}],
        }
        return config, bootstrap, bootstrap_revision, official, cursor, unrelated

    def integration_fixture(self, directory, marker_vendor=None):
        repo = Path(directory) / "repository"
        self.init_repo(repo)
        self.commit_file(repo, "main", "main")
        subprocess.run(
            ["git", "-C", str(repo), "checkout", "-q", "--orphan", "vendor"],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(repo), "rm", "-q", "-rf", "."], check=True
        )
        upstream = "1" * 40
        vendor = self.commit_file(
            repo, "vendor", f"vendor bootstrap\n\nzed-upstream: {upstream}"
        )
        subprocess.run(
            ["git", "-C", str(repo), "checkout", "-q", "main"], check=True
        )
        cursor = "a" * 40
        message = sync_zed.integration_message(
            "integrate bootstrap", marker_vendor or vendor, cursor
        )
        subprocess.run([
            "git", "-C", str(repo), "merge", "-q", "-s", "ours", "--no-ff",
            "--allow-unrelated-histories", "-m", message, "vendor",
        ], check=True)
        integration = self.git(repo, "rev-parse", "HEAD")
        state = {
            "history_algorithm": sync_zed.HISTORY_ALGORITHM,
            "bootstrap_vendor_tip": vendor,
            "vendor_tip": vendor,
            "last_synced_sha": cursor,
            "integration_commit": integration,
        }
        return repo, state

    def add_vendor_integration(self, repo, marked):
        subprocess.run(
            ["git", "-C", str(repo), "checkout", "-q", "vendor"], check=True
        )
        upstream = "2" * 40
        vendor = self.commit_file(
            repo, "vendor-two", f"vendor update\n\nzed-upstream: {upstream}"
        )
        subprocess.run(
            ["git", "-C", str(repo), "checkout", "-q", "main"], check=True
        )
        message = (
            sync_zed.integration_message("integrate update", vendor, "b" * 40)
            if marked
            else "integrate update without receipt markers"
        )
        subprocess.run([
            "git", "-C", str(repo), "merge", "-q", "-s", "ours", "--no-ff",
            "-m", message, "vendor",
        ], check=True)
        return self.git(repo, "rev-parse", "HEAD")

    def test_config_validation_rejects_short_sha(self):
        config = json.loads((MODULE.parent / "config.json").read_text())
        config["official_baseline"] = "abc"
        with self.assertRaises(sync_zed.SyncError):
            sync_zed.validate_config(config)

    def test_nested_and_arbitrary_remapping(self):
        mappings = [
            {"source": "crates/refineable", "destination": "vendor/refineable"},
            {"source": "crates/refineable/derive_refineable", "destination": "crates/derive"},
            {"source": "strange/location/perf", "destination": "tooling/perf"},
        ]
        self.assertEqual(sync_zed.remap("crates/refineable/src/lib.rs", mappings), "vendor/refineable/src/lib.rs")
        self.assertEqual(sync_zed.remap("crates/refineable/derive_refineable/src/lib.rs", mappings), "crates/derive/src/lib.rs")
        self.assertEqual(sync_zed.remap("strange/location/perf/a.rs", mappings), "tooling/perf/a.rs")

    def test_message_trailer(self):
        with tempfile.TemporaryDirectory() as directory:
            subprocess.run(["git", "init", "-q", directory], check=True)
            subprocess.run(["git", "-C", directory, "-c", "user.name=A", "-c", "user.email=a@b", "commit", "--allow-empty", "-m", "Subject"], check=True, stdout=subprocess.DEVNULL)
            sha = subprocess.check_output(["git", "-C", directory, "rev-parse", "HEAD"], text=True).strip()
            self.assertEqual(sync_zed.commit_message(directory, sha), f"Subject\n\nzed-upstream: {sha}\n")

    def test_filtered_noop(self):
        with tempfile.TemporaryDirectory() as directory:
            subprocess.run(["git", "init", "-q", directory], check=True)
            path = Path(directory) / "src"; path.mkdir(); (path / "a").write_text("same")
            subprocess.run(["git", "-C", directory, "add", "."], check=True)
            subprocess.run(["git", "-C", directory, "-c", "user.name=A", "-c", "user.email=a@b", "commit", "-m", "one"], check=True, stdout=subprocess.DEVNULL)
            one = subprocess.check_output(["git", "-C", directory, "rev-parse", "HEAD"], text=True).strip()
            subprocess.run(["git", "-C", directory, "commit", "--allow-empty", "-m", "irrelevant", "--author=A <a@b>"], check=True, stdout=subprocess.DEVNULL)
            two = subprocess.check_output(["git", "-C", directory, "rev-parse", "HEAD"], text=True).strip()
            mapping = [{"source": "src", "destination": "src"}]
            self.assertEqual(sync_zed.source_entries(directory, one, mapping), sync_zed.source_entries(directory, two, mapping))

    def test_filter_rejects_a_mapped_gitlink_instead_of_omitting_it(self):
        with tempfile.TemporaryDirectory() as directory:
            self.init_repo(directory)
            commit = self.commit_file(directory, "source", "source")
            subprocess.run([
                "git", "-C", directory, "update-index", "--add", "--cacheinfo",
                f"160000,{commit},src/submodule",
            ], check=True)
            subprocess.run(
                ["git", "-C", directory, "commit", "-q", "-m", "gitlink"], check=True
            )
            head = self.git(directory, "rev-parse", "HEAD")
            with self.assertRaisesRegex(sync_zed.SyncError, "unsupported mapped Git entry"):
                sync_zed.source_entries(
                    directory,
                    head,
                    [{"source": "src", "destination": "src"}],
                )

    def test_dry_run_bootstrap_does_not_call_git(self):
        old_load, old_run, old_provenance_errors = (
            sync_zed.load,
            sync_zed.run,
            sync_zed.provenance_errors,
        )
        config = json.loads((MODULE.parent / "config.json").read_text())
        state = self.unbootstrapped_state()
        sync_zed.load = lambda path: config if path == sync_zed.CONFIG else state
        sync_zed.run = lambda *args, **kwargs: self.fail("dry run invoked git")
        sync_zed.provenance_errors = lambda *_: []
        try:
            sync_zed.bootstrap(type("Args", (), {"dry_run": True})())
        finally:
            sync_zed.load, sync_zed.run, sync_zed.provenance_errors = (
                old_load,
                old_run,
                old_provenance_errors,
            )

    def test_development_validation_accepts_an_unbootstrapped_receipt(self):
        config = json.loads((MODULE.parent / "config.json").read_text())
        state = self.unbootstrapped_state()
        self.assertEqual(sync_zed.validate_state(config, state), [])

    def test_release_validation_requires_every_receipt_coordinate(self):
        config = json.loads((MODULE.parent / "config.json").read_text())
        state = self.unbootstrapped_state()
        errors = sync_zed.validate_state(config, state, release=True)
        for key in sync_zed.RECEIPT_KEYS:
            self.assertIn(f"release receipt requires {key}", errors)

    def test_partial_receipt_is_never_accepted_as_development_state(self):
        config = json.loads((MODULE.parent / "config.json").read_text())
        state = self.unbootstrapped_state()
        state["vendor_tip"] = "0" * 40
        self.assertIn(
            "receipt coordinates must be either all null or all full SHAs",
            sync_zed.validate_state(config, state),
        )

    def test_receipt_writer_updates_state_and_machine_provenance_together(self):
        with tempfile.TemporaryDirectory() as directory:
            config = json.loads((MODULE.parent / "config.json").read_text())
            state = json.loads((MODULE.parent / "state.json").read_text())
            for index, key in enumerate(sync_zed.RECEIPT_KEYS, start=1):
                state[key] = format(index, "040x")
            state_path = Path(directory) / "state.json"
            provenance_path = Path(directory) / "provenance.toml"
            provenance_path.write_text(sync_zed.PROVENANCE.read_text())
            old_state, old_provenance = sync_zed.STATE, sync_zed.PROVENANCE
            sync_zed.STATE, sync_zed.PROVENANCE = state_path, provenance_path
            try:
                sync_zed.write_receipt(config, state)
                self.assertEqual(json.loads(state_path.read_text()), state)
                self.assertEqual(sync_zed.provenance_errors(config, state), [])
            finally:
                sync_zed.STATE, sync_zed.PROVENANCE = old_state, old_provenance

    def test_release_cannot_bypass_source_replay(self):
        args = type("Args", (), {"release": True, "no_source_check": True})()
        with self.assertRaisesRegex(sync_zed.SyncError, "forbidden"):
            sync_zed.verify(args)

    def test_deterministic_replay_rejects_an_arbitrary_vendor_child(self):
        with tempfile.TemporaryDirectory() as directory:
            config, bootstrap, revision, official, cursor, _ = self.replay_fixture(directory)
            bootstrap_tip, vendor_tip = sync_zed.replay_vendor_history(
                config, bootstrap, revision, official, cursor
            )
            self.assertNotEqual(bootstrap_tip, vendor_tip)
            state = {
                "bootstrap_vendor_tip": bootstrap_tip,
                "vendor_tip": revision,
            }
            self.assertIn(
                "vendor_tip differs from deterministic replay of official first-parent history",
                sync_zed.replay_errors(
                    config, state, bootstrap, revision, official, cursor
                ),
            )

    def test_deterministic_replay_rejects_an_extra_vendor_commit(self):
        with tempfile.TemporaryDirectory() as directory:
            config, bootstrap, revision, official, cursor, _ = self.replay_fixture(directory)
            bootstrap_tip, _ = sync_zed.replay_vendor_history(
                config, bootstrap, revision, official, cursor
            )
            extra = self.commit_file(bootstrap, "extra", "unrecorded vendor commit")
            state = {
                "bootstrap_vendor_tip": bootstrap_tip,
                "vendor_tip": extra,
            }
            self.assertTrue(sync_zed.replay_errors(
                config, state, bootstrap, revision, official, cursor
            ))

    def test_replay_rejects_an_unrelated_but_existing_official_cursor(self):
        with tempfile.TemporaryDirectory() as directory:
            config, bootstrap, revision, official, _, unrelated = self.replay_fixture(directory)
            with self.assertRaisesRegex(sync_zed.SyncError, "first-parent history"):
                sync_zed.replay_vendor_history(
                    config, bootstrap, revision, official, unrelated
                )

    def test_integration_receipt_rejects_a_one_parent_commit(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory) / "repository"
            self.init_repo(repo)
            commit = self.commit_file(repo, "one", "one parent is not a merge")
            state = {
                "bootstrap_vendor_tip": commit,
                "vendor_tip": commit,
                "integration_commit": commit,
            }
            self.assertIn(
                "integration_commit must be an exact two-parent merge",
                sync_zed.integration_errors(state, commit, repo),
            )

    def test_receipt_rejects_a_newer_marked_integration(self):
        with tempfile.TemporaryDirectory() as directory:
            repo, state = self.integration_fixture(directory)
            head = self.add_vendor_integration(repo, marked=True)
            self.assertIn(
                "the newest marked Zed integration is not integration_commit",
                sync_zed.integration_errors(state, head, repo),
            )

    def test_receipt_rejects_a_newer_unmarked_vendor_merge(self):
        with tempfile.TemporaryDirectory() as directory:
            repo, state = self.integration_fixture(directory)
            head = self.add_vendor_integration(repo, marked=False)
            self.assertTrue(any(
                "newer unrecorded Zed vendor integration" in error
                for error in sync_zed.integration_errors(state, head, repo)
            ))

    def test_receipt_allows_a_later_unrelated_product_merge(self):
        with tempfile.TemporaryDirectory() as directory:
            repo, state = self.integration_fixture(directory)
            subprocess.run(
                ["git", "-C", str(repo), "checkout", "-q", "-b", "product"],
                check=True,
            )
            (repo / "product").write_text("product")
            subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
            subprocess.run(
                ["git", "-C", str(repo), "commit", "-q", "-m", "product"],
                check=True,
            )
            subprocess.run(
                ["git", "-C", str(repo), "checkout", "-q", "main"], check=True
            )
            subprocess.run([
                "git", "-C", str(repo), "merge", "-q", "--no-ff", "-m",
                "unrelated product merge", "product",
            ], check=True)
            head = self.git(repo, "rev-parse", "HEAD")
            self.assertEqual(sync_zed.integration_errors(state, head, repo), [])

    def test_receipt_rejects_integration_markers_for_another_vendor_tip(self):
        with tempfile.TemporaryDirectory() as directory:
            repo, state = self.integration_fixture(directory, marker_vendor="f" * 40)
            self.assertIn(
                "integration_commit markers disagree with the sync receipt",
                sync_zed.integration_errors(state, state["integration_commit"], repo),
            )

    def test_first_parent_sequence_records_a_merge_not_its_side_branch(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory) / "official"
            self.init_repo(repo)
            baseline = self.commit_file(repo, "baseline", "baseline")
            (repo / "unmapped").write_text("main")
            subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
            subprocess.run(
                ["git", "-C", str(repo), "commit", "-q", "-m", "main"], check=True
            )
            subprocess.run(
                ["git", "-C", str(repo), "checkout", "-q", "-b", "side", baseline],
                check=True,
            )
            side = self.commit_file(repo, "side", "side change")
            subprocess.run(
                ["git", "-C", str(repo), "checkout", "-q", "main"], check=True
            )
            subprocess.run(
                ["git", "-C", str(repo), "merge", "-q", "--no-ff", "-m", "merge", "side"],
                check=True,
            )
            merge = self.git(repo, "rev-parse", "HEAD")
            revisions = sync_zed.official_revisions(
                repo,
                baseline,
                merge,
                [{"source": "src", "destination": "src"}],
            )
            self.assertEqual(revisions, [merge])
            self.assertNotIn(side, revisions)


if __name__ == "__main__":
    unittest.main()
