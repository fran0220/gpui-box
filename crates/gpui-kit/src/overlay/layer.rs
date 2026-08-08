//! Placement, stacking, and dismissal for surfaces that float above content.
//!
//! Paint order comes from the `zIndex` tokens rather than from the order in
//! which a view happens to build its children, so a tooltip raised inside a
//! modal still paints above it.

use std::rc::Rc;

use gpui::{
    Anchor, AnyElement, App, ClickEvent, Div, ElementId, IntoElement, Pixels, Point, RenderOnce,
    Stateful, Window, div, prelude::*, px,
};
use gpui_kit_theme::{Elevation, Layer, Theme};

use crate::foundation::{ActiveTheme, Ident, StyledExt};

type DismissHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One side of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    /// True when the surface stretches vertically and is pinned horizontally.
    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// True when the surface hangs off the low end of its axis.
    pub fn is_leading(self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }
}

/// Where a floating surface sits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// Below the anchor element, left edges aligned.
    Below,
    /// Above the anchor element, left edges aligned.
    Above,
    /// At an absolute window position, such as a cursor. A surface that would
    /// leave the viewport flips to the other side of that position.
    At(Point<Pixels>),
    /// Centered in the window.
    Center,
    /// Pinned to one side of the window and stretched along it.
    Edge(Edge),
}

/// A floating surface.
///
/// The caller owns whether the overlay exists at all; this type owns only
/// where it paints, what sits behind it, and how a dismissal is reported.
#[derive(IntoElement)]
pub struct Overlay {
    ident: Ident,
    layer: Layer,
    placement: Placement,
    scrim: bool,
    content: Option<AnyElement>,
    on_dismiss: Option<DismissHandler>,
}

impl std::fmt::Debug for Overlay {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Overlay")
            .field("ident", &self.ident)
            .field("layer", &self.layer)
            .field("placement", &self.placement)
            .field("scrim", &self.scrim)
            .field("dismissible", &self.on_dismiss.is_some())
            .finish()
    }
}

impl Overlay {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            layer: Layer::Popover,
            placement: Placement::Below,
            scrim: false,
            content: None,
            on_dismiss: None,
        }
    }

    /// A dialog: centered, on the modal layer, behind a scrim.
    pub fn modal(ident: impl Into<Ident>) -> Self {
        Self::new(ident)
            .layer(Layer::Modal)
            .placement(Placement::Center)
            .scrim(true)
    }

    /// A drawer: pinned to one side of the window, on the modal layer, behind
    /// a scrim.
    pub fn edge(ident: impl Into<Ident>, edge: Edge) -> Self {
        Self::new(ident)
            .layer(Layer::Modal)
            .placement(Placement::Edge(edge))
            .scrim(true)
    }

    pub fn layer(mut self, layer: Layer) -> Self {
        self.layer = layer;
        self
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Dims and blocks the content behind the overlay.
    pub fn scrim(mut self, scrim: bool) -> Self {
        self.scrim = scrim;
        self
    }

    pub fn child(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    /// Reports a click on the scrim. Escape is the caller's to bind, because
    /// only the caller knows which action closing should dispatch.
    pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }

    fn anchor(&self) -> Anchor {
        match self.placement {
            Placement::Above => Anchor::BottomLeft,
            _ => Anchor::TopLeft,
        }
    }
}

impl RenderOnce for Overlay {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        // The overlay is painted out of its parent's layout, so the scrim has
        // to be sized to the window rather than inherited from a parent.
        let viewport = window.viewport_size();
        let element_id: ElementId = self.ident.element_id();
        let anchor = self.anchor();
        let content = self.content.unwrap_or_else(|| div().into_any_element());

        let surface = div()
            .id(element_id)
            .occlude()
            .child(content)
            .into_any_element();

        // An anchored surface flips to the opposite corner rather than being
        // slid along the edge, so a menu that would leave the viewport still
        // hangs off its anchor instead of covering it.
        let mut anchored = gpui::anchored().anchor(anchor);
        if let Placement::At(position) = self.placement {
            anchored = anchored.position(position);
        }
        if self.placement == Placement::Center {
            anchored = anchored.snap_to_window_with_margin(px(theme.spacing.sm));
        }

        let placed = match self.placement {
            Placement::Center => scrim_frame(&theme, viewport, self.scrim, self.on_dismiss.clone())
                .items_center()
                .justify_center()
                .child(surface)
                .into_any_element(),
            // The surface keeps its own size along the pinned axis and is
            // left to stretch across the other one, which is what makes a
            // drawer reach both ends of the side it hangs from.
            Placement::Edge(edge) => {
                scrim_frame(&theme, viewport, self.scrim, self.on_dismiss.clone())
                    .map(|frame| {
                        if edge.is_horizontal() {
                            frame.flex_row()
                        } else {
                            frame.flex_col()
                        }
                    })
                    .map(|frame| {
                        if edge.is_leading() {
                            frame.justify_start()
                        } else {
                            frame.justify_end()
                        }
                    })
                    .child(surface)
                    .into_any_element()
            }
            _ if self.scrim => scrim_frame(&theme, viewport, true, self.on_dismiss.clone())
                .child(anchored.child(surface))
                .into_any_element(),
            _ => anchored.child(surface).into_any_element(),
        };

        // Deferred painting is what lifts the overlay out of its parent's
        // stacking context; the token layer decides the order among overlays.
        pinned(
            gpui::deferred(placed)
                .priority(priority(&theme, self.layer))
                .into_any_element(),
        )
    }
}

/// A surface at one elevation, sized to its content.
pub fn surface(theme: &Theme, elevation: Elevation) -> Div {
    div()
        .column()
        .bg(theme.colors.overlay)
        .radius(theme, gpui_kit_theme::Radius::Card)
        .elevation(theme, elevation)
        .overflow_hidden()
        .text_color(theme.colors.text)
}

/// Maps a token layer onto GPUI's deferred paint priority.
pub fn priority(theme: &Theme, layer: Layer) -> usize {
    theme.layer(layer).max(0) as usize
}

fn scrim_frame(
    theme: &Theme,
    viewport: gpui::Size<Pixels>,
    visible: bool,
    on_dismiss: Option<DismissHandler>,
) -> Stateful<Div> {
    let mut frame = div()
        .id("overlay.scrim")
        .occlude()
        .absolute()
        .top_0()
        .left_0()
        .w(viewport.width)
        .h(viewport.height)
        .flex();
    if visible {
        frame = frame.bg(gpui::black().opacity(theme.opacity.scrim));
    }
    if let Some(handler) = on_dismiss {
        frame = frame.on_click(move |_: &ClickEvent, window, cx| handler(window, cx));
    }
    frame
}

/// Anchors the deferred subtree to the window origin without occupying layout
/// space in the parent.
pub(crate) fn pinned(layer: AnyElement) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_0()
        .child(layer)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_paint_in_token_order() {
        let theme = Theme::studio_dark();
        assert!(priority(&theme, Layer::Tooltip) > priority(&theme, Layer::Popover));
        assert!(priority(&theme, Layer::Toast) > priority(&theme, Layer::Modal));
        assert_eq!(priority(&theme, Layer::Content), 0);
    }

    #[test]
    fn a_modal_defaults_to_a_centered_scrimmed_dialog() {
        let overlay = Overlay::modal("confirm");
        assert_eq!(overlay.layer, Layer::Modal);
        assert_eq!(overlay.placement, Placement::Center);
        assert!(overlay.scrim);
    }

    #[test]
    fn placement_decides_which_edge_the_surface_hangs_from() {
        assert_eq!(
            Overlay::new("menu").placement(Placement::Above).anchor(),
            Anchor::BottomLeft
        );
        assert_eq!(
            Overlay::new("menu").placement(Placement::Below).anchor(),
            Anchor::TopLeft
        );
    }
}
