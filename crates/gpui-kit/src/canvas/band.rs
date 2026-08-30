//! Named regions of a canvas, in the same coordinates the nodes are in.
//!
//! A canvas usually has structure the positions alone do not carry. Two rows
//! of cards may be one baseline and one scope, or one tenant and another, and
//! a reader looking at the positions can see two rows without being told what
//! either of them is. A band is where the caller says.
//!
//! [`NodeGroup`](super::NodeGroup) is the other half of this and answers a
//! different question: it wraps children the *host* laid out, in the host's
//! own coordinates. A band names a rectangle of the *graph's* world, so it
//! pans and zooms with the cards it encloses and needs no layout from anyone.
//!
//! A band is declarative and never intercepts a pointer. Marquee selection,
//! node dragging and canvas presses all reach through it, because a region
//! label that swallowed the gestures crossing it would take the canvas away
//! from the reader in exchange for a caption.

use gpui::{Bounds, SharedString, point, size};
use gpui_kit_theme::ColorChoice;

use crate::foundation::Ident;

/// A labelled rectangle of canvas world space.
///
/// The rectangle is the caller's claim about the run, in the same coordinates
/// [`NodeGraph::node`](super::NodeGraph::node) places cards in. Nothing here
/// derives it from the cards inside: which nodes belong to a region is a
/// product question, and a band computed from a bounding box would answer it
/// by proximity — which is exactly the claim a reader is looking at the band
/// to check.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphBand {
    pub(crate) ident: Ident,
    pub(crate) label: SharedString,
    pub(crate) bounds: Bounds<f32>,
    pub(crate) color: Option<ColorChoice>,
    pub(crate) selected: bool,
}

impl GraphBand {
    /// Creates a band from a business identity, a name, and a world-space
    /// rectangle.
    pub fn new(
        ident: impl Into<Ident>,
        label: impl Into<SharedString>,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            bounds: Bounds::new(point(x, y), size(width.max(0.0), height.max(0.0))),
            color: None,
            selected: false,
        }
    }

    /// A caller-owned category colour.
    ///
    /// Resolved through the same tier a graph node's category takes, so a
    /// region and the cards that belong to it can be given one colour and
    /// keep it. Without one the band is neutral, which is the honest default:
    /// a region is not a category until somebody says which one.
    pub fn color(mut self, color: impl Into<ColorChoice>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn ident(&self) -> &Ident {
        &self.ident
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    /// The band's rectangle in canvas world coordinates.
    pub fn bounds(&self) -> Bounds<f32> {
        self.bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_band_keeps_the_world_rectangle_and_the_category_it_was_given() {
        let band = GraphBand::new("eval.baseline", "Baseline", -40.0, 20.0, 900.0, 260.0)
            .color("teal")
            .selected(true);
        assert_eq!(band.bounds().origin, point(-40.0, 20.0));
        assert_eq!(band.bounds().size, size(900.0, 260.0));
        assert_eq!(band.color, Some(ColorChoice::Palette("teal".into())));
        assert!(band.selected);
        assert_eq!(band.label(), "Baseline");
    }

    /// A negative extent is not a rectangle drawn backwards, it is a caller
    /// whose arithmetic produced nothing. Drawn as given it would be a band
    /// that reaches back over the region beside it and labels that instead.
    #[test]
    fn a_band_with_no_extent_is_empty_rather_than_inverted() {
        let band = GraphBand::new("eval.scope", "Scope", 0.0, 0.0, -100.0, -20.0);
        assert_eq!(band.bounds().size, size(0.0, 0.0));
    }
}
