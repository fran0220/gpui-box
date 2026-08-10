# Deploying the site and the hosted catalog

Live at <https://gpui-kit.origingame.dev>. One Cloudflare Worker
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
      "url": "https://gpui-kit.origingame.dev/mcp"
    }
  }
}
```

A client that only speaks stdio can use `mcp-remote` against the same URL. The
tool surface is identical to the local server's, because both read
`tools/mcp/tools.json`; only what they serve differs.

## Credentials

Local deployment uses the Cloudflare session `wrangler` already holds; a person
creates one with `wrangler login`. Set `CLOUDFLARE_ACCOUNT_ID` when that session
can see more than one account.

Every Amp orb receives deployment access from the `doufunao/gpui-kit` project
settings. Configure the account id as an environment variable and the
OriginGame Cloudflare deployment token as a secret (the commands read values
from standard input so they do not enter shell history):

```bash
printf '%s' '<account-id>' |
  amp secrets set CLOUDFLARE_ACCOUNT_ID --project doufunao/gpui-kit --env --data-file -
printf '%s' '<new-token>' |
  amp secrets set CLOUDFLARE_API_TOKEN --project doufunao/gpui-kit --secret --data-file -
```

Use a token limited to the deployment account with **Account / Workers Scripts /
Edit** and to the `origingame.dev` zone with **Zone / Workers Routes / Edit**.
Cloudflare's custom-domain route creates the DNS record and certificate during
`wrangler deploy`; no DNS credential or manually managed record is needed.
The current project secret is copied without printing it from the trusted local
OriginGame environment, because both projects deploy into the same account and
zone. A future rotation must update both Amp project secrets. Use a dedicated
token instead if the two projects need independent revocation or permissions.

The Orb lifecycle caches Wrangler during `.agents/setup` and reports whether
both variables were injected without printing either value. In an Orb,
`tools/site/deploy.sh` refuses to start unless both are present. There is no
long-running service to restart in `.agents/resume` because the deployed Worker
runs on Cloudflare, not inside the Orb.

The custom domain is the only production route (`workers_dev = false`). The
Worker configuration is authoritative for both DNS and routing:

```toml
[[routes]]
pattern = "gpui-kit.origingame.dev"
custom_domain = true
```

## What a deploy publishes

| Path | From |
|---|---|
| `/` | `xtask site generate`, out of `docs/api-index.json` |
| `/components/<Name>` | the index entry, signatures and all |
| `/scenes/<name>` | the scene's own source, and both captured themes |
| `/docs/<page>` | `docs/*.md` |
| `/llms.txt`, `/api-index.json` | served as themselves, for a program |
| `/images/<fingerprint>/*.png` | `snapshots/macos/scenes`; the content-derived directory prevents a new page from receiving an old cached capture |
| `/mcp` | `tools/site/worker/index.js` with `tools/mcp/tools.json` |

`tools/site/public/` is generated and not committed. A generated file belongs
in a repository when a reviewer should see it change, which is true of an API
index and false of the markup derived from it. `cargo run -p xtask -- site
check` builds the site into a scratch directory and throws it away, which is
what there is to prove: that the inputs still render.
