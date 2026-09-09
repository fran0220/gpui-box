//! Where a virtualized surface is scrolled to.
//!
//! A `RenderOnce` builder is rebuilt every frame and cannot carry anything, so
//! a list, a table, or a tree that only draws its viewport has nowhere of its
//! own to keep the offset. Keying one scroll handle by the surface's identity
//! keeps the position across rebuilds without making every caller own a GPUI
//! handle, and it lets a surface built on top of another one move it by name.

use std::{collections::HashMap, ops::Range, time::Duration};

use crate::foundation::{Ident, window_state};
use crate::motion::{Glide, MotionPolicy, MotionRole};
use gpui::{
    App, ListAlignment, ListOffset, ListState, Pixels, ScrollStrategy, SharedString,
    UniformListScrollHandle, Window, WindowId, px,
};

/// The interval a glide asks for its frames at, near enough to a 60Hz frame.
const FRAME: Duration = Duration::from_millis(16);

/// How far past the viewport a variable-height list lays rows out, so that a
/// row is measured before it is scrolled into view rather than popping in at
/// its estimated height and then jumping to its real one.
const OVERDRAW: f32 = 240.0;

/// What one variable-height surface has learned about itself.
struct Flow {
    state: ListState,
    /// The rows as they were last laid out. A surface that names its rows is
    /// diffed against this; one that only counts them keeps the count here as
    /// a run of anonymous names it can still compare the length of.
    keys: Vec<SharedString>,
    revisions: Vec<u64>,
}

/// How a surface describes the rows it is about to draw.
///
/// Counting them is enough to notice that rows arrived at the end, which is
/// what a log does. It is not enough to notice anything else: a row inserted
/// in the middle, one removed, or one replaced all read as "the count
/// changed", and the only safe answer to that is to forget every height the
/// surface had measured. Naming them says exactly which rows are the same
/// rows, so the measurements either side of a change survive it.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Rows<'a> {
    Counted(usize),
    Keyed(&'a [SharedString]),
}

impl Rows<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Counted(count) => *count,
            Self::Keyed(keys) => keys.len(),
        }
    }
}

/// The scroll position of the surface with this identity.
pub(crate) fn scroll_handle(
    ident: &Ident,
    window: &Window,
    cx: &mut App,
) -> UniformListScrollHandle {
    window_state::with_key(
        &ident.semantic_id(),
        window.window_handle().window_id(),
        cx,
        |handle: &mut UniformListScrollHandle| handle.clone(),
    )
}

/// The measured rows of the variable-height surface with this identity.
///
/// `estimate` is what an unmeasured row is assumed to be, so a scrollbar is
/// roughly the right size on the first frame and settles as rows are actually
/// laid out, instead of starting as a full-height thumb that shrinks.
///
/// Named rows preserve measurements by identity across arbitrary reorder;
/// content revisions invalidate geometry separately. Rows that were only
/// counted keep the older, blunter rule: a count that grew is taken to mean
/// rows arrived at the end, and any other change discards the measurements,
/// because they described rows that are no longer at those indices.
pub(crate) fn list_state(
    ident: &Ident,
    rows: Rows<'_>,
    revisions: Option<&[u64]>,
    alignment: ListAlignment,
    estimate: Pixels,
    window: &Window,
    cx: &mut App,
) -> ListState {
    let count = rows.len();
    window_state::with_key(
        &ident.semantic_id(),
        window.window_handle().window_id(),
        cx,
        |flow: &mut Option<Flow>| {
            let flow = flow.get_or_insert_with(|| Flow {
                state: ListState::new(count, alignment, px(OVERDRAW))
                    .with_uniform_item_height(estimate),
                keys: anonymous(count),
                revisions: vec![0; count],
            });

            match rows {
                Rows::Keyed(keys) => {
                    let revisions = revisions.map_or_else(
                        || vec![0; count],
                        |values| {
                            assert_eq!(values.len(), count, "one revision is required per row");
                            values.to_vec()
                        },
                    );
                    if flow.keys != keys {
                        let old: HashMap<_, _> = flow
                            .keys
                            .iter()
                            .enumerate()
                            .map(|(index, key)| (key, index))
                            .collect();
                        let unique: std::collections::HashSet<_> = keys.iter().collect();
                        assert_eq!(unique.len(), keys.len(), "row keys must be unique");
                        let mapping: Vec<_> =
                            keys.iter().map(|key| old.get(key).copied()).collect();
                        flow.state.remap_items(&mapping);
                        for (index, previous) in mapping.iter().enumerate() {
                            if previous.is_some_and(|old| flow.revisions[old] != revisions[index]) {
                                flow.state.remeasure_items(index..index + 1);
                            }
                        }
                        flow.keys = keys.to_vec();
                    } else {
                        for (index, (old, new)) in flow.revisions.iter().zip(&revisions).enumerate()
                        {
                            if old != new {
                                flow.state.remeasure_items(index..index + 1);
                            }
                        }
                    }
                    flow.revisions = revisions;
                }
                Rows::Counted(count) => {
                    let known = flow.keys.len();
                    if known != count {
                        if count > known {
                            flow.state.splice(known..known, count - known);
                        } else {
                            flow.state.reset_with_uniform_height(count, estimate);
                        }
                        flow.keys = anonymous(count);
                        flow.revisions = vec![0; count];
                    }
                }
            }
            flow.state.clone()
        },
    )
}

/// Stand-in names for a surface that counts its rows instead of naming them.
///
/// They are deliberately equal to each other only at equal indices, so a
/// counted surface that later starts naming its rows is diffed as a wholesale
/// replacement rather than being told nothing changed.
fn anonymous(count: usize) -> Vec<SharedString> {
    (0..count)
        .map(|index| SharedString::from(format!("\u{0}{index}")))
        .collect()
}

/// Invalidates measured geometry after an asynchronous row update. Identity
/// and the absolute pixel offset within the anchored row are retained. The
/// range uses current row order; callers resolving async work must look up
/// its stable key before calling. Missing surfaces are a no-op.
pub fn remeasure_rows(ident: &Ident, rows: Range<usize>, window: &Window, cx: &mut App) {
    if let Some(state) = flow_state(ident, window.window_handle().window_id(), cx) {
        state.remeasure_items(rows);
        cx.refresh_windows();
    }
}

/// Brings row `index` of the surface with this identity to the bottom edge.
///
/// Scroll position belongs to the surface, not to whoever draws over it, so a
/// surface built on a list — a conversation that follows its newest message —
/// moves it by naming the list rather than by owning a GPUI handle of its own.
pub fn scroll_to_row(ident: &Ident, index: usize, window: &Window, cx: &mut App) {
    if let Some(state) = flow_state(ident, window.window_handle().window_id(), cx) {
        state.scroll_to_reveal_item(index);
        return;
    }
    scroll_handle(ident, window, cx).scroll_to_item(index, ScrollStrategy::Bottom);
}

/// Brings row `index` into view by the shortest move that gets it there, and
/// leaves the offset alone when the row is already on screen.
pub fn reveal_row(ident: &Ident, index: usize, window: &Window, cx: &mut App) {
    if let Some(state) = flow_state(ident, window.window_handle().window_id(), cx) {
        state.scroll_to_reveal_item(index);
        return;
    }
    scroll_handle(ident, window, cx).scroll_to_item(index, ScrollStrategy::Nearest);
}

/// Travels to row `index` rather than arriving there.
///
/// A jump across a long conversation destroys the reader's place: the screen
/// they were looking at is replaced by another one, and nothing on it says
/// which direction they came from or how far they went. Moving there over half
/// a second says both, and costs nothing but the half second.
///
/// The distance is not known when the glide starts. Rows above the viewport
/// have never been laid out, so the pixels between here and there can only be
/// estimated, and the estimate is corrected as rows are measured. [`Glide`] is
/// what makes that survivable: each frame consumes the share of the *current*
/// remaining distance that the curve says belongs to it, so a correction
/// mid-flight continues the same timeline instead of restarting it.
///
/// A reader who has asked for reduced motion is taken straight there, and so
/// is a surface that is not a variable-height list: a uniform list knows every
/// row's height without laying it out, so it has no unmeasured distance for
/// this to solve and its own scroll already lands correctly.
pub fn glide_to_row(ident: &Ident, index: usize, window: &Window, cx: &mut App) {
    let window_id = window.window_handle().window_id();
    let navigation = MotionPolicy::resolve(MotionRole::Navigation, cx);
    let Some(state) = flow_state(ident, window_id, cx).filter(|_| navigation.animates()) else {
        reveal_row(ident, index, window, cx);
        return;
    };
    let glide_spec = navigation.spec();
    let total = glide_spec.total();
    cx.spawn(async move |cx| {
        let mut glide = Glide::new();
        // The executor's clock rather than the wall clock, because they are
        // the same thing everywhere except where they are not: a simulated
        // frame moves the one the timers wait on, and a glide that measured
        // itself against the wall would sit at frame zero for the whole of a
        // test that advanced a second in a microsecond.
        let started = cx.background_executor().now();
        // A bound rather than a `loop`, so a window that stops laying the list
        // out cannot leave a task asking for frames forever. The slack past
        // the duration covers frames that arrived late.
        let frames = total.as_millis() as usize / FRAME.as_millis() as usize + 90;
        let mut height = None;
        for _ in 0..frames {
            cx.background_executor().timer(FRAME).await;
            let elapsed = cx
                .background_executor()
                .now()
                .saturating_duration_since(started)
                .as_secs_f32()
                / total.as_secs_f32();
            let share = glide.step(glide_spec.curve.eval(elapsed.min(1.0)));
            if glide.arrived() {
                break;
            }
            cx.update(|cx| step_toward(&state, index, share, &mut height, cx));
        }
        // However the travel went, it ends on the row that was asked for.
        cx.update(|cx| {
            state.scroll_to(ListOffset {
                item_ix: index,
                offset_in_item: px(0.0),
            });
            cx.refresh_windows();
        });
    })
    .detach();
}

/// One frame of a glide: move `share` of whatever distance is left.
///
/// Where the answer comes from depends on what has been measured. A target
/// that is laid out has real bounds and the step is exact to the pixel. One
/// that is not is approached in row space, over an average row height learned
/// from the viewport — averaged over the whole visible span rather than taken
/// from one row, because a single sample whipsaws between a one-line paragraph
/// and a forty-line code block and the whipsaw is visible as an uneven step.
fn step_toward(
    state: &ListState,
    index: usize,
    share: f32,
    height: &mut Option<f32>,
    cx: &mut App,
) {
    let viewport = f32::from(state.viewport_bounds().size.height);
    if viewport > 0.0 {
        let top = state.logical_scroll_top().item_ix;
        let bottom = f32::from(state.viewport_bounds().bottom());
        let mut row = top;
        let mut rows = 0.0f32;
        while let Some(bounds) = state.bounds_for_item(row) {
            if f32::from(bounds.top()) >= bottom {
                break;
            }
            rows += 1.0;
            row += 1;
        }
        if rows > 0.0 {
            let mean = viewport / rows;
            let learned = height.get_or_insert(mean);
            *learned += 0.5 * (mean - *learned);
        }
    }

    if let Some(bounds) = state.bounds_for_item(index) {
        let away = bounds.top() - state.viewport_bounds().top();
        state.scroll_by(px(share * f32::from(away)));
        cx.refresh_windows();
        return;
    }

    // Unmeasured: travel in row space along the same timeline, and read the
    // position back next frame so a measurement that corrects the estimate is
    // simply where the glide now is.
    let top = state.logical_scroll_top();
    let measured = state
        .bounds_for_item(top.item_ix)
        .map(|bounds| f32::from(bounds.size.height).max(1.0));
    let estimate = height.or(measured).unwrap_or(0.0);
    let here = top.item_ix as f32
        + measured
            .map(|tall| (f32::from(top.offset_in_item) / tall).clamp(0.0, 1.0))
            .unwrap_or(0.0);
    let next = here + share * (index as f32 - here);
    let row = (next.floor().max(0.0) as usize).min(state.item_count().saturating_sub(1));
    state.scroll_to(ListOffset {
        item_ix: row,
        offset_in_item: px((next - row as f32) * estimate),
    });
    cx.refresh_windows();
}

/// What a surface drawn over a list can see of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewed {
    /// The first row the reader can see.
    pub first_row: usize,
    /// How tall the frame showing it is.
    pub height: Pixels,
}

/// The first row of this surface that the reader can see, and how tall the
/// frame showing it is.
///
/// A surface drawn *over* a list — an outline, a scrollbar, a position
/// readout — needs both and owns neither. It reads them by naming the list,
/// the same way it moves it by naming the list. `None` while the surface has
/// not been laid out as a variable-height list, which is one frame at most and
/// is not the same answer as "the top", so a caller can tell "not yet" from
/// "row zero".
pub fn viewed_rows(ident: &Ident, window: &Window, cx: &App) -> Option<Viewed> {
    let state = flow_state(ident, window.window_handle().window_id(), cx)?;
    Some(Viewed {
        first_row: state.logical_scroll_top().item_ix,
        height: state.viewport_bounds().size.height,
    })
}

pub(crate) fn flow_state(ident: &Ident, window_id: WindowId, cx: &App) -> Option<ListState> {
    window_state::read_key(
        &ident.semantic_id(),
        window_id,
        cx,
        |flow: &Option<Flow>| flow.as_ref().map(|flow| flow.state.clone()),
    )
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(names: &[&str]) -> Vec<SharedString> {
        names.iter().map(|name| SharedString::from(*name)).collect()
    }

    #[gpui::test]
    fn revisions_remeasure_offscreen_rows_without_reidentifying_them(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::{AppContext, Context, IntoElement, Render, Styled, div, list, point, size};
        use std::rc::Rc;
        let cx = cx.add_empty_window();
        let ident = Ident::from("revision-test");
        let names = keys(&["a", "b", "c", "d"]);
        let state = cx.update(|window, cx| {
            list_state(
                &ident,
                Rows::Keyed(&names),
                Some(&[0, 0, 0, 0]),
                ListAlignment::Top,
                px(40.),
                window,
                cx,
            )
        });
        struct RowsView(ListState, Rc<std::cell::Cell<f32>>);
        impl Render for RowsView {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let height = self.1.get();
                list(self.0.clone(), move |index, _, _| {
                    div()
                        .h(px(if index == 3 { height } else { 40. }))
                        .into_any_element()
                })
                .w_full()
                .h_full()
            }
        }
        let height = Rc::new(std::cell::Cell::new(80.));
        let view = cx.update(|_, cx| cx.new(|_| RowsView(state.clone(), height.clone())));
        cx.draw(point(px(0.), px(0.)), size(px(100.), px(40.)), |_, _| {
            view.clone().into_any_element()
        });
        assert_eq!(
            state
                .bounds_for_item(3)
                .expect("overdraw measured row")
                .size
                .height,
            px(80.)
        );
        height.set(125.);
        cx.update(|window, cx| {
            list_state(
                &ident,
                Rows::Keyed(&names),
                Some(&[0, 0, 0, 1]),
                ListAlignment::Top,
                px(40.),
                window,
                cx,
            );
        });
        assert!(
            state.bounds_for_item(3).is_none(),
            "revision must invalidate offscreen geometry"
        );
        assert_eq!(
            state
                .bounds_for_item(1)
                .expect("unchanged measurement")
                .size
                .height,
            px(40.)
        );
        cx.draw(point(px(0.), px(0.)), size(px(100.), px(40.)), |_, _| {
            view.clone().into_any_element()
        });
        assert_eq!(
            state
                .bounds_for_item(3)
                .expect("remeasured row")
                .size
                .height,
            px(125.)
        );
        state.scroll_to(ListOffset {
            item_ix: 1,
            offset_in_item: px(13.),
        });
        let reordered = keys(&["d", "a", "b", "c"]);
        cx.update(|window, cx| {
            list_state(
                &ident,
                Rows::Keyed(&reordered),
                Some(&[1, 0, 0, 0]),
                ListAlignment::Top,
                px(40.),
                window,
                cx,
            );
        });
        assert_eq!(state.logical_scroll_top().item_ix, 2);
        assert_eq!(state.logical_scroll_top().offset_in_item, px(13.));
    }
}
