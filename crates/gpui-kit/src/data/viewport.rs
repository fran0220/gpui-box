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

/// What one variable-height surface has learned about itself.
struct Flow {
    state: ListState,
    /// The rows as they were last laid out. A surface that names its rows is
    /// diffed against this; one that only counts them keeps the count here as
    /// a run of anonymous names it can still compare the length of.
    keys: Vec<SharedString>,
}

/// The rows a variable-height surface has measured, and where it is scrolled.
///
/// A uniform list needs no such thing: every row is the same height, so it
/// knows where row ten thousand starts without laying one out. A list whose
/// rows are as tall as their content only learns a row's height by laying it
/// out, so what it has learned has to survive the rebuild.
#[derive(Default)]
struct FlowStates(RefCell<HashMap<SharedString, Flow>>);

impl Global for FlowStates {}

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

/// The narrowest range that turns `before` into `after`, as a splice.
///
/// Rows are matched from both ends, so the answer to appending is a splice at
/// the end, the answer to a row changing is that row, and the answer to a
/// streaming reply whose last block grew is the last block. What it never
/// answers is "everything", unless everything did change.
///
/// A single contiguous range rather than a full edit script, because that is
/// what the list takes and because the sequences this serves change at one
/// place: a conversation grows at its end, a log appends, a diff re-renders a
/// file. A caller whose rows are shuffled wholesale gets one wide splice,
/// which is correct and no worse than the reset it replaces.
fn splice_range(before: &[SharedString], after: &[SharedString]) -> Option<(usize, usize)> {
    let prefix = before
        .iter()
        .zip(after)
        .take_while(|(old, new)| old == new)
        .count();
    if prefix == before.len() && prefix == after.len() {
        return None;
    }
    // The suffix may not reach back past what the prefix already claimed, or
    // the two would describe the same rows twice.
    let reach = before.len().min(after.len()) - prefix;
    let suffix = before
        .iter()
        .rev()
        .zip(after.iter().rev())
        .take(reach)
        .take_while(|(old, new)| old == new)
        .count();
    Some((prefix, before.len() - suffix - prefix))
}

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
/// Rows that were named are diffed, so only the range that actually changed is
/// spliced and every measurement outside it survives. Rows that were only
/// counted keep the older, blunter rule: a count that grew is taken to mean
/// rows arrived at the end, and any other change discards the measurements,
/// because they described rows that are no longer at those indices.
pub(crate) fn list_state(
    ident: &Ident,
    rows: Rows<'_>,
    alignment: ListAlignment,
    estimate: Pixels,
    cx: &mut App,
) -> ListState {
    if !cx.has_global::<FlowStates>() {
        cx.set_global(FlowStates::default());
    }
    let count = rows.len();
    let mut states = cx.global::<FlowStates>().0.borrow_mut();
    let flow = states.entry(ident.semantic_id()).or_insert_with(|| Flow {
        state: ListState::new(count, alignment, px(OVERDRAW)).with_uniform_item_height(estimate),
        keys: anonymous(count),
    });

    match rows {
        Rows::Keyed(keys) => {
            if flow.keys != keys {
                if let Some((start, removed)) = splice_range(&flow.keys, keys) {
                    let added = keys.len() - (flow.keys.len() - removed);
                    flow.state.splice(start..start + removed, added);
                }
                flow.keys = keys.to_vec();
            }
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
            }
        }
    }
    flow.state.clone()
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
pub(crate) fn flow_state(ident: &Ident, cx: &mut App) -> Option<ListState> {
    if !cx.has_global::<FlowStates>() {
        return None;
    }
    let states = cx.global::<FlowStates>().0.borrow();
    states
        .get(&ident.semantic_id())
        .map(|flow| flow.state.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(names: &[&str]) -> Vec<SharedString> {
        names.iter().map(|name| SharedString::from(*name)).collect()
    }

    #[test]
    fn an_unchanged_sequence_is_not_spliced_at_all() {
        let rows = keys(&["a", "b", "c"]);
        assert_eq!(splice_range(&rows, &rows), None);
    }

    #[test]
    fn appending_splices_only_the_end() {
        let before = keys(&["a", "b"]);
        let after = keys(&["a", "b", "c", "d"]);
        // Nothing is removed at index 2, and the two new rows land there.
        assert_eq!(splice_range(&before, &after), Some((2, 0)));
    }

    #[test]
    fn a_changed_last_row_splices_that_row_alone() {
        // The streaming case: a reply's final block is re-identified as it
        // grows while every settled block above it keeps its measurement.
        let before = keys(&["a", "b", "c@1"]);
        let after = keys(&["a", "b", "c@2"]);
        assert_eq!(splice_range(&before, &after), Some((2, 1)));
    }

    #[test]
    fn a_row_removed_from_the_middle_keeps_both_sides() {
        let before = keys(&["a", "b", "c", "d"]);
        let after = keys(&["a", "c", "d"]);
        assert_eq!(splice_range(&before, &after), Some((1, 1)));
    }

    #[test]
    fn an_insertion_in_the_middle_keeps_both_sides() {
        let before = keys(&["a", "d"]);
        let after = keys(&["a", "b", "c", "d"]);
        assert_eq!(splice_range(&before, &after), Some((1, 0)));
    }

    #[test]
    fn a_wholesale_replacement_is_one_wide_splice() {
        let before = keys(&["a", "b", "c"]);
        let after = keys(&["x", "y"]);
        assert_eq!(splice_range(&before, &after), Some((0, 3)));
    }

    #[test]
    fn emptying_and_filling_are_both_expressible() {
        let before = keys(&["a", "b"]);
        assert_eq!(splice_range(&before, &[]), Some((0, 2)));
        assert_eq!(splice_range(&[], &before), Some((0, 0)));
    }

    #[test]
    fn a_repeated_row_does_not_let_the_suffix_reach_past_the_prefix() {
        // Both ends match the same rows here. The ranges must not overlap, or
        // the splice would describe more removals than there are rows.
        let before = keys(&["a", "a"]);
        let after = keys(&["a", "a", "a"]);
        let (start, removed) = splice_range(&before, &after).expect("a change");
        assert!(start + removed <= before.len());
        let added = after.len() - (before.len() - removed);
        assert_eq!(before.len() - removed + added, after.len());
    }

    #[test]
    fn every_splice_reproduces_the_new_sequence() {
        // The property the list depends on: applying what this returns to the
        // old rows yields exactly the new ones.
        let cases = [
            (vec!["a", "b", "c"], vec!["a", "b", "c", "d"]),
            (vec!["a", "b", "c"], vec!["a", "c"]),
            (vec!["a"], vec!["b", "a"]),
            (vec!["a", "b", "c", "d"], vec!["a", "x", "y", "d"]),
            (vec![], vec!["a"]),
            (vec!["a", "b"], vec![]),
        ];
        for (before, after) in cases {
            let before = keys(&before);
            let after = keys(&after);
            let mut applied = before.clone();
            if let Some((start, removed)) = splice_range(&before, &after) {
                let added = after.len() - (before.len() - removed);
                applied.splice(start..start + removed, after[start..start + added].to_vec());
            }
            assert_eq!(applied, after);
        }
    }
}
