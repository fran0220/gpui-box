# Native SwiftUI glass button style experiment

This isolated, independently authored experiment captures public SwiftUI
`Button.buttonStyle(.glass)` and `.glassProminent` alongside a persistent custom
`.glassEffect(.regular.interactive())` surface. It does not modify the previous
960×640 fixture v1, its run-005 evidence, or GPUI code. This is native observation,
not GPUI calibration, a private material model, or universal optical constants.

## Reproduce

From the repository root (tests and analysis also run on Linux):

```sh
python3 -B -m unittest discover -s tools/liquid-glass-reference/buttons -p 'test_*.py'
python3 -B tools/liquid-glass-reference/buttons/analyze.py target/native-button-styles-evidence/run-002 > target/native-button-styles-evidence/run-002-hardened-measurements.json
```

New capture, only on an already screen-capture-authorized Mac with macOS/SDK26+:

```sh
python3 -B tools/liquid-glass-reference/buttons/run.py --output /tmp/native-buttons-new
python3 -B tools/liquid-glass-reference/buttons/analyze.py /tmp/native-buttons-new > /tmp/native-buttons-measurements.json
```

The output directory must be new. The app uses a real NSHostingView/WindowServer
window and ScreenCaptureKit desktop-independent screenshots. PNG/PPM encoding
converts those same captured CGImages; there is no alternate renderer. Capture
permission must already exist; no permission prompt, Accessibility changes,
global cursor movement or HID injection is performed. The native process exits
after capture. Build timeout is 180s and native execution timeout is 120s.

`fixture.json` is a descriptive fixed-fixture contract, not a dynamic loader.
Swift and JSON must change together when geometry/policy changes. The independent
schema names are `native-button-styles-fixture-1`,
`native-button-styles-evidence-1`, and `native-button-styles-measurements-1`.

## Fixture and input provenance

Canvas: 800×520 points, 32pt checkerboard and three RGB fiducials. Top-to-bottom:
glass centered (400,112), glassProminent (400,256), custom (400,400). Label frame
is 144×48 for the two system styles; custom surface is 144×48 with a centered
plain Button. All labels use system 15pt semibold. Control size is `.regular`,
border shape is the native default for system styles, custom radius is 24.
No tint override in the primary run; prominent uses the native blue accent on
this Mac. Appearance overrides are per-window, not global settings.

Each appearance captures one background, then each case captures before,
held0–3 and release0–8: 86 frames total. All three controls remain visible, but
only the named case receives input. Each button action records an acknowledgement
without changing label, shape, state, layout, or count on screen. This isolates
the style's native response from application-driven morphing.

Synthetic mouse-down/up NSEvents 101/102 are queued through
`NSApplication.postEvent` at the control's center, route through native hit
testing, and invoke its actual SwiftUI Button action. Receipt and action checks
reject wrong/missing events, actions before release, or anything other than one
action for the requested case. This is native application event input, not
physical hardware input, HID/WindowServer injection, hover, drag, or cross-view
matched-geometry morphing.

## Primary accepted capture: run-002 (default tint)

Mac: macOS27.0 build26A5416b; Xcode26.2 build17C52; SDK26.2; arm64 M4Pro;
main display3008×1692/logical1504×846. Scale2 captures are 1600×1040 pixels.
All five recorded accessibility display settings are false. Screen capture was
already authorized. No existing services, settings or checkout files changed.

### Native styles add layout insets and visibly expand while held

Measured SwiftUI layout bounds (top-left points):

| Case | Layout rectangle | Requested label/surface size |
|---|---|---|
| glass | [316,84,168,56] | 144×48 label |
| glassProminent | [316,228,168,56] | 144×48 label |
| custom | [328,376,144,48] | 144×48 surface |

Thus both system styles add 12pt on each horizontal side and 4pt on each vertical
side. These settled layout measurements are not the rendered material's animated
bounds; per-frame optical measurements below record that separately.

Measured visible footprints use the bounding box of pixels differing from the
same-appearance background by at least 4/255 in any sRGB channel. They include
optical/antialiased outsets, so they are not exact private shape geometry:

| Case | Before width×height | Held3 width×height | Settled release8 |
|---|---|---|---|
| both native styles, both appearances | 169×57 | 172×58 | 169×57 |
| custom, light | 145×49 | 145×49 | 145×49 |
| custom, dark | 145×48 | 145×48 | 145×48 |

Native styles expand symmetrically: glass x315.5→314, y83.5→83; prominent
x315.5→314, y227.5→227. Pixel precision is 0.5pt. Threshold 8/16 measurements
are also retained; weak dark edges change the inferred height by 0.5–1pt.
Do not use threshold boxes as exact optical contours or universal padding rules.

### Native styles change body optics; custom changes only the label

Mean signed RGB changes at held3, relative to before, in non-text interior strips:

| Case | Light ΔRGB | Dark ΔRGB |
|---|---|---|
| glass | [-18.91,-18.47,-18.96] | [+21.76,+21.64,+20.34] |
| glassProminent | [+4.35,-17.27,-19.94] | [+23.55,+21.89,+5.00] |
| custom | [0,0,0] | [0,0,0] |

Visual inspection confirms glass darkening in light and brightening in dark;
the prominent blue also darkens/brightens. Custom has no visible body deformation
or optical change under this input path; its label dims while held. The matched
interior strips return to their exact before means by release8 for every case.
Full settled .glass/custom frames are byte-identical to before. Prominent has
388 light/723 dark differing channel samples across the full image, at most2/255,
so byte-identical recovery is not claimed for it.

The three rows see different checker phases. Within-case background subtraction
and before/held comparisons are controlled; absolute cross-style optical fits
would need matched placement and additional backgrounds. This experiment proves
observable response, not a universal style-equivalence claim.

### Timing and verification

Four held samples span a ~357–377ms press. Capture brackets range 13.6–75.9ms
over the whole run. First release captures complete 36–41ms after dispatch;
release1 completes 136–142ms, when the threshold 4 footprint has usually recovered.
One light prominent frame at release3 shows a small 172→169→170pt width rebound;
all raw frames/times are retained. Final release samples complete 1.80–1.88s after
dispatch. These are screenshot request/completion brackets, NOT display
presentation timestamps or hardware latency. Six dispatch-to-action latencies
are 0.50–0.61ms; `clock_origin_uptime` aligns absolute events to frame brackets.

The independent author's native compile/sign/capture succeeded. Integration
revalidates its existing 86 frames without running a Mac app in the Linux orb.
`analyze.py` uses explicit errors (also with Python optimization), pins the exact
fixture bytes and recorded fixed layout, checks source/frame SHA-256, PNG chunk
CRC/decompression/dimensions and PPM payload/dimensions, RGB fiducials, safe unique
frame filenames with no symlinks, integer frame indices, exact ordered frame
identities, and six unique appearance/case runs. All capture/event clocks must be
finite nonnegative numbers, not booleans. Before frames precede down, held frames
follow down receipt and precede up, and release frames follow the sole action
after up receipt. Receipt locations and event timestamps must match dispatches.
The fixture hash deliberately rejects even descriptive edits until this fixed
contract is explicitly revised. No new manifest fields were added.

Eight tests include malformed-run/action/time/fixture/image/path regressions and
the original three asymmetric pixel tests. Synthetic test images are explicitly
validation fixtures, not native evidence. The PNG parser is loaded from the
parent `validate.py` by an explicit file-based import, independent of cwd.
PNG and PPM are checked independently, not decoded into a cross-format color
equivalence claim. Hashes establish recorded integrity, not trusted hardware
attestation. These checks do not replace visual review.

The independent author's visual review covered full light/dark fixtures and all 14 samples for each of
six case/appearance pairs. Native glass is visible, checkerboard remains fixed,
labels are readable, and all material shapes are unclipped. `review/*-all.png`
has 14 row-major tiles: before, held0..3, release0..8 (7×2). These cropped sheets
are review derivatives; originals remain the authoritative captures.

## Auxiliary run and immutable handoff

run-001 intentionally had an explicit `.tint(.blue)` applied at the root.
Both native styles then appeared blue/near-opaque. It is retained with its own
source, fixture hash, provenance and 86 frames as an auxiliary pilot, not silently
mixed into default-style evidence. The current source reproduces run-002.

Original source handoff: `target/native-button-styles-source/`. Original source,
fixture, runner and frames remain unchanged in
`target/native-button-styles-evidence/run-002/`. Integrated `Buttons.swift`,
`fixture.json` and `run.py` are byte-identical copies; only analysis/tests/docs
were hardened. No evidence binaries or archives belong in this source directory.

`measurements.json` describes run-002; exact manifest SHA is embedded. Native
manifests include source/fixture/executable and frame hashes. Partial raw reports
are updated after each screenshot so failures preserve available diagnostics.
The independently rerun output is `run-002-hardened-measurements.json` beside the
original evidence directory. This unit does not calibrate GPUI or claim Apple
equivalence, private contours, physical HID input, or custom-surface deformation.
