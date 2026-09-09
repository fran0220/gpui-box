# Windows developer debug transport

Build from Windows PowerShell or PowerShell 7 before running the app host tests:

```powershell
& tools/app-host/native/build-windows-debug.ps1
node --test tools/app-host/test/debug.test.mjs
```

The script uses the inbox x64 .NET Framework 4.x compiler and sets
`GPUI_DEBUG_PIPE_HELPER` in that PowerShell process. Its default output is
`target/app-host-windows/gpui-debug-pipe.exe`; `-OutputDirectory` overrides it.
Distributions must ship that helper and set its absolute path in
`GPUI_DEBUG_PIPE_HELPER`. There is no insecure fallback when it is missing.
`gpui-app build` on Windows includes the configured helper automatically and
refuses a missing helper before creating its output directory. `--debug-helper
PATH` selects an explicit build artifact. The package stores it at
`runtime/gpui-debug-pipe.exe`; its launchers set `GPUI_DEBUG_PIPE_HELPER` relative
to the launcher's own directory, so relocating the package preserves the
absolute runtime path. `build-info.json` records the included helper name.
The helper invokes Windows kernel named-pipe APIs; it is not a TCP service.

`debugEndpoint(data)` is a deterministic address derived from the absolute data
path (case-folded on Windows), not a credential. `CreateNamedPipe` atomically
installs a protected DACL containing only the current user's SID with full
control, sets that SID as owner, rejects remote clients, forbids inheritance of
the handle, and requires the first/only server instance. Startup reads back the
actual kernel ACL and refuses any other grants. No Everyone, administrator
group, AppContainer, or ALL APPLICATION PACKAGES grant is added. As with a POSIX
0700 directory/0600 socket, privileged administrators can override OS security;
this is not an isolation boundary against the owner or administrators.

Before forwarding a request, the helper impersonates the connected client and
checks its token user SID matches the owner. The native `probe-denied` command
uses a token restricted to Everyone to require Windows `ERROR_ACCESS_DENIED`,
not a connection error or a JavaScript refusal. `probe-anonymous` opens as the
owner but withholds an impersonation identity; it must receive no evaluation
response. The test also compares the actual pipe owner with `whoami /user`.
These checks must run on Windows; successful C#
cross-compilation on Linux is not Windows ACL execution evidence.

Each connection carries one UTF-8 JSON line, at most 256 KiB including its LF.
Windows serves one client at a time. Client IO and helper response waits are
bounded to five seconds, with one additional second to drain cancelled IO before
reusing the first-instance handle; evaluation has a 3.5-second deadline and receives an
AbortSignal as the second argument's `signal`. Evaluators must honor that signal
to cancel underlying work; the transport cannot preempt arbitrary synchronous
JavaScript. `evaluateDebug(..., {signal})` closes its connection on cancellation.
On Windows a disconnected evaluator is aborted by the evaluation deadline or
server shutdown, not immediately by pipe disconnect notification.

Closing the server cancels evaluation and closes helper stdin. EOF interrupts
idle accept and client reads. A five-second cleanup deadline terminates a stuck
helper and **rejects** `close()`; unexpected helper exit rejects `closed` and
subsequent `close()`. Hosts can observe `closed` to surface transport failure.
The kernel removes the endpoint when the last handle closes, including after
host/helper crashes. POSIX retains the private-directory/socket mode checks.
