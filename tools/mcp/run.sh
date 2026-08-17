#!/usr/bin/env bash
# Serve this checkout's catalog over stdio MCP.
#
# crates.io `gpui-box-mcp` and https://gpui-box.origingame.dev/mcp are the last
# published cohort. This script is the working tree. Do not call `cargo metadata`
# on the hot path: Cursor times out stdio MCP startup if another cargo holds
# the package-cache lock.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
export GPUI_BOX_ROOT="${GPUI_BOX_ROOT:-$root}"

if [[ ! -f "$GPUI_BOX_ROOT/package-authority.toml" || ! -f "$GPUI_BOX_ROOT/docs/api-index.json" ]]; then
  echo "gpui-box-mcp: GPUI_BOX_ROOT=$GPUI_BOX_ROOT is not a GPUI Box checkout" >&2
  exit 1
fi

bin="$GPUI_BOX_ROOT/target/debug/gpui-box-mcp"
if [[ ! -x "$bin" ]]; then
  cargo build --manifest-path "$GPUI_BOX_ROOT/Cargo.toml" -p gpui-box-mcp
fi
exec "$bin" "$@"
