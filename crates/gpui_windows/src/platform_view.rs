//! Hosting of native child windows inside GPUI windows.
//!
//! GPUI renders through DirectComposition into a window created with
//! `WS_EX_NOREDIRECTIONBITMAP`. Such a window has no redirection surface, and a
//! redirection surface is the only place the desktop composes child `HWND`s
//! into — so a child window parented straight to a GPUI window is never drawn,
//! however its styles and z-order are set.
//!
//! Hosted views therefore live in a *view host*: one owned popup window per
//! GPUI window, created lazily on the first attach. A popup has a redirection
//! surface of its own, so its children compose normally, and Windows keeps an
//! owned popup above its owner and below topmost windows. That is exactly the
//! layer a hosted view needs: above everything GPUI's base renderer drew, below
//! the topmost popups GPUI opens for menus and tooltips.
//!
//! The host is clipped by a window region to the union of the rectangles GPUI
//! laid its views out at, so it covers no pixel a hosted view does not own.
//! Each child also carries its own child-local region: a host union cannot
//! isolate overlapping children. Regions clip drawing and native hit testing
//! without changing the child's full layout dimensions.
//!
//! Everything in [`plan`] is free of Win32 calls: the sequencing of the
//! reparent, restyle and region work — and the geometry the host derives from a
//! frame — is decided there so it can be tested without a desktop.

use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    rc::Rc,
};

use anyhow::{Context as _, Result};
use gpui::{
    PlatformViewHandle, PlatformViewHosting, PlatformViewId, PlatformViewUpdate,
    platform_view_physical_bounds,
};
use gpui_util::ResultExt;
use windows::{
    Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*},
    core::*,
};

use crate::{get_module_handle, get_window_long, set_window_long};

use plan::{PhysicalRect, SavedViewState, ViewOp};

/// The Win32 state a hosted view had before GPUI took over its frame, plus the
/// window that state belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
struct HostedView {
    /// Retains any caller-owned controller until the child is detached. The
    /// previous rendered frame is cleared before this host receives its detach
    /// update, so that frame cannot own this lifetime on the host's behalf.
    handle: PlatformViewHandle,
    saved: SavedViewState,
    attached: Cell<bool>,
    // Plans borrow this handle. Only this shared owner deletes the saved copy;
    // SetWindowRgn receives a fresh copy, including on restoration retries.
    _region: Rc<SavedRegion>,
}

#[derive(Debug, PartialEq, Eq)]
struct SavedRegion(Option<isize>);

impl Drop for SavedRegion {
    fn drop(&mut self) {
        release_region(self.0);
    }
}

/// The owned popup a GPUI window parks its hosted views in, and the bookkeeping
/// that says where each of them belongs.
pub(crate) struct PlatformViewHost {
    /// The GPUI window whose client area hosted views are laid out in.
    owner: HWND,
    /// The popup window hosted views are parented to; created on first attach.
    host: Cell<Option<HWND>>,
    hosting: RefCell<PlatformViewHosting<HostedView>>,
    /// The rectangles the last frame placed views at, in client pixels, kept so
    /// the host can be re-clipped when the window moves or resizes without a
    /// frame having been drawn in between.
    view_rects: RefCell<Vec<PhysicalRect>>,
}

impl PlatformViewHost {
    pub(crate) fn new(owner: HWND) -> Self {
        Self {
            owner,
            host: Cell::new(None),
            hosting: RefCell::new(PlatformViewHosting::default()),
            view_rects: RefCell::new(Vec::new()),
        }
    }

    /// Applies one drawn frame's worth of platform-view work.
    pub(crate) fn update(&self, update: &PlatformViewUpdate, scale_factor: f32) {
        self.detach(&update.detached);
        // Placements are a complete frame, not a delta. In particular, move
        // and resize callbacks must never resurrect the preceding empty frame.
        self.view_rects.borrow_mut().clear();

        if !update.placements.is_empty() {
            match self.ensure_host() {
                Ok(host) => self.place(host, update, scale_factor),
                Err(error) => log::error!("failed to create the platform view host: {error:#}"),
            }
        }

        self.sync_geometry();
    }

    /// Re-derives the host window's bounds, clip and stacking from the owner's
    /// current client area.
    ///
    /// Called after every frame and whenever the owner moves, resizes, changes
    /// DPI or changes visibility, because none of those send the host a message
    /// of its own.
    pub(crate) fn sync_geometry(&self) {
        let Some(host) = self.host.get() else {
            return;
        };

        let client = match self.client_rect() {
            Ok(client) => client,
            Err(error) => {
                log::error!("failed to read the platform view host's client area: {error:#}");
                return;
            }
        };
        let owner_visible =
            unsafe { IsWindowVisible(self.owner).as_bool() && !IsIconic(self.owner).as_bool() };
        let view_rects = self.view_rects.borrow();
        let geometry = plan::host_geometry(client, owner_visible, &view_rects);

        // The clip is set before the move so a resize never flashes host
        // background where a view no longer reaches.
        self.clip_host(host, &plan::host_region_rects(client, &view_rects));

        unsafe {
            SetWindowPos(
                host,
                // Inserting after the owner puts the host directly above what
                // GPUI's base renderer drew while leaving every topmost overlay
                // window above it.
                Some(self.owner),
                geometry.rect.x,
                geometry.rect.y,
                geometry.rect.width,
                geometry.rect.height,
                SWP_NOACTIVATE
                    | SWP_NOOWNERZORDER
                    | (if geometry.visible {
                        SWP_SHOWWINDOW
                    } else {
                        SWP_HIDEWINDOW
                    }),
            )
            .context("failed to place the platform view host")
            .log_err();
        }
    }

    /// Unhosts every view, returning true once none is left attached.
    pub(crate) fn detach_all(&self) -> bool {
        let hosted = self.hosting.borrow_mut().detach_all();
        let mut stranded = Vec::new();
        for (id, view) in hosted {
            let unhosted = unhost_view(&view);
            if unhosted.is_err() {
                stranded.push((id, view));
            }
            unhosted.log_err();
        }

        let empty = stranded.is_empty();
        let mut hosting = self.hosting.borrow_mut();
        for (id, view) in stranded {
            hosting.attach(id, view);
        }
        empty
    }

    /// Tears the host window down. Views still attached are unhosted first, so
    /// destroying the GPUI window never destroys a view its owner still holds.
    pub(crate) fn destroy(&self) {
        let detached = self.detach_all();
        self.view_rects.borrow_mut().clear();
        if !detached {
            // Destroying either this popup or its owner also destroys caller
            // children. Quarantine the hidden popup without an owner instead;
            // keep the restoration state intact so destroy can be retried.
            if let Some(host) = self.host.get() {
                unsafe {
                    ShowWindow(host, SW_HIDE);
                    set_window_long(host, GWLP_HWNDPARENT, 0);
                }
            }
            return;
        }
        if let Some(host) = self.host.take() {
            unsafe {
                DestroyWindow(host)
                    .context("failed to destroy the platform view host")
                    .log_err();
            }
        }
    }

    fn detach(&self, detached: &[PlatformViewId]) {
        let mut hosting = self.hosting.borrow_mut();
        for id in detached {
            let Some(view) = hosting.attributes(*id) else {
                continue;
            };
            let unhosted = unhost_view(view);
            if unhosted.is_ok() {
                hosting.detach(*id);
            }
            unhosted.log_err();
        }
    }

    fn place(&self, host: HWND, update: &PlatformViewUpdate, scale_factor: f32) {
        let mut hosting = self.hosting.borrow_mut();

        for placement in &update.placements {
            let id = placement.handle.id();
            if hosting.contains(id) {
                continue;
            }
            match host_view(host, placement.handle.clone()) {
                Ok(view) => hosting.attach(id, view),
                Err(error) => log::error!("failed to host a platform view: {error:#}"),
            }
        }

        let order = update
            .placements
            .iter()
            .map(|placement| placement.handle.id())
            .collect::<Vec<_>>();
        let restacked = hosting.restack(&order);

        let mut rects = Vec::with_capacity(update.placements.len());
        let mut below: Option<HWND> = None;
        for placement in &update.placements {
            let id = placement.handle.id();
            let Some(view) = hosting.attributes(id) else {
                continue;
            };
            let hwnd = view.handle.as_hwnd();
            if !view.attached.get() {
                if apply(hwnd, &plan::attach_ops(view.saved, host.0 as isize))
                    .log_err()
                    .is_none()
                {
                    continue;
                }
                view.attached.set(true);
            }
            let bounds = PhysicalRect::from_bounds(platform_view_physical_bounds(
                placement.bounds,
                scale_factor,
            ));
            let clip = PhysicalRect::from_bounds(platform_view_physical_bounds(
                placement.clip_bounds(),
                scale_factor,
            ))
            .intersect(&bounds);
            // Each child needs its own region: the host's union alone lets
            // overlapping siblings paint and receive input through each other.
            // Apply even when only the clip changed, preserving layout size.
            let local = plan::child_region_rect(bounds, clip);
            // A clip-only expansion must invalidate newly exposed pixels even
            // when no SetWindowPos follows it.
            if let Err(error) = set_rect_region(hwnd, &[local], true) {
                unsafe {
                    ShowWindow(hwnd, SW_HIDE);
                }
                log::error!("failed to clip a hosted platform view: {error:#}");
                continue;
            }
            rects.push(clip);

            let moved = hosting.place(id, placement.bounds, scale_factor);
            if moved.is_some() || restacked || !unsafe { IsWindowVisible(hwnd) }.as_bool() {
                place_view(hwnd, below, moved.map(PhysicalRect::from_bounds))
                    .context("failed to place a hosted platform view")
                    .log_err();
            }
            below = Some(hwnd);
        }

        *self.view_rects.borrow_mut() = rects;
    }

    fn ensure_host(&self) -> Result<HWND> {
        if let Some(host) = self.host.get() {
            return Ok(host);
        }

        register_host_window_class();
        let host = unsafe {
            CreateWindowExW(
                // The host must never take activation away from the GPUI window
                // it covers, and it is an implementation detail rather than a
                // window a user can reach.
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_NOPARENTNOTIFY,
                HOST_WINDOW_CLASS_NAME,
                None,
                WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                0,
                0,
                0,
                0,
                Some(self.owner),
                None,
                Some(get_module_handle().into()),
                None,
            )
        }
        .context("failed to create the platform view host window")?;

        self.host.set(Some(host));
        Ok(host)
    }

    fn client_rect(&self) -> Result<PhysicalRect> {
        let mut client = RECT::default();
        unsafe { GetClientRect(self.owner, &mut client) }
            .context("failed to read the owner's client area")?;
        let mut origin = POINT { x: 0, y: 0 };
        unsafe { ClientToScreen(self.owner, &mut origin) }
            .ok()
            .context("failed to map the owner's client origin to the screen")?;

        Ok(PhysicalRect {
            x: origin.x,
            y: origin.y,
            width: client.right - client.left,
            height: client.bottom - client.top,
        })
    }

    fn clip_host(&self, host: HWND, rects: &[PhysicalRect]) {
        set_rect_region(host, rects, false).log_err();
    }
}

impl Drop for PlatformViewHost {
    fn drop(&mut self) {
        self.destroy();
        if self.host.get().is_some() {
            // An OS refusal cannot justify destroying a caller's child or
            // controller. A final failed teardown deliberately retains both.
            // Normal explicit destroy calls remain retryable and leak nothing.
            for (_, view) in self.hosting.get_mut().detach_all() {
                std::mem::forget(view);
            }
        }
    }
}

fn set_rect_region(hwnd: HWND, rects: &[PhysicalRect], redraw: bool) -> Result<()> {
    let region = combined_region(rects).unwrap_or_else(|| unsafe { CreateRectRgn(0, 0, 0, 0) });
    anyhow::ensure!(!region.is_invalid(), "failed to allocate window region");
    if unsafe { SetWindowRgn(hwnd, Some(region), redraw) } == 0 {
        unsafe { DeleteObject(region.into()).ok().log_err() };
        anyhow::bail!("failed to set window region");
    }
    Ok(())
}

/// Reads what a child window must get back at detach and moves it under the
/// host.
fn host_view(host: HWND, handle: PlatformViewHandle) -> Result<HostedView> {
    let hwnd = handle.as_hwnd();
    anyhow::ensure!(
        unsafe { IsWindow(Some(hwnd)) }.as_bool(),
        "platform view HWND is no longer valid"
    );

    let style = unsafe { get_window_long(hwnd, GWL_STYLE) };
    anyhow::ensure!(
        style & WS_CHILD.0 as isize != 0,
        "platform view HWND must have the WS_CHILD style"
    );
    let parent = unsafe { GetAncestor(hwnd, GA_PARENT) };
    let region = Rc::new(SavedRegion(capture_region(hwnd)));
    let view = HostedView {
        handle,
        attached: Cell::new(false),
        saved: SavedViewState {
            parent: (!parent.is_invalid()).then_some(parent.0 as isize),
            style,
            ex_style: unsafe { get_window_long(hwnd, GWL_EXSTYLE) },
            region: region.0,
        },
        _region: region,
    };

    if let Err(error) = apply(hwnd, &plan::attach_ops(view.saved, host.0 as isize)) {
        // A half-hosted view is worse than an unhosted one: put back everything
        // the failed attach may already have changed.
        if let Err(restore_error) = unhost_view(&view) {
            // Retain failed rollback state in the host for the next detach,
            // rather than dropping the controller of a half-hosted child.
            log::error!("failed platform view attach: {error:#}; rollback: {restore_error:#}");
            return Ok(view);
        }
        return Err(error);
    }

    view.attached.set(true);
    Ok(view)
}

/// Puts a hosted view back exactly as it was before GPUI took its frame.
fn unhost_view(view: &HostedView) -> Result<()> {
    // A failed restoration may already have changed styles or parent. If the
    // caller paints this view again, replay attachment before placing it.
    view.attached.set(false);
    let hwnd = view.handle.as_hwnd();
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return Ok(());
    }

    apply(hwnd, &plan::detach_ops(view.saved))
}

fn apply(hwnd: HWND, ops: &[ViewOp]) -> Result<()> {
    for op in ops {
        #[cfg(test)]
        if FAIL_PARENT.with(|fail| fail.get()) && matches!(op, ViewOp::SetParent(_)) {
            anyhow::bail!("injected SetParent failure");
        }
        match *op {
            ViewOp::Hide => unsafe {
                let _was_visible = ShowWindow(hwnd, SW_HIDE);
            },
            ViewOp::SetStyle(style) => unsafe {
                set_window_long(hwnd, GWL_STYLE, style);
                anyhow::ensure!(
                    get_window_long(hwnd, GWL_STYLE) == style,
                    "failed to set the platform view's window style"
                );
            },
            ViewOp::SetExStyle(ex_style) => unsafe {
                set_window_long(hwnd, GWL_EXSTYLE, ex_style);
                anyhow::ensure!(
                    get_window_long(hwnd, GWL_EXSTYLE) == ex_style,
                    "failed to set the platform view's extended window style"
                );
            },
            ViewOp::SetParent(parent) => unsafe {
                let parent = parent
                    .map(hwnd_from)
                    .filter(|parent| *parent != hwnd && IsWindow(Some(*parent)).as_bool());
                SetParent(hwnd, parent).context("failed to reparent the platform view")?;
            },
            ViewOp::SetRegion(region) => unsafe {
                let copy = region
                    .map(|region| {
                        let copy = CreateRectRgn(0, 0, 0, 0);
                        let source = HRGN(region as *mut c_void);
                        if !copy.is_invalid()
                            && CombineRgn(Some(copy), Some(source), None, RGN_COPY) != RGN_ERROR
                        {
                            Ok(copy)
                        } else {
                            if !copy.is_invalid() {
                                DeleteObject(copy.into()).ok().log_err();
                            }
                            Err(anyhow::anyhow!("failed to copy saved window region"))
                        }
                    })
                    .transpose()?;
                if SetWindowRgn(hwnd, copy, false) == 0 {
                    if let Some(copy) = copy {
                        DeleteObject(copy.into()).ok().log_err();
                    }
                    anyhow::bail!("failed to set the platform view's window region");
                }
            },
        }
    }
    Ok(())
}

#[cfg(test)]
thread_local! {
    // Thread-local because HWNDs and their restoration run on their UI thread.
    static FAIL_PARENT: Cell<bool> = const { Cell::new(false) };
}

fn place_view(hwnd: HWND, below: Option<HWND>, rect: Option<PhysicalRect>) -> Result<()> {
    let (x, y, width, height, moved) = match rect {
        Some(rect) => (rect.x, rect.y, rect.width, rect.height, SWP_NOFLAGS),
        None => (0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE),
    };
    unsafe {
        SetWindowPos(
            hwnd,
            // Views are stacked in paint order, so each one goes directly above
            // the view painted before it, and the first one goes at the bottom
            // of the host.
            Some(below.unwrap_or(HWND_BOTTOM)),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOCOPYBITS | SWP_SHOWWINDOW | moved,
        )?;
    }
    Ok(())
}

/// Takes a copy of a window's region, or `None` when it has none.
///
/// The copy stays owned by the caller. Restoration gives Windows a duplicate,
/// never this handle, so a partial failure can safely retry.
fn capture_region(hwnd: HWND) -> Option<isize> {
    let region = unsafe { CreateRectRgn(0, 0, 0, 0) };
    if region.is_invalid() {
        return None;
    }
    let captured = unsafe { GetWindowRgn(hwnd, region) };
    if captured == RGN_ERROR {
        unsafe { DeleteObject(region.into()).ok().log_err() };
        return None;
    }
    Some(region.0 as isize)
}

fn release_region(region: Option<isize>) {
    if let Some(region) = region {
        unsafe {
            DeleteObject(HRGN(region as *mut c_void).into())
                .ok()
                .log_err()
        };
    }
}

/// Builds one region covering every rectangle, or `None` when there is nothing
/// to cover.
fn combined_region(rects: &[PhysicalRect]) -> Option<HRGN> {
    let mut combined: Option<HRGN> = None;
    for rect in rects {
        let next = unsafe { CreateRectRgn(rect.x, rect.y, rect.right(), rect.bottom()) };
        if next.is_invalid() {
            continue;
        }
        match combined {
            None => combined = Some(next),
            Some(region) => {
                unsafe { CombineRgn(Some(region), Some(region), Some(next), RGN_OR) };
                unsafe { DeleteObject(next.into()).ok().log_err() };
            }
        }
    }
    combined
}

fn hwnd_from(value: isize) -> HWND {
    HWND(value as *mut c_void)
}

const SWP_NOFLAGS: SET_WINDOW_POS_FLAGS = SET_WINDOW_POS_FLAGS(0);
const HOST_WINDOW_CLASS_NAME: PCWSTR = w!("Zed::PlatformViewHost");

/// A click on a hosted view must not move activation off the GPUI window the
/// host floats over, or the window would look inactive while the user is
/// working in it. A view that wants the keyboard still takes focus explicitly.
unsafe extern "system" fn host_window_procedure(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn register_host_window_class() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(host_window_procedure),
            lpszClassName: PCWSTR(HOST_WINDOW_CLASS_NAME.as_ptr()),
            hInstance: get_module_handle().into(),
            // The host is clipped to the rectangles its children occupy, so it
            // has nothing of its own to paint; a black brush keeps whatever a
            // child has not drawn yet from showing stale desktop pixels.
            hbrBackground: unsafe { CreateSolidBrush(COLORREF(0x00000000)) },
            ..Default::default()
        };
        unsafe { RegisterClassW(&wc) };
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Bounds, PlatformViewPlacement, PlatformViewUpdate, point, px, size};
    use std::{cell::Cell, rc::Rc};

    struct DropProbe(Rc<Cell<bool>>);

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    fn create_test_window(parent: Option<HWND>, style: WINDOW_STYLE) -> HWND {
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                None,
                style,
                0,
                0,
                10,
                10,
                parent,
                None,
                None,
                None,
            )
            .expect("failed to create test HWND")
        }
    }

    fn view_handle(hwnd: HWND) -> PlatformViewHandle {
        unsafe { PlatformViewHandle::from_hwnd(hwnd) }
    }

    #[test]
    fn hosting_reparents_places_and_fully_restores_a_child_window() {
        let original_parent = create_test_window(None, WINDOW_STYLE::default());
        let host = create_test_window(None, WINDOW_STYLE::default());
        let child = create_test_window(Some(original_parent), WS_CHILD | WS_VISIBLE);
        unsafe {
            set_window_long(
                child,
                GWL_EXSTYLE,
                get_window_long(child, GWL_EXSTYLE) | WS_EX_TOPMOST.0 as isize,
            );
            SetWindowRgn(child, Some(CreateRectRgn(0, 0, 5, 5)), false);
        }
        let original_style = unsafe { get_window_long(child, GWL_STYLE) };
        let original_ex_style = unsafe { get_window_long(child, GWL_EXSTYLE) };

        let view = host_view(host, view_handle(child)).expect("failed to host child HWND");
        place_view(
            child,
            None,
            Some(PhysicalRect::from_bounds(platform_view_physical_bounds(
                Bounds {
                    origin: point(px(10.), px(20.)),
                    size: size(px(100.), px(50.)),
                },
                2.,
            ))),
        )
        .expect("failed to place child HWND");

        assert_eq!(unsafe { GetAncestor(child, GA_PARENT) }, host);
        assert_ne!(
            unsafe { get_window_long(child, GWL_STYLE) } & WS_CLIPSIBLINGS.0 as isize,
            0
        );
        let mut rect = RECT::default();
        unsafe { GetWindowRect(child, &mut rect) }.expect("failed to read child bounds");
        let mut origin = POINT {
            x: rect.left,
            y: rect.top,
        };
        unsafe { ScreenToClient(host, &mut origin) }
            .ok()
            .expect("failed to map child origin");
        assert_eq!((origin.x, origin.y), (20, 40));
        assert_eq!((rect.right - rect.left, rect.bottom - rect.top), (200, 100));

        unhost_view(&view).expect("failed to unhost child HWND");

        assert_eq!(unsafe { GetAncestor(child, GA_PARENT) }, original_parent);
        assert_eq!(
            unsafe { get_window_long(child, GWL_STYLE) },
            original_style & !(WS_VISIBLE.0 as isize)
        );
        assert_eq!(
            unsafe { get_window_long(child, GWL_EXSTYLE) },
            original_ex_style,
            "the extended style must come back exactly as it was"
        );
        let mut region = RECT::default();
        assert_ne!(
            unsafe { GetWindowRgnBox(child, &mut region) },
            RGN_ERROR,
            "the window region must come back too"
        );
        assert!(unsafe { IsWindow(Some(child)) }.as_bool());

        unsafe {
            DestroyWindow(host).expect("failed to destroy host HWND");
            DestroyWindow(original_parent).expect("failed to destroy original parent HWND");
        }
    }

    #[test]
    fn hosting_clips_without_resizing_the_child_window() {
        let owner = create_test_window(None, WS_VISIBLE);
        let original_parent = create_test_window(None, WINDOW_STYLE::default());
        let child = create_test_window(Some(original_parent), WS_CHILD | WS_VISIBLE);
        unsafe {
            SetWindowPos(
                owner,
                None,
                0,
                0,
                300,
                200,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOZORDER,
            )
            .expect("failed to size test owner");
        }
        let host = PlatformViewHost::new(owner);
        let full_bounds = Bounds {
            origin: point(px(10.), px(20.)),
            size: size(px(100.), px(80.)),
        };
        let clip_bounds = Bounds {
            origin: point(px(30.), px(35.)),
            size: size(px(50.), px(40.)),
        };
        host.update(
            &PlatformViewUpdate {
                placements: vec![PlatformViewPlacement::new(
                    view_handle(child),
                    full_bounds,
                    clip_bounds,
                )],
                detached: Vec::new(),
            },
            1.,
        );

        let native_host = host.host.get().expect("the platform host was created");
        let mut child_rect = RECT::default();
        unsafe { GetWindowRect(child, &mut child_rect) }.expect("failed to read child bounds");
        let mut child_origin = POINT {
            x: child_rect.left,
            y: child_rect.top,
        };
        unsafe { ScreenToClient(native_host, &mut child_origin) }
            .ok()
            .expect("failed to map child origin");
        assert_eq!((child_origin.x, child_origin.y), (10, 20));
        assert_eq!(
            (
                child_rect.right - child_rect.left,
                child_rect.bottom - child_rect.top
            ),
            (100, 80),
            "the child keeps its full layout size"
        );

        let mut region = RECT::default();
        assert_ne!(
            unsafe { GetWindowRgnBox(native_host, &mut region) },
            RGN_ERROR
        );
        assert_eq!(
            region,
            RECT {
                left: 30,
                top: 35,
                right: 80,
                bottom: 75,
            },
            "the host region, not the child frame, carries GPUI's clip"
        );

        host.destroy();
        assert_eq!(unsafe { GetAncestor(child, GA_PARENT) }, original_parent);
        unsafe {
            DestroyWindow(child).expect("failed to destroy child HWND");
            DestroyWindow(original_parent).expect("failed to destroy original parent HWND");
            DestroyWindow(owner).expect("failed to destroy owner HWND");
        }
    }

    #[test]
    fn hosting_empty_frame_stays_hidden_after_geometry_sync() {
        let owner = create_test_window(None, WS_VISIBLE);
        let parent = create_test_window(None, WINDOW_STYLE::default());
        let child = create_test_window(Some(parent), WS_CHILD | WS_VISIBLE);
        let host = PlatformViewHost::new(owner);
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(10.), px(10.)));
        let handle = view_handle(child);
        host.update(
            &PlatformViewUpdate {
                placements: vec![PlatformViewPlacement::new(handle.clone(), bounds, bounds)],
                detached: vec![],
            },
            1.,
        );
        let popup = host.host.get().expect("host created");
        assert!(unsafe { IsWindowVisible(popup) }.as_bool());
        host.update(
            &PlatformViewUpdate {
                placements: vec![],
                detached: vec![handle.id()],
            },
            1.,
        );
        assert!(host.view_rects.borrow().is_empty());
        unsafe {
            SetWindowPos(owner, None, 50, 70, 200, 160, SWP_NOZORDER).expect("move owner");
        }
        host.sync_geometry();
        assert!(!unsafe { IsWindowVisible(popup) }.as_bool());
        let mut region = RECT::default();
        assert_eq!(unsafe { GetWindowRgnBox(popup, &mut region) }, NULLREGION);
        host.destroy();
        unsafe {
            DestroyWindow(parent).expect("destroy parent");
            DestroyWindow(owner).expect("destroy owner");
        }
    }

    #[test]
    fn hosting_overlapping_children_have_independent_clips_and_clip_only_updates() {
        let owner = create_test_window(None, WS_VISIBLE);
        let parent = create_test_window(None, WINDOW_STYLE::default());
        let a = create_test_window(Some(parent), WS_CHILD | WS_VISIBLE);
        let b = create_test_window(Some(parent), WS_CHILD | WS_VISIBLE);
        unsafe {
            SetWindowPos(owner, None, 0, 0, 300, 200, SWP_NOZORDER).expect("size owner");
        }
        let host = PlatformViewHost::new(owner);
        let full = Bounds::new(point(px(-10.), px(-5.)), size(px(100.), px(80.)));
        for offset in [0., 10.] {
            let a_clip = Bounds::new(point(px(offset), px(0.)), size(px(20.), px(20.)));
            let b_clip = Bounds::new(point(px(40.), px(0.)), size(px(20.), px(20.)));
            host.update(
                &PlatformViewUpdate {
                    placements: vec![
                        PlatformViewPlacement::new(view_handle(a), full, a_clip),
                        PlatformViewPlacement::new(view_handle(b), full, b_clip),
                    ],
                    detached: vec![],
                },
                2.,
            );
            for (child, left) in [(a, (20. + offset * 2.) as i32), (b, 100)] {
                let region = unsafe { CreateRectRgn(0, 0, 0, 0) };
                assert_ne!(unsafe { GetWindowRgn(child, region) }, RGN_ERROR);
                assert!(unsafe { PtInRegion(region, left + 1, 11) }.as_bool());
                assert!(!unsafe { PtInRegion(region, left + 41, 11) }.as_bool());
                let mut rect = RECT::default();
                unsafe {
                    GetWindowRect(child, &mut rect).expect("read child bounds");
                    DeleteObject(region.into())
                        .ok()
                        .expect("release test region");
                }
                assert_eq!((rect.right - rect.left, rect.bottom - rect.top), (200, 160));
            }
            let popup = host.host.get().expect("host created");
            // Real HWND hit testing must skip the overlapping sibling whose
            // full bounds cover this point but whose own region does not.
            assert_eq!(
                unsafe {
                    RealChildWindowFromPoint(
                        popup,
                        POINT {
                            x: (offset * 2.) as i32 + 1,
                            y: 1,
                        },
                    )
                },
                a
            );
            assert_eq!(
                unsafe { RealChildWindowFromPoint(popup, POINT { x: 81, y: 1 }) },
                b
            );
        }
        host.destroy();
        unsafe {
            DestroyWindow(parent).expect("destroy parent");
            DestroyWindow(owner).expect("destroy owner");
        }
    }

    #[test]
    fn hosting_failed_detach_retries_keep_region_and_caller_child_alive() {
        let owner = create_test_window(None, WS_VISIBLE);
        let parent = create_test_window(None, WINDOW_STYLE::default());
        let child = create_test_window(Some(parent), WS_CHILD | WS_VISIBLE);
        unsafe {
            SetWindowRgn(child, Some(CreateRectRgn(1, 2, 7, 8)), false);
        }
        let host = PlatformViewHost::new(owner);
        let handle = view_handle(child);
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(10.), px(10.)));
        host.update(
            &PlatformViewUpdate {
                placements: vec![PlatformViewPlacement::new(handle.clone(), bounds, bounds)],
                detached: vec![],
            },
            1.,
        );
        let popup = host.host.get().expect("host created");
        FAIL_PARENT.with(|fail| fail.set(true));
        for _ in 0..3 {
            host.detach(&[handle.id()]);
        }
        host.destroy();
        FAIL_PARENT.with(|fail| fail.set(false));
        assert!(unsafe { IsWindow(Some(child)) }.as_bool());
        assert_eq!(unsafe { GetAncestor(child, GA_PARENT) }, popup);
        assert_eq!(unsafe { get_window_long(popup, GWLP_HWNDPARENT) }, 0);
        // Even owner destruction must not destroy the quarantined child.
        unsafe {
            DestroyWindow(owner).expect("destroy owner");
        }
        host.destroy();
        assert_eq!(unsafe { GetAncestor(child, GA_PARENT) }, parent);
        assert!(!unsafe { IsWindow(Some(popup)) }.as_bool());
        let mut restored = RECT::default();
        assert_ne!(unsafe { GetWindowRgnBox(child, &mut restored) }, RGN_ERROR);
        assert_eq!(
            restored,
            RECT {
                left: 1,
                top: 2,
                right: 7,
                bottom: 8
            }
        );
        unsafe {
            DestroyWindow(parent).expect("destroy parent");
        }
    }

    #[test]
    fn a_destroyed_view_stops_being_hosted() {
        let original_parent = create_test_window(None, WINDOW_STYLE::default());
        let host = create_test_window(None, WINDOW_STYLE::default());
        let child = create_test_window(Some(original_parent), WS_CHILD | WS_VISIBLE);

        let view = host_view(host, view_handle(child)).expect("failed to host child HWND");
        unsafe { DestroyWindow(child) }.expect("failed to destroy child HWND");

        unhost_view(&view).expect("unhosting a destroyed view is not an error");

        unsafe {
            DestroyWindow(host).expect("failed to destroy host HWND");
            DestroyWindow(original_parent).expect("failed to destroy original parent HWND");
        }
    }

    #[test]
    fn a_view_without_the_child_style_is_refused() {
        let host = create_test_window(None, WINDOW_STYLE::default());
        let popup = create_test_window(None, WS_POPUP);

        assert!(host_view(host, view_handle(popup)).is_err());

        unsafe {
            DestroyWindow(popup).expect("failed to destroy popup HWND");
            DestroyWindow(host).expect("failed to destroy host HWND");
        }
    }

    #[test]
    fn hosting_retains_the_view_owner_until_after_detach() {
        let original_parent = create_test_window(None, WINDOW_STYLE::default());
        let host = create_test_window(None, WINDOW_STYLE::default());
        let child = create_test_window(Some(original_parent), WS_CHILD | WS_VISIBLE);
        let dropped = Rc::new(Cell::new(false));
        let owner = Rc::new(DropProbe(Rc::clone(&dropped)));
        let handle = view_handle(child).keep_alive(owner);
        let id = handle.id();
        let view = host_view(host, handle).expect("failed to host child HWND");
        let mut hosting = PlatformViewHosting::default();
        hosting.attach(id, view);

        assert!(!dropped.get(), "the native host retains the controller");
        let view = hosting.detach(id).expect("hosted view");
        unhost_view(&view).expect("failed to unhost child HWND");
        assert!(
            !dropped.get(),
            "detach finishes before releasing the controller"
        );
        drop(view);
        assert!(dropped.get(), "the detached view releases the controller");

        unsafe {
            DestroyWindow(child).expect("failed to destroy child HWND");
            DestroyWindow(host).expect("failed to destroy host HWND");
            DestroyWindow(original_parent).expect("failed to destroy original parent HWND");
        }
    }
}

pub(crate) mod plan {
    //! The sequencing and geometry decisions behind hosting a native view, with
    //! no Win32 calls in sight so they can be exercised without a desktop.

    use gpui::{Bounds, DevicePixels};
    use windows::Win32::UI::WindowsAndMessaging::{
        WS_CHILD, WS_CLIPSIBLINGS, WS_EX_NOPARENTNOTIFY, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
    };

    /// A rectangle in physical pixels.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub(crate) struct PhysicalRect {
        pub x: i32,
        pub y: i32,
        pub width: i32,
        pub height: i32,
    }

    impl PhysicalRect {
        pub(crate) fn from_bounds(bounds: Bounds<DevicePixels>) -> Self {
            Self {
                x: bounds.origin.x.0,
                y: bounds.origin.y.0,
                width: bounds.size.width.0.max(0),
                height: bounds.size.height.0.max(0),
            }
        }

        pub(crate) fn right(&self) -> i32 {
            self.x.saturating_add(self.width)
        }

        pub(crate) fn bottom(&self) -> i32 {
            self.y.saturating_add(self.height)
        }

        pub(crate) fn is_empty(&self) -> bool {
            self.width <= 0 || self.height <= 0
        }

        /// Returns the part of this rectangle inside `other`, in the same
        /// coordinate space.
        pub(crate) fn intersect(&self, other: &Self) -> Self {
            let x = self.x.max(other.x);
            let y = self.y.max(other.y);
            Self {
                x,
                y,
                width: (self.right().min(other.right()) - x).max(0),
                height: (self.bottom().min(other.bottom()) - y).max(0),
            }
        }

        /// The same rectangle with its origin dropped, which is the space both
        /// the hosted views' bounds and the host's own region live in.
        fn at_origin(&self) -> Self {
            Self {
                x: 0,
                y: 0,
                width: self.width,
                height: self.height,
            }
        }
    }

    /// What a hosted view must get back when GPUI stops owning its frame.
    ///
    /// Windows keeps the extended styles and the window region apart from the
    /// plain styles, and a view that came back with any of the three altered
    /// would be a different window than the one its owner handed over.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct SavedViewState {
        pub parent: Option<isize>,
        pub style: isize,
        pub ex_style: isize,
        /// Borrowed from HostedView's saved-region owner. Applying this plan
        /// duplicates it before transferring the duplicate to Windows.
        pub region: Option<isize>,
    }

    /// One native call against a hosted view.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum ViewOp {
        Hide,
        SetStyle(isize),
        SetExStyle(isize),
        SetParent(Option<isize>),
        SetRegion(Option<isize>),
    }

    /// The styles a view carries while GPUI owns its frame.
    ///
    /// It is clipped against its siblings because several hosted views can
    /// overlap, and it is left hidden until the first placement so no frame is
    /// ever presented at the previous parent's coordinates.
    pub(crate) fn hosted_style(previous: isize) -> isize {
        (previous | WS_CHILD.0 as isize | WS_CLIPSIBLINGS.0 as isize)
            & !(WS_POPUP.0 as isize | WS_VISIBLE.0 as isize)
    }

    /// The extended styles a view carries while GPUI owns its frame.
    ///
    /// Stacking is GPUI's to decide, so a topmost bit left over from a previous
    /// life as a top-level window is dropped, and the host has no use for the
    /// parent notifications a hosted view would otherwise send it.
    pub(crate) fn hosted_ex_style(previous: isize) -> isize {
        (previous | WS_EX_NOPARENTNOTIFY.0 as isize) & !(WS_EX_TOPMOST.0 as isize)
    }

    /// The calls that host a view under `host`.
    ///
    /// The view is hidden before anything else so none of the intermediate
    /// states is ever composed, and its region is cleared last because from
    /// then on GPUI's layout, not the owner's clip, decides what of it shows.
    pub(crate) fn attach_ops(saved: SavedViewState, host: isize) -> Vec<ViewOp> {
        vec![
            ViewOp::Hide,
            ViewOp::SetStyle(hosted_style(saved.style)),
            ViewOp::SetExStyle(hosted_ex_style(saved.ex_style)),
            ViewOp::SetParent(Some(host)),
            ViewOp::SetRegion(None),
        ]
    }

    /// The calls that unhost a view, undoing [`attach_ops`] in reverse.
    ///
    /// The visible bit is the one thing not restored: the view is left hidden,
    /// because after detaching it has no frame anywhere until its owner gives
    /// it one.
    pub(crate) fn detach_ops(saved: SavedViewState) -> Vec<ViewOp> {
        vec![
            ViewOp::Hide,
            ViewOp::SetParent(saved.parent),
            ViewOp::SetStyle(saved.style & !(WS_VISIBLE.0 as isize)),
            ViewOp::SetExStyle(saved.ex_style),
            ViewOp::SetRegion(saved.region),
        ]
    }

    /// Where the view host belongs and whether it should show at all.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct HostGeometry {
        pub rect: PhysicalRect,
        pub visible: bool,
    }

    /// Derives the host's screen rectangle from the owner's client area.
    ///
    /// A host that covers nothing, or whose owner is hidden or minimized, stays
    /// hidden rather than floating over another window.
    pub(crate) fn host_geometry(
        client: PhysicalRect,
        owner_visible: bool,
        views: &[PhysicalRect],
    ) -> HostGeometry {
        HostGeometry {
            rect: client,
            visible: owner_visible
                && !client.is_empty()
                && views
                    .iter()
                    .any(|view| !view.intersect(&client.at_origin()).is_empty()),
        }
    }

    /// The rectangles the host's window region is built from: the hosted views
    /// clipped to the client area, in coordinates relative to the host itself.
    pub(crate) fn host_region_rects(
        client: PhysicalRect,
        views: &[PhysicalRect],
    ) -> Vec<PhysicalRect> {
        let bounds = client.at_origin();
        views
            .iter()
            .map(|view| view.intersect(&bounds))
            .filter(|rect| !rect.is_empty())
            .collect()
    }

    /// Clip in child-window coordinates, after rounding both screen-space
    /// rectangles at the same scale. Never resize the child's layout frame.
    pub(crate) fn child_region_rect(bounds: PhysicalRect, clip: PhysicalRect) -> PhysicalRect {
        let clip = clip.intersect(&bounds);
        PhysicalRect {
            x: clip.x.saturating_sub(bounds.x),
            y: clip.y.saturating_sub(bounds.y),
            width: clip.width,
            height: clip.height,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn saved() -> SavedViewState {
            SavedViewState {
                parent: Some(0x10),
                style: (WS_CHILD.0 | WS_VISIBLE.0) as isize,
                ex_style: (WS_EX_TOPMOST.0 | 0x40) as isize,
                region: Some(0x20),
            }
        }

        fn rect(x: i32, y: i32, width: i32, height: i32) -> PhysicalRect {
            PhysicalRect {
                x,
                y,
                width,
                height,
            }
        }

        #[test]
        fn child_clips_intersect_before_mapping_to_local_coordinates() {
            let bounds = rect(-20, -10, 200, 160);
            assert_eq!(
                child_region_rect(bounds, rect(0, 0, 40, 40)),
                rect(20, 10, 40, 40)
            );
            assert_eq!(
                child_region_rect(bounds, rect(-40, -30, 40, 40)),
                rect(0, 0, 20, 20)
            );
            assert!(child_region_rect(bounds, rect(300, 200, 40, 40)).is_empty());
            assert_eq!(child_region_rect(bounds, bounds), rect(0, 0, 200, 160));
        }

        #[test]
        fn hosting_a_view_hides_it_before_it_is_moved_anywhere() {
            let ops = attach_ops(saved(), 0x99);

            assert_eq!(ops[0], ViewOp::Hide);
            assert_eq!(
                ops.iter().position(|op| matches!(op, ViewOp::SetParent(_))),
                Some(3),
                "the parent changes only after the hosted styles are in place"
            );
            assert_eq!(ops.last(), Some(&ViewOp::SetRegion(None)));
        }

        #[test]
        fn hosted_styles_clip_against_siblings_and_drop_topmost() {
            let saved = saved();
            let ops = attach_ops(saved, 0x99);

            let ViewOp::SetStyle(style) = ops[1] else {
                panic!("the second op sets the window style");
            };
            let ViewOp::SetExStyle(ex_style) = ops[2] else {
                panic!("the third op sets the extended window style");
            };

            assert_ne!(style & WS_CLIPSIBLINGS.0 as isize, 0);
            assert_ne!(style & WS_CHILD.0 as isize, 0);
            assert_eq!(style & WS_VISIBLE.0 as isize, 0);
            assert_eq!(ex_style & WS_EX_TOPMOST.0 as isize, 0);
            assert_ne!(ex_style & WS_EX_NOPARENTNOTIFY.0 as isize, 0);
            assert_ne!(ex_style & 0x40, 0, "unrelated extended styles are kept");
        }

        #[test]
        fn unhosting_restores_the_parent_styles_and_region() {
            let saved = saved();

            let ops = detach_ops(saved);

            assert_eq!(
                ops,
                vec![
                    ViewOp::Hide,
                    ViewOp::SetParent(saved.parent),
                    ViewOp::SetStyle(saved.style & !(WS_VISIBLE.0 as isize)),
                    ViewOp::SetExStyle(saved.ex_style),
                    ViewOp::SetRegion(saved.region),
                ]
            );
        }

        #[test]
        fn unhosting_restores_the_extended_style_exactly() {
            let mut saved = saved();
            saved.ex_style = 0x0000_1234;

            let ops = detach_ops(saved);

            assert!(
                ops.contains(&ViewOp::SetExStyle(0x0000_1234)),
                "the extended style a view arrived with must come back untouched"
            );
            assert_ne!(
                hosted_ex_style(saved.ex_style),
                saved.ex_style & !(WS_EX_NOPARENTNOTIFY.0 as isize),
                "hosting really does change the extended style, so restoring it matters"
            );
        }

        #[test]
        fn unhosting_undoes_every_change_hosting_made() {
            let saved = saved();
            let attached = attach_ops(saved, 0x99);
            let detached = detach_ops(saved);

            for op in attached {
                let restored = match op {
                    ViewOp::SetStyle(_) => matches!(
                        detached.iter().find(|op| matches!(op, ViewOp::SetStyle(_))),
                        Some(ViewOp::SetStyle(style)) if *style == saved.style & !(WS_VISIBLE.0 as isize)
                    ),
                    ViewOp::SetExStyle(_) => detached.contains(&ViewOp::SetExStyle(saved.ex_style)),
                    ViewOp::SetParent(_) => detached.contains(&ViewOp::SetParent(saved.parent)),
                    ViewOp::SetRegion(_) => detached.contains(&ViewOp::SetRegion(saved.region)),
                    ViewOp::Hide => true,
                };
                assert!(restored, "{op:?} is never undone");
            }
        }

        #[test]
        fn a_view_with_no_region_of_its_own_gets_none_back() {
            let mut saved = saved();
            saved.region = None;

            assert!(detach_ops(saved).contains(&ViewOp::SetRegion(None)));
        }

        #[test]
        fn the_host_covers_the_owners_client_area() {
            let client = rect(100, 200, 800, 600);

            let geometry = host_geometry(client, true, &[rect(0, 0, 10, 10)]);

            assert_eq!(geometry.rect, client);
            assert!(geometry.visible);
        }

        #[test]
        fn the_host_hides_when_it_would_cover_nothing() {
            let client = rect(100, 200, 800, 600);

            assert!(!host_geometry(client, true, &[]).visible);
            assert!(!host_geometry(client, true, &[rect(0, 0, 0, 40)]).visible);
            assert!(!host_geometry(client, false, &[rect(0, 0, 10, 10)]).visible);
            assert!(!host_geometry(rect(0, 0, 0, 0), true, &[rect(0, 0, 10, 10)]).visible);
        }

        #[test]
        fn the_host_hides_when_every_view_sits_outside_the_client_area() {
            let client = rect(100, 200, 800, 600);

            assert!(!host_geometry(client, true, &[rect(900, 700, 40, 40)]).visible);
        }

        #[test]
        fn the_host_is_clipped_to_the_views_it_carries() {
            let client = rect(100, 200, 800, 600);
            let views = [rect(10, 20, 100, 50), rect(700, 550, 200, 200)];

            let region = host_region_rects(client, &views);

            assert_eq!(
                region,
                vec![rect(10, 20, 100, 50), rect(700, 550, 100, 50)],
                "views are clipped to the client area, not to the screen"
            );
        }

        #[test]
        fn the_host_clip_drops_views_that_fell_off_the_window() {
            let client = rect(0, 0, 400, 300);

            assert!(host_region_rects(client, &[rect(500, 500, 10, 10)]).is_empty());
            assert!(host_region_rects(client, &[rect(10, 10, 0, 0)]).is_empty());
        }
    }
}
