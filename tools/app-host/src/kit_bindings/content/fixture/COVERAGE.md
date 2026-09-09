# Content and media native adapter audit

The catalog contains **11 content and 4 media components**. Membership is checked
against `docs/api-index.json` by the family JS tests. These are native component
builders, not pictures or catalog metadata. This delivery does **not** claim that
every arbitrary Rust callback can execute in a JS worker, or that a poster is
playback. Closed schemas and generated family declarations describe the supported
data bridge; absent fields are rejected.

| Component | Native bridge | Boundary |
| --- | --- | --- |
| Markdown | Actual caller source/parser; streaming; max lines; selection ordering; card/flat code; all MarkdownEvent variants; exact code text/language highlight spans; image resource map | Arbitrary block_renderer callback transport absent; arbitrary synchronous highlighter replaced by caller-classified spans |
| CodeView | Caller text or numbered/marked/spanned lines; language, line numbers, visible lines, copyable, empty slot; actual `text` query | Native copy action remains owner-policy gated, not a worker gesture bypass |
| AgentDocument | Typed blocks; revisions; streaming; notices; native states; virtualization; named typed-block and loading/empty/failed slots; declarative markdown options and events | Arbitrary per-block configure_markdown callback transport absent; non-text typed blocks require caller slot factories |
| DiffView | Caller files/hunks/context/add/remove/paired lines and numbers; old/new spans; notes; cursor; language; split/unified; wrapping/fills; native events; empty slot | No file reads |
| LogStream | Caller entries; ANSI; search ranges/current hit; timestamp/source/level/tone; selection; loading/empty/unavailable/error/stale/ready; native select/copy intents; declared slots | Copy intent does not itself perform clipboard IO |
| MessageList | Actual text/Markdown MessageBody; author/time/delivery/refusal; attachments/reactions; streaming; grouping/layout; retry and Markdown events | No attachment download or arbitrary Markdown callback transport |
| Outline | Caller marks and row identities; over/slots; native select event | No product navigation |
| BrowserPanel | Native address display, back/forward/reload intents, states and caller viewport/loading/empty/failed slots | URL is display text only; no WebView creation, navigation, or browser execution |
| ImageViewer | Caller frames, loading/unavailable/error/ready; dimensions, showing, fit/zoom bounds, height, disabled; frame factories or authorized image refs; all native events | No path/URL loader; resource resolver must authorize current mount |
| Terminal | Retained pure ANSI Emulator; append-only text feeds incremental suffix, including split ANSI; replacement resets; native grid/layout; selection/scroll state and events; retained error grid; slots | No process, PTY, executable, or live grid callback transport |
| TransportBar | Native caller state/time/buffer/volume/mute/speed/step/seekability/previous/next options; disabled; all native intents | Intent surface, not an actual playback transport |
| AudioWaveform | Bounded normalized caller peaks/playhead; states; empty slot | No decoding or audio access |
| AudioPlayer | Native title/subtitle/peaks/time/step/speed/disabled controls | Genuine native no-transport state; no transport or fake playback events |
| VideoPlayer | Native title/time/ratio/step/speed/disabled; caller poster slot or image resource | Genuine no-transport state; poster is not a decoded video frame |
| ModelViewer | Native bounded inline glTF parser; title, orbit, shading, height, disabled; native orbit/shading events | Native parser rejects external references; no network or file resources |

## Commands and queries

The family declares CodeView `query.text` and AgentDocument
`query.duplicate_ids`, `query.work`, `invoke.remeasure_block({block})`.
All use native APIs with bounded named JSON contracts. Remeasure rejects unknown
mounted block IDs. There are no fabricated media commands. Central invocation
validates result schemas; runtime must validate current generation, revision,
mounted identity, native disabled state, and EffectOwner before dispatch.

## Resource and lifetime requirements

The borrowed resources module uses closed `{key}` references and
`Resources::image(&ResourceRef, &App)`. ImageViewer uses its existing image factory,
so no framework/native setter was necessary. Native layout/paint rechecks the
resource lease. Tests use `ResourceStore::reconcile(&HashMap::from([(owner,owner)]),cx)`;
the host must map mount owners to generation principals and register bounded bytes
only through approved resource protocol. No descriptor path or URL enters native IO.

Family reconciliation releases mounted query targets and terminal emulators.
It does not prove immediate destruction of every framework keyed cache. Runtime
generation/mount namespacing and framework cache-release work are separate owners.

## Executed evidence

- `node --test tools/js-runtime/tests/kit-{content,media}.test.mjs`: 6 passed,
  including strict TypeScript negative cases, membership, closed grammar,
  duplicate IDs/lines, typed slots, ambiguous constructors and resource locators.
- `cargo test -p gpui-box-app-host --all-features kit_bindings::content`: 8 passed.
  Actual native roots, denied/granted owner clipboard, parser/work queries,
  remeasure, streaming, recursive fresh slots, terminal ANSI/selection retention
  and adapter teardown, missing/foreign/revoked resource authority.
- `cargo test -p gpui-box-app-host --all-features kit_bindings::media`: 2 passed.
  Native unavailable/no-transport and parser external-resource refusal.
- `cargo clippy -p gpui-box-app-host --tests --all-features -- -D warnings`: passed.
  All-target Clippy is blocked by borrowed resource host-install/registration APIs
  being unused in this branch's production entry point; parent owns wiring.
- `GPUI_RESOURCE_CAPTURE=<png>` with the content tests executes headless rendering,
  asserts exact asymmetric red/blue pixels, revokes the owner, asserts red pixels
  disappear, and writes `<png>.revoked.png`. Both images were visually inspected.
  `GPUI_CONTENT_CAPTURE` and `GPUI_MEDIA_CAPTURE` render representative native
  content and honest unavailable media states. These are native headless fixtures,
  not browser execution, hardware playback, or a live terminal session.

## Integration hooks (not in owned commits)

Declare content/media modules and State fields in central KitState; call their
reconcile/render/invoke dispatch; include COMPONENTS; call family
validate_descriptor after shared validation. Spread familySchemas/familyMethods
into central JS schema and call validateDescriptor after slot validation.
Regenerate central schemas.json/methods.json and aggregate family TS method
contracts. Resource owner supplies resources*.rs and resource-schema.mjs; main
needs `mod resources;`, app-host needs `base64.workspace = true`, and runtime must
install/manage the resource store. Temporary central diff is delivered separately.

The older central every_declared_method test uses an empty AgentDocument with
synthetic `block: "fixture"`; its fixture must mount that block before remeasure.
Rejecting that unknown block is intentional. Parent runs the full integrated gate.
