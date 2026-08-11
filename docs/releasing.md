# Releasing GPUI Box

This is the runbook for a crates.io release. It describes the intended process;
it does not claim that any GPUI Box version has already been published.

## Cohort and authority

`package-authority.toml` defines every package name, version, cohort, license,
and publish flag. Framework packages are Apache-2.0; kit packages and
`gpui-box-mcp` are MIT. A release keeps mutually dependent publishable packages
in one compatible version cohort (currently `0.1.x`) and records the contract
in `compatibility.toml` and `provenance.toml`.

Never hand-maintain publication order. Derive it from Cargo metadata:

```bash
cargo run -p xtask -- dependencies check
cargo run -p xtask -- package plan
```

Save the plan in the release log. Independent packages may appear early, and
platform packages may appear after consumers; the generated order, not a prose
list, is authoritative.

## Preflight and dry run

1. Select the release commit; ensure versions and internal requirements form
   the intended cohort, changelog has the release date, machine-readable
   records are current, and the worktree is clean. For an initial release,
   confirm every package name is available; for later releases, confirm the
   expected crates.io owner set. `scripts/sync-zed/sync-zed verify --release`
   must prove the complete filtered-history receipt and its ancestry through
   this commit; an unbootstrapped receipt cannot be released.
2. Run the platform validation required by `compatibility.toml`. Record actual
   CI job URLs/results; do not infer a platform result from another platform.
3. Run the complete local package gate:

   ```bash
   cargo run -p xtask -- gate full
   cargo run -p xtask -- package plan
   cargo run -p xtask -- package check
   ```

   `package check` requires `cargo-local-registry` 0.2.12. It packages every
   publishable crate, constructs a registry under `target/package-check`, then
   builds framework-only and framework-plus-kit consumers offline with source
   replacement and without path patches. It also runs a packaged
   `gpui::property_test`, installs the MCP binary and checks its help/version,
   rejects retained internal dev-dependencies that would deadlock a first
   publication, and requires exactly one `lib gpui` owned by `gpui-box`. This
   proves registry-only resolution; it does not publish.
4. Create and push the annotated immutable `v<version>` tag on that exact
   verified commit. The workflow and publisher both refuse an untagged commit;
   do not move the tag after this point.
5. Run the manual `release.yml` workflow with `execute=false`. It performs the
   full preflight, runs the applicable native-platform semantic-version checks,
   and uploads the exact archives and reproducible CycloneDX 1.5 SBOMs without
   obtaining a crates.io credential. Offline registry consumers are the
   publication proof; a per-crate dry-run cannot resolve not-yet-published
   cohort dependencies. The workflow validates the exact tag on all three
   native targets, checks all three renderer-specific headless baselines, runs
   the full macOS gate, and runs the WASM build, Chromium smoke, and browser
   visual baseline. Neither publisher job can start unless every one of those
   jobs and the SemVer matrix succeeds. Preflight peels the annotated tag once;
   every downstream checkout and the archive artifact name are bound to that
   exact commit SHA, so moving the tag later can only fail publication, never
   switch the commit being validated.

## Publish

Only the protected release workflow may execute publication:

```bash
GPUI_BOX_PUBLISH=1 cargo run -p xtask -- package publish --execute
```

It refuses other arguments, a missing opt-in, a dirty worktree, a HEAD not
pointed to by an annotated `v<authority version>` tag, or missing `package
check` archives. Before each upload it independently regenerates that package
and requires its SHA-256 to equal the downloaded preflight archive. It checks
crates.io download bytes and the unyanked sparse-index entry before and after
each publish, and resumes only when the bytes equal that same archive and the
exact version is visible for dependency resolution. Cargo's `--no-verify` is
used only because the workflow already ran the complete offline `package
check`.

Publish each package in the exact `package plan` order. After each publish,
wait until the new crate/version is visible in the crates.io
index before publishing a dependent. Index propagation is asynchronous:
poll with bounded retries and backoff, and retry only the dependent publish or
index lookup. A response saying that the exact version already exists is
success after verifying its checksum/metadata; never attempt to overwrite it.

Pause on any other error. Determine whether crates.io accepted the upload
before retrying. Publication is immutable and a partially published cohort is
better documented and resumed than guessed at.

## Post-publication acceptance

From a clean environment with no workspace patches, Git sources, or path
dependencies, create consumers using only crates.io:

```toml
[dependencies]
gpui = { package = "gpui-box", version = "=0.1.0" }
gpui_kit = { package = "gpui-box-kit", version = "=0.1.0" }
```

Build the framework-only and framework-plus-kit smoke workspaces from the
registry on each claimed target. Install and start
`gpui-box-mcp --version` against a checkout and require its output to report
`gpui-box-mcp 0.1.0`. Archive commands and results.
Only after these pass:

1. create the GitHub release at <https://github.com/fran0220/gpui-box>, linking
   crates.io packages, compatibility/provenance records, and platform evidence.

Never move, delete, or reuse a release tag, and never overwrite a version.

## External crates.io setup

Before the first release, verify ownership for every publishable package.
The first publication of each crate requires a long-lived, least-privilege
token in the protected `crates-io` environment: crates.io cannot configure
trusted publishing until the crate exists. Later releases may use the official
OIDC trusted-publisher action. Require protected branch/tag rules, environment
approval, and auditable owners. These are crates.io/GitHub settings outside
this repository; their presence must be checked, not assumed from this
document.

There is no crates.io baseline for the first version, so the release workflow
explicitly skips semantic-version comparison only for `0.1.0`. Every later
release runs pinned `cargo-semver-checks` against the latest applicable
published crates.io baseline on Linux, macOS, and Windows before either
publisher job can start. Workspace selection checks every publishable
library-like package while excluding examples, galleries, and other
`publish = false` packages; the MCP binary remains covered by its packaged
install, `--help`, and `--version` acceptance tests rather than a Rust library
API comparison.

## Failure, yank, and recovery

Cargo releases cannot be rolled back. If a published package is unusable:

- stop the cohort and document exactly which versions were published;
- yank only affected versions when continued selection would harm users;
- do not delete or move a tag and do not unyank merely to reuse a version;
- fix forward with a new patch version for the whole affected cohort, rerun all
  gates and registry-only smoke tests, then publish by a newly generated plan;
- explain the yank and replacement in the changelog and GitHub release notes.

A yank prevents new resolution but does not remove source or break existing
lockfiles. Security incidents additionally follow the repository's disclosure
process; do not publish secrets in release logs.
