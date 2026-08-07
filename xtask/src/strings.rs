//! The gate that keeps English out of the components.
//!
//! `gpui-kit` puts every word it shows behind
//! [`StringKey`](gpui_kit::strings::StringKey), so a host can replace any of
//! them. That only stays true if a literal cannot creep back in, and a
//! reviewer cannot be expected to notice one `.label("Copy")` in a diff of a
//! thousand lines.
//!
//! # How it decides
//!
//! It reads every component source, drops comments and everything from the
//! first `#[cfg(test)]`, and pulls out the string literals that are left. A
//! literal is *suspect* when either is true:
//!
//! 1. it reads like a sentence — it has a letter, and either a space or a
//!    capital;
//! 2. it is the direct argument of a call that puts text on a screen, such as
//!    `label`, `placeholder`, `text`, or `SharedString::new_static`.
//!
//! Rule 1 catches `"Show {n} more lines"`; rule 2 catches `.label("copy")`,
//! which rule 1 would read as an id.
//!
//! Every suspect has to be listed in `docs/strings-allowlist.txt`. The
//! allowlist is an explicit file rather than a cleverer rule because the
//! remaining literals are not one shape: they are `Debug` names, panic
//! messages, `Display` implementations for host-facing errors, published
//! semantic state names, and id seeds. A heuristic that told those apart from
//! a label would be wrong often enough to be switched off, and a gate nobody
//! reads is worse than no gate. An entry is a line a reviewer has to add on
//! purpose, and the check also reports entries nothing matches any more, so
//! the file cannot quietly rot into a blanket permission.
//!
//! # What it gets wrong
//!
//! One shape, on purpose: a CamelCase identifier. `debug_struct("Button")` and
//! `key_context("TextInput")` are a capital with no space, which is exactly
//! the shape of a one-word label, and no rule can tell them apart without
//! knowing what `Button` means. They sit in the allowlist. That is a bounded
//! cost — a new component adds one line once — and it buys the rule that
//! catches `.label("Save")`, which is the failure this exists to prevent.
//!
//! It does not fire on ids, token keys, or element ids: those are lower case
//! with dots or dashes, which rule 1 ignores and rule 2 never sees, because
//! `Ident::child` is not a call that shows anything.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

/// Sources whose literals are not component copy.
///
/// - `strings.rs` is the catalogue: the English lives there by definition.
/// - `scenes.rs` is fixture content for the gallery and the capture task.
/// - `datetime/fixture.rs` is a stand-in [`DateAdapter`] host. This crate owns
///   no calendar, so month and weekday names come from the host adapter; the
///   fixture is what a host would supply, not what a component says.
const EXEMPT: &[&str] = &["strings.rs", "scenes.rs", "datetime/fixture.rs"];

/// Calls whose first argument is read by somebody.
const SHOWN: &[&str] = &[
    "label",
    "placeholder",
    "detail",
    "text",
    "title",
    "trigger_name",
    "refusal",
    "noun",
    "SharedString::new_static",
    "SharedString::from",
];

pub fn check(root: &Path) -> Result<()> {
    let allowlist_path = root.join("docs").join("strings-allowlist.txt");
    let allowed = read_allowlist(&allowlist_path)?;
    let found = scan(&root.join("crates").join("gpui-kit").join("src"))?;

    let mut unlisted: Vec<&Suspect> = found
        .iter()
        .filter(|suspect| !allowed.contains(&suspect.text))
        .collect();
    unlisted.sort_by(|left, right| {
        (&left.file, left.line, &left.text).cmp(&(&right.file, right.line, &right.text))
    });

    let seen: BTreeSet<&str> = found.iter().map(|suspect| suspect.text.as_str()).collect();
    let stale: Vec<&String> = allowed
        .iter()
        .filter(|text| !seen.contains(text.as_str()))
        .collect();

    if unlisted.is_empty() && stale.is_empty() {
        println!("{} literal(s) accounted for", found.len());
        return Ok(());
    }

    for suspect in &unlisted {
        println!(
            "unlisted {}:{} {:?}",
            suspect.file, suspect.line, suspect.text
        );
    }
    for text in &stale {
        println!("stale    {text:?}");
    }
    bail!(
        "{} unlisted and {} stale literal(s). A word a reader reads belongs in \
         `StringKey`, not in a component. If this one is a Debug name, a panic \
         message, a published state, or an id seed, add it to {} on purpose.",
        unlisted.len(),
        stale.len(),
        allowlist_path.display()
    );
}

/// Rewrites the allowlist from what is in the tree, for the one case where
/// that is honest: a reviewer who has just read every entry.
pub fn generate(root: &Path) -> Result<()> {
    let path = root.join("docs").join("strings-allowlist.txt");
    let found = scan(&root.join("crates").join("gpui-kit").join("src"))?;
    let texts: BTreeSet<String> = found.into_iter().map(|suspect| suspect.text).collect();
    let mut out = String::from(HEADER);
    for text in &texts {
        out.push_str(text);
        out.push('\n');
    }
    fs::write(&path, out)?;
    println!("wrote {} entries to {}", texts.len(), path.display());
    Ok(())
}

const HEADER: &str = "\
# Literals in `crates/gpui-kit/src` that a reader never reads.
#
# Generated by `cargo run -p xtask -- strings generate` and checked by
# `cargo run -p xtask -- strings check`, which runs inside `gate`.
#
# Every line is one exact literal. A literal that is not here and looks like
# copy fails the gate. Adding a line is a claim that nobody reads that string:
# it is a `Debug` name, a panic message, a `Display` implementation for an
# error the host renders itself, a state name published in the semantic tree,
# or a seed for a stable id. Anything a reader reads belongs in
# `gpui_kit::strings::StringKey` instead.
#
# Blank lines and lines starting with `#` are ignored; a literal that begins
# with `#` cannot be listed, which no literal here does.
";

fn read_allowlist(path: &Path) -> Result<BTreeSet<String>> {
    let text = fs::read_to_string(path)?;
    Ok(text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect())
}

#[derive(Debug, PartialEq, Eq)]
pub struct Suspect {
    pub file: String,
    pub line: usize,
    pub text: String,
}

fn scan(directory: &Path) -> Result<Vec<Suspect>> {
    let mut sources = Vec::new();
    collect(directory, &mut sources)?;
    sources.sort();

    let mut found = Vec::new();
    for path in sources {
        let relative = path
            .strip_prefix(directory)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if EXEMPT.iter().any(|exempt| relative == *exempt) {
            continue;
        }
        let source = fs::read_to_string(&path)?;
        found.extend(suspects(&source).into_iter().map(|(line, text)| Suspect {
            file: relative.clone(),
            line,
            text,
        }));
    }
    Ok(found)
}

fn collect(directory: &Path, into: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, into)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            into.push(path);
        }
    }
    Ok(())
}

/// Every suspect literal in one source, as (line, text).
pub fn suspects(source: &str) -> Vec<(usize, String)> {
    let source = match source.find("#[cfg(test)]") {
        Some(at) => &source[..at],
        None => source,
    };
    literals(source)
        .into_iter()
        .filter(|literal| is_prose(&literal.text) || SHOWN.contains(&literal.call.as_str()))
        .map(|literal| (literal.line, literal.text))
        .collect()
}

/// A sentence rather than an identifier: it has a letter, and either a space
/// or a capital. `"settings.save"` is an id; `"Save"` and `"more rows"` are
/// not.
fn is_prose(text: &str) -> bool {
    text.chars()
        .any(|character| character.is_ascii_alphabetic())
        && (text.contains(' ') || text.chars().any(|character| character.is_ascii_uppercase()))
}

struct Literal {
    line: usize,
    text: String,
    /// The identifier immediately before the opening quote's `(`, if any.
    call: String,
}

/// Pulls the normal and raw string literals out of Rust source, skipping
/// comments and char literals, and joining `\`-newline continuations the way
/// the compiler does.
fn literals(source: &str) -> Vec<Literal> {
    let bytes: Vec<char> = source.chars().collect();
    let mut found = Vec::new();
    let mut index = 0;
    let mut line = 1;

    while index < bytes.len() {
        let character = bytes[index];
        if character == '\n' {
            line += 1;
            index += 1;
            continue;
        }
        if character == '/' && bytes.get(index + 1) == Some(&'/') {
            while index < bytes.len() && bytes[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if character == '/' && bytes.get(index + 1) == Some(&'*') {
            index += 2;
            while index < bytes.len()
                && !(bytes[index] == '*' && bytes.get(index + 1) == Some(&'/'))
            {
                if bytes[index] == '\n' {
                    line += 1;
                }
                index += 1;
            }
            index = (index + 2).min(bytes.len());
            continue;
        }
        // A lifetime or a char literal, neither of which is a string.
        if character == '\'' {
            index += 1;
            continue;
        }
        if character == 'r'
            && matches!(bytes.get(index + 1), Some('"') | Some('#'))
            && !index
                .checked_sub(1)
                .is_some_and(|before| is_word(bytes[before]))
        {
            let start = index;
            let mut hashes = 0;
            let mut cursor = index + 1;
            while bytes.get(cursor) == Some(&'#') {
                hashes += 1;
                cursor += 1;
            }
            if bytes.get(cursor) == Some(&'"') {
                cursor += 1;
                let open = cursor;
                let mut text = String::new();
                while cursor < bytes.len() {
                    if bytes[cursor] == '"'
                        && (0..hashes).all(|step| bytes.get(cursor + 1 + step) == Some(&'#'))
                    {
                        break;
                    }
                    if bytes[cursor] == '\n' {
                        line += 1;
                    }
                    text.push(bytes[cursor]);
                    cursor += 1;
                }
                let _ = open;
                found.push(Literal {
                    line,
                    text,
                    call: preceding_call(&bytes, start),
                });
                index = cursor + 1 + hashes;
                continue;
            }
        }
        if character == '"' {
            let start = index;
            let opened = line;
            let mut cursor = index + 1;
            let mut text = String::new();
            while cursor < bytes.len() && bytes[cursor] != '"' {
                if bytes[cursor] == '\\' {
                    match bytes.get(cursor + 1) {
                        // A backslash at end of line eats the newline and the
                        // indentation after it, so a wrapped sentence is one
                        // string with single spaces, not two fragments.
                        Some('\n') => {
                            line += 1;
                            cursor += 2;
                            while matches!(bytes.get(cursor), Some(' ') | Some('\t')) {
                                cursor += 1;
                            }
                        }
                        Some(escaped) => {
                            text.push(match escaped {
                                'n' => '\n',
                                't' => '\t',
                                other => *other,
                            });
                            cursor += 2;
                        }
                        None => cursor += 1,
                    }
                    continue;
                }
                if bytes[cursor] == '\n' {
                    line += 1;
                }
                text.push(bytes[cursor]);
                cursor += 1;
            }
            found.push(Literal {
                line: opened,
                text,
                call: preceding_call(&bytes, start),
            });
            index = cursor + 1;
            continue;
        }
        index += 1;
    }
    found
}

fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character == ':' || character == '!'
}

/// The name of the call whose argument list opens immediately before `at`.
fn preceding_call(bytes: &[char], at: usize) -> String {
    let mut cursor = at;
    while cursor > 0 && bytes[cursor - 1].is_whitespace() {
        cursor -= 1;
    }
    if cursor == 0 || bytes[cursor - 1] != '(' {
        return String::new();
    }
    cursor -= 1;
    let end = cursor;
    while cursor > 0 && is_word(bytes[cursor - 1]) {
        cursor -= 1;
    }
    bytes[cursor..end]
        .iter()
        .collect::<String>()
        .trim_start_matches('.')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_added_to_a_component_is_caught() {
        let source = r#"
            fn render(self, cx: &mut App) -> impl IntoElement {
                Button::new(self.ident.child("copy")).label("Copy")
            }
        "#;
        let found = suspects(source);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].1, "Copy");
    }

    #[test]
    fn a_lowercase_label_is_caught_by_where_it_is_used() {
        // Rule 1 would read this as an id, which is exactly why rule 2 exists.
        let found = suspects(r#"fn f() { div().label("copy"); }"#);
        assert_eq!(found, vec![(1, "copy".to_string())]);
    }

    #[test]
    fn ids_and_token_keys_do_not_cry() {
        let source = r#"
            fn render(&self, cx: &mut App) -> impl IntoElement {
                let ident = self.ident.child("select-all");
                div().id(ident.element_id()).key_context("editor")
            }
        "#;
        assert!(suspects(source).is_empty(), "{:?}", suspects(source));
    }

    #[test]
    fn a_camel_case_name_is_the_one_thing_that_cries_wolf() {
        // `debug_struct("Button")` and `key_context("TextInput")` are capitals
        // with no space, which is the same shape as a one-word label, so rule
        // 1 cannot tell them apart. They are absorbed by the allowlist rather
        // than by a rule that would have to know what `Button` means. This
        // test exists so the behaviour is a decision on record and not a
        // surprise.
        let found = suspects(r#"fn f() { formatter.debug_struct("Button"); }"#);
        assert_eq!(found, vec![(1, "Button".to_string())]);
    }

    #[test]
    fn a_sentence_reaches_the_gate_however_it_is_built() {
        let found = suspects(r#"fn f() { let x = format!("Show {n} more lines"); }"#);
        assert_eq!(found, vec![(1, "Show {n} more lines".to_string())]);
    }

    #[test]
    fn a_wrapped_sentence_is_one_literal() {
        let source = "fn f() {\n    let x = \"the host cannot list the days, so none \\\n         were checked\";\n}";
        let found = suspects(source);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].1,
            "the host cannot list the days, so none were checked"
        );
        assert_eq!(found[0].0, 2, "it is reported where it opens");
    }

    #[test]
    fn comments_and_doc_comments_are_not_code() {
        let source = "//! A component that says \"Save\".\n/// Draws \"Cancel\".\nfn f() {}\n";
        assert!(suspects(source).is_empty());
    }

    #[test]
    fn a_test_module_is_not_a_component() {
        let source =
            "fn f() {}\n#[cfg(test)]\nmod tests {\n    const NAME: &str = \"Save changes\";\n}\n";
        assert!(suspects(source).is_empty());
    }

    #[test]
    fn the_allowlist_covers_the_tree() {
        // The real gate, run as a unit test so a failure names the file that
        // regressed rather than a step in a script.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask sits under the workspace root");
        check(root).expect("every literal in the tree is accounted for");
    }
}
