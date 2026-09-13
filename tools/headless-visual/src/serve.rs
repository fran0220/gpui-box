//! A long-lived session host an agent can drive without opening a window.
//!
//! The process reads one JSON object per stdin line and writes one JSON object
//! per stdout line. Diagnostics go to stderr so a caller can parse every
//! stdout line as a reply. Each session is one offscreen window showing one
//! scene; the semantic tree, input injection, and screenshots all come from
//! that window after it has been drawn.
//! `open` accepts optional `width` and `height` in logical pixels (1–4096).
//! Omitted dimensions retain the canonical 920×1000 defaults; this does not
//! change the capture/check baseline viewport. The reply reports the actual
//! logical viewport and scale factor, not a claim about a native mobile device.
//! Local-only playback: `motion {session,reduced_motion:false}` opts one session
//! into motion. `frame {session,ms,path?}` advances exactly that simulated duration
//! and captures the resulting draw without settling. The application clock and
//! reduced-motion setting are global, so playback requires one exclusive session.
//! Normal `open`/`screenshot` and catalog baselines retain their settling defaults.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use gpui::{
    AnyWindowHandle, App, Context, HeadlessAppContext, InputEvent, IntoElement, Keystroke,
    Modifiers, MouseButton, MouseCancelEvent, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    NavigationDirection, PlatformInput, Render, ScrollDelta, ScrollWheelEvent, TouchEvent, TouchId,
    TouchPhase, Window, div, point, prelude::*, px, size,
};
use gpui_kit::prelude::set_layout_direction;
use gpui_kit_semantics::{DiagnosticArm, SemanticCoordinator};
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
    playback: Option<String>,
    time_ms: u64,
    _diagnostics: DiagnosticArm,
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
        let diagnostics = cx.update(|cx| SemanticCoordinator::global(cx).arm());
        Ok(Self {
            cx,
            sessions: HashMap::new(),
            next_id: 1,
            playback: None,
            time_ms: 0,
            _diagnostics: diagnostics,
        })
    }

    fn dispatch(&mut self, method: &str, params: &Value) -> Result<Value> {
        match method {
            "open" => self.open(params),
            "snapshot" => self.snapshot(params),
            "act" => self.act(params),
            "advance" => self.advance(params),
            "motion" => self.motion(params),
            "frame" => self.frame(params),
            "screenshot" => self.screenshot(params),
            "audit" => self.audit(params),
            "close" => self.close(params),
            "ping" => Ok(json!({})),
            other => bail!("unknown method: {other}"),
        }
    }

    fn open(&mut self, params: &Value) -> Result<Value> {
        anyhow::ensure!(
            self.playback.is_none(),
            "close the playback session or restore reduced motion before opening another session"
        );
        let viewport = requested_viewport(params)?;
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
        let handle = self.cx.open_window(viewport, {
            let scene = scene.to_owned();
            move |_, cx: &mut App| cx.new(|_| Host { scene: Some(scene) })
        })?;
        let window = handle.into();
        self.settle(window)?;
        let viewport = self.cx.update_window(window, |_, window, _| {
            json!({
                "width": f32::from(window.viewport_size().width),
                "height": f32::from(window.viewport_size().height),
                "scale_factor": window.scale_factor(),
            })
        })?;
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
            "viewport": viewport,
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
            "pointer_move" | "pointer_down" | "pointer_up" | "pointer_cancel" | "wheel"
            | "touch" => {
                let event = raw_input(action)?;
                self.cx.update_window(session.window, |_, window, cx| {
                    window.dispatch_event(event, cx);
                })?;
            }
            "touch_cancel_all" => {
                self.cx.update_window(session.window, |_, window, cx| {
                    window.cancel_touch_input(cx);
                })?;
            }
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
                let modifiers = input_modifiers(action)?;
                let at = self.point_in(session.window, id)?;
                self.cx.update_window(session.window, |_, window, cx| {
                    window.dispatch_event(
                        ScrollWheelEvent {
                            position: at,
                            delta: ScrollDelta::Pixels(point(px(0.0), px(-(pixels as f32)))),
                            modifiers,
                            touch_phase: TouchPhase::Moved,
                        }
                        .to_platform_input(),
                        cx,
                    );
                })?;
            }
            other => bail!("unknown action {other:?}"),
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
        self.time_ms = self
            .time_ms
            .checked_add(ms)
            .context("simulated clock overflow")?;
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
        Self::save_image(params, &frame)
    }

    fn motion(&mut self, params: &Value) -> Result<Value> {
        let session = self.lookup(params)?;
        let reduced = params
            .get("reduced_motion")
            .and_then(Value::as_bool)
            .context("motion needs boolean reduced_motion")?;
        anyhow::ensure!(
            reduced || self.sessions.len() == 1,
            "motion playback requires exactly one open session; use a separate serve process for concurrent playback"
        );
        self.playback = if reduced {
            None
        } else {
            Some(required_str(params, "session")?.to_owned())
        };
        self.activate(&session.scene, &session.theme)?;
        self.cx.update(|cx| cx.set_reduce_motion(reduced));
        self.draw(session.window)?;
        Ok(
            json!({"time_ms":self.time_ms,"reduced_motion":reduced,"generation":self.generation(session.window)?}),
        )
    }

    fn frame(&mut self, params: &Value) -> Result<Value> {
        let session = self.lookup(params)?;
        // Reuse scheduling, never settled_image: intermediate pixels are the
        // result being requested, not a failure to wait for identical images.
        self.advance(params)?;
        let frame = self.cx.capture_screenshot(session.window)?;
        let mut result = Self::save_image(params, &frame)?;
        result["time_ms"] = json!(self.time_ms);
        result["reduced_motion"] = json!(self.cx.update(|cx| cx.reduce_motion()));
        result["generation"] = json!(self.generation(session.window)?);
        result["snapshot"] = self.cx.update(|cx| {
            serde_json::to_value(
                SemanticCoordinator::global(cx)
                    .snapshot(session.window.window_id())
                    .expect("sampled frame published semantics")
                    .redacted(),
            )
        })?;
        Ok(result)
    }

    fn save_image(params: &Value, frame: &image::RgbaImage) -> Result<Value> {
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
        if self.playback.as_deref() == Some(&id) {
            self.playback = None;
            self.cx.update(|cx| cx.set_reduce_motion(true));
        }
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

    fn point_in(&mut self, window: AnyWindowHandle, id: &str) -> Result<gpui::Point<gpui::Pixels>> {
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

// Logical window coordinates may lie outside the viewport while captured.
// Bound conversion before f64 -> f32; invalid requests never dispatch input.
fn bounded_pixel(value: &Value, key: &str) -> Result<f32> {
    let number = value
        .get(key)
        .and_then(Value::as_f64)
        .with_context(|| format!("{key} needs a number"))?;
    anyhow::ensure!(
        number.is_finite() && number.abs() <= 16384.,
        "{key} must be finite and within -16384..=16384 logical pixels"
    );
    Ok(number as f32)
}

fn input_modifiers(value: &Value) -> Result<Modifiers> {
    let mut modifiers = Modifiers::none();
    if let Some(names) = value.get("modifiers") {
        for name in names.as_array().context("modifiers needs an array")? {
            match name.as_str() {
                Some("shift") => modifiers.shift = true,
                Some("control") => modifiers.control = true,
                Some("alt") => modifiers.alt = true,
                Some("platform") => modifiers.platform = true,
                Some("function") => modifiers.function = true,
                _ => bail!("unknown modifier {name}"),
            }
        }
    }
    Ok(modifiers)
}

fn input_button(value: &Value, key: &str) -> Result<MouseButton> {
    match required_str(value, key)? {
        "left" => Ok(MouseButton::Left),
        "right" => Ok(MouseButton::Right),
        "middle" => Ok(MouseButton::Middle),
        "back" => Ok(MouseButton::Navigate(NavigationDirection::Back)),
        "forward" => Ok(MouseButton::Navigate(NavigationDirection::Forward)),
        other => bail!("unknown button {other}"),
    }
}

fn raw_input(action: &Value) -> Result<PlatformInput> {
    let kind = required_str(action, "type")?;
    if kind == "pointer_cancel" {
        return Ok(MouseCancelEvent.to_platform_input());
    }
    let position = point(
        px(bounded_pixel(action, "x")?),
        px(bounded_pixel(action, "y")?),
    );
    let modifiers = input_modifiers(action)?;
    Ok(match kind {
        "pointer_move" => MouseMoveEvent {
            position,
            modifiers,
            pressed_button: action
                .get("pressed_button")
                .filter(|v| !v.is_null())
                .map(|_| input_button(action, "pressed_button"))
                .transpose()?,
        }
        .to_platform_input(),
        "pointer_down" => MouseDownEvent {
            position,
            modifiers,
            button: input_button(action, "button")?,
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        "pointer_up" => MouseUpEvent {
            position,
            modifiers,
            button: input_button(action, "button")?,
            click_count: 1,
        }
        .to_platform_input(),
        "wheel" => ScrollWheelEvent {
            position,
            modifiers,
            delta: ScrollDelta::Pixels(point(
                px(bounded_pixel(action, "delta_x")?),
                px(bounded_pixel(action, "delta_y")?),
            )),
            touch_phase: TouchPhase::Moved,
        }
        .to_platform_input(),
        "touch" => TouchEvent {
            id: TouchId(
                action
                    .get("touch_id")
                    .and_then(Value::as_u64)
                    .context("touch_id needs u64")?,
            ),
            phase: match required_str(action, "phase")? {
                "started" => TouchPhase::Started,
                "moved" => TouchPhase::Moved,
                "ended" => TouchPhase::Ended,
                "cancelled" => TouchPhase::Cancelled,
                other => bail!("unknown touch phase {other}"),
            },
            position,
            predicted_position: None,
            force: None,
        }
        .to_platform_input(),
        other => bail!("unknown raw input {other}"),
    })
}

fn requested_viewport(params: &Value) -> Result<gpui::Size<gpui::Pixels>> {
    let dimension = |key: &str, default: f32| -> Result<gpui::Pixels> {
        let Some(value) = params.get(key) else {
            return Ok(px(default));
        };
        let value = value
            .as_f64()
            .with_context(|| format!("{key} must be a number in logical pixels"))?;
        anyhow::ensure!(
            value.is_finite() && (1.0..=4096.0).contains(&value),
            "{key} must be between 1 and 4096 logical pixels"
        );
        Ok(px(value as f32))
    };
    Ok(size(
        dimension("width", 920.0)?,
        dimension("height", 1000.0)?,
    ))
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
    fn motion_restores_selected_session_theme_before_drawing() -> Result<()> {
        let mut server = Server::new()?;
        let dark = server.open(&json!({"scene":"divider","theme":"studio-dark"}))?;
        let dark_session = server.lookup(&json!({"session":dark["session"]}))?;
        let expected = server.cx.capture_screenshot(dark_session.window)?;
        let light = server.open(&json!({"scene":"divider","theme":"studio-light"}))?;
        let light_session = server.lookup(&json!({"session":light["session"]}))?;
        let different = server.cx.capture_screenshot(light_session.window)?;
        assert_ne!(expected.as_raw(), different.as_raw());
        server.motion(&json!({"session":dark["session"],"reduced_motion":true}))?;
        assert_eq!(
            server.cx.capture_screenshot(dark_session.window)?.as_raw(),
            expected.as_raw()
        );
        server.snapshot(&json!({"session":light["session"]}))?;
        server.close(&json!({"session":light["session"]}))?;
        server.motion(&json!({"session":dark["session"],"reduced_motion":false}))?;
        assert_eq!(
            server.cx.capture_screenshot(dark_session.window)?.as_raw(),
            expected.as_raw()
        );
        Ok(())
    }

    #[test]
    fn raw_coordinates_buttons_modifiers_and_touch_are_explicit() -> Result<()> {
        let event = raw_input(&json!({"type":"wheel","x":-43.5,"y":701.25,
            "delta_x":17,"delta_y":-83,"modifiers":["control","shift"]}))?;
        let PlatformInput::ScrollWheel(event) = event else {
            panic!("wheel event")
        };
        assert_eq!(event.position, point(px(-43.5), px(701.25)));
        assert_eq!(event.delta.pixel_delta(px(20.)), point(px(17.), px(-83.)));
        assert!(event.modifiers.control && event.modifiers.shift && !event.modifiers.alt);
        let PlatformInput::MouseMove(event) = raw_input(
            &json!({"type":"pointer_move","x":16384,"y":-16384,"pressed_button":"right"}),
        )?
        else {
            panic!("move event")
        };
        assert_eq!(event.pressed_button, Some(MouseButton::Right));
        for action in [
            json!({"type":"pointer_move","x":16385,"y":0}),
            json!({"type":"pointer_move","x":0,"y":-16385}),
            json!({"type":"pointer_move","x":"1","y":0}),
            json!({"type":"pointer_down","x":0,"y":0,"button":"unknown"}),
            json!({"type":"pointer_move","x":0,"y":0,"modifiers":["ctrl"]}),
            json!({"type":"touch","x":0,"y":0,"touch_id":-1,"phase":"started"}),
        ] {
            assert!(raw_input(&action).is_err(), "{action}");
        }
        let PlatformInput::Touch(event) = raw_input(
            &json!({"type":"touch","x":31,"y":79,"touch_id":18446744073709551615u64,"phase":"cancelled"}),
        )?
        else {
            panic!("touch event")
        };
        assert_eq!(event.id, TouchId(u64::MAX));
        assert_eq!(event.phase, TouchPhase::Cancelled);
        assert!(event.force.is_none() && event.predicted_position.is_none());
        Ok(())
    }

    #[test]
    fn raw_dispatch_captures_outside_releases_cancels_and_respects_refusal() -> Result<()> {
        use gpui_kit::display::chart::{
            cartesian::{CartesianChart, CartesianEvent},
            data::{ChartScale, ChartValue, RawPoint, RawSeries, SeriesMark, ValueAxis},
            scale::{NumericScale, ScaleKind},
        };
        use std::{cell::RefCell, rc::Rc};
        struct Probe(Rc<RefCell<Vec<CartesianEvent>>>);
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                SemanticCoordinator::global(cx).begin_frame(window);
                let events = self.0.clone();
                let scale =
                    NumericScale::new(ScaleKind::Linear, [0., 100.]).expect("fixture domain");
                div().w(px(300.)).child(
                    CartesianChart::new(
                        "probe",
                        "Probe",
                        ChartScale::Numeric(scale),
                        [ValueAxis {
                            id: "y".into(),
                            label: "Units".into(),
                            scale,
                        }],
                    )
                    .series([RawSeries::new("readings", "y", SeriesMark::Scatter)
                        .points([RawPoint::new("west", ChartValue::Number(25.), Some(40.))
                            .text("West", "40")])])
                    .on_event(move |event, _, _| events.borrow_mut().push(event)),
                )
            }
        }
        let mut server = Server::new()?;
        let events = Rc::new(RefCell::new(Vec::new()));
        let logging = events.clone();
        let window = server
            .cx
            .open_window(size(px(400.), px(350.)), move |_, cx| {
                cx.new(|_| Probe(logging))
            })?
            .into();
        server.sessions.insert(
            "probe".into(),
            Session {
                window,
                scene: "button".into(),
                theme: "studio-light".into(),
            },
        );
        server.draw(window)?;
        server.draw(window)?;
        let at = server.point_in(window, "probe.series.readings.point.west")?;
        let x = f32::from(at.x);
        let y = f32::from(at.y);
        let send = |server: &mut Server, mut action: Value| -> Result<()> {
            action["session"] = json!("probe");
            server.act(&action)?;
            Ok(())
        };
        send(&mut server, json!({"type":"pointer_move","x":x,"y":y}))?;
        assert!(
            events
                .borrow()
                .iter()
                .any(|e| matches!(e, CartesianEvent::Hover(Some(_))))
        );
        send(
            &mut server,
            json!({"type":"wheel","x":x,"y":y,"delta_x":17,"delta_y":-83,"modifiers":["control"]}),
        )?;
        assert!(
            events
                .borrow()
                .iter()
                .any(|e| matches!(e, CartesianEvent::Viewport(_)))
        );
        assert_eq!(
            server.point_in(window, "probe.series.readings.point.west")?,
            at,
            "host refused viewport; source geometry unchanged"
        );
        events.borrow_mut().clear();
        send(
            &mut server,
            json!({"type":"pointer_down","x":x,"y":y,"button":"left"}),
        )?;
        send(
            &mut server,
            json!({"type":"pointer_move","x":-73,"y":-39,"pressed_button":"left"}),
        )?;
        assert!(
            events
                .borrow()
                .iter()
                .any(|e| matches!(e, CartesianEvent::Viewport(_)))
        );
        assert_eq!(
            server.point_in(window, "probe.series.readings.point.west")?,
            at,
            "refused pan redraw remains controlled"
        );
        send(
            &mut server,
            json!({"type":"pointer_up","x":-73,"y":-39,"button":"right"}),
        )?;
        assert!(
            server
                .cx
                .update_window(window, |_, w, _| w.captured_hitbox().is_some())?,
            "unrelated release must not end left capture"
        );
        send(&mut server, json!({"type":"pointer_cancel"}))?;
        assert!(
            !server
                .cx
                .update_window(window, |_, w, _| w.captured_hitbox().is_some())?
        );
        events.borrow_mut().clear();
        for cancel in [false, true] {
            send(
                &mut server,
                json!({"type":"pointer_down","x":x,"y":y,"button":"left","modifiers":["shift"]}),
            )?;
            send(
                &mut server,
                json!({"type":"pointer_move","x":900,"y":700,"pressed_button":"left","modifiers":["shift"]}),
            )?;
            assert!(
                server
                    .cx
                    .update_window(window, |_, w, _| w.captured_hitbox().is_some())?
            );
            let preview = server.frame(
                &json!({"session":"probe","ms":0,"path":"target/sessions/raw-input-preview.png"}),
            )?;
            assert!(
                preview["snapshot"]["nodes"]
                    .as_array()
                    .expect("nodes")
                    .iter()
                    .any(|n| n["id"] == "probe.brush-preview")
            );
            if cancel {
                send(&mut server, json!({"type":"pointer_cancel"}))?;
            }
            send(
                &mut server,
                json!({"type":"pointer_up","x":900,"y":700,"button":"left"}),
            )?;
            assert!(
                !server
                    .cx
                    .update_window(window, |_, w, _| w.captured_hitbox().is_some())?
            );
            let brushes = events
                .borrow()
                .iter()
                .filter(|e| matches!(e, CartesianEvent::Brush(_)))
                .count();
            assert_eq!(
                brushes,
                usize::from(!cancel),
                "cancellation never commits even with a later release"
            );
            if !cancel {
                let logged = events.borrow();
                let Some(CartesianEvent::Brush(
                    [ChartValue::Number(start), ChartValue::Number(end)],
                )) = logged
                    .iter()
                    .find(|e| matches!(e, CartesianEvent::Brush(_)))
                else {
                    panic!("numeric brush proposal")
                };
                assert!((*start - 25.).abs() < 0.1);
                assert_eq!(
                    *end, 100.,
                    "component bounds brush, not harness coordinates"
                );
            }
            events.borrow_mut().clear();
        }
        for phase in ["started", "cancelled"] {
            send(
                &mut server,
                json!({"type":"touch","x":x,"y":y,"touch_id":53,"phase":phase}),
            )?;
        }
        send(&mut server, json!({"type":"touch_cancel_all"}))?;
        assert!(!events.borrow().iter().any(|e| matches!(
            e,
            CartesianEvent::Select(Some(_)) | CartesianEvent::Brush(_)
        )));
        Ok(())
    }

    #[test]
    fn playback_samples_interruption_and_reduced_motion_without_settling() -> Result<()> {
        let mut server = Server::new()?;
        let opened = server.open(&json!({"scene":"motion-primitives", "theme":"studio-light"}))?;
        let id = &opened["session"];
        let click = |target: &str| json!({"session":id,"type":"click","id":target});
        server.act(&click("scene.motion.tabs.spring"))?;
        let indicator_x = |snapshot: &Value| -> f64 {
            snapshot["nodes"]
                .as_array()
                .expect("semantic nodes")
                .iter()
                .find(|node| node["id"] == "scene.motion.spring.indicator")
                .expect("spring indicator")["bounds"]["x"]
                .as_f64()
                .expect("indicator x")
        };
        let left = indicator_x(&server.snapshot(&json!({"session":id}))?);
        server.act(&click("scene.motion.spring.timeline"))?;
        let right = indicator_x(&server.snapshot(&json!({"session":id}))?);
        assert!(
            (right - left - 120.0).abs() < 0.1,
            "default reduced motion settles immediately"
        );
        let params = |ms| json!({"session":id,"ms":ms,"path":"target/sessions/playback-test.png"});
        let default = server.frame(&params(0))?;
        assert_eq!(default["reduced_motion"], true);
        assert_eq!(default["time_ms"], 0);
        server.motion(&json!({"session":id,"reduced_motion":false}))?;
        assert!(server.open(&json!({"scene":"button"})).is_err());
        server.act(&click("scene.motion.spring.queue"))?;
        let start = server.frame(&params(0))?;
        assert_eq!(indicator_x(&start["snapshot"]), right);
        let moving = server.frame(&params(80))?;
        let middle = indicator_x(&moving["snapshot"]);
        assert!(
            middle > left && middle < right,
            "intermediate geometry {left} < {middle} < {right}"
        );
        assert_ne!(
            start["png_base64"], moving["png_base64"],
            "actual rendered intermediate frame"
        );
        assert_eq!(moving["time_ms"], 80);
        server.act(&click("scene.motion.spring.timeline"))?;
        let interrupted = server.frame(&params(0))?;
        assert!(
            (indicator_x(&interrupted["snapshot"]) - middle).abs() < 0.1,
            "retarget preserves current geometry"
        );
        let resumed = server.frame(&params(80))?;
        assert_eq!(resumed["time_ms"], 160);
        assert_ne!(resumed["png_base64"], interrupted["png_base64"]);
        server.motion(&json!({"session":id,"reduced_motion":true}))?;
        let settled = server.frame(&params(0))?;
        assert_eq!(indicator_x(&settled["snapshot"]), right);
        assert_eq!(
            settled["time_ms"], 160,
            "reduced motion does not secretly advance time"
        );
        assert_eq!(settled["reduced_motion"], true);
        let other = server.open(&json!({"scene":"button"}))?;
        assert!(
            server
                .motion(&json!({"session":id,"reduced_motion":false}))
                .is_err()
        );
        server.close(&json!({"session":other["session"]}))?;
        server.motion(&json!({"session":id,"reduced_motion":false}))?;
        server.close(&json!({"session":id}))?;
        assert!(server.cx.update(|cx| cx.reduce_motion()));
        Ok(())
    }

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
        assert_eq!(opened["viewport"]["width"], 920.0);
        assert_eq!(opened["viewport"]["height"], 1000.0);
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

    #[test]
    fn requested_dimensions_reject_invalid_values_before_opening() {
        assert_eq!(
            requested_viewport(&json!({"width": 361})).unwrap(),
            size(px(361.0), px(1000.0))
        );
        assert_eq!(
            requested_viewport(&json!({"height": 701})).unwrap(),
            size(px(920.0), px(701.0))
        );
        for value in [
            json!(0),
            json!(-1),
            json!(4097),
            json!(0.5),
            json!("390"),
            Value::Null,
        ] {
            for key in ["width", "height"] {
                let mut params = json!({});
                params[key] = value.clone();
                assert!(requested_viewport(&params).is_err(), "{params}");
            }
        }
    }

    #[test]
    fn narrow_asymmetric_viewport_is_the_rendered_window() -> Result<()> {
        let mut server = Server::new()?;
        for (width, height) in [(390, 844), (361, 701)] {
            let opened = server.dispatch(
                "open",
                &json!({"scene": "button", "width": width, "height": height}),
            )?;
            assert_eq!(opened["viewport"]["width"], width as f64);
            assert_eq!(opened["viewport"]["height"], height as f64);
            let session = server.lookup(&json!({"session": opened["session"]}))?;
            let frame = server.settled_image(session.window)?;
            let scale = opened["viewport"]["scale_factor"].as_f64().unwrap();
            assert_eq!(
                frame.dimensions(),
                (
                    (width as f64 * scale) as u32,
                    (height as f64 * scale) as u32
                )
            );
            server.dispatch("close", &json!({"session": opened["session"]}))?;
        }
        Ok(())
    }
}
