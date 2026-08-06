use gpui::{FontWeight, Styled, px};
use gpui_kit_theme::{Radius, Space, Surface, TextTone, Theme, TypeScale};

/// Token-addressed styling helpers.
///
/// These exist so component code names a semantic role instead of repeating a
/// literal, which keeps `tokens/*.json` the single authority for values that
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

    fn hairline(self, theme: &Theme) -> Self {
        self.border(px(theme.borders.hairline))
            .border_color(theme.colors.hairline)
    }

    fn hairline_strong(self, theme: &Theme) -> Self {
        self.border(px(theme.borders.hairline))
            .border_color(theme.colors.hairline_strong)
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
