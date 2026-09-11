//! Affinity-aware native geometry retains both logical edges of every cluster.

use super::*;
use crate::{NativeTextPosition, TextAffinity, offset_from_utf16, offset_to_utf16};

#[derive(Clone, Copy)]
struct Stop {
    byte: usize,
    affinity: TextAffinity,
    at: Point<Pixels>,
}

impl Stop {
    fn native(self, text: &str) -> NativeTextPosition {
        NativeTextPosition {
            utf16_offset: offset_to_utf16(text, self.byte),
            affinity: self.affinity,
        }
    }
}

impl EditableTextLayout {
    /// Nearest actual caret to a point relative to the painted text origin.
    /// Optional bounds are UTF-16 offsets in the exact painted text. Candidates
    /// are restricted before hit testing; logical clamping after a bidi hit
    /// would choose an unrelated visual location. Rows take precedence over
    /// horizontal distance. Coincident bidi aliases use the incident cell
    /// under the pointer, then downstream affinity as the deterministic tie.
    pub fn native_position_for_point(
        &self,
        text: &str,
        position: Point<Pixels>,
        within_range: Option<Range<usize>>,
        align: TextAlign,
        align_width: Pixels,
    ) -> Option<NativeTextPosition> {
        let cells = self.native_cells(text, align, align_width);
        let mut stops = stops_for_cells(text, &cells);
        if let Some(range) = within_range {
            let start = offset_from_utf16(text, range.start);
            let end = offset_from_utf16(text, range.end);
            if range.start > range.end
                || offset_to_utf16(text, start) != range.start
                || offset_to_utf16(text, end) != range.end
            {
                return None;
            }
            stops.retain(|stop| start <= stop.byte && stop.byte <= end);
        }
        let row = ((position.y / self.line_height).max(0.0) as usize).min(self.total_rows - 1);
        let nearest = stops.iter().min_by(|a, b| {
            let a_row = (a.at.y / self.line_height) as usize;
            let b_row = (b.at.y / self.line_height) as usize;
            a_row
                .abs_diff(row)
                .cmp(&b_row.abs_diff(row))
                .then_with(|| {
                    (a.at.x - position.x)
                        .abs()
                        .partial_cmp(&(b.at.x - position.x).abs())
                        .expect("finite shaped coordinates")
                })
                .then_with(|| {
                    affinity_rank(a.affinity, TextAffinity::Downstream)
                        .cmp(&affinity_rank(b.affinity, TextAffinity::Downstream))
                })
        })?;
        let incident = cells.iter().find_map(|cell| {
            if cell.bounds.top() != nearest.at.y
                || position.x < cell.bounds.left()
                || position.x >= cell.bounds.right()
            {
                return None;
            }
            stops.iter().find(|stop| {
                stop.at == nearest.at
                    && ((stop.byte == cell.range.start
                        && stop.affinity == TextAffinity::Downstream)
                        || (stop.byte == cell.range.end && stop.affinity == TextAffinity::Upstream))
            })
        });
        Some(incident.unwrap_or(nearest).native(text))
    }

    /// Resolves either incident caret at a UTF-16 native position. The text
    /// must be the exact painted value, including any masking transformation.
    /// At document endpoints the sole available edge serves both affinities.
    /// Missing intra-ligature carets remain explicitly unavailable.
    /// This paint hot path resolves only the incident hard paragraph. Lazy
    /// source layouts convert UTF-16 through their document index and never
    /// shape unrelated offscreen lines or segment the entire document.
    #[allow(clippy::too_many_arguments)]
    pub fn native_position_bounds(
        &self,
        text: &str,
        position: NativeTextPosition,
        origin: Point<Pixels>,
        caret_width: Pixels,
        align: TextAlign,
        align_width: Pixels,
    ) -> Option<Bounds<Pixels>> {
        if text.len() != self.text_len {
            return None;
        }
        let (byte, paragraph, line, row) = if let Some(source) = &self.source {
            let byte = source.document.offset_from_utf16(position.utf16_offset);
            if source.document.offset_to_utf16(byte) != position.utf16_offset {
                return None;
            }
            let index = source.document.line_at(byte);
            let row = source.projection.row(index);
            if source.projection.source_line(row) != index {
                return None;
            }
            (
                byte,
                source.document.line_range(index)?,
                source.line(index),
                row,
            )
        } else {
            let byte = offset_from_utf16(text, position.utf16_offset);
            if offset_to_utf16(text, byte) != position.utf16_offset {
                return None;
            }
            let index = self.line_for_offset(byte);
            let line = self.lines.get(index)?.clone();
            let start = self.starts[index];
            let end = self.starts.get(index + 1).copied().unwrap_or(self.text_len);
            (byte, start..end, line, self.rows[index])
        };
        let mut cells = Vec::new();
        self.native_line_cells(
            text,
            &line,
            paragraph.start,
            row,
            align,
            align_width,
            &mut cells,
        );
        let stops = stops_for_cells_in_range(text, &cells, paragraph);
        let stop = find_byte_stop(byte, position.affinity, &stops)?;
        Some(Bounds::new(
            stop.at + origin,
            size(caret_width, self.line_height),
        ))
    }

    /// Moves through physical caret edges while retaining bidi and soft-wrap
    /// affinity. Horizontal steps cross one distinct visual location; aliases
    /// at the same location do not consume a step. The arriving cluster's edge
    /// determines the result affinity, so opposite travel can select different
    /// logical aliases at a bidi boundary. Vertical steps preserve the initial
    /// x across exact visual rows. All native offsets are UTF-16, not bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn native_position_in_direction(
        &self,
        text: &str,
        position: NativeTextPosition,
        direction: TextNavigationDirection,
        offset: usize,
        align: TextAlign,
        align_width: Pixels,
    ) -> Option<NativeTextPosition> {
        let cells = self.native_cells(text, align, align_width);
        let stops = stops_for_cells(text, &cells);
        let mut current = *find_stop(text, position, &stops)?;
        if offset == 0 {
            return Some(position);
        }
        match direction {
            TextNavigationDirection::Up | TextNavigationDirection::Down => {
                let row = (current.at.y / self.line_height) as usize;
                let row = if direction == TextNavigationDirection::Up {
                    row.checked_sub(offset)?
                } else {
                    row.checked_add(offset)?
                };
                if row >= self.total_rows {
                    return None;
                }
                let y = self.line_height * row;
                return stops
                    .iter()
                    .filter(|s| s.at.y == y)
                    .min_by(|a, b| {
                        (a.at.x - current.at.x)
                            .abs()
                            .partial_cmp(&(b.at.x - current.at.x).abs())
                            .expect("finite shaped coordinates")
                            .then_with(|| {
                                affinity_rank(a.affinity, position.affinity)
                                    .cmp(&affinity_rank(b.affinity, position.affinity))
                            })
                    })
                    .map(|s| s.native(text));
            }
            TextNavigationDirection::Left | TextNavigationDirection::Right => {}
        }
        // Every successful step reaches a different coordinate. This bound
        // also makes untrusted native offset requests finite without looping.
        if offset > stops.len() {
            return None;
        }
        let right = direction == TextNavigationDirection::Right;
        for _ in 0..offset {
            let next = if right {
                stops.iter().find(|s| visual_cmp(s.at, current.at).is_gt())
            } else {
                stops
                    .iter()
                    .rev()
                    .find(|s| visual_cmp(s.at, current.at).is_lt())
            }?;
            if cells.iter().any(|cell| {
                cell.bounds.top() == current.at.y
                    && cell.bounds.top() == next.at.y
                    && cell.bounds.left() >= current.at.x.min(next.at.x)
                    && cell.bounds.right() <= current.at.x.max(next.at.x)
                    && text[cell.range.clone()].graphemes(true).count() > 1
            }) {
                // The next grapheme lies inside a ligature, but the shaping
                // result supplies no interior caret. Do not skip over it.
                return None;
            }
            // Use the edge of the cell we just crossed, not an arbitrary
            // logical index among coincident bidi aliases.
            current = cells
                .iter()
                .find_map(|cell| {
                    let start = right == (cell.direction == TextWritingDirection::RightToLeft);
                    let at = cell.edge(start);
                    if at != next.at || cell.bounds.size.width == px(0.0) {
                        return None;
                    }
                    let byte = if start {
                        cell.range.start
                    } else {
                        cell.range.end
                    };
                    let affinity = if start {
                        TextAffinity::Downstream
                    } else {
                        TextAffinity::Upstream
                    };
                    stops
                        .iter()
                        .find(|s| s.byte == byte && s.affinity == affinity && s.at == at)
                        .copied()
                })
                .unwrap_or(*next);
        }
        Some(current.native(text))
    }

    /// The actual visual endpoint inside a UTF-16 range, including secondary
    /// bidi and wrap affinities omitted by the legacy offset-only projection.
    pub fn farthest_native_position(
        &self,
        text: &str,
        range: Range<usize>,
        direction: TextNavigationDirection,
        align: TextAlign,
        align_width: Pixels,
    ) -> Option<NativeTextPosition> {
        let stops = self.affinity_stops(text, align, align_width);
        stops
            .iter()
            .filter(|s| {
                let at = offset_to_utf16(text, s.byte);
                range.start <= at && at <= range.end
            })
            .min_by(|a, b| {
                let (a, b) = match direction {
                    TextNavigationDirection::Left => (a.at.x, b.at.x),
                    TextNavigationDirection::Right => (b.at.x, a.at.x),
                    TextNavigationDirection::Up => (a.at.y, b.at.y),
                    TextNavigationDirection::Down => (b.at.y, a.at.y),
                };
                a.partial_cmp(&b).expect("finite shaped coordinates")
            })
            .map(|s| s.native(text))
    }

    fn affinity_stops(&self, text: &str, align: TextAlign, width: Pixels) -> Vec<Stop> {
        stops_for_cells(text, &self.native_cells(text, align, width))
    }
}

fn visual_cmp(a: Point<Pixels>, b: Point<Pixels>) -> std::cmp::Ordering {
    a.y.partial_cmp(&b.y)
        .expect("finite shaped coordinates")
        .then(a.x.partial_cmp(&b.x).expect("finite shaped coordinates"))
}

fn affinity_rank(affinity: TextAffinity, preferred: TextAffinity) -> u8 {
    u8::from(affinity != preferred)
}

fn find_stop<'a>(text: &str, position: NativeTextPosition, stops: &'a [Stop]) -> Option<&'a Stop> {
    let byte = offset_from_utf16(text, position.utf16_offset);
    if offset_to_utf16(text, byte) != position.utf16_offset {
        return None;
    }
    find_byte_stop(byte, position.affinity, stops)
}

fn find_byte_stop(byte: usize, affinity: TextAffinity, stops: &[Stop]) -> Option<&Stop> {
    stops
        .iter()
        .find(|s| s.byte == byte && s.affinity == affinity)
        .or_else(|| stops.iter().find(|s| s.byte == byte))
}

fn stops_for_cells(text: &str, cells: &[Cell]) -> Vec<Stop> {
    stops_for_cells_in_range(text, cells, 0..text.len())
}

fn stops_for_cells_in_range(text: &str, cells: &[Cell], range: Range<usize>) -> Vec<Stop> {
    let Some(paragraph) = text.get(range.clone()) else {
        return Vec::new();
    };
    let boundaries: std::collections::BTreeSet<_> = paragraph
        .grapheme_indices(true)
        .map(|(at, _)| range.start + at)
        .chain([range.end])
        .collect();
    let mut stops = Vec::new();
    for cell in cells {
        for (byte, affinity, start) in [
            (cell.range.start, TextAffinity::Downstream, true),
            (cell.range.end, TextAffinity::Upstream, false),
        ] {
            if boundaries.contains(&byte) {
                // A hard break advances to the next row. Unlike a soft wrap,
                // its trailing edge is not another caret on the old row.
                let at = if !start && text[cell.range.clone()].ends_with('\n') {
                    let Some(next) = cells.iter().find(|next| next.range.start == byte) else {
                        continue;
                    };
                    next.edge(true)
                } else {
                    cell.edge(start)
                };
                stops.push(Stop { byte, affinity, at });
            }
        }
    }
    stops.sort_by(|a, b| {
        visual_cmp(a.at, b.at).then(a.byte.cmp(&b.byte)).then(
            affinity_rank(a.affinity, TextAffinity::Downstream)
                .cmp(&affinity_rank(b.affinity, TextAffinity::Downstream)),
        )
    });
    stops.dedup_by(|a, b| a.byte == b.byte && a.affinity == b.affinity && a.at == b.at);
    stops
}

#[cfg(test)]
mod tests {
    use super::super::tests::line;
    use super::*;

    fn at(utf16_offset: usize, affinity: TextAffinity) -> NativeTextPosition {
        NativeTextPosition {
            utf16_offset,
            affinity,
        }
    }

    #[test]
    fn affinity_distinguishes_both_bidi_edges_and_moves_through_real_cells() {
        use TextAffinity::{Downstream as D, Upstream as U};
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
        let caret = |p| {
            layout
                .native_position_bounds(text, p, Point::default(), px(1.), TextAlign::Left, px(40.))
                .expect("bidi incident caret")
                .origin
        };
        assert_eq!(caret(at(1, U)), point(px(7.), px(0.)));
        assert_eq!(caret(at(1, D)), point(px(29.), px(0.)));
        let right = |p| {
            layout
                .native_position_in_direction(
                    text,
                    p,
                    TextNavigationDirection::Right,
                    1,
                    TextAlign::Left,
                    px(40.),
                )
                .expect("right visual stop")
        };
        let mut p = at(0, D);
        for expected in [at(1, U), at(2, D), at(1, D), at(4, U)] {
            p = right(p);
            assert_eq!(p, expected);
        }
        let left = |p| {
            layout
                .native_position_in_direction(
                    text,
                    p,
                    TextNavigationDirection::Left,
                    1,
                    TextAlign::Left,
                    px(40.),
                )
                .expect("left visual stop")
        };
        for expected in [at(3, D), at(2, U), at(3, U), at(0, D)] {
            p = left(p);
            assert_eq!(p, expected);
        }
        assert_eq!(
            layout.farthest_native_position(
                text,
                1..1,
                TextNavigationDirection::Left,
                TextAlign::Left,
                px(40.)
            ),
            Some(at(1, U))
        );
        assert_eq!(
            layout.farthest_native_position(
                text,
                1..1,
                TextNavigationDirection::Right,
                TextAlign::Left,
                px(40.)
            ),
            Some(at(1, D))
        );
    }

    #[test]
    fn affinity_preserves_both_soft_wrap_rows_at_the_same_offset() {
        use TextAffinity::{Downstream as D, Upstream as U};
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
        let bounds = |p| {
            layout
                .native_position_bounds(
                    text,
                    p,
                    point(px(-5.), px(7.)),
                    px(1.),
                    TextAlign::Right,
                    px(80.),
                )
                .expect("wrap incident caret")
        };
        assert_eq!(bounds(at(3, U)).origin, point(px(75.), px(7.)));
        assert_eq!(bounds(at(3, D)).origin, point(px(45.), px(30.)));
        assert_eq!(
            layout.native_position_in_direction(
                text,
                at(3, U),
                TextNavigationDirection::Right,
                1,
                TextAlign::Right,
                px(80.)
            ),
            Some(at(3, D))
        );
        assert_eq!(
            layout.native_position_in_direction(
                text,
                at(3, D),
                TextNavigationDirection::Left,
                1,
                TextAlign::Right,
                px(80.)
            ),
            Some(at(3, U))
        );
        assert_eq!(
            layout.native_position_in_direction(
                text,
                at(3, D),
                TextNavigationDirection::Up,
                1,
                TextAlign::Right,
                px(80.)
            ),
            Some(at(0, D))
        );
    }

    #[test]
    fn affinity_navigation_rejects_surrogates_and_missing_ligature_carets() {
        use TextAffinity::Downstream as D;
        let text = "😀fi";
        let layout = EditableTextLayout::new(
            text,
            vec![line(text, &[(0, 0.), (4, 25.)], 37., None)],
            px(20.),
        );
        assert_eq!(
            layout.native_position_bounds(
                text,
                at(1, D),
                Point::default(),
                px(1.),
                TextAlign::Left,
                px(40.)
            ),
            None
        );
        assert_eq!(
            layout.native_position_bounds(
                text,
                at(3, D),
                Point::default(),
                px(1.),
                TextAlign::Left,
                px(40.)
            ),
            None
        );
        assert_eq!(
            layout.native_position_in_direction(
                text,
                at(2, D),
                TextNavigationDirection::Right,
                1,
                TextAlign::Left,
                px(40.)
            ),
            None
        );
        assert_eq!(
            layout.native_position_in_direction(
                text,
                at(0, D),
                TextNavigationDirection::Right,
                usize::MAX,
                TextAlign::Left,
                px(40.)
            ),
            None
        );
    }

    #[test]
    fn hard_newline_trailing_edge_is_on_the_next_row_not_a_soft_wrap_alias() {
        use TextAffinity::{Downstream as D, Upstream as U};
        let text = "ab\nc";
        let layout = EditableTextLayout::new(
            text,
            vec![
                line("ab", &[(0, 0.), (1, 8.)], 23., None),
                line("c", &[(0, 0.)], 11., None),
            ],
            px(19.),
        );
        let bounds = |p| {
            layout
                .native_position_bounds(text, p, Point::default(), px(1.), TextAlign::Left, px(40.))
                .expect("hard-break caret")
                .origin
        };
        assert_eq!(bounds(at(2, D)), point(px(23.), px(0.)));
        assert_eq!(bounds(at(3, U)), point(px(0.), px(19.)));
        assert_eq!(bounds(at(3, D)), point(px(0.), px(19.)));
        let next = layout
            .native_position_in_direction(
                text,
                at(2, D),
                TextNavigationDirection::Right,
                1,
                TextAlign::Left,
                px(40.),
            )
            .expect("next hard row");
        assert_eq!(next.utf16_offset, 3);
        assert_eq!(bounds(next), point(px(0.), px(19.)));
    }

    #[test]
    fn point_hit_testing_constrains_visual_candidates_before_bidi_selection() {
        use TextAffinity::Upstream as U;
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
        let hit = |x, range| {
            layout.native_position_for_point(
                text,
                point(px(x), px(8.)),
                range,
                TextAlign::Left,
                px(40.),
            )
        };
        assert_eq!(hit(6., None), Some(at(1, U)), "left incident LTR cell");
        assert_eq!(
            hit(8., None),
            Some(at(3, U)),
            "right incident RTL cell shares same x"
        );
        assert_eq!(
            hit(8., Some(1..2)),
            Some(at(1, U)),
            "logical post-clamp would incorrectly choose offset 2 at x=20"
        );
        assert_eq!(hit(8., Some(9..10)), None);
        assert_eq!(hit(8., Some(Range { start: 2, end: 1 })), None);
    }

    #[test]
    fn point_hit_testing_keeps_wrap_affinity_and_rejects_unrepresented_ranges() {
        use TextAffinity::{Downstream as D, Upstream as U};
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
        assert_eq!(
            layout.native_position_for_point(
                text,
                point(px(80.), px(4.)),
                None,
                TextAlign::Right,
                px(80.)
            ),
            Some(at(3, U))
        );
        assert_eq!(
            layout.native_position_for_point(
                text,
                point(px(50.), px(23.)),
                None,
                TextAlign::Right,
                px(80.)
            ),
            Some(at(3, D))
        );
        assert_eq!(
            layout.native_position_for_point(
                text,
                point(px(50.), px(500.)),
                Some(0..2),
                TextAlign::Right,
                px(80.)
            ),
            Some(at(0, D))
        );
        let ligature = EditableTextLayout::new(
            "😀fi",
            vec![line("😀fi", &[(0, 0.), (4, 25.)], 37., None)],
            px(20.),
        );
        assert_eq!(
            ligature.native_position_for_point(
                "😀fi",
                point(px(30.), px(10.)),
                Some(3..3),
                TextAlign::Left,
                px(40.)
            ),
            None
        );
        assert_eq!(
            ligature.native_position_for_point(
                "😀fi",
                Point::default(),
                Some(1..2),
                TextAlign::Left,
                px(40.)
            ),
            None
        );
    }

    #[crate::test]
    fn native_caret_bounds_shape_only_the_incident_source_paragraph(
        cx: &mut crate::TestAppContext,
    ) {
        use TextAffinity::{Downstream as D, Upstream as U};
        // Seven UTF-16 units and eleven UTF-8 bytes per hard line.
        let text = "aאב😀z\n".repeat(512);
        let document = EditSnapshot::new(&text);
        let layout = EditableTextLayout::unwrapped(
            document.clone(),
            Arc::new(WindowTextSystem::new(cx.text_system().clone())),
            px(14.),
            px(20.),
            vec![crate::TextStyle::default().to_run(text.len())],
            100..108,
        );
        assert_eq!(layout.painted_lines().count(), 8);
        assert_eq!(layout.shaping_work().shaped_lines, 8);
        let caret = |offset, affinity| {
            layout.native_position_bounds(
                &text,
                at(offset, affinity),
                Point::default(),
                px(1.),
                TextAlign::Left,
                px(400.),
            )
        };
        assert!(caret(104 * 7 + 1, D).is_some());
        assert_eq!(
            layout.shaping_work().shaped_lines,
            8,
            "painted caret reuses its paragraph"
        );
        assert!(caret(400 * 7 + 1, D).is_some());
        assert!(caret(400 * 7 + 1, U).is_some());
        assert_eq!(
            layout.shaping_work().shaped_lines,
            9,
            "offscreen caret shapes exactly one line once"
        );
        assert!(
            caret(450 * 7 + 4, D).is_none(),
            "surrogate interior rejected before shaping"
        );
        assert_eq!(layout.shaping_work().shaped_lines, 9);
        assert_eq!(caret(450 * 7, U).expect("hard-line start").top(), px(9000.));
        assert_eq!(
            layout.shaping_work().shaped_lines,
            10,
            "upstream hard-line start needs no preceding paragraph"
        );
        assert_eq!(
            document.materialized_bytes(),
            0,
            "indexed conversion never flattens the snapshot"
        );
        assert_eq!(layout.painted_lines().count(), 8);
        assert_eq!(layout.shaping_work().shaped_lines, 10);
    }
}
