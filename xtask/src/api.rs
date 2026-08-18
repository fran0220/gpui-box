//! The index that stops a reader from inventing an API.
//!
//! `docs/components.md` describes the components in prose, which is what a
//! person wants and what a program cannot use. An agent writing against this
//! library fails in one particular way: it guesses `Badge::new("Ready")
//! .tone(Tone::Success)` when the builder is `.success()`, and it guesses
//! because nothing it can read says otherwise. Prose also drifts, because
//! nothing fails when a signature changes and a sentence does not.
//!
//! So this generates `docs/api-index.json` from the source and `gate` fails
//! when the file no longer matches the tree, the same arrangement as
//! `token-reference.md` and `strings-allowlist.txt`. A signature in the index
//! is one a compiler agreed to.
//!
//! # How it decides
//!
//! It reads every source under `crates/gpui-kit/src`, drops comments and
//! everything from the first `#[cfg(test)]`, and then:
//!
//! - a `pub struct` that derives `IntoElement` is a **builder**, and one that
//!   something implements `Render` for is a **view**. Those are the two shapes
//!   a caller mounts, and they are exactly the distinction that decides
//!   whether the caller needs an `Entity`;
//! - the `pub fn`s in its inherent `impl` are sorted by their receiver, which
//!   is what a caller actually needs to know: no receiver is a constructor,
//!   `self` chains, `&mut self` is a command that needs a `Context`, and
//!   `&self` only answers;
//! - a `pub enum` named `<Component>Event` is what the component **reports**,
//!   so the variants are listed against the component rather than adrift;
//! - every other `pub struct` or `pub enum` in a component source is a
//!   supporting type, listed separately because a signature mentions it.
//!
//! Scenes come from the registry in `gpui_kit::scenes::catalog()`, which
//! declares what each rendering is for, and their bodies are read out of
//! `crates/gpui-kit/src/scenes/`. A component therefore carries the scenes
//! that are the review of it, and each scene carries its own body as an
//! example. That example is worth more than a written one because `gate`
//! compiles it and `headless check` renders it, so an example here cannot be
//! stale without a gate going red.
//!
//! # What it gets wrong
//!
//! It matches text, not syntax, so it believes what the source looks like.
//! Two known consequences: a type aliased or re-exported under another name is
//! indexed under the name it was declared with, and a builder assembled by a
//! macro rather than an `impl` block would be missed entirely. Neither shape
//! is in the tree, and a component that grows one will be absent from the
//! index rather than wrong in it — which is the failure worth having, because
//! an agent that cannot find a component asks, and one that reads a wrong
//! signature does not.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use pulldown_cmark::{Event, Options, Parser};

/// Sources that declare no component.
const SKIP: &[&str] = &["lib.rs"];

/// The scene catalog, which renders components rather than declaring them.
const SCENES: &str = "crates/gpui-kit/src/scenes/";

pub fn generate(root: &Path) -> Result<()> {
    let index = build(root)?;
    let path = index_path(root);
    fs::write(&path, &index)?;
    println!("wrote {} to {}", size(&index), path.display());
    Ok(())
}

pub fn check(root: &Path) -> Result<()> {
    let path = index_path(root);
    let current = fs::read_to_string(&path).unwrap_or_default();
    if same_index(&current, &build(root)?) {
        println!("{} is current", path.display());
        return Ok(());
    }
    bail!(
        "{} is stale. Run `cargo run -p xtask -- api generate`. An agent reads \
         this file to find out what exists and what it is called, so a stale \
         entry is a signature somebody will be told to write and the compiler \
         will reject.",
        path.display()
    );
}

fn same_index(current: &str, expected: &str) -> bool {
    // Git commonly materializes tracked text with CRLF when core.autocrlf is
    // enabled. The generated string is LF, but those files have identical
    // logical contents and must pass the same cross-platform gate.
    current.replace("\r\n", "\n") == expected
}

fn index_path(root: &Path) -> PathBuf {
    root.join("docs").join("api-index.json")
}

fn size(index: &str) -> String {
    format!("{} line(s)", index.lines().count())
}

// ---------------------------------------------------------------------------
// The index
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct Item {
    name: String,
    module: String,
    source: String,
    summary: String,
    kind: Kind,
    variants: Vec<String>,
    constructors: Vec<String>,
    options: Vec<String>,
    commands: Vec<String>,
    queries: Vec<String>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
enum Kind {
    Builder,
    View,
    #[default]
    Type,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Builder => "builder",
            Self::View => "view",
            Self::Type => "type",
        }
    }
}

#[derive(Debug)]
struct SceneRecord {
    name: String,
    /// `"exhibit"` when the scene is the review of its components,
    /// `"composition"` when it arranges components reviewed elsewhere.
    kind: &'static str,
    /// The components the scene is *about*, empty for a composition.
    subjects: Vec<String>,
    /// Every component the scene names, whichever kind it is.
    uses: Vec<String>,
    example: String,
}

fn build(root: &Path) -> Result<String> {
    let source_root = root.join("crates").join("gpui-kit").join("src");
    let mut items: BTreeMap<String, Item> = BTreeMap::new();
    let mut events: BTreeMap<String, Vec<String>> = BTreeMap::new();

    let mut files = Vec::new();
    collect(&source_root, &mut files)?;
    files.sort();

    for file in &files {
        let relative = file
            .strip_prefix(root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        if SKIP.iter().any(|skip| relative.ends_with(skip)) || relative.starts_with(SCENES) {
            continue;
        }
        let module = module_of(&relative);
        let source = strip(&fs::read_to_string(file)?);
        read_source(&source, &module, &relative, &mut items, &mut events);
    }

    let scenes = read_scenes(&scene_source(&source_root)?, &items, root)?;

    // `scenes` on a component answers "where do I go to look at this", so it
    // lists the scenes the component is the subject of. A composition draws it
    // beside a dozen other things and is nobody's review, which is why being
    // in one is not being covered by one.
    let mut used: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for scene in &scenes {
        for name in &scene.subjects {
            used.entry(name.as_str()).or_default().push(&scene.name);
        }
    }

    let uncovered: Vec<&str> = items
        .values()
        .filter(|item| item.kind != Kind::Type && !item.name.is_empty())
        .map(|item| item.name.as_str())
        .filter(|name| !used.contains_key(name))
        .collect();
    if !uncovered.is_empty() {
        bail!(
            "no scene is the review of {}. A component that only ever appears \
inside a composition has been recognised, not reviewed: nobody has seen its \
states side by side, and no captured image will fail when they change. Give it \
a scene that declares it in `Shows::Subjects`.",
            uncovered.join(", ")
        );
    }

    Ok(render(&items, &events, &used, &scenes))
}

/// The scene catalog as one text.
///
/// The registrations live in `scenes/mod.rs` and the function that each one
/// names lives in the file for that component's family, so following a scene
/// to its body means reading the whole directory. Concatenating them is enough
/// because a scene function is brace-matched from its own signature and the
/// names were unique before the split.
fn scene_source(source_root: &Path) -> Result<String> {
    let mut files = Vec::new();
    collect(&source_root.join("scenes"), &mut files)?;
    files.sort();
    let mut source = String::new();
    for file in files {
        source.push_str(&fs::read_to_string(file)?);
        source.push('\n');
    }
    Ok(source)
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

/// The public module a caller reaches the item through, which is the first
/// path segment under `src` because `lib.rs` publishes exactly those.
fn module_of(relative: &str) -> String {
    relative
        .trim_start_matches("crates/gpui-kit/src/")
        .split('/')
        .next()
        .unwrap_or_default()
        .trim_end_matches(".rs")
        .to_string()
}

/// Drops comments that are not documentation, and everything from the first
/// `#[cfg(test)]`, so a test fixture never enters the index as an API.
fn strip(source: &str) -> String {
    let source = match source.find("#[cfg(test)]") {
        Some(at) => &source[..at],
        None => source,
    };

    let mut out = String::with_capacity(source.len());
    let characters: Vec<char> = source.chars().collect();
    let mut at = 0;
    let mut in_string = false;
    while at < characters.len() {
        let character = characters[at];
        let next = characters.get(at + 1).copied().unwrap_or('\0');
        if in_string {
            if character == '\\' {
                out.push(character);
                if at + 1 < characters.len() {
                    out.push(next);
                }
                at += 2;
                continue;
            }
            if character == '"' {
                in_string = false;
            }
            out.push(character);
            at += 1;
            continue;
        }
        if character == '"' {
            in_string = true;
            out.push(character);
            at += 1;
            continue;
        }
        // `///` is documentation and stays; `//` and `/* */` are not.
        if character == '/' && next == '/' {
            if characters.get(at + 2) == Some(&'/') {
                while at < characters.len() && characters[at] != '\n' {
                    out.push(characters[at]);
                    at += 1;
                }
                continue;
            }
            while at < characters.len() && characters[at] != '\n' {
                at += 1;
            }
            continue;
        }
        if character == '/' && next == '*' {
            at += 2;
            while at + 1 < characters.len() && !(characters[at] == '*' && characters[at + 1] == '/')
            {
                at += 1;
            }
            at += 2;
            continue;
        }
        out.push(character);
        at += 1;
    }
    out
}

fn read_source(
    source: &str,
    module: &str,
    relative: &str,
    items: &mut BTreeMap<String, Item>,
    events: &mut BTreeMap<String, Vec<String>>,
) {
    let lines: Vec<&str> = source.lines().collect();
    let mut docs: Vec<String> = Vec::new();
    let mut derives = String::new();

    let mut at = 0;
    while at < lines.len() {
        let line = lines[at].trim();

        if let Some(text) = line.strip_prefix("///") {
            docs.push(text.trim().to_string());
            at += 1;
            continue;
        }
        if line.starts_with("#[derive") {
            derives = line.to_string();
            at += 1;
            continue;
        }
        if line.starts_with('#') {
            at += 1;
            continue;
        }

        if let Some(name) = declared(line, "pub struct ") {
            let entry = items.entry(name.clone()).or_default();
            entry.name = name;
            entry.module = module.to_string();
            entry.source = relative.to_string();
            entry.summary = summary(&docs);
            if derives.contains("IntoElement") {
                entry.kind = Kind::Builder;
            }
            docs.clear();
            derives.clear();
            at += 1;
            continue;
        }

        if let Some(name) = declared(line, "pub enum ") {
            let (variants, next) = read_variants(&lines, at);
            if let Some(owner) = name.strip_suffix("Event") {
                events.insert(owner.to_string(), variants.clone());
            }
            let entry = items.entry(name.clone()).or_default();
            entry.name = name;
            entry.module = module.to_string();
            entry.source = relative.to_string();
            entry.summary = summary(&docs);
            entry.variants = variants;
            docs.clear();
            derives.clear();
            at = next;
            continue;
        }

        if let Some(name) = rendered(line) {
            // `impl Render for X` says X is mounted as an entity. It says
            // nothing about whether a caller can name X, and a view a
            // component spawns for itself — a drag ghost, a tooltip's own
            // view — is an implementation detail. Publishing one invites a
            // caller to write a type that is not in scope for them, which is
            // the single failure this index exists to prevent.
            let reachable = items.contains_key(&name)
                || source.contains(&format!("pub struct {name}"))
                || source.contains(&format!("pub enum {name}"));
            if reachable {
                let entry = items.entry(name.clone()).or_default();
                entry.name = name;
                entry.kind = Kind::View;
                if entry.module.is_empty() {
                    entry.module = module.to_string();
                    entry.source = relative.to_string();
                }
            }
            docs.clear();
            derives.clear();
            at += 1;
            continue;
        }

        if let Some(name) = inherent(line) {
            let functions = read_impl(&lines, at);
            // An inherent impl does not make a private declaration public.
            // Only attach methods to a declaration already indexed above.
            if let Some(entry) = items.get_mut(&name) {
                for signature in functions {
                    let how = receiver(&signature);
                    let signature = without_receiver(&signature);
                    match how {
                        Receiver::None => entry.constructors.push(signature),
                        Receiver::Owned => entry.options.push(signature),
                        Receiver::Mutable => entry.commands.push(signature),
                        Receiver::Shared => entry.queries.push(signature),
                    }
                }
            }
            docs.clear();
            derives.clear();
            at += 1;
            continue;
        }

        if !line.is_empty() {
            docs.clear();
            derives.clear();
        }
        at += 1;
    }
}

/// The name in `pub struct Name` / `pub enum Name`, without generics.
fn declared(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    let name: String = rest
        .chars()
        .take_while(|character| character.is_alphanumeric() || *character == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// The name in `impl Render for Name`, which is what makes it a view.
fn rendered(line: &str) -> Option<String> {
    let rest = line.strip_prefix("impl Render for ")?;
    let name: String = rest
        .chars()
        .take_while(|character| character.is_alphanumeric() || *character == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// The name in an inherent `impl Name {`, skipping `impl Trait for Name`.
fn inherent(line: &str) -> Option<String> {
    let rest = line.strip_prefix("impl")?;
    if !rest.starts_with([' ', '<']) {
        return None;
    }
    let rest = rest.trim_start();
    if !rest.ends_with('{') {
        return None;
    }
    let head = rest.trim_end_matches('{').trim();
    if head.contains(" for ") {
        return None;
    }
    // `impl<T> Name` and `impl Name<T>` both name `Name`.
    let head = match head.strip_prefix('<') {
        Some(after) => after.split_once('>').map(|(_, rest)| rest).unwrap_or(after),
        None => head,
    };
    let name: String = head
        .trim()
        .chars()
        .take_while(|character| character.is_alphanumeric() || *character == '_')
        .collect();
    (name.chars().next().is_some_and(char::is_uppercase)).then_some(name)
}

fn read_variants(lines: &[&str], at: usize) -> (Vec<String>, usize) {
    let mut variants = Vec::new();
    let mut depth = 0usize;
    let mut index = at;
    while index < lines.len() {
        let line = lines[index].trim();
        let opens = line.matches('{').count();
        let closes = line.matches('}').count();
        if depth == 1 && !line.starts_with("///") && !line.starts_with('#') {
            let name: String = line
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if name.chars().next().is_some_and(char::is_uppercase) {
                variants.push(name);
            }
        }
        depth = depth + opens - closes.min(depth + opens);
        index += 1;
        if depth == 0 && index > at {
            break;
        }
    }
    (variants, index)
}

/// The `pub fn` signatures of one `impl` block, each collected up to the body.
fn read_impl(lines: &[&str], at: usize) -> Vec<String> {
    let mut signatures = Vec::new();
    let mut depth = 0usize;
    let mut index = at;
    while index < lines.len() {
        let line = lines[index].trim();
        let opens = line.matches('{').count();
        let closes = line.matches('}').count();

        if depth == 1 && line.starts_with("pub fn ") {
            let mut signature = String::new();
            let mut scan = index;
            while scan < lines.len() {
                let piece = lines[scan].trim();
                let (text, complete) = match piece.find(['{', ';']) {
                    Some(at) => (&piece[..at], true),
                    None => (piece, false),
                };
                if !signature.is_empty() {
                    signature.push(' ');
                }
                signature.push_str(text.trim());
                if complete {
                    break;
                }
                scan += 1;
            }
            signatures.push(normalize(signature.trim_start_matches("pub fn ").trim()));
        }

        depth = depth + opens - closes.min(depth + opens);
        index += 1;
        if depth == 0 && index > at {
            break;
        }
    }
    signatures
}

/// Collapses the whitespace a wrapped signature carries, so the index holds
/// one line per function no matter how rustfmt broke the source.
fn normalize(signature: &str) -> String {
    let mut out = String::with_capacity(signature.len());
    let mut space = false;
    for character in signature.chars() {
        if character.is_whitespace() {
            space = true;
            continue;
        }
        // rustfmt leaves a trailing comma when it wraps an argument list, and
        // that comma is not part of what a caller types.
        if character == ')' {
            while out.ends_with(',') {
                out.pop();
            }
        }
        let joins = space
            && !out.is_empty()
            && !matches!(character, ')' | ',')
            && !out.ends_with('(')
            && !out.ends_with('<');
        if joins {
            out.push(' ');
        }
        space = false;
        out.push(character);
    }
    out.trim_end_matches(';').trim().to_string()
}

/// Drops the receiver, because a caller writes `.tone(Tone::Accent)` and
/// never writes the `mut self` in front of it. The index is what to type.
fn without_receiver(signature: &str) -> String {
    let Some((name, rest)) = signature.split_once('(') else {
        return signature.to_string();
    };
    let trimmed = rest.trim_start();
    let after = ["&mut self", "&self", "mut self", "self"]
        .into_iter()
        .find_map(|receiver| trimmed.strip_prefix(receiver));
    let Some(after) = after else {
        return signature.to_string();
    };
    let after = after.trim_start().strip_prefix(',').unwrap_or(after);
    format!("{name}({}", after.trim_start())
}

enum Receiver {
    None,
    Owned,
    Mutable,
    Shared,
}

fn receiver(signature: &str) -> Receiver {
    let Some(arguments) = signature.split_once('(').map(|(_, rest)| rest) else {
        return Receiver::None;
    };
    let first = arguments
        .split([',', ')'])
        .next()
        .unwrap_or_default()
        .trim();
    if first.starts_with("&mut self") {
        Receiver::Mutable
    } else if first.starts_with("&self") {
        Receiver::Shared
    } else if first == "self" || first == "mut self" {
        Receiver::Owned
    } else {
        Receiver::None
    }
}

// ---------------------------------------------------------------------------
// Scenes
// ---------------------------------------------------------------------------

/// What each scene is for, taken from the registry rather than from the shape
/// of the source that builds it.
///
/// The previous version of this read the answer back out of the scene source by
/// following every helper a scene called and collecting the types it touched.
/// That answers "what does this code path reach", which is an upper bound, not
/// "what is this scene for": three scenes sharing one fixture helper each
/// reported the same seven components. The registry now declares it, and the
/// source walk is kept below as the bound the declaration is held to.
fn read_scenes(
    source: &str,
    items: &BTreeMap<String, Item>,
    root: &Path,
) -> Result<Vec<SceneRecord>> {
    let lines: Vec<&str> = source.lines().collect();

    // `Scene { name: "badge", build: badge }` pairs a catalog name with the
    // function whose body becomes the published example.
    let mut builders: BTreeMap<String, String> = BTreeMap::new();
    for (at, line) in lines.iter().enumerate() {
        let Some(rest) = line.trim().strip_prefix("name: \"") else {
            continue;
        };
        let Some((name, _)) = rest.split_once('"') else {
            continue;
        };
        if let Some(function) = lines
            .get(at + 1)
            .and_then(|next| next.trim().strip_prefix("build: "))
            .map(|value| value.trim_end_matches(',').trim().to_string())
        {
            builders.insert(name.to_string(), function);
        }
    }

    let bodies = local_bodies(&lines);
    let mut records = Vec::new();

    for scene in gpui_kit::scenes::catalog() {
        let function = builders.get(scene.name).with_context(|| {
            format!(
                "scene `{}` is registered but its build function could not be found",
                scene.name
            )
        })?;
        let example = body(&lines, function)
            .map(|text| text.trim_start_matches("pub(super) ").to_string())
            .with_context(|| {
                format!(
                    "scene `{}` has no readable body for `{function}`",
                    scene.name
                )
            })?;

        let reachable = mentions(&reach(function, &bodies), items);
        let reachable_slice = reachable.as_slice();
        let declared: Vec<String> = scene
            .shows
            .components()
            .iter()
            .map(|name| (*name).to_string())
            .collect();
        for name in &declared {
            if !can_reach(name, reachable_slice, items, root) {
                bail!(
                    "scene `{}` declares `{name}`, which nothing it renders can reach. \
A scene may name a component it builds, or one that a component it builds mounts; \
it may not name a component it does not show.",
                    scene.name
                );
            }
        }

        records.push(SceneRecord {
            name: scene.name.to_string(),
            kind: match scene.shows {
                gpui_kit::scenes::Shows::Subjects(_) => "exhibit",
                gpui_kit::scenes::Shows::Composition(_) => "composition",
            },
            subjects: scene
                .shows
                .subjects()
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
            uses: declared,
            example,
        });
    }

    Ok(records)
}

/// Every line of source a scene's build function can reach through the helpers
/// it calls, concatenated.
fn reach(function: &str, bodies: &BTreeMap<String, String>) -> String {
    let mut reached = String::new();
    let mut pending = vec![function.to_string()];
    let mut seen = BTreeSet::new();
    while let Some(next) = pending.pop() {
        if !seen.insert(next.clone()) {
            continue;
        }
        let Some(text) = bodies.get(&next) else {
            continue;
        };
        reached.push_str(text);
        reached.push('\n');
        for candidate in bodies.keys() {
            if !seen.contains(candidate) && calls(text, candidate) {
                pending.push(candidate.clone());
            }
        }
    }
    reached
}

/// Whether a scene that reaches `reachable` can be said to show `name`.
///
/// Directly, when the scene builds it. Or through one component: a tooltip's
/// view, a drag ghost, an avatar inside an agent card are all rendered by a
/// component the scene builds rather than by the scene, so the scene's own
/// source never names them and they would otherwise be uncoverable.
fn can_reach(
    name: &str,
    reachable: &[String],
    items: &BTreeMap<String, Item>,
    root: &Path,
) -> bool {
    if reachable.iter().any(|found| found == name) {
        return true;
    }
    reachable.iter().any(|host| {
        items
            .get(host)
            .and_then(|item| fs::read_to_string(root.join(&item.source)).ok())
            .is_some_and(|source| {
                source.contains(&format!("{name}::")) || source.contains(&format!("{name} {{"))
            })
    })
}

fn calls(source: &str, function: &str) -> bool {
    let needle = format!("{function}(");
    source.match_indices(&needle).any(|(at, _)| {
        source[..at]
            .chars()
            .next_back()
            .is_none_or(|before| !before.is_alphanumeric() && !matches!(before, '_' | '.' | ':'))
    })
}

/// Every function declared in the scene catalog, by name, so a scene can be followed
/// into the helpers it calls.
fn local_bodies(lines: &[&str]) -> BTreeMap<String, String> {
    let mut bodies = BTreeMap::new();
    for line in lines {
        let trimmed = line.trim_start();
        let Some(at) = trimmed.find("fn ") else {
            continue;
        };
        if !trimmed[..at]
            .chars()
            .all(|c| c.is_alphanumeric() || "pub()super ".contains(c))
        {
            continue;
        }
        let name: String = trimmed[at + 3..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() || bodies.contains_key(&name) {
            continue;
        }
        if let Some(text) = body(lines, &name) {
            bodies.insert(name, text);
        }
    }
    bodies
}

/// The source of one scene function, brace-matched from its signature.
fn body(lines: &[&str], function: &str) -> Option<String> {
    let head = format!("fn {function}(");
    let start = lines.iter().position(|line| {
        let line = line.trim_start();
        line.starts_with(&head)
            || line
                .strip_prefix("pub")
                .map(|rest| rest.trim_start_matches(|c| c != 'f').starts_with(&head))
                .unwrap_or(false)
    })?;
    let mut depth = 0usize;
    let mut out = Vec::new();
    for line in &lines[start..] {
        out.push(*line);
        depth = depth + line.matches('{').count() - line.matches('}').count().min(depth + 1);
        if depth == 0 && out.len() > 1 {
            break;
        }
    }
    Some(out.join("\n"))
}

/// The indexed types a scene names, which is how a component finds the scenes
/// that prove it works.
fn mentions(example: &str, items: &BTreeMap<String, Item>) -> Vec<String> {
    let mut found = BTreeSet::new();
    for (name, item) in items {
        if item.kind == Kind::Type {
            continue;
        }
        if example.contains(&format!("{name}::")) || example.contains(&format!("{name} {{")) {
            found.insert(name.clone());
        }
    }
    found.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

const NOTE: &str = "Generated by `cargo run -p xtask -- api generate` and \
verified by `gate`. Every signature here was compiled and every scene example \
was rendered, so this file is the API, not a description of it.";

fn render(
    items: &BTreeMap<String, Item>,
    events: &BTreeMap<String, Vec<String>>,
    used: &BTreeMap<&str, Vec<&str>>,
    scenes: &[SceneRecord],
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"note\": {},\n", quote(NOTE)));
    out.push_str("  \"library\": \"gpui-box-kit\",\n");

    let components: Vec<&Item> = items
        .values()
        .filter(|item| item.kind != Kind::Type && !item.name.is_empty())
        .collect();

    out.push_str("  \"components\": [\n");
    for (at, item) in components.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!("      \"name\": {},\n", quote(&item.name)));
        out.push_str(&format!("      \"kind\": {},\n", quote(item.kind.name())));
        out.push_str(&format!(
            "      \"path\": {},\n",
            quote(&format!("gpui_kit::{}::{}", item.module, item.name))
        ));
        out.push_str(&format!("      \"source\": {},\n", quote(&item.source)));
        out.push_str(&format!("      \"summary\": {},\n", quote(&item.summary)));
        out.push_str(&list("construct", &item.constructors));
        out.push_str(&list("options", &item.options));
        out.push_str(&list("commands", &item.commands));
        out.push_str(&list("queries", &item.queries));
        out.push_str(&list(
            "reports",
            events.get(&item.name).map(Vec::as_slice).unwrap_or(&[]),
        ));
        let scenes_for: Vec<String> = used
            .get(item.name.as_str())
            .map(|names| names.iter().map(|name| name.to_string()).collect())
            .unwrap_or_default();
        out.push_str(&last("scenes", &scenes_for));
        out.push_str(if at + 1 == components.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    out.push_str("  ],\n");

    let types: Vec<&Item> = items
        .values()
        .filter(|item| item.kind == Kind::Type && !item.name.is_empty())
        .collect();

    out.push_str("  \"types\": [\n");
    for (at, item) in types.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!("      \"name\": {},\n", quote(&item.name)));
        out.push_str(&format!(
            "      \"path\": {},\n",
            quote(&format!("gpui_kit::{}::{}", item.module, item.name))
        ));
        out.push_str(&format!("      \"summary\": {},\n", quote(&item.summary)));
        out.push_str(&list("variants", &item.variants));
        out.push_str(&list("construct", &item.constructors));
        out.push_str(&list("options", &item.options));
        out.push_str(&list("commands", &item.commands));
        out.push_str(&last("queries", &item.queries));
        out.push_str(if at + 1 == types.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    out.push_str("  ],\n");

    out.push_str("  \"scenes\": [\n");
    for (at, scene) in scenes.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!("      \"name\": {},\n", quote(&scene.name)));
        out.push_str(&format!(
            "      \"capture\": {},\n",
            quote(&format!(
                "cargo run -p xtask -- headless capture {}",
                scene.name
            ))
        ));
        out.push_str(&format!("      \"kind\": {},\n", quote(scene.kind)));
        out.push_str(&list("subjects", &scene.subjects));
        out.push_str(&list("uses", &scene.uses));
        out.push_str(&format!("      \"example\": {}\n", quote(&scene.example)));
        out.push_str(if at + 1 == scenes.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

fn list(key: &str, values: &[String]) -> String {
    format!("{}\n", entry(key, values))
}

fn last(key: &str, values: &[String]) -> String {
    format!("{}\n", entry(key, values).trim_end_matches(','))
}

fn entry(key: &str, values: &[String]) -> String {
    if values.is_empty() {
        return format!("      \"{key}\": [],");
    }
    let body: Vec<String> = values
        .iter()
        .map(|value| format!("        {}", quote(value)))
        .collect();
    format!("      \"{key}\": [\n{}\n      ],", body.join(",\n"))
}

fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if (character as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => out.push(character),
        }
    }
    out.push('"');
    out
}

fn summary(docs: &[String]) -> String {
    let markdown = docs
        .iter()
        .take_while(|line| !line.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let mut plain = String::with_capacity(markdown.len());
    let parser = Parser::new_with_broken_link_callback(
        &markdown,
        Options::empty(),
        Some(|_| Some(("".into(), "".into()))),
    );
    for event in parser {
        match event {
            Event::Text(text)
            | Event::Code(text)
            | Event::InlineMath(text)
            | Event::DisplayMath(text)
            | Event::FootnoteReference(text) => plain.push_str(&text),
            Event::SoftBreak | Event::HardBreak | Event::Rule => {
                if plain.chars().last().is_some_and(|c| !c.is_whitespace()) {
                    plain.push(' ');
                }
            }
            Event::Start(_)
            | Event::End(_)
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::TaskListMarker(_) => {}
        }
    }
    plain.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_derived_element_is_a_builder_and_a_render_impl_is_a_view() {
        let source = strip(
            r#"
/// A compact status label.
#[derive(Debug, IntoElement)]
pub struct Badge { tone: Tone }
impl Badge {
    pub fn new(label: impl Into<SharedString>) -> Self { todo!() }
    pub fn tone(mut self, tone: Tone) -> Self { todo!() }
}
pub struct Select { open: bool }
impl Render for Select {
}
"#,
        );
        let mut items = BTreeMap::new();
        let mut events = BTreeMap::new();
        read_source(&source, "display", "badge.rs", &mut items, &mut events);

        assert_eq!(items["Badge"].kind, Kind::Builder);
        assert_eq!(items["Select"].kind, Kind::View);
        assert_eq!(items["Badge"].summary, "A compact status label.");
        assert_eq!(
            items["Badge"].constructors,
            vec!["new(label: impl Into<SharedString>) -> Self"]
        );
        assert_eq!(items["Badge"].options, vec!["tone(tone: Tone) -> Self"]);
    }

    #[test]
    fn a_summary_is_plain_text_instead_of_rustdoc_markup() {
        let docs = vec![
            "A [`Select`](crate::controls::Select) with [`Entity`] and <b>safe</b> text."
                .to_string(),
        ];

        assert_eq!(summary(&docs), "A Select with Entity and safe text.");
    }

    /// A caller needs to know whether a call chains, needs a `Context`, or
    /// only answers, and that is exactly what the receiver says.
    #[test]
    fn methods_are_sorted_by_what_the_caller_has_to_hold() {
        let source = strip(
            r#"
pub struct Select { open: bool }
impl Select {
    pub fn new(ident: impl Into<Ident>) -> Self { todo!() }
    pub fn options(mut self, options: Vec<SelectOption>) -> Self { todo!() }
    pub fn set_selected(&mut self, id: Option<SharedString>, cx: &mut Context<Self>) { todo!() }
    pub fn is_open(&self) -> bool { todo!() }
}
"#,
        );
        let mut items = BTreeMap::new();
        let mut events = BTreeMap::new();
        read_source(&source, "controls", "select.rs", &mut items, &mut events);

        let select = &items["Select"];
        assert_eq!(select.constructors.len(), 1);
        assert_eq!(select.options.len(), 1);
        assert_eq!(select.commands.len(), 1);
        assert_eq!(select.queries.len(), 1);
    }

    #[test]
    fn a_private_type_with_an_impl_is_not_advertised() {
        let source = strip(
            r#"
struct Internal;
impl Internal {
    pub fn new() -> Self { todo!() }
}
pub struct Public;
impl Public {
    pub fn value(&self) -> bool { true }
}
"#,
        );
        let mut items = BTreeMap::new();
        let mut events = BTreeMap::new();
        read_source(&source, "controls", "private.rs", &mut items, &mut events);

        assert!(!items.contains_key("Internal"));
        assert_eq!(items["Public"].queries, vec!["value() -> bool"]);
    }

    /// An event enum is what the component tells the host, so it belongs to
    /// the component rather than floating as a type nobody connects.
    #[test]
    fn an_event_enum_is_recorded_against_its_component() {
        let source = strip(
            r#"
pub enum SelectEvent {
    Selected { id: SharedString },
    Opened,
    Closed,
}
"#,
        );
        let mut items = BTreeMap::new();
        let mut events = BTreeMap::new();
        read_source(&source, "controls", "select.rs", &mut items, &mut events);

        assert_eq!(events["Select"], vec!["Selected", "Opened", "Closed"]);
    }

    /// A trait implementation is not the type's own API, and indexing one
    /// would advertise a method the caller cannot reach without the trait.
    #[test]
    fn a_trait_impl_is_not_an_inherent_impl() {
        assert_eq!(inherent("impl Badge {"), Some("Badge".to_string()));
        assert_eq!(inherent("impl<T> Grid<T> {"), Some("Grid".to_string()));
        assert_eq!(inherent("impl RenderOnce for Badge {"), None);
        assert_eq!(inherent("impl Default for Badge {"), None);
    }

    /// Test-only fixtures are not API, and a signature behind `#[cfg(test)]`
    /// is one a caller cannot call.
    #[test]
    fn nothing_behind_a_test_gate_reaches_the_index() {
        let source = strip("pub struct A;\n#[cfg(test)]\nmod tests { pub struct B; }\n");
        assert!(source.contains("pub struct A"));
        assert!(!source.contains("pub struct B"));
    }

    /// A wrapped signature has to collapse the same way every time.
    #[test]
    fn a_wrapped_signature_collapses_to_one_line() {
        assert_eq!(
            normalize("new(  ident: impl Into<Ident>,\n    label: SharedString,\n) -> Self"),
            "new(ident: impl Into<Ident>, label: SharedString) -> Self"
        );
    }

    #[test]
    fn a_windows_checkout_matches_the_generated_lf_index() {
        assert!(same_index("one\r\ntwo\r\n", "one\ntwo\n"));
        assert!(!same_index("one\r\nchanged\r\n", "one\ntwo\n"));
    }

    /// The types a scene builds are the bound a declaration is held to, and a
    /// type is only a component when a caller can name it.
    #[test]
    fn a_scene_reaches_the_components_it_builds_and_not_the_type_arguments() {
        let mut items = BTreeMap::new();
        items.insert(
            "Badge".to_string(),
            Item {
                name: "Badge".to_string(),
                kind: Kind::Builder,
                ..Item::default()
            },
        );
        items.insert(
            "Tone".to_string(),
            Item {
                name: "Tone".to_string(),
                kind: Kind::Type,
                ..Item::default()
            },
        );

        let source = "fn badge(_window: &mut Window, cx: &mut App) -> AnyElement {\n\
             \x20   Badge::new(\"Neutral\").tone(Tone::Accent).into_any_element()\n}\n";
        let lines: Vec<&str> = source.lines().collect();
        let bodies = local_bodies(&lines);

        assert_eq!(mentions(&reach("badge", &bodies), &items), vec!["Badge"]);
        assert!(
            body(&lines, "badge")
                .expect("the body is readable")
                .contains("Badge::new")
        );
    }

    /// A scene may declare a component it builds, and one that a component it
    /// builds mounts, and nothing else. Without the second case a tooltip's
    /// own view or an avatar drawn inside an agent card could never be
    /// declared; without the third the declaration would be a wish.
    #[test]
    fn a_declaration_may_not_name_something_the_scene_never_shows() {
        let items = BTreeMap::new();
        let root = Path::new(".");
        let reachable = vec!["Card".to_string()];
        assert!(can_reach("Card", &reachable, &items, root));
        assert!(!can_reach("Table", &reachable, &items, root));
    }

    /// `impl Render for X` is how a view is found, but a view a component
    /// spawns for itself is not something a caller can write.
    #[test]
    fn a_private_view_is_not_published_as_a_component() {
        let source = strip(
            r#"
pub struct Tooltip { label: SharedString }
struct TooltipView(Tooltip);
impl Render for TooltipView {
}
impl Render for Tooltip {
}
"#,
        );
        let mut items = BTreeMap::new();
        let mut events = BTreeMap::new();
        read_source(&source, "overlay", "tooltip.rs", &mut items, &mut events);

        assert!(items.contains_key("Tooltip"));
        assert!(
            !items.contains_key("TooltipView"),
            "a private view is an implementation detail, not API"
        );
    }

    #[test]
    fn helper_calls_require_an_identifier_boundary() {
        assert!(calls("let value = form(window, cx);", "form"));
        assert!(!calls(
            "SpriteTransform::identity().transform(form)",
            "form"
        ));
        assert!(!calls("builder.transform(value)", "form"));
    }

    #[test]
    fn the_rendered_index_is_valid_json() {
        let text = quote("a \"quoted\" line\nand a tab\there");
        assert!(text.starts_with('"') && text.ends_with('"'));
        assert!(text.contains("\\\""));
        assert!(text.contains("\\n"));
        assert!(text.contains("\\t"));
    }
}
