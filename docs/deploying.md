# Deploying the site and the hosted catalog

Live at <https://gpui-kit.zhangfan0220.workers.dev>. One Cloudflare Worker
serves both halves of the same catalog: a site a person reads at `/`, and an
MCP endpoint an agent calls at `/mcp`.

```bash
tools/site/deploy.sh
```

That checks the API index is current, regenerates the site from it, copies the
shared tool list next to the Worker, and deploys. Everything published is
derived from the repository, so a deploy cannot state something the gate has
not already checked.

## There is no server

This is the part worth understanding before anyone provisions a host.

`render_scene` looks like it needs a renderer, and hosted it does not. It can
only ever draw scenes from the catalog, and the catalog is 82 scenes in two
themes — a fixed set of 164 images that are already captured, already
committed, and already compared pixel for pixel on every run. Because captures
in this repository are deterministic, the bytes a hosted renderer would produce
are the bytes in `snapshots/macos/scenes`. Rendering per request would spend a
GPU-less rasterizer, a long cold build, a request timeout and an attack surface
to produce a file that is already there.

So the whole deployment is 11 MB of images, a 340 KB index, and a few hundred
lines of JavaScript. It fits in a Worker's free tier and needs no VM, no
container, no GPU and no scaling story.

**What it therefore cannot do** is show a component you are in the middle of
changing. The hosted server serves the published revision. For a component that
only exists in your working copy, run the stdio server described in
[`docs/mcp.md`](mcp.md), which renders from the code as it currently stands.
The two answer different questions, and `initialize` says which one you reached
rather than letting a caller assume.

Live rendering would only become worth building if the goal changed from
"show the catalog" to "render arbitrary submitted code", which is a different
product with a sandboxing problem attached. For the record, the shape it would
need is a Linux VM with two cores and four gigabytes running the software
rasterizer from `tools/headless-visual` — no GPU required. That harness also
rides an unmerged GPUI pull request, which is not a thing to put in production.

## Connecting an agent to the hosted catalog

```json
{
  "mcpServers": {
    "gpui-kit": {
      "url": "https://gpui-kit.zhangfan0220.workers.dev/mcp"
    }
  }
}
```

A client that only speaks stdio can use `mcp-remote` against the same URL. The
tool surface is identical to the local server's, because both read
`tools/mcp/tools.json`; only what they serve differs.

## Credentials

Deployment uses whatever Cloudflare session `wrangler` already holds, which for
a person is `wrangler login`. Nothing is stored in this repository and no
account id is committed — pass one as `CLOUDFLARE_ACCOUNT_ID` when the session
can see more than one account.

For CI, mint a **new** API token scoped to `Workers Scripts: Edit` for the one
account, and store it as a repository secret. Do not copy an existing token
from another project: a long-lived credential duplicated into a third place is
a third place it can leak from, and a scoped token can be revoked without
taking anything else down with it.

## What a deploy publishes

| Path | From |
|---|---|
| `/` | `xtask site generate`, out of `docs/api-index.json` |
| `/components/<Name>` | the index entry, signatures and all |
| `/scenes/<name>` | the scene's own source, and both captured themes |
| `/docs/<page>` | `docs/*.md` |
| `/llms.txt`, `/api-index.json` | served as themselves, for a program |
| `/images/*.png` | `snapshots/macos/scenes` |
| `/mcp` | `tools/site/worker/index.js` with `tools/mcp/tools.json` |

`tools/site/public/` is generated and not committed. A generated file belongs
in a repository when a reviewer should see it change, which is true of an API
index and false of the markup derived from it. `cargo run -p xtask -- site
check` builds the site into a scratch directory and throws it away, which is
what there is to prove: that the inputs still render.
