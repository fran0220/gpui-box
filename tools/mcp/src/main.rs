//! GPUI Box Developer MCP.
//!
//! One catalog implementation serves two deliberately different transports:
//! checkout stdio reads and renders the current working tree and can drive a
//! persistent offscreen GPUI session; stateless Streamable HTTP reads only an
//! immutable generated deployment. Neither surface edits projects, compiles
//! caller code, or turns the component library into a remote UI runtime.

use std::collections::BTreeSet;
use std::io::{self, BufRead, BufReader, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;

use anyhow::{Context, Result, bail, ensure};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};

const PROTOCOL: &str = "2025-06-18";
const MAX_HTTP_BODY: usize = 1024 * 1024;
const REMOTE_TOOLS: &str = include_str!("../tools.json");
const LOCAL_TOOLS: &str = include_str!("../local-tools.json");

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("-h" | "--help") => {
            ensure!(args.len() == 1, "--help takes no arguments");
            print_help();
            Ok(())
        }
        Some("-V" | "--version") => {
            ensure!(args.len() == 1, "--version takes no arguments");
            println!("gpui-box-mcp {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("serve") => serve_command(&args[1..]),
        Some("check") => check_command(&args[1..]),
        Some("snapshot-catalog") => snapshot_command(&args[1..]),
        Some("stdio") => {
            ensure!(args.len() == 1, "stdio takes no arguments");
            stdio()
        }
        Some(other) => bail!("unknown command {other:?}; use --help"),
        None => stdio(),
    }
}

fn print_help() {
    println!(
        "gpui-box-mcp {}\n\n\
         Usage:\n  gpui-box-mcp [stdio]\n  gpui-box-mcp serve --listen 127.0.0.1:9350 --catalog <site-public>\n  \
         gpui-box-mcp check --catalog <site-public> --revision <40-char-sha>\n  \
         gpui-box-mcp snapshot-catalog --output <site-public>\n\n\
         Stdio serves the checkout found through GPUI_BOX_ROOT or cwd. The HTTP server is \
         stateless and serves only its immutable catalog directory.",
        env!("CARGO_PKG_VERSION")
    );
}

fn serve_command(args: &[String]) -> Result<()> {
    let listen = option(args, "--listen")?.unwrap_or_else(|| "127.0.0.1:9350".to_string());
    let catalog = option(args, "--catalog")?.context("serve needs --catalog <directory>")?;
    reject_unknown_options(args, &["--listen", "--catalog"])?;
    let address: SocketAddr = listen
        .parse()
        .with_context(|| format!("invalid --listen address {listen:?}"))?;
    let catalog = Arc::new(Catalog::hosted(PathBuf::from(catalog))?);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(serve_http(address, catalog))
}

fn check_command(args: &[String]) -> Result<()> {
    let catalog = option(args, "--catalog")?.context("check needs --catalog <directory>")?;
    let revision = option(args, "--revision")?.context("check needs --revision <sha>")?;
    reject_unknown_options(args, &["--catalog", "--revision"])?;
    ensure!(
        is_revision(&revision),
        "--revision is not a 40-character commit SHA"
    );
    let catalog = Catalog::hosted(PathBuf::from(catalog))?;
    catalog.check_complete(&revision)?;
    println!(
        "catalog {} is complete: {} packages, {} symbols, {} components, {} scenes, {} tools",
        revision,
        catalog.count("packages"),
        catalog.count("symbols"),
        catalog.count("components"),
        catalog.count("scenes"),
        catalog.tools.len()
    );
    Ok(())
}

fn snapshot_command(args: &[String]) -> Result<()> {
    let output =
        option(args, "--output")?.context("snapshot-catalog needs --output <directory>")?;
    reject_unknown_options(args, &["--output"])?;
    let catalog = Catalog::local(root()?)?;
    let output = PathBuf::from(output);
    let mut host = SessionHost::start(&catalog.root)?;
    for scene in catalog.array("scenes") {
        let name = string(scene, "name");
        for theme in ["studio-dark", "studio-light"] {
            let snapshot = snapshot_once(&mut host, name, theme)?;
            let path = output
                .join("semantic")
                .join(theme)
                .join(format!("{name}.json"));
            std::fs::create_dir_all(path.parent().expect("snapshot has a directory"))?;
            std::fs::write(
                &path,
                format!("{}\n", serde_json::to_string_pretty(&snapshot)?),
            )?;
            println!("snapshot {name} {theme}");
        }
    }
    Ok(())
}

fn option(args: &[String], name: &str) -> Result<Option<String>> {
    let Some(at) = args.iter().position(|argument| argument == name) else {
        return Ok(None);
    };
    args.get(at + 1)
        .filter(|value| !value.starts_with("--"))
        .cloned()
        .map(Some)
        .with_context(|| format!("{name} needs a value"))
}

fn reject_unknown_options(args: &[String], allowed: &[&str]) -> Result<()> {
    let mut at = 0;
    while at < args.len() {
        ensure!(
            allowed.contains(&args[at].as_str()),
            "unknown option {:?}",
            args[at]
        );
        ensure!(at + 1 < args.len(), "{} needs a value", args[at]);
        at += 2;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Catalog ownership
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Catalog {
    root: PathBuf,
    hosted: bool,
    developer: Arc<Value>,
    tools: Arc<Vec<Value>>,
    local_tools: Arc<Vec<Value>>,
    revision: Arc<String>,
}

impl Catalog {
    fn local(root: PathBuf) -> Result<Self> {
        let api = read_json(root.join("docs/api-index.json"))?;
        let developer = read_json(root.join("docs/developer-index.json"))?;
        validate_indexes(&api, &developer)?;
        let revision = git_revision(&root).unwrap_or_else(|| "working-copy".to_string());
        Ok(Self {
            root,
            hosted: false,
            developer: Arc::new(developer),
            tools: Arc::new(parse_tool_list(REMOTE_TOOLS)?),
            local_tools: Arc::new(parse_tool_list(LOCAL_TOOLS)?),
            revision: Arc::new(revision),
        })
    }

    fn hosted(root: PathBuf) -> Result<Self> {
        let api = read_json(root.join("api-index.json"))?;
        let developer = read_json(root.join("developer-index.json"))?;
        validate_indexes(&api, &developer)?;
        let tools = read_json(root.join("mcp/tools.json"))?
            .as_array()
            .cloned()
            .context("mcp/tools.json is not an array")?;
        let build = read_json(root.join("build-info.json"))?;
        let revision = build
            .get("revision")
            .and_then(Value::as_str)
            .context("build-info.json has no revision")?
            .to_string();
        ensure!(
            is_revision(&revision),
            "build-info revision is not a commit SHA"
        );
        ensure!(
            build.get("schema").and_then(Value::as_u64) == Some(1),
            "unsupported build-info schema"
        );
        ensure!(
            build.get("catalogSchema") == developer.get("schema"),
            "build-info catalog schema differs from developer-index"
        );
        for key in [
            "packages",
            "symbols",
            "components",
            "types",
            "themes",
            "guides",
            "recipes",
            "scenes",
        ] {
            ensure!(
                build["counts"].get(key).and_then(Value::as_u64)
                    == developer
                        .get(key)
                        .and_then(Value::as_array)
                        .map(|items| items.len() as u64),
                "build-info {key} count differs from developer-index"
            );
        }
        ensure!(
            build["counts"].get("tools").and_then(Value::as_u64) == Some(tools.len() as u64),
            "build-info tool count differs from mcp/tools.json"
        );
        Ok(Self {
            root,
            hosted: true,
            developer: Arc::new(developer),
            tools: Arc::new(tools),
            local_tools: Arc::new(Vec::new()),
            revision: Arc::new(revision),
        })
    }

    fn array(&self, key: &str) -> &[Value] {
        self.developer
            .get(key)
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    fn count(&self, key: &str) -> usize {
        self.array(key).len()
    }

    fn all_tools(&self) -> Vec<Value> {
        self.tools
            .iter()
            .chain(self.local_tools.iter())
            .cloned()
            .collect()
    }

    fn guide(&self, slug: &str) -> Result<String> {
        let path = if self.hosted {
            self.root
                .join("resources/guides")
                .join(format!("{slug}.md"))
        } else {
            self.root.join("docs").join(format!("{slug}.md"))
        };
        std::fs::read_to_string(&path).with_context(|| format!("read guide {slug:?}"))
    }

    fn rules(&self) -> Result<String> {
        let path = if self.hosted {
            self.root.join("llms.txt")
        } else {
            self.root.join("docs/llms.txt")
        };
        Ok(std::fs::read_to_string(path)?)
    }

    fn check_complete(&self, revision: &str) -> Result<()> {
        ensure!(self.hosted, "only a hosted catalog can be release-checked");
        ensure!(
            self.revision.as_str() == revision,
            "catalog revision differs from bundle"
        );
        for key in [
            "packages",
            "symbols",
            "components",
            "themes",
            "guides",
            "scenes",
        ] {
            ensure!(self.count(key) > 0, "developer-index {key} is empty");
        }
        ensure!(
            self.tools.len() == 10,
            "hosted MCP must expose exactly ten tools"
        );
        for guide in self.array("guides") {
            self.guide(string(guide, "slug"))?;
        }
        let image_version = std::fs::read_to_string(self.root.join("image-version.txt"))?;
        for scene in self.array("scenes") {
            let name = string(scene, "name");
            for theme in ["studio-dark", "studio-light"] {
                ensure!(
                    self.root
                        .join("images")
                        .join(image_version.trim())
                        .join(format!("{name}-{theme}.png"))
                        .is_file(),
                    "missing committed image for {name} {theme}"
                );
                ensure!(
                    self.root
                        .join("semantic")
                        .join(theme)
                        .join(format!("{name}.json"))
                        .is_file(),
                    "missing semantic snapshot for {name} {theme}"
                );
            }
        }
        Ok(())
    }
}

fn validate_indexes(api: &Value, developer: &Value) -> Result<()> {
    ensure!(
        developer.get("schema").and_then(Value::as_u64) == Some(1),
        "unsupported developer-index schema"
    );
    for key in ["components", "types", "scenes"] {
        ensure!(
            api.get(key) == developer.get(key),
            "developer-index {key} differs from api-index"
        );
    }
    Ok(())
}

fn read_json(path: impl AsRef<Path>) -> Result<Value> {
    let path = path.as_ref();
    let body = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&body).with_context(|| format!("parse {}", path.display()))
}

fn parse_tool_list(body: &str) -> Result<Vec<Value>> {
    serde_json::from_str::<Value>(body)?
        .as_array()
        .cloned()
        .context("tool definition is not an array")
}

fn git_revision(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn is_revision(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// MCP protocol and transports
// ---------------------------------------------------------------------------

struct Server {
    catalog: Catalog,
    sessions: Option<SessionHost>,
}

impl Server {
    fn new(catalog: Catalog) -> Self {
        Self {
            catalog,
            sessions: None,
        }
    }

    fn response(&mut self, request: &Value) -> Option<Value> {
        let id = request.get("id").cloned()?;
        if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Some(rpc_error(id, -32600, "request must use JSON-RPC 2.0"));
        }
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            return Some(rpc_error(id, -32600, "request has no method"));
        };
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        match self.dispatch(method, &params) {
            Ok(result) => Some(json!({ "jsonrpc": "2.0", "id": id, "result": result })),
            Err(error) => Some(rpc_error(id, error.code, &error.message)),
        }
    }

    fn dispatch(&mut self, method: &str, params: &Value) -> RpcResult<Value> {
        match method {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL,
                "capabilities": {
                    "tools": { "listChanged": false },
                    "resources": { "subscribe": false, "listChanged": false }
                },
                "serverInfo": {
                    "name": env!("CARGO_PKG_NAME"),
                    "version": env!("CARGO_PKG_VERSION")
                },
                "instructions": format!(
                    "GPUI Box Developer MCP at revision {}. Search the generated library index before guessing Rust APIs. Resources carry complete package, symbol, component, token, guide, recipe, scene, and compatibility data. {}",
                    self.catalog.revision,
                    if self.catalog.hosted {
                        "This hosted server is immutable and stateless; rendering and semantic snapshots are deploy artifacts."
                    } else {
                        "This checkout server can render and drive persistent local headless sessions."
                    }
                )
            })),
            "tools/list" => Ok(json!({ "tools": self.catalog.all_tools() })),
            "tools/call" => Ok(self.call_tool(params)),
            "resources/list" => Ok(resource_list()),
            "resources/templates/list" => Ok(resource_templates()),
            "resources/read" => self
                .read_resource(params)
                .map_err(|error| RpcError::internal(error.to_string())),
            "ping" => Ok(json!({})),
            other => Err(RpcError::method(format!("unknown method: {other}"))),
        }
    }

    fn call_tool(&mut self, params: &Value) -> Value {
        let result = (|| -> Result<Value> {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .context("tools/call needs a name")?;
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match name {
                "search_library" => self.search_result(&arguments, false),
                "library_item" => self.item_result(&arguments),
                "recipe" => self.recipe_result(&arguments),
                "render_scene" => self.render_result(&arguments),
                "scene_snapshot" => self.snapshot_result(&arguments),
                "release_info" => Ok(structured_result(
                    self.release_info(),
                    "GPUI Box release info",
                )),
                "search_components" => self.search_result(&arguments, true),
                "component" => self.compat_component(&arguments),
                "scene" => self.compat_scene(&arguments),
                "rules" => Ok(text_result(self.catalog.rules()?)),
                "session_open" => self.session_result("open", &arguments),
                "session_snapshot" => self.session_result("snapshot", &arguments),
                "session_act" => self.session_result("act", &arguments),
                "session_advance" => self.session_result("advance", &arguments),
                "session_screenshot" => self.session_result("screenshot", &arguments),
                "session_audit" => self.session_result("audit", &arguments),
                "session_close" => self.session_result("close", &arguments),
                other => bail!("unknown tool: {other}"),
            }
        })();
        match result {
            Ok(result) => result,
            Err(error) => json!({
                "content": [{ "type": "text", "text": error.to_string() }],
                "isError": true
            }),
        }
    }

    fn release_info(&self) -> Value {
        json!({
            "revision": self.catalog.revision.as_str(),
            "mode": if self.catalog.hosted { "hosted-immutable" } else { "checkout-live" },
            "protocolVersion": PROTOCOL,
            "serverVersion": env!("CARGO_PKG_VERSION"),
            "project": self.catalog.developer.get("project"),
            "counts": {
                "packages": self.catalog.count("packages"),
                "symbols": self.catalog.count("symbols"),
                "components": self.catalog.count("components"),
                "types": self.catalog.count("types"),
                "themes": self.catalog.count("themes"),
                "guides": self.catalog.count("guides"),
                "recipes": self.catalog.count("recipes"),
                "scenes": self.catalog.count("scenes"),
                "tools": self.catalog.all_tools().len(),
            },
            "compatibility": self.catalog.developer.get("compatibility"),
        })
    }

    fn search_result(&self, arguments: &Value, compatibility: bool) -> Result<Value> {
        let mut arguments = arguments.clone();
        if compatibility {
            let kind = arguments
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let kinds = if kind == "type" {
                json!(["type"])
            } else {
                json!(["component"])
            };
            arguments["kinds"] = kinds;
            arguments["limit"] = json!(500);
        }
        let result = search(&self.catalog, &arguments, compatibility)?;
        let matches = result["matches"].as_array().cloned().unwrap_or_default();
        let mut lines = vec![format!(
            "{} match(es); revision {}",
            result["total"].as_u64().unwrap_or_default(),
            self.catalog.revision
        )];
        for item in matches {
            lines.push(format!(
                "{} ({}) — {}\n  id: {}\n  path: {}",
                string(&item, "name"),
                string(&item, "displayKind"),
                string(&item, "summary"),
                string(&item, "id"),
                string(&item, "path")
            ));
        }
        Ok(structured_result(result, lines.join("\n\n")))
    }

    fn item_result(&self, arguments: &Value) -> Result<Value> {
        let id = arguments
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let name = arguments
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let kind = arguments
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let (item_kind, item_id, mut item) = find_item(&self.catalog, id, name, kind)?;
        if item_kind == "guide" {
            item["content"] = Value::String(self.catalog.guide(string(&item, "slug"))?);
        }
        if item_kind == "recipe" {
            let guide = string(&item, "guide");
            let title = string(&item, "title");
            item["content"] = Value::String(markdown_section(&self.catalog.guide(guide)?, title));
        }
        let structured = json!({
            "id": item_id,
            "kind": item_kind,
            "revision": self.catalog.revision.as_str(),
            "item": item,
        });
        Ok(structured_result(
            structured.clone(),
            serde_json::to_string_pretty(&structured)?,
        ))
    }

    fn recipe_result(&self, arguments: &Value) -> Result<Value> {
        let id = arguments
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let goal = arguments
            .get("goal")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let recipe = if !id.is_empty() {
            self.catalog
                .array("recipes")
                .iter()
                .find(|item| string(item, "id") == id.trim_start_matches("recipe:"))
        } else {
            best_match(
                self.catalog.array("recipes"),
                goal,
                &["id", "title", "guide"],
            )
        }
        .with_context(|| format!("no recipe matches {id}{goal:?}"))?;
        let guide = string(recipe, "guide");
        let title = string(recipe, "title");
        let content = markdown_section(&self.catalog.guide(guide)?, title);
        let structured = json!({
            "id": format!("recipe:{}", string(recipe, "id")),
            "title": title,
            "guide": guide,
            "anchor": string(recipe, "anchor"),
            "revision": self.catalog.revision.as_str(),
            "content": content,
        });
        Ok(structured_result(structured, content))
    }

    fn render_result(&self, arguments: &Value) -> Result<Value> {
        let name = required_str(arguments, "name")?;
        let theme = theme(arguments)?;
        ensure_scene(&self.catalog, name)?;
        let (png, source) = if self.catalog.hosted {
            let version = std::fs::read_to_string(self.catalog.root.join("image-version.txt"))?;
            let path = self
                .catalog
                .root
                .join("images")
                .join(version.trim())
                .join(format!("{name}-{theme}.png"));
            (
                std::fs::read(&path)?,
                format!("committed:{revision}", revision = self.catalog.revision),
            )
        } else {
            render_live(&self.catalog.root, name, theme)?
        };
        let metadata = json!({
            "scene": name,
            "theme": theme,
            "bytes": png.len(),
            "source": source,
            "revision": self.catalog.revision.as_str(),
        });
        Ok(json!({
            "content": [
                { "type": "text", "text": format!("{name} in {theme}, {} bytes ({source})", png.len()) },
                { "type": "image", "mimeType": "image/png", "data": base64(&png) }
            ],
            "structuredContent": metadata
        }))
    }

    fn snapshot_result(&mut self, arguments: &Value) -> Result<Value> {
        let name = required_str(arguments, "name")?;
        let theme = theme(arguments)?;
        ensure_scene(&self.catalog, name)?;
        let snapshot = if self.catalog.hosted {
            read_json(
                self.catalog
                    .root
                    .join("semantic")
                    .join(theme)
                    .join(format!("{name}.json")),
            )?
        } else {
            let root = self.catalog.root.clone();
            let host = self.sessions.get_or_insert(SessionHost::start(&root)?);
            snapshot_once(host, name, theme)?
        };
        let structured = json!({
            "scene": name,
            "theme": theme,
            "revision": self.catalog.revision.as_str(),
            "snapshot": snapshot,
        });
        Ok(structured_result(
            structured.clone(),
            serde_json::to_string_pretty(&structured)?,
        ))
    }

    fn compat_component(&self, arguments: &Value) -> Result<Value> {
        let name = required_str(arguments, "name")?;
        self.item_result(&json!({ "name": name }))
    }

    fn compat_scene(&self, arguments: &Value) -> Result<Value> {
        let name = required_str(arguments, "name")?;
        self.item_result(&json!({ "name": name, "kind": "scene" }))
    }

    fn session_result(&mut self, method: &str, arguments: &Value) -> Result<Value> {
        ensure!(!self.catalog.hosted, "session tools are checkout-only");
        let root = self.catalog.root.clone();
        let host = self.sessions.get_or_insert(SessionHost::start(&root)?);
        let mut result = host.request(method, arguments)?;
        if method == "screenshot" {
            let data = result
                .get("png_base64")
                .and_then(Value::as_str)
                .context("session screenshot returned no PNG")?
                .to_string();
            if let Some(object) = result.as_object_mut() {
                object.remove("png_base64");
            }
            return Ok(json!({
                "content": [
                    { "type": "text", "text": serde_json::to_string_pretty(&result)? },
                    { "type": "image", "mimeType": "image/png", "data": data }
                ],
                "structuredContent": result
            }));
        }
        Ok(structured_result(
            result.clone(),
            serde_json::to_string_pretty(&result)?,
        ))
    }

    fn read_resource(&self, params: &Value) -> Result<Value> {
        let uri = params
            .get("uri")
            .and_then(Value::as_str)
            .context("resources/read needs a uri")?;
        let (mime_type, text) = resource(&self.catalog, uri)?;
        Ok(json!({
            "contents": [{ "uri": uri, "mimeType": mime_type, "text": text }]
        }))
    }
}

type RpcResult<T> = std::result::Result<T, RpcError>;

struct RpcError {
    code: i32,
    message: String,
}

impl RpcError {
    fn method(message: String) -> Self {
        Self {
            code: -32601,
            message,
        }
    }

    fn internal(message: String) -> Self {
        Self {
            code: -32603,
            message,
        }
    }
}

fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn stdio() -> Result<()> {
    let catalog = Catalog::local(root()?)?;
    let mut server = Server::new(catalog);
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
                let response = rpc_error(Value::Null, -32700, &format!("parse error: {error}"));
                writeln!(stdout, "{response}")?;
                stdout.flush()?;
                continue;
            }
        };
        if let Some(response) = server.response(&request) {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

async fn serve_http(address: SocketAddr, catalog: Arc<Catalog>) -> Result<()> {
    let app = Router::new()
        .route("/healthz", get(health))
        .route("/mcp", post(http_mcp).get(http_method_not_allowed))
        .route("/mcp/", post(http_mcp).get(http_method_not_allowed))
        .layer(DefaultBodyLimit::max(MAX_HTTP_BODY))
        .with_state(catalog);
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("gpui-box-mcp serving stateless HTTP on {address}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

async fn health(State(catalog): State<Arc<Catalog>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "revision": catalog.revision.as_str(),
        "toolCount": catalog.tools.len(),
        "packageCount": catalog.count("packages"),
        "symbolCount": catalog.count("symbols"),
        "componentCount": catalog.count("components"),
        "sceneCount": catalog.count("scenes"),
    }))
}

async fn http_method_not_allowed() -> impl IntoResponse {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(header::ALLOW, "POST")],
        "stateless MCP accepts POST only",
    )
}

async fn http_mcp(
    State(catalog): State<Arc<Catalog>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(message) = validate_http_headers(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": message }))).into_response();
    }
    let request: Value = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(rpc_error(
                    Value::Null,
                    -32700,
                    &format!("parse error: {error}"),
                )),
            )
                .into_response();
        }
    };
    let mut server = Server::new((*catalog).clone());
    let response = if let Some(batch) = request.as_array() {
        let responses = batch
            .iter()
            .filter_map(|request| server.response(request))
            .collect::<Vec<_>>();
        (!responses.is_empty()).then_some(Value::Array(responses))
    } else {
        server.response(&request)
    };
    match response {
        Some(response) => (StatusCode::OK, Json(response)).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    }
}

fn validate_http_headers(headers: &HeaderMap) -> std::result::Result<(), String> {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let host = host.split(':').next().unwrap_or_default();
    if !matches!(
        host,
        "gpui-box.origingame.dev" | "gpui-kit.origingame.dev" | "127.0.0.1" | "localhost"
    ) {
        return Err(format!("untrusted Host header {host:?}"));
    }
    if let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        && !matches!(
            origin,
            "https://gpui-box.origingame.dev" | "https://gpui-kit.origingame.dev"
        )
    {
        return Err(format!("untrusted Origin header {origin:?}"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Search, items, resources
// ---------------------------------------------------------------------------

fn search(catalog: &Catalog, arguments: &Value, compatibility: bool) -> Result<Value> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let words = words(query);
    let kinds = arguments
        .get("kinds")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let package_filter = arguments
        .get("package")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let selected = |kind: &str| kinds.is_empty() || kinds.contains(kind);
    let mut hits = Vec::new();

    if selected("package") {
        append_hits(
            &mut hits,
            catalog.array("packages"),
            "package",
            "name",
            &words,
            |item| format!("package: {}", string(item, "name")),
        );
    }
    if selected("symbol") {
        append_hits_filtered(
            &mut hits,
            catalog.array("symbols"),
            "symbol",
            "id",
            &words,
            |item| package_filter.is_empty() || string(item, "package") == package_filter,
            |item| string(item, "kind").to_string(),
        );
    }
    if selected("component") {
        append_hits_filtered(
            &mut hits,
            catalog.array("components"),
            "component",
            "name",
            &words,
            |item| {
                if !compatibility {
                    return true;
                }
                let requested = arguments
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                requested.is_empty() || string(item, "kind") == requested
            },
            |item| string(item, "kind").to_string(),
        );
    }
    if selected("type") {
        append_hits(
            &mut hits,
            catalog.array("types"),
            "type",
            "name",
            &words,
            |_| "type".to_string(),
        );
    }
    if selected("theme") {
        append_hits(
            &mut hits,
            catalog.array("themes"),
            "theme",
            "name",
            &words,
            |_| "theme".to_string(),
        );
    }
    if selected("guide") {
        append_hits(
            &mut hits,
            catalog.array("guides"),
            "guide",
            "slug",
            &words,
            |_| "guide".to_string(),
        );
    }
    if selected("recipe") {
        append_hits(
            &mut hits,
            catalog.array("recipes"),
            "recipe",
            "id",
            &words,
            |_| "recipe".to_string(),
        );
    }
    if selected("scene") {
        append_hits(
            &mut hits,
            catalog.array("scenes"),
            "scene",
            "name",
            &words,
            |_| "scene".to_string(),
        );
    }
    if selected("asset") {
        for group in ["icons", "fonts"] {
            for asset in catalog
                .developer
                .get("assets")
                .and_then(|assets| assets.get(group))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if words.iter().all(|word| asset.to_lowercase().contains(word)) {
                    hits.push(json!({
                        "id": format!("asset:{group}:{asset}"),
                        "name": asset,
                        "kind": "asset",
                        "displayKind": group.trim_end_matches('s'),
                        "summary": format!("Bundled GPUI Box {group}"),
                        "path": format!("gpui_kit_assets::{group}::{asset}"),
                        "package": "gpui-box-kit-assets",
                    }));
                }
            }
        }
    }

    hits.sort_by(|a, b| {
        let exact_a = string(a, "name").eq_ignore_ascii_case(query);
        let exact_b = string(b, "name").eq_ignore_ascii_case(query);
        exact_b
            .cmp(&exact_a)
            .then_with(|| string(a, "kind").cmp(string(b, "kind")))
            .then_with(|| string(a, "name").cmp(string(b, "name")))
    });
    let total = hits.len();
    let cursor = arguments
        .get("cursor")
        .and_then(Value::as_str)
        .unwrap_or("0")
        .parse::<usize>()
        .context("cursor is invalid")?;
    ensure!(cursor <= total, "cursor is past the result set");
    let requested_limit = arguments.get("limit").and_then(Value::as_u64).unwrap_or(20) as usize;
    let limit = requested_limit.clamp(1, if compatibility { 500 } else { 100 });
    let page = hits
        .into_iter()
        .skip(cursor)
        .take(limit)
        .collect::<Vec<_>>();
    let next = (cursor + page.len() < total).then(|| (cursor + page.len()).to_string());
    Ok(json!({
        "query": query,
        "matches": page,
        "total": total,
        "nextCursor": next,
        "revision": catalog.revision.as_str(),
        "releaseVersion": catalog.developer["project"]["releaseVersion"],
    }))
}

fn append_hits(
    hits: &mut Vec<Value>,
    items: &[Value],
    kind: &str,
    id_key: &str,
    words: &[String],
    display_kind: impl Fn(&Value) -> String,
) {
    append_hits_filtered(hits, items, kind, id_key, words, |_| true, display_kind);
}

fn append_hits_filtered(
    hits: &mut Vec<Value>,
    items: &[Value],
    kind: &str,
    id_key: &str,
    words: &[String],
    include: impl Fn(&Value) -> bool,
    display_kind: impl Fn(&Value) -> String,
) {
    for item in items {
        if !include(item) || !matches_words(item, words) {
            continue;
        }
        let raw_id = string(item, id_key);
        let id = if kind == "symbol" {
            raw_id.to_string()
        } else {
            format!("{kind}:{raw_id}")
        };
        let name = if kind == "recipe" {
            string(item, "title")
        } else {
            string(item, "name")
        };
        let name = if name.is_empty() { raw_id } else { name };
        let summary = if kind == "recipe" {
            format!("Recipe in guide {}", string(item, "guide"))
        } else {
            string(item, "summary").to_string()
        };
        let path = match kind {
            "package" => string(item, "manifest"),
            "guide" => string(item, "path"),
            "recipe" => string(item, "guide"),
            _ => string(item, "path"),
        };
        hits.push(json!({
            "id": id,
            "name": name,
            "kind": kind,
            "displayKind": display_kind(item),
            "summary": summary,
            "path": path,
            "package": item.get("package").or_else(|| (kind == "package").then(|| item.get("name")).flatten()),
        }));
    }
}

fn matches_words(item: &Value, words: &[String]) -> bool {
    if words.is_empty() {
        return true;
    }
    let haystack = serde_json::to_string(item)
        .unwrap_or_default()
        .to_lowercase();
    words.iter().all(|word| haystack.contains(word))
}

fn words(value: &str) -> Vec<String> {
    value.split_whitespace().map(str::to_lowercase).collect()
}

fn find_item(
    catalog: &Catalog,
    id: &str,
    name: &str,
    kind: &str,
) -> Result<(String, String, Value)> {
    let exact = |key: &str, expected: &str, item: &Value| string(item, key) == expected;
    for (candidate_kind, key, prefix) in [
        ("package", "name", "package:"),
        ("component", "name", "component:"),
        ("type", "name", "type:"),
        ("theme", "name", "theme:"),
        ("guide", "slug", "guide:"),
        ("recipe", "id", "recipe:"),
        ("scene", "name", "scene:"),
    ] {
        if !kind.is_empty() && kind != candidate_kind {
            continue;
        }
        let expected = if !id.is_empty() {
            let Some(expected) = id.strip_prefix(prefix) else {
                continue;
            };
            expected
        } else {
            name
        };
        let key_name = match candidate_kind {
            "package" => "packages",
            "component" => "components",
            "type" => "types",
            "theme" => "themes",
            "guide" => "guides",
            "recipe" => "recipes",
            "scene" => "scenes",
            _ => unreachable!(),
        };
        if let Some(item) = catalog
            .array(key_name)
            .iter()
            .find(|item| exact(key, expected, item))
        {
            return Ok((
                candidate_kind.to_string(),
                format!("{prefix}{expected}"),
                item.clone(),
            ));
        }
    }
    if kind.is_empty() || kind == "symbol" {
        let expected = if !id.is_empty() { id } else { name };
        let candidates = catalog
            .array("symbols")
            .iter()
            .filter(|item| string(item, "id") == expected || string(item, "name") == expected)
            .collect::<Vec<_>>();
        if candidates.len() == 1 {
            return Ok((
                "symbol".to_string(),
                string(candidates[0], "id").to_string(),
                candidates[0].clone(),
            ));
        }
        if candidates.len() > 1 {
            bail!(
                "symbol name {expected:?} is ambiguous; use one id: {}",
                candidates
                    .iter()
                    .map(|item| string(item, "id"))
                    .take(12)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    if (kind.is_empty() || kind == "asset") && id.starts_with("asset:") {
        let mut parts = id.splitn(3, ':');
        let _ = parts.next();
        let group = parts.next().unwrap_or_default();
        let asset = parts.next().unwrap_or_default();
        if catalog.developer["assets"][group]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(asset)))
        {
            return Ok((
                "asset".to_string(),
                id.to_string(),
                json!({ "name": asset, "group": group, "package": "gpui-box-kit-assets" }),
            ));
        }
    }
    bail!("no library item matches id={id:?}, name={name:?}, kind={kind:?}")
}

fn best_match<'a>(items: &'a [Value], query: &str, keys: &[&str]) -> Option<&'a Value> {
    let words = words(query);
    items
        .iter()
        .filter_map(|item| {
            let haystack = keys
                .iter()
                .map(|key| string(item, key).to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            let score = words
                .iter()
                .filter(|word| haystack.contains(word.as_str()))
                .count();
            (score > 0).then_some((score, item))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, item)| item)
}

fn markdown_section(body: &str, title: &str) -> String {
    let marker = format!("## {title}");
    let Some(start) = body.find(&marker) else {
        return body.to_string();
    };
    let section = &body[start..];
    let end = section[marker.len()..]
        .find("\n## ")
        .map(|at| marker.len() + at)
        .unwrap_or(section.len());
    section[..end].trim().to_string()
}

fn resource_list() -> Value {
    let resources = [
        (
            "gpui-box://library",
            "Complete developer index",
            "application/json",
        ),
        ("gpui-box://packages", "Package catalog", "application/json"),
        (
            "gpui-box://components",
            "Component catalog",
            "application/json",
        ),
        (
            "gpui-box://tokens",
            "Theme and token catalog",
            "application/json",
        ),
        ("gpui-box://guides", "Developer guides", "application/json"),
        ("gpui-box://recipes", "Guide recipes", "application/json"),
        ("gpui-box://scenes", "Verified scenes", "application/json"),
        ("gpui-box://assets", "Bundled assets", "application/json"),
        (
            "gpui-box://compatibility",
            "Compatibility contract",
            "application/json",
        ),
        (
            "gpui-box://release",
            "Catalog revision and counts",
            "application/json",
        ),
    ]
    .into_iter()
    .map(|(uri, name, mime)| json!({ "uri": uri, "name": name, "mimeType": mime }))
    .collect::<Vec<_>>();
    json!({ "resources": resources })
}

fn resource_templates() -> Value {
    json!({
        "resourceTemplates": [
            { "uriTemplate": "gpui-box://packages/{name}", "name": "Package", "mimeType": "application/json" },
            { "uriTemplate": "gpui-box://symbols/{id}", "name": "Public Rust symbol", "mimeType": "application/json" },
            { "uriTemplate": "gpui-box://components/{name}", "name": "Component", "mimeType": "application/json" },
            { "uriTemplate": "gpui-box://types/{name}", "name": "Supporting type", "mimeType": "application/json" },
            { "uriTemplate": "gpui-box://tokens/{theme}", "name": "Theme tokens", "mimeType": "application/json" },
            { "uriTemplate": "gpui-box://guides/{slug}", "name": "Developer guide", "mimeType": "text/markdown" },
            { "uriTemplate": "gpui-box://recipes/{id}", "name": "Implementation recipe", "mimeType": "text/markdown" },
            { "uriTemplate": "gpui-box://scenes/{name}", "name": "Verified scene", "mimeType": "application/json" },
            { "uriTemplate": "gpui-box://semantic/{theme}/{scene}", "name": "Redacted scene semantics", "mimeType": "application/json" }
        ]
    })
}

fn resource(catalog: &Catalog, uri: &str) -> Result<(&'static str, String)> {
    let json_text = |value: &Value| Ok(("application/json", serde_json::to_string_pretty(value)?));
    match uri {
        "gpui-box://library" => return json_text(&catalog.developer),
        "gpui-box://packages" => return json_text(&catalog.developer["packages"]),
        "gpui-box://components" => return json_text(&catalog.developer["components"]),
        "gpui-box://tokens" => return json_text(&catalog.developer["themes"]),
        "gpui-box://guides" => return json_text(&catalog.developer["guides"]),
        "gpui-box://recipes" => return json_text(&catalog.developer["recipes"]),
        "gpui-box://scenes" => return json_text(&catalog.developer["scenes"]),
        "gpui-box://assets" => return json_text(&catalog.developer["assets"]),
        "gpui-box://compatibility" => return json_text(&catalog.developer["compatibility"]),
        "gpui-box://release" => {
            return json_text(&json!({
                "revision": catalog.revision.as_str(),
                "project": catalog.developer["project"],
            }));
        }
        _ => {}
    }
    if let Some(name) = uri.strip_prefix("gpui-box://packages/") {
        let (_, _, item) = find_item(catalog, &format!("package:{name}"), "", "package")?;
        return json_text(&item);
    }
    if let Some(id) = uri.strip_prefix("gpui-box://symbols/") {
        let (_, _, item) = find_item(catalog, id, "", "symbol")?;
        return json_text(&item);
    }
    if let Some(name) = uri.strip_prefix("gpui-box://components/") {
        let (_, _, item) = find_item(catalog, &format!("component:{name}"), "", "component")?;
        return json_text(&item);
    }
    if let Some(name) = uri.strip_prefix("gpui-box://types/") {
        let (_, _, item) = find_item(catalog, &format!("type:{name}"), "", "type")?;
        return json_text(&item);
    }
    if let Some(theme) = uri.strip_prefix("gpui-box://tokens/") {
        let (_, _, item) = find_item(catalog, &format!("theme:{theme}"), "", "theme")?;
        return json_text(&item);
    }
    if let Some(slug) = uri.strip_prefix("gpui-box://guides/") {
        find_item(catalog, &format!("guide:{slug}"), "", "guide")?;
        return Ok(("text/markdown", catalog.guide(slug)?));
    }
    if let Some(id) = uri.strip_prefix("gpui-box://recipes/") {
        let (_, _, item) = find_item(catalog, &format!("recipe:{id}"), "", "recipe")?;
        let content = markdown_section(
            &catalog.guide(string(&item, "guide"))?,
            string(&item, "title"),
        );
        return Ok(("text/markdown", content));
    }
    if let Some(name) = uri.strip_prefix("gpui-box://scenes/") {
        let (_, _, item) = find_item(catalog, &format!("scene:{name}"), "", "scene")?;
        return json_text(&item);
    }
    if let Some(path) = uri.strip_prefix("gpui-box://semantic/") {
        ensure!(
            catalog.hosted,
            "semantic resources are deploy artifacts on the remote MCP"
        );
        let (theme, scene) = path
            .split_once('/')
            .context("semantic URI needs theme and scene")?;
        ensure!(
            matches!(theme, "studio-dark" | "studio-light"),
            "unknown semantic snapshot theme {theme:?}"
        );
        ensure_scene(catalog, scene)?;
        return json_text(&read_json(
            catalog
                .root
                .join("semantic")
                .join(theme)
                .join(format!("{scene}.json")),
        )?);
    }
    bail!("unknown resource URI {uri:?}")
}

// ---------------------------------------------------------------------------
// Local renderer bridge
// ---------------------------------------------------------------------------

struct SessionHost {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next_id: u64,
}

impl SessionHost {
    fn start(root: &Path) -> Result<Self> {
        let mut child = Command::new(env!("CARGO"))
            .args([
                "run",
                "--quiet",
                "--locked",
                "--manifest-path",
                "tools/headless-visual/Cargo.toml",
                "--",
                "serve",
            ])
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("start headless-visual serve")?;
        let input = child.stdin.take().context("headless host has no stdin")?;
        let output = child.stdout.take().context("headless host has no stdout")?;
        Ok(Self {
            child,
            input,
            output: BufReader::new(output),
            next_id: 1,
        })
    }

    fn request(&mut self, method: &str, params: &Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        writeln!(
            self.input,
            "{}",
            json!({ "id": id, "method": method, "params": params })
        )?;
        self.input.flush()?;
        let mut line = String::new();
        ensure!(
            self.output.read_line(&mut line)? > 0,
            "headless session host exited"
        );
        let response: Value = serde_json::from_str(&line)?;
        ensure!(response["id"] == id, "headless session response id differs");
        ensure!(
            response["ok"].as_bool() == Some(true),
            "{}",
            response["error"]
                .as_str()
                .unwrap_or("headless session failed")
        );
        Ok(response["result"].clone())
    }
}

impl Drop for SessionHost {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn snapshot_once(host: &mut SessionHost, scene: &str, theme: &str) -> Result<Value> {
    let opened = host.request("open", &json!({ "scene": scene, "theme": theme }))?;
    let session = opened["session"]
        .as_str()
        .context("headless open returned no session")?
        .to_string();
    let snapshot = host.request("snapshot", &json!({ "session": session }))?;
    host.request("close", &json!({ "session": session }))?;
    Ok(snapshot)
}

fn render_live(root: &Path, name: &str, theme: &str) -> Result<(Vec<u8>, String)> {
    let out = root
        .join("target")
        .join("mcp")
        .join(format!("{name}-{theme}.png"));
    std::fs::create_dir_all(out.parent().expect("render has a directory"))?;
    let output = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "-p", "gpui-box-gallery", "--", "--scene"])
        .arg(name)
        .arg("--theme")
        .arg(theme)
        .arg("--capture")
        .arg(&out)
        .current_dir(root)
        .output()
        .context("run the GPUI gallery")?;
    ensure!(
        output.status.success(),
        "rendering {name} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok((std::fs::read(&out)?, out.display().to_string()))
}

fn ensure_scene(catalog: &Catalog, name: &str) -> Result<()> {
    ensure!(
        catalog
            .array("scenes")
            .iter()
            .any(|scene| string(scene, "name") == name),
        "unknown scene {name:?}"
    );
    Ok(())
}

fn theme(arguments: &Value) -> Result<&str> {
    match arguments
        .get("theme")
        .and_then(Value::as_str)
        .unwrap_or("studio-dark")
    {
        theme @ ("studio-dark" | "studio-light") => Ok(theme),
        other => bail!("unknown theme {other:?}: expected studio-dark or studio-light"),
    }
}

// ---------------------------------------------------------------------------
// Small response and path helpers
// ---------------------------------------------------------------------------

fn text_result(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

fn structured_result(structured: Value, text: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": text.into() }],
        "structuredContent": structured
    })
}

fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("missing {key}"))
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

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

fn root() -> Result<PathBuf> {
    if let Ok(set) = std::env::var("GPUI_BOX_ROOT") {
        return checked_root(PathBuf::from(set));
    }
    let current = std::env::current_dir().context("read the current directory")?;
    find_root(&current).with_context(|| {
        format!(
            "could not find a GPUI Box checkout above {}; set GPUI_BOX_ROOT",
            current.display()
        )
    })
}

fn checked_root(root: PathBuf) -> Result<PathBuf> {
    find_root(&root)
        .filter(|found| found == &root)
        .with_context(|| format!("GPUI_BOX_ROOT={} is not a checkout root", root.display()))
}

fn find_root(start: &Path) -> Option<PathBuf> {
    start.ancestors().find_map(|candidate| {
        (candidate.join("package-authority.toml").is_file()
            && candidate.join("docs/api-index.json").is_file()
            && candidate.join("docs/developer-index.json").is_file())
        .then(|| candidate.to_path_buf())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc_4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn tool_surfaces_are_deliberately_different() {
        assert_eq!(
            parse_tool_list(REMOTE_TOOLS).expect("remote tools").len(),
            10
        );
        assert_eq!(parse_tool_list(LOCAL_TOOLS).expect("local tools").len(), 7);
        for tool in parse_tool_list(REMOTE_TOOLS)
            .expect("remote tools")
            .into_iter()
            .chain(parse_tool_list(LOCAL_TOOLS).expect("local tools"))
        {
            assert!(tool["name"].is_string(), "{tool}");
            assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        }
    }

    #[test]
    fn complete_search_reaches_framework_symbols() {
        let catalog = Catalog::local(root().expect("checkout")).expect("catalog");
        let result = search(
            &catalog,
            &json!({ "query": "Window", "kinds": ["symbol"], "package": "gpui-box", "limit": 100 }),
            false,
        )
        .expect("search");
        assert!(
            result["matches"].as_array().is_some_and(|items| items
                .iter()
                .any(|item| string(item, "id").contains("Window"))),
            "{result}"
        );
    }

    #[test]
    fn resources_read_guides_and_components() {
        let catalog = Catalog::local(root().expect("checkout")).expect("catalog");
        let (mime, guide) = resource(&catalog, "gpui-box://guides/motion").expect("guide");
        assert_eq!(mime, "text/markdown");
        assert!(guide.contains("# Motion"));
        let (_, component) = resource(&catalog, "gpui-box://components/Button").expect("button");
        assert!(component.contains("Button"));
    }

    #[test]
    fn http_host_and_origin_are_pinned() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::HOST,
            "gpui-box.origingame.dev".parse().expect("host"),
        );
        validate_http_headers(&headers).expect("production host");
        headers.insert(
            header::ORIGIN,
            "https://evil.example".parse().expect("origin"),
        );
        assert!(validate_http_headers(&headers).is_err());
    }

    #[test]
    fn notifications_get_no_response() {
        let catalog = Catalog::local(root().expect("checkout")).expect("catalog");
        let mut server = Server::new(catalog);
        assert!(
            server
                .response(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
                .is_none()
        );
    }

    #[test]
    fn markdown_recipe_stops_at_the_next_section() {
        let body = "# Guide\n\n## One\nA\n\n## Two\nB\n";
        assert_eq!(markdown_section(body, "One"), "## One\nA");
    }
}
