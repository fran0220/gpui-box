use crate::{BrowserEvent, EventSender};
use gpui::{App, PlatformViewHandle, Task};
use gtk::prelude::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::time::Duration;
use webkit2gtk::WebViewExt;
use wry::{WebView, WebViewBuilder, WebViewExtUnix};

pub(super) struct Lifetime {
    _pump: Task<()>,
}

pub(super) fn initialize() -> anyhow::Result<()> {
    gtk::init()?;
    anyhow::ensure!(
        gtk::gdk::Display::default().is_some_and(|display| display.is::<gdkx11::X11Display>()),
        "WebKitGTK needs the X11 GDK backend; launch with GDK_BACKEND=x11 and an X11 GPUI window"
    );
    Ok(())
}

// Wry accepts Xlib, while GPUI's X11 backend exposes Xcb. An XID names the
// same server resource regardless of client library; Wry uses GDK's connection.
struct Parent(raw_window_handle::XlibWindowHandle);
impl HasWindowHandle for Parent {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(self.0.into()) })
    }
}

pub(super) fn build(
    builder: WebViewBuilder<'_>,
    raw: RawWindowHandle,
    sender: EventSender,
    cx: &App,
) -> anyhow::Result<(WebView, PlatformViewHandle, Lifetime)> {
    let parent = Parent(match raw {
        RawWindowHandle::Xlib(handle) => handle,
        RawWindowHandle::Xcb(handle) => {
            raw_window_handle::XlibWindowHandle::new(handle.window.get().into())
        }
        _ => anyhow::bail!("Linux native browser requires an X11/XWayland GPUI window"),
    });
    let view = builder.build_as_child(&parent)?;
    let widget = view.webview();
    let send = sender.clone();
    widget.connect_load_failed(move |_, _, url, error| {
        let _ = send.send(BrowserEvent::LoadFailed {
            url: url.into(),
            description: error.to_string(),
        });
        // Suppress WebKit's replacement error page; host displays the refusal.
        true
    });
    let send = sender.clone();
    widget.connect_load_failed_with_tls_errors(move |_, url, _, errors| {
        let _ = send.send(BrowserEvent::LoadFailed {
            url: url.into(),
            description: format!("TLS validation failed: {errors:?}"),
        });
        true
    });
    widget.connect_web_process_terminated(move |_, reason| {
        let _ = sender.send(BrowserEvent::ProcessFailed(format!("{reason:?}")));
    });
    let top = widget
        .toplevel()
        .ok_or_else(|| anyhow::anyhow!("WebKitGTK has no native toplevel"))?;
    let native = top
        .window()
        .ok_or_else(|| anyhow::anyhow!("WebKitGTK toplevel is not realized"))?;
    let native = native
        .downcast::<gdkx11::X11Window>()
        .map_err(|_| anyhow::anyhow!("WebKitGTK is not using X11"))?;
    // Hide only the native container; the GTK widget tree must remain shown so
    // external ConfigureNotify/MapNotify events allocate and paint its contents.
    native.upcast_ref::<gtk::gdk::Window>().hide();
    let handle = unsafe { PlatformViewHandle::from_x11_window(native.xid().try_into()?) }
        .with_x11_resize_handler(move |size| {
            let scale = top.scale_factor().max(1);
            top.size_allocate(&gtk::Allocation::new(
                0,
                0,
                size.width.0 / scale,
                size.height.0 / scale,
            ));
        });
    let background = cx.background_executor().clone();
    let pump = cx.foreground_executor().spawn(async move {
        loop {
            // Bound work per tick so a busy page cannot starve GPUI input.
            for _ in 0..64 {
                if !gtk::events_pending() {
                    break;
                }
                gtk::main_iteration_do(false);
            }
            background.timer(Duration::from_millis(8)).await;
        }
    });
    Ok((view, handle, Lifetime { _pump: pump }))
}
