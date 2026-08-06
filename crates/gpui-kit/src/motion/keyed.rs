//! Per-identity animation state for builders that are rebuilt every frame.
//!
//! A `RenderOnce` builder cannot carry anything across frames, so motion that
//! has to remember where an element was, or what a number used to read, keeps
//! one cell per semantic id in an application global — the same arrangement
//! [`crate::layout::measure`] uses for measurements.
//!
//! Entries are dropped once their id stops rendering. The frame counter is the
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
    let frame = frame(cx);
    if !cx.has_global::<Keyed<T>>() {
        cx.set_global(Keyed::<T>::default());
    }
    let mut entries = cx.global::<Keyed<T>>().0.borrow_mut();
    entries.retain(|_, entry| frame.saturating_sub(entry.seen) < GRACE);
    let entry = entries.entry(id.clone()).or_insert_with(|| Entry {
        seen: frame,
        value: Rc::new(RefCell::new(T::default())),
    });
    entry.seen = frame;
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
    SemanticRegistry::try_global(cx)
        .map(|registry| registry.generation())
        .unwrap_or_default()
}
