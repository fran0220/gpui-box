//! Lossless native text positions at UTF-16 platform boundaries.

/// Which incident logical text owns a caret at a bidi or soft-wrap boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAffinity {
    /// The trailing edge of the preceding logical text.
    Upstream,
    /// The leading edge of the following logical text. Legacy offset-only
    /// APIs use this affinity explicitly.
    #[default]
    Downstream,
}

/// A UTF-16 position and its visual affinity. Offsets alone do not identify
/// a unique painted caret at bidi and soft-wrap boundaries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NativeTextPosition {
    /// UTF-16 code units from the beginning of the caller's text.
    pub utf16_offset: usize,
    /// Incident logical edge used for this caret.
    pub affinity: TextAffinity,
}

/// Atomic primary selection, retaining both visual endpoints even when their
/// logical offsets coincide. Secondary cursors use the legacy logical policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeTextSelection {
    /// The fixed endpoint of the primary selection.
    pub anchor: NativeTextPosition,
    /// The moving endpoint of the primary selection.
    pub head: NativeTextPosition,
}
