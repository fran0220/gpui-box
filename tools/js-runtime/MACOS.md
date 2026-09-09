# macOS candidate OS backend — native receipt required

Build on macOS with Xcode Command Line Tools, then run the native adversarial tests:

```sh
mkdir -p target
cc -Wall -Wextra -Werror -O2 tools/js-runtime/native/macos-launch.c -o target/gpui-sandbox-launch
npm --prefix tools/app-host ci
node --test tools/js-runtime/test/macos.test.mjs
GPUI_SANDBOX_LAUNCHER="$PWD/target/gpui-sandbox-launch" \
  node tools/app-host/cli.mjs dev tools/app-host/example-kit
```

The profile is default-deny Seatbelt, launched through the system `sandbox-exec`.
It allows package/runtime/system-library reads and file metadata; it does not
grant network, fork/spawn, writes to package/host files, or signals to other
processes. Existing stdio carries the bounded protocol. A missing/deprecated
`sandbox-exec`, invalid profile, unavailable launcher, or failed resource setup
refuses execution rather than selecting trusted mode.

The native launcher uses two trusted supervisors. The inner supervisor parents
the worker; both supervisors confirm worker identity before opening its execution
gate. Either supervisor dying causes the survivor to kill the worker. The outer
supervisor compares PID start timestamps before signaling, preventing PID-reuse
confusion. Worker exit is reaped by its parent. The inner watcher samples the
worker's `ri_phys_footprint` every 25 ms and kills it above 256 MiB, or when resource
measurement fails. CPU 30 seconds, 64 descriptors, 1 MiB file size and zero core
size are hard inherited kernel rlimits.

**The memory budget is enforced termination, not a kernel allocation-time cap.**
The worker can overshoot between samples. macOS RSS rlimits are advisory and are
not used as the security claim. Hosts requiring a strict allocation-time memory
ceiling must refuse this backend or use a separately resource-capped VM. This
is a real limitation versus Linux's address-space limit and Windows Job memory
limit, not equivalent resource guarantees.

The native tests compile a raw syscall probe, check read/write/socket/fork/spawn
denial and working threads, check the inherited limits, force a touched native
allocation over budget, kill the outer supervisor, reject a malformed profile,
and execute/dispose an actual TypeScript worker. Linux skips these tests; a skip
is not evidence. The implementation has not yet been executed on macOS in this
thread and must pass the parent's Platforms lane before a supported-lane claim.
