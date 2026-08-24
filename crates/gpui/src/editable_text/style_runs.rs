//! Normalized style coverage that follows source replacements.

use std::ops::Range;

use super::buffer::clamp_grapheme_range;
use unicode_segmentation::UnicodeSegmentation;

/// One contiguous style run. Its length is measured in UTF-8 bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditableStyleRun<S> {
    /// The number of source bytes carrying this style.
    pub len: usize,
    /// Product-neutral style metadata owned by the caller.
    pub style: S,
}

/// Complete, normalized style coverage for an editable UTF-8 string.
///
/// Runs never overlap, adjacent equal styles are merged, and their lengths
/// always sum to the associated text length. The collection does not own the
/// text or decide what a style means; rich text, syntax highlighting, and
/// diagnostics can each supply their own style vocabulary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditableStyleRuns<S> {
    default_style: S,
    len: usize,
    runs: Vec<EditableStyleRun<S>>,
}

impl<S: Clone + Eq> EditableStyleRuns<S> {
    /// Covers `text` with one initial style.
    pub fn new(text: &str, style: S) -> Self {
        let runs = (!text.is_empty())
            .then(|| EditableStyleRun {
                len: text.len(),
                style: style.clone(),
            })
            .into_iter()
            .collect();
        Self {
            default_style: style,
            len: text.len(),
            runs,
        }
    }

    /// The UTF-8 byte length covered by these runs.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the associated text is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The normalized runs in source order.
    pub fn runs(&self) -> &[EditableStyleRun<S>] {
        &self.runs
    }

    /// The style at a source offset, preferring the preceding character at
    /// the end of the document.
    pub fn style_at(&self, offset: usize) -> &S {
        if self.runs.is_empty() {
            return &self.default_style;
        }
        let target = offset.min(self.len.saturating_sub(1));
        let mut cursor = 0;
        for run in &self.runs {
            cursor += run.len;
            if target < cursor {
                return &run.style;
            }
        }
        &self.default_style
    }

    /// Applies a style to a grapheme-safe source range.
    pub fn set(&mut self, text: &str, range: Range<usize>, style: S) {
        self.assert_text(text);
        let range = style_range(text, range);
        if range.is_empty() {
            self.default_style = style;
            return;
        }

        let mut next = Vec::with_capacity(self.runs.len() + 2);
        self.copy_segment(0..range.start, &mut next);
        push_run(
            &mut next,
            EditableStyleRun {
                len: range.end - range.start,
                style,
            },
        );
        self.copy_segment(range.end..self.len, &mut next);
        self.runs = next;
    }

    /// Follows one grapheme-safe replacement from `text_before` to the text
    /// obtained by inserting `replacement` at `range`.
    ///
    /// Inserted text inherits the style at the replacement start, or the
    /// preceding style when appending. An empty document uses its remembered
    /// default style, including a style set at an empty caret.
    pub fn replace(&mut self, text_before: &str, range: Range<usize>, replacement: &str) {
        self.assert_text(text_before);
        let range = clamp_grapheme_range(text_before, range);
        let inherited = self.style_at(range.start).clone();
        let mut next = Vec::with_capacity(self.runs.len() + 1);
        self.copy_segment(0..range.start, &mut next);
        if !replacement.is_empty() {
            push_run(
                &mut next,
                EditableStyleRun {
                    len: replacement.len(),
                    style: inherited.clone(),
                },
            );
        }
        self.copy_segment(range.end..self.len, &mut next);
        self.len = self.len - (range.end - range.start) + replacement.len();
        self.runs = next;
        if self.runs.is_empty() {
            self.default_style = inherited;
            return;
        }

        // Grapheme segmentation is not compositional: a combining mark or a
        // ZWJ sequence can join source that used to sit on opposite sides of
        // the replacement. Rebuild coverage by the resulting graphemes so a
        // style boundary can never remain inside the character the user sees.
        let result =
            text_before[..range.start].to_owned() + replacement + &text_before[range.end..];
        let mut normalized = Vec::with_capacity(self.runs.len());
        for (offset, grapheme) in result.grapheme_indices(true) {
            push_run(
                &mut normalized,
                EditableStyleRun {
                    len: grapheme.len(),
                    style: self.style_at(offset).clone(),
                },
            );
        }
        self.runs = normalized;
    }

    fn copy_segment(&self, range: Range<usize>, target: &mut Vec<EditableStyleRun<S>>) {
        if range.is_empty() {
            return;
        }
        let mut start = 0;
        for run in &self.runs {
            let end = start + run.len;
            let overlap_start = start.max(range.start);
            let overlap_end = end.min(range.end);
            if overlap_start < overlap_end {
                push_run(
                    target,
                    EditableStyleRun {
                        len: overlap_end - overlap_start,
                        style: run.style.clone(),
                    },
                );
            }
            start = end;
        }
    }

    fn assert_text(&self, text: &str) {
        assert_eq!(
            text.len(),
            self.len,
            "editable style runs must describe the text being edited"
        );
    }
}

fn push_run<S: Eq>(runs: &mut Vec<EditableStyleRun<S>>, run: EditableStyleRun<S>) {
    if run.len == 0 {
        return;
    }
    if let Some(last) = runs.last_mut()
        && last.style == run.style
    {
        last.len += run.len;
    } else {
        runs.push(run);
    }
}

fn style_range(text: &str, range: Range<usize>) -> Range<usize> {
    let start = text
        .grapheme_indices(true)
        .map(|(offset, _)| offset)
        .take_while(|offset| *offset <= range.start.min(text.len()))
        .last()
        .unwrap_or(0);
    let requested_end = range.end.min(text.len());
    let end = if requested_end == text.len() || text.is_char_boundary(requested_end) {
        text.grapheme_indices(true)
            .map(|(offset, _)| offset)
            .find(|offset| *offset >= requested_end)
            .unwrap_or(text.len())
    } else {
        text.grapheme_indices(true)
            .map(|(offset, _)| offset)
            .find(|offset| *offset > requested_end)
            .unwrap_or(text.len())
    };
    start.min(end)..start.max(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Mark {
        Plain,
        Strong,
        Code,
    }

    fn lengths(runs: &EditableStyleRuns<Mark>) -> Vec<(usize, Mark)> {
        runs.runs().iter().map(|run| (run.len, run.style)).collect()
    }

    #[test]
    fn styles_split_and_merge_without_losing_coverage() {
        let mut runs = EditableStyleRuns::new("alpha beta", Mark::Plain);
        runs.set("alpha beta", 0..5, Mark::Strong);
        runs.set("alpha beta", 6..10, Mark::Strong);
        assert_eq!(
            lengths(&runs),
            vec![(5, Mark::Strong), (1, Mark::Plain), (4, Mark::Strong)]
        );

        runs.set("alpha beta", 5..6, Mark::Strong);
        assert_eq!(lengths(&runs), vec![(10, Mark::Strong)]);
    }

    #[test]
    fn replacement_inherits_and_moves_the_surrounding_runs() {
        let mut runs = EditableStyleRuns::new("alpha beta", Mark::Plain);
        runs.set("alpha beta", 6..10, Mark::Code);
        runs.replace("alpha beta", 6..10, "γ");
        assert_eq!(runs.len(), "alpha γ".len());
        assert_eq!(
            lengths(&runs),
            vec![(6, Mark::Plain), ("γ".len(), Mark::Code)]
        );

        runs.replace("alpha γ", 5..6, " ");
        assert_eq!(lengths(&runs), vec![(6, Mark::Plain), (2, Mark::Code)]);
    }

    #[test]
    fn empty_caret_style_becomes_the_style_of_new_text() {
        let mut runs = EditableStyleRuns::new("", Mark::Plain);
        runs.set("", 0..0, Mark::Strong);
        runs.replace("", 0..0, "é");
        assert_eq!(lengths(&runs), vec![(2, Mark::Strong)]);
    }

    #[test]
    fn style_operations_snap_to_whole_graphemes() {
        let text = "a👩‍💻b";
        let mut runs = EditableStyleRuns::new(text, Mark::Plain);
        runs.set(text, 2..7, Mark::Strong);
        assert_eq!(
            lengths(&runs),
            vec![
                (1, Mark::Plain),
                ("👩‍💻".len(), Mark::Strong),
                (1, Mark::Plain)
            ]
        );
    }

    #[test]
    fn a_replacement_cannot_leave_a_style_boundary_inside_a_new_grapheme() {
        let mut runs = EditableStyleRuns::new("ab", Mark::Plain);
        runs.set("ab", 1..2, Mark::Strong);
        runs.replace("ab", 1..1, "\u{301}");
        assert_eq!(
            lengths(&runs),
            vec![("a\u{301}".len(), Mark::Plain), (1, Mark::Strong)]
        );
    }

    #[test]
    fn deleting_everything_remembers_the_style_at_the_replacement() {
        let mut runs = EditableStyleRuns::new("bold", Mark::Strong);
        runs.replace("bold", 0..4, "");
        runs.replace("", 0..0, "again");
        assert_eq!(lengths(&runs), vec![(5, Mark::Strong)]);
    }
}
