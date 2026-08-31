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

use crate::foundation::{Ident, StyledExt};
use crate::motion;

/// Which fact the surface is reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmptyKind {
    /// The query succeeded and returned nothing.
    #[default]
    Empty,
    /// Nothing has been asked for yet.
    Unstarted,
    /// Asked for, and waiting its turn.
    Queued,
    /// Started, and waiting on something outside the surface.
    Blocked,
    /// Started, and withdrawn before it finished.
    Cancelled,
    /// The host refused, or could not be reached.
    Unavailable,
    /// The attempt failed.
    Failed,
    /// The host refused because the reader is not allowed.
    Unauthorized,
}

impl EmptyKind {
    /// The name the node publishes, so a test asserts the fact rather than
    /// the picture drawn for it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Unstarted => "unstarted",
            Self::Queued => "queued",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
            Self::Unavailable => "unavailable",
            Self::Failed => "failed",
            Self::Unauthorized => "unauthorized",
        }
    }
}

/// A centred explanation with an optional action.
#[derive(IntoElement)]
pub struct EmptyState {
    ident: Ident,
    kind: EmptyKind,
    glyph: Option<Icon>,
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
            glyph: None,
            title: title.into(),
            detail: None,
            action: None,
        }
    }

    pub fn kind(mut self, kind: EmptyKind) -> Self {
        self.kind = kind;
        self
    }

    /// Replaces the generic state mark when the empty surface has a concrete
    /// product-neutral subject, such as a document, image, or folder.
    ///
    /// The state still decides its semantic value and tone; this changes only
    /// the noun the picture names.
    pub fn icon(mut self, glyph: Icon) -> Self {
        self.glyph = Some(glyph);
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
        // Eight facts, eight pictures. Two of these surfaces drawn with one
        // glyph is two different sentences a reader is asked to tell apart by
        // their wording alone, which is how a withdrawn run and an empty list
        // came to look like the same thing.
        let (default_glyph, tint) = match self.kind {
            EmptyKind::Empty => (Icon::Archive, theme.colors.text_faint),
            EmptyKind::Unstarted => (Icon::Document, theme.colors.text_faint),
            EmptyKind::Queued => (Icon::List, theme.colors.text_faint),
            EmptyKind::Blocked => (Icon::Chat, theme.colors.warning),
            EmptyKind::Cancelled => (Icon::Close, theme.colors.text_faint),
            EmptyKind::Unavailable => (Icon::CloseCircle, theme.colors.warning),
            EmptyKind::Failed => (Icon::Danger, theme.colors.danger),
            EmptyKind::Unauthorized => (Icon::Key, theme.colors.warning),
        };
        let glyph = self.glyph.unwrap_or(default_glyph);

        let content = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(theme.space(Space::Sm)))
            .w_full()
            .text_align(gpui::TextAlign::Center)
            .child(
                icon(glyph)
                    .size(px(theme.measures.standalone_icon))
                    .text_color(tint),
            )
            .child(
                div()
                    .text_size(px(theme.typography.body.size))
                    .text_color(theme.colors.text)
                    .child(self.title.clone()),
            )
            .when_some(self.detail.clone(), |element, detail| {
                element.child(
                    div()
                        .max_w(px(theme.measures.readable_width))
                        .text_size(px(theme.typography.caption.size))
                        .text_color(theme.colors.text_muted)
                        .child(detail),
                )
            })
            // The way out of an empty surface is the only control on it, so it
            // is given room and drawn as a control rather than trailing the
            // explanation as its quietest line.
            .children(self.action.map(|action| {
                div()
                    .row()
                    .flex_none()
                    .mt(px(theme.space(Space::Xs)))
                    .child(action)
            }));

        // The rise happens inside the element that publishes the node, so the
        // published box is the settled one and only the pixels travel.
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
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
                    .value(self.kind.name()),
            )
    }
}

/// Which way a rule runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DividerAxis {
    /// Between one group of rows and the next.
    #[default]
    Horizontal,
    /// Between two columns standing side by side.
    Vertical,
}

/// A soft rule between groups.
///
/// A divider is an explicit author request for separation, so it remains a
/// line. It uses the low-alpha divider paint, rounded ends, and an inset by
/// default rather than cutting a hard rule from edge to edge.
#[derive(Debug, IntoElement)]
pub struct Divider {
    ident: Option<Ident>,
    label: Option<SharedString>,
    axis: DividerAxis,
    /// How far the rule stops short of the edges of what holds it.
    inset: Option<Space>,
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
            axis: DividerAxis::default(),
            inset: Some(Space::Sm),
        }
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    /// A caption sitting in the rule, naming what follows. A vertical rule
    /// takes no label: a caption turned on its side is not read, it is
    /// deciphered.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn axis(mut self, axis: DividerAxis) -> Self {
        self.axis = axis;
        self
    }

    /// The rule standing up, for two columns rather than two groups of rows.
    pub fn vertical(self) -> Self {
        self.axis(DividerAxis::Vertical)
    }

    /// Stops the rule short of both ends by one spacing step, for a rule
    /// inside a padded container that should not touch its corners.
    pub fn inset(mut self, inset: Space) -> Self {
        self.inset = Some(inset);
        self
    }
}

impl RenderOnce for Divider {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let vertical = self.axis == DividerAxis::Vertical;
        let weight = px(theme.borders.hairline);
        let color = theme.colors.divider.opacity(theme.opacity.muted);
        let rule = move || {
            let bar = div().flex_1().rounded_full().bg(color);
            if vertical {
                bar.w(weight)
            } else {
                bar.h(weight)
            }
        };
        let spec = self.ident.as_ref().map(|ident| {
            let mut spec = NodeSpec::new(ident.semantic_id(), Role::Separator);
            if let Some(label) = self.label.clone() {
                spec = spec.text(label);
            }
            spec
        });
        let inset = self.inset.map(|inset| px(theme.space(inset)));

        let element = div()
            .flex()
            .items_center()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .gap(px(theme.space(Space::Sm)))
            .map(|element| {
                if vertical {
                    element
                        .flex_col()
                        .h_full()
                        .flex_none()
                        .when_some(inset, |element, inset| element.py(inset))
                } else {
                    element
                        .flex_row()
                        .w_full()
                        .when_some(inset, |element, inset| element.px(inset))
                }
            })
            .child(rule())
            .when(!vertical, |element| {
                element
                    .when_some(self.label.clone(), |element, label| {
                        element.child(
                            div()
                                .flex_none()
                                .text_size(px(theme.typography.caption.size))
                                .text_color(theme.colors.text_faint)
                                .child(label),
                        )
                    })
                    .when(self.label.is_some(), |element| element.child(rule()))
            });
        match spec {
            Some(spec) => element.semantic_in(cx, spec).into_any_element(),
            None => element.into_any_element(),
        }
    }
}
