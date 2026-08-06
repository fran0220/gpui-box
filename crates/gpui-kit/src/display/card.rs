use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface};

use crate::foundation::{Ident, Selectable, StyledExt};

/// A bordered panel that groups related rows or content.
#[derive(IntoElement)]
pub struct Card {
    ident: Option<Ident>,
    children: Vec<AnyElement>,
    padded: bool,
}

impl std::fmt::Debug for Card {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Card")
            .field("ident", &self.ident)
            .field("children", &self.children.len())
            .finish()
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl Card {
    pub fn new() -> Self {
        Self {
            ident: None,
            children: Vec::new(),
            padded: false,
        }
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    /// Adds interior padding. Row-based cards leave this off so dividers can
    /// reach the card edge.
    pub fn padded(mut self, padded: bool) -> Self {
        self.padded = padded;
        self
    }
}

impl ParentElement for Card {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .w_full()
            .radius(&theme, Radius::Card)
            .hairline(&theme)
            .surface(&theme, Surface::Panel)
            .overflow_hidden()
            .column()
            .when(self.padded, |element| element.p_token(&theme, Space::Lg))
            .children(self.children)
            .when_some(self.ident, |element, ident| {
                element.semantic_in(cx, NodeSpec::new(ident.semantic_id(), Role::Group))
            })
    }
}

/// One row inside a [`Card`].
#[derive(IntoElement)]
pub struct ListRow {
    ident: Option<Ident>,
    first: bool,
    selected: bool,
    children: Vec<AnyElement>,
}

impl std::fmt::Debug for ListRow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ListRow")
            .field("ident", &self.ident)
            .field("first", &self.first)
            .field("selected", &self.selected)
            .finish()
    }
}

impl Default for ListRow {
    fn default() -> Self {
        Self::new()
    }
}

impl ListRow {
    pub fn new() -> Self {
        Self {
            ident: None,
            first: false,
            selected: false,
            children: Vec::new(),
        }
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    /// Suppresses the top divider on the first row of a card.
    pub fn first(mut self, first: bool) -> Self {
        self.first = first;
        self
    }
}

impl Selectable for ListRow {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl ParentElement for ListRow {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ListRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let selected = self.selected;
        div()
            .w_full()
            .px(px(theme.spacing.lg + theme.spacing.xs))
            .py(px(theme.spacing.md + 2.0))
            .when(!self.first, |element| {
                element
                    .border_t(px(theme.borders.hairline))
                    .border_color(theme.colors.hairline)
            })
            .when(selected, |element| element.bg(theme.colors.selected))
            .when(!selected, |element| {
                element.hover(|style| style.bg(theme.colors.hover.opacity(0.3)))
            })
            .row()
            .gap(px(theme.spacing.md + 2.0))
            .children(self.children)
            .when_some(self.ident, |element, ident| {
                element.semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Row).selected(selected),
                )
            })
    }
}
