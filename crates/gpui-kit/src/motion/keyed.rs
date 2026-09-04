//! Per-identity animation state for builders that are rebuilt every frame.
//!
//! A `RenderOnce` builder cannot carry anything across frames, so motion that
//! has to remember where an element was, or what a number used to read, keeps
//! one cell per semantic id in an application global — the same arrangement
//! [`crate::layout::measure`] uses for measurements.
//!
//! Entries are dropped once their id stops rendering, after a grace the caller
//! chooses: two frames for anything that renders every frame, longer for an
//! identity handed from one element tree to another. The frame counter is the
//! semantic registry's generation, which a host bumps at the top of every root
//! render; a host that installed no registry keeps its entries, because there
//! is then no frame boundary to count.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::foundation::window_state;
use gpui::{App, SharedString, WindowId};

/// How many frames an untouched entry survives.
///
/// One frame of slack, so an element that renders on every frame is never
/// dropped by a frame boundary that falls between two of its own renders.
const GRACE: u64 = 2;

/// The cell holding `id`'s state, creating it on first use.
///
/// Every call also drops the entries whose ids have stopped rendering, so the
/// global tracks the elements on screen rather than every element ever shown.
pub(crate) fn slot<T: Default + 'static>(
    id: &SharedString,
    window_id: WindowId,
    cx: &mut App,
) -> Rc<RefCell<T>> {
    slot_retained(id, GRACE, window_id, cx)
}

/// The same, for an id that has to survive a gap in which nothing renders it.
///
/// `grace` is bounded in frames rather than open-ended, so state kept for a
/// handoff between two trees is still dropped once the handoff plainly is not
/// happening.
pub(crate) fn slot_retained<T: Default + 'static>(
    id: &SharedString,
    grace: u64,
    window_id: WindowId,
    cx: &mut App,
) -> Rc<RefCell<T>> {
    window_state::with_key_retained(id, grace, window_id, cx, |value: &mut Rc<RefCell<T>>| {
        Rc::clone(value)
    })
}

/// One container's worth of per-entry state.
struct Scope<T> {
    entries: HashMap<SharedString, Rc<RefCell<T>>>,
}

impl<T> Default for Scope<T> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

/// State belonging to one entry of a container, which outlives the element.
///
/// [`slot`] ties state to the thing that renders it, and for most motion that
/// is right: an element that stops rendering has stopped animating. It is
/// wrong for anything inside a virtualized surface, because there a row that
/// scrolled out of view stops rendering while very much still existing. Under
/// [`slot`] its state is dropped, and when the reader scrolls back the row is
/// a new element with nothing behind it: an entrance replays, a fade that had
/// settled dissolves again, a fold that was open reads as never opened.
///
/// So this measures life against the *container* instead. The container
/// renders every frame it is on screen and says which entries it still holds
/// through [`retain_scope`]; an entry is dropped when the container stops
/// naming it, and the whole scope is dropped once the container itself stops
/// rendering. Scrolling is then not an event this state can observe, which is
/// the point.
pub(crate) fn scoped_slot<T: Default + 'static>(
    container: &SharedString,
    entry: &SharedString,
    window_id: WindowId,
    cx: &mut App,
) -> Rc<RefCell<T>> {
    window_state::with_key(container, window_id, cx, |scope: &mut Scope<T>| {
        Rc::clone(
            scope
                .entries
                .entry(entry.clone())
                .or_insert_with(|| Rc::new(RefCell::new(T::default()))),
        )
    })
}

/// Declares which entries of `container` still exist.
///
/// This is the half a virtualized container has to supply, because it is the
/// only party that knows: the rows themselves cannot report their own removal,
/// having stopped rendering either way. Called once per frame with the same
/// identities the rows are drawn under, it drops the state of entries that are
/// genuinely gone while leaving the state of entries that merely scrolled
/// away.
pub(crate) fn retain_scope<T: 'static>(
    container: &SharedString,
    live: &[SharedString],
    window_id: WindowId,
    cx: &mut App,
) {
    window_state::with_key(container, window_id, cx, |scope: &mut Scope<T>| {
        scope.entries.retain(|id, _| live.contains(id));
    });
}

/// One entry's state, without claiming the entry rendered.
///
/// [`scoped_slot`] is a render-time call: it says an entry is here, and
/// creates its state if it was not. This is for a reader asking after the
/// fact — a caller that wants to know what an entry's state says without
/// bringing that entry back into existence, and which has only `&App` to ask
/// with.
pub(crate) fn scoped_peek<T: 'static>(
    container: &SharedString,
    entry: &SharedString,
    window_id: WindowId,
    cx: &App,
) -> Option<Rc<RefCell<T>>> {
    window_state::read_key(container, window_id, cx, |scope: &Scope<T>| {
        scope.entries.get(entry).map(Rc::clone)
    })
    .flatten()
}

/// The entries currently held for one container.
///
/// Retention is the whole point of this module and is invisible from the
/// outside, so it is asserted on directly rather than inferred from what an
/// element happened to draw.
#[cfg(test)]
pub(crate) fn scoped_ids<T: 'static>(
    container: &SharedString,
    window_id: WindowId,
    cx: &App,
) -> Vec<SharedString> {
    window_state::read_key(container, window_id, cx, |scope: &Scope<T>| {
        let mut ids: Vec<SharedString> = scope.entries.keys().cloned().collect();
        ids.sort();
        ids
    })
    .unwrap_or_default()
}

/// The ids currently retained, for diagnostics and tests.
pub(crate) fn ids<T: 'static>(window_id: WindowId, cx: &App) -> Vec<SharedString> {
    window_state::keyed_ids::<Rc<RefCell<T>>>(window_id, cx)
}

/// The current frame, or `None` where the host installed no semantic registry
/// and there is therefore no frame boundary to count.
pub(crate) fn frame_counter(window_id: WindowId, cx: &App) -> Option<u64> {
    gpui_kit_semantics::SemanticCoordinator::try_global(cx)
        .and_then(|coordinator| coordinator.generation(window_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::WindowId;
    use gpui_kit_semantics::SemanticCoordinator;

    /// The state a virtualized row would keep: something that must not start
    /// over when the row comes back.
    #[derive(Default, PartialEq, Debug)]
    struct Settled(bool);

    fn container() -> SharedString {
        SharedString::new_static("transcript")
    }

    fn row(id: &str) -> SharedString {
        SharedString::from(id.to_string())
    }

    fn window_id() -> WindowId {
        WindowId::from(1)
    }

    /// A host that counts frames, which is what gives this state a clock.
    fn install(cx: &mut App) {
        gpui_kit_semantics::install(cx);
    }

    /// Ends the frame the way a host does at the top of a root render.
    fn next_frame(cx: &mut App) {
        SemanticCoordinator::global(cx).begin_window_frame(window_id());
    }

    fn mark(container: &SharedString, id: &str, cx: &mut App) {
        scoped_slot::<Settled>(container, &row(id), window_id(), cx)
            .borrow_mut()
            .0 = true;
    }

    fn settled(container: &SharedString, id: &str, cx: &mut App) -> bool {
        scoped_slot::<Settled>(container, &row(id), window_id(), cx)
            .borrow()
            .0
    }

    #[gpui::test]
    fn a_row_that_scrolled_away_keeps_what_it_had(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            install(cx);
            let container = container();
            let live = [row("a"), row("b"), row("c")];

            retain_scope::<Settled>(&container, &live, window_id(), cx);
            mark(&container, "b", cx);
            assert!(settled(&container, "b", cx));

            // Frames pass in which the row is not rendered at all, because it
            // is outside the viewport. The container is still on screen and
            // still holds the row.
            for _ in 0..10 {
                next_frame(cx);
                retain_scope::<Settled>(&container, &live, window_id(), cx);
            }

            assert!(
                settled(&container, "b", cx),
                "a row scrolled out of view must come back settled, not new"
            );
        });
    }

    #[gpui::test]
    fn a_row_the_container_stopped_holding_is_dropped(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            install(cx);
            let container = container();
            retain_scope::<Settled>(&container, &[row("a"), row("b")], window_id(), cx);
            mark(&container, "a", cx);
            mark(&container, "b", cx);

            next_frame(cx);
            retain_scope::<Settled>(&container, &[row("a")], window_id(), cx);

            assert_eq!(
                scoped_ids::<Settled>(&container, window_id(), cx),
                vec![row("a")]
            );
            assert!(settled(&container, "a", cx), "the kept row is untouched");
        });
    }

    #[gpui::test]
    fn a_container_that_stopped_rendering_takes_its_rows_with_it(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            install(cx);
            let container = container();
            retain_scope::<Settled>(&container, &[row("a")], window_id(), cx);
            mark(&container, "a", cx);

            // The surface is unmounted: a session was switched away from, a
            // tab was closed. Nothing names the container any more.
            for _ in 0..GRACE + 1 {
                next_frame(cx);
            }
            // Any call prunes, so ask about a different container.
            retain_scope::<Settled>(&SharedString::new_static("elsewhere"), &[], window_id(), cx);

            assert!(
                scoped_ids::<Settled>(&container, window_id(), cx).is_empty(),
                "an unmounted container must not keep its rows alive forever"
            );
        });
    }

    #[gpui::test]
    fn two_containers_do_not_share_a_row_identity(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            install(cx);
            // The same row id in two conversations is two rows. Keying by the
            // row alone would let one settle the other.
            let left = SharedString::new_static("left");
            let right = SharedString::new_static("right");
            retain_scope::<Settled>(&left, &[row("shared")], window_id(), cx);
            retain_scope::<Settled>(&right, &[row("shared")], window_id(), cx);
            mark(&left, "shared", cx);

            assert!(settled(&left, "shared", cx));
            assert!(!settled(&right, "shared", cx));
        });
    }

    #[gpui::test]
    fn two_windows_do_not_share_an_animation_identity(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let left = WindowId::from(1);
            let right = WindowId::from(2);
            let id = row("shared");

            slot::<Settled>(&id, left, cx).borrow_mut().0 = true;

            assert!(slot::<Settled>(&id, left, cx).borrow().0);
            assert!(!slot::<Settled>(&id, right, cx).borrow().0);
            assert_eq!(ids::<Settled>(left, cx), vec![id.clone()]);
            assert_eq!(ids::<Settled>(right, cx), vec![id]);
        });
    }

    #[gpui::test]
    fn a_frame_boundary_between_two_renders_is_not_a_removal(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            install(cx);
            // The grace exists because a container's own render can straddle a
            // frame boundary. One frame of silence must not drop anything.
            let container = container();
            retain_scope::<Settled>(&container, &[row("a")], window_id(), cx);
            mark(&container, "a", cx);
            next_frame(cx);
            assert!(settled(&container, "a", cx));
        });
    }
}
