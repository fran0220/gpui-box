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
  curl --fail --silent --show-error --connect-timeout 5 --max-time 30 --resolve "$host:443:$origin" "$@"
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
  fetch "$host" \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json' \
    --data "$1" \
    "https://$host/mcp"
}

expected_tools="$(jq -cS 'sort_by(.name)' "$root/tools/mcp/tools.json")"
expected_components="$(jq -cS '[.components[].name | "component:" + .] | sort' "$root/docs/developer-index.json")"
for host in "${hosts[@]}"; do
  actual_tools="$(mcp '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | jq -ecS '.result.tools | sort_by(.name)')"
  [[ "$actual_tools" == "$expected_tools" ]] || {
    echo "$host tools/list differs in names or complete schemas" >&2; exit 1;
  }
  cursor=0
  ids='[]'
  while :; do
    request="$(jq -nc --arg cursor "$cursor" '{jsonrpc:"2.0",id:2,method:"tools/call",params:{name:"search_components",arguments:{query:"",cursor:$cursor,limit:500}}}')"
    search="$(mcp "$request" | jq -ec '.result.structuredContent')"
    page="$(jq -ec '[.matches[].id]' <<<"$search")"
    ids="$(jq -nc --argjson ids "$ids" --argjson page "$page" '$ids + $page')"
    next="$(jq -r '.nextCursor // empty' <<<"$search")"
    [[ -n "$next" ]] || break
    [[ "$next" =~ ^[0-9]+$ && "$next" -gt "$cursor" && "$next" -le "$(jq length <<<"$expected_components")" ]] || {
      echo "$host invalid component pagination" >&2; exit 1;
    }
    cursor="$next"
  done
  [[ "$(jq -cS sort <<<"$ids")" == "$expected_components" ]] || {
    echo "$host structured component identities differ (including duplicates)" >&2; exit 1;
  }
  echo "$host tools match full schemas; structured component identities match the catalog"
done
