# Host/view boundary

## View responsibilities

A GPUI view may:

- read an immutable or application-owned view model;
- render components;
- emit typed application actions;
- hold hover, focus, open, scroll, animation, and local input draft state.

It must not:

- open databases or filesystem paths;
- launch processes;
- read raw credentials;
- call product RPC transports;
- infer success from a click;
- become the durable owner of product facts.

## Suggested shape

```rust
pub struct SettingsViewModel {
    pub account: Loadable<AccountSummary>,
    pub saving: bool,
    pub refusal: Option<String>,
}

pub enum SettingsAction {
    Refresh,
    SignIn,
    SignOut,
}
```

The view receives `SettingsViewModel` and sends `SettingsAction`. Application
code maps actions to host operations and later projects authoritative state
back into the model.

## Secret boundary

Views receive safe summaries, never raw tokens:

```rust
pub struct AccountSummary {
    pub display_name: String,
    pub email: String,
}
```

Semantic trees are diagnostic output and require the same boundary. The
semantics crate redacts common credential shapes before export, but redaction
is defense in depth, not permission to put secrets in nodes.

## Blocking work

Filesystem, Git, network, process, and database operations run outside the
window thread. Completion returns through the owning application's executor
and attempt identity. Components only display the resulting state.

## Fixtures

Fixtures may instantiate view models directly. They must be labeled as fixture
data and cannot serve as proof that a host-backed behavior works. Product smoke
tests exercise the host boundary separately.
