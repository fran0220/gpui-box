//! Paint-local frost and edge-fade effects.
//!
//! These use the GPUI fork pinned by this workspace. The fork adds EdgeFade
//! and BackdropBlur primitives while preserving an opaque platform fallback.

use gpui::{
    AnyElement, App, Bounds, Corners, EdgeFade, Element, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window, px,
};
use gpui_kit_theme::Theme;

pub fn frosted(theme: &Theme, corner_radius: f32, child: impl IntoElement) -> Frosted {
    Frosted {
        enabled: theme.effects.glass_alpha < 1.0,
        corner_radius,
        blur_radius: theme.effects.backdrop_blur,
        child: child.into_any_element(),
    }
}

pub struct Frosted {
    enabled: bool,
    corner_radius: f32,
    blur_radius: f32,
    child: AnyElement,
}

impl std::fmt::Debug for Frosted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Frosted")
            .field("enabled", &self.enabled)
            .field("corner_radius", &self.corner_radius)
            .field("blur_radius", &self.blur_radius)
            .finish_non_exhaustive()
    }
}

impl Element for Frosted {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.enabled {
            window.paint_layer(bounds, |window| {
                window.paint_backdrop_blur(
                    bounds,
                    Corners::all(px(self.corner_radius)),
                    px(self.blur_radius),
                );
                self.child.paint(window, cx);
            });
        } else {
            self.child.paint(window, cx);
        }
    }
}

impl IntoElement for Frosted {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

pub fn edge_faded(theme: &Theme, top: bool, bottom: bool, child: impl IntoElement) -> EdgeFaded {
    EdgeFaded {
        band: theme.effects.edge_fade_band,
        top,
        bottom,
        left: false,
        right: false,
        child: child.into_any_element(),
    }
}

pub struct EdgeFaded {
    band: f32,
    top: bool,
    bottom: bool,
    left: bool,
    right: bool,
    child: AnyElement,
}

impl std::fmt::Debug for EdgeFaded {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EdgeFaded")
            .field("band", &self.band)
            .field("top", &self.top)
            .field("bottom", &self.bottom)
            .field("left", &self.left)
            .field("right", &self.right)
            .finish_non_exhaustive()
    }
}

impl EdgeFaded {
    pub fn fade_left(mut self, enabled: bool) -> Self {
        self.left = enabled;
        self
    }

    pub fn fade_right(mut self, enabled: bool) -> Self {
        self.right = enabled;
        self
    }
}

impl Element for EdgeFaded {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let fade = (self.top || self.bottom || self.left || self.right).then_some(EdgeFade {
            bounds,
            band: px(self.band),
            top: self.top,
            bottom: self.bottom,
            left: self.left,
            right: self.right,
        });
        window.with_edge_fade(fade, |window| self.child.paint(window, cx));
    }
}

impl IntoElement for EdgeFaded {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
