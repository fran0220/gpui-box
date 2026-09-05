#!/usr/bin/env bash
# Build one immutable Linux/x86-64 site + remote-MCP release from HEAD.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
out="${1:-$root/target/site-release}"
revision="$(git -C "$root" rev-parse HEAD)"
short_revision="${revision:0:12}"
bundle_name="gpui-box-linux-x64-$short_revision"
bundle="$out/$bundle_name"
archive="$out/$bundle_name.tar.gz"
target="x86_64-unknown-linux-musl"

if [[ -n "$(git -C "$root" status --porcelain --untracked-files=normal)" ]]; then
  echo "release builds require a clean working tree so the catalog equals $revision" >&2
  exit 1
fi

rm -rf "$out"
mkdir -p "$bundle/bin" "$bundle/public/mcp"

cd "$root"
cargo run --locked -q -p xtask -- api check
cargo run --locked -q -p xtask -- site generate "$bundle/public"
cp tools/mcp/tools.json "$bundle/public/mcp/tools.json"

python3 - "$bundle/public" "$revision" <<'PY'
import json
import pathlib
import subprocess
import sys

public = pathlib.Path(sys.argv[1])
revision = sys.argv[2]
developer = json.loads((public / "developer-index.json").read_text())
committed_at = subprocess.check_output(
    ["git", "show", "-s", "--format=%cI", revision], text=True
).strip()
counts = {
    key: len(developer[key])
    for key in (
        "packages",
        "symbols",
        "components",
        "types",
        "themes",
        "guides",
        "recipes",
        "scenes",
    )
}
counts["tools"] = len(json.loads((public / "mcp/tools.json").read_text()))
build = {
    "schema": 1,
    "revision": revision,
    "committedAt": committed_at,
    "catalogSchema": developer["schema"],
    "releaseVersion": developer["project"]["releaseVersion"],
    "counts": counts,
}
(public / "build-info.json").write_text(
    json.dumps(build, indent=2, ensure_ascii=False) + "\n"
)
PY

# Semantic snapshots are generated through the same persistent offscreen host
# used by checkout MCP sessions. The hosted server never starts a renderer.
cargo run --locked -q -p gpui-box-mcp -- snapshot-catalog --output "$bundle/public"

rustup target list --installed | grep -qx "$target" || {
  echo "missing Rust target $target" >&2
  exit 1
}
if [[ "$(uname -s)" == Linux ]]; then
  cargo build --locked --release -p gpui-box-mcp --target "$target"
else
  command -v cargo-zigbuild >/dev/null || {
    echo "cargo-zigbuild is required for a Linux release from $(uname -s)" >&2
    exit 1
  }
  cargo zigbuild --locked --release -p gpui-box-mcp --target "$target"
fi
cp "target/$target/release/gpui-box-mcp" "$bundle/bin/gpui-box-mcp"
chmod 0755 "$bundle/bin/gpui-box-mcp"
printf '%s\n' "$revision" > "$bundle/REVISION"
printf '%s\n' "${GPUI_BOX_EXPECTED_REVISION:?deployment needs the expected installed revision (or none for bootstrap)}" > "$bundle/EXPECTED_REVISION"

# Validate the exact catalog with a native binary. The Linux installer repeats
# this check using the bundled static executable before switching `current`.
cargo run --locked -q -p gpui-box-mcp -- check \
  --catalog "$bundle/public" --revision "$revision"

manifest="$(mktemp)"
trap 'rm -f "$manifest"' EXIT
(
  cd "$bundle"
  find . -type f ! -name SHA256SUMS -print0 \
    | LC_ALL=C sort -z \
    | xargs -0 sha256sum
) > "$manifest"
mv "$manifest" "$bundle/SHA256SUMS"
trap - EXIT
COPYFILE_DISABLE=1 tar -C "$out" -czf "$archive" "$bundle_name"
printf '%s  %s\n' "$(sha256sum "$archive" | cut -d' ' -f1)" "$(basename "$archive")" \
  > "$archive.sha256"
printf '%s\n' "$archive"
