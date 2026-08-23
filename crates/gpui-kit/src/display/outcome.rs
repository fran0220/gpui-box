//! The counterpart to [`crate::display::failure_panel::FailurePanel`]:
//! what happened, including when some of it succeeded.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};

use crate::display::badge::Tone;
use crate::foundation::{Ident, StyledExt};
use crate::strings::{ActiveStrings, StringKey};

/// Which of the three outcomes the host is reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeKind {
    Success,
    /// Some work finished and some did not. The counts stay the host's words.
    Partial,
    Failed,
}

impl OutcomeKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Partial => "partial",
            Self::Failed => "failed",
        }
    }

    fn tone(self) -> Tone {
        match self {
            Self::Success => Tone::Success,
            Self::Partial => Tone::Warning,
            Self::Failed => Tone::Danger,
        }
    }

    fn glyph(self) -> Icon {
        match self {
            Self::Success => Icon::Check,
            Self::Partial => Icon::Danger,
            Self::Failed => Icon::Danger,
        }
    }

    fn title_key(self) -> StringKey {
        match self {
            Self::Success => StringKey::OutcomeSuccess,
            Self::Partial => StringKey::OutcomePartial,
            Self::Failed => StringKey::OutcomeFailed,
        }
    }
}

/// A panel that states what finished, including a partial success.
#[derive(IntoElement)]
pub struct OutcomePanel {
    ident: Ident,
    kind: OutcomeKind,
    title: Option<SharedString>,
    detail: Option<SharedString>,
    /// A count the host already worded, such as "47 succeeded, 3 failed".
    count: Option<SharedString>,
    action: Option<AnyElement>,
}

impl std::fmt::Debug for OutcomePanel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OutcomePanel")
            .field("ident", &self.ident)
            .field("kind", &self.kind)
            .finish()
    }
}

impl OutcomePanel {
    pub fn new(ident: impl Into<Ident>, kind: OutcomeKind) -> Self {
        Self {
            ident: ident.into(),
            kind,
            title: None,
            detail: None,
            count: None,
            action: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn count(mut self, count: impl Into<SharedString>) -> Self {
        self.count = Some(count.into());
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl RenderOnce for OutcomePanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let tone = self.kind.tone();
        let color = tone.color(&theme);
        let title = self
            .title
            .clone()
            .unwrap_or_else(|| cx.strings().text(self.kind.title_key()));

        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .p_token(&theme, Space::Lg)
            .radius(&theme, Radius::Card)
            .bg(theme.colors.panel)
            .glow(&theme, color)
            .child(
                div()
                    .row()
                    .gap_token(&theme, Space::Sm)
                    .child(
                        icon(self.kind.glyph())
                            .size(px(theme.control.md.icon_size))
                            .text_color(color),
                    )
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Label)
                            .text_color(theme.colors.text)
                            .child(title.clone()),
                    ),
            )
            .when_some(self.count.clone(), |element, count| {
                element.child(
                    div()
                        .type_scale(&theme, TypeScale::Body)
                        .text_color(theme.colors.text)
                        .child(count),
                )
            })
            .when_some(self.detail.clone(), |element, detail| {
                element.child(
                    div()
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_muted)
                        .child(detail),
                )
            })
            .children(self.action)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(title)
                    .value(self.kind.name()),
            )
    }
}
