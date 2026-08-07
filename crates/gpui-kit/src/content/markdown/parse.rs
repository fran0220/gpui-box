//! Markdown source turned into an owned tree, before anything is drawn.
//!
//! Rendering straight from the event stream would mean deciding a block's
//! layout before its contents are known, and would leave nothing for a test to
//! assert against but pixels. The tree is the seam: parsing is pure and
//! testable on its own, truncation is arithmetic over it, and the renderer
//! only ever walks a finished structure.
//!
//! Raw HTML survives parsing as [`Block::Html`] and [`Inline::Html`]. It is
//! never interpreted and never discarded, because a document that could delete
//! its own content from the reader's view by wrapping it in a tag would be a
//! document nobody could trust.

use gpui::SharedString;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Which way a table column's cells are set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellAlign {
    #[default]
    Start,
    Center,
    End,
}

/// A run of content inside one line of prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(SharedString),
    /// Backticked text, set in the mono face and never highlighted.
    Code(SharedString),
    Emphasis(Vec<Inline>),
    Strong(Vec<Inline>),
    Struck(Vec<Inline>),
    Link {
        href: SharedString,
        title: Option<SharedString>,
        content: Vec<Inline>,
    },
    Image {
        src: SharedString,
        alt: SharedString,
        title: Option<SharedString>,
    },
    /// HTML written inside a line, kept verbatim and never interpreted.
    Html(SharedString),
    SoftBreak,
    HardBreak,
}

/// One entry of a bulleted or numbered list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListEntry {
    /// `Some` when the entry carried a task marker, and whether it was ticked.
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

/// One block of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    Code {
        /// The fence's info string, exactly as it was written.
        language: Option<SharedString>,
        text: SharedString,
    },
    Quote(Vec<Block>),
    List {
        ordered: bool,
        start: u64,
        entries: Vec<ListEntry>,
    },
    Rule,
    Table {
        alignment: Vec<CellAlign>,
        head: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    /// An HTML block, kept verbatim and never interpreted.
    Html(SharedString),
}

impl Block {
    /// How many lines this block occupies, counted the way truncation counts.
    ///
    /// A line here is a line of the document's own structure — a heading, a
    /// paragraph, one line of a code fence, one list entry — not a line of
    /// wrapped text, whose count only layout knows.
    pub fn lines(&self) -> usize {
        match self {
            Self::Heading { .. } | Self::Paragraph(_) | Self::Rule => 1,
            Self::Code { text, .. } | Self::Html(text) => text.lines().count().max(1),
            Self::Quote(blocks) => blocks.iter().map(Block::lines).sum::<usize>().max(1),
            Self::List { entries, .. } => entries.iter().map(ListEntry::lines).sum(),
            Self::Table { rows, .. } => 1 + rows.len(),
        }
    }

    /// The first `room` lines of this block, when it can be cut at all.
    ///
    /// Only blocks that are lists of lines can be cut. A table split across
    /// its header, or a paragraph split mid-sentence, would say something the
    /// document does not.
    fn head(&self, room: usize) -> Option<Self> {
        if room == 0 {
            return None;
        }
        match self {
            Self::Code { language, text } => {
                let kept: Vec<&str> = text.lines().take(room).collect();
                Some(Self::Code {
                    language: language.clone(),
                    text: SharedString::from(kept.join("\n")),
                })
            }
            Self::Html(text) => {
                let kept: Vec<&str> = text.lines().take(room).collect();
                Some(Self::Html(SharedString::from(kept.join("\n"))))
            }
            Self::List {
                ordered,
                start,
                entries,
            } => {
                let mut kept = Vec::new();
                let mut used = 0;
                for entry in entries {
                    let lines = entry.lines();
                    if used + lines > room {
                        break;
                    }
                    used += lines;
                    kept.push(entry.clone());
                }
                if kept.is_empty() {
                    None
                } else {
                    Some(Self::List {
                        ordered: *ordered,
                        start: *start,
                        entries: kept,
                    })
                }
            }
            _ => None,
        }
    }
}

impl ListEntry {
    fn lines(&self) -> usize {
        self.blocks.iter().map(Block::lines).sum::<usize>().max(1)
    }
}

/// A parsed document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    pub blocks: Vec<Block>,
}

impl Document {
    /// Parses `source` with tables, strikethrough, and task lists enabled.
    ///
    /// Nothing else is enabled: footnotes, smart punctuation, and math would
    /// each change what a plain document means, and a reader who wrote three
    /// dots did not ask for an ellipsis.
    pub fn parse(source: &str) -> Self {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        build(Parser::new_ext(source, options))
    }

    pub fn lines(&self) -> usize {
        self.blocks.iter().map(Block::lines).sum()
    }

    /// The first `max` lines, and how many lines were left behind.
    ///
    /// The count is the point. A document cut to fit says how much was cut, so
    /// a reader knows there is more rather than having to notice a fade.
    pub fn truncate(&self, max: usize) -> (Self, usize) {
        let total = self.lines();
        if total <= max {
            return (self.clone(), 0);
        }
        let mut blocks = Vec::new();
        let mut kept = 0;
        for block in &self.blocks {
            let lines = block.lines();
            if kept + lines <= max {
                kept += lines;
                blocks.push(block.clone());
                continue;
            }
            if let Some(part) = block.head(max - kept) {
                kept += part.lines();
                blocks.push(part);
            }
            break;
        }
        (Self { blocks }, total.saturating_sub(kept))
    }
}

/// What the walker is currently inside.
#[derive(Debug)]
enum Frame {
    Root,
    Paragraph,
    Heading(u8),
    Quote,
    List {
        ordered: bool,
        start: u64,
        entries: Vec<ListEntry>,
    },
    Entry {
        task: Option<bool>,
    },
    Code {
        language: Option<SharedString>,
    },
    Html,
    Table {
        alignment: Vec<CellAlign>,
        head: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    /// A row, and whether it is the header row.
    Row {
        heading: bool,
        cells: Vec<Vec<Inline>>,
    },
    Cell,
    Emphasis,
    Strong,
    Struck,
    Link {
        href: SharedString,
        title: Option<SharedString>,
    },
    Image {
        src: SharedString,
        title: Option<SharedString>,
    },
    /// A container this renderer has no separate shape for, whose contents
    /// still belong in the document.
    Transparent,
}

/// What has accumulated inside one frame.
#[derive(Debug, Default)]
struct Level {
    blocks: Vec<Block>,
    inlines: Vec<Inline>,
    text: String,
}

fn build<'a>(events: impl Iterator<Item = Event<'a>>) -> Document {
    let mut stack: Vec<(Frame, Level)> = vec![(Frame::Root, Level::default())];

    for event in events {
        match event {
            Event::Start(tag) => stack.push((frame(tag), Level::default())),
            Event::End(end) => close(&mut stack, end),
            Event::Text(text) => push_text(&mut stack, text.as_ref()),
            Event::Code(code) => push_inline(&mut stack, Inline::Code(code.as_ref().into())),
            Event::Html(html) | Event::InlineHtml(html) => push_html(&mut stack, html.as_ref()),
            Event::SoftBreak => push_inline(&mut stack, Inline::SoftBreak),
            Event::HardBreak => push_inline(&mut stack, Inline::HardBreak),
            Event::Rule => push_block(&mut stack, Block::Rule),
            Event::TaskListMarker(checked) => mark_task(&mut stack, checked),
            // Footnotes and math are not enabled, so they never arrive; a
            // reference in the source stays the literal text it was written as.
            _ => {}
        }
    }

    let mut root = stack.remove(0).1;
    flush_paragraph(&mut root);
    Document {
        blocks: root.blocks,
    }
}

fn frame(tag: Tag<'_>) -> Frame {
    match tag {
        Tag::Paragraph => Frame::Paragraph,
        Tag::Heading { level, .. } => Frame::Heading(match level {
            HeadingLevel::H1 => 1,
            HeadingLevel::H2 => 2,
            HeadingLevel::H3 => 3,
            HeadingLevel::H4 => 4,
            HeadingLevel::H5 => 5,
            HeadingLevel::H6 => 6,
        }),
        Tag::BlockQuote(_) => Frame::Quote,
        Tag::CodeBlock(kind) => Frame::Code {
            language: match kind {
                CodeBlockKind::Fenced(info) => {
                    let info = info.trim();
                    (!info.is_empty()).then(|| SharedString::from(info.to_string()))
                }
                CodeBlockKind::Indented => None,
            },
        },
        Tag::HtmlBlock => Frame::Html,
        Tag::List(start) => Frame::List {
            ordered: start.is_some(),
            start: start.unwrap_or(1),
            entries: Vec::new(),
        },
        Tag::Item => Frame::Entry { task: None },
        Tag::Table(alignment) => Frame::Table {
            alignment: alignment
                .into_iter()
                .map(|align| match align {
                    pulldown_cmark::Alignment::Center => CellAlign::Center,
                    pulldown_cmark::Alignment::Right => CellAlign::End,
                    _ => CellAlign::Start,
                })
                .collect(),
            head: Vec::new(),
            rows: Vec::new(),
        },
        Tag::TableHead => Frame::Row {
            heading: true,
            cells: Vec::new(),
        },
        Tag::TableRow => Frame::Row {
            heading: false,
            cells: Vec::new(),
        },
        Tag::TableCell => Frame::Cell,
        Tag::Emphasis => Frame::Emphasis,
        Tag::Strong => Frame::Strong,
        Tag::Strikethrough => Frame::Struck,
        Tag::Link {
            dest_url, title, ..
        } => Frame::Link {
            href: dest_url.as_ref().into(),
            title: optional(title.as_ref()),
        },
        Tag::Image {
            dest_url, title, ..
        } => Frame::Image {
            src: dest_url.as_ref().into(),
            title: optional(title.as_ref()),
        },
        _ => Frame::Transparent,
    }
}

fn optional(text: &str) -> Option<SharedString> {
    (!text.is_empty()).then(|| SharedString::from(text.to_string()))
}

/// Closes the innermost frame and folds it into the one around it.
///
/// The stream is well formed, so the popped frame decides the fold; `end` only
/// guards against unwinding past the root.
fn close(stack: &mut Vec<(Frame, Level)>, end: TagEnd) {
    if stack.len() < 2 {
        debug_assert!(false, "markdown stream closed `{end:?}` past its root");
        return;
    }
    let Some((frame, mut level)) = stack.pop() else {
        return;
    };
    let Some((parent_frame, parent)) = stack.last_mut() else {
        return;
    };

    match frame {
        Frame::Paragraph => {
            if !level.inlines.is_empty() {
                add_block(parent, Block::Paragraph(level.inlines));
            }
        }
        Frame::Heading(heading) => add_block(
            parent,
            Block::Heading {
                level: heading,
                content: level.inlines,
            },
        ),
        Frame::Quote => add_block(parent, Block::Quote(level.blocks)),
        Frame::Code { language } => add_block(
            parent,
            Block::Code {
                language,
                text: SharedString::from(level.text.trim_end_matches('\n').to_string()),
            },
        ),
        Frame::Html => {
            let text = level.text.trim_end_matches('\n').to_string();
            if !text.is_empty() {
                add_block(parent, Block::Html(SharedString::from(text)));
            }
        }
        Frame::List {
            ordered,
            start,
            entries,
        } => add_block(
            parent,
            Block::List {
                ordered,
                start,
                entries,
            },
        ),
        Frame::Entry { task } => {
            flush_paragraph(&mut level);
            if let Frame::List { entries, .. } = parent_frame {
                entries.push(ListEntry {
                    task,
                    blocks: level.blocks,
                });
            }
        }
        Frame::Table {
            alignment,
            head,
            rows,
        } => add_block(
            parent,
            Block::Table {
                alignment,
                head,
                rows,
            },
        ),
        Frame::Row { heading, cells } => {
            if let Frame::Table { head, rows, .. } = parent_frame {
                if heading {
                    *head = cells;
                } else {
                    rows.push(cells);
                }
            }
        }
        Frame::Cell => {
            if let Frame::Row { cells, .. } = parent_frame {
                cells.push(level.inlines);
            }
        }
        Frame::Emphasis => parent.inlines.push(Inline::Emphasis(level.inlines)),
        Frame::Strong => parent.inlines.push(Inline::Strong(level.inlines)),
        Frame::Struck => parent.inlines.push(Inline::Struck(level.inlines)),
        Frame::Link { href, title } => parent.inlines.push(Inline::Link {
            href,
            title,
            content: level.inlines,
        }),
        Frame::Image { src, title } => parent.inlines.push(Inline::Image {
            src,
            alt: SharedString::from(level.text),
            title,
        }),
        Frame::Root | Frame::Transparent => {
            if !level.blocks.is_empty() {
                flush_paragraph(parent);
            }
            parent.blocks.append(&mut level.blocks);
            parent.inlines.append(&mut level.inlines);
        }
    }
}

/// Adds a block to a level, closing whatever prose was already open there.
///
/// A tight list entry's own text arrives as bare inlines, and a nested list
/// arrives as a block, so a level can hold both at once. Flushing first keeps
/// them in the order they were written; flushing at the end of the entry would
/// print the entry's text after its own sublist.
fn add_block(level: &mut Level, block: Block) {
    flush_paragraph(level);
    level.blocks.push(block);
}

/// Turns inlines stranded outside a paragraph — a loose list entry — into one.
fn flush_paragraph(level: &mut Level) {
    if !level.inlines.is_empty() {
        let inlines = std::mem::take(&mut level.inlines);
        level.blocks.push(Block::Paragraph(inlines));
    }
}

fn push_text(stack: &mut [(Frame, Level)], text: &str) {
    let Some((frame, level)) = stack.last_mut() else {
        return;
    };
    match frame {
        // A code fence's body and an image's alt text are strings, not prose.
        Frame::Code { .. } | Frame::Html | Frame::Image { .. } => level.text.push_str(text),
        _ => level.inlines.push(Inline::Text(text.into())),
    }
}

fn push_html(stack: &mut [(Frame, Level)], html: &str) {
    let Some((frame, level)) = stack.last_mut() else {
        return;
    };
    match frame {
        Frame::Html => level.text.push_str(html),
        // Outside an HTML block the fragment sits in a line of prose, so it
        // stays in that line rather than breaking the paragraph in two.
        Frame::Paragraph
        | Frame::Heading(_)
        | Frame::Emphasis
        | Frame::Strong
        | Frame::Struck
        | Frame::Link { .. }
        | Frame::Cell => level.inlines.push(Inline::Html(html.into())),
        _ => level
            .blocks
            .push(Block::Html(SharedString::from(html.trim_end().to_string()))),
    }
}

fn push_inline(stack: &mut [(Frame, Level)], inline: Inline) {
    if let Some((_, level)) = stack.last_mut() {
        level.inlines.push(inline);
    }
}

fn push_block(stack: &mut [(Frame, Level)], block: Block) {
    if let Some((_, level)) = stack.last_mut() {
        add_block(level, block);
    }
}

fn mark_task(stack: &mut [(Frame, Level)], checked: bool) {
    for (frame, _) in stack.iter_mut().rev() {
        if let Frame::Entry { task } = frame {
            *task = Some(checked);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks(source: &str) -> Vec<Block> {
        Document::parse(source).blocks
    }

    fn plain(inlines: &[Inline]) -> String {
        inlines
            .iter()
            .map(|inline| match inline {
                Inline::Text(text) | Inline::Code(text) | Inline::Html(text) => text.to_string(),
                Inline::Emphasis(inner) | Inline::Strong(inner) | Inline::Struck(inner) => {
                    plain(inner)
                }
                Inline::Link { content, .. } => plain(content),
                Inline::Image { alt, .. } => alt.to_string(),
                Inline::SoftBreak | Inline::HardBreak => " ".into(),
            })
            .collect()
    }

    #[test]
    fn every_block_kind_reaches_the_tree() {
        let document = Document::parse(
            "# Title\n\nA paragraph.\n\n> Quoted\n\n```rust\nfn main() {}\n```\n\n\
             - one\n- two\n\n1. first\n\n---\n\n| a | b |\n|---|---|\n| 1 | 2 |\n",
        );
        let kinds: Vec<&str> = document
            .blocks
            .iter()
            .map(|block| match block {
                Block::Heading { .. } => "heading",
                Block::Paragraph(_) => "paragraph",
                Block::Code { .. } => "code",
                Block::Quote(_) => "quote",
                Block::List { ordered: true, .. } => "ordered",
                Block::List { .. } => "bullets",
                Block::Rule => "rule",
                Block::Table { .. } => "table",
                Block::Html(_) => "html",
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                "heading",
                "paragraph",
                "quote",
                "code",
                "bullets",
                "ordered",
                "rule",
                "table"
            ]
        );
    }

    #[test]
    fn a_fences_info_string_is_kept_as_written() {
        let Some(Block::Code { language, text }) = blocks("```rust,no_run\nlet x = 1;\n```").pop()
        else {
            panic!("expected a code block");
        };
        assert_eq!(language.as_deref(), Some("rust,no_run"));
        assert_eq!(text.as_ref(), "let x = 1;");
    }

    #[test]
    fn an_indented_block_claims_no_language() {
        let Some(Block::Code { language, .. }) = blocks("    indented\n").pop() else {
            panic!("expected a code block");
        };
        assert_eq!(language, None);
    }

    #[test]
    fn raw_html_survives_parsing_as_literal_text() {
        let document = Document::parse("<div onclick=\"go()\">hidden</div>\n");
        assert_eq!(
            document.blocks,
            vec![Block::Html("<div onclick=\"go()\">hidden</div>".into())]
        );
    }

    #[test]
    fn inline_html_stays_inside_its_line() {
        let Some(Block::Paragraph(inlines)) = blocks("before <b>bold</b> after").pop() else {
            panic!("expected a paragraph");
        };
        assert!(
            inlines
                .iter()
                .any(|inline| matches!(inline, Inline::Html(html) if html.as_ref() == "<b>")),
            "the tag itself must survive: {inlines:?}"
        );
        assert!(plain(&inlines).contains("bold"));
    }

    #[test]
    fn nested_lists_keep_their_nesting() {
        let Some(Block::List { entries, .. }) = blocks("- outer\n  - inner\n").pop() else {
            panic!("expected a list");
        };
        assert_eq!(entries.len(), 1);
        assert!(
            entries[0]
                .blocks
                .iter()
                .any(|block| matches!(block, Block::List { .. })),
            "the inner list must stay inside its entry"
        );
    }

    #[test]
    fn a_list_entry_is_read_before_the_list_beneath_it() {
        let Some(Block::List { entries, .. }) = blocks("- outer\n  - inner\n").pop() else {
            panic!("expected a list");
        };
        let Some(Block::Paragraph(inlines)) = entries[0].blocks.first() else {
            panic!("the entry's own words come first: {:?}", entries[0].blocks);
        };
        assert_eq!(plain(inlines).trim(), "outer");
        assert!(matches!(entries[0].blocks.get(1), Some(Block::List { .. })));
    }

    #[test]
    fn a_task_marker_is_carried_by_its_entry() {
        let Some(Block::List { entries, .. }) = blocks("- [x] done\n- [ ] open\n").pop() else {
            panic!("expected a list");
        };
        assert_eq!(
            entries.iter().map(|entry| entry.task).collect::<Vec<_>>(),
            vec![Some(true), Some(false)]
        );
    }

    #[test]
    fn a_links_destination_and_an_images_source_are_kept_apart() {
        let Some(Block::Paragraph(inlines)) =
            blocks("[docs](https://example.test/a) ![a cat](cat.png)").pop()
        else {
            panic!("expected a paragraph");
        };
        assert!(inlines.iter().any(|inline| matches!(
            inline,
            Inline::Link { href, .. } if href.as_ref() == "https://example.test/a"
        )));
        assert!(inlines.iter().any(|inline| matches!(
            inline,
            Inline::Image { src, alt, .. } if src.as_ref() == "cat.png" && alt.as_ref() == "a cat"
        )));
    }

    #[test]
    fn a_table_keeps_its_header_apart_from_its_rows() {
        let Some(Block::Table {
            alignment,
            head,
            rows,
        }) = blocks("| a | b |\n|:--|--:|\n| 1 | 2 |\n| 3 | 4 |\n").pop()
        else {
            panic!("expected a table");
        };
        assert_eq!(alignment, vec![CellAlign::Start, CellAlign::End]);
        assert_eq!(head.len(), 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(plain(&rows[1][1]), "4");
    }

    #[test]
    fn lines_count_the_documents_own_structure() {
        let document = Document::parse("# Title\n\n```\na\nb\nc\n```\n\n- one\n- two\n");
        assert_eq!(document.lines(), 1 + 3 + 2);
    }

    #[test]
    fn truncation_reports_exactly_what_it_left_behind() {
        let document = Document::parse("# Title\n\n```\na\nb\nc\nd\n```\n");
        let (short, hidden) = document.truncate(3);
        assert_eq!(hidden, 2);
        assert_eq!(short.lines(), 3);
        let Some(Block::Code { text, .. }) = short.blocks.last() else {
            panic!("the code block must survive, shortened");
        };
        assert_eq!(text.as_ref(), "a\nb");
    }

    #[test]
    fn a_document_that_fits_is_not_truncated_and_hides_nothing() {
        let document = Document::parse("one\n\ntwo\n");
        let (kept, hidden) = document.truncate(10);
        assert_eq!(hidden, 0);
        assert_eq!(kept, document);
    }

    #[test]
    fn an_uncuttable_block_is_left_out_whole_rather_than_split() {
        let document = Document::parse("| a |\n|---|\n| 1 |\n| 2 |\n\ntail\n");
        let (short, hidden) = document.truncate(1);
        assert!(short.blocks.is_empty(), "{short:?}");
        assert_eq!(hidden, document.lines());
    }
}
