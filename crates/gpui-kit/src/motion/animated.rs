//! One call that puts a token-backed entrance on any element.
//!
//! The primitives underneath this module are complete, and were almost unused
//! for a reason that had nothing to do with them: reaching for one meant
//! naming an id, borrowing the theme, choosing a specification and writing the
//! closure that applies the progress. Four decisions is enough friction that
//! the honest answer at most call sites was to skip the motion, which is why
//! most of this library arrived instantly and only a handful of components
//! moved at all.
//!
//! [`Animated`] collapses those four decisions into one. The element says
//! which arrival it is making and everything else comes from the token
//! document, so adding motion to a component is a single call and stays as
//! reviewable as the rest of the styling around it.

use gpui::{AnimationElement, AnimationExt, App, ElementId, IntoElement, Styled, px};
use gpui_kit_theme::{ActiveTheme, SpringPreset, Theme};

use super::{MotionSpec, Spring, Stagger, dialog_arrival, entrance, menu, spec::state_change};

/// How an element arrives.
///
/// Each variant is a claim about what the element is, not about how far it
/// should travel: a menu answering a click and a dialog taking the page over
/// arrive differently because they mean different things, and the distances
/// and curves that express that live in the token document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Entrance {
    /// Opacity alone.
    ///
    /// The arrival for anything whose position is load-bearing while it
    /// arrives — a row in a list, a cell in a grid — because a rise is a
    /// layout input and an element that slid into place would publish a
    /// moving box the whole way.
    Fade,
    /// Opacity with a short rise: a block of content replacing what was there.
    #[default]
    Rise,
    /// The opening a menu makes: quick, and downward from its anchor.
    Menu,
    /// The arrival a modal makes, on a spring, so it lands with weight.
    Dialog,
}

impl Entrance {
    fn spec(self, theme: &Theme) -> MotionSpec {
        match self {
            Self::Fade => entrance(theme),
            Self::Rise => entrance(theme),
            Self::Menu => menu(theme),
            Self::Dialog => dialog_arrival(theme),
        }
    }

    /// How far the element travels, in pixels, and in which direction.
    ///
    /// Negative is downward from the settled position, which is what a menu
    /// dropping from its anchor does.
    fn travel(self) -> f32 {
        match self {
            Self::Fade => 0.0,
            Self::Rise => 6.0,
            Self::Menu => -2.0,
            Self::Dialog => 8.0,
        }
    }

    /// The opacity the element starts from.
    ///
    /// A menu starts partly visible because it is answering a click that has
    /// already happened, and a menu that faded up from nothing would read as
    /// slower than the click that asked for it.
    fn opening_opacity(self) -> f32 {
        match self {
            Self::Menu => 0.3,
            _ => 0.0,
        }
    }
}

/// A token-backed entrance in one call.
pub trait Animated: Styled + IntoElement + Sized + 'static {
    /// Runs `entrance` on this element.
    ///
    /// The travel is applied as a relative offset, so the element occupies its
    /// settled box for the whole run and nothing beside it moves. Under
    /// [`gpui::App::reduce_motion`] GPUI finishes the animation immediately,
    /// which lands the element exactly where it was going to be anyway.
    fn animate_in(
        self,
        id: impl Into<ElementId>,
        cx: &App,
        entrance: Entrance,
    ) -> AnimationElement<Self> {
        self.animate_with(id, entrance, entrance.spec(cx.theme()))
    }

    /// The same arrival, delayed into place as one member of a group.
    ///
    /// The wave is capped however long the group is, so a five hundred row
    /// list finishes arriving in the same window an eight row one does.
    fn animate_in_staggered(
        self,
        id: impl Into<ElementId>,
        cx: &App,
        entrance: Entrance,
        index: usize,
        count: usize,
    ) -> AnimationElement<Self> {
        let spec = Stagger::rows(cx.theme()).spec(index, count, entrance.spec(cx.theme()));
        self.animate_with(id, entrance, spec)
    }

    /// The arrival with a specification the caller has already composed —
    /// sequenced after another, delayed, or re-sprung.
    fn animate_with(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        spec: MotionSpec,
    ) -> AnimationElement<Self> {
        let travel = entrance.travel();
        let from = entrance.opening_opacity();
        self.with_animation(id, spec.animation(), move |element, progress| {
            let element = element.opacity(from + (1.0 - from) * progress);
            if travel == 0.0 {
                element
            } else {
                element.relative().top(px(travel * (1.0 - progress)))
            }
        })
    }

    /// The response an element gives when the value it is showing changes.
    ///
    /// Short and opacity-only, because it is answering something the user has
    /// just done and the element is not going anywhere.
    fn animate_change(self, id: impl Into<ElementId>, cx: &App) -> AnimationElement<Self> {
        self.animate_with(id, Entrance::Fade, state_change(cx.theme()))
    }

    /// The arrival on a named spring rather than along a curve.
    fn animate_sprung(
        self,
        id: impl Into<ElementId>,
        cx: &App,
        entrance: Entrance,
        preset: SpringPreset,
    ) -> AnimationElement<Self> {
        let spec = MotionSpec::sprung(Spring::preset(cx.theme(), preset));
        self.animate_with(id, entrance, spec)
    }
}

impl<T: Styled + IntoElement + Sized + 'static> Animated for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_faded_arrival_never_moves_the_element() {
        assert_eq!(Entrance::Fade.travel(), 0.0);
    }

    #[test]
    fn a_menu_drops_from_its_anchor_and_the_rest_rise_to_theirs() {
        assert!(Entrance::Menu.travel() < 0.0);
        assert!(Entrance::Rise.travel() > 0.0);
        assert!(Entrance::Dialog.travel() > 0.0);
    }

    #[test]
    fn a_dialog_arrives_on_a_spring_and_a_menu_on_a_curve() {
        let theme = Theme::studio_dark();
        assert!(Entrance::Dialog.spec(&theme).is_sprung());
        assert!(!Entrance::Menu.spec(&theme).is_sprung());
    }

    #[test]
    fn every_arrival_ends_fully_opaque() {
        let theme = Theme::studio_dark();
        for entrance in [
            Entrance::Fade,
            Entrance::Rise,
            Entrance::Menu,
            Entrance::Dialog,
        ] {
            let spec = entrance.spec(&theme);
            let from = entrance.opening_opacity();
            let settled = from + (1.0 - from) * spec.progress(1.0);
            assert!(
                (settled - 1.0).abs() < f32::EPSILON,
                "{entrance:?} settled at {settled}"
            );
        }
    }

    /// A menu is answering a click that already happened, so it may not start
    /// from nothing; everything else may.
    #[test]
    fn only_a_menu_starts_partly_visible() {
        assert!(Entrance::Menu.opening_opacity() > 0.0);
        for entrance in [Entrance::Fade, Entrance::Rise, Entrance::Dialog] {
            assert_eq!(entrance.opening_opacity(), 0.0);
        }
    }

    #[test]
    fn a_staggered_group_finishes_within_the_row_cap() {
        let theme = Theme::studio_dark();
        let spec = Entrance::Fade.spec(&theme);
        let stagger = Stagger::rows(&theme);
        for count in [2, 8, 50, 500] {
            let last = stagger.spec(count - 1, count, spec);
            assert!(
                last.delay_ms <= super::super::row_stagger_cap(&theme).as_millis() as u64,
                "{count} rows waited {}ms",
                last.delay_ms
            );
        }
    }
}
