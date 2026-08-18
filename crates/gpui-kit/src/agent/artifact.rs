//! A caller-owned generated artifact, shown as the kind it is.
//!
//! The host supplies the title, the kind, and the body text. This component
//! never fetches, executes, or applies the artifact.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, TypeScale};

use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::loading::PulseLoader;
use crate::foundation::{Ident, StyledExt};
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

/// A titled pane for a generated artifact.
#[derive(IntoElement)]
pub struct ArtifactPreview {
    ident: Ident,
    title: SharedString,
    kind: ArtifactKind,
    body: SharedString,
    state: ArtifactPreviewState,
}

impl ArtifactPreview {
    pub fn new(ident: impl Into<Ident>, title: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            title: title.into(),
            kind: ArtifactKind::Document,
            body: SharedString::default(),
            state: ArtifactPreviewState::Ready,
        }
    }

    pub fn kind(mut self, kind: ArtifactKind) -> Self {
        self.kind = kind;
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

impl RenderOnce for ArtifactPreview {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (inner, value): (AnyElement, SharedString) = match &self.state {
            ArtifactPreviewState::Loading => (
                PulseLoader::new(self.ident.child("loading"))
                    .label(cx.strings().text(StringKey::ArtifactLoading))
                    .into_any_element(),
                "loading".into(),
            ),
            ArtifactPreviewState::Empty => (
                EmptyState::new(
                    self.ident.child("empty"),
                    cx.strings().text(StringKey::ArtifactEmpty),
                )
                .kind(EmptyKind::Empty)
                .into_any_element(),
                "empty".into(),
            ),
            ArtifactPreviewState::Unavailable(reason) => (
                EmptyState::new(
                    self.ident.child("unavailable"),
                    cx.strings().text(StringKey::ArtifactUnavailable),
                )
                .kind(EmptyKind::Unavailable)
                .detail(reason.clone())
                .into_any_element(),
                "unavailable".into(),
            ),
            ArtifactPreviewState::Error(reason) => (
                EmptyState::new(self.ident.child("error"), reason.clone())
                    .kind(EmptyKind::Failed)
                    .into_any_element(),
                "error".into(),
            ),
            ArtifactPreviewState::Ready => (
                div()
                    .type_scale(&theme, TypeScale::Code)
                    .text_color(theme.colors.text)
                    .child(self.body.clone())
                    .into_any_element(),
                "ready".into(),
            ),
        };
        div()
            .column()
            .gap_token(&theme, Space::Sm)
            .p_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .surface(&theme, Surface::Panel)
            .child(
                div()
                    .row()
                    .justify_between()
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
            .child(inner)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.title)
                    .value(value),
            )
    }
}
