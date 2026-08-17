//! The catalog as a site.
//!
//! Everything here is a rendering of things this repository already generates
//! and already checks: `docs/api-index.json` for what exists and what it is
//! called, `snapshots/headless/macos/scenes` for what it looks like, and `docs/*.md`
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
use gpui_kit_tokens::{
    Color, Elevation, InteractiveColor, Radius, SemanticColor, Space, Surface, TextTone,
    TokenDocument, bundled,
};
use pulldown_cmark::{Options, Parser, html};
use serde_json::Value;

/// Prose worth publishing. `strings-allowlist.txt` and `api-index.json` are
/// machine artifacts, and `llms.txt` is served as itself rather than as a page.
const OMIT: &[&str] = &["strings-allowlist.txt", "api-index.json", "llms.txt"];
const BROWSER_GALLERY_FILES: &[&str] = &[
    "index.html",
    "gpui_kit_browser_gallery.js",
    "gpui_kit_browser_gallery_bg.wasm",
];

pub fn generate(root: &Path, out: Option<&str>, browser_gallery: &Path) -> Result<PathBuf> {
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
    let image_version = image_version(root)?;
    let image_root = format!("/images/{image_version}");

    write(&out.join("assets/site.css"), &site_style())?;
    write(&out.join("assets/site.js"), SCRIPT)?;
    write(&out.join("image-version.txt"), &image_version)?;
    write(
        &out.join("llms.txt"),
        &fs::read_to_string(root.join("docs/llms.txt"))?,
    )?;
    write(
        &out.join("api-index.json"),
        &fs::read_to_string(root.join("docs/api-index.json"))?,
    )?;

    write(
        &out.join("index.html"),
        &home(&components, &scenes, &image_root),
    )?;
    write(
        &out.join("mcp/index.html"),
        &mcp_page(root, &components, &scenes)?,
    )?;

    write(
        &out.join("components/index.html"),
        &redirect_page("/#components"),
    )?;
    for component in &components {
        let name = string(component, "name");
        write(
            &out.join(format!("components/{name}.html")),
            &redirect_page(&format!("/?component={name}#components")),
        )?;
    }

    write(&out.join("scenes/index.html"), &redirect_page("/#scenes"))?;
    for scene in &scenes {
        let name = string(scene, "name");
        write(
            &out.join(format!("scenes/{name}.html")),
            &redirect_page(&format!("/?scene={name}#compose")),
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

    let images = out.join("images").join(&image_version);
    fs::create_dir_all(&images)?;
    let mut copied = 0;
    for entry in fs::read_dir(root.join("snapshots/headless/macos/scenes"))? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "png") {
            let name = path.file_name().context("an image has a name")?;
            fs::copy(&path, images.join(name))?;
            copied += 1;
        }
    }

    write(
        &out.join("assets/search.json"),
        &search(&components, &scenes),
    )?;
    copy_browser_gallery(browser_gallery, &out.join("compose"))?;
    write(
        &out.join("playground/index.html"),
        &redirect_page("/compose/"),
    )?;

    println!(
        "site: {} components, {} scenes, {} pages, {copied} images ({image_version}) -> {}",
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
    let browser_gallery = root
        .join("target")
        .join("site-check-browser-gallery-fixture");
    if browser_gallery.exists() {
        fs::remove_dir_all(&browser_gallery)?;
    }
    fs::create_dir_all(&browser_gallery)?;
    for name in BROWSER_GALLERY_FILES {
        write(&browser_gallery.join(name), "site-check fixture\n")?;
    }
    let result = check_with_browser(root, &browser_gallery);
    let cleanup: Result<()> = fs::remove_dir_all(&browser_gallery).map_err(Into::into);
    result.and(cleanup)
}

/// Checks the complete publishable site against an actual browser build.
pub fn check_with_browser(root: &Path, browser_gallery: &Path) -> Result<()> {
    let out = root.join("target").join("site-check");
    let result = generate(root, out.to_str(), browser_gallery).map(|_| ());
    let cleanup: Result<()> = if out.exists() {
        fs::remove_dir_all(&out)
    } else {
        Ok(())
    }
    .map_err(Into::into);
    result.and(cleanup)?;
    println!("site and browser compose build");
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

fn copy_browser_gallery(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for name in BROWSER_GALLERY_FILES {
        let from = source.join(name);
        fs::copy(&from, destination.join(name)).with_context(|| {
            format!(
                "copy browser gallery asset {}; run `cargo run -p xtask -- web build` first",
                from.display()
            )
        })?;
    }
    Ok(())
}

/// A cache identity for the complete visual catalog.
///
/// Static asset caches can retain an old response at a stable path after a
/// deployment. Put every capture set under a path derived from its bytes so a
/// page can never pair a current API with a previous image. This is an FNV-1a
/// content fingerprint, not a security boundary.
fn image_version(root: &Path) -> Result<String> {
    let mut paths = fs::read_dir(root.join("snapshots/headless/macos/scenes"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.retain(|path| path.extension().is_some_and(|extension| extension == "png"));
    paths.sort();

    let mut hash = 0xcbf29ce484222325_u64;
    for path in paths {
        let name = path.file_name().context("an image has a name")?;
        let bytes = fs::read(&path)?;
        for byte in name.as_encoded_bytes().iter().chain(bytes.iter()) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    Ok(format!("{hash:016x}"))
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

fn shell(title: &str, active: &str, body: &str) -> String {
    let nav = [
        ("/#compose", "Compose"),
        ("/#scenes", "Scenes"),
        ("/#components", "Components"),
        ("/mcp/", "MCP"),
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
  <a class="brand" href="/">GPUI Box</a>
  <nav>{nav}</nav>
  <a class="repo" href="https://github.com/fran0220/gpui-box">GitHub</a>
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

fn redirect_page(target: &str) -> String {
    let escaped = escape(target);
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta http-equiv="refresh" content="0; url={escaped}">
<link rel="canonical" href="{escaped}">
<title>Redirecting — GPUI Box</title>
<script>location.replace({js});</script>
</head>
<body>
<p>This page now lives on the catalog home. <a href="{escaped}">Continue</a>.</p>
</body>
</html>
"#,
        js = Value::from(target)
    )
}

fn home(components: &[Value], scenes: &[Value], image_root: &str) -> String {
    let builders = components
        .iter()
        .filter(|c| string(c, "kind") == "builder")
        .count();
    let views = components.len() - builders;
    let featured = [
        "node-graph",
        "ide-shell",
        "browser-panel",
        "data-grid",
        "schema-form",
        "date-time",
        "conversation",
        "notification-center",
        "loading",
        "progress-circle",
        "thinking",
        "upload-list",
        "dialog",
        "settings",
        "failure-panel",
    ];
    let featured_name = featured
        .iter()
        .copied()
        .find(|name| scenes.iter().any(|scene| string(scene, "name") == *name))
        .unwrap_or("button");

    let body = format!(
        r##"<section class="hero">
  <h1>Native desktop components for GPUI.</h1>
  <p class="lead">A design system, component library, semantic automation layer
  and visual test kit. Surfaces group with colour rather than lines, every word
  is replaceable, and every state is the state it claims to be.</p>
  <p class="cta">
    <a class="button" href="#compose">Compose a scene</a>
    <a class="button quiet" href="#scenes">Browse {scenes_count} scenes</a>
    <a class="button quiet" href="/mcp/">Open MCP</a>
    <a class="button quiet" href="/docs/">Read the docs</a>
  </p>
</section>

<section class="stats">
  <div><b>{builders}</b><span>builders</span></div>
  <div><b>{views}</b><span>views</span></div>
  <div><b>{scenes_count}</b><span>verified scenes</span></div>
  <div><b>{images}</b><span>gate-checked images</span></div>
</section>

<section id="compose" class="compose">
  <h2>Compose</h2>
  <p class="lead">The documentation stays searchable static HTML. These two
  surfaces are the same Rust scene, live in both themes. Until a renderer is
  ready, or if this browser cannot start one, the verified capture remains.</p>
  <div class="compose-toolbar">
    <label class="compose-search">
      <span>Scene</span>
      <input id="compose-filter" type="search" list="compose-scenes" placeholder="Filter {scenes_count} scenes" autocomplete="off">
    </label>
    <datalist id="compose-scenes">{scene_options}</datalist>
    <a class="button quiet" id="compose-full" href="/compose/?scene={featured}&amp;theme=studio-dark">Open full compose</a>
  </div>
  <div class="compose-grid">
    {dark}
    {light}
  </div>
</section>

<section id="scenes" class="catalog">
  <h2>Scenes</h2>
  <p class="lead">{scenes_count} canonical renderings, each captured in both
  themes and compared pixel for pixel on every run. Open one to compose it live
  and see the components it builds.</p>
  <input id="scene-filter" type="search" placeholder="Filter scenes" autocomplete="off">
  <div class="scene-stack">
{scene_cards}
  </div>
  <p id="scenes-empty" class="empty" hidden>Nothing matches.</p>
</section>

<section id="components" class="catalog">
  <h2>Components</h2>
  <p class="lead">A <b>builder</b> is <code>RenderOnce</code>: construct and
  mount it in one expression. A <b>view</b> survives a frame, so it is held in
  an <code>Entity</code>. Both report an intent and apply nothing.</p>
  <input id="component-filter" type="search" placeholder="Filter {count} components" autocomplete="off">
  <div class="rows">
{component_rows}
  </div>
  <p id="components-empty" class="empty" hidden>Nothing matches.</p>
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
"##,
        count = components.len(),
        scenes_count = scenes.len(),
        images = scenes.len() * 2,
        featured = featured_name,
        scene_options = scene_options(scenes),
        scene_cards = scene_cards(scenes, components, image_root),
        component_rows = component_rows(components),
        dark = live_embed(featured_name, "studio-dark", image_root, false),
        light = live_embed(featured_name, "studio-light", image_root, false),
    );
    shell("GPUI Box — native desktop components for GPUI", "/", &body)
}

fn component_rows(components: &[Value]) -> String {
    components
        .iter()
        .map(|component| {
            let name = string(component, "name");
            let kind = string(component, "kind");
            format!(
                r#"<article class="row" id="component-{anchor}" data-component="{name}" data-search="{search}">
  <div class="row-head">
    <b>{name}</b><span class="kind {kind}">{kind}</span>
  </div>
  <p class="summary">{summary}</p>
  <p class="note">{held}</p>
  <pre class="path"><code>use {path};</code></pre>
  {sections}
  <p class="source">Source: <a href="https://github.com/fran0220/gpui-box/blob/main/{source}">{source}</a></p>
</article>"#,
                anchor = slug(&name),
                summary = escape(&string(component, "summary")),
                held = if kind == "view" {
                    "A view survives a frame. Hold it in an <code>Entity</code> with \
                     <code>cx.new(..)</code> and reach it with <code>.update(..)</code>."
                } else {
                    "A builder is <code>RenderOnce</code>. Construct and mount it in one \
                     expression."
                },
                path = escape(&string(component, "path")),
                source = escape(&string(component, "source")),
                sections = component_sections(component),
                search = escape(&format!(
                    "{name} {kind} {} {}",
                    string(component, "summary"),
                    array(component, "scenes")
                        .iter()
                        .map(as_string)
                        .collect::<Vec<_>>()
                        .join(" ")
                ))
                .to_lowercase(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn component_sections(component: &Value) -> String {
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
            "<h3>{title}</h3>{}<pre class=\"sig\"><code>{}</code></pre>",
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
            "<h3>Reports</h3><p class=\"note\">The variants of the event it emits. \
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
        sections.push_str(&format!(
            "<p class=\"note\">Rendered by {}.</p>",
            used_in
                .iter()
                .map(as_string)
                .map(|scene| format!("<a href=\"/?scene={scene}#compose\">{scene}</a>"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    sections
}

fn scene_options(scenes: &[Value]) -> String {
    scenes
        .iter()
        .map(|scene| {
            let name = string(scene, "name");
            format!("<option value=\"{name}\">")
        })
        .collect::<Vec<_>>()
        .join("")
}

fn scene_cards(scenes: &[Value], components: &[Value], image_root: &str) -> String {
    scenes
        .iter()
        .map(|scene| {
            let name = string(scene, "name");
            let uses = array(scene, "uses")
                .iter()
                .map(as_string)
                .filter(|used| components.iter().any(|component| string(component, "name") == *used))
                .map(|used| {
                    format!(
                        "<a href=\"/?component={used}#component-{anchor}\">{used}</a>",
                        anchor = slug(&used)
                    )
                })
                .collect::<Vec<_>>();
            let search = escape(&format!(
                "{name} {}",
                array(scene, "uses")
                    .iter()
                    .map(as_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            ))
            .to_lowercase();
            format!(
                r##"<article class="scene-card" id="scene-{name}" data-scene="{name}" data-search="{search}">
  <a class="tile" href="/?scene={name}#compose">
    <img loading="lazy" src="{image_root}/{name}-studio-dark.png" alt="The {name} scene in studio-dark">
    <span>{name}</span>
  </a>
  <div class="scene-body">
    <h3>{name}</h3>
    <p class="note">Builds {uses}</p>
    <div class="themes compact">
      <figure><img loading="lazy" src="{image_root}/{name}-studio-dark.png" alt="{name} in the dark theme"><figcaption>studio-dark</figcaption></figure>
      <figure><img loading="lazy" src="{image_root}/{name}-studio-light.png" alt="{name} in the light theme"><figcaption>studio-light</figcaption></figure>
    </div>
    <pre class="code"><code>{example}</code></pre>
  </div>
</article>"##,
                uses = if uses.is_empty() {
                    "no catalogued components".to_string()
                } else {
                    uses.join(" ")
                },
                example = highlight(&string(scene, "example")),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn live_embed(scene: &str, theme: &str, image_root: &str, detail: bool) -> String {
    let size = if detail { " detail" } else { "" };
    format!(
        r#"<div class="live-embed{size}" data-live-scene="{scene}" data-live-theme="{theme}">
  <a class="live-fallback" href="/compose/?scene={scene}&amp;theme={theme}">
    <img src="{image_root}/{scene}-{theme}.png" alt="The verified {scene} scene in {theme}">
    <span>Open {scene} in {theme}</span>
  </a>
  <iframe class="live-frame" loading="lazy" tabindex="-1"
    title="Live GPUI Box {scene} scene in {theme}"
    src="/compose/?scene={scene}&amp;theme={theme}&amp;embed=1"></iframe>
</div>"#
    )
}

fn mcp_page(root: &Path, components: &[Value], scenes: &[Value]) -> Result<String> {
    let tools = fs::read_to_string(root.join("tools/mcp/tools.json"))
        .context("tools/mcp/tools.json is missing")?;
    let tools: Value = serde_json::from_str(&tools)?;
    let items = tools
        .as_array()
        .into_iter()
        .flatten()
        .map(|tool| {
            format!(
                "<li><b>{}</b><span>{}</span></li>",
                escape(&string(tool, "name")),
                escape(&string(tool, "description"))
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let body = format!(
        r#"<h1>MCP</h1>
<p class="lead">The same catalog as this repository, answered for an agent —
not the crates.io cohort. Hosted <code>render_scene</code> returns the
committed capture. Working-copy rendering needs the stdio server from a
checkout.</p>
<pre><code>{{
  "mcpServers": {{
    "gpui-box": {{ "url": "https://gpui-box.origingame.dev/mcp" }}
  }}
}}</code></pre>
<p class="note">People read this page. Agents POST JSON-RPC to
<code>/mcp</code>. A GET there is refused because that endpoint opens no
stream.</p>
<h2>Tools</h2>
<ul class="mcp-tools">{items}</ul>
<section class="stats">
  <div><b>{components}</b><span>indexed components</span></div>
  <div><b>{scenes}</b><span>verified scenes</span></div>
</section>
<p class="note">Also published as <a href="/llms.txt">llms.txt</a> and
<a href="/api-index.json">api-index.json</a>.</p>
"#,
        components = components.len(),
        scenes = scenes.len(),
    );
    Ok(shell("MCP — GPUI Box", "/mcp/", &body))
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
    shell("Docs — GPUI Box", "/docs/", &body)
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
    shell(&format!("{page} — GPUI Box"), "/docs/", &body)
}

fn search(components: &[Value], scenes: &[Value]) -> String {
    let components = components.iter().map(|component| {
        format!(
            r#"{{"kind":"component","n":{},"k":{},"s":{}}}"#,
            Value::from(string(component, "name")),
            Value::from(string(component, "kind")),
            Value::from(string(component, "summary"))
        )
    });
    let scenes = scenes.iter().map(|scene| {
        format!(
            r#"{{"kind":"scene","n":{},"s":{}}}"#,
            Value::from(string(scene, "name")),
            Value::from(
                array(scene, "uses")
                    .iter()
                    .map(as_string)
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        )
    });
    format!(
        "[{}]",
        components.chain(scenes).collect::<Vec<_>>().join(",")
    )
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

fn slug(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

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

/// Projects the typed token authority into the browser shell.
///
/// The static site remains DOM so its documentation can be selected, searched,
/// linked, and indexed. Its visual roles still come from exactly the same
/// documents as the GPUI components instead of being copied into a second
/// theme in CSS.
fn site_style() -> String {
    let mut css = String::from(
        "/* @generated token projection; edit crates/gpui-kit-tokens/tokens/*.json */\n",
    );
    for (index, document) in bundled().into_iter().enumerate() {
        let selector = if index == 0 {
            format!(":root, [data-theme=\"{}\"]", document.meta.id)
        } else {
            format!("[data-theme=\"{}\"]", document.meta.id)
        };
        css.push_str(&theme_css(&selector, document));
    }
    css.push('\n');
    css.push_str(STYLE);
    css
}

fn theme_css(selector: &str, tokens: &TokenDocument) -> String {
    let mut css = format!("{selector} {{\n");
    for (name, color) in [
        ("canvas", tokens.surface(Surface::Canvas)),
        ("sunken", tokens.surface(Surface::Sunken)),
        ("panel", tokens.surface(Surface::Panel)),
        ("raised", tokens.surface(Surface::Raised)),
        ("overlay", tokens.surface(Surface::Overlay)),
        ("text", tokens.text(TextTone::Primary)),
        ("muted", tokens.text(TextTone::Muted)),
        ("faint", tokens.text(TextTone::Faint)),
        ("on-accent", tokens.text(TextTone::OnAccent)),
        ("hover", tokens.interactive(InteractiveColor::Hover)),
        ("active", tokens.interactive(InteractiveColor::Active)),
        ("selected", tokens.interactive(InteractiveColor::Selected)),
        ("hairline", tokens.interactive(InteractiveColor::Hairline)),
        (
            "hairline-strong",
            tokens.interactive(InteractiveColor::HairlineStrong),
        ),
        ("focus", tokens.interactive(InteractiveColor::Focus)),
        ("accent", tokens.semantic(SemanticColor::Accent)),
        (
            "accent-strong",
            tokens.semantic(SemanticColor::AccentStrong),
        ),
        ("success", tokens.semantic(SemanticColor::Success)),
        ("warning", tokens.semantic(SemanticColor::Warning)),
        ("danger", tokens.semantic(SemanticColor::Danger)),
        ("info", tokens.semantic(SemanticColor::Info)),
    ] {
        css.push_str(&format!("  --{name}: {};\n", color_css(color)));
    }
    css.push_str(&format!(
        "  --canvas-glass: {};\n  --accent-wash: {};\n  --success-wash: {};\n",
        color_css(with_alpha(tokens.surface(Surface::Canvas), 0.82)),
        color_css(with_alpha(tokens.semantic(SemanticColor::Accent), 0.16)),
        color_css(with_alpha(tokens.semantic(SemanticColor::Success), 0.16)),
    ));
    let overlay = tokens.elevation(Elevation::Overlay);
    css.push_str(&format!(
        "  --shadow-overlay: 0 {}px {}px {}px {};\n",
        overlay.y,
        overlay.blur,
        overlay.spread,
        color_css(overlay.color),
    ));
    for (name, step) in [
        ("xs", Space::Xs),
        ("sm", Space::Sm),
        ("md", Space::Md),
        ("lg", Space::Lg),
        ("xl", Space::Xl),
        ("xxl", Space::Xxl),
    ] {
        css.push_str(&format!("  --space-{name}: {}px;\n", tokens.spacing(step)));
    }
    for (name, step) in [
        ("small", Radius::Small),
        ("control", Radius::Control),
        ("card", Radius::Card),
        ("dialog", Radius::Dialog),
        ("bubble", Radius::Bubble),
        ("pill", Radius::Pill),
    ] {
        css.push_str(&format!("  --radius-{name}: {}px;\n", tokens.radius(step)));
    }
    css.push_str(&format!(
        "  --sans: {};\n  --mono: {};\n",
        font_css(&tokens.typography.sans, "sans-serif"),
        font_css(&tokens.typography.mono, "monospace")
    ));
    css.push_str(&format!(
        "  --motion-quick: {}ms;\n  --motion-standard: cubic-bezier({}, {}, {}, {});\n",
        tokens.motion.duration_ms.quick,
        tokens.motion.easing.standard[0],
        tokens.motion.easing.standard[1],
        tokens.motion.easing.standard[2],
        tokens.motion.easing.standard[3],
    ));
    css.push_str("}\n");
    css
}

fn font_css(tokens: &gpui_kit_tokens::FontTokens, generic: &str) -> String {
    [
        tokens.family.as_str(),
        tokens.fallback_macos.as_str(),
        tokens.fallback_windows.as_str(),
        tokens.fallback_linux.as_str(),
    ]
    .into_iter()
    .map(|family| format!("\"{}\"", family.replace('"', "\\\"")))
    .chain([generic.to_string()])
    .collect::<Vec<_>>()
    .join(", ")
}

fn color_css(color: Color) -> String {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    let red = channel(color.red);
    let green = channel(color.green);
    let blue = channel(color.blue);
    if (color.alpha - 1.0).abs() < f32::EPSILON {
        format!("#{red:02x}{green:02x}{blue:02x}")
    } else {
        let alpha = format!("{:.3}", color.alpha.clamp(0.0, 1.0));
        let alpha = alpha.trim_end_matches('0').trim_end_matches('.');
        format!("rgba({red}, {green}, {blue}, {alpha})")
    }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { alpha, ..color }
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

    #[test]
    fn site_css_is_projected_from_bundled_tokens() {
        let css = site_style();
        assert!(css.contains(":root, [data-theme=\"studio-dark\"]"));
        assert!(css.contains("[data-theme=\"studio-light\"]"));
        assert!(css.contains("--canvas: #131313;"));
        assert!(css.contains("--canvas: #ebebef;"));
        assert!(css.contains("--radius-card: 12px;"));
        assert!(css.contains("--motion-quick: 150ms;"));
    }

    #[test]
    fn embeds_keep_fallbacks() {
        let html = live_embed("button", "studio-dark", "/images/revision", true);
        assert!(html.contains("class=\"live-embed detail\""), "{html}");
        assert!(
            html.contains("/images/revision/button-studio-dark.png"),
            "{html}"
        );
        assert!(html.contains("loading=\"lazy\""), "{html}");
        assert!(
            html.contains("title=\"Live GPUI Box button scene in studio-dark\""),
            "{html}"
        );
        assert!(
            html.contains("/compose/?scene=button&amp;theme=studio-dark&amp;embed=1"),
            "{html}"
        );
    }

    #[test]
    fn browser_gallery_bundle_is_copied_as_one_compose() {
        let fixture = std::env::temp_dir().join(format!(
            "gpui-box-site-browser-fixture-{}",
            std::process::id()
        ));
        let destination = fixture.join("output");
        let _ = fs::remove_dir_all(&fixture);
        fs::create_dir_all(&fixture).expect("create browser fixture");
        for name in BROWSER_GALLERY_FILES {
            write(&fixture.join(name), name).expect("write browser fixture");
        }

        copy_browser_gallery(&fixture, &destination).expect("copy browser gallery");
        for name in BROWSER_GALLERY_FILES {
            assert_eq!(
                fs::read_to_string(destination.join(name)).expect("read copied asset"),
                *name
            );
        }
        fs::remove_dir_all(&fixture).expect("remove browser fixture");
    }

    #[test]
    fn home_keeps_compose_scenes_and_components_on_one_page() {
        let component = serde_json::json!({
            "name": "Button",
            "kind": "builder",
            "path": "gpui_kit::Button",
            "source": "crates/gpui-kit/src/controls/button.rs",
            "summary": "A labeled action.",
            "construct": ["new(ident: impl Into<Ident>) -> Self"],
            "options": [],
            "commands": [],
            "queries": [],
            "reports": [],
            "scenes": ["button"]
        });
        let scene = serde_json::json!({
            "name": "button",
            "uses": ["Button"],
            "example": "fn button() {}"
        });
        let html = home(&[component], &[scene], "/images/revision");
        assert!(html.contains("href=\"/#compose\">Compose</a>"), "{html}");
        assert!(html.contains("href=\"/#scenes\">Scenes</a>"), "{html}");
        assert!(
            html.contains("href=\"/#components\">Components</a>"),
            "{html}"
        );
        assert!(html.contains("href=\"/mcp/\">MCP</a>"), "{html}");
        assert!(html.contains("href=\"/docs/\">Docs</a>"), "{html}");
        assert!(!html.contains("Playground"), "{html}");
        assert!(html.contains("id=\"compose\""), "{html}");
        assert!(html.contains("data-live-theme=\"studio-dark\""), "{html}");
        assert!(html.contains("data-live-theme=\"studio-light\""), "{html}");
        assert!(html.contains("id=\"scene-button\""), "{html}");
        assert!(html.contains("id=\"component-button\""), "{html}");
        assert!(
            html.contains("href=\"/?component=Button#component-button\">Button</a>"),
            "{html}"
        );
        assert!(STYLE.contains("color: var(--text);"), "{STYLE}");
    }

    #[test]
    fn old_catalog_paths_redirect_onto_the_home_or_compose() {
        let home = redirect_page("/#scenes");
        assert!(home.contains("url=/#scenes"), "{home}");
        let compose = redirect_page("/compose/");
        assert!(compose.contains("url=/compose/"), "{compose}");
    }
}
