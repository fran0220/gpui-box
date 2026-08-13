# GPUI Box catalog tools

Install the published stdio server with:

```bash
cargo install gpui-box-mcp --version 0.1.2
gpui-box-mcp --help
```

This is the registry interface for the `0.1.2` release. For workspace
development use `cargo build --release -p gpui-box-mcp`.

The server deliberately contains no embedded static catalog. It reads the
calling project's GPUI Box checkout: set `GPUI_BOX_ROOT` to the checkout root,
or start the server with cwd inside the checkout and it discovers the root by
walking upward. It does not infer a checkout from the installed binary.

```json
{
  "mcpServers": {
    "gpui-box": {
      "command": "gpui-box-mcp",
      "env": { "GPUI_BOX_ROOT": "/absolute/path/to/gpui-box" }
    }
  }
}
```

| Tool | Result |
|---|---|
| `search_components` | Matches catalog names, summaries, and modules |
| `component` | Exact constructors, options, commands, queries, and reports |
| `scene` | Canonical compiling scene source |
| `render_scene` | PNG rendered from the checkout's real gallery |
| `rules` | The checkout's `docs/llms.txt` |

`render_scene` shells out to the gallery and therefore requires a **complete
checkout**, Rust toolchain, dependencies, platform renderer, and build time. A
crate installation alone cannot render. The server reads and renders but does
not edit files; it does not replace `cargo run -p xtask -- gate`.

The hosted endpoint at <https://gpui-box.origingame.dev/mcp> is different: it
serves the currently deployed static catalog and committed images. A browser
GET lands on the human MCP page; agents POST JSON-RPC to the same path. See
[`deploying.md`](deploying.md).
