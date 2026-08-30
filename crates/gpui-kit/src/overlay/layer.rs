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
use gpui_kit_theme::{Elevation, Layer, Radius, Surface, Theme};

use crate::foundation::{ActiveTheme, Ident, StyledExt};

type DismissHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The complete token recipe for an overlay entity.
///
/// Placement and elevation are not enough to infer shape: a dialog and a
/// drawer are both modal, but the dialog is detached while the drawer is a
/// window plane pinned to an edge. These recipes keep radius and elevation
/// together without copying either value into each component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlaySurface {
    radius: Option<Radius>,
    elevation: Elevation,
}

impl OverlaySurface {
    /// A menu, popover, hover card, or toast detached above content.
    pub const FLOATING: Self = Self {
        radius: Some(Radius::Card),
        elevation: Elevation::Overlay,
    };

    /// A centered decision surface under a modal scrim.
    pub const MODAL: Self = Self {
        radius: Some(Radius::Dialog),
        elevation: Elevation::Modal,
    };

    /// A modal plane attached to a window edge, such as a drawer.
    pub const EDGE: Self = Self {
        radius: None,
        elevation: Elevation::Modal,
    };
}

/// Keeps the earlier elevation-addressed call form source-compatible.
///
/// New component code should name one of [`OverlaySurface`]'s entity recipes;
/// an elevation on its own cannot distinguish an edge-attached plane.
impl From<Elevation> for OverlaySurface {
    fn from(elevation: Elevation) -> Self {
        Self {
            radius: Some(if elevation == Elevation::Modal {
                Radius::Dialog
            } else {
                Radius::Card
            }),
            elevation,
        }
    }
}

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

/// Which of its anchor's edges a floating surface hangs from.
///
/// [`Placement`] says whether a menu opens up or down; this says which way it
/// grows across the page. They are separate because a trigger near the
/// trailing edge of a window needs the second answer and not the first: the
/// menu still opens downward, it just has to grow back toward the middle
/// instead of off the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Hang {
    /// Leading edges together, which is where a menu goes unless it cannot.
    #[default]
    Start,
    /// Trailing edges together, so a surface wider than its trigger grows back
    /// across the page rather than off it.
    End,
}

impl Hang {
    /// The edge a surface lines up with when this one leaves the window.
    pub fn opposite(self) -> Self {
        match self {
            Self::Start => Self::End,
            Self::End => Self::Start,
        }
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
    /// How many modal surfaces sit under this one. Added to the token layer
    /// so a nested dialog paints above the one that opened it.
    stack: usize,
    placement: Placement,
    hang: Hang,
    window_snap_margin: Option<Pixels>,
    scrim: bool,
    /// How far through its arrival the surface is. A scrim that snapped off
    /// while the surface it belongs to was still leaving would report the
    /// content behind as reachable a whole exit before it is.
    progress: f32,
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
            .field("hang", &self.hang)
            .field("window_snap_margin", &self.window_snap_margin)
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
            stack: 0,
            placement: Placement::Below,
            hang: Hang::Start,
            window_snap_margin: None,
            scrim: false,
            progress: 1.0,
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

    /// Paints this surface `depth` steps above others on the same token layer.
    pub fn stack(mut self, depth: usize) -> Self {
        self.stack = depth;
        self
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Which of the anchor's edges the surface hangs from.
    ///
    /// The caller has to put the anchor slot on the same edge, which
    /// [`crate::overlay::popover::anchored_slot`] does.
    pub fn hang(mut self, hang: Hang) -> Self {
        self.hang = hang;
        self
    }

    /// Keeps an already side-resolved anchored surface inside the window.
    ///
    /// Choosing above or below remains the caller's policy because only the
    /// caller knows the surface's effective height. This is the final collision
    /// guard for the window edges.
    pub(crate) fn window_snap_margin(mut self, margin: Pixels) -> Self {
        self.window_snap_margin = Some(margin);
        self
    }

    /// Dims and blocks the content behind the overlay.
    pub fn scrim(mut self, scrim: bool) -> Self {
        self.scrim = scrim;
        self
    }

    /// How far through its arrival the surface is, from
    /// [`Presenting::progress`](crate::motion::Presenting::progress).
    ///
    /// Only the scrim reads it: the surface's own appearance is the caller's,
    /// because only the caller knows which element inside the overlay is the
    /// one that should move. What the overlay owns is that the veil behind a
    /// departing surface goes with it. Left unset the overlay is fully
    /// arrived, which is what a caller with no lifecycle of its own means.
    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0.0, 1.0);
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

    /// The corner of the surface that sits on the anchor point.
    ///
    /// Above puts the surface's bottom on the anchor and below puts its top
    /// there; where it hangs from picks which end of that edge is pinned.
    fn anchor(&self) -> Anchor {
        match (self.placement, self.hang) {
            (Placement::Above, Hang::Start) => Anchor::BottomLeft,
            (Placement::Above, Hang::End) => Anchor::BottomRight,
            (_, Hang::Start) => Anchor::TopLeft,
            (_, Hang::End) => Anchor::TopRight,
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

        // Text inside a floating surface is its own document. A drag started
        // in a dialog does not reach the page behind it, and a drag started on
        // the page does not run through a menu that happens to be open over
        // it. The seed is the overlay's own identity, so two open surfaces are
        // also separate from each other.
        let surface = div()
            .id(element_id)
            .occlude()
            .child(gpui::selection_scope(self.ident.as_str(), content))
            .into_any_element();

        // An anchored surface flips to the opposite corner rather than being
        // slid along the edge, so a menu that would leave the viewport still
        // hangs off its anchor instead of covering it.
        let mut anchored = gpui::anchored().anchor(anchor);
        if let Placement::At(position) = self.placement {
            anchored = anchored.position(position);
        }
        if let Some(margin) = self.window_snap_margin {
            anchored = anchored.snap_to_window_with_margin(margin);
        } else if self.placement == Placement::Center {
            anchored = anchored.snap_to_window_with_margin(px(theme.spacing.sm));
        }

        let placed = match self.placement {
            Placement::Center => scrim_frame(
                &theme,
                viewport,
                self.scrim,
                self.progress,
                self.on_dismiss.clone(),
            )
            .items_center()
            .justify_center()
            .child(surface)
            .into_any_element(),
            // The surface keeps its own size along the pinned axis and is
            // left to stretch across the other one, which is what makes a
            // drawer reach both ends of the side it hangs from.
            Placement::Edge(edge) => scrim_frame(
                &theme,
                viewport,
                self.scrim,
                self.progress,
                self.on_dismiss.clone(),
            )
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
            .into_any_element(),
            _ if self.scrim => scrim_frame(
                &theme,
                viewport,
                true,
                self.progress,
                self.on_dismiss.clone(),
            )
            .child(anchored.child(surface))
            .into_any_element(),
            _ => anchored.child(surface).into_any_element(),
        };

        // Deferred painting is what lifts the overlay out of its parent's
        // stacking context; the token layer decides the order among overlays.
        pinned(
            gpui::deferred(placed)
                .priority(priority(&theme, self.layer).saturating_add(self.stack))
                .into_any_element(),
        )
    }
}

/// An overlay entity built from one complete overlay recipe.
pub fn surface(theme: &Theme, recipe: impl Into<OverlaySurface>) -> Div {
    let recipe = recipe.into();
    div()
        .column()
        .when_some(recipe.radius, |element, radius| {
            element.radius(theme, radius)
        })
        .frame(theme, Surface::Overlay, recipe.elevation)
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
    progress: f32,
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
        // The veil colour is the token document's: dark themes carry a cast
        // the page does not have, because black over near-black is invisible.
        frame = frame.bg(theme.colors.scrim.opacity(theme.opacity.scrim * progress));
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
    fn each_overlay_entity_takes_one_complete_token_recipe() {
        assert_eq!(OverlaySurface::FLOATING.radius, Some(Radius::Card));
        assert_eq!(OverlaySurface::FLOATING.elevation, Elevation::Overlay);
        assert_eq!(OverlaySurface::MODAL.radius, Some(Radius::Dialog));
        assert_eq!(OverlaySurface::MODAL.elevation, Elevation::Modal);
        assert_eq!(OverlaySurface::EDGE.radius, None);
        assert_eq!(OverlaySurface::EDGE.elevation, Elevation::Modal);
        assert_eq!(
            OverlaySurface::from(Elevation::Modal),
            OverlaySurface::MODAL
        );
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

    #[test]
    fn where_it_hangs_is_the_other_axis_and_leaves_the_side_alone() {
        // A trigger near the trailing edge still opens downward. What changes
        // is which way the surface grows from there.
        assert_eq!(
            Overlay::new("menu")
                .placement(Placement::Below)
                .hang(Hang::End)
                .anchor(),
            Anchor::TopRight
        );
        assert_eq!(
            Overlay::new("menu")
                .placement(Placement::Above)
                .hang(Hang::End)
                .anchor(),
            Anchor::BottomRight
        );
    }
}
