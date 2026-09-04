# Contributing to GPUI Box

1. Read `AGENTS.md`, `PROVENANCE.md`, and the relevant design document.
2. Preserve the boundary: framework primitives are product-neutral; kit
   components read caller data, emit caller actions, and hold transient visual
   state only.
3. Add semantic tokens before duplicating repeated visual values. Give every
   action/assertion target a stable business-derived semantic id.
4. Cover reducers and transitions with tests and add gallery states when a
   component gains visible behavior.
5. Iterate on visible components with `cargo run -p xtask -- gate only
   <scene>`. Before committing, run `cargo run -p xtask -- gate` for changes
   that cannot affect rendering, or run `cargo run -p xtask -- gate full`
   instead when they can. `gate full` already includes the regular gate; do not
   run both back to back.
6. For UI changes inspect every image reported by `headless check`, and use
   `headless capture` only after review. Linux, macOS, and Windows each carry
   a renderer-specific baseline. The Linux one is compared by `gate full` at
   every commit; the other two are refreshed by dispatching the `Platforms`
   workflow and accepting its frames with
   `tools/headless-visual/accept-run.sh`.
7. Nothing builds on push in GitHub. After pushing to `main`, run
   `tools/site/deploy-main.sh` so the hosted catalog follows the commit.
8. Add a reader-facing changelog entry. Rust API, token-key, and semantic-id
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

Routine dev and test profiles retain source locations for backtraces while
omitting variable-level debug data, which reduces this native workspace's
target size and link time. For an interactive debugger session, opt back
in explicitly with `CARGO_PROFILE_DEV_DEBUG=full` or
`CARGO_PROFILE_TEST_DEBUG=full`. The two Cargo workspaces have separate target
directories; avoid an unconditional `cargo clean`, and clean the root and
`tools/headless-visual` workspace separately only when their artifacts are no
longer useful.

Kit integration tests live as modules under `crates/gpui-kit/tests/it/` and
compile through the single `tests/main.rs` harness. Keep new suites in that
harness: one binary lets Cargo share the large Kit and testkit link result while
the module-qualified test names still support ordinary substring filters and
`xtask gate only <scene>`.
