# Windows native sandbox candidate

This is a Windows 10+ / Windows Server 2016+ native implementation, not a
Node permission-model substitute. Windows-native test execution is required
before claiming support. Cross-compilation on Linux proves compilation/linkage,
not AppContainer compatibility, Node startup, or enforcement.

## Build and run the native tests

Use pinned **Node 26.5.1** and a Visual Studio 2022 developer PowerShell, or
MinGW-w64 GCC on PATH:

```powershell
& tools/js-runtime/native/build-windows.ps1
node --test tools/js-runtime/test/windows.test.mjs
```

Run both commands in the same PowerShell process: the build script sets
`GPUI_SANDBOX_LAUNCHER` and `GPUI_WINDOWS_SANDBOX_PROBE`. Native tests fail if
either required executable is missing; non-Windows runs explicitly skip them.
The CPU exhaustion test can take over two minutes on a one-vCPU machine.
The real-worker test additionally needs the integrated runtime's `worker.mjs`
and its modules; this Windows-only delivery does not duplicate those files.
The dispatch-only Platforms lane owns Windows execution after integration.

Linux cross-compilation (MinGW-w64; no Wine claim):

```sh
mkdir -p target/js-runtime-windows
x86_64-w64-mingw32-gcc -std=c11 -O2 -Wall -Wextra -Werror -municode \
  tools/js-runtime/native/windows-launch.c \
  -o target/js-runtime-windows/gpui-sandbox-launch.exe \
  -luserenv -ladvapi32 -lrpcrt4 -lws2_32 -lole32
x86_64-w64-mingw32-gcc -std=c11 -O2 -Wall -Wextra -Werror -municode \
  -DGPUI_SANDBOX_PROBE tools/js-runtime/native/windows-launch.c \
  -o target/js-runtime-windows/windows-probe.exe \
  -luserenv -ladvapi32 -lrpcrt4 -lws2_32 -lole32
```

Only the launcher ships, as `runtime/gpui-sandbox-launch.exe`. Packaging sets
`GPUI_SANDBOX_LAUNCHER`; source-development hosts can supply the packaged
host's `--sandbox-launcher` option. The probe is test-only.

## Launch and per-instance ownership

`await windowsSandbox(root, runtimeRoot, { executable, node, launcher })`
returns `{ execPath, execArgv, stdio: ['pipe','pipe','pipe'], cleanup }`.
Defaults are `process.execPath`, `true`, and `GPUI_SANDBOX_LAUNCHER`.
It copies ordinary package/runtime files and the executable to one unique
temporary directory. Links/junctions and other special entries are rejected.
The host must keep source trees stable during staging; hostile processes already
running with the host user's authority are outside this containment boundary.

The exact helper invocation is:

```text
gpui-sandbox-launch.exe --instance <private-copy-directory>
  --root <original-package-root> --runtime <original-runtime-root>
  --parent <host-process-id> --profile <gpui-js-UUID>
  -- <Node options> <worker> <entry> <generation> <mode>
```

The helper remaps original absolute package/runtime path arguments and
`--allow-fs-read=<path>` to the private copies (longest prefix wins). It quotes
each argument using Windows CRT rules. Debug's `--allow-inspector` stays before
the worker. Relative imports continue to work; absolute host paths embedded
inside package code do not become grants.

Before launch, the helper applies protected ACLs recursively to the **copies
only**: SYSTEM and the host user retain full access; one random AppContainer SID
gets read/execute. It never changes ACLs on package sources, the installed Node,
the original runtime, Windows system files, or existing user files. Reparse points
are checked again before ACL changes. No broad group gets a new grant.

The host allocates a unique profile name; the helper provisions it with
`CreateAppContainerProfile`, never reusing an existing profile. Merely deriving
a SID does not provision the documented unpackaged AppContainer environment.
The newly-created profile directory is protected with the same RX-only ACL
before any worker starts. No capability SIDs or network exemptions are added.
Windows also creates per-instance registry profile state; this is not a claim
of a filesystem/registry namespace with no backing state.
The child receives a clean environment containing only
Windows directory variables and `NODE_NO_WARNINGS`. An explicit handle list
passes only duplicated standard pipes; no job or host-process handle leaks.
`DETACHED_PROCESS` avoids requesting an invisible console. Staging paths are
expanded to long backslash paths, and image/cwd existence plus host image-open
are checked before launch. On failure, diagnostics report those exact paths,
attributes, creation flags and the original Win32 error, without dumping
payload arguments or host environment values.

`PROC_THREAD_ATTRIBUTE_JOB_LIST` assigns the job atomically at process creation.
The job handle is private to the helper, so killing the helper also kills its
worker, including during startup. The helper waits on both the worker and the
host process; host death also closes the job and kills the worker. Any failed
security API aborts launch with exit code 125; there is no unrestricted fallback.

The session **must call `cleanup()` after the child `close` event**, including
launch failure, and before resolving stop/exit. Do not call it from `afterSpawn`.
The helper kills/reaps its worker before deleting its owned AppContainer
profile, including normal failure and observed host death. Deferred host
cleanup invokes `--delete-profile <gpui-js-UUID>` as well, covering abrupt
helper death, then removes staging. Cleanup is idempotent and reports profile
deletion refusal; it does not silently report successful cleanup.
An abrupt death of both host and helper, OS crash, or power loss can leave
read-only copies and registered per-instance profile state. No worker survives
job-handle closure, and no source ACL grant was made. Such crash leftovers need
host maintenance; the host must never reuse an old staging/profile identity.

The first native lane returned error 2 from `CreateProcessW` for both C probes
and Node. Profile provisioning, console mode and staging path interpretation
are shared launch concerns; the old log did not identify which lookup failed.
These corrections remain candidates until native execution confirms startup
and all containment assertions. The test ACL reader now calls Win32 APIs from
the native probe, so missing PowerShell modules cannot erase ACL assertions.

## Resource semantics and remaining differences

- The process and job each permit **256 MiB committed memory**, not 256 MiB
  virtual address space or resident memory. A failed allocation is refused;
  Windows need not terminate a process which handles allocation failure.
  Node additionally uses a 64 MiB old-generation heap limit.
- **30 seconds of accumulated user-mode CPU time** terminates the process.
  This is not elapsed time and does not include kernel time. A **25% rate hard
  cap** additionally bounds CPU scheduling per interval across all processors,
  not 25% of one core. These semantics differ from Linux `RLIMIT_CPU`.
- **One active process** and no breakaway flag prohibit child creation while
  preserving threads. **Kill-on-job-close** provides non-cooperative teardown.
- Windows Jobs have no direct `RLIMIT_NOFILE` or `RLIMIT_FSIZE` counterpart.
  The sandbox grants no writable package/runtime storage. Protocol/output byte
  limits remain enforced by the shared host and are not kernel file-size limits.
- A standard AppContainer retains Windows' baseline access to system resources
  and files already granted to all application packages. It is **not a private
  filesystem namespace** and does not promise that every host pathname is
  invisible. It denies ordinary host-private data and grants no additional host
  data access. LPAC, broad denial ACLs on the host, and network capabilities are
  deliberately not substituted to make startup/tests pass.

Tests include a raw native probe (AppContainer token, zero capabilities, job
configuration, denied host read/write, live-listener network denial, normal and
breakaway spawn denial, inherited-handle leakage, and allocation exhaustion),
actual CPU exhaustion, argument/path mapping, environment clearing, real Node
TypeScript worker and isolated inspector execution, junction rejection,
unchanged source ACLs, host/helper-death teardown and deferred directory cleanup.
Passing skipped tests on Linux does not establish Windows parity.

API references: [AppContainer launch](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer),
[Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects),
[CPU rate control](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_cpu_rate_control_information).
