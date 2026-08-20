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

use gpui::SharedString;

use super::history::{EditHistory, EditSelection};
use super::{fit_to_max_graphemes, fit_to_max_length};

pub(crate) use super::history::EditCause as Cause;

/// What a particular control accepts.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EditRules {
    /// A byte limit, applied by trimming an over-long insertion rather than
    /// refusing it.
    pub(crate) max_length: Option<usize>,
    /// A limit in user-perceived characters, for a field whose slots are
    /// graphemes rather than bytes.
    pub(crate) max_graphemes: Option<usize>,
    /// True for a field that holds one line, where a newline arriving from a
    /// paste becomes a space rather than a second line nobody can see.
    pub(crate) single_line: bool,
}

/// The result of an edit: what actually happened, for the caller to report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EditOutcome {
    /// Whether the text is different from what it was.
    pub(crate) changed: bool,
}

/// One control's editable value.
#[derive(Debug)]
pub(crate) struct EditBuffer {
    text: SharedString,
    /// A caret is an empty selection, so one range describes both.
    selection: Range<usize>,
    reversed: bool,
    /// The range an input method is composing, underlined and replaced whole
    /// as composition continues.
    marked: Option<Range<usize>>,
    history: EditHistory,
    rules: EditRules,
}

impl Default for EditBuffer {
    fn default() -> Self {
        Self {
            text: SharedString::default(),
            selection: 0..0,
            reversed: false,
            marked: None,
            history: EditHistory::default(),
            rules: EditRules::default(),
        }
    }
}

impl EditBuffer {
    pub(crate) fn new(rules: EditRules) -> Self {
        Self {
            rules,
            ..Default::default()
        }
    }

    pub(crate) fn text(&self) -> &SharedString {
        &self.text
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub(crate) fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }

    pub(crate) fn is_reversed(&self) -> bool {
        self.reversed
    }

    pub(crate) fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    pub(crate) fn set_marked(&mut self, marked: Option<Range<usize>>) {
        self.marked = marked;
    }

    pub(crate) fn rules_mut(&mut self) -> &mut EditRules {
        &mut self.rules
    }

    /// Stops recording, permanently.
    ///
    /// A credential that could be undone back into view would outlive the
    /// moment it was replaced, so a secret field never has a history rather
    /// than having one it clears.
    pub(crate) fn forbid_history(&mut self) {
        if !self.history.is_disabled() {
            self.history = EditHistory::disabled();
        }
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// The current caret, as a transaction records it.
    fn selection_state(&self) -> EditSelection {
        EditSelection {
            range: self.selection.clone(),
            reversed: self.reversed,
        }
    }

    pub(crate) fn set_caret(&mut self, offset: usize) {
        let offset = offset.min(self.text.len());
        self.selection = offset..offset;
        self.reversed = false;
    }

    pub(crate) fn set_selection(&mut self, range: Range<usize>, reversed: bool) {
        let end = self.text.len();
        self.selection = range.start.min(end)..range.end.min(end);
        self.reversed = reversed;
    }

    /// Moves the end that is moving, keeping the other one anchored.
    pub(crate) fn extend_selection(&mut self, offset: usize) {
        let offset = offset.min(self.text.len());
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
    pub(crate) fn replace(&mut self, range: Range<usize>, text: &str, cause: Cause) -> EditOutcome {
        let range = self.clamp(range);
        let insertion = self.fit(&range, text);

        let before = self.text[range.clone()].to_owned();
        if before == insertion {
            // Nothing changed, but the caret still lands where the edit
            // pointed, which is what makes a delete over an empty selection
            // still move nothing rather than mis-report a change.
            let caret = range.start + insertion.len();
            self.set_caret(caret);
            self.marked = None;
            return EditOutcome { changed: false };
        }

        let selection_before = self.selection_state();
        let next = self.text[..range.start].to_owned() + &insertion + &self.text[range.end..];
        self.text = next.into();
        let caret = range.start + insertion.len();
        self.selection = caret..caret;
        self.reversed = false;
        self.marked = None;

        self.history.record(
            range.start,
            &before,
            &insertion,
            cause,
            selection_before,
            self.selection_state(),
        );
        if cause == Cause::Programmatic {
            self.history.clear();
        }
        EditOutcome { changed: true }
    }

    /// Replaces `range` with `text` and marks the result as composing.
    ///
    /// The composition is not a step of its own until it ends; see
    /// [`EditBuffer::end_composition`].
    pub(crate) fn replace_and_mark(
        &mut self,
        range: Range<usize>,
        text: &str,
        selection_in_insertion: Option<Range<usize>>,
    ) -> EditOutcome {
        let range = self.clamp(range);
        let insertion = self.fit(&range, text);

        self.history.begin_composition(
            range.clone(),
            &self.text[range.clone()],
            self.selection_state(),
        );

        let before = self.text[range.clone()].to_owned();
        let changed = before != insertion;
        if changed {
            let next = self.text[..range.start].to_owned() + &insertion + &self.text[range.end..];
            self.text = next.into();
        }

        self.marked = (!insertion.is_empty()).then(|| range.start..range.start + insertion.len());
        self.selection = match selection_in_insertion {
            // The caller reports this range relative to the composing
            // replacement, not the whole value. Converting it against the
            // already-mutated value can land inside an astral scalar.
            Some(inside) => inside.start + range.start..inside.end + range.start,
            None => {
                let caret = range.start + insertion.len();
                caret..caret
            }
        };
        self.reversed = false;
        EditOutcome { changed }
    }

    /// Closes a composition, recording the whole run as one step.
    pub(crate) fn end_composition(&mut self) {
        if !self.history.is_composing() {
            return;
        }
        let composed = self
            .marked
            .clone()
            .map(|range| self.text[self.clamp(range)].to_owned())
            .unwrap_or_default();
        self.history
            .end_composition(&composed, self.selection_state());
    }

    /// Applies the last transaction backwards. Returns whether anything moved.
    pub(crate) fn undo(&mut self) -> bool {
        let Some(step) = self.history.undo() else {
            return false;
        };
        self.apply(step)
    }

    /// Applies the last undone transaction forwards.
    pub(crate) fn redo(&mut self) -> bool {
        let Some(step) = self.history.redo() else {
            return false;
        };
        self.apply(step)
    }

    fn apply(&mut self, step: super::history::EditStep) -> bool {
        let range = self.clamp(step.range);
        let next = self.text[..range.start].to_owned() + &step.text + &self.text[range.end..];
        self.text = next.into();
        self.marked = None;
        let end = self.text.len();
        self.selection = step.selection.range.start.min(end)..step.selection.range.end.min(end);
        self.reversed = step.selection.reversed;
        true
    }

    /// Puts a value in place without it becoming a step the reader can walk
    /// back through, and forgets the steps that described the old one.
    pub(crate) fn set_text(&mut self, text: &str) -> EditOutcome {
        // A host value is stored as it was given when the control can show
        // it. Only a single-line field rewrites one, because it has nowhere
        // to put a line break and would otherwise hold text nobody can see.
        let normalised = if self.rules.single_line {
            self.normalise(text)
        } else {
            text.to_owned()
        };
        let changed = self.text.as_ref() != normalised.as_str();
        self.text = normalised.into();
        let end = self.text.len();
        self.selection = end..end;
        self.reversed = false;
        self.marked = None;
        self.history.clear();
        EditOutcome { changed }
    }

    fn clamp(&self, range: Range<usize>) -> Range<usize> {
        let end = self.text.len();
        let start = floor_boundary(&self.text, range.start.min(end));
        let stop = floor_boundary(&self.text, range.end.min(end));
        start.min(stop)..stop.max(start)
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
        let text = fit_to_max_length(&self.text, self.rules.max_length, range, &text);
        fit_to_max_graphemes(&self.text, self.rules.max_graphemes, range, &text)
    }
}

/// The nearest character boundary at or below `offset`.
///
/// An offset that arrived from an input method or a stored transaction can
/// point inside a multi-byte scalar after the text around it moved. Slicing
/// there would panic, so it is walked back to somewhere that exists.
fn floor_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(text: &str) -> EditBuffer {
        let mut buffer = EditBuffer::new(EditRules {
            single_line: true,
            ..Default::default()
        });
        buffer.replace(0..0, text, Cause::Programmatic);
        buffer
    }

    #[test]
    fn an_edit_can_be_taken_back_and_put_again() {
        let mut buffer = buffer("alpha");
        buffer.replace(5..5, " beta", Cause::Paste);
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
        buffer.replace(0..5, "gamma", Cause::Paste);
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
            buffer.replace(index..index, &letter.to_string(), Cause::Typing);
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
        buffer.replace(0..0, "abc", Cause::Typing);
        buffer.replace(3..3, "defghij", Cause::Paste);
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
        buffer.replace(0..0, "one\ntwo", Cause::Paste);
        assert_eq!(buffer.text().as_ref(), "one two");
    }

    #[test]
    fn a_value_the_host_set_cannot_be_undone_back_out() {
        let mut buffer = buffer("");
        buffer.replace(0..0, "typed", Cause::Typing);
        buffer.set_text("from the host");

        assert!(!buffer.can_undo());
        assert_eq!(buffer.text().as_ref(), "from the host");
    }

    #[test]
    fn a_secret_field_keeps_no_way_back_to_what_it_held() {
        let mut buffer = buffer("");
        buffer.forbid_history();
        buffer.replace(0..0, "hunter2", Cause::Typing);
        buffer.replace(0..7, "", Cause::Deleting);

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
        buffer.replace(1..1, "x", Cause::Typing);
        assert_eq!(buffer.text().as_ref(), "xé");
    }

    #[test]
    fn an_edit_that_changes_nothing_is_not_a_step() {
        let mut buffer = buffer("abc");
        let outcome = buffer.replace(1..1, "", Cause::Deleting);

        assert!(!outcome.changed);
        assert!(!buffer.can_undo());
    }
}
