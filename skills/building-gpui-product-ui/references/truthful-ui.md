# Truthful UI quick reference

```text
Intent is not acceptance.
Acceptance is not completion.
Failure is not empty.
Unavailable is not disabled.
Fixture data is not product evidence.
```

Prefer:

```rust
AsyncValue {
    value: Some(last_verified),
    status: AsyncStatus::Error(refresh_error),
}
```

over clearing the value on refresh failure.

Prefer a disabled control with a reason over a clickable control that silently
does nothing.

Keep attempt identity in the application layer. A completion from attempt A
must not mutate the screen now displaying attempt B.
