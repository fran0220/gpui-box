//! A column-oriented table over caller-owned rows.
//!
//! The table sorts nothing and selects nothing. A click on a sortable header
//! reports the key and the direction that click implies; the table then
//! renders exactly the order and the sort indicator the caller passes back, so
//! a host that refuses to re-sort keeps the order that still holds.
//!
//! # Which cells publish
//!
//! Every rendered row publishes a [`Role::Row`] node. Cells do not: a table of
//! two hundred rows and six columns would bury every other assertion target in
//! twelve hundred nodes that say nothing a row does not already say. A cell
//! publishes a [`Role::Cell`] node only where the caller marks it with
//! [`Cell::published`], and its id is `<row id>.<column key>`.
//!
//! # Why the body is not virtualized
//!
//! `Table` takes materialized rows, so the caller has already built every cell
//! element by the time the table sees them, and an element can be laid out
//! once. Virtualization needs a row it can build on demand, which is what
//! [`crate::data::List`] is for. The table therefore renders every row it is
//! given, under a header that stays put while the body scrolls.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Theme, TypeScale};

use crate::foundation::{Disableable, FocusRing, Ident, Sizable, StyledExt};

type SortHandler = Rc<dyn Fn(SharedString, SortDirection, &mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// Which way a sorted column runs. The table reports a direction and renders
/// whatever order it is handed; it never sorts the rows itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    /// The direction a second click on the same header implies.
    pub fn reversed(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        }
    }
}

/// How wide a column is: a fixed measure, or a share of what is left over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColumnWidth {
    Fixed(f32),
    Flex(f32),
}

/// Where a cell's content sits inside its column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

/// One column. `key` addresses the cells that belong to it and appears in
/// every id the column publishes.
#[derive(Debug, Clone)]
pub struct Column {
    key: SharedString,
    header: SharedString,
    width: ColumnWidth,
    align: Align,
    sortable: bool,
}

impl Column {
    pub fn new(key: impl Into<SharedString>, header: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            header: header.into(),
            width: ColumnWidth::Flex(1.0),
            align: Align::default(),
            sortable: false,
        }
    }

    pub fn width(mut self, width: ColumnWidth) -> Self {
        self.width = width;
        self
    }

    pub fn fixed(self, width: f32) -> Self {
        self.width(ColumnWidth::Fixed(width))
    }

    pub fn flex(self, share: f32) -> Self {
        self.width(ColumnWidth::Flex(share))
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    pub fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }
}

/// One cell's content, and whether it is an assertion target.
pub struct Cell {
    content: AnyElement,
    text: Option<SharedString>,
    published: bool,
}

impl std::fmt::Debug for Cell {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Cell")
            .field("text", &self.text)
            .field("published", &self.published)
            .finish()
    }
}

impl Cell {
    pub fn new(content: impl IntoElement) -> Self {
        Self {
            content: content.into_any_element(),
            text: None,
            published: false,
        }
    }

    /// The name the cell publishes when it is published.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Publishes a [`Role::Cell`] node at `<row id>.<column key>`.
    pub fn published(mut self, published: bool) -> Self {
        self.published = published;
        self
    }
}

impl From<SharedString> for Cell {
    fn from(value: SharedString) -> Self {
        Self {
            content: value.clone().into_any_element(),
            text: Some(value),
            published: false,
        }
    }
}

impl From<&'static str> for Cell {
    fn from(value: &'static str) -> Self {
        SharedString::from(value).into()
    }
}

impl From<String> for Cell {
    fn from(value: String) -> Self {
        SharedString::from(value).into()
    }
}

/// One row, keyed by the identity the row already has.
pub struct Row {
    id: SharedString,
    text: Option<SharedString>,
    disabled: bool,
    cells: Vec<(SharedString, Cell)>,
}

impl std::fmt::Debug for Row {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Row")
            .field("id", &self.id)
            .field("cells", &self.cells.len())
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl Row {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: None,
            disabled: false,
            cells: Vec::new(),
        }
    }

    pub fn cell(mut self, key: impl Into<SharedString>, cell: impl Into<Cell>) -> Self {
        self.cells.push((key.into(), cell.into()));
        self
    }

    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn take(&mut self, key: &SharedString) -> Option<Cell> {
        let position = self.cells.iter().position(|(name, _)| name == key)?;
        Some(self.cells.remove(position).1)
    }
}

/// A table with a header that stays put while the body scrolls.
#[derive(IntoElement)]
pub struct Table {
    ident: Ident,
    columns: Vec<Column>,
    rows: Vec<Row>,
    sort: Option<(SharedString, SortDirection)>,
    selected: Option<SharedString>,
    row_height: Option<f32>,
    visible_rows: Option<usize>,
    size: ControlSize,
    disabled: bool,
    on_sort: Option<SortHandler>,
    on_select: Option<SelectHandler>,
}

impl std::fmt::Debug for Table {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Table")
            .field("ident", &self.ident)
            .field("columns", &self.columns.len())
            .field("rows", &self.rows.len())
            .field("sort", &self.sort)
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl Table {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            columns: Vec::new(),
            rows: Vec::new(),
            sort: None,
            selected: None,
            row_height: None,
            visible_rows: None,
            size: ControlSize::Md,
            disabled: false,
            on_sort: None,
            on_select: None,
        }
    }

    pub fn column(mut self, column: Column) -> Self {
        self.columns.push(column);
        self
    }

    pub fn columns(mut self, columns: impl IntoIterator<Item = Column>) -> Self {
        self.columns.extend(columns);
        self
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = Row>) -> Self {
        self.rows.extend(rows);
        self
    }

    /// The sort the caller applied, which is the only sort the table shows.
    pub fn sort(mut self, sort: Option<(SharedString, SortDirection)>) -> Self {
        self.sort = sort;
        self
    }

    pub fn sorted_by(self, key: impl Into<SharedString>, direction: SortDirection) -> Self {
        self.sort(Some((key.into(), direction)))
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = Some(height);
        self
    }

    /// Caps the body at `rows` rows and scrolls past that, leaving the header
    /// in place.
    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = Some(rows);
        self
    }

    pub fn on_sort(
        mut self,
        handler: impl Fn(SharedString, SortDirection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_sort = Some(Rc::new(handler));
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    fn header(&self, theme: &Theme, height: f32, cx: &mut App) -> AnyElement {
        let mut header = div()
            .row()
            .w_full()
            .h(px(height))
            .px(px(theme.space(Space::Sm)))
            .gap(px(theme.space(Space::Sm)))
            .border_b(px(theme.borders.hairline))
            .border_color(theme.colors.hairline)
            .type_scale(theme, TypeScale::Caption)
            .text_color(theme.colors.text_muted);

        for column in &self.columns {
            let ident = self.ident.child("header").child(column.key.as_ref());
            let direction = self
                .sort
                .as_ref()
                .filter(|(key, _)| key == &column.key)
                .map(|(_, direction)| *direction);
            let actionable = column.sortable && !self.disabled && self.on_sort.is_some();

            let content = div()
                .row()
                .gap(px(theme.space(Space::Xs)))
                .child(column.header.clone())
                .children(direction.map(|direction| {
                    div()
                        .text_color(theme.colors.text)
                        .child(SharedString::from(match direction {
                            SortDirection::Ascending => "↑",
                            SortDirection::Descending => "↓",
                        }))
                }));

            let mut cell = cell_frame(div().id(ident.element_id()), column, theme)
                .when(actionable, |element| {
                    element
                        .cursor_pointer()
                        .tab_index(0)
                        .hover(|style| style.text_color(theme.colors.text))
                        .focus_ring(theme)
                })
                .child(content);

            if let (true, Some(handler)) = (actionable, self.on_sort.clone()) {
                let key = column.key.clone();
                let next = direction.map_or(SortDirection::Ascending, SortDirection::reversed);
                let click = Rc::clone(&handler);
                let clicked = key.clone();
                cell = cell
                    .on_click(move |_, window, cx| click(clicked.clone(), next, window, cx))
                    .on_key_down(move |event, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            handler(key.clone(), next, window, cx);
                            cx.stop_propagation();
                        }
                    });
            }

            let spec = if column.sortable {
                NodeSpec::new(ident.semantic_id(), Role::Button)
                    .parent(self.ident.semantic_id())
                    .text(column.header.clone())
                    .disabled(!actionable)
                    // The direction a header reports is the one it currently
                    // shows, not the one a click would ask for.
                    .value(direction.map_or("unsorted", SortDirection::as_str))
            } else {
                NodeSpec::new(ident.semantic_id(), Role::Cell)
                    .parent(self.ident.semantic_id())
                    .text(column.header.clone())
            };

            header = header.child(cell.semantic_in(cx, spec));
        }

        header.into_any_element()
    }

    fn row_element(
        &self,
        theme: &Theme,
        height: f32,
        mut row: Row,
        last: bool,
        cx: &mut App,
    ) -> AnyElement {
        let ident = self.ident.child(row.id.as_ref());
        let selected = self.selected.as_ref() == Some(&row.id);
        let actionable = !row.disabled && !self.disabled && self.on_select.is_some();

        let mut element = div()
            .id(ident.element_id())
            .row()
            .w_full()
            .h(px(height))
            .px(px(theme.space(Space::Sm)))
            .gap(px(theme.space(Space::Sm)))
            // The last row leans on the table's own edge instead of drawing a
            // second line beside it.
            .when(!last, |element| {
                element
                    .border_b(px(theme.borders.hairline))
                    .border_color(theme.colors.hairline)
            })
            .when(selected, |element| element.bg(theme.colors.selected))
            .when(row.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .when(!selected, |element| {
                        element.hover(|style| style.bg(theme.colors.hover.opacity(0.3)))
                    })
                    .focus_ring(theme)
            });

        for column in &self.columns {
            let cell = row.take(&column.key);
            let published = cell.as_ref().is_some_and(|cell| cell.published);
            let text = cell.as_ref().and_then(|cell| cell.text.clone());
            let frame = cell_frame(div(), column, theme)
                .overflow_hidden()
                .children(cell.map(|cell| cell.content));

            element = element.child(if published {
                let cell_ident = ident.child(column.key.as_ref());
                let mut spec =
                    NodeSpec::new(cell_ident.semantic_id(), Role::Cell).parent(ident.semantic_id());
                if let Some(text) = text {
                    spec = spec.text(text);
                }
                frame.semantic_in(cx, spec)
            } else {
                frame
            });
        }

        if let (true, Some(handler)) = (actionable, self.on_select.clone()) {
            let id = row.id.clone();
            element = element.on_click(move |_, window, cx| handler(id.clone(), window, cx));
        }

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Row)
            .parent(self.ident.semantic_id())
            .selected(selected)
            .disabled(row.disabled || self.disabled);
        if let Some(text) = row.text.clone() {
            spec = spec.text(text);
        }
        element.semantic_in(cx, spec).into_any_element()
    }
}

impl Disableable for Table {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Table {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Table {
    fn render(mut self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let height = self.row_height.unwrap_or(metrics.height);
        let count = self.rows.len();
        let header = self.header(&theme, height, cx);

        let rows = std::mem::take(&mut self.rows);
        let body = div()
            .id(self.ident.child("body").element_id())
            .column()
            .w_full()
            .overflow_y_scroll()
            .when_some(self.visible_rows, |element, rows| {
                element.max_h(px(height * rows as f32))
            })
            .children(
                rows.into_iter()
                    .enumerate()
                    .map(|(index, row)| {
                        self.row_element(&theme, height, row, index + 1 == count, cx)
                    })
                    .collect::<Vec<_>>(),
            );

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .radius(&theme, Radius::Card)
            .hairline(&theme)
            .overflow_hidden()
            .text_size(px(metrics.font_size))
            .child(header)
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Table).value(count.to_string()),
            )
    }
}

fn cell_frame<E: Styled>(element: E, column: &Column, theme: &Theme) -> E {
    let element = match column.width {
        ColumnWidth::Fixed(width) => element.w(px(width)).flex_none(),
        // A zero basis makes a share depend on the column's flex factor alone,
        // so a header cell and the body cell under it always agree on where
        // the column starts.
        ColumnWidth::Flex(share) => element
            .flex_grow(share)
            .flex_shrink(1.0)
            .flex_basis(px(0.0)),
    };
    let element = element.row().h_full().gap(px(theme.space(Space::Xs)));
    match column.align {
        Align::Start => element.justify_start(),
        Align::Center => element.justify_center(),
        Align::End => element.justify_end(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_click_on_a_sorted_header_reverses_it() {
        assert_eq!(
            SortDirection::Ascending.reversed(),
            SortDirection::Descending
        );
        assert_eq!(
            SortDirection::Descending.reversed(),
            SortDirection::Ascending
        );
    }

    #[test]
    fn a_cell_takes_its_name_from_the_string_it_renders() {
        let cell: Cell = "Indexing".into();
        assert_eq!(cell.text.as_deref(), Some("Indexing"));
        assert!(!cell.published);
    }
}
