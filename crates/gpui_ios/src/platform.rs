use crate::ffi::{self, Callbacks, Edges, Rect, TextRange};
use anyhow::{Result, anyhow, bail};
use futures::channel::oneshot;
use gpui::*;
use gpui_wgpu::{CosmicTextSystem, GpuContext, WgpuRenderer, WgpuSurfaceConfig, wgpu};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    UiKitWindowHandle, WindowHandle,
};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    ffi::{CStr, CString, c_char, c_void},
    path::{Path, PathBuf},
    ptr::NonNull,
    rc::{Rc, Weak},
    sync::Arc,
    time::Duration,
};

/// UIKit application host. Construct and run on the main thread.
///
/// Fonts are caller-owned assets, shared by shaping and rasterization through
/// the existing GPUI wgpu text system. The fallback family must be present in
/// `fonts`; no AppKit system-font authority is silently assumed on iOS.
pub struct IosPlatform {
    state: Rc<State>,
    foreground: ForegroundExecutor,
    background: BackgroundExecutor,
    text: Arc<dyn PlatformTextSystem>,
}

type LifecycleCallback = Box<dyn FnMut(AppLifecyclePhase)>;
type UrlCallback = Box<dyn FnMut(Vec<String>)>;
type ResizeCallback = Box<dyn FnMut(Size<Pixels>, f32)>;
type FrameCompletion = Box<dyn FnOnce(Result<()>)>;

struct State {
    window: RefCell<Option<Rc<WindowState>>>,
    launched: RefCell<Option<Box<dyn FnOnce()>>>,
    lifecycle: RefCell<Option<LifecycleCallback>>,
    memory: RefCell<Option<Box<dyn FnMut()>>>,
    urls: RefCell<Option<UrlCallback>>,
    thermal: RefCell<Option<Box<dyn FnMut()>>>,
    input_mode: RefCell<Option<Box<dyn FnMut()>>>,
    error: RefCell<Option<String>>,
    frame_completion: RefCell<Option<FrameCompletion>>,
    display: Rc<IosDisplay>,
}

impl IosPlatform {
    pub fn new(fallback_family: &str, fonts: Vec<Cow<'static, [u8]>>) -> Result<Self> {
        // SAFETY: This query does not dereference caller memory.
        if !unsafe { gpui_ios_is_main_thread() } {
            bail!("UIKit platform must be created on the main thread");
        }
        let text = load_text_system(fallback_family, fonts)?;
        let dispatcher = Arc::new(IosDispatcher);
        Ok(Self {
            state: Rc::new(State {
                window: RefCell::new(None),
                launched: RefCell::new(None),
                lifecycle: RefCell::new(None),
                memory: RefCell::new(None),
                urls: RefCell::new(None),
                thermal: RefCell::new(None),
                input_mode: RefCell::new(None),
                error: RefCell::new(None),
                frame_completion: RefCell::new(None),
                display: Rc::new(IosDisplay),
            }),
            foreground: ForegroundExecutor::new(dispatcher.clone()),
            background: BackgroundExecutor::new(dispatcher),
            text,
        })
    }

    /// Most recent native surface failure. A failed resume never permits draw.
    pub fn last_error(&self) -> Option<String> {
        self.state.error.borrow().clone()
    }

    /// Verifies completion of the next successfully submitted GPUI window frame.
    ///
    /// Diagnostic use only: waits up to five seconds for GPU work after a real
    /// draw, reports pending validation/device errors, and delivers the callback
    /// on the main queue after the GPUI window borrow ends. No callback means no
    /// successful draw occurred; the harness must impose its own overall timeout.
    /// This proves submission/completion, not displayed pixels or input behavior.
    pub fn verify_next_frame(&self, callback: impl FnOnce(Result<()>) + 'static) -> Result<()> {
        let mut pending = self.state.frame_completion.borrow_mut();
        if pending.is_some() {
            bail!("frame verification is already pending");
        }
        *pending = Some(Box::new(callback));
        Ok(())
    }
}

fn load_text_system(
    family: &str,
    fonts: Vec<Cow<'static, [u8]>>,
) -> Result<Arc<dyn PlatformTextSystem>> {
    if fonts.is_empty() {
        bail!("iOS requires an explicit font asset authority");
    }
    let text = Arc::new(CosmicTextSystem::new_without_system_fonts(family));
    text.add_fonts(fonts)?;
    if !text.all_font_names().iter().any(|name| name == family) {
        bail!("required iOS fallback family {family:?} is absent from supplied font assets");
    }
    Ok(text)
}

fn unsupported(operation: &str) -> ! {
    panic!(
        "UIKit does not support {operation}; query the platform operation capability before invoking it"
    )
}

fn refused<T>(operation: &str) -> oneshot::Receiver<Result<T>> {
    let (tx, rx) = oneshot::channel();
    let _ = tx.send(Err(anyhow!("UIKit {operation} is not implemented")));
    rx
}

impl Platform for IosPlatform {
    fn check_app_operation(&self, _: AppOperation) -> Result<(), PlatformOperationError> {
        Err(PlatformOperationError::Unsupported(
            "UIKit application command",
        ))
    }
    fn background_executor(&self) -> BackgroundExecutor {
        self.background.clone()
    }
    fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground.clone()
    }
    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text.clone()
    }
    fn run(&self, launched: Box<dyn FnOnce()>) {
        *self.state.launched.borrow_mut() = Some(launched);
        // UIApplicationMain owns the process loop; retain its context for the
        // complete invocation, including launch before any window exists.
        let callbacks = native_callbacks(Rc::as_ptr(&self.state) as *mut c_void);
        let status = unsafe { ffi::gpui_ios_run(callbacks) };
        if status != 0 {
            *self.state.error.borrow_mut() = Some(format!("UIApplicationMain returned {status}"));
        }
    }
    fn quit(&self) {
        unsupported("programmatic quit")
    }
    fn restart(&self, _: Option<PathBuf>) {
        unsupported("restart")
    }
    fn activate(&self, _: bool) {
        unsupported("foreground activation; iOS owns application activation")
    }
    fn hide(&self) {
        unsupported("hide")
    }
    fn hide_other_apps(&self) {
        unsupported("hide other apps")
    }
    fn unhide_other_apps(&self) {
        unsupported("unhide other apps")
    }
    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        vec![self.state.display.clone()]
    }
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.state.display.clone())
    }
    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.state
            .window
            .borrow()
            .as_ref()
            .filter(|w| w.active.get())
            .map(|w| w.handle)
    }
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        if self.state.window.borrow().is_some() {
            bail!("UIKit currently supports one application window");
        }
        if !matches!(options.kind, WindowKind::Normal) {
            bail!("UIKit separate popup/panel windows are unsupported");
        }
        let window = Rc::new(WindowState::new(handle, Rc::downgrade(&self.state)));
        *self.state.window.borrow_mut() = Some(window.clone());
        let host = unsafe {
            ffi::gpui_ios_create(native_callbacks(Rc::as_ptr(&self.state) as *mut c_void))
        };
        let Some(host) = NonNull::new(host) else {
            self.state.window.borrow_mut().take();
            bail!("UIKit refused window creation");
        };
        window.host.set(Some(host));
        let result = window.create_renderer();
        if let Err(error) = result {
            unsafe { ffi::gpui_ios_destroy(host.as_ptr()) };
            window.host.set(None);
            self.state.window.borrow_mut().take();
            return Err(error);
        }
        Ok(Box::new(IosWindow(window)))
    }
    fn window_appearance(&self) -> WindowAppearance {
        native_appearance()
    }
    fn open_url(&self, url: &str) {
        let url = CString::new(url).expect("URL contains NUL");
        unsafe { gpui_ios_open_url(url.as_ptr()) };
    }
    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        *self.state.urls.borrow_mut() = Some(callback);
    }
    fn register_url_scheme(&self, _: &str) -> Task<Result<()>> {
        Task::ready(Err(anyhow!(
            "URL schemes must be declared in the signed Info.plist"
        )))
    }
    fn prompt_for_paths(
        &self,
        _: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        refused("document picker")
    }
    fn prompt_for_new_path(
        &self,
        _: &Path,
        _: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        refused("document export picker")
    }
    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }
    fn reveal_path(&self, _: &Path) {
        unsupported("reveal path")
    }
    fn open_with_system(&self, _: &Path) {
        unsupported("open file with system")
    }
    fn on_quit(&self, _: Box<dyn FnMut()>) { /* iOS does not promise termination notification. */
    }
    fn on_reopen(&self, _: Box<dyn FnMut()>) { /* No desktop reopen event on iOS. */
    }
    fn on_system_wake(&self, _: Box<dyn FnMut()>) { /* Foreground lifecycle is authoritative. */
    }
    fn on_app_lifecycle(&self, cb: Box<dyn FnMut(AppLifecyclePhase)>) {
        *self.state.lifecycle.borrow_mut() = Some(cb);
    }
    fn on_memory_warning(&self, cb: Box<dyn FnMut()>) {
        *self.state.memory.borrow_mut() = Some(cb);
    }
    fn set_menus(&self, _: Vec<Menu>, _: &Keymap) {
        unsupported("application menu bar")
    }
    fn set_dock_menu(&self, _: Vec<MenuItem>, _: &Keymap) {
        unsupported("dock menu")
    }
    fn on_app_menu_action(&self, _: Box<dyn FnMut(&dyn Action)>) { /* No application menu bar. */
    }
    fn on_will_open_app_menu(&self, _: Box<dyn FnMut()>) { /* No application menu bar. */
    }
    fn on_validate_app_menu_command(&self, _: Box<dyn FnMut(&dyn Action) -> bool>) { /* No application menu bar. */
    }
    fn thermal_state(&self) -> ThermalState {
        match unsafe { gpui_ios_thermal_state() } {
            0 => ThermalState::Nominal,
            1 => ThermalState::Fair,
            2 => ThermalState::Serious,
            _ => ThermalState::Critical,
        }
    }
    fn on_thermal_state_change(&self, callback: Box<dyn FnMut()>) {
        *self.state.thermal.borrow_mut() = Some(callback);
    }
    fn app_path(&self) -> Result<PathBuf> {
        native_string(0)
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("NSBundle has no application path"))
    }
    fn path_for_auxiliary_executable(&self, _: &str) -> Result<PathBuf> {
        bail!("iOS cannot launch auxiliary executables")
    }
    fn set_cursor_style(&self, _: CursorStyle) {
        unsupported("pointer cursor styling")
    }
    fn hide_cursor_until_mouse_moves(&self) {
        unsupported("cursor hiding")
    }
    fn is_cursor_visible(&self) -> bool {
        false
    }
    fn should_auto_hide_scrollbars(&self) -> bool {
        true
    }
    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        native_string(1).map(ClipboardItem::new_string)
    }
    fn write_to_clipboard(&self, item: ClipboardItem) {
        let Some(text) = item.text() else {
            unsupported("non-text clipboard")
        };
        let text = CString::new(text).expect("clipboard text contains NUL");
        unsafe { gpui_ios_set_clipboard(text.as_ptr()) };
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        None
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, _: ClipboardItem) {
        unsupported("primary selection")
    }
    fn write_credentials(&self, _: &str, _: &str, _: &[u8]) -> Task<Result<()>> {
        Task::ready(Err(anyhow!("iOS Keychain adapter unavailable")))
    }
    fn read_credentials(&self, _: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Err(anyhow!("iOS Keychain adapter unavailable")))
    }
    fn delete_credentials(&self, _: &str) -> Task<Result<()>> {
        Task::ready(Err(anyhow!("iOS Keychain adapter unavailable")))
    }
    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(IosKeyboard(
            native_string(2).unwrap_or_else(|| "uikit-text-input".into()),
        ))
    }
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }
    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        *self.state.input_mode.borrow_mut() = Some(callback);
    }
}

struct IosKeyboard(String);
impl PlatformKeyboardLayout for IosKeyboard {
    fn id(&self) -> &str {
        &self.0
    }
    fn name(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
struct IosDisplay;
impl PlatformDisplay for IosDisplay {
    fn id(&self) -> DisplayId {
        DisplayId::new(1)
    }
    fn uuid(&self) -> Result<uuid::Uuid> {
        bail!("UIKit does not expose a persistent screen UUID")
    }
    fn bounds(&self) -> Bounds<Pixels> {
        bounds(unsafe { gpui_ios_screen_bounds() })
    }
}

struct IosDispatcher;
impl PlatformDispatcher for IosDispatcher {
    fn is_main_thread(&self) -> bool {
        unsafe { gpui_ios_is_main_thread() }
    }
    fn dispatch(&self, runnable: RunnableVariant, _: Priority) {
        std::thread::spawn(move || {
            runnable.run();
        });
    }
    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, _: Priority) {
        unsafe { gpui_ios_dispatch(runnable.into_raw().as_ptr().cast(), run_task, 0) };
    }
    fn dispatch_after(&self, delay: Duration, runnable: RunnableVariant) {
        unsafe {
            gpui_ios_dispatch(
                runnable.into_raw().as_ptr().cast(),
                run_task,
                delay.as_nanos().min(i64::MAX as u128) as u64,
            )
        };
    }
    fn spawn_realtime(&self, _: Box<dyn FnOnce() + Send>) {
        unsupported("real-time audio scheduling")
    }
}
unsafe extern "C" fn run_task(context: *mut c_void) {
    unsafe {
        RunnableVariant::from_raw(NonNull::new(context.cast()).expect("owned runnable pointer"))
    }
    .run();
}

fn bounds(rect: Rect) -> Bounds<Pixels> {
    Bounds::new(
        point(px(rect.x as f32), px(rect.y as f32)),
        size(px(rect.width as f32), px(rect.height as f32)),
    )
}
fn rect(bounds: Bounds<Pixels>) -> Rect {
    Rect {
        x: f32::from(bounds.origin.x) as f64,
        y: f32::from(bounds.origin.y) as f64,
        width: f32::from(bounds.size.width) as f64,
        height: f32::from(bounds.size.height) as f64,
    }
}
fn native_appearance() -> WindowAppearance {
    if unsafe { gpui_ios_dark() } {
        WindowAppearance::Dark
    } else {
        WindowAppearance::Light
    }
}
fn native_string(which: u32) -> Option<String> {
    let pointer = unsafe { gpui_ios_copy_string(which) };
    if pointer.is_null() {
        return None;
    }
    let value = unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned();
    unsafe { gpui_ios_free_string(pointer) };
    Some(value)
}

unsafe extern "C" {
    fn gpui_ios_is_main_thread() -> bool;
    fn gpui_ios_screen_bounds() -> Rect;
    fn gpui_ios_dark() -> bool;
    fn gpui_ios_thermal_state() -> u32;
    fn gpui_ios_open_url(url: *const c_char);
    fn gpui_ios_copy_string(which: u32) -> *mut c_char;
    fn gpui_ios_free_string(string: *mut c_char);
    fn gpui_ios_set_clipboard(text: *const c_char);
    fn gpui_ios_dispatch(
        context: *mut c_void,
        callback: unsafe extern "C" fn(*mut c_void),
        delay: u64,
    );
}

#[derive(Default)]
struct WindowCallbacks {
    frame: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    input: Option<Box<dyn FnMut(PlatformInput) -> DispatchEventResult>>,
    active: Option<Box<dyn FnMut(bool)>>,
    resize: Option<ResizeCallback>,
    insets: Option<Box<dyn FnMut(WindowInsets)>>,
    should_close: Option<Box<dyn FnMut() -> bool>>,
    close: Option<Box<dyn FnOnce()>>,
    appearance: Option<Box<dyn FnMut()>>,
}

// Release the callback slot before entering GPUI. GPUI can replace callbacks
// or destroy a window from an event; never hold a RefCell borrow over that.
macro_rules! emit {
    ($window:expr, $field:ident $(, $argument:expr)*) => {{
        let callback = $window.callbacks.borrow_mut().$field.take();
        if let Some(mut callback) = callback {
            let result = callback($($argument),*);
            let mut callbacks = $window.callbacks.borrow_mut();
            if callbacks.$field.is_none() { callbacks.$field = Some(callback); }
            Some(result)
        } else { None }
    }};
}

struct WindowState {
    handle: AnyWindowHandle,
    owner: Weak<State>,
    host: Cell<Option<NonNull<c_void>>>,
    bounds: Cell<Bounds<Pixels>>,
    scale: Cell<f32>,
    insets: RefCell<WindowInsets>,
    active: Cell<bool>,
    renderable: Cell<bool>,
    force_render: Cell<bool>,
    renderer: RefCell<Option<WgpuRenderer>>,
    callbacks: RefCell<WindowCallbacks>,
    input: RefCell<Option<PlatformInputHandler>>,
    input_generation: Cell<u64>,
    title: RefCell<String>,
    background: Cell<WindowBackgroundAppearance>,
    ime_bounds: Cell<Bounds<Pixels>>,
    text_notification_pending: Cell<bool>,
    content_notification_pending: Cell<bool>,
    keyboard_visible: Cell<Option<bool>>,
    a11y: RefCell<crate::accessibility::AccessibilityTree>,
    a11y_callbacks: RefCell<Option<Rc<A11yCallbacks>>>,
}

impl WindowState {
    fn new(handle: AnyWindowHandle, owner: Weak<State>) -> Self {
        Self {
            handle,
            owner,
            host: Cell::new(None),
            bounds: Cell::new(Bounds::default()),
            scale: Cell::new(1.0),
            insets: RefCell::new(WindowInsets::default()),
            active: Cell::new(false),
            renderable: Cell::new(false),
            force_render: Cell::new(true),
            renderer: RefCell::new(None),
            callbacks: RefCell::new(WindowCallbacks::default()),
            input: RefCell::new(None),
            input_generation: Cell::new(0),
            title: RefCell::new(String::new()),
            background: Cell::new(WindowBackgroundAppearance::Opaque),
            ime_bounds: Cell::new(Bounds::default()),
            text_notification_pending: Cell::new(false),
            content_notification_pending: Cell::new(false),
            keyboard_visible: Cell::new(None),
            a11y: RefCell::new(crate::accessibility::AccessibilityTree::default()),
            a11y_callbacks: RefCell::new(None),
        }
    }
    fn native_handle(&self) -> Result<NativeHandle> {
        let host = self
            .host
            .get()
            .ok_or_else(|| anyhow!("UIKit host is closed"))?;
        let view = NonNull::new(unsafe { ffi::gpui_ios_view(host.as_ptr()) })
            .ok_or_else(|| anyhow!("UIKit view is unavailable"))?;
        let controller = NonNull::new(unsafe { ffi::gpui_ios_controller(host.as_ptr()) });
        Ok(NativeHandle { view, controller })
    }
    fn config(&self) -> WgpuSurfaceConfig {
        let size = self.bounds.get().size;
        WgpuSurfaceConfig {
            size: gpui::size(
                DevicePixels((f32::from(size.width) * self.scale.get()).round() as i32),
                DevicePixels((f32::from(size.height) * self.scale.get()).round() as i32),
            ),
            transparent: self.background.get() != WindowBackgroundAppearance::Opaque,
            color_space: wgpu::SurfaceColorSpace::Auto,
            preferred_present_mode: Some(wgpu::PresentMode::Fifo),
        }
    }
    fn create_renderer(&self) -> Result<()> {
        let context: GpuContext = Rc::new(RefCell::new(None));
        let renderer = WgpuRenderer::new(context, &self.native_handle()?, self.config(), None)?;
        *self.renderer.borrow_mut() = Some(renderer);
        self.renderable.set(true);
        Ok(())
    }
    fn fail(&self, error: anyhow::Error) {
        self.renderable.set(false);
        log::error!("UIKit surface unavailable: {error:#}");
        if let Some(owner) = self.owner.upgrade() {
            *owner.error.borrow_mut() = Some(format!("{error:#}"));
        }
    }
    fn suspend(&self) {
        self.renderable.set(false);
        let result = self
            .renderer
            .borrow_mut()
            .as_mut()
            .map(WgpuRenderer::unconfigure_surface);
        if let Some(Err(error)) = result {
            self.fail(error);
        }
    }
    fn resume(&self) -> Result<()> {
        let handle = self.native_handle()?;
        if let Some(renderer) = self.renderer.borrow_mut().as_mut() {
            if renderer.device_lost() {
                renderer.recover(&handle)?;
            }
            renderer.replace_surface(&handle, self.config())?;
        } else {
            bail!("UIKit renderer is not initialized");
        }
        self.renderable.set(true);
        self.force_render.set(true);
        Ok(())
    }
    fn notify_text_later(self: &Rc<Self>) {
        if self.text_notification_pending.replace(true) {
            return;
        }
        let pointer = Rc::into_raw(self.clone()) as *mut c_void;
        unsafe { gpui_ios_dispatch(pointer, notify_text, 0) };
    }
    fn publish_accessibility(&self) {
        let Some(host) = self.host.get() else {
            return;
        };
        let projected = self.a11y.borrow_mut().project(self.scale.get() as f64);
        let native: Vec<_> = projected.iter().map(|node| node.native()).collect();
        unsafe { ffi::gpui_ios_accessibility(host.as_ptr(), native.as_ptr(), native.len()) };
    }
}

// The wrapper transports opaque pointers to wgpu, which creates the UIKit
// surface on the calling main thread. It never owns or releases UIKit objects.
// WindowState drops its renderer before gpui_ios_destroy releases these views.
#[derive(Clone, Debug)]
struct NativeHandle {
    view: NonNull<c_void>,
    controller: Option<NonNull<c_void>>,
}
unsafe impl Send for NativeHandle {}
unsafe impl Sync for NativeHandle {}
impl HasDisplayHandle for NativeHandle {
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::uikit())
    }
}
impl HasWindowHandle for NativeHandle {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        let mut handle = UiKitWindowHandle::new(self.view);
        handle.ui_view_controller = self.controller;
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::UiKit(handle)) })
    }
}

struct IosWindow(Rc<WindowState>);
impl HasDisplayHandle for IosWindow {
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::uikit())
    }
}
impl HasWindowHandle for IosWindow {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        let handle = self
            .0
            .native_handle()
            .map_err(|_| HandleError::Unavailable)?;
        let mut raw = UiKitWindowHandle::new(handle.view);
        raw.ui_view_controller = handle.controller;
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::UiKit(raw)) })
    }
}
impl Drop for IosWindow {
    fn drop(&mut self) {
        self.0.renderable.set(false);
        if let Some(owner) = self.0.owner.upgrade() {
            owner.window.borrow_mut().take();
        }
        if let Some(mut renderer) = self.0.renderer.borrow_mut().take()
            && let Err(error) = renderer.destroy()
        {
            self.0.fail(error);
        }
        if let Some(host) = self.0.host.take() {
            unsafe { ffi::gpui_ios_destroy(host.as_ptr()) };
        }
    }
}
impl PlatformWindow for IosWindow {
    fn check_window_operation(&self, _: WindowOperation) -> Result<(), PlatformOperationError> {
        Err(PlatformOperationError::Unsupported(
            "UIKit owns window geometry",
        ))
    }
    fn bounds(&self) -> Bounds<Pixels> {
        self.0.bounds.get()
    }
    fn is_maximized(&self) -> bool {
        false
    }
    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Windowed(self.bounds())
    }
    fn content_size(&self) -> Size<Pixels> {
        self.bounds().size
    }
    fn resize(&mut self, _: Size<Pixels>) {
        unsupported("window resize; UIKit owns viewport geometry")
    }
    fn scale_factor(&self) -> f32 {
        self.0.scale.get()
    }
    fn appearance(&self) -> WindowAppearance {
        native_appearance()
    }
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.0
            .owner
            .upgrade()
            .map(|o| o.display.clone() as Rc<dyn PlatformDisplay>)
    }
    fn mouse_position(&self) -> Point<Pixels> {
        Point::default()
    }
    fn modifiers(&self) -> Modifiers {
        Modifiers::default()
    }
    fn capslock(&self) -> Capslock {
        Capslock::default()
    }
    fn set_input_handler(&mut self, input: PlatformInputHandler) {
        self.0
            .input_generation
            .set(self.0.input_generation.get().wrapping_add(1));
        *self.0.input.borrow_mut() = Some(input);
        self.0.notify_text_later();
    }
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.0
            .input_generation
            .set(self.0.input_generation.get().wrapping_add(1));
        self.0.input.borrow_mut().take()
    }
    fn prompt(
        &self,
        _: PromptLevel,
        _: &str,
        _: Option<&str>,
        _: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        None
    }
    fn activate(&self) {
        if let Some(h) = self.0.host.get() {
            unsafe { gpui_ios_activate_window(h.as_ptr()) };
        }
    }
    fn is_active(&self) -> bool {
        self.0.active.get()
    }
    fn is_hovered(&self) -> bool {
        false
    }
    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.0.background.get()
    }
    fn set_title(&mut self, title: &str) {
        *self.0.title.borrow_mut() = title.to_owned();
        if let Some(h) = self.0.host.get() {
            let title = CString::new(title).expect("title contains NUL");
            unsafe { ffi::gpui_ios_set_title(h.as_ptr(), title.as_ptr()) };
        }
    }
    fn set_background_appearance(&self, appearance: WindowBackgroundAppearance) {
        self.0.background.set(appearance);
        if let Some(renderer) = self.0.renderer.borrow_mut().as_mut() {
            renderer.update_transparency(appearance != WindowBackgroundAppearance::Opaque);
        }
    }
    fn minimize(&self) {
        unsupported("minimize")
    }
    fn zoom(&self) {
        unsupported("zoom")
    }
    fn request_close(&self) {
        if emit!(self.0, should_close) == Some(false) {
            return;
        }
        let close = self.0.callbacks.borrow_mut().close.take();
        if let Some(close) = close {
            close();
        }
    }
    fn toggle_fullscreen(&self) {
        unsupported("toggle fullscreen")
    }
    fn is_fullscreen(&self) -> bool {
        false
    }
    fn on_request_frame(&self, cb: Box<dyn FnMut(RequestFrameOptions)>) {
        self.0.callbacks.borrow_mut().frame = Some(cb);
    }
    fn on_input(&self, cb: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.0.callbacks.borrow_mut().input = Some(cb);
    }
    fn on_active_status_change(&self, cb: Box<dyn FnMut(bool)>) {
        self.0.callbacks.borrow_mut().active = Some(cb);
    }
    fn on_hover_status_change(&self, _: Box<dyn FnMut(bool)>) { /* Touch has no hover. */
    }
    fn on_resize(&self, cb: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.0.callbacks.borrow_mut().resize = Some(cb);
    }
    fn on_moved(&self, _: Box<dyn FnMut()>) { /* The UIKit application window has no desktop position. */
    }
    fn on_should_close(&self, cb: Box<dyn FnMut() -> bool>) {
        self.0.callbacks.borrow_mut().should_close = Some(cb);
    }
    fn on_hit_test_window_control(
        &self,
        _: Box<dyn FnMut(Point<Pixels>) -> Option<WindowControlArea>>,
    ) { /* UIKit owns system chrome. */
    }
    fn on_close(&self, cb: Box<dyn FnOnce()>) {
        self.0.callbacks.borrow_mut().close = Some(cb);
    }
    fn on_appearance_changed(&self, cb: Box<dyn FnMut()>) {
        self.0.callbacks.borrow_mut().appearance = Some(cb);
    }
    fn draw(&self, scene: &Scene) {
        if !self.0.active.get() || !self.0.renderable.get() {
            return;
        }
        let result = {
            let mut renderer = self.0.renderer.borrow_mut();
            let Some(renderer) = renderer.as_mut() else {
                return;
            };
            if renderer.device_lost() {
                Err(anyhow!("UIKit Metal device was lost"))
            } else {
                let presented = renderer.draw(scene);
                self.0
                    .force_render
                    .set(!presented || renderer.needs_redraw());
                if presented {
                    let verification = self
                        .0
                        .owner
                        .upgrade()
                        .and_then(|owner| owner.frame_completion.borrow_mut().take());
                    if let Some(callback) = verification {
                        let completion = renderer.wait_for_gpu_completion(Duration::from_secs(5));
                        let failed = completion.as_ref().err().map(|error| format!("{error:#}"));
                        let delivery: Box<Box<dyn FnOnce()>> =
                            Box::new(Box::new(move || callback(completion)));
                        unsafe {
                            gpui_ios_dispatch(Box::into_raw(delivery).cast(), deliver_completion, 0)
                        };
                        if let Some(error) = failed {
                            return self.0.fail(anyhow!(error));
                        }
                    }
                }
                Ok(())
            }
        };
        if let Err(error) = result {
            self.0.fail(error);
        }
    }
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.0
            .renderer
            .borrow()
            .as_ref()
            .expect("open window has renderer")
            .sprite_atlas()
            .clone()
    }
    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }
    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.0
            .renderer
            .borrow()
            .as_ref()
            .map(WgpuRenderer::gpu_specs)
    }
    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        if self.0.ime_bounds.replace(bounds) != bounds {
            self.0.notify_text_later();
        }
    }
    fn insets(&self) -> WindowInsets {
        self.0.insets.borrow().clone()
    }
    fn on_insets_changed(&self, cb: Box<dyn FnMut(WindowInsets)>) {
        self.0.callbacks.borrow_mut().insets = Some(cb);
    }
    fn show_soft_keyboard(&self) {
        self.0.keyboard_visible.set(Some(true));
        self.0.notify_text_later();
    }
    fn hide_soft_keyboard(&self) {
        self.0.keyboard_visible.set(Some(false));
        self.0.notify_text_later();
    }
    fn text_input_state_changed(&self, change: TextInputStateChange) {
        if matches!(
            change,
            TextInputStateChange::ContentChanged | TextInputStateChange::FocusGained
        ) {
            self.0.content_notification_pending.set(true);
        }
        self.0.notify_text_later();
    }
    fn a11y_init(&self, callbacks: A11yCallbacks) {
        *self.0.a11y_callbacks.borrow_mut() = Some(Rc::new(callbacks));
        unsafe {
            gpui_ios_dispatch(
                Rc::into_raw(self.0.clone()) as *mut c_void,
                activate_accessibility,
                0,
            )
        };
    }
    fn a11y_tree_update(&self, update: accesskit::TreeUpdate) {
        let result = self.0.a11y.borrow_mut().update(update, self.0.active.get());
        if let Err(error) = result {
            log::error!("UIKit accessibility unavailable: {error:#}");
            if let Some(owner) = self.0.owner.upgrade() {
                *owner.error.borrow_mut() = Some(format!("{error:#}"));
            }
            return;
        }
        self.0.publish_accessibility();
    }
    fn a11y_update_window_bounds(&self) {
        self.0.publish_accessibility();
    }
}

unsafe extern "C" {
    fn gpui_ios_activate_window(host: *mut c_void);
}

unsafe extern "C" fn deliver_completion(pointer: *mut c_void) {
    let callback = unsafe { Box::from_raw(pointer.cast::<Box<dyn FnOnce()>>()) };
    callback();
}

unsafe extern "C" fn notify_text(pointer: *mut c_void) {
    let window = unsafe { Rc::from_raw(pointer.cast::<WindowState>()) };
    window.text_notification_pending.set(false);
    let Some(host) = window.host.get() else {
        return;
    };
    if let Some(owner) = window.owner.upgrade() {
        let context = Rc::as_ptr(&owner) as *mut c_void;
        if let Some(options) =
            unsafe { with_input(context, None, PlatformInputHandler::text_input_options) }
        {
            let purpose = match options.purpose {
                KeyboardPurpose::Text => 0,
                KeyboardPurpose::Number => 1,
                KeyboardPurpose::Decimal => 2,
                KeyboardPurpose::Phone => 3,
                KeyboardPurpose::Email => 4,
                KeyboardPurpose::Url => 5,
                KeyboardPurpose::Search => 6,
            };
            let action = match options.action {
                TextInputAction::Default => 0,
                TextInputAction::Newline => 1,
                TextInputAction::Done => 2,
                TextInputAction::Go => 3,
                TextInputAction::Next => 4,
                TextInputAction::Previous => 5,
                TextInputAction::Search => 6,
                TextInputAction::Send => 7,
            };
            let autofill = match options.autofill {
                None => 0,
                Some(AutofillPurpose::Name) => 1,
                Some(AutofillPurpose::Username) => 2,
                Some(AutofillPurpose::CurrentPassword) => 3,
                Some(AutofillPurpose::NewPassword) => 4,
                Some(AutofillPurpose::Email) => 5,
                Some(AutofillPurpose::Phone) => 6,
                Some(AutofillPurpose::OneTimeCode) => 7,
            };
            unsafe {
                gpui_ios_text_options(
                    host.as_ptr(),
                    purpose,
                    action,
                    autofill,
                    options.secure,
                    options.multiline,
                )
            };
        }
    }
    unsafe {
        ffi::gpui_ios_text_changed(
            host.as_ptr(),
            !window.content_notification_pending.replace(false),
        )
    };
    if let Some(visible) = window.keyboard_visible.take() {
        unsafe { ffi::gpui_ios_keyboard(host.as_ptr(), visible) };
    }
}

unsafe extern "C" {
    fn gpui_ios_text_options(
        host: *mut c_void,
        purpose: u32,
        action: u32,
        autofill: u32,
        secure: bool,
        multiline: bool,
    );
}

unsafe extern "C" fn activate_accessibility(pointer: *mut c_void) {
    let window = unsafe { Rc::from_raw(pointer.cast::<WindowState>()) };
    if window.host.get().is_none() {
        return;
    }
    let callbacks = window.a11y_callbacks.borrow().clone();
    if let Some(callbacks) = callbacks
        && let Some(update) = (callbacks.activation)()
    {
        let result = window.a11y.borrow_mut().update(update, window.active.get());
        if let Err(error) = result {
            log::error!("UIKit accessibility activation failed: {error:#}");
            return;
        }
        window.publish_accessibility();
    }
}

unsafe extern "C" fn accessibility_action(pointer: *mut c_void, id: u64, action: u32) -> bool {
    let Some(window) = (unsafe { window(pointer) }) else {
        return false;
    };
    let request = window.a11y.borrow().action(id, action);
    let callbacks = window.a11y_callbacks.borrow().clone();
    match (request, callbacks) {
        (Some(request), Some(callbacks)) => {
            (callbacks.action)(request);
            true
        }
        _ => false,
    }
}

unsafe fn window(pointer: *mut c_void) -> Option<Rc<WindowState>> {
    unsafe { &*pointer.cast::<State>() }.window.borrow().clone()
}

fn native_callbacks(context: *mut c_void) -> Callbacks {
    Callbacks {
        context,
        launched: Some(launched),
        lifecycle: Some(lifecycle),
        memory_warning: Some(memory),
        frame: Some(frame),
        geometry: Some(geometry),
        touch: Some(touch),
        cancel_input: Some(cancel_input),
        text,
        text_length,
        selection,
        marked,
        select,
        replace,
        mark,
        unmark,
        text_bounds,
        text_hit_test,
        accessibility_action: Some(accessibility_action),
        system_event: Some(system_event),
        text_action: Some(text_action),
        native_selection: Some(native_selection),
        set_native_selection: Some(set_native_selection),
        move_position: Some(move_position),
        farthest_position: Some(farthest_position),
        position_bounds: Some(position_bounds),
        selection_rects: Some(selection_rects),
        grapheme: Some(grapheme),
        writing_direction: Some(writing_direction),
        set_writing_direction: Some(set_writing_direction),
        position_for_point: Some(position_for_point),
    }
}
unsafe extern "C" fn text_action(pointer: *mut c_void, action: u32) -> bool {
    let action = match action {
        0 => TextInputAction::Default,
        1 => TextInputAction::Newline,
        2 => TextInputAction::Done,
        3 => TextInputAction::Go,
        4 => TextInputAction::Next,
        5 => TextInputAction::Previous,
        6 => TextInputAction::Search,
        7 => TextInputAction::Send,
        _ => return false,
    };
    unsafe {
        with_input(pointer, false, |input| {
            input.perform_text_input_action(action)
        })
    }
}
unsafe extern "C" fn launched(pointer: *mut c_void) {
    let state = unsafe { &*pointer.cast::<State>() };
    let callback = state.launched.borrow_mut().take();
    if let Some(callback) = callback {
        callback();
    }
}
unsafe extern "C" fn lifecycle(pointer: *mut c_void, phase: u32) {
    let state = unsafe { &*pointer.cast::<State>() };
    let phase = match phase {
        0 => AppLifecyclePhase::Active,
        1 => AppLifecyclePhase::Inactive,
        2 => AppLifecyclePhase::Background,
        _ => AppLifecyclePhase::Foreground,
    };
    if let Some(window) = unsafe { window(pointer) } {
        if phase == AppLifecyclePhase::Inactive || phase == AppLifecyclePhase::Background {
            window.active.set(false);
            emit!(window, active, false);
            window.suspend();
        } else if phase == AppLifecyclePhase::Active {
            match window.resume() {
                Ok(()) => {
                    window.active.set(true);
                    emit!(window, active, true);
                }
                Err(error) => window.fail(error),
            }
        }
    }
    let callback = state.lifecycle.borrow_mut().take();
    if let Some(mut callback) = callback {
        callback(phase);
        *state.lifecycle.borrow_mut() = Some(callback);
    }
}
unsafe extern "C" fn memory(pointer: *mut c_void) {
    let state = unsafe { &*pointer.cast::<State>() };
    let callback = state.memory.borrow_mut().take();
    if let Some(mut callback) = callback {
        callback();
        *state.memory.borrow_mut() = Some(callback);
    }
}
unsafe extern "C" fn frame(pointer: *mut c_void, _: f64) {
    if let Some(window) = unsafe { window(pointer) }
        && window.active.get()
        && window.renderable.get()
    {
        emit!(
            window,
            frame,
            RequestFrameOptions {
                require_presentation: true,
                force_render: window.force_render.replace(false)
            }
        );
    }
}
unsafe extern "C" fn cancel_input(pointer: *mut c_void) {
    if let Some(window) = unsafe { window(pointer) } {
        emit!(window, active, false);
    }
}
unsafe extern "C" fn geometry(
    pointer: *mut c_void,
    rectangle: Rect,
    scale: f64,
    safe: Edges,
    ime: Edges,
) {
    let Some(window) = (unsafe { window(pointer) }) else {
        return;
    };
    let bounds = bounds(rectangle);
    let scale = scale as f32;
    let resized = window.bounds.replace(bounds) != bounds || window.scale.get() != scale;
    window.scale.set(scale);
    let edges = |e: Edges| gpui::Edges {
        top: px(e.top as f32),
        right: px(e.right as f32),
        bottom: px(e.bottom as f32),
        left: px(e.left as f32),
    };
    let insets = WindowInsets {
        safe_area: edges(safe),
        ime: edges(ime),
    };
    let changed = window.insets.replace(insets.clone()) != insets;
    if resized {
        if window.renderable.get()
            && let Some(renderer) = window.renderer.borrow_mut().as_mut()
        {
            renderer.update_drawable_size(window.config().size);
        }
        emit!(window, resize, bounds.size, scale);
    }
    if changed {
        emit!(window, insets, insets);
    }
}
unsafe extern "C" fn touch(pointer: *mut c_void, id: u64, phase: u32, x: f64, y: f64, force: f64) {
    let Some(window) = (unsafe { window(pointer) }) else {
        return;
    };
    let phase = match phase {
        0 => TouchPhase::Started,
        1 => TouchPhase::Moved,
        2 => TouchPhase::Ended,
        _ => TouchPhase::Cancelled,
    };
    emit!(
        window,
        input,
        PlatformInput::Touch(TouchEvent {
            id: TouchId(id),
            phase,
            position: point(px(x as f32), px(y as f32)),
            predicted_position: None,
            force: (force >= 0.0).then_some(force as f32)
        })
    );
}

unsafe fn with_input<T>(
    pointer: *mut c_void,
    default: T,
    f: impl FnOnce(&mut PlatformInputHandler) -> T,
) -> T {
    let Some(window) = (unsafe { window(pointer) }) else {
        return default;
    };
    let generation = window.input_generation.get();
    let input = window.input.borrow_mut().take();
    let Some(mut input) = input else {
        return default;
    };
    let result = f(&mut input);
    if window.input_generation.get() == generation {
        *window.input.borrow_mut() = Some(input);
    }
    result
}
unsafe extern "C" fn text(
    pointer: *mut c_void,
    range: TextRange,
    output: *mut c_char,
    capacity: usize,
) -> usize {
    unsafe {
        with_input(pointer, 0, |input| {
            let Some(range) = range.into_range() else {
                return 0;
            };
            let Some(text) = input.text_for_range(range, &mut None) else {
                return 0;
            };
            if !output.is_null() && capacity >= text.len() {
                std::ptr::copy_nonoverlapping(text.as_ptr(), output.cast(), text.len());
            }
            text.len()
        })
    }
}
unsafe extern "C" fn text_length(p: *mut c_void) -> usize {
    unsafe { with_input(p, 0, |i| i.text_length_utf16().unwrap_or(0)) }
}
unsafe extern "C" fn selection(p: *mut c_void) -> TextRange {
    unsafe {
        with_input(p, TextRange::default(), |i| {
            i.selected_text_range(false)
                .map(|s| s.range.into())
                .unwrap_or_default()
        })
    }
}
unsafe extern "C" fn marked(p: *mut c_void) -> TextRange {
    unsafe {
        with_input(p, TextRange::default(), |i| {
            i.marked_text_range().map(Into::into).unwrap_or_default()
        })
    }
}
unsafe extern "C" fn select(p: *mut c_void, range: TextRange) {
    unsafe {
        with_input(p, (), |i| {
            if let Some(range) = range.into_range() {
                i.set_selected_text_range(range);
            }
        })
    };
}
unsafe fn input_string<'a>(text: *const c_char, length: usize) -> &'a str {
    if length == 0 {
        return "";
    }
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(text.cast(), length) })
        .expect("UIKit supplies valid UTF-8")
}
unsafe extern "C" fn replace(p: *mut c_void, range: TextRange, text: *const c_char, length: usize) {
    let text = unsafe { input_string(text, length) };
    unsafe { with_input(p, (), |i| i.replace_text_in_range(range.into_range(), text)) };
}
unsafe extern "C" fn mark(
    p: *mut c_void,
    text: *const c_char,
    length: usize,
    selection: TextRange,
) {
    let text = unsafe { input_string(text, length) };
    unsafe {
        with_input(p, (), |i| {
            i.replace_and_mark_text_in_range(None, text, selection.into_range())
        })
    };
}
unsafe extern "C" fn unmark(p: *mut c_void) {
    unsafe { with_input(p, (), PlatformInputHandler::unmark_text) };
}
unsafe extern "C" fn text_bounds(p: *mut c_void, range: TextRange) -> Rect {
    unsafe {
        with_input(p, Rect::default(), |i| {
            range
                .into_range()
                .and_then(|r| i.bounds_for_range(r))
                .map(rect)
                .unwrap_or_default()
        })
    }
}
unsafe extern "C" fn text_hit_test(p: *mut c_void, x: f64, y: f64) -> usize {
    unsafe {
        with_input(p, usize::MAX, |i| {
            i.character_index_for_point(point(px(x as f32), px(y as f32)))
                .unwrap_or(usize::MAX)
        })
    }
}

fn native_position(p: ffi::NativePosition) -> Option<NativeTextPosition> {
    p.valid.then_some(NativeTextPosition {
        utf16_offset: p.offset,
        affinity: if p.upstream {
            TextAffinity::Upstream
        } else {
            TextAffinity::Downstream
        },
    })
}
unsafe extern "C" fn position_for_point(
    p: *mut c_void,
    x: f64,
    y: f64,
    range: TextRange,
) -> ffi::NativePosition {
    let restriction = range.into_range();
    if range.valid && restriction.is_none() {
        return ffi::NativePosition::default();
    }
    unsafe {
        with_input(p, ffi::NativePosition::default(), |i| {
            i.native_position_for_point(point(px(x as f32), px(y as f32)), restriction)
                .map(ffi_position)
                .unwrap_or_default()
        })
    }
}
fn ffi_position(p: NativeTextPosition) -> ffi::NativePosition {
    ffi::NativePosition {
        offset: p.utf16_offset,
        upstream: p.affinity == TextAffinity::Upstream,
        valid: true,
    }
}
fn direction(d: u32) -> Option<TextNavigationDirection> {
    Some(match d {
        0 => TextNavigationDirection::Left,
        1 => TextNavigationDirection::Right,
        2 => TextNavigationDirection::Up,
        3 => TextNavigationDirection::Down,
        _ => return None,
    })
}
unsafe extern "C" fn native_selection(p: *mut c_void) -> ffi::NativeSelection {
    unsafe {
        with_input(p, ffi::NativeSelection::default(), |i| {
            i.native_selection()
                .map(|s| ffi::NativeSelection {
                    anchor: ffi_position(s.anchor),
                    head: ffi_position(s.head),
                })
                .unwrap_or_default()
        })
    }
}
unsafe extern "C" fn set_native_selection(p: *mut c_void, s: ffi::NativeSelection) -> bool {
    let (Some(anchor), Some(head)) = (native_position(s.anchor), native_position(s.head)) else {
        return false;
    };
    unsafe {
        with_input(p, false, |i| {
            i.set_native_selection(NativeTextSelection { anchor, head })
        })
    }
}
unsafe extern "C" fn move_position(
    p: *mut c_void,
    from: ffi::NativePosition,
    d: u32,
    offset: usize,
) -> ffi::NativePosition {
    let (Some(from), Some(d)) = (native_position(from), direction(d)) else {
        return ffi::NativePosition::default();
    };
    unsafe {
        with_input(p, ffi::NativePosition::default(), |i| {
            i.native_position_in_direction(from, d, offset)
                .map(ffi_position)
                .unwrap_or_default()
        })
    }
}
unsafe extern "C" fn farthest_position(
    p: *mut c_void,
    range: TextRange,
    d: u32,
) -> ffi::NativePosition {
    let (Some(range), Some(d)) = (range.into_range(), direction(d)) else {
        return ffi::NativePosition::default();
    };
    unsafe {
        with_input(p, ffi::NativePosition::default(), |i| {
            i.farthest_native_position(range, d)
                .map(ffi_position)
                .unwrap_or_default()
        })
    }
}
unsafe extern "C" fn position_bounds(
    p: *mut c_void,
    position: ffi::NativePosition,
    output: *mut Rect,
) -> bool {
    let Some(position) = native_position(position) else {
        return false;
    };
    unsafe {
        with_input(p, false, |i| {
            let Some(bounds) = i.native_position_bounds(position) else {
                return false;
            };
            if output.is_null() {
                return false;
            }
            *output = rect(bounds);
            true
        })
    }
}
unsafe extern "C" fn selection_rects(
    p: *mut c_void,
    range: TextRange,
    output: *mut ffi::SelectionRect,
    capacity: usize,
) -> usize {
    let Some(range) = range.into_range() else {
        return 0;
    };
    unsafe {
        with_input(p, 0, |i| {
            let rects = i.selection_rects_for_range(range);
            if !output.is_null() && capacity >= rects.len() {
                for (index, r) in rects.iter().enumerate() {
                    output.add(index).write(ffi::SelectionRect {
                        bounds: rect(r.bounds),
                        rtl: r.writing_direction == TextWritingDirection::RightToLeft,
                        contains_start: r.contains_start,
                        contains_end: r.contains_end,
                        vertical: r.is_vertical,
                    });
                }
            }
            rects.len()
        })
    }
}
unsafe extern "C" fn grapheme(p: *mut c_void, position: usize) -> TextRange {
    unsafe {
        with_input(p, TextRange::default(), |i| {
            i.grapheme_range_at(position)
                .map(Into::into)
                .unwrap_or_default()
        })
    }
}
unsafe extern "C" fn writing_direction(p: *mut c_void, position: usize) -> i32 {
    unsafe {
        with_input(p, -1, |i| match i.base_writing_direction(position) {
            Some(TextWritingDirection::LeftToRight) => 0,
            Some(TextWritingDirection::RightToLeft) => 1,
            None => -1,
        })
    }
}
unsafe extern "C" fn set_writing_direction(p: *mut c_void, range: TextRange, rtl: bool) -> bool {
    let Some(range) = range.into_range() else {
        return false;
    };
    unsafe {
        with_input(p, false, |i| {
            i.set_base_writing_direction(
                if rtl {
                    TextWritingDirection::RightToLeft
                } else {
                    TextWritingDirection::LeftToRight
                },
                range,
            )
        })
    }
}

unsafe extern "C" fn system_event(pointer: *mut c_void, event: u32, text: *const c_char) {
    let state = unsafe { &*pointer.cast::<State>() };
    match event {
        0 => {
            if text.is_null() {
                return;
            }
            let url = unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned();
            let callback = state.urls.borrow_mut().take();
            if let Some(mut callback) = callback {
                callback(vec![url]);
                *state.urls.borrow_mut() = Some(callback);
            }
        }
        1 | 2 => {
            let slot = if event == 1 {
                &state.thermal
            } else {
                &state.input_mode
            };
            let callback = slot.borrow_mut().take();
            if let Some(mut callback) = callback {
                callback();
                *slot.borrow_mut() = Some(callback);
            }
        }
        3 => {
            if let Some(window) = unsafe { window(pointer) } {
                emit!(window, appearance);
            }
        }
        4 if !text.is_null() => {
            *state.error.borrow_mut() = Some(
                unsafe { CStr::from_ptr(text) }
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{direction, ffi_position, load_text_system, native_position};
    use crate::ffi;
    use gpui::{NativeTextPosition, TextAffinity, TextNavigationDirection};
    use std::borrow::Cow;

    #[test]
    fn caller_font_authority_rejects_empty_invalid_and_missing_family() {
        assert!(load_text_system("Geist", vec![]).is_err());
        assert!(load_text_system("Geist", vec![Cow::Borrowed(b"invalid font")]).is_err());
        let font = include_bytes!("../../gpui-kit-assets/assets/fonts/Geist.ttf");
        assert!(load_text_system("Absent family", vec![Cow::Borrowed(font)]).is_err());
        let text =
            load_text_system("Geist", vec![Cow::Borrowed(font)]).expect("supplied Geist font");
        assert!(text.all_font_names().iter().any(|name| name == "Geist"));
    }

    #[test]
    fn abi_preserves_both_affinities_at_same_utf16_offset() {
        for affinity in [TextAffinity::Upstream, TextAffinity::Downstream] {
            let position = NativeTextPosition {
                utf16_offset: 3,
                affinity,
            };
            assert_eq!(native_position(ffi_position(position)), Some(position));
        }
        assert!(native_position(ffi::NativePosition::default()).is_none());
        assert_eq!(direction(0), Some(TextNavigationDirection::Left));
        assert_eq!(direction(3), Some(TextNavigationDirection::Down));
        assert_eq!(direction(4), None);
    }
}
