# Native browser host

`gpui-box-webview` is an independent caller-owned host, not a Kit transport or
the application/plugin JavaScript runtime. `BrowserHost` owns Wry 0.57.0 and
the operating-system browser: WKWebView on macOS, WebView2 on Windows,
WebKitGTK 4.1 on Linux X11/XWayland. These engines supply full HTML, CSS,
JavaScript, DOM, text selection and native keyboard/IME behavior. Their web
standards/codec coverage follows the installed engine, not a GPUI HTML subset.

Kit's BrowserPanel should receive caller-owned URL/loading/error/history data
and emit navigation/back/forward/reload actions. Its host maps actions to
`BrowserHost`, drains events on the GPUI UI thread, and mounts
`gpui::platform_view(host.handle())` as the browser content. Neither Kit nor
this package imports the shell/plugin runtime. Never send page IPC directly to
that privileged runtime.

```rust,ignore
let browser = gpui_webview::BrowserHost::new(window, cx, Default::default())?;
browser.navigate("https://example.com")?;
// Retain browser in caller state; in Render:
gpui::platform_view(browser.handle()).size_full()
```

## Linux route and native constraints

Run an X11 GPUI window, including XWayland under a Wayland desktop. Launch with
`env -u WAYLAND_DISPLAY GDK_BACKEND=x11 cargo run -p gpui-box-webview --example browser`.
`DISPLAY` must address the same X server for GPUI and GDK. Install
`libwebkit2gtk-4.1-dev` (GTK3/WebKitGTK 4.1 development dependencies) on Debian
or Ubuntu. `.agents/setup` includes it. No WebKitGTK libraries enter the
headless workspace or the Kit dependency graph.

Wry accepts Xlib handles whereas GPUI exposes Xcb: the adapter passes the same
server XID as an Xlib handle. GTK initialization and a bounded GLib iteration
task run on GPUI's foreground thread, including when GPUI has no input events.
The task is cancelled when the last native view owner is released. GTK's
foreign toplevel requires an explicit allocation callback; changing only the
X window size leaves the engine viewport at its initial dimensions. The
framework passes the full physical viewport to the host's toolkit allocator.

Native Wayland embedding is **rejected**, not an inert successful WebView.
WebKitGTK's Wayland API requires a GTK container; it cannot insert a GTK
toplevel into GPUI's independently owned raw Wayland surface. XWayland is the
supported product route. A native Wayland route needs either a framework GTK
window/backend with GTK-owned containment, or an offscreen browser engine
(for example CEF OSR or WPE WebKit) plus full texture/damage, input, IME,
accessibility, popup and lifecycle integration. Merely obtaining pixels is not
a complete platform-view implementation. Neither alternative is implemented.

Native views sit above GPUI's base GPU surface. The framework's macOS/Windows
scene-overlay surfaces can draw above them; X11 has no equivalent GPUI overlay
surface. Hide/unmount the browser when a GPUI overlay must cover its area.
Only rectangular clipping is supported: the native viewport keeps full bounds
and an independent clip parent limits both pixels and input. Rotation,
nontrivial scale transforms, arbitrary opacity, rounded masks, and arbitrary
GPU/native z-order interleaving are not promised. Do not apply them. Resize and
DPI changes update physical bounds. Do not move one host between GPUI windows.

Unmounting detaches the native child. Handles retain engine/controller state
until the platform layer has finished detaching. Dropping the caller's host
alone cannot destroy a still-painted child. Focus delegates to native controls;
`focus_parent()` returns to the GPUI window. GPUI keyboard shortcuts do not
automatically intercept keys consumed by a native browser/IME.

## Security and truthful events

Navigation defaults to HTTP(S); file/data/javascript URLs, credentials in URLs,
and malformed URLs are refused. An optional normalized origin allowlist checks
origins, not string prefixes. This is a navigation policy, **not** a subresource
network sandbox. `load_html` is caller-owned HTML with no application base URL.
Scripts run with ordinary web engine privileges, never shell/plugin authority.

Popups, downloads and permissions are denied and reported. They do not
implicitly launch system applications, select a download path, grant devices,
or persist permission grants. Wry's permission hook has no requesting-frame
origin, so this package does not offer misleading origin-aware permission
grants. On macOS Wry covers camera/microphone only; OS/browser policies still
govern APIs not surfaced by Wry (including clipboard). User-facing asynchronous
approval with origin/gesture identity requires native request responders, not
the current synchronous kind-only Wry hook.

IPC is absent by default. Opt-in messages are untrusted data and are limited to
64 KiB per message. Event delivery is bounded to 256 queued events and never
blocks native callbacks. `EventsDropped` reports overflow; mark state unverified
instead of inferring success from an incomplete stream. The reported URL is
informational: Linux Wry substitutes the main-frame URL for iframe senders.
Never authorize commands from it. This is
not a capability bridge, per-origin RPC mechanism, or defense against all
resource exhaustion by an adversarial page. Host-authored `evaluate_script`
must not interpolate untrusted input as JavaScript source.

Command success means the native command was accepted, not that a page loaded.
`PageFinished` is not a success assertion. Linux native load/TLS failure and
process termination signals, Windows NavigationCompleted error status, and a
macOS forwarding WKNavigationDelegate report errors independently of Wry's
completion hook. The forwarding delegate preserves Wry's policy callbacks.
Hosts must retain a failure rather than overwrite it on PageFinished; a new
navigation may clear the error. The native reported URL can be empty or the
last committed URL during provisional failures. Navigation cancellation can
also produce a native failure. WebView2 ProcessFailed reports native process
failures. History availability is queried from the engine, not inferred from
a caller-created URL stack.

## Reproduce validation

```sh
cargo test -p gpui-box-webview
cargo test -p gpui-box --features test-support platform_view
xvfb-run -a cargo test -p gpui-box-linux x11_native_children -- --ignored
env -u WAYLAND_DISPLAY GDK_BACKEND=x11 xvfb-run -a cargo run -p gpui-box-webview --example smoke
cargo run -p xtask -- dependencies check
env -u WAYLAND_DISPLAY GDK_BACKEND=x11 cargo run -p gpui-box-webview --example browser
```

The example is explicit fixture HTML, not live product data. Review CSS grid,
gradient, rounded cards, text selection, typing/IME, navigation, back/forward,
reload, popup refusal, hide/show, resize, focus return and Offline error. Native
browser surfaces are not part of GPUI's offscreen scene texture and cannot be
proven by the existing headless screenshot catalog. Capture the native desktop
for this example; never accept it as a headless scene baseline.

macOS and Windows need their native compile/test lanes and a logged-in native
host to validate WK delegate forwarding, WebView2 runtime installation, focus,
IME, accessibility bounds and native overlays. A Linux Xvfb pass proves none of
those. Cross-platform source support is not a claim of completed native evidence.

The dispatch-only `Platforms` workflow executes
`cargo run --locked -p gpui-box-webview --example smoke` in both native jobs:
WKWebView on `macos-15`, WebView2 on `windows-2025`. It needs a usable native
desktop/event loop and loopback networking; Windows additionally needs the
installed WebView2 runtime. Missing or broken runtime/desktop initialization is
a failure, not a skipped success. No Linux GTK/Xvfb setup belongs in these jobs.
The in-app deadline is 30 seconds after host creation; the workflow step's
10-minute timeout also bounds compilation and native initialization hangs.
Always-upload artifacts `webview-smoke-macos` and `webview-smoke-windows` retain
combined command output, platform/compiler metadata, engine user agent, phase
events and failure backtraces (partial logs on timeout). Native execution is
only certified when that OS's command exits successfully and its log contains
`native browser smoke passed`; compilation or this workflow wiring alone is
not evidence. A failure in an earlier job step can prevent smoke execution.

Smoke checks initial DOM CSS-grid/viewport metrics, opt-in IPC and script
evaluation, HTTP navigation, back/forward, reload, file-URL policy refusal and
a real closed-loopback-port native load failure. Each successful navigation
waits for both page-script readiness and native completion, in either order,
before advancing, avoiding cancellation failures from overlapping loads.
Unexpected early load/process failures and event loss fail the smoke.
It does **not** certify rendered pixels, clipping/stacking, dynamic resize/DPI,
hide/show/detach/destruction, keyboard focus/IME, accessibility, TLS errors,
process-fault recovery, popup/download/permission callbacks, or hostile iframe
IPC/security behavior. Those need separate native interaction/fault tests and
desktop inspection; OS-specific permission-hook limitations above remain.

The Linux orb's inspected native capture shows all three CSS grid cards,
gradient/rounded corners, authored keyboard input and an untrusted-message
result. Under this Xvfb session the GPUI GPU surface itself is invisible (the
unchanged `hello_world` control has the same symptom, with and without an X11
compositor/window manager). Native browser pixels and X11 geometry/input tests
are therefore evidence; a correct GPUI-toolbar/browser composite and real IME
are still desktop validation requirements. No native desktop capture is used
as a headless baseline.
