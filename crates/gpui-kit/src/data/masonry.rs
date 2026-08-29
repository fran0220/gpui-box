//! A variable-height, token-spaced masonry surface.
//!
//! Items are assigned to the shortest measured column. The item height is
//! supplied by the caller after measuring its content; this keeps placement
//! deterministic and avoids a second layout engine inside the component.
//! Semantic children retain their item identities rather than their column
//! positions.

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface};

use crate::foundation::{Ident, StyledExt};
use crate::layout::{Breakpoint, GridColumns, measure};
use crate::strings::ActiveNumbers;

/// A measured masonry item.
pub struct MasonryItem {
    pub id: SharedString,
    pub content: AnyElement,
    pub height: f32,
}

impl std::fmt::Debug for MasonryItem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MasonryItem")
            .field("id", &self.id)
            .field("height", &self.height)
            .finish()
    }
}

impl MasonryItem {
    pub fn new(id: impl Into<SharedString>, content: impl IntoElement, height: f32) -> Self {
        Self {
            id: id.into(),
            content: content.into_any_element(),
            height: height.max(0.0),
        }
    }
}

/// A measured masonry layout over caller-owned content.
#[derive(IntoElement)]
pub struct Masonry {
    ident: Ident,
    items: Vec<MasonryItem>,
    columns: GridColumns,
    gap: Space,
}

impl std::fmt::Debug for Masonry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Masonry")
            .field("ident", &self.ident)
            .field("items", &self.items.len())
            .field("columns", &self.columns)
            .finish()
    }
}

impl Masonry {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            items: Vec::new(),
            columns: GridColumns::new(3),
            gap: Space::Md,
        }
    }

    pub fn items(mut self, items: impl IntoIterator<Item = MasonryItem>) -> Self {
        self.items = items.into_iter().collect();
        self
    }

    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = GridColumns::new(columns as u16);
        self
    }

    /// Sets the column count at a theme-defined measured-container breakpoint.
    pub fn columns_at(mut self, breakpoint: Breakpoint, columns: usize) -> Self {
        self.columns = self.columns.at(breakpoint, columns as u16);
        self
    }

    pub fn gap(mut self, gap: Space) -> Self {
        self.gap = gap;
        self
    }
}

impl RenderOnce for Masonry {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let gap = theme.space(self.gap);
        let item_count = self.items.len();
        let measured = measure::cell(&self.ident.semantic_id(), window, cx);
        let width = {
            let width = f32::from(measured.get().size.width);
            (width > 0.0).then_some(width)
        };
        let columns_count = self.columns.resolve(width, &theme) as usize;
        let mut heights = vec![0.0_f32; columns_count];
        let mut columns: Vec<Vec<(MasonryItem, usize)>> =
            (0..columns_count).map(|_| Vec::new()).collect();
        for item in self.items {
            let column = heights
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| left.total_cmp(right))
                .map(|(column, _)| column)
                .unwrap_or(0);
            heights[column] += item.height + gap;
            columns[column].push((item, column));
        }

        let mut body = div()
            .id(self.ident.child("columns").element_id())
            .row()
            .items_start()
            .gap(px(gap));
        for (column, items) in columns.into_iter().enumerate() {
            let column_id = self.ident.child(format!("column-{column}"));
            let mut column_view = div()
                .id(column_id.element_id())
                .column()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(gap));
            for (item, _) in items {
                let item_id = item.id.clone();
                let item_ident = self.ident.child("item").child(item_id.as_ref());
                column_view = column_view.child(
                    div()
                        .id(item_ident.element_id())
                        .h(px(item.height))
                        .surface(&theme, Surface::Panel)
                        .radius(&theme, Radius::Card)
                        .overflow_hidden()
                        .child(item.content)
                        .semantic_in(
                            cx,
                            NodeSpec::new(item_ident.semantic_id(), Role::Group)
                                .parent(self.ident.semantic_id())
                                .text(item_id),
                        ),
                );
            }
            body = body.child(column_view);
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
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::List)
                    .value(cx.numbers().count(item_count)),
            )
    }
}
