# Deploying the GPUI Box catalog

The production catalog endpoint is <https://gpui-box.origingame.dev>. The
former <https://gpui-kit.origingame.dev> hostname remains a compatibility alias
served by the same Worker, so existing clients continue to receive the exact
same catalog and MCP implementation.

```bash
tools/site/deploy.sh
```

The script checks the API index, generates the site, copies the shared MCP tool
description, and deploys one Cloudflare Worker. `/` serves people and `/mcp`
serves agents. Hosted `render_scene` returns committed catalog captures rather
than compiling per request; the stdio server in [`mcp.md`](mcp.md) is required
for working-copy rendering.

```json
{
  "mcpServers": {
    "gpui-box": { "url": "https://gpui-box.origingame.dev/mcp" }
  }
}
```

## Credentials and routing

Authenticate locally with `wrangler login`; set `CLOUDFLARE_ACCOUNT_ID` when
needed. CI/orb deployment requires `CLOUDFLARE_ACCOUNT_ID` and a
least-privilege `CLOUDFLARE_API_TOKEN` with Workers Scripts Edit and the current
zone's Workers Routes Edit permission. Keep those values in protected project
settings and never print them.

Both hostnames are bound to the same Worker:

```toml
[[routes]]
pattern = "gpui-box.origingame.dev"
custom_domain = true

[[routes]]
pattern = "gpui-kit.origingame.dev"
custom_domain = true
```

The GPUI Box hostname is canonical. The GPUI Kit hostname is compatibility
infrastructure, not a second site or a forked catalog. `workers_dev = false`,
so these custom domains are the production routes.

## Published inputs

| Path | Source |
|---|---|
| `/` and `/components/*` | generated from `docs/api-index.json` |
| `/scenes/*` | canonical scene source and committed theme captures |
| `/docs/*` | public documentation |
| `/llms.txt`, `/api-index.json` | machine-readable catalog inputs |
| `/images/*` | fingerprinted macOS scene captures |
| `/mcp` | `tools/site/worker/index.js` and `tools/mcp/tools.json` |

`tools/site/public/` is generated and uncommitted. `cargo run -p xtask -- site
check` builds it in scratch space without deploying.
