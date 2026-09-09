# Canvas and remaining overlay adapters

This fixture uses native Kit builders/entities, not catalog renderers. The five
canvas components are CanvasToolbar, GraphNode, NodeGroup, Minimap, NodeGraph.
The thirteen overlay components are CommandPalette, ContextMenu, Drawer, Frost,
Glass, HoverCard, Kbd, Menu, Menubar, NotificationCenter, Overlay, ToastLayer,
Tooltip. Popover/Dialog and shared DnD remain separately owned.

## Central integration (not included in the family commits)

- Declare `mod canvas`, `mod overlay_extra`, and the shared `mod icon` in
  `kit_bindings`. Add both `COMPONENTS` slices to the central membership list.
- Retain one `overlay_extra::State` in KitState. Call its `reconcile` with the
  current root and route its family through `State::render`; canvas uses free
  `render`. All receive the current guarded reusable `KitSlots` and emitter.
- Call each family's `validate_props` after closed grammar validation.
- Merge `familySchemas` and `familyMethods` from `kit-canvas-schema.mjs` and
  `kit-overlay-schema.mjs` centrally. Call `validateCanvasProps` and
  `validateOverlayProps` after base JS prop validation. Regenerate embedded
  `schemas.json`/`methods.json`; extend KitFactories with CanvasFactories and
  OverlayFactories, and regenerate the shared method declarations.
- Route overlay data commands/queries to `State::invoke`, preserving central
  argument/result validation and generation/revision/mount/owner checks.
- With the shared `references::Registration` scoped by the mounted node:
  route `focus_handle` and `set_focus_stops` through `invoke_reference`;
  route CommandPalette `query_input` through `query_input`, supplying the
  input binding owner's fixed native TextInput `EntityDispatch`:
  `crate::kit_bindings::reference_dispatch::text_input`.
- Call `apply_reference_props` after Drawer render on every frame, even when
  the option did not change. It revalidates both builder and imperative focus
  stops and removes invalid native stops rather than retaining old handles.
  Surface its error as a refusal. Use `native_entity_id` for shared registry
  reconciliation anchors. Never substitute a family-local reference registry.

The temporary checkout hooks additionally declare the borrowed top-level
`references` and `kit_bindings::reference_dispatch` modules. The reference tests
use that owner's registry and real shared TextInput dispatcher, including closed
arguments, wrong-mode refusal, nested focus registration, and native disabled
checks. Production transport registration still requires the runtime owner's
final integration. The family schemas use
the agreed closed `$nativeRef`/literal `type` marker shape; strings alone grant
no authority. The SDK currently expresses that wire shape structurally; a shared
nominal TypeScript brand can replace it at the central generator boundary.

## Data adaptations

- Graph node/edge/band arrays adapt repeated native builders. `offset` and
  `zoom` override their corresponding `viewport` fields when both are supplied.
  `fit: null` means Never; a nonnegative integer means Whole(revision).
  `can_connect` is a precomputed caller-owned output/input endpoint-pair list,
  not a JS callback. It controls native validity previews; correctly directed
  proposals still reach the caller, which owns acceptance/refusal. Rendering,
  transforms, port bounds and gestures stay native.
- `empty` configures the native EmptyState builder. `empty_action` is its child
  slot; `empty`/`failed`/`loading` slots replace native named surfaces.
  Node slots are exactly `${id}:content` and `${id}:thumbnail`.
  `node_click` reports the clicked placed node's id; generic graph proposals
  retain their full discriminated event data, including back/forward buttons.
- `set_content`/`set_footer`/`set_trigger` accept a currently mounted local slot
  key, or null where native clearing exists. Factories are weak and create fresh
  elements. They never retain a consumed AnyElement or a worker closure.
- Menu `offered` and Menubar `menus` return all public getter values on the
  returned native records, not their private layout/closure fields.
- Toast and notification action labels bind native handlers that emit the
  caller-owned identity. Notification.show reports actual toast delivery.

## Evidence and remaining integration limits

`methods.json` is explicit expected native data-method behavior; both Rust and
JS consume it. Reference methods have separate real-registry native tests.
Native tests cover transformed nested graph movement/ports, pointer selection,
keyboard deletion and menu invocation, retained open menu focus, drawer refusal
and weak removal, context cancellation refusal/replacement/removal, capacity
eviction/read retention and unreset toast timers. The core HoverCard regression
covers pending timer updates/cancellation and focus recovery.

`main.mjs` and the canvas fixture run in real isolated Session workers. Their JS
tests cover caller state updates, slot descriptors and revision revocation.
The canvas fixture `types.ts` checks positive and rejected TS calls.

The inspected native image is `.amp/in/artifacts/native-canvas-overlay.png`:
open drawer, disabled toolbar, succeeded/failed graph nodes and failed curve.
The modal scrim intentionally dims the canvas. Native macOS/Windows execution
and the parent's regenerated, fully integrated full gate remain required.
Reference transport must be tested after the shared runtime hooks are wired;
the Rust registry tests do not claim that end-to-end transport is complete.
