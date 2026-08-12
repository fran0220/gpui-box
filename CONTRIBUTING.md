# Contributing to GPUI Box

1. Read `AGENTS.md`, `PROVENANCE.md`, and the relevant design document.
2. Preserve the boundary: framework primitives are product-neutral; kit
   components read caller data, emit caller actions, and hold transient visual
   state only.
3. Add semantic tokens before duplicating repeated visual values. Give every
   action/assertion target a stable business-derived semantic id.
4. Cover reducers and transitions with tests and add gallery states when a
   component gains visible behavior.
5. Iterate with `cargo run -p xtask -- gate only <scene>`, then run
   `cargo run -p xtask -- gate` and `gate full` before committing.
6. For UI changes run `headless check`, inspect every reported image, and use
   `headless capture` only after review. macOS, Linux, and Windows each carry an
   active renderer-specific CI baseline.
7. Add a reader-facing changelog entry. Rust API, token-key, and semantic-id
   compatibility changes must be explicit.

Use public package names in commands (`gpui-box-gallery`, `gpui-box-mcp`,
`gpui-box-kit`). Internal Cargo aliases and physical directories that still say
`gpui-kit` are implementation details and need not be renamed.

Framework and platform work is developed directly in this repository. The
receipt in `scripts/sync-zed/` is read-only historical provenance, not an update
workflow; never restore Zed as a Cargo Git dependency or synchronization source.
New derived source/assets require exact URL, revision, license, and scope in
`PROVENANCE.md` and `THIRD_PARTY_NOTICES`. GPUI Box is independent and must not
be presented as an official Zed project.
