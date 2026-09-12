use super::*;

pub(super) struct AndroidWindow {
    pub handle: AnyWindowHandle,
    pub renderer: RefCell<WgpuRenderer>,
    pub surface: RefCell<Option<NativeSurface>>,
    pub input: RefCell<Option<PlatformInputHandler>>,
    pub input_generation: Cell<i64>,
    pub presented_frames: Cell<i64>,
    pub frame: Callback<dyn FnMut(RequestFrameOptions)>,
    pub event: Callback<dyn FnMut(PlatformInput) -> DispatchEventResult>,
    pub active: Callback<dyn FnMut(bool)>,
    pub resize: Callback<dyn FnMut(Size<Pixels>, f32)>,
    pub appearance_changed: Callback<dyn FnMut()>,
    pub should_close: Callback<dyn FnMut() -> bool>,
    pub close: Callback<dyn FnOnce()>,
    pub back: Callback<dyn FnMut()>,
    pub back_enabled: Cell<bool>,
    pub insets: RefCell<WindowInsets>,
    pub insets_changed: Callback<dyn FnMut(WindowInsets)>,
    pub contacts: RefCell<std::collections::BTreeMap<u64, Point<Pixels>>>,
    pub a11y: RefCell<accessibility::Accessibility>,
    pub a11y_callbacks: RefCell<Option<A11yCallbacks>>,
}
impl AndroidWindow {
    pub fn new(handle: AnyWindowHandle, renderer: WgpuRenderer, surface: NativeSurface) -> Self {
        Self {
            handle,
            renderer: RefCell::new(renderer),
            surface: RefCell::new(Some(surface)),
            input: RefCell::new(None),
            input_generation: Cell::new(1),
            presented_frames: Cell::new(0),
            frame: RefCell::new(None),
            event: RefCell::new(None),
            active: RefCell::new(None),
            resize: RefCell::new(None),
            appearance_changed: RefCell::new(None),
            should_close: RefCell::new(None),
            close: RefCell::new(None),
            back: RefCell::new(None),
            back_enabled: Cell::new(false),
            insets: RefCell::new(WindowInsets::default()),
            insets_changed: RefCell::new(None),
            contacts: RefCell::new(Default::default()),
            a11y: RefCell::new(Default::default()),
            a11y_callbacks: RefCell::new(None),
        }
    }
    pub fn emit(&self, event: PlatformInput) -> DispatchEventResult {
        let callback = self.event.take();
        if let Some(mut callback) = callback {
            let result = callback(event);
            self.event.replace(Some(callback));
            result
        } else {
            DispatchEventResult::default()
        }
    }
    pub fn cancel_contacts(&self) {
        let contacts = std::mem::take(&mut *self.contacts.borrow_mut());
        for (id, position) in contacts {
            self.emit(PlatformInput::Touch(TouchEvent {
                id: TouchId(id),
                phase: TouchPhase::Cancelled,
                position,
                ..Default::default()
            }));
        }
    }
    pub fn with_input<T>(&self, f: impl FnOnce(&mut PlatformInputHandler) -> T) -> Option<T> {
        let mut handler = self.input.take()?;
        let result = f(&mut handler);
        // A focus change during the handler may install a replacement.
        if self.input.borrow().is_none() {
            self.input.replace(Some(handler));
        }
        Some(result)
    }
}

pub(super) struct WindowHandle(pub Rc<AndroidWindow>);
impl HasWindowHandle for WindowHandle {
    fn window_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        let surface = self.0.surface.borrow();
        let raw = surface
            .as_ref()
            .ok_or(HandleError::Unavailable)?
            .window_handle()?
            .as_raw();
        // The Activity calls surfaceDestroyed synchronously on this UI thread;
        // no lifecycle callback can run during a borrowed window handle.
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}
impl HasDisplayHandle for WindowHandle {
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::android())
    }
}
impl PlatformWindow for WindowHandle {
    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND {
        unsupported("Windows HWND; an Android surface is not a Win32 window")
    }
    fn check_window_operation(
        &self,
        operation: WindowOperation,
    ) -> std::result::Result<(), PlatformOperationError> {
        if operation != WindowOperation::Minimize {
            return Err(PlatformOperationError::Unsupported(
                "Android Activity window command",
            ));
        }
        if self.0.surface.borrow().is_none() {
            return Err(PlatformOperationError::Unavailable(
                "Android surface is detached",
            ));
        }
        Ok(())
    }
    fn bounds(&self) -> Bounds<Pixels> {
        platform().display.bounds()
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
        unsupported("resizing Activity windows; Android owns window metrics")
    }
    fn scale_factor(&self) -> f32 {
        platform().scale.get()
    }
    fn appearance(&self) -> WindowAppearance {
        platform().window_appearance()
    }
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(platform().display.clone())
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
    fn set_input_handler(&mut self, handler: PlatformInputHandler) {
        self.0.input.replace(Some(handler));
    }
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.0.input.take()
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
        platform().java.call("focusContent", "()V", &[]);
    }
    fn is_active(&self) -> bool {
        platform().active.get()
    }
    fn is_hovered(&self) -> bool {
        false
    }
    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }
    fn set_title(&mut self, title: &str) {
        platform().java.string("setTaskTitle", title);
    }
    fn set_background_appearance(&self, appearance: WindowBackgroundAppearance) {
        if appearance != WindowBackgroundAppearance::Opaque {
            unsupported("transparent Activity surfaces")
        }
    }
    fn minimize(&self) {
        platform().hide();
    }
    fn zoom(&self) {
        unsupported("desktop window zoom")
    }
    fn request_close(&self) {
        let callback = self.0.should_close.take();
        let allowed = if let Some(mut cb) = callback {
            let allowed = cb();
            self.0.should_close.replace(Some(cb));
            allowed
        } else {
            true
        };
        if allowed {
            platform().quit();
        }
    }
    fn toggle_fullscreen(&self) {
        unsupported("programmatic fullscreen; configure Activity policy in the host")
    }
    fn is_fullscreen(&self) -> bool {
        false
    }
    fn on_request_frame(&self, cb: Box<dyn FnMut(RequestFrameOptions)>) {
        self.0.frame.replace(Some(cb));
    }
    fn on_input(&self, cb: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.0.event.replace(Some(cb));
    }
    fn on_active_status_change(&self, cb: Box<dyn FnMut(bool)>) {
        self.0.active.replace(Some(cb));
    }
    fn on_hover_status_change(&self, _: Box<dyn FnMut(bool)>) { /* A touch-only window has no hover transitions. */
    }
    fn on_resize(&self, cb: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.0.resize.replace(Some(cb));
    }
    fn on_moved(&self, _: Box<dyn FnMut()>) { /* Activity content origin remains zero. */
    }
    fn on_should_close(&self, cb: Box<dyn FnMut() -> bool>) {
        self.0.should_close.replace(Some(cb));
    }
    fn on_hit_test_window_control(
        &self,
        _: Box<dyn FnMut(Point<Pixels>) -> Option<WindowControlArea>>,
    ) { /* Android has no client titlebar. */
    }
    fn on_close(&self, cb: Box<dyn FnOnce()>) {
        self.0.close.replace(Some(cb));
    }
    fn on_appearance_changed(&self, cb: Box<dyn FnMut()>) {
        self.0.appearance_changed.replace(Some(cb));
    }
    fn draw(&self, scene: &Scene) {
        if platform().active.get() && self.0.surface.borrow().is_some() {
            let mut renderer = self.0.renderer.borrow_mut();
            if renderer.device_lost() {
                let surface = self.0.surface.borrow();
                renderer
                    .recover(surface.as_ref().expect("attached surface"))
                    .expect("Android GPU recovery failed");
            }
            if renderer.draw(scene) {
                self.0
                    .presented_frames
                    .set(self.0.presented_frames.get() + 1);
            }
        }
    }
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.0.renderer.borrow().sprite_atlas().clone()
    }
    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }
    fn gpu_specs(&self) -> Option<GpuSpecs> {
        Some(self.0.renderer.borrow().gpu_specs())
    }
    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        let s = self.scale_factor();
        platform().java.call(
            "updateCaret",
            "(FFFF)V",
            &[
                JValue::Float(f32::from(bounds.origin.x) * s),
                JValue::Float(f32::from(bounds.origin.y) * s),
                JValue::Float(f32::from(bounds.size.width) * s),
                JValue::Float(f32::from(bounds.size.height) * s),
            ],
        );
    }
    fn insets(&self) -> WindowInsets {
        self.0.insets.borrow().clone()
    }
    fn on_insets_changed(&self, cb: Box<dyn FnMut(WindowInsets)>) {
        self.0.insets_changed.replace(Some(cb));
    }
    fn set_back_handler(&self, cb: Box<dyn FnMut()>) {
        self.0.back.replace(Some(cb));
    }
    fn set_back_enabled(&self, enabled: bool) {
        self.0.back_enabled.set(enabled);
        platform()
            .java
            .call("setBackEnabled", "(Z)V", &[JValue::Bool(enabled.into())]);
    }
    fn show_soft_keyboard(&self) {
        platform().java.call("showKeyboard", "()V", &[]);
    }
    fn hide_soft_keyboard(&self) {
        platform().java.call("hideKeyboard", "()V", &[]);
    }
    fn text_input_state_changed(&self, change: TextInputStateChange) {
        let code = match change {
            TextInputStateChange::FocusGained => 0,
            TextInputStateChange::FocusLost => 1,
            TextInputStateChange::SelectionChanged => 2,
            TextInputStateChange::ContentChanged => 3,
            TextInputStateChange::OptionsChanged => 4,
        };
        if code <= 1 {
            self.0
                .input_generation
                .set(self.0.input_generation.get() + 1);
        }
        // Java posts this update rather than calling JNI while GPUI holds App.
        platform()
            .java
            .call("inputChanged", "(I)V", &[JValue::Int(code)]);
    }
    fn window_controls(&self) -> WindowControls {
        WindowControls {
            fullscreen: false,
            maximize: false,
            minimize: true,
            window_menu: false,
        }
    }
    fn a11y_init(&self, callbacks: A11yCallbacks) {
        self.0.a11y_callbacks.replace(Some(callbacks));
    }
    fn a11y_tree_update(&self, update: accesskit::TreeUpdate) {
        self.0.a11y.borrow_mut().update(update);
        platform()
            .java
            .string("updateAccessibility", &self.0.a11y.borrow().json());
    }
    fn backdrop_luminance(&self, id: u32) -> Option<f32> {
        self.0.renderer.borrow_mut().backdrop_luminance(id)
    }

    fn backdrop_statistics(&self, id: u32) -> Option<gpui::BackdropStatistics> {
        self.0.renderer.borrow_mut().backdrop_statistics(id)
    }
}
