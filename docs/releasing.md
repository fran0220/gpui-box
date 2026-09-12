# Releasing GPUI Box

This is the runbook for GPUI Box source releases and crates.io publication.
Source releases do not imply that a version exists on crates.io.

## Git-only 0.2.0 release

Version 0.2.0 ships as an immutable annotated Git tag and GitHub Release only.
Do not dispatch the registry publishing workflow, upload crates, or change
registry credentials or access controls for this release.

Run the Linux full gate, the macOS/Windows Platforms matrix, the offline
historical-source audit, and the package check. Record their exact source
revisions and results. Documentation-only release metadata changes must be
identified separately from the native implementation that was validated.
Verify a downstream consumer outside this workspace using Git dependencies:
all GPUI Box dependencies and the root `block` patch must use the same full
revision. Require exactly one `lib gpui`, no GPUI Box registry/path sources,
and a successful locked build. Cargo does not inherit dependency-workspace
patches; use the complete example in the root README.

Commit the release records, create and push the annotated `v0.2.0` tag, and
never move it. The GitHub Release must include the full commit SHA, copyable
`rev`-pinned dependencies, validation evidence, migration notes, and platform
limitations. State explicitly that no 0.2.0 packages were uploaded to crates.io.
Consumers commit their lockfile and use `--locked`, not a moving branch.

The remaining sections govern a separately authorized crates.io publication;
their registry dry-run and post-publication requirements do not describe the
Git-only release.

## Cohort and authority

`package-authority.toml` defines every package name, version, cohort, license,
and publish flag. Framework packages are Apache-2.0; kit packages and
`gpui-box-mcp` are MIT. A release keeps mutually dependent publishable packages
in one compatible version cohort (currently `0.2.x`) and records the contract
in `compatibility.toml` and `provenance.toml`.

Never hand-maintain publication order. Derive it from Cargo metadata:

```bash
cargo run -p xtask -- dependencies check
cargo run -p xtask -- package plan
```

Save the plan in the release log. Independent packages may appear early, and
platform packages may appear after consumers; the generated order, not a prose
list, is authoritative.

### Experimental mobile packaging

The `0.2.0` cohort includes the real `gpui-box-android` and `gpui-box-ios`
adapters as Apache-2.0 framework packages (28 publishable packages total).
Cargo resolves even target-specific optional registry dependencies, so the
public mobile features of `gpui-box-platform` require both adapters in the
publication graph, before the platform package, not in its external list.
Publication does not promote mobile acceptance: both remain experimental,
with the native acceptance limitations recorded in `compatibility.toml`.
Android `host-check` and iOS `platform-check` prove host type checking only,
not APK/Xcode linking or device execution.

iOS's Kit and mobile-reference integration-test dependencies stay path-only:
Cargo omits them from the published manifest while retaining local tests.
A versioned Kit dev-dependency would create an
`ios -> kit -> platform -> ios` publication cycle. The package plan rejects
published local dependencies outside the publishable authority rather than
silently classifying an unpublished adapter as an external registry crate.

For the first `0.2.0` publication only, the native SemVer jobs exclude
these two new names and `gpui-box-webview`, which also has no published version:
cargo-semver-checks 0.50.0 errors on a missing registry
baseline instead of skipping it. Existing packages remain checked. Later
versions include all three adapters automatically. Their first upload requires the
protected bootstrap-token path; an OIDC publisher cannot be configured for a
crate name that does not yet exist. Verify credentials before starting a cohort.

## Preflight and dry run

1. Select the release commit; ensure versions and internal requirements form
   the intended cohort, changelog has the release date, machine-readable
   records are current, and the worktree is clean. For an initial release,
   confirm every package name is available; for later releases, confirm the
   expected crates.io owner set. `scripts/sync-zed/sync-zed verify` must prove
   the complete frozen official and fork-overlay history receipts, both
   canonical vendor refs, and their ancestry through this commit entirely
   offline; an incomplete receipt cannot be released.
2. Run the platform validation required by `compatibility.toml`: `gate full`
   on the orb for the authority lane, and one dispatched `Platforms` run of
   the release commit for macOS and Windows (`gh workflow run platforms.yml
   --ref main`). Record the actual run URLs/results; do not infer a platform
   result from another platform.
3. Run the complete local package gate:

   ```bash
   cargo run -p xtask -- gate full
   cargo run -p xtask -- package plan
   cargo run -p xtask -- package check
   ```

   `package check` requires `cargo-local-registry` 0.2.12. It packages every
   publishable crate, constructs a registry under `target/package-check`, then
   builds framework-only and framework-plus-kit consumers offline with source
   replacement and without GPUI Box path patches. The workspace's sole Cargo
   patch is the separately receipted `block` 0.1.6 compatibility fork; the gate
   packages that source into the temporary registry explicitly, so its result
   does not depend on an existing developer Cargo cache. A successful run keeps
   only the authoritative archives under `target/package-check`; failed runs
   retain the complete temporary registry and consumers for diagnosis. It also
   runs a packaged `gpui::property_test`, installs the MCP binary and checks its
   help/version, rejects retained internal dev-dependencies that would deadlock
   a first publication, and requires exactly one `lib gpui` owned by
   `gpui-box`. This proves registry-only resolution; it does not publish.
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
   the Linux authority gate and rustdoc, and runs the WASM build, Chromium
   smoke, and browser visual baseline. The macOS and Windows native jobs add
   their platform-specific all-feature warning proof, while macOS also runs
   platform tests, without rerunning the Linux gate;
   the headless jobs do not repeat the authority checks. Windows builds the
   visual executable once and fans its stable catalog out over eight WARP
   shards, with the required platform check aggregating all eight. Neither
   publisher job can start unless every one of those jobs and the SemVer matrix
   succeeds.
   Preflight peels the annotated tag once;
   every downstream checkout and the archive artifact name are bound to that
   exact commit SHA, so moving the tag later can only fail publication, never
   switch the commit being validated.

## Publish

Only the protected release workflow, dispatched from `main`, may execute
publication:

```bash
GPUI_BOX_PUBLISH=1 cargo run -p xtask -- package publish --execute
```

For `0.2.0`, select **`auth=mixed`**, `execute=true`, and
`bootstrap_trusted_publishing=false`. Existing crates require OIDC because
they already enforce trusted-publishing-only; `gpui-box-android`,
`gpui-box-ios`, and `gpui-box-webview` need their first-ever publication using
the least-privilege `CRATES_IO_TOKEN` secret in the protected `crates-io`
environment. Do not disable or change existing registry access controls.
The mixed publishing job obtains both credentials: the official action's
short-lived token is `CARGO_REGISTRY_TOKEN`, while the protected secret is
`GPUI_BOX_BOOTSTRAP_TOKEN`, enabled only by `GPUI_BOX_PUBLISH_AUTH=mixed`.

Before any upload, mixed mode checks every name using
`GET https://crates.io/api/v1/crates/<name>` and preflights OIDC plus the
bootstrap credential if any name is new. Only a validated crate-not-found
404 selects bootstrap; malformed responses, mismatched identities, and other
HTTP or transport errors stop publication. A missing **target version** never
makes an existing name eligible for bootstrap. Classification is repeated
before every upload attempt, including retries; existing names always receive
OIDC, never a token fallback. The normal exact-version/checksum/index resume
checks still apply. On a partial-cohort rerun, newly created names now require
OIDC for any further upload; configure their trusted publishers separately
before trying to publish another version.

Pure `auth=token` (all-token initial cohorts) and `auth=oidc` (all-existing
cohorts) remain available unchanged. Neither alone can publish a mixed cohort
of trusted-only existing names and first-ever names. Mixed publication does
not configure publishers, harden crates, or invoke the separate
`bootstrap_trusted_publishing` operation.

It refuses other arguments, a missing opt-in, an unprotected workflow ref, a
dirty release worktree, a release HEAD not pointed to by an annotated
`v<authority version>` tag, or missing `package check` archives. The publisher
tooling comes from the exact protected `main` commit that defines the workflow;
it operates on a separate release-tag checkout, so a publisher fix never
changes or moves immutable release source. Before each upload it independently
regenerates that package from the release checkout and requires its SHA-256 to
equal the downloaded preflight archive. Reproduction and `cargo publish` receive
the same generated cohort source overrides so Cargo's internal repackaging
cannot resolve a different dependency graph. The exact-version crates.io API
is the authority for whether a version exists; when it does, the publisher also
checks the download bytes and unyanked sparse-index entry. It resumes only when
those bytes equal the same preflight archive and the exact version is visible
for dependency resolution. Cargo's `--no-verify` is used only because the
workflow already ran the complete offline `package check`.

Publish each package in the exact `package plan` order. After each publish,
wait until the new crate/version is visible in the crates.io
index before publishing a dependent. Index propagation is asynchronous:
poll with bounded retries and backoff, and retry only the dependent publish or
index lookup. A response saying that the exact version already exists is
success after verifying its checksum/metadata; never attempt to overwrite it.

Crates.io permits an initial burst of five brand-new crate names and then earns
one additional new-crate publish every ten minutes. The publisher recognizes
only that specific 429 response, waits ten minutes plus a small clock-skew
margin, and retries the same package a bounded number of times. Before every
retry it asks the exact-version API whether crates.io accepted the prior
request; an accepted version is resumed only after the archive checksum and
unyanked index entry match. Other 429 responses and all other upload failures
stop the cohort for inspection.

Pause on any other error. Determine whether crates.io accepted the upload
before retrying. Publication is immutable and a partially published cohort is
better documented and resumed than guessed at.

If publisher tooling fails before an upload, fix it on protected `main` and
rerun the workflow from `main` against the same immutable tag. Never copy the
fix into the tagged source or move the tag. The rerun repeats every release
gate, downloads newly generated preflight archives, reproduces them from the
tag checkout, and skips an already-published package only after its registry
metadata, archive checksum, and unyanked index entry all match.
The initial `v0.1.0` recovery is additionally pinned in both workflow preflight
and publisher code to commit
`888369c73c258567664785a761faebdc64d39d4e`; tag self-consistency alone is not
accepted as proof of that reviewed source.

## Post-publication acceptance

From a clean environment with no GPUI Box workspace patches, Git sources, or
path dependencies, create consumers using only crates.io:

```toml
[dependencies]
gpui = { package = "gpui-box", version = "=0.2.0" }
gpui_kit = { package = "gpui-box-kit", version = "=0.2.0" }
```

Build the framework-only and framework-plus-kit smoke workspaces from the
registry on each claimed target. Install and start
`gpui-box-mcp --version` against a checkout and require its output to report
`gpui-box-mcp 0.2.0`. Archive commands and results.
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

Immediately after the initial cohort is accepted, rerun `release.yml` from
protected `main` with `execute=false` and
`bootstrap_trusted_publishing=true`. That mode still requires every release
gate. It verifies that all authority packages and their exact versions exist,
that `fran0220` is an individual owner, and that each crate has either zero or
exactly one matching GitHub publisher. It refuses conflicting or duplicate
configurations because the crates.io create endpoint is not idempotent. It then
uses the official action to exchange the `release.yml` OIDC identity for a
short-lived token and enables trusted-publishing-only mode on every crate.
Scoped crates.io tokens cannot revoke themselves: both token-management
endpoints have no selectable endpoint scope and reject scoped API-token
authentication. After the workflow succeeds, revoke the bootstrap token from
crates.io **Account Settings → API Tokens** using the owner's browser session,
verify that it disappeared, then delete the now-invalid secret from the GitHub
environment and any external secret stores. Never report revocation from the
workflow's hardening result alone. Later publication runs use `auth=oidc`
unless the cohort adds new names, in which case use `auth=mixed` with a new
least-privilege bootstrap token. Token authentication exists only for
first-publication recovery; never relax trusted-only controls to reuse it for
existing crates.

There is no crates.io baseline for the first version, so the release workflow
explicitly skips semantic-version comparison only for `0.1.0`. Every later
release runs pinned `cargo-semver-checks` against the latest applicable
published crates.io baseline on Linux, macOS, and Windows before either
publisher job can start, with only the three first-time `0.2.0` adapters
excluded as described above. Workspace selection checks every publishable
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
