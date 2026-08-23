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
            state: ArtifactPreviewState::Ready,
            slots: Slots::default(),
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
                    EmptyState::new(
                        self.ident.child("empty"),
                        cx.strings().text(StringKey::ArtifactEmpty),
                    )
                    .kind(EmptyKind::Empty)
                    .into_any_element()
                }),
                self.state.name().into(),
            ),
            ArtifactPreviewState::Unavailable(reason) => (
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        self.ident.child("unavailable"),
                        cx.strings().text(StringKey::ArtifactUnavailable),
                    )
                    .kind(EmptyKind::Unavailable)
                    .detail(reason.clone())
                    .into_any_element()
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
            ArtifactPreviewState::Ready => (
                div()
                    .type_scale(&theme, TypeScale::Code)
                    .text_color(theme.colors.text)
                    .child(self.body.clone())
                    .into_any_element(),
                self.state.name().into(),
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
