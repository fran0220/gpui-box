# Deploying the GPUI Box catalog

The existing production catalog endpoint is
<https://gpui-kit.origingame.dev>. The hostname is real current infrastructure;
it has not been renamed and this document makes no DNS-migration claim. Pages
and client labels should identify the product as **GPUI Box**.

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
    "gpui-box": { "url": "https://gpui-kit.origingame.dev/mcp" }
  }
}
```

## Credentials and routing

Authenticate locally with `wrangler login`; set `CLOUDFLARE_ACCOUNT_ID` when
needed. CI/orb deployment requires `CLOUDFLARE_ACCOUNT_ID` and a
least-privilege `CLOUDFLARE_API_TOKEN` with Workers Scripts Edit and the current
zone's Workers Routes Edit permission. Keep those values in protected project
settings and never print them.

The current Worker route remains:

```toml
[[routes]]
pattern = "gpui-kit.origingame.dev"
custom_domain = true
```

Do not change this key merely to match branding; a hostname migration requires
an intentional infrastructure/DNS change. `workers_dev = false`, so this custom
domain is the production route.

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
