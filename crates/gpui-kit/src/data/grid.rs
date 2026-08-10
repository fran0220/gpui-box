//! A virtualized, column-oriented grid over caller-owned rows.
//!
//! [`crate::data::Table`] takes materialized rows, so every cell element
//! already exists by the time the table sees it and every row it is given is
//! laid out. `DataGrid` takes a *render closure* instead — the same shape
//! [`crate::data::List`] uses — so the body can live inside
//! [`gpui::uniform_list`] and only the rows the viewport holds are ever built.
//!
//! Everything the grid shows is the caller's answer. Order, sort, column
//! widths, column order, selection, expansion, and the value in an editing
//! cell are all host state; the grid reports what was operated and renders
//! exactly what it was handed back, so a host that refuses a change keeps
//! showing the state that still holds.
//!
//! # Which nodes publish
//!
//! Only the rows the viewport drew publish nodes. The grid node itself carries
//! the number of rows it was given in `value`, which is how a snapshot stays
//! honest about the difference between twelve thousand rows and the fourteen
//! that are on screen. A cell publishes a [`Role::Cell`] node only where the
//! caller marks it with [`Cell::published`] or declares its column editable.
//!
//! # What this grid does not do
//!
//! **It does not scroll horizontally.** `uniform_list` owns its own scroll
//! offset and lays every row out at the width it is given; a frozen left group
//! under a horizontal scroll needs either two vertically-synchronised uniform
//! lists — and nothing in this GPUI revision keeps two
//! [`gpui::UniformListScrollHandle`]s in step without one writing the other
//! every frame, which is a redraw loop — or a per-row counter-translation that
//! fights the list's own content mask. So [`GridColumn::pinned`] means "this
//! column keeps the left edge, whatever order the caller puts the columns in,
//! and cannot be dragged out of it", not "this column stays while the rest
//! scrolls away". Columns share the grid's width the way `Table`'s do.
//!
//! **It does not measure a column to its content.** A double click on a
//! resize handle reports a fit request and stops: the grid can only measure
//! the rows it drew, and a width fitted to fourteen of twelve thousand rows is
//! a guess wearing a measurement's clothes. The host owns the data and can
//! answer properly.

use std::cell::{Cell as StdCell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext, ClickEvent, Entity, Focusable, Global, InteractiveElement,
    IntoElement, ListSizingBehavior, MouseButton, ParentElement, RenderOnce, ScrollStrategy,
    SharedString, StatefulInteractiveElement, Styled, UniformListScrollHandle, Window, div,
    prelude::FluentBuilder, px, uniform_list,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, Theme, TypeScale,
};

use crate::controls::input::{Cancel, Submit, TextInput};
use crate::data::table::{Align, Cell, ColumnWidth, SortDirection};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt};
use crate::interaction::dnd::{
    self, DragItem, DropAxis, DropIntent, DropPosition, RowTarget, SurfaceDrag,
};
use crate::layout::measure;
use crate::motion::{Flipping, Presence, entrance, flip, state_change};
use crate::strings::{ActiveStrings, StringKey};

/// How wide the grab area of a column edge is. The value occurs only here.
const RESIZE_HANDLE: f32 = 7.0;

/// How wide the leading gutter columns are — the select-all box and the
/// disclosure. Neither holds data, so neither takes part in the column widths
/// the caller owns.
const GUTTER: f32 = 28.0;

/// How much of a row's height the disclosure mark occupies.
const MARK: f32 = 14.0;

type RenderRow = Rc<dyn Fn(usize, &mut Window, &mut App) -> GridRow>;
type RenderDetail = Rc<dyn Fn(SharedString, &mut Window, &mut App) -> AnyElement>;
type SortHandler = Rc<dyn Fn(SharedString, SortDirection, &mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(&SelectionChange, &mut Window, &mut App)>;
type ResizeHandler = Rc<dyn Fn(SharedString, f32, &mut Window, &mut App)>;
type FitHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type ReorderHandler = Rc<dyn Fn(&DropIntent, &mut Window, &mut App)>;
type ExpandHandler = Rc<dyn Fn(SharedString, bool, &mut Window, &mut App)>;
type EditRequestHandler = Rc<dyn Fn(SharedString, SharedString, &mut Window, &mut App)>;
type EditHandler = Rc<dyn Fn(&EditIntent, &mut Window, &mut App)>;

/// One column of a grid.
///
/// `key` addresses the cells that belong to it and appears in every id the
/// column publishes, so it is the column's business identity rather than its
/// place in the header.
#[derive(Debug, Clone)]
pub struct GridColumn {
    key: SharedString,
    header: SharedString,
    width: ColumnWidth,
    min_width: f32,
    align: Align,
    sortable: bool,
    resizable: bool,
    reorderable: bool,
    pinned: bool,
    editable: bool,
}

impl GridColumn {
    pub fn new(key: impl Into<SharedString>, header: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            header: header.into(),
            width: ColumnWidth::Flex(1.0),
            min_width: 48.0,
            align: Align::default(),
            sortable: false,
            resizable: false,
            reorderable: false,
            pinned: false,
            editable: false,
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

    /// The narrowest a resize may report, and the narrowest a flexible column
    /// is allowed to shrink to.
    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width.max(0.0);
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    pub fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Puts a grab handle on the column's trailing edge. The grid reports the
    /// width the pointer asks for and applies nothing.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Lets the header be picked up and put down somewhere else. The order is
    /// the caller's state; a drop reports where the column should go.
    pub fn reorderable(mut self, reorderable: bool) -> Self {
        self.reorderable = reorderable;
        self
    }

    /// Keeps the column at the left edge whatever order the caller declares,
    /// and takes it out of the reorder. See the module documentation for what
    /// pinning does not do here.
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    /// Marks the column's cells as fields a double click or enter opens. The
    /// grid never writes the value; it reports the edit.
    pub fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }

    pub fn key(&self) -> &SharedString {
        &self.key
    }
}

/// One row, built on demand, keyed by the identity the row already has.
pub struct GridRow {
    id: SharedString,
    text: Option<SharedString>,
    disabled: bool,
    cells: Vec<(SharedString, Cell)>,
    hierarchy: Option<HierarchyRow>,
}

#[derive(Debug, Clone)]
pub(crate) struct HierarchyRow {
    pub level: u32,
    pub has_children: bool,
    pub expanded: bool,
    pub parent: Option<SharedString>,
}

impl std::fmt::Debug for GridRow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GridRow")
            .field("id", &self.id)
            .field("cells", &self.cells.len())
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl GridRow {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: None,
            disabled: false,
            cells: Vec::new(),
            hierarchy: None,
        }
    }

    pub fn cell(mut self, key: impl Into<SharedString>, cell: impl Into<Cell>) -> Self {
        self.cells.push((key.into(), cell.into()));
        self
    }

    /// The name the row publishes, for a test or a reader that has only the
    /// tree to go on.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub(crate) fn hierarchy(mut self, hierarchy: HierarchyRow) -> Self {
        self.hierarchy = Some(hierarchy);
        self
    }

    fn take(&mut self, key: &SharedString) -> Option<Cell> {
        let position = self.cells.iter().position(|(name, _)| name == key)?;
        Some(self.cells.remove(position).1)
    }
}

/// How many rows may be selected at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    #[default]
    None,
    Single,
    Multiple,
}

/// What a gesture asked the selection to become.
///
/// The grid applies none of it. Every variant names rows by identity, and the
/// two "all" variants are kept apart on purpose: a header checkbox can only
/// speak for the rows the host has loaded, and claiming otherwise is the
/// oldest lie in this part of the interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionChange {
    /// A plain click: this row and nothing else.
    Replace(SharedString),
    /// A cmd or ctrl click: add this row, or take it away.
    Toggle(SharedString),
    /// A shift click: everything between the row last operated and this one.
    /// The host owns the order, so the host resolves the span.
    Range {
        anchor: SharedString,
        to: SharedString,
    },
    /// Every row the grid was given — which is every row the host has loaded,
    /// and may be fewer than exist.
    Loaded,
    /// Every row that exists, including the ones nobody has loaded. Only ever
    /// reported from a control that says that is what it does.
    Everything,
    Clear,
}

impl SelectionChange {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Replace(_) => "replace",
            Self::Toggle(_) => "toggle",
            Self::Range { .. } => "range",
            Self::Loaded => "loaded",
            Self::Everything => "everything",
            Self::Clear => "clear",
        }
    }
}

/// One opened row: its identity, and where it sits.
///
/// A virtualized body reserves room by counting fixed-height slots, so it has
/// to know where an opened row is before it has drawn it. The caller owns the
/// order and can answer at once; the grid would have to build every row to
/// find out. The index is layout arithmetic and never reaches an id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expanded {
    pub id: SharedString,
    pub index: usize,
}

impl Expanded {
    pub fn new(id: impl Into<SharedString>, index: usize) -> Self {
        Self {
            id: id.into(),
            index,
        }
    }
}

/// How an edit ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditOutcome {
    /// Enter, or tab. The typed value is the caller's to apply.
    Commit,
    /// Escape. Nothing was typed as far as the host is concerned.
    Revert,
}

impl EditOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Revert => "revert",
        }
    }
}

/// One finished edit, as it is reported to the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditIntent {
    pub row: SharedString,
    pub column: SharedString,
    /// What the field held. On a revert this is the value the grid was given,
    /// unchanged.
    pub value: SharedString,
    pub outcome: EditOutcome,
    /// The cell tab asked to open next, when tab is what ended the edit.
    pub next: Option<(SharedString, SharedString)>,
}

/// The cell the caller has opened for editing, and the value it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditingCell {
    pub row: SharedString,
    pub column: SharedString,
    pub value: SharedString,
}

impl EditingCell {
    pub fn new(
        row: impl Into<SharedString>,
        column: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        Self {
            row: row.into(),
            column: column.into(),
            value: value.into(),
        }
    }

    fn target(&self) -> (SharedString, SharedString) {
        (self.row.clone(), self.column.clone())
    }
}

/// Whether a grid draws rules between its rows.
///
/// A grid groups with alignment and with the wash a row takes under the
/// pointer, which is enough at the row heights this library ships. Rules are
/// the answer to one specific problem — a dense grid whose rows are short
/// enough that the eye loses which cell belongs to which record — and they
/// stay off until a caller says that is the problem they have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridLines {
    #[default]
    None,
    Rows,
}

/// A grid that renders only the rows its viewport holds.
#[derive(IntoElement)]
pub struct DataGrid {
    ident: Ident,
    count: usize,
    total: Option<usize>,
    render_row: RenderRow,
    render_detail: Option<RenderDetail>,
    columns: Vec<GridColumn>,
    lines: GridLines,
    sort: Option<(SharedString, SortDirection)>,
    selection_mode: SelectionMode,
    selected: BTreeSet<SharedString>,
    expanded: Vec<Expanded>,
    detail_rows: usize,
    editing: Option<EditingCell>,
    row_height: Option<f32>,
    visible_rows: Option<usize>,
    size: ControlSize,
    disabled: bool,
    loading: bool,
    failure: Option<SharedString>,
    empty: Option<EmptyState>,
    on_sort: Option<SortHandler>,
    on_select: Option<SelectHandler>,
    on_resize: Option<ResizeHandler>,
    on_fit: Option<FitHandler>,
    on_reorder: Option<ReorderHandler>,
    on_expand: Option<ExpandHandler>,
    on_edit_request: Option<EditRequestHandler>,
    on_edit: Option<EditHandler>,
    hierarchy: bool,
}

impl std::fmt::Debug for DataGrid {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DataGrid")
            .field("ident", &self.ident)
            .field("count", &self.count)
            .field("total", &self.total)
            .field("columns", &self.columns.len())
            .field("sort", &self.sort)
            .field("selection_mode", &self.selection_mode)
            .field("selected", &self.selected.len())
            .field("expanded", &self.expanded.len())
            .field("editing", &self.editing)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl DataGrid {
    /// `count` is how many rows the caller has; `render_row` builds one of
    /// them, and is only ever called for a row that is about to be drawn or
    /// about to be reported.
    pub fn new(
        ident: impl Into<Ident>,
        count: usize,
        render_row: impl Fn(usize, &mut Window, &mut App) -> GridRow + 'static,
    ) -> Self {
        Self {
            ident: ident.into(),
            count,
            total: None,
            render_row: Rc::new(render_row),
            render_detail: None,
            columns: Vec::new(),
            lines: GridLines::default(),
            sort: None,
            selection_mode: SelectionMode::None,
            selected: BTreeSet::new(),
            expanded: Vec::new(),
            detail_rows: 2,
            editing: None,
            row_height: None,
            visible_rows: None,
            size: ControlSize::Md,
            disabled: false,
            loading: false,
            failure: None,
            empty: None,
            on_sort: None,
            on_select: None,
            on_resize: None,
            on_fit: None,
            on_reorder: None,
            on_expand: None,
            on_edit_request: None,
            on_edit: None,
            hierarchy: false,
        }
    }

    /// How many rows exist on the host, when the grid has been given only some
    /// of them. Without it the grid assumes it has everything.
    pub fn total(mut self, total: usize) -> Self {
        self.total = Some(total);
        self
    }

    pub fn column(mut self, column: GridColumn) -> Self {
        self.columns.push(column);
        self
    }

    pub fn columns(mut self, columns: impl IntoIterator<Item = GridColumn>) -> Self {
        self.columns.extend(columns);
        self
    }

    /// Draws rules between rows, for a grid dense enough to need them.
    pub fn lines(mut self, lines: GridLines) -> Self {
        self.lines = lines;
        self
    }

    /// The sort the caller applied, which is the only sort the grid shows.
    pub fn sort(mut self, sort: Option<(SharedString, SortDirection)>) -> Self {
        self.sort = sort;
        self
    }

    pub fn sorted_by(self, key: impl Into<SharedString>, direction: SortDirection) -> Self {
        self.sort(Some((key.into(), direction)))
    }

    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// The rows the caller says are selected. The grid renders these and
    /// nothing else, whatever a click reported a moment ago.
    pub fn selected(mut self, ids: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.selected = ids.into_iter().map(Into::into).collect();
        self
    }

    /// The rows the caller has opened, and where each one sits.
    pub fn expanded(mut self, rows: impl IntoIterator<Item = Expanded>) -> Self {
        self.expanded = rows.into_iter().collect();
        self.expanded.sort_by_key(|row| row.index);
        self
    }

    /// How many row heights an opened detail region occupies.
    pub fn detail_rows(mut self, rows: usize) -> Self {
        self.detail_rows = rows.max(1);
        self
    }

    /// Builds the detail region for an opened row. Called only for rows the
    /// caller has opened *and* the viewport is drawing.
    pub fn detail(
        mut self,
        render: impl Fn(SharedString, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.render_detail = Some(Rc::new(render));
        self
    }

    /// The cell the caller has opened for editing, and the value it holds.
    pub fn editing(mut self, cell: Option<EditingCell>) -> Self {
        self.editing = cell;
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = Some(height);
        self
    }

    /// Bounds the viewport to `rows` rows, which is what lets the grid skip
    /// the rows it does not show.
    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = Some(rows);
        self
    }

    /// A first load with nothing to show yet. A refresh over rows that already
    /// exist is not this; see [`DataGrid::failure`].
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// A refresh that failed. Rows the grid already has stay on screen with
    /// the failure stated above them; only a failure with nothing behind it
    /// takes the surface over.
    pub fn failure(mut self, failure: impl Into<SharedString>) -> Self {
        self.failure = Some(failure.into());
        self
    }

    /// What to show when the query succeeded and returned nothing.
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
        handler: impl Fn(&SelectionChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Reports the width a drag on a column edge asked for.
    pub fn on_resize(
        mut self,
        handler: impl Fn(SharedString, f32, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resize = Some(Rc::new(handler));
        self
    }

    /// Reports that a double click on a column edge asked for a width that
    /// fits the content. The grid measures nothing; see the module docs.
    pub fn on_fit(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_fit = Some(Rc::new(handler));
        self
    }

    /// Reports where a dragged column should go. The grid does not move it.
    pub fn on_reorder(
        mut self,
        handler: impl Fn(&DropIntent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reorder = Some(Rc::new(handler));
        self
    }

    /// Reports the disclosure state a row should take.
    pub fn on_expand(
        mut self,
        handler: impl Fn(SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_expand = Some(Rc::new(handler));
        self
    }

    /// Reports that a cell was asked to become a field.
    pub fn on_edit_request(
        mut self,
        handler: impl Fn(SharedString, SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_edit_request = Some(Rc::new(handler));
        self
    }

    /// Reports how an edit ended, and what was in the field when it did.
    pub fn on_edit(
        mut self,
        handler: impl Fn(&EditIntent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_edit = Some(Rc::new(handler));
        self
    }

    /// The number of rows that exist, which is the number loaded unless the
    /// caller said otherwise.
    fn total_rows(&self) -> usize {
        self.total.unwrap_or(self.count).max(self.count)
    }

    /// Pinned columns first, in the order the caller declared them.
    fn ordered_columns(&self) -> Vec<&GridColumn> {
        let mut ordered: Vec<&GridColumn> = self.columns.iter().filter(|c| c.pinned).collect();
        ordered.extend(self.columns.iter().filter(|c| !c.pinned));
        ordered
    }

    fn expanded_indices(&self) -> Vec<usize> {
        self.expanded.iter().map(|row| row.index).collect()
    }

    pub(crate) fn hierarchy_mode(mut self) -> Self {
        self.hierarchy = true;
        self
    }
}

impl Disableable for DataGrid {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for DataGrid {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

// -- slot arithmetic ----------------------------------------------------------

/// What a virtualized slot holds.
///
/// A `uniform_list` gives every slot the same height, so an opened row cannot
/// simply grow. Instead the detail region is painted over the slots that
/// follow the row, and those slots are drawn empty so nothing lands on top of
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    Row(usize),
    /// Space held open beneath an opened row.
    Detail,
}

pub(crate) fn slot_count(count: usize, expanded: &[usize], detail_rows: usize) -> usize {
    count + expanded.iter().filter(|index| **index < count).count() * detail_rows
}

/// Which row, if any, a slot draws. `expanded` must be sorted.
pub(crate) fn slot_at(slot: usize, expanded: &[usize], detail_rows: usize) -> Slot {
    let mut consumed = 0;
    for index in expanded {
        let opened = index + consumed;
        if slot < opened {
            break;
        }
        if slot == opened {
            return Slot::Row(*index);
        }
        if slot <= opened + detail_rows {
            return Slot::Detail;
        }
        consumed += detail_rows;
    }
    Slot::Row(slot - consumed)
}

/// Which slot a row is drawn in. `expanded` must be sorted.
pub(crate) fn slot_of(index: usize, expanded: &[usize], detail_rows: usize) -> usize {
    index + expanded.iter().filter(|opened| **opened < index).count() * detail_rows
}

// -- per-identity state -------------------------------------------------------

/// What a grid remembers between two frames.
///
/// A `RenderOnce` builder is rebuilt every frame and cannot carry anything, so
/// the scroll position, the anchor a shift click measures from, the drag a
/// resize handle started, and the field an editing cell holds all live in an
/// application global keyed by the grid's identity — the same arrangement
/// [`crate::layout::measure`] uses for measurements.
#[derive(Default)]
struct Memory {
    scroll: UniformListScrollHandle,
    /// The row the last plain click or keyboard move landed on, which is what
    /// a shift click measures its span from.
    anchor: RefCell<Option<SharedString>>,
    /// The column whose edge is currently being dragged.
    resizing: RefCell<Option<SharedString>>,
    editor: RefCell<Option<Entity<TextInput>>>,
    /// The cell the field is currently showing, so the grid seeds it once
    /// rather than on every frame.
    edit_target: RefCell<Option<(SharedString, SharedString)>>,
    /// Whether the bulk bar has been shown before, so a bar that exists on the
    /// first frame is already there rather than arriving.
    bulk: RefCell<Option<Presence>>,
}

#[derive(Default)]
struct Memories(RefCell<HashMap<SharedString, Rc<Memory>>>);

impl Global for Memories {}

fn memory(id: &SharedString, cx: &mut App) -> Rc<Memory> {
    if !cx.has_global::<Memories>() {
        cx.set_global(Memories::default());
    }
    let mut memories = cx.global::<Memories>().0.borrow_mut();
    Rc::clone(memories.entry(id.clone()).or_default())
}

/// Which index published which row id on the last frame.
type Drawn = Rc<RefCell<HashMap<usize, SharedString>>>;

// -- rendering ----------------------------------------------------------------

impl RenderOnce for DataGrid {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let row_height = self.row_height.unwrap_or(metrics.height);
        let ident = self.ident.clone();
        let state = memory(&ident.semantic_id(), cx);
        let columns: Vec<GridColumn> = self.ordered_columns().into_iter().cloned().collect();
        let expanded = self.expanded_indices();
        let detail_rows = self.detail_rows;
        let slots = slot_count(self.count, &expanded, detail_rows);
        let drawn: Drawn = Rc::new(RefCell::new(HashMap::new()));
        let reorder = self.reorder(window, cx);
        let editor = self.editor(&state, window, cx);

        let header = self.header(&theme, row_height, &columns, reorder.as_ref(), window, cx);
        let vacancy = self.empty.take();
        let body = self.body(
            &theme,
            row_height,
            &columns,
            &expanded,
            slots,
            &state,
            &drawn,
            editor.clone(),
            vacancy,
            cx,
        );

        let mut frame = div()
            .id(ident.element_id())
            .column()
            .w_full()
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .overflow_hidden()
            .text_size(px(metrics.font_size))
            .children(self.banner(&theme, cx))
            .child(header)
            .child(body);

        frame = self.wire_resize_drag(frame, &state, &columns, cx);
        frame = self.wire_keyboard(frame, &state, &drawn, &expanded);

        frame.semantic_in(
            cx,
            NodeSpec::new(
                ident.semantic_id(),
                if self.hierarchy {
                    Role::TreeGrid
                } else {
                    Role::Table
                },
            )
            .value(self.count.to_string()),
        )
    }
}

impl DataGrid {
    /// The refusal or failure shown above rows that are still true.
    fn banner(&self, theme: &Theme, cx: &mut App) -> Option<AnyElement> {
        let failure = self.failure.clone()?;
        if self.count == 0 {
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
                    .opacity(theme.effects.selected_ring_alpha))
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text)
                .child(
                    icon(Icon::Danger)
                        .size(px(theme.control.sm.icon_size))
                        .text_color(theme.colors.danger),
                )
                .child(failure.clone())
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

    fn reorder(&self, window: &mut Window, cx: &mut App) -> Option<Reorder> {
        if self.disabled || !self.columns.iter().any(|column| column.reorderable) {
            return None;
        }
        let on_drop = self.on_reorder.clone()?;
        let surface = self.ident.child("header").semantic_id();
        let pinned: Vec<SharedString> = self
            .columns
            .iter()
            .filter(|column| column.pinned)
            .map(|column| column.key.clone())
            .collect();
        let own = surface.clone();
        Some(Reorder {
            drag: dnd::surface_drag(&surface, window, cx),
            surface,
            // A pinned column holds the left edge, so nothing may be dropped
            // across it and it may not be picked up.
            accepts: Rc::new(move |item: &DragItem, position: &DropPosition| {
                item.source == own && !pinned.contains(position.anchor())
            }),
            on_drop,
        })
    }

    /// The field an editing cell shows, created once per grid and seeded only
    /// when the cell it belongs to changes.
    fn editor(
        &self,
        state: &Rc<Memory>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Entity<TextInput>> {
        let cell = self.editing.clone()?;
        if self.disabled {
            return None;
        }
        let existing = state.editor.borrow().clone();
        let input = match existing {
            Some(input) => input,
            None => {
                let ident = self.ident.child("edit");
                let input = cx.new(|cx| TextInput::new(ident, window, cx).bare(true));
                *state.editor.borrow_mut() = Some(input.clone());
                input
            }
        };

        let target = cell.target();
        let changed = state.edit_target.borrow().as_ref() != Some(&target);
        if changed {
            *state.edit_target.borrow_mut() = Some(target);
            input.update(cx, |field, cx| {
                field.set_text_quietly(cell.value.clone(), cx);
            });
            let handle = input.focus_handle(cx);
            window.focus(&handle, cx);
        }
        Some(input)
    }
}

/// The question a drop asks before it is allowed to land.
type Accepts = Rc<dyn Fn(&DragItem, &DropPosition) -> bool>;

/// A column edge as the resize drag sees it: where the header was last painted,
/// and the width it may not go below.
type MeasuredEdge = (Rc<StdCell<gpui::Bounds<gpui::Pixels>>>, f32);

/// What a header cell needs to take part in a column reorder.
#[derive(Clone)]
struct Reorder {
    surface: SharedString,
    drag: Option<SurfaceDrag>,
    accepts: Accepts,
    on_drop: ReorderHandler,
}

impl DataGrid {
    #[allow(clippy::too_many_arguments)]
    fn header(
        &self,
        theme: &Theme,
        height: f32,
        columns: &[GridColumn],
        reorder: Option<&Reorder>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let mut header = div()
            .row()
            .w_full()
            .h(px(height))
            .flex_none()
            .px_token(theme, Space::Sm)
            .gap_token(theme, Space::Sm)
            .surface(theme, Surface::Raised)
            .type_scale(theme, TypeScale::Caption)
            .text_color(theme.colors.text_muted)
            .when(self.hierarchy, |header| {
                header.row_reading(cx.layout_direction())
            });

        if let Some(box_element) = self.select_all(theme, cx) {
            header = header.child(box_element);
        }
        if self.on_expand.is_some() {
            header = header.child(div().w(px(GUTTER)).flex_none());
        }

        let pinned = columns.iter().filter(|column| column.pinned).count();
        for (index, column) in columns.iter().enumerate() {
            header =
                header.child(self.header_cell(theme, height, column, index, reorder, window, cx));
            if index + 1 == pinned {
                header = header.child(pinned_edge(theme));
            }
        }

        header.into_any_element()
    }

    /// The box that speaks for the rows the host has loaded, and says so.
    fn select_all(&self, theme: &Theme, cx: &mut App) -> Option<AnyElement> {
        if self.selection_mode != SelectionMode::Multiple {
            return None;
        }
        let ident = self.ident.child("select-all");
        let loaded = self.count;
        let chosen = self.selected.len();
        let total = self.total_rows();
        let all = loaded > 0 && chosen >= loaded;
        let mixed = chosen > 0 && chosen < loaded;
        let actionable = !self.disabled && self.on_select.is_some() && loaded > 0;

        let mark = div()
            .size(px(MARK))
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .radius(theme, Radius::Small)
            .border(px(theme.borders.hairline))
            .border_color(if all || mixed {
                theme.colors.accent
            } else {
                theme.colors.hairline_strong
            })
            .when(all || mixed, |element| element.bg(theme.colors.accent))
            .when(all, |element| {
                element.child(
                    icon(Icon::Check)
                        .size(px(MARK * 0.7))
                        .text_color(theme.colors.text_on_accent),
                )
            })
            .when(mixed, |element| {
                element.child(
                    div()
                        .w(px(MARK * 0.5))
                        .h(px(theme.borders.thick))
                        .bg(theme.colors.text_on_accent),
                )
            });

        let mut element = div()
            .id(ident.element_id())
            .w(px(GUTTER))
            .h_full()
            .flex_none()
            .flex()
            .items_center()
            .when(actionable, |element| {
                element.cursor_pointer().tab_index(0).focus_ring(theme)
            })
            .child(mark);

        if let (true, Some(handler)) = (actionable, self.on_select.clone()) {
            let next = if all {
                SelectionChange::Clear
            } else {
                SelectionChange::Loaded
            };
            element = element.on_click(move |_, window, cx| handler(&next, window, cx));
        }

        Some(
            element
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Checkbox)
                        .parent(self.ident.semantic_id())
                        .text(cx.strings().text(StringKey::GridSelectAllLoaded))
                        .disabled(!actionable)
                        .tristate(if mixed { None } else { Some(all) })
                        // A box in a virtualized grid can only ever speak for
                        // the rows the host handed over, so it publishes both
                        // numbers rather than one that reads like the whole
                        // data set.
                        .value(format!("{chosen} of {loaded} loaded, {total} total")),
                )
                .into_any_element(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn header_cell(
        &self,
        theme: &Theme,
        height: f32,
        column: &GridColumn,
        index: usize,
        reorder: Option<&Reorder>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let ident = self.ident.child("header").child(column.key.as_ref());
        let direction = self
            .sort
            .as_ref()
            .filter(|(key, _)| key == &column.key)
            .map(|(_, direction)| *direction);
        let sortable = column.sortable && !self.disabled && self.on_sort.is_some();
        // A column that cannot be moved can still be landed beside, so every
        // unpinned header offers slots while only a reorderable one is a
        // handle.
        let target = reorder.filter(|_| !column.pinned);
        let draggable = target.filter(|_| column.reorderable);
        let drag = target.and_then(|reorder| reorder.drag.as_ref());
        let carried = drag.is_some_and(|drag| drag.carries(&column.key));
        let landing = drag.and_then(|drag| drag.indicator_for(&column.key));

        // The header cell is measured so a drag on its trailing edge can turn
        // a pointer position into a width.
        let measured = measure::cell(&ident.semantic_id(), cx);

        let content = div()
            .row()
            .overflow_hidden()
            .gap_token(theme, Space::Xs)
            .child(column.header.clone())
            .children(direction.map(|direction| {
                div()
                    .text_color(theme.colors.text)
                    .child(SharedString::from(match direction {
                        SortDirection::Ascending => "↑",
                        SortDirection::Descending => "↓",
                    }))
            }));

        let mut cell = column_frame(div().id(ident.element_id()), column, theme)
            .relative()
            .when(carried, |element| element.opacity(theme.opacity.muted))
            .when(sortable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .hover(|style| style.text_color(theme.colors.text))
                    .focus_ring(theme)
            })
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .on_children_prepainted({
                        let measured = Rc::clone(&measured);
                        move |bounds, window, _| {
                            if let Some(first) = bounds.first() {
                                measure::record(&measured, *first, window);
                            }
                        }
                    })
                    .child(content),
            )
            .children(landing.map(|(position, accepted)| {
                dnd::indicator(&position, accepted, DropAxis::Horizontal, cx)
            }));

        if let (true, Some(handler)) = (sortable, self.on_sort.clone()) {
            let key = column.key.clone();
            let next = direction.map_or(SortDirection::Ascending, SortDirection::reversed);
            let clicked = key.clone();
            let click = Rc::clone(&handler);
            cell = cell
                .on_click(move |_, window, cx| click(clicked.clone(), next, window, cx))
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        handler(key.clone(), next, window, cx);
                        cx.stop_propagation();
                    }
                });
        }

        if let Some(handle) = self.resize_handle(theme, height, column, &measured, cx) {
            cell = cell.child(handle);
        }

        if let Some(reorder) = draggable {
            cell = dnd::draggable(
                cell,
                DragItem::new(
                    reorder.surface.clone(),
                    column.key.clone(),
                    column.header.clone(),
                ),
            );
        }

        if let Some(reorder) = target {
            cell = dnd::drop_target(
                cell,
                RowTarget {
                    surface: reorder.surface.clone(),
                    id: column.key.clone(),
                    index,
                    allow_into: false,
                    axis: DropAxis::Horizontal,
                    accepts: Rc::clone(&reorder.accepts),
                    on_drop: Rc::clone(&reorder.on_drop),
                },
            );
        }

        let spec = if column.sortable {
            NodeSpec::new(ident.semantic_id(), Role::Button)
                .parent(self.ident.semantic_id())
                .text(column.header.clone())
                .disabled(!sortable)
                // A header reports the direction it currently shows, not the
                // one a click would ask for.
                .value(direction.map_or("unsorted", SortDirection::as_str))
        } else {
            NodeSpec::new(ident.semantic_id(), Role::Cell)
                .parent(self.ident.semantic_id())
                .text(column.header.clone())
        };
        let cell = cell.semantic_in(cx, spec);

        // A column that moved slides from where it was; the layout already
        // put it where the caller says it belongs.
        let handle = flip(ident.child("slide").semantic_id(), cx);
        cell.flip(&handle, window, cx).into_any_element()
    }

    /// The grab area on a column's trailing edge.
    fn resize_handle(
        &self,
        theme: &Theme,
        height: f32,
        column: &GridColumn,
        measured: &Rc<StdCell<gpui::Bounds<gpui::Pixels>>>,
        cx: &mut App,
    ) -> Option<AnyElement> {
        if self.disabled || !column.resizable {
            return None;
        }
        let ident = self
            .ident
            .child("header")
            .child(column.key.as_ref())
            .child("resize");
        let state = memory(&self.ident.semantic_id(), cx);

        let mut handle = div()
            .id(ident.element_id())
            .absolute()
            .top_0()
            .right(px(-RESIZE_HANDLE / 2.0))
            .w(px(RESIZE_HANDLE))
            .h(px(height))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(
                div()
                    .w(px(theme.borders.hairline))
                    .h_full()
                    .bg(theme.colors.hairline),
            )
            .hover(|style| style.bg(theme.colors.hover));

        if self.on_resize.is_some() {
            let key = column.key.clone();
            let started = Rc::clone(&state);
            handle = handle.on_mouse_down(MouseButton::Left, move |_, _, _| {
                *started.resizing.borrow_mut() = Some(key.clone());
            });
        }

        if let Some(fit) = self.on_fit.clone() {
            let key = column.key.clone();
            handle = handle.on_click(move |event: &ClickEvent, window, cx| {
                if event.click_count() < 2 {
                    return;
                }
                fit(key.clone(), window, cx);
                cx.stop_propagation();
            });
        }

        let width = f32::from(measured.get().size.width).max(column.min_width);
        Some(
            handle
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Separator)
                        .parent(self.ident.semantic_id())
                        .text(
                            cx.strings()
                                .format(StringKey::GridResizeColumn, &[&column.header]),
                        )
                        .value(format!("{width:.0}")),
                )
                // `semantic_in` makes its host relative so the probe measures
                // it, which would put the handle back in the flow.
                .absolute()
                .into_any_element(),
        )
    }

    /// Follows a resize drag across the whole grid.
    ///
    /// A seven-pixel handle cannot receive the moves of a drag that has left
    /// it, so the handle only records that one started and the frame does the
    /// following — the same arrangement [`crate::layout::SplitPane`] uses.
    fn wire_resize_drag(
        &self,
        frame: gpui::Stateful<gpui::Div>,
        state: &Rc<Memory>,
        columns: &[GridColumn],
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let Some(handler) = self.on_resize.clone().filter(|_| !self.disabled) else {
            return frame;
        };
        let edges: HashMap<SharedString, MeasuredEdge> = columns
            .iter()
            .filter(|column| column.resizable)
            .map(|column| {
                let ident = self.ident.child("header").child(column.key.as_ref());
                (
                    column.key.clone(),
                    (measure::cell(&ident.semantic_id(), cx), column.min_width),
                )
            })
            .collect();
        if edges.is_empty() {
            return frame;
        }

        let held = Rc::clone(state);
        let frame = frame.on_mouse_move(move |event, window, cx| {
            let key = held.resizing.borrow().clone();
            let Some(key) = key else {
                return;
            };
            if event.pressed_button != Some(MouseButton::Left) {
                *held.resizing.borrow_mut() = None;
                return;
            }
            let Some((bounds, min_width)) = edges.get(&key) else {
                return;
            };
            let left = f32::from(bounds.get().left());
            let width = (f32::from(event.position.x) - left).max(*min_width);
            handler(key, width, window, cx);
        });

        let released = Rc::clone(state);
        frame.on_mouse_up(MouseButton::Left, move |_, _, _| {
            *released.resizing.borrow_mut() = None;
        })
    }

    /// Up, down, home and end move the reported selection and scroll the row
    /// they name into view, even when the viewport has never drawn it.
    fn wire_keyboard(
        &self,
        frame: gpui::Stateful<gpui::Div>,
        state: &Rc<Memory>,
        drawn: &Drawn,
        expanded: &[usize],
    ) -> gpui::Stateful<gpui::Div> {
        let Some(handler) = self
            .on_select
            .clone()
            .filter(|_| !self.disabled)
            .filter(|_| self.selection_mode != SelectionMode::None)
            .filter(|_| self.count > 0)
        else {
            return frame;
        };
        let render_row = Rc::clone(&self.render_row);
        let drawn = Rc::clone(drawn);
        let state = Rc::clone(state);
        let count = self.count;
        let expanded = expanded.to_vec();
        let detail_rows = self.detail_rows;
        let selected = self.selected.clone();

        let hierarchy = self.hierarchy;
        let on_expand = self.on_expand.clone();
        frame.on_key_down(move |event, window, cx| {
            // The anchor is read now rather than when the frame was built: a
            // click sets it without the caller having to redraw, and the move
            // that follows should start from the row that was clicked.
            let anchor = state.anchor.borrow().clone();
            let from = current_index(&drawn, anchor.as_ref(), &selected);
            if hierarchy && let Some(index) = from {
                let row = render_row(index, window, cx);
                if let Some(meta) = row.hierarchy {
                    if row.disabled {
                        return;
                    }
                    let logical = cx
                        .layout_direction()
                        .arrow_step(event.keystroke.key.as_str());
                    match logical {
                        Some(-1) if meta.has_children && meta.expanded => {
                            if let Some(expand) = &on_expand {
                                expand(row.id, false, window, cx);
                                cx.stop_propagation();
                            }
                            return;
                        }
                        Some(-1) => {
                            if let Some(parent) = meta.parent {
                                *state.anchor.borrow_mut() = Some(parent.clone());
                                handler(&SelectionChange::Replace(parent), window, cx);
                                cx.stop_propagation();
                            }
                            return;
                        }
                        Some(1) if meta.has_children && !meta.expanded => {
                            if let Some(expand) = &on_expand {
                                expand(row.id, true, window, cx);
                                cx.stop_propagation();
                            }
                            return;
                        }
                        Some(1) if meta.has_children && meta.expanded => {
                            if let Some((_, child)) = reachable(
                                &render_row,
                                index.saturating_add(1),
                                1,
                                count,
                                window,
                                cx,
                            ) {
                                *state.anchor.borrow_mut() = Some(child.clone());
                                handler(&SelectionChange::Replace(child), window, cx);
                                cx.stop_propagation();
                            }
                            return;
                        }
                        Some(1) => return,
                        _ => {}
                    }
                }
            }
            let Some(target) = target_index(event.keystroke.key.as_str(), from, count) else {
                return;
            };
            let step = if from.is_some_and(|from| target < from) {
                -1
            } else {
                1
            };
            let Some((index, id)) = reachable(&render_row, target, step, count, window, cx) else {
                return;
            };
            state.scroll.scroll_to_item(
                slot_of(index, &expanded, detail_rows),
                ScrollStrategy::Nearest,
            );
            window.refresh();
            *state.anchor.borrow_mut() = Some(id.clone());
            handler(&SelectionChange::Replace(id), window, cx);
            cx.stop_propagation();
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn body(
        &self,
        theme: &Theme,
        row_height: f32,
        columns: &[GridColumn],
        expanded: &[usize],
        slots: usize,
        state: &Rc<Memory>,
        drawn: &Drawn,
        editor: Option<Entity<TextInput>>,
        vacancy: Option<EmptyState>,
        cx: &mut App,
    ) -> AnyElement {
        if self.count == 0 {
            return self.vacant(theme, row_height, vacancy, cx);
        }

        let ident = self.ident.clone();
        let theme = theme.clone();
        let columns = columns.to_vec();
        let expanded = expanded.to_vec();
        let detail_rows = self.detail_rows;
        let render_row = Rc::clone(&self.render_row);
        let render_detail = self.render_detail.clone();
        let drawn = Rc::clone(drawn);
        let opened: Vec<SharedString> = self.expanded.iter().map(|row| row.id.clone()).collect();
        let context = Rc::new(RowContext {
            lines: self.lines,
            selected: self.selected.clone(),
            selection_mode: self.selection_mode,
            disabled: self.disabled,
            on_select: self.on_select.clone(),
            on_expand: self.on_expand.clone(),
            on_edit_request: self.on_edit_request.clone(),
            on_edit: self.on_edit.clone(),
            editing: self.editing.clone(),
            editor,
            state: Rc::clone(state),
            hierarchy: self.hierarchy,
        });

        let list = uniform_list(
            ident.child("rows").element_id(),
            slots,
            move |range: Range<usize>, window, cx| {
                drawn.borrow_mut().clear();
                range
                    .map(|slot| match slot_at(slot, &expanded, detail_rows) {
                        Slot::Detail => div().w_full().h(px(row_height)).into_any_element(),
                        Slot::Row(index) => {
                            let row = render_row(index, window, cx);
                            drawn.borrow_mut().insert(index, row.id.clone());
                            let open = opened.contains(&row.id);
                            row_element(
                                &ident,
                                &theme,
                                row_height,
                                detail_rows,
                                &columns,
                                row,
                                open,
                                render_detail.as_ref(),
                                &context,
                                window,
                                cx,
                            )
                        }
                    })
                    .collect::<Vec<_>>()
            },
        )
        .track_scroll(&state.scroll)
        .w_full()
        .with_sizing_behavior(if self.visible_rows.is_some() {
            ListSizingBehavior::Auto
        } else {
            ListSizingBehavior::Infer
        })
        .when_some(self.visible_rows, |element, rows| {
            element.h(px(row_height * rows as f32))
        });

        list.into_any_element()
    }

    /// What a grid with no rows shows, which is never the same thing twice.
    fn vacant(
        &self,
        theme: &Theme,
        row_height: f32,
        vacancy: Option<EmptyState>,
        cx: &mut App,
    ) -> AnyElement {
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
            return EmptyState::new(
                self.ident.child("empty"),
                cx.strings().text(StringKey::GridLoadFailed),
            )
            .kind(EmptyKind::Failed)
            .detail(failure)
            .into_any_element();
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

/// Everything a row needs that does not come from the row itself.
struct RowContext {
    lines: GridLines,
    selected: BTreeSet<SharedString>,
    selection_mode: SelectionMode,
    disabled: bool,
    on_select: Option<SelectHandler>,
    on_expand: Option<ExpandHandler>,
    on_edit_request: Option<EditRequestHandler>,
    on_edit: Option<EditHandler>,
    editing: Option<EditingCell>,
    editor: Option<Entity<TextInput>>,
    state: Rc<Memory>,
    hierarchy: bool,
}

#[allow(clippy::too_many_arguments)]
fn row_element(
    grid: &Ident,
    theme: &Theme,
    height: f32,
    detail_rows: usize,
    columns: &[GridColumn],
    mut row: GridRow,
    open: bool,
    render_detail: Option<&RenderDetail>,
    context: &RowContext,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let ident = grid.child(row.id.as_ref());
    let selected = context.selected.contains(&row.id);
    let selectable = !row.disabled
        && !context.disabled
        && context.selection_mode != SelectionMode::None
        && context.on_select.is_some();

    let mut element = div()
        .id(ident.element_id())
        .relative()
        .row()
        .w_full()
        .h(px(height))
        .px_token(theme, Space::Sm)
        .gap_token(theme, Space::Sm)
        .when(context.lines == GridLines::Rows, |element| {
            element
                .border_b(px(theme.borders.hairline))
                .border_color(theme.colors.hairline)
        })
        .when(selected, |element| element.bg(theme.colors.selected))
        .when(row.disabled, |element| {
            element.opacity(theme.opacity.disabled)
        })
        .when(selectable, |element| {
            element
                .cursor_pointer()
                .tab_index(0)
                .pressable(cx)
                .when(!selected, |element| {
                    element.hover(|style| style.bg(theme.colors.hover.opacity(0.3)))
                })
                .focus_ring(theme)
        })
        .when(context.hierarchy, |element| {
            element.row_reading(cx.layout_direction())
        });

    if context.selection_mode == SelectionMode::Multiple {
        element = element.child(row_mark(theme, selected));
    }

    if !context.hierarchy
        && let Some(expand) = context.on_expand.clone().filter(|_| !context.disabled)
    {
        element = element.child(disclosure(&ident, theme, &row, open, expand, cx));
    }

    let pinned = columns.iter().filter(|column| column.pinned).count();
    for (position, column) in columns.iter().enumerate() {
        // Tab leaves an open cell for the next editable column in the same
        // row. A row whose editable columns are exhausted simply commits: the
        // row after it may not have been drawn, and the grid will not build a
        // row nobody asked to see in order to guess where a caret should go.
        let next = columns
            .iter()
            .skip(position + 1)
            .find(|next| next.editable)
            .map(|next| (row.id.clone(), next.key.clone()));
        element = element.child(cell_element(
            &ident,
            theme,
            column,
            &mut row,
            next,
            position == 0,
            context,
            window,
            cx,
        ));
        if position + 1 == pinned {
            element = element.child(pinned_edge(theme));
        }
    }

    if let (true, Some(handler)) = (selectable, context.on_select.clone()) {
        let id = row.id.clone();
        let anchor = Rc::clone(&context.state);
        let multiple = context.selection_mode == SelectionMode::Multiple;
        element = element.on_click(move |event: &ClickEvent, window, cx| {
            let modifiers = event.modifiers();
            let previous = anchor.anchor.borrow().clone();
            let change = match (
                multiple,
                modifiers.shift,
                modifiers.platform || modifiers.control,
            ) {
                (true, true, _) => match previous {
                    Some(previous) => SelectionChange::Range {
                        anchor: previous,
                        to: id.clone(),
                    },
                    None => SelectionChange::Replace(id.clone()),
                },
                (true, _, true) => SelectionChange::Toggle(id.clone()),
                _ => SelectionChange::Replace(id.clone()),
            };
            // A shift click measures from the anchor and does not move it, so
            // widening a span twice keeps starting where the typist started.
            if !matches!(change, SelectionChange::Range { .. }) {
                *anchor.anchor.borrow_mut() = Some(id.clone());
            }
            handler(&change, window, cx);
        });
    }

    let mut spec = NodeSpec::new(ident.semantic_id(), Role::Row)
        .parent(grid.semantic_id())
        .selected(selected)
        .disabled(row.disabled || context.disabled);
    if context.on_expand.is_some() {
        spec = spec.expanded(open);
    }
    if let Some(meta) = &row.hierarchy {
        spec = spec.level(meta.level);
        if meta.has_children {
            spec = spec.expanded(meta.expanded);
        }
    }
    if let Some(text) = row.text.clone() {
        spec = spec.text(text);
    }
    let mut element = element.semantic_in(cx, spec);

    // The detail is painted over the slots the grid held open beneath the
    // row, because a uniform list gives every slot the same height and an
    // opened row cannot simply grow.
    if let (true, Some(render)) = (open, render_detail) {
        let detail = ident.child("detail");
        element = element.child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(height))
                .h(px(height * detail_rows as f32))
                .overflow_hidden()
                .bg(theme.colors.panel)
                .p_token(theme, Space::Sm)
                .child(render(row.id.clone(), window, cx))
                .semantic_in(
                    cx,
                    NodeSpec::new(detail.semantic_id(), Role::Group)
                        .parent(ident.semantic_id())
                        .text(row.text.clone().unwrap_or_else(|| row.id.clone())),
                )
                // `semantic_in` makes its host relative so the probe measures
                // it, which would put the detail back in the flow.
                .absolute(),
        );
    }

    element.into_any_element()
}

/// The line that says where the pinned group ends.
fn pinned_edge(theme: &Theme) -> gpui::Div {
    div()
        .w(px(theme.borders.hairline))
        .h_full()
        .flex_none()
        .bg(theme.colors.hairline_strong)
}

/// The mark that says a row is in the selection. It is not a control: the row
/// itself takes the click, and a second box would be a second thing to aim at.
fn row_mark(theme: &Theme, selected: bool) -> gpui::Div {
    div().w(px(GUTTER)).flex_none().flex().items_center().child(
        div()
            .size(px(MARK))
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .radius(theme, Radius::Small)
            .border(px(theme.borders.hairline))
            .border_color(if selected {
                theme.colors.accent
            } else {
                theme.colors.hairline_strong
            })
            .when(selected, |element| {
                element.bg(theme.colors.accent).child(
                    icon(Icon::Check)
                        .size(px(MARK * 0.7))
                        .text_color(theme.colors.text_on_accent),
                )
            }),
    )
}

fn disclosure(
    ident: &Ident,
    theme: &Theme,
    row: &GridRow,
    open: bool,
    handler: ExpandHandler,
    cx: &mut App,
) -> AnyElement {
    let toggle = ident.child("expand");
    let id = row.id.clone();
    let next = !open;
    let key_handler = Rc::clone(&handler);
    let key_id = id.clone();

    div()
        .id(toggle.element_id())
        .w(px(GUTTER))
        .h_full()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .tab_index(0)
        .radius(theme, Radius::Small)
        .hover(|style| style.bg(theme.colors.hover))
        .focus_ring(theme)
        .child(
            icon(if open {
                Icon::AltArrowDown
            } else {
                Icon::AltArrowRight
            })
            .size(px(theme.control.sm.icon_size))
            .text_color(theme.colors.text_muted),
        )
        .on_click(move |_, window, cx| {
            handler(id.clone(), next, window, cx);
            cx.stop_propagation();
        })
        .on_key_down(move |event, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                key_handler(key_id.clone(), next, window, cx);
                cx.stop_propagation();
            }
        })
        .semantic_in(
            cx,
            NodeSpec::new(toggle.semantic_id(), Role::Button)
                .parent(ident.semantic_id())
                .text(format!(
                    "{} {}",
                    cx.strings().text(if open {
                        StringKey::Collapse
                    } else {
                        StringKey::Expand
                    }),
                    row.text.clone().unwrap_or_else(|| row.id.clone())
                ))
                .expanded(open),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn cell_element(
    ident: &Ident,
    theme: &Theme,
    column: &GridColumn,
    row: &mut GridRow,
    next: Option<(SharedString, SharedString)>,
    logical_start: bool,
    context: &RowContext,
    _window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let cell = row.take(&column.key);
    let editing = context
        .editing
        .as_ref()
        .filter(|edit| edit.row == row.id && edit.column == column.key);
    let editable = column.editable && !row.disabled && !context.disabled;

    if let (Some(edit), Some(field)) = (editing, context.editor.clone()) {
        return editor_cell(theme, column, row, edit, field, next, context);
    }

    // A treegrid's rendered columns are structural gridcells, not optional
    // diagnostic detail. DataGrid retains its opt-in publication policy.
    let published =
        context.hierarchy || cell.as_ref().is_some_and(|cell| cell.published) || editable;
    let text = cell.as_ref().and_then(|cell| cell.text.clone());
    let mut content = cell.map(|cell| cell.content);
    if logical_start && context.hierarchy {
        let hierarchy = row.hierarchy.clone();
        let direction = cx.layout_direction();
        let mut leading = div()
            .row_reading(direction)
            .items_center()
            .min_w_0()
            .w_full();
        if let Some(meta) = hierarchy {
            leading = leading.ps(
                direction,
                px((meta.level.saturating_sub(1) as f32) * theme.spacing.md),
            );
            if meta.has_children {
                if let Some(expand) = context
                    .on_expand
                    .clone()
                    .filter(|_| !context.disabled && !row.disabled)
                {
                    leading =
                        leading.child(disclosure(ident, theme, row, meta.expanded, expand, cx));
                }
            } else {
                leading = leading.child(div().w(px(GUTTER)).flex_none());
            }
        }
        leading = leading.children(content.take());
        content = Some(leading.into_any_element());
    }

    if !published {
        return column_frame(div(), column, theme)
            .overflow_hidden()
            .children(content)
            .into_any_element();
    }

    let cell_ident = ident.child(column.key.as_ref());
    let mut spec = NodeSpec::new(
        cell_ident.semantic_id(),
        if context.hierarchy {
            Role::GridCell
        } else {
            Role::Cell
        },
    )
    .parent(ident.semantic_id());
    if let Some(text) = text {
        spec = spec.text(text);
    }

    let mut frame = column_frame(div().id(cell_ident.element_id()), column, theme)
        .overflow_hidden()
        .children(content);

    if let (true, Some(request)) = (editable, context.on_edit_request.clone()) {
        let row_id = row.id.clone();
        let key = column.key.clone();
        let key_request = Rc::clone(&request);
        let key_row = row_id.clone();
        let key_key = key.clone();
        frame = frame
            .tab_index(0)
            .cursor_pointer()
            .focus_ring(theme)
            .on_click(move |event: &ClickEvent, window, cx| {
                if event.click_count() < 2 {
                    return;
                }
                request(row_id.clone(), key.clone(), window, cx);
                cx.stop_propagation();
            })
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key.as_str() == "enter" {
                    key_request(key_row.clone(), key_key.clone(), window, cx);
                    cx.stop_propagation();
                }
            });
    }

    frame.semantic_in(cx, spec).into_any_element()
}

/// A cell the caller has opened, showing the one field the grid keeps.
#[allow(clippy::too_many_arguments)]
fn editor_cell(
    theme: &Theme,
    column: &GridColumn,
    row: &GridRow,
    edit: &EditingCell,
    field: Entity<TextInput>,
    next: Option<(SharedString, SharedString)>,
    context: &RowContext,
) -> AnyElement {
    let frame = column_frame(div(), column, theme)
        .overflow_hidden()
        .px(px(theme.spacing.xs))
        .radius(theme, Radius::Small)
        .well(theme)
        .shadow(theme.focus_ring());

    let Some(handler) = context.on_edit.clone() else {
        return frame.child(field).into_any_element();
    };

    let row_id = row.id.clone();
    let key = column.key.clone();
    let seed = edit.value.clone();
    let reading = field.clone();

    // The field's own key bindings dispatch before any ancestor's key
    // listener, so enter and escape are taken in the capture phase of the
    // action rather than as keystrokes that never arrive.
    let commit = {
        let handler = Rc::clone(&handler);
        let field = reading.clone();
        let row_id = row_id.clone();
        let key = key.clone();
        move |_: &Submit, window: &mut Window, cx: &mut App| {
            let value = field.read(cx).value().clone();
            handler(
                &EditIntent {
                    row: row_id.clone(),
                    column: key.clone(),
                    value,
                    outcome: EditOutcome::Commit,
                    next: None,
                },
                window,
                cx,
            );
            cx.stop_propagation();
        }
    };

    let revert = {
        let handler = Rc::clone(&handler);
        let row_id = row_id.clone();
        let key = key.clone();
        move |_: &Cancel, window: &mut Window, cx: &mut App| {
            handler(
                &EditIntent {
                    row: row_id.clone(),
                    column: key.clone(),
                    value: seed.clone(),
                    outcome: EditOutcome::Revert,
                    next: None,
                },
                window,
                cx,
            );
            cx.stop_propagation();
        }
    };

    let advance = {
        let handler = Rc::clone(&handler);
        let field = reading.clone();
        let row_id = row_id.clone();
        let key = key.clone();
        move |event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut App| {
            if event.keystroke.key.as_str() != "tab" {
                return;
            }
            let value = field.read(cx).value().clone();
            handler(
                &EditIntent {
                    row: row_id.clone(),
                    column: key.clone(),
                    value,
                    outcome: EditOutcome::Commit,
                    next: next.clone(),
                },
                window,
                cx,
            );
            cx.stop_propagation();
        }
    };

    frame
        .capture_action::<Submit>(commit)
        .capture_action::<Cancel>(revert)
        .capture_key_down(advance)
        .child(field)
        .into_any_element()
}

fn column_frame<E: Styled>(element: E, column: &GridColumn, theme: &Theme) -> E {
    let element = match column.width {
        ColumnWidth::Fixed(width) => element.w(px(width)).flex_none(),
        // A zero basis makes a share depend on the flex factor alone, so a
        // header cell and the body cell under it always agree on where the
        // column starts.
        ColumnWidth::Flex(share) => element
            .flex_grow(share)
            .flex_shrink(1.0)
            .flex_basis(px(0.0))
            .min_w(px(column.min_width)),
    };
    let element = element.row().h_full().gap(px(theme.space(Space::Xs)));
    match column.align {
        Align::Start => element.justify_start(),
        Align::Center => element.justify_center(),
        Align::End => element.justify_end(),
    }
}

/// Where the row a keyboard move starts from sits among the rows that were
/// drawn.
///
/// A selection scrolled out of the viewport has no known index, so a move
/// starts from the top of what is visible rather than from nowhere.
fn current_index(
    drawn: &Drawn,
    anchor: Option<&SharedString>,
    selected: &BTreeSet<SharedString>,
) -> Option<usize> {
    let drawn = drawn.borrow();
    let matches = |id: &SharedString| {
        drawn
            .iter()
            .find(|(_, row)| *row == id)
            .map(|(index, _)| *index)
    };
    anchor
        .and_then(matches)
        .or_else(|| selected.iter().find_map(matches))
        .or_else(|| drawn.keys().min().copied())
}

fn target_index(key: &str, from: Option<usize>, count: usize) -> Option<usize> {
    match key {
        "up" => from?.checked_sub(1),
        "down" => match from {
            Some(from) => Some(from + 1).filter(|next| *next < count),
            None => Some(0),
        },
        "home" => Some(0),
        "end" => count.checked_sub(1),
        _ => None,
    }
}

/// The first row from `target` in `step`'s direction that accepts a selection.
///
/// Naming a row that was never drawn means asking the caller to build it,
/// which is the only way a grid that does not hold the data can report a row
/// the typist cannot yet see.
fn reachable(
    render_row: &RenderRow,
    target: usize,
    step: isize,
    count: usize,
    window: &mut Window,
    cx: &mut App,
) -> Option<(usize, SharedString)> {
    let mut index = target as isize;
    while index >= 0 && (index as usize) < count {
        let row = render_row(index as usize, window, cx);
        if !row.disabled {
            return Some((index as usize, row.id));
        }
        index += step;
    }
    None
}

// -- the bulk bar -------------------------------------------------------------

type BulkHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The bar that appears over a selection, says how large it is, and offers
/// what can be done to it.
///
/// The count is the number of rows the caller says are selected, which is the
/// number the bar states. When more rows exist than the host has loaded, the
/// bar offers the wider intent as a separate, named action rather than
/// quietly claiming the selection already covers them.
#[derive(IntoElement)]
pub struct BulkBar {
    ident: Ident,
    count: usize,
    total: Option<usize>,
    noun: Option<SharedString>,
    actions: Vec<AnyElement>,
    on_select_all: Option<BulkHandler>,
    on_dismiss: Option<BulkHandler>,
}

impl std::fmt::Debug for BulkBar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BulkBar")
            .field("ident", &self.ident)
            .field("count", &self.count)
            .field("total", &self.total)
            .field("actions", &self.actions.len())
            .finish()
    }
}

impl BulkBar {
    pub fn new(ident: impl Into<Ident>, count: usize) -> Self {
        Self {
            ident: ident.into(),
            count,
            total: None,
            noun: None,
            actions: Vec::new(),
            on_select_all: None,
            on_dismiss: None,
        }
    }

    /// How many rows exist. When it is larger than the count, the bar offers
    /// to widen the selection and says exactly how far.
    pub fn total(mut self, total: usize) -> Self {
        self.total = Some(total);
        self
    }

    /// What the count is counting, for a surface whose rows are not "rows".
    pub fn noun(mut self, noun: impl Into<SharedString>) -> Self {
        self.noun = Some(noun.into());
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    pub fn on_select_all(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select_all = Some(Rc::new(handler));
        self
    }

    /// Dismissing a bulk bar clears the selection: there is nothing else it
    /// could mean, and a bar that hid while the selection stood would leave
    /// the typist operating rows they can no longer see.
    pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for BulkBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state = memory(&self.ident.semantic_id(), cx);
        let progress = {
            let mut held = state.bulk.borrow_mut();
            let presence = held.get_or_insert_with(|| {
                if self.count > 0 {
                    // A bar the first frame already needs is not an arrival.
                    Presence::visible(entrance(&theme), state_change(&theme))
                } else {
                    Presence::hidden(entrance(&theme), state_change(&theme))
                }
            });
            if self.count > 0 {
                presence.show();
            } else {
                presence.hide();
            }
            presence.animate(window, cx)
        };

        if progress <= 0.0 {
            return div().into_any_element();
        }

        let total = self.total.unwrap_or(self.count);
        let noun = self
            .noun
            .clone()
            .unwrap_or_else(|| cx.strings().text(StringKey::GridSelectedNoun));
        let label = SharedString::from(format!("{} {}", self.count, noun));
        let wider = self
            .on_select_all
            .clone()
            .filter(|_| total > self.count)
            .map(|handler| (handler, total));

        let mut bar = div()
            .id(self.ident.element_id())
            .row()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .px_token(&theme, Space::Sm)
            .py_token(&theme, Space::Xs)
            .radius(&theme, Radius::Control)
            .frame(&theme, Surface::Raised, Elevation::Raised)
            .opacity(progress)
            .type_scale(&theme, TypeScale::Label)
            .text_color(theme.colors.text)
            .child(div().flex_none().child(label.clone()));

        if let Some((handler, total)) = wider {
            let ident = self.ident.child("select-all");
            bar = bar.child(
                div()
                    .id(ident.element_id())
                    .flex_none()
                    .cursor_pointer()
                    .tab_index(0)
                    .text_color(theme.colors.accent)
                    .focus_ring(&theme)
                    .child(
                        cx.strings()
                            .format(StringKey::GridSelectAllTotal, &[&total.to_string()]),
                    )
                    .on_click(move |_, window, cx| handler(window, cx))
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.semantic_id(), Role::Button)
                            .parent(self.ident.semantic_id())
                            .text(
                                cx.strings()
                                    .format(StringKey::GridSelectAllTotal, &[&total.to_string()]),
                            )
                            .value(total.to_string()),
                    ),
            );
        }

        bar = bar.child(div().flex_1());

        for action in self.actions {
            bar = bar.child(div().flex_none().child(action));
        }

        if let Some(handler) = self.on_dismiss.clone() {
            bar = bar.child(
                crate::controls::button::IconButton::new(
                    self.ident.child("dismiss"),
                    Icon::Close,
                    cx.strings().text(StringKey::GridClearSelection),
                )
                .semantic_parent(self.ident.semantic_id())
                .on_click(move |window, cx| handler(window, cx)),
            );
        }

        bar.semantic_in(
            cx,
            NodeSpec::new(self.ident.semantic_id(), Role::Toolbar)
                .text(label)
                .value(self.count.to_string()),
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unopened_grid_maps_every_slot_onto_its_own_row() {
        assert_eq!(slot_count(5, &[], 2), 5);
        assert_eq!(slot_at(0, &[], 2), Slot::Row(0));
        assert_eq!(slot_at(4, &[], 2), Slot::Row(4));
        assert_eq!(slot_of(3, &[], 2), 3);
    }

    #[test]
    fn an_opened_row_holds_the_slots_beneath_it_open() {
        let expanded = [2usize];
        assert_eq!(slot_count(5, &expanded, 2), 7);
        assert_eq!(slot_at(1, &expanded, 2), Slot::Row(1));
        assert_eq!(slot_at(2, &expanded, 2), Slot::Row(2));
        assert_eq!(slot_at(3, &expanded, 2), Slot::Detail);
        assert_eq!(slot_at(4, &expanded, 2), Slot::Detail);
        assert_eq!(slot_at(5, &expanded, 2), Slot::Row(3));
        assert_eq!(slot_of(3, &expanded, 2), 5);
    }

    #[test]
    fn two_opened_rows_each_hold_their_own_slots() {
        let expanded = [0usize, 3];
        assert_eq!(slot_count(5, &expanded, 1), 7);
        assert_eq!(slot_at(0, &expanded, 1), Slot::Row(0));
        assert_eq!(slot_at(1, &expanded, 1), Slot::Detail);
        assert_eq!(slot_at(2, &expanded, 1), Slot::Row(1));
        assert_eq!(slot_at(4, &expanded, 1), Slot::Row(3));
        assert_eq!(slot_at(5, &expanded, 1), Slot::Detail);
        assert_eq!(slot_at(6, &expanded, 1), Slot::Row(4));
        assert_eq!(slot_of(4, &expanded, 1), 6);
    }

    #[test]
    fn a_move_stops_at_the_ends_instead_of_wrapping() {
        assert_eq!(target_index("up", Some(0), 10), None);
        assert_eq!(target_index("down", Some(9), 10), None);
        assert_eq!(target_index("home", Some(3), 10), Some(0));
        assert_eq!(target_index("end", Some(3), 10), Some(9));
        assert_eq!(target_index("down", None, 10), Some(0));
    }
}
