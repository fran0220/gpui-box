#!/usr/bin/env bash
# Builds the site and the MCP endpoint from the checked-in catalog, then
# deploys both as one Worker.
#
# Everything it publishes is derived: the pages come from `docs/api-index.json`
# and `docs/*.md`, the images come from `snapshots/macos/scenes`, and the tool
# list comes from `tools/mcp/tools.json`. Nothing here is authored, so a deploy
# cannot say something the repository does not.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

# The index is what both the pages and the tools answer out of, so a stale one
# would publish signatures that no longer compile.
cargo run -q -p xtask -- api check

cargo run -q -p xtask -- site generate tools/site/public

# The tool surface is shared with the stdio server rather than written twice.
cp tools/mcp/tools.json tools/site/worker/tools.json

cd tools/site
npx --yes wrangler@4 deploy "$@"
