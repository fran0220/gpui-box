//! Hosting of native platform views inside GPUI windows.
//!
//! A [`PlatformViewHandle`] refers to a view owned by the operating system's
//! toolkit — an `NSView` on macOS or a child `HWND` on Windows. The
//! [`crate::platform_view`] element gives such a view a place in GPUI's layout;
//! GPUI owns its frame from then on and the platform layer repositions it after
//! each drawn frame.
//!
//! Everything in this module apart from the handle itself is platform neutral:
//! the frame-to-frame diffing, the device-pixel snapping and the y-flip used by
//! bottom-left-origin coordinate systems live here so they can be unit tested
//! without a window.

use crate::{Bounds, DevicePixels, Pixels, Point, point, px, size, util::round_to_device_pixel};
use std::{any::Any, fmt, rc::Rc};

/// Caller-owned state that must outlive every clone of a platform view handle.
///
/// This is separate from native ownership: AppKit retains its `NSView`, while
/// Win32 has no equivalent retain operation. A media player, web view, or other
/// native controller often owns resources beyond the view itself, so the
/// handle keeps that controller alive until the platform layer has detached
/// and forgotten its last clone.
#[derive(Clone, Default)]
struct PlatformViewLifetime(Option<Rc<dyn Any>>);

impl PlatformViewLifetime {
    fn keep<T: 'static>(&mut self, owner: Rc<T>) {
        self.0 = Some(owner);
    }
}

/// The stable identity of a hosted platform view.
///
/// Two handles referring to the same native view compare equal.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlatformViewId(usize);

impl PlatformViewId {
    /// Returns the identity as an opaque integer, useful for logging and for
    /// platform layers that key their own bookkeeping by view identity.
    pub fn as_usize(self) -> usize {
        self.0
    }
}

impl fmt::Debug for PlatformViewId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PlatformViewId({:#x})", self.0)
    }
}

#[cfg(target_os = "macos")]
mod handle {
    use super::{PlatformViewId, PlatformViewLifetime};
    use objc::{msg_send, runtime::Object, sel, sel_impl};
    use std::{ffi::c_void, fmt, ptr::NonNull, rc::Rc};

    /// A retained reference to a native view hosted inside a GPUI window.
    ///
    /// On macOS this owns a strong reference to an `NSView`. Cloning retains and
    /// dropping releases, so the view outlives every element that paints it.
    ///
    /// Construct, clone and drop handles on the main thread: the wrapped object
    /// is an AppKit view, and AppKit only promises main-thread safety.
    pub struct PlatformViewHandle {
        view: NonNull<Object>,
        lifetime: PlatformViewLifetime,
    }

    impl PlatformViewHandle {
        /// Wraps an `NSView` so it can be hosted by a GPUI window. The view is
        /// retained for the lifetime of the handle.
        ///
        /// # Safety
        ///
        /// `ns_view` must be a non-null pointer to a live `NSView` instance, and
        /// this must be called on the main thread.
        pub unsafe fn from_ns_view(ns_view: *mut c_void) -> Self {
            let view = NonNull::new(ns_view.cast::<Object>())
                .expect("PlatformViewHandle::from_ns_view requires a non-null NSView");
            unsafe {
                let _: *mut Object = msg_send![view.as_ptr(), retain];
            }
            Self {
                view,
                lifetime: PlatformViewLifetime::default(),
            }
        }

        /// Keeps caller-owned native state alive for as long as this handle or
        /// any clone remains retained by GPUI's platform-view host.
        ///
        /// Use this when the view alone does not own the controller or service
        /// that makes it valid. The owner is released on the thread that drops
        /// the final handle, which for a painted platform view is the window's
        /// platform thread.
        pub fn keep_alive<T: 'static>(mut self, owner: Rc<T>) -> Self {
            self.lifetime.keep(owner);
            self
        }

        /// Returns the hosted `NSView` pointer without transferring ownership.
        pub fn as_ns_view(&self) -> *mut c_void {
            self.view.as_ptr().cast()
        }

        /// Returns this view's stable identity.
        pub fn id(&self) -> PlatformViewId {
            PlatformViewId(self.view.as_ptr() as usize)
        }
    }

    impl Clone for PlatformViewHandle {
        fn clone(&self) -> Self {
            unsafe {
                let _: *mut Object = msg_send![self.view.as_ptr(), retain];
            }
            Self {
                view: self.view,
                lifetime: self.lifetime.clone(),
            }
        }
    }

    impl Drop for PlatformViewHandle {
        fn drop(&mut self) {
            unsafe {
                let _: () = msg_send![self.view.as_ptr(), release];
            }
        }
    }

    impl fmt::Debug for PlatformViewHandle {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("PlatformViewHandle")
                .field("id", &self.id())
                .finish()
        }
    }

    impl PartialEq for PlatformViewHandle {
        fn eq(&self, other: &Self) -> bool {
            self.id() == other.id()
        }
    }

    impl Eq for PlatformViewHandle {}
}

#[cfg(target_os = "windows")]
mod handle {
    use super::{PlatformViewId, PlatformViewLifetime};
    use std::{fmt, rc::Rc};
    use windows::Win32::Foundation::HWND;

    /// A non-owning reference to a child `HWND` hosted inside a GPUI window.
    ///
    /// Windows does not provide retain/release semantics for window handles.
    /// Cloning duplicates this reference without transferring lifetime
    /// ownership. The component that created the `HWND` remains responsible for
    /// destroying it after it is no longer hosted and all handles have been
    /// dropped.
    #[derive(Clone)]
    pub struct PlatformViewHandle {
        hwnd: HWND,
        lifetime: PlatformViewLifetime,
    }

    impl PlatformViewHandle {
        /// Wraps a child `HWND` so it can be hosted by a GPUI window.
        ///
        /// # Safety
        ///
        /// `hwnd` must be a non-null, live child window owned by the calling
        /// process, and must have been created on the thread that owns the GPUI
        /// window hosting it — hosting reparents it, and reparenting across
        /// threads would attach their input queues to each other. It must remain
        /// valid, and must not be destroyed, for as long as this handle or any
        /// clone is painted or retained by GPUI. The caller remains responsible
        /// for destroying it on its owning thread.
        pub unsafe fn from_hwnd(hwnd: HWND) -> Self {
            assert!(
                !hwnd.is_invalid(),
                "PlatformViewHandle::from_hwnd requires a non-null HWND"
            );
            Self {
                hwnd,
                lifetime: PlatformViewLifetime::default(),
            }
        }

        /// Keeps caller-owned native state alive for as long as this handle or
        /// any clone remains retained by GPUI's platform-view host.
        ///
        /// Win32 window handles have no retain operation. Use this for an owner
        /// that destroys the child `HWND` on drop, so destruction cannot race
        /// the frame that detaches the view.
        pub fn keep_alive<T: 'static>(mut self, owner: Rc<T>) -> Self {
            self.lifetime.keep(owner);
            self
        }

        /// Returns the hosted child `HWND` without transferring ownership.
        pub fn as_hwnd(&self) -> HWND {
            self.hwnd
        }

        /// Returns this view's stable identity.
        pub fn id(&self) -> PlatformViewId {
            PlatformViewId(self.hwnd.0 as usize)
        }
    }

    impl fmt::Debug for PlatformViewHandle {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("PlatformViewHandle")
                .field("id", &self.id())
                .finish()
        }
    }

    impl PartialEq for PlatformViewHandle {
        fn eq(&self, other: &Self) -> bool {
            self.id() == other.id()
        }
    }

    impl Eq for PlatformViewHandle {}
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod handle {
    use super::{PlatformViewId, PlatformViewLifetime};
    use std::{
        fmt,
        rc::Rc,
        sync::atomic::{AtomicUsize, Ordering},
    };

    /// An inert stand-in for a natively hosted view.
    ///
    /// This stub exists so cross-platform code that mentions
    /// [`PlatformViewHandle`] still compiles; it does not refer to or host a
    /// native view. Painting the [`crate::platform_view`] element with one only
    /// reserves layout space.
    #[derive(Clone)]
    pub struct PlatformViewHandle {
        id: PlatformViewId,
        lifetime: PlatformViewLifetime,
    }

    impl Default for PlatformViewHandle {
        fn default() -> Self {
            Self::inert()
        }
    }

    impl PlatformViewHandle {
        /// Creates a handle that refers to no native view.
        pub fn inert() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
            Self {
                id: PlatformViewId(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
                lifetime: PlatformViewLifetime::default(),
            }
        }

        /// Keeps caller-owned state alive for the same lifetime cross-platform
        /// code would give a native view and every clone of its handle.
        pub fn keep_alive<T: 'static>(mut self, owner: Rc<T>) -> Self {
            self.lifetime.keep(owner);
            self
        }

        /// Returns this handle's stable identity.
        pub fn id(&self) -> PlatformViewId {
            self.id
        }
    }

    impl fmt::Debug for PlatformViewHandle {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("PlatformViewHandle")
                .field("id", &self.id())
                .finish()
        }
    }

    impl PartialEq for PlatformViewHandle {
        fn eq(&self, other: &Self) -> bool {
            self.id() == other.id()
        }
    }

    impl Eq for PlatformViewHandle {}
}

pub use handle::PlatformViewHandle;

/// A hosted view and the window-space bounds GPUI laid it out at.
///
/// Bounds use GPUI's window coordinate space: logical pixels with the origin at
/// the window's top-left corner and y growing downwards. They are already
/// snapped to the display's device-pixel grid.
#[derive(Clone, Debug, PartialEq)]
pub struct PlatformViewPlacement {
    /// The hosted view.
    pub handle: PlatformViewHandle,
    /// Where the view belongs within the window.
    pub bounds: Bounds<Pixels>,
}

/// The platform-view work owed by a single drawn frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlatformViewUpdate {
    /// Views painted by this frame, in paint order, each at its current bounds.
    /// A view that was not hosted before is attached; one that already was is
    /// repositioned.
    pub placements: Vec<PlatformViewPlacement>,
    /// Views that were hosted by the previous frame but were not painted by this
    /// one, and so must be hidden and detached.
    pub detached: Vec<PlatformViewId>,
}

impl PlatformViewUpdate {
    /// Returns true when the frame asks for no platform-view work at all.
    pub fn is_empty(&self) -> bool {
        self.placements.is_empty() && self.detached.is_empty()
    }
}

/// Tracks which platform views a window currently hosts so each frame can be
/// turned into an attach/reposition/detach instruction set.
#[derive(Default)]
pub(crate) struct PlatformViewRegistry {
    hosted: Vec<PlatformViewId>,
}

impl PlatformViewRegistry {
    /// Reconciles the views painted by the frame just drawn against the views
    /// hosted after the previous frame.
    ///
    /// Returns `None` when nothing is hosted and nothing was painted, so windows
    /// that never use the element never reach the platform layer.
    pub(crate) fn sync(
        &mut self,
        painted: &[PlatformViewPlacement],
        scale_factor: f32,
    ) -> Option<PlatformViewUpdate> {
        if self.hosted.is_empty() && painted.is_empty() {
            return None;
        }

        let painted_ids = painted
            .iter()
            .map(|placement| placement.handle.id())
            .collect::<Vec<_>>();
        let placements = last_placement_indices(&painted_ids)
            .into_iter()
            .map(|index| PlatformViewPlacement {
                handle: painted[index].handle.clone(),
                bounds: snap_platform_view_bounds(painted[index].bounds, scale_factor),
            })
            .collect::<Vec<_>>();
        let detached = detached_ids(&self.hosted, &painted_ids);

        self.hosted = placements
            .iter()
            .map(|placement| placement.handle.id())
            .collect();

        Some(PlatformViewUpdate {
            placements,
            detached,
        })
    }

    /// Detaches every hosted view, as when the window goes away.
    pub(crate) fn detach_all(&mut self) -> Option<PlatformViewUpdate> {
        if self.hosted.is_empty() {
            return None;
        }

        Some(PlatformViewUpdate {
            placements: Vec::new(),
            detached: std::mem::take(&mut self.hosted),
        })
    }
}

/// Returns the indices of `ids` that should survive deduplication: one index per
/// distinct id, the last occurrence of that id, in first-appearance order.
///
/// A view painted more than once in a frame has a single native instance, so the
/// topmost paint — the last one — owns its bounds.
fn last_placement_indices(ids: &[PlatformViewId]) -> Vec<usize> {
    let mut indices: Vec<usize> = Vec::with_capacity(ids.len());
    for (index, id) in ids.iter().enumerate() {
        match indices.iter_mut().find(|existing| ids[**existing] == *id) {
            Some(existing) => *existing = index,
            None => indices.push(index),
        }
    }
    indices.sort_unstable();
    indices
}

/// Returns the hosted ids that no longer appear among the painted ids, in the
/// order they were hosted.
fn detached_ids(hosted: &[PlatformViewId], painted: &[PlatformViewId]) -> Vec<PlatformViewId> {
    hosted
        .iter()
        .filter(|id| !painted.contains(id))
        .copied()
        .collect()
}

/// Rounds window-space bounds onto the display's device-pixel grid, returning
/// logical pixels again.
///
/// Native views are positioned in logical points, so honoring the scale factor
/// means landing on the same grid the renderer rasterizes GPUI content to;
/// otherwise a hosted view drifts by a fraction of a pixel against the GPUI
/// content around it.
pub fn snap_platform_view_bounds(bounds: Bounds<Pixels>, scale_factor: f32) -> Bounds<Pixels> {
    if !scale_factor.is_finite() || scale_factor <= 0.0 {
        return bounds;
    }

    let left = round_to_device_pixel(bounds.left().0, scale_factor) / scale_factor;
    let top = round_to_device_pixel(bounds.top().0, scale_factor) / scale_factor;
    let right = (round_to_device_pixel(bounds.right().0, scale_factor) / scale_factor).max(left);
    let bottom = (round_to_device_pixel(bounds.bottom().0, scale_factor) / scale_factor).max(top);

    Bounds::from_corners(point(px(left), px(top)), point(px(right), px(bottom)))
}

/// Converts window-space bounds, whose origin is the window's top-left corner
/// with y growing downwards, into the origin a bottom-left-origin coordinate
/// system wants — the convention AppKit uses for a non-flipped `NSView`.
///
/// `container_height` is the height of the view the hosted view is placed in.
pub fn flip_bounds_origin_y(bounds: Bounds<Pixels>, container_height: Pixels) -> Point<Pixels> {
    point(
        bounds.origin.x,
        container_height - bounds.origin.y - bounds.size.height,
    )
}

/// Converts window-space bounds into the physical-pixel rectangle a platform
/// layer positions a native view at.
///
/// Win32 and the other window systems that address windows in physical pixels
/// need this conversion; the size is clamped at zero so a degenerate layout
/// cannot ask a native view for a negative extent.
pub fn platform_view_physical_bounds(
    bounds: Bounds<Pixels>,
    scale_factor: f32,
) -> Bounds<DevicePixels> {
    let scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };

    let left = (bounds.left().0 * scale_factor).round() as i32;
    let top = (bounds.top().0 * scale_factor).round() as i32;
    let right = ((bounds.right().0 * scale_factor).round() as i32).max(left);
    let bottom = ((bounds.bottom().0 * scale_factor).round() as i32).max(top);

    Bounds {
        origin: point(DevicePixels(left), DevicePixels(top)),
        size: size(DevicePixels(right - left), DevicePixels(bottom - top)),
    }
}

/// The bookkeeping a platform layer needs while it hosts native views, kept
/// apart from the native calls so attach, reposition, restack and detach
/// sequencing can be tested without a window.
///
/// `A` carries whatever the platform layer captured at attach time and must put
/// back verbatim at detach: on Windows the child window's parent, window styles
/// and window region.
pub struct PlatformViewHosting<A> {
    /// Hosted views in the stacking order last applied, bottom-most first.
    hosted: Vec<HostedView<A>>,
}

struct HostedView<A> {
    id: PlatformViewId,
    attributes: A,
    geometry: Option<HostedGeometry>,
}

/// The frame last applied to a hosted view, remembered so unchanged frames cost
/// no native calls.
///
/// The scale factor is part of the identity because a per-monitor DPI change
/// leaves the logical bounds alone while moving the view's physical rectangle.
#[derive(Clone, Copy, PartialEq)]
struct HostedGeometry {
    bounds: Bounds<DevicePixels>,
    scale_factor: f32,
}

impl<A> Default for PlatformViewHosting<A> {
    fn default() -> Self {
        Self { hosted: Vec::new() }
    }
}

impl<A> PlatformViewHosting<A> {
    /// Returns true while no view is hosted.
    pub fn is_empty(&self) -> bool {
        self.hosted.is_empty()
    }

    /// Returns whether the given view is hosted.
    pub fn contains(&self, id: PlatformViewId) -> bool {
        self.hosted.iter().any(|hosted| hosted.id == id)
    }

    /// Records a view as hosted, stacked above every view hosted so far.
    ///
    /// Attaching a view that is already hosted replaces what has to be restored
    /// for it and forgets its applied frame, so the next placement is applied
    /// unconditionally.
    pub fn attach(&mut self, id: PlatformViewId, attributes: A) {
        self.hosted.retain(|hosted| hosted.id != id);
        self.hosted.push(HostedView {
            id,
            attributes,
            geometry: None,
        });
    }

    /// Forgets a hosted view, returning what the platform layer must restore.
    pub fn detach(&mut self, id: PlatformViewId) -> Option<A> {
        let index = self.hosted.iter().position(|hosted| hosted.id == id)?;
        Some(self.hosted.remove(index).attributes)
    }

    /// Forgets every hosted view, bottom-most first.
    pub fn detach_all(&mut self) -> Vec<(PlatformViewId, A)> {
        std::mem::take(&mut self.hosted)
            .into_iter()
            .map(|hosted| (hosted.id, hosted.attributes))
            .collect()
    }

    /// Adopts `order` — bottom-most first — as the stacking order, returning
    /// true when it differs from the order last applied and the platform layer
    /// therefore has to restack.
    ///
    /// Ids that are not hosted are ignored, and hosted views the frame did not
    /// mention keep their relative order beneath the ordered ones.
    pub fn restack(&mut self, order: &[PlatformViewId]) -> bool {
        let mut ordered: Vec<usize> = Vec::with_capacity(self.hosted.len());
        for id in order {
            match self.hosted.iter().position(|hosted| hosted.id == *id) {
                Some(index) if !ordered.contains(&index) => ordered.push(index),
                _ => {}
            }
        }
        let mut target = (0..self.hosted.len())
            .filter(|index| !ordered.contains(index))
            .collect::<Vec<_>>();
        target.extend(ordered);

        if target.iter().copied().eq(0..self.hosted.len()) {
            return false;
        }

        let mut source = self
            .hosted
            .drain(..)
            .map(Some)
            .collect::<Vec<Option<HostedView<A>>>>();
        self.hosted = target
            .into_iter()
            .map(|index| {
                source[index]
                    .take()
                    .expect("every index appears exactly once")
            })
            .collect();
        true
    }

    /// Records the frame a hosted view was laid out at, returning the physical
    /// rectangle to move it to, or `None` when it already sits there.
    pub fn place(
        &mut self,
        id: PlatformViewId,
        bounds: Bounds<Pixels>,
        scale_factor: f32,
    ) -> Option<Bounds<DevicePixels>> {
        let hosted = self.hosted.iter_mut().find(|hosted| hosted.id == id)?;
        let geometry = HostedGeometry {
            bounds: platform_view_physical_bounds(bounds, scale_factor),
            scale_factor,
        };
        if hosted.geometry == Some(geometry) {
            return None;
        }
        hosted.geometry = Some(geometry);
        Some(geometry.bounds)
    }

    /// Returns what the platform layer must restore for a hosted view.
    pub fn attributes(&self, id: PlatformViewId) -> Option<&A> {
        self.hosted
            .iter()
            .find(|hosted| hosted.id == id)
            .map(|hosted| &hosted.attributes)
    }

    /// Returns the hosted views in the stacking order last applied, bottom-most
    /// first.
    pub fn ids(&self) -> Vec<PlatformViewId> {
        self.hosted.iter().map(|hosted| hosted.id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct DropProbe(Rc<Cell<bool>>);

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    use crate::size;

    fn id(value: usize) -> PlatformViewId {
        PlatformViewId(value)
    }

    #[test]
    fn platform_view_bounds_snap_to_the_device_pixel_grid() {
        let bounds = Bounds {
            origin: point(px(10.3), px(20.4)),
            size: size(px(100.2), px(50.1)),
        };

        let snapped = snap_platform_view_bounds(bounds, 2.0);

        assert_eq!(snapped.origin, point(px(10.5), px(20.5)));
        assert_eq!(snapped.size, size(px(100.0), px(50.0)));
    }

    #[test]
    fn platform_view_bounds_are_left_alone_without_a_usable_scale_factor() {
        let bounds = Bounds {
            origin: point(px(10.3), px(20.4)),
            size: size(px(100.2), px(50.1)),
        };

        assert_eq!(snap_platform_view_bounds(bounds, 0.0), bounds);
        assert_eq!(snap_platform_view_bounds(bounds, f32::NAN), bounds);
    }

    #[test]
    fn platform_view_bounds_never_snap_to_a_negative_size() {
        let bounds = Bounds {
            origin: point(px(10.0), px(20.0)),
            size: size(px(0.0), px(0.0)),
        };

        let snapped = snap_platform_view_bounds(bounds, 2.0);

        assert_eq!(snapped.size, size(px(0.0), px(0.0)));
    }

    #[test]
    fn platform_view_bounds_flip_to_a_bottom_left_origin() {
        let bounds = Bounds {
            origin: point(px(10.0), px(30.0)),
            size: size(px(100.0), px(50.0)),
        };

        let origin = flip_bounds_origin_y(bounds, px(200.0));

        assert_eq!(origin, point(px(10.0), px(120.0)));
    }

    #[test]
    fn platform_view_flip_is_its_own_inverse() {
        let bounds = Bounds {
            origin: point(px(4.0), px(7.0)),
            size: size(px(20.0), px(11.0)),
        };
        let container_height = px(90.0);

        let flipped = flip_bounds_origin_y(bounds, container_height);
        let round_tripped = flip_bounds_origin_y(
            Bounds {
                origin: flipped,
                size: bounds.size,
            },
            container_height,
        );

        assert_eq!(round_tripped, bounds.origin);
    }

    #[test]
    fn platform_view_deduplication_keeps_the_last_paint_of_a_view() {
        let ids = [id(1), id(2), id(1), id(3)];

        assert_eq!(last_placement_indices(&ids), vec![1, 2, 3]);
    }

    #[test]
    fn platform_view_lifetime_waits_for_the_last_handle_clone() {
        let dropped = Rc::new(Cell::new(false));
        let owner = Rc::new(DropProbe(Rc::clone(&dropped)));
        let mut lifetime = PlatformViewLifetime::default();
        lifetime.keep(owner);
        let platform_clone = lifetime.clone();

        drop(lifetime);
        assert!(!dropped.get(), "the platform still retains one handle");

        drop(platform_clone);
        assert!(dropped.get(), "the last handle releases its native owner");
    }

    #[test]
    fn platform_view_deduplication_preserves_paint_order() {
        let ids = [id(7), id(4), id(9)];

        assert_eq!(last_placement_indices(&ids), vec![0, 1, 2]);
        assert!(last_placement_indices(&[]).is_empty());
    }

    #[test]
    fn platform_view_diff_detaches_only_views_that_stopped_painting() {
        let hosted = [id(1), id(2), id(3)];
        let painted = [id(2), id(4)];

        assert_eq!(detached_ids(&hosted, &painted), vec![id(1), id(3)]);
    }

    #[test]
    fn platform_view_registry_is_inert_until_a_view_is_painted() {
        let mut registry = PlatformViewRegistry::default();

        assert!(registry.sync(&[], 2.0).is_none());
        assert!(registry.detach_all().is_none());
    }

    #[test]
    fn platform_view_registry_detaches_views_that_stop_painting() {
        let mut registry = PlatformViewRegistry::default();
        registry.hosted = vec![id(1), id(2)];

        let update = registry
            .sync(&[], 2.0)
            .expect("hosted views need an update");

        assert!(update.placements.is_empty());
        assert_eq!(update.detached, vec![id(1), id(2)]);
        assert!(registry.hosted.is_empty());
        assert!(registry.sync(&[], 2.0).is_none());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    #[test]
    fn platform_view_registry_retains_frame_placement_and_handle_identity() {
        let mut registry = PlatformViewRegistry::default();
        let handle = PlatformViewHandle::inert();
        let placement = PlatformViewPlacement {
            handle: handle.clone(),
            bounds: Bounds {
                origin: point(px(10.3), px(20.4)),
                size: size(px(100.2), px(50.1)),
            },
        };

        let update = registry
            .sync(&[placement], 2.0)
            .expect("painted views need an update");

        assert_eq!(update.placements.len(), 1);
        assert_eq!(update.placements[0].handle, handle);
        assert_eq!(
            update.placements[0].bounds.origin,
            point(px(10.5), px(20.5))
        );
        assert_eq!(update.placements[0].bounds.size, size(px(100.), px(50.)));
        assert!(update.detached.is_empty());

        let detached = registry
            .sync(&[], 2.0)
            .expect("hosted views need a detach update");
        assert_eq!(detached.detached, vec![handle.id()]);
        let handle_id = handle.id();
        #[expect(
            clippy::redundant_clone,
            reason = "this assertion covers clone identity"
        )]
        let cloned_handle = handle.clone();
        assert_eq!(handle_id, cloned_handle.id());
    }

    #[test]
    fn platform_view_physical_bounds_scale_the_snapped_rectangle() {
        let bounds = Bounds {
            origin: point(px(10.5), px(20.5)),
            size: size(px(100.0), px(50.0)),
        };

        let physical = platform_view_physical_bounds(bounds, 2.0);

        assert_eq!(physical.origin, point(DevicePixels(21), DevicePixels(41)));
        assert_eq!(physical.size, size(DevicePixels(200), DevicePixels(100)));
    }

    #[test]
    fn platform_view_physical_bounds_follow_a_dpi_change() {
        let bounds = Bounds {
            origin: point(px(10.0), px(20.0)),
            size: size(px(100.0), px(50.0)),
        };

        assert_ne!(
            platform_view_physical_bounds(bounds, 1.0),
            platform_view_physical_bounds(bounds, 1.5)
        );
        assert_eq!(
            platform_view_physical_bounds(bounds, 1.5).origin,
            point(DevicePixels(15), DevicePixels(30))
        );
    }

    #[test]
    fn platform_view_physical_bounds_never_go_negative() {
        let inverted = Bounds {
            origin: point(px(40.0), px(30.0)),
            size: size(px(-30.0), px(-25.0)),
        };

        let physical = platform_view_physical_bounds(inverted, 2.0);

        assert_eq!(physical.origin, point(DevicePixels(80), DevicePixels(60)));
        assert_eq!(physical.size, size(DevicePixels(0), DevicePixels(0)));
    }

    #[test]
    fn platform_view_physical_bounds_fall_back_to_an_unscaled_rectangle() {
        let bounds = Bounds {
            origin: point(px(10.0), px(20.0)),
            size: size(px(30.0), px(40.0)),
        };

        assert_eq!(
            platform_view_physical_bounds(bounds, 0.0),
            platform_view_physical_bounds(bounds, 1.0)
        );
        assert_eq!(
            platform_view_physical_bounds(bounds, f32::NAN),
            platform_view_physical_bounds(bounds, 1.0)
        );
    }

    #[test]
    fn platform_view_hosting_records_what_detaching_must_restore() {
        let mut hosting = PlatformViewHosting::<&'static str>::default();

        assert!(hosting.is_empty());
        hosting.attach(id(1), "before-1");
        hosting.attach(id(2), "before-2");

        assert!(hosting.contains(id(1)));
        assert_eq!(hosting.attributes(id(2)), Some(&"before-2"));
        assert_eq!(hosting.detach(id(1)), Some("before-1"));
        assert!(!hosting.contains(id(1)));
        assert_eq!(hosting.detach(id(1)), None);
        assert_eq!(hosting.detach_all(), vec![(id(2), "before-2")]);
        assert!(hosting.is_empty());
    }

    #[test]
    fn platform_view_hosting_reattaching_replaces_the_restore_state() {
        let mut hosting = PlatformViewHosting::<&'static str>::default();
        hosting.attach(id(1), "stale");
        hosting.place(id(1), Bounds::default(), 1.0);

        hosting.attach(id(1), "fresh");

        assert_eq!(hosting.ids(), vec![id(1)]);
        assert_eq!(hosting.attributes(id(1)), Some(&"fresh"));
        assert!(
            hosting.place(id(1), Bounds::default(), 1.0).is_some(),
            "a freshly attached view has no applied frame to skip"
        );
    }

    #[test]
    fn platform_view_hosting_moves_only_when_the_frame_changed() {
        let mut hosting = PlatformViewHosting::<()>::default();
        hosting.attach(id(1), ());
        let bounds = Bounds {
            origin: point(px(10.0), px(20.0)),
            size: size(px(100.0), px(50.0)),
        };

        assert_eq!(
            hosting.place(id(1), bounds, 2.0),
            Some(platform_view_physical_bounds(bounds, 2.0))
        );
        assert_eq!(hosting.place(id(1), bounds, 2.0), None);
        assert_eq!(
            hosting.place(id(1), bounds, 1.5),
            Some(platform_view_physical_bounds(bounds, 1.5)),
            "a scale factor change must move the view even at unchanged logical bounds"
        );
        assert_eq!(hosting.place(id(2), bounds, 1.5), None);
    }

    #[test]
    fn platform_view_hosting_restacks_into_paint_order() {
        let mut hosting = PlatformViewHosting::<()>::default();
        hosting.attach(id(1), ());
        hosting.attach(id(2), ());
        hosting.attach(id(3), ());

        assert!(!hosting.restack(&[id(1), id(2), id(3)]));
        assert!(hosting.restack(&[id(3), id(1), id(2)]));
        assert_eq!(hosting.ids(), vec![id(3), id(1), id(2)]);
        assert!(!hosting.restack(&[id(3), id(1), id(2)]));
    }

    #[test]
    fn platform_view_hosting_restack_keeps_unmentioned_views_underneath() {
        let mut hosting = PlatformViewHosting::<()>::default();
        hosting.attach(id(1), ());
        hosting.attach(id(2), ());
        hosting.attach(id(3), ());

        assert!(hosting.restack(&[id(9), id(1)]));

        assert_eq!(hosting.ids(), vec![id(2), id(3), id(1)]);
    }

    #[test]
    fn platform_view_hosting_restack_preserves_applied_frames() {
        let mut hosting = PlatformViewHosting::<()>::default();
        hosting.attach(id(1), ());
        hosting.attach(id(2), ());
        let bounds = Bounds {
            origin: point(px(1.0), px(2.0)),
            size: size(px(3.0), px(4.0)),
        };
        hosting.place(id(1), bounds, 1.0);

        assert!(hosting.restack(&[id(2), id(1)]));

        assert_eq!(hosting.place(id(1), bounds, 1.0), None);
    }

    #[test]
    fn platform_view_registry_detaches_everything_on_teardown() {
        let mut registry = PlatformViewRegistry::default();
        registry.hosted = vec![id(5), id(6)];

        let update = registry.detach_all().expect("hosted views need detaching");

        assert!(update.placements.is_empty());
        assert_eq!(update.detached, vec![id(5), id(6)]);
        assert!(registry.detach_all().is_none());
    }
}
