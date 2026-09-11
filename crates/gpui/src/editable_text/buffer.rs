//! The editable value of a control: its text, its caret, the composition in
//! flight over it, and the transactions that got it here.
//!
//! A single-line field and a wrapped area disagree about layout and about what
//! a newline means. They do not disagree about what an edit *is*, so the
//! replacement itself, the limits that trim it, and the history that can take
//! it back live here once. Each control supplies its own [`EditRules`] and
//! keeps its own geometry.
//!
//! Every mutation goes through [`EditBuffer::replace`]. That is the whole
//! point: a control that also wrote to the text directly would have a history
//! that describes a value nobody has.

use std::ops::Range;

use crate::{NativeTextPosition, NativeTextSelection, SharedString, TextAffinity};
use unicode_segmentation::UnicodeSegmentation;

use super::history::{EditHistory, EditSelection};
use super::{EditSnapshot, fit_to_max_graphemes, fit_to_max_length};

use super::history::EditCause;

/// What a particular control accepts.
#[derive(Clone, Copy, Debug, Default)]
pub struct EditRules {
    /// A byte limit, applied by trimming an over-long insertion rather than
    /// refusing it.
    pub max_length: Option<usize>,
    /// A limit in user-perceived characters, for a field whose slots are
    /// graphemes rather than bytes.
    pub max_graphemes: Option<usize>,
    /// True for a field that holds one line, where a newline arriving from a
    /// paste becomes a space rather than a second line nobody can see.
    pub single_line: bool,
}

/// The result of an edit: what actually happened, for the caller to report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditOutcome {
    /// Whether the text is different from what it was.
    pub changed: bool,
}

/// One control's editable value.
#[derive(Debug)]
pub struct EditBuffer {
    text: EditSnapshot,
    /// A caret is an empty selection, so one range describes both.
    selection: Range<usize>,
    reversed: bool,
    /// Primary anchor/head affinity, independent of normalized range order.
    affinities: [TextAffinity; 2],
    secondary: Vec<(Range<usize>, bool)>,
    /// The range an input method is composing, underlined and replaced whole
    /// as composition continues.
    marked: Option<Range<usize>>,
    history: EditHistory,
    rules: EditRules,
}

impl Default for EditBuffer {
    fn default() -> Self {
        Self {
            text: EditSnapshot::default(),
            selection: 0..0,
            reversed: false,
            affinities: [TextAffinity::Downstream; 2],
            secondary: Vec::new(),
            marked: None,
            history: EditHistory::default(),
            rules: EditRules::default(),
        }
    }
}

impl EditBuffer {
    /// Creates an empty editable value governed by `rules`.
    pub fn new(rules: EditRules) -> Self {
        Self {
            rules,
            ..Default::default()
        }
    }

    /// Returns the current UTF-8 value.
    pub fn text(&self) -> &SharedString {
        self.text.text()
    }

    /// Shares the current document without copying its bytes. Indexed line and
    /// range access on this snapshot does not materialize a contiguous value.
    pub fn snapshot(&self) -> EditSnapshot {
        self.text.clone()
    }

    /// Returns whether the current value is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Returns the current normalized UTF-8 selection range.
    pub fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }

    /// Returns whether the selection focus precedes its anchor.
    pub fn is_reversed(&self) -> bool {
        self.reversed
    }

    /// The atomic UTF-16 primary selection, including both visual affinities.
    /// Secondary selections retain their existing logical-only policy.
    pub fn native_selection(&self) -> NativeTextSelection {
        let (anchor, head) = if self.reversed {
            (self.selection.end, self.selection.start)
        } else {
            (self.selection.start, self.selection.end)
        };
        NativeTextSelection {
            anchor: NativeTextPosition {
                utf16_offset: self.text.offset_to_utf16(anchor),
                affinity: self.affinities[0],
            },
            head: NativeTextPosition {
                utf16_offset: self.text.offset_to_utf16(head),
                affinity: self.affinities[1],
            },
        }
    }

    /// Validates both UTF-16 endpoints as complete grapheme boundaries before
    /// changing anything. Successful native selection clears secondary cursors,
    /// just like set_selection, but retains both primary endpoint affinities.
    /// This does not end an active composition or create an undo transaction.
    pub fn set_native_selection(&mut self, selection: NativeTextSelection) -> bool {
        let anchor = self.text.offset_from_utf16(selection.anchor.utf16_offset);
        let head = self.text.offset_from_utf16(selection.head.utf16_offset);
        if self.text.offset_to_utf16(anchor) != selection.anchor.utf16_offset
            || self.text.offset_to_utf16(head) != selection.head.utf16_offset
            || self.text.floor_grapheme(anchor) != anchor
            || self.text.floor_grapheme(head) != head
        {
            return false;
        }
        self.set_selection(anchor.min(head)..anchor.max(head), head < anchor);
        self.affinities = [selection.anchor.affinity, selection.head.affinity];
        true
    }

    /// Returns the UTF-8 range currently owned by an input composition.
    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    /// Replaces the marked-composition range without changing text.
    pub fn set_marked(&mut self, marked: Option<Range<usize>>) {
        self.marked = marked.map(|range| self.clamp(range));
    }

    /// Returns the limits applied to subsequent replacements.
    pub fn rules_mut(&mut self) -> &mut EditRules {
        &mut self.rules
    }

    /// Stops recording, permanently.
    ///
    /// A credential that could be undone back into view would outlive the
    /// moment it was replaced, so a secret field never has a history rather
    /// than having one it clears.
    pub fn forbid_history(&mut self) {
        if !self.history.is_disabled() {
            self.history = EditHistory::disabled();
        }
    }

    /// Returns whether an edit can be undone.
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Returns whether an undone edit can be replayed.
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// The current caret, as a transaction records it.
    fn selection_state(&self) -> EditSelection {
        EditSelection {
            range: self.selection.clone(),
            reversed: self.reversed,
            affinities: self.affinities,
            secondary: self.secondary.clone(),
        }
    }

    /// Selections in primary-first order. Secondary ranges do not overlap.
    pub fn selections(&self) -> Vec<(Range<usize>, bool)> {
        std::iter::once((self.selection(), self.reversed))
            .chain(self.secondary.iter().cloned())
            .collect()
    }

    /// Returns whether secondary selections accompany the primary selection.
    pub fn has_multiple_selections(&self) -> bool {
        !self.secondary.is_empty()
    }

    /// Deletes selections, extending empty carets by one grapheme. Overlapping
    /// deletion spans merge, so shared text is deleted only once. Undo restores
    /// the original primary-first selections, not the expanded deletion spans.
    pub fn delete_selections(&mut self, backward: bool) -> EditOutcome {
        let ranges: Vec<_> = self
            .selections()
            .into_iter()
            .map(|(mut range, _)| {
                if range.is_empty() {
                    if backward {
                        range.start = self.text.previous_grapheme_boundary(range.start);
                    } else {
                        range.end = self.text.next_grapheme_boundary(range.end);
                    }
                }
                range
            })
            .collect();
        self.delete_ranges(ranges)
    }

    /// Deletes primary-first ranges as one transaction, merging overlaps and
    /// clamping to graphemes. Undo restores the selections before expansion.
    pub fn delete_ranges(&mut self, ranges: impl IntoIterator<Item = Range<usize>>) -> EditOutcome {
        let mut ranges: Vec<_> = ranges
            .into_iter()
            .enumerate()
            .map(|(index, range)| (self.clamp(range), index == 0))
            .collect();
        if ranges.is_empty() {
            return EditOutcome { changed: false };
        }
        ranges.sort_by_key(|(range, _)| (range.start, range.end));
        let mut merged: Vec<(Range<usize>, bool)> = Vec::new();
        for (range, primary) in ranges {
            if let Some(last) = merged.last_mut()
                && last.0.end >= range.start
            {
                last.0.end = last.0.end.max(range.end);
                last.1 |= primary;
            } else {
                merged.push((range, primary));
            }
        }
        let primary = merged
            .iter()
            .position(|(_, primary)| *primary)
            .expect("primary deletion");
        merged.swap(0, primary);
        self.replace_many(
            merged
                .into_iter()
                .map(|(range, _)| (range, SharedString::default())),
            EditCause::Deleting,
        )
        .expect("merged deletion ranges are disjoint")
    }

    /// Sets primary-first selections, clamped to graphemes. Overlaps and
    /// duplicate carets merge, preserving primary direction. Empty input is refused.
    pub fn set_selections(
        &mut self,
        selections: impl IntoIterator<Item = (Range<usize>, bool)>,
    ) -> bool {
        let mut selections: Vec<_> = selections
            .into_iter()
            .enumerate()
            .map(|(index, (range, reversed))| (index, self.clamp(range), reversed))
            .collect();
        if selections.is_empty() {
            return false;
        }
        selections.sort_by_key(|(_, range, _)| (range.start, range.end));
        let mut merged: Vec<(usize, Range<usize>, bool)> = Vec::new();
        for (index, range, reversed) in selections {
            if let Some(last) = merged.last_mut()
                && (last.1.end > range.start || last.1 == range)
            {
                last.1.end = last.1.end.max(range.end);
                if index < last.0 {
                    last.0 = index;
                    last.2 = reversed;
                }
            } else {
                merged.push((index, range, reversed));
            }
        }
        merged.sort_by_key(|(index, _, _)| *index);
        self.end_composition();
        self.selection = merged[0].1.clone();
        self.reversed = merged[0].2;
        self.affinities = [TextAffinity::Downstream; 2];
        self.secondary = merged
            .into_iter()
            .skip(1)
            .map(|(_, range, reversed)| (range, reversed))
            .collect();
        true
    }

    /// Replaces all selections atomically in original source coordinates.
    /// One call is one undo transaction; single-caret typing keeps its normal
    /// coalescing. Each insertion follows the control's ordinary edit rules.
    pub fn replace_selections(&mut self, text: &str, cause: EditCause) -> EditOutcome {
        if self.secondary.is_empty() {
            return self.replace(self.selection(), text, cause);
        }
        let text = SharedString::from(text.to_owned());
        let edits = self
            .selections()
            .into_iter()
            .map(|(range, _)| (range, text.clone()));
        self.replace_many(edits, cause)
            .expect("selections do not overlap")
    }

    /// Applies original-document replacements as one undo transaction. Invalid
    /// grapheme boundaries, overlaps and duplicate insertions are refused
    /// before mutation. Each edit produces a caret; the first is primary.
    pub fn replace_many(
        &mut self,
        edits: impl IntoIterator<Item = (Range<usize>, SharedString)>,
        cause: EditCause,
    ) -> Option<EditOutcome> {
        let mut ordered: Vec<_> = edits.into_iter().enumerate().collect();
        if ordered.is_empty()
            || ordered.iter().any(|(_, (range, _))| {
                range.start > range.end
                    || range.end > self.text.len()
                    || self.text.floor_grapheme(range.start) != range.start
                    || self.text.floor_grapheme(range.end) != range.end
            })
        {
            return None;
        }
        ordered.sort_by_key(|(_, (range, _))| (range.start, range.end));
        if ordered
            .windows(2)
            .any(|pair| pair[0].1.0.end > pair[1].1.0.start || pair[0].1.0 == pair[1].1.0)
        {
            return None;
        }
        self.end_composition();
        let before = self.selection_state();
        self.history.begin_group();
        let mut inserted = vec![0; ordered.len()];
        let mut changed = false;
        for (index, (range, text)) in ordered.iter().rev() {
            changed |= self.replace_clamped(range.clone(), text, cause).changed;
            inserted[*index] = self.selection.end - range.start;
        }
        let mut selections = vec![(0..0, false); ordered.len()];
        let mut added = 0;
        let mut removed = 0;
        for (index, (range, _)) in ordered {
            let caret = range.start + added - removed + inserted[index];
            let caret = self.text.floor_grapheme(caret);
            selections[index] = (caret..caret, false);
            added += inserted[index];
            removed += range.len();
        }
        self.selection = selections[0].0.clone();
        self.reversed = false;
        self.affinities = [TextAffinity::Downstream; 2];
        let mut seen = std::collections::HashSet::from([self.selection.start]);
        self.secondary = selections
            .into_iter()
            .skip(1)
            .filter(|(range, _)| seen.insert(range.start))
            .collect();
        self.history.end_group(before, self.selection_state());
        Some(EditOutcome { changed })
    }

    /// Collapses the selection to the nearest valid offset at or before `offset`.
    pub fn set_caret(&mut self, offset: usize) {
        let offset = self.text.floor_grapheme(offset);
        self.selection = offset..offset;
        self.reversed = false;
        self.affinities = [TextAffinity::Downstream; 2];
        self.secondary.clear();
    }

    /// Sets a normalized selection and which endpoint carries focus.
    pub fn set_selection(&mut self, range: Range<usize>, reversed: bool) {
        self.selection = self.clamp(range);
        self.reversed = reversed;
        self.affinities = [TextAffinity::Downstream; 2];
        self.secondary.clear();
    }

    /// Moves the end that is moving, keeping the other one anchored.
    pub fn extend_selection(&mut self, offset: usize) {
        let offset = self.text.floor_grapheme(offset);
        self.affinities = [TextAffinity::Downstream; 2];
        self.secondary.clear();
        if self.reversed {
            self.selection.start = offset;
        } else {
            self.selection.end = offset;
        }
        if self.selection.end < self.selection.start {
            self.reversed = !self.reversed;
            self.selection = self.selection.end..self.selection.start;
        }
    }

    /// Replaces `range` with `text`, recording it as `cause` requires.
    ///
    /// The insertion is normalised and trimmed to the control's rules first,
    /// so what is recorded is what was actually kept rather than what was
    /// offered.
    pub fn replace(&mut self, range: Range<usize>, text: &str, cause: EditCause) -> EditOutcome {
        let range = self.clamp(range);
        self.replace_clamped(range, text, cause)
    }

    // A batch's ranges were clamped in the original document. Re-clamping
    // after a neighbouring insertion can consume a newly joined grapheme.
    fn replace_clamped(
        &mut self,
        range: Range<usize>,
        text: &str,
        cause: EditCause,
    ) -> EditOutcome {
        let insertion = self.fit(&range, text);

        let before = self.text.slice(range.clone()).expect("clamped edit range");
        if before == insertion {
            // Nothing changed, but the caret still lands where the edit
            // pointed, which is what makes a delete over an empty selection
            // still move nothing rather than mis-report a change.
            let caret = range.start + insertion.len();
            self.selection = caret..caret;
            self.reversed = false;
            self.affinities = [TextAffinity::Downstream; 2];
            self.secondary.clear();
            self.marked = None;
            return EditOutcome { changed: false };
        }

        let selection_before = self.selection_state();
        self.text.replace(range.clone(), &insertion);
        let caret = range.start + insertion.len();
        self.selection = caret..caret;
        self.reversed = false;
        self.affinities = [TextAffinity::Downstream; 2];
        self.secondary.clear();
        self.marked = None;

        self.history.record(
            range.start,
            &before,
            &insertion,
            cause,
            selection_before,
            self.selection_state(),
        );
        if cause == EditCause::Programmatic {
            self.history.clear();
        }
        EditOutcome { changed: true }
    }

    /// Replaces `range` with `text` and marks the result as composing.
    ///
    /// The composition is not a step of its own until it ends; see
    /// [`EditBuffer::end_composition`].
    pub fn replace_and_mark(
        &mut self,
        range: Range<usize>,
        text: &str,
        selection_in_insertion: Option<Range<usize>>,
    ) -> EditOutcome {
        let range = self.clamp(range);
        let insertion = self.fit(&range, text);

        self.history.begin_composition(
            range.clone(),
            &self
                .text
                .slice(range.clone())
                .expect("clamped composition range"),
            self.selection_state(),
        );
        // Native IME protocols designate one replacement range. Keep that
        // primary composition authoritative; undo restores the prior set.
        self.secondary.clear();

        let before = self
            .text
            .slice(range.clone())
            .expect("clamped composition range");
        let changed = before != insertion;
        if changed {
            self.text.replace(range.clone(), &insertion);
        }

        self.marked = (!insertion.is_empty()).then(|| range.start..range.start + insertion.len());
        self.selection = match selection_in_insertion {
            // The caller reports this range relative to the composing
            // replacement, not the whole value. Converting it against the
            // already-mutated value can land inside an astral scalar.
            Some(inside) => {
                let inside = clamp_grapheme_range(&insertion, inside);
                inside.start + range.start..inside.end + range.start
            }
            None => {
                let caret = range.start + insertion.len();
                caret..caret
            }
        };
        self.reversed = false;
        self.affinities = [TextAffinity::Downstream; 2];
        EditOutcome { changed }
    }

    /// Closes a composition, recording the whole run as one step.
    pub fn end_composition(&mut self) {
        if !self.history.is_composing() {
            return;
        }
        let composed = self
            .marked
            .clone()
            .map(|range| {
                self.text
                    .slice(self.clamp(range))
                    .expect("clamped marked range")
            })
            .unwrap_or_default();
        self.history
            .end_composition(&composed, self.selection_state());
    }

    /// Applies the last transaction backwards. Returns whether anything moved.
    /// Native composition cancellation uses cancel_composition instead.
    pub fn undo(&mut self) -> bool {
        let Some(step) = self.history.undo() else {
            return false;
        };
        self.apply(step)
    }

    /// Applies the last undone transaction forwards.
    pub fn redo(&mut self) -> bool {
        let Some(step) = self.history.redo() else {
            return false;
        };
        self.apply(step)
    }

    /// Rolls back only the active IME composition, restoring its original
    /// primary affinities and secondary selection set. Unlike undo, this is
    /// available for secret fields and retains no completed text history.
    pub fn cancel_composition(&mut self) -> bool {
        let current_len = self.marked.as_ref().map_or(0, |range| range.len());
        let Some(step) = self.history.cancel_composition(current_len) else {
            return false;
        };
        self.apply(vec![step])
    }

    fn apply(&mut self, steps: Vec<super::history::EditStep>) -> bool {
        // History stores exact byte replacements, including insertions that
        // joined an adjacent grapheme. Re-clamping would delete its neighbour.
        for step in steps {
            self.text.replace(step.range, &step.text);
            self.marked = None;
            let end = self.text.len();
            self.selection = step.selection.range.start.min(end)..step.selection.range.end.min(end);
            self.reversed = step.selection.reversed;
            self.affinities = step.selection.affinities;
            self.secondary = step.selection.secondary;
        }
        true
    }

    /// Puts a value in place without it becoming a step the reader can walk
    /// back through, and forgets the steps that described the old one.
    pub fn set_text(&mut self, text: &str) -> EditOutcome {
        // A host value is stored as it was given when the control can show
        // it. Only a single-line field rewrites one, because it has nowhere
        // to put a line break and would otherwise hold text nobody can see.
        let normalised = if self.rules.single_line {
            self.normalise(text)
        } else {
            text.to_owned()
        };
        let changed = self.text.text().as_ref() != normalised.as_str();
        self.text = EditSnapshot::new(&normalised);
        let end = self.text.len();
        self.selection = end..end;
        self.reversed = false;
        self.affinities = [TextAffinity::Downstream; 2];
        self.secondary.clear();
        self.marked = None;
        self.history.clear();
        EditOutcome { changed }
    }

    fn clamp(&self, range: Range<usize>) -> Range<usize> {
        let start = self.text.floor_grapheme(range.start);
        let end = self.text.floor_grapheme(range.end);
        start.min(end)..start.max(end)
    }

    fn normalise(&self, text: &str) -> String {
        if self.rules.single_line {
            super::normalize_single_line(text)
        } else {
            super::normalize_multiline(text)
        }
    }

    fn fit(&self, range: &Range<usize>, text: &str) -> String {
        let text = self.normalise(text);
        if self.rules.max_length.is_none() && self.rules.max_graphemes.is_none() {
            return text;
        }
        let text = fit_to_max_length(self.text.text(), self.rules.max_length, range, &text);
        fit_to_max_graphemes(self.text.text(), self.rules.max_graphemes, range, &text)
    }
}

/// The nearest extended-grapheme boundary at or below `offset`.
///
/// An offset that arrived from an input method or a stored transaction can
/// point inside a multi-byte scalar or a user-perceived character after the
/// text around it moved. Slicing there would panic or split the character, so
/// it is walked back to a boundary the editor can expose.
pub(super) fn floor_grapheme_boundary(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    if offset == text.len() {
        return text.len();
    }
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|index| *index <= offset)
        .last()
        .unwrap_or(0)
}

pub(crate) fn clamp_grapheme_range(text: &str, range: Range<usize>) -> Range<usize> {
    let start = floor_grapheme_boundary(text, range.start);
    let end = floor_grapheme_boundary(text, range.end);
    start.min(end)..start.max(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native(anchor: usize, head: usize) -> NativeTextSelection {
        NativeTextSelection {
            anchor: NativeTextPosition {
                utf16_offset: anchor,
                affinity: TextAffinity::Upstream,
            },
            head: NativeTextPosition {
                utf16_offset: head,
                affinity: TextAffinity::Downstream,
            },
        }
    }

    #[test]
    fn native_selection_is_atomic_preserves_affinity_and_validates_utf16_graphemes() {
        let mut buffer = buffer("a😀אבz");
        let coincident = native(3, 3);
        assert!(buffer.set_native_selection(coincident));
        assert_eq!(buffer.native_selection(), coincident);
        assert_eq!(buffer.selection(), 5..5);
        let reversed = native(5, 1);
        assert!(buffer.set_native_selection(reversed));
        assert_eq!(buffer.native_selection(), reversed);
        assert_eq!(buffer.selection(), 1..9);
        assert!(buffer.is_reversed());
        assert!(
            !buffer.set_native_selection(native(3, 2)),
            "surrogate interior is refused, not rounded"
        );
        assert_eq!(
            buffer.native_selection(),
            reversed,
            "invalid head must not mutate valid anchor"
        );
        assert!(!buffer.set_native_selection(native(usize::MAX, 0)));
        buffer.set_selection(1..9, true);
        assert_eq!(
            buffer.native_selection().anchor.affinity,
            TextAffinity::Downstream
        );
        let mut combined = super::tests::buffer("a\u{301}z");
        assert!(
            !combined.set_native_selection(native(0, 1)),
            "combining sequence is indivisible"
        );
    }

    #[test]
    fn native_selection_history_restores_reversed_endpoints_and_composition_affinity() {
        let mut buffer = buffer("a😀אבz");
        let before = native(5, 1);
        assert!(buffer.set_native_selection(before));
        buffer.replace(1..9, "Q", EditCause::Paste);
        let after = buffer.native_selection();
        assert!(buffer.undo());
        assert_eq!(buffer.text().as_ref(), "a😀אבz");
        assert_eq!(buffer.native_selection(), before);
        assert!(buffer.redo());
        assert_eq!(buffer.native_selection(), after);
        assert!(buffer.undo());
        buffer.replace_and_mark(1..9, "界", Some(0..3));
        let composed = native(2, 1);
        assert!(buffer.set_native_selection(composed));
        buffer.end_composition();
        assert!(buffer.undo());
        assert_eq!(buffer.native_selection(), before);
        assert!(buffer.redo());
        assert_eq!(buffer.native_selection(), composed);
    }

    #[test]
    fn native_composition_cancel_restores_affinities_without_enabling_secret_history() {
        for secret in [false, true] {
            let mut buffer = buffer("a😀אבz");
            if secret {
                buffer.forbid_history();
            }
            let initial = native(5, 1);
            assert!(buffer.set_native_selection(initial));
            buffer.replace_and_mark(1..9, "界", None);
            buffer.replace_and_mark(1..4, "", None);
            assert!(buffer.cancel_composition());
            assert_eq!(buffer.text().as_ref(), "a😀אבz");
            assert_eq!(buffer.native_selection(), initial);
            assert!(!buffer.can_undo());
            assert!(!buffer.can_redo());
            assert!(!buffer.cancel_composition());
            buffer.replace_and_mark(1..9, "x", None);
            buffer.end_composition();
            assert_eq!(buffer.can_undo(), !secret);
            assert!(
                !buffer.cancel_composition(),
                "committed composition is no longer rollback state"
            );
        }
    }

    #[test]
    fn native_composition_keeps_primary_authority_and_undo_restores_secondary() {
        let mut buffer = buffer("é middle 😀 end");
        let initial = vec![(10..14, true), (0..2, false)];
        buffer.set_selections(initial.clone());
        buffer.replace_and_mark(10..14, "界", Some(3..3));
        assert_eq!(buffer.selections(), vec![(13..13, false)]);
        buffer.replace_and_mark(10..13, "日本", Some(6..6));
        buffer.end_composition();
        assert_eq!(buffer.text().as_ref(), "é middle 日本 end");
        assert!(buffer.undo());
        assert_eq!(buffer.text().as_ref(), "é middle 😀 end");
        assert_eq!(buffer.selections(), initial);
        assert!(!buffer.can_undo());
        assert!(buffer.redo());
        assert_eq!(buffer.selections(), vec![(16..16, false)]);
    }

    #[test]
    fn multiple_selections_replay_as_one_atomic_edit_with_primary_identity() {
        let mut buffer = buffer("é middle 😀 end");
        let initial = vec![(10..14, true), (0..2, false)];
        assert!(buffer.set_selections(initial.clone()));
        assert!(buffer.replace_selections("Q", EditCause::Typing).changed);
        assert_eq!(buffer.text().as_ref(), "Q middle Q end");
        assert_eq!(buffer.selections(), vec![(10..10, false), (1..1, false)]);
        assert!(buffer.replace_selections("R", EditCause::Typing).changed);
        assert_eq!(buffer.text().as_ref(), "QR middle QR end");
        assert!(buffer.undo());
        assert_eq!(buffer.text().as_ref(), "Q middle Q end");
        assert!(buffer.undo());
        assert_eq!(buffer.text().as_ref(), "é middle 😀 end");
        assert_eq!(buffer.selections(), initial);
        assert!(!buffer.can_undo());
        assert!(buffer.redo());
        assert_eq!(buffer.text().as_ref(), "Q middle Q end");
        assert_eq!(buffer.selections(), vec![(10..10, false), (1..1, false)]);
    }

    #[test]
    fn multiple_deletions_merge_shared_text_and_restore_unexpanded_carets() {
        let mut buffer = buffer("a😀bc");
        let initial = vec![(5..5, false), (0..5, true), (7..7, false)];
        assert!(buffer.set_selections(initial.clone()));
        assert!(buffer.delete_selections(true).changed);
        assert_eq!(buffer.text().as_ref(), "b");
        assert_eq!(buffer.selections(), vec![(0..0, false), (1..1, false)]);
        assert!(buffer.undo());
        assert_eq!(buffer.text().as_ref(), "a😀bc");
        assert_eq!(buffer.selections(), initial);
        assert!(buffer.set_selections([(1..1, false), (6..6, false)]));
        buffer.delete_selections(false);
        assert_eq!(buffer.text().as_ref(), "ab");
    }

    #[test]
    fn different_batch_replacements_are_atomic_and_invalid_ranges_are_refused() {
        let mut buffer = buffer("é middle 😀 end");
        assert!(
            buffer
                .replace_many([(0..1, "x".into()), (10..14, "Q".into())], EditCause::Paste)
                .is_none()
        );
        assert_eq!(buffer.text().as_ref(), "é middle 😀 end");
        assert!(!buffer.can_undo());
        assert!(
            buffer
                .replace_many(
                    [(10..14, "two".into()), (0..2, "1".into())],
                    EditCause::Paste
                )
                .expect("valid batch")
                .changed
        );
        assert_eq!(buffer.text().as_ref(), "1 middle two end");
        assert!(buffer.undo());
        assert_eq!(buffer.text().as_ref(), "é middle 😀 end");
        assert!(buffer.redo());
        assert_eq!(buffer.text().as_ref(), "1 middle two end");
    }

    #[test]
    fn batch_noop_prefix_keeps_original_selections_and_does_not_merge_next_typing() {
        let mut buffer = buffer("aZ");
        let initial = vec![(0..1, true), (1..2, false)];
        assert!(buffer.set_selections(initial.clone()));
        buffer.replace_selections("Z", EditCause::Typing);
        assert_eq!(buffer.text().as_ref(), "ZZ");
        buffer.set_caret(1);
        buffer.replace(1..1, "t", EditCause::Typing);
        assert!(buffer.undo());
        assert_eq!(buffer.text().as_ref(), "ZZ");
        assert!(buffer.undo());
        assert_eq!(buffer.text().as_ref(), "aZ");
        assert_eq!(buffer.selections(), initial);
    }

    #[test]
    fn batch_uses_original_grapheme_boundaries_and_coalesces_coincident_carets() {
        let mut buffer = buffer("ab");
        assert!(buffer.set_selections([(0..1, false), (1..2, false)]));
        buffer.replace_selections("\u{301}", EditCause::Paste);
        assert_eq!(buffer.text().as_ref(), "\u{301}\u{301}");
        assert_eq!(buffer.selections(), vec![(0..0, false), (4..4, false)]);
        assert!(buffer.undo());
        assert_eq!(buffer.text().as_ref(), "ab");
        buffer.replace_selections("", EditCause::Deleting);
        assert_eq!(buffer.text().as_ref(), "");
        assert_eq!(buffer.selections(), vec![(0..0, false)]);
        buffer.replace_selections("x", EditCause::Typing);
        assert_eq!(buffer.text().as_ref(), "x");
    }

    #[test]
    fn multiselections_merge_overlaps_without_losing_primary_direction() {
        let mut buffer = buffer("a😀b");
        assert!(buffer.set_selections([(2..5, true), (0..5, false)]));
        assert_eq!(buffer.selections(), vec![(0..5, true)]);
        assert!(buffer.set_selections([(1..1, false), (1..1, false)]));
        assert_eq!(buffer.selections(), vec![(1..1, false)]);
        assert!(!buffer.set_selections([]));
        assert_eq!(buffer.selections(), vec![(1..1, false)]);
    }

    fn buffer(text: &str) -> EditBuffer {
        let mut buffer = EditBuffer::new(EditRules {
            single_line: true,
            ..Default::default()
        });
        buffer.replace(0..0, text, EditCause::Programmatic);
        buffer
    }

    #[test]
    fn undo_replays_exact_bytes_when_insertion_joined_a_grapheme() {
        let mut buffer = buffer("az");
        buffer.set_caret(1);
        buffer.replace(1..1, "\u{301}", EditCause::Paste);
        assert_eq!(buffer.text().as_ref(), "a\u{301}z");
        assert!(buffer.undo());
        assert_eq!(buffer.text().as_ref(), "az");
        assert_eq!(buffer.selection(), 1..1);
        assert!(buffer.redo());
        assert_eq!(buffer.text().as_ref(), "a\u{301}z");
    }

    #[test]
    fn chunk_crossing_graphemes_match_contiguous_clamping() {
        let text = "prefix\n".to_owned() + &"e\u{301}".repeat(800) + "👩‍💻🇺🇳\r\nlast";
        let mut buffer = EditBuffer::default();
        buffer.set_text(&text);
        for offset in 0..text.len() {
            buffer.set_caret(offset);
            assert_eq!(
                buffer.selection().start,
                floor_grapheme_boundary(&text, offset)
            );
        }
    }

    #[test]
    fn an_edit_can_be_taken_back_and_put_again() {
        let mut buffer = buffer("alpha");
        buffer.replace(5..5, " beta", EditCause::Paste);
        assert_eq!(buffer.text().as_ref(), "alpha beta");

        assert!(buffer.undo());
        assert_eq!(buffer.text().as_ref(), "alpha");
        assert_eq!(buffer.selection(), 5..5);

        assert!(buffer.redo());
        assert_eq!(buffer.text().as_ref(), "alpha beta");
        assert_eq!(buffer.selection(), 10..10);
    }

    #[test]
    fn undo_restores_the_selection_the_edit_replaced() {
        let mut buffer = buffer("alpha beta");
        buffer.set_selection(0..5, false);
        buffer.replace(0..5, "gamma", EditCause::Paste);
        assert_eq!(buffer.text().as_ref(), "gamma beta");

        buffer.undo();
        assert_eq!(buffer.text().as_ref(), "alpha beta");
        assert_eq!(
            buffer.selection(),
            0..5,
            "what was replaced is selected again, so the reader can see what came back"
        );
    }

    #[test]
    fn a_run_of_typing_is_taken_back_as_one_word() {
        let mut buffer = buffer("");
        for (index, letter) in "abc".chars().enumerate() {
            buffer.replace(index..index, &letter.to_string(), EditCause::Typing);
        }
        assert_eq!(buffer.text().as_ref(), "abc");

        buffer.undo();
        assert_eq!(buffer.text().as_ref(), "");
        assert!(!buffer.can_undo());
    }

    #[test]
    fn a_limit_trims_the_insertion_that_is_recorded() {
        let mut buffer = EditBuffer::new(EditRules {
            max_length: Some(6),
            single_line: true,
            ..Default::default()
        });
        buffer.replace(0..0, "abc", EditCause::Typing);
        buffer.replace(3..3, "defghij", EditCause::Paste);
        assert_eq!(buffer.text().as_ref(), "abcdef");

        buffer.undo();
        assert_eq!(
            buffer.text().as_ref(),
            "abc",
            "undo takes back what was kept, not what was offered"
        );
    }

    #[test]
    fn a_single_line_field_turns_a_pasted_newline_into_a_space() {
        let mut buffer = buffer("");
        buffer.replace(0..0, "one\ntwo", EditCause::Paste);
        assert_eq!(buffer.text().as_ref(), "one two");
    }

    #[test]
    fn a_value_the_host_set_cannot_be_undone_back_out() {
        let mut buffer = buffer("");
        buffer.replace(0..0, "typed", EditCause::Typing);
        buffer.set_text("from the host");

        assert!(!buffer.can_undo());
        assert_eq!(buffer.text().as_ref(), "from the host");
    }

    #[test]
    fn a_secret_field_keeps_no_way_back_to_what_it_held() {
        let mut buffer = buffer("");
        buffer.forbid_history();
        buffer.replace(0..0, "hunter2", EditCause::Typing);
        buffer.replace(0..7, "", EditCause::Deleting);

        assert!(!buffer.can_undo());
        assert!(!buffer.undo());
        assert_eq!(buffer.text().as_ref(), "");
    }

    #[test]
    fn a_composition_is_one_step_however_many_times_it_was_rewritten() {
        let mut buffer = buffer("");
        buffer.replace_and_mark(0..0, "n", None);
        buffer.replace_and_mark(0..1, "ni", None);
        buffer.replace_and_mark(0..2, "に", None);
        buffer.end_composition();

        assert_eq!(buffer.text().as_ref(), "に");
        buffer.undo();
        assert_eq!(
            buffer.text().as_ref(),
            "",
            "the whole composition is one thing the reader did"
        );
        assert!(!buffer.can_undo());
    }

    #[test]
    fn an_offset_inside_a_character_is_walked_back_rather_than_panicking() {
        let mut buffer = buffer("é");
        // One byte into a two-byte scalar.
        buffer.replace(1..1, "x", EditCause::Typing);
        assert_eq!(buffer.text().as_ref(), "xé");
    }

    #[test]
    fn selections_and_edits_never_split_an_extended_grapheme() {
        let mut buffer = buffer("ae\u{301}👩‍💻z");
        buffer.set_selection(2..10, false);
        assert_eq!(buffer.selection(), 1..4);

        buffer.replace(5..15, "x", EditCause::Typing);
        assert_eq!(buffer.text().as_ref(), "ae\u{301}xz");
    }

    #[test]
    fn composition_selection_is_clamped_inside_its_replacement() {
        let mut buffer = buffer("");
        buffer.replace_and_mark(0..0, "e\u{301}", Some(1..2));
        assert_eq!(buffer.selection(), 0..0);
        assert_eq!(buffer.marked(), Some(0..3));
    }

    #[test]
    fn an_edit_that_changes_nothing_is_not_a_step() {
        let mut buffer = buffer("abc");
        let outcome = buffer.replace(1..1, "", EditCause::Deleting);

        assert!(!outcome.changed);
        assert!(!buffer.can_undo());
    }
}
