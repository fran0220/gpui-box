# Standalone native app host

The Rust package renders JS-created trees with this repository's local GPUI and
Kit. No component depends back on the host, runtime or plugins. No WebView is used.
This is a runnable partial implementation, **not full Kit binding or platform security parity**.

## Run

Requirements: Rust from `rust-toolchain.toml`, external Node **26.5.1 or newer**,
and a native display. Linux isolation additionally requires `bubblewrap`, `prlimit`
and working unprivileged user namespaces; `.agents/setup` installs Bubblewrap.

```sh
cargo build -p gpui-box-app-host
node tools/app-host/cli.mjs init /tmp/my-gpui-app
node tools/app-host/cli.mjs dev /tmp/my-gpui-app --data-dir /tmp/my-gpui-data
```

Linux defaults to OS-isolated execution. Unsupported OS backends refuse startup.
`--trust-local` explicitly runs known/trusted app code with user-level trust instead;
it is not a substitute sandbox. macOS/Windows OS sandbox support remains incomplete.
The native plugin enable button separately names whether it requests isolation or
full code trust; app trust never silently enables installed plugins.

The template imports a second TS module, retains a counter, emits an async action,
requests host storage via the consent UI, and declares cleanup. Change either TS
file in `dev`: a fresh process loads the module graph and replaces the previous
generation only after mounting a valid view. State resets on reload; durable state
belongs in storage, not the discarded JS heap. A failed reload retains the verified
view and displays the failure. Disabled controls install no action handler.

## Plugins and packaging

```sh
node tools/app-host/cli.mjs bundle tools/app-host/example-plugin /tmp/panel.bundle.json
node tools/app-host/cli.mjs plugin-install /tmp/panel.bundle.json /tmp/my-gpui-data/plugins
# Start/restart the host, enable sample-panel, then click its command or press Ctrl-J.
node tools/app-host/cli.mjs plugin-list /tmp/my-gpui-data/plugins

cargo build --release -p gpui-box-app-host
node tools/app-host/cli.mjs build /tmp/my-gpui-app /tmp/my-gpui-package
/tmp/my-gpui-package/run.sh
```

The package contains the native executable, app sources and runtime/plugin scripts,
plus build metadata. Node is external by default. `build --bundle-node` downloads
the official pinned26.5.1 archive, verifies its SHA256 and includes the runtime
binary and license. The Linux binary adds about142MiB; system libraries/Bubblewrap
remain external. This digest checks integrity, not independently verified authorship.
`--host PATH` packages an explicitly selected executable (useful for debug builds).
This is a local directory package, not a signed `.app`, MSIX, AppImage or installer.
It does not execute package installation hooks. `--sandbox-launcher BIN` copies a
separately compiled native security launcher and wires GPUI_SANDBOX_LAUNCHER into
run.sh and run.cmd. Native Windows/macOS package execution awaits those lanes.
For stack traces and diagnostics use stderr; `RUST_LOG=info` enables framework logs.
`dev --debug` enables a private Unix socket (0700 directory/0600 socket), and
`gpui-app debug DATA '7 + 3'` evaluates inside the isolated worker through inspector
IPC, not a TCP port. Shutdown removes the endpoint. This is opt-in developer code
execution, not a full debugger UI; Windows debugger ACL support remains unavailable.

## Contracts and verification

Native objects stay in one retained Rust `Host` entity. Frames are validated before
replacement, with bounded input/output queues. JS state/events use script generation
and render revision; native identities are derived from declared semantic IDs,
never array positions. Plugin panels/commands are namespaced by plugin identity.
Permissions are host-owned controls in the supervisor, not plugin-provided prompts.

`gpui.invoke(target, method, args)` and `gpui.query(target, method, args)` return
Promises for explicitly implemented native methods. A target is a typed Kit node
or `{id, component}` matching a mounted descriptor; names and named JSON arguments
come from the adapter's closed schemas. No Rust object, closure, arbitrary method
lookup, or focus-based routing crosses the process boundary. Native execution uses
the target generation's EffectOwner and cannot create a trusted clipboard gesture.
Commands refuse disabled native controls even if their descriptor still says enabled;
queries may read them.

Each worker permits 32 pending native requests with three-second deadlines.
Responses correlate request ID, generation and revision; rerender and disposal
cancel pending Promises, and stale responses cannot resolve a replacement request.
Cancellation is not rollback: an already dispatched setter may have executed.
Use quiet setters when a subsequent query must complete before an event-driven
rerender. Results are bounded JSON, validated by the native method schema.

Lazy lists and overlays receive reusable host-owned factories over validated slot
descriptors. Each call constructs fresh elements without holding the retained-state
map borrow. Factories retain only a weak state handle and reject expired render
revisions; dropping the Host invalidates factories and revokes clipboard grants.

```sh
node --test tools/js-runtime/test/*.test.mjs tools/plugin-platform/test/*.test.mjs tools/app-host/test/*.test.mjs
node tools/js-runtime/catalog.mjs --check
cargo test -p gpui-box-app-host
cargo run -p xtask -- dependencies check
cargo run -p xtask -- gate full
```

`binding-coverage.json` and `catalog.d.ts` are generated from the exact Kit API index,
including source handles and all Rust signatures. Their `unbound` entries explicitly
record missing native adapters. Coverage metadata is **not** a working TS constructor
for those components. The primitives and typed Kit adapters are partial; full
component binding remains outstanding. The adapter list is generated from the
separately owned executable Kit schemas.

The optional `capture` feature uses the existing GPUI headless renderer, not a
WebView screenshot or fabricated preview:

```sh
cargo build -p gpui-box-app-host --features capture
target/debug/gpui-box-app-host tools/app-host/runner.mjs /tmp/my-gpui-app \
  --data-dir /tmp/my-gpui-data --capture /tmp/native-app.png
# Real native mouse down/up → supervisor → isolated JS callback → retained frame:
GPUI_CAPTURE_CLICK=65,230 GPUI_CAPTURE_EXPECT='Count: 10' \
  target/debug/gpui-box-app-host tools/app-host/runner.mjs /tmp/my-gpui-app \
  --data-dir /tmp/my-gpui-data --capture /tmp/native-counter.png
```

Captures are review artifacts, not window-derived baselines. Tests exercise the
real supervisor and worker processes: imported-module reload, retained last-good
views, stale callbacks, permission denial, isolated plugins, command/keymap routing,
upgrade failure/rollback and cleanup. The Linux syscall probe bypasses JS entirely.
See the runtime and plugin READMEs for the threat model and incomplete scope.

Linux offscreen rendering, native click-to-state, permission prompts and the
directory-packaged app were rendered and inspected in the orb. The Xvfb live-window
probe presented a black client area despite correct offscreen rendering; live X11
presentation is an unresolved limitation, not a passed test. Native macOS/Windows
execution and OS backend parity need their native environments and are not claimed.
