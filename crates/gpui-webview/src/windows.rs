use crate::{BrowserEvent, EventSender};
use gpui::{App, PlatformViewHandle};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use webview2_com::{
    Microsoft::Web::WebView2::Win32::*, NavigationCompletedEventHandler, ProcessFailedEventHandler,
    take_pwstr,
};
use windows_native::Win32::Foundation::RECT;
use windows_native::core::{BOOL, PWSTR};
use wry::{WebView, WebViewBuilder, WebViewExtWindows};

pub(super) struct Lifetime;
pub(super) fn initialize() -> anyhow::Result<()> {
    Ok(())
}

struct Parent(RawWindowHandle);
impl HasWindowHandle for Parent {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(self.0) })
    }
}

pub(super) fn build(
    builder: WebViewBuilder<'_>,
    raw: RawWindowHandle,
    sender: EventSender,
    _: &App,
) -> anyhow::Result<(WebView, PlatformViewHandle, Lifetime)> {
    let parent = Parent(raw);
    let view = builder.build_as_child(&parent)?;
    let allocation_sender = sender.clone();
    // Wry's public completion callback drops IsSuccess and WebErrorStatus.
    // Add an independent handler without replacing Wry's navigation policy.
    unsafe {
        let mut token = 0;
        let process_sender = sender.clone();
        view.webview().add_ProcessFailed(
            &ProcessFailedEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED;
                    args.ProcessFailedKind(&mut kind)?;
                    let _ = process_sender.send(BrowserEvent::ProcessFailed(format!(
                        "WebView2 process failure {}",
                        kind.0
                    )));
                }
                Ok(())
            })),
            &mut token,
        )?;
        view.webview().add_NavigationCompleted(
            &NavigationCompletedEventHandler::create(Box::new(move |webview, args| {
                if let (Some(webview), Some(args)) = (webview, args) {
                    let mut success = BOOL::default();
                    args.IsSuccess(&mut success)?;
                    if !success.as_bool() {
                        let mut status = COREWEBVIEW2_WEB_ERROR_STATUS_UNKNOWN;
                        args.WebErrorStatus(&mut status)?;
                        let mut source = PWSTR::null();
                        webview.Source(&mut source)?;
                        let _ = sender.send(BrowserEvent::LoadFailed {
                            url: take_pwstr(source),
                            description: format!("WebView2 navigation error {}", status.0),
                        });
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;
    }
    // Wry and GPUI use different windows-rs minor versions. Only the ABI HWND
    // pointer crosses that boundary, never COM interface wrapper types.
    let hwnd = windows::Win32::Foundation::HWND(view.hwnd().0);
    let controller = view.controller();
    let handle =
        unsafe { PlatformViewHandle::from_hwnd(hwnd) }.with_win32_resize_handler(move |size| {
            // GPUI owns the container HWND's position and clip region. Wry's
            // build_as_child deliberately leaves controller allocation to us.
            // Calling Wry set_bounds here would reposition the container too.
            let bounds = RECT {
                left: 0,
                top: 0,
                right: size.width.0,
                bottom: size.height.0,
            };
            if let Err(error) = unsafe { controller.SetBounds(bounds) } {
                let _ = allocation_sender.send(BrowserEvent::ViewportAllocationFailed(format!(
                    "WebView2 controller bounds: {error}"
                )));
            }
        });
    Ok((view, handle, Lifetime))
}
