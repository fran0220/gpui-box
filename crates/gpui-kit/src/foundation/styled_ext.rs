use gpui::{Div, FontWeight, InteractiveElement, ParentElement, SharedString, Styled, div, px};
use gpui_kit_theme::{Elevation, Radius, Space, Surface, TextTone, Theme, TypeScale};

/// Creates text with an explicit complete type step and primary text tone.
///
/// This is the only entry point for putting a string directly into the GPUI
/// element tree. A component cannot rely on a host ancestor's font or colour:
/// the same component must keep its metrics when embedded by itself. Callers
/// that need a secondary colour apply [`StyledExt::text_tone`] to the returned
/// element; callers that need edge alignment apply
/// [`DirectionalExt::text_start`](super::DirectionalExt::text_start) or
/// [`DirectionalExt::text_end`](super::DirectionalExt::text_end). The helper
/// deliberately chooses no physical left/right alignment, so it does not turn
/// a right-to-left run back into a left-to-right one.
pub fn text(theme: &Theme, scale: TypeScale, content: impl Into<SharedString>) -> Div {
    div()
        .type_scale(theme, scale)
        .text_tone(theme, TextTone::Primary)
        .child(content.into())
}

/// How a card separates itself from what is behind it.
///
/// The colour step does the separating in every case: a theme that meets the
/// surface separation floor has already made the boundary legible, so what a
/// variant chooses is the second piece of evidence on top of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CardVariant {
    /// A shadow. One card, or a few, on the surface behind them.
    #[default]
    Elevated,
    /// A filled plane with no shadow, for a dense grid of cards whose nearby
    /// elevations would otherwise stack into a wash.
    Filled,
    /// Neither: structure, padding, identity and interaction, without
    /// claiming to be a plane of its own.
    Ghost,
}

impl CardVariant {
    /// Whether this card presents itself as a plane of its own.
    ///
    /// The two responses a surface can give a pointer are not
    /// interchangeable, and which one is honest here follows from this answer
    /// rather than from a caller's preference. Rising off the page says "this
    /// is a plane, and it has come forward"; a card is the one place in the
    /// library where that reads as a response rather than as a component
    /// climbing out of its own frame. A ghost card has no plane to come
    /// forward — the lift would raise a shadow around a transparent rectangle
    /// and move a line of text a pixel to announce it — so what it gives is
    /// the wash any row gives.
    pub const fn claims_a_plane(self) -> bool {
        match self {
            CardVariant::Elevated | CardVariant::Filled => true,
            CardVariant::Ghost => false,
        }
    }
}

/// Token-addressed styling helpers.
///
/// These exist so component code names a semantic role instead of repeating a
/// literal, which keeps `crates/gpui-kit-tokens/tokens/*.json` the single authority for values that
/// occur more than once.
pub trait StyledExt: Styled + Sized {
    fn surface(self, theme: &Theme, surface: Surface) -> Self {
        self.bg(theme.surface(surface))
    }

    fn text_tone(self, theme: &Theme, tone: TextTone) -> Self {
        self.text_color(theme.text_color(tone))
    }

    /// The plane a window's content stands on.
    ///
    /// Every other surface reads against this one: a panel is a region *of*
    /// the page, a card is a thing *on* it, and an overlay floats *above* it.
    /// A page body painted panel collapses that ladder — the first card on it
    /// has no colour step left to take — so a host names the page's plane
    /// exactly once, with this, and the surfaces on it keep their footing.
    fn page(self, theme: &Theme) -> Self {
        self.surface(theme, Surface::Canvas)
            .text_tone(theme, TextTone::Primary)
    }

    /// Monospace text that shows exactly the characters it was handed.
    ///
    /// The family alone is not enough. The bundled mono face carries `liga`
    /// ligatures whose glyphs advance one cell however many characters they
    /// stand for, with the ink hanging left into the cell in front — so a
    /// shaped `--`, `==` or `->` paints over the space before it and the line
    /// finishes a cell short of what it says. In a code view, a diff, a
    /// terminal or a command a reader is being shown, that is not a
    /// typographic preference: the text on screen stops being the text. Off,
    /// one character is one cell again.
    fn mono(self, theme: &Theme) -> Self {
        self.font_family(theme.typography.mono.clone())
            .font_features(gpui::FontFeatures::disable_ligatures())
    }

    /// Applies size, line height and weight from one typographic step.
    fn type_scale(self, theme: &Theme, scale: TypeScale) -> Self {
        let style = theme.type_style(scale);
        self.text_size(px(style.size))
            .line_height(px(style.line_height))
            .font_weight(FontWeight(style.weight))
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
    }

    fn radius(self, theme: &Theme, radius: Radius) -> Self {
        self.rounded(px(theme.radius(radius)))
    }

    fn gap_token(self, theme: &Theme, space: Space) -> Self {
        self.gap(px(theme.space(space)))
    }

    fn p_token(self, theme: &Theme, space: Space) -> Self {
        self.p(px(theme.space(space)))
    }

    fn px_token(self, theme: &Theme, space: Space) -> Self {
        self.px(px(theme.space(space)))
    }

    fn py_token(self, theme: &Theme, space: Space) -> Self {
        self.py(px(theme.space(space)))
    }

    fn mt_token(self, theme: &Theme, space: Space) -> Self {
        self.mt(px(theme.space(space)))
    }

    /// Applies the shadow for an elevation step. Flat applies nothing.
    fn elevation(self, theme: &Theme, level: Elevation) -> Self {
        let shadow = theme.shadow(level);
        if shadow.is_empty() {
            self
        } else {
            self.shadow(shadow.to_vec())
        }
    }

    /// The shell every card-shaped surface in the library is made of.
    ///
    /// [`Card`](crate::display::card::Card) is the component a caller reaches
    /// for; this is the same shell for a component that already owns a richer
    /// semantic node than a grouping and so cannot be wrapped in one. Both go
    /// through here, because a card that means the same thing and is drawn two
    /// ways is two components wearing one name.
    fn card_surface(self, theme: &Theme, variant: CardVariant) -> Self {
        self.card_surface_on(theme, variant, Surface::Canvas)
    }

    /// The same shell for a card standing somewhere other than the page.
    ///
    /// A card is a plane of its own, and the colour step above what holds it
    /// is what says so. The default assumes the page's canvas and paints the
    /// panel step; a host mounting cards inside a region that is already the
    /// panel plane names that ground instead, so the card takes the raised
    /// step and the boundary survives instead of dissolving into its ground.
    fn card_surface_on(self, theme: &Theme, variant: CardVariant, ground: Surface) -> Self {
        let plane = match ground {
            Surface::Panel => Surface::Raised,
            _ => Surface::Panel,
        };
        let element = self.radius(theme, Radius::Card);
        match variant {
            CardVariant::Elevated => element.frame(theme, plane, Elevation::Raised),
            CardVariant::Filled => element.surface(theme, plane),
            CardVariant::Ghost => element,
        }
    }

    /// A surface that is a distinct thing from the one behind it.
    ///
    /// The colour step and the shadow do the separating, which is why a card,
    /// a popover and a dialog carry no line around them: a line drawn around
    /// something already legible is decoration, and this library reserves
    /// lines for what they alone can say — focus, invalidity, a drop target.
    fn frame(self, theme: &Theme, surface: Surface, level: Elevation) -> Self {
        self.surface(theme, surface).elevation(theme, level)
    }

    /// The recess an editable value sits in.
    ///
    /// A field is a well rather than an outlined box, so the resting state of
    /// every editable control in the library is a colour and nothing else.
    /// Focus and invalidity are paint-only halos, so neither state needs a
    /// transparent border to reserve geometry.
    fn well(self, theme: &Theme) -> Self {
        self.surface(theme, Surface::Sunken)
    }

    /// The colour a surface in a named state bleeds into the pixels around it.
    fn glow(self, theme: &Theme, color: gpui::Hsla) -> Self {
        self.shadow(theme.glow(color))
    }

    /// A horizontal flex row, the layout most component frames start from.
    fn row(self) -> Self {
        self.flex().flex_row().items_center()
    }

    fn column(self) -> Self {
        self.flex().flex_col()
    }
}

impl<T: Styled + Sized> StyledExt for T {}

/// The soft, inset line this library draws between two pieces of content.
///
/// A rule divides content that shares a surface; it is not an outline around
/// the surface, which is what a colour step and an elevation are for. It is a
/// child element rather than a border so it stops short of both edges. The
/// low-alpha rounded stroke carries the grouping without becoming a hard rule
/// across the whole plane.
pub fn rule(theme: &Theme) -> Div {
    div()
        .w_full()
        .flex_none()
        .px_token(theme, Space::Sm)
        .py_token(theme, Space::Xs)
        .child(inset_rule(theme).w_full())
}

/// The stroke used by data rows that explicitly opt into alignment rules.
pub(crate) fn inset_rule(theme: &Theme) -> Div {
    div()
        .h(px(theme.borders.hairline))
        .rounded_full()
        .bg(theme.colors.divider.opacity(theme.opacity.muted))
}

/// The one selected treatment shared by rows, tabs, tiles, and controls.
///
/// Selection is a stronger tonal fill and never an outline or an edge rail.
pub trait SelectedFill: Styled + Sized {
    fn selected_fill(self, theme: &Theme, selected: bool) -> Self {
        if !selected {
            return self;
        }
        self.bg(theme.colors.selected)
    }
}

impl<T: Styled + Sized> SelectedFill for T {}

/// The wash a row takes while the pointer is over it.
///
/// Hover is already a low-alpha token, so a component that dims it again is
/// drawing a state nobody can see. This exists so no component has to decide
/// that a second time.
pub trait Hoverable: gpui::InteractiveElement + Sized {
    fn hover_row(self, theme: &Theme) -> Self {
        let hover = theme.colors.hover;
        self.hover(move |style| style.bg(hover))
    }
}

impl<T: gpui::InteractiveElement + Sized> Hoverable for T {}

/// The one focus treatment in the library.
///
/// Every keyboard-reachable element wears the same ring from the same tokens,
/// so "the keyboard is here" looks identical whether it is on a button, a
/// table header, or a tree row. It is a shadow rather than a border so turning
/// focus on never reflows what is around it.
///
/// The ring answers "the keyboard is here", so it is drawn for keyboard focus
/// and not for a pointer that landed on the same element. A person who just
/// clicked a strip knows where they clicked; outlining it tells them nothing
/// and reads as chrome the borderless catalogue does not otherwise draw. An
/// editable control is the case this does not cover, and it does not go
/// through here: a field says where the caret is with [`StyledExt::well`] and
/// its own focused state, which a click must show.
pub trait FocusRing: InteractiveElement + Sized {
    fn focus_ring(self, theme: &Theme) -> Self {
        self.focus_visible(|style| style.shadow(theme.focus_ring()))
    }

    /// The same halo resolved against a control-owned resting fill.
    fn focus_ring_on(self, theme: &Theme, background: gpui::Hsla) -> Self {
        self.focus_visible(|style| style.shadow(theme.focus_ring_on(background)))
    }

    /// The same halo for an element that has no resting fill of its own.
    ///
    /// The halo is a shadow, and a non-inset shadow is painted under the whole
    /// element rather than clipped to the outside of it the way CSS clips one.
    /// A control with a fill covers that interior and the halo reads as a
    /// ring. A transparent row does not, so the identical call that rings a
    /// button washes a row in the focus colour at its full ring alpha — which
    /// is louder than the selected fill and reads as a selection the reader
    /// did not make.
    ///
    /// This lays the ground the halo needs, for exactly as long as the halo is
    /// worn: `ground` is the colour already behind the element, so a focused
    /// row is its own resting colour plus a ring, and nothing moves.
    fn focus_ring_over(self, theme: &Theme, ground: gpui::Hsla) -> Self {
        self.focus_visible(move |style| style.bg(ground).shadow(theme.focus_ring_on(ground)))
    }
}

impl<T: InteractiveElement + Sized> FocusRing for T {}
