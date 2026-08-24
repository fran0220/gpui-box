//! Storage-neutral rich-text document editing.
//!
//! This is the standard document and command vocabulary behind a basic rich
//! editor. It deliberately knows nothing about Markdown, HTML, persistence,
//! collaboration, URL policy, or GPUI elements. A host converts its durable
//! format to this model and owns the [`RichTextEditSession`]; Kit supplies the
//! grapheme-safe reducer and transaction history so every host does not have
//! to invent formatting, list, split, merge, composition, and undo semantics.
//!
//! A block boundary is a hard paragraph break. A `\n` inside a block is a
//! soft break. Inline style is complete coverage rather than overlapping
//! spans, so replacement can never leave an ambiguous formatting edge.

use std::collections::HashSet;
use std::fmt;
use std::ops::Range;

use gpui::{EditableStyleRuns, SharedString, normalize_multiline};
use unicode_segmentation::UnicodeSegmentation;

/// Stable caller-visible identity for one rich-text block.
///
/// It is content identity, not a list position. A host should preserve it
/// while converting to and from its durable document so selection, semantics,
/// and collaboration adapters do not rename every following paragraph after
/// one insertion.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RichTextBlockId(SharedString);

impl RichTextBlockId {
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<&'static str> for RichTextBlockId {
    fn from(value: &'static str) -> Self {
        Self(value.into())
    }
}

impl fmt::Display for RichTextBlockId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Inline formats whose active state can be toggled independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RichTextFormat {
    Bold,
    Italic,
    Underline,
    Strike,
    Code,
}

/// The complete inline style at one source byte.
///
/// Links are data, not actions. The editor reports their destination but does
/// not validate, fetch, or open it; those remain host policy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RichTextInlineStyle {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    code: bool,
    link: Option<SharedString>,
}

impl RichTextInlineStyle {
    pub fn format(&self, format: RichTextFormat) -> bool {
        match format {
            RichTextFormat::Bold => self.bold,
            RichTextFormat::Italic => self.italic,
            RichTextFormat::Underline => self.underline,
            RichTextFormat::Strike => self.strike,
            RichTextFormat::Code => self.code,
        }
    }

    pub fn with_format(mut self, format: RichTextFormat, active: bool) -> Self {
        match format {
            RichTextFormat::Bold => self.bold = active,
            RichTextFormat::Italic => self.italic = active,
            RichTextFormat::Underline => self.underline = active,
            RichTextFormat::Strike => self.strike = active,
            RichTextFormat::Code => self.code = active,
        }
        self
    }

    pub fn link(&self) -> Option<&SharedString> {
        self.link.as_ref()
    }

    pub fn with_link(mut self, destination: Option<SharedString>) -> Self {
        self.link = destination;
        self
    }

    fn set_link(&mut self, destination: Option<SharedString>) {
        self.link = destination;
    }
}

/// Logical paragraph alignment. Start and end follow reading direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RichTextAlignment {
    #[default]
    Start,
    Center,
    End,
}

/// The marker family of a list item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RichTextListKind {
    Unordered,
    Ordered,
}

/// List metadata for one paragraph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RichTextListItem {
    pub kind: RichTextListKind,
    pub depth: u8,
}

impl RichTextListItem {
    pub fn new(kind: RichTextListKind) -> Self {
        Self { kind, depth: 0 }
    }

    pub fn depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }
}

/// Block presentation metadata independent of any storage syntax.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RichTextParagraphStyle {
    alignment: RichTextAlignment,
    list: Option<RichTextListItem>,
}

impl RichTextParagraphStyle {
    pub fn alignment(&self) -> RichTextAlignment {
        self.alignment
    }

    pub fn with_alignment(mut self, alignment: RichTextAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn list(&self) -> Option<RichTextListItem> {
        self.list
    }

    pub fn with_list(mut self, list: Option<RichTextListItem>) -> Self {
        self.list = list;
        self
    }
}

/// One paragraph-like block and its complete inline style coverage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichTextBlock {
    id: RichTextBlockId,
    text: SharedString,
    styles: EditableStyleRuns<RichTextInlineStyle>,
    paragraph: RichTextParagraphStyle,
}

impl RichTextBlock {
    pub fn new(id: impl Into<RichTextBlockId>, text: impl Into<SharedString>) -> Self {
        let id = id.into();
        let text = text.into();
        let styles = EditableStyleRuns::new(text.as_ref(), RichTextInlineStyle::default());
        Self {
            id,
            text,
            styles,
            paragraph: RichTextParagraphStyle::default(),
        }
    }

    pub fn id(&self) -> &RichTextBlockId {
        &self.id
    }

    pub fn text(&self) -> &SharedString {
        &self.text
    }

    pub fn styles(&self) -> &EditableStyleRuns<RichTextInlineStyle> {
        &self.styles
    }

    pub fn paragraph(&self) -> RichTextParagraphStyle {
        self.paragraph
    }

    pub fn with_paragraph(mut self, paragraph: RichTextParagraphStyle) -> Self {
        self.paragraph = paragraph;
        self
    }

    /// Applies one complete style over a grapheme-safe range.
    pub fn with_style(mut self, range: Range<usize>, style: RichTextInlineStyle) -> Self {
        self.styles.set(self.text.as_ref(), range, style);
        self
    }

    fn insertion_style(&self, offset: usize) -> RichTextInlineStyle {
        if self.text.is_empty() {
            return self.styles.style_at(0).clone();
        }
        let at = floor_grapheme_boundary(self.text.as_ref(), offset);
        let preceding = if at == 0 {
            0
        } else {
            previous_grapheme_boundary(self.text.as_ref(), at)
        };
        self.styles.style_at(preceding).clone()
    }

    fn replace(
        &mut self,
        range: Range<usize>,
        replacement: &str,
        style: RichTextInlineStyle,
    ) -> Range<usize> {
        let range = clamp_range(self.text.as_ref(), range);
        let before = self.text.to_string();
        self.styles.replace(&before, range.clone(), replacement);
        let next = before[..range.start].to_owned() + replacement + &before[range.end..];
        let inserted = range.start..range.start + replacement.len();
        if !inserted.is_empty() {
            self.styles.set(&next, inserted.clone(), style);
        }
        self.text = next.into();
        inserted
    }

    fn map_styles(
        &mut self,
        range: Range<usize>,
        mut map: impl FnMut(RichTextInlineStyle) -> RichTextInlineStyle,
    ) {
        let range = clamp_range(self.text.as_ref(), range);
        if range.is_empty() {
            return;
        }
        let runs = self.styles.runs().to_vec();
        let mut start = 0;
        for run in runs {
            let end = start + run.len;
            let overlap = start.max(range.start)..end.min(range.end);
            if !overlap.is_empty() {
                self.styles.set(self.text.as_ref(), overlap, map(run.style));
            }
            start = end;
        }
    }

    fn every_style(
        &self,
        range: Range<usize>,
        predicate: impl Fn(&RichTextInlineStyle) -> bool,
    ) -> bool {
        let range = clamp_range(self.text.as_ref(), range);
        if range.is_empty() {
            return false;
        }
        let mut start = 0;
        let mut covered = 0;
        for run in self.styles.runs() {
            let end = start + run.len;
            let overlap = start.max(range.start)..end.min(range.end);
            if !overlap.is_empty() {
                if !predicate(&run.style) {
                    return false;
                }
                covered += overlap.len();
            }
            start = end;
        }
        covered == range.len()
    }
}

/// A non-empty storage-neutral rich-text document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichTextDocument {
    blocks: Vec<RichTextBlock>,
}

impl RichTextDocument {
    /// Validates a caller-built document.
    pub fn new(blocks: impl IntoIterator<Item = RichTextBlock>) -> Result<Self, RichTextError> {
        let blocks = blocks.into_iter().collect::<Vec<_>>();
        if blocks.is_empty() {
            return Err(RichTextError::EmptyDocument);
        }
        let mut ids = HashSet::with_capacity(blocks.len());
        for block in &blocks {
            if block.id.as_str().is_empty() {
                return Err(RichTextError::EmptyBlockId);
            }
            if !ids.insert(block.id.clone()) {
                return Err(RichTextError::DuplicateBlockId(block.id.clone()));
            }
        }
        Ok(Self { blocks })
    }

    pub fn empty(block_id: impl Into<RichTextBlockId>) -> Result<Self, RichTextError> {
        Self::new([RichTextBlock::new(block_id, SharedString::default())])
    }

    pub fn plain(
        block_id: impl Into<RichTextBlockId>,
        text: impl Into<SharedString>,
    ) -> Result<Self, RichTextError> {
        Self::new([RichTextBlock::new(block_id, text)])
    }

    pub fn blocks(&self) -> &[RichTextBlock] {
        &self.blocks
    }

    pub fn block(&self, id: &RichTextBlockId) -> Option<&RichTextBlock> {
        self.blocks.iter().find(|block| block.id == *id)
    }

    pub fn selection_at_start(&self) -> RichTextSelection {
        RichTextSelection::caret(RichTextPosition::new(self.blocks[0].id.clone(), 0))
    }

    pub fn selection_at_end(&self) -> RichTextSelection {
        let block = self.blocks.last().expect("rich text is never blockless");
        RichTextSelection::caret(RichTextPosition::new(block.id.clone(), block.text.len()))
    }

    fn index_of(&self, id: &RichTextBlockId) -> Result<usize, RichTextError> {
        self.blocks
            .iter()
            .position(|block| block.id == *id)
            .ok_or_else(|| RichTextError::UnknownBlock(id.clone()))
    }

    fn clamp_position(
        &self,
        position: &RichTextPosition,
    ) -> Result<RichTextPosition, RichTextError> {
        let index = self.index_of(&position.block)?;
        Ok(RichTextPosition::new(
            position.block.clone(),
            floor_grapheme_boundary(self.blocks[index].text.as_ref(), position.offset),
        ))
    }

    fn resolve(&self, selection: &RichTextSelection) -> Result<ResolvedSelection, RichTextError> {
        let anchor = self.clamp_position(&selection.anchor)?;
        let head = self.clamp_position(&selection.head)?;
        let anchor_index = self.index_of(&anchor.block)?;
        let head_index = self.index_of(&head.block)?;
        let reversed = (head_index, head.offset) < (anchor_index, anchor.offset);
        let (start, start_index, end, end_index) = if reversed {
            (head, head_index, anchor, anchor_index)
        } else {
            (anchor, anchor_index, head, head_index)
        };
        Ok(ResolvedSelection {
            start,
            start_index,
            end,
            end_index,
            reversed,
        })
    }
}

/// One UTF-8 position inside a stable block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichTextPosition {
    pub block: RichTextBlockId,
    pub offset: usize,
}

impl RichTextPosition {
    pub fn new(block: impl Into<RichTextBlockId>, offset: usize) -> Self {
        Self {
            block: block.into(),
            offset,
        }
    }
}

/// An anchor and moving head. Their order is preserved across bidi and drag
/// selection; document operations normalize them only while applying an edit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichTextSelection {
    pub anchor: RichTextPosition,
    pub head: RichTextPosition,
}

impl RichTextSelection {
    pub fn new(anchor: RichTextPosition, head: RichTextPosition) -> Self {
        Self { anchor, head }
    }

    pub fn caret(position: RichTextPosition) -> Self {
        Self {
            anchor: position.clone(),
            head: position,
        }
    }

    pub fn is_caret(&self) -> bool {
        self.anchor == self.head
    }
}

/// One normalized document range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichTextRange {
    pub start: RichTextPosition,
    pub end: RichTextPosition,
}

/// What supplied a text replacement, for transaction grouping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RichTextInputKind {
    Typing,
    Deleting,
    Paste,
    Cut,
}

/// A typed command accepted by [`RichTextEditSession::apply`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RichTextIntent {
    Select(RichTextSelection),
    Replace {
        text: SharedString,
        kind: RichTextInputKind,
    },
    /// Replaces the selection with plain text whose normalized line breaks
    /// become hard blocks. There must be one fresh stable id per break.
    ReplaceMultiline {
        text: SharedString,
        new_blocks: Vec<RichTextBlockId>,
        kind: RichTextInputKind,
    },
    InsertSoftBreak,
    InsertHardBreak {
        new_block: RichTextBlockId,
    },
    BackspaceAtStart,
    ToggleFormat(RichTextFormat),
    SetLink(Option<SharedString>),
    SetAlignment(RichTextAlignment),
    SetList(Option<RichTextListKind>),
    ChangeListDepth(i8),
    Compose {
        text: SharedString,
        selection_in_text: Option<Range<usize>>,
    },
    EndComposition,
    Undo,
    Redo,
}

/// What one command changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RichTextEditResult {
    pub document_changed: bool,
    pub selection_changed: bool,
    pub pending_style_changed: bool,
}

impl RichTextEditResult {
    pub fn changed(self) -> bool {
        self.document_changed || self.selection_changed || self.pending_style_changed
    }
}

/// Invalid caller-owned document or position data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RichTextError {
    EmptyDocument,
    EmptyBlockId,
    DuplicateBlockId(RichTextBlockId),
    UnknownBlock(RichTextBlockId),
    BlockIdCount { expected: usize, actual: usize },
}

impl fmt::Display for RichTextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDocument => formatter.write_str("a rich-text document needs one block"),
            Self::EmptyBlockId => formatter.write_str("a rich-text block id cannot be empty"),
            Self::DuplicateBlockId(id) => {
                write!(formatter, "rich-text block id `{id}` is repeated")
            }
            Self::UnknownBlock(id) => write!(formatter, "rich-text block `{id}` does not exist"),
            Self::BlockIdCount { expected, actual } => write!(
                formatter,
                "multiline rich-text replacement needs {expected} new block ids, got {actual}"
            ),
        }
    }
}

impl std::error::Error for RichTextError {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SessionSnapshot {
    document: RichTextDocument,
    selection: RichTextSelection,
    pending_style: RichTextInlineStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HistoryKind {
    Typing,
    Other,
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    before: SessionSnapshot,
    after: SessionSnapshot,
    kind: HistoryKind,
}

#[derive(Clone, Debug)]
struct Composition {
    before: SessionSnapshot,
    marked: RichTextRange,
}

/// Caller-owned editing state and reusable rich-document transaction history.
///
/// A component may hold an entity containing this session, but the host owns
/// that entity and decides its lifetime. Replacing the authoritative document
/// clears history; undo can therefore never resurrect a document the host has
/// superseded.
#[derive(Debug)]
pub struct RichTextEditSession {
    document: RichTextDocument,
    selection: RichTextSelection,
    pending_style: RichTextInlineStyle,
    done: Vec<HistoryEntry>,
    undone: Vec<HistoryEntry>,
    history_allowed: bool,
    composition: Option<Composition>,
}

impl RichTextEditSession {
    pub fn new(document: RichTextDocument) -> Self {
        let selection = document.selection_at_start();
        let pending_style = document.blocks[0].insertion_style(0);
        Self {
            document,
            selection,
            pending_style,
            done: Vec::new(),
            undone: Vec::new(),
            history_allowed: true,
            composition: None,
        }
    }

    pub fn document(&self) -> &RichTextDocument {
        &self.document
    }

    pub fn selection(&self) -> &RichTextSelection {
        &self.selection
    }

    pub fn pending_style(&self) -> &RichTextInlineStyle {
        &self.pending_style
    }

    pub fn marked_range(&self) -> Option<&RichTextRange> {
        self.composition
            .as_ref()
            .map(|composition| &composition.marked)
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// Stops recording permanently and clears recoverable text.
    pub fn forbid_history(&mut self) {
        self.history_allowed = false;
        self.done.clear();
        self.undone.clear();
        self.composition = None;
    }

    /// Replaces the host authority and forgets transactions over the old one.
    pub fn replace_document(
        &mut self,
        document: RichTextDocument,
        selection: RichTextSelection,
    ) -> Result<(), RichTextError> {
        let selection = clamp_selection(&document, &selection)?;
        let pending_style = insertion_style_at_head(&document, &selection)?;
        self.document = document;
        self.selection = selection;
        self.pending_style = pending_style;
        self.done.clear();
        self.undone.clear();
        self.composition = None;
        Ok(())
    }

    pub fn apply(&mut self, intent: RichTextIntent) -> Result<RichTextEditResult, RichTextError> {
        match intent {
            RichTextIntent::Select(selection) => return self.select(selection),
            RichTextIntent::Compose {
                text,
                selection_in_text,
            } => return self.compose(text.as_ref(), selection_in_text),
            RichTextIntent::EndComposition => return self.end_composition(),
            RichTextIntent::Undo => return self.undo(),
            RichTextIntent::Redo => return self.redo(),
            _ => {}
        }

        let _ = self.end_composition()?;
        let before = self.snapshot();
        let old_selection = self.selection.clone();
        let old_pending = self.pending_style.clone();
        let (_, kind) = match intent {
            RichTextIntent::Replace { text, kind } => {
                let normalized = normalize_multiline(text.as_ref());
                let changed = self.replace_selection(&normalized)?;
                let history = if kind == RichTextInputKind::Typing
                    && !normalized.chars().any(char::is_whitespace)
                {
                    HistoryKind::Typing
                } else {
                    HistoryKind::Other
                };
                (changed, history)
            }
            RichTextIntent::ReplaceMultiline {
                text,
                new_blocks,
                kind: _,
            } => {
                let normalized = normalize_multiline(text.as_ref());
                let expected = normalized.matches('\n').count();
                if new_blocks.len() != expected {
                    return Err(RichTextError::BlockIdCount {
                        expected,
                        actual: new_blocks.len(),
                    });
                }
                let mut seen = self
                    .document
                    .blocks
                    .iter()
                    .map(|block| block.id.clone())
                    .collect::<HashSet<_>>();
                for id in &new_blocks {
                    if id.as_str().is_empty() {
                        return Err(RichTextError::EmptyBlockId);
                    }
                    if !seen.insert(id.clone()) {
                        return Err(RichTextError::DuplicateBlockId(id.clone()));
                    }
                }
                let mut parts = normalized.split('\n');
                let mut changed = self.replace_selection(parts.next().unwrap_or_default())?;
                for (part, id) in parts.zip(new_blocks) {
                    changed |= self.split_block(id)?;
                    if !part.is_empty() {
                        changed |= self.replace_selection(part)?;
                    }
                }
                (changed, HistoryKind::Other)
            }
            RichTextIntent::InsertSoftBreak => (self.replace_selection("\n")?, HistoryKind::Other),
            RichTextIntent::InsertHardBreak { new_block } => {
                (self.split_block(new_block)?, HistoryKind::Other)
            }
            RichTextIntent::BackspaceAtStart => (self.backspace_at_start()?, HistoryKind::Other),
            RichTextIntent::ToggleFormat(format) => {
                (self.toggle_format(format)?, HistoryKind::Other)
            }
            RichTextIntent::SetLink(destination) => {
                (self.set_link(destination)?, HistoryKind::Other)
            }
            RichTextIntent::SetAlignment(alignment) => {
                (self.set_alignment(alignment)?, HistoryKind::Other)
            }
            RichTextIntent::SetList(kind) => (self.set_list(kind)?, HistoryKind::Other),
            RichTextIntent::ChangeListDepth(delta) => {
                (self.change_list_depth(delta)?, HistoryKind::Other)
            }
            RichTextIntent::Select(_)
            | RichTextIntent::Compose { .. }
            | RichTextIntent::EndComposition
            | RichTextIntent::Undo
            | RichTextIntent::Redo => unreachable!("handled before mutation"),
        };

        let document_changed = self.document != before.document;
        if document_changed {
            self.record(before, kind);
        }
        Ok(RichTextEditResult {
            document_changed,
            selection_changed: self.selection != old_selection,
            pending_style_changed: self.pending_style != old_pending,
        })
    }

    fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            document: self.document.clone(),
            selection: self.selection.clone(),
            pending_style: self.pending_style.clone(),
        }
    }

    fn restore(&mut self, snapshot: SessionSnapshot) {
        self.document = snapshot.document;
        self.selection = snapshot.selection;
        self.pending_style = snapshot.pending_style;
        self.composition = None;
    }

    fn select(
        &mut self,
        selection: RichTextSelection,
    ) -> Result<RichTextEditResult, RichTextError> {
        let _ = self.end_composition()?;
        let old_selection = self.selection.clone();
        let old_pending = self.pending_style.clone();
        self.selection = clamp_selection(&self.document, &selection)?;
        self.pending_style = insertion_style_at_head(&self.document, &self.selection)?;
        Ok(RichTextEditResult {
            selection_changed: self.selection != old_selection,
            pending_style_changed: self.pending_style != old_pending,
            ..Default::default()
        })
    }

    fn replace_selection(&mut self, replacement: &str) -> Result<bool, RichTextError> {
        let resolved = self.document.resolve(&self.selection)?;
        let old_document = self.document.clone();
        let inserted_style = self.pending_style.clone();
        let start_id = resolved.start.block.clone();
        let insertion_start = resolved.start.offset;

        if resolved.start_index == resolved.end_index {
            let block = &mut self.document.blocks[resolved.start_index];
            block.replace(
                resolved.start.offset..resolved.end.offset,
                replacement,
                inserted_style,
            );
        } else {
            let paragraph = self.document.blocks[resolved.start_index].paragraph;
            let start_block = &self.document.blocks[resolved.start_index];
            let end_block = &self.document.blocks[resolved.end_index];
            let mut segments = style_segments(start_block, 0..resolved.start.offset);
            if !replacement.is_empty() {
                segments.push((replacement.to_owned(), inserted_style.clone()));
            }
            segments.extend(style_segments(
                end_block,
                resolved.end.offset..end_block.text.len(),
            ));
            self.document.blocks[resolved.start_index] =
                block_from_segments(start_id.clone(), paragraph, inserted_style, segments);
            self.document
                .blocks
                .drain(resolved.start_index + 1..=resolved.end_index);
        }

        let caret = RichTextPosition::new(start_id, insertion_start + replacement.len());
        self.selection = RichTextSelection::caret(caret);
        self.pending_style = insertion_style_at_head(&self.document, &self.selection)?;
        Ok(self.document != old_document)
    }

    fn split_block(&mut self, new_block: RichTextBlockId) -> Result<bool, RichTextError> {
        if new_block.as_str().is_empty() {
            return Err(RichTextError::EmptyBlockId);
        }
        if self.document.block(&new_block).is_some() {
            return Err(RichTextError::DuplicateBlockId(new_block));
        }
        if !self.selection.is_caret() {
            self.replace_selection("")?;
        }
        let resolved = self.document.resolve(&self.selection)?;
        let index = resolved.start_index;
        let offset = resolved.start.offset;
        let block = &self.document.blocks[index];

        // Enter on an empty list item leaves the list rather than creating an
        // endless chain of empty bullets.
        if block.text.is_empty() && block.paragraph.list.is_some() {
            self.document.blocks[index].paragraph.list = None;
            self.pending_style = self.document.blocks[index].insertion_style(0);
            return Ok(true);
        }

        let head = style_segments(block, 0..offset);
        let tail = style_segments(block, offset..block.text.len());
        let id = block.id.clone();
        let paragraph = block.paragraph;
        let default_style = block.insertion_style(offset);
        self.document.blocks[index] =
            block_from_segments(id, paragraph, default_style.clone(), head);
        self.document.blocks.insert(
            index + 1,
            block_from_segments(new_block.clone(), paragraph, default_style, tail),
        );
        self.selection = RichTextSelection::caret(RichTextPosition::new(new_block, 0));
        self.pending_style = self.document.blocks[index + 1].insertion_style(0);
        Ok(true)
    }

    fn backspace_at_start(&mut self) -> Result<bool, RichTextError> {
        if !self.selection.is_caret() {
            return self.replace_selection("");
        }
        let resolved = self.document.resolve(&self.selection)?;
        if resolved.start.offset > 0 {
            return Ok(false);
        }
        let index = resolved.start_index;
        let paragraph = &mut self.document.blocks[index].paragraph;
        if let Some(mut list) = paragraph.list {
            if list.depth > 0 {
                list.depth -= 1;
                paragraph.list = Some(list);
            } else {
                paragraph.list = None;
            }
            return Ok(true);
        }
        if index == 0 {
            return Ok(false);
        }

        let previous = &self.document.blocks[index - 1];
        let current = &self.document.blocks[index];
        let caret = previous.text.len();
        let mut segments = style_segments(previous, 0..previous.text.len());
        segments.extend(style_segments(current, 0..current.text.len()));
        let id = previous.id.clone();
        let paragraph = previous.paragraph;
        let default_style = previous.insertion_style(caret);
        self.document.blocks[index - 1] =
            block_from_segments(id.clone(), paragraph, default_style, segments);
        self.document.blocks.remove(index);
        self.selection = RichTextSelection::caret(RichTextPosition::new(id, caret));
        self.pending_style = self.document.blocks[index - 1].insertion_style(caret);
        Ok(true)
    }

    fn toggle_format(&mut self, format: RichTextFormat) -> Result<bool, RichTextError> {
        let resolved = self.document.resolve(&self.selection)?;
        if resolved.start == resolved.end {
            let active = !self.pending_style.format(format);
            self.pending_style = self.pending_style.clone().with_format(format, active);
            return Ok(false);
        }
        let active = !self.every_selected_style(&resolved, |style| style.format(format));
        for index in resolved.start_index..=resolved.end_index {
            let range = selected_range_in_block(&self.document.blocks[index], index, &resolved);
            self.document.blocks[index]
                .map_styles(range, |style| style.with_format(format, active));
        }
        self.pending_style = insertion_style_at_head(&self.document, &self.selection)?;
        Ok(true)
    }

    fn set_link(&mut self, destination: Option<SharedString>) -> Result<bool, RichTextError> {
        let resolved = self.document.resolve(&self.selection)?;
        if resolved.start == resolved.end {
            self.pending_style.set_link(destination);
            return Ok(false);
        }
        let changed = !self.every_selected_style(&resolved, |style| style.link == destination);
        for index in resolved.start_index..=resolved.end_index {
            let range = selected_range_in_block(&self.document.blocks[index], index, &resolved);
            let destination = destination.clone();
            self.document.blocks[index].map_styles(range, |mut style| {
                style.set_link(destination.clone());
                style
            });
        }
        self.pending_style = insertion_style_at_head(&self.document, &self.selection)?;
        Ok(changed)
    }

    fn set_alignment(&mut self, alignment: RichTextAlignment) -> Result<bool, RichTextError> {
        let resolved = self.document.resolve(&self.selection)?;
        let mut changed = false;
        for block in &mut self.document.blocks[resolved.start_index..=resolved.end_index] {
            changed |= block.paragraph.alignment != alignment;
            block.paragraph.alignment = alignment;
        }
        Ok(changed)
    }

    fn set_list(&mut self, kind: Option<RichTextListKind>) -> Result<bool, RichTextError> {
        let resolved = self.document.resolve(&self.selection)?;
        let mut changed = false;
        for block in &mut self.document.blocks[resolved.start_index..=resolved.end_index] {
            let next = kind.map(|kind| RichTextListItem {
                kind,
                depth: block.paragraph.list.map(|item| item.depth).unwrap_or(0),
            });
            changed |= block.paragraph.list != next;
            block.paragraph.list = next;
        }
        Ok(changed)
    }

    fn change_list_depth(&mut self, delta: i8) -> Result<bool, RichTextError> {
        if delta == 0 {
            return Ok(false);
        }
        let resolved = self.document.resolve(&self.selection)?;
        let mut changed = false;
        for block in &mut self.document.blocks[resolved.start_index..=resolved.end_index] {
            let Some(mut item) = block.paragraph.list else {
                continue;
            };
            let next = if delta.is_positive() {
                item.depth.saturating_add(delta as u8)
            } else {
                item.depth.saturating_sub(delta.unsigned_abs())
            };
            changed |= next != item.depth;
            item.depth = next;
            block.paragraph.list = Some(item);
        }
        Ok(changed)
    }

    fn every_selected_style(
        &self,
        resolved: &ResolvedSelection,
        predicate: impl Fn(&RichTextInlineStyle) -> bool + Copy,
    ) -> bool {
        (resolved.start_index..=resolved.end_index).all(|index| {
            let block = &self.document.blocks[index];
            block.every_style(selected_range_in_block(block, index, resolved), predicate)
        })
    }

    fn compose(
        &mut self,
        text: &str,
        selection_in_text: Option<Range<usize>>,
    ) -> Result<RichTextEditResult, RichTextError> {
        let old_selection = self.selection.clone();
        let old_pending = self.pending_style.clone();
        let before_document = self.document.clone();
        let composition_before = self
            .composition
            .as_ref()
            .map(|composition| composition.before.clone())
            .unwrap_or_else(|| self.snapshot());
        if let Some(composition) = &self.composition {
            self.selection = RichTextSelection::new(
                composition.marked.start.clone(),
                composition.marked.end.clone(),
            );
        }
        let normalized = normalize_multiline(text);
        let resolved = self.document.resolve(&self.selection)?;
        let start = resolved.start.clone();
        self.replace_selection(&normalized)?;
        let end = RichTextPosition::new(start.block.clone(), start.offset + normalized.len());
        let inside = selection_in_text
            .map(|range| clamp_range(&normalized, range))
            .unwrap_or(normalized.len()..normalized.len());
        self.selection = RichTextSelection::new(
            RichTextPosition::new(start.block.clone(), start.offset + inside.start),
            RichTextPosition::new(start.block.clone(), start.offset + inside.end),
        );
        self.pending_style = insertion_style_at_head(&self.document, &self.selection)?;
        self.composition = Some(Composition {
            before: composition_before,
            marked: RichTextRange { start, end },
        });
        Ok(RichTextEditResult {
            document_changed: self.document != before_document,
            selection_changed: self.selection != old_selection,
            pending_style_changed: self.pending_style != old_pending,
        })
    }

    fn end_composition(&mut self) -> Result<RichTextEditResult, RichTextError> {
        let Some(composition) = self.composition.take() else {
            return Ok(RichTextEditResult::default());
        };
        let changed = composition.before.document != self.document;
        if changed {
            self.record(composition.before, HistoryKind::Other);
        }
        Ok(RichTextEditResult {
            document_changed: changed,
            ..Default::default()
        })
    }

    fn record(&mut self, before: SessionSnapshot, kind: HistoryKind) {
        if !self.history_allowed || before.document == self.document {
            return;
        }
        let after = self.snapshot();
        self.undone.clear();
        if kind == HistoryKind::Typing
            && let Some(last) = self.done.last_mut()
            && last.kind == HistoryKind::Typing
            && last.after == before
        {
            last.after = after;
            return;
        }
        self.done.push(HistoryEntry {
            before,
            after,
            kind,
        });
    }

    fn undo(&mut self) -> Result<RichTextEditResult, RichTextError> {
        let _ = self.end_composition()?;
        let Some(entry) = self.done.pop() else {
            return Ok(RichTextEditResult::default());
        };
        let before = self.snapshot();
        self.restore(entry.before.clone());
        self.undone.push(entry);
        Ok(snapshot_result(&before, &self.snapshot()))
    }

    fn redo(&mut self) -> Result<RichTextEditResult, RichTextError> {
        let Some(entry) = self.undone.pop() else {
            return Ok(RichTextEditResult::default());
        };
        let before = self.snapshot();
        self.restore(entry.after.clone());
        self.done.push(entry);
        Ok(snapshot_result(&before, &self.snapshot()))
    }
}

#[derive(Clone, Debug)]
struct ResolvedSelection {
    start: RichTextPosition,
    start_index: usize,
    end: RichTextPosition,
    end_index: usize,
    #[allow(dead_code)]
    reversed: bool,
}

fn clamp_selection(
    document: &RichTextDocument,
    selection: &RichTextSelection,
) -> Result<RichTextSelection, RichTextError> {
    Ok(RichTextSelection::new(
        document.clamp_position(&selection.anchor)?,
        document.clamp_position(&selection.head)?,
    ))
}

fn insertion_style_at_head(
    document: &RichTextDocument,
    selection: &RichTextSelection,
) -> Result<RichTextInlineStyle, RichTextError> {
    let head = document.clamp_position(&selection.head)?;
    let index = document.index_of(&head.block)?;
    Ok(document.blocks[index].insertion_style(head.offset))
}

fn selected_range_in_block(
    block: &RichTextBlock,
    index: usize,
    selection: &ResolvedSelection,
) -> Range<usize> {
    let start = if index == selection.start_index {
        selection.start.offset
    } else {
        0
    };
    let end = if index == selection.end_index {
        selection.end.offset
    } else {
        block.text.len()
    };
    start..end
}

fn style_segments(
    block: &RichTextBlock,
    range: Range<usize>,
) -> Vec<(String, RichTextInlineStyle)> {
    let range = clamp_range(block.text.as_ref(), range);
    let mut result = Vec::new();
    let mut start = 0;
    for run in block.styles.runs() {
        let end = start + run.len;
        let overlap = start.max(range.start)..end.min(range.end);
        if !overlap.is_empty() {
            result.push((block.text[overlap].to_owned(), run.style.clone()));
        }
        start = end;
    }
    result
}

fn block_from_segments(
    id: RichTextBlockId,
    paragraph: RichTextParagraphStyle,
    default_style: RichTextInlineStyle,
    segments: Vec<(String, RichTextInlineStyle)>,
) -> RichTextBlock {
    let text = segments
        .iter()
        .map(|(text, _)| text.as_str())
        .collect::<String>();
    let mut styles = EditableStyleRuns::new(&text, default_style);
    let mut offset = 0;
    for (segment, style) in segments {
        let end = offset + segment.len();
        styles.set(&text, offset..end, style);
        offset = end;
    }
    RichTextBlock {
        id,
        text: text.into(),
        styles,
        paragraph,
    }
}

fn snapshot_result(before: &SessionSnapshot, after: &SessionSnapshot) -> RichTextEditResult {
    RichTextEditResult {
        document_changed: before.document != after.document,
        selection_changed: before.selection != after.selection,
        pending_style_changed: before.pending_style != after.pending_style,
    }
}

fn floor_grapheme_boundary(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    if offset == text.len() {
        return offset;
    }
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|index| *index <= offset)
        .last()
        .unwrap_or(0)
}

fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .rev()
        .find_map(|(index, _)| (index < offset).then_some(index))
        .unwrap_or(0)
}

fn clamp_range(text: &str, range: Range<usize>) -> Range<usize> {
    let start = floor_grapheme_boundary(text, range.start);
    let end = floor_grapheme_boundary(text, range.end);
    start.min(end)..start.max(end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn session(text: &str) -> RichTextEditSession {
        RichTextEditSession::new(RichTextDocument::plain("a", text).expect("valid document"))
    }

    fn select(
        session: &mut RichTextEditSession,
        start: (&'static str, usize),
        end: (&'static str, usize),
    ) {
        session
            .apply(RichTextIntent::Select(RichTextSelection::new(
                RichTextPosition::new(start.0, start.1),
                RichTextPosition::new(end.0, end.1),
            )))
            .expect("valid selection");
    }

    #[test]
    fn a_document_rejects_missing_empty_and_duplicate_identity() {
        assert_eq!(
            RichTextDocument::new(Vec::<RichTextBlock>::new()),
            Err(RichTextError::EmptyDocument)
        );
        assert_eq!(
            RichTextDocument::new([RichTextBlock::new("", "x")]),
            Err(RichTextError::EmptyBlockId)
        );
        assert_eq!(
            RichTextDocument::new([
                RichTextBlock::new("same", "a"),
                RichTextBlock::new("same", "b"),
            ]),
            Err(RichTextError::DuplicateBlockId("same".into()))
        );
    }

    #[test]
    fn selection_snaps_to_a_whole_grapheme() {
        let mut session = session("a👩‍💻b");
        select(&mut session, ("a", 2), ("a", 8));
        assert_eq!(session.selection().anchor.offset, 1);
        assert_eq!(session.selection().head.offset, 1);
    }

    #[test]
    fn a_caret_format_becomes_the_style_of_inserted_text() {
        let mut session = session("");
        session
            .apply(RichTextIntent::ToggleFormat(RichTextFormat::Bold))
            .expect("format");
        session
            .apply(RichTextIntent::Replace {
                text: "hello".into(),
                kind: RichTextInputKind::Typing,
            })
            .expect("type");
        let block = &session.document().blocks()[0];
        assert_eq!(block.text().as_ref(), "hello");
        assert!(block.styles().style_at(0).format(RichTextFormat::Bold));
    }

    #[test]
    fn formatting_crosses_blocks_without_touching_unselected_text() {
        let document = RichTextDocument::new([
            RichTextBlock::new("a", "alpha"),
            RichTextBlock::new("b", "beta"),
        ])
        .expect("document");
        let mut session = RichTextEditSession::new(document);
        select(&mut session, ("a", 2), ("b", 2));
        session
            .apply(RichTextIntent::ToggleFormat(RichTextFormat::Italic))
            .expect("format");

        let first = &session.document().blocks()[0];
        let second = &session.document().blocks()[1];
        assert!(!first.styles().style_at(0).format(RichTextFormat::Italic));
        assert!(first.styles().style_at(2).format(RichTextFormat::Italic));
        assert!(second.styles().style_at(0).format(RichTextFormat::Italic));
        assert!(!second.styles().style_at(3).format(RichTextFormat::Italic));
    }

    #[test]
    fn soft_and_hard_breaks_are_different_shapes() {
        let mut session = session("ab");
        select(&mut session, ("a", 1), ("a", 1));
        session
            .apply(RichTextIntent::InsertSoftBreak)
            .expect("soft break");
        assert_eq!(session.document().blocks().len(), 1);
        assert_eq!(session.document().blocks()[0].text().as_ref(), "a\nb");

        session
            .apply(RichTextIntent::InsertHardBreak {
                new_block: "b".into(),
            })
            .expect("hard break");
        assert_eq!(session.document().blocks().len(), 2);
        assert_eq!(session.selection().head, RichTextPosition::new("b", 0));
    }

    #[test]
    fn split_and_merge_preserve_inline_styles() {
        let block = RichTextBlock::new("a", "bold plain").with_style(
            0..4,
            RichTextInlineStyle::default().with_format(RichTextFormat::Bold, true),
        );
        let mut session =
            RichTextEditSession::new(RichTextDocument::new([block]).expect("valid document"));
        select(&mut session, ("a", 5), ("a", 5));
        session
            .apply(RichTextIntent::InsertHardBreak {
                new_block: "b".into(),
            })
            .expect("split");
        assert_eq!(session.document().blocks()[1].text().as_ref(), "plain");
        assert!(
            !session.document().blocks()[1]
                .styles()
                .style_at(0)
                .format(RichTextFormat::Bold)
        );

        session
            .apply(RichTextIntent::BackspaceAtStart)
            .expect("merge");
        assert_eq!(session.document().blocks().len(), 1);
        assert_eq!(session.document().blocks()[0].text().as_ref(), "bold plain");
        assert!(
            session.document().blocks()[0]
                .styles()
                .style_at(0)
                .format(RichTextFormat::Bold)
        );
        assert!(
            !session.document().blocks()[0]
                .styles()
                .style_at(6)
                .format(RichTextFormat::Bold)
        );
    }

    #[test]
    fn deleting_across_blocks_keeps_the_outer_content_and_styles() {
        let bold = RichTextInlineStyle::default().with_format(RichTextFormat::Bold, true);
        let document = RichTextDocument::new([
            RichTextBlock::new("a", "alpha").with_style(0..2, bold.clone()),
            RichTextBlock::new("b", "middle"),
            RichTextBlock::new("c", "omega").with_style(3..5, bold),
        ])
        .expect("document");
        let mut session = RichTextEditSession::new(document);
        select(&mut session, ("a", 2), ("c", 3));
        session
            .apply(RichTextIntent::Replace {
                text: SharedString::default(),
                kind: RichTextInputKind::Deleting,
            })
            .expect("delete");
        let block = &session.document().blocks()[0];
        assert_eq!(session.document().blocks().len(), 1);
        assert_eq!(block.text().as_ref(), "alga");
        assert!(block.styles().style_at(0).format(RichTextFormat::Bold));
        assert!(block.styles().style_at(3).format(RichTextFormat::Bold));
    }

    #[test]
    fn list_backspace_outdents_then_leaves_then_merges() {
        let paragraph = RichTextParagraphStyle::default().with_list(Some(
            RichTextListItem::new(RichTextListKind::Unordered).depth(1),
        ));
        let document = RichTextDocument::new([
            RichTextBlock::new("a", "first"),
            RichTextBlock::new("b", "second").with_paragraph(paragraph),
        ])
        .expect("document");
        let mut session = RichTextEditSession::new(document);
        select(&mut session, ("b", 0), ("b", 0));

        session
            .apply(RichTextIntent::BackspaceAtStart)
            .expect("outdent");
        assert_eq!(
            session.document().blocks()[1]
                .paragraph()
                .list()
                .expect("the item remains in its list")
                .depth,
            0
        );
        session
            .apply(RichTextIntent::BackspaceAtStart)
            .expect("leave list");
        assert_eq!(session.document().blocks()[1].paragraph().list(), None);
        session
            .apply(RichTextIntent::BackspaceAtStart)
            .expect("merge");
        assert_eq!(
            session.document().blocks()[0].text().as_ref(),
            "firstsecond"
        );
    }

    #[test]
    fn one_word_of_typing_is_one_undo_step() {
        let mut session = session("");
        for character in ["a", "b", "c"] {
            session
                .apply(RichTextIntent::Replace {
                    text: character.into(),
                    kind: RichTextInputKind::Typing,
                })
                .expect("type");
        }
        session.apply(RichTextIntent::Undo).expect("undo");
        assert_eq!(session.document().blocks()[0].text().as_ref(), "");
        assert!(session.can_redo());
        session.apply(RichTextIntent::Redo).expect("redo");
        assert_eq!(session.document().blocks()[0].text().as_ref(), "abc");
    }

    #[test]
    fn formatting_and_structure_undo_atomically() {
        let mut session = session("alpha");
        select(&mut session, ("a", 0), ("a", 5));
        session
            .apply(RichTextIntent::ToggleFormat(RichTextFormat::Underline))
            .expect("underline");
        session.apply(RichTextIntent::Undo).expect("undo");
        assert!(
            !session.document().blocks()[0]
                .styles()
                .style_at(0)
                .format(RichTextFormat::Underline)
        );

        select(&mut session, ("a", 2), ("a", 2));
        session
            .apply(RichTextIntent::InsertHardBreak {
                new_block: "b".into(),
            })
            .expect("split");
        session.apply(RichTextIntent::Undo).expect("undo split");
        assert_eq!(session.document().blocks().len(), 1);
        assert_eq!(session.selection().head.offset, 2);
    }

    #[test]
    fn composition_updates_become_one_transaction() {
        let mut session = session("");
        for text in ["n", "ni", "你"] {
            session
                .apply(RichTextIntent::Compose {
                    text: text.into(),
                    selection_in_text: None,
                })
                .expect("compose");
        }
        assert!(session.marked_range().is_some());
        session
            .apply(RichTextIntent::EndComposition)
            .expect("commit composition");
        session.apply(RichTextIntent::Undo).expect("undo");
        assert_eq!(session.document().blocks()[0].text().as_ref(), "");
    }

    #[test]
    fn host_replacement_and_secret_sessions_cannot_resurrect_old_text() {
        let mut session = session("old");
        select(&mut session, ("a", 3), ("a", 3));
        session
            .apply(RichTextIntent::Replace {
                text: " text".into(),
                kind: RichTextInputKind::Paste,
            })
            .expect("edit");
        let replacement = RichTextDocument::plain("new", "authority").expect("valid document");
        let selection = replacement.selection_at_end();
        session
            .replace_document(replacement, selection)
            .expect("replace authority");
        assert!(!session.can_undo());

        session.forbid_history();
        session
            .apply(RichTextIntent::Replace {
                text: " secret".into(),
                kind: RichTextInputKind::Typing,
            })
            .expect("type without history");
        assert!(!session.can_undo());
    }

    #[test]
    fn multiline_replacement_is_one_atomic_history_step() {
        let mut session = session("old");
        select(&mut session, ("a", 0), ("a", 3));
        session
            .apply(RichTextIntent::ReplaceMultiline {
                text: "first\nsecond\n".into(),
                new_blocks: vec!["b".into(), "c".into()],
                kind: RichTextInputKind::Paste,
            })
            .expect("multiline replacement");
        assert_eq!(
            session
                .document()
                .blocks()
                .iter()
                .map(|block| block.text().as_ref())
                .collect::<Vec<_>>(),
            ["first", "second", ""]
        );

        session.apply(RichTextIntent::Undo).expect("undo");
        assert_eq!(session.document().blocks().len(), 1);
        assert_eq!(session.document().blocks()[0].text().as_ref(), "old");
        session.apply(RichTextIntent::Redo).expect("redo");
        assert_eq!(session.document().blocks().len(), 3);
    }

    #[test]
    fn multiline_replacement_refuses_bad_ids_before_mutating() {
        let mut session = session("old");
        let before = session.document().clone();
        assert_eq!(
            session.apply(RichTextIntent::ReplaceMultiline {
                text: "first\nsecond".into(),
                new_blocks: Vec::new(),
                kind: RichTextInputKind::Paste,
            }),
            Err(RichTextError::BlockIdCount {
                expected: 1,
                actual: 0,
            })
        );
        assert_eq!(session.document(), &before);

        assert_eq!(
            session.apply(RichTextIntent::ReplaceMultiline {
                text: "first\nsecond".into(),
                new_blocks: vec!["a".into()],
                kind: RichTextInputKind::Paste,
            }),
            Err(RichTextError::DuplicateBlockId("a".into()))
        );
        assert_eq!(session.document(), &before);
    }

    #[test]
    fn every_edit_keeps_complete_style_coverage() {
        let mut session = session("a👩‍💻b");
        let edits = [
            RichTextIntent::Replace {
                text: "é".into(),
                kind: RichTextInputKind::Typing,
            },
            RichTextIntent::InsertSoftBreak,
            RichTextIntent::ToggleFormat(RichTextFormat::Code),
            RichTextIntent::Replace {
                text: "x".into(),
                kind: RichTextInputKind::Paste,
            },
        ];
        for edit in edits {
            session.apply(edit).expect("valid edit");
            for block in session.document().blocks() {
                assert_eq!(
                    block
                        .styles()
                        .runs()
                        .iter()
                        .map(|run| run.len)
                        .sum::<usize>(),
                    block.text().len()
                );
                let mut offset = 0;
                for grapheme in block.text().graphemes(true) {
                    offset += grapheme.len();
                    assert!(block.text().is_char_boundary(offset));
                }
            }
        }
    }

    proptest! {
        #[test]
        fn arbitrary_replacements_keep_style_edges_on_graphemes(
            source in proptest::collection::vec(any::<char>(), 0..32),
            replacement in proptest::collection::vec(any::<char>(), 0..16),
            first in any::<usize>(),
            second in any::<usize>(),
        ) {
            let source = source.into_iter().collect::<String>();
            let replacement = replacement.into_iter().collect::<String>();
            let mut session = RichTextEditSession::new(
                RichTextDocument::plain("a", source.clone()).expect("valid document"),
            );
            let divisor = source.len().saturating_add(1);
            let first = first % divisor;
            let second = second % divisor;
            select(&mut session, ("a", first), ("a", second));
            session
                .apply(RichTextIntent::ToggleFormat(RichTextFormat::Bold))
                .expect("format");
            session
                .apply(RichTextIntent::Replace {
                    text: replacement.into(),
                    kind: RichTextInputKind::Paste,
                })
                .expect("replacement");

            for block in session.document().blocks() {
                let text = block.text().as_ref();
                let boundaries = std::iter::once(0)
                    .chain(text.grapheme_indices(true).map(|(offset, _)| offset))
                    .chain(std::iter::once(text.len()))
                    .collect::<HashSet<_>>();
                let mut offset = 0;
                for run in block.styles().runs() {
                    offset += run.len;
                    prop_assert!(boundaries.contains(&offset));
                }
                prop_assert_eq!(offset, text.len());
            }
        }
    }
}
