//! Kit hit-region adapter for the framework-owned directional touch recognizer.

use gpui::{App, ElementId, IntoElement, Styled, TouchPanEvent, TouchPhase, Window, canvas};

/// A listener takes no layout space; its parent must establish a relative
/// containing block. The visible content mask is intersected with that block,
/// so a clipped row cannot acquire input outside its measured visible bounds.
pub(crate) fn touch_pan(
    id: ElementId,
    mut listener: impl FnMut(&TouchPanEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    canvas(
        |bounds, window, _| bounds.intersect(&window.content_mask().bounds),
        move |_, bounds, window, _| {
            window.on_touch_pan(id, move |event, window, cx| {
                if event.phase != TouchPhase::Started
                    || bounds.contains(&event.touch_start_position)
                {
                    listener(event, window, cx);
                }
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}
