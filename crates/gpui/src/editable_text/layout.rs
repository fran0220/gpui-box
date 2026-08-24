//! Shared geometry for editable shaped text.
//!
//! A plain field, rich-text editor, input method, find overlay, and native
//! accessibility adapter must agree about where a byte offset was painted.
//! This layout is that authority: it translates between hard lines, wrapped
//! visual rows, UTF-8 offsets, points, selections, and carets using the same
//! [`WrappedLine`] values that are painted.

use std::ops::Range;

use crate::{Bounds, Pixels, Point, TextAlign, WrappedLine, point, px, size};

/// Far enough right to land past the end of any shaped row.
const PAST_END: Pixels = px(1.0e6);

/// The shaped geometry of one editable UTF-8 document.
pub struct EditableTextLayout {
    lines: Vec<WrappedLine>,
    /// Where each hard line starts in the source text.
    starts: Vec<usize>,
    /// The first visual row of each hard line.
    rows: Vec<usize>,
    total_rows: usize,
    text_len: usize,
    line_height: Pixels,
}

impl std::fmt::Debug for EditableTextLayout {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Editable content may contain secrets, so only its shape is printed.
        formatter
            .debug_struct("EditableTextLayout")
            .field("lines", &self.lines.len())
            .field("rows", &self.total_rows)
            .field("text_len", &self.text_len)
            .finish()
    }
}

impl EditableTextLayout {
    /// Builds document geometry from the shaped hard lines that will paint.
    pub fn new(text: &str, lines: Vec<WrappedLine>, line_height: Pixels) -> Self {
        let starts = std::iter::once(0)
            .chain(
                text.match_indices('\n')
                    .map(|(offset, character)| offset + character.len()),
            )
            .collect::<Vec<_>>();
        debug_assert_eq!(starts.len(), lines.len());
        let mut rows = Vec::with_capacity(lines.len());
        let mut row = 0;
        for line in &lines {
            rows.push(row);
            row += line.wrap_boundaries().len() + 1;
        }
        Self {
            lines,
            starts,
            rows,
            total_rows: row.max(1),
            text_len: text.len(),
            line_height,
        }
    }

    /// The line height used to shape and paint every visual row.
    pub fn line_height(&self) -> Pixels {
        self.line_height
    }

    /// The number of shaped visual rows, including an empty final row.
    pub fn total_rows(&self) -> usize {
        self.total_rows
    }

    /// The UTF-8 byte length represented by this layout.
    pub fn len(&self) -> usize {
        self.text_len
    }

    /// Returns whether this layout represents no source bytes.
    pub fn is_empty(&self) -> bool {
        self.text_len == 0
    }

    /// The full shaped document height.
    pub fn height(&self) -> Pixels {
        self.line_height * self.total_rows as f32
    }

    /// How wide the widest hard line would be if nothing had wrapped it.
    pub fn text_width(&self) -> Pixels {
        self.lines
            .iter()
            .map(|line| line.unwrapped_layout.width)
            .fold(px(0.0), Pixels::max)
    }

    /// Where each hard line should be painted relative to the text origin.
    pub fn painted_lines(&self) -> impl Iterator<Item = (&WrappedLine, Pixels)> {
        self.lines
            .iter()
            .zip(&self.rows)
            .map(|(line, row)| (line, self.line_height * *row as f32))
    }

    /// The top-left position of a UTF-8 byte offset relative to the text origin.
    pub fn position_for_offset(&self, offset: usize) -> Point<Pixels> {
        if self.lines.is_empty() {
            return point(px(0.0), px(0.0));
        }
        let offset = offset.min(self.text_len);
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

    /// The visual row containing a UTF-8 byte offset.
    pub fn row_for_offset(&self, offset: usize) -> usize {
        let y = self.position_for_offset(offset).y;
        ((y / self.line_height) as usize).min(self.total_rows - 1)
    }

    /// The UTF-8 offset nearest a point relative to the text origin.
    pub fn offset_for_position(&self, position: Point<Pixels>) -> usize {
        let row = if position.y < px(0.0) {
            0
        } else {
            ((position.y / self.line_height) as usize).min(self.total_rows - 1)
        };
        self.offset_at_row(row, position.x)
    }

    /// The offset at an x coordinate on one visual row.
    pub fn offset_at_row(&self, row: usize, x: Pixels) -> usize {
        if self.lines.is_empty() {
            return 0;
        }
        let row = row.min(self.total_rows - 1);
        let index = self.line_for_row(row);
        let line = &self.lines[index];
        // Sample halfway down the row so exact row boundaries cannot round to
        // the preceding row in a platform text implementation.
        let local = point(
            x,
            self.line_height * (row - self.rows[index]) as f32 + self.line_height / 2.0,
        );
        let offset = line
            .closest_index_for_position(local, self.line_height)
            .unwrap_or_else(|offset| offset);
        (self.starts[index] + offset).min(self.text_len)
    }

    /// The source byte range occupied by a visual row.
    pub fn row_range(&self, row: usize) -> Range<usize> {
        self.offset_at_row(row, px(-1.0))..self.offset_at_row(row, PAST_END)
    }

    /// Source byte ranges for every shaped visual row.
    ///
    /// The final row of a hard line owns its line break. GPUI shapes hard
    /// lines separately, but selection and accessibility must retain the
    /// break as one source character on the preceding row.
    pub fn visual_rows(&self, text: &str) -> Vec<Range<usize>> {
        debug_assert_eq!(text.len(), self.text_len);
        (0..self.total_rows)
            .map(|row| {
                let mut range = self.row_range(row);
                if range.start > range.end {
                    std::mem::swap(&mut range.start, &mut range.end);
                }
                range.start = range.start.min(text.len());
                range.end = range.end.min(text.len());
                if row + 1 == self.total_rows
                    || self.line_for_row(row + 1) != self.line_for_row(row)
                {
                    if text[range.end..].starts_with("\r\n") {
                        range.end += 2;
                    } else if text[range.end..].starts_with('\n') {
                        range.end += 1;
                    }
                }
                range
            })
            .collect()
    }

    /// Painted rectangles occupied by a logical source range.
    ///
    /// Wrapped and bidirectional ranges may produce several rectangles. The
    /// returned bounds are relative to `origin` and use the same alignment
    /// and width that the caller will pass to [`WrappedLine::paint`].
    pub fn bounds_for_range(
        &self,
        range: Range<usize>,
        origin: Point<Pixels>,
        align: TextAlign,
        align_width: Pixels,
    ) -> Vec<Bounds<Pixels>> {
        let start = range.start.min(self.text_len);
        let end = range.end.min(self.text_len);
        if start >= end {
            return Vec::new();
        }

        let mut result = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            let line_start = self.starts[index];
            let line_end = line_start + line.len();
            let local_start = start.max(line_start).min(line_end) - line_start;
            let local_end = end.max(line_start).min(line_end) - line_start;
            if local_start < local_end {
                result.extend(line.bounds_for_range(
                    local_start..local_end,
                    point(
                        origin.x,
                        origin.y + self.line_height * self.rows[index] as f32,
                    ),
                    self.line_height,
                    align,
                    align_width,
                ));
            }
        }
        result
    }

    /// The caret rectangle for an offset, relative to `origin`.
    pub fn caret_bounds(
        &self,
        offset: usize,
        origin: Point<Pixels>,
        width: Pixels,
    ) -> Bounds<Pixels> {
        let position = self.position_for_offset(offset);
        Bounds::new(
            point(origin.x + position.x, origin.y + position.y),
            size(width, self.line_height),
        )
    }

    /// The smallest rectangle containing a logical range.
    ///
    /// Input-method and native accessibility APIs often accept one rectangle
    /// even though wrapped or bidirectional text occupies several. Painting
    /// should use [`Self::bounds_for_range`]; this method is the explicit
    /// lossy projection for those single-rectangle platform boundaries.
    pub fn enclosing_bounds_for_range(
        &self,
        range: Range<usize>,
        origin: Point<Pixels>,
        align: TextAlign,
        align_width: Pixels,
    ) -> Bounds<Pixels> {
        if range.is_empty() {
            return self.caret_bounds(range.start, origin, px(0.0));
        }
        let mut fragments = self
            .bounds_for_range(range.clone(), origin, align, align_width)
            .into_iter();
        let Some(first) = fragments.next() else {
            return self.caret_bounds(range.start, origin, px(0.0));
        };
        fragments.fold(first, |bounds, fragment| {
            Bounds::from_corners(
                point(
                    bounds.left().min(fragment.left()),
                    bounds.top().min(fragment.top()),
                ),
                point(
                    bounds.right().max(fragment.right()),
                    bounds.bottom().max(fragment.bottom()),
                ),
            )
        })
    }

    /// A horizontal scroll offset that reveals the caret while moving no
    /// more than necessary.
    pub fn horizontal_scroll_offset_to_reveal(
        &self,
        offset: usize,
        viewport_width: Pixels,
        current: Pixels,
    ) -> Pixels {
        let caret = self.position_for_offset(offset);
        let mut scroll = current
            .min((self.text_width() - viewport_width).max(px(0.0)))
            .max(px(0.0));
        if caret.x < scroll {
            scroll = caret.x;
        }
        if caret.x > scroll + viewport_width {
            scroll = caret.x - viewport_width;
        }
        scroll
    }

    /// A vertical scroll offset that reveals the caret while moving no more
    /// than necessary.
    pub fn scroll_offset_to_reveal(
        &self,
        offset: usize,
        viewport_height: Pixels,
        current: Pixels,
    ) -> Pixels {
        let caret = self.position_for_offset(offset);
        let mut scroll = current
            .min((self.height() - viewport_height).max(px(0.0)))
            .max(px(0.0));
        if caret.y < scroll {
            scroll = caret.y;
        }
        if caret.y + self.line_height > scroll + viewport_height {
            scroll = caret.y + self.line_height - viewport_height;
        }
        scroll
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
