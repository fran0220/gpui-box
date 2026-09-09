# Native Kit adapters

`COMPONENTS` is the native registration authority, not the generated developer
catalog. `schemas.json` is the data-only serialization of `kitSchemas` from
`tools/js-runtime/kit-schema.mjs`. JS tests compare its entire contents; Rust
tests compare its component keys with the native registration list.

The original **16 partial adapters** instantiate real Kit components: Checkbox, Radio,
Switch, Slider, SegmentedControl, TextInput, Select, Pagination, Tabs, Accordion,
ScrollArea, SplitPane, Divider, List, Popover, and Dialog. They are not full catalog coverage. Supported
props/events are exactly the schema fields and TypeScript `KitAPI` declarations;
unknown fields fail closed in both JS and native validation.

Central registration now includes **51 partial adapters**: those 16, the 11
`controls_extra` adapters, and 24 navigation/layout/datetime adapters. Every
family uses its own native render, retained state, reconcile, invoke, and
supplemental validation hooks through the real host. This is data-only adapter
integration, not full native binding coverage. The family contracts and limits
are documented in [the family receipt](navigation_extra/fixture/README.md).

The native entities for TextInput, Select, Popover, and Dialog persist by host-injected process
instance and semantic identity. Reconciliation drops subscriptions for removed
or replaced entities. Retained subscription routes update to the current frame
emitter and action map; disabled controls publish no callable action. Caller
props remain authoritative. Select emits a choice without applying it.

Named slots are host-owned factories over validated data, producing fresh
elements on each invocation. No executable closures cross JSON. Shared
`KitState` drops its internal map borrow before constructing nested elements.
Accordion slots use section identity; ScrollArea uses `content`;
SplitPane uses `start` and `end`; List lazily requests slots by row identity;
Popover and Dialog retain a `content` factory across close/reopen.
The host owns recursive node validation,
aggregate budgets, revision/generation checks, namespaces, and permissions.

`methods.json` exactly mirrors `kitMethods`: 76 commands and 45 queries across
13 components. The typed target contains only id and
component. Native dispatch checks retained identity and actual disabled state;
the host checks mounted generation/revision and applies owner policy. Explicit
controlled text/selection props win on the next render. Argument-free methods
accept omitted arguments; other methods require their declared named fields.

Every mount rerender cancels pending requests for the old worker revision.
Cancellation does not roll back an already executed setter. In particular,
`set_value` can emit a change handler that rerenders before an awaited follow-up
query; this is not an atomic setter/event transaction. The method fixture uses
`set_text_quietly` to avoid that echo while testing the native round trip.

## Explicit remaining gaps

- Native Entity/Focus references, typed native child composites, and arbitrary
  locale callbacks remain unsupported. The four date `*_snapshot` wire queries
  return data, not native references. Coverage metadata names the Rust getters
  they read; it does not make the original reference getters callable.
- `controls_extra.familyBindings` and `ControlsExtraBindingValues` await the
  separately owned central `kit.bind` helper. This integration does not expose
  family reactive bindings or add native predicate/construction-context plumbing.
- Only declared commands and queries are bound; other public methods, reactive
  bindings, arbitrary callbacks, and native entity references remain unsupported.
- Segment icons/tints, token/style options,
  Tabs reorder/overflow-menu/save-state, Pagination page-size entity, and
  ScrollArea bound scroll targets are not yet adapted. List currently supports
  native same-list reorder intent but not caller-supplied cross-list acceptance
  predicates or drag velocity in its event payload.
- Markdown is held until the host provides owner-aware native clipboard policy.
  TextInput uses the framework's fallible clipboard API and emits
  `clipboardDenied: 'missingOwner' | 'denied'` without editing on refusal.
  The host must install the owner policy and scope native elements; this adapter
  does not grant a trusted-gesture exception. End-to-end permission coverage
  remains conditional on the runtime and framework owners' integrated tests.
- Components absent from registration have no adapter. No generic placeholder
  is counted as an implementation.

## Verification and schema generation

```sh
npm ci --prefix tools/app-host --ignore-scripts
node --test tools/js-runtime/tests/kit-bindings.test.mjs
cargo test -p gpui-box-app-host --features capture kit_bindings
cargo test -p gpui-box-kit --lib retained_options_tests
cargo clippy -p gpui-box-app-host --features capture --all-targets -- -D warnings
cargo run -p xtask -- api generate
node tools/js-runtime/catalog.mjs
node tools/js-runtime/catalog.mjs --check
```

`catalog.mjs` generates binding coverage, native schemas/methods, and the central
SDK method section together. Factory interfaces extend family declarations;
`gpui-app init` copies all runtime sibling declarations into new projects.

The GPUI tests dispatch simulated native input and verify semantic bounds,
typed intents, refused selection, retained editing, and teardown. They are not
process-only event tests and are not a hardware-renderer receipt.

`fixture/` is explicit non-product data for the real Host offscreen capture:

```sh
cargo build -p gpui-box-app-host --features capture
target/debug/gpui-box-app-host tools/app-host/runner.mjs \
  tools/app-host/src/kit_bindings/fixture --data-dir /tmp/gpui-kit-layout-data \
  --capture .amp/in/artifacts/kit-layout.png
```

This requires the runtime owner's untrusted Linux sandbox dependencies, and
does not authorize a fallback to trusted execution. An offscreen frame is not
evidence that the live Xvfb window works.

## Local recursive schema documents

Each props, event, predicate, method-args, or method-result schema is its own
document. Only that document's root may declare `$defs: {Name: schema}`.
`{$ref: 'Name'}` resolves an exact local name matching
`[A-Za-z_][A-Za-z0-9_]{0,63}`. No URI, JSON pointer, file, network lookup, nested
definition scope, or sibling ref constraint other than `nullable` is supported.
`nullable: true` accepts null before data dispatch, including refs and unions;
document validation still rejects unknown refs and non-progressing cycles.

Definitions are serialized once and references remain references in the native
JSON catalog. Do not unfold them to a fixed-depth approximation. Unknown refs
and cycles made only of refs/union branches fail document validation, including
unused definitions. Recursion is valid only after an object field or array item
advances to a child data value. Except for the explicit nullable override,
`oneOf` still requires exactly one matching branch; it is not a first-match union.

JS/native validators enforce root data depth 0 through 32 inclusive, 100,000
validation steps shared by all branches (including failed branches and refs),
and a 256-call validation stack limit. Documents allow 4,096 schema nodes and
128 nested schema/ref edges. Budget exhaustion aborts validation; another union
branch cannot turn it into a success. Existing host payload/descriptor limits
remain independent. JS reads data descriptors without executing accessors.

For generated TypeScript, emit `schemaDefinitions(document, 'FamilyDefs')`
once, then `schemaType(document, 'FamilyDefs')` for its root type. Recursive
members use `FamilyDefs['Name']`, retaining linear declaration size. Plain
schemas still use `schemaType(schema)`; `generateKitMethodTypes` emits named
definitions automatically when args/results need them. TypeScript describes
the structural union; exact-one overlap, depth and budgets remain runtime
checks. Family relational/topology validation runs after structural validation.

Dynamic slots may declare `slotPaths: ['entries', 'groups.entries']`. Each dot
selects an own property and arrays are flattened at that step; only the final
items' `id` values become slot names. Missing branches add nothing. Static slots
and existing `slotIds` remain additive; `slotSuffixes` applies to either source.
Paths use local identifier segments (at most 32), never indices, wildcards,
prototype traversal, executable selectors, or filesystem paths. Props are
structurally validated before this slot lookup; JS rejects accessor traversal.
