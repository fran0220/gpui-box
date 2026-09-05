#!/usr/bin/env bash
# Restricted root SSH command: receive and safely unpack one GPUI Box bundle.
set -euo pipefail

[[ "$(id -u)" -eq 0 ]] || { echo "receiver must run as root" >&2; exit 1; }
if [[ -n "${SSH_ORIGINAL_COMMAND:-}" && "$SSH_ORIGINAL_COMMAND" != "/usr/local/sbin/receive-gpui-box-release" ]]; then
  echo "unsupported deployment command" >&2
  exit 1
fi

umask 077
ulimit -f 262144
work="$(mktemp -d /var/lib/gpui-box/incoming/release.XXXXXX)"
trap 'rm -rf "$work"' EXIT
archive="$work/release.tar.gz"
cat > "$archive"

bundle_name="$(python3 - "$archive" "$work/unpacked" <<'PY'
import pathlib
import re
import shutil
import sys
import tarfile

archive_path = pathlib.Path(sys.argv[1])
destination = pathlib.Path(sys.argv[2])
destination.mkdir(mode=0o700)
tops = set()
paths = set()
total = 0
limit = 512 * 1024 * 1024

with tarfile.open(archive_path, "r:gz") as archive:
    for member in archive:
        path = pathlib.PurePosixPath(member.name)
        parts = path.parts
        if not parts or path.is_absolute() or ".." in parts or "." in parts:
            raise SystemExit(f"unsafe archive path: {member.name!r}")
        if member.name in paths:
            raise SystemExit(f"duplicate archive path: {member.name!r}")
        paths.add(member.name)
        tops.add(parts[0])
        if not (member.isdir() or member.isreg()):
            raise SystemExit(f"links and special files are forbidden: {member.name!r}")
        total += member.size
        if total > limit:
            raise SystemExit("release expands past 512 MiB")

        target = destination.joinpath(*parts)
        if member.isdir():
            target.mkdir(mode=0o700, parents=True, exist_ok=True)
            continue
        target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        source = archive.extractfile(member)
        if source is None:
            raise SystemExit(f"could not read archive member: {member.name!r}")
        with target.open("xb") as output:
            shutil.copyfileobj(source, output)

if len(tops) != 1:
    raise SystemExit("release must contain exactly one top-level directory")
name = next(iter(tops))
match = re.fullmatch(r"gpui-box-linux-x64-([0-9a-f]{12})", name)
if match is None:
    raise SystemExit(f"invalid release directory: {name!r}")
print(name)
PY
)"

# Bound validation, lock acquisition, activation and rollback as a whole.
# TERM lets the installer's EXIT trap restore the previous release; KILL is
# only a last resort for an unresponsive external command.
timeout --kill-after=40 240 /usr/local/sbin/install-gpui-box-release "$work/unpacked/$bundle_name"
