# Native Liquid Glass references (independently authored)

This directory owns native acquisition, GPUI candidate production, evidence
validation and static diagnostic calibration. Native capture has run on
macOS 27.0 build 26A5416b with Xcode 26.2; artifacts and their exact source hashes
are separate from this code. A selected static trial does not establish optical
or dynamic equivalence. Portable synthetic-image tests do not establish native
behavior.

The fixture uses public SwiftUI `glassEffect`, `.regular`, `.clear`, `.tint`,
`.interactive()`, `GlassEffectContainer`, and `glassEffectID`. It is original
code, not an Apple sample source port. API authority:

- <https://developer.apple.com/documentation/swiftui/applying-liquid-glass-to-custom-views>
- <https://developer.apple.com/documentation/swiftui/glass/clear>
- <https://developer.apple.com/documentation/swiftui/glasseffectcontainer>
- <https://developer.apple.com/documentation/screencapturekit/scscreenshotmanager>

## Run

On a logged-in macOS 26+ desktop with full Xcode 26+ selected and its license
accepted, from the checkout root:

```sh
python3 -B -m unittest discover -s tools/liquid-glass-reference -p 'test_*.py'
python3 -B tools/liquid-glass-reference/run.py --output /tmp/glass-reference-001
python3 -B tools/liquid-glass-reference/validate.py /tmp/glass-reference-001
open /tmp/glass-reference-001/light-static-0.png
open /tmp/glass-reference-001/dark-static-0.png
```

Choose a **new output directory** each time. Existing directories are refused,
so stale successful captures cannot mask a failed run. Build is limited to 180s,
native execution to 90s, other commands to 30s each; the workflow has a 10 minute
limit. No third-party dependencies, Rust workspace changes, private APIs, TCC
database manipulation, or screen-permission bypasses are involved.

Screen Recording permission must already be granted to the generated
`GlassReference.app` (or the responsible terminal host, as macOS determines).
On a permission failure, add the generated app under System Settings → Privacy
& Security → Screen & System Audio Recording, enable access, then rerun using
a fresh directory. Ad-hoc signing or a changed source can require reapproval.
There is no unbounded permission prompt. macOS runner availability does **not**
guarantee a usable logged-in WindowServer or Screen Recording permission.
The dispatch-only `glass-reference.yml` uses `macos-26` and always uploads the
output directory; missing permission, SDK, window, or content is a failing job,
not an alternative rendering path. Select Xcode externally using
`DEVELOPER_DIR` if the machine default is too old. No workflow is dispatched by
this harness.

## Exact fixture contract

`fixture.json` is shipped in every artifact and SHA-256 identified. Version 1
is fixed, not a general configurable scene loader: update Swift and JSON
together for geometry changes. Coordinates are **top-left logical points** on a
960 × 640 borderless active window. Bounds are intended material shape bounds,
not optical outsets or measured intermediate morph contours. No title bar,
window shadow, cursor, or automatic scaling/cropping is included. Only native
1× or 2× capture scale is accepted; pixel sizes are 960×640 or 1920×1280.

Every appearance has the same fixture:

| Case | Rectangles `(x,y,width,height)` | Material / shape |
|---|---|---|
| regular-small | `(64,80,144,40)` | regular capsule |
| clear-small | `(304,80,144,40)` | clear capsule |
| regular-large | `(64,160,208,72)` | regular capsule |
| clear-large | `(304,160,208,72)` | clear capsule |
| tinted | `(592,80,208,56)` | regular, system orange tint, interactive capsule |
| fused-near | `(64,304,128,72)`, `(204,304,128,72)` | regular rounded rect r20; gap12, container spacing32 |
| fused-far | `(448,304,128,72)`, `(640,304,128,72)` | regular rounded rect r20; gap64, separate container spacing32 |
| button → menu | `(64,448,144,48)` → `(64,448,272,128)` | regular interactive rounded rect r24; persistent identity `menu`, container spacing32 |

The background is a 32pt checkerboard (`#D9E6F2` / `#F2CC99`, top-left first),
with 1pt `#8090A0` strokes on each cell (shared edges are stroked by both cells),
black system-monospaced 12pt text `Glass reference 012345` centered at
`(480,608)`, and solid RGB orientation markers from the JSON. SwiftUI `Canvas`
draws **only this background**, never the glass. Clear pills have a black 30%
capsule beneath the material and white foreground, as Apple's clear-material
guidance recommends; do not compare them as an untreated clear material.
Other foreground is system primary; pill labels use system 15pt semibold,
other labels use SwiftUI's default body font. The menu is a custom three-label
VStack (Copy, Share, Delete; leading alignment, 12pt spacing), **not NSMenu**.
System orange and system typography are intentionally OS-owned; OS build and
SDK are part of the reference identity, not portable numeric color/font claims.

Light and dark are explicit per-window Aqua/Dark Aqua overrides, not changes to
the user's global appearance. Reduced motion/transparency, increased contrast,
inverted colors, and differentiate-without-color must all be off; their actual
values are recorded and unsupported settings cause a diagnostic.

## What is captured, and what is not

For each appearance: one background-only control, one settled static fixture,
and 16 transition samples (36 PNGs total). The transition requests a linear
0.8s animation, with an 80ms sleep **after** each capture and encoding. Actual
spacing is variable; never infer 12.5fps from frame indices. Each frame records
system-uptime capture request/completion brackets, wall-clock completion,
pixel size, and transition trigger uptime. These are **not** compositor
presentation timestamps, and timing uncertainty is the bracket width.
At least three captures must complete inside the requested 0.8s animation and
one must begin after it; slow acquisition fails instead of pretending to be a
useful time series. This is a sampled screenshot series, not a display-rate
motion recording. SwiftUI chooses the actual glass morph behavior.

The glass-bearing view persists while its frame animates inside a fixed
top-left-aligned container. This is **native surface resizing**, not evidence of
cross-view matched-geometry morphing. The original two conditional glass views
on macOS 27 jumped immediately to the large shape and then revealed labels;
those failed captures are diagnostics, not successful motion references.
Foreground is clipped before applying glass so outgoing/incoming labels stay
inside the current animated shape; the glass and its optical outsets are not
clipped there. Endpoint geometry and fixture version 1 remain unchanged; use
the source hash as well as the fixture hash to distinguish transition semantics.

The app runs an actual AppKit event loop and hosted SwiftUI window.
`ScreenCaptureKit.SCScreenshotManager` captures that WindowServer window via a
desktop-independent window filter. `NSBitmapImageRep` encodes its returned
CGImage; a CGContext converts **that same captured image** to sRGB RGB PPM for
stdlib validation. Neither is a replacement view renderer. There is no
ImageRenderer, CPU glass approximation, or headless substitute.

The 36-frame manifest changes SwiftUI state programmatically and contains no
pointer input. A **separate 40-frame experiment** then posts synthetic native
`NSEvent` mouse-down/up events through `NSApplication.postEvent` at the Actions
label center `(136,472)` in top-left fixture points. It uses AppKit hit testing
and the SwiftUI Button handler, never invokes the action directly, and does not
move the system cursor or require/change Accessibility permissions. It is not
physical hardware input or a WindowServer/HID injection test. No hover/drag or
interactive material deformation is claimed: held-button text dimming and the
release-triggered native surface resize are the observed response.

Each appearance has one before frame, three held frames (80ms sleeps before
capture), and sixteen release frames (40ms sleeps before capture). `pointer.json`
records absolute event/dispatch/window-receipt/action-acknowledgement uptimes,
plus `clock_origin_uptime` to align them with relative capture brackets. The
native app rejects action before release, missing mouse receipts, or anything
other than exactly one action. Pointer evidence has independent frame hashes
and baseline/source provenance; the 36-frame validator and GPUI pairing tool
do **not** validate or pair these extra frames. `validate_pointer.py` independently
checks their hashes, dimensions, timing and receipt/action ordering; visual
review and GPUI input pairing remain separate.

The dedicated `buttons/` experiment isolates `.buttonStyle(.glass)` and
`.glassProminent` from a custom outer interactive glass surface. Its distinct
86-frame schema records acknowledgements without app-driven shape changes.
Do not substitute it for the resize fixture or mix its frames into fixture v1.
`color-resolution/` preserves a separate public `Color.orange` resolution on
the captured OS: encoded sRGB #FF8D28 in light and #FF9230 in dark, not #FF9500.
The GPUI fixture uses these measured input colours and the observed white tint
label; this is not a fitted output colour, an in-window measurement of run-005,
or a universal system-colour promise.

## Artifacts and inspection

- `manifest.json`: written only after all capture prerequisites pass; contains
  OS version/build, Xcode/SDK, architecture, source/fixture/executable identities,
  settings, scale, frame paths/hashes, and timing. Source and fixture copies
  accompany it. `provenance.json` is also retained before app execution.
- `*.png`: original compositor images. `*.ppm`: sRGB comparison sidecars.
- `native.json`: raw app report; `candidate.json`: enriched pre-validation
  report. Neither alone is a successful reference.
- `pointer-native.json`: separate raw native input report; `pointer.json` adds
  source/baseline identity and frame hashes. `pointer-failure.json` retains an
  input-acknowledgement failure when available.
- `validation.json`: prerequisite result, explicitly requiring visual review
  and stating calibration was not performed.
- `commands.log`, `diagnostic.log`, `failure.json`: build/runtime/validation
  diagnostics; partial frames are diagnostic-only when no manifest exists.
- `GlassReference.app`: exact ad-hoc-signed executable for investigation.

The validator rejects missing/duplicate frames, unknown backend/provenance,
source/hash/dimension mismatches, nonfinite/out-of-order timing, wrong markers,
an unchanged background in a non-text regular-glass strip, or an unchanging
transition ROI. PNG chunks/CRC and compressed data are checked. These are
content **sanity checks**, not proof of accurate optical behavior or automated
legibility/shape verification; no automated image test authenticates an
arbitrarily forged manifest. Preserve the workflow/run origin of artifacts.

Inspect both static PNGs at native scale for readable foreground, real material
distortion, clear-vs-regular differences, near fusion/far separation, and all
three corner fiducials. Step through each appearance's `transition-0.png` to
`transition-15.png` in **numeric** order and consult capture timestamps; reject
blank/flat glass, clipping, layout shifts, wrong appearance, or a jump cut.
This visual acceptance remains manual; passing the portable validator does
not waive it. No baseline is silently installed into GPUI snapshots.

## GPUI integration / calibration handoff

`compare.py` enforces pairing prerequisites without treating a match as a fit:

```sh
python3 -B tools/liquid-glass-reference/compare.py /tmp/glass-reference-001 /tmp/gpui-candidate
```

The GPUI producer writes `/tmp/gpui-candidate/candidate.json` with `schema: 1`,
`producer: "gpui"`, nonempty `revision`, `renderer`, `parameters_sha256`, the
native `fixture_sha256`, `logical_size`, `scale`, and a `frames` array. Each
frame has `appearance`, `phase`, `index` matching the native manifest and a
unique `rgb_file` basename plus `rgb_file_sha256` for an sRGB P6/255 sidecar.
Transition frames additionally have numeric `sample_time_after_trigger`, which
must fall **inside** the corresponding native capture bracket relative to its
trigger. All 36 frames are required, including background controls. This tool
revalidates native evidence, rejects missing/hash/size/provenance/time mismatches,
and emits explicit file pairs; it does not yet compute optical scores. GPUI
sample times should come from its actual rendering/simulated-time record, never
be copied from the native manifest to make a mismatched rendering pass.

1. Acquire and visually approve a native run, preserving its immutable artifact
   and manifest hash. Failed runs and synthetic tests are not training data.
2. GPUI reproduces this JSON geometry/background, scale, clear dimming treatment,
   appearance, and labels. Store implementation revision, renderer, shader and
   parameter identities separately; never stamp a GPUI rendering as Apple data.
3. Compare material interiors, edge profiles/optical outsets, near/far fusion
   bridges, tint, and foreground legibility separately. Exclude text when
   fitting material parameters because system fonts may differ. Use the clean
   background control to measure displacement and transmission. Fit on selected
   cases and hold out the other size/appearance to check generalization.
4. For temporal fitting, sample GPUI against each reference's measured
   trigger-relative capture bracket; do not match indices or assume fixed frame
   time. Reject mismatched fixture hashes, dimensions/scales, appearances,
   missing files or timing ambiguity exceeding the fit's declared tolerance.
5. Publish metrics, overlays/difference images, parameter changes, held-out
   failures and human review separately as **proposed fitting**, then validate
   GPUI's platform lanes. This directory supplies no invented optical constants,
   no threshold declaring native equivalence, and no automatic shader tuning.

The GPUI implementation owner owns those remaining implementation/calibration
steps. macOS compilation, native capture permission, content sanity checks,
and visual acceptance were exercised on the recorded Mac run. Each new source
or environment still needs native verification; Linux tests alone cannot provide it.

## Executable comparison and static trial selection

```sh
cargo build --locked --manifest-path tools/headless-visual/Cargo.toml --example glass_reference
python3 -B tools/liquid-glass-reference/gpui_capture.py REFERENCE CANDIDATE \
  --executable tools/headless-visual/target/debug/examples/glass_reference \
  --revision SOURCE_ID
python3 -B tools/liquid-glass-reference/metrics.py REFERENCE CANDIDATE
python3 -B tools/liquid-glass-reference/validate_pointer.py REFERENCE
python3 -B tools/liquid-glass-reference/buttons/analyze.py BUTTON_STYLE_REFERENCE
```

`gpui_capture.py --help` lists bounded production builder/theme trial settings.
`regular_*` scopes exclude Clear; omitted settings retain production defaults.
`protect_text` explicitly selects default body protection (1) or caller-owned
legibility (0). The supplied revision and current-source hashes are **not a
build attestation**: executable identity is hashed separately. Preserve the
exact checkout/build when recording a working-tree trial.

```sh
python3 -B tools/liquid-glass-reference/calibrate.py REFERENCE FRESH_OUTPUT \
  --candidates EXPLICIT_CANDIDATES.json --appearance light \
  --executable tools/headless-visual/target/debug/examples/glass_reference \
  --revision SOURCE_ID
```

The candidate file is a finite list of 1–32 distinct nonempty `regular_*`
parameter objects. Calibration records `protect_text=0`, since it measures
the material rather than the body's contrast policy. It writes a plan before
rendering four actual static/control frames per candidate. Only `regular-large`
in the selected appearance determines the objective: interior MAE plus 0.25 ×
rim MAE in encoded sRGB. The other size is held out, the other appearance is
reported as transfer, and Clear invariance is compared with candidate zero.
Each candidate's metrics and identities are retained; failure never reuses an
output directory. A chosen profile applies only to the selected appearance.
This score is diagnostic, not physical/perceptual truth or an acceptance
threshold. No production tokens or snapshots are changed automatically.

`metrics.py` separately reports checker contrast, background regression, rim
gradients, fusion-bridge residuals and masked transition morphology. These are
image-derived proxies; text masks are conservative geometry, not OCR. The
current GPUI producer explicitly retains the initial transition body and must
not be used to claim a dynamic fit until actual motion is implemented.

Portable checks run in the Linux gate; the real renderer example tests run in
`gate full`. Run them directly while iterating:

```sh
python3 -B -m unittest discover -s tools/liquid-glass-reference -p 'test_*.py'
python3 -B -m unittest discover -s tools/liquid-glass-reference/buttons -p 'test_*.py'
cargo test --locked --manifest-path tools/headless-visual/Cargo.toml \
  --example glass_reference -- --test-threads=1
```
