# Native release-time predicates

List, Tabs and Tree use the shared native `DeferredDrop` primitive. The host
owns one controller per mounted predicate surface, and passes it through
`NativeBuildContext` before erasing the element. Families do not own a second
controller or evaluate guest code during rendering or hover.

A real pointer release produces a pending native `DropRequestId`. The runner
maps the displayed mount alias back to exactly one active worker target and
calls `Session.evaluatePredicate`. Only a boolean or `Promise<boolean>` is
accepted. The host resolves using the original typed ID retained by the native
controller, not an ID reconstructed from the wire. Native data, source, target,
owner, revision and deadline checks still run before reporting reorder/move.
These three families keep their native own-surface acceptance policy; a guest
predicate may restrict that policy but cannot widen it.

The bridge allows at most 32 outstanding requests, with the same per-worker
bound and a three-second deadline. Exceptions, malformed
results, unavailable targets and deadline expiry refuse the move. Revision,
generation, mount removal and disposal cancel old requests. Cancellation
detaches the decision; it cannot preempt arbitrary JavaScript already running.
No pointer capture is retained while awaiting JavaScript, and caller-owned
data changes only through the eventual native reorder/move event.

Run the real isolated worker/native pointer review explicitly:

```sh
cargo test -p gpui-box-app-host --all-features \
  native_pointer_release_resolves_isolated_predicates_without_stale_mutation \
  -- --ignored --nocapture
node --test tools/app-host/test/drop-bridge.test.mjs tools/js-runtime/test/predicates.test.mjs
```

The native test does not use `--trust-local`, direct controller requests, or a
fixture button pretending to drop. It dispatches down/move/up input, asserts
no pending request during hover, verifies accepted List/Tabs/Tree ordering,
and checks false, nonboolean, timeout, stale revision and removal paths leave
caller data unchanged. Its explicit synthetic fixture captures accepted,
refused and stale List states under `.amp/in/artifacts/`.
