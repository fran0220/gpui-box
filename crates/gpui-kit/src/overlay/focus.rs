//! Keeping keyboard focus inside an open overlay, and giving it back on close.
//!
//! GPUI's tab order is window-wide. A modal that used it would let Tab walk
//! out of the dialog and into the content behind the scrim, which the user
//! cannot see or click, so an overlay tracks its own stops instead.

use gpui::{App, FocusHandle, Window};

/// The focus stops of one open overlay, in tab order.
///
/// Content re-registers its stops on every render, because the set changes
/// with the content itself.
#[derive(Debug, Clone, Default)]
pub struct FocusTrap {
    stops: Vec<FocusHandle>,
    restore: Option<FocusHandle>,
    engaged: bool,
}

impl FocusTrap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_engaged(&self) -> bool {
        self.engaged
    }

    pub fn stops(&self) -> &[FocusHandle] {
        &self.stops
    }

    /// Records what was focused before the overlay opened, so the same element
    /// can be focused again when it closes.
    pub fn engage(&mut self, window: &Window, cx: &App) {
        if self.engaged {
            return;
        }
        self.restore = window.focused(cx);
        self.engaged = true;
    }

    /// Restores the previously focused element. Nothing is focused when the
    /// element that had focus is gone, rather than guessing at a replacement.
    pub fn release(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(handle) = self.restore.take() {
            handle.focus(window, cx);
        }
        self.stops.clear();
        self.engaged = false;
    }

    /// Clears the registered stops before a render republishes them.
    pub fn begin_frame(&mut self) {
        self.stops.clear();
    }

    pub fn register(&mut self, handle: FocusHandle) {
        if !self.stops.iter().any(|stop| stop == &handle) {
            self.stops.push(handle);
        }
    }

    /// Focuses the first stop, which is what opening an overlay should do.
    pub fn focus_first(&self, window: &mut Window, cx: &mut App) {
        if let Some(handle) = self.stops.first() {
            handle.focus(window, cx);
        }
    }

    pub fn focus_next(&self, window: &mut Window, cx: &mut App) {
        self.step(1, window, cx);
    }

    pub fn focus_prev(&self, window: &mut Window, cx: &mut App) {
        self.step(-1, window, cx);
    }

    /// True while focus is inside the overlay.
    pub fn contains_focus(&self, window: &Window, cx: &App) -> bool {
        window
            .focused(cx)
            .is_some_and(|focused| self.stops.contains(&focused))
    }

    fn step(&self, delta: isize, window: &mut Window, cx: &mut App) {
        if self.stops.is_empty() {
            return;
        }
        let current = window
            .focused(cx)
            .and_then(|focused| self.stops.iter().position(|stop| stop == &focused));
        let next = match current {
            // Focus entering from outside starts at the appropriate end rather
            // than jumping to the middle of the overlay.
            None if delta >= 0 => 0,
            None => self.stops.len() - 1,
            Some(index) => (index as isize + delta).rem_euclid(self.stops.len() as isize) as usize,
        };
        self.stops[next].focus(window, cx);
    }
}
