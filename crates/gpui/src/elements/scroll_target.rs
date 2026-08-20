//! One way to ask anything scrollable where it is.
//!
//! A scrollbar, a host restoring a reading position, and a component that
//! fades an edge as content moves under it all need the same four facts, and
//! they need them from an overflowing container, from a uniform list, and from
//! a variable-height list alike. Each of those grew its own pair of methods
//! named after whoever first called them — `offset` and `max_offset` on one,
//! `scroll_px_offset_for_scrollbar` and `max_offset_for_scrollbar` on another
//! — so a component that drew a scrollbar could only be attached to the kind
//! of scrolling its author happened to have in front of them. Binding it to
//! the other kind meant writing the component again.
//!
//! [`ScrollTarget`] is those four facts, named once.
//!
//! # The sign
//!
//! An offset is the distance from the top left of the viewport to the top left
//! of the content, so it is zero at the start and grows *negative* as the
//! reader moves down and to the right. That is the convention
//! [`ScrollHandle`] already used, and every implementation here reports the
//! same one, because a trait whose implementations disagreed about the
//! direction of travel would be worse than no trait.
//!
//! # Dragging
//!
//! A list that measures its rows as they come into view learns that it is
//! taller than it thought while the reader is dragging its thumb, which moves
//! the thumb out from under the pointer. [`ScrollTarget::set_scroll_dragging`]
//! lets such a list hold its reported height still for the length of a drag.
//! A target with nothing to hold still ignores it, which is why it has a
//! default.

use crate::{Bounds, ListState, Pixels, Point, ScrollHandle, UniformListScrollHandle, point};

/// Something that scrolls, and can say where it is and how much further it
/// could go.
pub trait ScrollTarget {
    /// The distance from the viewport to the start of the content: zero at the
    /// start, negative once the reader has moved down or to the right.
    fn scroll_offset(&self) -> Point<Pixels>;

    /// How much further the content reaches beyond the viewport, in each axis.
    /// Zero on an axis means there is nothing more that way.
    fn max_scroll_offset(&self) -> Point<Pixels>;

    /// The region the content is seen through.
    ///
    /// A target that has not been laid out yet reports a zero-sized bounds,
    /// which is the honest answer: it does not know, and it is not scrolled.
    fn scroll_viewport(&self) -> Bounds<Pixels>;

    /// Puts the content at `offset`, in the sign [`Self::scroll_offset`]
    /// reports.
    fn set_scroll_offset(&self, offset: Point<Pixels>);

    /// Notes that a scrollbar drag has begun or ended.
    ///
    /// A target that measures lazily holds the height it reports still while
    /// this is true, so the thumb stays under the pointer that is dragging it.
    fn set_scroll_dragging(&self, _dragging: bool) {}
}

impl ScrollTarget for ScrollHandle {
    fn scroll_offset(&self) -> Point<Pixels> {
        self.offset()
    }

    fn max_scroll_offset(&self) -> Point<Pixels> {
        self.max_offset()
    }

    fn scroll_viewport(&self) -> Bounds<Pixels> {
        self.bounds()
    }

    fn set_scroll_offset(&self, offset: Point<Pixels>) {
        self.set_offset(offset);
    }
}

impl ScrollTarget for UniformListScrollHandle {
    fn scroll_offset(&self) -> Point<Pixels> {
        self.0.borrow().base_handle.offset()
    }

    fn max_scroll_offset(&self) -> Point<Pixels> {
        self.0.borrow().base_handle.max_offset()
    }

    fn scroll_viewport(&self) -> Bounds<Pixels> {
        self.0.borrow().base_handle.bounds()
    }

    fn set_scroll_offset(&self, offset: Point<Pixels>) {
        self.0.borrow().base_handle.set_offset(offset);
    }
}

impl ScrollTarget for ListState {
    fn scroll_offset(&self) -> Point<Pixels> {
        self.scroll_px_offset_for_scrollbar()
    }

    fn max_scroll_offset(&self) -> Point<Pixels> {
        self.max_offset_for_scrollbar()
    }

    fn scroll_viewport(&self) -> Bounds<Pixels> {
        self.viewport_bounds()
    }

    fn set_scroll_offset(&self, offset: Point<Pixels>) {
        self.set_offset_from_scrollbar(offset);
    }

    fn set_scroll_dragging(&self, dragging: bool) {
        if dragging {
            self.scrollbar_drag_started();
        } else {
            self.scrollbar_drag_ended();
        }
    }
}

/// How far along a target is, from `0.0` at the start to `1.0` at the end.
///
/// A target with nothing to scroll is at the start, not at the end: there is
/// no content below, but the reader has not gone anywhere either.
pub fn scroll_progress(target: &dyn ScrollTarget) -> Point<f32> {
    let offset = target.scroll_offset();
    let max = target.max_scroll_offset();
    let along = |offset: Pixels, max: Pixels| {
        if max <= Pixels::ZERO {
            0.0
        } else {
            (-f32::from(offset) / f32::from(max)).clamp(0.0, 1.0)
        }
    };
    point(along(offset.x, max.x), along(offset.y, max.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ListAlignment, px, size};

    #[test]
    fn a_container_reports_where_it_is_in_the_sign_the_trait_names() {
        let handle = ScrollHandle::new();
        handle.set_offset(point(px(0.0), px(-120.0)));

        let target: &dyn ScrollTarget = &handle;
        assert_eq!(target.scroll_offset().y, px(-120.0));
    }

    #[test]
    fn a_uniform_list_answers_for_the_container_underneath_it() {
        let handle = UniformListScrollHandle::new();
        handle.set_scroll_offset(point(px(0.0), px(-40.0)));

        assert_eq!(handle.scroll_offset().y, px(-40.0));
        assert_eq!(
            handle.0.borrow().base_handle.offset().y,
            px(-40.0),
            "the uniform list scrolls the container it is drawn in"
        );
    }

    #[test]
    fn a_list_that_has_never_been_laid_out_is_not_scrolled() {
        let state = ListState::new(100, ListAlignment::Top, px(0.0));
        let target: &dyn ScrollTarget = &state;

        assert_eq!(target.scroll_offset(), point(px(0.0), px(0.0)));
        assert_eq!(target.scroll_viewport().size, size(px(0.0), px(0.0)));
    }

    #[test]
    fn nothing_to_scroll_is_the_start_rather_than_the_end() {
        let handle = ScrollHandle::new();
        assert_eq!(scroll_progress(&handle), point(0.0, 0.0));
    }

    #[test]
    fn progress_runs_from_the_start_to_the_end_of_what_there_is() {
        let handle = ScrollHandle::new();
        handle.set_measured_for_test(
            Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(100.0), px(100.0)),
            },
            point(px(0.0), px(400.0)),
        );

        handle.set_offset(point(px(0.0), px(-100.0)));
        assert_eq!(scroll_progress(&handle).y, 0.25);

        handle.set_offset(point(px(0.0), px(-400.0)));
        assert_eq!(scroll_progress(&handle).y, 1.0);

        handle.set_offset(point(px(0.0), px(-4000.0)));
        assert_eq!(
            scroll_progress(&handle).y,
            1.0,
            "past the end is still the end"
        );
    }
}
