//! Colouring code without pretending to understand it.
//!
//! This is a scanner, not a parser. It finds keywords, strings, comments and
//! numbers, and says nothing about anything else — no types, no calls, no
//! scopes, no errors. That is a deliberate ceiling: those need a grammar per
//! language and a resolver behind it, and a design system that shipped a
//! half-right one would be putting a claim on the screen it cannot support.
//! Four classes are what can be found reliably by looking, and they are what
//! makes code read as code.
//!
//! It is paint and only paint. Every span lands on the same glyphs at the same
//! size in the same font, so a block looks identical whether or not the pass
//! has run: no reflow when colour arrives, and nothing to hold layout back
//! while it is computed.
//!
//! Scanning is line by line with a small carry for what crosses lines — a
//! block comment, a triple-quoted string. That is what lets a block being
//! streamed be coloured as it arrives instead of once it stops.

use std::ops::Range;
use std::rc::Rc;

use gpui::SharedString;
use gpui_kit_theme::SyntaxColor;

use crate::content::markdown::CodeSpan;

/// A language this can scan.
///
/// Deliberately short. Each entry is a table someone has to keep true, and a
/// language coloured from a stale table reads worse than one left plain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    TypeScript,
    Python,
    Go,
    Json,
    Shell,
    Toml,
    Yaml,
    Markdown,
}

impl Language {
    /// The language a fence's info string names, when it names one of these.
    ///
    /// Only what was written is read. Guessing a language from the shape of
    /// the code would put a claim on the screen that nobody made, and would be
    /// wrong exactly where code is shortest.
    pub fn named(tag: &str) -> Option<Self> {
        match tag.trim().to_ascii_lowercase().as_str() {
            "rust" | "rs" => Some(Self::Rust),
            "ts" | "tsx" | "typescript" | "js" | "jsx" | "javascript" | "mjs" | "cjs" => {
                Some(Self::TypeScript)
            }
            "python" | "py" => Some(Self::Python),
            "go" | "golang" => Some(Self::Go),
            "json" | "jsonc" => Some(Self::Json),
            "bash" | "sh" | "shell" | "zsh" | "console" => Some(Self::Shell),
            "toml" => Some(Self::Toml),
            "yaml" | "yml" => Some(Self::Yaml),
            "md" | "markdown" => Some(Self::Markdown),
            _ => None,
        }
    }
}

/// What the scanner is in the middle of, carried from one line to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Carry {
    #[default]
    None,
    Comment,
    /// Inside a string, identified by which of the language's string forms
    /// opened it.
    Text(u8),
}

/// Colours a block a line at a time, in byte offsets into each line.
///
/// This is the shape the scanner works in, and the shape a view that draws one
/// element per line needs.
pub fn line_spans_of(language: Language, code: &str) -> Vec<Vec<CodeSpan>> {
    let mut carry = Carry::None;
    code.split('\n')
        .map(|line| {
            let (spans, next) = line_spans(language, line, carry);
            carry = next;
            spans
        })
        .collect()
}

/// Colours a whole block, in byte offsets into it.
///
/// The same colouring as [`line_spans_of`], moved onto the block's own
/// offsets, because it is derived from it rather than found again — two scans
/// that could disagree about the same code would be one too many.
pub fn spans(language: Language, code: &str) -> Vec<CodeSpan> {
    flatten(&line_spans_of(language, code), code)
}

fn flatten(lines: &[Vec<CodeSpan>], code: &str) -> Vec<CodeSpan> {
    let mut offset = 0;
    let mut spans = Vec::new();
    for (line, found) in code.split('\n').zip(lines) {
        spans.extend(found.iter().map(|span| CodeSpan {
            range: span.range.start + offset..span.range.end + offset,
            role: span.role,
        }));
        // Every line but the last gave up a newline to the split.
        offset += line.len() + 1;
    }
    spans
}

/// What a block was last coloured as, so a frame that changed nothing scans
/// nothing.
///
/// Held in a keyed slot beside the block it belongs to. Losing it costs a
/// rescan and nothing else, which is why it is a cache and not state.
#[derive(Default)]
pub(crate) struct Cache {
    of: Option<(Language, SharedString)>,
    lines: Rc<Vec<Vec<CodeSpan>>>,
    block: Rc<Vec<CodeSpan>>,
}

impl Cache {
    fn scan(&mut self, language: Language, code: &SharedString) {
        let stale = match &self.of {
            Some((was, had)) => *was != language || had != code,
            None => true,
        };
        if stale {
            let lines = line_spans_of(language, code);
            self.block = Rc::new(flatten(&lines, code));
            self.lines = Rc::new(lines);
            self.of = Some((language, code.clone()));
        }
    }

    /// The colouring in offsets into the whole block.
    pub(crate) fn block(&mut self, language: Language, code: &SharedString) -> Rc<Vec<CodeSpan>> {
        self.scan(language, code);
        self.block.clone()
    }

    /// The colouring in offsets into each line.
    pub(crate) fn lines(
        &mut self,
        language: Language,
        code: &SharedString,
    ) -> Rc<Vec<Vec<CodeSpan>>> {
        self.scan(language, code);
        self.lines.clone()
    }
}

/// Colours one line, given what the line before it left open.
pub fn line_spans(language: Language, line: &str, carry: Carry) -> (Vec<CodeSpan>, Carry) {
    let grammar = grammar(language);
    let bytes = line.as_bytes();
    let mut spans = Vec::new();

    // Markdown is not scanned so much as glanced at: a heading line reads as
    // one, and nothing else claims to be understood.
    if language == Language::Markdown && line.trim_start().starts_with('#') {
        if !line.is_empty() {
            spans.push(span(0..line.len(), SyntaxColor::Keyword));
        }
        return (spans, Carry::None);
    }

    let mut at = 0usize;

    match carry {
        Carry::Comment => {
            let close = grammar.block_comment.map_or("*/", |(_, close)| close);
            match line.find(close) {
                Some(found) => {
                    let end = found + close.len();
                    spans.push(span(0..end, SyntaxColor::Comment));
                    at = end;
                }
                None => {
                    if !line.is_empty() {
                        spans.push(span(0..line.len(), SyntaxColor::Comment));
                    }
                    return (spans, Carry::Comment);
                }
            }
        }
        Carry::Text(which) => {
            let Some(text) = grammar.texts.get(which as usize) else {
                return (spans, Carry::None);
            };
            match closing(line, 0, text) {
                Some(end) => {
                    spans.push(span(0..end, SyntaxColor::StringLiteral));
                    at = end;
                }
                None => {
                    if !line.is_empty() {
                        spans.push(span(0..line.len(), SyntaxColor::StringLiteral));
                    }
                    return (spans, Carry::Text(which));
                }
            }
        }
        Carry::None => {}
    }

    while at < bytes.len() {
        let rest = &line[at..];

        if grammar
            .line_comments
            .iter()
            .any(|opener| rest.starts_with(opener))
        {
            // In languages where the comment character is also an ordinary
            // character, it only opens a comment where a word could start.
            let boundary =
                !grammar.comment_needs_boundary || at == 0 || bytes[at - 1].is_ascii_whitespace();
            if boundary {
                spans.push(span(at..line.len(), SyntaxColor::Comment));
                return (spans, Carry::None);
            }
        }

        if let Some((open, close)) = grammar.block_comment
            && rest.starts_with(open)
        {
            match line[at + open.len()..].find(close) {
                Some(found) => {
                    let end = at + open.len() + found + close.len();
                    spans.push(span(at..end, SyntaxColor::Comment));
                    at = end;
                    continue;
                }
                None => {
                    spans.push(span(at..line.len(), SyntaxColor::Comment));
                    return (spans, Carry::Comment);
                }
            }
        }

        // Ordered longest delimiter first per language, so `"""` is not read
        // as an empty `""` followed by a quote.
        if let Some((which, text)) = grammar
            .texts
            .iter()
            .enumerate()
            .find(|(_, text)| rest.starts_with(text.open))
        {
            match closing(line, at + text.open.len(), text) {
                Some(end) => {
                    spans.push(span(at..end, SyntaxColor::StringLiteral));
                    at = end;
                    continue;
                }
                None => {
                    spans.push(span(at..line.len(), SyntaxColor::StringLiteral));
                    let carry = if text.multiline {
                        Carry::Text(which as u8)
                    } else {
                        // A quote left open at the end of a line is a typo or
                        // a line that has not finished arriving, not a string
                        // that swallows the rest of the file.
                        Carry::None
                    };
                    return (spans, carry);
                }
            }
        }

        let byte = bytes[at];

        if byte.is_ascii_digit() && (at == 0 || !is_word(bytes[at - 1])) {
            let mut end = at + 1;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_' || bytes[end] == b'.')
            {
                end += 1;
            }
            spans.push(span(at..end, SyntaxColor::Number));
            at = end;
            continue;
        }

        if byte.is_ascii_alphabetic() || byte == b'_' {
            let mut end = at + 1;
            while end < bytes.len() && is_word(bytes[end]) {
                end += 1;
            }
            if grammar.keywords.contains(&&line[at..end]) {
                spans.push(span(at..end, SyntaxColor::Keyword));
            }
            at = end;
            continue;
        }

        at += 1;
        while at < bytes.len() && !line.is_char_boundary(at) {
            at += 1;
        }
    }

    (spans, Carry::None)
}

fn span(range: Range<usize>, role: SyntaxColor) -> CodeSpan {
    CodeSpan { range, role }
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Where a string that opened before `from` ends, delimiter included.
fn closing(line: &str, from: usize, text: &Text) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut at = from;
    while at < bytes.len() {
        if text.escapes && bytes[at] == b'\\' {
            at += 2;
            continue;
        }
        if line[at..].starts_with(text.close) {
            return Some(at + text.close.len());
        }
        at += 1;
        while at < bytes.len() && !line.is_char_boundary(at) {
            at += 1;
        }
    }
    None
}

/// One way a language writes a string.
struct Text {
    open: &'static str,
    close: &'static str,
    /// Whether it may run past the end of a line.
    multiline: bool,
    /// Whether a backslash hides the next character from the closer.
    escapes: bool,
}

/// What one language looks like, as far as this can tell.
struct Grammar {
    line_comments: &'static [&'static str],
    /// Whether a line comment only opens at the start of a word, which is what
    /// keeps `#` in the middle of a string or an identifier from swallowing a
    /// line.
    comment_needs_boundary: bool,
    block_comment: Option<(&'static str, &'static str)>,
    texts: &'static [Text],
    keywords: &'static [&'static str],
}

const DOUBLE: Text = Text {
    open: "\"",
    close: "\"",
    multiline: false,
    escapes: true,
};
const SINGLE: Text = Text {
    open: "'",
    close: "'",
    multiline: false,
    escapes: true,
};

fn grammar(language: Language) -> &'static Grammar {
    match language {
        Language::Rust => &Grammar {
            line_comments: &["//"],
            comment_needs_boundary: false,
            block_comment: Some(("/*", "*/")),
            texts: &[DOUBLE, SINGLE],
            keywords: &[
                "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
                "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
                "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct",
                "super", "trait", "true", "type", "unsafe", "use", "where", "while",
            ],
        },
        Language::TypeScript => &Grammar {
            line_comments: &["//"],
            comment_needs_boundary: false,
            block_comment: Some(("/*", "*/")),
            texts: &[
                Text {
                    open: "`",
                    close: "`",
                    multiline: true,
                    escapes: true,
                },
                DOUBLE,
                SINGLE,
            ],
            keywords: &[
                "abstract",
                "any",
                "as",
                "async",
                "await",
                "boolean",
                "break",
                "case",
                "catch",
                "class",
                "const",
                "continue",
                "default",
                "delete",
                "do",
                "else",
                "enum",
                "export",
                "extends",
                "false",
                "finally",
                "for",
                "from",
                "function",
                "if",
                "implements",
                "import",
                "in",
                "instanceof",
                "interface",
                "let",
                "new",
                "null",
                "number",
                "of",
                "private",
                "protected",
                "public",
                "readonly",
                "return",
                "static",
                "string",
                "super",
                "switch",
                "this",
                "throw",
                "true",
                "try",
                "type",
                "typeof",
                "undefined",
                "var",
                "void",
                "while",
                "yield",
            ],
        },
        Language::Python => &Grammar {
            line_comments: &["#"],
            comment_needs_boundary: true,
            block_comment: None,
            texts: &[
                Text {
                    open: "\"\"\"",
                    close: "\"\"\"",
                    multiline: true,
                    escapes: true,
                },
                Text {
                    open: "'''",
                    close: "'''",
                    multiline: true,
                    escapes: true,
                },
                DOUBLE,
                SINGLE,
            ],
            keywords: &[
                "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
                "continue", "def", "del", "elif", "else", "except", "finally", "for", "from",
                "global", "if", "import", "in", "is", "lambda", "match", "nonlocal", "not", "or",
                "pass", "raise", "return", "try", "while", "with", "yield",
            ],
        },
        Language::Go => &Grammar {
            line_comments: &["//"],
            comment_needs_boundary: false,
            block_comment: Some(("/*", "*/")),
            texts: &[
                Text {
                    open: "`",
                    close: "`",
                    multiline: true,
                    escapes: false,
                },
                DOUBLE,
                SINGLE,
            ],
            keywords: &[
                "break",
                "case",
                "chan",
                "const",
                "continue",
                "default",
                "defer",
                "else",
                "fallthrough",
                "false",
                "for",
                "func",
                "go",
                "goto",
                "if",
                "import",
                "interface",
                "map",
                "nil",
                "package",
                "range",
                "return",
                "select",
                "struct",
                "switch",
                "true",
                "type",
                "var",
            ],
        },
        Language::Json => &Grammar {
            line_comments: &[],
            comment_needs_boundary: false,
            block_comment: None,
            texts: &[DOUBLE],
            keywords: &["true", "false", "null"],
        },
        Language::Shell => &Grammar {
            line_comments: &["#"],
            comment_needs_boundary: true,
            block_comment: None,
            texts: &[
                DOUBLE,
                Text {
                    open: "'",
                    close: "'",
                    multiline: false,
                    // A shell's single quotes take everything literally,
                    // including a backslash.
                    escapes: false,
                },
            ],
            keywords: &[
                "case", "do", "done", "elif", "else", "esac", "exit", "export", "fi", "for",
                "function", "if", "in", "local", "return", "select", "then", "until", "while",
            ],
        },
        Language::Toml => &Grammar {
            line_comments: &["#"],
            comment_needs_boundary: true,
            block_comment: None,
            texts: &[
                Text {
                    open: "\"\"\"",
                    close: "\"\"\"",
                    multiline: true,
                    escapes: true,
                },
                DOUBLE,
                Text {
                    open: "'",
                    close: "'",
                    multiline: false,
                    escapes: false,
                },
            ],
            keywords: &["true", "false"],
        },
        Language::Yaml => &Grammar {
            line_comments: &["#"],
            comment_needs_boundary: true,
            block_comment: None,
            texts: &[
                DOUBLE,
                Text {
                    open: "'",
                    close: "'",
                    multiline: false,
                    escapes: false,
                },
            ],
            keywords: &[
                "false", "False", "FALSE", "true", "True", "TRUE", "null", "Null", "NULL", "yes",
                "Yes", "YES", "no", "No", "NO", "on", "On", "ON", "off", "Off", "OFF",
            ],
        },
        Language::Markdown => &Grammar {
            line_comments: &[],
            comment_needs_boundary: false,
            block_comment: None,
            texts: &[Text {
                open: "`",
                close: "`",
                multiline: false,
                escapes: false,
            }],
            keywords: &[],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(language: Language, line: &str) -> Vec<(String, SyntaxColor)> {
        line_spans(language, line, Carry::None)
            .0
            .into_iter()
            .map(|span| (line[span.range.clone()].to_string(), span.role))
            .collect()
    }

    #[test]
    fn a_language_is_read_from_what_the_fence_says() {
        assert_eq!(Language::named("rs"), Some(Language::Rust));
        assert_eq!(Language::named("TypeScript"), Some(Language::TypeScript));
        assert_eq!(Language::named(" python "), Some(Language::Python));
        assert_eq!(Language::named("yml"), Some(Language::Yaml));
        assert_eq!(
            Language::named("brainfuck"),
            None,
            "a language with no table is left plain rather than guessed at"
        );
    }

    #[test]
    fn every_default_language_requested_by_markdown_has_a_table() {
        for tag in [
            "json",
            "rust",
            "rs",
            "typescript",
            "ts",
            "shell",
            "bash",
            "toml",
            "yaml",
            "yml",
        ] {
            assert!(
                Language::named(tag).is_some(),
                "missing built-in table for {tag}"
            );
        }
    }

    #[test]
    fn yaml_scalars_and_comments_use_the_shared_palette_roles() {
        let read = read(Language::Yaml, "enabled: true # host policy");
        assert!(read.contains(&("true".into(), SyntaxColor::Keyword)));
        assert!(
            read.iter().any(|(text, role)| {
                *role == SyntaxColor::Comment && text.contains("host policy")
            })
        );
    }

    #[test]
    fn the_four_classes_are_found_where_they_are() {
        let read = read(Language::Rust, r#"let x = 42; // the "answer""#);
        assert!(read.contains(&("let".into(), SyntaxColor::Keyword)));
        assert!(read.contains(&("42".into(), SyntaxColor::Number)));
        assert!(
            read.iter()
                .any(|(text, role)| *role == SyntaxColor::Comment && text.contains("answer")),
            "a quote inside a comment is part of the comment"
        );
    }

    #[test]
    fn a_keyword_inside_a_word_is_not_a_keyword() {
        let read = read(Language::Rust, "letter selfish iffy");
        assert!(read.is_empty(), "found {read:?}");
    }

    #[test]
    fn an_escaped_quote_does_not_end_a_string() {
        let read = read(Language::Rust, r#"let s = "a \" b"; let n = 1;"#);
        let strings: Vec<&String> = read
            .iter()
            .filter(|(_, role)| *role == SyntaxColor::StringLiteral)
            .map(|(text, _)| text)
            .collect();
        assert_eq!(strings, vec![r#""a \" b""#]);
        assert!(read.contains(&("1".into(), SyntaxColor::Number)));
    }

    #[test]
    fn a_comment_that_crosses_lines_stays_a_comment() {
        let code = "let a = 1;\n/* still\ngoing\n*/ let b = 2;";
        let lines = line_spans_of(Language::Rust, code);
        assert_eq!(lines.len(), 4);
        assert!(
            lines[2]
                .iter()
                .all(|span| span.role == SyntaxColor::Comment),
            "a line entirely inside a comment is entirely a comment"
        );
        assert!(
            lines[3]
                .iter()
                .any(|span| span.role == SyntaxColor::Keyword),
            "and the code after it is code again"
        );
    }

    #[test]
    fn a_quote_left_open_does_not_swallow_the_rest_of_the_file() {
        // A line that has not finished arriving, or a typo. Either way the
        // next line is not part of a string.
        let lines = line_spans_of(Language::Rust, "let a = \"unclosed\nlet b = 2;");
        assert!(
            lines[1]
                .iter()
                .any(|span| span.role == SyntaxColor::Keyword),
            "the following line reads normally"
        );
    }

    #[test]
    fn a_hash_inside_a_word_is_not_a_comment() {
        let inside = read(Language::Python, "value = colour#tag");
        assert!(
            !inside.iter().any(|(_, role)| *role == SyntaxColor::Comment),
            "found {inside:?}"
        );
        let after_space = read(Language::Python, "value = 1  # a real comment");
        assert!(
            after_space
                .iter()
                .any(|(_, role)| *role == SyntaxColor::Comment)
        );
    }

    #[test]
    fn a_triple_quote_is_not_two_quotes_and_one_more() {
        let lines = line_spans_of(Language::Python, "doc = \"\"\"first\nsecond\"\"\"\nx = 1");
        assert!(
            lines[1]
                .iter()
                .any(|span| span.role == SyntaxColor::StringLiteral),
            "the middle of a triple-quoted string is string"
        );
        assert!(
            lines[2].iter().any(|span| span.role == SyntaxColor::Number),
            "and the code after it is code"
        );
    }

    #[test]
    fn a_blocks_spans_are_offsets_into_the_block() {
        let code = "fn main() {\n    let x = 1;\n}";
        let found = spans(Language::Rust, code);
        let read: Vec<&str> = found.iter().map(|span| &code[span.range.clone()]).collect();
        assert_eq!(read, vec!["fn", "let", "1"]);
        assert!(
            found
                .windows(2)
                .all(|pair| pair[0].range.end <= pair[1].range.start),
            "sorted and non-overlapping, which is what the renderer requires"
        );
    }

    #[test]
    fn colouring_the_same_block_twice_scans_it_once() {
        let mut cache = Cache::default();
        let code = SharedString::from("let x = 1;");
        let first = cache.block(Language::Rust, &code);
        let again = cache.block(Language::Rust, &code);
        assert!(Rc::ptr_eq(&first, &again));

        let grown = SharedString::from("let x = 1;\nlet y = 2;");
        let after = cache.block(Language::Rust, &grown);
        assert!(
            !Rc::ptr_eq(&first, &after),
            "a block that grew is coloured again"
        );
        assert_eq!(after.len(), 4, "both lines' keywords and numbers");
    }

    #[test]
    fn every_span_falls_on_a_character_boundary() {
        // The renderer slices the text with these, so a span landing inside a
        // character would panic on text nobody thought was unusual.
        let code = "let emoji = \"héllo 🌍\"; // café\nlet n = 1;";
        for span in spans(Language::Rust, code) {
            assert!(code.is_char_boundary(span.range.start), "{span:?}");
            assert!(code.is_char_boundary(span.range.end), "{span:?}");
        }
    }
}
