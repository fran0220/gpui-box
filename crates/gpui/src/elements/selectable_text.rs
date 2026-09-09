use crate::{A11ySubtreeBuilder, Bounds, Pixels, SharedString, accesskit};
use std::{ops::Range, sync::Arc};
use unicode_bidi::{BidiInfo, Level};
use unicode_segmentation::UnicodeSegmentation;

const MAX_ACCESSIBLE_RUN_CHARS: usize = 255;

#[derive(Debug, Clone)]
struct AccessibleRun {
    value: Range<usize>,
    start_byte: usize,
    start_character: usize,
    character_lengths: Vec<u8>,
    word_starts: Vec<u8>,
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
    runs: Arc<[PublishedRun]>,
}

#[derive(Clone, Debug)]
struct PublishedRun {
    node: accesskit::NodeId,
    start_character: usize,
    character_count: usize,
}

/// Revision-keyed logical text publication. Offscreen unchanged leaves are
/// retained in AccessKit; visible and previously visible leaves refresh their
/// geometry. Own one cache per text surface, never share it between windows.
#[derive(Default)]
pub struct AccessibleTextCache {
    source: SharedString,
    revision: u64,
    parent: Option<accesskit::NodeId>,
    direction: Option<accesskit::TextDirection>,
    rows: Vec<Range<usize>>,
    runs: Arc<[AccessibleRun]>,
    ids: Arc<[accesskit::NodeId]>,
    published: Option<PublishedAccessibleText>,
    visible: Range<usize>,
}

impl AccessibleTextCache {
    /// Publishes current selection and viewport geometry without rebuilding
    /// unchanged logical runs. `visible` must cover every byte whose geometry
    /// callback can return cells. Text/row/direction changes invalidate the
    /// logical cache; clipping and viewport changes refresh visible leaves.
    #[allow(clippy::too_many_arguments)]
    pub fn publish(
        &mut self,
        builder: &mut A11ySubtreeBuilder,
        text: &str,
        anchor: usize,
        focus: usize,
        direction: accesskit::TextDirection,
        rows: &[Range<usize>],
        revision: u64,
        visible: Range<usize>,
        scale: f32,
        geometry: impl Fn(Range<usize>) -> Vec<Bounds<Pixels>>,
    ) -> Option<PublishedAccessibleText> {
        publish_accessible_text_inner(
            builder,
            text,
            anchor,
            focus,
            direction,
            rows,
            revision,
            Some((&geometry, scale)),
            Some((self, visible)),
        )
    }
}

fn run_end_character(run: &AccessibleRun) -> usize {
    run.start_character + run.character_lengths.len()
}

// Segment the document once, rather than rescanning it for every published
// run. Counting starts strictly before the word preserves prefix-grapheme
// semantics even if a word boundary is inside an extended grapheme.
fn indexed_word_starts(text: &str, graphemes: &[(usize, &str)]) -> (Vec<(usize, usize)>, usize) {
    let mut character = 0;
    let mut visited_bytes = 0;
    let words = text
        .unicode_word_indices()
        .map(|(offset, _)| {
            while character < graphemes.len() && graphemes[character].0 < offset {
                visited_bytes += graphemes[character].1.len();
                character += 1;
            }
            (offset, character)
        })
        .collect();
    (words, visited_bytes)
}

fn accessible_runs(
    text: &str,
    visual_rows: &[Range<usize>],
    fallback_direction: accesskit::TextDirection,
) -> Vec<AccessibleRun> {
    if text.is_empty() {
        return vec![AccessibleRun {
            value: 0..0,
            start_byte: 0,
            start_character: 0,
            character_lengths: Vec::new(),
            word_starts: Vec::new(),
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
    let (words, _) = indexed_word_starts(text, &graphemes);
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
            let limit = (start + MAX_ACCESSIBLE_RUN_CHARS).min(row_end);
            let direction_end = graphemes[start + 1..limit]
                .iter()
                .position(|(offset, _)| bidi.levels[*offset].is_rtl() != level.is_rtl())
                .map(|offset| start + offset + 1)
                .unwrap_or(limit);
            let end = direction_end;
            let start_byte = graphemes[start].0;
            let end_byte = graphemes
                .get(end)
                .map(|(offset, _)| *offset)
                .unwrap_or(text.len());
            runs.push(AccessibleRun {
                value: start_byte..end_byte,
                start_byte,
                start_character: start,
                character_lengths: graphemes[start..end]
                    .iter()
                    .map(|(_, grapheme)| grapheme.len() as u8)
                    .collect(),
                word_starts: words[words.partition_point(|(offset, _)| *offset < start_byte)
                    ..words.partition_point(|(offset, _)| *offset < end_byte)]
                    .iter()
                    .map(|(_, character)| (character - start) as u8)
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
            value: text.len()..text.len(),
            start_byte: text.len(),
            start_character: graphemes.len(),
            character_lengths: Vec::new(),
            word_starts: Vec::new(),
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
        None,
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
    mut cache: Option<(&mut AccessibleTextCache, Range<usize>)>,
) -> Option<PublishedAccessibleText> {
    let reused = cache.as_ref().is_some_and(|(cache, _)| {
        cache.revision == revision
            && cache.parent == Some(builder.synthetic_node_id(0u8))
            && cache.direction == Some(fallback_direction)
            && cache.source.as_ref() == text
            && cache.rows == visual_rows
    });
    let runs: Arc<[AccessibleRun]> = if reused {
        cache.as_ref().expect("reused cache").0.runs.clone()
    } else {
        accessible_runs(text, visual_rows, fallback_direction).into()
    };
    if runs.is_empty() {
        return None;
    }
    let run_count = runs.len();
    let run_ids: Arc<[accesskit::NodeId]> = if reused {
        cache.as_ref().expect("reused cache").0.ids.clone()
    } else {
        runs.iter()
            .map(|run| {
                builder.synthetic_node_id((
                    revision,
                    run.line,
                    run.start_character,
                    run.character_lengths.len(),
                ))
            })
            .collect()
    };
    for run in 0..run_count {
        let accessible_run = &runs[run];
        let visible = cache.as_ref().is_none_or(|(_, visible)| {
            accessible_run.value.start < visible.end && visible.start < accessible_run.value.end
        });
        let was_visible = cache.as_ref().is_some_and(|(cache, _)| {
            accessible_run.value.start < cache.visible.end
                && cache.visible.start < accessible_run.value.end
        });
        if reused && !visible && !was_visible && builder.retain_child(run_ids[run]) {
            continue;
        }
        let mut node = accesskit::Node::new(accesskit::Role::TextRun);
        node.set_text_direction(accessible_run.direction);
        node.set_value(&text[accessible_run.value.clone()]);
        node.set_character_lengths(accessible_run.character_lengths.clone());
        if !accessible_run.word_starts.is_empty() {
            node.set_word_starts(accessible_run.word_starts.clone());
        }
        if let Some((bounds_for_range, scale)) = geometry.filter(|_| visible) {
            let mut positions = Vec::with_capacity(accessible_run.character_lengths.len());
            let mut widths = Vec::with_capacity(accessible_run.character_lengths.len());
            let mut advance = 0.0;
            let mut union: Option<Bounds<Pixels>> = None;
            for (offset, grapheme) in text[accessible_run.value.clone()].grapheme_indices(true) {
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
    let published = if reused {
        cache.as_ref().expect("reused cache").0.published.clone()
    } else {
        Some(PublishedAccessibleText {
            source: text.into(),
            revision,
            runs: runs
                .iter()
                .zip(run_ids.iter().copied())
                .map(|(run, node)| PublishedRun {
                    node,
                    start_character: run.start_character,
                    character_count: run.character_lengths.len(),
                })
                .collect(),
        })
    };
    if let Some((cache, visible)) = cache.as_mut() {
        if !reused {
            cache.source = text.into();
            cache.revision = revision;
            cache.parent = Some(builder.synthetic_node_id(0u8));
            cache.direction = Some(fallback_direction);
            cache.rows = visual_rows.to_vec();
            cache.runs = runs;
            cache.ids = run_ids;
            cache.published = published.clone();
        }
        cache.visible = visible.clone();
    }
    published
}

fn accessible_position(
    text: &str,
    byte_offset: usize,
    runs: &[AccessibleRun],
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

    #[test]
    fn indexed_words_preserve_global_boundaries_across_runs() {
        for text in [
            format!("{}tail e\u{301}👩‍💻 אבג123 abc\r\n界中文", "a".repeat(254)),
            "can't a\u{301}b אבג DEF\nfoo_bar 12.34 ไทย".into(),
        ] {
            let mut rows = hard_rows(&text);
            // A visual break inside a word must not invent a new word start.
            rows.splice(0..1, [0..2, 2..rows[0].end]);
            let runs = accessible_runs(&text, &rows, accesskit::TextDirection::LeftToRight);
            for run in runs {
                let expected = text
                    .unicode_word_indices()
                    .filter(|(offset, _)| {
                        run.start_byte <= *offset && *offset < run.start_byte + run.value.len()
                    })
                    .map(|(offset, _)| {
                        (text[..offset].graphemes(true).count() - run.start_character) as u8
                    })
                    .collect::<Vec<_>>();
                assert_eq!(run.word_starts, expected, "{}", &text[run.value.clone()]);
            }
        }
    }

    #[test]
    fn large_accessible_word_index_visits_each_byte_at_most_once() {
        for rows in [1000, 10000] {
            let text = format!(
                "[\n{}{{\"tail\":7}}\n]",
                "{\"asymmetric\":\"界\",\"value\":13},\n".repeat(rows)
            );
            let graphemes = text.grapheme_indices(true).collect::<Vec<_>>();
            let (words, visited_bytes) = indexed_word_starts(&text, &graphemes);
            assert!(visited_bytes <= text.len());
            assert_eq!(words.len(), rows * 4 + 2);
            let runs = accessible_runs(
                &text,
                &hard_rows(&text),
                accesskit::TextDirection::LeftToRight,
            );
            assert_eq!(
                runs.iter().map(|run| run.word_starts.len()).sum::<usize>(),
                words.len()
            );
            assert_eq!(
                runs.iter().map(|run| run.value.len()).sum::<usize>(),
                text.len()
            );
        }
    }

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
        assert_eq!(&text[runs[0].value.clone()], "e\u{301}👩‍💻\n");
        assert_eq!(runs[0].character_lengths, vec![3, 11, 1]);
        assert_eq!(&text[runs[1].value.clone()], "אב");
        assert_eq!(runs[1].direction, accesskit::TextDirection::RightToLeft);
    }

    #[test]
    fn trailing_lf_and_crlf_publish_a_distinct_empty_line() {
        for text in ["a\n", "a\r\n"] {
            let rows = hard_rows(text);
            let runs = accessible_runs(text, &rows, accesskit::TextDirection::LeftToRight);
            assert_eq!(runs.len(), 2);
            assert_eq!(&text[runs[0].value.clone()], text);
            assert_eq!(&text[runs[1].value.clone()], "");
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
