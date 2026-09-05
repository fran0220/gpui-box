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

The bundle carries `EXPECTED_REVISION`, observed before the main-revision
check and before building. Under its activation lock the installer compares
that value with the installed `REVISION`; a late build cannot overwrite a
release activated since its observation. Redeploying the current revision is
idempotent. Explicit recovery can build a previous revision with the current
installed revision as its expectation; first installation uses `none`.
`build-release.sh` requires `GPUI_BOX_EXPECTED_REVISION`; `deploy.sh` observes
it automatically unless explicitly supplied for recovery.

Health probes have 1-second connection and 2-second total timeouts, with a
60-second health deadline. Lock acquisition and service commands are bounded;
the receiver gives the whole installation 240 seconds plus 40 seconds for
termination/rollback before its last-resort kill. Local fault/CAS tests run via
`python3 -m unittest discover -s tools/site/tests -v` without touching BWG.

**Host-command rollout:** the restricted archive receiver does not update its
own root scripts from an uploaded release. Existing hosts must install the
updated `ops/install-release.sh` and `ops/receive-release.sh` through the
authorized host bootstrap procedure before the CAS/deadline protection is
active. An ordinary catalog deployment alone does not prove this migration.

```bash
tools/site/deploy-main.sh
```

That is the ordinary deployment, run from the orb right after `git push`.
It fetches `origin/main`, refuses a `HEAD` that differs from it or a dirty
tree, resolves the deployment identity (`GPUI_BOX_DEPLOY_SSH_KEY`, or the Amp
environment secret `ORIGINGAME_SSH_PRIVATE_KEY_B64` written once to
`~/.ssh/gpui-box-deploy`), addresses the receiver as
`${ORIGINGAME_BWG_SSH_USER:-root}@$ORIGINGAME_BWG_SSH_HOST` unless
`GPUI_BOX_DEPLOY_HOST` says otherwise, pins the host key to the committed
`tools/site/ops/bwg-known-hosts`, runs `tools/site/deploy.sh`, and finishes
with `tools/site/verify-deployment.sh`. `.agents/setup` installs `musl-tools`
and the musl target so a fresh orb can do this; on macOS the release build
uses `cargo-zigbuild` instead.

`tools/site/verify-deployment.sh [revision]` is the proof, and it is the same
proof whichever lane deployed: both hostnames serve `/build-info.json` naming
the revision with the package, symbol, component, type, theme, guide, recipe,
and scene counts of the committed `docs/developer-index.json`; `POST /mcp`
`tools/list` matches every tool name and full schema in `tools/mcp/tools.json`;
and paginated `search_components` with an empty query matches the structured
component ID multiset (including duplicate detection). Both hostnames receive
the same MCP protocol checks, not only build-info requests. Every
request pins the BWG address so the answer is about the origin, not DNS.

Nothing deploys on push. `.github/workflows/deploy-site.yml` is a
dispatch-only fallback for when no machine holds the deployment identity; it
runs the same two scripts on a hosted runner with the repository secret
`BWG_GPUI_BOX_DEPLOY_KEY`, whose key is restricted to
`receive-gpui-box-release`. The release builder independently checks the
generated indexes, builds the site, semantic snapshots, and static MCP
binary, and validates the complete immutable bundle before BWG accepts it, so
either lane keeps the hosted catalog on the exact `origin/main` revision.

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
