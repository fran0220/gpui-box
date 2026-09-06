//! A ramp that separates scrolling content from a floating glass surface.
//!
//! Soft fades and scatters the content under the overlap. Hard paints an
//! opaque backing. Neither is interactive: the ramp is a visual treatment
//! over content the caller already laid out.

use gpui::{
    AnyElement, App, Bounds, Corners, Element, GlassEdge, GlassMaterial, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels, RenderOnce, Styled, Window,
    div, point, px, size,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::ActiveTheme;

use crate::foundation::Ident;
use crate::layout::scroll_fade::FadeEdges;

/// How a scroll-edge ramp separates floating chrome from the content under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollEdgeKind {
    /// Progressive blur and fade into the content.
    #[default]
    Soft,
    /// An opaque backing, used when transparency is reduced.
    Hard,
}

/// A visual ramp at the edges where floating glass overlaps scrolling content.
#[derive(IntoElement)]
pub struct ScrollEdgeEffect {
    ident: Ident,
    edges: FadeEdges,
    kind: ScrollEdgeKind,
    band: Option<f32>,
    blur: Option<f32>,
    child: Option<AnyElement>,
}

impl std::fmt::Debug for ScrollEdgeEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScrollEdgeEffect")
            .field("ident", &self.ident)
            .field("edges", &self.edges)
            .field("kind", &self.kind)
            .field("band", &self.band)
            .field("blur", &self.blur)
            .field("has_child", &self.child.is_some())
            .finish()
    }
}

impl ScrollEdgeEffect {
    /// A ramp that paints no edge until the caller names one.
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            edges: FadeEdges::default(),
            kind: ScrollEdgeKind::Soft,
            band: None,
            blur: None,
            child: None,
        }
    }

    /// Which edges hide content under floating chrome.
    pub fn edges(mut self, edges: FadeEdges) -> Self {
        self.edges = edges;
        self
    }

    /// Whether the top edge ramps.
    pub fn top(mut self, fade: bool) -> Self {
        self.edges.top = fade;
        self
    }

    /// Whether the bottom edge ramps.
    pub fn bottom(mut self, fade: bool) -> Self {
        self.edges.bottom = fade;
        self
    }

    /// Whether the left edge ramps.
    pub fn left(mut self, fade: bool) -> Self {
        self.edges.left = fade;
        self
    }

    /// Whether the right edge ramps.
    pub fn right(mut self, fade: bool) -> Self {
        self.edges.right = fade;
        self
    }

    /// Soft scatters; Hard is an opaque backing.
    pub fn kind(mut self, kind: ScrollEdgeKind) -> Self {
        self.kind = kind;
        self
    }

    /// Progressive blur and fade.
    pub fn soft(self) -> Self {
        self.kind(ScrollEdgeKind::Soft)
    }

    /// Opaque backing, for reduced transparency.
    pub fn hard(self) -> Self {
        self.kind(ScrollEdgeKind::Hard)
    }

    /// How far the ramp reaches, overriding `effect.scrollEdgeBand`.
    pub fn band(mut self, band: f32) -> Self {
        self.band = Some(band.max(0.0));
        self
    }

    /// How far the soft ramp scatters, overriding `effect.scrollEdgeBlur`.
    pub fn blur(mut self, blur: f32) -> Self {
        self.blur = Some(blur.max(0.0));
        self
    }

    /// Content the ramp is painted over.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
}

impl RenderOnce for ScrollEdgeEffect {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let band = self.band.unwrap_or(theme.effects.scroll_edge_band);
        let blur = self.blur.unwrap_or(theme.effects.scroll_edge_blur);
        let fill = theme.colors.canvas;
        let region = div()
            .relative()
            .w_full()
            .h_full()
            .children(self.child)
            .semantic_in(cx, {
                let spec = NodeSpec::new(self.ident.semantic_id(), Role::Region);
                match self.edges.names().as_slice() {
                    [] => spec.value("none"),
                    names => spec.value(names.join(" ")),
                }
            });

        EdgeRamp {
            edges: self.edges,
            kind: self.kind,
            band: px(band),
            blur: px(blur),
            fill,
            child: region.into_any_element(),
        }
    }
}

struct EdgeRamp {
    edges: FadeEdges,
    kind: ScrollEdgeKind,
    band: Pixels,
    blur: Pixels,
    fill: Hsla,
    child: AnyElement,
}

impl Element for EdgeRamp {
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
        self.child.paint(window, cx);
        if !self.edges.any() {
            return;
        }
        let band = f32::from(self.band)
            .min(f32::from(bounds.size.height))
            .max(0.0);
        let band_x = f32::from(self.band)
            .min(f32::from(bounds.size.width))
            .max(0.0);
        if self.edges.top {
            paint_band(
                window,
                Bounds {
                    origin: bounds.origin,
                    size: size(bounds.size.width, px(band)),
                },
                GlassEdge::Top,
                self.kind,
                self.blur,
                px(band),
                self.fill,
            );
        }
        if self.edges.bottom {
            paint_band(
                window,
                Bounds {
                    origin: point(
                        bounds.origin.x,
                        bounds.origin.y + bounds.size.height - px(band),
                    ),
                    size: size(bounds.size.width, px(band)),
                },
                GlassEdge::Bottom,
                self.kind,
                self.blur,
                px(band),
                self.fill,
            );
        }
        if self.edges.left {
            paint_band(
                window,
                Bounds {
                    origin: bounds.origin,
                    size: size(px(band_x), bounds.size.height),
                },
                GlassEdge::Left,
                self.kind,
                self.blur,
                px(band_x),
                self.fill,
            );
        }
        if self.edges.right {
            paint_band(
                window,
                Bounds {
                    origin: point(
                        bounds.origin.x + bounds.size.width - px(band_x),
                        bounds.origin.y,
                    ),
                    size: size(px(band_x), bounds.size.height),
                },
                GlassEdge::Right,
                self.kind,
                self.blur,
                px(band_x),
                self.fill,
            );
        }
    }
}

impl IntoElement for EdgeRamp {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

fn paint_band(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    edge: GlassEdge,
    kind: ScrollEdgeKind,
    blur: Pixels,
    band: Pixels,
    fill: Hsla,
) {
    if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
        return;
    }
    match kind {
        ScrollEdgeKind::Soft => {
            let material = GlassMaterial::frosted(blur).with_edge_mask(edge, band);
            window.paint_backdrop_glass(bounds, Corners::all(px(0.0)), material, &[]);
        }
        ScrollEdgeKind::Hard => {
            window.paint_quad(gpui::fill(bounds, fill));
        }
    }
}
