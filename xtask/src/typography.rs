//! The gate that prevents component text from inheriting a host font.
//!
//! A `SharedString` is an element in GPUI, so `.child(label)` compiles even
//! when the element that emits it never selected a type step. That makes the
//! component depend on whichever font happened to be above it. Visible text in
//! `gpui-kit` instead enters through `foundation::text`, which applies a whole
//! `TypeScale` step and the primary `TextTone`; a caller can then override the
//! tone or logical alignment without separating size, line height, and weight.

use std::fs;
use std::path::Path;

use anyhow::{Result, bail};

use crate::strings;

const EXEMPT: &[&str] = &["foundation/styled_ext.rs"];

#[derive(Debug, PartialEq, Eq)]
struct Violation {
    line: usize,
    expression: String,
}

pub fn check(root: &Path) -> Result<()> {
    let directory = root.join("crates").join("gpui-kit").join("src");
    let mut sources = Vec::new();
    strings::collect(&directory, &mut sources)?;
    sources.sort();

    let mut found = Vec::new();
    for path in sources {
        let relative = path
            .strip_prefix(&directory)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if EXEMPT.contains(&relative.as_str()) {
            continue;
        }
        let source = fs::read_to_string(&path)?;
        for violation in violations(&source) {
            found.push((relative.clone(), violation));
        }
    }

    if found.is_empty() {
        println!("component text has an explicit type step");
        return Ok(());
    }

    for (file, violation) in &found {
        println!(
            "bare text child {}:{} {}",
            file, violation.line, violation.expression
        );
    }
    bail!(
        "{} bare text child expression(s); wrap visible strings with \
         `foundation::text(theme, TypeScale, content)` so an embedded component \
         cannot inherit its host's font",
        found.len()
    )
}

/// Finds direct string-producing arguments to `.child(...)`.
///
/// The strings gate already owns source traversal and the prose/id distinction;
/// this scanner reuses both and adds only the call-site question it needs. A
/// lower-case id seed such as `ident.child("row")` is not prose. A direct
/// `SharedString` or `format!` constructor is always text and cannot be an id
/// by accident without first being converted into an `Ident` argument.
fn violations(source: &str) -> Vec<Violation> {
    let source = match source.find("#[cfg(test)]") {
        Some(at) => &source[..at],
        None => source,
    };
    let bytes = source.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    let mut line = 1;

    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                line += 1;
                index += 1;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index < bytes.len()
                    && !(bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/'))
                {
                    if bytes[index] == b'\n' {
                        line += 1;
                    }
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
            }
            b'"' => skip_quoted(bytes, &mut index, &mut line),
            b'r' if raw_string_hashes(bytes, index).is_some() => {
                skip_raw(bytes, &mut index, &mut line)
            }
            b'.' if source[index..].starts_with(".child") => {
                let opened = line;
                let mut cursor = index + ".child".len();
                skip_space(bytes, &mut cursor);
                if bytes.get(cursor) != Some(&b'(') {
                    index += 1;
                    continue;
                }
                cursor += 1;
                skip_space(bytes, &mut cursor);
                if let Some((expression, prose)) = child_expression(source, bytes, cursor)
                    && prose
                {
                    found.push(Violation {
                        line: opened,
                        expression,
                    });
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    found
}

fn child_expression(source: &str, bytes: &[u8], at: usize) -> Option<(String, bool)> {
    if bytes.get(at) == Some(&b'"') {
        let end = quoted_end(bytes, at);
        let text = source.get(at + 1..end.saturating_sub(1))?;
        return Some((source.get(at..end)?.to_string(), strings::is_prose(text)));
    }
    if let Some(hashes) = raw_string_hashes(bytes, at) {
        let quote = at + 1 + hashes;
        let end = raw_end(bytes, quote + 1, hashes);
        let text_end = end.saturating_sub(1 + hashes);
        let text = source.get(quote + 1..text_end)?;
        return Some((source.get(at..end)?.to_string(), strings::is_prose(text)));
    }
    for constructor in ["SharedString::", "format!"] {
        if source[at..].starts_with(constructor) {
            return Some((constructor.to_string(), true));
        }
    }
    None
}

fn skip_space(bytes: &[u8], cursor: &mut usize) {
    while bytes.get(*cursor).is_some_and(u8::is_ascii_whitespace) {
        *cursor += 1;
    }
}

fn skip_quoted(bytes: &[u8], index: &mut usize, line: &mut usize) {
    *index = quoted_end_counting(bytes, *index, line);
}

fn quoted_end(bytes: &[u8], at: usize) -> usize {
    let mut ignored_line = 1;
    quoted_end_counting(bytes, at, &mut ignored_line)
}

fn quoted_end_counting(bytes: &[u8], at: usize, line: &mut usize) -> usize {
    let mut cursor = at + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor = (cursor + 2).min(bytes.len()),
            b'"' => return cursor + 1,
            b'\n' => {
                *line += 1;
                cursor += 1;
            }
            _ => cursor += 1,
        }
    }
    cursor
}

fn raw_string_hashes(bytes: &[u8], at: usize) -> Option<usize> {
    if bytes.get(at) != Some(&b'r') {
        return None;
    }
    let mut cursor = at + 1;
    while bytes.get(cursor) == Some(&b'#') {
        cursor += 1;
    }
    (bytes.get(cursor) == Some(&b'"')).then_some(cursor - at - 1)
}

fn raw_end(bytes: &[u8], mut cursor: usize, hashes: usize) -> usize {
    while cursor < bytes.len() {
        if bytes[cursor] == b'"'
            && (0..hashes).all(|step| bytes.get(cursor + 1 + step) == Some(&b'#'))
        {
            return (cursor + 1 + hashes).min(bytes.len());
        }
        cursor += 1;
    }
    cursor
}

fn skip_raw(bytes: &[u8], index: &mut usize, line: &mut usize) {
    let hashes = raw_string_hashes(bytes, *index).expect("caller checked raw string");
    let quote = *index + 1 + hashes;
    let end = raw_end(bytes, quote + 1, hashes);
    *line += bytes[*index..end]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count();
    *index = end;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_literal_and_shared_string_children_are_caught() {
        let source = r#"
            fn render() {
                div().child("Save changes");
                div().child(SharedString::from("Save"));
                div().child(format!("{count} rows"));
            }
        "#;
        let found = violations(source);
        assert_eq!(found.len(), 3, "{found:?}");
    }

    #[test]
    fn ids_comments_tests_and_the_text_entry_are_not_caught() {
        let source = r#"
            fn render(theme: &Theme) {
                let row = ident.child("row");
                // div().child("Not code");
                div().child(text(theme, TypeScale::Body, "Visible"));
            }
            #[cfg(test)]
            mod tests { fn fixture() { div().child("Fixture copy"); } }
        "#;
        assert!(violations(source).is_empty(), "{:?}", violations(source));
    }

    #[test]
    fn raw_visible_text_is_caught_but_a_raw_id_is_not() {
        let source = r##"fn f() { div().child(r#"Visible text"#); ident.child(r#"row"#); }"##;
        assert_eq!(violations(source).len(), 1);
    }
}
