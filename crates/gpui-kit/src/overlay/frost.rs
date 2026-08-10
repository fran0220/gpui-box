//! A surface that shows what is behind it, out of focus.
//!
//! [`Frost`] is the backing a popover, a dialog or a rail is placed on when
//! the window itself is translucent: the pixels underneath are blurred, and
//! the surface colour is laid over the blur at `effect.glassAlpha` so the
//! content on top keeps its contrast.
//!
//! # One layer, in one order
//!
//! The whole subtree paints inside a single scene layer, which is the reason
//! this is an element and not a styled `div`. Paint order is per-primitive
//! otherwise, so a repaint elsewhere in the frame can reorder the surface's
//! own quads underneath the blur — a divider or a border is then snapshotted
//! and blurred away, intermittently, in a way no test reproduces. Inside one
//! layer the relationship is structural: blur first, fill and content after.
//!
//! # Where blur does not exist
//!
//! A backdrop blur is a renderer capability, not a paintable colour. Where
//! the renderer has none the blur is dropped and the tinted fill is all that
//! remains, which is a legible surface rather than a broken one — this is why
//! the fill is painted whether or not the blur was. A theme that declares
//! itself opaque by setting `effect.glassAlpha` to 1 takes the same path
//! deliberately: there is nothing to see through, so nothing is blurred.

use gpui::{
    AnyElement, App, Bounds, Corners, Element, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, ParentElement, Pixels, RenderOnce, Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Surface};

use crate::foundation::Ident;

/// A frosted-glass surface: blurred backdrop, tinted fill, caller's content.
#[derive(IntoElement)]
pub struct Frost {
    ident: Ident,
    surface: Surface,
    radius: Radius,
    blur: Option<f32>,
    child: Option<AnyElement>,
}

impl std::fmt::Debug for Frost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Frost")
            .field("ident", &self.ident)
            .field("surface", &self.surface)
            .field("radius", &self.radius)
            .field("blur", &self.blur)
            .field("has_child", &self.child.is_some())
            .finish()
    }
}

impl Frost {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            surface: Surface::Overlay,
            radius: Radius::Card,
            blur: None,
            child: None,
        }
    }

    /// Which surface colour is laid over the blur. The overlay surface is the
    /// default because that is what a floating thing is made of.
    pub fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    /// The rounding of the glass. It clips the blur as well as the fill, so a
    /// caller rounding the card inside must say the same thing here or the
    /// blur will show past the corners.
    pub fn radius(mut self, radius: Radius) -> Self {
        self.radius = radius;
        self
    }

    /// How far the backdrop is blurred, in pixels, when `effect.glassBlur` is
    /// not what this particular surface wants.
    pub fn blur(mut self, blur: f32) -> Self {
        self.blur = Some(blur.max(0.0));
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
}

impl RenderOnce for Frost {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let radius = theme.radius(self.radius);
        let alpha = theme.effects.glass_alpha.clamp(0.0, 1.0);
        let fill = theme.surface(self.surface).opacity(alpha);
        let blur = self.blur.unwrap_or(theme.effects.glass_blur);
        let translucent = blurs(alpha, blur);

        let glass = div()
            .rounded(px(radius))
            .bg(fill)
            .children(self.child)
            .semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::Region));

        Glass {
            radius: px(radius),
            blur: px(blur),
            translucent,
            child: glass.into_any_element(),
        }
    }
}

/// Whether there is anything for a blur to show. Blurring what a fully opaque
/// fill is about to cover costs a render pass and changes no pixel, and a
/// radius of zero is a caller saying not to blur at all.
fn blurs(alpha: f32, blur: f32) -> bool {
    alpha < 1.0 && blur > 0.0
}

/// The single scene layer, with the backdrop blur painted first inside it.
struct Glass {
    radius: Pixels,
    blur: Pixels,
    translucent: bool,
    child: AnyElement,
}

impl Element for Glass {
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
        _request_layout: &mut Self::RequestLayoutState,
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
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if !self.translucent {
            self.child.paint(window, cx);
            return;
        }
        window.paint_layer(bounds, |window| {
            window.paint_backdrop_blur(bounds, Corners::all(self.radius), self.blur);
            self.child.paint(window, cx);
        });
    }
}

impl IntoElement for Glass {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_opaque_theme_blurs_nothing() {
        assert!(blurs(0.72, 24.0));
        assert!(!blurs(1.0, 24.0), "an opaque fill hides what it blurred");
        assert!(!blurs(0.72, 0.0), "no radius is no blur");
    }
}
