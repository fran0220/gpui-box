//! Shared geometry for editable shaped text.
//!
//! A plain field, rich-text editor, input method, find overlay, and native
//! accessibility adapter must agree about where a byte offset was painted.
//! This layout is that authority: it translates between hard lines, wrapped
//! visual rows, UTF-8 offsets, points, selections, and carets using the same
//! [`WrappedLine`] values that are painted.

mod wrapped;
pub use wrapped::EditableWrappedCache;

use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    ops::Range,
    rc::Rc,
    sync::Arc,
};

use crate::{
    Bounds, EditSnapshot, EditableLineProjection, Pixels, Point, TextAlign, TextRun,
    WindowTextSystem, WrappedLine, point, px, size,
};

/// Far enough right to land past the end of any shaped row.
const PAST_END: Pixels = px(1.0e6);

/// The shaped geometry of one editable UTF-8 document.
#[derive(Clone)]
pub struct EditableTextLayout {
    lines: Arc<[Arc<WrappedLine>]>,
    /// Where each hard line starts in the source text.
    starts: Arc<[usize]>,
    /// The first visual row of each hard line.
    rows: Arc<[usize]>,
    total_rows: usize,
    text_len: usize,
    line_height: Pixels,
    source: Option<Rc<SourceLines>>,
    paint_rows: Option<Range<usize>>,
    work: Option<EditableTextWork>,
    width: Pixels,
}

/// Actual shaping input consumed by one layout, including on-demand geometry.
/// Bytes count UTF-8 input, not allocation calls or a guessed glyph count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditableTextWork {
    /// Number of hard lines actually passed to the shaper.
    pub shaped_lines: usize,
    /// Sum of UTF-8 bytes actually passed to the shaper.
    pub shaped_bytes: usize,
}

#[derive(Clone)]
struct SourceLines {
    document: EditSnapshot,
    projection: EditableLineProjection,
    text_system: Arc<WindowTextSystem>,
    font_size: Pixels,
    runs: Vec<TextRun>,
    run_starts: Vec<usize>,
    visible: Range<usize>,
    shaped: RefCell<BTreeMap<usize, Arc<WrappedLine>>>,
    work: Cell<EditableTextWork>,
}

impl SourceLines {
    fn line(&self, row: usize) -> Arc<WrappedLine> {
        if let Some(line) = self.shaped.borrow().get(&row) {
            return line.clone();
        }
        let range = self.document.line_range(row).expect("indexed source row");
        let mut text = self
            .document
            .slice(range.clone())
            .expect("indexed source range");
        if text.ends_with('\n') {
            text.pop();
        }
        let end = range.start + text.len();
        let first = self
            .run_starts
            .partition_point(|start| *start <= range.start)
            .saturating_sub(1);
        let runs = self
            .runs
            .iter()
            .zip(&self.run_starts)
            .skip(first)
            .take_while(|(_, start)| **start < end)
            .filter_map(|(run, start)| {
                let len = (start + run.len)
                    .min(end)
                    .saturating_sub((*start).max(range.start));
                (len > 0).then(|| {
                    let mut run = run.clone();
                    run.len = len;
                    run
                })
            })
            .collect::<Vec<_>>();
        let mut work = self.work.get();
        work.shaped_lines += 1;
        work.shaped_bytes += text.len();
        self.work.set(work);
        let line = Arc::new(
            self.text_system
                .shape_text(text.into(), self.font_size, &runs, None, None)
                .ok()
                .and_then(|mut lines| lines.pop())
                .unwrap_or_default(),
        );
        self.shaped.borrow_mut().insert(row, line.clone());
        line
    }
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
        let width = lines
            .iter()
            .map(|line| line.unwrapped_layout.width)
            .fold(px(0.0), Pixels::max);
        Self {
            lines: lines.into_iter().map(Arc::new).collect(),
            starts: starts.into(),
            rows: rows.into(),
            total_rows: row.max(1),
            text_len: text.len(),
            line_height,
            source: None,
            paint_rows: None,
            work: None,
            width,
        }
    }

    /// No-wrap document layout with indexed hard lines. Only `visible` rows
    /// paint; geometry queries shape other rows lazily through the same text
    /// system and style partition, so hit testing and IME never guess widths.
    /// `painted_bounds_for_range` limits selection painting to visible rows;
    /// ordinary geometry queries address every offset. Soft wrapping uses `new`.
    pub fn unwrapped(
        document: EditSnapshot,
        text_system: Arc<WindowTextSystem>,
        font_size: Pixels,
        line_height: Pixels,
        runs: Vec<TextRun>,
        visible: Range<usize>,
    ) -> Self {
        let projection = EditableLineProjection::new(document.line_count(), [])
            .expect("nonempty source line index");
        Self::unwrapped_projected(
            document,
            text_system,
            font_size,
            line_height,
            runs,
            visible,
            projection,
        )
        .expect("identity projection")
    }

    /// No-wrap geometry with omitted complete source lines. `visible` indexes
    /// projected rows, not source lines. Painting, hit testing, selections and
    /// IME share this mapping; omitted positions map to the preceding displayed
    /// row's end, or the first row's start when no predecessor exists. Returns
    /// None for a projection of a different source line count.
    #[allow(clippy::too_many_arguments)]
    pub fn unwrapped_projected(
        document: EditSnapshot,
        text_system: Arc<WindowTextSystem>,
        font_size: Pixels,
        line_height: Pixels,
        runs: Vec<TextRun>,
        visible: Range<usize>,
        projection: EditableLineProjection,
    ) -> Option<Self> {
        if projection.source_lines() != document.line_count() {
            return None;
        }
        let mut start = 0;
        let run_starts = runs
            .iter()
            .map(|run| {
                let at = start;
                start += run.len;
                at
            })
            .collect();
        let total_rows = projection.rows();
        Some(Self {
            lines: Arc::default(),
            starts: Arc::default(),
            rows: Arc::default(),
            total_rows,
            text_len: document.len(),
            line_height,
            source: Some(Rc::new(SourceLines {
                document,
                projection,
                text_system,
                font_size,
                runs,
                run_starts,
                visible: visible.start.min(total_rows)..visible.end.min(total_rows),
                shaped: RefCell::new(BTreeMap::new()),
                work: Cell::default(),
            })),
            paint_rows: None,
            work: None,
            width: px(0.0),
        })
    }

    /// Shaping performed so far by this layout, including on-demand queries.
    pub fn shaping_work(&self) -> EditableTextWork {
        if let Some(work) = self.work {
            return work;
        }
        self.source
            .as_ref()
            .map(|source| source.work.get())
            .unwrap_or(EditableTextWork {
                shaped_lines: self.lines.len(),
                shaped_bytes: self.text_len,
            })
    }

    /// Source ranges eligible to paint in this layout. Accessibility geometry
    /// collectors can skip unpainted text without losing its logical content.
    pub fn painted_source_ranges(&self) -> Vec<Range<usize>> {
        if let Some(source) = &self.source {
            return source
                .visible
                .clone()
                .filter_map(|row| {
                    source
                        .document
                        .line_range(source.projection.source_line(row))
                })
                .collect();
        }
        if let Some(rows) = &self.paint_rows {
            return rows
                .clone()
                .map(|row| {
                    let range = self.row_range(row);
                    range.start.min(range.end)..range.start.max(range.end)
                })
                .collect();
        }
        std::iter::once(0..self.text_len).collect()
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

    /// Restricts painting and painted-range geometry to these visual rows.
    /// Logical geometry and native text remain complete. Rows are clamped to
    /// the exact index, so this can be applied after scrolling/caret reveal.
    pub fn set_painted_rows(&mut self, rows: Range<usize>) {
        let end = rows.end.min(self.total_rows);
        let rows = rows.start.min(end)..end;
        if let Some(source) = &mut self.source {
            Rc::make_mut(source).visible = rows;
        } else {
            self.paint_rows = Some(rows);
        }
    }

    /// Proves that two layouts share the same logical row index without
    /// enumerating source lines. False means unknown, not necessarily unequal.
    /// Viewport changes and line height do not change source row boundaries.
    pub fn shares_row_index_with(&self, other: &Self) -> bool {
        match (&self.source, &other.source) {
            (Some(left), Some(right)) => {
                left.document.shares_storage_with(&right.document)
                    && left.projection == right.projection
            }
            (None, None) => {
                Arc::ptr_eq(&self.lines, &other.lines) && Arc::ptr_eq(&self.starts, &other.starts)
            }
            _ => false,
        }
    }

    /// The full shaped document height.
    pub fn height(&self) -> Pixels {
        self.line_height * self.total_rows as f32
    }

    /// Width of the widest measured hard line. A lazy no-wrap document reports
    /// the maximum of visible and explicitly queried lines, not unseen lines.
    pub fn text_width(&self) -> Pixels {
        if let Some(source) = &self.source {
            return source
                .shaped
                .borrow()
                .values()
                .map(|line| line.unwrapped_layout.width)
                .fold(px(0.0), Pixels::max);
        }
        self.width
    }

    /// Where each hard line should be painted relative to the text origin.
    pub fn painted_lines(&self) -> Box<dyn Iterator<Item = (Arc<WrappedLine>, Pixels)> + '_> {
        if let Some(source) = &self.source {
            return Box::new(source.visible.clone().map(|row| {
                (
                    source.line(source.projection.source_line(row)),
                    self.line_height * row as f32,
                )
            }));
        }
        let lines = self
            .paint_rows
            .as_ref()
            .map_or(0..self.lines.len(), |rows| {
                if rows.is_empty() {
                    0..0
                } else {
                    self.line_for_row(rows.start)..self.line_for_row(rows.end - 1) + 1
                }
            });
        Box::new(
            self.lines[lines.clone()]
                .iter()
                .zip(self.rows[lines].iter())
                .map(|(line, row)| (line.clone(), self.line_height * *row as f32)),
        )
    }

    /// The top-left position of a UTF-8 byte offset relative to the text origin.
    pub fn position_for_offset(&self, offset: usize) -> Point<Pixels> {
        if self.source.is_some() {
            return self.position_for_offset_aligned(offset, TextAlign::Left, px(0.0));
        }
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

    /// The painted position of an offset when every hard line uses the same
    /// alignment and width.
    pub fn position_for_offset_aligned(
        &self,
        offset: usize,
        align: TextAlign,
        align_width: Pixels,
    ) -> Point<Pixels> {
        if let Some(source) = &self.source {
            let offset = offset.min(self.text_len);
            let source_line = source.document.line_at(offset);
            let row = source.projection.row(source_line);
            let displayed_line = source.projection.source_line(row);
            let start = source
                .document
                .line_range(displayed_line)
                .expect("indexed source row")
                .start;
            let line = source.line(displayed_line);
            let position = line
                .position_for_index_aligned(
                    offset.saturating_sub(start).min(line.len()),
                    self.line_height,
                    align,
                    align_width,
                )
                .unwrap_or_default();
            return point(position.x, position.y + self.line_height * row as f32);
        }
        if self.lines.is_empty() {
            return point(px(0.0), px(0.0));
        }
        let offset = offset.min(self.text_len);
        let index = self.line_for_offset(offset);
        let local = offset.saturating_sub(self.starts[index]);
        let line = &self.lines[index];
        let position = line
            .position_for_index_aligned(local.min(line.len()), self.line_height, align, align_width)
            .unwrap_or_default();
        point(
            position.x,
            position.y + self.line_height * self.rows[index] as f32,
        )
    }

    /// The visual row containing a UTF-8 byte offset.
    pub fn row_for_offset(&self, offset: usize) -> usize {
        if let Some(source) = &self.source {
            return source.projection.row(source.document.line_at(offset));
        }
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

    /// The offset nearest a painted point when every hard line uses the same
    /// alignment and width.
    pub fn offset_for_position_aligned(
        &self,
        position: Point<Pixels>,
        align: TextAlign,
        align_width: Pixels,
    ) -> usize {
        let row = if position.y < px(0.0) {
            0
        } else {
            ((position.y / self.line_height) as usize).min(self.total_rows - 1)
        };
        self.offset_at_row_aligned(row, position.x, align, align_width)
    }

    /// The offset at an x coordinate on one visual row.
    pub fn offset_at_row(&self, row: usize, x: Pixels) -> usize {
        if self.source.is_some() {
            return self.offset_at_row_aligned(row, x, TextAlign::Left, px(0.0));
        }
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

    /// The offset at an x coordinate on one aligned visual row.
    pub fn offset_at_row_aligned(
        &self,
        row: usize,
        x: Pixels,
        align: TextAlign,
        align_width: Pixels,
    ) -> usize {
        if let Some(source) = &self.source {
            let row = source.projection.source_line(row);
            let start = source
                .document
                .line_range(row)
                .expect("indexed source row")
                .start;
            let local = source
                .line(row)
                .closest_index_for_position_aligned(
                    point(x, self.line_height / 2.0),
                    self.line_height,
                    align,
                    align_width,
                )
                .unwrap_or_else(|offset| offset);
            return (start + local).min(self.text_len);
        }
        if self.lines.is_empty() {
            return 0;
        }
        let row = row.min(self.total_rows - 1);
        let index = self.line_for_row(row);
        let line = &self.lines[index];
        let local = point(
            x,
            self.line_height * (row - self.rows[index]) as f32 + self.line_height / 2.0,
        );
        let offset = line
            .closest_index_for_position_aligned(local, self.line_height, align, align_width)
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
        if let Some(source) = &self.source {
            return (0..self.total_rows)
                .filter_map(|row| {
                    source
                        .document
                        .line_range(source.projection.source_line(row))
                })
                .collect();
        }
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

    /// Selection rectangles limited to rows eligible to paint. Unlike ordinary
    /// range geometry, selecting the whole document never shapes hidden rows.
    pub fn painted_bounds_for_range(
        &self,
        mut range: Range<usize>,
        origin: Point<Pixels>,
        align: TextAlign,
        align_width: Pixels,
    ) -> Vec<Bounds<Pixels>> {
        if let Some(source) = &self.source {
            if source.visible.is_empty() {
                return Vec::new();
            }
            let start = source
                .document
                .line_range(source.projection.source_line(source.visible.start))
                .expect("visible first row")
                .start;
            let end = source
                .document
                .line_range(source.projection.source_line(source.visible.end - 1))
                .expect("visible last row")
                .end;
            range.start = range.start.max(start);
            range.end = range.end.min(end);
        }
        if let Some(rows) = &self.paint_rows {
            if rows.is_empty() {
                return Vec::new();
            }
            let first = self.row_range(rows.start);
            let last = self.row_range(rows.end - 1);
            range.start = range.start.max(first.start.min(first.end));
            range.end = range.end.min(last.start.max(last.end));
        }
        self.bounds_for_range(range, origin, align, align_width)
    }

    /// Rectangles occupied by a logical source range, including offscreen rows.
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
        if let Some(source) = &self.source {
            let first = source.document.line_at(start);
            let last = source.document.line_at(end - 1) + 1;
            for row in source.projection.lines_in(first..last) {
                let line_start = source
                    .document
                    .line_range(row)
                    .expect("indexed source row")
                    .start;
                let line = source.line(row);
                let local_start = start.saturating_sub(line_start).min(line.len());
                let local_end = end.saturating_sub(line_start).min(line.len());
                if local_start < local_end {
                    result.extend(line.bounds_for_range(
                        local_start..local_end,
                        point(
                            origin.x,
                            origin.y + self.line_height * source.projection.row(row) as f32,
                        ),
                        self.line_height,
                        align,
                        align_width,
                    ));
                }
            }
            return result;
        }
        let first = self.line_for_offset(start);
        let last = self.line_for_offset(end - 1);
        for index in first..=last {
            let Some(line) = self.lines.get(index) else {
                continue;
            };
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

    /// The caret rectangle when every hard line uses the given alignment.
    pub fn caret_bounds_aligned(
        &self,
        offset: usize,
        origin: Point<Pixels>,
        width: Pixels,
        align: TextAlign,
        align_width: Pixels,
    ) -> Bounds<Pixels> {
        let position = self.position_for_offset_aligned(offset, align, align_width);
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
            return self.caret_bounds_aligned(range.start, origin, px(0.0), align, align_width);
        }
        let mut fragments = self
            .bounds_for_range(range.clone(), origin, align, align_width)
            .into_iter();
        let Some(first) = fragments.next() else {
            return self.caret_bounds_aligned(range.start, origin, px(0.0), align, align_width);
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
            .partition_point(|start| *start <= offset)
            .saturating_sub(1)
    }

    fn line_for_row(&self, row: usize) -> usize {
        self.rows
            .partition_point(|first| *first <= row)
            .saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TestAppContext, TextStyle};

    #[crate::test]
    fn row_index_identity_ignores_viewport_but_not_edits_or_projection(cx: &mut TestAppContext) {
        let document = EditSnapshot::new("ab\n界x\nz");
        let text_system = Arc::new(WindowTextSystem::new(cx.text_system().clone()));
        let layout = |document: EditSnapshot, visible, omitted: Vec<Range<usize>>| {
            let projection = EditableLineProjection::new(document.line_count(), omitted)
                .expect("valid projection");
            let runs = vec![TextStyle::default().to_run(document.len())];
            EditableTextLayout::unwrapped_projected(
                document,
                text_system.clone(),
                px(14.0),
                px(20.0),
                runs,
                visible,
                projection,
            )
            .expect("matching source")
        };
        let first = layout(document.clone(), 0..1, vec![]);
        let scrolled = layout(document.clone(), 2..3, vec![]);
        assert!(first.shares_row_index_with(&scrolled));
        let folded = layout(document.clone(), 0..1, std::iter::once(1..2).collect());
        assert!(!first.shares_row_index_with(&folded));
        let mut edited = document.clone();
        edited.replace(1..3, "\nb");
        assert_eq!(edited.len(), document.len());
        assert_eq!(edited.line_count(), document.line_count());
        assert!(!document.shares_storage_with(&edited));
        let changed = layout(edited, 0..1, vec![]);
        assert!(!first.shares_row_index_with(&changed));
        assert_eq!(document.materialized_bytes(), 0);
        assert_eq!(first.shaping_work(), EditableTextWork::default());
        assert_eq!(scrolled.shaping_work(), EditableTextWork::default());
    }

    #[crate::test]
    fn projected_geometry_skips_hidden_source_for_paint_hit_testing_and_selection(
        cx: &mut TestAppContext,
    ) {
        let text = format!("header界\n{}tail😀\nend", "hidden\n".repeat(20_000));
        let document = EditSnapshot::new(&text);
        let projection =
            EditableLineProjection::new(document.line_count(), std::iter::once(1..20_001))
                .expect("projection");
        let layout = EditableTextLayout::unwrapped_projected(
            document.clone(),
            Arc::new(WindowTextSystem::new(cx.text_system().clone())),
            px(14.0),
            px(20.0),
            vec![TextStyle::default().to_run(document.len())],
            0..3,
            projection,
        )
        .expect("matching line index");
        assert_eq!(layout.height(), px(60.0));
        assert_eq!(layout.painted_lines().count(), 3);
        assert_eq!(
            layout.shaping_work(),
            EditableTextWork {
                shaped_lines: 3,
                shaped_bytes: "header界tail😀end".len()
            }
        );
        let tail = document.line_range(20_001).expect("tail");
        let position = layout.position_for_offset(tail.start + 4);
        assert_eq!(position.y, px(20.0));
        assert_eq!(layout.offset_for_position(position), tail.start + 4);
        assert_eq!(layout.offset_at_row(1, px(-1.0)), tail.start);
        assert_eq!(layout.row_for_offset(tail.start), 1);
        assert!(
            layout
                .bounds_for_range(
                    10..tail.start,
                    point(px(0.0), px(0.0)),
                    TextAlign::Left,
                    px(400.0)
                )
                .is_empty()
        );
        let selection = layout.painted_bounds_for_range(
            0..document.len(),
            point(px(0.0), px(0.0)),
            TextAlign::Left,
            px(400.0),
        );
        assert_eq!(selection.len(), 3);
        assert!(selection.iter().all(|bounds| bounds.bottom() <= px(60.0)));
        assert_eq!(layout.shaping_work().shaped_lines, 3);
        assert_eq!(
            layout.position_for_offset(50),
            layout.position_for_offset("header界".len())
        );
        assert_eq!(
            layout.visual_rows(&text),
            [0..10, tail.clone(), tail.end..document.len()]
        );
    }

    #[crate::test]
    fn source_layout_shapes_viewport_bytes_and_exact_offscreen_queries(cx: &mut TestAppContext) {
        let row = "let asymmetric = '界';\n";
        let document = EditSnapshot::new(&row.repeat(200_000));
        let runs = vec![TextStyle::default().to_run(document.len())];
        let layout = EditableTextLayout::unwrapped(
            document.clone(),
            Arc::new(WindowTextSystem::new(cx.text_system().clone())),
            px(14.0),
            px(20.0),
            runs,
            150_000..150_012,
        );
        assert_eq!(layout.shaping_work(), EditableTextWork::default());
        assert_eq!(layout.painted_lines().count(), 12);
        assert_eq!(
            layout.shaping_work(),
            EditableTextWork {
                shaped_lines: 12,
                shaped_bytes: 12 * (row.len() - 1)
            }
        );
        assert_eq!(layout.height(), px(4_000_020.0));
        let start = document.line_range(3).expect("fourth row").start;
        let position = layout.position_for_offset(start + 3);
        assert_eq!(position.y, px(60.0));
        assert!(position.x > px(0.0));
        assert_eq!(layout.offset_for_position(position), start + 3);
        assert_eq!(layout.shaping_work().shaped_lines, 13);
        assert_eq!(layout.painted_lines().count(), 12);
        let selection = layout.painted_bounds_for_range(
            0..document.len(),
            point(px(0.0), px(0.0)),
            TextAlign::Left,
            px(500.0),
        );
        assert!(!selection.is_empty());
        assert!(selection.iter().all(|bounds| bounds.top() >= px(3_000_000.0) && bounds.bottom() <= px(3_000_240.0)));
        assert_eq!(
            layout.shaping_work().shaped_lines,
            13,
            "selection only shapes painted rows"
        );
    }

    #[crate::test]
    fn lazy_and_dense_source_geometry_agree_for_unicode_and_style_boundaries(
        cx: &mut TestAppContext,
    ) {
        let text: crate::SharedString = "wide\n界😀z\nאבcd\nlast".into();
        let system = Arc::new(WindowTextSystem::new(cx.text_system().clone()));
        let runs = vec![
            TextStyle::default().to_run(5),
            TextStyle::default().to_run(text.len() - 5),
        ];
        let dense = EditableTextLayout::new(
            &text,
            system
                .shape_text(text.clone(), px(14.0), &runs, None, None)
                .expect("shape text")
                .into_vec(),
            px(20.0),
        );
        let lazy = EditableTextLayout::unwrapped(
            EditSnapshot::new(&text),
            system,
            px(14.0),
            px(20.0),
            runs,
            1..3,
        );
        for offset in text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.len()))
        {
            assert_eq!(
                lazy.position_for_offset(offset),
                dense.position_for_offset(offset),
                "byte {offset}"
            );
        }
        for row in 0..4 {
            for x in [px(0.0), px(11.0), px(37.0), px(500.0)] {
                assert_eq!(lazy.offset_at_row(row, x), dense.offset_at_row(row, x));
            }
        }
        assert_eq!(lazy.visual_rows(&text), dense.visual_rows(&text));
    }
}
