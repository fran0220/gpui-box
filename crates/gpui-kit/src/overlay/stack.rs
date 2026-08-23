//! Which modal surface is on top.
//!
//! A dialog and a drawer both listen for escape. When two of them are open,
//! only the one that opened last may dismiss: otherwise a nested question
//! would close the surface that asked it. Depth is also the paint order, so
//! the same stack decides which card sits in front.

use gpui::{App, SharedString, Window};

use crate::foundation::window_state;

#[derive(Default)]
struct OpenModals(Vec<SharedString>);

/// Records that this surface is now the top of the modal stack.
pub fn push(id: SharedString, window: &Window, cx: &mut App) {
    window_state::with(
        window.window_handle().window_id(),
        cx,
        |stack: &mut OpenModals| {
            stack.0.retain(|held| held != &id);
            stack.0.push(id);
        },
    );
}

/// Forgets a surface that has closed.
pub fn pop(id: &SharedString, window: &Window, cx: &mut App) {
    window_state::with(
        window.window_handle().window_id(),
        cx,
        |stack: &mut OpenModals| stack.0.retain(|held| held != id),
    );
}

/// Whether escape and the scrim belong to this surface.
pub fn is_top(id: &SharedString, window: &Window, cx: &App) -> bool {
    window_state::read(
        window.window_handle().window_id(),
        cx,
        |stack: &OpenModals| stack.0.last() == Some(id),
    )
    .unwrap_or(false)
}

/// How many modal surfaces sit under this one, which is also how far above
/// the modal layer it paints.
pub fn depth(id: &SharedString, window: &Window, cx: &App) -> usize {
    window_state::read(
        window.window_handle().window_id(),
        cx,
        |stack: &OpenModals| stack.0.iter().position(|held| held == id),
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

    #[test]
    fn the_last_push_is_the_top() {
        let mut stack = OpenModals::default();
        stack.0.push("outer".into());
        stack.0.push("inner".into());
        assert_eq!(stack.0.last().map(|id| id.as_ref()), Some("inner"));
        stack.0.retain(|held| held.as_ref() != "inner");
        assert_eq!(stack.0.last().map(|id| id.as_ref()), Some("outer"));
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
