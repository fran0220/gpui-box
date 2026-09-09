use crate::BrowserEvent;
use gpui::{App, PlatformViewHandle};
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{BOOL, Class, NO, Object, Sel, YES},
    sel, sel_impl,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    ffi::CStr,
    sync::{OnceLock, mpsc::Sender},
};
use wry::{WebView, WebViewBuilder, WebViewExtMacOS};

// WKNavigationDelegate is weak. Keep the forwarding proxy and Wry's original
// delegate alive, and restore the original before Wry tears down the WebView.
pub(super) struct Lifetime {
    view: *mut Object,
    proxy: *mut Object,
    original: *mut Object,
    _sender: Box<Sender<BrowserEvent>>,
}

impl Drop for Lifetime {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![self.view, setNavigationDelegate: self.original];
            let _: () = msg_send![self.proxy, release];
            let _: () = msg_send![self.original, release];
        }
    }
}

unsafe fn string(object: *mut Object) -> String {
    if object.is_null() {
        return String::new();
    }
    let bytes: *const std::ffi::c_char = unsafe { msg_send![object, UTF8String] };
    if bytes.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(bytes) }
            .to_string_lossy()
            .into_owned()
    }
}

extern "C" fn failed(this: &Object, _: Sel, view: *mut Object, _: *mut Object, error: *mut Object) {
    unsafe {
        let sender = &*(*this.get_ivar::<usize>("sender") as *const Sender<BrowserEvent>);
        let url: *mut Object = msg_send![view, URL];
        let address: *mut Object = msg_send![url, absoluteString];
        let description: *mut Object = msg_send![error, localizedDescription];
        let _ = sender.send(BrowserEvent::LoadFailed {
            url: string(address),
            description: string(description),
        });
    }
}

extern "C" fn terminated(this: &Object, _: Sel, _: *mut Object) {
    unsafe {
        let sender = &*(*this.get_ivar::<usize>("sender") as *const Sender<BrowserEvent>);
        let _ = sender.send(BrowserEvent::ProcessTerminated(
            "WebKit content process terminated".into(),
        ));
    }
}

extern "C" fn forwarding(this: &Object, _: Sel, _: Sel) -> *mut Object {
    unsafe { *this.get_ivar::<*mut Object>("original") }
}

extern "C" fn responds(this: &Object, _: Sel, selector: Sel) -> BOOL {
    if this.class().instance_method(selector).is_some() {
        return YES;
    }
    unsafe {
        let original = *this.get_ivar::<*mut Object>("original");
        if original.is_null() {
            NO
        } else {
            msg_send![original, respondsToSelector: selector]
        }
    }
}

fn delegate_class() -> &'static Class {
    static CLASS: OnceLock<&'static Class> = OnceLock::new();
    CLASS.get_or_init(|| {
        let mut class = ClassDecl::new("GPUIBoxBrowserNavigationDelegate", class!(NSObject))
            .expect("unique navigation delegate class");
        class.add_ivar::<usize>("sender");
        class.add_ivar::<*mut Object>("original");
        unsafe {
            class.add_method(
                sel!(webView:didFailNavigation:withError:),
                failed as extern "C" fn(&Object, Sel, *mut Object, *mut Object, *mut Object),
            );
            class.add_method(
                sel!(webView:didFailProvisionalNavigation:withError:),
                failed as extern "C" fn(&Object, Sel, *mut Object, *mut Object, *mut Object),
            );
            class.add_method(
                sel!(webViewWebContentProcessDidTerminate:),
                terminated as extern "C" fn(&Object, Sel, *mut Object),
            );
            class.add_method(
                sel!(forwardingTargetForSelector:),
                forwarding as extern "C" fn(&Object, Sel, Sel) -> *mut Object,
            );
            class.add_method(
                sel!(respondsToSelector:),
                responds as extern "C" fn(&Object, Sel, Sel) -> BOOL,
            );
        }
        class.register()
    })
}
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
    sender: Sender<BrowserEvent>,
    _: &App,
) -> anyhow::Result<(WebView, PlatformViewHandle, Lifetime)> {
    let parent = Parent(raw);
    let view = builder.build_as_child(&parent)?;
    let native = view.webview();
    let pointer: *mut Object = objc2::rc::Retained::as_ptr(&native).cast_mut().cast();
    let sender = Box::new(sender);
    let (handle, lifetime) = unsafe {
        let original: *mut Object = msg_send![pointer, navigationDelegate];
        let _: *mut Object = msg_send![original, retain];
        let proxy: *mut Object = msg_send![delegate_class(), new];
        (*proxy).set_ivar("sender", &*sender as *const Sender<BrowserEvent> as usize);
        (*proxy).set_ivar("original", original);
        let _: () = msg_send![pointer, setNavigationDelegate: proxy];
        (
            PlatformViewHandle::from_ns_view(pointer.cast()),
            Lifetime {
                view: pointer,
                proxy,
                original,
                _sender: sender,
            },
        )
    };
    Ok((view, handle, lifetime))
}
