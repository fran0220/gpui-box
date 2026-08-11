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

/// Hosts a native platform view — on macOS an `NSView` such as a `WKWebView` —
/// inside the window, at the bounds this element is laid out at.
///
/// The element itself paints nothing. Size it the way you would size any other
/// element; GPUI owns the native view's frame from then on and repositions it
/// after every frame it is painted in. A frame that does not paint the element
/// hides and detaches the view, so unmounting is just not rendering it.
///
/// # Stacking
///
/// The hosted view is a sibling of GPUI's rendering surface, ordered above it.
/// Within the element's bounds it therefore covers GPUI content unconditionally:
/// GPUI clipping, overlays, tooltips, modals and anything else drawn on the same
/// surface do **not** composite over it. Keep interface elements that must stay
/// visible outside the hosted view's bounds.
///
/// # Platforms
///
/// Only macOS hosts native views today. Elsewhere the element still lays out and
/// reserves space, and no view appears.
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
