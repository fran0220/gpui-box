use crate::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, PlatformViewHandle, Style, StyleRefinement, Styled, Window,
};
use refineable::Refineable;

/// An element that gives a natively hosted view a place in GPUI's layout.
///
/// See [`platform_view`].
pub struct PlatformView {
    handle: PlatformViewHandle,
    style: StyleRefinement,
}

/// Hosts a native platform view — an `NSView` on macOS or a child `HWND` on
/// Windows — inside the window, at the bounds this element is laid out at.
///
/// The element itself paints nothing. Size it the way you would size any other
/// element; GPUI owns the native view's frame from then on and repositions it
/// after every frame it is painted in. A frame that does not paint the element
/// hides and detaches the view, so unmounting is just not rendering it.
///
/// # Stacking
///
/// The hosted view is ordered above GPUI's root scene. Content drawn on that
/// same base surface does not composite over the native view. When the window's
/// scene overlay is enabled, deferred and window-level overlay content is drawn
/// on a separate surface above hosted views.
///
/// # Platforms
///
/// macOS and Windows host native views. Elsewhere the element still lays out,
/// reserves space, and does not host a view.
pub fn platform_view(handle: PlatformViewHandle) -> PlatformView {
    PlatformView {
        handle,
        style: StyleRefinement::default(),
    }
}

impl Element for PlatformView {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        window.paint_platform_view(bounds, self.handle.clone());
    }
}

impl IntoElement for PlatformView {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Styled for PlatformView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
