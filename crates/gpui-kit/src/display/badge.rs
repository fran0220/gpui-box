use gpui::{
    App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Theme, TypeScale};

use crate::foundation::{Ident, StyledExt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
    Info,
}

impl Tone {
    pub(crate) fn color(self, theme: &Theme) -> Hsla {
        match self {
            Self::Neutral => theme.colors.text_faint,
            Self::Accent => theme.colors.accent,
            Self::Success => theme.colors.success,
            Self::Warning => theme.colors.warning,
            Self::Danger => theme.colors.danger,
            Self::Info => theme.colors.info,
        }
    }
}

/// A compact status label.
///
/// A badge only carries a semantic node when the caller gives it an id, so
/// decorative badges do not add noise to assertion snapshots.
#[derive(Debug, IntoElement)]
pub struct Badge {
    ident: Option<Ident>,
    label: SharedString,
    tone: Tone,
}

impl Badge {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            ident: None,
            label: label.into(),
            tone: Tone::default(),
        }
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn neutral(self) -> Self {
        self.tone(Tone::Neutral)
    }

    pub fn accent(self) -> Self {
        self.tone(Tone::Accent)
    }

    pub fn success(self) -> Self {
        self.tone(Tone::Success)
    }

    pub fn warning(self) -> Self {
        self.tone(Tone::Warning)
    }

    pub fn danger(self) -> Self {
        self.tone(Tone::Danger)
    }

    pub fn info(self) -> Self {
        self.tone(Tone::Info)
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (foreground, background, border) = match self.tone {
            Tone::Neutral => (
                theme.colors.text_muted,
                theme.colors.raised,
                theme.colors.hairline,
            ),
            tone => {
                let color = tone.color(&theme);
                (color, color.opacity(0.12), color.opacity(0.2))
            }
        };

        div()
            .flex_none()
            .px_token(&theme, gpui_kit_theme::Space::Sm)
            .py(px(2.0))
            .rounded_full()
            .border(px(theme.borders.hairline))
            .border_color(border)
            .bg(background)
            .type_scale(&theme, TypeScale::Caption)
            .text_color(foreground)
            .child(self.label.clone())
            .when_some(self.ident, |element, ident| {
                element.semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Status).text(self.label.clone()),
                )
            })
    }
}
