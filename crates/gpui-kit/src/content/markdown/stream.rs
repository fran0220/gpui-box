//! Reparsing a document that is still arriving, without reparsing all of it.
//!
//! A reply written by a model arrives a few characters at a time and is
//! redrawn on every one of them. Parsing the whole source each time makes the
//! cost of one token proportional to everything said before it, so a long
//! answer gets slower exactly as it gets longer — the reader watches it grind.
//!
//! It does not have to. Text before the start of the last top-level block
//! cannot be changed by something appended after it, so a delta only ever
//! costs the tail. Two blocks are reparsed rather than one, because a block
//! can still merge with the one before it: a trailing `3` becoming `3.` joins
//! the list above. It cannot cascade further back than that, since whether a
//! block stands apart from its predecessor was settled by bytes that arrived
//! before either of them ended.
//!
//! One construct breaks the premise. A link reference definition acts at a
//! distance — `[a]: /x` anywhere resolves `[a]` anywhere — so a source
//! containing one is parsed whole, every time. That is a rare document and a
//! correct answer beats a fast wrong one.

use super::mend;
use super::parse::{Block, Document};

/// A document being read as it arrives.
#[derive(Debug, Default)]
pub(crate) struct Stream {
    source: String,
    document: Document,
    /// Where each block of [`Self::document`] begins in [`Self::source`].
    ///
    /// Empty when the source has to be parsed whole, which is also what makes
    /// that state unrepresentable as a stale set of boundaries.
    starts: Vec<usize>,
    /// Whether this source contains something whose meaning is not local.
    whole: bool,
    /// How many leading blocks the last update left untouched.
    stable: usize,
    /// How many bytes the last update actually parsed, which is the claim this
    /// module makes and therefore the one its tests check.
    parsed: usize,
}

impl Stream {
    /// Reads `source`, doing as little work as it can to be correct about it.
    ///
    /// Appending takes the incremental path. Anything else — an edit, a
    /// replacement, a different document under the same identity — is parsed
    /// whole, because only an append has the property this relies on.
    pub(crate) fn read(&mut self, source: &str) {
        if source == self.source {
            self.parsed = 0;
            self.stable = self.document.blocks.len();
            return;
        }
        let appended = source.len() > self.source.len() && source.starts_with(&self.source);
        if !appended {
            self.reset(source);
            return;
        }

        // A definition may be completed by the bytes that just arrived, so the
        // scan starts at the beginning of the line the source ended in.
        let line = self.source.rfind('\n').map_or(0, |index| index + 1);
        if !self.whole && has_reference_definition(&source[line..]) {
            self.whole = true;
        }
        if self.whole || self.starts.is_empty() {
            self.reset(source);
            return;
        }

        // Back to the start of the second-to-last block, then back to the
        // start of its line: indentation is context, and a fragment that
        // begins mid-line has lost it.
        let boundary = match self.starts.len() {
            0 | 1 => 0,
            count => self.starts[count - 2],
        };
        let boundary = source[..boundary].rfind('\n').map_or(0, |index| index + 1);

        let tail = &source[boundary..];
        let Some(tail_starts) = Document::block_starts(tail) else {
            self.reset(source);
            return;
        };

        let kept = self.starts.partition_point(|start| *start < boundary);
        self.document.blocks.truncate(kept);
        self.starts.truncate(kept);
        self.stable = kept;
        self.parsed = tail.len();
        self.document.blocks.extend(Document::parse(tail).blocks);
        self.starts
            .extend(tail_starts.into_iter().map(|start| start + boundary));
        self.source.clear();
        self.source.push_str(source);
    }

    fn reset(&mut self, source: &str) {
        self.source.clear();
        self.source.push_str(source);
        self.whole = has_reference_definition(source);
        self.document = Document::parse(source);
        self.starts = if self.whole {
            Vec::new()
        } else {
            Document::block_starts(source).unwrap_or_default()
        };
        self.stable = 0;
        self.parsed = source.len();
    }

    /// The document as it is, with every marker read literally.
    pub(crate) fn document(&self) -> &Document {
        &self.document
    }

    /// The document to draw while it is still arriving.
    ///
    /// The last block is the only one that can have a marker still hanging: a
    /// blank line settles a block, and an unclosed marker stays literal across
    /// one. So the mend costs one scan of that block, and a reparse of it only
    /// when something actually hangs.
    ///
    /// The canonical document is never touched by this. A marker that turns
    /// out never to close settles honestly, once, instead of being asserted
    /// forever.
    pub(crate) fn mended(&self) -> Option<Document> {
        let start = *self.starts.last()?;
        let last = self.document.blocks.last()?;
        // A fence renders its own contents verbatim and is stable already; a
        // rule and a table have no inline tail to hang.
        if matches!(last, Block::Code { .. } | Block::Rule | Block::Table { .. }) {
            return None;
        }
        let mended = mend::close_hanging(&self.source[start..])?;

        let mut blocks = self.document.blocks[..self.document.blocks.len() - 1].to_vec();
        blocks.extend(Document::parse(&mended).blocks);
        Some(Document { blocks })
    }

    /// How many leading blocks the last read left alone.
    #[cfg(test)]
    pub(crate) fn stable(&self) -> usize {
        self.stable
    }

    /// How many bytes the last read parsed.
    #[cfg(test)]
    pub(crate) fn parsed(&self) -> usize {
        self.parsed
    }
}

/// Whether this text has a line that could be a link reference definition.
///
/// Deliberately generous. Being wrong in this direction costs a full parse of
/// a document that did not need one; being wrong in the other direction means
/// a link that silently stops resolving partway through a document.
fn has_reference_definition(text: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim_start();
        line.len() - trimmed.len() <= 3 && trimmed.starts_with('[') && trimmed.contains("]:")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds `text` through in `chunks` pieces, the way it would arrive.
    fn streamed(text: &str, chunks: usize) -> Stream {
        let mut stream = Stream::default();
        let size = text.len().div_ceil(chunks.max(1));
        let mut end = 0;
        while end < text.len() {
            end = (end + size).min(text.len());
            while !text.is_char_boundary(end) {
                end += 1;
            }
            stream.read(&text[..end]);
        }
        stream
    }

    const CORPUS: &str = "\
# A heading

Some prose with **bold** and `code` in it, long enough to wrap somewhere.

- a list entry
- another entry
  with a continuation

```rust
fn main() {
    println!(\"hi\");
}
```

> a quotation
> over two lines

| a | b |
|---|---|
| 1 | 2 |

A closing paragraph.
";

    #[test]
    fn arriving_in_pieces_parses_to_the_same_document_as_arriving_at_once() {
        // The whole claim. Anything else this module does is worthless if the
        // reader ends up looking at a different document for having watched it
        // being written.
        let whole = Document::parse(CORPUS);
        for chunks in [1, 2, 3, 7, 13, 40, 200] {
            let streamed = streamed(CORPUS, chunks);
            assert_eq!(
                streamed.document(),
                &whole,
                "{chunks} chunks parsed to something else"
            );
        }
    }

    #[test]
    fn a_delta_costs_the_tail_rather_than_the_document() {
        let mut stream = Stream::default();
        stream.read(CORPUS);

        // Well past the end of everything already settled.
        let before = stream.document().blocks.len();
        let grown = format!("{CORPUS}\nAnd one more sentence arrives.");
        stream.read(&grown);
        assert!(
            stream.parsed() < grown.len() / 2,
            "reparsed {} of {} bytes, which is not incremental",
            stream.parsed(),
            grown.len()
        );
        assert_eq!(
            stream.stable(),
            before - 2,
            "everything but the last two blocks should have been left alone"
        );
        assert_eq!(stream.document(), &Document::parse(&grown));
    }

    #[test]
    fn a_block_that_merges_with_the_one_above_it_is_still_reparsed() {
        // The reason two blocks are reparsed and not one: this paragraph joins
        // the list above it the moment the dot lands.
        let mut stream = Stream::default();
        stream.read("- one\n- two\n\n3");
        stream.read("- one\n- two\n\n3.");
        stream.read("- one\n- two\n\n3. three");
        assert_eq!(
            stream.document(),
            &Document::parse("- one\n- two\n\n3. three")
        );
    }

    #[test]
    fn a_document_with_a_reference_definition_is_parsed_whole() {
        // `[home]` resolves against a definition that may not have arrived
        // yet, so no prefix of this document can be trusted to be finished.
        let mut stream = Stream::default();
        stream.read("See [home].\n\nMore text.\n");
        stream.read("See [home].\n\nMore text.\n\n[home]: /index\n");
        assert_eq!(
            stream.document(),
            &Document::parse("See [home].\n\nMore text.\n\n[home]: /index\n")
        );
        let grown = "See [home].\n\nMore text.\n\n[home]: /index\n\nAnd more.\n";
        stream.read(grown);
        assert_eq!(
            stream.parsed(),
            grown.len(),
            "a definition anywhere means the whole document is reparsed"
        );
        assert_eq!(stream.document(), &Document::parse(grown));
    }

    #[test]
    fn a_hanging_marker_is_mended_for_display_and_not_for_keeps() {
        let mut stream = Stream::default();
        stream.read("Settled text.\n\nThis is **bold");

        let mended = stream.mended().expect("a hanging marker is repaired");
        assert_eq!(
            mended.blocks.len(),
            stream.document().blocks.len(),
            "mending changes how a block reads, not how many there are"
        );
        assert_ne!(
            &mended,
            stream.document(),
            "the display document should differ from the literal one"
        );
        assert_eq!(
            stream.document(),
            &Document::parse("Settled text.\n\nThis is **bold"),
            "what is kept stays exactly what arrived"
        );
    }

    #[test]
    fn a_settled_document_is_not_mended_at_all() {
        let mut stream = Stream::default();
        stream.read("Nothing hangs **here**.\n");
        assert!(stream.mended().is_none());
    }

    #[test]
    fn an_unclosed_fence_is_left_exactly_as_it_arrived() {
        // A fence shows its contents verbatim, so there is nothing to repair
        // and repairing it would put characters on screen nobody wrote.
        let mut stream = Stream::default();
        stream.read("```rust\nfn main() { // **not bold\n");
        assert!(stream.mended().is_none());
    }

    #[test]
    fn text_that_is_not_an_append_is_parsed_whole() {
        let mut stream = Stream::default();
        stream.read("# One\n\nfirst\n");
        stream.read("# Two\n\nsecond\n");
        assert_eq!(stream.document(), &Document::parse("# Two\n\nsecond\n"));
        assert_eq!(stream.stable(), 0, "nothing survives a replacement");
    }

    #[test]
    fn reading_the_same_source_twice_parses_nothing() {
        let mut stream = Stream::default();
        stream.read(CORPUS);
        stream.read(CORPUS);
        assert_eq!(stream.parsed(), 0, "a frame that changed nothing costs");
    }

    #[test]
    fn a_document_arriving_one_byte_at_a_time_is_never_wrong() {
        // Every prefix of the corpus, checked against a parse of that same
        // prefix: not just the destination but every state the reader sees on
        // the way to it.
        let mut stream = Stream::default();
        for end in 1..=CORPUS.len() {
            if !CORPUS.is_char_boundary(end) {
                continue;
            }
            let prefix = &CORPUS[..end];
            stream.read(prefix);
            assert_eq!(
                stream.document(),
                &Document::parse(prefix),
                "diverged at byte {end}"
            );
        }
    }
}
