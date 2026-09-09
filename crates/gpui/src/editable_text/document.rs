//! Persistent UTF-8 storage. Snapshots share unchanged rope nodes; only callers
//! requesting the legacy contiguous string pay to materialize a whole value.
//! Hard lines are LF-delimited, matching editable layout (not Unicode paragraph
//! separators). Byte offsets remain the public editing coordinate system.

use std::{ops::Range, sync::OnceLock};

use crate::SharedString;
use ropey::Rope;

/// An immutable, cheap-to-clone document revision with indexed hard lines.
#[derive(Clone, Default)]
pub struct EditSnapshot {
    rope: Rope,
    contiguous: OnceLock<SharedString>,
}

impl std::fmt::Debug for EditSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditSnapshot")
            .field("bytes", &self.len())
            .field("lines", &self.line_count())
            .finish()
    }
}

impl EditSnapshot {
    pub(super) fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            contiguous: OnceLock::new(),
        }
    }

    /// UTF-8 byte length, without flattening the rope.
    pub fn len(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Whether there are no bytes (an empty document still has one line).
    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
    }

    /// Number of LF-delimited lines, including an empty trailing line.
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Zero-based hard line containing the clamped byte offset.
    pub fn line_at(&self, offset: usize) -> usize {
        self.rope.byte_to_line(offset.min(self.len()))
    }

    /// Converts a byte offset to UTF-16 units, rounding an interior scalar
    /// offset upward, as the platform editable-text API does.
    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        let offset = offset.min(self.len());
        let mut character = self.rope.byte_to_char(offset);
        if self.rope.char_to_byte(character) < offset {
            character += 1;
        }
        self.rope.char_to_utf16_cu(character)
    }

    /// Converts UTF-16 units to bytes. An offset inside a surrogate pair
    /// rounds upward to the end of that scalar, matching the existing API.
    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        let offset = offset.min(self.rope.len_utf16_cu());
        let mut character = self.rope.utf16_cu_to_char(offset);
        if self.rope.char_to_utf16_cu(character) < offset {
            character += 1;
        }
        self.rope.char_to_byte(character)
    }

    /// Byte range of a zero-based hard line, including its LF if present.
    pub fn line_range(&self, line: usize) -> Option<Range<usize>> {
        if line >= self.line_count() {
            return None;
        }
        let start = self.rope.line_to_byte(line);
        let end = if line + 1 < self.line_count() {
            self.rope.line_to_byte(line + 1)
        } else {
            self.len()
        };
        Some(start..end)
    }

    /// Copies only the requested range. Invalid UTF-8 boundaries return None.
    pub fn slice(&self, range: Range<usize>) -> Option<String> {
        if range.start > range.end || range.end > self.len() {
            return None;
        }
        let start = self.rope.byte_to_char(range.start);
        let end = self.rope.byte_to_char(range.end);
        if self.rope.char_to_byte(start) != range.start || self.rope.char_to_byte(end) != range.end
        {
            return None;
        }
        Some(self.rope.slice(start..end).to_string())
    }

    /// Compatibility snapshot. The first call in a revision copies all bytes;
    /// subsequent calls reuse that allocation. Prefer indexed slices for views.
    pub fn text(&self) -> &SharedString {
        self.contiguous.get_or_init(|| self.rope.to_string().into())
    }

    pub(super) fn replace(&mut self, range: Range<usize>, text: &str) {
        let start = self.rope.byte_to_char(range.start);
        let end = self.rope.byte_to_char(range.end);
        self.rope.remove(start..end);
        self.rope.insert(start, text);
        self.contiguous.take();
    }

    pub(super) fn floor_grapheme(&self, offset: usize) -> usize {
        let offset = offset.min(self.len());
        if offset == self.len() {
            return offset;
        }
        // A hard LF is a grapheme boundary (except CRLF, which remains in
        // this same line). Scan only that line, preserving arbitrarily long
        // combining/ZWJ/regional-indicator sequences across rope chunks.
        let range = self.line_range(self.line_at(offset)).unwrap();
        let start = range.start;
        let line = self.slice(range).unwrap();
        start + super::buffer::floor_grapheme_boundary(&line, offset - start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_unicode_offsets_match_platform_rounding() {
        let text = "a😀e\u{301}\n界z";
        let doc = EditSnapshot::new(text);
        for offset in 0..=text.len() + 3 {
            assert_eq!(
                doc.offset_to_utf16(offset),
                crate::editable_text::offset_to_utf16(text, offset)
            );
            assert_eq!(
                doc.offset_from_utf16(offset),
                crate::editable_text::offset_from_utf16(text, offset)
            );
        }
        assert_eq!(doc.offset_from_utf16(2), 5);
        assert_eq!(doc.offset_to_utf16(3), 3);
    }

    #[test]
    fn indexed_lines_survive_asymmetric_edits_and_persistent_snapshots() {
        let mut doc = EditSnapshot::new("a\r\nβ界\nlast\n");
        let old = doc.clone();
        doc.replace(3..8, "😀\nx");
        assert_eq!(doc.slice(0..doc.len()).unwrap(), "a\r\n😀\nx\nlast\n");
        assert_eq!(doc.line_count(), 5);
        assert_eq!(doc.line_range(1), Some(3..8));
        assert_eq!(doc.line_range(2), Some(8..10));
        assert_eq!(doc.line_at(8), 2);
        assert_eq!(doc.line_range(4), Some(15..15));
        assert_eq!(old.slice(3..8).as_deref(), Some("β界"));
        assert!(doc.slice(4..7).is_none());
        assert!(doc.contiguous.get().is_none());
    }

    #[test]
    fn large_file_edits_do_not_materialize_document_and_lines_stay_indexed() {
        let text = "let asymmetric = '界';\n".repeat(200_000);
        let mut doc = EditSnapshot::new(&text);
        let last = doc.line_range(199_999).unwrap().start;
        for _ in 0..1_000 {
            doc.replace(last..last, "x");
            doc.replace(last..last + 1, "");
            assert_eq!(doc.floor_grapheme(last + 3), last + 3);
        }
        assert_eq!(doc.len(), text.len());
        assert_eq!(doc.line_count(), 200_001);
        assert!(doc.contiguous.get().is_none());
        assert_eq!(doc.slice(last..last + 3).as_deref(), Some("let"));
    }
}
