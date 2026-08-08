//! What to show when there is nothing to show.
//!
//! Empty, unavailable and failed are different facts, and a surface that
//! renders all three the same way tells the typist that their data is gone
//! when the truth may be that nobody asked for it yet.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space};

use crate::foundation::Ident;
use crate::motion;

/// Which fact the surface is reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmptyKind {
    /// The query succeeded and returned nothing.
    #[default]
    Empty,
    /// Nothing has been asked for yet.
    Unstarted,
    /// The host refused, or could not be reached.
    Unavailable,
    /// The attempt failed.
    Failed,
}

/// A centred explanation with an optional action.
#[derive(IntoElement)]
pub struct EmptyState {
    ident: Ident,
    kind: EmptyKind,
    title: SharedString,
    detail: Option<SharedString>,
    action: Option<AnyElement>,
}

impl std::fmt::Debug for EmptyState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EmptyState")
            .field("ident", &self.ident)
            .field("kind", &self.kind)
            .field("title", &self.title)
            .finish()
    }
}

impl EmptyState {
    pub fn new(ident: impl Into<Ident>, title: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            kind: EmptyKind::default(),
            title: title.into(),
            detail: None,
            action: None,
        }
    }

    pub fn kind(mut self, kind: EmptyKind) -> Self {
        self.kind = kind;
        self
    }

    /// Why the surface is empty, in the host's own words. A refusal is shown
    /// as the refusal it is rather than rewritten as an absence of data.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// What the typist can do about it, usually a retry or a first step.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (glyph, tint) = match self.kind {
            EmptyKind::Empty => (Icon::Checklist, theme.colors.text_faint),
            EmptyKind::Unstarted => (Icon::Document, theme.colors.text_faint),
            EmptyKind::Unavailable => (Icon::CloseCircle, theme.colors.warning),
            EmptyKind::Failed => (Icon::Danger, theme.colors.danger),
        };

        let content = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(theme.space(Space::Sm)))
            .w_full()
            .text_align(gpui::TextAlign::Center)
            .child(icon(glyph).size(px(20.0)).text_color(tint))
            .child(
                div()
                    .text_size(px(theme.typography.body.size))
                    .text_color(theme.colors.text)
                    .child(self.title.clone()),
            )
            .when_some(self.detail.clone(), |element, detail| {
                element.child(
                    div()
                        .max_w(px(360.0))
                        .text_size(px(theme.typography.caption.size))
                        .text_color(theme.colors.text_muted)
                        .child(detail),
                )
            })
            .children(self.action);

        // The rise happens inside the element that publishes the node, so the
        // published box is the settled one and only the pixels travel.
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .p(px(theme.space(Space::Lg)))
            .w_full()
            .child(motion::content_in(
                self.ident.child("in").element_id(),
                &theme,
                content,
            ))
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Status)
                    .text(self.title.clone())
                    .value(match self.kind {
                        EmptyKind::Empty => "empty",
                        EmptyKind::Unstarted => "unstarted",
                        EmptyKind::Unavailable => "unavailable",
                        EmptyKind::Failed => "failed",
                    }),
            )
    }
}

/// A horizontal rule between groups.
#[derive(Debug, IntoElement)]
pub struct Divider {
    ident: Option<Ident>,
    label: Option<SharedString>,
}

impl Default for Divider {
    fn default() -> Self {
        Self::new()
    }
}

impl Divider {
    pub fn new() -> Self {
        Self {
            ident: None,
            label: None,
        }
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    /// A caption sitting in the rule, naming what follows.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl RenderOnce for Divider {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rule = || {
            div()
                .h(px(theme.borders.hairline))
                .flex_1()
                .bg(theme.colors.hairline)
        };
        let spec = self.ident.as_ref().map(|ident| {
            let mut spec = NodeSpec::new(ident.semantic_id(), Role::Separator);
            if let Some(label) = self.label.clone() {
                spec = spec.text(label);
            }
            spec
        });

        let element = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .gap(px(theme.space(Space::Sm)))
            .child(rule())
            .when_some(self.label.clone(), |element, label| {
                element.child(
                    div()
                        .flex_none()
                        .text_size(px(theme.typography.caption.size))
                        .text_color(theme.colors.text_faint)
                        .child(label),
                )
            })
            .when(self.label.is_some(), |element| element.child(rule()));
        match spec {
            Some(spec) => element.semantic_in(cx, spec).into_any_element(),
            None => element.into_any_element(),
        }
    }
}
