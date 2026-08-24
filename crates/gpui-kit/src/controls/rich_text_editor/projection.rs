//! Flat platform projection of a block document.
//!
//! Native text input, clipboards, and AccessKit speak in one string while the
//! rich model speaks in stable block positions. This mapping is the only
//! place that inserts the one-byte hard break between blocks, so keyboard,
//! pointer, IME, clipboard, and accessibility ranges cannot disagree about
//! which document position a flat offset names.

use std::ops::Range;

use gpui::SharedString;

use crate::content::{RichTextBlockId, RichTextDocument, RichTextPosition, RichTextSelection};

#[derive(Clone, Debug)]
pub(super) struct ProjectedBlock {
    pub id: RichTextBlockId,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug)]
pub(super) struct Projection {
    text: SharedString,
    blocks: Vec<ProjectedBlock>,
}

impl Projection {
    pub fn new(document: &RichTextDocument) -> Self {
        let capacity = document
            .blocks()
            .iter()
            .map(|block| block.text().len())
            .sum::<usize>()
            + document.blocks().len().saturating_sub(1);
        let mut text = String::with_capacity(capacity);
        let mut blocks = Vec::with_capacity(document.blocks().len());
        for (index, block) in document.blocks().iter().enumerate() {
            if index > 0 {
                text.push('\n');
            }
            let start = text.len();
            text.push_str(block.text());
            blocks.push(ProjectedBlock {
                id: block.id().clone(),
                start,
                end: text.len(),
            });
        }
        Self {
            text: text.into(),
            blocks,
        }
    }

    pub fn text(&self) -> &SharedString {
        &self.text
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn blocks(&self) -> &[ProjectedBlock] {
        &self.blocks
    }

    pub fn block(&self, id: &RichTextBlockId) -> Option<&ProjectedBlock> {
        self.blocks.iter().find(|block| block.id == *id)
    }

    pub fn offset_for_position(&self, position: &RichTextPosition) -> Option<usize> {
        let block = self.block(&position.block)?;
        Some(block.start + position.offset.min(block.end - block.start))
    }

    pub fn position_for_offset(&self, offset: usize) -> RichTextPosition {
        let offset = offset.min(self.text.len());
        for (index, block) in self.blocks.iter().enumerate() {
            if offset <= block.end || index + 1 == self.blocks.len() {
                return RichTextPosition::new(
                    block.id.clone(),
                    offset
                        .saturating_sub(block.start)
                        .min(block.end - block.start),
                );
            }
        }
        let block = self
            .blocks
            .last()
            .expect("a rich text document always has one block");
        RichTextPosition::new(block.id.clone(), block.end - block.start)
    }

    pub fn offsets_for_selection(&self, selection: &RichTextSelection) -> Option<(usize, usize)> {
        Some((
            self.offset_for_position(&selection.anchor)?,
            self.offset_for_position(&selection.head)?,
        ))
    }

    pub fn range_for_selection(&self, selection: &RichTextSelection) -> Option<Range<usize>> {
        let (anchor, head) = self.offsets_for_selection(selection)?;
        Some(anchor.min(head)..anchor.max(head))
    }

    pub fn selection_for_range(&self, range: Range<usize>) -> RichTextSelection {
        RichTextSelection::new(
            self.position_for_offset(range.start),
            self.position_for_offset(range.end),
        )
    }

    #[cfg(test)]
    pub fn selected_text(&self, selection: &RichTextSelection) -> Option<String> {
        let range = self.range_for_selection(selection)?;
        self.text.get(range).map(ToOwned::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{RichTextBlock, RichTextDocument};

    fn document() -> RichTextDocument {
        RichTextDocument::new([
            RichTextBlock::new("first", "one"),
            RichTextBlock::new("empty", ""),
            RichTextBlock::new("last", "三"),
        ])
        .expect("fixture is valid")
    }

    #[test]
    fn hard_breaks_have_one_flat_offset_and_stable_block_ends() {
        let projection = Projection::new(&document());
        assert_eq!(projection.text().as_ref(), "one\n\n三");
        assert_eq!(
            projection.position_for_offset(3),
            RichTextPosition::new("first", 3)
        );
        assert_eq!(
            projection.position_for_offset(4),
            RichTextPosition::new("empty", 0)
        );
        assert_eq!(
            projection.position_for_offset(5),
            RichTextPosition::new("last", 0)
        );
    }

    #[test]
    fn a_cross_block_selection_round_trips_through_the_platform_string() {
        let projection = Projection::new(&document());
        let selection = RichTextSelection::new(
            RichTextPosition::new("first", 1),
            RichTextPosition::new("last", "三".len()),
        );
        let range = projection
            .range_for_selection(&selection)
            .expect("known blocks");
        assert_eq!(range, 1..8);
        assert_eq!(
            projection.selected_text(&selection).as_deref(),
            Some("ne\n\n三")
        );
        assert_eq!(projection.selection_for_range(range), selection);
    }
}
