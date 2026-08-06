//! The wrapped shaping a [`super::TextArea`] measures, paints, and hits.
//!
//! GPUI shapes one [`WrappedLine`] per hard line, each carrying its own wrap
//! boundaries. Motion and hit testing want a single sequence of visual rows
//! across the whole document, so this translates between the two: a content
//! offset, a row index, and a point are three views of the same place.

use std::ops::Range;

use gpui::{Pixels, Point, WrappedLine, point, px};

/// Far enough right to land past the end of any row.
const PAST_END: Pixels = px(1.0e6);

pub struct Layout {
    lines: Vec<WrappedLine>,
    /// Where each hard line starts in the shaped text.
    starts: Vec<usize>,
    /// The first visual row of each hard line.
    rows: Vec<usize>,
    total_rows: usize,
    line_height: Pixels,
}

impl std::fmt::Debug for Layout {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The shaped text is the typist's, so only its shape is printed.
        formatter
            .debug_struct("Layout")
            .field("lines", &self.lines.len())
            .field("rows", &self.total_rows)
            .finish()
    }
}

impl Layout {
    pub fn new(lines: Vec<WrappedLine>, line_height: Pixels) -> Self {
        let mut starts = Vec::with_capacity(lines.len());
        let mut rows = Vec::with_capacity(lines.len());
        let mut offset = 0;
        let mut row = 0;
        for line in &lines {
            starts.push(offset);
            rows.push(row);
            offset += line.len() + '\n'.len_utf8();
            row += line.wrap_boundaries().len() + 1;
        }
        Self {
            lines,
            starts,
            rows,
            total_rows: row.max(1),
            line_height,
        }
    }

    pub fn line_height(&self) -> Pixels {
        self.line_height
    }

    pub fn total_rows(&self) -> usize {
        self.total_rows
    }

    pub fn height(&self) -> Pixels {
        self.line_height * self.total_rows as f32
    }

    /// Where each hard line should be painted, relative to the text origin.
    pub fn painted_lines(&self) -> impl Iterator<Item = (&WrappedLine, Pixels)> {
        self.lines
            .iter()
            .zip(&self.rows)
            .map(|(line, row)| (line, self.line_height * *row as f32))
    }

    /// The top left of the character at an offset, relative to the text
    /// origin and before scrolling.
    pub fn position_for_offset(&self, offset: usize) -> Point<Pixels> {
        if self.lines.is_empty() {
            return point(px(0.0), px(0.0));
        }
        let index = self.line_for_offset(offset);
        let local = offset.saturating_sub(self.starts[index]);
        let line = &self.lines[index];
        let position = line
            .position_for_index(local.min(line.len()), self.line_height)
            .unwrap_or_default();
        point(
            position.x,
            position.y + self.line_height * self.rows[index] as f32,
        )
    }

    pub fn row_for_offset(&self, offset: usize) -> usize {
        let y = self.position_for_offset(offset).y;
        ((y / self.line_height) as usize).min(self.total_rows - 1)
    }

    /// The offset nearest a point relative to the text origin.
    pub fn offset_for_position(&self, position: Point<Pixels>) -> usize {
        let row = if position.y < px(0.0) {
            0
        } else {
            ((position.y / self.line_height) as usize).min(self.total_rows - 1)
        };
        self.offset_at_row(row, position.x)
    }

    /// The offset at a column of one visual row, which is what vertical
    /// motion with a goal column asks for.
    pub fn offset_at_row(&self, row: usize, x: Pixels) -> usize {
        if self.lines.is_empty() {
            return 0;
        }
        let row = row.min(self.total_rows - 1);
        let index = self.line_for_row(row);
        let line = &self.lines[index];
        // Half a row down, because the row index is recovered by a floor
        // division that an exact multiple can land just below.
        let local = point(
            x,
            self.line_height * (row - self.rows[index]) as f32 + self.line_height / 2.0,
        );
        let offset = line
            .closest_index_for_position(local, self.line_height)
            .unwrap_or_else(|offset| offset);
        self.starts[index] + offset
    }

    /// The offsets a visual row spans, for a home or end key.
    pub fn row_range(&self, row: usize) -> Range<usize> {
        self.offset_at_row(row, px(-1.0))..self.offset_at_row(row, PAST_END)
    }

    fn line_for_offset(&self, offset: usize) -> usize {
        self.starts
            .iter()
            .rposition(|start| *start <= offset)
            .unwrap_or(0)
    }

    fn line_for_row(&self, row: usize) -> usize {
        self.rows
            .iter()
            .rposition(|first| *first <= row)
            .unwrap_or(0)
    }
}
