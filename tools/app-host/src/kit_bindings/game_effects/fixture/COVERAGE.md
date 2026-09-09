# Native game, effects and motion boundary

| Catalog member | Implemented native surface |
| --- | --- |
| AbilityBar | AbilitySet and full Ability state/details/shortcut/cost/icon/charges, selection, Activate intent; unavailable/cooldown/disabled entries install no activation |
| ObjectiveTracker | ObjectiveSnapshot with parent/progress/all native states/selection, Select intent |
| PartyRoster | caller AgentSnapshot, expression/tint/resource image/gauges/selection, SelectMember intent |
| RewardReveal | full RewardSnapshot/items/quantity/icon/resource image/state/effect/sample time, RevealRequested/ClaimRequested intents |
| EffectParticles | complete caller-owned EffectPlan and exact sample time, actual native particle canvas |
| MicroMark | all five native Micro variants and caller label |
| CinematicEffect | native unavailable(error) fallback and sample time ONLY; untrusted clip assets remain unsupported |

No commands/queries exist on these native builder components. Props build
native models, not game rules. AbilityCharges requires maximum > 0 structurally
and current <= maximum relationally before calling the native constructor.
Tests cover equality, overflow by one, and zero maximum. Builtin icon selection
uses the shared exact key/weight grammar. Resource images never accept arbitrary
paths/URLs or perform component-owned IO. Reward image refusal renders a named
native warning glyph instead of silently inventing a replacement image.

`cargo test -p gpui-box-app-host --all-features kit_bindings::game_effects`
covers all seven native render branches, exact ability/objective/party/claim
actions, disabled/cooldown refusal and unchanged reward state after intent.
EffectParticles intentionally has no semantic node: it is a decorative canvas.
Cinematic fallback has native fallback semantics, not successful clip semantics.

Real renderer review runs separately (requires Linux headless renderer):

```sh
GPUI_FAMILY_REVIEW=/tmp/family-motion cargo test -p gpui-box-app-host \
  --all-features native_motion_and_reward_image_refusal_review -- --ignored
```

It runs reduced motion off with no sampleAt on live particles/MicroMark,
captures 40 native frames, asserts changed pixels and the missing-resource
warning semantic ID. The inspected video is sampled presentation evidence, not
a frame-rate benchmark. The separate JS host fixture uses reduced-motion
captures and explicitly labelled fixture data; those stills prove no motion.

## Why native clip bytes are refused before parsing

Correction to the initial investigation: a production `RasterDotLottieAdapter`
exists behind Kit's `dotlottie` feature. The missing primitive is **safe untrusted
preflight and enforceable work/cancellation budgeting**, not a decoder. Do not
enable the host feature and call prepare on JS-supplied bytes on the assumption
that resource byte quotas or isolated JS quotas protect synchronous native work.
The wire schema rejects clip/resource/bytes fields on CinematicEffect.

Local source `crates/gpui-kit/src/effects/dotlottie.rs`:

1. `prepare` validates limits and encoded size, calls `validate_archive`, then
   `Animation::from_dotlottie_bytes`, `validate_animation`,
   `validate_image_assets`, and `Renderer::prepare`, in that order.
2. `strict()` bounds encoded bytes to 2 MiB, entry bytes to 4 MiB, expanded bytes
   to 8 MiB, archive entries to 256 and compression ratio to 64. Archive headers
   are checked before reading entries; `.take(max_entry_bytes + 1)` and exact
   declared/actual size comparison bound each read. The archive is rebuilt with
   Stored entries before passing it to the decoder.
3. Canvas edge 1024, canvas pixels 1,048,576, fps 60, frame count 600 and duration
   10 seconds are checked **after** Animation parsing. Images receive source and
   target dimension checks before renderer preparation, but there is no
   shape/layer/keyframe/repeater/reference-graph work budget.
4. Raster clip rendering calls `prepared.render_frame(..., RenderConfig::default())`
   synchronously. Its four-frame image cache is not a computation/cancellation
   budget. JS execution quotas do not cover this call.

Authoritative rasterlottie 0.2.2, published revision
[`b0d4601`](https://github.com/neodyland/rasterlottie/commit/b0d4601285323975daeb21cad98b204f9455af74):

- [Model parsing](https://github.com/neodyland/rasterlottie/blob/b0d4601285323975daeb21cad98b204f9455af74/src/model.rs#L11-L34)
  delegates to archive loading and `serde_json::from_str`, without pre-parse
  complexity accounting.
- [Repeaters](https://github.com/neodyland/rasterlottie/blob/b0d4601285323975daeb21cad98b204f9455af74/src/render/drawing_paths.rs#L782-L827)
  allocate copies.ceil() indices without a cap, then repeatedly concatenate
  transforms up to each copy index/offset. A short scalar can cause quadratic
  work; byte and output-canvas limits do not bound it.
- [Procedural shapes](https://github.com/neodyland/rasterlottie/blob/b0d4601285323975daeb21cad98b204f9455af74/src/render/drawing_paths.rs#L494-L579)
  allocate polygon/star vertices from numeric point counts without a maximum.
- [Precomposition analysis](https://github.com/neodyland/rasterlottie/blob/b0d4601285323975daeb21cad98b204f9455af74/src/support/analyzer.rs#L355-L379)
  follows references recursively without a visited/depth/work budget; references
  can multiply work independently of JSON nesting.
- [RenderConfig](https://github.com/neodyland/rasterlottie/blob/b0d4601285323975daeb21cad98b204f9455af74/src/render/renderer_types.rs#L27-L35)
  has background and scale only, no fuel/deadline/cancellation contract.

These are source-level findings, not executed hostile-input experiments.
The complete target remains real resource-backed native clip construction and
playback. Reopening it requires framework-owned bounded parsing, graph-cycle/
depth checks, procedural-expansion/aggregate complexity limits, and enforced
prepare/frame work and memory budgets with effective cancellation, or killable
process isolation covering the entire operation. It is not complete today.
