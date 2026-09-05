//! Interaction systems that span more than one component.
//!
//! A component family owns its own pointer and keyboard behaviour. What lives
//! here is the machinery several families have to agree on, because the
//! gesture starts in one component and finishes in another.
//!
//! - [`dnd`] — carrying an item from where it is to where it should go.

pub mod dnd;

/// A paint-only listener for abandoning component-owned pointer state. It
/// adds no hitbox and emits no completion action. Cancellation is window-wide
/// and is deliberately distinct from release, including outside the target.
pub(crate) fn on_pointer_cancel(
    cancel: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> impl gpui::IntoElement {
    use gpui::Styled;
    gpui::canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            window.on_mouse_event(move |_: &gpui::MouseCancelEvent, phase, window, cx| {
                if phase == gpui::DispatchPhase::Capture {
                    cancel(window, cx);
                    window.refresh();
                }
            });
        },
    )
    .absolute()
    .size_full()
}

pub use dnd::{
    ActiveDrag, DRAG_NODE_ID, DragItem, DropAxis, DropIntent, DropPosition, FILE_KIND, ROW_KIND,
    StagedDrag,
};

/// Installs the drag session and the key that cancels a drag.
pub fn install(cx: &mut gpui::App) {
    dnd::install(cx);
}
