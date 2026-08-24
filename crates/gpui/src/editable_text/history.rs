//! What an edit was, so it can be taken back.
//!
//! A history is a stack of *transactions*, and a transaction is one whole
//! replacement: the range it covered, the text that was there, the text that
//! replaced it, and where the caret stood on each side of it. Undo is that
//! replacement read backwards, which is why nothing here needs to keep a copy
//! of the whole value at every keystroke.
//!
//! # What counts as one edit
//!
//! Typing is a run of single-character insertions, and taking back one letter
//! at a time is not what a reader means by undo. Consecutive insertions merge
//! into one transaction while they stay a run: same cause, caret where the
//! last one left it, and nothing typed that ends a word. Anything else opens a
//! new transaction, so an undo lands on a boundary somebody would recognise.
//!
//! # What is never recorded
//!
//! A secret field keeps no history at all: a credential that could be undone
//! back into view would outlive the moment it was replaced. A value the host
//! set is not the user's edit either, so it clears what came before rather
//! than becoming a step the user can walk back through.

use std::ops::Range;

/// How far the caret and its selection reached, and which end was moving.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EditSelection {
    pub(super) range: Range<usize>,
    pub(super) reversed: bool,
}

impl EditSelection {
    /// A caret, which is a selection that reached nowhere.
    #[cfg(test)]
    pub(super) fn caret(offset: usize) -> Self {
        Self {
            range: offset..offset,
            reversed: false,
        }
    }
}

/// What the reader did, which decides whether an edit joins the step before
/// it and whether it is remembered at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditCause {
    /// Text the reader typed or an input method committed.
    Typing,
    /// A backspace or a delete.
    Deleting,
    /// One whole clipboard insertion, which is one step whatever its length.
    Paste,
    /// A cut, which is a deletion that also left with something.
    Cut,
    /// The value the host set. It is not the reader's edit, so it ends the
    /// history rather than joining it.
    Programmatic,
    /// Undo or redo replaying a transaction, which must never record one.
    Replay,
}

impl EditCause {
    /// Whether two consecutive edits of this cause may become one step.
    fn coalesces(self) -> bool {
        matches!(self, Self::Typing | Self::Deleting)
    }
}

/// One whole replacement: where it happened, what was there, what replaced it,
/// and where the caret stood on each side of it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct EditTransaction {
    start: usize,
    before: String,
    after: String,
    cause: EditCause,
    selection_before: EditSelection,
    selection_after: EditSelection,
}

impl EditTransaction {
    /// The range it occupies now that it has been applied, which is what undo
    /// replaces.
    fn range_after(&self) -> Range<usize> {
        self.start..self.start + self.after.len()
    }

    /// The range it occupied before it was applied, which is what redo
    /// replaces once undo has put that text back.
    fn range_before(&self) -> Range<usize> {
        self.start..self.start + self.before.len()
    }
}

/// One step to apply, handed back by [`EditHistory::undo`] or
/// [`EditHistory::redo`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EditStep {
    /// The range of the current text to replace.
    pub(super) range: Range<usize>,
    /// What to put there.
    pub(super) text: String,
    /// Where the caret belongs once it is applied.
    pub(super) selection: EditSelection,
}

/// The transactions of one editable control.
#[derive(Clone, Debug, Default)]
pub(super) struct EditHistory {
    done: Vec<EditTransaction>,
    undone: Vec<EditTransaction>,
    /// A history that refuses to record. A secret field sets this and never
    /// unsets it, so no credential can be recovered by walking backwards.
    disabled: bool,
    /// True while an input method is composing. A composition rewrites its own
    /// marked run many times before it means anything, so the run is recorded
    /// once, when it ends.
    composing: Option<CompositionStart>,
}

/// Where a composition began, so the whole run can be recorded as one step.
#[derive(Clone, Debug)]
struct CompositionStart {
    start: usize,
    before: String,
    selection: EditSelection,
}

impl EditHistory {
    /// A history that records nothing, for a field holding a credential.
    pub(super) fn disabled() -> Self {
        Self {
            disabled: true,
            ..Default::default()
        }
    }

    pub(super) fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub(super) fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub(super) fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// Forgets everything. The value stopped being the one these transactions
    /// describe, so they no longer describe anything.
    pub(super) fn clear(&mut self) {
        self.done.clear();
        self.undone.clear();
        self.composing = None;
    }

    /// Notes that an input method has started composing at `range`, whose
    /// current content is `before`.
    ///
    /// Repeated calls while a composition is already open are ignored, which
    /// is what makes the whole run one step rather than one step per keypress
    /// the input method consumed.
    pub(super) fn begin_composition(
        &mut self,
        range: Range<usize>,
        before: &str,
        selection: EditSelection,
    ) {
        if self.disabled || self.composing.is_some() {
            return;
        }
        self.composing = Some(CompositionStart {
            start: range.start,
            before: before.to_owned(),
            selection,
        });
    }

    /// Records the whole composition as one transaction, if it changed
    /// anything.
    pub(super) fn end_composition(&mut self, after: &str, selection: EditSelection) {
        let Some(start) = self.composing.take() else {
            return;
        };
        if start.before == after {
            return;
        }
        self.done.push(EditTransaction {
            start: start.start,
            before: start.before,
            after: after.to_owned(),
            cause: EditCause::Typing,
            selection_before: start.selection,
            selection_after: selection,
        });
        self.undone.clear();
    }

    pub(super) fn is_composing(&self) -> bool {
        self.composing.is_some()
    }

    /// Records one replacement.
    ///
    /// A replay records nothing, or undo would only ever undo itself. A
    /// programmatic value clears the history instead of joining it.
    pub(super) fn record(
        &mut self,
        start: usize,
        before: &str,
        after: &str,
        cause: EditCause,
        selection_before: EditSelection,
        selection_after: EditSelection,
    ) {
        if self.disabled || cause == EditCause::Replay {
            return;
        }
        if cause == EditCause::Programmatic {
            self.clear();
            return;
        }
        if self.composing.is_some() {
            return;
        }
        if before == after {
            return;
        }
        self.undone.clear();

        if let Some(last) = self.done.last_mut()
            && merges(last, start, before, after, cause)
        {
            last.after.push_str(after);
            last.selection_after = selection_after;
            return;
        }

        self.done.push(EditTransaction {
            start,
            before: before.to_owned(),
            after: after.to_owned(),
            cause,
            selection_before,
            selection_after,
        });
    }

    /// The step that takes the last transaction back.
    pub(super) fn undo(&mut self) -> Option<EditStep> {
        let transaction = self.done.pop()?;
        let step = EditStep {
            range: transaction.range_after(),
            text: transaction.before.clone(),
            selection: transaction.selection_before.clone(),
        };
        self.undone.push(transaction);
        Some(step)
    }

    /// The step that puts it back.
    pub(super) fn redo(&mut self) -> Option<EditStep> {
        let transaction = self.undone.pop()?;
        let step = EditStep {
            range: transaction.range_before(),
            text: transaction.after.clone(),
            selection: transaction.selection_after.clone(),
        };
        self.done.push(transaction);
        Some(step)
    }
}

/// Whether a new edit continues the run the last transaction is.
///
/// Only a plain insertion continues one, and only where the last one ended.
/// A word boundary closes the run, so undo after a sentence lands between
/// words rather than removing the sentence or one letter of it.
fn merges(
    last: &EditTransaction,
    start: usize,
    before: &str,
    after: &str,
    cause: EditCause,
) -> bool {
    if cause != last.cause || !cause.coalesces() {
        return false;
    }
    if cause != EditCause::Typing {
        return false;
    }
    if !before.is_empty() || after.is_empty() {
        return false;
    }
    if start != last.start + last.after.len() {
        return false;
    }
    // A space or a newline is where a reader would expect undo to stop.
    !after.contains(|character: char| character.is_whitespace())
        && !last
            .after
            .ends_with(|character: char| character.is_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(history: &mut EditHistory, at: usize, text: &str) {
        history.record(
            at,
            "",
            text,
            EditCause::Typing,
            EditSelection::caret(at),
            EditSelection::caret(at + text.len()),
        );
    }

    #[test]
    fn a_run_of_typing_is_one_step() {
        let mut history = EditHistory::default();
        typed(&mut history, 0, "a");
        typed(&mut history, 1, "b");
        typed(&mut history, 2, "c");

        let step = history.undo().expect("one step");
        assert_eq!(step.range, 0..3);
        assert_eq!(step.text, "");
        assert!(!history.can_undo(), "the run was a single step");
    }

    #[test]
    fn a_space_closes_the_run() {
        let mut history = EditHistory::default();
        typed(&mut history, 0, "a");
        typed(&mut history, 1, " ");
        typed(&mut history, 2, "b");

        let step = history.undo().expect("the second word");
        assert_eq!(step.range, 2..3, "undo lands between words");
        assert!(history.can_undo());
    }

    #[test]
    fn typing_somewhere_else_opens_a_new_step() {
        let mut history = EditHistory::default();
        typed(&mut history, 0, "a");
        typed(&mut history, 5, "b");

        assert_eq!(history.undo().expect("the far edit").range, 5..6);
        assert_eq!(history.undo().expect("the first edit").range, 0..1);
    }

    #[test]
    fn a_paste_is_one_step_however_long_it_is() {
        let mut history = EditHistory::default();
        history.record(
            0,
            "",
            "a whole paragraph",
            EditCause::Paste,
            EditSelection::caret(0),
            EditSelection::caret(17),
        );
        history.record(
            17,
            "",
            " and another",
            EditCause::Paste,
            EditSelection::caret(17),
            EditSelection::caret(29),
        );

        assert_eq!(history.undo().expect("the second paste").range, 17..29);
        assert_eq!(history.undo().expect("the first paste").range, 0..17);
    }

    #[test]
    fn a_deletion_never_joins_the_typing_before_it() {
        let mut history = EditHistory::default();
        typed(&mut history, 0, "a");
        history.record(
            0,
            "a",
            "",
            EditCause::Deleting,
            EditSelection::caret(1),
            EditSelection::caret(0),
        );

        assert_eq!(history.undo().expect("the deletion").text, "a");
        assert_eq!(history.undo().expect("the typing").text, "");
    }

    #[test]
    fn redo_puts_back_exactly_what_undo_took() {
        let mut history = EditHistory::default();
        history.record(
            2,
            "old",
            "new value",
            EditCause::Paste,
            EditSelection::caret(2),
            EditSelection::caret(11),
        );

        let undo = history.undo().expect("a step");
        assert_eq!(undo.range, 2..11);
        assert_eq!(undo.text, "old");

        let redo = history.redo().expect("a step back");
        assert_eq!(redo.range, 2..5);
        assert_eq!(redo.text, "new value");
    }

    #[test]
    fn a_new_edit_drops_what_was_undone() {
        let mut history = EditHistory::default();
        typed(&mut history, 0, "a");
        history.undo();
        assert!(history.can_redo());

        typed(&mut history, 0, "b");
        assert!(
            !history.can_redo(),
            "the branch that was undone is not somewhere the reader can still get to"
        );
    }

    #[test]
    fn a_replay_is_not_itself_an_edit() {
        let mut history = EditHistory::default();
        history.record(
            0,
            "a",
            "b",
            EditCause::Replay,
            EditSelection::caret(0),
            EditSelection::caret(1),
        );
        assert!(!history.can_undo());
    }

    #[test]
    fn a_value_the_host_set_ends_the_history() {
        let mut history = EditHistory::default();
        typed(&mut history, 0, "a");
        history.record(
            0,
            "a",
            "from the host",
            EditCause::Programmatic,
            EditSelection::caret(1),
            EditSelection::caret(13),
        );

        assert!(
            !history.can_undo(),
            "there is nothing to walk back to: the value is no longer the one those steps describe"
        );
    }

    #[test]
    fn a_secret_field_remembers_nothing() {
        let mut history = EditHistory::disabled();
        typed(&mut history, 0, "hunter2");

        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn a_composition_is_recorded_once_when_it_ends() {
        let mut history = EditHistory::default();
        history.begin_composition(3..3, "", EditSelection::caret(3));
        // Everything the input method did while composing.
        history.begin_composition(3..3, "に", EditSelection::caret(3));
        history.record(
            3,
            "に",
            "にほ",
            EditCause::Typing,
            EditSelection::caret(4),
            EditSelection::caret(5),
        );
        history.end_composition("日本", EditSelection::caret(9));

        let step = history.undo().expect("one step for the whole run");
        assert_eq!(step.text, "");
        assert!(!history.can_undo());
    }

    #[test]
    fn a_composition_that_settled_on_nothing_records_nothing() {
        let mut history = EditHistory::default();
        history.begin_composition(0..0, "", EditSelection::caret(0));
        history.end_composition("", EditSelection::caret(0));

        assert!(!history.can_undo());
    }
}
