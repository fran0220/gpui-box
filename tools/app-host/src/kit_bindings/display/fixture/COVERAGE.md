# Native DISPLAY adapter coverage

Membership: every component whose api-index source is `src/display/`, minus
Divider and the 13 chart-family components. All 37 instantiate real native
builders; none uses a catalog factory or fixture substitution. All are value
builders: there are no native entity commands to advertise. Controlled prop
updates rebuild builders; GPUI owns their visual transient state.

The table lists native public method names, not imaginary RPC methods. `new`
is covered for every row; `id` is the descriptor identity wherever native `id`
exists. Option names on the wire are camelCase, command/query names snake_case.
Callback options install native handlers and emit only the documented data.
Native theme/window/context parameters are host-owned. `disabled` suppresses
handlers even for builders without a native disabled option.

| Component | Native options / constructor alternatives | Events and slots | Queries |
|---|---|---|---|
| AnimatedNumber | format (bounded decimal/prefix/suffix formatter), spec (Bezier or physical Spring plus delay), type_scale | — | — |
| AttachmentTile | description, state | media/title/description/actions native Slotted factories | — |
| Avatar | presence, id, size, tint; image adapted securely to image_source(ResourceRef) | lazy loader failure: named image-unavailable warning, not initials | — |
| AvatarGroup | id, members, size, overflow | members use typed Avatar builders | — |
| Badge | count, icon, dot, id, tone, tint, variant, color; neutral/accent/success/warning/danger/info normalized to tone | — | — |
| Banner | title, action, on_dismiss | dismiss(null); action element slot | — |
| BarLoader | label, tint | — | — |
| Bubble | content, actions, placement, grouped, max_width | content/actions element slots | — |
| Callout | id, constructor message/tone | — | — |
| Card | id, name, variant, ground, header, media, footer, padded, padding, on_click; ParentElement content | click(null); content/media/footer/headerAction, native CardHeader subtitle/action | — |
| DescriptionList | item normalized to items; items, columns, on_copy | copy(item id), never copied content | — |
| EmptyState | kind, icon, detail, action | action element slot | — |
| FailurePanel | new or from_result (closed success/error union); title, detail, attempts, retrying, on_retry | retry(null); success from_result renders no panel | — |
| Heatmap | tint, rows, columns, cells, state | empty native slot | — |
| HighlightedText | id, selectable, in_document, hits (UTF-8 byte ranges), current, monospace | native selectable text/document integration | published_hits() calls native filter |
| Icon | new or named; spinning, breathing, reacting; tone, follow_direction; muted/faint/on_accent/accent/danger/warning/success/info normalized to tone | closed built-in glyph; named icon semantic identity | resolved_size(), resolved_color(), flips_in({direction}) |
| ListRow | id, leading, trailing, on_click; ParentElement content | click(null); leading/trailing/content slots | — |
| LoadMore | state, on_more | more(null) | — |
| MetricCard | constructor caller MetricState, tint | empty/failed/loading native slots | — |
| OutcomePanel | title, detail, count, action | action element slot | — |
| PerformanceHud | expanded, on_expanded, caller FrameTimingSummary | expanded(bool) | — |
| ProgressBar | label, fraction or count, display, stalled, paused, on_cancel | cancel(null) | — |
| ProgressCircle | label, fraction or count, display, centre, stalled, paused, on_cancel | cancel(null) | — |
| PulseLoader | label, tint | — | — |
| Rating | label, value, maximum, precision, clearable, on_change | change(number or null) | — |
| RefreshVeil | constructor content, label | content element slot | — |
| Skeleton | label, rows, row_height, widths, shapes (all five native variants) | — | — |
| SpanTimeline | spans, axis, ticks, current, on_select | select(span id); empty native slot | — |
| Spinner | label, tint | — | — |
| StageProgress | stages (caller ProgressStage) | — | — |
| StaleMark | updated, constructor reason | — | — |
| StateView | new(HasPhase data) or from_async(AsyncValue data + content slot), content, elapsed | content plus empty/failed/loading native slots | — |
| StatusDot | tint, busy, activity | deliberately native decorative, no invented semantic node | — |
| StatusLine | id, tint, busy, activity | — | — |
| Tag | tone, tint, variant, color, on_remove | remove(null) | — |
| Timeline | group normalized to groups; groups, entries; TimelineEntry time/time_unknown/actor/tone/detail | entry-ID detail slots, direct and grouped; group IDs are not slot names | — |
| TraceView | spans, axis, ticks, current, on_select | select(span id); empty native slot | — |

## Data-only adaptations and explicit limits

- Arbitrary native closures are not serializable. AnimatedNumber.format exposes
  decimal precision/prefix/suffix, not arbitrary user JavaScript execution in
  native paint. These are bounded adapters, not claims of unrestricted closure
  transport. Slot closures belong to the host and produce fresh elements.
- Native convenience tone methods and singular item/group methods normalize to
  their actual native tone/items/groups setters; no separate RPC commands are
  claimed for aliases. Native Signal bind is not transported. Worker state can
  mirror controlled props and typed events through the shared runtime.
- Avatar string paths/URLs are intentionally unavailable to JS. ResourceRef
  resolution needs the resource owner's installed store, mounted EffectOwner,
  and mount→generation authorization. Selective icon asset absence is never
  repaired by loading a full catalog.
- Motion uses native GPUI policies. Static captures use reduced motion; they do
  not verify animation timing. No macOS or Windows renderer claim is made by
  the Linux captures.

## Integration (owned centrally, not in this delivery)

1. Declare `mod icon; mod display; mod charts;`, add family COMPONENTS, delegate
   render with existing KitSlots and Emit. Neither family needs retained State.
2. Merge family schemas.json/methods.json; run family `validate(node)` after
   closed schema validation. Delegate query invocation after host authority,
   generation/revision/mount checks, then validate the returned data.
3. Merge JS familySchemas/familyMethods and call validateFamilyProps after
   generic validation. Extend SDK factory/method interfaces using both family
   declarations. Regenerate central registry and API/developer indexes.
4. Timeline uses generic `slotPaths: ['entries','groups.entries']` from central
   slot-path commit f3fa310 (parent owns integration). No permissive family bypass.
5. Install resource module and its base64 dependency. Resolve images under the
   mounted effect owner. Keep resource store lifetime and revocation in host.

## Executed evidence

Native tests render every builder in both themes, exercise all declared display
event names, all four queries, alternate constructors, fresh/lazy slots and
slot capture teardown. Actual headless Resources register→resolve→revoke
replaces red pixels and publishes avatar.image-unavailable in both themes.
JS tests use real createKitBindings factories as well as closed schemas; strict
TS fixtures include positive and expected-error cases. Review captures are
opt-in via GPUI_FAMILY_CAPTURE_DIR and must be inspected, not just generated.
The parent must still run the combined full gate after central integration.
