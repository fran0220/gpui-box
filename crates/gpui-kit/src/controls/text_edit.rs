//! Text arithmetic shared by the editable controls.
//!
//! A single-line input and a wrapped text area disagree about layout, not
//! about how text is walked: grapheme and word boundaries, the UTF-16 offsets
//! an input method speaks in, and a length limit that truncates rather than
//! rejects are the same arithmetic in both.

use std::ops::Range;

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
}
