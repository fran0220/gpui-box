# Native Kit adapters

`COMPONENTS` is the native registration authority, not the generated developer
catalog. `schemas.json` is the data-only serialization of `kitSchemas` from
`tools/js-runtime/kit-schema.mjs`. JS tests compare its entire contents; Rust
tests compare its component keys with the native registration list.

These **13 partial adapters** instantiate real Kit components: Checkbox, Radio,
Switch, Slider, SegmentedControl, TextInput, Select, Pagination, Tabs, Accordion,
ScrollArea, SplitPane, and Divider. They are not full catalog coverage. Supported
props/events are exactly the schema fields and TypeScript `KitAPI` declarations;
unknown fields fail closed in both JS and native validation.

The native entities for TextInput and Select persist by host-injected process
instance and semantic identity. Reconciliation drops subscriptions for removed
or replaced entities. Retained subscription routes update to the current frame
emitter and action map; disabled controls publish no callable action. Caller
props remain authoritative. Select emits a choice without applying it.

Named slots are pre-rendered by the host and contain no callbacks or native
handles. Accordion slots use section identity; ScrollArea uses `content`;
SplitPane uses `start` and `end`. The host owns recursive node validation,
aggregate budgets, revision/generation checks, namespaces, and permissions.

## Explicit remaining gaps

- Public commands, queries, reactive bindings, arbitrary callbacks, and native
  entity references are not exposed as JS handles.
- Segment icons/tints, Select option description/groups, token/style options,
  Tabs reorder/overflow-menu/save-state, Pagination page-size entity, and
  ScrollArea bound scroll targets are not yet adapted.
- Markdown is held until the host provides owner-aware native clipboard policy.
  TextInput native clipboard gestures are also **not permission-complete**:
  denying a JS capability does not by itself gate GPUI copy/cut/paste. A refused
  cut must preserve text, and a refused copy must not report success.
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
