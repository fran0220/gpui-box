//! The catalog, as tools an agent can call.
//!
//! `docs/api-index.json` already answers what exists and what it is called,
//! and `scenes render` already answers what it looks like. Both are files and
//! commands, which means an agent has to know they are there, guess the right
//! path, and parse the result. This exposes them as tools instead, so finding
//! a component and looking at one are single calls with typed arguments.
//!
//! The one that matters is `render_scene`. Every other tool moves text around;
//! that one runs the real renderer and hands back the image, so an agent can
//! see what it built rather than believe a description of it. Everything else
//! here exists to make that call reachable.
//!
//! # Transport
//!
//! Model Context Protocol over stdio is line-delimited JSON-RPC 2.0, so this
//! is a blocking read loop and needs no async runtime and no protocol crate.
//! Requests carry an id and get a reply; notifications have none and get
//! silence. Nothing is written to stdout that is not a response, because the
//! client parses every line — diagnostics go to stderr.
//!
//! # What it will not do
//!
//! It reads the repository and renders scenes from it. It does not write to
//! it. An agent that wants to change a component edits the source and runs
//! `gate`, which is the check that means something; a tool here that patched
//! files would be a way around it.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

const PROTOCOL: &str = "2025-06-18";

fn main() -> Result<()> {
    let root = root()?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("gpui-kit-mcp: unreadable request: {error}");
                continue;
            }
        };

        // A notification has no id, and answering one is a protocol error.
        let Some(id) = request.get("id").cloned() else {
            continue;
        };
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or(json!({}));

        let response = match dispatch(&root, method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32603, "message": error.to_string() }
            }),
        };
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}

fn dispatch(root: &Path, method: &str, params: &Value) -> Result<Value> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "gpui-kit", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "The gpui-kit component catalog, served from a working \
                             copy of the repository. Signatures come from an index \
                             generated out of the source, and render_scene draws the \
                             scene now, from the code as it currently stands — so it \
                             shows a component you are in the middle of changing. \
                             Prefer these over recall."
        })),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call(root, params),
        "ping" => Ok(json!({})),
        other => bail!("unknown method: {other}"),
    }
}

/// The tool surface, shared with the hosted server so the two cannot describe
/// the same tool differently. What each one *serves* differs, and that is said
/// in `initialize` rather than smuggled into a description.
fn tools() -> Value {
    serde_json::from_str(include_str!("../tools.json")).expect("the tool list is valid JSON")
}
fn call(root: &Path, params: &Value) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .context("tools/call needs a name")?;
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    let argument = |key: &str| {
        arguments
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };

    match name {
        "search_components" => text(search(root, &argument("query"), &argument("kind"))?),
        "component" => text(component(root, &argument("name"))?),
        "scene" => text(scene(root, &argument("name"))?),
        "rules" => text(std::fs::read_to_string(root.join("docs").join("llms.txt"))?),
        "render_scene" => render(root, &argument("name"), &argument("theme")),
        other => bail!("unknown tool: {other}"),
    }
}

fn text(body: String) -> Result<Value> {
    Ok(json!({ "content": [{ "type": "text", "text": body }] }))
}

// ---------------------------------------------------------------------------
// The index
// ---------------------------------------------------------------------------

fn index(root: &Path) -> Result<Value> {
    let path = root.join("docs").join("api-index.json");
    let body = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "{} is missing. Run `cargo run -p xtask -- api generate`.",
            path.display()
        )
    })?;
    Ok(serde_json::from_str(&body)?)
}

fn search(root: &Path, query: &str, kind: &str) -> Result<String> {
    let index = index(root)?;
    let empty = Vec::new();
    let components = index["components"].as_array().unwrap_or(&empty);
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();

    let mut lines = Vec::new();
    for component in components {
        if !kind.is_empty() && component["kind"].as_str() != Some(kind) {
            continue;
        }
        let haystack = format!(
            "{} {} {}",
            component["name"], component["summary"], component["path"]
        )
        .to_lowercase();
        if !words.iter().all(|word| haystack.contains(word)) {
            continue;
        }
        lines.push(format!(
            "{} ({}) — {}\n  path: {}\n  scenes: {}",
            component["name"].as_str().unwrap_or_default(),
            component["kind"].as_str().unwrap_or_default(),
            component["summary"].as_str().unwrap_or("(no summary)"),
            component["path"].as_str().unwrap_or_default(),
            names(&component["scenes"])
        ));
    }

    if lines.is_empty() {
        return Ok(format!(
            "Nothing matches {query:?}. Search with one word, or an empty query \
             to list all {} components.",
            components.len()
        ));
    }
    Ok(format!(
        "{} match(es)\n\n{}",
        lines.len(),
        lines.join("\n\n")
    ))
}

fn component(root: &Path, name: &str) -> Result<String> {
    let index = index(root)?;
    let empty = Vec::new();
    let found = index["components"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .find(|component| component["name"].as_str() == Some(name));

    let Some(component) = found else {
        // A near miss is more useful than a refusal, because the caller is
        // usually one character or one plural away.
        let close: Vec<&str> = index["components"]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .filter_map(|component| component["name"].as_str())
            .filter(|candidate| {
                candidate.to_lowercase().contains(&name.to_lowercase())
                    || name.to_lowercase().contains(&candidate.to_lowercase())
            })
            .collect();
        if close.is_empty() {
            bail!("no component named {name:?}. Call search_components to find one.");
        }
        bail!(
            "no component named {name:?}. Did you mean: {}?",
            close.join(", ")
        );
    };

    let kind = component["kind"].as_str().unwrap_or_default();
    let mut out = format!(
        "{} ({})\n{}\n\npath:   {}\nsource: {}\n",
        component["name"].as_str().unwrap_or_default(),
        kind,
        component["summary"].as_str().unwrap_or("(no summary)"),
        component["path"].as_str().unwrap_or_default(),
        component["source"].as_str().unwrap_or_default(),
    );
    out.push_str(match kind {
        "view" => {
            "\nA view survives a frame: hold it in an Entity with cx.new(...) \
                   and reach it with .update(...).\n"
        }
        _ => "\nA builder is RenderOnce: construct and mount it in one expression.\n",
    });

    section(&mut out, "construct", &component["construct"]);
    section(
        &mut out,
        "options (chain onto the value)",
        &component["options"],
    );
    section(
        &mut out,
        "commands (need a Context)",
        &component["commands"],
    );
    section(&mut out, "queries", &component["queries"]);
    section(&mut out, "reports", &component["reports"]);

    let scenes = names(&component["scenes"]);
    if !scenes.is_empty() {
        out.push_str(&format!(
            "\nscenes that render it: {scenes}\nCall scene(name) for verified example code, \
             or render_scene(name) to look at it.\n"
        ));
    }
    Ok(out)
}

fn scene(root: &Path, name: &str) -> Result<String> {
    let index = index(root)?;
    let empty = Vec::new();
    let found = index["scenes"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .find(|scene| scene["name"].as_str() == Some(name));
    let Some(scene) = found else {
        bail!("no scene named {name:?}. A component's scenes are listed by component(name).");
    };
    Ok(format!(
        "scene {}\nuses: {}\n\n{}\n",
        name,
        names(&scene["uses"]),
        scene["example"].as_str().unwrap_or_default()
    ))
}

fn section(out: &mut String, title: &str, values: &Value) {
    let Some(values) = values.as_array().filter(|values| !values.is_empty()) else {
        return;
    };
    out.push_str(&format!("\n{title}:\n"));
    for value in values {
        out.push_str(&format!("  {}\n", value.as_str().unwrap_or_default()));
    }
}

fn names(value: &Value) -> String {
    value
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Draws the scene and returns the PNG itself, because the point of this tool
/// is that the caller looks at the result rather than reads about it.
fn render(root: &Path, name: &str, theme: &str) -> Result<Value> {
    if name.is_empty() {
        bail!("render_scene needs a scene name");
    }
    let theme = match theme {
        "" => "studio-dark",
        "studio-dark" | "studio-light" => theme,
        other => bail!("unknown theme {other:?}: expected studio-dark or studio-light"),
    };

    let out = root
        .join("target")
        .join("mcp")
        .join(format!("{name}-{theme}.png"));
    std::fs::create_dir_all(out.parent().expect("the path has a parent"))?;

    let output = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "-p", "gpui-kit-gallery", "--", "--scene"])
        .arg(name)
        .arg("--theme")
        .arg(theme)
        .arg("--capture")
        .arg(&out)
        .current_dir(root)
        .output()
        .context("could not run the gallery")?;

    if !output.status.success() {
        bail!(
            "rendering {name} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let png = std::fs::read(&out).with_context(|| {
        format!(
            "the gallery reported success but wrote no image to {}. Is {name:?} a \
             registered scene? Call search_components to find one.",
            out.display()
        )
    })?;

    Ok(json!({
        "content": [
            { "type": "text", "text": format!("{name} in {theme}, {} bytes, at {}", png.len(), out.display()) },
            { "type": "image", "mimeType": "image/png", "data": base64(&png) }
        ]
    }))
}

/// MCP carries an image inline, so the bytes have to be base64. This is the
/// whole of RFC 4648 that a PNG needs, which is cheaper than a dependency.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let block = chunk.iter().enumerate().fold(0u32, |block, (at, byte)| {
            block | (u32::from(*byte) << (16 - 8 * at))
        });
        for at in 0..=chunk.len() {
            out.push(ALPHABET[(block >> (18 - 6 * at) & 0x3f) as usize] as char);
        }
        for _ in chunk.len()..3 {
            out.push('=');
        }
    }
    out
}

/// The repository this server serves, found from the binary rather than from
/// the working directory, because a client starts a server wherever it likes.
fn root() -> Result<PathBuf> {
    if let Ok(set) = std::env::var("GPUI_KIT_ROOT") {
        return Ok(PathBuf::from(set));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .context("could not find the repository root")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vectors from RFC 4648, because an image that decodes to nothing
    /// would look like a rendering failure rather than an encoding one.
    #[test]
    fn base64_matches_the_specification() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    /// Every byte has to survive, since a PNG is not text and a lost high bit
    /// is an image that will not open.
    #[test]
    fn base64_carries_every_byte_value() {
        let all: Vec<u8> = (0..=255).collect();
        let encoded = base64(&all);
        assert_eq!(encoded.len(), 344);
        assert!(encoded.ends_with('='));
    }

    /// A notification carries no id, and a reply to one is a protocol error.
    #[test]
    fn initialize_announces_tools() {
        let root = root().expect("the repository root");
        let result = dispatch(&root, "initialize", &json!({})).expect("initialize");
        assert_eq!(result["protocolVersion"], PROTOCOL);
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn every_tool_declares_a_schema() {
        let tools = tools();
        let tools = tools.as_array().expect("a list");
        assert_eq!(tools.len(), 5);
        for tool in tools {
            assert!(tool["name"].is_string(), "{tool}");
            assert!(tool["description"].is_string(), "{tool}");
            assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        }
    }

    #[test]
    fn an_unknown_tool_is_refused_rather_than_guessed() {
        let root = root().expect("the repository root");
        let error = call(&root, &json!({ "name": "nope", "arguments": {} }))
            .expect_err("an unknown tool is an error");
        assert!(error.to_string().contains("nope"), "{error}");
    }
}
