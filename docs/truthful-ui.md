# Truthful UI

Truthful UI means the display does not claim more than the owning application
layer knows.

## Required states

`Loadable<T, E>` distinguishes:

- `Idle`: never requested;
- `Loading`: request is in progress and there is no verified value;
- `Ready(T)`: verified value;
- `Empty`: successful result with no items;
- `Unavailable(reason)`: capability cannot currently be provided;
- `Error(E)`: request failed.

These states are not interchangeable.

## Phase

Per-surface enums keep the typed payload they already carry. `Phase` is the
shared projection: what a surface knows about its own content, not what that
surface is doing. `HasPhase` is how those enums, and `Loadable` / `AsyncValue`,
answer the same question.

`Phase` is:

- `Idle`: never requested;
- `Queued`: accepted, and not yet started;
- `Blocked`: waiting for an answer (approval, credentials);
- `Loading`: in flight, with no verified value;
- `Refreshing`: in flight, keeping a verified value on screen;
- `Ready`: a verified value;
- `Empty`: a successful result with nothing in it;
- `Unavailable`: a refusal, or a capability that does not exist;
- `Error`: the attempt failed;
- `Cancelled`: withdrawn.

A transport that is playing and one that is paused are both `Ready`. The
difference belongs to the transport.

`StateView` renders a `Phase` without inventing one. A refresh keeps the last
verified content under `RefreshVeil`. `is_stale()` is true only after a
failed refresh that still holds a verified value. `EmptyKind::Unauthorized`
is the authorization refusal; it is not `Unavailable`.

The projection for each surface enum, asserted next to the `HasPhase` impl:

| Surface | Mapping |
|---|---|
| `ChartState` / `SparklineState` / `MetricState` / `OfferingSourceState` | `Stale` → `Error` + `is_stale()`; other variants keep their name |
| `LogStreamState` | `Stale(reason)` → `Error` + `is_stale()` |
| `HeatmapState` / `KanbanState` / `AudioWaveformState` / `PromptBuilderState` | three-phase direct |
| `ImageState` | `Failed` → `Error` |
| `AgentDocumentState` | `Idle` → `Idle`, `Failed` → `Error` |
| `ViewportState` / `TerminalState` / `ArtifactPreviewState` | same-name direct |
| `GraphState` | `Refused` → `Unavailable`, `Failed` → `Error` |
| `ModelState` | `Rejected` → `Unavailable`, reason from `ModelError` |
| `ServerState` | `Connected` → `Ready`, `Connecting` → `Loading`, `Disconnected` → `Idle`, `Failed` → `Error`, `Disabled` → `Unavailable` |
| `TransportState` | `Playing` / `Paused` → `Ready`, `Buffering` → `Refreshing` |
| `UploadState` | `Queued` → `Queued`, `Uploading` → `Loading`, `Done` → `Ready`, `Cancelled` → `Cancelled`, `Refused` → `Unavailable` |
| `ToolCallState` | `PendingApproval` → `Blocked`, `Running` → `Loading`, `Succeeded` → `Ready`, `Refused` → `Unavailable` |
| `DropzoneState` | `Idle` → `Idle`, `Accepting` → `Ready`, `Refusing` → `Unavailable` |

## Refresh without erasure

`AsyncValue<T, E>` keeps the last verified value separately from request
status. A refresh can therefore move through:

```text
Ready(value)
  → Refreshing + value
  → Error(error) + value
```

The UI continues showing `value`. `Refreshing` covers it with `RefreshVeil`.
A failed refresh keeps the same value, marks it stale, and reports the error.
It does not replace verified content with an empty card.

## Actions

An interaction follows:

```text
user intent
  → application action
  → host acceptance or refusal
  → authoritative projection update
  → rendered result
```

The click itself is not success. Progress UI may start when the application
accepts the request, but completed UI waits for the authoritative projection.

## Refusals

Host refusals retain their meaning. The presentation may add context, but must
not rewrite:

- permission denied into “not found”;
- unsupported into an empty list;
- authentication required into a generic retry loop;
- failure into a completed badge.

## Disabled behavior

Disabled state has three obligations:

1. action callback is not installed;
2. semantic node reports `disabled: true`;
3. the reason is available in nearby text, tooltip, or status when it is not
   obvious.

Opacity alone is not a disabled implementation.

## Attempts and stale results

For host-backed async flows, the application should identify each attempt.
Results from an older attempt cannot overwrite newer state or close a newer
dialog. Filesystem operations should reserve destinations atomically and clean
up only paths owned by the current attempt.

These rules belong to the application layer. The component library provides
the visible states but does not become the authority.
