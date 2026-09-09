# Native agent adapters

The 24 agent catalog members below construct the actual Kit component. Arrays
normalize singular/plural builder conveniences; caller IDs remain unchanged.
Rendering never executes tools, approves requests, advances tasks, discovers
services, or supplies missing measurements. Closed schemas and generated TS
declarations are the wire contract, not an invitation to expose native handles.

| Component | Native surface exposed |
| --- | --- |
| AgentPlan | item states/reasons; pick; native done query |
| ThinkingBlock | present/withheld/absent reasoning, expanded, thinking, elapsed, inset/flow; toggle |
| FeedbackRating | vote/tags/current tag/disabled; vote and tag intents |
| PromptBuilder | label/body/slots, all five states, disabled; slot intent; empty visual slot |
| CostMeter | measured/estimated/unavailable lines, labels, last-verified markers |
| ContextGauge | used/known or unknown limit/stale/label; native nullable fraction query |
| AgentAvatar | full caller snapshot, size/parent/tint, resolved image source |
| AgentActivityLine | full execution-state union, tint |
| AgentCard | snapshot/selection/task label/tint/image; tagged native actions |
| AgentGroup | snapshots/appearances/max visible/size |
| AgentRoster | direct agents/tasks or from_run, appearances/selection/visible rows; actions |
| SubagentTree | run/expanded/selected/visible rows; toggle/actions |
| AgentRunIssues | direct typed model issues or native issues derived from caller run |
| ApprovalPrompt | details/standing scopes/status; native approve/decline/set_status/current_status |
| ClarificationPanel | candidates/detail/unavailable/multiple/skippable/status; native choose/answer/skip/set_status/current_status/candidates/chosen |
| ArtifactPreview | title/body/language/kind/all five states; empty/failed/loading slots |
| ToolCall | family/name/summary/arguments, explicit results/refusal/state/elapsed/expanded/presentation; toggle/retry; diff slot |
| ServerList | full server/catalog unions, offerings, expanded/selected/size/disabled; select/retry/toggle; empty/loading slots |
| OfferingCatalog | ready/stale/loading/empty/unavailable/error sources, query/kinds/selection/size/disabled; activate; empty/failed/loading slots |
| PermissionMatrix | actions/subjects/cells/inheritance; native change intent, never applied by renderer |
| VoiceReactive | caller sample/state/amplitude/envelope; exact sample time |
| PersonaPortrait | snapshot/expression/voice/image/tint/effect/sample time/size |
| PersonaDialogue | text or Markdown turn, streaming/choices, expression/voice/image/tint; choice and Markdown events; exact-match highlighting and resource image callbacks |
| AgentRunCanvas | complete caller run/layout/selection/positions/viewport/arrangeability; selection/position/viewport events |

Approval and clarification entities are retained by instance and semantic ID.
Status-only prop changes use native setters. Constructor-only changes replace
the native request; the native APIs have no setters for question/options/scopes.
Native intent methods do not change caller-owned approval/answer status.
Resolved controls and unoffered choices/scopes refuse commands; queries remain
available. Reconcile drops absent instance/ID entries and subscriptions. The
central host, not this map, must enforce generation/revision/mount/owner.

Images are closed resource references, never paths or URLs. Native image_source
builders preserve Custom loaders; resolution errors become Custom errors and
native fallback glyphs remain visible. Persona Markdown source strings are only
lookup keys. Native highlighting checks UTF-8 byte spans; invalid spans follow
native skip behavior. Shared content helpers own Markdown event serialization.

## Evidence and limits

`cargo test -p gpui-box-app-host --all-features kit_bindings::agent` exercises
every component's representative constructor, every declared native command/query, exact feedback/prompt/
plan/server retry/permission/dialogue intents, unavailable choices, disabled
feedback, retained entity identity and teardown, five fallback slot branches,
and seven independently counted native image-source forwarding paths.

`node --test tools/js-runtime/tests/kit-{agent,game-effects}.test.mjs` checks
fixture/schema parity, closed and exclusive unions, charge relations, data-only
rejection, generated declarations and TypeScript negative examples. Generate
fixture JSON/declarations with `KIT_WRITE_TYPES=1` using the same tests.

The real host fixture has request/evidence pages. Inspect both captures; the
event log starts at `No action requested`, not a fabricated service result.
Native renderer review and untrusted cinematic limitation are documented in
the adjacent game_effects fixture coverage. All Linux/mock tests are focused
evidence, not a substitute for the parent's integration/full gate or fresh
macOS/Windows validation. Event mappings not named above have constructor and
schema evidence but not individual pointer/keyboard regression coverage here.

## Excluded central integration hooks

Borrowed prerequisite is parent LOCAL 574b0f9, not origin/main. Borrowed icon,
Avatar image_source/fallback, content helpers, resource store, and central
oneOf/schemaType work are not owned delivery.

Parent must register both family COMPONENTS in admission; add agent::State to
KitState and call reconcile/render/invoke; route game_effects::render. Call
agent::validate and game_effects::validate after structural validation (the
latter currently also delegates image validation), before constructors. Merge
familySchemas/familyMethods into schemas.json/methods.json and JS kitSchemas/
kitMethods; call family validateProps after JS structural validation. Extend
central TS factories/method contracts with the family interfaces. Register
icon/content and resources plus the resource owner's base64 Cargo dependency.
Regenerate native API/developer indexes for the additive image_source builders.
No dotlottie feature hook is included in this delivery.

Local all-target/all-feature app-host `clippy -- -D warnings` remains blocked
by borrowed content/resource dead_code awaiting central registration. Those
diagnostics are not suppressed; owned adapter diagnostics were fixed. Native
Kit all-target/all-feature strict Clippy passes.
