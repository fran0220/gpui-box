//! Which modal surface is on top.
//!
//! A dialog and a drawer both listen for escape. When two of them are open,
//! only the one that opened last may dismiss: otherwise a nested question
//! would close the surface that asked it. Depth is also the paint order, so
//! the same stack decides which card sits in front.

use gpui::{App, Global, SharedString};

#[derive(Default)]
struct OpenModals(Vec<SharedString>);

impl Global for OpenModals {}

fn stack_mut(cx: &mut App) -> &mut OpenModals {
    if !cx.has_global::<OpenModals>() {
        cx.set_global(OpenModals::default());
    }
    cx.global_mut::<OpenModals>()
}

/// Records that this surface is now the top of the modal stack.
pub fn push(id: SharedString, cx: &mut App) {
    let stack = stack_mut(cx);
    stack.0.retain(|held| held != &id);
    stack.0.push(id);
}

/// Forgets a surface that has closed.
pub fn pop(id: &SharedString, cx: &mut App) {
    if !cx.has_global::<OpenModals>() {
        return;
    }
    cx.global_mut::<OpenModals>().0.retain(|held| held != id);
}

/// Whether escape and the scrim belong to this surface.
pub fn is_top(id: &SharedString, cx: &App) -> bool {
    cx.try_global::<OpenModals>()
        .and_then(|stack| stack.0.last())
        == Some(id)
}

/// How many modal surfaces sit under this one, which is also how far above
/// the modal layer it paints.
pub fn depth(id: &SharedString, cx: &App) -> usize {
    cx.try_global::<OpenModals>()
        .and_then(|stack| stack.0.iter().position(|held| held == id))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::OpenModals;

    #[test]
    fn the_last_push_is_the_top() {
        let mut stack = OpenModals::default();
        stack.0.push("outer".into());
        stack.0.push("inner".into());
        assert_eq!(stack.0.last().map(|id| id.as_ref()), Some("inner"));
        stack.0.retain(|held| held.as_ref() != "inner");
        assert_eq!(stack.0.last().map(|id| id.as_ref()), Some("outer"));
    }
}
