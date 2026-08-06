//! The two responses a control gives the pointer.
//!
//! Both are painted, never laid out: they move an element by a relative inset
//! and change its shadow, so a control that is pressed or hovered occupies
//! exactly the space it did before and nothing beside it shifts. Both are
//! nothing at all under [`gpui::App::reduce_motion`].

use gpui::{App, InteractiveElement, StatefulInteractiveElement, Styled, px};
use gpui_kit_theme::{ActiveTheme, Elevation};

/// The response a control gives while the pointer is held on it.
pub trait Pressable: StatefulInteractiveElement + Styled + Sized {
    /// Sinks the control by `motion.pressOffsetPx` while it is held.
    ///
    /// This is a downward shift, not a scale: the GPUI revision this library
    /// pins offers a transform on `svg` alone, so there is no way to scale a
    /// div and pretending otherwise would mean redrawing the control at a
    /// different size, which is a layout change wearing a costume. A press is
    /// therefore reported the way a physical key reports one — it goes down.
    fn pressable(self, cx: &App) -> Self {
        if cx.reduce_motion() {
            return self;
        }
        let sink = px(cx.theme().motion.press_offset);
        self.active(|style| style.top(sink))
    }
}

impl<T: StatefulInteractiveElement + Styled + Sized> Pressable for T {}

/// The response a surface gives while the pointer is over it.
pub trait HoverLift: InteractiveElement + Styled + Sized {
    /// Raises the surface by `motion.hoverLiftPx` onto the raised elevation
    /// shadow while it is hovered.
    ///
    /// The shadow does the work and the pixel of travel only sells it, so a
    /// theme that casts no shadow at all still gets a legible response.
    fn hover_lift(self, cx: &App) -> Self {
        if cx.reduce_motion() {
            return self;
        }
        let theme = cx.theme();
        let lift = px(-theme.motion.hover_lift);
        let shadow = theme.shadow(Elevation::Raised).to_vec();
        self.hover(move |style| style.top(lift).shadow(shadow.clone()))
    }
}

impl<T: InteractiveElement + Styled + Sized> HoverLift for T {}

#[cfg(test)]
mod tests {
    use gpui_kit_theme::Theme;

    /// A response that travelled far enough to be measured would be a layout
    /// change, and this library does not move a control because a pointer
    /// arrived over it.
    #[test]
    fn the_pointer_responses_stay_within_a_hairline() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            assert!(theme.motion.press_offset > 0.0);
            assert!(theme.motion.press_offset <= theme.borders.thick);
            assert!(theme.motion.hover_lift > 0.0);
            assert!(theme.motion.hover_lift <= theme.borders.thick);
        }
    }
}
