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
netsh wfp capture start keywords=19
if ($LASTEXITCODE -ne 0) { throw 'WFP capture start failed' }
try {
    node --test tools/js-runtime/test/windows.test.mjs
    if ($LASTEXITCODE -ne 0) { throw 'Windows native tests failed' }
} finally {
    netsh wfp capture stop
}
```

Run both commands in the same PowerShell process: the build script sets
`GPUI_SANDBOX_LAUNCHER` and `GPUI_WINDOWS_SANDBOX_PROBE`. Native tests fail if
either required executable is missing; non-Windows runs explicitly skip them.
Use a disposable native test machine with permission to capture/read WFP
diagnostics. Do not start another capture if the Platforms lane already owns
one. TCP timeout acceptance requires live event/filter readback; missing
diagnostics fail the test rather than treating the timeout as isolation.
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
  -luserenv -ladvapi32 -lrpcrt4 -lws2_32 -lole32 -lshell32 -luuid
x86_64-w64-mingw32-gcc -std=c11 -O2 -Wall -Wextra -Werror -municode \
  -DGPUI_SANDBOX_PROBE tools/js-runtime/native/windows-launch.c \
  -o target/js-runtime-windows/windows-probe.exe \
  -luserenv -ladvapi32 -lrpcrt4 -lws2_32 -lole32 -lshell32 -luuid
```

Only the launcher ships, as `runtime/gpui-sandbox-launch.exe`. Packaging sets
`GPUI_SANDBOX_LAUNCHER`; source-development hosts can supply the packaged
host's `--sandbox-launcher` option. The probe is test-only.

## Launch and per-instance ownership

`await windowsSandbox(root, runtimeRoot, { executable, node, launcher })`
returns `{ execPath, execArgv, stdio: ['pipe','pipe','pipe'], detached: true, cleanup }`.
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
Existing absolute arguments are expanded from 8.3 aliases before prefix matching;
arguments outside both source roots retain their exact original spelling.
Node preserves symlink paths for main and imported modules because the staged
trees reject links. This avoids its realpath walk through denied host ancestors;
it does not grant access to drive roots or user directories.

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
The child receives an explicit environment containing only Windows directory
variables, `NODE_NO_WARNINGS`, and the documented AppContainer bootstrap keys
`LOCALAPPDATA`, `TEMP`, `TMP`. The launch-input `LOCALAPPDATA` is obtained from
`SHGetKnownFolderPath(FOLDERID_LocalAppData)`: Windows appends the private
`Packages/<moniker>/AC` itself. Pre-appending it caused a duplicated path in
actual native execution. TEMP/TMP are seeded with the private `AC/Temp`.
No host environment block or host directory ACL grant is inherited. The probe
requires the resulting child paths to equal its API-derived AC and AC/Temp,
to exist, and to deny writes with ACCESS_DENIED.
The profile root is bounded to the host's `FOLDERID_LocalAppData` plus
`Packages/<fresh UUID moniker>`, verified absent before provisioning. Only that
new root receives recursive ACL changes. The identity-dependent
`GetAppContainerFolderPath` result is not authority for host-side ACL changes
or post-deletion existence tests. Host LocalAppData and Packages ACLs are tested
for exact preservation, including when the adversarial probe fails.
The temp directory is created before applying the RX-only ACL; the native
probe requires the resulting environment paths to remain inside the profile
and rejects writes through all three. An explicit handle list
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
The host must pass `config.detached` to its spawn of the **trusted helper**,
without unref or changing pipes. Node 26.5.1 otherwise assigns that helper to
its own kill-on-close Job and forcibly kills it on host death, preventing
profile cleanup. Detached skips that libuv assignment; the helper still watches
the parent and the untrusted worker still joins the atomic sandbox Job.

The session **must call `cleanup()` after the child `close` event**, including
launch failure, and before resolving stop/exit. Do not call it from `afterSpawn`.
The helper kills/reaps its worker before deleting its owned AppContainer
profile, including normal failure and observed host death. Deferred host
cleanup invokes `--delete-profile <gpui-js-UUID>` as well, covering abrupt
helper death, then removes staging. Cleanup is idempotent and reports profile
deletion refusal; it does not silently report successful cleanup.
Even a successful deletion API result must leave the exact profile root absent.
The helper retries the API for at most two seconds while storage remains and
reports failure if it persists; it never recursively deletes a guessed path.
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

The next actual run, [34396298822](https://github.com/fran0220/gpui-box/actions/runs/34396298822)
at source [02468534](https://github.com/fran0220/gpui-box/commit/02468534f3863e1f627559808f55128eb8bc8c4d),
reached `CreateProcessW` with existing staged executable/cwd but failed with
203 (`ERROR_ENVVAR_NOT_FOUND`) for both valid and invalid images. Its environment
contained only `NODE_NO_WARNINGS,SystemRoot,WINDIR`. The explicit profile-backed
bootstrap entries above are a candidate correction based on the documented
AppContainer environment rewrite, not a natively verified fix. Failure logs
retain the exact supplied key list and original Win32 error. Tests also now
extract `--profile`/`--instance` values by flag: the former positional profile
index accidentally selected `--`, corrupting teardown verification.

## Resource semantics and remaining differences

Actual run [34406540756](https://github.com/fran0220/gpui-box/actions/runs/34406540756)
at [8939172a](https://github.com/fran0220/gpui-box/commit/8939172a6c3ae1c18f0cb22c8989d14d280413f3)
launched native payloads: committed-allocation refusal, helper-death teardown,
and reparse rejection passed. It did **not** pass the Windows sandbox suite.
The candidate now corrects short-path mapping and Node ancestor resolution;
the CPU test expects native `STATUS_QUOTA_EXCEEDED`, with actual process user
time accounting, instead of a Win32 error mapping. Invalid images can report
193 or 216; they must still execute nothing and leave no profile. Environment
write denial and profile removal assertions remain strict. Exact environment
key/path/attributes and bounded profile-path diagnostics distinguish missing
directories from access denials and actual leftovers from identity lookup errors.
These corrections require a new native run; Linux compilation is not a pass.

Run [34413155692](https://github.com/fran0220/gpui-box/actions/runs/34413155692)
at [0703a7a7](https://github.com/fran0220/gpui-box/commit/0703a7a78bda414191822a19c9d3aa9c452bfbbe)
passed real Node argument mapping, TS rendering/inspector denial, CPU status
and measured 30-second user budget, allocation refusal, helper-death cleanup,
invalid-image cleanup and reparse rejection. The combined adversarial probe
failed at its first environment write: LOCALAPPDATA contained duplicated
`Packages/<moniker>/AC`, so later raw filesystem/network/spawn/handle assertions
were not reached. Source and host parent ACL equality passed before that failure.
The host-death test observed both processes gone but the exact profile root
still present. Its log does not establish why removal failed. The bootstrap
correction and deletion-postcondition retry above are candidates, not verified
native fixes; both strict assertions must pass on a subsequent native run.

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

Run [34415773540](https://github.com/fran0220/gpui-box/actions/runs/34415773540)
at [101cffd3](https://github.com/fran0220/gpui-box/commit/101cffd3134a0a8d41b9b5445897dfe6567b9f47)
proved exact environment paths and ACCESS_DENIED writes, host filesystem/ACL
denial, and normal/breakaway spawn denial. TCP loopback returned 10060, so UDP
and inherited-handle assertions were not reached; the raw probe remained red.
The native probe now runs handle assertions first and logs package SID, PID,
destination port and exact Winsock error. An unsandboxed native positive control
must connect to the same listener, with exactly one host-observed connection.
The network assertion still requires WSAEACCES: timeout alone is not accepted.

The Windows workflow captures WFP events with `netsh wfp capture start keywords=19`
and stops in `finally`, retaining CAB and extracted evidence under
`runtime-native-windows/wfp`. This is diagnostic capture only, not firewall,
capability or loopback-exemption configuration. Correlate the logged package SID
and port with CLASSIFY_DROP and its filter before attributing a timeout to
AppContainer policy. Documentation describing receive-layer loopback drops is
not evidence that this specific run hit one.

Run [34419428965](https://github.com/fran0220/gpui-box/actions/runs/34419428965)
at [ed009d5f](https://github.com/fran0220/gpui-box/commit/ed009d5f6ec285410d4cc391eb97536fd65d6604)
passed detached host-death cleanup, including exact profile-root absence, in
both raw and combined suites. CAB/XML capture and extraction also executed.
The raw probe stopped at its blanket rejection of disk handles, before network
connect. That assertion did not distinguish Windows-created cwd/image handles
from the intentionally inherited host sentinel; the log did not identify the
particular handle. The revised check logs disk paths, flags and file identities,
rejects the sentinel identity even if INHERIT was cleared, and rejects every
inheritable disk handle. Native clean and deliberately leaking controls must
respectively pass and fail with the exact sentinel assertion. The production
stdio-only handle allowlist is unchanged; this test correction awaits native
execution.

The captured XML contains no events matching either sandbox probe SID
(PIDs 1660 and 7832, destination ports 62271 and 62292). Those ports have
PUBLIC_CLASSIFY_ALLOW events for the unsandboxed native positive controls
(PIDs 5076 and 988) and host listeners. They are not sandbox drop evidence.
Network remained unverified in that run.

Run [34680808729](https://github.com/fran0220/gpui-box/actions/runs/34680808729)
at [3d0fe1ec](https://github.com/fran0220/gpui-box/commit/3d0fe1eccbb3aaa2a41144af5a8eab9aa14f2381)
passed filesystem, spawn and handle checks inside AppContainer. Its captured
WFP XML binds both raw and combined probes to outbound allows by package SID,
image and TCP tuple, followed by reversed inbound drops at the live listener.
The causal filter is `AppContainerLoopback`, in
`FWPM_SUBLAYER_MPSSVC_APP_ISOLATION`, with `FWP_ACTION_BLOCK`. This is evidence
for those attempts, not blanket acceptance of error 10060.

The current probe reports TCP error and explicitly bound source port instead
of claiming `networkDenied`. Error 10013 remains direct denial; error 10060
requires fresh live WFP records matching the attempt's time interval, worker
SID/image, full TCP tuple, listener image and isolation blocking filter.
PIDs are checked when the event schema supplies them. An unsandboxed native
connection must succeed both before and after the attempt. `windows-wfp.ps1`
only reads diagnostics and never changes collection,
policy, capabilities or exemptions. Its XML dumps remain under
`target/runtime-native/wfp`. Captured XML parser tests do not prove live
readback works on a particular Windows kernel; the native lane must pass.

Run [34683324192](https://github.com/fran0220/gpui-box/actions/runs/34683324192)
passed strict handle readback/wait controls and all six low-rights controls,
but `sendto` to the original non-loopback `192.0.2.1:9` returned success.
Its captured XML proves both raw and combined sends hit the outbound
`UWP Default Outbound Block Rule` in `MPSSVC_APP_ISOLATION`, at
`ALE_AUTH_CONNECT_V4`. The earlier allow event was socket resource assignment,
not permission to send to that destination. Successful `sendto` is not
successful delivery, as its [Microsoft API contract](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-sendto) explicitly states.

The probe retains TEST-NET-1, reports UDP result and a bound source port, and
does not assert delivery or denial from API success. Success requires a fresh
protocol-17 outbound non-loopback drop matching the worker SID/image, source
port, a host interface address, destination and time interval, resolving to
an AppContainer-isolation blocking filter at the connect layer. A direct
10013 refusal remains valid. Missing evidence fails; an unrelated firewall
drop or socket-allocation event cannot pass. This is local outbound enforcement,
not remote reception or a substitute loopback-only check. Live readback of both
TCP and UDP remains required before accepting the native lane.

Strict handle checks read back both mitigation bits. Separate controls compare
a valid event wait, a closed-event wait without strict policy, and a
closed-event wait with strict policy, which must raise `STATUS_INVALID_HANDLE`.
The prior double-`CloseHandle` control did not raise on this kernel; it did not
establish the behavior of the documented invalid-reference wait path. All six
low-rights high-handle snapshot controls run independently of that control.

API references: [AppContainer launch](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer),
[Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects),
[CPU rate control](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_cpu_rate_control_information).
