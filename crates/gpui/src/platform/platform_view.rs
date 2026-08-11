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

use crate::{Bounds, Pixels, Point, point, px, util::round_to_device_pixel};
use std::fmt;

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
    use super::PlatformViewId;
    use objc::{msg_send, runtime::Object, sel, sel_impl};
    use std::{ffi::c_void, fmt, ptr::NonNull};

    /// A retained reference to a native view hosted inside a GPUI window.
    ///
    /// On macOS this owns a strong reference to an `NSView`. Cloning retains and
    /// dropping releases, so the view outlives every element that paints it.
    ///
    /// Construct, clone and drop handles on the main thread: the wrapped object
    /// is an AppKit view, and AppKit only promises main-thread safety.
    pub struct PlatformViewHandle {
        view: NonNull<Object>,
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
            Self { view }
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
            Self { view: self.view }
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
    use super::PlatformViewId;
    use std::fmt;
    use windows::Win32::Foundation::HWND;

    /// A non-owning reference to a child `HWND` hosted inside a GPUI window.
    ///
    /// Windows does not provide retain/release semantics for window handles.
    /// Cloning this value retains the GPUI hosting reference, but the component
    /// that created the `HWND` remains responsible for destroying it after it is
    /// no longer hosted and all handles have been dropped.
    #[derive(Clone)]
    pub struct PlatformViewHandle {
        hwnd: HWND,
    }

    impl PlatformViewHandle {
        /// Wraps a child `HWND` so it can be hosted by a GPUI window.
        ///
        /// # Safety
        ///
        /// `hwnd` must be a non-null, live child window owned by the calling
        /// process. It must remain valid, and must not be destroyed, for as long
        /// as this handle or any clone is painted or retained by GPUI. The
        /// caller remains responsible for destroying it on its owning thread.
        pub unsafe fn from_hwnd(hwnd: HWND) -> Self {
            assert!(
                !hwnd.is_invalid(),
                "PlatformViewHandle::from_hwnd requires a non-null HWND"
            );
            Self { hwnd }
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
    use super::PlatformViewId;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// An inert stand-in for a natively hosted view.
    ///
    /// This stub exists so cross-platform code that mentions
    /// [`PlatformViewHandle`] still compiles; it does not refer to or host a
    /// native view. Painting the [`crate::platform_view`] element with one only
    /// reserves layout space.
    #[derive(Clone, PartialEq, Eq, Debug)]
    pub struct PlatformViewHandle {
        id: PlatformViewId,
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
            }
        }

        /// Returns this handle's stable identity.
        pub fn id(&self) -> PlatformViewId {
            self.id
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
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
    fn platform_view_registry_detaches_everything_on_teardown() {
        let mut registry = PlatformViewRegistry::default();
        registry.hosted = vec![id(5), id(6)];

        let update = registry.detach_all().expect("hosted views need detaching");

        assert!(update.placements.is_empty());
        assert_eq!(update.detached, vec![id(5), id(6)]);
        assert!(registry.detach_all().is_none());
    }
}
