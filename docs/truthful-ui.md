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

## Refresh without erasure

`AsyncValue<T, E>` keeps the last verified value separately from request
status. A refresh can therefore move through:

```text
Ready(value)
  → Refreshing + value
  → Error(error) + value
```

The UI continues showing `value`, marks it stale, and reports the refresh
error. It does not replace verified content with an empty card.

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
