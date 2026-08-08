# The catalog as tools

`tools/mcp` is a Model Context Protocol server over stdio. It exposes what this
repository already generates — the API index and the scene renderer — as tools
an agent can call, so finding a component and looking at one are single calls
rather than a guess about which file to open.

It reads the repository and renders from it. It does not write to it. An agent
that wants to change a component edits the source and runs `gate`; a tool here
that patched files would be a way around the check that means something.

## Connecting it

```bash
cargo build --release -p gpui-kit-mcp
```

Then point a client at the binary. The shape differs by client, but every one
needs a command, and most accept something close to this:

```json
{
  "mcpServers": {
    "gpui-kit": {
      "command": "/absolute/path/to/gpui-kit/target/release/gpui-kit-mcp",
      "env": { "GPUI_KIT_ROOT": "/absolute/path/to/gpui-kit" }
    }
  }
}
```

`GPUI_KIT_ROOT` is optional. Without it the server finds the repository from
its own location, which is correct for a binary built in this workspace and
wrong for one copied elsewhere.

## The tools

| Tool | Answers |
|---|---|
| `search_components` | What is there? Matches name, summary and module. |
| `component` | What exactly is it called? Constructors, chainable options, commands that need a `Context`, queries, and the events it reports. |
| `scene` | What does using it look like? The source of a canonical scene, which the gate compiles and renders. |
| `render_scene` | What does it look like? Runs the real renderer and returns the PNG. |
| `rules` | What will fail the build? `docs/llms.txt`. |

`render_scene` is the one worth having. Every other tool moves text around;
that one draws the component and hands back the image, so the answer to "does
this look right" is the picture rather than a description of it. It shells out
to the gallery, so a cold build takes tens of seconds and a warm one a few.

## What it is not

It is not a way to build an interface without the repository. The signatures
it returns are Rust, the scenes it renders are this catalog, and the images it
produces come from the same renderer `scenes check` uses. It is a faster route
to the same facts, which is the only kind of tool worth adding.

Nor does it replace `gate`. The server can tell an agent what a component is
called and show it what one looks like; only `cargo run -p xtask -- gate` can
tell it whether what it wrote is allowed to land.
