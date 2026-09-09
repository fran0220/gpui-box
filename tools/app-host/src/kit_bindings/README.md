# Native Kit adapters

`COMPONENTS` is the native registration authority, not the generated developer
catalog. `schemas.json` is the data-only serialization of `kitSchemas` from
`tools/js-runtime/kit-schema.mjs`. JS tests compare its entire contents; Rust
tests compare its component keys with the native registration list.

These **16 partial adapters** instantiate real Kit components: Checkbox, Radio,
Switch, Slider, SegmentedControl, TextInput, Select, Pagination, Tabs, Accordion,
ScrollArea, SplitPane, Divider, List, Popover, and Dialog. They are not full catalog coverage. Supported
props/events are exactly the schema fields and TypeScript `KitAPI` declarations;
unknown fields fail closed in both JS and native validation.

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

`methods.json` exactly mirrors `kitMethods`: 39 commands and 14 queries across
TextInput, Select, Popover, and Dialog. The typed target contains only id and
component. Native dispatch checks retained identity and actual disabled state;
the host checks mounted generation/revision and applies owner policy. Explicit
controlled text/selection props win on the next render. Argument-free methods
accept omitted arguments; other methods require their declared named fields.

## Explicit remaining gaps

- Only declared commands and queries are bound; other public methods, reactive
  bindings, arbitrary callbacks, and native entity references remain unsupported.
- Segment icons/tints, Select option description/groups, token/style options,
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
node --input-type=module -e "import {kitSchemas} from './tools/js-runtime/kit-schema.mjs'; import {writeFileSync} from 'node:fs'; writeFileSync('tools/app-host/src/kit_bindings/schemas.json',JSON.stringify(kitSchemas,null,2)+'\n')"
```

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
