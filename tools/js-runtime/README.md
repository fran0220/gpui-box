# Native JS runtime contract (stage 1)

This is a separate host-side package, never a dependency of Kit. Run the end-to-end
app through `tools/app-host/cli.mjs`. Run runtime tests with
`node --test tools/js-runtime/test/*.test.mjs`.

## Engine decision

Node/V8 **26.5.1** is the verified version and the declared minimum. It is an
external runtime, not embedded in the Rust executable. Node provides actual ESM
loading (including relative imports and dynamic import), async functions, promises,
timers, stack traces and erasable TypeScript. Type checking is a separate `tsc`
step. JSX, TS enums, parameter properties and tsconfig path aliases are not
transformed; unsupported syntax reports Node's error. No browser/WebView is used.

QuickJS offers a substantially smaller engine and interrupt/memory APIs, but would
need a host module resolver, timer/promise scheduling integration and a TS compiler.
Embedded V8 preserves the ecosystem but adds engine build/toolchain coupling and
shares native host fate unless separately hosted anyway. Node child processes were
chosen for the already-required failure boundary and module/async implementation.

References: [Node TypeScript](https://nodejs.org/api/typescript.html),
[Node permissions](https://nodejs.org/api/permissions.html),
[QuickJS](https://bellard.org/quickjs/quickjs.html),
[Bubblewrap](https://manpages.debian.org/bookworm/bubblewrap/bwrap.1.en.html).
No upstream implementation was copied. Node and Bubblewrap remain separately
installed dependencies with their own licenses, not vendored framework authorities.

## Trust and authority

The supervisor is trusted native-host code; app/plugin code is not. Selecting
`sandbox: 'linux'` requires Linux x86-64, `/usr/bin/bwrap`, `/usr/bin/prlimit`,
unprivileged user namespaces and the expected Linux library directories. Startup
failure is a refusal, **never a fallback to unrestricted execution**. The backend:

- starts a new user/mount/PID/network/IPC/UTS namespace, drops all capabilities,
  clears environment variables and makes the root and package/runtime mounts read-only;
- exposes only the runtime binary, system shared libraries, package directory,
  runtime scripts, a private read-only `/proc`, null/urandom devices, and a 16 MiB
  private `/tmp`; no home directory, host `/etc`, storage, credentials or sockets;
- installs an x86-64 seccomp filter before Node executes. Fork/vfork, clone without
  `CLONE_THREAD`, clone3, sockets, namespace changes, ptrace and privileged kernel
  interfaces are denied. Kernel clone invariants require accepted threads to share
  the address space; the native probe checks both rejection and valid pthreads;
- sets hard limits: 2 GiB address space, 30 CPU-seconds per process lifetime,
  64 file descriptors, 1 MiB per file, no core dumps; V8 additionally has a 64 MiB
  old-generation heap limit. Wasm uses explicit bounds checks, because its normal
  virtual-memory reservation conflicts with the address-space limit;
- bounds launcher/runtime startup to 30 seconds without a heartbeat. The worker
  sends its first heartbeat before importing guest code; thereafter a 2-second
  heartbeat gap kills an unresponsive event loop, including a hung initial import.
  Disposal gives cleanup callbacks 250 ms, then kills and reaps the process.
  Bubblewrap destroys the PID namespace when its parent exits.

This is a real OS restriction, not a guarantee against kernel or engine exploits.
The host's system libraries and its own runtime scripts are visible. Metadata and
timing side channels are not hidden. Limits are per worker; the embedding host must
also bound how many workers it activates. Host-mediated process grants deliberately
cross the boundary: only administrator-configured executable aliases can run; an
alias accepting arbitrary scripts is effectively an execution grant. Network
grants allow exact HTTPS origins, reject credentials and redirects, and bound time
and response bytes. The stock app host configures neither network origins nor
executables, and its permission UI truthfully marks those capabilities unavailable.

`trusted: true` without an OS backend is **full user-level code trust**. Node
`--permission` remains defense-in-depth only, not an adversarial security boundary.
Never use this option as the answer to a refused untrusted launch. Candidate native
macOS and Windows launchers must be built explicitly; missing launchers refuse.
The macOS Seatbelt/dual-supervisor implementation and its tests are described in
`MACOS.md`. Its 256 MiB physical-footprint watchdog is an enforced termination
budget, not an allocation-time memory cap. Windows AppContainer/Job delivery is
owned separately. Neither native backend has execution evidence from this Linux
orb, and Linux tests are not evidence of native platform parity.

## Protocol and ownership

`Session` owns one child; worker and supervisor exchange newline-delimited JSON.
`wire.mjs` caps bytes at 256 KiB **before JSON parsing**, including incomplete lines.
Output is rate-limited to 1 MiB/s; host request concurrency and outgoing buffers are
bounded. Native host queues are bounded as well. Trees have at most 1,000 nodes,
depth 32, unique semantic IDs and bounded text. Unknown component types/fields fail.

JS `gpui.mount` submits a data-only tree. GPUI entities, rendering, layout, input,
and accessibility stay in Rust. Every event carries generation and revision; old
events are refused. Reload starts a new process, validates its first tree, then
replaces the old process. Old callbacks cannot target the new generation. Initial
activation errors are not converted to empty views; reload errors retain the last
verified view. Stack traces retain module source filenames and line numbers.

`sdk.d.ts` describes the implemented API; `kit-sdk.d.ts` declares only registered
native adapters. `binding-coverage.json` records exact Rust source handles and
signatures, with unsupported components explicitly unbound. Columns, rows, text,
partial Button and the separately owned typed controls are **not the entire Kit
catalog**. Remaining bindings need real adapters and native tests, not generated
constructor names. Native state is reconciled by worker generation/business id;
removal/reload drops retained entities, while a rerender updates event routes.

Capabilities are requested in manifests and granted separately for each session.
Undeclared, denied, unsupported and unavailable operations reject promises.
Storage is rooted outside the package and partitioned by the host/plugin identity;
keys cannot be paths. Filesystem reads resolve beneath the package root, reject
traversal and outward symlinks, and only read bounded regular files. Same-user
tampering with host-owned store directories is outside the untrusted-code model.
Storage writes already authorized before disposal may finish; their replies are
discarded rather than delivered into a replacement generation.

Storage is capped at 1 MiB/128 keys per identity with serialized writes and a
64 KiB UTF-8 value cap. At most eight plugins can be active. These limits do not
replace an aggregate cgroup memory budget. Broker process aliases must resolve to
absolute host-selected binaries; their arguments never select an executable or a
shell. The binary runs under the same Linux OS boundary, so a grant cannot create
unbounded descendants or gain direct host filesystem/network access. There is no
unsandboxed broker-process fallback on other platforms. The stock host supplies
no executable/origin allowlists and truthfully disables those consent controls.

## Evidence and remaining work

The tests execute actual Node processes and a compiled native syscall probe, not
source-text assertions. They cover TS module loading, async state/events, cleanup,
stale revision/generation rejection, denied permissions, rooted reads, storage,
process broker argument separation, infinite loops, and protocol byte caps. The
native probe prints all syscall results and hard resource limits for review.

Native clipboard access is scoped by the framework's opaque EffectOwner token.
Only the trusted host supplies the per-generation read/write grants; stale and
missing owners fail closed. Native gestures do not bypass consent. The framework
propagation and Kit denial-safe cut/copy changes are independently owned
prerequisites, not a JS whitelist. Denied native operations request host consent;
the user must retry the operation after granting it.

The host optionally bundles pinned Node26.5.1 plus its license and offers a private
opt-in inspector evaluation socket. Not complete: full Kit bindings; executed
native macOS/Windows security parity; allocation-time macOS memory enforcement;
aggregate per-application resource budgeting; package authenticity/publisher
verification; signed platform installers; full debugger UI and source-map
transformation. These must not be represented as shipped support by a product.
