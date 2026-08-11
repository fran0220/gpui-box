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

    /// Applies size, line height and weight from one typographic step.
    fn type_scale(self, theme: &Theme, scale: TypeScale) -> Self {
        let style = theme.type_style(scale);
        self.text_size(px(style.size))
            .line_height(px(style.line_height))
            .font_weight(FontWeight(style.weight))
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

/// The one focus treatment in the library.
///
/// Every keyboard-reachable element wears the same ring from the same tokens,
/// so "the keyboard is here" looks identical whether it is on a button, a
/// table header, or a tree row. It is a shadow rather than a border so turning
/// focus on never reflows what is around it.
pub trait FocusRing: InteractiveElement + Sized {
    fn focus_ring(self, theme: &Theme) -> Self {
        self.focus(|style| {
            style
                .border_color(theme.colors.focus)
                .shadow(theme.focus_ring())
        })
    }
}

impl<T: InteractiveElement + Sized> FocusRing for T {}
