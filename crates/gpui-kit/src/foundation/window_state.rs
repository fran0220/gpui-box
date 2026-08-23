//! Transient Kit state scoped to one GPUI window.
//!
//! `RenderOnce` builders need somewhere to retain measurements, scroll
//! handles, and motion between frames. The application owns that storage, but
//! a component identity is only unique inside its window. This helper keeps
//! every registry under [`WindowId`] and removes the whole window entry when
//! GPUI closes it.

use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{App, Global, WindowId};

struct WindowStates<T>(RefCell<HashMap<WindowId, T>>);

impl<T> Default for WindowStates<T> {
    fn default() -> Self {
        Self(RefCell::new(HashMap::new()))
    }
}

impl<T: 'static> Global for WindowStates<T> {}

fn install<T: 'static>(cx: &mut App) {
    if cx.has_global::<WindowStates<T>>() {
        return;
    }
    cx.set_global(WindowStates::<T>::default());
    cx.on_window_closed(|cx, window_id| {
        if let Some(states) = cx.try_global::<WindowStates<T>>() {
            states.0.borrow_mut().remove(&window_id);
        }
    })
    .detach();
}

/// Mutates the state belonging to `window_id`, creating its default on first
/// use.
pub(crate) fn with<T: Default + 'static, R>(
    window_id: WindowId,
    cx: &mut App,
    update: impl FnOnce(&mut T) -> R,
) -> R {
    install::<T>(cx);
    let states = cx.global::<WindowStates<T>>();
    let mut states = states.0.borrow_mut();
    update(states.entry(window_id).or_default())
}

/// Reads the state of a known window without creating one.
pub(crate) fn read<T: 'static, R>(
    window_id: WindowId,
    cx: &App,
    read: impl FnOnce(&T) -> R,
) -> Option<R> {
    let states = cx.try_global::<WindowStates<T>>()?;
    let states = states.0.borrow();
    states.get(&window_id).map(read)
}

#[cfg(test)]
mod tests {
    use gpui::{
        AnyWindowHandle, AppContext, Context, IntoElement, Render, TestAppContext, Window, div,
    };

    use super::*;

    struct Fixture;

    impl Render for Fixture {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[gpui::test]
    fn identical_local_keys_are_isolated_and_closed_windows_are_removed(cx: &mut TestAppContext) {
        let left = AnyWindowHandle::from(cx.add_window(|_, _| Fixture));
        let right = AnyWindowHandle::from(cx.add_window(|_, _| Fixture));

        cx.update(|cx| {
            with::<HashMap<&'static str, usize>, _>(left.window_id(), cx, |state| {
                state.insert("shared", 1);
            });
            with::<HashMap<&'static str, usize>, _>(right.window_id(), cx, |state| {
                state.insert("shared", 2);
            });
            assert_eq!(
                read(
                    left.window_id(),
                    cx,
                    |state: &HashMap<&'static str, usize>| state["shared"]
                ),
                Some(1)
            );
            assert_eq!(
                read(
                    right.window_id(),
                    cx,
                    |state: &HashMap<&'static str, usize>| state["shared"]
                ),
                Some(2)
            );
        });

        cx.update_window(left, |_, window, _| window.remove_window())
            .expect("left window");
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                read(left.window_id(), cx, |state: &HashMap<&str, usize>| state
                    .len()),
                None
            );
            assert_eq!(
                read(right.window_id(), cx, |state: &HashMap<&str, usize>| state
                    ["shared"]),
                Some(2)
            );
        });
    }
}
