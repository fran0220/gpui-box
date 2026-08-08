//! Text arithmetic shared by the editable controls.
//!
//! A single-line input and a wrapped text area disagree about layout, not
//! about how text is walked: grapheme and word boundaries, the UTF-16 offsets
//! an input method speaks in, and a length limit that truncates rather than
//! rejects are the same arithmetic in both.

use std::ops::Range;

use gpui::{A11ySubtreeBuilder, accesskit};
use unicode_bidi::{BidiClass, bidi_class};
use unicode_segmentation::UnicodeSegmentation;

const MAX_ACCESSIBLE_RUN_CHARS: usize = 255;

#[derive(Debug, Clone)]
struct AccessibleRun<'a> {
    value: &'a str,
    start_character: usize,
    character_lengths: Vec<u8>,
    line: usize,
}

fn run_end_character(run: &AccessibleRun<'_>) -> usize {
    run.start_character + run.character_lengths.len()
}

fn accessible_runs(text: &str) -> Vec<AccessibleRun<'_>> {
    if text.is_empty() {
        return vec![AccessibleRun {
            value: "",
            start_character: 0,
            character_lengths: Vec::new(),
            line: 0,
        }];
    }

    let graphemes = text.grapheme_indices(true).collect::<Vec<_>>();
    if graphemes
        .iter()
        .any(|(_, grapheme)| grapheme.len() > u8::MAX as usize)
    {
        // AccessKit stores each selectable unit's UTF-8 length in a u8. An
        // extended grapheme has no Unicode length limit, so an adversarially
        // long combining sequence cannot be represented truthfully. Keep the
        // parent value but omit its text-run pattern rather than crashing or
        // publishing lengths whose sum does not match the value.
        return Vec::new();
    }
    let mut runs = Vec::new();
    let mut start = 0;
    let mut line = 0;
    while start < graphemes.len() {
        let hard_break = graphemes[start..]
            .iter()
            .position(|(_, grapheme)| grapheme.ends_with('\n'))
            .map(|offset| start + offset + 1)
            .unwrap_or(graphemes.len());
        let line_end = hard_break;
        while start < line_end {
            let end = (start + MAX_ACCESSIBLE_RUN_CHARS).min(line_end);
            let start_byte = graphemes[start].0;
            let end_byte = graphemes
                .get(end)
                .map(|(offset, _)| *offset)
                .unwrap_or(text.len());
            runs.push(AccessibleRun {
                value: &text[start_byte..end_byte],
                start_character: start,
                character_lengths: graphemes[start..end]
                    .iter()
                    .map(|(_, grapheme)| grapheme.len() as u8)
                    .collect(),
                line,
            });
            start = end;
        }
        line += 1;
    }
    runs
}

fn text_direction(text: &str, fallback: accesskit::TextDirection) -> accesskit::TextDirection {
    text.chars()
        .find_map(|character| match bidi_class(character) {
            BidiClass::L => Some(accesskit::TextDirection::LeftToRight),
            BidiClass::R | BidiClass::AL => Some(accesskit::TextDirection::RightToLeft),
            _ => None,
        })
        .unwrap_or(fallback)
}

pub(crate) fn previous_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .rev()
        .find_map(|(index, _)| (index < offset).then_some(index))
        .unwrap_or(0)
}

pub(crate) fn next_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .find_map(|(index, _)| (index > offset).then_some(index))
        .unwrap_or(text.len())
}

pub(crate) fn previous_word_boundary(text: &str, offset: usize) -> usize {
    text.split_word_bound_indices()
        .rev()
        .find_map(|(index, word)| (index < offset && !word.trim().is_empty()).then_some(index))
        .unwrap_or(0)
}

pub(crate) fn next_word_boundary(text: &str, offset: usize) -> usize {
    text.split_word_bound_indices()
        .find_map(|(index, word)| {
            (index >= offset && !word.trim().is_empty()).then_some(index + word.len())
        })
        .unwrap_or(text.len())
}

/// The word surrounding an offset, for a double click.
///
/// An offset that falls on whitespace selects that whitespace run, which is
/// what a double click between two words does elsewhere.
pub(crate) fn word_at(text: &str, offset: usize) -> Range<usize> {
    let mut last = text.len()..text.len();
    for (index, word) in text.split_word_bound_indices() {
        last = index..index + word.len();
        if offset < last.end {
            break;
        }
    }
    last
}

/// The paragraph surrounding an offset: everything between two hard breaks.
pub(crate) fn paragraph_at(text: &str, offset: usize) -> Range<usize> {
    let offset = offset.min(text.len());
    let start = text[..offset]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let end = text[offset..]
        .find('\n')
        .map(|index| offset + index)
        .unwrap_or(text.len());
    start..end
}

pub(crate) fn offset_from_utf16(text: &str, offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;
    for character in text.chars() {
        if utf16_count >= offset {
            break;
        }
        utf16_count += character.len_utf16();
        utf8_offset += character.len_utf8();
    }
    utf8_offset
}

pub(crate) fn offset_to_utf16(text: &str, offset: usize) -> usize {
    let mut utf16_offset = 0;
    let mut utf8_count = 0;
    for character in text.chars() {
        if utf8_count >= offset {
            break;
        }
        utf8_count += character.len_utf8();
        utf16_offset += character.len_utf16();
    }
    utf16_offset
}

pub(crate) fn range_to_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    offset_to_utf16(text, range.start)..offset_to_utf16(text, range.end)
}

pub(crate) fn range_from_utf16(text: &str, range_utf16: &Range<usize>) -> Range<usize> {
    offset_from_utf16(text, range_utf16.start)..offset_from_utf16(text, range_utf16.end)
}

pub(crate) fn publish_accessible_text(
    builder: &mut A11ySubtreeBuilder,
    text: &str,
    anchor_byte: usize,
    focus_byte: usize,
    fallback_direction: accesskit::TextDirection,
) -> Vec<accesskit::NodeId> {
    let runs = accessible_runs(text);
    if runs.is_empty() {
        return Vec::new();
    }
    let run_count = runs.len();
    let run_ids = (0..run_count)
        .map(|run| builder.synthetic_node_id(run))
        .collect::<Vec<_>>();
    for run in 0..run_count {
        let accessible_run = &runs[run];
        let mut node = accesskit::Node::new(accesskit::Role::TextRun);
        node.set_text_direction(text_direction(accessible_run.value, fallback_direction));
        node.set_value(accessible_run.value);
        node.set_character_lengths(accessible_run.character_lengths.clone());
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
    run_ids
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

pub(crate) fn byte_offset_for_accessible_position(
    text: &str,
    position: accesskit::TextPosition,
    run_ids: &[accesskit::NodeId],
) -> Option<usize> {
    let runs = accessible_runs(text);
    let run_count = runs.len();
    let run = (0..run_count).find(|run| run_ids.get(*run) == Some(&position.node))?;
    if position.character_index > runs[run].character_lengths.len() {
        return None;
    }
    let character = runs[run].start_character + position.character_index;
    Some(
        text.grapheme_indices(true)
            .nth(character)
            .map(|(offset, _)| offset)
            .unwrap_or(text.len()),
    )
}

/// Truncates an insertion that would push the content past its limit, rather
/// than rejecting the whole insertion, so a paste that is slightly too long
/// still delivers what fits.
pub(crate) fn fit_to_max_length(
    text: &str,
    max_length: Option<usize>,
    replacing: &Range<usize>,
    new_text: &str,
) -> String {
    let Some(max_length) = max_length else {
        return new_text.to_string();
    };
    let remaining = max_length.saturating_sub(text.len() - (replacing.end - replacing.start));
    if new_text.len() <= remaining {
        return new_text.to_string();
    }
    new_text
        .grapheme_indices(true)
        .take_while(|(index, grapheme)| index + grapheme.len() <= remaining)
        .map(|(_, grapheme)| grapheme)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries_step_over_a_whole_grapheme() {
        let text = "héllo";
        assert_eq!(next_boundary(text, 0), 1);
        assert_eq!(next_boundary(text, 1), 3);
        assert_eq!(previous_boundary(text, 3), 1);
    }

    #[test]
    fn a_word_is_selected_from_anywhere_inside_it() {
        let text = "alpha beta";
        assert_eq!(word_at(text, 7), 6..10);
        assert_eq!(word_at(text, 6), 6..10);
        assert_eq!(word_at(text, 10), 6..10);
    }

    #[test]
    fn a_paragraph_stops_at_the_hard_breaks_around_it() {
        let text = "one\ntwo\nthree";
        assert_eq!(paragraph_at(text, 5), 4..7);
        assert_eq!(paragraph_at(text, 0), 0..3);
        assert_eq!(paragraph_at(text, 13), 8..13);
    }

    #[test]
    fn a_limit_truncates_on_a_grapheme_boundary() {
        assert_eq!(fit_to_max_length("", Some(4), &(0..0), "héllo"), "hél");
        assert_eq!(fit_to_max_length("ab", Some(3), &(0..0), "cd"), "c");
        assert_eq!(fit_to_max_length("ab", None, &(0..0), "cd"), "cd");
    }

    #[test]
    fn utf16_offsets_account_for_surrogate_pairs() {
        let text = "a😀é";
        assert_eq!(offset_to_utf16(text, 1), 1);
        assert_eq!(offset_to_utf16(text, 5), 3);
        assert_eq!(offset_to_utf16(text, text.len()), 4);
        assert_eq!(offset_from_utf16(text, 3), 5);
        assert_eq!(range_from_utf16(text, &(1..3)), 1..5);
    }

    #[test]
    fn accessible_positions_round_trip_utf8_text() {
        let text = format!("{}e\u{301}👩‍💻\nאב", "x".repeat(255));
        let nodes = |run| accesskit::NodeId(100 + run as u64);
        let runs = accessible_runs(&text);
        let run_ids = (0..runs.len()).map(nodes).collect::<Vec<_>>();
        for offset in [0, 255, 258, 269, 270, text.len()] {
            let position = accessible_position(&text, offset, &runs, nodes);
            assert_eq!(
                byte_offset_for_accessible_position(&text, position, &run_ids),
                Some(offset)
            );
        }
    }

    #[test]
    fn accessible_runs_use_graphemes_and_do_not_link_hard_lines() {
        let text = "e\u{301}👩‍💻\nאב";
        let runs = accessible_runs(text);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].value, "e\u{301}👩‍💻\n");
        assert_eq!(runs[0].character_lengths, vec![3, 11, 1]);
        assert_eq!(runs[1].value, "אב");
        assert_eq!(
            text_direction(runs[1].value, accesskit::TextDirection::LeftToRight),
            accesskit::TextDirection::RightToLeft
        );
    }

    #[test]
    fn an_unrepresentable_grapheme_omits_runs_instead_of_publishing_false_lengths() {
        let text = format!("a{}", "\u{301}".repeat(128));
        assert!(text.len() > u8::MAX as usize);
        assert!(accessible_runs(&text).is_empty());
    }
}
