//! Content that fades out where it runs off the edge of its region.
//!
//! [`ScrollFade`] says the same thing the shadow along the top of a
//! [`ScrollArea`](crate::layout::ScrollArea) says — there is more content past
//! this edge — and says it where a shadow cannot: over a translucent or
//! frosted surface, where "the colour of what is behind the window" is not a
//! colour anything can paint. Instead of covering the content with a gradient,
//! the fade multiplies the opacity of every primitive by how close it is to an
//! active edge, so a glyph half inside the band is half faded rather than the
//! whole row dimming at once.
//!
//! The edges are the caller's statement about overflow, not this component's
//! guess: a region scrolled to its end fades at the start edge only, and one
//! that fits fades at neither. Reading those from
//! [`scroll_offset`](crate::layout::scroll_offset) keeps the fade truthful,
//! and turning both on unconditionally would tell the reader there is content
//! past an edge where there is none.
//!
//! A fade is information rather than decoration, so it is not suppressed under
//! reduced motion; it does not animate, and nothing about it moves on its own.

use gpui::{
    AnyElement, App, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    ParentElement, Pixels, RenderOnce, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::ActiveTheme;

use crate::foundation::Ident;

/// Which edges of a region its content fades towards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FadeEdges {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

impl FadeEdges {
    /// Both ends of a column.
    pub fn vertical() -> Self {
        Self {
            top: true,
            bottom: true,
            ..Self::default()
        }
    }

    /// Both ends of a row.
    pub fn horizontal() -> Self {
        Self {
            left: true,
            right: true,
            ..Self::default()
        }
    }

    /// Whether any edge fades at all. None of them is a region that says
    /// nothing is hidden, which is a fade that must not be painted.
    pub fn any(self) -> bool {
        self.top || self.bottom || self.left || self.right
    }

    /// The edges as a stable list, for a reader that has only the tree.
    pub fn names(self) -> Vec<&'static str> {
        [
            self.top.then_some("top"),
            self.bottom.then_some("bottom"),
            self.left.then_some("left"),
            self.right.then_some("right"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

/// A region whose content fades towards the edges it is scrolled past.
#[derive(IntoElement)]
pub struct ScrollFade {
    ident: Ident,
    edges: FadeEdges,
    band: Option<f32>,
    /// Whether the region is as tall as what it holds instead of as tall as
    /// the space it is offered.
    fit_height: bool,
    /// Whether this region is one a reader can name. A fade a component draws
    /// inside itself is not: the component is the region, and a second node
    /// under the same identity would answer for it.
    published: bool,
    child: Option<AnyElement>,
}

impl std::fmt::Debug for ScrollFade {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScrollFade")
            .field("ident", &self.ident)
            .field("edges", &self.edges)
            .field("band", &self.band)
            .field("has_child", &self.child.is_some())
            .finish()
    }
}

impl ScrollFade {
    /// A region that fades at no edge until the caller says which ones hide
    /// something.
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            edges: FadeEdges::default(),
            band: None,
            fit_height: false,
            published: true,
            child: None,
        }
    }

    /// The same fade, drawn inside a component that already publishes the
    /// region it belongs to. Nothing is added to the tree, so a caller that
    /// wraps that component in a fade of its own still owns the only node
    /// under that identity.
    pub(crate) fn inside(ident: impl Into<Ident>) -> Self {
        Self {
            published: false,
            ..Self::new(ident)
        }
    }

    pub fn edges(mut self, edges: FadeEdges) -> Self {
        self.edges = edges;
        self
    }

    pub fn top(mut self, fade: bool) -> Self {
        self.edges.top = fade;
        self
    }

    pub fn bottom(mut self, fade: bool) -> Self {
        self.edges.bottom = fade;
        self
    }

    pub fn left(mut self, fade: bool) -> Self {
        self.edges.left = fade;
        self
    }

    pub fn right(mut self, fade: bool) -> Self {
        self.edges.right = fade;
        self
    }

    /// How far the ramp reaches inside each active edge, in pixels, when
    /// `effect.edgeFadeBand` is not what this region wants.
    pub fn band(mut self, band: f32) -> Self {
        self.band = Some(band.max(0.0));
        self
    }

    /// Makes the region as tall as its content rather than as tall as the
    /// space around it, which is what a caller who already bounded the scroll
    /// area inside wants. This follows
    /// [`ScrollArea::fit_height`](crate::layout::ScrollArea::fit_height): a
    /// region that fills a height nobody offered has nowhere to draw, and
    /// takes its content with it.
    pub fn fit_height(mut self) -> Self {
        self.fit_height = true;
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
}

impl RenderOnce for ScrollFade {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let band = self.band.unwrap_or(cx.theme().effects.edge_fade_band);
        let edges = self.edges;
        // Which edges are fading is a visible statement about hidden content,
        // so it is published rather than left for a test to infer from pixels.
        let region = div()
            .w_full()
            .when(!self.fit_height, |element| element.h_full())
            .children(self.child);
        let region = if self.published {
            region
                .semantic_in(cx, {
                    let spec = NodeSpec::new(self.ident.semantic_id(), Role::Region);
                    match edges.names().as_slice() {
                        [] => spec.value("none"),
                        names => spec.value(names.join(" ")),
                    }
                })
                .into_any_element()
        } else {
            region.into_any_element()
        };

        Faded {
            edges,
            band: px(band),
            child: region,
        }
    }
}

/// The scope every primitive underneath is faded inside.
struct Faded {
    edges: FadeEdges,
    band: Pixels,
    child: AnyElement,
}

impl Element for Faded {
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
        let fade = (self.edges.any() && self.band > px(0.0)).then_some(gpui::EdgeFade {
            bounds,
            band: self.band,
            top: self.edges.top,
            bottom: self.edges.bottom,
            left: self.edges.left,
            right: self.edges.right,
        });
        window.with_edge_fade(fade, |window| self.child.paint(window, cx));
    }
}

impl IntoElement for Faded {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_region_that_hides_nothing_fades_at_no_edge() {
        assert!(!FadeEdges::default().any());
        assert!(FadeEdges::vertical().any());
        assert_eq!(FadeEdges::horizontal().names(), vec!["left", "right"]);
    }
}
