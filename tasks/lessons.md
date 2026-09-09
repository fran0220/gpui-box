# Lessons

- A visual-completion pass must re-check every added line against the
  borderless contract. Do not use a hard rule to attach a group label or mark a
  frozen edge when placement, typography, surface, or a soft cast can express
  the relationship. Report local and CI macOS visual validation explicitly,
  rather than letting a Windows-only caveat make macOS coverage look absent.
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
- GPUI Box is the application substrate, not the older Kit-era component
  subset. A surface a downstream desktop app needs is in scope unless it
  invents a host fact, a locale, a transport, or OS chrome. Do not keep
  refusing charts, form shapes, or other application primitives because an
  earlier Kit coverage note called them another library.
- After every push to `main`, require the BWG deployment workflow (or a manual
  `tools/site/deploy.sh`) to publish that exact commit, then check
  `/build-info.json` and the remote MCP counts. crates.io `gpui-box-mcp` is the
  last published cohort, not the live catalog. Do not treat a successful Git
  push as a catalog update.

## 2026-08-12: CI cache cleanup must also invalidate polluted restore keys

- Deleting transient package proof trees before cache save does not repair an
  older cache that already contains them: rust-cache can emit annotations while
  restoring that cache before the check starts. Give the corrected lifecycle a
  fresh shared key in every workflow that runs the proof.
- A package-cohort patch list should contain only the package's transitive local
  dependencies. Passing every cohort member to every `cargo package` succeeds,
  but floods the proof log with truthful "patch was not used" warnings.

## 2026-08-27: verify component capabilities before describing them

- Read the public builder and generated API before listing a component's
  capabilities. Do not infer an orientation or mode from the component name;
  `Slider` is horizontal and RTL-aware, but has no vertical API.

## 2026-08-27: an internal catalog cannot reveal absent component families

- A component-ecosystem audit needs a dated, authoritative external catalog
  mapped item by item into strong equivalents, composition/partial support,
  real gaps, and deliberate exclusions. Searching only the repository can
  verify what exists, but cannot establish that missing families such as
  `Transfer`, `TreeSelect`, or `Tour` were considered.

## 2026-08-27: benchmark choice follows the user's library preference

- When comparing a component ecosystem, use the library the user names as the
  benchmark. Material UI and shadcn/ui are both valid open-source coverage
  baselines here; compare each catalog by behavior, keep their differing name
  granularity explicit, and never substitute an unrequested library.

## 2026-09-08: glass review includes magnified corners and text backdrops

- A whole-frame readability pass does not establish optical correctness. Inspect
  all four rounded corners at 4× and use high-frequency text behind refractive
  edges. Lock defects with pixel tests before accepting replacement baselines;
  do not hide false normals or sharp-source leakage by reducing highlights.
- A medial-axis regression must sample both sides of a rounded arc's centre,
  including small menu radii. Analytic SDF derivatives alone do not make an
  over-deep optical dome regular: its slope must vanish before the arc collapses.
  Exercise interleaved foreground batches as well as frame-tail glass; enforce
  shared material geometry at the common upload boundary, not one caller.
- A reusable GPU slot is not a cache identity. Retain owner generation and
  activation through asynchronous readback; unsubmitted surfaces have no value.
  Diagnose remaining image differences with probe inputs and atlas coordinates
  before treating them as the same defect or claiming byte-for-byte equality.
- Readable overlay text does not prove its material rendered. Exercise Pill
  token radii against actual small bounds and compare interior pixels to the
  aligned backdrop. Keep material attenuation inside the material rather than
  depending on same-layer primitive batching to define its optical order.

## 2026-09-09: text rules need a visual specimen that exercises them

- A passing catalog cannot establish CJK line-breaking behavior if its text
  never exercises punctuation at wrap boundaries. Add a narrow mixed-script
  specimen, verify the bundled fallback family directly, and inspect both
  themes. Keep existing exhibit states visible when adding the specimen.
