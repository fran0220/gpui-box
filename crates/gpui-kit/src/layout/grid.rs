//! Token-backed declarative layout primitives.
//!
//! [`Grid`] keeps the source order of its items while changing columns and
//! spans from the width it was given. [`Container`] provides the shared page
//! widths used by application shells. Both are layout primitives, not visual
//! shells: content, data, and actions remain caller-owned.

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space};

use crate::foundation::{Ident, StyledExt};
use crate::layout::measure;
use crate::strings::ActiveNumbers;

/// A theme-defined container width at which a grid may change arrangement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Breakpoint {
    Small,
    Medium,
    Large,
    ExtraLarge,
}

impl Breakpoint {
    fn width(self, theme: &gpui_kit_theme::Theme) -> f32 {
        match self {
            Self::Small => theme.measures.container_small,
            Self::Medium => theme.measures.container_medium,
            Self::Large => theme.measures.container_large,
            Self::ExtraLarge => theme.measures.container_extra_large,
        }
    }
}

/// Number of columns at the base width and at optional token breakpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridColumns {
    base: u16,
    small: Option<u16>,
    medium: Option<u16>,
    large: Option<u16>,
    extra_large: Option<u16>,
}

impl Default for GridColumns {
    fn default() -> Self {
        Self {
            base: 1,
            small: None,
            medium: None,
            large: None,
            extra_large: None,
        }
    }
}

impl GridColumns {
    /// Starts a responsive column recipe with a base column count.
    pub fn new(base: u16) -> Self {
        Self {
            base: base.max(1),
            ..Self::default()
        }
    }

    /// Sets the column count at a theme-defined container breakpoint.
    pub fn at(mut self, breakpoint: Breakpoint, columns: u16) -> Self {
        let columns = columns.max(1);
        match breakpoint {
            Breakpoint::Small => self.small = Some(columns),
            Breakpoint::Medium => self.medium = Some(columns),
            Breakpoint::Large => self.large = Some(columns),
            Breakpoint::ExtraLarge => self.extra_large = Some(columns),
        }
        self
    }

    pub(crate) fn resolve(self, width: Option<f32>, theme: &gpui_kit_theme::Theme) -> u16 {
        let width = width.unwrap_or(0.0);
        [
            (Breakpoint::ExtraLarge, self.extra_large),
            (Breakpoint::Large, self.large),
            (Breakpoint::Medium, self.medium),
            (Breakpoint::Small, self.small),
        ]
        .into_iter()
        .find_map(|(breakpoint, columns)| columns.filter(|_| width >= breakpoint.width(theme)))
        .unwrap_or(self.base)
        .max(1)
    }
}

/// One item in a declarative [`Grid`].
pub struct GridItem {
    pub id: SharedString,
    pub content: AnyElement,
    span: u16,
    small_span: Option<u16>,
    medium_span: Option<u16>,
    large_span: Option<u16>,
    extra_large_span: Option<u16>,
}

impl std::fmt::Debug for GridItem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GridItem")
            .field("id", &self.id)
            .field("span", &self.span)
            .finish()
    }
}

impl GridItem {
    pub fn new(id: impl Into<SharedString>, content: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            content: content.into_any_element(),
            span: 1,
            small_span: None,
            medium_span: None,
            large_span: None,
            extra_large_span: None,
        }
    }

    pub fn span(mut self, span: u16) -> Self {
        self.span = span.max(1);
        self
    }

    pub fn span_at(mut self, breakpoint: Breakpoint, span: u16) -> Self {
        let span = span.max(1);
        match breakpoint {
            Breakpoint::Small => self.small_span = Some(span),
            Breakpoint::Medium => self.medium_span = Some(span),
            Breakpoint::Large => self.large_span = Some(span),
            Breakpoint::ExtraLarge => self.extra_large_span = Some(span),
        }
        self
    }

    fn resolve_span(&self, width: Option<f32>, theme: &gpui_kit_theme::Theme) -> u16 {
        let width = width.unwrap_or(0.0);
        [
            (Breakpoint::ExtraLarge, self.extra_large_span),
            (Breakpoint::Large, self.large_span),
            (Breakpoint::Medium, self.medium_span),
            (Breakpoint::Small, self.small_span),
        ]
        .into_iter()
        .find_map(|(breakpoint, span)| span.filter(|_| width >= breakpoint.width(theme)))
        .unwrap_or(self.span)
        .max(1)
    }
}

/// A measured, token-guttered grid that retains caller item order.
#[derive(IntoElement)]
pub struct Grid {
    ident: Ident,
    items: Vec<GridItem>,
    columns: GridColumns,
    gap: Space,
}

impl std::fmt::Debug for Grid {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Grid")
            .field("ident", &self.ident)
            .field("items", &self.items.len())
            .field("columns", &self.columns)
            .finish()
    }
}

impl Grid {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            items: Vec::new(),
            columns: GridColumns::default(),
            gap: Space::Md,
        }
    }

    pub fn items(mut self, items: impl IntoIterator<Item = GridItem>) -> Self {
        self.items = items.into_iter().collect();
        self
    }

    pub fn columns(mut self, columns: u16) -> Self {
        self.columns.base = columns.max(1);
        self
    }

    pub fn columns_at(mut self, breakpoint: Breakpoint, columns: u16) -> Self {
        self.columns = self.columns.at(breakpoint, columns);
        self
    }

    pub fn gap(mut self, gap: Space) -> Self {
        self.gap = gap;
        self
    }
}

impl RenderOnce for Grid {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let measured = measure::cell(&self.ident.semantic_id(), window, cx);
        let width = {
            let width = f32::from(measured.get().size.width);
            (width > 0.0).then_some(width)
        };
        let columns = self.columns.resolve(width, &theme);
        let item_count = self.items.len();
        let mut body = div()
            .id(self.ident.child("items").element_id())
            .grid()
            .grid_cols(columns)
            .gap(px(theme.space(self.gap)));
        for item in self.items {
            let item_id = item.id.clone();
            let item_ident = self.ident.child("item").child(item.id.as_ref());
            body = body.child(
                div()
                    .id(item_ident.element_id())
                    .col_span(item.resolve_span(width, &theme).min(columns))
                    .child(item.content)
                    .semantic_in(
                        cx,
                        NodeSpec::new(item_ident.semantic_id(), Role::Group)
                            .parent(self.ident.semantic_id())
                            .text(item_id),
                    ),
            );
        }
        div()
            .on_children_prepainted({
                let measured = measured.clone();
                move |bounds, window, _| {
                    if let Some(bounds) = bounds.first() {
                        measure::record(&measured, *bounds, window);
                    }
                }
            })
            .id(self.ident.element_id())
            .w_full()
            .child(
                body.semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("items").semantic_id(), Role::List)
                        .parent(self.ident.semantic_id()),
                ),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .value(cx.numbers().count(item_count)),
            )
    }
}

/// The max-width and padding recipe used by page-level content.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum ContainerWidth {
    Full,
    #[default]
    Readable,
    Dialog,
    Custom(f32),
}

/// A centered, token-backed page container.
#[derive(IntoElement)]
pub struct Container {
    ident: Ident,
    width: ContainerWidth,
    padding: Space,
    child: Option<AnyElement>,
}

impl Container {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            width: ContainerWidth::default(),
            padding: Space::Md,
            child: None,
        }
    }

    pub fn width(mut self, width: ContainerWidth) -> Self {
        self.width = width;
        self
    }

    pub fn padding(mut self, padding: Space) -> Self {
        self.padding = padding;
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
}

impl RenderOnce for Container {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let max_width = match self.width {
            ContainerWidth::Full => None,
            ContainerWidth::Readable => Some(theme.measures.readable_width),
            ContainerWidth::Dialog => Some(theme.measures.dialog_width),
            ContainerWidth::Custom(width) if width.is_finite() && width > 0.0 => Some(width),
            ContainerWidth::Custom(_) => None,
        };
        let mut content = div()
            .id(self.ident.child("content").element_id())
            .w_full()
            .px(px(theme.space(self.padding)))
            .children(self.child);
        if let Some(max_width) = max_width {
            content = content.max_w(px(max_width));
        }
        div()
            .id(self.ident.element_id())
            .w_full()
            .row()
            .justify_center()
            .child(content)
            .semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::Region))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responsive_columns_switch_at_the_medium_breakpoint() {
        let theme = gpui_kit_theme::Theme::studio_dark();
        let columns = GridColumns::new(1).at(Breakpoint::Medium, 3);

        assert_eq!(
            columns.resolve(Some(theme.measures.container_medium - 1.0), &theme),
            1
        );
        assert_eq!(
            columns.resolve(Some(theme.measures.container_medium), &theme),
            3
        );
    }
}
