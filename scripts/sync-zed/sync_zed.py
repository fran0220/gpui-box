#!/usr/bin/env python3
"""Reproducible filtered Zed history synchronizer for GPUI Box."""
from __future__ import annotations

import argparse
import hashlib
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
VERSION = "2.0.0"
HISTORY_ALGORITHM = "first-parent-v1"
OVERLAY_ALGORITHM = "exact-linear-overlay-v1"
SHA = re.compile(r"^[0-9a-f]{40}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
RECEIPT_KEYS = ("bootstrap_vendor_tip", "vendor_tip", "last_synced_sha", "integration_commit")
OVERLAY_DYNAMIC_KEYS = ("base_vendor_tip", "vendor_tip", "integration_commit")


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
    if config.get("schema_version") != 2:
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
    digest = filter_digest(config["mappings"])
    if config.get("filter_digest_sha256") != digest:
        raise SyncError("filter_digest_sha256 differs from canonical mappings")
    overlay = config.get("fork_overlay")
    if not isinstance(overlay, dict):
        raise SyncError("fork_overlay definition is required")
    if overlay.get("algorithm") != OVERLAY_ALGORITHM:
        raise SyncError("unsupported fork overlay algorithm")
    if overlay.get("source_url") != config.get("bootstrap_url"):
        raise SyncError("fork overlay source_url must equal bootstrap_url")
    if overlay.get("base_revision") != config.get("bootstrap_revision"):
        raise SyncError("fork overlay base_revision must equal bootstrap_revision")
    if overlay.get("vendor_ref") == config.get("vendor_ref"):
        raise SyncError("official and fork overlay vendor refs must differ")
    for key in ("vendor_ref",):
        value = overlay.get(key, "")
        if not value.startswith("refs/heads/vendor/") or ".." in value:
            raise SyncError(f"unsafe fork overlay {key}: {value}")
    revisions = overlay.get("source_revisions")
    if not isinstance(revisions, list) or not revisions:
        raise SyncError("fork overlay source_revisions must be a nonempty list")
    if len(set(revisions)) != len(revisions):
        raise SyncError("fork overlay source_revisions contains duplicates")
    for revision in revisions:
        if not isinstance(revision, str) or not SHA.fullmatch(revision):
            raise SyncError("fork overlay source_revisions must contain full lowercase SHAs")


def filter_digest(mappings):
    encoded = json.dumps(
        mappings, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


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


def init_bare(path):
    run(
        ["git", "init", "--bare", "--object-format=sha1", str(path)],
        env=deterministic_env(),
    )


def fetch_temp(url, revision=None):
    temp = tempfile.TemporaryDirectory(prefix="sync-zed-")
    init_bare(temp.name)
    spec = revision or "HEAD"
    run(["git", "-C", temp.name, "fetch", "--quiet", "--no-tags", url, spec])
    return temp, run(["git", "-C", temp.name, "rev-parse", "FETCH_HEAD"]).stdout.strip()


def fetch_overlay_source(config):
    overlay = config["fork_overlay"]
    temp = tempfile.TemporaryDirectory(prefix="sync-zed-overlay-source-")
    init_bare(temp.name)
    requested = [overlay["base_revision"], *overlay["source_revisions"]]
    for index, revision in enumerate(requested):
        target = f"refs/source-overlay/{index}-{revision}"
        run([
            "git", "-C", temp.name, "fetch", "--quiet", "--no-tags",
            overlay["source_url"], f"{revision}:{target}",
        ])
        resolved = run(["git", "-C", temp.name, "rev-parse", target]).stdout.strip()
        if resolved != revision:
            temp.cleanup()
            raise SyncError(f"fork overlay remote did not resolve exact revision {revision}")
    validate_overlay_source_chain(temp.name, config)
    return temp


def validate_overlay_source_chain(repo, config):
    overlay = config["fork_overlay"]
    expected_parent = overlay["base_revision"]
    for revision in overlay["source_revisions"]:
        parents = run([
            "git", "-C", str(repo), "show", "-s", "--format=%P", revision
        ]).stdout.split()
        if parents != [expected_parent]:
            raise SyncError(
                f"fork overlay source {revision} must have exactly parent {expected_parent}"
            )
        expected_parent = revision


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


def commit_message(repo, commit, trailer="zed-upstream"):
    message = run(
        ["git", "-C", str(repo), "show", "-s", "--format=%B", commit],
        env=deterministic_env(),
    ).stdout.rstrip()
    return f"{message}\n\n{trailer}: {commit}\n"


def integration_message(subject, vendor_tip, cursor):
    return (
        f"{subject}\n\n"
        f"zed-sync-algorithm: {HISTORY_ALGORITHM}\n"
        f"zed-vendor-tip: {vendor_tip}\n"
        f"zed-upstream-cursor: {cursor}\n"
    )


def overlay_integration_message(config, vendor_tip, base_vendor_tip):
    overlay = config["fork_overlay"]
    return (
        "chore(sync): integrate Zed fork PlatformView overlay\n\n"
        f"zed-overlay-algorithm: {overlay['algorithm']}\n"
        f"zed-overlay-base-vendor-tip: {base_vendor_tip}\n"
        f"zed-overlay-vendor-tip: {vendor_tip}\n"
        f"zed-overlay-source-tip: {overlay['source_revisions'][-1]}\n"
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


def overlay_integration_markers(repo, commit):
    message = run([
        "git", "-C", str(repo), "show", "-s", "--format=%B", commit
    ]).stdout
    names = (
        "zed-overlay-algorithm",
        "zed-overlay-base-vendor-tip",
        "zed-overlay-vendor-tip",
        "zed-overlay-source-tip",
    )
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
            f"incomplete overlay integration markers in {commit}: missing {', '.join(missing)}"
        )
    if values["zed-overlay-algorithm"] != OVERLAY_ALGORITHM:
        raise SyncError(f"invalid zed-overlay-algorithm marker in integration {commit}")
    for name in names[1:]:
        if not SHA.fullmatch(values[name]):
            raise SyncError(f"invalid {name} marker in integration {commit}")
    if integration_markers(repo, commit) is not None:
        raise SyncError(f"integration {commit} carries both official and overlay markers")
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


def is_synthetic_overlay_commit(repo, commit):
    message = run([
        "git", "-C", str(repo), "show", "-s", "--format=%B", commit
    ]).stdout
    trailers = [
        line[len("zed-fork-overlay-upstream: "):]
        for line in message.splitlines()
        if line.startswith("zed-fork-overlay-upstream: ")
    ]
    return bool(trailers) and SHA.fullmatch(trailers[-1]) is not None


def commit_filtered(
    repo,
    upstream,
    parent,
    config,
    object_dir=ROOT,
    trailer="zed-upstream",
):
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
    oid = run(
        args,
        cwd=object_dir,
        env=env,
        input_text=commit_message(repo, upstream, trailer=trailer),
    ).stdout.strip()
    return oid, True


def provenance_section_values(section):
    values = {}
    in_section = False
    lines = PROVENANCE.read_text().splitlines()
    index = 0
    while index < len(lines):
        line = lines[index]
        stripped = line.strip()
        if re.fullmatch(r"\[[a-z_]+\]", stripped):
            in_section = stripped == f"[{section}]"
            index += 1
            continue
        if not in_section or not stripped or stripped.startswith("#"):
            index += 1
            continue
        match = re.match(r"^([a-z0-9_]+)\s*=\s*(.*)$", stripped)
        if not match:
            raise SyncError(f"cannot parse provenance [{section}] line: {stripped}")
        key, raw = match.groups()
        if key in values:
            raise SyncError(f"duplicate provenance [{section}] key: {key}")
        if raw.startswith("["):
            while not raw.rstrip().endswith("]"):
                index += 1
                if index >= len(lines):
                    raise SyncError(f"unterminated provenance [{section}] array: {key}")
                raw += "\n" + lines[index].strip()
            raw = re.sub(r",\s*]$", "]", raw)
        if raw.startswith('"'):
            values[key] = json.loads(raw)
        elif raw.startswith("["):
            values[key] = json.loads(raw)
        elif raw in ("true", "false"):
            values[key] = raw == "true"
        elif re.fullmatch(r"[0-9]+", raw):
            values[key] = int(raw)
        else:
            raise SyncError(f"unsupported provenance [{section}] value: {key}")
        index += 1
    return values


def provenance_errors(config, state):
    actual_sync = provenance_section_values("sync")
    expected_sync = {
        "filter_schema_version": config["filter_schema_version"],
        "history_algorithm": config["history_algorithm"],
        "history_bootstrapped": state["vendor_tip"] is not None,
        **{key: state[key] or "" for key in RECEIPT_KEYS},
    }
    errors = [
        f"provenance.toml [sync] {key} differs from the sync receipt"
        for key, value in expected_sync.items()
        if actual_sync.get(key) != value
    ]
    actual_overlay = provenance_section_values("sync_overlay")
    overlay = state["fork_overlay"]
    expected_overlay = {
        key: value or "" for key, value in overlay.items()
    }
    errors.extend(
        f"provenance.toml [sync_overlay] {key} differs from the overlay receipt"
        for key, value in expected_overlay.items()
        if actual_overlay.get(key) != value
    )
    return errors


def replace_section_scalars(lines, section, replacements):
    in_section = False
    found = set()
    for index, line in enumerate(lines):
        stripped = line.strip()
        if re.fullmatch(r"\[[a-z_]+\]", stripped):
            in_section = stripped == f"[{section}]"
            continue
        if not in_section:
            continue
        match = re.match(r"^([a-z0-9_]+)\s*=", stripped)
        if match and match.group(1) in replacements:
            key = match.group(1)
            if key in found:
                raise SyncError(f"duplicate provenance [{section}] key: {key}")
            lines[index] = f"{key} = {replacements[key]}"
            found.add(key)
    missing = set(replacements) - found
    if missing:
        raise SyncError(
            f"provenance.toml is missing [{section}] receipt keys: "
            + ", ".join(sorted(missing))
        )


def write_receipt(config, state):
    sync_replacements = {
        "history_bootstrapped": "true" if state["vendor_tip"] is not None else "false",
        **{key: json.dumps(state[key] or "") for key in RECEIPT_KEYS},
    }
    lines = PROVENANCE.read_text().splitlines()
    replace_section_scalars(lines, "sync", sync_replacements)
    overlay_replacements = {
        key: json.dumps(state["fork_overlay"][key] or "")
        for key in OVERLAY_DYNAMIC_KEYS
    }
    replace_section_scalars(lines, "sync_overlay", overlay_replacements)
    expected_static = {
        "filter_schema_version": config["filter_schema_version"],
        "history_algorithm": config["history_algorithm"],
    }
    actual_static = provenance_section_values("sync")
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
    overlay_config = config["fork_overlay"]
    overlay = state.get("fork_overlay")
    if not isinstance(overlay, dict):
        errors.append("state fork_overlay receipt is required")
        return errors
    expected_overlay = {
        "algorithm": overlay_config["algorithm"],
        "source_url": overlay_config["source_url"],
        "base_revision": overlay_config["base_revision"],
        "source_revisions": overlay_config["source_revisions"],
        "source_tip": overlay_config["source_revisions"][-1],
        "filter_schema_version": config["filter_schema_version"],
        "filter_digest_sha256": config["filter_digest_sha256"],
        "vendor_ref": overlay_config["vendor_ref"],
    }
    for key, expected in expected_overlay.items():
        if overlay.get(key) != expected:
            errors.append(f"state/config disagree on fork_overlay.{key}")
    if not SHA256.fullmatch(overlay.get("filter_digest_sha256", "")):
        errors.append("invalid fork_overlay filter_digest_sha256")
    for key in OVERLAY_DYNAMIC_KEYS:
        value = overlay.get(key)
        if value is not None and not SHA.fullmatch(value):
            errors.append(f"invalid fork_overlay coordinate {key}: {value}")
        if release and not SHA.fullmatch(value or ""):
            errors.append(f"release overlay receipt requires {key}")
    overlay_present = [overlay.get(key) is not None for key in OVERLAY_DYNAMIC_KEYS]
    if any(overlay_present) and not all(overlay_present):
        errors.append("overlay receipt coordinates must be either all null or all full SHAs")
    if overlay.get("base_vendor_tip") is not None and overlay.get("base_vendor_tip") != state.get("bootstrap_vendor_tip"):
        errors.append("fork_overlay.base_vendor_tip must equal bootstrap_vendor_tip")
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
        init_bare(replay)
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


def build_overlay_replay(config, source_repo):
    replay = tempfile.TemporaryDirectory(prefix="sync-zed-overlay-replay-")
    init_bare(replay.name)
    bootstrap_tip, _ = commit_filtered(
        source_repo,
        config["bootstrap_revision"],
        None,
        config,
        object_dir=replay.name,
    )
    parent = bootstrap_tip
    for revision in config["fork_overlay"]["source_revisions"]:
        parent, changed = commit_filtered(
            source_repo,
            revision,
            parent,
            config,
            object_dir=replay.name,
            trailer="zed-fork-overlay-upstream",
        )
        if not changed:
            replay.cleanup()
            raise SyncError(f"configured fork overlay source is a filtered no-op: {revision}")
    return replay, bootstrap_tip, parent


def overlay_replay_errors(config, state, source_repo):
    replay, expected_bootstrap, expected_overlay = build_overlay_replay(config, source_repo)
    replay.cleanup()
    errors = []
    overlay = state["fork_overlay"]
    if state["bootstrap_vendor_tip"] != expected_bootstrap:
        errors.append("fork overlay replay bootstrap differs from bootstrap_vendor_tip")
    if overlay["base_vendor_tip"] != expected_bootstrap:
        errors.append("fork overlay base_vendor_tip differs from deterministic bootstrap")
    if overlay["vendor_tip"] != expected_overlay:
        errors.append("fork overlay vendor_tip differs from deterministic exact-chain replay")
    return errors


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


def exact_ref_errors(ref, expected, label, repo=ROOT):
    result = run(
        ["git", "-C", str(repo), "rev-parse", "--verify", ref], check=False
    )
    if result.returncode:
        return [f"missing canonical {label} ref: {ref}"]
    if result.stdout.strip() != expected:
        return [f"canonical {label} ref differs from receipt: {ref}"]
    return []


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


def overlay_integration_errors(config, state, head, repo=ROOT):
    errors = []
    overlay = state["fork_overlay"]
    commits = {
        "bootstrap_vendor_tip": state["bootstrap_vendor_tip"],
        "fork_overlay.vendor_tip": overlay["vendor_tip"],
        "fork_overlay.integration_commit": overlay["integration_commit"],
    }
    for key, commit in commits.items():
        if not commit_exists(commit, repo):
            errors.append(f"overlay receipt commit does not exist: {key}={commit}")
    if errors:
        return errors
    if not is_ancestor(state["bootstrap_vendor_tip"], overlay["vendor_tip"], repo):
        errors.append("fork overlay vendor tip does not descend from bootstrap vendor tip")
    merge_bases = run([
        "git", "-C", str(repo), "merge-base", "--all",
        state["vendor_tip"], overlay["vendor_tip"],
    ]).stdout.splitlines()
    if merge_bases != [state["bootstrap_vendor_tip"]]:
        errors.append("official and fork overlay vendor lanes must meet only at bootstrap_vendor_tip")
    parents = run([
        "git", "-C", str(repo), "show", "-s", "--format=%P", overlay["integration_commit"]
    ]).stdout.split()
    if len(parents) != 2:
        errors.append("fork overlay integration_commit must be an exact two-parent merge")
    elif parents[1] != overlay["vendor_tip"]:
        errors.append("fork overlay integration_commit must merge overlay vendor_tip as second parent")
    history = run([
        "git", "-C", str(repo), "rev-list", "--first-parent", head
    ]).stdout.splitlines()
    if overlay["integration_commit"] not in history:
        errors.append("fork overlay integration_commit is not on HEAD's first-parent history")
        return errors
    try:
        markers = overlay_integration_markers(repo, overlay["integration_commit"])
    except SyncError as exc:
        errors.append(str(exc))
        markers = None
    expected_markers = {
        "zed-overlay-algorithm": overlay["algorithm"],
        "zed-overlay-base-vendor-tip": overlay["base_vendor_tip"],
        "zed-overlay-vendor-tip": overlay["vendor_tip"],
        "zed-overlay-source-tip": overlay["source_tip"],
    }
    if markers is None:
        errors.append("fork overlay integration_commit is missing overlay markers")
    elif markers != expected_markers:
        errors.append("fork overlay integration markers disagree with the receipt")
    receipt_index = history.index(overlay["integration_commit"])
    newest_marked = None
    for commit in history[:receipt_index + 1]:
        try:
            candidate = overlay_integration_markers(repo, commit)
        except SyncError as exc:
            errors.append(str(exc))
            continue
        if candidate is not None:
            newest_marked = commit
            break
    if newest_marked != overlay["integration_commit"]:
        errors.append("the newest marked fork overlay integration is not the receipt integration")
    for commit in history[:receipt_index]:
        candidate_parents = run([
            "git", "-C", str(repo), "show", "-s", "--format=%P", commit
        ]).stdout.split()
        if any(is_synthetic_overlay_commit(repo, parent) for parent in candidate_parents[1:]):
            errors.append(
                f"newer unrecorded fork overlay integration appears on first-parent history: {commit}"
            )
    errors.extend(exact_ref_errors(config["vendor_ref"], state["vendor_tip"], "official vendor", repo))
    errors.extend(exact_ref_errors(overlay["vendor_ref"], overlay["vendor_tip"], "fork overlay vendor", repo))
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
        errors.extend(overlay_integration_errors(config, state, head))
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
                    if not errors:
                        overlay_temp = fetch_overlay_source(config)
                        try:
                            errors.extend(overlay_replay_errors(
                                config, state, overlay_temp.name
                            ))
                        finally:
                            overlay_temp.cleanup()
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


def finalize_overlay_receipt(config, state, overlay_tip, integration):
    overlay = state["fork_overlay"]
    markers = overlay_integration_markers(ROOT, integration)
    expected = {
        "zed-overlay-algorithm": overlay["algorithm"],
        "zed-overlay-base-vendor-tip": state["bootstrap_vendor_tip"],
        "zed-overlay-vendor-tip": overlay_tip,
        "zed-overlay-source-tip": overlay["source_tip"],
    }
    if markers != expected:
        raise SyncError("cannot finalize fork overlay: integration markers are not exact")
    parents = run(["git", "show", "-s", "--format=%P", integration]).stdout.split()
    if len(parents) != 2 or parents[1] != overlay_tip:
        raise SyncError("cannot finalize fork overlay: integration has the wrong parents")
    current_ref = run(["git", "rev-parse", "--verify", overlay["vendor_ref"]], check=False)
    if current_ref.returncode:
        run(["git", "update-ref", overlay["vendor_ref"], overlay_tip, "0" * 40])
    elif current_ref.stdout.strip() != overlay_tip:
        raise SyncError("fork overlay vendor ref exists at an unexpected commit")
    overlay.update(
        base_vendor_tip=state["bootstrap_vendor_tip"],
        vendor_tip=overlay_tip,
        integration_commit=integration,
    )
    write_receipt(config, state)
    run(["git", "add", str(STATE.relative_to(ROOT)), str(PROVENANCE.relative_to(ROOT))])
    run(["git", "commit", "-m", "chore(sync): record Zed fork overlay receipt"])
    print(f"integrated fork overlay {overlay_tip}; nothing was pushed")


def continue_overlay(config, state, expected_tip):
    unresolved = run(["git", "diff", "--name-only", "--diff-filter=U"]).stdout.splitlines()
    if unresolved:
        raise SyncError("cannot continue fork overlay with unresolved paths: " + ", ".join(unresolved))
    merge_head = run(["git", "rev-parse", "-q", "--verify", "MERGE_HEAD"], check=False)
    if not merge_head.returncode:
        if merge_head.stdout.strip() != expected_tip:
            raise SyncError("MERGE_HEAD is not the deterministic fork overlay tip")
        merge_message_path = run(["git", "rev-parse", "--git-path", "MERGE_MSG"]).stdout.strip()
        message = Path(merge_message_path).read_text()
        expected_message = overlay_integration_message(
            config, expected_tip, state["bootstrap_vendor_tip"]
        )
        for marker in expected_message.splitlines()[2:]:
            if marker and message.splitlines().count(marker) != 1:
                raise SyncError(f"MERGE_MSG is missing exact overlay marker: {marker}")
        if "zed-sync-algorithm:" in message:
            raise SyncError("MERGE_MSG mixes official and fork overlay markers")
        run(["git", "commit", "--no-edit"])
        integration = run(["git", "rev-parse", "HEAD"]).stdout.strip()
    else:
        integration = None
        for commit in run(["git", "rev-list", "--first-parent", "HEAD"]).stdout.splitlines():
            markers = overlay_integration_markers(ROOT, commit)
            if markers is not None:
                integration = commit
                break
        if integration is None:
            raise SyncError("no committed fork overlay integration is available to continue")
    finalize_overlay_receipt(config, state, expected_tip, integration)


def overlay(args):
    config, state = load(CONFIG), load(STATE)
    validate_config(config)
    errors = validate_state(config, state)
    errors.extend(provenance_errors(config, state))
    if errors:
        raise SyncError("invalid state:\n- " + "\n- ".join(errors))
    receipt = state["fork_overlay"]
    if all(receipt[key] is not None for key in OVERLAY_DYNAMIC_KEYS):
        head = run(["git", "rev-parse", "HEAD"]).stdout.strip()
        errors = overlay_integration_errors(config, state, head)
        if errors:
            raise SyncError("recorded fork overlay is invalid:\n- " + "\n- ".join(errors))
        print(f"fork overlay already integrated at {receipt['vendor_tip']}")
        return
    if not args.continue_overlay:
        require_clean()
    source = fetch_overlay_source(config)
    replay = None
    try:
        replay, bootstrap_tip, overlay_tip = build_overlay_replay(config, source.name)
        if bootstrap_tip != state["bootstrap_vendor_tip"]:
            raise SyncError("fork overlay deterministic bootstrap differs from receipt")
        print(
            f"fork overlay source count={len(config['fork_overlay']['source_revisions'])} "
            f"source_tip={config['fork_overlay']['source_revisions'][-1]} "
            f"vendor_tip={overlay_tip}"
        )
        if args.dry_run:
            print("fork overlay dry run completed without repository mutations")
            return
        if args.continue_overlay:
            continue_overlay(config, state, overlay_tip)
            return
        errors = exact_ref_errors(
            config["vendor_ref"], state["vendor_tip"], "official vendor"
        )
        if errors:
            raise SyncError("\n".join(errors))
        existing = run([
            "git", "rev-parse", "--verify", receipt["vendor_ref"]
        ], check=False)
        if not existing.returncode and existing.stdout.strip() != overlay_tip:
            raise SyncError("fork overlay vendor ref already exists at an unexpected commit")
        scratch = f"refs/heads/sync/zed-overlay-{receipt['source_tip'][:12]}"
        scratch_current = run(["git", "rev-parse", "--verify", scratch], check=False)
        if not scratch_current.returncode and scratch_current.stdout.strip() != overlay_tip:
            raise SyncError("fork overlay scratch ref exists at an unexpected commit")
        run([
            "git", "fetch", "--quiet", "--no-tags", replay.name,
            f"{overlay_tip}:{scratch}",
        ])
        merge_bases = run([
            "git", "merge-base", "--all", "HEAD", overlay_tip
        ]).stdout.splitlines()
        if merge_bases != [state["bootstrap_vendor_tip"]]:
            raise SyncError("HEAD and fork overlay must meet only at bootstrap_vendor_tip")
        message = overlay_integration_message(
            config, overlay_tip, state["bootstrap_vendor_tip"]
        )
        result = run([
            "git", "merge", "--no-ff", "--no-commit", "-m", message, scratch
        ], check=False)
        if result.returncode:
            if not run(["git", "rev-parse", "-q", "--verify", "MERGE_HEAD"], check=False).returncode:
                raise SyncError(
                    "fork overlay merge stopped with conflicts preserved; resolve them and run "
                    "sync-zed overlay --continue"
                )
            raise SyncError(f"fork overlay merge failed before starting:\n{result.stderr.strip()}")
        run(["git", "commit", "--no-edit"])
        integration = run(["git", "rev-parse", "HEAD"]).stdout.strip()
        finalize_overlay_receipt(config, state, overlay_tip, integration)
    finally:
        if replay is not None:
            replay.cleanup()
        source.cleanup()


def status(_args):
    state = load(STATE)
    print(json.dumps({
        "official": {k: state[k] for k in ("vendor_ref", *RECEIPT_KEYS)},
        "fork_overlay": state["fork_overlay"],
    }, indent=2))


def main(argv=None):
    parser = argparse.ArgumentParser(prog="sync-zed")
    subs = parser.add_subparsers(dest="command", required=True)
    p = subs.add_parser("bootstrap"); p.add_argument("--dry-run", action="store_true"); p.set_defaults(func=bootstrap)
    p = subs.add_parser("sync"); p.add_argument("--ref", default="HEAD"); p.add_argument("--dry-run", action="store_true"); p.set_defaults(func=sync)
    p = subs.add_parser("overlay")
    group = p.add_mutually_exclusive_group()
    group.add_argument("--dry-run", action="store_true")
    group.add_argument("--continue", dest="continue_overlay", action="store_true")
    p.set_defaults(func=overlay, dry_run=False, continue_overlay=False)
    p = subs.add_parser("verify"); p.add_argument("--release", action="store_true"); p.add_argument("--no-source-check", action="store_true", help=argparse.SUPPRESS); p.set_defaults(func=verify)
    p = subs.add_parser("status"); p.set_defaults(func=status)
    args = parser.parse_args(argv)
    try: args.func(args)
    except SyncError as exc: print(f"sync-zed: {exc}", file=sys.stderr); return 1
    return 0


if __name__ == "__main__": raise SystemExit(main())
