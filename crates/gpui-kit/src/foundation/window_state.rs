//! Transient Kit state scoped to one GPUI window.
//!
//! `RenderOnce` builders need somewhere to retain measurements, scroll
//! handles, and motion between frames. The application owns that storage, but
//! a component identity is only unique inside its window. This helper keeps
//! every registry under [`WindowId`], ages keyed entries by semantic frame,
//! and removes the whole window entry when GPUI closes it.

use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{App, Global, SharedString, WindowId};
use gpui_kit_semantics::SemanticCoordinator;

/// How many generations an untouched key survives.
///
/// One generation of slack prevents a frame boundary between two consecutive
/// renders from looking like an unmount. An entry last seen in generation N is
/// therefore removed when another key of the same state type is touched in
/// generation N + 2.
const KEY_GRACE: u64 = 2;

struct WindowStates<T>(RefCell<HashMap<WindowId, T>>);

impl<T> Default for WindowStates<T> {
    fn default() -> Self {
        Self(RefCell::new(HashMap::new()))
    }
}

impl<T: 'static> Global for WindowStates<T> {}

struct KeyedEntry<T> {
    seen: u64,
    grace: u64,
    value: T,
}

struct KeyedStates<T>(HashMap<SharedString, KeyedEntry<T>>);

impl<T> Default for KeyedStates<T> {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

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

/// Mutates one identity's state and marks it live in this frame.
///
/// Pruning happens before the key is touched. It is deliberately lazy: if no
/// identity of a state type is used again, its backing allocation remains
/// dormant until that window closes; the next use discards all expired keys
/// before exposing state.
pub(crate) fn with_key<T: Default + 'static, R>(
    id: &SharedString,
    window_id: WindowId,
    cx: &mut App,
    update: impl FnOnce(&mut T) -> R,
) -> R {
    with_key_retained(id, KEY_GRACE, window_id, cx, update)
}

/// The keyed state operation with a caller-selected bounded handoff grace.
pub(crate) fn with_key_retained<T: Default + 'static, R>(
    id: &SharedString,
    grace: u64,
    window_id: WindowId,
    cx: &mut App,
    update: impl FnOnce(&mut T) -> R,
) -> R {
    let generation = generation(window_id, cx);
    with(window_id, cx, |states: &mut KeyedStates<T>| {
        states
            .0
            .retain(|_, entry| generation.saturating_sub(entry.seen) < entry.grace);
        let entry = states.0.entry(id.clone()).or_insert_with(|| KeyedEntry {
            seen: generation,
            grace,
            value: T::default(),
        });
        entry.seen = generation;
        entry.grace = grace;
        update(&mut entry.value)
    })
}

/// Reads one identity without reviving or creating it.
pub(crate) fn read_key<T: 'static, R>(
    id: &SharedString,
    window_id: WindowId,
    cx: &App,
    read: impl FnOnce(&T) -> R,
) -> Option<R> {
    self::read(window_id, cx, |states: &KeyedStates<T>| {
        states.0.get(id).map(|entry| read(&entry.value))
    })
    .flatten()
}

pub(crate) fn keyed_ids<T: 'static>(window_id: WindowId, cx: &App) -> Vec<SharedString> {
    self::read(window_id, cx, |states: &KeyedStates<T>| {
        let mut ids: Vec<_> = states.0.keys().cloned().collect();
        ids.sort();
        ids
    })
    .unwrap_or_default()
}

fn generation(window_id: WindowId, cx: &App) -> u64 {
    SemanticCoordinator::try_global(cx)
        .and_then(|coordinator| coordinator.generation(window_id))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use gpui::{
        AnyWindowHandle, AppContext, Context, IntoElement, Render, TestAppContext, Window, div,
    };

    use super::*;

    struct Fixture;

    #[derive(Default)]
    struct Remembered(usize);

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
            let key = SharedString::new_static("keyed");
            with_key(&key, left.window_id(), cx, |state: &mut Remembered| {
                state.0 = 3
            });
            with_key(&key, right.window_id(), cx, |state: &mut Remembered| {
                state.0 = 4
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
            assert_eq!(
                read_key(&key, left.window_id(), cx, |state: &Remembered| state.0),
                Some(3)
            );
            assert_eq!(
                read_key(&key, right.window_id(), cx, |state: &Remembered| state.0),
                Some(4)
            );
        });

        cx.update_window(left, |_, window, _| window.remove_window())
            .expect("left window");
        cx.run_until_parked();
        cx.update(|cx| {
            let key = SharedString::new_static("keyed");
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
            assert_eq!(
                read_key(&key, left.window_id(), cx, |state: &Remembered| state.0),
                None
            );
            assert_eq!(
                read_key(&key, right.window_id(), cx, |state: &Remembered| state.0),
                Some(4)
            );
        });
    }

    #[gpui::test]
    fn keyed_state_keeps_live_ids_and_reclaims_missing_ids(cx: &mut TestAppContext) {
        let window_id = WindowId::from(1);
        let kept = SharedString::new_static("kept");
        let removed = SharedString::new_static("removed");

        cx.update(|cx| {
            gpui_kit_semantics::install(cx);
            let coordinator = SemanticCoordinator::global(cx);
            coordinator.begin_window_frame(window_id);
            with_key(&kept, window_id, cx, |state: &mut Remembered| state.0 = 1);
            with_key(&removed, window_id, cx, |state: &mut Remembered| {
                state.0 = 2
            });

            coordinator.begin_window_frame(window_id);
            with_key(&kept, window_id, cx, |state: &mut Remembered| {
                assert_eq!(state.0, 1)
            });
            assert_eq!(
                keyed_ids::<Remembered>(window_id, cx),
                vec![kept.clone(), removed.clone()]
            );

            coordinator.begin_window_frame(window_id);
            with_key(&kept, window_id, cx, |state: &mut Remembered| {
                assert_eq!(state.0, 1)
            });
            assert_eq!(keyed_ids::<Remembered>(window_id, cx), vec![kept]);

            with_key(&removed, window_id, cx, |state: &mut Remembered| {
                assert_eq!(state.0, 0, "an expired identity returns with fresh state")
            });
        });
    }
}
