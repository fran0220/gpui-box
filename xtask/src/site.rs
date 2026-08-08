//! The catalog as a site.
//!
//! Everything here is a rendering of things this repository already generates
//! and already checks: `docs/api-index.json` for what exists and what it is
//! called, `snapshots/macos/scenes` for what it looks like, and `docs/*.md`
//! for the prose. Nothing is authored twice, so the site cannot disagree with
//! the library — it can only be regenerated.
//!
//! That is also why the output is not committed. A generated file is worth
//! checking into a repository when a reviewer should see it change, which is
//! true of an API index and false of ten thousand lines of markup derived from
//! it. The index is the artifact under review; this is a projection of it, and
//! `site check` proves the projection builds rather than pinning its bytes.
//!
//! The site is styled from the same token document the components read, so a
//! surface here is the surface a component would draw. That is not decoration:
//! a component library whose site invents its own colours is showing you
//! something other than the thing you are about to use.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use pulldown_cmark::{Options, Parser, html};
use serde_json::Value;

/// Prose worth publishing. `strings-allowlist.txt` and `api-index.json` are
/// machine artifacts, and `llms.txt` is served as itself rather than as a page.
const OMIT: &[&str] = &["strings-allowlist.txt", "api-index.json", "llms.txt"];

pub fn generate(root: &Path, out: Option<&str>) -> Result<PathBuf> {
    let out = out
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target").join("site"));
    let index = index(root)?;

    if out.exists() {
        fs::remove_dir_all(&out)?;
    }
    fs::create_dir_all(&out)?;

    let components = array(&index, "components");
    let scenes = array(&index, "scenes");

    write(&out.join("assets/site.css"), STYLE)?;
    write(&out.join("assets/site.js"), SCRIPT)?;
    write(
        &out.join("llms.txt"),
        &fs::read_to_string(root.join("docs/llms.txt"))?,
    )?;
    write(
        &out.join("api-index.json"),
        &fs::read_to_string(root.join("docs/api-index.json"))?,
    )?;

    write(&out.join("index.html"), &home(&components, &scenes))?;
    write(
        &out.join("components/index.html"),
        &component_list(&components),
    )?;
    for component in &components {
        let name = string(component, "name");
        write(
            &out.join(format!("components/{name}.html")),
            &component_page(component, &scenes),
        )?;
    }

    write(&out.join("scenes/index.html"), &scene_list(&scenes))?;
    for scene in &scenes {
        let name = string(scene, "name");
        write(
            &out.join(format!("scenes/{name}.html")),
            &scene_page(scene, &components),
        )?;
    }

    let mut pages = Vec::new();
    for entry in fs::read_dir(root.join("docs"))? {
        let path = entry?.path();
        let file = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if OMIT.contains(&file.as_str()) || !file.ends_with(".md") {
            continue;
        }
        pages.push(file.trim_end_matches(".md").to_string());
    }
    pages.sort();
    for page in &pages {
        let body = fs::read_to_string(root.join("docs").join(format!("{page}.md")))?;
        write(
            &out.join(format!("docs/{page}.html")),
            &doc_page(page, &body, &pages),
        )?;
    }
    write(&out.join("docs/index.html"), &doc_list(&pages))?;

    let images = out.join("images");
    fs::create_dir_all(&images)?;
    let mut copied = 0;
    for entry in fs::read_dir(root.join("snapshots/macos/scenes"))? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "png") {
            let name = path.file_name().context("an image has a name")?;
            fs::copy(&path, images.join(name))?;
            copied += 1;
        }
    }

    write(&out.join("assets/search.json"), &search(&components))?;

    println!(
        "site: {} components, {} scenes, {} pages, {copied} images -> {}",
        components.len(),
        scenes.len(),
        pages.len(),
        out.display()
    );
    Ok(out)
}

/// Builds into a scratch directory and throws it away. The site's inputs are
/// gate-checked already, so what is left to prove is that they still render.
pub fn check(root: &Path) -> Result<()> {
    let out = root.join("target").join("site-check");
    generate(root, out.to_str())?;
    fs::remove_dir_all(&out)?;
    println!("site builds");
    Ok(())
}

fn index(root: &Path) -> Result<Value> {
    let path = root.join("docs").join("api-index.json");
    let body = fs::read_to_string(&path).with_context(|| {
        format!(
            "{} is missing. Run `cargo run -p xtask -- api generate`.",
            path.display()
        )
    })?;
    Ok(serde_json::from_str(&body)?)
}

fn write(path: &Path, body: &str) -> Result<()> {
    fs::create_dir_all(path.parent().context("a page has a directory")?)?;
    fs::write(path, body)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

fn shell(title: &str, active: &str, body: &str) -> String {
    let nav = [
        ("/components/", "Components"),
        ("/scenes/", "Scenes"),
        ("/docs/", "Docs"),
    ]
    .iter()
    .map(|(href, label)| {
        let current = if *href == active { " class=\"on\"" } else { "" };
        format!("<a href=\"{href}\"{current}>{label}</a>")
    })
    .collect::<Vec<_>>()
    .join("");

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title}</title>
<meta name="description" content="A design system, component library, semantic automation layer and visual test kit for native desktop applications built with GPUI.">
<link rel="stylesheet" href="/assets/site.css">
</head>
<body>
<header>
  <a class="brand" href="/">gpui-kit</a>
  <nav>{nav}</nav>
  <a class="repo" href="https://github.com/fran0220/gpui-kit">GitHub</a>
</header>
<main>
{body}
</main>
<footer>
  <p>Every signature on this site was compiled and every image was rendered by
  the same gate that guards the library. Regenerate with
  <code>cargo run -p xtask -- site generate</code>.</p>
</footer>
<script src="/assets/site.js"></script>
</body>
</html>
"#
    )
}

fn home(components: &[Value], scenes: &[Value]) -> String {
    let builders = components
        .iter()
        .filter(|c| string(c, "kind") == "builder")
        .count();
    let views = components.len() - builders;

    let featured = [
        "node-graph",
        "data-grid",
        "conversation",
        "dialog",
        "settings",
        "ide-shell",
    ];
    let tiles = featured
        .iter()
        .filter(|name| scenes.iter().any(|s| string(s, "name") == **name))
        .map(|name| {
            format!(
                r#"<a class="tile" href="/scenes/{name}">
  <img loading="lazy" src="/images/{name}-studio-dark.png" alt="The {name} scene">
  <span>{name}</span>
</a>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let body = format!(
        r#"<section class="hero">
  <h1>Native desktop components for GPUI.</h1>
  <p class="lead">A design system, component library, semantic automation layer
  and visual test kit. Surfaces group with colour rather than lines, every word
  is replaceable, and every state is the state it claims to be.</p>
  <p class="cta">
    <a class="button" href="/components/">Browse {count} components</a>
    <a class="button quiet" href="/llms.txt">Read the contracts</a>
  </p>
</section>

<section class="stats">
  <div><b>{builders}</b><span>builders</span></div>
  <div><b>{views}</b><span>views</span></div>
  <div><b>{scenes_count}</b><span>verified scenes</span></div>
  <div><b>{images}</b><span>gate-checked images</span></div>
</section>

<section class="gallery">
{tiles}
</section>

<section class="rules">
  <h2>Four rules that fail a build</h2>
  <p class="lead">These are not review comments. A change that breaks one does
  not land.</p>
  <ol>
    <li><b>No literal a reader reads.</b> Text the library authors is named by a
    <code>StringKey</code> and read from the installed catalogue, so a host can
    replace any of it. Text the caller passes in is the host's, and shown
    verbatim.</li>
    <li><b>No hard-coded visual value.</b> Colour, spacing, radius, type, motion
    and effect come from one token document through the theme.</li>
    <li><b>States are distinct and truthful.</b> Loading, Empty, Unavailable,
    Error and Ready are five things. A refresh failure keeps the last verified
    value on screen. A host refusal is shown as a refusal, never as empty data.
    A disabled control does not install its handler at all.</li>
    <li><b>Anything actionable carries a stable semantic id</b>, derived from
    business identity rather than list position.</li>
  </ol>
</section>

<section class="loop">
  <h2>Built for an agent as much as for a person</h2>
  <p class="lead">The API on this site is generated from the source and checked
  by the gate, so it cannot drift from the library. There is a
  <a href="/llms.txt">llms.txt</a>, a machine-readable
  <a href="/api-index.json">index</a>, and an MCP endpoint at
  <code>/mcp</code> that answers the same questions in one call.</p>
  <pre><code>cargo run -p xtask -- gate                    # fmt, check, test, clippy, tokens, strings, api
cargo run -p xtask -- scenes capture badge   # render what changed
cargo run -p xtask -- scenes check badge     # compare against the committed image</code></pre>
</section>
"#,
        count = components.len(),
        scenes_count = scenes.len(),
        images = scenes.len() * 2,
    );
    shell("gpui-kit — native desktop components for GPUI", "/", &body)
}

fn component_list(components: &[Value]) -> String {
    let rows = components
        .iter()
        .map(|component| {
            let name = string(component, "name");
            let kind = string(component, "kind");
            format!(
                r#"<a class="row" href="/components/{name}" data-search="{search}">
  <b>{name}</b><span class="kind {kind}">{kind}</span>
  <span class="summary">{summary}</span>
</a>"#,
                summary = escape(&string(component, "summary")),
                search = escape(&format!("{name} {kind} {}", string(component, "summary")))
                    .to_lowercase(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let body = format!(
        r#"<h1>Components</h1>
<p class="lead">A <b>builder</b> is <code>RenderOnce</code>: construct and mount it
in one expression. A <b>view</b> survives a frame, so it is held in an
<code>Entity</code>. Both report an intent and apply nothing — the host decides,
which is why a refused change is visible as the control not moving.</p>
<input id="filter" type="search" placeholder="Filter {count} components" autocomplete="off">
<div class="rows">
{rows}
</div>
<p id="empty" class="empty" hidden>Nothing matches.</p>
"#,
        count = components.len()
    );
    shell("Components — gpui-kit", "/components/", &body)
}

fn component_page(component: &Value, scenes: &[Value]) -> String {
    let name = string(component, "name");
    let kind = string(component, "kind");

    let held = if kind == "view" {
        "A view survives a frame. Hold it in an <code>Entity</code> with \
         <code>cx.new(..)</code> and reach it with <code>.update(..)</code>."
    } else {
        "A builder is <code>RenderOnce</code>. Construct and mount it in one \
         expression."
    };

    let mut sections = String::new();
    for (key, title, note) in [
        ("construct", "Construct", ""),
        ("options", "Options", "Chain onto the value."),
        (
            "commands",
            "Commands",
            "Need a <code>Context</code>, so they need a view.",
        ),
        ("queries", "Queries", "Only answer; they change nothing."),
    ] {
        let values = array(component, key);
        if values.is_empty() {
            continue;
        }
        sections.push_str(&format!(
            "<h2>{title}</h2>{}<pre class=\"sig\"><code>{}</code></pre>",
            if note.is_empty() {
                String::new()
            } else {
                format!("<p class=\"note\">{note}</p>")
            },
            values
                .iter()
                .map(|value| escape(&as_string(value)))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    let reports = array(component, "reports");
    if !reports.is_empty() {
        sections.push_str(&format!(
            "<h2>Reports</h2><p class=\"note\">The variants of the event it emits. \
             It never applies the change itself.</p><ul class=\"variants\">{}</ul>",
            reports
                .iter()
                .map(|value| format!("<li>{}</li>", escape(&as_string(value))))
                .collect::<Vec<_>>()
                .join("")
        ));
    }

    let used_in = array(component, "scenes");
    if !used_in.is_empty() {
        let shots = used_in
            .iter()
            .map(as_string)
            .filter(|scene| scenes.iter().any(|s| string(s, "name") == *scene))
            .map(|scene| {
                format!(
                    r#"<a class="tile" href="/scenes/{scene}">
  <img loading="lazy" src="/images/{scene}-studio-dark.png" alt="The {scene} scene">
  <span>{scene}</span>
</a>"#
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push_str(&format!(
            "<h2>Rendered by</h2><p class=\"note\">Each of these compiles and is \
             captured by the gate, so the code behind them is verified rather \
             than written.</p><div class=\"gallery\">{shots}</div>"
        ));
    }

    let body = format!(
        r#"<p class="crumb"><a href="/components/">Components</a></p>
<h1>{name} <span class="kind {kind}">{kind}</span></h1>
<p class="lead">{summary}</p>
<p class="note">{held}</p>
<pre class="path"><code>use {path};</code></pre>
{sections}
<p class="source">Source: <a href="https://github.com/fran0220/gpui-kit/blob/main/{source}">{source}</a></p>
"#,
        summary = escape(&string(component, "summary")),
        path = escape(&string(component, "path")),
        source = escape(&string(component, "source")),
    );
    shell(&format!("{name} — gpui-kit"), "/components/", &body)
}

fn scene_list(scenes: &[Value]) -> String {
    let tiles = scenes
        .iter()
        .map(|scene| {
            let name = string(scene, "name");
            format!(
                r#"<a class="tile" href="/scenes/{name}">
  <img loading="lazy" src="/images/{name}-studio-dark.png" alt="The {name} scene">
  <span>{name}</span>
</a>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let body = format!(
        r#"<h1>Scenes</h1>
<p class="lead">{count} canonical renderings, each captured in both themes and
compared pixel for pixel on every run. A scene is also the example: the code
below each one is what the gate compiled to produce the image above it.</p>
<div class="gallery wide">
{tiles}
</div>
"#,
        count = scenes.len()
    );
    shell("Scenes — gpui-kit", "/scenes/", &body)
}

fn scene_page(scene: &Value, components: &[Value]) -> String {
    let name = string(scene, "name");
    let uses = array(scene, "uses")
        .iter()
        .map(as_string)
        .filter(|used| components.iter().any(|c| string(c, "name") == *used))
        .map(|used| format!("<a href=\"/components/{used}.html\">{used}</a>"))
        .collect::<Vec<_>>()
        .join(" ");

    let body = format!(
        r#"<p class="crumb"><a href="/scenes/">Scenes</a></p>
<h1>{name}</h1>
<p class="note">Builds {uses}</p>
<div class="themes">
  <figure><img src="/images/{name}-studio-dark.png" alt="{name} in the dark theme"><figcaption>studio-dark</figcaption></figure>
  <figure><img src="/images/{name}-studio-light.png" alt="{name} in the light theme"><figcaption>studio-light</figcaption></figure>
</div>
<h2>The code that drew it</h2>
<p class="note">A still frame holds a repeating animation at its first frame and
a one-shot at its last, because a still of a moving thing is not reproducible.
Run the gallery to review motion.</p>
<pre class="code"><code>{example}</code></pre>
"#,
        example = highlight(&string(scene, "example")),
    );
    shell(&format!("{name} — gpui-kit scenes"), "/scenes/", &body)
}

fn doc_list(pages: &[String]) -> String {
    let items = pages
        .iter()
        .map(|page| format!("<li><a href=\"/docs/{page}.html\">{page}</a></li>"))
        .collect::<Vec<_>>()
        .join("");
    let body = format!(
        "<h1>Docs</h1><p class=\"lead\">The contracts, in full.</p><ul class=\"docs\">{items}</ul>"
    );
    shell("Docs — gpui-kit", "/docs/", &body)
}

fn doc_page(page: &str, markdown: &str, pages: &[String]) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);

    let mut rendered = String::new();
    html::push_html(&mut rendered, Parser::new_ext(markdown, options));

    let side = pages
        .iter()
        .map(|other| {
            let current = if other == page { " class=\"on\"" } else { "" };
            format!("<li><a href=\"/docs/{other}.html\"{current}>{other}</a></li>")
        })
        .collect::<Vec<_>>()
        .join("");

    let body = format!(
        r#"<div class="doc">
<aside><ul>{side}</ul></aside>
<article class="prose">{rendered}</article>
</div>"#
    );
    shell(&format!("{page} — gpui-kit"), "/docs/", &body)
}

fn search(components: &[Value]) -> String {
    let entries = components
        .iter()
        .map(|component| {
            format!(
                "{{\"n\":{},\"k\":{},\"s\":{}}}",
                Value::from(string(component, "name")),
                Value::from(string(component, "kind")),
                Value::from(string(component, "summary"))
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{entries}]")
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(character),
        }
    }
    out
}

/// Marks comments, strings and type names in a Rust example.
///
/// It tokenizes first and escapes each token afterwards, because escaping
/// first would turn a quote into `&quot;` and leave nothing to find. Anything
/// it does not recognize stays plain, so the worst case is unhighlighted code
/// rather than wrong code.
fn highlight(source: &str) -> String {
    let characters: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut at = 0;

    while at < characters.len() {
        let character = characters[at];

        if character == '/' && characters.get(at + 1) == Some(&'/') {
            let start = at;
            while at < characters.len() && characters[at] != '\n' {
                at += 1;
            }
            let text: String = characters[start..at].iter().collect();
            out.push_str(&format!("<i class=\"c\">{}</i>", escape(&text)));
            continue;
        }

        if character == '"' {
            let start = at;
            at += 1;
            while at < characters.len() {
                if characters[at] == '\\' {
                    at += 2;
                    continue;
                }
                if characters[at] == '"' {
                    at += 1;
                    break;
                }
                at += 1;
            }
            let text: String = characters[start..at.min(characters.len())].iter().collect();
            out.push_str(&format!("<i class=\"s\">{}</i>", escape(&text)));
            continue;
        }

        if character.is_uppercase() {
            let start = at;
            while at < characters.len()
                && (characters[at].is_alphanumeric() || characters[at] == '_')
            {
                at += 1;
            }
            let text: String = characters[start..at].iter().collect();
            out.push_str(&format!("<i class=\"t\">{}</i>", escape(&text)));
            continue;
        }

        // An identifier is consumed whole so a capital inside one does not
        // become a type name.
        if character.is_alphanumeric() || character == '_' {
            let start = at;
            while at < characters.len()
                && (characters[at].is_alphanumeric() || characters[at] == '_')
            {
                at += 1;
            }
            let text: String = characters[start..at].iter().collect();
            out.push_str(&escape(&text));
            continue;
        }

        out.push_str(&escape(&character.to_string()));
        at += 1;
    }
    out
}

fn array(value: &Value, key: &str) -> Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn as_string(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}

const STYLE: &str = include_str!("site/site.css");
const SCRIPT: &str = include_str!("site/site.js");

#[cfg(test)]
mod tests {
    use super::*;

    /// Markup in a summary would otherwise become markup on the page.
    #[test]
    fn text_is_escaped_before_it_reaches_a_page() {
        assert_eq!(
            escape("Vec<T> & \"quoted\""),
            "Vec&lt;T&gt; &amp; &quot;quoted&quot;"
        );
    }

    /// Escaping before tokenizing would hide the quote the scanner looks for,
    /// so the order is the whole correctness of the highlighter.
    #[test]
    fn highlighting_escapes_after_it_tokenizes() {
        let out = highlight("Badge::new(\"a < b\") // note <b>");
        assert!(out.contains("<i class=\"t\">Badge</i>"), "{out}");
        assert!(
            out.contains("<i class=\"s\">&quot;a &lt; b&quot;</i>"),
            "{out}"
        );
        assert!(
            out.contains("<i class=\"c\">// note &lt;b&gt;</i>"),
            "{out}"
        );
        assert!(!out.contains("<b>"), "{out}");
    }

    /// A capital inside a word is not a type, and treating it as one would
    /// split every identifier the examples use.
    #[test]
    fn a_capital_inside_an_identifier_is_not_a_type() {
        let out = highlight("into_any_element");
        assert_eq!(out, "into_any_element");
        let out = highlight("scene_Name");
        assert!(!out.contains("<i class=\"t\">"), "{out}");
    }
}
