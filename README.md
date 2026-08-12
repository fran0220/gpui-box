# GPUI Box

GPUI Box is an independent crates.io distribution of the GPUI framework
and a product-neutral component system for native and browser-hosted Rust
interfaces. The project lives at <https://github.com/fran0220/gpui-box>.
It is derived from GPUI source imported from Zed, but it is **not an official
Zed project** and Zed is not a Cargo dependency.

The umbrella workspace provides one framework, one token authority, truthful
async states, caller-owned data and actions, stable semantic automation, and
deterministic scene fixtures. It contains no OriginGame/Forge domain logic,
credentials, transports, accounts, or product models.

**Catalog endpoint:** <https://gpui-box.origingame.dev> ·
**Hosted MCP:** <https://gpui-box.origingame.dev/mcp>

## Packages

All publishable packages are versioned as the `0.1.x` cohort. Cargo package
names and Rust crate names intentionally differ:

| Cargo package(s) | Rust import | Purpose | License |
|---|---|---|---|
| `gpui-box` | `gpui` | Core framework and the authoritative GPUI type universe | Apache-2.0 |
| `gpui-box-platform` | `gpui_platform` | Platform application construction | Apache-2.0 |
| `gpui-box-macos`, `gpui-box-linux`, `gpui-box-windows`, `gpui-box-web` | platform-specific | Platform implementations | Apache-2.0 |
| `gpui-box-wgpu` | `gpui_wgpu` | WGPU renderer and offscreen support | Apache-2.0 |
| `gpui-box-collections`, `-http-client`, `-macros`, `-media`, `-refineable`, `-refineable-derive`, `-scheduler`, `-shared-string`, `-sum-tree`, `-util`, `-util-macros` | manifest-defined | Framework support crates | Apache-2.0 |
| `gpui-box-kit` | `gpui_kit` | Components, scenes, motion, strings, themes, and truthful state | MIT |
| `gpui-box-kit-tokens`, `-theme`, `-assets`, `-semantics`, `-testkit` | `gpui_kit_*` | Kit package family | MIT (bundled assets retain their licenses) |
| `gpui-box-mcp` | binary | Checkout-backed MCP catalog tools | MIT |

Gallery, browser-gallery, headless, perf, and `xtask` packages are workspace
tools and are not published. `package-authority.toml` is the exact authority
for package identity and publishability.

## Depend on GPUI Box

Use Cargo aliases so source code uses the conventional `gpui` and `gpui_kit`
imports while resolving the GPUI Box packages:

```toml
[dependencies]
gpui = { package = "gpui-box", version = "0.1.1" }
gpui_platform = { package = "gpui-box-platform", version = "0.1.1" }
gpui_kit = { package = "gpui-box-kit", version = "0.1.1" }

[dev-dependencies]
gpui_kit_testkit = { package = "gpui-box-kit-testkit", version = "0.1.1", features = ["test-support"] }
```

Do not add another GPUI implementation to the same application. Every
framework and kit crate in this cohort resolves through `gpui-box`, producing a
single package/type/global universe. Consumers neither pin Zed nor add a Zed
Git dependency.

The core defaults enable `font-kit`, Wayland, X11, and Windows manifest support;
turn defaults off and select `wayland`, `x11`, `windows-manifest`,
`screen-capture`, `inspector`, or other core features when a host needs a
narrower build. `gpui-box-kit`'s `fixtures` feature is only the deterministic
calendar used by scenes and tests; keep it off in products.

Requires Rust 1.97 and edition 2024.

## Boot and component model

```rust
use gpui_kit::prelude::*;

let app = gpui_platform::application().with_assets(gpui_kit::assets::Assets);
app.run(|cx| gpui_kit::install(cx));
```

Install the asset source while constructing the application, then call
`gpui_kit::install` before opening a window. Components hold only transient
visual state. Values, selections, persistence, transports, and refusals belong
to the caller; disabled controls install no action handler. See
[`docs/host-view-boundary.md`](docs/host-view-boundary.md) and
[`docs/truthful-ui.md`](docs/truthful-ui.md).

Tokens in `crates/gpui-kit-tokens/tokens/*.json` are authoritative. The kit
re-exports its common assets, theme, token, and semantics APIs. Public Rust API,
token keys, and stable semantic ids are all compatibility surfaces.

## Gallery and validation

```bash
cargo run -p gpui-box-gallery
cargo run -p xtask -- scenes render
cargo run -p xtask -- gate
cargo run -p xtask -- gate full
cargo run -p xtask -- headless check
cargo run -p xtask -- web check
cargo run -p xtask -- web build
cargo run -p xtask -- web smoke
cargo run -p xtask -- web visual check button input dialog node-graph
cargo run -p xtask -- package plan
cargo run -p xtask -- package check
```

`scenes render` writes real-window review images; it is not a baseline gate.
Headless baselines and CI gates cover macOS, Linux, and Windows, and the native
matrix compiles every feature on all three. Browser CI cross-checks WASM and
drives a real Chromium smoke. Browser validation is single-threaded and does
not claim screen-reader announcements. Exact support and limitations are in
[`docs/compatibility.md`](docs/compatibility.md).

## Imported framework and independence

Framework source was filtered into this repository from
`fran0220/zed@0b9c8dc932b65cba2dc87464148984e93f60ae18`, against official Zed
baseline `a6a23c7b80a5cefa0487b7856335be89ace7e483`. That import is frozen:
GPUI Box now develops its framework, platforms, media, and Kit directly in this
repository. `scripts/sync-zed` only verifies the offline historical receipt;
Zed is neither a linked package nor an ongoing development source. See
[`PROVENANCE.md`](PROVENANCE.md).

## License

The framework cohort is Apache-2.0. GPUI Box Kit and MCP are MIT. Bundled Geist
and Noto fonts are OFL 1.1; Solar Icons are CC BY 4.0; other derived assets
retain their stated terms. See [`THIRD_PARTY_NOTICES`](THIRD_PARTY_NOTICES).

The former <https://gpui-kit.origingame.dev> endpoint remains a compatibility
alias backed by the same Worker; it is not a second deployment. Deployment
details are in [`docs/deploying.md`](docs/deploying.md), and release operations
are in [`docs/releasing.md`](docs/releasing.md).
