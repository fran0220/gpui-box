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

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use gpui_kit_tokens::{
    Color, Elevation, InteractiveColor, Radius, ResolvedElevation, SemanticColor, Space, Surface,
    TextTone, TokenDocument, bundled,
};
use pulldown_cmark::{Options, Parser, html};
use serde_json::Value;

/// Prose worth publishing. Generated indexes are machine artifacts, and
/// `llms.txt` is served as itself rather than as a page.
const OMIT: &[&str] = &[
    "strings-allowlist.txt",
    "api-index.json",
    "developer-index.json",
    "llms.txt",
];
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
        &out.join("developer-index.json"),
        &fs::read_to_string(root.join("docs/developer-index.json"))?,
    )?;

    let pages = doc_pages(root)?;
    write(
        &out.join("index.html"),
        &home(&components, &scenes, &image_root),
    )?;
    write(
        &out.join("mcp/index.html"),
        &mcp_page(root, &components, &scenes, &pages)?,
    )?;

    write(
        &out.join("components/index.html"),
        &components_index(&components, &scenes, &image_root),
    )?;
    for component in &components {
        let name = string(component, "name");
        write(
            &out.join(format!("components/{name}.html")),
            &component_page(component, &components, &scenes, &image_root),
        )?;
    }

    write(&out.join("scenes/index.html"), &redirect_page("/compose/"))?;
    for scene in &scenes {
        let name = string(scene, "name");
        write(
            &out.join(format!("scenes/{name}.html")),
            &redirect_page(&format!("/compose/?scene={name}")),
        )?;
    }

    for page in &pages {
        let body = fs::read_to_string(root.join("docs").join(format!("{page}.md")))?;
        write(&out.join(format!("resources/guides/{page}.md")), &body)?;
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
    let token_root = root.join("crates/gpui-kit-tokens/tokens");
    for entry in fs::read_dir(&token_root)? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let name = path
                .file_name()
                .context("a token document has a name")?
                .to_owned();
            fs::create_dir_all(out.join("resources/tokens"))?;
            fs::copy(&path, out.join("resources/tokens").join(name))?;
        }
    }
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

const MODULE_ORDER: &[&str] = &[
    "agent",
    "canvas",
    "content",
    "controls",
    "data",
    "datetime",
    "display",
    "effects",
    "game",
    "interaction",
    "layout",
    "media",
    "navigation",
    "overlay",
    "structured",
];

const DOC_GROUPS: &[(&str, &[(&str, &str)])] = &[
    (
        "Start",
        &[
            ("mcp", "MCP"),
            ("components", "Components"),
            ("design-principles", "Design principles"),
            ("host-view-boundary", "Host/view boundary"),
        ],
    ),
    (
        "Contracts",
        &[
            ("truthful-ui", "Truthful UI"),
            ("component-contracts", "Component contracts"),
            ("interaction", "Interaction"),
            ("content", "Content"),
            ("motion", "Motion"),
            ("datetime", "Date and time"),
            ("accessibility", "Accessibility"),
            ("semantic-automation", "Semantic automation"),
        ],
    ),
    (
        "System",
        &[
            ("token-model", "Token model"),
            ("token-reference", "Token reference"),
            ("coverage", "Coverage"),
        ],
    ),
    (
        "Operate",
        &[
            ("deploying", "Deploying"),
            ("screenshot-testing", "Screenshot testing"),
            ("compatibility", "Compatibility"),
            ("migration-guide", "Migration guide"),
            ("releasing", "Releasing"),
            ("gpui-recipes", "GPUI recipes"),
            ("abi-vocabulary", "ABI vocabulary"),
        ],
    ),
];

const FEATURED_SCENES: &[&str] = &[
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

fn shell(title: &str, active: &str, layout: &str, body: &str) -> String {
    let nav = [("/components/", "Components"), ("/docs/", "Docs")]
        .iter()
        .map(|(href, label)| {
            let current = if *href == active { " class=\"on\"" } else { "" };
            format!("<a href=\"{href}\"{current}>{label}</a>")
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title}</title>
<meta name="description" content="A design system, component library, semantic automation layer and visual test kit for native desktop applications built with GPUI.">
<link rel="stylesheet" href="/assets/site.css">
<script>
(function () {{
  try {{
    var theme = localStorage.getItem("gpui-box-theme");
    if (theme === "studio-light" || theme === "studio-dark") {{
      document.documentElement.setAttribute("data-theme", theme);
    }}
  }} catch (error) {{}}
  if (location.pathname !== "/" && location.pathname !== "/index.html") return;
  var query = new URLSearchParams(location.search);
  var component = query.get("component");
  if (component) {{
    location.replace("/components/" + component + ".html");
    return;
  }}
  var scene = query.get("scene");
  var hash = location.hash;
  if (hash === "#components") {{
    location.replace("/components/");
    return;
  }}
  if (scene || hash === "#compose" || hash === "#scenes") {{
    location.replace(scene ? "/compose/?scene=" + encodeURIComponent(scene) : "/compose/");
  }}
}})();
</script>
</head>
<body class="{layout}">
<header>
  <a class="brand" href="/">GPUI Box</a>
  <nav>{nav}</nav>
  <button type="button" class="theme" id="theme-toggle" aria-label="Use studio-light">Theme</button>
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
"##
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
<p>This page has moved. <a href="{escaped}">Continue</a>.</p>
</body>
</html>
"#,
        js = Value::from(target)
    )
}

fn home(components: &[Value], scenes: &[Value], image_root: &str) -> String {
    let builders = components
        .iter()
        .filter(|component| string(component, "kind") == "builder")
        .count();
    let views = components.len() - builders;
    let featured_name = featured_scene(scenes);
    let plates = FEATURED_SCENES
        .iter()
        .copied()
        .filter(|name| scenes.iter().any(|scene| string(scene, "name") == *name))
        .take(8)
        .map(|name| {
            format!(
                r#"<a class="tile" href="/compose/?scene={name}">
  <img loading="lazy" src="{image_root}/{name}-studio-dark.png" alt="The {name} scene in studio-dark">
  <span>{name}</span>
</a>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let body = format!(
        r##"<section class="hero">
  <p class="eyebrow">GPUI Box</p>
  <h1>Independent GPUI, and the kit that sits on it.</h1>
  <p class="lead">Independent GPUI framework distribution plus a product-neutral
  component, semantic automation, and visual-test kit. Not an official Zed
  project. Surfaces group with colour rather than lines, every word is
  replaceable, and every state is the state it claims to be.</p>
  <p class="cta">
    <a class="button" href="/components/">Browse the catalog</a>
    <a class="button quiet" href="/compose/?scene={featured}&amp;theme=studio-dark">Compose a scene</a>
    <a class="button quiet" href="/docs/">Read the docs</a>
  </p>
</section>

<section id="specimen" class="specimen">
  <div class="specimen-copy">
    <h2>Specimen</h2>
    <p class="lead">The same Rust scene, live in both themes. Until a renderer
    is ready, or if this browser cannot start one, the verified capture
    remains. The documentation stays searchable static HTML.</p>
    <div class="compose-toolbar">
      <label class="compose-search">
        <span>Scene</span>
        <input id="compose-filter" type="search" list="compose-scenes" placeholder="Try {scenes_count} scenes" autocomplete="off">
      </label>
      <datalist id="compose-scenes">{scene_options}</datalist>
      <a class="button quiet" id="compose-full" href="/compose/?scene={featured}&amp;theme=studio-dark">Open full compose</a>
    </div>
  </div>
  <div class="compose-grid">
    {dark}
    {light}
  </div>
</section>

<section class="features">
  <h2>The complete Box</h2>
  <p class="lead">Framework, kit, tokens, truthful states, a visual gate, and
  the catalog an agent can post to.</p>
  <div class="feature-grid">
    <article>
      <h3>Framework</h3>
      <p>Cargo package <code>gpui-box</code> imports as <code>gpui</code>. It
      is derived from GPUI source imported from Zed, but it is not an official
      Zed project.</p>
    </article>
    <article>
      <h3>Kit</h3>
      <p>{builders} builders and {views} views. Components read caller-owned
      data, emit caller-owned actions, and hold visual transient state only.
      <a href="/components/">Open the catalog</a>.</p>
    </article>
    <article>
      <h3>Tokens</h3>
      <p>Colour, spacing, radius, type, motion and effect come from one token
      document through the theme. Studio ships dark and light.
      <a href="/docs/token-model.html">Token model</a>.</p>
    </article>
    <article>
      <h3>Truthful UI</h3>
      <p>Loading, Empty, Unavailable, Error and Ready are five things. A
      refresh failure keeps the last verified value on screen. Anything
      actionable carries a stable semantic id.
      <a href="/docs/truthful-ui.html">Truthful UI</a> ·
      <a href="/docs/semantic-automation.html">Semantics</a>.</p>
    </article>
    <article>
      <h3>Visual gate</h3>
      <p>Every signature on this site was compiled and every image was
      rendered by the same gate that guards the library.
      <a href="/compose/?scene={featured}&amp;theme=studio-dark">Compose</a> ·
      <a href="/docs/screenshot-testing.html">Screenshot testing</a>.</p>
    </article>
    <article>
      <h3>MCP</h3>
      <p>The same catalog as this repository, answered for an agent — not the
      crates.io cohort. Hosted <code>render_scene</code> returns the committed
      capture. <a href="/mcp/">Open MCP</a>.</p>
    </article>
  </div>
</section>

<section class="start">
  <h2>Depend on it</h2>
  <p class="lead">Use Cargo aliases so source code keeps the conventional
  <code>gpui</code> and <code>gpui_kit</code> imports. Do not add another GPUI
  implementation to the same application.</p>
  <div class="copy-block">
    <button type="button" class="copy" data-copy>Copy</button>
    <pre><code>[dependencies]
gpui = {{ package = "gpui-box", version = "0.1.1" }}
gpui_platform = {{ package = "gpui-box-platform", version = "0.1.1" }}
gpui_kit = {{ package = "gpui-box-kit", version = "0.1.1" }}</code></pre>
  </div>
  <div class="copy-block">
    <button type="button" class="copy" data-copy>Copy</button>
    <pre><code>use gpui_kit::prelude::*;

let app = gpui_platform::application().with_assets(gpui_kit::assets::Assets);
app.run(|cx| gpui_kit::install(cx));</code></pre>
  </div>
</section>

<section class="plates">
  <h2>Selected plates</h2>
  <p class="lead">Canonical renderings, captured in both themes and compared
  pixel for pixel. The rest live in compose.</p>
  <div class="gallery">
{plates}
  </div>
</section>
"##,
        scenes_count = scenes.len(),
        featured = featured_name,
        scene_options = scene_options(scenes),
        dark = live_embed(featured_name, "studio-dark", image_root, false),
        light = live_embed(featured_name, "studio-light", image_root, false),
    );
    shell(
        "GPUI Box — independent GPUI and a product-neutral kit",
        "/",
        "home",
        &body,
    )
}

fn components_index(components: &[Value], scenes: &[Value], image_root: &str) -> String {
    let groups = grouped_components(components);
    let catalog = groups
        .iter()
        .map(|(module, items)| {
            let cards = items
                .iter()
                .map(|component| component_card(component, scenes, image_root))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                r#"<section class="module" id="module-{module}" data-module="{module}">
  <h2>{module}</h2>
  <div class="catalog-grid">
{cards}
  </div>
</section>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let body = format!(
        r#"<div class="library">
{rail}
  <div class="library-main">
    <h1>Components</h1>
    <p class="lead">A <b>builder</b> is <code>RenderOnce</code>: construct and
    mount it in one expression. A <b>view</b> survives a frame, so it is held
    in an <code>Entity</code>. Both report an intent and apply nothing.</p>
    <p id="components-empty" class="empty" hidden>Nothing matches.</p>
{catalog}
  </div>
</div>"#,
        rail = component_rail(components, None),
    );
    shell("Components — GPUI Box", "/components/", "library", &body)
}

fn component_page(
    component: &Value,
    components: &[Value],
    scenes: &[Value],
    image_root: &str,
) -> String {
    let name = string(component, "name");
    let kind = string(component, "kind");
    let first = first_scene(component, scenes);
    let preview = match first {
        Some(scene) => {
            let scene_name = string(scene, "name");
            format!(
                r#"<div class="preview">
  {}
  <a class="tile" href="/compose/?scene={scene_name}&amp;theme=studio-light">
    <img src="{image_root}/{scene_name}-studio-light.png" alt="The {scene_name} scene in studio-light">
    <span>studio-light</span>
  </a>
</div>"#,
                live_embed(&scene_name, "studio-dark", image_root, true)
            )
        }
        None => String::new(),
    };
    let scenes_line = review_line(component, scenes);
    let examples = first
        .filter(|scene| !string(scene, "example").is_empty())
        .map(|scene| {
            let scene_name = string(scene, "name");
            fold(
                "example",
                &format!("Example · {scene_name}"),
                "",
                &format!(
                    r#"<pre class="code"><code>{}</code></pre>"#,
                    highlight(&string(scene, "example"))
                ),
            )
        })
        .unwrap_or_default();
    let held = if kind == "view" {
        "A view survives a frame. Hold it in an <code>Entity</code> with \
         <code>cx.new(..)</code> and reach it with <code>.update(..)</code>."
    } else {
        "A builder is <code>RenderOnce</code>. Construct and mount it in one \
         expression."
    };
    let body = format!(
        r#"<div class="library">
{rail}
  <div class="library-main">
    <p class="crumb"><a href="/components/">Components</a> / {module}</p>
    <h1>{name} <span class="kind {kind}">{kind}</span></h1>
    <p class="lead">{summary}</p>
    <p class="note">{held}</p>
    {preview}
    <pre class="path"><code>use {path};</code></pre>
    {scenes_line}
    {sections}
    {examples}
    <p class="source">Source: <a href="https://github.com/fran0220/gpui-box/blob/main/{source}">{source}</a></p>
  </div>
</div>"#,
        rail = component_rail(components, Some(&name)),
        module = component_module(component),
        summary = escape(&string(component, "summary")),
        path = escape(&string(component, "path")),
        source = escape(&string(component, "source")),
        sections = component_sections(component),
    );
    shell(
        &format!("{name} — GPUI Box"),
        "/components/",
        "library",
        &body,
    )
}

fn component_card(component: &Value, scenes: &[Value], image_root: &str) -> String {
    let name = string(component, "name");
    let kind = string(component, "kind");
    let image = first_scene(component, scenes)
        .map(|scene| {
            let scene_name = string(scene, "name");
            format!(
                r#"<img loading="lazy" src="{image_root}/{scene_name}-studio-dark.png" alt="The {name} component in {scene_name}">"#
            )
        })
        .unwrap_or_default();
    format!(
        r#"<a class="tile" href="/components/{name}.html" data-component="{name}" data-search="{search}">
  {image}
  <span class="tile-meta"><b>{name}</b><span class="kind {kind}">{kind}</span></span>
  <p class="summary">{summary}</p>
</a>"#,
        summary = escape(&string(component, "summary")),
        search = component_search(component),
    )
}

fn component_rail(components: &[Value], current: Option<&str>) -> String {
    let groups = grouped_components(components);
    let groups = groups
        .iter()
        .map(|(module, items)| {
            let links = items
                .iter()
                .map(|component| {
                    let name = string(component, "name");
                    let on = if current == Some(name.as_str()) {
                        " class=\"on\""
                    } else {
                        ""
                    };
                    format!(
                        r#"<a href="/components/{name}.html"{on} data-component="{name}" data-search="{search}">{name}</a>"#,
                        search = component_search(component),
                    )
                })
                .collect::<Vec<_>>()
                .join("");
            format!(
                r#"<div class="rail-group" data-module="{module}">
  <a class="rail-module" href="/components/#module-{module}">{module}</a>
  {links}
</div>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"<aside class="rail">
  <label class="compose-search">
    <span>Filter</span>
    <input id="component-filter" type="search" placeholder="Filter components" autocomplete="off">
  </label>
{groups}
</aside>"#
    )
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
        sections.push_str(&fold(
            key,
            title,
            note,
            &format!(
                r#"<pre class="sig"><code>{}</code></pre>"#,
                values
                    .iter()
                    .map(|value| escape(&as_string(value)))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        ));
    }

    let reports = array(component, "reports");
    if !reports.is_empty() {
        sections.push_str(&fold(
            "reports",
            "Reports",
            "The variants of the event it emits. It never applies the change itself.",
            &format!(
                r#"<ul class="variants">{}</ul>"#,
                reports
                    .iter()
                    .map(|value| format!("<li>{}</li>", escape(&as_string(value))))
                    .collect::<Vec<_>>()
                    .join("")
            ),
        ));
    }
    sections
}

fn fold(id: &str, title: &str, note: &str, inner: &str) -> String {
    let note = if note.is_empty() {
        String::new()
    } else {
        format!(r#"<p class="note">{note}</p>"#)
    };
    format!(
        r#"<details class="fold" id="{id}">
  <summary>{title}</summary>
  {note}{inner}
</details>"#
    )
}

/// Where a component is reviewed, and separately where it merely appears.
///
/// The catalog draws the distinction and this is the reader who needs it. An
/// exhibit lays out a component's states and is the answer to "where do I go
/// to look at this". A composition draws it beside a dozen other things to
/// show how they behave together, which is worth linking to and is not a
/// review of anything.
fn review_line(component: &Value, scenes: &[Value]) -> String {
    let name = string(component, "name");
    let exhibits: Vec<String> = array(component, "scenes")
        .iter()
        .map(as_string)
        .map(|scene| format!(r#"<a href="/compose/?scene={scene}">{scene}</a>"#))
        .collect();
    let compositions: Vec<String> = scenes
        .iter()
        .filter(|scene| string(scene, "kind") == "composition")
        .filter(|scene| {
            array(scene, "uses")
                .iter()
                .any(|used| as_string(used) == name)
        })
        .map(|scene| {
            let scene = string(scene, "name");
            format!(r#"<a href="/compose/?scene={scene}">{scene}</a>"#)
        })
        .collect();

    let mut lines = String::new();
    if !exhibits.is_empty() {
        lines.push_str(&format!(
            r#"<p class="note">Reviewed in {}.</p>"#,
            exhibits.join(" ")
        ));
    }
    if !compositions.is_empty() {
        lines.push_str(&format!(
            r#"<p class="note">Also drawn by {}, which arranges components rather than reviewing them.</p>"#,
            compositions.join(" ")
        ));
    }
    lines
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

fn featured_scene(scenes: &[Value]) -> &str {
    FEATURED_SCENES
        .iter()
        .copied()
        .find(|name| scenes.iter().any(|scene| string(scene, "name") == *name))
        .unwrap_or("button")
}

fn component_module(component: &Value) -> String {
    let path = string(component, "path");
    let parts: Vec<&str> = path.split("::").collect();
    if parts.len() >= 3 {
        return parts[1].to_string();
    }
    let source = string(component, "source");
    let parts: Vec<&str> = source.split('/').collect();
    if let Some(index) = parts.iter().position(|part| *part == "src")
        && let Some(module) = parts.get(index + 1)
    {
        return module.trim_end_matches(".rs").to_string();
    }
    "kit".to_string()
}

fn grouped_components(components: &[Value]) -> Vec<(String, Vec<&Value>)> {
    let mut map = BTreeMap::<String, Vec<&Value>>::new();
    for component in components {
        map.entry(component_module(component))
            .or_default()
            .push(component);
    }
    let mut groups = Vec::new();
    for module in MODULE_ORDER {
        if let Some(items) = map.remove(*module) {
            groups.push(((*module).to_string(), items));
        }
    }
    groups.extend(map);
    groups
}

fn first_scene<'a>(component: &Value, scenes: &'a [Value]) -> Option<&'a Value> {
    array(component, "scenes").into_iter().find_map(|name| {
        let name = as_string(&name);
        scenes.iter().find(|scene| string(scene, "name") == name)
    })
}

fn component_search(component: &Value) -> String {
    escape(
        &format!(
            "{} {} {} {} {}",
            string(component, "name"),
            string(component, "kind"),
            component_module(component),
            string(component, "summary"),
            array(component, "scenes")
                .iter()
                .map(as_string)
                .collect::<Vec<_>>()
                .join(" ")
        )
        .to_lowercase(),
    )
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

fn doc_pages(root: &Path) -> Result<Vec<String>> {
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
    Ok(pages)
}

fn mcp_page(
    root: &Path,
    components: &[Value],
    scenes: &[Value],
    pages: &[String],
) -> Result<String> {
    let tools = fs::read_to_string(root.join("tools/mcp/tools.json"))
        .context("tools/mcp/tools.json is missing")?;
    let tools: Value = serde_json::from_str(&tools)?;
    let developer: Value =
        serde_json::from_str(&fs::read_to_string(root.join("docs/developer-index.json"))?)?;
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
        r#"<div class="library">
{rail}
  <div class="library-main">
    <h1>Developer MCP</h1>
    <p class="lead">The complete generated developer catalog of this repository,
    answered by a stateless remote server — not the crates.io cohort. Search
    packages, Rust symbols, components, tokens, guides, recipes, scenes, and
    assets. Working-copy rendering and interactive sessions stay in checkout
    stdio.</p>
    <div class="copy-block">
      <button type="button" class="copy" data-copy>Copy</button>
      <pre><code>{{
  "mcpServers": {{
    "gpui-box": {{ "url": "https://gpui-box.origingame.dev/mcp" }}
  }}
}}</code></pre>
    </div>
    <p class="note">People read this page. Agents POST Streamable HTTP JSON-RPC
    to <code>/mcp</code>. The public service is read-only, stateless, and pinned
    to one Git revision.</p>
    <h2>Tools</h2>
    <ul class="mcp-tools">{items}</ul>
    <p class="note">{packages} packages · {symbols} indexed source symbols ·
    {components} indexed components · {scenes} verified scenes. Also published
    as <a href="/llms.txt">llms.txt</a>,
    <a href="/api-index.json">api-index.json</a>, and
    <a href="/developer-index.json">developer-index.json</a>.</p>
  </div>
</div>"#,
        rail = doc_rail(pages, "mcp"),
        packages = array(&developer, "packages").len(),
        symbols = array(&developer, "symbols").len(),
        components = components.len(),
        scenes = scenes.len(),
    );
    Ok(shell("MCP — GPUI Box", "/docs/", "library", &body))
}

fn doc_list(pages: &[String]) -> String {
    let sections = doc_sections(pages)
        .into_iter()
        .map(|(title, entries)| {
            let items = entries
                .into_iter()
                .map(|(slug, label)| {
                    format!("<li><a href=\"{}\">{label}</a></li>", doc_href(&slug))
                })
                .collect::<Vec<_>>()
                .join("");
            format!("<h2>{title}</h2><ul class=\"docs\">{items}</ul>")
        })
        .collect::<Vec<_>>()
        .join("");
    let body = format!(
        r#"<div class="library">
{rail}
  <div class="library-main">
    <h1>Docs</h1>
    <p class="lead">The contracts, in full. MCP is the catalog an agent
    posts to; the rest is what the gate enforces.</p>
    {sections}
  </div>
</div>"#,
        rail = doc_rail(pages, ""),
    );
    shell("Docs — GPUI Box", "/docs/", "library", &body)
}

fn doc_page(page: &str, markdown: &str, pages: &[String]) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);

    let mut rendered = String::new();
    html::push_html(&mut rendered, Parser::new_ext(markdown, options));

    let body = format!(
        r#"<div class="library">
{rail}
  <article class="library-main prose">{rendered}</article>
</div>"#,
        rail = doc_rail(pages, page),
    );
    shell(
        &format!("{} — GPUI Box", doc_title(page)),
        "/docs/",
        "library",
        &body,
    )
}

fn doc_href(slug: &str) -> String {
    if slug == "mcp" {
        "/mcp/".to_string()
    } else {
        format!("/docs/{slug}.html")
    }
}

fn doc_title(slug: &str) -> String {
    for (_, entries) in DOC_GROUPS {
        for (id, title) in *entries {
            if *id == slug {
                return (*title).to_string();
            }
        }
    }
    slug.to_string()
}

fn doc_sections(pages: &[String]) -> Vec<(String, Vec<(String, String)>)> {
    let mut seen = std::collections::BTreeSet::new();
    let mut sections = Vec::new();
    for (title, entries) in DOC_GROUPS {
        let present = entries
            .iter()
            .filter(|(slug, _)| *slug == "mcp" || pages.iter().any(|page| page == *slug))
            .map(|(slug, label)| {
                seen.insert((*slug).to_string());
                ((*slug).to_string(), (*label).to_string())
            })
            .collect::<Vec<_>>();
        if !present.is_empty() {
            sections.push(((*title).to_string(), present));
        }
    }
    let rest = pages
        .iter()
        .filter(|page| !seen.contains(page.as_str()))
        .map(|page| (page.clone(), page.clone()))
        .collect::<Vec<_>>();
    if !rest.is_empty() {
        sections.push(("Other".to_string(), rest));
    }
    sections
}

fn doc_rail(pages: &[String], current: &str) -> String {
    let groups = doc_sections(pages)
        .into_iter()
        .map(|(title, entries)| {
            let links = entries
                .into_iter()
                .map(|(slug, label)| {
                    let on = if slug == current { " class=\"on\"" } else { "" };
                    format!("<a href=\"{}\"{on}>{label}</a>", doc_href(&slug))
                })
                .collect::<Vec<_>>()
                .join("");
            format!(
                r#"<div class="rail-group"><span class="rail-module">{title}</span>{links}</div>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(r#"<aside class="rail">{groups}</aside>"#)
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
        ("backdrop", tokens.surface(Surface::Backdrop)),
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
    css.push_str(&format!(
        "  --shadow-overlay: {};\n",
        css_shadow(&tokens.elevation(Elevation::Overlay)),
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

fn css_shadow(step: &ResolvedElevation) -> String {
    if step.layers.is_empty() {
        return "none".into();
    }
    step.layers
        .iter()
        .map(|layer| {
            format!(
                "0 {}px {}px {}px {}",
                layer.y,
                layer.blur,
                layer.spread,
                color_css(layer.color)
            )
        })
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
    fn home_is_the_box_not_the_catalog() {
        let component = serde_json::json!({
            "name": "Button",
            "kind": "builder",
            "path": "gpui_kit::controls::Button",
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
        let html = home(
            std::slice::from_ref(&component),
            std::slice::from_ref(&scene),
            "/images/revision",
        );
        assert!(
            html.contains("href=\"/components/\">Components</a>"),
            "{html}"
        );
        assert!(html.contains("href=\"/docs/\">Docs</a>"), "{html}");
        assert!(!html.contains("href=\"/#compose\">Compose</a>"), "{html}");
        assert!(!html.contains("href=\"/#scenes\">Scenes</a>"), "{html}");
        assert!(!html.contains("Playground"), "{html}");
        assert!(!html.contains("id=\"component-button\""), "{html}");
        assert!(!html.contains("id=\"scene-button\""), "{html}");
        assert!(html.contains("id=\"specimen\""), "{html}");
        assert!(html.contains("The complete Box"), "{html}");
        assert!(html.contains("data-live-theme=\"studio-dark\""), "{html}");
        assert!(html.contains("data-live-theme=\"studio-light\""), "{html}");
        assert!(html.contains("gpui = { package = \"gpui-box\""), "{html}");
        assert!(STYLE.contains("color: var(--text);"), "{STYLE}");

        let catalog = components_index(
            std::slice::from_ref(&component),
            std::slice::from_ref(&scene),
            "/images/revision",
        );
        assert!(catalog.contains("id=\"module-controls\""), "{catalog}");
        assert!(
            catalog.contains("href=\"/components/Button.html\""),
            "{catalog}"
        );

        let detail = component_page(
            &component,
            std::slice::from_ref(&component),
            std::slice::from_ref(&scene),
            "/images/revision",
        );
        assert!(detail.contains("<h1>Button"), "{detail}");
        assert!(
            detail.contains("<details class=\"fold\" id=\"construct\">"),
            "{detail}"
        );
        assert!(
            !detail.contains("<details class=\"fold\" id=\"construct\" open"),
            "{detail}"
        );
        assert!(
            detail.contains("href=\"/compose/?scene=button\""),
            "{detail}"
        );
    }

    #[test]
    fn docs_put_mcp_first_and_group_the_rest() {
        let pages = vec![
            "abi-vocabulary".to_string(),
            "components".to_string(),
            "mcp".to_string(),
            "token-model".to_string(),
        ];
        let html = doc_list(&pages);
        let mcp = html.find("href=\"/mcp/\">MCP</a>").expect("mcp");
        let components = html
            .find("href=\"/docs/components.html\">Components</a>")
            .expect("components");
        let tokens = html
            .find("href=\"/docs/token-model.html\">Token model</a>")
            .expect("tokens");
        let abi = html
            .find("href=\"/docs/abi-vocabulary.html\">ABI vocabulary</a>")
            .expect("abi");
        assert!(mcp < components, "{html}");
        assert!(components < tokens, "{html}");
        assert!(tokens < abi, "{html}");
        assert!(html.contains("<h2>Start</h2>"), "{html}");
        assert!(html.contains("<h2>System</h2>"), "{html}");
        assert!(html.contains("<h2>Operate</h2>"), "{html}");
    }

    #[test]
    fn old_catalog_paths_redirect_onto_compose() {
        let scenes = redirect_page("/compose/");
        assert!(scenes.contains("url=/compose/"), "{scenes}");
        let scene = redirect_page("/compose/?scene=button");
        assert!(scene.contains("url=/compose/?scene=button"), "{scene}");
        assert!(scene.contains("This page has moved."), "{scene}");
    }
}
