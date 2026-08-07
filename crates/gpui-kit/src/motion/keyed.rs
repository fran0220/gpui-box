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

use gpui::{App, Global, SharedString};
use gpui_kit_semantics::SemanticRegistry;

/// How many frames an untouched entry survives.
///
/// One frame of slack, so an element that renders on every frame is never
/// dropped by a frame boundary that falls between two of its own renders.
const GRACE: u64 = 2;

struct Entry<T> {
    seen: u64,
    /// How many frames this entry survives, which the caller that touched it
    /// last decides: a handoff between two element trees needs longer than an
    /// element that renders on every frame.
    grace: u64,
    value: Rc<RefCell<T>>,
}

struct Keyed<T>(RefCell<HashMap<SharedString, Entry<T>>>);

impl<T> Default for Keyed<T> {
    fn default() -> Self {
        Self(RefCell::new(HashMap::new()))
    }
}

impl<T: 'static> Global for Keyed<T> {}

/// The cell holding `id`'s state, creating it on first use.
///
/// Every call also drops the entries whose ids have stopped rendering, so the
/// global tracks the elements on screen rather than every element ever shown.
pub(crate) fn slot<T: Default + 'static>(id: &SharedString, cx: &mut App) -> Rc<RefCell<T>> {
    slot_retained(id, GRACE, cx)
}

/// The same, for an id that has to survive a gap in which nothing renders it.
///
/// `grace` is bounded in frames rather than open-ended, so state kept for a
/// handoff between two trees is still dropped once the handoff plainly is not
/// happening.
pub(crate) fn slot_retained<T: Default + 'static>(
    id: &SharedString,
    grace: u64,
    cx: &mut App,
) -> Rc<RefCell<T>> {
    let frame = frame(cx);
    if !cx.has_global::<Keyed<T>>() {
        cx.set_global(Keyed::<T>::default());
    }
    let mut entries = cx.global::<Keyed<T>>().0.borrow_mut();
    entries.retain(|_, entry| frame.saturating_sub(entry.seen) < entry.grace);
    let entry = entries.entry(id.clone()).or_insert_with(|| Entry {
        seen: frame,
        grace,
        value: Rc::new(RefCell::new(T::default())),
    });
    entry.seen = frame;
    entry.grace = grace;
    Rc::clone(&entry.value)
}

/// The ids currently retained, for diagnostics and tests.
pub(crate) fn ids<T: 'static>(cx: &App) -> Vec<SharedString> {
    let Some(keyed) = cx.try_global::<Keyed<T>>() else {
        return Vec::new();
    };
    let mut ids: Vec<SharedString> = keyed.0.borrow().keys().cloned().collect();
    ids.sort();
    ids
}

fn frame(cx: &App) -> u64 {
    frame_counter(cx).unwrap_or_default()
}

/// The current frame, or `None` where the host installed no semantic registry
/// and there is therefore no frame boundary to count.
pub(crate) fn frame_counter(cx: &App) -> Option<u64> {
    SemanticRegistry::try_global(cx).map(|registry| registry.generation())
}
