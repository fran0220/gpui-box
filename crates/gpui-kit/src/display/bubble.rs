//! A neutral, caller-owned message bubble.
//!
//! `Bubble` supplies placement and surface treatment only. It does not own a
//! conversation model, author identity, timestamps, or message actions; the
//! caller provides those as content and optional action elements.

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface};

use crate::foundation::{Ident, StyledExt};

/// Which reading edge a bubble is placed against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BubblePlacement {
    #[default]
    Start,
    End,
}

/// A token-backed message surface with caller-owned content.
#[derive(IntoElement)]
pub struct Bubble {
    ident: Ident,
    label: SharedString,
    content: Option<AnyElement>,
    actions: Vec<AnyElement>,
    placement: BubblePlacement,
    grouped: bool,
    max_width: Option<f32>,
}

impl std::fmt::Debug for Bubble {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Bubble")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("actions", &self.actions.len())
            .field("placement", &self.placement)
            .field("grouped", &self.grouped)
            .finish()
    }
}

impl Bubble {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            content: None,
            actions: Vec::new(),
            placement: BubblePlacement::default(),
            grouped: false,
            max_width: None,
        }
    }

    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    pub fn actions(mut self, actions: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.actions = actions
            .into_iter()
            .map(IntoElement::into_any_element)
            .collect();
        self
    }

    pub fn placement(mut self, placement: BubblePlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn grouped(mut self, grouped: bool) -> Self {
        self.grouped = grouped;
        self
    }

    /// Overrides the maximum width when the caller has a measured layout
    /// constraint. The regular page width remains the token-backed default.
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = width.is_finite().then_some(width.max(0.0));
        self
    }
}

impl RenderOnce for Bubble {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let bubble_id = self.ident.child("surface");
        let max_width = self
            .max_width
            .unwrap_or(theme.measures.readable_width)
            .max(theme.control.xs.height);
        let mut surface = div()
            .id(bubble_id.element_id())
            .column()
            .gap(px(theme.space(Space::Sm)))
            .p(px(theme.space(Space::Md)))
            .max_w(px(max_width))
            .surface(&theme, Surface::Panel)
            .radius(
                &theme,
                if self.grouped {
                    Radius::Control
                } else {
                    Radius::Bubble
                },
            )
            .children(self.content)
            .when(!self.actions.is_empty(), |element| {
                element.child(
                    div()
                        .row()
                        .items_center()
                        .gap(px(theme.space(Space::Xs)))
                        .children(self.actions),
                )
            })
            .semantic_in(
                cx,
                NodeSpec::new(bubble_id.semantic_id(), Role::Group)
                    .parent(self.ident.semantic_id())
                    .text(self.label.clone()),
            );
        surface = match self.placement {
            BubblePlacement::Start => surface.self_start(),
            BubblePlacement::End => surface.self_end(),
        };
        div()
            .id(self.ident.element_id())
            .w_full()
            .child(surface)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group).text(self.label),
            )
    }
}
