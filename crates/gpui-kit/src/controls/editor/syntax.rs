//! Optional in-process incremental parsing. Grammar/query configuration is
//! caller-owned; this adapter owns the parser and revision-paired tree, never
//! a language-server process, filesystem, or workspace.

use std::{collections::HashMap, ops::Range};

use gpui::{EditSnapshot, HighlightStyle, SharedString};
use gpui_kit_theme::{SyntaxColor, Theme};
use tree_sitter::{
    InputEdit, Language, Node, Parser, Point, Query, QueryCursor, StreamingIterator, Tree,
};

use super::{EditorHighlight, TextAreaEdit};

/// Observed input work and reuse for the last syntax parse. Offered bytes may
/// be requested more than once by Tree-sitter; no source bytes are copied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditorParseWork {
    pub incremental: bool,
    pub input_bytes_offered: usize,
    pub input_requests: usize,
}

/// One query capture in UTF-8 source coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorSyntaxCapture {
    pub range: Range<usize>,
    pub name: SharedString,
}

/// A grammar error or missing node, not an LSP diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorSyntaxError {
    pub id: usize,
    pub range: Range<usize>,
    pub kind: SharedString,
    pub missing: bool,
}

/// Revision-aware Tree-sitter adapter, available with the `syntax` feature.
pub struct EditorSyntax {
    parser: Parser,
    query: Query,
    tree: Option<Tree>,
    document: EditSnapshot,
    revision: Option<u64>,
    styles: HashMap<SharedString, HighlightStyle>,
    work: EditorParseWork,
}

impl std::fmt::Debug for EditorSyntax {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorSyntax")
            .field("revision", &self.revision)
            .field("work", &self.work)
            .finish()
    }
}

impl EditorSyntax {
    /// Configures a registry-provided grammar and its highlighting query.
    pub fn new(language: Language, highlights: &str) -> Result<Self, SharedString> {
        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|error| SharedString::from(error.to_string()))?;
        let query = Query::new(&language, highlights)
            .map_err(|error| SharedString::from(error.to_string()))?;
        Ok(Self {
            parser,
            query,
            tree: None,
            document: EditSnapshot::default(),
            revision: None,
            styles: HashMap::new(),
            work: EditorParseWork::default(),
        })
    }

    /// Working bundled JSON grammar; other languages use `new` with their
    /// grammar crate and query. No grammar is fetched at runtime.
    pub fn json() -> Self {
        Self::new(
            tree_sitter_json::LANGUAGE.into(),
            tree_sitter_json::HIGHLIGHTS_QUERY,
        )
        .expect("bundled JSON grammar and query are compatible")
    }

    /// Overrides one named query capture's theme-derived appearance.
    pub fn capture_style(mut self, name: impl Into<SharedString>, style: HighlightStyle) -> Self {
        self.styles.insert(name.into(), style);
        self
    }

    pub fn revision(&self) -> Option<u64> {
        self.revision
    }

    pub fn work(&self) -> EditorParseWork {
        self.work
    }

    /// Parses a revision. `edit`, when supplied, must be the exact transition
    /// from the preceding revision to `document`; missing/skipped revisions
    /// deliberately reparse without an old tree. Stale revisions are refused.
    pub fn update(
        &mut self,
        revision: u64,
        document: EditSnapshot,
        edit: Option<&TextAreaEdit>,
    ) -> bool {
        if self.revision.is_some_and(|previous| revision <= previous) {
            return false;
        }
        let mut old_tree = self.tree.clone();
        let incremental = edit.filter(|edit| {
            self.revision.and_then(|old| old.checked_add(1)) == Some(revision)
                && edit.revision == revision
                && edit.replaced.start <= edit.replaced.end
                && edit.replaced.end <= self.document.len()
                && edit.replaced.start + edit.inserted.len() <= document.len()
        });
        if let (Some(edit), Some(tree)) = (incremental, old_tree.as_mut()) {
            tree.edit(&InputEdit {
                start_byte: edit.replaced.start,
                old_end_byte: edit.replaced.end,
                new_end_byte: edit.replaced.start + edit.inserted.len(),
                start_position: point_at(&self.document, edit.replaced.start),
                old_end_position: point_at(&self.document, edit.replaced.end),
                new_end_position: point_at(&document, edit.replaced.start + edit.inserted.len()),
            });
        } else {
            old_tree = None;
        }
        let mut work = EditorParseWork {
            incremental: old_tree.is_some(),
            ..Default::default()
        };
        let tree = self
            .parser
            .parse_with_options(
                &mut |byte, _| {
                    let bytes = document
                        .byte_chunks(byte..document.len())
                        .next()
                        .unwrap_or_default();
                    work.input_requests += 1;
                    work.input_bytes_offered += bytes.len();
                    bytes
                },
                old_tree.as_ref(),
                None,
            )
            .expect("configured parser without cancellation");
        self.tree = Some(tree);
        self.document = document;
        self.revision = Some(revision);
        self.work = work;
        true
    }

    /// Queries only the requested source region. Captures retain their full
    /// node ranges; callers can clip at paint boundaries. Query exhaustion is
    /// reported rather than presenting a partial result as complete.
    pub fn captures(&self, range: Range<usize>) -> Result<Vec<EditorSyntaxCapture>, SharedString> {
        if range.start > range.end || range.end > self.document.len() {
            return Err("Syntax range is outside the document".into());
        }
        if range.is_empty() {
            return Ok(Vec::new());
        }
        let Some(tree) = &self.tree else {
            return Ok(Vec::new());
        };
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(range.clone());
        let mut output = Vec::new();
        {
            let provider = |node: Node<'_>| self.document.byte_chunks(node.byte_range());
            let mut captures = cursor.captures(&self.query, tree.root_node(), provider);
            while let Some((matched, slot)) = captures.next() {
                let capture = matched.captures[*slot];
                let captured = capture.node.byte_range();
                if captured.start < range.end && range.start < captured.end {
                    output.push(EditorSyntaxCapture {
                        range: captured,
                        name: self.query.capture_names()[capture.index as usize]
                            .to_owned()
                            .into(),
                    });
                }
            }
        }
        if cursor.did_exceed_match_limit() {
            return Err("Syntax query exceeded its match limit".into());
        }
        Ok(output)
    }

    /// Error nodes in source order. Error-free subtrees are skipped outright.
    pub fn errors(&self) -> Vec<EditorSyntaxError> {
        let Some(tree) = &self.tree else {
            return Vec::new();
        };
        let mut pending = vec![tree.root_node()];
        let mut errors = Vec::new();
        while let Some(node) = pending.pop() {
            if node.is_error() || node.is_missing() {
                errors.push(EditorSyntaxError {
                    id: node.id(),
                    range: node.byte_range(),
                    kind: node.kind().to_owned().into(),
                    missing: node.is_missing(),
                });
            } else if node.has_error() {
                let mut cursor = node.walk();
                pending.extend(node.children(&mut cursor));
            }
        }
        errors.sort_by_key(|error| error.range.start);
        errors
    }

    pub(super) fn highlights(
        &self,
        range: Range<usize>,
        theme: &Theme,
    ) -> Result<Vec<EditorHighlight>, SharedString> {
        let captures = self.captures(range.clone())?;
        let mut boundaries = vec![range.start, range.end];
        for capture in &captures {
            boundaries.extend([
                capture.range.start.max(range.start),
                capture.range.end.min(range.end),
            ]);
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        Ok(boundaries
            .windows(2)
            .filter_map(|edge| {
                let range = edge[0]..edge[1];
                let capture = captures
                    .iter()
                    .filter(|capture| {
                        capture.range.start <= range.start && range.end <= capture.range.end
                    })
                    .min_by_key(|capture| {
                        (capture.range.len(), std::cmp::Reverse(capture.name.len()))
                    })?;
                let style = self.styles.get(&capture.name).copied().unwrap_or_else(|| {
                    let class = if capture.name.starts_with("comment") {
                        SyntaxColor::Comment
                    } else if capture.name.starts_with("number") {
                        SyntaxColor::Number
                    } else if capture.name.starts_with("string") {
                        SyntaxColor::StringLiteral
                    } else {
                        SyntaxColor::Keyword
                    };
                    HighlightStyle {
                        color: Some(theme.colors.syntax.get(class)),
                        ..Default::default()
                    }
                });
                Some(EditorHighlight::new(range, style))
            })
            .collect())
    }
}

fn point_at(document: &EditSnapshot, byte: usize) -> Point {
    let row = document.line_at(byte);
    let start = document.line_range(row).expect("indexed syntax row").start;
    Point::new(row, byte - start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{EditBuffer, EditCause};

    #[test]
    fn incremental_json_edits_use_utf8_points_and_match_a_fresh_parse() {
        let before = "{\r\n  \"é\": 1,\n  \"items\": [false, 20]\n}\n";
        let mut buffer = EditBuffer::default();
        buffer.set_text(before);
        let mut syntax = EditorSyntax::json();
        assert!(syntax.update(10, buffer.snapshot(), None));
        assert!(syntax.errors().is_empty());
        let start = before.find('1').expect("number");
        let inserted = "\"😀\",\n  \"new\": true";
        buffer.replace(start..start + 1, inserted, EditCause::Paste);
        let edit = TextAreaEdit {
            revision: 11,
            replaced: start..start + 1,
            inserted: inserted.into(),
        };
        assert!(syntax.update(11, buffer.snapshot(), Some(&edit)));
        assert!(syntax.work().incremental);
        assert!(syntax.errors().is_empty());
        let mut fresh = EditorSyntax::json();
        fresh.update(11, buffer.snapshot(), None);
        let len = buffer.snapshot().len();
        assert_eq!(syntax.captures(0..len), fresh.captures(0..len));
        assert_eq!(
            syntax.tree.as_ref().expect("tree").root_node().to_sexp(),
            fresh
                .tree
                .as_ref()
                .expect("fresh tree")
                .root_node()
                .to_sexp()
        );
        assert_eq!(point_at(&buffer.snapshot(), start), Point::new(1, 8));
        assert!(!syntax.update(9, EditSnapshot::default(), None));
        assert_eq!(syntax.revision(), Some(11));
        syntax.update(15, buffer.snapshot(), Some(&edit));
        assert!(
            !syntax.work().incremental,
            "skipped revisions cannot reuse unedited trees"
        );
    }

    #[test]
    fn grammar_errors_and_invalid_queries_are_reported() {
        let mut buffer = EditBuffer::default();
        buffer.set_text("{\"items\": [1, }");
        let mut syntax = EditorSyntax::json();
        syntax.update(0, buffer.snapshot(), None);
        assert!(!syntax.errors().is_empty());
        assert!(syntax.captures(0..usize::MAX).is_err());
        assert!(
            EditorSyntax::new(tree_sitter_json::LANGUAGE.into(), "(not_a_json_node) @bad").is_err()
        );
    }

    #[test]
    fn a_large_json_edit_reuses_the_tree_and_queries_only_the_requested_region() {
        let text = "[\n".to_owned()
            + &"  {\"label\":\"asymmetric\",\"value\":0},\n".repeat(50_000)
            + "null\n]";
        let mut buffer = EditBuffer::default();
        buffer.set_text(&text);
        let mut syntax = EditorSyntax::json();
        syntax.update(0, buffer.snapshot(), None);
        let start = text.rfind(":0").expect("last value") + 1;
        buffer.replace(start..start + 1, "987321", EditCause::Paste);
        syntax.update(
            1,
            buffer.snapshot(),
            Some(&TextAreaEdit {
                revision: 1,
                replaced: start..start + 1,
                inserted: "987321".into(),
            }),
        );
        assert!(syntax.errors().is_empty());
        assert!(syntax.work().incremental);
        assert!(
            syntax.work().input_bytes_offered < 65_536,
            "{:?}",
            syntax.work()
        );
        let captures = syntax.captures(start..start + 6).expect("query");
        assert_eq!(
            captures,
            vec![EditorSyntaxCapture {
                range: start..start + 6,
                name: "number".into()
            }]
        );
    }
}
