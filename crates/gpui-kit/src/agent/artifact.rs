//! A caller-owned generated artifact, shown as the kind it is.
//!
//! The host supplies the title, the kind, and the body text. This component
//! never fetches, executes, or applies the artifact.

use gpui::{
    AnyElement, App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, TypeScale};

use crate::content::code_view::styled_code;
use crate::content::highlight::{self, Language};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::loading::PulseLoader;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Ident, StyledExt};
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

/// What kind of artifact the host is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Code,
    Document,
    Markup,
}

impl ArtifactKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Document => "document",
            Self::Markup => "markup",
        }
    }
}

/// How the artifact was asked for.
#[derive(Debug, Clone, PartialEq)]
pub enum ArtifactPreviewState {
    Loading,
    Ready,
    Empty,
    Unavailable(SharedString),
    Error(SharedString),
}

impl ArtifactPreviewState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Empty => "empty",
            Self::Unavailable(_) => "unavailable",
            Self::Error(_) => "error",
        }
    }
}

impl HasPhase for ArtifactPreviewState {
    fn phase(&self) -> Phase {
        match self {
            Self::Loading => Phase::Loading,
            Self::Ready => Phase::Ready,
            Self::Empty => Phase::Empty,
            Self::Unavailable(_) => Phase::Unavailable,
            Self::Error(_) => Phase::Error,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(reason) | Self::Error(reason) => Some(reason.as_ref()),
            _ => None,
        }
    }
}

/// A titled pane for a generated artifact.
#[derive(IntoElement)]
pub struct ArtifactPreview {
    ident: Ident,
    title: SharedString,
    kind: ArtifactKind,
    body: SharedString,
    language: Option<Language>,
    state: ArtifactPreviewState,
    slots: Slots,
}

impl ArtifactPreview {
    pub fn new(ident: impl Into<Ident>, title: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            title: title.into(),
            kind: ArtifactKind::Document,
            body: SharedString::default(),
            language: None,
            state: ArtifactPreviewState::Ready,
            slots: Slots::default(),
        }
    }

    pub fn kind(mut self, kind: ArtifactKind) -> Self {
        self.kind = kind;
        self
    }

    /// What the body is written in, when the host knows.
    ///
    /// Only what a caller says is read, the same rule a fenced block in
    /// [`crate::prelude::Markdown`] keeps: an artifact whose language nobody
    /// named is set in the same face at the same size and left uncoloured,
    /// rather than coloured against a grammar somebody guessed at.
    pub fn language(mut self, language: Language) -> Self {
        self.language = Some(language);
        self
    }

    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = body.into();
        self
    }

    pub fn state(mut self, state: ArtifactPreviewState) -> Self {
        self.state = state;
        self
    }
}

impl Slotted for ArtifactPreview {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for ArtifactPreview {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (inner, value): (AnyElement, SharedString) = match &self.state {
            ArtifactPreviewState::Loading => (
                self.slots.or_else(slot::LOADING, window, cx, |_, cx| {
                    PulseLoader::new(self.ident.child("loading"))
                        .label(cx.strings().text(StringKey::ArtifactLoading))
                        .into_any_element()
                }),
                self.state.name().into(),
            ),
            ArtifactPreviewState::Empty => (
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    state_surface(
                        self.ident.child("empty"),
                        cx.strings().text(StringKey::ArtifactEmpty),
                        Icon::Archive,
                        theme.colors.text_faint,
                        None,
                        &theme,
                        cx,
                    )
                }),
                self.state.name().into(),
            ),
            ArtifactPreviewState::Unavailable(reason) => (
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    state_surface(
                        self.ident.child("unavailable"),
                        cx.strings().text(StringKey::ArtifactUnavailable),
                        Icon::CloseCircle,
                        theme.colors.warning,
                        Some(reason.clone()),
                        &theme,
                        cx,
                    )
                }),
                self.state.name().into(),
            ),
            ArtifactPreviewState::Error(reason) => (
                self.slots.or_else(slot::FAILED, window, cx, |_, _cx| {
                    EmptyState::new(self.ident.child("error"), reason.clone())
                        .kind(EmptyKind::Failed)
                        .into_any_element()
                }),
                self.state.name().into(),
            ),
            // Code is drawn as code: the same monospaced face, size, leading
            // and syntax colours a fenced block or a diff gets, because an
            // artifact set in the prose face is the one place in the library
            // where code stops looking like code.
            ArtifactPreviewState::Ready if self.kind == ArtifactKind::Code => {
                let spans = self
                    .language
                    .map(|language| highlight::spans(language, &self.body))
                    .unwrap_or_default();
                (
                    div()
                        .w_full()
                        .mono(&theme)
                        .text_size(px(theme.typography.code.size))
                        .line_height(px(theme.typography.code.line_height))
                        .text_color(theme.colors.text)
                        .child(styled_code(&theme, self.body.clone(), &spans))
                        .into_any_element(),
                    self.state.name().into(),
                )
            }
            ArtifactPreviewState::Ready => (
                div()
                    .type_scale(&theme, TypeScale::Code)
                    .text_color(theme.colors.text)
                    .child(self.body.clone())
                    .into_any_element(),
                self.state.name().into(),
            ),
        };
        let ready = matches!(self.state, ArtifactPreviewState::Ready);
        // The body sits in its own well, so the panel is the frame around the
        // artifact and the artifact is the thing inside it. Every state fills
        // that same well, which is how loading comes to occupy the shape of
        // what is loading rather than floating in the card.
        let body = div()
            .w_full()
            .min_h(px(60.0))
            .column()
            .p_token(&theme, Space::Sm)
            .radius(&theme, Radius::Control)
            .well(&theme)
            .when(!ready, |element| element.items_center().justify_center())
            .child(inner);

        div()
            .column()
            .gap_token(&theme, Space::Sm)
            .p_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .child(
                div()
                    .row()
                    .justify_between()
                    .items_center()
                    .pb(px(theme.spacing.xs))
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Subtitle)
                            .child(self.title.clone()),
                    )
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Caption)
                            .text_color(theme.colors.text_muted)
                            .child(SharedString::from(self.kind.name())),
                    ),
            )
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.title)
                    .description(self.state.reason().unwrap_or_default())
                    .value(value),
            )
    }
}

fn state_surface(
    ident: Ident,
    label: SharedString,
    glyph: Icon,
    tint: Hsla,
    detail: Option<SharedString>,
    theme: &gpui_kit_theme::Theme,
    cx: &mut App,
) -> AnyElement {
    div()
        .column()
        .items_center()
        .justify_center()
        .gap_token(theme, Space::Sm)
        .child(
            icon(glyph)
                .size(px(theme.measures.standalone_icon))
                .text_color(tint),
        )
        .children(detail.clone().map(|detail| {
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_muted)
                .child(detail)
        }))
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Status)
                .text(label)
                .description(detail.unwrap_or_default()),
        )
        .into_any_element()
}

#[cfg(test)]
mod artifact_phase_tests {
    use super::*;

    #[test]
    fn error_is_not_unavailable() {
        let error = ArtifactPreviewState::Error("parse failed".into());
        assert_eq!(error.phase(), Phase::Error);
        assert_eq!(error.name(), "error");
        assert_eq!(
            ArtifactPreviewState::Unavailable("denied".into()).phase(),
            Phase::Unavailable
        );
    }
}
