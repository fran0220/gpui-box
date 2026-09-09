use crate::{BrowserEvent, NavigationPolicy};
use gpui::{App, PlatformViewHandle, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    rc::Rc,
    sync::mpsc::{self, Receiver, Sender},
};
use wry::{WebView, WebViewBuilder};

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod platform;

/// Security defaults: HTTP(S) navigation, no popup, download, permission grants
/// or script bridge. HTML is rendered by the OS browser, not GPUI's text parser.
#[derive(Default)]
pub struct BrowserOptions {
    pub navigation: NavigationPolicy,
    /// Opt in to receiving untrusted messages. Never authorizes native actions.
    pub receive_messages: bool,
}

struct Engine {
    _platform: platform::Lifetime,
    view: WebView,
}

/// A UI-thread-only, caller-owned browser. The handle retains its engine until
/// GPUI has detached the last painted view. Never move it between GPUI windows.
pub struct BrowserHost {
    engine: Rc<Engine>,
    handle: PlatformViewHandle,
    events: Receiver<BrowserEvent>,
    sender: Sender<BrowserEvent>,
    navigation: NavigationPolicy,
}

impl BrowserHost {
    /// Creates a child on the window's UI thread. Linux requires an X11 GPUI
    /// window (including XWayland) and GTK using the same X server. Native
    /// Wayland is rejected, never silently replaced with an inert handle.
    pub fn new(window: &Window, cx: &App, options: BrowserOptions) -> anyhow::Result<Self> {
        let raw = HasWindowHandle::window_handle(window)?.as_raw();
        anyhow::ensure!(
            matches!(
                raw,
                RawWindowHandle::AppKit(_)
                    | RawWindowHandle::Win32(_)
                    | RawWindowHandle::Xlib(_)
                    | RawWindowHandle::Xcb(_)
            ),
            "native WebView requires AppKit, Win32, or X11/XWayland; raw Wayland embedding is unavailable"
        );
        platform::initialize()?;
        let (sender, events) = mpsc::channel();
        let send = sender.clone();
        let policy = options.navigation.clone();
        let mut builder = WebViewBuilder::new().with_navigation_handler(move |url| {
            // about:blank is needed for the initial empty document and
            // caller-supplied HTML. It never grants IPC privileges.
            let allowed = url == "about:blank" || policy.allows(&url);
            let event = if allowed {
                BrowserEvent::NavigationStarted(url)
            } else {
                BrowserEvent::NavigationRefused(url)
            };
            let _ = send.send(event);
            allowed
        });
        let send = sender.clone();
        builder = builder.with_on_page_load_handler(move |event, url| {
            if matches!(event, wry::PageLoadEvent::Finished) {
                let _ = send.send(BrowserEvent::PageFinished(url));
            }
        });
        let send = sender.clone();
        builder = builder.with_new_window_req_handler(move |url, _| {
            let _ = send.send(BrowserEvent::PopupRefused(url));
            wry::NewWindowResponse::Deny
        });
        let send = sender.clone();
        builder = builder.with_download_started_handler(move |url, _| {
            let _ = send.send(BrowserEvent::DownloadRefused(url));
            false
        });
        let send = sender.clone();
        builder = builder.with_permission_handler(move |kind| {
            let _ = send.send(BrowserEvent::PermissionRefused(format!("{kind:?}")));
            wry::PermissionResponse::Deny
        });
        if options.receive_messages {
            let send = sender.clone();
            builder = builder.with_ipc_handler(move |request| {
                let event = if request.body().len() <= 65_536 {
                    BrowserEvent::Message {
                        reported_url: request.uri().to_string(),
                        body: request.into_body(),
                    }
                } else {
                    BrowserEvent::MessageRefused
                };
                let _ = send.send(event);
            });
        }
        let (view, handle, lifetime) = platform::build(builder, raw, sender.clone(), cx)?;
        let engine = Rc::new(Engine {
            view,
            _platform: lifetime,
        });
        Ok(Self {
            handle: handle.keep_alive(engine.clone()),
            engine,
            events,
            sender,
            navigation: options.navigation,
        })
    }

    /// Render with `gpui::platform_view`. Rectangular clipping only; no rotation,
    /// scaling, rounded masks, opacity, or GPU-scene interleaving is promised.
    pub fn handle(&self) -> PlatformViewHandle {
        self.handle.clone()
    }

    /// Drains notifications without reentering GPUI from a native callback.
    pub fn drain_events(&self) -> impl Iterator<Item = BrowserEvent> + '_ {
        self.events.try_iter()
    }

    pub fn navigate(&self, url: &str) -> anyhow::Result<()> {
        if !self.navigation.allows(url) {
            let _ = self
                .sender
                .send(BrowserEvent::NavigationRefused(url.into()));
            anyhow::bail!("navigation refused by host policy");
        }
        Ok(self.engine.view.load_url(url)?)
    }

    /// Loads caller-owned HTML with the engine's complete HTML/CSS support.
    /// Relative URLs have no application base URL. Content is not trusted by IPC.
    pub fn load_html(&self, html: &str) -> anyhow::Result<()> {
        Ok(self.engine.view.load_html(html)?)
    }
    pub fn back(&self) -> anyhow::Result<()> {
        Ok(self.engine.view.go_back()?)
    }
    pub fn forward(&self) -> anyhow::Result<()> {
        Ok(self.engine.view.go_forward()?)
    }
    pub fn reload(&self) -> anyhow::Result<()> {
        Ok(self.engine.view.reload()?)
    }
    pub fn can_go_back(&self) -> anyhow::Result<bool> {
        Ok(self.engine.view.can_go_back()?)
    }
    pub fn can_go_forward(&self) -> anyhow::Result<bool> {
        Ok(self.engine.view.can_go_forward()?)
    }
    pub fn url(&self) -> anyhow::Result<String> {
        Ok(self.engine.view.url()?)
    }
    pub fn focus(&self) -> anyhow::Result<()> {
        Ok(self.engine.view.focus()?)
    }
    pub fn focus_parent(&self) -> anyhow::Result<()> {
        Ok(self.engine.view.focus_parent()?)
    }
    /// Executes caller-authored code. Do not interpolate page-controlled text;
    /// this is not the isolated application/plugin runtime.
    pub fn evaluate_script(&self, script: &str) -> anyhow::Result<()> {
        Ok(self.engine.view.evaluate_script(script)?)
    }
}
