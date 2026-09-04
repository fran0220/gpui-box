#!/usr/bin/env bash
# Prove that the BWG origin serves one exact revision of this catalog.
#
# Usage: tools/site/verify-deployment.sh [revision]
#
# The revision defaults to HEAD. Every request pins the origin address so the
# answer is about BWG rather than about DNS or a cache in front of it. The
# checks are the ones AGENTS.md asks for after every deployment:
#
# 1. `/build-info.json` on both hostnames names the revision and carries the
#    same package, symbol, component, type, theme, guide, recipe, and scene
#    counts as the committed `docs/developer-index.json`;
# 2. `POST /mcp` `tools/list` returns every remote tool in
#    `tools/mcp/tools.json`;
# 3. `search_components` with an empty query returns every component.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
revision="${1:-$(git -C "$root" rev-parse HEAD)}"
origin="${GPUI_BOX_ORIGIN_ADDRESS:-67.230.183.147}"
hosts=(gpui-box.origingame.dev gpui-kit.origingame.dev)

expected_counts="$(python3 - "$root/docs/developer-index.json" "$root/tools/mcp/tools.json" <<'PY'
import json
import sys

developer = json.load(open(sys.argv[1]))
tools = json.load(open(sys.argv[2]))
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
counts["tools"] = len(tools)
print(json.dumps(counts))
PY
)"
expected_counts="$(jq -c -S . <<<"$expected_counts")"

fetch() {
  local host="$1"
  shift
  curl --fail --silent --show-error --resolve "$host:443:$origin" "$@"
}

for host in "${hosts[@]}"; do
  build="$(fetch "$host" "https://$host/build-info.json")"
  actual_revision="$(jq -r '.revision' <<<"$build")"
  if [[ "$actual_revision" != "$revision" ]]; then
    echo "$host serves $actual_revision, expected $revision" >&2
    exit 1
  fi
  actual_counts="$(jq -c -S '.counts' <<<"$build")"
  if [[ "$actual_counts" != "$expected_counts" ]]; then
    echo "$host counts $actual_counts differ from committed $expected_counts" >&2
    exit 1
  fi
  echo "$host serves $revision with counts $actual_counts"
done

mcp() {
  fetch gpui-box.origingame.dev \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json' \
    --data "$1" \
    https://gpui-box.origingame.dev/mcp
}

expected_tools="$(jq -r '.tools' <<<"$expected_counts")"
actual_tools="$(mcp '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | jq '.result.tools | length')"
if [[ "$actual_tools" != "$expected_tools" ]]; then
  echo "tools/list returned $actual_tools tools, expected $expected_tools" >&2
  exit 1
fi
echo "tools/list returns $actual_tools tools"

expected_components="$(jq -r '.components' <<<"$expected_counts")"
search="$(mcp '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_components","arguments":{"query":""}}}')"
actual_components="$(jq -r '.result.content[0].text' <<<"$search" | sed -n '1s/^\([0-9]*\) match(es).*/\1/p')"
if [[ "$actual_components" != "$expected_components" ]]; then
  echo "search_components returned '$actual_components' matches, expected $expected_components" >&2
  exit 1
fi
echo "search_components with an empty query returns all $actual_components components"
