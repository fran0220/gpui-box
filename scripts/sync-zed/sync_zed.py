#!/usr/bin/env python3
"""Reproducible filtered Zed history synchronizer for GPUI Box."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
CONFIG = HERE / "config.json"
STATE = HERE / "state.json"
PROVENANCE = ROOT / "provenance.toml"
VERSION = "1.2.0"
HISTORY_ALGORITHM = "first-parent-v1"
SHA = re.compile(r"^[0-9a-f]{40}$")
RECEIPT_KEYS = ("bootstrap_vendor_tip", "vendor_tip", "last_synced_sha", "integration_commit")


class SyncError(RuntimeError):
    pass


def run(args, cwd=ROOT, env=None, input_text=None, check=True):
    result = subprocess.run(args, cwd=cwd, env=env, input=input_text, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if check and result.returncode:
        raise SyncError(f"command failed: {' '.join(map(str, args))}\n{result.stderr.strip()}")
    return result


def load(path):
    return json.loads(path.read_text())


def deterministic_env(extra=None):
    env = os.environ.copy()
    env.update({
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
    })
    if extra:
        env.update(extra)
    return env


def package_name(path):
    in_package = False
    for line in path.read_text().splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            in_package = stripped == "[package]"
        elif in_package:
            match = re.fullmatch(r'name\s*=\s*"([^"]+)"\s*', stripped)
            if match:
                return match.group(1)
    raise SyncError("missing [package] name")


def validate_config(config):
    if config.get("schema_version") != 1:
        raise SyncError("unsupported config schema")
    if config.get("filter_schema_version") != 1:
        raise SyncError("unsupported filter schema")
    if config.get("history_algorithm") != HISTORY_ALGORITHM:
        raise SyncError("unsupported history algorithm")
    for key in ("official_baseline", "bootstrap_revision"):
        if not SHA.fullmatch(config.get(key, "")):
            raise SyncError(f"{key} must be a full lowercase SHA")
    seen_source, seen_dest = set(), set()
    for item in config.get("mappings", []):
        source, dest = item.get("source", ""), item.get("destination", "")
        for value in (source, dest):
            path = PurePosixPath(value)
            if not value or path.is_absolute() or ".." in path.parts:
                raise SyncError(f"unsafe mapping path: {value}")
        if source in seen_source or dest in seen_dest:
            # Nested mappings are intentional; exact duplicates are not.
            raise SyncError(f"duplicate mapping: {source} -> {dest}")
        seen_source.add(source); seen_dest.add(dest)
    if not config.get("mappings"):
        raise SyncError("at least one mapping is required")


def remap(path, mappings):
    matches = [m for m in mappings if path == m["source"] or path.startswith(m["source"] + "/")]
    if not matches:
        return None
    mapping = max(matches, key=lambda m: len(PurePosixPath(m["source"]).parts))
    suffix = path[len(mapping["source"]):].lstrip("/")
    return mapping["destination"] + (("/" + suffix) if suffix else "")


def require_clean():
    if run(["git", "status", "--porcelain"]).stdout:
        raise SyncError("worktree must be clean and fully committed")


def fetch_temp(url, revision=None):
    temp = tempfile.TemporaryDirectory(prefix="sync-zed-")
    run(["git", "init", "--bare", temp.name])
    spec = revision or "HEAD"
    run(["git", "-C", temp.name, "fetch", "--quiet", "--no-tags", url, spec])
    return temp, run(["git", "-C", temp.name, "rev-parse", "FETCH_HEAD"]).stdout.strip()


def source_entries(repo, commit, mappings):
    paths = [m["source"] for m in mappings]
    out = run(["git", "-C", str(repo), "ls-tree", "-r", "-z", commit, "--", *paths]).stdout
    entries = {}
    for record in out.rstrip("\0").split("\0") if out else []:
        metadata, source = record.split("\t", 1)
        mode, kind, oid = metadata.split()
        if kind != "blob":
            raise SyncError(f"unsupported mapped Git entry {kind} at {source}")
        destination = remap(source, mappings)
        prior = entries.get(destination)
        value = (mode, oid, source)
        if prior and prior != value:
            raise SyncError(f"mapping collision at {destination}")
        entries[destination] = value
    return entries


def filtered_tree(repo, commit, mappings, object_dir):
    entries = source_entries(repo, commit, mappings)
    with tempfile.NamedTemporaryFile(prefix="sync-zed-index-", delete=True) as index:
        env = deterministic_env({"GIT_INDEX_FILE": index.name})
        index.close()
        run(["git", "read-tree", "--empty"], cwd=object_dir, env=env)
        for destination, (mode, oid, _) in sorted(entries.items()):
            run(["git", "update-index", "--add", "--cacheinfo", mode, oid, destination], cwd=object_dir, env=env)
        return run(["git", "write-tree"], cwd=object_dir, env=env).stdout.strip()


def commit_message(repo, commit):
    message = run(
        ["git", "-C", str(repo), "show", "-s", "--format=%B", commit],
        env=deterministic_env(),
    ).stdout.rstrip()
    return f"{message}\n\nzed-upstream: {commit}\n"


def integration_message(subject, vendor_tip, cursor):
    return (
        f"{subject}\n\n"
        f"zed-sync-algorithm: {HISTORY_ALGORITHM}\n"
        f"zed-vendor-tip: {vendor_tip}\n"
        f"zed-upstream-cursor: {cursor}\n"
    )


def integration_markers(repo, commit):
    message = run([
        "git", "-C", str(repo), "show", "-s", "--format=%B", commit
    ]).stdout
    names = ("zed-sync-algorithm", "zed-vendor-tip", "zed-upstream-cursor")
    values = {}
    for line in message.splitlines():
        for name in names:
            prefix = name + ": "
            if line.startswith(prefix):
                if name in values:
                    raise SyncError(f"duplicate {name} marker in integration {commit}")
                values[name] = line[len(prefix):]
    if not values:
        return None
    missing = [name for name in names if name not in values]
    if missing:
        raise SyncError(
            f"incomplete integration markers in {commit}: missing {', '.join(missing)}"
        )
    if not SHA.fullmatch(values["zed-vendor-tip"]):
        raise SyncError(f"invalid zed-vendor-tip marker in integration {commit}")
    if not SHA.fullmatch(values["zed-upstream-cursor"]):
        raise SyncError(f"invalid zed-upstream-cursor marker in integration {commit}")
    return values


def is_synthetic_vendor_commit(repo, commit):
    message = run([
        "git", "-C", str(repo), "show", "-s", "--format=%B", commit
    ]).stdout
    trailers = [
        line[len("zed-upstream: "):]
        for line in message.splitlines()
        if line.startswith("zed-upstream: ")
    ]
    return bool(trailers) and SHA.fullmatch(trailers[-1]) is not None


def commit_filtered(repo, upstream, parent, config, object_dir=ROOT):
    # Import objects, but no refs, so the resulting vendor commit remains valid after
    # the temporary source repository disappears.
    run(["git", "fetch", "--quiet", "--no-tags", str(repo), upstream], cwd=object_dir)
    tree = filtered_tree(repo, upstream, config["mappings"], object_dir)
    if parent and tree == run(["git", "show", "-s", "--format=%T", parent], cwd=object_dir).stdout.strip():
        return parent, False
    fmt = "%an%x00%ae%x00%aI%x00%cn%x00%ce%x00%cI"
    values = run(
        ["git", "-C", str(repo), "show", "-s", f"--format={fmt}", upstream],
        env=deterministic_env(),
    ).stdout.strip().split("\0")
    if len(values) != 6 or any("\n" in value or "\r" in value for value in values):
        raise SyncError(f"cannot reproduce malformed commit identity metadata for {upstream}")
    env = deterministic_env()
    for key, value in zip(("GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_AUTHOR_DATE", "GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE"), values):
        env[key] = value
    args = ["git", "commit-tree", tree]
    if parent: args += ["-p", parent]
    oid = run(args, cwd=object_dir, env=env, input_text=commit_message(repo, upstream)).stdout.strip()
    return oid, True


def provenance_sync_values():
    values = {}
    in_sync = False
    for line in PROVENANCE.read_text().splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            in_sync = stripped == "[sync]"
            continue
        if not in_sync:
            continue
        match = re.fullmatch(r'([a-z_]+)\s*=\s*("[^"]*"|true|false|[0-9]+)', stripped)
        if not match:
            continue
        key, raw = match.groups()
        if key in values:
            raise SyncError(f"duplicate provenance [sync] key: {key}")
        if raw.startswith('"'):
            values[key] = json.loads(raw)
        elif raw in ("true", "false"):
            values[key] = raw == "true"
        else:
            values[key] = int(raw)
    return values


def provenance_errors(config, state):
    actual = provenance_sync_values()
    expected = {
        "filter_schema_version": config["filter_schema_version"],
        "history_algorithm": config["history_algorithm"],
        "history_bootstrapped": state["vendor_tip"] is not None,
        **{key: state[key] or "" for key in RECEIPT_KEYS},
    }
    return [
        f"provenance.toml [sync] {key} differs from the sync receipt"
        for key, value in expected.items()
        if actual.get(key) != value
    ]


def write_receipt(config, state):
    replacements = {
        "history_bootstrapped": "true" if state["vendor_tip"] is not None else "false",
        **{key: json.dumps(state[key] or "") for key in RECEIPT_KEYS},
    }
    lines = PROVENANCE.read_text().splitlines()
    in_sync = False
    found = set()
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("["):
            in_sync = stripped == "[sync]"
            continue
        if not in_sync:
            continue
        match = re.match(r"^([a-z_]+)\s*=", stripped)
        if match and match.group(1) in replacements:
            key = match.group(1)
            if key in found:
                raise SyncError(f"duplicate provenance [sync] key: {key}")
            lines[index] = f"{key} = {replacements[key]}"
            found.add(key)
    missing = set(replacements) - found
    if missing:
        raise SyncError(
            "provenance.toml is missing sync receipt keys: " + ", ".join(sorted(missing))
        )
    expected_static = {
        "filter_schema_version": config["filter_schema_version"],
        "history_algorithm": config["history_algorithm"],
    }
    actual_static = provenance_sync_values()
    for key, value in expected_static.items():
        if actual_static.get(key) != value:
            raise SyncError(f"provenance.toml [sync] {key} differs from sync config")
    STATE.write_text(json.dumps(state, indent=2) + "\n")
    PROVENANCE.write_text("\n".join(lines) + "\n")


def validate_state(config, state, release=False):
    errors = []
    if state.get("schema_version") != config.get("schema_version"):
        errors.append("state/config disagree on schema_version")
    for key in (
        "official_url",
        "official_baseline",
        "bootstrap_url",
        "bootstrap_revision",
        "vendor_ref",
        "filter_schema_version",
        "history_algorithm",
    ):
        if state.get(key) != config.get(key):
            errors.append(f"state/config disagree on {key}")
    if state.get("tool_version") != VERSION:
        errors.append("state/tool version disagree")
    for key in ("official_baseline", "bootstrap_revision"):
        if not SHA.fullmatch(state.get(key, "")):
            errors.append(f"invalid state {key}")
    for key in RECEIPT_KEYS:
        value = state.get(key)
        if value is not None and not SHA.fullmatch(value):
            errors.append(f"invalid cursor {key}: {value}")
        if release and not SHA.fullmatch(value or ""):
            errors.append(f"release receipt requires {key}")
    present = [state.get(key) is not None for key in RECEIPT_KEYS]
    if any(present) and not all(present):
        errors.append("receipt coordinates must be either all null or all full SHAs")
    return errors


def commit_exists(commit, repo=ROOT):
    return run(["git", "-C", str(repo), "cat-file", "-e", f"{commit}^{{commit}}"], check=False).returncode == 0


def is_ancestor(ancestor, descendant, repo=ROOT):
    return run(["git", "-C", str(repo), "merge-base", "--is-ancestor", ancestor, descendant], check=False).returncode == 0


def is_first_parent_ancestor(ancestor, descendant, repo=ROOT):
    history = run(["git", "-C", str(repo), "rev-list", "--first-parent", descendant]).stdout.splitlines()
    return ancestor in history


def official_revisions(repo, start, end, mappings):
    if not is_first_parent_ancestor(start, end, repo):
        raise SyncError(f"official cursor {end} does not descend from {start} on first-parent history")
    return run([
        "git", "-C", str(repo), "rev-list", "--first-parent", "--reverse", "--full-history",
        f"{start}..{end}", "--", *[mapping["source"] for mapping in mappings],
    ]).stdout.splitlines()


def replay_vendor_history(config, bootstrap_repo, bootstrap_revision, official_repo, cursor):
    with tempfile.TemporaryDirectory(prefix="sync-zed-replay-") as replay:
        run(["git", "init", "--bare", replay])
        bootstrap_tip, _ = commit_filtered(
            bootstrap_repo, bootstrap_revision, None, config, object_dir=replay
        )
        parent = bootstrap_tip
        for revision in official_revisions(
            official_repo, config["official_baseline"], cursor, config["mappings"]
        ):
            parent, _ = commit_filtered(
                official_repo, revision, parent, config, object_dir=replay
            )
        return bootstrap_tip, parent


def replay_errors(config, state, bootstrap_repo, bootstrap_revision, official_repo, cursor):
    expected_bootstrap, expected_vendor = replay_vendor_history(
        config, bootstrap_repo, bootstrap_revision, official_repo, cursor
    )
    errors = []
    if state["bootstrap_vendor_tip"] != expected_bootstrap:
        errors.append(
            "bootstrap_vendor_tip differs from the deterministic filtered bootstrap commit"
        )
    if state["vendor_tip"] != expected_vendor:
        errors.append(
            "vendor_tip differs from deterministic replay of official first-parent history"
        )
    return errors


def integration_errors(state, head, repo=ROOT):
    errors = []
    for key in ("bootstrap_vendor_tip", "vendor_tip", "integration_commit"):
        if not commit_exists(state[key], repo):
            errors.append(f"receipt commit does not exist: {key}={state[key]}")
    if errors:
        return errors
    if not is_ancestor(state["bootstrap_vendor_tip"], state["vendor_tip"], repo):
        errors.append("bootstrap vendor tip is not an ancestor of current vendor tip")
    parents = run([
        "git", "-C", str(repo), "show", "-s", "--format=%P", state["integration_commit"]
    ]).stdout.split()
    if len(parents) != 2:
        errors.append("integration_commit must be an exact two-parent merge")
    elif parents[1] != state["vendor_tip"]:
        errors.append("integration_commit must merge vendor_tip as its second parent")
    history = run([
        "git", "-C", str(repo), "rev-list", "--first-parent", head
    ]).stdout.splitlines()
    if state["integration_commit"] not in history:
        errors.append("integration_commit is not on HEAD's first-parent history")
        return errors

    recorded_markers = None
    try:
        recorded_markers = integration_markers(repo, state["integration_commit"])
    except SyncError as exc:
        errors.append(str(exc))
    if recorded_markers is None:
        errors.append("integration_commit is missing its Zed sync markers")
    else:
        expected = {
            "zed-sync-algorithm": state.get("history_algorithm", HISTORY_ALGORITHM),
            "zed-vendor-tip": state["vendor_tip"],
            "zed-upstream-cursor": state["last_synced_sha"],
        }
        if recorded_markers != expected:
            errors.append("integration_commit markers disagree with the sync receipt")

    newer = history[:history.index(state["integration_commit"])]
    newest_marked = None
    for commit in history[:history.index(state["integration_commit"]) + 1]:
        try:
            markers = integration_markers(repo, commit)
        except SyncError as exc:
            errors.append(str(exc))
            continue
        if markers is not None:
            newest_marked = commit
            break
    if newest_marked != state["integration_commit"]:
        errors.append("the newest marked Zed integration is not integration_commit")

    for commit in newer:
        candidate_parents = run([
            "git", "-C", str(repo), "show", "-s", "--format=%P", commit
        ]).stdout.split()
        if any(is_synthetic_vendor_commit(repo, parent) for parent in candidate_parents[1:]):
            errors.append(
                f"newer unrecorded Zed vendor integration appears on first-parent history: {commit}"
            )
    return errors


def verify(args):
    if args.release and args.no_source_check:
        raise SyncError("--no-source-check is forbidden for release verification")
    config, state = load(CONFIG), load(STATE)
    validate_config(config)
    errors = validate_state(config, state, args.release)
    errors.extend(provenance_errors(config, state))
    tracked = set(run(["git", "ls-files"]).stdout.splitlines())
    for mapping in config["mappings"]:
        if not (ROOT / mapping["destination"]).exists(): errors.append(f"missing destination: {mapping['destination']}")
    forbidden = tuple(p + "/" for p in config["forbidden_destinations"])
    for path in tracked:
        if path.startswith(forbidden): errors.append(f"forbidden tracked Zed path: {path}")
    for manifest, expected in config["packages"].items():
        try: actual = package_name(ROOT / manifest)
        except Exception as exc: errors.append(f"cannot read package {manifest}: {exc}"); continue
        if actual != expected: errors.append(f"package identity {manifest}: expected {expected}, got {actual}")
    if args.release and not errors:
        head = run(["git", "rev-parse", "HEAD"]).stdout.strip()
        errors.extend(integration_errors(state, head))
    if not args.no_source_check:
        bootstrap_temp, revision = fetch_temp(
            config["bootstrap_url"], config["bootstrap_revision"]
        )
        try:
            if revision != config["bootstrap_revision"]:
                errors.append("bootstrap remote did not resolve the exact bootstrap_revision")
            for mapping in config["mappings"]:
                result = run([
                    "git", "-C", bootstrap_temp.name, "cat-file", "-e",
                    f"{revision}:{mapping['source']}",
                ], check=False)
                if result.returncode: errors.append(f"bootstrap source missing: {mapping['source']}")
            if args.release and not errors:
                official_temp, cursor = fetch_temp(
                    config["official_url"], state["last_synced_sha"]
                )
                try:
                    if cursor != state["last_synced_sha"]:
                        errors.append("official remote did not resolve the exact last_synced_sha")
                    if not errors:
                        errors.extend(replay_errors(
                            config,
                            state,
                            bootstrap_temp.name,
                            revision,
                            official_temp.name,
                            cursor,
                        ))
                except SyncError as exc:
                    errors.append(str(exc))
                finally:
                    official_temp.cleanup()
        finally:
            bootstrap_temp.cleanup()
    if errors: raise SyncError("verification failed:\n- " + "\n- ".join(errors))
    print(f"sync-zed {'release ' if args.release else ''}verification passed")


def bootstrap(args):
    config, state = load(CONFIG), load(STATE); validate_config(config)
    errors = validate_state(config, state)
    errors.extend(provenance_errors(config, state))
    if errors: raise SyncError("invalid state:\n- " + "\n- ".join(errors))
    if state["vendor_tip"] is not None: raise SyncError("already bootstrapped")
    if args.dry_run:
        print(f"would bootstrap {config['bootstrap_revision']} into {config['vendor_ref']}")
        return
    require_clean()
    temp, revision = fetch_temp(config["bootstrap_url"], config["bootstrap_revision"])
    try: tip, _ = commit_filtered(temp.name, revision, None, config)
    finally: temp.cleanup()
    run(["git", "update-ref", config["vendor_ref"], tip])
    message = integration_message(
        "chore(sync): record Zed vendor ancestry",
        tip,
        config["official_baseline"],
    )
    run(["git", "merge", "-s", "ours", "--no-ff", "--allow-unrelated-histories",
         "-m", message, config["vendor_ref"]])
    state.update(bootstrap_vendor_tip=tip, vendor_tip=tip, last_synced_sha=config["official_baseline"], integration_commit=run(["git", "rev-parse", "HEAD"]).stdout.strip())
    write_receipt(config, state)
    run(["git", "add", str(STATE.relative_to(ROOT)), str(PROVENANCE.relative_to(ROOT))])
    run(["git", "commit", "-m", "chore(sync): record Zed bootstrap state"])
    print(f"bootstrapped {tip}")


def sync(args):
    config, state = load(CONFIG), load(STATE); validate_config(config)
    errors = validate_state(config, state)
    errors.extend(provenance_errors(config, state))
    if errors: raise SyncError("invalid state:\n- " + "\n- ".join(errors))
    if not state["vendor_tip"] or not state["last_synced_sha"]: raise SyncError("run bootstrap first")
    if not args.dry_run: require_clean()
    temp, head = fetch_temp(config["official_url"], args.ref)
    try:
        revs = official_revisions(
            temp.name, state["last_synced_sha"], head, config["mappings"]
        )
        if args.dry_run:
            print(f"would inspect {len(revs)} touching first-parent commit(s); no repository mutations")
            return
        vendor_ref = run([
            "git", "rev-parse", "--verify", config["vendor_ref"]
        ], check=False)
        if not vendor_ref.returncode and vendor_ref.stdout.strip() != state["vendor_tip"]:
            raise SyncError("vendor ref and committed vendor_tip disagree")
        if vendor_ref.returncode:
            raise SyncError("configured vendor ref does not exist")
        parent = state["vendor_tip"]; created = 0
        for revision in revs:
            parent, changed = commit_filtered(temp.name, revision, parent, config); created += int(changed)
        if parent == state["vendor_tip"]:
            print("no filtered changes; receipt remains at its last integrated cursor"); return
        branch = f"sync/zed-{head[:12]}"
        run(["git", "update-ref", f"refs/heads/{branch}", parent])
        message = integration_message(
            f"chore(sync): integrate Zed through {head[:12]}", parent, head
        )
        run(["git", "merge", "--no-ff", "--no-commit", "-m", message, branch], check=True)
        run(["git", "commit", "--no-edit"])
        integration = run(["git", "rev-parse", "HEAD"]).stdout.strip()
        run(["git", "update-ref", config["vendor_ref"], parent, state["vendor_tip"]])
        state.update(vendor_tip=parent, last_synced_sha=head, integration_commit=integration)
        write_receipt(config, state)
        run(["git", "add", str(STATE.relative_to(ROOT)), str(PROVENANCE.relative_to(ROOT))])
        run(["git", "commit", "-m", "chore(sync): update Zed sync cursor"])
        print(f"integrated {created} filtered commit(s) on {branch}; nothing was pushed")
    except SyncError as exc:
        if run(["git", "rev-parse", "-q", "--verify", "MERGE_HEAD"], check=False).returncode == 0:
            raise SyncError(f"merge stopped with raw conflicts preserved. Resolve them, commit the integration, then update state manually; or run git merge --abort.\n{exc}")
        raise
    finally: temp.cleanup()


def status(_args):
    state = load(STATE)
    print(json.dumps({k: state[k] for k in ("vendor_ref", *RECEIPT_KEYS)}, indent=2))


def main(argv=None):
    parser = argparse.ArgumentParser(prog="sync-zed")
    subs = parser.add_subparsers(dest="command", required=True)
    p = subs.add_parser("bootstrap"); p.add_argument("--dry-run", action="store_true"); p.set_defaults(func=bootstrap)
    p = subs.add_parser("sync"); p.add_argument("--ref", default="HEAD"); p.add_argument("--dry-run", action="store_true"); p.set_defaults(func=sync)
    p = subs.add_parser("verify"); p.add_argument("--release", action="store_true"); p.add_argument("--no-source-check", action="store_true", help=argparse.SUPPRESS); p.set_defaults(func=verify)
    p = subs.add_parser("status"); p.set_defaults(func=status)
    args = parser.parse_args(argv)
    try: args.func(args)
    except SyncError as exc: print(f"sync-zed: {exc}", file=sys.stderr); return 1
    return 0


if __name__ == "__main__": raise SystemExit(main())
