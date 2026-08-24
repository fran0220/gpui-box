//! Framework editable-text vocabulary used by Kit's plain text controls.
//!
//! GPUI owns editing transactions and Unicode arithmetic. This private module
//! keeps the controls' existing local names while making that authority
//! explicit; it contains no second edit engine.

use std::collections::HashMap;
use std::ops::Range;

use gpui::{A11ySubtreeBuilder, Bounds, Pixels, SharedString, accesskit};
use unicode_segmentation::UnicodeSegmentation;

pub(crate) use gpui::{
    EditBuffer, EditCause as Cause, EditRules, PublishedAccessibleText,
    accessible_text_is_representable, byte_offset_for_published_position,
    next_grapheme_boundary as next_boundary, next_word_boundary, normalize_multiline,
    normalize_single_line, offset_to_utf16, paragraph_at,
    previous_grapheme_boundary as previous_boundary, previous_word_boundary, range_from_utf16,
    range_to_utf16, word_at,
};

/// Current-frame painted cells for the text AccessKit exposes.
///
/// Editable controls shape inside a child element, after their semantic owner
/// is built. Capturing those child bounds during prepaint lets the owner publish
/// the same geometry without cloning or independently reshaping the layout.
pub(crate) struct AccessibleTextGeometry {
    source: SharedString,
    scale_factor: f32,
    graphemes: HashMap<(usize, usize), Vec<Bounds<Pixels>>>,
}

impl AccessibleTextGeometry {
    pub(crate) fn capture(
        source: SharedString,
        scale_factor: f32,
        mut bounds_for_range: impl FnMut(Range<usize>) -> Vec<Bounds<Pixels>>,
    ) -> Self {
        let graphemes = source
            .grapheme_indices(true)
            .map(|(start, grapheme)| {
                let range = start..start + grapheme.len();
                let bounds = bounds_for_range(range.clone());
                ((range.start, range.end), bounds)
            })
            .collect();
        Self {
            source,
            scale_factor,
            graphemes,
        }
    }

    fn matches(&self, source: &str) -> bool {
        self.source.as_ref() == source
    }

    fn bounds_for_range(&self, range: Range<usize>) -> Vec<Bounds<Pixels>> {
        self.graphemes
            .get(&(range.start, range.end))
            .cloned()
            .unwrap_or_default()
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn publish_accessible_text(
    builder: &mut A11ySubtreeBuilder,
    text: &str,
    anchor_byte: usize,
    focus_byte: usize,
    fallback_direction: accesskit::TextDirection,
    visual_rows: &[Range<usize>],
    revision: u64,
    geometry: Option<&AccessibleTextGeometry>,
) -> Option<PublishedAccessibleText> {
    if let Some(geometry) = geometry.filter(|geometry| geometry.matches(text)) {
        gpui::publish_accessible_text_with_geometry(
            builder,
            text,
            anchor_byte,
            focus_byte,
            fallback_direction,
            visual_rows,
            revision,
            geometry.scale_factor,
            |range| geometry.bounds_for_range(range),
        )
    } else {
        gpui::publish_accessible_text(
            builder,
            text,
            anchor_byte,
            focus_byte,
            fallback_direction,
            visual_rows,
            revision,
        )
    }
}
