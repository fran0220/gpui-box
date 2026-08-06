//! Large-data surfaces: a virtualized list, two tables, and a tree.
//!
//! [`Table`] takes materialized rows and lays every one of them out.
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
//! rows publish semantics.** A virtualized list holds a viewport, not a data
//! set, so a test can assert only what is on screen. The container node
//! carries the total in `value`, which is how a snapshot stays honest about
//! the difference between a thousand items and the twelve that are drawn.

pub mod grid;
pub mod list;
pub mod table;
pub mod tree;

pub use grid::{
    BulkBar, DataGrid, EditIntent, EditOutcome, EditingCell, Expanded, GridColumn, GridRow,
    SelectionChange, SelectionMode,
};
pub use list::{List, ListItem};
pub use table::{Align, Cell, Column, ColumnWidth, Row, SortDirection, Table};
pub use tree::{Tree, TreeNode};
