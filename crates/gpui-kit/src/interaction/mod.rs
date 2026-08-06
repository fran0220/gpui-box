//! Interaction systems that span more than one component.
//!
//! A component family owns its own pointer and keyboard behaviour. What lives
//! here is the machinery several families have to agree on, because the
//! gesture starts in one component and finishes in another.
//!
//! - [`dnd`] — carrying an item from where it is to where it should go.

pub mod dnd;

pub use dnd::{
    ActiveDrag, DRAG_NODE_ID, DragItem, DropAxis, DropIntent, DropPosition, FILE_KIND, ROW_KIND,
    StagedDrag,
};

/// Installs the drag session and the key that cancels a drag.
pub fn install(cx: &mut gpui::App) {
    dnd::install(cx);
}
