use anyhow::{Result, bail};
use futures::channel::oneshot;
use gpui::*;
use gpui_wgpu::{GpuContext, WgpuRenderer, WgpuSurfaceConfig, wgpu};
use jni::{
    JNIEnv, JavaVM,
    objects::{GlobalRef, JObject, JValue},
};
use raw_window_handle::*;
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

mod events;
mod window;
use crate::accessibility;

type Callback<T> = RefCell<Option<Box<T>>>;
type ActionCallback<R> = Callback<dyn FnMut(&dyn Action) -> R>;

thread_local! {
    static PLATFORM: RefCell<Option<Rc<AndroidPlatform>>> = const { RefCell::new(None) };
    static APPLICATION: RefCell<Option<ApplicationHandle>> = const { RefCell::new(None) };
}

pub(super) fn platform() -> Rc<AndroidPlatform> {
    PLATFORM.with(|p| {
        p.borrow()
            .as_ref()
            .expect("Activity has not initialized GPUI")
            .clone()
    })
}

/// Installs one Activity on its Android UI thread. The Activity must use the
/// accompanying GpuiActivity Java class. Call once from its nativeCreate method.
/// Return Application::run_embedded's handle; this host retains it until
/// Activity destruction. Android continues owning the run loop.
pub fn initialize(
    env: &mut JNIEnv<'_>,
    activity: JObject<'_>,
    launch: impl FnOnce(Rc<AndroidPlatform>) -> ApplicationHandle,
) -> Result<()> {
    anyhow::ensure!(
        PLATFORM.with(|p| p.borrow().is_none()),
        "only one GPUI Activity per UI thread is supported"
    );
    let java = Arc::new(Java {
        vm: env.get_java_vm()?,
        activity: env.new_global_ref(activity)?,
    });
    let appearance = crate::android_appearance(
        env.call_method(java.activity.as_obj(), "uiMode", "()I", &[])?
            .i()?,
    );
    let dispatcher = Arc::new(Dispatcher::new(java.clone()));
    let text = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        "IBM Plex Sans",
    ));
    text.add_fonts(vec![std::borrow::Cow::Borrowed(include_bytes!(
        "../../gpui_web/assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"
    ))])?;
    let platform = Rc::new(AndroidPlatform {
        java,
        dispatcher,
        text,
        window: RefCell::new(None),
        launch: RefCell::new(None),
        lifecycle: RefCell::new(None),
        memory: RefCell::new(None),
        quit: RefCell::new(None),
        menu_action: RefCell::new(None),
        menu_open: RefCell::new(None),
        menu_validate: RefCell::new(None),
        thermal_changed: RefCell::new(None),
        keyboard_changed: RefCell::new(None),
        display: Rc::new(AndroidDisplay {
            bounds: Cell::new(Bounds::default()),
        }),
        gpu: Rc::new(RefCell::new(None)),
        surface: RefCell::new(None),
        presentation: RefCell::new(crate::SurfaceLifecycle::default()),
        size: Cell::new(Size::default()),
        scale: Cell::new(1.),
        active: Cell::new(false),
        appearance: Cell::new(appearance),
    });
    PLATFORM.with(|p| p.replace(Some(platform.clone())));
    let application = launch(platform);
    APPLICATION.with(|app| app.replace(Some(application)));
    Ok(())
}

pub(super) struct Java {
    vm: JavaVM,
    activity: GlobalRef,
}
impl Java {
    fn call(&self, name: &str, signature: &str, args: &[JValue<'_, '_>]) {
        let mut env = self
            .vm
            .attach_current_thread()
            .expect("attach Android thread");
        if let Err(error) = env.call_method(self.activity.as_obj(), name, signature, args) {
            // Propagate a Java error at JNI boundaries rather than silently
            // reporting that a platform request succeeded.
            panic!("Android host {name} failed: {error}");
        }
    }
    fn string(&self, name: &str, value: &str) {
        let mut env = self
            .vm
            .attach_current_thread()
            .expect("attach Android thread");
        let string = env.new_string(value).expect("allocate Java string");
        env.call_method(
            self.activity.as_obj(),
            name,
            "(Ljava/lang/String;)V",
            &[JValue::Object(&string)],
        )
        .expect("call Android string method");
    }
}

struct Dispatcher {
    java: Arc<Java>,
    main: std::thread::ThreadId,
    tx: mpsc::Sender<RunnableVariant>,
    rx: Mutex<mpsc::Receiver<RunnableVariant>>,
    background: PriorityQueueSender<RunnableVariant>,
    timers: mpsc::Sender<(std::time::Instant, RunnableVariant)>,
}
impl Dispatcher {
    fn new(java: Arc<Java>) -> Self {
        let (tx, rx) = mpsc::channel();
        let (background, receiver) = PriorityQueueReceiver::<RunnableVariant>::new();
        for _ in 0..2 {
            let receiver = receiver.clone();
            std::thread::spawn(move || {
                for runnable in receiver.iter() {
                    runnable.run();
                }
            });
        }
        let (timers, timer_rx) = mpsc::channel::<(std::time::Instant, RunnableVariant)>();
        std::thread::spawn(move || {
            let mut pending: Vec<(std::time::Instant, RunnableVariant)> = Vec::new();
            loop {
                pending.sort_by_key(|item| std::cmp::Reverse(item.0));
                let next = pending
                    .last()
                    .map(|item| item.0.saturating_duration_since(std::time::Instant::now()));
                let received = match next {
                    Some(wait) => timer_rx.recv_timeout(wait),
                    None => timer_rx
                        .recv()
                        .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
                };
                match received {
                    Ok(item) => pending.push(item),
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Some((_, runnable)) = pending.pop() {
                            runnable.run();
                        }
                    }
                }
            }
        });
        Self {
            java,
            main: std::thread::current().id(),
            tx,
            rx: Mutex::new(rx),
            background,
            timers,
        }
    }
    fn drain(&self) {
        // Do not hold the receiver mutex while invoking application code.
        loop {
            let runnable = self
                .rx
                .lock()
                .expect("Android main queue mutex poisoned")
                .try_recv()
                .ok();
            match runnable {
                Some(runnable) => {
                    runnable.run();
                }
                None => break,
            }
        }
    }
}
impl PlatformDispatcher for Dispatcher {
    fn is_main_thread(&self) -> bool {
        self.main == std::thread::current().id()
    }
    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        self.background
            .send(priority, runnable)
            .expect("background dispatcher stopped");
    }
    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, _: Priority) {
        self.tx.send(runnable).expect("UI dispatcher stopped");
        self.java.call("wake", "()V", &[]);
    }
    fn dispatch_after(&self, delay: Duration, runnable: RunnableVariant) {
        self.timers
            .send((std::time::Instant::now() + delay, runnable))
            .expect("timer dispatcher stopped");
    }
    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        std::thread::spawn(f);
    }
}

#[derive(Debug)]
struct AndroidDisplay {
    bounds: Cell<Bounds<Pixels>>,
}
impl PlatformDisplay for AndroidDisplay {
    fn id(&self) -> DisplayId {
        DisplayId::new(1)
    }
    fn uuid(&self) -> Result<uuid::Uuid> {
        Ok(uuid::Uuid::from_u128(1))
    }
    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds.get()
    }
    fn visible_bounds(&self) -> Bounds<Pixels> {
        self.bounds()
    }
    fn default_bounds(&self) -> Bounds<Pixels> {
        self.bounds()
    }
}

/// Acquires ANativeWindow from a Java Surface; the final clone releases the NDK
/// reference only after the renderer has detached its wgpu Surface.
#[derive(Clone, Debug)]
struct NativeSurface(Arc<NativeWindow>);
#[derive(Debug)]
struct NativeWindow(usize);
#[cfg(target_os = "android")]
#[link(name = "android")]
unsafe extern "C" {
    #[link_name = "ANativeWindow_fromSurface"]
    fn native_window_from_surface(
        env: *mut jni::sys::JNIEnv,
        surface: jni::sys::jobject,
    ) -> *mut std::ffi::c_void;
    #[link_name = "ANativeWindow_release"]
    fn native_window_release(window: *mut std::ffi::c_void);
}
// Host-check builds type-check the real bridge but cannot acquire NDK resources.
#[cfg(not(target_os = "android"))]
unsafe fn native_window_from_surface(
    _: *mut jni::sys::JNIEnv,
    _: jni::sys::jobject,
) -> *mut std::ffi::c_void {
    unsupported("NDK surface acquisition outside Android")
}
#[cfg(not(target_os = "android"))]
unsafe fn native_window_release(_: *mut std::ffi::c_void) {
    unsupported("NDK surface release outside Android")
}
impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe { native_window_release(self.0 as *mut _) };
    }
}
impl HasWindowHandle for NativeSurface {
    fn window_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        let handle = AndroidNdkWindowHandle::new(
            std::ptr::NonNull::new(self.0.0 as *mut _).ok_or(HandleError::Unavailable)?,
        );
        // The Arc owns the NDK reference for the lifetime of this borrow.
        Ok(unsafe {
            raw_window_handle::WindowHandle::borrow_raw(RawWindowHandle::AndroidNdk(handle))
        })
    }
}
impl HasDisplayHandle for NativeSurface {
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::android())
    }
}

/// One Android Activity and one top-level GPUI window. Desktop-only APIs fail
/// explicitly; Activity identity, task placement and permissions belong to the host.
pub struct AndroidPlatform {
    java: Arc<Java>,
    dispatcher: Arc<Dispatcher>,
    text: Arc<dyn PlatformTextSystem>,
    window: RefCell<Option<Rc<window::AndroidWindow>>>,
    display: Rc<AndroidDisplay>,
    launch: Callback<dyn FnOnce()>,
    lifecycle: Callback<dyn FnMut(AppLifecyclePhase)>,
    memory: Callback<dyn FnMut()>,
    quit: Callback<dyn FnMut()>,
    // GPUI registers these during App construction. No menu events are emitted;
    // attempts to install a desktop menu are refused by set_menus.
    menu_action: ActionCallback<()>,
    menu_open: Callback<dyn FnMut()>,
    menu_validate: ActionCallback<bool>,
    thermal_changed: Callback<dyn FnMut()>,
    keyboard_changed: Callback<dyn FnMut()>,
    presentation: RefCell<crate::SurfaceLifecycle>,
    gpu: GpuContext,
    surface: RefCell<Option<NativeSurface>>,
    size: Cell<Size<DevicePixels>>,
    scale: Cell<f32>,
    active: Cell<bool>,
    appearance: Cell<WindowAppearance>,
}

impl AndroidPlatform {
    /// Returns the Activity's installed platform on its UI thread. An Android
    /// platform cannot be created without an Activity or in headless mode.
    pub fn current() -> Result<Rc<Self>> {
        PLATFORM.with(|p| {
            p.borrow().clone().ok_or_else(|| {
                anyhow::anyhow!("Call gpui_android::initialize from GpuiActivity first")
            })
        })
    }
}

fn unsupported(operation: &str) -> ! {
    panic!("Android GPUI does not support {operation}; see crates/gpui_android/README.md")
}
fn refusal<T>(operation: &str) -> oneshot::Receiver<Result<Option<T>>> {
    let (tx, rx) = oneshot::channel();
    let _ = tx.send(Err(anyhow::anyhow!(
        "Android GPUI does not support {operation}"
    )));
    rx
}
struct AndroidKeyboard;
impl PlatformKeyboardLayout for AndroidKeyboard {
    fn id(&self) -> &str {
        "android-ime"
    }
    fn name(&self) -> &str {
        "Android input method"
    }
}

impl Platform for AndroidPlatform {
    fn check_app_operation(
        &self,
        operation: AppOperation,
    ) -> std::result::Result<(), PlatformOperationError> {
        if !matches!(
            operation,
            AppOperation::Quit | AppOperation::Hide | AppOperation::SetCursorStyle
        ) {
            return Err(PlatformOperationError::Unsupported(
                "Android application command",
            ));
        }
        if PLATFORM.with(|p| p.borrow().is_none()) {
            return Err(PlatformOperationError::Unavailable(
                "Android Activity was destroyed",
            ));
        }
        Ok(())
    }
    fn background_executor(&self) -> BackgroundExecutor {
        BackgroundExecutor::new(self.dispatcher.clone())
    }
    fn foreground_executor(&self) -> ForegroundExecutor {
        ForegroundExecutor::new(self.dispatcher.clone())
    }
    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text.clone()
    }
    fn run(&self, launch: Box<dyn FnOnce()>) {
        self.launch.replace(Some(launch));
    }
    fn quit(&self) {
        self.java.call("finish", "()V", &[]);
    }
    fn restart(&self, _: Option<PathBuf>) {
        unsupported("process restart")
    }
    fn activate(&self, _: bool) {
        unsupported("foregrounding an Activity without user action")
    }
    fn hide(&self) {
        self.java.call("backgroundTask", "()V", &[]);
    }
    fn hide_other_apps(&self) {
        unsupported("hiding other applications")
    }
    fn unhide_other_apps(&self) {
        unsupported("unhiding other applications")
    }
    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        vec![self.display.clone()]
    }
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.display.clone())
    }
    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.window
            .borrow()
            .as_ref()
            .filter(|_| self.active.get())
            .map(|w| w.handle)
    }
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        params: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        anyhow::ensure!(
            matches!(params.kind, WindowKind::Normal),
            "Android supports one normal Activity window, not popups or panels"
        );
        anyhow::ensure!(
            self.window.borrow().is_none(),
            "Android Activity already owns a GPUI window"
        );
        let surface = self.surface.borrow().clone().ok_or_else(|| {
            anyhow::anyhow!("open Android windows from the launch callback, after surface creation")
        })?;
        let renderer = WgpuRenderer::new(
            self.gpu.clone(),
            &surface,
            surface_config(self.size.get()),
            None,
        )?;
        let window = Rc::new(window::AndroidWindow::new(handle, renderer, surface));
        self.window.replace(Some(window.clone()));
        Ok(Box::new(window::WindowHandle(window)))
    }
    fn window_appearance(&self) -> WindowAppearance {
        self.appearance.get()
    }
    fn open_url(&self, url: &str) {
        self.java.string("openUrl", url);
    }
    fn on_open_urls(&self, _: Box<dyn FnMut(Vec<String>)>) {
        unsupported("incoming deep links")
    }
    fn register_url_scheme(&self, _: &str) -> Task<Result<()>> {
        Task::ready(Err(anyhow::anyhow!(
            "Declare Android intent filters in AndroidManifest.xml"
        )))
    }
    fn prompt_for_paths(
        &self,
        _: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        refusal("document picker")
    }
    fn prompt_for_new_path(
        &self,
        _: &Path,
        _: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        refusal("document creation picker")
    }
    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }
    fn reveal_path(&self, _: &Path) {
        unsupported("revealing filesystem paths")
    }
    fn open_with_system(&self, _: &Path) {
        unsupported("opening filesystem paths without a ContentProvider")
    }
    fn on_quit(&self, cb: Box<dyn FnMut()>) {
        self.quit.replace(Some(cb));
    }
    fn on_reopen(&self, _: Box<dyn FnMut()>) {
        unsupported("desktop reopen events")
    }
    fn on_system_wake(&self, _: Box<dyn FnMut()>) {
        unsupported("system wake events; use lifecycle")
    }
    fn on_app_lifecycle(&self, cb: Box<dyn FnMut(AppLifecyclePhase)>) {
        self.lifecycle.replace(Some(cb));
    }
    fn on_memory_warning(&self, cb: Box<dyn FnMut()>) {
        self.memory.replace(Some(cb));
    }
    fn set_menus(&self, _: Vec<Menu>, _: &Keymap) {
        unsupported("desktop application menus")
    }
    fn set_dock_menu(&self, _: Vec<MenuItem>, _: &Keymap) {
        unsupported("dock menus")
    }
    fn on_app_menu_action(&self, cb: Box<dyn FnMut(&dyn Action)>) {
        self.menu_action.replace(Some(cb));
    }
    fn on_will_open_app_menu(&self, cb: Box<dyn FnMut()>) {
        self.menu_open.replace(Some(cb));
    }
    fn on_validate_app_menu_command(&self, cb: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.menu_validate.replace(Some(cb));
    }
    fn thermal_state(&self) -> ThermalState {
        let mut env = self
            .java
            .vm
            .attach_current_thread()
            .expect("attach Android thread");
        let status = env
            .call_method(self.java.activity.as_obj(), "thermalStatus", "()I", &[])
            .expect("query thermal status")
            .i()
            .expect("integer status");
        match status {
            0 => ThermalState::Nominal,
            1 => ThermalState::Fair,
            2 => ThermalState::Serious,
            _ => ThermalState::Critical,
        }
    }
    fn on_thermal_state_change(&self, cb: Box<dyn FnMut()>) {
        self.thermal_changed.replace(Some(cb));
    }
    fn app_path(&self) -> Result<PathBuf> {
        bail!("Android packages are not executable filesystem paths")
    }
    fn path_for_auxiliary_executable(&self, _: &str) -> Result<PathBuf> {
        bail!("Android auxiliary executables are unsupported")
    }
    fn set_cursor_style(&self, style: CursorStyle) {
        self.java.string("setPointerStyle", &format!("{style:?}"));
    }
    fn hide_cursor_until_mouse_moves(&self) {
        unsupported("hardware pointer visibility")
    }
    fn is_cursor_visible(&self) -> bool {
        false
    }
    fn should_auto_hide_scrollbars(&self) -> bool {
        true
    }
    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        let mut env = self
            .java
            .vm
            .attach_current_thread()
            .expect("attach Android thread");
        let value = env
            .call_method(
                self.java.activity.as_obj(),
                "readClipboard",
                "()Ljava/lang/String;",
                &[],
            )
            .expect("read clipboard")
            .l()
            .expect("clipboard string");
        if value.is_null() {
            return None;
        }
        let value: String = env
            .get_string(&value.into())
            .expect("clipboard UTF16")
            .into();
        Some(ClipboardItem::new_string(value))
    }
    fn write_to_clipboard(&self, item: ClipboardItem) {
        if let Some(text) = item.text() {
            self.java.string("writeClipboard", &text);
        } else {
            unsupported("non-text clipboard items")
        }
    }
    // Desktop-only trait requirements when compiling the adapter for host checks.
    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        unsupported("macOS find pasteboard")
    }
    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, _: ClipboardItem) {
        unsupported("macOS find pasteboard")
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        None
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, _: ClipboardItem) {
        unsupported("primary clipboard")
    }
    fn write_credentials(&self, _: &str, _: &str, _: &[u8]) -> Task<Result<()>> {
        Task::ready(Err(anyhow::anyhow!(
            "Android credential storage is caller-owned"
        )))
    }
    fn read_credentials(&self, _: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Err(anyhow::anyhow!(
            "Android credential storage is caller-owned"
        )))
    }
    fn delete_credentials(&self, _: &str) -> Task<Result<()>> {
        Task::ready(Err(anyhow::anyhow!(
            "Android credential storage is caller-owned"
        )))
    }
    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(AndroidKeyboard)
    }
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }
    fn on_keyboard_layout_change(&self, cb: Box<dyn FnMut()>) {
        self.keyboard_changed.replace(Some(cb));
    }
}

fn surface_config(size: Size<DevicePixels>) -> WgpuSurfaceConfig {
    WgpuSurfaceConfig {
        size,
        transparent: false,
        color_space: wgpu::SurfaceColorSpace::Auto,
        preferred_present_mode: None,
    }
}

#[cfg(all(test, not(target_os = "android")))]
mod host_tests {
    #[test]
    fn ndk_surface_operations_refuse_on_non_android_hosts() {
        assert!(
            std::panic::catch_unwind(|| unsafe {
                super::native_window_from_surface(std::ptr::null_mut(), std::ptr::null_mut())
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| unsafe {
                super::native_window_release(std::ptr::null_mut())
            })
            .is_err()
        );
    }
}
