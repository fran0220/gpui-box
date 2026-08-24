//! Framework editable-text vocabulary used by Kit's plain text controls.
//!
//! GPUI owns editing transactions and Unicode arithmetic. This private module
//! keeps the controls' existing local names while making that authority
//! explicit; it contains no second edit engine.

pub(crate) use gpui::{
    EditBuffer, EditCause as Cause, EditRules, PublishedAccessibleText,
    accessible_text_is_representable, byte_offset_for_published_position,
    next_grapheme_boundary as next_boundary, next_word_boundary, normalize_multiline,
    normalize_single_line, offset_to_utf16, paragraph_at,
    previous_grapheme_boundary as previous_boundary, previous_word_boundary,
    publish_accessible_text, range_from_utf16, range_to_utf16, word_at,
};
