//! Where a virtualized surface is scrolled to.
//!
//! A `RenderOnce` builder is rebuilt every frame and cannot carry anything, so
//! a list, a table, or a tree that only draws its viewport has nowhere of its
//! own to keep the offset. Keying one scroll handle by the surface's identity
//! keeps the position across rebuilds without making every caller own a GPUI
//! handle, and it lets a surface built on top of another one move it by name.

use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{
    App, Global, ListAlignment, ListState, Pixels, ScrollStrategy, SharedString,
    UniformListScrollHandle, px,
};

use crate::foundation::Ident;

/// How far past the viewport a variable-height list lays rows out, so that a
/// row is measured before it is scrolled into view rather than popping in at
/// its estimated height and then jumping to its real one.
const OVERDRAW: f32 = 240.0;

#[derive(Default)]
struct ScrollHandles(RefCell<HashMap<SharedString, UniformListScrollHandle>>);

impl Global for ScrollHandles {}

/// The rows a variable-height surface has measured, and where it is scrolled.
///
/// A uniform list needs no such thing: every row is the same height, so it
/// knows where row ten thousand starts without laying one out. A list whose
/// rows are as tall as their content only learns a row's height by laying it
/// out, so what it has learned has to survive the rebuild.
#[derive(Default)]
struct FlowStates(RefCell<HashMap<SharedString, (ListState, usize)>>);

impl Global for FlowStates {}

/// The scroll position of the surface with this identity.
pub(crate) fn scroll_handle(ident: &Ident, cx: &mut App) -> UniformListScrollHandle {
    if !cx.has_global::<ScrollHandles>() {
        cx.set_global(ScrollHandles::default());
    }
    let mut handles = cx.global::<ScrollHandles>().0.borrow_mut();
    handles.entry(ident.semantic_id()).or_default().clone()
}

/// The measured rows of the variable-height surface with this identity.
///
/// `estimate` is what an unmeasured row is assumed to be, so a scrollbar is
/// roughly the right size on the first frame and settles as rows are actually
/// laid out, instead of starting as a full-height thumb that shrinks.
///
/// A count that grew is taken to mean rows arrived at the end, which is what a
/// conversation and a log do, and the measurements of the rows already there
/// are kept. Any other change to the count discards them, because they
/// described rows that are no longer at those indices.
pub(crate) fn list_state(
    ident: &Ident,
    count: usize,
    alignment: ListAlignment,
    estimate: Pixels,
    cx: &mut App,
) -> ListState {
    if !cx.has_global::<FlowStates>() {
        cx.set_global(FlowStates::default());
    }
    let mut states = cx.global::<FlowStates>().0.borrow_mut();
    let entry = states.entry(ident.semantic_id()).or_insert_with(|| {
        (
            ListState::new(count, alignment, px(OVERDRAW)).with_uniform_item_height(estimate),
            count,
        )
    });
    let (state, known) = entry;
    if *known != count {
        if count > *known {
            state.splice(*known..*known, count - *known);
        } else {
            state.reset_with_uniform_height(count, estimate);
        }
        *known = count;
    }
    state.clone()
}

/// Brings row `index` of the surface with this identity to the bottom edge.
///
/// Scroll position belongs to the surface, not to whoever draws over it, so a
/// surface built on a list — a conversation that follows its newest message —
/// moves it by naming the list rather than by owning a GPUI handle of its own.
pub fn scroll_to_row(ident: &Ident, index: usize, cx: &mut App) {
    if let Some(state) = flow_state(ident, cx) {
        state.scroll_to_reveal_item(index);
        return;
    }
    scroll_handle(ident, cx).scroll_to_item(index, ScrollStrategy::Bottom);
}

/// Brings row `index` into view by the shortest move that gets it there, and
/// leaves the offset alone when the row is already on screen.
pub fn reveal_row(ident: &Ident, index: usize, cx: &mut App) {
    if let Some(state) = flow_state(ident, cx) {
        state.scroll_to_reveal_item(index);
        return;
    }
    scroll_handle(ident, cx).scroll_to_item(index, ScrollStrategy::Nearest);
}

/// The variable-height state of this surface, if it has one.
///
/// A surface has one only once it has rendered as a variable-height list, so
/// moving a list by name reaches whichever kind it turned out to be rather
/// than moving a uniform handle nothing is drawn from.
fn flow_state(ident: &Ident, cx: &mut App) -> Option<ListState> {
    if !cx.has_global::<FlowStates>() {
        return None;
    }
    let states = cx.global::<FlowStates>().0.borrow();
    states
        .get(&ident.semantic_id())
        .map(|(state, _)| state.clone())
}
