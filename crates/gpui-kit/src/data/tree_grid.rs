//! A virtualized grid over a caller-flattened hierarchy.
//!
//! `TreeGrid` does not discover topology, flatten nodes, or own expansion and
//! selection. The caller supplies visible rows in reading order and receives
//! selection and expansion intents. The flatten cache lives on [`crate::data::Tree`];
//! this surface virtualizes whatever the host already flattened. Only
//! materialized viewport rows publish the ARIA hierarchy
//! `TreeGrid > Row > GridCell`.
//!
//! Columns are fixed or flexible within the available width. There is no
//! horizontal scrolling or frozen-column behavior.

use std::rc::Rc;

use gpui::{App, IntoElement, RenderOnce, SharedString, Window};

use crate::data::grid::{DataGrid, GridRow, HierarchyRow};
use crate::data::{Cell, GridColumn, GridLines, SelectionChange, SelectionMode};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Disableable, Ident};

type RenderRow = Rc<dyn Fn(usize, &mut Window, &mut App) -> TreeGridRow>;
type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type ExpandHandler = Rc<dyn Fn(SharedString, bool, &mut Window, &mut App)>;

/// One already-visible hierarchy row, identified by stable business identity.
#[derive(Debug)]
pub struct TreeGridRow {
    row: GridRow,
    level: u32,
    has_children: bool,
    expanded: bool,
    parent: Option<SharedString>,
}

impl TreeGridRow {
    pub fn new(id: impl Into<SharedString>, level: u32) -> Self {
        Self {
            row: GridRow::new(id),
            level: level.max(1),
            has_children: false,
            expanded: false,
            parent: None,
        }
    }

    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.row = self.row.text(text);
        self
    }

    pub fn cell(mut self, key: impl Into<SharedString>, cell: impl Into<Cell>) -> Self {
        self.row = self.row.cell(key, cell);
        self
    }

    pub fn branch(mut self, expanded: bool) -> Self {
        self.has_children = true;
        self.expanded = expanded;
        self
    }

    /// The caller-supplied visible parent used by logical-start navigation.
    pub fn parent(mut self, id: impl Into<SharedString>) -> Self {
        self.parent = Some(id.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.row = self.row.disabled(disabled);
        self
    }

    fn into_grid_row(self) -> GridRow {
        self.row.hierarchy(HierarchyRow {
            level: self.level,
            has_children: self.has_children,
            expanded: self.expanded,
            parent: self.parent,
        })
    }
}

/// A product-neutral, caller-owned virtualized treegrid adapter.
#[derive(IntoElement)]
pub struct TreeGrid {
    ident: Ident,
    count: usize,
    render_row: RenderRow,
    columns: Vec<GridColumn>,
    selected: Option<SharedString>,
    visible_rows: Option<usize>,
    row_height: Option<f32>,
    lines: GridLines,
    loading: bool,
    failure: Option<SharedString>,
    vacancy: Option<EmptyState>,
    disabled: bool,
    on_select: Option<SelectHandler>,
    on_expand: Option<ExpandHandler>,
    slots: Slots,
}

impl std::fmt::Debug for TreeGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreeGrid")
            .field("ident", &self.ident)
            .field("count", &self.count)
            .field("columns", &self.columns.len())
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl TreeGrid {
    pub fn new(
        ident: impl Into<Ident>,
        visible_row_count: usize,
        render_row: impl Fn(usize, &mut Window, &mut App) -> TreeGridRow + 'static,
    ) -> Self {
        Self {
            ident: ident.into(),
            count: visible_row_count,
            render_row: Rc::new(render_row),
            columns: vec![],
            selected: None,
            visible_rows: None,
            row_height: None,
            lines: GridLines::None,
            loading: false,
            failure: None,
            vacancy: None,
            disabled: false,
            on_select: None,
            on_expand: None,
            slots: Slots::default(),
        }
    }

    pub fn column(mut self, column: GridColumn) -> Self {
        self.columns.push(column);
        self
    }
    pub fn columns(mut self, columns: impl IntoIterator<Item = GridColumn>) -> Self {
        self.columns.extend(columns);
        self
    }
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }
    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = Some(rows);
        self
    }
    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = Some(height);
        self
    }
    pub fn lines(mut self, lines: GridLines) -> Self {
        self.lines = lines;
        self
    }
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }
    pub fn failure(mut self, detail: impl Into<SharedString>) -> Self {
        self.failure = Some(detail.into());
        self
    }
    pub fn empty(mut self, state: EmptyState) -> Self {
        self.vacancy = Some(state);
        self
    }
    pub fn unavailable(mut self, title: impl Into<SharedString>) -> Self {
        self.vacancy = Some(
            EmptyState::new(self.ident.child("unavailable"), title).kind(EmptyKind::Unavailable),
        );
        self
    }
    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
    pub fn on_expand(
        mut self,
        handler: impl Fn(SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_expand = Some(Rc::new(handler));
        self
    }
}

impl Disableable for TreeGrid {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Slotted for TreeGrid {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for TreeGrid {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let render = self.render_row;
        let mut grid = DataGrid::new(self.ident, self.count, move |index, window, cx| {
            render(index, window, cx).into_grid_row()
        })
        .columns(self.columns)
        .lines(self.lines)
        .selection_mode(SelectionMode::Single)
        .selected(self.selected)
        .loading(self.loading)
        .hierarchy_mode()
        .disabled(self.disabled);
        if let Some(rows) = self.visible_rows {
            grid = grid.visible_rows(rows);
        }
        if let Some(height) = self.row_height {
            grid = grid.row_height(height);
        }
        if let Some(failure) = self.failure {
            grid = grid.failure(failure);
        }
        if let Some(empty) = self.vacancy {
            grid = grid.empty(empty);
        }
        if let Some(handler) = self.on_select {
            grid = grid.on_select(move |change, window, cx| {
                if let SelectionChange::Replace(id) = change {
                    handler(id.clone(), window, cx);
                }
            });
        }
        if let Some(handler) = self.on_expand {
            grid = grid.on_expand(move |id, open, window, cx| handler(id, open, window, cx));
        }
        let slots = self.slots;
        for name in [slot::EMPTY, slot::FAILED, slot::LOADING] {
            if slots.holds(name) {
                let slots = slots.clone();
                grid = grid.slot(name, move |window, cx| {
                    slots.render(name, window, cx).expect("slot was filled")
                });
            }
        }
        grid
    }
}
