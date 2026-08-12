# Lessons

- Before describing platform support, check the current CI matrix and visual
  baselines rather than repeating an older README qualification. Distinguish
  supported platforms from the platform-specific validation mechanism and
  narrower accessibility limitations.
- Treat the user-selected public project name as authoritative before carrying
  it into package identities, repository metadata, or a broad migration. If
  branding changes mid-task, replace the superseded name across the entire
  release surface rather than preserving aliases to the discarded name.
- Before freezing an imported-framework receipt or public release, compare the
  selected source revision with every first-party consumer pin and every newer
  fork commit. A complete filtered import proves that no file was dropped from
  that revision; it does not prove that the revision includes capabilities
  added later and already required by a downstream project.
- Do not describe the frozen Zed provenance receipt as a two-repository
  development model. GPUI Box is the only development, build, test, and release
  authority; historical repository URLs and vendor refs are immutable evidence,
  while all project-owned framework, platform, media, Kit, and tooling code is
  developed in this repository over public registry dependencies.

## 2026-08-12: CI cache cleanup must also invalidate polluted restore keys

- Deleting transient package proof trees before cache save does not repair an
  older cache that already contains them: rust-cache can emit annotations while
  restoring that cache before the check starts. Give the corrected lifecycle a
  fresh shared key in every workflow that runs the proof.
- A package-cohort patch list should contain only the package's transitive local
  dependencies. Passing every cohort member to every `cargo package` succeeds,
  but floods the proof log with truthful "patch was not used" warnings.
