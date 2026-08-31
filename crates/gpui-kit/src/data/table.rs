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
//! # Materialized rows are not virtualized; a row source is
//!
//! [`Table::rows`] takes materialized rows, so the caller has already built
//! every cell element by the time the table sees them, and an element can be
//! laid out once. Virtualization needs a row it can build on demand, and it
//! needs to build it more than once per frame — a `uniform_list` measures one
//! row to learn the height before it builds the range it shows. A vector
//! cannot answer that twice. So the materialized body renders every row it is
//! given, under a header that stays put while the body scrolls.
//!
//! [`Table::rows_from`] is the way in for a collection larger than the
//! viewport: a count and a closure, the same shape [`crate::data::List`] takes.
//! A table built that way lays out only the rows [`Table::visible_rows`]
//! admits, and asks the caller for those and no others. It is offered
//! alongside [`Table::rows`] rather than replacing it, because a table of six
//! settings should not have to be written as a closure over an index.
//!
//! A table has no directional keyboard navigation of its own. Focused rows do
//! answer Enter and Space, but a caller that moves the selection somewhere the
//! viewport has never drawn brings it into view with
//! [`crate::data::reveal_row`], naming the table's body as
//! `<table ident>.body`. A surface that wants the keyboard to walk a
//! collection larger than its viewport wants [`crate::data::DataGrid`], which
//! does that itself.
//!
//! # Table or DataGrid
//!
//! [`crate::data::DataGrid`] is the heavyweight alternative: it takes a render
//! closure instead of rows, so it virtualizes over
//! [`gpui::uniform_list`], and it carries the machinery an administrative
//! surface needs — resizable and reorderable columns, a left-pinned group,
//! three selection modes with a truthful select-all, opened rows with a detail
//! region, and cells that become fields.
//!
//! Reach for `Table` for a settings summary, a short run list, a preview of a
//! result set — anything a reader takes in at a glance. Reach for `DataGrid`
//! when the data set is larger than the viewport, or when the surface needs
//! any of the above. If a surface would work as either, pick `Table`: it is
//! smaller, and a grid's machinery costs something even when nothing uses it.
//!
//! Loading, empty, and a refresh failure are distinct. A failure over rows
//! that are still true keeps those rows and states the refusal above them.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ListSizingBehavior, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px, uniform_list,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, TextTone, Theme, TypeScale,
};

use crate::data::grid::GridLines;
use crate::data::viewport::scroll_handle;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{
    Disableable, FocusRing, Hoverable, Ident, Pressable, SelectedFill, Sizable, StyledExt,
    inset_rule, text,
};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type SortHandler = Rc<dyn Fn(SharedString, SortDirection, &mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type RenderRow = Rc<dyn Fn(usize, &mut Window, &mut App) -> Row>;

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
    pub(crate) content: CellContent,
    pub(crate) text: Option<SharedString>,
    pub(crate) published: bool,
}

pub(crate) enum CellContent {
    Element(AnyElement),
    Plain(SharedString),
}

impl CellContent {
    pub(crate) fn into_element(self, theme: &Theme, disabled: bool) -> AnyElement {
        match self {
            Self::Element(element) => element,
            Self::Plain(value) => text(theme, TypeScale::Body, value)
                .when(disabled, |element| {
                    element.text_tone(theme, TextTone::Disabled)
                })
                .into_any_element(),
        }
    }
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
            content: CellContent::Element(content.into_any_element()),
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
            content: CellContent::Plain(value.clone()),
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

/// A row set the table builds one row at a time.
#[derive(Clone)]
struct RowSource {
    count: usize,
    render_row: RenderRow,
}

/// Everything a row needs that does not come from the row itself.
///
/// A virtualized body builds its rows inside a `'static` closure, which cannot
/// borrow the table, so the few fields a row reads travel into the closure by
/// value and the materialized body reads the same ones.
#[derive(Clone)]
struct Body {
    ident: Ident,
    columns: Vec<Column>,
    selected: Option<SharedString>,
    disabled: bool,
    lines: GridLines,
    on_select: Option<SelectHandler>,
}

/// A table with a header that stays put while the body scrolls.
#[derive(IntoElement)]
pub struct Table {
    ident: Ident,
    columns: Vec<Column>,
    rows: Vec<Row>,
    source: Option<RowSource>,
    sort: Option<(SharedString, SortDirection)>,
    selected: Option<SharedString>,
    row_height: Option<f32>,
    visible_rows: Option<usize>,
    size: ControlSize,
    disabled: bool,
    lines: GridLines,
    loading: bool,
    failure: Option<SharedString>,
    empty: Option<EmptyState>,
    slots: Slots,
    on_sort: Option<SortHandler>,
    on_select: Option<SelectHandler>,
}

impl std::fmt::Debug for Table {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Table")
            .field("ident", &self.ident)
            .field("columns", &self.columns.len())
            .field("rows", &self.count())
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
            source: None,
            sort: None,
            selected: None,
            row_height: None,
            visible_rows: None,
            size: ControlSize::Md,
            disabled: false,
            lines: GridLines::default(),
            loading: false,
            failure: None,
            empty: None,
            slots: Slots::default(),
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

    /// Takes a count and a closure instead of a vector, so the table can build
    /// only the rows its viewport holds.
    ///
    /// Pair it with [`Table::visible_rows`]: without a bounded viewport there
    /// is no window to skip rows outside of, and the table lays out the whole
    /// collection just as a materialized one does. A source supersedes
    /// anything passed to [`Table::rows`].
    pub fn rows_from(
        mut self,
        count: usize,
        render_row: impl Fn(usize, &mut Window, &mut App) -> Row + 'static,
    ) -> Self {
        self.source = Some(RowSource {
            count,
            render_row: Rc::new(render_row),
        });
        self
    }

    fn count(&self) -> usize {
        self.source
            .as_ref()
            .map_or(self.rows.len(), |source| source.count)
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

    /// Row rules. The default is [`GridLines::None`].
    pub fn lines(mut self, lines: GridLines) -> Self {
        self.lines = lines;
        self
    }

    /// A first load with nothing to show yet. A refresh over rows that already
    /// exist is not this; see [`Table::failure`].
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// A refresh that failed. Rows the table already has stay on screen with
    /// the failure stated above them; only a failure with nothing behind it
    /// takes the surface over.
    pub fn failure(mut self, failure: impl Into<SharedString>) -> Self {
        self.failure = Some(failure.into());
        self
    }

    /// What to show when the query succeeded and returned nothing.
    ///
    /// This is the typed form of the [`slot::EMPTY`] slot. A filled slot wins
    /// over it.
    pub fn empty(mut self, empty: EmptyState) -> Self {
        self.empty = Some(empty);
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
        // The header carries no fill of its own. Its labels are a smaller,
        // dimmer step, and a low-alpha inset rule keeps the columns aligned
        // without cutting a hard line across the whole surface.
        let mut header = div()
            .row()
            .relative()
            .w_full()
            .h(px(height))
            .px(px(theme.space(Space::Sm)))
            .gap(px(theme.space(Space::Sm)))
            .child(
                inset_rule(theme)
                    .absolute()
                    .bottom_0()
                    .left(px(theme.space(Space::Sm)))
                    .right(px(theme.space(Space::Sm))),
            );

        for column in &self.columns {
            let ident = self.ident.child("header").child(column.key.as_ref());
            let hover_group = ident.child("hover").semantic_id();
            let direction = self
                .sort
                .as_ref()
                .filter(|(key, _)| key == &column.key)
                .map(|(_, direction)| *direction);
            let actionable = column.sortable && !self.disabled && self.on_sort.is_some();

            let content = div()
                .row()
                .gap(px(theme.space(Space::Xs)))
                .child(
                    text(theme, TypeScale::Caption, column.header.clone())
                        .text_tone(theme, TextTone::Faint)
                        .when(actionable, |element| {
                            element.group_hover(hover_group.clone(), |style| {
                                style.text_color(theme.colors.text)
                            })
                        }),
                )
                .children(direction.map(|direction| {
                    text(
                        theme,
                        TypeScale::Caption,
                        SharedString::from(match direction {
                            SortDirection::Ascending => "↑",
                            SortDirection::Descending => "↓",
                        }),
                    )
                    .text_color(theme.colors.accent)
                }));

            let mut cell = cell_frame(div().id(ident.element_id()), column, theme)
                .group(hover_group)
                .when(actionable, |element| {
                    element
                        .cursor_pointer()
                        .tab_index(0)
                        .pressable(cx)
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

    fn body(&self) -> Body {
        Body {
            ident: self.ident.clone(),
            columns: self.columns.clone(),
            selected: self.selected.clone(),
            disabled: self.disabled,
            lines: self.lines,
            on_select: self.on_select.clone(),
        }
    }
}

impl Body {
    fn row_element(&self, theme: &Theme, height: f32, mut row: Row, cx: &mut App) -> AnyElement {
        let ident = self.ident.child(row.id.as_ref());
        let selected = self.selected.as_ref() == Some(&row.id);
        let actionable = !row.disabled && !self.disabled && self.on_select.is_some();

        let mut element = div()
            .id(ident.element_id())
            .row()
            .relative()
            .w_full()
            .h(px(height))
            .px(px(theme.space(Space::Sm)))
            .gap(px(theme.space(Space::Sm)))
            .when(self.lines == GridLines::Rows, |element| {
                element.child(
                    inset_rule(theme)
                        .absolute()
                        .bottom_0()
                        .left(px(theme.space(Space::Sm)))
                        .right(px(theme.space(Space::Sm))),
                )
            })
            .selected_fill(theme, selected)
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .when(!selected, |element| element.hover_row(theme))
                    .focus_ring(theme)
            });

        for column in &self.columns {
            let cell = row.take(&column.key);
            let published = cell.as_ref().is_some_and(|cell| cell.published);
            let text = cell.as_ref().and_then(|cell| cell.text.clone());
            let frame = cell_frame(div(), column, theme)
                .overflow_hidden()
                .children(cell.map(|cell| cell.content.into_element(theme, row.disabled)));

            let frame = if published {
                let cell_ident = ident.child(column.key.as_ref());
                let mut spec =
                    NodeSpec::new(cell_ident.semantic_id(), Role::Cell).parent(ident.semantic_id());
                if let Some(text) = text {
                    spec = spec.text(text);
                }
                frame.semantic_in(cx, spec).into_any_element()
            } else {
                frame.into_any_element()
            };
            element = element.child(frame);
        }

        if let (true, Some(handler)) = (actionable, self.on_select.clone()) {
            let id = row.id.clone();
            let click = Rc::clone(&handler);
            let clicked = id.clone();
            element = element
                .on_click(move |_, window, cx| click(clicked.clone(), window, cx))
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        handler(id.clone(), window, cx);
                        cx.stop_propagation();
                    }
                });
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
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let height = self.row_height.unwrap_or(metrics.height);
        let count = self.count();
        let extra = self.slots.render(slot::HEADER_EXTRA, window, cx);
        let header = self.header(&theme, height, cx);
        let banner = self.banner(&theme, cx);
        let vacancy = self.empty.take();
        let body = if count == 0 {
            self.vacant(&theme, height, vacancy, window, cx)
        } else {
            let context = self.body();
            match self.source.take() {
                Some(source) => {
                    let ident = self.ident.child("body");
                    let scroll = scroll_handle(&ident, window, cx);
                    let theme = theme.clone();
                    uniform_list(
                        ident.element_id(),
                        count,
                        move |range: Range<usize>, window, cx| {
                            range
                                .map(|index| {
                                    let row = (source.render_row)(index, window, cx);
                                    context.row_element(&theme, height, row, cx)
                                })
                                .collect::<Vec<_>>()
                        },
                    )
                    .track_scroll(&scroll)
                    .w_full()
                    .with_sizing_behavior(if self.visible_rows.is_some() {
                        ListSizingBehavior::Auto
                    } else {
                        ListSizingBehavior::Infer
                    })
                    // A short collection still ends where its last row ends, the
                    // way a materialized body capped by `max_h` does, so a cap is
                    // not a claim about how much data there is.
                    .when_some(self.visible_rows, |element, rows| {
                        element.h(px(height * count.min(rows) as f32))
                    })
                    .into_any_element()
                }
                None => {
                    let rows = std::mem::take(&mut self.rows);
                    div()
                        .id(self.ident.child("body").element_id())
                        .column()
                        .w_full()
                        .overflow_y_scroll()
                        .when_some(self.visible_rows, |element, rows| {
                            element.max_h(px(height * rows as f32))
                        })
                        .children(
                            rows.into_iter()
                                .map(|row| context.row_element(&theme, height, row, cx))
                                .collect::<Vec<_>>(),
                        )
                        .into_any_element()
                }
            }
        };

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .overflow_hidden()
            .children(banner)
            .children(extra)
            .child(header)
            // The body is inset from the card's own corners. A row that ends
            // exactly where the rounded edge begins has its wash and its
            // selection rail cut by the corner, which reads as the collection
            // running out of room rather than ending.
            .child(div().w_full().py_token(&theme, Space::Xs).child(body))
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Table)
                    .value(cx.numbers().count(count)),
            )
    }
}

impl Table {
    /// The refusal or failure shown above rows that are still true.
    fn banner(&self, theme: &Theme, cx: &mut App) -> Option<AnyElement> {
        let failure = self.failure.clone()?;
        if self.count() == 0 {
            return None;
        }
        let ident = self.ident.child("failure");
        Some(
            div()
                .row()
                .w_full()
                .gap_token(theme, Space::Xs)
                .px_token(theme, Space::Sm)
                .py_token(theme, Space::Xs)
                .bg(theme
                    .colors
                    .danger
                    .opacity(theme.effects.semantic_wash_faint_alpha))
                .child(
                    icon(Icon::Danger)
                        .size(px(theme.control.sm.icon_size))
                        .text_color(theme.colors.danger),
                )
                .child(text(theme, TypeScale::Body, failure.clone()))
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .text(failure)
                        .value("stale")
                        .invalid(true),
                )
                .into_any_element(),
        )
    }

    /// What a table with no rows shows, which is never the same thing twice.
    fn vacant(
        &self,
        theme: &Theme,
        row_height: f32,
        vacancy: Option<EmptyState>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        if let Some(replacement) = self
            .slots
            .render(slot::LOADING, window, cx)
            .filter(|_| self.loading)
        {
            return replacement;
        }

        if self.loading {
            let ident = self.ident.child("loading");
            return div()
                .column()
                .w_full()
                .children((0..self.visible_rows.unwrap_or(4)).map(|index| {
                    div()
                        .id(ident.indexed_element_id(index))
                        .w_full()
                        .h(px(row_height))
                        .px_token(theme, Space::Sm)
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .w_full()
                                .h(px(theme.spacing.sm))
                                .radius(theme, Radius::Small)
                                .bg(theme.colors.hover),
                        )
                }))
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .text(cx.strings().text(StringKey::GridLoadingRows))
                        .value("loading")
                        .busy(true),
                )
                .into_any_element();
        }

        if let Some(failure) = self.failure.clone() {
            if let Some(replacement) = self.slots.render(slot::FAILED, window, cx) {
                return replacement;
            }
            return EmptyState::new(
                self.ident.child("empty"),
                cx.strings().text(StringKey::GridLoadFailed),
            )
            .kind(EmptyKind::Failed)
            .detail(failure)
            .into_any_element();
        }

        if let Some(replacement) = self.slots.render(slot::EMPTY, window, cx) {
            return replacement;
        }

        match vacancy {
            Some(empty) => empty.into_any_element(),
            None => EmptyState::new(
                self.ident.child("empty"),
                cx.strings().text(StringKey::GridEmpty),
            )
            .kind(EmptyKind::Empty)
            .into_any_element(),
        }
    }
}

impl Slotted for Table {
    const SLOTS: &'static [&'static str] =
        &[slot::EMPTY, slot::FAILED, slot::LOADING, slot::HEADER_EXTRA];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
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
