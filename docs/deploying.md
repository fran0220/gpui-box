# Deploying the GPUI Box catalog

The production catalog endpoint is <https://gpui-box.origingame.dev>. The
former <https://gpui-kit.origingame.dev> hostname remains a compatibility alias
served by the same Worker, so existing clients continue to receive the exact
same catalog and MCP implementation.

```bash
tools/site/deploy.sh
```

The script checks the API index, builds the browser gallery, generates the
static catalog around it, copies the shared MCP tool description, and deploys
one Cloudflare Worker. `/` is the catalog home: compose, scenes, and
components. `/docs/` and `/mcp/` are the only other pages. `/compose/` is the
full GPUI surface, also embedded twice on the home page so both themes stay
visible. Agents POST JSON-RPC to `/mcp`. The hosted catalog is this repository's last
deploy, not a crates.io install. Hosted `render_scene` returns committed
catalog captures rather than compiling per request; the stdio server in
[`mcp.md`](mcp.md) is required for working-copy rendering.

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
| `/` | catalog home: compose, scenes with their components, and the component list |
| `/compose/*` | release WASM browser gallery, also embedded lazily in both themes |
| `/docs/*` | public documentation |
| `/mcp/` | human catalog page; POST `/mcp` remains the JSON-RPC endpoint |
| `/llms.txt`, `/api-index.json` | machine-readable catalog inputs |
| `/images/*` | fingerprinted macOS scene captures |
| `/components/*`, `/scenes/*`, `/playground/` | compatibility redirects onto the home or compose surface |

`tools/site/public/` is generated and uncommitted. Both `site generate` and
`site check` require the `wasm32-unknown-unknown` target and the locked
`wasm-bindgen-cli` version; `cargo run -p xtask -- site check` builds the
complete output in scratch space without deploying.
