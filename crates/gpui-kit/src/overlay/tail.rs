//! The mark that says which thing a floating surface came out of.
//!
//! A tooltip, a hover card and a popover are all rectangles of panel colour
//! on top of a page of panel colour, and a rectangle on its own says only
//! that it is there. The tail says what it belongs to, which is the whole
//! reason a reader can tell a tooltip from a button drawn beside one.
//!
//! It is a painted path rather than a rotated square because GPUI rotates
//! sprites and not layout boxes, and a staircase of one-pixel rows is a
//! triangle only until somebody looks at it.

use gpui::{Hsla, IntoElement, ParentElement, Styled, canvas, div, px};
use gpui_kit_theme::{Space, Theme};

/// Which way the point faces, which is the side of the surface it hangs off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailSide {
    /// Above the surface, pointing up at the trigger over it.
    Up,
    /// Below the surface, pointing down at the trigger under it.
    Down,
}

/// How wide the tail is at the surface it grows out of.
fn width(theme: &Theme) -> f32 {
    theme.space(Space::Md)
}

/// How far it reaches from that edge.
fn reach(theme: &Theme) -> f32 {
    theme.space(Space::Sm)
}

/// A tail in `color`, sized from the spacing scale so it stays in proportion
/// to the surface's own padding at every density.
pub fn tail(theme: &Theme, side: TailSide, color: Hsla) -> impl IntoElement {
    let element = div().flex_none().w(px(width(theme))).h(px(reach(theme)));
    element.child(
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let center = bounds.center();
                let mut builder = gpui::PathBuilder::fill();
                match side {
                    TailSide::Up => {
                        builder.move_to(gpui::point(center.x, bounds.top()));
                        builder.line_to(gpui::point(bounds.right(), bounds.bottom()));
                        builder.line_to(gpui::point(bounds.left(), bounds.bottom()));
                    }
                    TailSide::Down => {
                        builder.move_to(gpui::point(center.x, bounds.bottom()));
                        builder.line_to(gpui::point(bounds.right(), bounds.top()));
                        builder.line_to(gpui::point(bounds.left(), bounds.top()));
                    }
                }
                builder.close();
                if let Ok(path) = builder.build() {
                    window.paint_path(path, color);
                }
            },
        )
        .size_full(),
    )
}

/// How far in from the surface's leading edge the tail sits.
///
/// Far enough in that the point is not on the corner radius, and no further,
/// so the tail still reads as coming from the edge the trigger is on.
pub fn inset(theme: &Theme) -> f32 {
    theme.space(Space::Md)
}
