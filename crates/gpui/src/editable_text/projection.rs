//! Product-neutral projection of complete source lines onto displayed rows.
//! Policy (fold identities, toggles, automatic expansion) belongs to callers.
//! This index stores only visible spans, never one entry per document line.

use std::{ops::Range, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Span {
    source: Range<usize>,
    row: usize,
}

/// An immutable source-line ↔ displayed-row mapping. Omitted source remains
/// in the document; this changes geometry, never editing coordinates or text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditableLineProjection {
    source_lines: usize,
    rows: usize,
    spans: Arc<[Span]>,
}

impl EditableLineProjection {
    /// Builds an identity or omitted-line projection. Ranges are zero-based,
    /// half-open source-line indices. Overlap/nesting is unioned. Invalid
    /// bounds, an empty document line index, or hiding every line are refused.
    pub fn new(
        source_lines: usize,
        omitted: impl IntoIterator<Item = Range<usize>>,
    ) -> Option<Self> {
        if source_lines == 0 {
            return None;
        }
        let mut omitted: Vec<_> = omitted.into_iter().collect();
        if omitted
            .iter()
            .any(|range| range.start > range.end || range.end > source_lines)
        {
            return None;
        }
        omitted.sort_unstable_by_key(|range| (range.start, range.end));
        let mut spans = Vec::with_capacity(omitted.len() + 1);
        let mut source = 0;
        let mut rows = 0;
        for range in omitted {
            if range.is_empty() {
                continue;
            }
            if source < range.start {
                spans.push(Span {
                    source: source..range.start,
                    row: rows,
                });
                rows += range.start - source;
            }
            source = source.max(range.end);
        }
        if source < source_lines {
            spans.push(Span {
                source: source..source_lines,
                row: rows,
            });
            rows += source_lines - source;
        }
        (rows > 0).then(|| Self {
            source_lines,
            rows,
            spans: spans.into(),
        })
    }

    /// Number of source hard lines, including an empty final line.
    pub fn source_lines(&self) -> usize {
        self.source_lines
    }

    /// Number of displayed hard rows after omissions.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Source line for a displayed row, clamped at the last displayed row.
    pub fn source_line(&self, row: usize) -> usize {
        let row = row.min(self.rows - 1);
        let index = self
            .spans
            .partition_point(|span| span.row <= row)
            .saturating_sub(1);
        self.spans[index].source.start + row - self.spans[index].row
    }

    /// Displayed row for a source line. Omitted lines map to the preceding
    /// displayed row, or row zero when the omitted range precedes all content.
    pub fn row(&self, line: usize) -> usize {
        let index = self
            .spans
            .partition_point(|span| span.source.start <= line)
            .saturating_sub(1);
        let span = &self.spans[index];
        span.row
            + line
                .saturating_sub(span.source.start)
                .min(span.source.len() - 1)
    }

    /// Whether a source line has its own displayed row.
    pub fn contains(&self, line: usize) -> bool {
        let index = self
            .spans
            .partition_point(|span| span.source.start <= line)
            .saturating_sub(1);
        self.spans[index].source.contains(&line)
    }

    pub(super) fn lines_in(&self, range: Range<usize>) -> impl Iterator<Item = usize> + '_ {
        let first = self
            .spans
            .partition_point(|span| span.source.end <= range.start);
        self.spans[first..]
            .iter()
            .take_while(move |span| span.source.start < range.end)
            .flat_map(move |span| {
                span.source.start.max(range.start)..span.source.end.min(range.end)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_omissions_preserve_asymmetric_source_identities_and_boundaries() {
        let projection = EditableLineProjection::new(20, [8..13, 3..7, 4..6]).expect("projection");
        let visible = [0, 1, 2, 7, 13, 14, 15, 16, 17, 18, 19];
        assert_eq!(projection.rows(), visible.len());
        for (row, source) in visible.into_iter().enumerate() {
            assert_eq!(projection.source_line(row), source);
            assert_eq!(projection.row(source), row);
            assert!(projection.contains(source));
        }
        assert_eq!(projection.row(6), 2);
        assert_eq!(projection.row(8), 3);
        assert!(!projection.contains(12));
        assert_eq!(
            projection.lines_in(2..15).collect::<Vec<_>>(),
            [2, 7, 13, 14]
        );
    }

    #[test]
    fn large_omission_is_indexed_by_spans_not_document_length() {
        let projection = EditableLineProjection::new(1_000_000, std::iter::once(1..999_990))
            .expect("projection");
        assert_eq!(projection.spans.len(), 2);
        assert_eq!(projection.rows(), 11);
        assert_eq!(projection.source_line(1), 999_990);
        assert_eq!(projection.row(900_000), 0);
        assert!(EditableLineProjection::new(4, std::iter::once(0..4)).is_none());
        assert!(EditableLineProjection::new(4, std::iter::once(3..5)).is_none());
        let leading =
            EditableLineProjection::new(4, std::iter::once(0..2)).expect("leading omission");
        assert_eq!(leading.row(0), 0);
        assert_eq!(leading.source_line(0), 2);
    }
}
