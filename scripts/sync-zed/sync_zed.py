#!/usr/bin/env python3
"""Offline verifier for GPUI Box's frozen historical Zed import."""
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
VERSION = "3.0.0"
MODE = "frozen-historical-import"
HISTORY_ALGORITHM = "first-parent-v1"
OVERLAY_ALGORITHM = "exact-linear-overlay-v1"
SHA = re.compile(r"^[0-9a-f]{40}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
RECEIPT_KEYS = (
    "bootstrap_vendor_tip",
    "vendor_tip",
    "last_synced_sha",
    "integration_commit",
)
OVERLAY_RECEIPT_KEYS = ("base_vendor_tip", "vendor_tip", "integration_commit")


class VerificationError(RuntimeError):
    pass


def run(args, cwd=ROOT, check=True, env=None, input_text=None):
    result = subprocess.run(
        args,
        cwd=cwd,
        env=env,
        input=input_text,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if check and result.returncode:
        raise VerificationError(
            f"command failed: {' '.join(map(str, args))}\n{result.stderr.strip()}"
        )
    return result


def load(path):
    return json.loads(path.read_text())


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
    raise VerificationError("missing [package] name")


def filter_digest(mappings):
    encoded = json.dumps(
        mappings, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def remap(path, mappings):
    matches = [
        mapping
        for mapping in mappings
        if path == mapping["source"] or path.startswith(mapping["source"] + "/")
    ]
    if not matches:
        return None
    mapping = max(
        matches, key=lambda item: len(PurePosixPath(item["source"]).parts)
    )
    suffix = path[len(mapping["source"]) :].lstrip("/")
    return mapping["destination"] + (("/" + suffix) if suffix else "")


def deterministic_env(extra=None):
    env = os.environ.copy()
    env.update(
        {
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "LANG": "C",
            "LC_ALL": "C",
            "TZ": "UTC",
        }
    )
    if extra:
        env.update(extra)
    return env


def source_entries(repo, commit, mappings):
    source_paths = [mapping["source"] for mapping in mappings]
    output = run(
        [
            "git",
            "-C",
            str(repo),
            "ls-tree",
            "-r",
            "-z",
            commit,
            "--",
            *source_paths,
        ]
    ).stdout
    entries = {}
    records = output.rstrip("\0").split("\0") if output else []
    for record in records:
        metadata, source = record.split("\t", 1)
        mode, kind, oid = metadata.split()
        if kind != "blob":
            raise VerificationError(f"unsupported mapped Git entry {kind} at {source}")
        destination = remap(source, mappings)
        if destination is None:
            continue
        value = (mode, oid, source)
        prior = entries.get(destination)
        if prior is not None and prior != value:
            raise VerificationError(f"mapping collision at {destination}")
        entries[destination] = value
    return entries


def rewrite_package_name(manifest, expected):
    lines = manifest.splitlines(keepends=True)
    in_package = False
    found = False
    original = None
    for index, line in enumerate(lines):
        ending = ""
        if line.endswith("\r\n"):
            line, ending = line[:-2], "\r\n"
        elif line.endswith("\n"):
            line, ending = line[:-1], "\n"
        stripped = line.strip()
        if stripped.startswith("["):
            in_package = stripped == "[package]"
            continue
        if not in_package:
            continue
        match = re.match(r'^(\s*name\s*=\s*)"([^"]+)"(.*)$', line)
        if match:
            if found:
                raise VerificationError("duplicate [package] name")
            original = match.group(2)
            lines[index] = (
                f'{match.group(1)}"{expected}"{match.group(3)}{ending}'
            )
            found = True
    if not found:
        raise VerificationError("missing [package] name")
    return "".join(lines), original


def rewrite_filtered_manifests(repo, entries, packages):
    rewritten = dict(entries)
    package_names = {}
    missing = []
    manifests = {}
    for destination, expected in packages.items():
        entry = rewritten.get(destination)
        if entry is None:
            missing.append(destination)
            continue
        mode, oid, source = entry
        manifest = run(["git", "-C", str(repo), "cat-file", "blob", oid]).stdout
        manifest, original = rewrite_package_name(manifest, expected)
        manifests[destination] = (mode, source, manifest)
        package_names[original] = expected

    package_pattern = None
    if package_names:
        alternatives = "|".join(
            re.escape(name) for name in sorted(package_names, key=len, reverse=True)
        )
        package_pattern = re.compile(
            rf'(package\s*=\s*")({alternatives})(")'
        )

    for destination, (mode, oid, source) in list(rewritten.items()):
        if not destination.endswith("Cargo.toml"):
            continue
        if destination in manifests:
            manifest = manifests[destination][2]
        else:
            manifest = run(["git", "-C", str(repo), "cat-file", "blob", oid]).stdout
        if package_pattern is not None:
            manifest = package_pattern.sub(
                lambda match: (
                    match.group(1) + package_names[match.group(2)] + match.group(3)
                ),
                manifest,
            )
        new_oid = run(
            ["git", "-C", str(repo), "hash-object", "-w", "--stdin"],
            input_text=manifest,
        ).stdout.strip()
        rewritten[destination] = (mode, new_oid, source)

    renamed = sum(old != new for old, new in package_names.items())
    return rewritten, {
        "configured": len(packages),
        "present": len(package_names),
        "renamed": renamed,
        "missing": sorted(missing),
    }


def write_filtered_tree(repo, entries):
    index = Path(repo) / "assessment.index"
    index.unlink(missing_ok=True)
    env = deterministic_env({"GIT_INDEX_FILE": str(index)})
    run(["git", "read-tree", "--empty"], cwd=repo, env=env)
    records = "".join(
        f"{mode} {oid}\t{destination}\0"
        for destination, (mode, oid, _) in sorted(entries.items())
    )
    run(
        ["git", "update-index", "-z", "--index-info"],
        cwd=repo,
        env=env,
        input_text=records,
    )
    return run(["git", "write-tree"], cwd=repo, env=env).stdout.strip()


SUBSYSTEMS = (
    "window",
    "scene",
    "renderer_macos",
    "renderer_windows",
    "renderer_wgpu",
    "renderer_web",
    "renderer_linux",
    "renderer_other",
    "elements",
    "text",
    "platform_view",
    "a11y",
    "other_kit_unrelated",
)


def classify_subsystem(path):
    lowered = path.lower()
    name = PurePosixPath(lowered).name
    if "platform_view" in lowered or "platform-view" in lowered:
        return "platform_view"
    if "/a11y" in lowered or "accessibility" in lowered or "accesskit" in lowered:
        return "a11y"
    renderer_path = any(
        part in name
        for part in ("renderer", "shader", "atlas", "display_link", "surface")
    )
    if lowered.startswith("crates/gpui_wgpu/"):
        return "renderer_wgpu"
    if renderer_path and lowered.startswith("crates/gpui_macos/"):
        return "renderer_macos"
    if renderer_path and lowered.startswith("crates/gpui_windows/"):
        return "renderer_windows"
    if renderer_path and lowered.startswith("crates/gpui_web/"):
        return "renderer_web"
    if renderer_path and lowered.startswith("crates/gpui_linux/"):
        return "renderer_linux"
    if renderer_path:
        return "renderer_other"
    if name == "scene.rs" or "/scene/" in lowered:
        return "scene"
    if name == "window.rs" or "/window/" in lowered:
        return "window"
    if any(
        marker in lowered
        for marker in ("/text", "font", "glyph", "line_layout", "shap")
    ):
        return "text"
    if "/elements/" in lowered:
        return "elements"
    return "other_kit_unrelated"


def conflict_marker_stats(path):
    try:
        lines = Path(path).read_text(errors="surrogateescape").splitlines()
    except (OSError, UnicodeError):
        return 1, 0, True
    hunks = 0
    payload_lines = 0
    in_conflict = False
    for line in lines:
        if line.startswith("<<<<<<< "):
            hunks += 1
            in_conflict = True
        elif in_conflict and line.startswith(">>>>>>> "):
            in_conflict = False
        elif in_conflict and line != "=======":
            payload_lines += 1
    if hunks == 0:
        return 1, 0, True
    return hunks, payload_lines, False


def numstat(repo, args, excluded=frozenset()):
    output = run(
        ["git", "-C", str(repo), "diff", "--numstat", "--no-renames", *args]
    ).stdout
    files = additions = deletions = binaries = 0
    for line in output.splitlines():
        added, deleted, path = line.split("\t", 2)
        if path in excluded:
            continue
        files += 1
        if added == "-" or deleted == "-":
            binaries += 1
            continue
        additions += int(added)
        deletions += int(deleted)
    return {
        "files": files,
        "additions": additions,
        "deletions": deletions,
        "changed_lines": additions + deletions,
        "binary_files": binaries,
    }


def validate_config(config):
    if config.get("schema_version") != 3:
        raise VerificationError("unsupported config schema")
    if config.get("mode") != MODE:
        raise VerificationError("config must freeze the historical import")
    if config.get("filter_schema_version") != 1:
        raise VerificationError("unsupported filter schema")
    if config.get("history_algorithm") != HISTORY_ALGORITHM:
        raise VerificationError("unsupported historical import algorithm")
    for key in ("official_baseline", "bootstrap_revision"):
        if not SHA.fullmatch(config.get(key, "")):
            raise VerificationError(f"{key} must be a full lowercase SHA")
    vendor_ref = config.get("vendor_ref", "")
    if not vendor_ref.startswith("refs/heads/vendor/") or ".." in vendor_ref:
        raise VerificationError(f"unsafe historical vendor_ref: {vendor_ref}")

    mappings = config.get("mappings")
    if not isinstance(mappings, list) or not mappings:
        raise VerificationError("at least one historical mapping is required")
    seen_source, seen_destination = set(), set()
    for mapping in mappings:
        source = mapping.get("source", "")
        destination = mapping.get("destination", "")
        for value in (source, destination):
            path = PurePosixPath(value)
            if not value or path.is_absolute() or ".." in path.parts:
                raise VerificationError(f"unsafe historical mapping path: {value}")
        if source in seen_source or destination in seen_destination:
            raise VerificationError(
                f"duplicate historical mapping: {source} -> {destination}"
            )
        seen_source.add(source)
        seen_destination.add(destination)
    if config.get("filter_digest_sha256") != filter_digest(mappings):
        raise VerificationError(
            "filter_digest_sha256 differs from the frozen mapping list"
        )

    overlay = config.get("fork_overlay")
    if not isinstance(overlay, dict):
        raise VerificationError("historical fork overlay definition is required")
    if overlay.get("algorithm") != OVERLAY_ALGORITHM:
        raise VerificationError("unsupported historical fork overlay algorithm")
    if overlay.get("source_url") != config.get("bootstrap_url"):
        raise VerificationError("fork overlay source_url must equal bootstrap_url")
    if overlay.get("base_revision") != config.get("bootstrap_revision"):
        raise VerificationError("fork overlay base_revision must equal bootstrap_revision")
    overlay_ref = overlay.get("vendor_ref", "")
    if (
        not overlay_ref.startswith("refs/heads/vendor/")
        or ".." in overlay_ref
        or overlay_ref == vendor_ref
    ):
        raise VerificationError(f"unsafe historical overlay vendor_ref: {overlay_ref}")
    revisions = overlay.get("source_revisions")
    if not isinstance(revisions, list) or not revisions:
        raise VerificationError("fork overlay source_revisions must be nonempty")
    if len(set(revisions)) != len(revisions):
        raise VerificationError("fork overlay source_revisions contains duplicates")
    if not all(isinstance(revision, str) and SHA.fullmatch(revision) for revision in revisions):
        raise VerificationError(
            "fork overlay source_revisions must contain full lowercase SHAs"
        )


def validate_state(config, state):
    errors = []
    if state.get("schema_version") != config.get("schema_version"):
        errors.append("state/config disagree on schema_version")
    if state.get("mode") != MODE:
        errors.append("state must freeze the historical import")
    if state.get("tool_version") != VERSION:
        errors.append("state/verifier version disagree")
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
    for key in RECEIPT_KEYS:
        value = state.get(key)
        if not isinstance(value, str) or not SHA.fullmatch(value):
            errors.append(f"frozen import receipt requires {key}")

    overlay_config = config["fork_overlay"]
    overlay = state.get("fork_overlay")
    if not isinstance(overlay, dict):
        errors.append("state historical fork_overlay receipt is required")
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
    for key in OVERLAY_RECEIPT_KEYS:
        value = overlay.get(key)
        if not isinstance(value, str) or not SHA.fullmatch(value):
            errors.append(f"frozen overlay receipt requires {key}")
    if overlay.get("base_vendor_tip") != state.get("bootstrap_vendor_tip"):
        errors.append("fork_overlay.base_vendor_tip must equal bootstrap_vendor_tip")
    return errors


def provenance_section_values(section):
    values = {}
    in_section = False
    lines = PROVENANCE.read_text().splitlines()
    index = 0
    while index < len(lines):
        stripped = lines[index].strip()
        if re.fullmatch(r"\[[a-z_]+\]", stripped):
            in_section = stripped == f"[{section}]"
            index += 1
            continue
        if not in_section or not stripped or stripped.startswith("#"):
            index += 1
            continue
        match = re.match(r"^([a-z0-9_]+)\s*=\s*(.*)$", stripped)
        if not match:
            raise VerificationError(
                f"cannot parse provenance [{section}] line: {stripped}"
            )
        key, raw = match.groups()
        if key in values:
            raise VerificationError(f"duplicate provenance [{section}] key: {key}")
        if raw.startswith("["):
            while not raw.rstrip().endswith("]"):
                index += 1
                if index >= len(lines):
                    raise VerificationError(
                        f"unterminated provenance [{section}] array: {key}"
                    )
                raw += "\n" + lines[index].strip()
            raw = re.sub(r",\s*]$", "]", raw)
        if raw.startswith(('"', "[")):
            values[key] = json.loads(raw)
        elif raw in ("true", "false"):
            values[key] = raw == "true"
        elif re.fullmatch(r"[0-9]+", raw):
            values[key] = int(raw)
        else:
            raise VerificationError(
                f"unsupported provenance [{section}] value: {key}"
            )
        index += 1
    return values


def provenance_errors(config, state):
    source = provenance_section_values("historical_source")
    expected_source = {
        "mode": MODE,
        "official_url": config["official_url"],
        "official_baseline": config["official_baseline"],
        "bootstrap_url": config["bootstrap_url"],
        "bootstrap_revision": config["bootstrap_revision"],
        "relationship": "frozen-filtered-import",
        "cargo_git_dependency": False,
        "official_project": False,
    }
    errors = [
        f"provenance.toml [historical_source] {key} differs from the frozen receipt"
        for key, value in expected_source.items()
        if source.get(key) != value
    ]
    historical_import = provenance_section_values("historical_import")
    expected_import = {
        "config": "scripts/sync-zed/config.json",
        "state": "scripts/sync-zed/state.json",
        "filter_schema_version": config["filter_schema_version"],
        "history_algorithm": config["history_algorithm"],
        **{key: state[key] for key in RECEIPT_KEYS},
    }
    errors.extend(
        f"provenance.toml [historical_import] {key} differs from the frozen receipt"
        for key, value in expected_import.items()
        if historical_import.get(key) != value
    )
    overlay = provenance_section_values("historical_overlay")
    errors.extend(
        f"provenance.toml [historical_overlay] {key} differs from the frozen receipt"
        for key, value in state["fork_overlay"].items()
        if overlay.get(key) != value
    )
    return errors


def commit_exists(commit, repo=ROOT):
    return (
        run(
            ["git", "-C", str(repo), "cat-file", "-e", f"{commit}^{{commit}}"],
            check=False,
        ).returncode
        == 0
    )


def exact_ref_errors(ref, expected, label, repo=ROOT):
    result = run(
        ["git", "-C", str(repo), "rev-parse", "--verify", ref], check=False
    )
    if result.returncode:
        return [f"missing frozen {label} ref: {ref}"]
    if result.stdout.strip() != expected:
        return [f"frozen {label} ref differs from receipt: {ref}"]
    return []


def markers(repo, commit, names):
    message = run(
        ["git", "-C", str(repo), "show", "-s", "--format=%B", commit]
    ).stdout
    values = {}
    for line in message.splitlines():
        for name in names:
            prefix = name + ": "
            if line.startswith(prefix):
                if name in values:
                    raise VerificationError(
                        f"duplicate {name} marker in integration {commit}"
                    )
                values[name] = line[len(prefix) :]
    if not values:
        return None
    missing = [name for name in names if name not in values]
    if missing:
        raise VerificationError(
            f"incomplete integration markers in {commit}: missing {', '.join(missing)}"
        )
    return values


def integration_markers(repo, commit):
    return markers(
        repo,
        commit,
        ("zed-sync-algorithm", "zed-vendor-tip", "zed-upstream-cursor"),
    )


def overlay_integration_markers(repo, commit):
    result = markers(
        repo,
        commit,
        (
            "zed-overlay-algorithm",
            "zed-overlay-base-vendor-tip",
            "zed-overlay-vendor-tip",
            "zed-overlay-source-tip",
        ),
    )
    if result is not None and integration_markers(repo, commit) is not None:
        raise VerificationError(
            f"integration {commit} carries both import and overlay markers"
        )
    return result


def trailer(repo, commit, name):
    message = run(
        ["git", "-C", str(repo), "show", "-s", "--format=%B", commit]
    ).stdout
    values = [
        line[len(name) + 2 :]
        for line in message.splitlines()
        if line.startswith(name + ": ")
    ]
    if len(values) != 1 or not SHA.fullmatch(values[0]):
        return None
    return values[0]


def merge_receipt_errors(
    repo,
    head,
    integration,
    vendor_tip,
    expected_markers,
    marker_reader,
    source_trailer,
    label,
):
    errors = []
    parents = run(
        ["git", "-C", str(repo), "show", "-s", "--format=%P", integration]
    ).stdout.split()
    if len(parents) != 2 or parents[1] != vendor_tip:
        errors.append(f"{label} integration must merge its frozen vendor tip second")
    history = run(
        ["git", "-C", str(repo), "rev-list", "--first-parent", head]
    ).stdout.splitlines()
    if integration not in history:
        errors.append(f"{label} integration is not on HEAD's first-parent history")
        return errors
    try:
        actual_markers = marker_reader(repo, integration)
    except VerificationError as exc:
        errors.append(str(exc))
        actual_markers = None
    if actual_markers != expected_markers:
        errors.append(f"{label} integration markers disagree with the frozen receipt")
    newer = history[: history.index(integration)]
    for commit in newer:
        try:
            if marker_reader(repo, commit) is not None:
                errors.append(f"newer marked {label} integration exists: {commit}")
        except VerificationError as exc:
            errors.append(str(exc))
        parents = run(
            ["git", "-C", str(repo), "show", "-s", "--format=%P", commit]
        ).stdout.split()
        if any(trailer(repo, parent, source_trailer) is not None for parent in parents[1:]):
            errors.append(f"newer unrecorded {label} integration exists: {commit}")
    return errors


def historical_git_errors(config, state, head, repo=ROOT):
    overlay = state["fork_overlay"]
    commits = {
        "bootstrap_vendor_tip": state["bootstrap_vendor_tip"],
        "vendor_tip": state["vendor_tip"],
        "integration_commit": state["integration_commit"],
        "fork_overlay.vendor_tip": overlay["vendor_tip"],
        "fork_overlay.integration_commit": overlay["integration_commit"],
    }
    errors = [
        f"frozen receipt commit does not exist: {name}={commit}"
        for name, commit in commits.items()
        if not commit_exists(commit, repo)
    ]
    if errors:
        return errors
    errors.extend(
        exact_ref_errors(
            config["vendor_ref"], state["vendor_tip"], "official vendor", repo
        )
    )
    errors.extend(
        exact_ref_errors(
            overlay["vendor_ref"], overlay["vendor_tip"], "fork overlay vendor", repo
        )
    )
    if state["vendor_tip"] != state["bootstrap_vendor_tip"]:
        errors.append("frozen official lane must remain at its bootstrap vendor tip")
    if trailer(repo, state["bootstrap_vendor_tip"], "zed-upstream") != config[
        "bootstrap_revision"
    ]:
        errors.append("bootstrap vendor commit has the wrong source trailer")

    paths = run(
        [
            "git",
            "-C",
            str(repo),
            "ls-tree",
            "-r",
            "--name-only",
            state["bootstrap_vendor_tip"],
        ]
    ).stdout.splitlines()
    for mapping in config["mappings"]:
        destination = mapping["destination"]
        if not any(
            path == destination or path.startswith(destination + "/") for path in paths
        ):
            errors.append(f"frozen vendor tree lacks mapped destination: {destination}")

    overlay_chain = run(
        [
            "git",
            "-C",
            str(repo),
            "rev-list",
            "--reverse",
            f"{state['bootstrap_vendor_tip']}..{overlay['vendor_tip']}",
        ]
    ).stdout.splitlines()
    source_revisions = overlay["source_revisions"]
    if len(overlay_chain) != len(source_revisions):
        errors.append("frozen overlay commit count differs from its source receipt")
    else:
        expected_parent = state["bootstrap_vendor_tip"]
        for commit, source_revision in zip(overlay_chain, source_revisions):
            parents = run(
                ["git", "-C", str(repo), "show", "-s", "--format=%P", commit]
            ).stdout.split()
            if parents != [expected_parent]:
                errors.append(f"historical overlay commit has wrong parent: {commit}")
            if trailer(repo, commit, "zed-fork-overlay-upstream") != source_revision:
                errors.append(f"historical overlay commit has wrong source trailer: {commit}")
            expected_parent = commit
    merge_bases = run(
        [
            "git",
            "-C",
            str(repo),
            "merge-base",
            "--all",
            state["vendor_tip"],
            overlay["vendor_tip"],
        ]
    ).stdout.splitlines()
    if merge_bases != [state["bootstrap_vendor_tip"]]:
        errors.append("frozen vendor lanes do not meet only at the bootstrap tip")

    errors.extend(
        merge_receipt_errors(
            repo,
            head,
            state["integration_commit"],
            state["vendor_tip"],
            {
                "zed-sync-algorithm": state["history_algorithm"],
                "zed-vendor-tip": state["vendor_tip"],
                "zed-upstream-cursor": state["last_synced_sha"],
            },
            integration_markers,
            "zed-upstream",
            "historical import",
        )
    )
    errors.extend(
        merge_receipt_errors(
            repo,
            head,
            overlay["integration_commit"],
            overlay["vendor_tip"],
            {
                "zed-overlay-algorithm": overlay["algorithm"],
                "zed-overlay-base-vendor-tip": overlay["base_vendor_tip"],
                "zed-overlay-vendor-tip": overlay["vendor_tip"],
                "zed-overlay-source-tip": overlay["source_tip"],
            },
            overlay_integration_markers,
            "zed-fork-overlay-upstream",
            "historical overlay",
        )
    )
    return errors


def repository_errors(config):
    errors = []
    tracked = set(run(["git", "ls-files"]).stdout.splitlines())
    forbidden = tuple(path + "/" for path in config["forbidden_destinations"])
    for path in tracked:
        if path.startswith(forbidden):
            errors.append(f"forbidden tracked Zed product path: {path}")
    for mapping in config["mappings"]:
        if not (ROOT / mapping["destination"]).exists():
            errors.append(f"missing historical import destination: {mapping['destination']}")
    for manifest, expected in config["packages"].items():
        try:
            actual = package_name(ROOT / manifest)
        except Exception as exc:
            errors.append(f"cannot read package {manifest}: {exc}")
            continue
        if actual != expected:
            errors.append(
                f"package identity {manifest}: expected {expected}, got {actual}"
            )
    for license_path in ("LICENSE-APACHE", "licenses/ZED-APACHE-2.0.txt"):
        if not (ROOT / license_path).is_file():
            errors.append(f"missing imported-source license: {license_path}")
    return errors


def verify(_args):
    config, state = load(CONFIG), load(STATE)
    validate_config(config)
    errors = validate_state(config, state)
    errors.extend(provenance_errors(config, state))
    errors.extend(repository_errors(config))
    if not errors:
        head = run(["git", "rev-parse", "HEAD"]).stdout.strip()
        errors.extend(historical_git_errors(config, state, head))
    if errors:
        raise VerificationError("verification failed:\n- " + "\n- ".join(errors))
    print("frozen Zed historical import verification passed (offline)")


def status(_args):
    state = load(STATE)
    print(
        json.dumps(
            {
                "mode": state["mode"],
                "historical_import": {
                    key: state[key] for key in ("vendor_ref", *RECEIPT_KEYS)
                },
                "historical_overlay": state["fork_overlay"],
            },
            indent=2,
        )
    )


def assess(args):
    config, state = load(CONFIG), load(STATE)
    validate_config(config)
    errors = validate_state(config, state)
    if errors:
        raise VerificationError("invalid frozen receipt:\n- " + "\n- ".join(errors))
    revision = args.revision.lower()
    if not SHA.fullmatch(revision):
        raise VerificationError("--revision must be a full 40-character SHA")

    with tempfile.TemporaryDirectory(prefix="sync-zed-assess-") as directory:
        repo = Path(directory) / "merge"
        run(["git", "init", "--quiet", str(repo)], env=deterministic_env())
        run(
            ["git", "config", "user.name", "GPUI Box assessment"], cwd=repo
        )
        run(
            ["git", "config", "user.email", "assessment@invalid"], cwd=repo
        )
        run(["git", "config", "merge.conflictStyle", "merge"], cwd=repo)
        run(["git", "config", "rerere.enabled", "false"], cwd=repo)

        run(
            [
                "git",
                "fetch",
                "--quiet",
                "--no-tags",
                str(ROOT),
                "+HEAD:refs/assess/ours",
                f"+{config['vendor_ref']}:refs/assess/base",
            ],
            cwd=repo,
            env=deterministic_env(),
        )
        base = run(["git", "rev-parse", "refs/assess/base"], cwd=repo).stdout.strip()
        ours = run(["git", "rev-parse", "refs/assess/ours"], cwd=repo).stdout.strip()
        if base != state["vendor_tip"]:
            raise VerificationError(
                "local frozen vendor ref differs from the recorded vendor tip"
            )

        run(
            [
                "git",
                "fetch",
                "--quiet",
                "--no-tags",
                "--depth=1",
                config["official_url"],
                f"+{revision}:refs/assess/source",
            ],
            cwd=repo,
            env=deterministic_env(),
        )
        fetched = run(
            ["git", "rev-parse", "refs/assess/source^{commit}"], cwd=repo
        ).stdout.strip()
        if fetched != revision:
            raise VerificationError(
                f"fetched revision differs from requested revision: {fetched}"
            )

        entries = source_entries(repo, "refs/assess/source", config["mappings"])
        entries, package_stats = rewrite_filtered_manifests(
            repo, entries, config["packages"]
        )
        tree = write_filtered_tree(repo, entries)
        theirs = run(
            ["git", "commit-tree", tree, "-p", base],
            cwd=repo,
            env=deterministic_env(),
            input_text=f"Filtered Zed assessment for {revision}\n",
        ).stdout.strip()
        merge_base = run(["git", "merge-base", ours, theirs], cwd=repo).stdout.strip()
        if merge_base != base:
            raise VerificationError(
                f"assessment merge base differs from frozen vendor base: {merge_base}"
            )

        run(["git", "checkout", "--quiet", "--detach", ours], cwd=repo)
        merge_result = run(
            ["git", "merge", "--no-commit", "--no-ff", theirs],
            cwd=repo,
            check=False,
            env=deterministic_env(),
        )
        if merge_result.returncode not in (0, 1):
            raise VerificationError(
                "temporary merge failed:\n" + merge_result.stderr.strip()
            )

        conflict_output = run(
            [
                "git",
                "diff",
                "--name-only",
                "--diff-filter=U",
                "-z",
            ],
            cwd=repo,
        ).stdout
        conflict_paths = sorted(
            path for path in conflict_output.rstrip("\0").split("\0") if path
        )
        if merge_result.returncode == 1 and not conflict_paths:
            raise VerificationError(
                "temporary merge failed without producing inspectable conflicts:\n"
                + merge_result.stderr.strip()
            )

        subsystem_stats = {
            subsystem: {"files": 0, "hunks": 0, "payload_lines": 0}
            for subsystem in SUBSYSTEMS
        }
        conflict_files = []
        unmarked = 0
        total_hunks = total_lines = 0
        for path in conflict_paths:
            hunks, payload_lines, has_no_markers = conflict_marker_stats(repo / path)
            subsystem = classify_subsystem(path)
            total_hunks += hunks
            total_lines += payload_lines
            unmarked += int(has_no_markers)
            subsystem_stats[subsystem]["files"] += 1
            subsystem_stats[subsystem]["hunks"] += hunks
            subsystem_stats[subsystem]["payload_lines"] += payload_lines
            conflict_files.append(
                {
                    "path": path,
                    "subsystem": subsystem,
                    "hunks": hunks,
                    "payload_lines": payload_lines,
                    "has_text_markers": not has_no_markers,
                }
            )

        top_files = sorted(
            conflict_files,
            key=lambda item: (-item["payload_lines"], -item["hunks"], item["path"]),
        )
        clean_merge = numstat(
            repo,
            ["--cached", "HEAD"],
            excluded=frozenset(conflict_paths),
        )
        upstream_change = numstat(repo, [base, theirs])
        report = {
            "schema_version": 1,
            "assessment_only": True,
            "official_url": config["official_url"],
            "revision": revision,
            "base": base,
            "ours": ours,
            "merge_base": merge_base,
            "filter": {
                "schema_version": config["filter_schema_version"],
                "digest_sha256": config["filter_digest_sha256"],
                "files": len(entries),
                "packages": package_stats,
            },
            "filtered_upstream_change": upstream_change,
            "conflicts": {
                "files": len(conflict_paths),
                "hunks": total_hunks,
                "payload_lines": total_lines,
                "unmarked_or_binary_files": unmarked,
                "by_subsystem": subsystem_stats,
                "top_files": top_files[:10],
            },
            "non_conflict_auto_merge": clean_merge,
            "definitions": {
                "conflict_payload_lines": (
                    "Both sides' text between Git conflict markers; marker lines are excluded. "
                    "An unmarked or binary conflict counts as one hunk and zero payload lines."
                ),
                "non_conflict_auto_merge": (
                    "Additions plus deletions relative to ours in staged, fully conflict-free "
                    "files; automatically merged portions of conflicted files are excluded."
                ),
            },
        }
        print(json.dumps(report, indent=2))


def parser():
    command_parser = argparse.ArgumentParser(
        prog="sync-zed",
        description=(
            "Verify GPUI Box's frozen Zed import history or assess an upstream "
            "revision without importing it. Mutation commands are permanently retired."
        ),
    )
    commands = command_parser.add_subparsers(dest="command", required=True)
    verify_parser = commands.add_parser("verify")
    verify_parser.set_defaults(func=verify)
    status_parser = commands.add_parser("status")
    status_parser.set_defaults(func=status)
    assess_parser = commands.add_parser(
        "assess",
        help="measure a temporary three-way merge without changing the repository",
    )
    assess_parser.add_argument("--revision", required=True)
    assess_parser.set_defaults(func=assess)
    return command_parser


def main(argv=None):
    args = parser().parse_args(argv)
    try:
        args.func(args)
    except VerificationError as exc:
        print(f"sync-zed: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
