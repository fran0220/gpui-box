use crate::{A11ySubtreeBuilder, Bounds, Pixels, SharedString, accesskit};
use std::ops::Range;
use unicode_bidi::{BidiInfo, Level};
use unicode_segmentation::UnicodeSegmentation;

const MAX_ACCESSIBLE_RUN_CHARS: usize = 255;

#[derive(Debug, Clone)]
struct AccessibleRun<'a> {
    value: &'a str,
    start_byte: usize,
    start_character: usize,
    character_lengths: Vec<u8>,
    line: usize,
    direction: accesskit::TextDirection,
}

/// A snapshot of the text runs most recently published to AccessKit.
///
/// Keep this beside the text revision that produced it. Accessibility actions
/// may arrive after a new frame, so positions from a stale tree must not be
/// interpreted against different text.
#[derive(Clone, Debug)]
pub struct PublishedAccessibleText {
    source: SharedString,
    revision: u64,
    runs: Vec<PublishedRun>,
}

#[derive(Clone, Debug)]
struct PublishedRun {
    node: accesskit::NodeId,
    start_character: usize,
    character_count: usize,
}

fn run_end_character(run: &AccessibleRun<'_>) -> usize {
    run.start_character + run.character_lengths.len()
}

fn accessible_runs<'a>(
    text: &'a str,
    visual_rows: &[Range<usize>],
    fallback_direction: accesskit::TextDirection,
) -> Vec<AccessibleRun<'a>> {
    if text.is_empty() {
        return vec![AccessibleRun {
            value: "",
            start_byte: 0,
            start_character: 0,
            character_lengths: Vec::new(),
            line: 0,
            direction: fallback_direction,
        }];
    }

    let graphemes = text.grapheme_indices(true).collect::<Vec<_>>();
    if graphemes
        .iter()
        .any(|(_, grapheme)| grapheme.len() > u8::MAX as usize)
    {
        // AccessKit stores each selectable unit's UTF-8 length in a u8. An
        // extended grapheme has no Unicode length limit, so an adversarially
        // long combining sequence cannot be represented truthfully.
        return Vec::new();
    }
    let fallback_level = Some(match fallback_direction {
        accesskit::TextDirection::RightToLeft => Level::rtl(),
        _ => Level::ltr(),
    });
    let bidi = BidiInfo::new(text, fallback_level);
    let mut runs = Vec::new();
    for (line, row) in visual_rows.iter().enumerate() {
        let mut start =
            graphemes.partition_point(|(offset, grapheme)| offset + grapheme.len() <= row.start);
        let row_end = graphemes.partition_point(|(offset, _)| *offset < row.end);
        while start < row_end {
            let level = bidi.levels[graphemes[start].0];
            let direction = if level.is_rtl() {
                accesskit::TextDirection::RightToLeft
            } else {
                accesskit::TextDirection::LeftToRight
            };
            let direction_end = graphemes[start + 1..row_end]
                .iter()
                .position(|(offset, _)| bidi.levels[*offset].is_rtl() != level.is_rtl())
                .map(|offset| start + offset + 1)
                .unwrap_or(row_end);
            let end = (start + MAX_ACCESSIBLE_RUN_CHARS).min(direction_end);
            let start_byte = graphemes[start].0;
            let end_byte = graphemes
                .get(end)
                .map(|(offset, _)| *offset)
                .unwrap_or(text.len());
            runs.push(AccessibleRun {
                value: &text[start_byte..end_byte],
                start_byte,
                start_character: start,
                character_lengths: graphemes[start..end]
                    .iter()
                    .map(|(_, grapheme)| grapheme.len() as u8)
                    .collect(),
                line,
                direction,
            });
            start = end;
        }
    }
    // A terminating hard break introduces another (empty) logical line. It
    // needs its own position: the position before the break and the caret on
    // the following empty line must not collapse onto the same TextRun.
    if graphemes
        .last()
        .is_some_and(|(_, grapheme)| grapheme.ends_with('\n'))
    {
        runs.push(AccessibleRun {
            value: "",
            start_byte: text.len(),
            start_character: graphemes.len(),
            character_lengths: Vec::new(),
            line: visual_rows.len(),
            direction: fallback_direction,
        });
    }
    runs
}

/// Returns whether every selectable grapheme can be represented by AccessKit.
pub fn accessible_text_is_representable(text: &str) -> bool {
    !text
        .graphemes(true)
        .any(|grapheme| grapheme.len() > u8::MAX as usize)
}

/// Publishes logical text runs and the current selection below an accessible
/// text element.
///
/// `visual_rows` contains UTF-8 ranges in visual row order. Runs are split at
/// bidirectional boundaries and AccessKit's 255-character run limit.
pub fn publish_accessible_text(
    builder: &mut A11ySubtreeBuilder,
    text: &str,
    anchor_byte: usize,
    focus_byte: usize,
    fallback_direction: accesskit::TextDirection,
    visual_rows: &[Range<usize>],
    revision: u64,
) -> Option<PublishedAccessibleText> {
    publish_accessible_text_inner(
        builder,
        text,
        anchor_byte,
        focus_byte,
        fallback_direction,
        visual_rows,
        revision,
        None,
    )
}

/// Publishes selectable text with per-grapheme geometry.
///
/// Bounds returned by `bounds_for_range` are in GPUI logical pixels. They are
/// scaled here to the physical coordinates used by the parent AccessKit node.
#[allow(clippy::too_many_arguments)]
pub fn publish_accessible_text_with_geometry(
    builder: &mut A11ySubtreeBuilder,
    text: &str,
    anchor_byte: usize,
    focus_byte: usize,
    fallback_direction: accesskit::TextDirection,
    visual_rows: &[Range<usize>],
    revision: u64,
    scale_factor: f32,
    bounds_for_range: impl Fn(Range<usize>) -> Vec<Bounds<Pixels>>,
) -> Option<PublishedAccessibleText> {
    publish_accessible_text_inner(
        builder,
        text,
        anchor_byte,
        focus_byte,
        fallback_direction,
        visual_rows,
        revision,
        Some((&bounds_for_range, scale_factor)),
    )
}

#[allow(clippy::too_many_arguments)]
fn publish_accessible_text_inner(
    builder: &mut A11ySubtreeBuilder,
    text: &str,
    anchor_byte: usize,
    focus_byte: usize,
    fallback_direction: accesskit::TextDirection,
    visual_rows: &[Range<usize>],
    revision: u64,
    geometry: Option<(&dyn Fn(Range<usize>) -> Vec<Bounds<Pixels>>, f32)>,
) -> Option<PublishedAccessibleText> {
    let runs = accessible_runs(text, visual_rows, fallback_direction);
    if runs.is_empty() {
        return None;
    }
    let run_count = runs.len();
    let run_ids = runs
        .iter()
        .map(|run| {
            builder.synthetic_node_id((
                revision,
                run.line,
                run.start_character,
                run.character_lengths.len(),
            ))
        })
        .collect::<Vec<_>>();
    for run in 0..run_count {
        let accessible_run = &runs[run];
        let mut node = accesskit::Node::new(accesskit::Role::TextRun);
        node.set_text_direction(accessible_run.direction);
        node.set_value(accessible_run.value);
        node.set_character_lengths(accessible_run.character_lengths.clone());
        let run_end_byte = accessible_run.start_byte + accessible_run.value.len();
        let word_starts = text
            .unicode_word_indices()
            .filter(|(offset, _)| accessible_run.start_byte <= *offset && *offset < run_end_byte)
            .map(|(offset, _)| {
                (text[..offset].graphemes(true).count() - accessible_run.start_character) as u8
            })
            .collect::<Vec<_>>();
        if !word_starts.is_empty() {
            node.set_word_starts(word_starts);
        }
        if let Some((bounds_for_range, scale)) = geometry {
            let mut positions = Vec::with_capacity(accessible_run.character_lengths.len());
            let mut widths = Vec::with_capacity(accessible_run.character_lengths.len());
            let mut advance = 0.0;
            let mut union: Option<Bounds<Pixels>> = None;
            for (offset, grapheme) in accessible_run.value.grapheme_indices(true) {
                positions.push(advance * scale);
                let range = accessible_run.start_byte + offset
                    ..accessible_run.start_byte + offset + grapheme.len();
                let cells = bounds_for_range(range);
                let width = cells.iter().map(|cell| cell.size.width.0).sum::<f32>();
                widths.push(width);
                advance += width;
                for cell in cells {
                    union = Some(match union {
                        Some(bounds) => bounds.union(&cell),
                        None => cell,
                    });
                }
            }
            if let Some(bounds) = union {
                let normalization = if advance > 0.0 {
                    bounds.size.width.0 / advance
                } else {
                    1.0
                };
                let mut normalized_advance = 0.0;
                for (position, width) in positions.iter_mut().zip(&mut widths) {
                    *position = normalized_advance * scale;
                    *width *= normalization;
                    normalized_advance += *width;
                    *width *= scale;
                }
                node.set_bounds(accesskit::Rect {
                    x0: (bounds.left().0 * scale) as f64,
                    y0: (bounds.top().0 * scale) as f64,
                    x1: (bounds.right().0 * scale) as f64,
                    y1: (bounds.bottom().0 * scale) as f64,
                });
                node.set_character_positions(positions);
                node.set_character_widths(widths);
            }
        }
        if run > 0 && runs[run - 1].line == accessible_run.line {
            node.set_previous_on_line(run_ids[run - 1]);
        }
        if run + 1 < run_count && runs[run + 1].line == accessible_run.line {
            node.set_next_on_line(run_ids[run + 1]);
        }
        builder.push_child(run_ids[run], node);
    }
    let anchor = accessible_position(text, anchor_byte, &runs, |run| run_ids[run]);
    let focus = accessible_position(text, focus_byte, &runs, |run| run_ids[run]);
    builder
        .parent_node()
        .set_text_selection(accesskit::TextSelection { anchor, focus });
    Some(PublishedAccessibleText {
        source: text.into(),
        revision,
        runs: runs
            .iter()
            .zip(run_ids)
            .map(|(run, node)| PublishedRun {
                node,
                start_character: run.start_character,
                character_count: run.character_lengths.len(),
            })
            .collect(),
    })
}

fn accessible_position(
    text: &str,
    byte_offset: usize,
    runs: &[AccessibleRun<'_>],
    node_id: impl Fn(usize) -> accesskit::NodeId,
) -> accesskit::TextPosition {
    let character = text
        .grapheme_indices(true)
        .take_while(|(offset, _)| *offset < byte_offset)
        .count();
    let run = runs
        .iter()
        .enumerate()
        .find_map(|(index, run)| {
            let end = run_end_character(run);
            if character < end
                || (character == end
                    && runs.get(index + 1).is_none_or(|next| next.line == run.line))
            {
                Some(index)
            } else {
                None
            }
        })
        .unwrap_or(runs.len() - 1);
    accesskit::TextPosition {
        node: node_id(run),
        character_index: character - runs[run].start_character,
    }
}

fn byte_offset_for_accessible_position(
    text: &str,
    position: accesskit::TextPosition,
    runs: &[PublishedRun],
) -> Option<usize> {
    let run = runs.iter().find(|run| run.node == position.node)?;
    if position.character_index > run.character_count {
        return None;
    }
    let character = run.start_character + position.character_index;
    Some(
        text.grapheme_indices(true)
            .nth(character)
            .map(|(offset, _)| offset)
            .unwrap_or(text.len()),
    )
}

/// Resolves an AccessKit text position only against the exact value and
/// revision for which its synthetic run ids were published.
pub fn byte_offset_for_published_position(
    current_text: &str,
    current_revision: u64,
    published: &PublishedAccessibleText,
    position: accesskit::TextPosition,
) -> Option<usize> {
    (current_text == published.source.as_ref() && current_revision == published.revision)
        .then(|| byte_offset_for_accessible_position(current_text, position, &published.runs))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hard_rows(text: &str) -> Vec<Range<usize>> {
        let mut rows = Vec::new();
        let mut start = 0;
        for (offset, grapheme) in text.grapheme_indices(true) {
            if grapheme.ends_with('\n') {
                rows.push(start..offset + grapheme.len());
                start = offset + grapheme.len();
            }
        }
        if start < text.len() || rows.is_empty() {
            rows.push(start..text.len());
        }
        rows
    }

    #[test]
    fn accessible_positions_round_trip_utf8_text() {
        let text = format!("{}e\u{301}👩‍💻\nאב", "x".repeat(255));
        let nodes = |run| accesskit::NodeId(100 + run as u64);
        let rows = hard_rows(&text);
        let runs = accessible_runs(&text, &rows, accesskit::TextDirection::LeftToRight);
        let run_ids = (0..runs.len()).map(nodes).collect::<Vec<_>>();
        let published = runs
            .iter()
            .zip(&run_ids)
            .map(|(run, node)| PublishedRun {
                node: *node,
                start_character: run.start_character,
                character_count: run.character_lengths.len(),
            })
            .collect::<Vec<_>>();
        for offset in [0, 255, 258, 269, 270, text.len()] {
            let position = accessible_position(&text, offset, &runs, nodes);
            assert_eq!(
                byte_offset_for_accessible_position(&text, position, &published),
                Some(offset)
            );
        }
    }

    #[test]
    fn accessible_runs_use_graphemes_and_do_not_link_hard_lines() {
        let text = "e\u{301}👩‍💻\nאב";
        let rows = hard_rows(text);
        let runs = accessible_runs(text, &rows, accesskit::TextDirection::LeftToRight);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].value, "e\u{301}👩‍💻\n");
        assert_eq!(runs[0].character_lengths, vec![3, 11, 1]);
        assert_eq!(runs[1].value, "אב");
        assert_eq!(runs[1].direction, accesskit::TextDirection::RightToLeft);
    }

    #[test]
    fn trailing_lf_and_crlf_publish_a_distinct_empty_line() {
        for text in ["a\n", "a\r\n"] {
            let rows = hard_rows(text);
            let runs = accessible_runs(text, &rows, accesskit::TextDirection::LeftToRight);
            assert_eq!(runs.len(), 2);
            assert_eq!(runs[0].value, text);
            assert_eq!(runs[1].value, "");
            let ids = [accesskit::NodeId(1), accesskit::NodeId(2)];
            let published = runs
                .iter()
                .zip(ids)
                .map(|(run, node)| PublishedRun {
                    node,
                    start_character: run.start_character,
                    character_count: run.character_lengths.len(),
                })
                .collect::<Vec<_>>();
            let end = accessible_position(text, text.len(), &runs, |run| ids[run]);
            assert_eq!(end.node, ids[1]);
            assert_eq!(end.character_index, 0);
            assert_eq!(
                byte_offset_for_accessible_position(text, end, &published),
                Some(text.len())
            );
        }
    }

    #[test]
    fn unrepresentable_graphemes_are_not_published() {
        let text = format!("a{}", "\u{301}".repeat(128));
        assert!(text.len() > u8::MAX as usize);
        let rows = std::iter::once(0..text.len()).collect::<Vec<_>>();
        assert!(accessible_runs(&text, &rows, accesskit::TextDirection::LeftToRight).is_empty());
    }
}
