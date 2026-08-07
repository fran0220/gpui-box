//! Where a virtualized surface is scrolled to.
//!
//! A `RenderOnce` builder is rebuilt every frame and cannot carry anything, so
//! a list, a table, or a tree that only draws its viewport has nowhere of its
//! own to keep the offset. Keying one scroll handle by the surface's identity
//! keeps the position across rebuilds without making every caller own a GPUI
//! handle, and it lets a surface built on top of another one move it by name.

use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{App, Global, ScrollStrategy, SharedString, UniformListScrollHandle};

use crate::foundation::Ident;

#[derive(Default)]
struct ScrollHandles(RefCell<HashMap<SharedString, UniformListScrollHandle>>);

impl Global for ScrollHandles {}

/// The scroll position of the surface with this identity.
pub(crate) fn scroll_handle(ident: &Ident, cx: &mut App) -> UniformListScrollHandle {
    if !cx.has_global::<ScrollHandles>() {
        cx.set_global(ScrollHandles::default());
    }
    let mut handles = cx.global::<ScrollHandles>().0.borrow_mut();
    handles.entry(ident.semantic_id()).or_default().clone()
}

/// Brings row `index` of the surface with this identity to the bottom edge.
///
/// Scroll position belongs to the surface, not to whoever draws over it, so a
/// surface built on a list — a conversation that follows its newest message —
/// moves it by naming the list rather than by owning a GPUI handle of its own.
pub fn scroll_to_row(ident: &Ident, index: usize, cx: &mut App) {
    scroll_handle(ident, cx).scroll_to_item(index, ScrollStrategy::Bottom);
}

/// Brings row `index` into view by the shortest move that gets it there, and
/// leaves the offset alone when the row is already on screen.
pub fn reveal_row(ident: &Ident, index: usize, cx: &mut App) {
    scroll_handle(ident, cx).scroll_to_item(index, ScrollStrategy::Nearest);
}
