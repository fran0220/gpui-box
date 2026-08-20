//! Text arithmetic shared by the editable controls.
//!
//! A single-line input and a wrapped text area disagree about layout, not
//! about how text is walked: grapheme and word boundaries, the UTF-16 offsets
//! an input method speaks in, and a length limit that truncates rather than
//! rejects are the same arithmetic in both.

mod buffer;
mod history;

pub(crate) use buffer::{EditBuffer, EditRules};
pub(crate) use history::EditCause as Cause;

use std::ops::Range;

pub(crate) use gpui::{
    PublishedAccessibleText, accessible_text_is_representable, byte_offset_for_published_position,
    publish_accessible_text,
};
use unicode_segmentation::UnicodeSegmentation;

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

/// Truncates an insertion to a grapheme limit without splitting a user-
/// perceived character. This is separate from the byte-oriented
/// `max_length` contract so existing callers keep their exact limit meaning.
pub(crate) fn fit_to_max_graphemes(
    text: &str,
    max_graphemes: Option<usize>,
    replacing: &Range<usize>,
    new_text: &str,
) -> String {
    let Some(max_graphemes) = max_graphemes else {
        return new_text.to_string();
    };

    // Grapheme segmentation is not compositional: a combining mark or ZWJ
    // sequence at either insertion edge can join what was already there. Test
    // candidate prefixes in the reconstructed value rather than adding three
    // independently computed counts. Only the two edge graphemes can merge,
    // so no accepted prefix can contain more than `max + 2` standalone
    // insertion graphemes.
    let prefix = &text[..replacing.start];
    let suffix = &text[replacing.end..];
    let mut candidate = String::with_capacity(prefix.len() + new_text.len() + suffix.len());
    let mut accepted = 0;
    for (index, grapheme) in new_text
        .grapheme_indices(true)
        .take(max_graphemes.saturating_add(2))
    {
        let end = index + grapheme.len();
        candidate.clear();
        candidate.push_str(prefix);
        candidate.push_str(&new_text[..end]);
        candidate.push_str(suffix);
        if candidate.graphemes(true).count() <= max_graphemes {
            accepted = end;
        }
    }
    new_text[..accepted].to_string()
}

pub(crate) fn normalize_single_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

pub(crate) fn normalize_multiline(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
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
    fn a_grapheme_limit_counts_extended_characters_not_bytes() {
        assert_eq!(
            fit_to_max_graphemes("", Some(2), &(0..0), "e\u{301}👩‍💻x"),
            "e\u{301}👩‍💻"
        );
        assert_eq!(fit_to_max_graphemes("ab", Some(3), &(1..2), "👩‍💻xy"), "👩‍💻x");
        assert_eq!(fit_to_max_graphemes("ab", None, &(0..0), "cd"), "cd");
    }

    #[test]
    fn a_grapheme_limit_segments_the_reconstructed_value_at_both_edges() {
        assert_eq!(
            fit_to_max_graphemes("a", Some(1), &(1..1), "\u{301}"),
            "\u{301}"
        );
        assert_eq!(fit_to_max_graphemes("\u{301}", Some(1), &(0..0), "a"), "a");
        assert_eq!(
            fit_to_max_graphemes("👩", Some(1), &("👩".len().."👩".len()), "‍💻"),
            "‍💻"
        );
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
}
