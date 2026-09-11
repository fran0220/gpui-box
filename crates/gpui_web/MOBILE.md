# Mobile browser contract and validation

The browser backend routes every `pointerType="touch"` contact through GPUI's
`TouchEvent`, preserving contact identity through release or cancellation. It
does not also synthesize mouse or native pinch events for those contacts. The
framework owns tap, pan, long press, pinch, and momentum policy. Pointer capture
keeps a contact routed to the canvas; capture loss and `pointercancel` cancel
only that contact. Window blur, document hiding, and pagehide cancel outstanding
contacts and report inactive so the framework also cancels released momentum.
The canvas owns touch gestures (`touch-action: none`); surrounding DOM retains
its browser gesture behavior. This is not native browser pinch zoom on canvas.

## Geometry

CSS content-box dimensions are authoritative logical layout dimensions.
DevicePixelContentBox measurements provide physical backing dimensions when
they agree with CSS × DPR within one device pixel (edge rounding). Otherwise
the backing size uses rounded CSS × DPR. The rule does not inspect user agent,
automation, or device model. Texture-limit clamping remains explicit.

CDP-only DPR emulation can report the host's physical box with an emulated DPR.
It cannot establish physical-device correctness. The mobile tests exercise a
contradictory observation and a valid one-pixel rounding observation separately;
a native Chromium `--force-device-scale-factor=2` control disables viewport/DPR
emulation and checks CSS 920, backing 1840, and DPR 2.

Hosts opt into edge-to-edge layout using `viewport-fit=cover`. The backend reads
CSS `env(safe-area-inset-*)` and exposes residual canvas-relative logical edges
through `WindowInsets`. It observes window and VisualViewport resize/scroll,
not just canvas resizing. With its input focused, an unzoomed VisualViewport
reports top/bottom occlusion relative to the canvas. A keyboard that resizes
the layout viewport therefore does not also add the same inset. Pinch zoom is
not treated as IME. VisualViewport cannot prove keyboard identity; floating
keyboards and arbitrary interior occlusion cannot be represented by edge
insets and are not claimed. The gallery consumes `insets().effective()`.

## Editing

Touch tap focuses an editable handler without opening a keyboard for ordinary
controls. The hidden native input has no startup autofocus, uses 16px font to
avoid small-input focus zoom, and follows the GPUI IME bounds in CSS coordinates.
Focus uses `preventScroll`. Core focus hooks may hold a Window borrow: native
focus changes happen immediately, but handler option queries defer to a
microtask to avoid reentering that borrow.

Keyboard purpose, action and autofill hints map to `inputmode`, `enterkeyhint`,
and `autocomplete`; secure input uses `type=password`. Browser/OS policy may
ignore hints or refuse keyboard presentation without current user activation.
Return actions call the caller-owned handler before ordinary Enter fallback.
Keyless `input` handles soft-keyboard/dictation insertion; cancellable
`beforeinput` handles deletion and line-break actions. Desktop keydown/paste
already prevent native edits, avoiding duplicate insertion. Composition owns
its keys and commits via compositionend; `insertFromComposition` is not inserted
again. Blur/background clear the transient DOM buffer and composition state.
This small native input is an IME bridge, not a full surrounding-text mirror:
contextual autocorrect, native selection handles, password-manager behavior,
multiline OS editing, and SMS autofill require further native-device evidence.

## Executable browser evidence

```sh
cargo run -p xtask -- web build
npm --prefix examples/browser-gallery run mobile
```

`mobile.spec.mjs` uses Chromium WebGL2 with touch emulation and verifies coarse
pointer mode. CDP touch and text/IME events exercise backend delivery; explicit
InputEvent and PageTransitionEvent fixtures exercise deletion and lifecycle
boundaries. Safe-area values use Chromium's CSS environment override;
VisualViewport keyboard geometry is explicitly a fixture, not an OS keyboard.
The suite covers portrait/landscape, a focused form, composition, pan vs tap,
cancel/recovery, asymmetric insets and zoom exclusion, and DPR consistency.
Representative screenshots are inspected, not accepted as behavioral tests.

This is **not native Android Chrome or iOS Safari acceptance**. Before making
those claims, run on devices: keyboard show/hide under user activation, keyboard
resize modes and rotation, notched safe areas, candidate composition and
deletion with platform IMEs, autofill/OTP and secure fields, multitouch/pinch,
screen reader focus, app switching, back-forward cache restore, and GPU context
loss/recovery. Linux WebKit emulation is also not iOS Safari device evidence.

## Provenance and shared-document integration notes

Original GPUI Box changes; no imported source, new third-party library, or
license change. The historical framework import receipt remains frozen. Parent
integration should add this mobile browser boundary and evidence limitation to
`PROVENANCE.md`, `THIRD_PARTY_NOTICES`, and `compatibility.toml`/coverage prose,
and regenerate catalogs after integrating the coordinated core contracts.

Authoritative specifications consulted (behavior guidance, not copied code):

- https://developer.mozilla.org/en-US/docs/Web/API/Pointer_events
- https://developer.mozilla.org/en-US/docs/Web/API/Element/pointercancel_event
- https://developer.mozilla.org/en-US/docs/Web/API/VisualViewport
- https://developer.mozilla.org/en-US/docs/Web/API/Element/beforeinput_event
- https://developer.mozilla.org/en-US/docs/Web/API/Document/visibilitychange_event
