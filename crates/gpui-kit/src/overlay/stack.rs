//! Which modal surface is on top.
//!
//! A dialog and a drawer both listen for escape. When two of them are open,
//! only the one that opened last may dismiss: otherwise a nested question
//! would close the surface that asked it. Depth is also the paint order, so
//! the same stack decides which card sits in front.

use gpui::{App, FocusHandle, SharedString, Window};

use crate::foundation::window_state;

/// A custom modal's membership in the same per-window stack as [`super::Dialog`]
/// and [`super::Drawer`]. This handle owns no content or input policy.
///
/// Call `open` before moving focus into the surface and `close` on every removal
/// path (after an exit animation, if any). Repeated calls are harmless. The id
/// must be unique among modal surfaces in a window. Gate Escape, scrim and Tab
/// handlers with `is_top`, and pass `depth` to [`super::Overlay::stack`]. Register
/// custom Tab stops with [`super::FocusTrap`], but do not engage/release its
/// independent restoration lifecycle: this stack owns focus restoration.
#[derive(Debug, Clone)]
pub struct ModalScope {
    id: SharedString,
}

impl ModalScope {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self { id: id.into() }
    }

    /// Captures the current focus once, before the surface receives focus.
    pub fn open(&self, window: &Window, cx: &mut App) {
        push(self.id.clone(), window, cx);
    }

    /// Removes this surface. Only closing the top restores focus; removing a
    /// covered surface splices the return chain without moving focus.
    pub fn close(&self, window: &mut Window, cx: &mut App) {
        pop(&self.id, window, cx);
    }

    pub fn is_open(&self, window: &Window, cx: &App) -> bool {
        window_state::read(
            window.window_handle().window_id(),
            cx,
            |stack: &OpenModals| stack.0.iter().any(|held| held.id == self.id),
        )
        .unwrap_or(false)
    }

    pub fn is_top(&self, window: &Window, cx: &App) -> bool {
        is_top(&self.id, window, cx)
    }

    /// Zero-based paint depth, or zero when this surface is closed.
    pub fn depth(&self, window: &Window, cx: &App) -> usize {
        depth(&self.id, window, cx)
    }
}

#[derive(Default)]
struct OpenModals(Vec<Modal>);

struct Modal {
    id: SharedString,
    restore: Option<FocusHandle>,
}

/// Records that this surface is now the top of the modal stack.
pub fn push(id: SharedString, window: &Window, cx: &mut App) {
    let restore = window.focused(cx);
    window_state::with(
        window.window_handle().window_id(),
        cx,
        |stack: &mut OpenModals| {
            if !stack.0.iter().any(|held| held.id == id) {
                stack.0.push(Modal { id, restore });
            }
        },
    );
}

/// Forgets a closed surface and splices its restoration chain. Closing a
/// covered modal never steals focus; its successor inherits the return target
/// so closing that successor cannot restore focus into the closed surface.
pub fn pop(id: &SharedString, window: &mut Window, cx: &mut App) {
    let restore = window_state::with(
        window.window_handle().window_id(),
        cx,
        |stack: &mut OpenModals| {
            let index = stack.0.iter().position(|held| &held.id == id)?;
            let removed = stack.0.remove(index);
            if let Some(successor) = stack.0.get_mut(index) {
                successor.restore = removed.restore;
                None
            } else {
                removed.restore
            }
        },
    );
    if let Some(restore) = restore {
        restore.focus(window, cx);
    }
}

/// Whether escape and the scrim belong to this surface.
pub fn is_top(id: &SharedString, window: &Window, cx: &App) -> bool {
    window_state::read(
        window.window_handle().window_id(),
        cx,
        |stack: &OpenModals| stack.0.last().is_some_and(|held| &held.id == id),
    )
    .unwrap_or(false)
}

/// How many modal surfaces sit under this one, which is also how far above
/// the modal layer it paints.
pub fn depth(id: &SharedString, window: &Window, cx: &App) -> usize {
    window_state::read(
        window.window_handle().window_id(),
        cx,
        |stack: &OpenModals| stack.0.iter().position(|held| &held.id == id),
    )
    .flatten()
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext, Context, IntoElement, Render, TestAppContext, Window, div};

    use super::*;

    struct Fixture;

    impl Render for Fixture {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[gpui::test]
    fn custom_scope_splices_focus_with_a_nested_dialog(cx: &mut TestAppContext) {
        let window = cx.add_window(|_, _| Fixture);
        cx.update_window(*window, |_, window, cx| {
            let page_focus = cx.focus_handle();
            let custom_focus = cx.focus_handle();
            let dialog_focus = cx.focus_handle();
            page_focus.focus(window, cx);
            let custom = ModalScope::new("custom");
            custom.open(window, cx);
            custom_focus.focus(window, cx);
            custom.open(window, cx); // Must not overwrite the return target.
            let dialog = cx.new(|cx| super::super::Dialog::new("nested", window, cx));
            dialog.update(cx, |dialog, cx| dialog.open(window, cx));
            dialog_focus.focus(window, cx);
            let nested = ModalScope::new("nested");
            assert!(custom.is_open(window, cx));
            assert!(!custom.is_top(window, cx));
            assert!(nested.is_top(window, cx));
            assert_eq!(nested.depth(window, cx), 1);
            custom.close(window, cx);
            assert_eq!(window.focused(cx), Some(dialog_focus));
            assert_eq!(nested.depth(window, cx), 0);
            dialog.update(cx, |dialog, cx| dialog.close(window, cx));
            assert_eq!(window.focused(cx), Some(page_focus));
            assert!(!custom.is_open(window, cx));
            assert!(!nested.is_open(window, cx));
        })
        .expect("window");
    }

    #[test]
    fn the_last_push_is_the_top() {
        let mut stack = OpenModals::default();
        stack.0.push(Modal {
            id: "outer".into(),
            restore: None,
        });
        stack.0.push(Modal {
            id: "inner".into(),
            restore: None,
        });
        assert_eq!(stack.0.last().map(|held| held.id.as_ref()), Some("inner"));
        stack.0.retain(|held| held.id.as_ref() != "inner");
        assert_eq!(stack.0.last().map(|held| held.id.as_ref()), Some("outer"));
    }

    #[gpui::test]
    fn equal_modal_ids_are_isolated_by_window(cx: &mut TestAppContext) {
        let left = cx.add_window(|_, _| Fixture);
        let right = cx.add_window(|_, _| Fixture);
        let shared = SharedString::new_static("shared");

        cx.update_window(*left, |_, window, cx| push(shared.clone(), window, cx))
            .expect("left window");
        cx.update_window(*right, |_, window, cx| push(shared.clone(), window, cx))
            .expect("right window");
        cx.update_window(*left, |_, window, cx| {
            pop(&shared, window, cx);
            assert!(!is_top(&shared, window, cx));
        })
        .expect("left window");
        cx.update_window(*right, |_, window, cx| {
            assert!(is_top(&shared, window, cx));
            assert_eq!(depth(&shared, window, cx), 0);
        })
        .expect("right window");
    }
}
