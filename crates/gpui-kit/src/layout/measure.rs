//! Bounds a layout component measured on the previous frame.
//!
//! A `RenderOnce` builder is rebuilt every frame and cannot carry anything, but
//! a split divider and a scrollbar both need the extent GPUI actually produced
//! before they can publish a truthful range. Keying one cell by identity keeps
//! the measurement across rebuilds, and a measurement that changed asks for the
//! frame that publishes it.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{App, Bounds, Global, Pixels, SharedString, Window};

/// How much a measurement has to move before it is worth another frame.
///
/// Sub-pixel drift would otherwise ask for a redraw on every frame forever.
const EPSILON: f32 = 0.5;

#[derive(Default)]
struct Measurements(RefCell<HashMap<SharedString, Rc<Cell<Bounds<Pixels>>>>>);

impl Global for Measurements {}

/// The cell holding the bounds last measured for `id`.
pub(crate) fn cell(id: &SharedString, cx: &mut App) -> Rc<Cell<Bounds<Pixels>>> {
    if !cx.has_global::<Measurements>() {
        cx.set_global(Measurements::default());
    }
    let mut cells = cx.global::<Measurements>().0.borrow_mut();
    Rc::clone(cells.entry(id.clone()).or_default())
}

/// Records `bounds` and asks for another frame when they moved, because the
/// frame that measured them has already published the older ones.
///
/// The request is an animation frame rather than a refresh: measuring happens
/// during prepaint, and a window that is drawing ignores a refresh.
pub(crate) fn record(cell: &Rc<Cell<Bounds<Pixels>>>, bounds: Bounds<Pixels>, window: &mut Window) {
    let previous = cell.get();
    let moved = |a: Pixels, b: Pixels| (f32::from(a) - f32::from(b)).abs() > EPSILON;
    if moved(previous.size.width, bounds.size.width)
        || moved(previous.size.height, bounds.size.height)
        || moved(previous.origin.x, bounds.origin.x)
        || moved(previous.origin.y, bounds.origin.y)
    {
        cell.set(bounds);
        window.request_animation_frame();
    }
}
