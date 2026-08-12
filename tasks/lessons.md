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
