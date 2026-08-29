//! Large-data surfaces: a virtualized list, two tables, and a tree.
//!
//! [`Table`] lays out every row it is given as [`Table::rows`], because the
//! caller built those elements already; handed a count and a closure through
//! [`Table::rows_from`], it lays out only the ones its viewport holds.
//! [`DataGrid`] takes a render closure and lays out only the rows its viewport
//! holds, which is what buys it column resizing and reordering, a pinned
//! group, selection over an incompletely loaded set, opened rows, and cell
//! editing. `docs/components.md` has the guidance on which to reach for.
//!
//! None of these owns the data. Rows, order, expansion, and selection are all
//! caller-owned; each surface reports what was operated and renders exactly
//! what the caller says is true, so a host that refuses a change keeps showing
//! the state that still holds.
//!
//! The rule that separates these from the rest of the library: **only rendered
//! rows publish semantics.** A virtualized surface holds a viewport, not a
//! data set, so a test can assert only what is on screen. The container node
//! carries the total in `value`, which is how a snapshot stays honest about
//! the difference between a thousand items and the twelve that are drawn. A
//! [`Tree`] counts the rows it disclosed, so a node under a shut branch, a
//! disclosed node that scrolled past the edge, and a node that is not in the
//! data at all remain three different things.
//!
//! Virtualization needs a bounded viewport. Every surface here takes a
//! `visible_rows` bound, and without one it sizes itself to its content and
//! lays every row out — which is the right answer for a settings summary and
//! the wrong one for a hundred thousand log lines.

pub mod diagnostics_list;
pub mod flow;
pub mod grid;
pub mod image_list;
pub mod kanban;
pub mod list;
pub mod masonry;
pub mod table;
pub mod tree;
pub mod tree_grid;
pub mod viewport;

pub use diagnostics_list::{
    Diagnostic, DiagnosticAction, DiagnosticFilter, DiagnosticLocation, DiagnosticSeverity,
    DiagnosticsList,
};
pub use flow::Flow;
pub use grid::{
    BulkBar, CellRange, ColumnGroup, DataGrid, EditIntent, EditOutcome, EditingCell, Expanded,
    GridColumn, GridLines, GridRow, SelectionChange, SelectionMode,
};
pub use image_list::{ImageList, ImageListItem};
pub use kanban::{KanbanBoard, KanbanCard, KanbanColumn, KanbanEvent, KanbanState};
pub use list::{List, ListItem};
pub use masonry::{Masonry, MasonryItem};
pub use table::{Align, Cell, Column, ColumnWidth, Row, SortDirection, Table};
pub use tree::{BranchState, Tree, TreeNode};
pub use tree_grid::{TreeGrid, TreeGridRow};
pub use viewport::{Viewed, glide_to_row, reveal_row, scroll_to_row, viewed_rows};
