use gpui::{Div, FontWeight, InteractiveElement, ParentElement, SharedString, Styled, div, px};
use gpui_kit_theme::{Elevation, Radius, Space, Surface, TextTone, Theme, TypeScale};

use super::direction::LayoutDirection;

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
    /// A hairline and no shadow, for a grid of cards. Shadows are drawn per
    /// card and know nothing of each other, so a dozen at close range stack
    /// into a wash that reads as one smudged region instead of twelve things.
    Outlined,
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
            CardVariant::Elevated | CardVariant::Outlined => true,
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

    fn hairline(self, theme: &Theme) -> Self {
        self.border(px(theme.borders.hairline))
            .border_color(theme.colors.hairline)
    }

    fn hairline_strong(self, theme: &Theme) -> Self {
        self.border(px(theme.borders.hairline))
            .border_color(theme.colors.hairline_strong)
    }

    /// The shell every card-shaped surface in the library is made of.
    ///
    /// [`Card`](crate::display::card::Card) is the component a caller reaches
    /// for; this is the same shell for a component that already owns a richer
    /// semantic node than a grouping and so cannot be wrapped in one. Both go
    /// through here, because a card that means the same thing and is drawn two
    /// ways is two components wearing one name.
    fn card_surface(self, theme: &Theme, variant: CardVariant) -> Self {
        let element = self.radius(theme, Radius::Card);
        match variant {
            CardVariant::Elevated => element.frame(theme, Surface::Panel, Elevation::Raised),
            CardVariant::Outlined => element.surface(theme, Surface::Panel).hairline(theme),
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
    /// The border it still carries is transparent and exists only to hold the
    /// space that an invalid state will colour, so becoming invalid never
    /// reflows the row the field is in.
    fn well(self, theme: &Theme) -> Self {
        self.surface(theme, Surface::Sunken)
            .border(px(theme.borders.hairline))
            .border_color(gpui::transparent_black())
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

/// The one line this library draws between two pieces of content.
///
/// A rule divides content that shares a surface; it is not an outline around
/// the surface, which is what a colour step and an elevation are for. It is a
/// child element rather than a border so it can be inset from the edges of
/// what holds it, and so a component that already spends its border on focus
/// or invalidity can still draw one.
pub fn rule(theme: &Theme) -> Div {
    div()
        .w_full()
        .h(px(theme.borders.hairline))
        .flex_none()
        .bg(theme.colors.divider)
}

/// The same rule, standing up.
pub fn rule_vertical(theme: &Theme) -> Div {
    div()
        .h_full()
        .w(px(theme.borders.hairline))
        .flex_none()
        .bg(theme.colors.divider)
}

/// The bar that marks the selected row of a collection.
///
/// Absolutely positioned, so it consumes no layout and a row does not reflow
/// when the selection arrives on it. It is rounded on its outer end only,
/// which is what keeps it reading as a mark against the edge rather than as a
/// floating capsule.
pub fn selection_rail(theme: &Theme, direction: LayoutDirection) -> Div {
    let width = px(theme.effects.selection_rail_width);
    let radius = width / 2.0;
    let bar = div()
        .absolute()
        .top_0()
        .bottom_0()
        .w(width)
        .flex_none()
        .bg(theme.colors.accent);
    if direction.is_rtl() {
        bar.right_0().rounded_bl(radius).rounded_tl(radius)
    } else {
        bar.left_0().rounded_br(radius).rounded_tr(radius)
    }
}

/// How every collection in this library says which row it is on.
///
/// One recipe, because selection that is drawn a dozen ways is a dozen
/// different statements wearing one name. A neutral wash carries the row and
/// an accent rail at the reading edge names it: the wash alone cannot be made
/// strong enough in a light theme to read as *chosen* without also reading as
/// *inactive*, and a whole row of accent would spend on one line the area
/// this library reserves for the decision a surface is asking for.
pub trait SelectedRow: Styled + ParentElement + Sized {
    fn selected_row(self, theme: &Theme, direction: LayoutDirection, selected: bool) -> Self {
        if !selected {
            return self;
        }
        self.relative()
            .bg(theme.colors.selected)
            .child(selection_rail(theme, direction))
    }

    /// The same statement for a row that reads across, such as a tab in a
    /// strip or a step in a wizard: the rail lies along the bottom edge.
    fn selected_column(self, theme: &Theme, selected: bool) -> Self {
        if !selected {
            return self;
        }
        let width = px(theme.effects.selection_rail_width);
        self.relative().child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(width)
                .flex_none()
                .rounded_t(width / 2.0)
                .bg(theme.colors.accent),
        )
    }
}

impl<T: Styled + ParentElement + Sized> SelectedRow for T {}

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
        self.focus_visible(|style| {
            style
                .border_color(theme.colors.focus)
                .shadow(theme.focus_ring())
        })
    }
}

impl<T: InteractiveElement + Sized> FocusRing for T {}
