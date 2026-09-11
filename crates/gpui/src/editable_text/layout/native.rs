//! Native text geometry uses painted visual cells, never UTF-16 arithmetic.

mod affinity;

use super::*;
use crate::{NativeTextPosition, TextAffinity, offset_from_utf16, offset_to_utf16};
use unicode_bidi::BidiInfo;
use unicode_segmentation::UnicodeSegmentation;

/// A physical direction in the painted text coordinate system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextNavigationDirection {
    /// Toward decreasing x.
    Left,
    /// Toward increasing x.
    Right,
    /// Toward the preceding visual row.
    Up,
    /// Toward the following visual row.
    Down,
}

/// Resolved horizontal writing direction, not a request to mutate a paragraph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextWritingDirection {
    /// Logical text advances toward increasing x.
    LeftToRight,
    /// Logical text advances toward decreasing x.
    RightToLeft,
}

/// One visual selection fragment in the caller's logical coordinate system.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextSelectionRect {
    /// Painted fragment bounds, without viewport clipping.
    pub bounds: Bounds<Pixels>,
    /// Resolved direction of this fragment.
    pub writing_direction: TextWritingDirection,
    /// This fragment contains the logical start of the selection.
    pub contains_start: bool,
    /// This fragment contains the logical end of the selection.
    pub contains_end: bool,
    /// Whether the text is vertically oriented (not merely on another row).
    pub is_vertical: bool,
}

struct Cell {
    range: Range<usize>,
    bounds: Bounds<Pixels>,
    direction: TextWritingDirection,
}

impl Cell {
    fn edge(&self, start: bool) -> Point<Pixels> {
        point(
            if start == (self.direction == TextWritingDirection::LeftToRight) {
                self.bounds.left()
            } else {
                self.bounds.right()
            },
            self.bounds.top(),
        )
    }
}

impl EditableTextLayout {
    /// Annotated visual fragments for UTF-8 offsets in the exact painted text.
    /// Fragments are ordered top-to-bottom, then left-to-right, and never
    /// merged across a direction boundary. Partial shaping clusters occupy
    /// their complete painted cell; no font ligature caret is fabricated.
    pub fn native_selection_rects(
        &self,
        text: &str,
        range: Range<usize>,
        origin: Point<Pixels>,
        align: TextAlign,
        align_width: Pixels,
    ) -> Vec<TextSelectionRect> {
        if range.start >= range.end || range.end > text.len() {
            return Vec::new();
        }
        let mut result: Vec<TextSelectionRect> = Vec::new();
        for cell in self.native_cells(text, align, align_width) {
            if cell.range.start >= range.end || cell.range.end <= range.start {
                continue;
            }
            let mut bounds = cell.bounds;
            bounds.origin += origin;
            let fragment = TextSelectionRect {
                bounds,
                writing_direction: cell.direction,
                contains_start: cell.range.contains(&range.start),
                contains_end: cell.range.start < range.end && range.end <= cell.range.end,
                is_vertical: false,
            };
            if let Some(last) = result.last_mut()
                && last.writing_direction == fragment.writing_direction
                && last.bounds.top() == bounds.top()
                && last.bounds.right() == bounds.left()
            {
                last.bounds.size.width += bounds.size.width;
                last.contains_start |= fragment.contains_start;
                last.contains_end |= fragment.contains_end;
            } else {
                result.push(fragment);
            }
        }
        result
    }

    /// Actual cluster-edge caret, using downstream logical affinity at bidi
    /// and soft-wrap boundaries. Returns None for an interior ligature stop
    /// absent from the shaping result, rather than interpolating its width.
    #[allow(clippy::too_many_arguments)]
    pub fn native_caret_bounds(
        &self,
        text: &str,
        position: usize,
        origin: Point<Pixels>,
        width: Pixels,
        align: TextAlign,
        align_width: Pixels,
    ) -> Option<Bounds<Pixels>> {
        if !text.is_char_boundary(position) {
            return None;
        }
        self.native_position_bounds(
            text,
            NativeTextPosition {
                utf16_offset: offset_to_utf16(text, position),
                affinity: TextAffinity::Downstream,
            },
            origin,
            width,
            align,
            align_width,
        )
    }

    /// Moves between actual visual cluster-edge stops, never UTF-16 units.
    /// Vertical movement addresses the layout's row index and preserves x
    /// across the entire request. Out-of-document movement returns None.
    /// This layout paints uniform-height horizontal rows; it does not claim
    /// support for a different rich-text layout's variable-height rows.
    #[allow(clippy::too_many_arguments)]
    pub fn native_text_position(
        &self,
        text: &str,
        position: usize,
        direction: TextNavigationDirection,
        offset: usize,
        align: TextAlign,
        align_width: Pixels,
    ) -> Option<usize> {
        if !text.is_char_boundary(position) {
            return None;
        }
        self.native_position_in_direction(
            text,
            NativeTextPosition {
                utf16_offset: offset_to_utf16(text, position),
                affinity: TextAffinity::Downstream,
            },
            direction,
            offset,
            align,
            align_width,
        )
        .map(|position| offset_from_utf16(text, position.utf16_offset))
    }

    /// The visual extreme among available caret stops inside a UTF-8 range.
    pub fn native_farthest_position(
        &self,
        text: &str,
        range: Range<usize>,
        direction: TextNavigationDirection,
        align: TextAlign,
        align_width: Pixels,
    ) -> Option<usize> {
        if !text.is_char_boundary(range.start) || !text.is_char_boundary(range.end) {
            return None;
        }
        self.farthest_native_position(
            text,
            offset_to_utf16(text, range.start)..offset_to_utf16(text, range.end),
            direction,
            align,
            align_width,
        )
        .map(|position| offset_from_utf16(text, position.utf16_offset))
    }

    /// Resolves the Unicode paragraph base direction. This is a query only:
    /// the editable model has no paragraph-direction override to mutate.
    pub fn native_base_writing_direction(
        &self,
        text: &str,
        position: usize,
    ) -> Option<TextWritingDirection> {
        if text.len() != self.text_len || !text.is_char_boundary(position) {
            return None;
        }
        let start = text[..position].rfind('\n').map_or(0, |at| at + 1);
        let end = text[position..]
            .find('\n')
            .map_or(text.len(), |at| position + at);
        let bidi = BidiInfo::new(&text[start..end], None);
        Some(direction(
            bidi.paragraphs.first().is_some_and(|p| p.level.is_rtl()),
        ))
    }

    fn native_cells(&self, text: &str, align: TextAlign, width: Pixels) -> Vec<Cell> {
        if text.len() != self.text_len {
            return Vec::new();
        }
        let mut cells = Vec::new();
        if let Some(source) = &self.source {
            for row in 0..self.total_rows {
                let line_index = source.projection.source_line(row);
                let start = source
                    .document
                    .line_range(line_index)
                    .expect("projected source line")
                    .start;
                self.native_line_cells(
                    text,
                    &source.line(line_index),
                    start,
                    row,
                    align,
                    width,
                    &mut cells,
                );
            }
        } else {
            for (index, line) in self.lines.iter().enumerate() {
                self.native_line_cells(
                    text,
                    line,
                    self.starts[index],
                    self.rows[index],
                    align,
                    width,
                    &mut cells,
                );
            }
        }
        cells
    }

    #[allow(clippy::too_many_arguments)]
    fn native_line_cells(
        &self,
        text: &str,
        line: &WrappedLine,
        start: usize,
        first_row: usize,
        align: TextAlign,
        width: Pixels,
        cells: &mut Vec<Cell>,
    ) {
        let Some(paragraph) = text.get(start..start + line.len()) else {
            return;
        };
        if paragraph != line.text.as_ref() {
            return;
        }
        let bidi = BidiInfo::new(paragraph, None);
        let glyphs: Vec<_> = line
            .unwrapped_layout
            .runs
            .iter()
            .enumerate()
            .flat_map(|(run, shaped)| {
                shaped.glyphs.iter().enumerate().map(move |(glyph, value)| {
                    (
                        crate::WrapBoundary {
                            run_ix: run,
                            glyph_ix: glyph,
                        },
                        value,
                    )
                })
            })
            .collect();
        let mut ends: Vec<_> = glyphs
            .iter()
            .map(|(_, g)| g.index)
            .chain([line.len()])
            .collect();
        ends.sort_unstable();
        ends.dedup();
        let mut row = 0;
        let mut row_start = px(0.0);
        for (i, (boundary, glyph)) in glyphs.iter().enumerate() {
            if line.wrap_boundaries().contains(boundary) {
                row += 1;
                row_start = glyph.position.x;
            }
            let row_end = line
                .wrap_boundaries()
                .get(row)
                .map(|b| {
                    line.unwrapped_layout.runs[b.run_ix].glyphs[b.glyph_ix]
                        .position
                        .x
                })
                .unwrap_or(line.unwrapped_layout.width);
            let shift = match align {
                TextAlign::Left => px(0.0),
                TextAlign::Center => (width - (row_end - row_start)) / 2.0,
                TextAlign::Right => width - (row_end - row_start),
            };
            let end_x = glyphs
                .get(i + 1)
                .map_or(row_end, |(_, g)| g.position.x)
                .min(row_end);
            let end = ends[ends.partition_point(|at| *at <= glyph.index)];
            let bounds = Bounds::new(
                point(
                    glyph.position.x - row_start + shift,
                    self.line_height * (first_row + row),
                ),
                size((end_x - glyph.position.x).max(px(0.0)), self.line_height),
            );
            // Several glyphs can share one shaping cluster. Keep its complete
            // visual advance, rather than retaining an interior combining mark.
            if let Some(last) = cells.last_mut()
                && last.range == (start + glyph.index..start + end)
                && last.bounds.top() == bounds.top()
            {
                last.bounds.size.width = bounds.right() - last.bounds.left();
            } else {
                cells.push(Cell {
                    range: start + glyph.index..start + end,
                    bounds,
                    direction: direction(bidi.levels.get(glyph.index).is_some_and(|l| l.is_rtl())),
                });
            }
        }
        // Hard line breaks and empty rows have no glyph, but remain real
        // insertion stops. Their zero-width cells do not invent a line width.
        let end = start + line.len();
        if glyphs.is_empty() || text[end..].starts_with('\n') {
            let x = cells
                .iter()
                .rev()
                .find(|c| c.range.end == end && c.range.start >= start)
                .map_or(
                    match align {
                        TextAlign::Left => px(0.0),
                        TextAlign::Center => width / 2.0,
                        TextAlign::Right => width,
                    },
                    |c| c.edge(false).x,
                );
            cells.push(Cell {
                range: end..end + usize::from(text[end..].starts_with('\n')),
                bounds: Bounds::new(
                    point(x, self.line_height * (first_row + row)),
                    size(px(0.0), self.line_height),
                ),
                direction: direction(bidi.paragraphs.first().is_some_and(|p| p.level.is_rtl())),
            });
        }
    }
}

fn direction(rtl: bool) -> TextWritingDirection {
    if rtl {
        TextWritingDirection::RightToLeft
    } else {
        TextWritingDirection::LeftToRight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FontId, GlyphId, LineLayout, ShapedGlyph, ShapedRun, WrapBoundary, WrappedLineLayout,
    };

    // Explicit asymmetric shaper output makes expected coordinates independent
    // of the methods under test, and exercises decreasing logical indices.
    pub(super) fn line(
        text: &str,
        glyphs: &[(usize, f32)],
        width: f32,
        wrap: Option<usize>,
    ) -> WrappedLine {
        WrappedLine {
            text: text.to_owned().into(),
            layout: Arc::new(WrappedLineLayout {
                unwrapped_layout: Arc::new(LineLayout {
                    len: text.len(),
                    width: px(width),
                    runs: vec![ShapedRun {
                        font_id: FontId(0),
                        glyphs: glyphs
                            .iter()
                            .map(|(index, x)| ShapedGlyph {
                                id: GlyphId(0),
                                position: point(px(*x), px(0.0)),
                                index: *index,
                                is_emoji: false,
                            })
                            .collect(),
                    }],
                    ..Default::default()
                }),
                wrap_boundaries: wrap
                    .into_iter()
                    .map(|glyph_ix| WrapBoundary {
                        run_ix: 0,
                        glyph_ix,
                    })
                    .collect(),
                wrap_width: wrap.map(|_| px(30.0)),
            }),
            decoration_runs: Vec::new(),
        }
    }

    #[test]
    fn native_fragments_preserve_bidi_logical_endpoint_flags_and_scroll_origin() {
        let text = "aאבz";
        let layout = EditableTextLayout::new(
            text,
            vec![line(
                text,
                &[(0, 0.), (3, 7.), (1, 20.), (5, 29.)],
                40.,
                None,
            )],
            px(17.),
        );
        let fragments = layout.native_selection_rects(
            text,
            0..3,
            point(px(-3.), px(11.)),
            TextAlign::Right,
            px(60.),
        );
        assert_eq!(
            fragments,
            vec![
                TextSelectionRect {
                    bounds: Bounds::new(point(px(17.), px(11.)), size(px(7.), px(17.))),
                    writing_direction: TextWritingDirection::LeftToRight,
                    contains_start: true,
                    contains_end: false,
                    is_vertical: false
                },
                TextSelectionRect {
                    bounds: Bounds::new(point(px(37.), px(11.)), size(px(9.), px(17.))),
                    writing_direction: TextWritingDirection::RightToLeft,
                    contains_start: false,
                    contains_end: true,
                    is_vertical: false
                },
            ]
        );
        let all = layout.native_selection_rects(
            text,
            0..text.len(),
            Point::default(),
            TextAlign::Left,
            px(40.),
        );
        assert_eq!(all.len(), 3, "adjacent LTR/RTL cells must not merge");
        assert_eq!(all[1].bounds.size.width, px(22.));
    }

    #[test]
    fn native_rtl_caret_and_visual_navigation_reverse_logical_offsets() {
        let text = "אבג";
        let layout = EditableTextLayout::new(
            text,
            vec![line(text, &[(4, 0.), (2, 11.), (0, 28.)], 37., None)],
            px(19.),
        );
        let caret = layout
            .native_caret_bounds(text, 0, Point::default(), px(1.), TextAlign::Left, px(37.))
            .expect("RTL start caret");
        assert_eq!(caret.origin, point(px(37.), px(0.)));
        assert_eq!(
            layout.native_text_position(
                text,
                0,
                TextNavigationDirection::Left,
                2,
                TextAlign::Left,
                px(37.)
            ),
            Some(4)
        );
        assert_eq!(
            layout.native_text_position(
                text,
                4,
                TextNavigationDirection::Right,
                1,
                TextAlign::Left,
                px(37.)
            ),
            Some(2)
        );
        assert_eq!(
            layout.native_text_position(
                text,
                6,
                TextNavigationDirection::Left,
                1,
                TextAlign::Left,
                px(37.)
            ),
            None
        );
        assert_eq!(
            layout.native_farthest_position(
                text,
                0..6,
                TextNavigationDirection::Left,
                TextAlign::Left,
                px(37.)
            ),
            Some(6)
        );
        assert_eq!(
            layout.native_farthest_position(
                text,
                0..6,
                TextNavigationDirection::Right,
                TextAlign::Left,
                px(37.)
            ),
            Some(0)
        );
    }

    #[test]
    fn native_wrapped_vertical_navigation_uses_rows_alignment_and_downstream_affinity() {
        let text = "abcde";
        let layout = EditableTextLayout::new(
            text,
            vec![line(
                text,
                &[(0, 0.), (1, 8.), (2, 23.), (3, 30.), (4, 42.)],
                60.,
                Some(3),
            )],
            px(23.),
        );
        let fragments =
            layout.native_selection_rects(text, 2..5, Point::default(), TextAlign::Right, px(80.));
        assert_eq!(
            fragments[0].bounds,
            Bounds::new(point(px(73.), px(0.)), size(px(7.), px(23.)))
        );
        assert_eq!(
            fragments[1].bounds,
            Bounds::new(point(px(50.), px(23.)), size(px(30.), px(23.)))
        );
        assert_eq!(
            layout
                .native_caret_bounds(text, 3, Point::default(), px(1.), TextAlign::Right, px(80.))
                .expect("wrapped downstream caret")
                .origin,
            point(px(50.), px(23.))
        );
        assert_eq!(
            layout.native_text_position(
                text,
                1,
                TextNavigationDirection::Down,
                1,
                TextAlign::Right,
                px(80.)
            ),
            Some(4)
        );
        assert_eq!(
            layout.native_text_position(
                text,
                4,
                TextNavigationDirection::Up,
                1,
                TextAlign::Right,
                px(80.)
            ),
            Some(1)
        );
        assert_eq!(
            layout.native_text_position(
                text,
                4,
                TextNavigationDirection::Down,
                1,
                TextAlign::Right,
                px(80.)
            ),
            None
        );
    }

    #[test]
    fn native_stops_do_not_split_extended_graphemes_or_guess_ligature_carets() {
        let text = "a\u{301}👩‍💻z";
        let emoji_end = text.len() - 1;
        let layout = EditableTextLayout::new(
            text,
            vec![line(
                text,
                &[(0, 0.), (1, 5.), (3, 9.), (emoji_end, 33.)],
                41.,
                None,
            )],
            px(20.),
        );
        assert_eq!(
            layout.native_text_position(
                text,
                0,
                TextNavigationDirection::Right,
                1,
                TextAlign::Left,
                px(41.)
            ),
            Some(3)
        );
        assert_eq!(
            layout.native_text_position(
                text,
                3,
                TextNavigationDirection::Right,
                1,
                TextAlign::Left,
                px(41.)
            ),
            Some(emoji_end)
        );
        assert_eq!(
            layout.native_caret_bounds(text, 1, Point::default(), px(1.), TextAlign::Left, px(41.)),
            None
        );
        let ligature =
            EditableTextLayout::new("fi", vec![line("fi", &[(0, 0.)], 12., None)], px(20.));
        assert_eq!(
            ligature.native_caret_bounds(
                "fi",
                1,
                Point::default(),
                px(1.),
                TextAlign::Left,
                px(12.)
            ),
            None
        );
        assert!(
            ligature
                .native_selection_rects("xx", 0..1, Point::default(), TextAlign::Left, px(12.))
                .is_empty(),
            "same length is not sufficient to match painted text"
        );
    }

    #[test]
    fn native_empty_rows_newlines_and_paragraph_base_direction_are_real_stops() {
        let text = "אב\n\nz";
        let layout = EditableTextLayout::new(
            text,
            vec![
                line("אב", &[(2, 0.), (0, 13.)], 21., None),
                line("", &[], 0., None),
                line("z", &[(0, 0.)], 8., None),
            ],
            px(18.),
        );
        assert_eq!(
            layout
                .native_caret_bounds(text, 4, Point::default(), px(1.), TextAlign::Left, px(40.))
                .expect("paragraph end caret")
                .origin,
            point(px(0.), px(0.))
        );
        assert_eq!(
            layout.native_text_position(
                text,
                4,
                TextNavigationDirection::Down,
                1,
                TextAlign::Left,
                px(40.)
            ),
            Some(5)
        );
        assert_eq!(
            layout.native_text_position(
                text,
                5,
                TextNavigationDirection::Down,
                1,
                TextAlign::Left,
                px(40.)
            ),
            Some(6)
        );
        assert_eq!(
            layout.native_base_writing_direction(text, 2),
            Some(TextWritingDirection::RightToLeft)
        );
        assert_eq!(
            layout.native_base_writing_direction(text, 6),
            Some(TextWritingDirection::LeftToRight)
        );
        assert_eq!(layout.native_base_writing_direction(text, 1), None);
        let empty = EditableTextLayout::new("", vec![line("", &[], 0., None)], px(18.));
        assert_eq!(
            empty.native_text_position(
                "",
                0,
                TextNavigationDirection::Right,
                0,
                TextAlign::Left,
                px(40.)
            ),
            Some(0)
        );
    }
}
