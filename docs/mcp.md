# GPUI Box catalog tools

The catalog is this repository, not the last crates.io cohort (`0.1.1`).
Workspace `main` is ahead of that cohort. Both MCP surfaces read the generated
index from this tree:

| Surface | What it serves |
|---|---|
| Stdio `gpui-box-mcp` / `tools/mcp/run.sh` | This checkout. `render_scene` compiles the gallery now. |
| Hosted `https://gpui-box.origingame.dev/mcp` | The last **deploy** of this repository. `render_scene` returns committed captures. |

Neither surface reads a crates.io install. `cargo install gpui-box-mcp`
without a git revision still installs `0.1.1`. Do not pass `--version 0.1.2`:
that tag was never published.

```bash
tools/mcp/run.sh --help
cargo build -p gpui-box-mcp
```

The stdio server contains no embedded static catalog. Set `GPUI_BOX_ROOT` to
the checkout root, or start it with cwd inside the checkout and it discovers
the root by walking upward. It does not infer a checkout from the installed
binary.

```json
{
  "mcpServers": {
    "gpui-box": {
      "command": "bash",
      "args": ["tools/mcp/run.sh"]
    }
  }
}
```

| Tool | Result |
|---|---|
| `search_components` | Matches catalog names, summaries, and modules. A non-empty query also matches supporting types (`CardHeader`, `CardVariant`, `AsyncValue`). `kind=type` lists only those. |
| `component` | Exact constructors, options, commands, queries, reports, and variants |
| `scene` | Canonical compiling scene source |
| `render_scene` | PNG from this checkout's gallery (stdio) or the committed capture (hosted) |
| `rules` | This checkout's `docs/llms.txt` |

`render_scene` on stdio shells out to the gallery and therefore requires a
**complete checkout**, Rust toolchain, dependencies, platform renderer, and
build time. A crate installation alone cannot render. The server reads and
renders but does not edit files; it does not replace
`cargo run -p xtask -- gate`.

The hosted endpoint deploys with [`tools/site/deploy.sh`](deploying.md). A
browser GET lands on the human MCP page; agents POST JSON-RPC to the same
path. See [`deploying.md`](deploying.md).
