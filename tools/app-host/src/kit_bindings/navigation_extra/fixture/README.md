# Native navigation, layout and datetime adapters

These adapters build the actual Kit controls. The JSON alongside this document
is a reproducible wire contract, not runtime sample data. `datetime/fixture/data.json`
is used only by tests. Production calendar facts are supplied by the caller.

## Surface

Every row has a native constructor and closed options schema. The generated SDKs
list the exact option, nested record, event and method types. Selection, history,
topology, floating records and date tables remain caller-owned.

| Component | Native options/data | Events | Fresh slots / queries |
| --- | --- | --- | --- |
| AnchorList | anchors, active, size, disabled, retained overflow menu/threshold | navigate | — |
| Breadcrumb | crumbs, maxVisible | select, reveal | — |
| Carousel | items, active, phase/reason/stale, looped, size | event | item ids |
| Collapsible | title, description, open, size, disabled | toggle | body |
| NavStack | restored entries and cursor, label; retained per-visit focus | caller changes history | entry ids; active entry alone is mounted |
| Sidebar | sections, recursively assembled caller items, active/collapsed, size/disabled | select | header, footer |
| UndoHistory | entries, current, label, disabled | jump | — |
| Wizard | steps, layout, back/finish/advance policy, labels, size/disabled | navigate (exact variants) | body |
| AspectRatio | ratio, fit | — | content; native ratio query |
| Container | width/customWidth, padding | — | content |
| DesktopTitlebar | title/subtitle, button layout, platform controls | event | left, right |
| Dock | panels, region active/collapsed/share/minSize, disabled | event (exact variants) | panel ids |
| DockTree | native restored tree and floating topology, panels, disabled | event (exact variants) | panel ids |
| Grid | columns/breakpoints, gap, item spans/breakpoints | — | item ids |
| Responsive | measured native size threshold, fill | — | unmeasured, narrow, wide |
| ScrollEdgeEffect | edges, band, blur, soft/hard | — | content |
| ScrollFade | edges, band, fitHeight | — | content |
| SplitTree | native restored pane/split records, disabled | change (exact variants) | pane ids |
| StatusBar | grouped native text/state/progress/action/element items | click | element item ids |
| Toolbar | groups/items, size/label, retained overflow menu/threshold | overflowSelect | item ids; native item_count query |
| Calendar | caller adapter table, selected/multi/month/range/marks/disabled | pick, monthShown, hover | empty; all native commands and value queries |
| DateInput | adapter, value/required/invalid/disabled/size | change, unparsable, open, close, submit | all native commands and value queries |
| RangePicker | adapter, range/marks/invalid/disabled | startPick, endPick | all native commands and value queries |
| TimeInput | adapter, value/seconds/invalid/disabled/size | change | all native commands and value queries |

The date `fixture/methods.json` lists each actual native dispatch. Added core
setters reconfigure Calendar multi/navigation, DateInput required/density and
TimeInput seconds/density without replacing entities. Native disabled queries
remain available; native disabled commands are refused, including re-enabling
through a command (the caller can re-enable through props).

## Deliberate data-only boundaries

- Calendar adapters implement a finite, bounded caller table, not arbitrary
  locale callbacks. Opaque ordered day tokens and month keys, exact parse aliases,
  blocked reasons, weekday/month labels and clock labels are caller data. No
  process clock, locale guesses, fixture substitution, URL/path or loader exists.
- `completeRange` is an explicit completeness assertion for that finite day
  table. Without it, range coverage is **Unchecked**, not Clear or a checked empty
  result. Dates/months outside the declared domain are refused. Locale/label-only
  updates preserve edits/open state; domain changes reset invalid navigation.
- Native reference getters are **unsupported**, excluded from the schema/SDK,
  and refused by native dispatch until the shared owner/generation/mount-guarded
  opaque typed-ref primitive is integrated. A snapshot is not handle equivalence:

  | Unsupported native getter | Actual native return | Separate data-only query |
  | --- | --- | --- |
  | `DateInput.field()` | `&Entity<TextInput>` | `field_snapshot` |
  | `DateInput.calendar()` | `&Entity<Calendar>` | `calendar_snapshot` |
  | `RangePicker.calendar()` | `&Entity<Calendar>` | `calendar_snapshot` |
  | `Calendar.adapter()` | `&SharedDateAdapter` (`Rc<dyn DateAdapter>`) | `adapter_snapshot` |

  No FocusHandle getter is exposed; NavStack focus handles stay internal. Ordinary
  value getters retain their native names and behavior. These four reference
  getters must not be counted as complete native surface coverage.
- `Responsive` offers three slots selected from native measured size; arbitrary
  synchronous JS render callbacks do not cross the worker. Calendar overlays are
  supplied as day-mark records for the same reason.
- Dock, DockTree and SplitTree call their actual native topology builders and
  emit caller intents. Floating changes keep live `finished:false`, completed
  `finished:true` and geometry-free cancellation distinct. No async accepts policy
  is simulated: shared async intent/DnD remains the runtime owner's work.
- This worker verifies Linux native input and offscreen output. Real macOS/Windows
  window-button delivery and their renderers require the parent's platform lanes.

## Parent integration hooks (not part of this worker's commits)

1. Declare `mod navigation_extra; mod layout_extra; mod datetime;` plus the shared
   display-owned `mod icon;`. Add all family `COMPONENTS` to central registration.
2. Retain one `State` per family inside `KitState`. Call every family's
   `reconcile(root,cx)` on accepted trees. Dispatch `render` and `invoke` by family
   membership. The family methods use the existing `Node`, `KitSlots`, `Emit`,
   `Route`, `(instance,id)` key and existing small shared helpers from `super`.
3. Call each native family's `validate(node)` from descriptor validation; retain
   central event/slot/owner/generation/mount validation. Datetime validates its
   own invocation args/results too; central method registration still needs the
   freshly generated family method schemas, including all snapshot getters.
4. Import/spread each `familySchemas`/`familyMethods` into the central JS grammar,
   and invoke `validateFamilyProps(component,props)` from central validation.
   Merge the corresponding generated Rust schemas/methods. Extend central SDK
   factories/methods with `NavigationExtraFactories/Methods`,
   `LayoutExtraFactories/Methods`, and `DatetimeFactories/Methods`.
5. Regenerate API/developer indexes for the narrow new core setters/getters.
   Update the old central `every_declared_method_has_native_dispatch` test, which
   assumes only four original components; family tests cover every declared new
   command/query. Run the combined gate after integration.

## Reproduction

```sh
node tools/app-host/src/kit_bindings/navigation_extra/fixture/generate.mjs --write
TSC_BIN=/path/to/typescript-5.9.3/bin/tsc node --test tools/js-runtime/tests/kit-{navigation_extra,layout_extra,datetime}.test.mjs
cargo test -p gpui-box-app-host --all-features kit_bindings::navigation_extra
cargo test -p gpui-box-app-host --all-features kit_bindings::layout_extra
cargo test -p gpui-box-app-host --all-features kit_bindings::datetime
cargo clippy -p gpui-box-app-host --all-targets --all-features -- -D warnings
GPUI_FAMILY_CAPTURE=.amp/in/artifacts/native-families cargo test -p gpui-box-app-host --all-features capture_native_family_review -- --ignored
```

Tests exercise native pointer and keyboard actions, asymmetric history/focus
restoration, toolbar reopen/removal, all layout builders, floating pointer
gestures, typed date refusals, retained entity identity and teardown, disabled
command refusal, all declared commands/queries, exact TypeScript and closed
schema rejection. Offscreen review covers selected/floating DockTree, nested
selected/disabled Sidebar and a marked multi-select finite Calendar.
