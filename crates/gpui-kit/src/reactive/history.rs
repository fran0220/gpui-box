//! Bounded caller-owned undo and redo records.
//!
//! A [`History`] stores records; it never applies them. The caller decides what
//! a record means and how an undo or redo changes its own state. Recording a
//! new branch clears redo, ignored work is not recorded, and both stacks stay
//! inside the declared bound.

use std::collections::VecDeque;

/// A bounded stack of caller-owned reversible records.
#[derive(Clone, Debug)]
pub struct History<T> {
    capacity: usize,
    undo: VecDeque<T>,
    redo: VecDeque<T>,
    ignoring: bool,
}

impl<T> History<T> {
    /// Creates an empty history holding at most `capacity` records per stack.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            undo: VecDeque::with_capacity(capacity),
            redo: VecDeque::with_capacity(capacity),
            ignoring: false,
        }
    }

    /// Records one reversible change.
    ///
    /// A recorded change starts a new branch and therefore clears redo. While
    /// ignoring, the record is refused and the existing branch is untouched.
    /// Returns whether the record was kept.
    pub fn push(&mut self, record: T) -> bool {
        if self.ignoring || self.capacity == 0 {
            return false;
        }
        self.redo.clear();
        if self.undo.len() == self.capacity {
            self.undo.pop_front();
        }
        self.undo.push_back(record);
        true
    }

    /// Moves the newest undo record to redo and returns it for the caller to
    /// apply in reverse.
    pub fn undo(&mut self) -> Option<T>
    where
        T: Clone,
    {
        let record = self.undo.pop_back()?;
        self.push_redo(record.clone());
        Some(record)
    }

    /// Moves the newest redo record back to undo and returns it for the caller
    /// to reapply.
    pub fn redo(&mut self) -> Option<T>
    where
        T: Clone,
    {
        let record = self.redo.pop_back()?;
        self.push_undo(record.clone());
        Some(record)
    }

    /// Temporarily refuses new records. Undo and redo remain available.
    pub fn set_ignoring(&mut self, ignoring: bool) {
        self.ignoring = ignoring;
    }

    pub fn is_ignoring(&self) -> bool {
        self.ignoring
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// Forgets both branches without changing whether recording is ignored.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    fn push_undo(&mut self, record: T) {
        if self.capacity == 0 {
            return;
        }
        if self.undo.len() == self.capacity {
            self.undo.pop_front();
        }
        self.undo.push_back(record);
    }

    fn push_redo(&mut self, record: T) {
        if self.capacity == 0 {
            return;
        }
        if self.redo.len() == self.capacity {
            self.redo.pop_front();
        }
        self.redo.push_back(record);
    }
}

impl<T> Default for History<T> {
    fn default() -> Self {
        Self::new(1_000)
    }
}

#[cfg(test)]
mod tests {
    use super::History;

    #[test]
    fn a_divergent_push_clears_redo() {
        let mut history = History::new(4);
        history.push("first");
        history.push("second");
        assert_eq!(history.undo(), Some("second"));
        assert!(history.can_redo());

        history.push("branch");

        assert!(!history.can_redo());
        assert_eq!(history.undo(), Some("branch"));
    }

    #[test]
    fn ignored_records_neither_mutate_a_branch_nor_overflow_it() {
        let mut history = History::new(2);
        history.push(1);
        history.push(2);
        assert_eq!(history.undo(), Some(2));
        history.set_ignoring(true);

        assert!(!history.push(3));
        assert_eq!(history.redo(), Some(2));
        assert_eq!(history.undo_len(), 2);
    }

    #[test]
    fn both_directions_stay_bounded_and_keep_the_nearest_records() {
        let mut history = History::new(2);
        history.push(1);
        history.push(2);
        history.push(3);
        assert_eq!(history.undo_len(), 2);
        assert_eq!(history.undo(), Some(3));
        assert_eq!(history.undo(), Some(2));
        assert_eq!(history.undo(), None);
        assert_eq!(history.redo_len(), 2);
        assert_eq!(history.redo(), Some(2));
        assert_eq!(history.redo(), Some(3));
    }

    #[test]
    fn a_zero_capacity_history_records_nothing() {
        let mut history = History::new(0);
        assert!(!history.push("change"));
        assert_eq!(history.undo(), None);
        assert_eq!(history.redo(), None);
    }
}
