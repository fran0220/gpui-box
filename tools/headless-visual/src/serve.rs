//! A long-lived session host an agent can drive without opening a window.
//!
//! The process reads one JSON object per stdin line and writes one JSON object
//! per stdout line. Diagnostics go to stderr so a caller can parse every
//! stdout line as a reply. Each session is one offscreen window showing one
//! scene; the semantic tree, input injection, and screenshots all come from
//! that window after it has been drawn.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use gpui::{
    AnyWindowHandle, App, Context, HeadlessAppContext, InputEvent, IntoElement, Keystroke,
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Render, ScrollDelta, ScrollWheelEvent,
    TouchPhase, Window, div, point, prelude::*, px, size,
};
use gpui_kit::prelude::set_layout_direction;
use gpui_kit_semantics::SemanticCoordinator;
use gpui_kit_testkit::audit_or_error;
use gpui_kit_theme::{Theme, activate_theme};
use serde_json::{Value, json};

pub fn run() -> Result<()> {
    let mut server = Server::new()?;
    eprintln!("headless-visual serve ready");
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
                eprintln!("headless-visual serve: unreadable request: {error}");
                continue;
            }
        };
        let Some(id) = request.get("id").cloned() else {
            continue;
        };
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or(json!({}));
        let response = match server.dispatch(method, &params) {
            Ok(result) => json!({ "id": id, "ok": true, "result": result }),
            Err(error) => json!({ "id": id, "ok": false, "error": error.to_string() }),
        };
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}

struct Host {
    scene: Option<String>,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SemanticCoordinator::global(cx).begin_frame(window);
        let theme = Theme::get(cx).clone();
        let root = div().size_full().bg(theme.colors.canvas);
        let Some(name) = self.scene.as_deref() else {
            return root;
        };
        let scene = gpui_kit::scenes::find(name).expect("open already checked the catalog");
        root.child((scene.build)(window, cx))
    }
}

#[derive(Clone)]
struct Session {
    window: AnyWindowHandle,
    scene: String,
    theme: String,
}

struct Server {
    cx: HeadlessAppContext,
    sessions: HashMap<String, Session>,
    next_id: u64,
}

impl Server {
    fn new() -> Result<Self> {
        let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
            "Geist",
        ));
        let mut cx = HeadlessAppContext::with_platform(
            text_system,
            Arc::new(gpui_kit::assets::Assets),
            gpui_platform::current_headless_renderer,
        );
        cx.update(|cx| {
            gpui_kit::install(cx);
            cx.set_reduce_motion(true);
        });
        Ok(Self {
            cx,
            sessions: HashMap::new(),
            next_id: 1,
        })
    }

    fn dispatch(&mut self, method: &str, params: &Value) -> Result<Value> {
        match method {
            "open" => self.open(params),
            "snapshot" => self.snapshot(params),
            "act" => self.act(params),
            "advance" => self.advance(params),
            "screenshot" => self.screenshot(params),
            "audit" => self.audit(params),
            "close" => self.close(params),
            "ping" => Ok(json!({})),
            other => bail!("unknown method: {other}"),
        }
    }

    fn open(&mut self, params: &Value) -> Result<Value> {
        let scene = required_str(params, "scene")?;
        let theme = match params.get("theme").and_then(Value::as_str).unwrap_or("") {
            "" => "studio-dark",
            "studio-dark" | "studio-light" => params
                .get("theme")
                .and_then(Value::as_str)
                .unwrap_or("studio-dark"),
            other => bail!("unknown theme {other:?}: expected studio-dark or studio-light"),
        };
        if gpui_kit::scenes::find(scene).is_none() {
            bail!("unknown scene `{scene}`");
        }
        self.activate(scene, theme)?;
        let handle = self.cx.open_window(size(px(920.0), px(1000.0)), {
            let scene = scene.to_owned();
            move |_, cx: &mut App| cx.new(|_| Host { scene: Some(scene) })
        })?;
        let window = handle.into();
        self.settle(window)?;
        let id = format!("s{}", self.next_id);
        self.next_id += 1;
        self.sessions.insert(
            id.clone(),
            Session {
                window,
                scene: scene.to_owned(),
                theme: theme.to_owned(),
            },
        );
        Ok(json!({
            "session": id,
            "scene": scene,
            "theme": theme,
            "generation": self.generation(window)?,
        }))
    }

    fn snapshot(&mut self, params: &Value) -> Result<Value> {
        let session = self.lookup(params)?;
        self.activate(&session.scene, &session.theme)?;
        self.draw(session.window)?;
        let snapshot = self.cx.update(|cx| {
            SemanticCoordinator::global(cx)
                .snapshot(session.window.window_id())
                .expect("draw published this window's semantics")
                .redacted()
        });
        Ok(serde_json::to_value(snapshot)?)
    }

    fn act(&mut self, params: &Value) -> Result<Value> {
        let session = self.lookup(params)?;
        self.activate(&session.scene, &session.theme)?;
        self.draw(session.window)?;
        let action = params.get("action").unwrap_or(params);
        let kind = action
            .get("type")
            .and_then(Value::as_str)
            .context("act needs a type")?;
        match kind {
            "click" => {
                let id = required_str(action, "id")?;
                let at = self.point_in(session.window, id)?;
                self.cx.update_window(session.window, |_, window, cx| {
                    window.dispatch_event(
                        MouseDownEvent {
                            position: at,
                            modifiers: Modifiers::none(),
                            button: MouseButton::Left,
                            click_count: 1,
                            first_mouse: false,
                        }
                        .to_platform_input(),
                        cx,
                    );
                    window.dispatch_event(
                        MouseUpEvent {
                            position: at,
                            modifiers: Modifiers::none(),
                            button: MouseButton::Left,
                            click_count: 1,
                        }
                        .to_platform_input(),
                        cx,
                    );
                })?;
            }
            "keystrokes" => {
                let keys = required_str(action, "keys")?;
                let strokes = keys
                    .split_whitespace()
                    .map(|token| {
                        Keystroke::parse(token).map_err(|error| {
                            anyhow::anyhow!("invalid keystroke `{token}`: {error}")
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                self.cx.update_window(session.window, |_, window, cx| {
                    for keystroke in strokes {
                        window.dispatch_keystroke(keystroke, cx);
                    }
                })?;
            }
            "text" => {
                let text = required_str(action, "text")?;
                self.cx.update_window(session.window, |_, window, cx| {
                    for character in text.chars() {
                        let key = character.to_string();
                        window.dispatch_keystroke(
                            Keystroke {
                                modifiers: Modifiers::default(),
                                key: key.clone(),
                                key_char: Some(key),
                            },
                            cx,
                        );
                    }
                })?;
            }
            "scroll" => {
                let id = required_str(action, "id")?;
                let pixels = action
                    .get("pixels")
                    .and_then(Value::as_f64)
                    .context("scroll needs pixels")?;
                let at = self.point_in(session.window, id)?;
                self.cx.update_window(session.window, |_, window, cx| {
                    window.dispatch_event(
                        ScrollWheelEvent {
                            position: at,
                            delta: ScrollDelta::Pixels(point(px(0.0), px(-(pixels as f32)))),
                            modifiers: Modifiers::none(),
                            touch_phase: TouchPhase::Moved,
                        }
                        .to_platform_input(),
                        cx,
                    );
                })?;
            }
            other => bail!("unknown action {other:?}: expected click, keystrokes, text, or scroll"),
        }
        self.cx.run_until_parked();
        self.draw(session.window)?;
        Ok(json!({ "generation": self.generation(session.window)? }))
    }

    fn advance(&mut self, params: &Value) -> Result<Value> {
        let session = self.lookup(params)?;
        let ms = params
            .get("ms")
            .and_then(Value::as_u64)
            .context("advance needs ms")?;
        self.activate(&session.scene, &session.theme)?;
        self.cx.advance_clock(Duration::from_millis(ms));
        self.cx.update_window(session.window, |_, window, cx| {
            window.simulate_next_frame(cx);
        })?;
        self.cx.run_until_parked();
        self.draw(session.window)?;
        Ok(json!({ "generation": self.generation(session.window)? }))
    }

    fn screenshot(&mut self, params: &Value) -> Result<Value> {
        let session = self.lookup(params)?;
        self.activate(&session.scene, &session.theme)?;
        let frame = self.settled_image(session.window)?;
        let path = match params.get("path").and_then(Value::as_str) {
            Some(path) => {
                let requested = PathBuf::from(path);
                if requested.is_absolute() {
                    requested
                } else {
                    repo_root().join(requested)
                }
            }
            None => repo_root()
                .join("target")
                .join("sessions")
                .join(format!("{}.png", required_str(params, "session")?)),
        };
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        frame
            .save(&path)
            .with_context(|| format!("write {}", path.display()))?;
        let png = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        Ok(json!({
            "path": path.display().to_string(),
            "bytes": png.len(),
            "png_base64": base64(&png),
        }))
    }

    fn audit(&mut self, params: &Value) -> Result<Value> {
        let session = self.lookup(params)?;
        self.activate(&session.scene, &session.theme)?;
        self.draw(session.window)?;
        let snapshot = self.cx.update(|cx| {
            SemanticCoordinator::global(cx)
                .snapshot(session.window.window_id())
                .expect("draw published this window's semantics")
        });
        match audit_or_error(&snapshot) {
            Ok(()) => Ok(json!({ "ok": true, "findings": [] })),
            Err(error) => Ok(json!({
                "ok": false,
                "findings": error
                    .findings
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            })),
        }
    }

    fn close(&mut self, params: &Value) -> Result<Value> {
        let id = required_str(params, "session")?.to_owned();
        let Some(session) = self.sessions.remove(&id) else {
            bail!("unknown session `{id}`");
        };
        self.cx
            .update_window(session.window, |_, window, _| window.remove_window())?;
        Ok(json!({}))
    }

    fn lookup(&self, params: &Value) -> Result<Session> {
        let id = required_str(params, "session")?;
        self.sessions
            .get(id)
            .cloned()
            .with_context(|| format!("unknown session `{id}`"))
    }

    fn activate(&mut self, scene: &str, theme: &str) -> Result<()> {
        let known = self.cx.update(|cx| {
            let known = activate_theme(theme, cx);
            if known {
                set_layout_direction(gpui_kit::scenes::direction(scene), cx);
            }
            known
        });
        if !known {
            bail!("unknown theme `{theme}`");
        }
        Ok(())
    }

    fn draw(&mut self, window: AnyWindowHandle) -> Result<()> {
        self.cx.run_until_parked();
        self.cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear(cx);
        })?;
        Ok(())
    }

    fn settle(&mut self, window: AnyWindowHandle) -> Result<()> {
        self.settled_image(window).map(|_| ())
    }

    fn settled_image(&mut self, window: AnyWindowHandle) -> Result<image::RgbaImage> {
        let mut previous: Option<image::RgbaImage> = None;
        for _ in 0..32 {
            self.cx.run_until_parked();
            self.cx.update_window(window, |_, window, cx| {
                window.draw(cx).clear(cx);
            })?;
            let frame = self.cx.capture_screenshot(window)?;
            if previous
                .as_ref()
                .is_some_and(|previous| previous.as_raw() == frame.as_raw())
            {
                return Ok(frame);
            }
            previous = Some(frame);
        }
        bail!("the scene did not settle within 32 draws")
    }

    fn generation(&mut self, window: AnyWindowHandle) -> Result<u64> {
        self.cx.update(|cx| {
            SemanticCoordinator::global(cx)
                .generation(window.window_id())
                .context("window has not published a semantic frame")
        })
    }

    fn point_in(
        &mut self,
        window: AnyWindowHandle,
        id: &str,
    ) -> Result<gpui::Point<gpui::Pixels>> {
        let snapshot = self.cx.update(|cx| {
            SemanticCoordinator::global(cx)
                .snapshot(window.window_id())
                .context("window has not published a semantic frame")
        })?;
        let node = snapshot
            .find(id)
            .with_context(|| format!("semantic node `{id}` is missing"))?;
        let (x, y) = node.bounds.center();
        Ok(point(px(x), px(y)))
    }
}

fn required_str<'a>(params: &'a Value, key: &str) -> Result<&'a str> {
    params
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{key} is required"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_specification() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
    }

    #[test]
    fn serve_protocol_drives_a_scene() -> Result<()> {
        let mut server = Server::new()?;
        let missing = server
            .dispatch(
                "open",
                &json!({ "scene": "does-not-exist", "theme": "studio-dark" }),
            )
            .expect_err("unknown scene");
        assert!(missing.to_string().contains("does-not-exist"), "{missing}");

        let theme = server
            .dispatch("open", &json!({ "scene": "button", "theme": "nope" }))
            .expect_err("unknown theme");
        assert!(theme.to_string().contains("nope"), "{theme}");

        let opened = server.dispatch(
            "open",
            &json!({ "scene": "button", "theme": "studio-dark" }),
        )?;
        let session = opened["session"].as_str().expect("session id").to_owned();
        assert!(opened["generation"].as_u64().unwrap_or(0) > 0);

        let snapshot = server.dispatch("snapshot", &json!({ "session": session }))?;
        assert!(
            snapshot["nodes"]
                .as_array()
                .is_some_and(|nodes| !nodes.is_empty()),
            "{snapshot}"
        );

        let audit = server.dispatch("audit", &json!({ "session": session }))?;
        assert_eq!(audit["ok"], true, "{audit}");

        let shot = server.dispatch("screenshot", &json!({ "session": session }))?;
        assert!(shot["bytes"].as_u64().unwrap_or(0) > 0, "{shot}");
        assert!(
            shot["png_base64"]
                .as_str()
                .is_some_and(|data| !data.is_empty()),
            "{shot}"
        );

        server.dispatch("close", &json!({ "session": session }))?;
        let closed = server
            .dispatch("snapshot", &json!({ "session": session }))
            .expect_err("closed session");
        assert!(closed.to_string().contains(&session), "{closed}");
        Ok(())
    }
}
