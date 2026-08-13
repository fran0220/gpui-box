#!/usr/bin/env bash
# Builds the site and the MCP endpoint from the checked-in catalog, then
# deploys both as one Worker.
#
# Everything it publishes is derived: the pages come from `docs/api-index.json`
# and `docs/*.md`, the images come from `snapshots/headless/macos/scenes`, the
# live playground comes from the browser-gallery WASM build, and the tool list
# comes from `tools/mcp/tools.json`. Nothing here is authored twice, so a deploy
# cannot say something the repository does not.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

if [[ -n "${AMP_ORB:-}" ]]; then
  : "${CLOUDFLARE_ACCOUNT_ID:?Set CLOUDFLARE_ACCOUNT_ID as an Amp project environment variable}"
  : "${CLOUDFLARE_API_TOKEN:?Set CLOUDFLARE_API_TOKEN as an Amp project secret}"
fi

# The index is what both the pages and the tools answer out of, so a stale one
# would publish signatures that no longer compile.
cargo run -q -p xtask -- api check

# `site generate` builds and bundles the browser gallery before projecting the
# static catalog, so the Worker receives one self-contained output directory.
cargo run -q -p xtask -- site generate tools/site/public

# The tool surface is shared with the stdio server rather than written twice.
cp tools/mcp/tools.json tools/site/worker/tools.json

cd tools/site
npx --yes wrangler@4 deploy "$@"
