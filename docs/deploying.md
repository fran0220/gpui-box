# Deploying the GPUI Box catalog

The production service is owned entirely by this repository and runs on BWG.
Nginx serves the generated site and terminates TLS; a dedicated unprivileged
systemd service listens on `127.0.0.1:9350` for stateless MCP requests.
`gpui-box.origingame.dev` is canonical and `gpui-kit.origingame.dev` remains an
alias to the same release. There is no Cloudflare Worker or Pages runtime and
no OriginGame repository dependency.

```diagram
DNS ──▶ BWG Nginx ──┬──▶ /opt/gpui-box/current/public
                    └──▶ 127.0.0.1:9350 (gpui-box-mcp)
```

## Release flow

`tools/site/build-release.sh` requires a clean checkout and builds one
`gpui-box-linux-x64-<12-sha>.tar.gz`:

1. check both generated indexes;
2. build the release WASM browser gallery and static site;
3. copy the shared remote tool schemas and write `build-info.json`;
4. generate every scene's redacted semantic snapshot through the offscreen
   session host;
5. build a static Linux/x86-64 MCP executable;
6. validate the complete hosted catalog and write a SHA-256 manifest.

`tools/site/deploy.sh` streams that archive over SSH. The deployment key is
restricted to `receive-gpui-box-release`; the receiver rejects traversal,
links, special files, multiple roots, oversized archives, and invalid bundle
names. The root installer verifies every file, binary architecture/static
linkage, revision, catalog completeness, and runtime health before switching
the `current` symlink atomically. Failure restores the previous release. Five
releases are retained.

```bash
rustup target add x86_64-unknown-linux-musl
tools/site/deploy.sh
```

On macOS the release build uses `cargo-zigbuild`; Linux uses `musl-tools`.
Normal deployments run in `.github/workflows/deploy-site.yml` on every push to
`main`, in parallel with the platform CI. The release builder independently
checks the generated indexes, builds the site, semantic snapshots, and static
MCP binary, and validates the complete immutable bundle before BWG accepts it.
This keeps the hosted catalog on the exact `origin/main` revision even when an
unrelated platform CI job is unavailable or failing. The only repository
secret is the restricted private key `BWG_GPUI_BOX_DEPLOY_KEY`.

## One-time host bootstrap

Generate a dedicated key, install its private half as the GitHub secret, then
bootstrap from this repository:

```bash
ssh-keygen -t ed25519 -N '' -C gpui-box-github-production \
  -f /tmp/gpui-box-production
tools/site/bootstrap-bwg.sh /tmp/gpui-box-production.pub
gh secret set BWG_GPUI_BOX_DEPLOY_KEY --repo fran0220/gpui-box \
  < /tmp/gpui-box-production
```

Bootstrap installs only GPUI Box files: `/etc/nginx/conf.d/gpui-box.conf`,
`/etc/systemd/system/gpui-box-mcp.service`, and two GPUI Box release commands
under `/usr/local/sbin`. It creates the `gpui-box` runtime user and
`/opt/gpui-box`; it does not modify an OriginGame checkout or service. The
pinned BWG Ed25519 host key is in `tools/site/ops/bwg-known-hosts`.

## Published inputs

| Path | Source |
|---|---|
| `/` | generated catalog home and live browser specimen |
| `/components/*`, `/docs/*`, `/compose/*` | component pages, guides, and release WASM gallery |
| GET `/mcp/` | human Developer MCP page |
| POST `/mcp` or `/mcp/` | stateless Streamable HTTP MCP |
| `/api-index.json` | compiler-checked Kit component/scene contract |
| `/developer-index.json` | package, symbol, token, guide, asset, compatibility, and Kit catalogs |
| `/resources/guides/*`, `/resources/tokens/*` | raw MCP-readable authority documents |
| `/build-info.json` | deployed revision and catalog counts |
| `/semantic/{theme}/{scene}.json` | deploy-time redacted semantics |
| `/images/*` | fingerprinted committed macOS scene captures |

## Routing and rollback

Both DNS records point to `67.230.183.147`; the wildcard certificate on BWG
covers both names. During migration, verify the origin before changing DNS:

```bash
curl --resolve gpui-box.origingame.dev:443:67.230.183.147 \
  https://gpui-box.origingame.dev/build-info.json
curl --resolve gpui-box.origingame.dev:443:67.230.183.147 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  https://gpui-box.origingame.dev/mcp
```

The retired `gpui-kit` Worker may remain unattached briefly as a migration
rollback artifact, but it is not a deployment lane. For a manual service
rollback, resolve `/opt/gpui-box/previous`, create a temporary symlink to that
release, atomically move it over `/opt/gpui-box/current`, and restart
`gpui-box-mcp`. Do not point `current` at the `previous` symlink itself.

`tools/site/public/` and `target/site-release/` are generated and uncommitted.
`cargo run -p xtask -- site check` builds the complete static projection in
scratch space without deploying.
