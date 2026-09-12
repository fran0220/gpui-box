//! Independently authored GPUI candidate, not a native reference or fitted material.
//! Usage: glass_reference REQUEST.json OUTPUT_DIRECTORY
//! Requests and parameters are produced/validated by gpui_capture.py. Transition
//! samples replay a persistent surface resize, not cross-view matched geometry.
use anyhow::{Result, bail, ensure};
use gpui::{
    AnyWindowHandle, App, Context, FontWeight, HeadlessAppContext, IntoElement, Render, Window,
    div, hsla, prelude::*, px, rgb, size,
};
use gpui_kit::motion::Animator;
use gpui_kit::prelude::{Glass, GlassGroup, GlassPreset, ThemeOverlay};
use gpui_kit_theme::{Radius, activate_theme};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    fs,
    path::Path,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

#[path = "glass_reference/observable.rs"]
mod observable;

struct Fixture {
    fixture: Value,
    parameters: Value,
    background: bool,
    dark: bool,
    resize: Option<Animator>,
    measured: Rc<Cell<([f32; 4], Option<Instant>)>>,
}

fn rect(v: &Value) -> [f32; 4] {
    std::array::from_fn(|i| v[i].as_f64().expect("validated rectangle") as f32)
}
fn positioned(r: [f32; 4]) -> gpui::Div {
    div()
        .absolute()
        .left(px(r[0]))
        .top(px(r[1]))
        .w(px(r[2]))
        .h(px(r[3]))
}
fn label(text: &str, w: f32, h: f32, dark: bool, semibold: bool) -> gpui::Div {
    div()
        .w(px(w))
        .h(px(h))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(if semibold { 15.0 } else { 17.0 }))
        .font_weight(if semibold {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .text_color(rgb(if dark { 0xffffff } else { 0x000000 }))
        .child(text.to_owned())
}
fn material(mut glass: Glass, p: &Value) -> Glass {
    for (key, value) in p.as_object().unwrap() {
        let v = value.as_f64().unwrap() as f32;
        glass = match key.as_str() {
            "blur" => glass.blur(v),
            "thickness" => glass.thickness(v),
            "refractive_index" => glass.refractive_index(v),
            "backdrop_depth" => glass.backdrop_depth(v),
            "protect_text" => glass.protect_text_contrast(v == 1.0),
            key if key.starts_with("regular_") => glass,
            _ => unreachable!("validated material option"),
        };
    }
    glass
}

// Trial-only theme scope: never wrap Clear, whose production preset shares
// some effects with Liquid. Explicit builder blur wins over this theme blur.
fn regular(child: impl IntoElement, p: &Value) -> gpui::AnyElement {
    if !p
        .as_object()
        .unwrap()
        .keys()
        .any(|k| k.starts_with("regular_"))
    {
        return child.into_any_element();
    }
    let p = p.clone();
    ThemeOverlay::new(
        move |t| {
            t.clone().modify(|t| {
                for (key, value) in p.as_object().unwrap() {
                    let v = value.as_f64().unwrap() as f32;
                    match key.as_str() {
                        "regular_blur" => t.effects.glass_frost_blur = v,
                        "regular_saturation" => t.effects.glass_saturation = v,
                        "regular_wash" => t.effects.glass_wash = v,
                        "regular_gain" => t.effects.glass_transmission_gain = v,
                        "regular_lift" => t.effects.glass_optical_lift = v,
                        "regular_hairline" => t.effects.glass_hairline = v,
                        "regular_specular" => t.effects.glass_specular = v,
                        "regular_refraction" => t.effects.glass_refraction = v,
                        "regular_thickness" => t.effects.glass_thickness = v,
                        "regular_backdrop_depth" => t.effects.glass_backdrop_depth = v,
                        "regular_refractive_index" => t.effects.glass_refractive_index = v,
                        _ => {}
                    }
                }
            })
        },
        child,
    )
    .into_any_element()
}

fn validate_parameters(parameters: &Value) -> Result<()> {
    let options = parameters
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("parameters must be object"))?;
    for (k, v) in options {
        let (lo, hi) = match k.as_str() {
            "blur" | "thickness" | "regular_blur" | "regular_thickness" => (0.0, 64.0),
            "refractive_index" | "regular_refractive_index" => (1.0, 2.5),
            "backdrop_depth" | "regular_backdrop_depth" => (0.0, 128.0),
            "regular_refraction" => (0.0, 1.0),
            "regular_saturation" => (0.0, 3.0),
            "regular_wash" | "regular_lift" | "regular_specular" | "protect_text" => (0.0, 1.0),
            "regular_hairline" => (0.0, 4.0),
            "regular_gain" => (0.0, 2.0),
            _ => bail!("unknown parameter"),
        };
        ensure!(
            v.as_f64().is_some_and(|v| v.is_finite()
                && v >= lo
                && v <= hi
                && (k != "protect_text" || v == 0.0 || v == 1.0)),
            "parameter out of bounds"
        );
    }
    Ok(())
}
impl Render for Fixture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
            .relative()
            .w(px(960.0))
            .h(px(640.0))
            .overflow_hidden()
            .font_family("Geist");
        // Canvas strokes are centered, not div borders inset into each cell.
        for y in 0..20 {
            for x in 0..30 {
                root = root.child(
                    positioned([x as f32 * 32.0, y as f32 * 32.0, 32.0, 32.0])
                        .bg(rgb(if (x + y) % 2 == 0 { 0xd9e6f2 } else { 0xf2cc99 })),
                );
            }
        }
        for x in 0..=30 {
            root =
                root.child(positioned([x as f32 * 32.0 - 0.5, 0.0, 1.0, 640.0]).bg(rgb(0x8090a0)));
        }
        for y in 0..=20 {
            root =
                root.child(positioned([0.0, y as f32 * 32.0 - 0.5, 960.0, 1.0]).bg(rgb(0x8090a0)));
        }
        root = root.child(
            positioned([0.0, 600.0, 960.0, 16.0])
                .flex()
                .items_center()
                .justify_center()
                .font_family("Geist Mono")
                .text_size(px(12.0))
                .text_color(rgb(0))
                .child("Glass reference 012345"),
        );
        for marker in self.fixture["fiducials"].as_array().unwrap() {
            let c = &marker["rgb"];
            root = root.child(
                positioned(rect(&marker["rect"])).bg(rgb((c[0].as_u64().unwrap() as u32) << 16
                    | (c[1].as_u64().unwrap() as u32) << 8
                    | c[2].as_u64().unwrap() as u32)),
            );
        }
        if !self.background {
            for pill in self.fixture["pills"].as_array().unwrap() {
                let r = rect(&pill["rect"]);
                let clear = pill["material"] == "clear";
                let tinted = pill["material"] == "tinted";
                let mut glass = material(
                    Glass::new(pill["id"].as_str().unwrap().to_owned())
                        .preset(if clear {
                            GlassPreset::Clear
                        } else {
                            GlassPreset::Liquid
                        })
                        .radius_px(r[3] / 2.0),
                    &self.parameters,
                );
                if tinted {
                    // Public Color.orange resolved on the recorded macOS 27
                    // appearance, not a fitted glass colour or a universal token.
                    // See liquid-glass-reference/color-resolution/output.txt.
                    glass = glass
                        .tint(rgb(if self.dark { 0xff9230 } else { 0xff8d28 }))
                        .pressable(true)
                        .track_pointer(true);
                }
                // Exact explicit native treatment, not the theme's dimming token.
                if clear {
                    root = root.child(
                        positioned(r)
                            .rounded(px(r[3] / 2.0))
                            .bg(hsla(0.0, 0.0, 0.0, 0.3)),
                    );
                }
                let body = glass.child(label(
                    pill["label"].as_str().unwrap(),
                    r[2],
                    r[3],
                    clear || tinted || self.dark,
                    true,
                ));
                root = root.child(positioned(r).child(if clear {
                    body.into_any_element()
                } else {
                    regular(body, &self.parameters)
                }));
            }
            for name in ["near", "far"] {
                let panes = &self.fixture["fusion"][name];
                let a = rect(&panes[0]);
                let b = rect(&panes[1]);
                let mut group = GlassGroup::new(format!("fusion-{name}"))
                    .radius(Radius::Dialog)
                    .gap(b[0] - a[0] - a[2])
                    .merge(
                        self.fixture["fusion"]["container_spacing"]
                            .as_f64()
                            .unwrap() as f32,
                    );
                for (key, value) in self.parameters.as_object().unwrap() {
                    let v = value.as_f64().unwrap() as f32;
                    group = match key.as_str() {
                        "blur" => group.blur(v),
                        "thickness" => group.thickness(v),
                        "refractive_index" => group.refractive_index(v),
                        "backdrop_depth" => group.backdrop_depth(v),
                        "protect_text" => group.protect_text_contrast(v == 1.0),
                        key if key.starts_with("regular_") => group,
                        _ => unreachable!(),
                    };
                }
                group = group
                    .pane(
                        format!("{name}-a"),
                        label("Pane A", a[2], a[3], self.dark, false),
                    )
                    .pane(
                        format!("{name}-b"),
                        label("Pane B", b[2], b[3], self.dark, false),
                    );
                let radius = self.fixture["fusion"]["corner_radius"].as_f64().unwrap() as f32;
                root = root.child(positioned([a[0], a[1], b[0] + b[2] - a[0], a[3]]).child(
                    ThemeOverlay::new(
                        move |t| t.clone().modify(|t| t.radii.dialog = radius),
                        regular(group, &self.parameters),
                    ),
                ));
            }
            let mut r = rect(&self.fixture["transition"]["button"]);
            if let Some(animator) = self.resize {
                let now = cx.background_executor().now();
                let progress = animator.head(now);
                let target = rect(&self.fixture["transition"]["menu"]);
                for i in 0..4 {
                    r[i] += (target[i] - r[i]) * progress;
                }
                if animator.running(now) {
                    window.request_animation_frame();
                }
            }
            let content = if self.resize.is_some() {
                div()
                    .w(px(r[2]))
                    .h(px(r[3]))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div().flex().flex_col().gap(px(12.0)).children(
                            self.fixture["transition"]["menu_labels"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|text| {
                                    div()
                                        .text_size(px(17.0))
                                        .line_height(px(20.0))
                                        .text_color(rgb(if self.dark {
                                            0xffffff
                                        } else {
                                            0x000000
                                        }))
                                        .child(text.as_str().unwrap().to_owned())
                                }),
                        ),
                    )
            } else {
                label("Actions", r[2], r[3], self.dark, false)
            };
            let measured = self.measured.clone();
            root = root.child(
                positioned(r)
                    .on_children_prepainted(move |bounds, _, cx| {
                        if let Some(b) = bounds.first() {
                            measured.set((
                                [
                                    b.origin.x.into(),
                                    b.origin.y.into(),
                                    b.size.width.into(),
                                    b.size.height.into(),
                                ],
                                Some(cx.background_executor().now()),
                            ));
                        }
                    })
                    .child(regular(
                        material(
                            Glass::new("menu")
                                .radius_px(24.0)
                                .pressable(true)
                                .track_pointer(true),
                            &self.parameters,
                        )
                        .child(content),
                        &self.parameters,
                    )),
            );
        }
        root
    }
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() == 4 && args[1] == "--observable" {
        return observable::run(Path::new(&args[2]), Path::new(&args[3]));
    }
    ensure!(
        args.len() == 3,
        "usage: glass_reference [--observable] REQUEST.json OUTPUT_DIRECTORY"
    );
    let request: Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let out = Path::new(&args[2]);
    ensure!(out.is_dir(), "output directory must exist");
    let scale = request["scale"].as_u64().unwrap_or(0);
    ensure!(scale == 1 || scale == 2, "scale must be 1 or 2");
    let parameters = &request["parameters"];
    validate_parameters(parameters)?;
    let fixture = request["fixture"].clone();
    // Geometry v1 is intentionally fixed. Do not silently accept a new fixture.
    let authority: Value =
        serde_json::from_str(include_str!("../../liquid-glass-reference/fixture.json"))?;
    ensure!(fixture == authority, "unsupported fixture");
    let mut cx = HeadlessAppContext::with_platform(
        Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
            "Geist",
        )),
        Arc::new(gpui_kit::assets::Assets),
        gpui_platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::install(cx);
        cx.set_reduce_motion(false);
    });
    let window = cx.open_window(size(px(960.0), px(640.0)), |_, cx: &mut App| {
        cx.new(|_| Fixture {
            fixture,
            parameters: parameters.clone(),
            background: true,
            dark: false,
            resize: None,
            measured: Rc::new(Cell::new(([0.0; 4], None))),
        })
    })?;
    let handle: AnyWindowHandle = window.into();
    let mut frames = Vec::new();
    let requests = request["frames"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("frames must be array"))?;
    ensure!(
        !requests.is_empty() && requests.len() <= 512,
        "invalid frame count"
    );
    for (ordinal, f) in requests.iter().enumerate() {
        let appearance = f["appearance"].as_str().unwrap_or("");
        let phase = f["phase"].as_str().unwrap_or("");
        ensure!(
            ["light", "dark"].contains(&appearance)
                && ["background", "static", "transition"].contains(&phase),
            "unsupported case"
        );
        cx.update(|cx| {
            activate_theme(
                if appearance == "dark" {
                    "studio-dark"
                } else {
                    "studio-light"
                },
                cx,
            );
        });
        window.update(&mut cx, |view, window, cx| {
            view.dark = appearance == "dark";
            view.background = phase == "background";
            view.resize = None;
            view.measured.set(([0.0; 4], None));
            window.set_scale_factor(scale as f32);
            cx.notify();
        })?;
        // Replay each sample from compact state on the same view and Glass id.
        let mut previous: Option<image::RgbaImage> = None;
        let mut settled = false;
        for _ in 0..32 {
            cx.run_until_parked();
            cx.update_window(handle, |_, w, cx| {
                w.draw(cx).clear(cx);
            })?;
            let frame = cx.capture_screenshot(handle)?;
            if previous
                .as_ref()
                .is_some_and(|p| p.as_raw() == frame.as_raw())
            {
                settled = true;
                break;
            }
            previous = Some(frame);
        }
        ensure!(settled, "fixture did not settle");
        let trigger = cx.background_executor.now();
        if phase == "transition" {
            let t = f["requested_sample_time"].as_f64().unwrap_or(-1.0);
            ensure!(
                t.is_finite() && (0.0..=120.0).contains(&t),
                "invalid sample time"
            );
            window.update(&mut cx, |view, _, cx| {
                let mut animator = Animator::new(Duration::from_secs_f64(
                    view.fixture["transition"]["duration_seconds"]
                        .as_f64()
                        .unwrap(),
                ));
                animator.play(trigger);
                view.resize = Some(animator);
                cx.notify();
            })?;
            cx.advance_clock(Duration::from_secs_f64(t));
        }
        cx.run_until_parked();
        let actual = cx.update_window(handle, |_, w, cx| {
            w.draw(cx).clear(cx);
            cx.background_executor()
                .now()
                .duration_since(trigger)
                .as_secs_f64()
        })?;
        let geometry = window.update(&mut cx, |view, _, _| {
            let (bounds, measured_at) = view.measured.get();
            json!({"identity":"menu", "bounds":bounds,
                "sample_time_after_trigger":measured_at.map(|t| t.duration_since(trigger).as_secs_f64()),
                "phase":if actual >= 0.8 {"settled"} else {"resizing"},
                "motion":"Animator linear 0.8s; independent compact-state replay"})
        })?;
        let frame = cx.capture_screenshot(handle)?;
        // TestPlatform allocates at 2x. At 1x the scene itself is rendered at
        // 1x; discard only the unused right/bottom allocation, never resample.
        let w = 960 * scale as u32;
        let h = 640 * scale as u32;
        ensure!(
            frame.width() >= w && frame.height() >= h,
            "short renderer output"
        );
        let frame = image::imageops::crop_imm(&frame, 0, 0, w, h).to_image();
        let stem = format!("frame-{ordinal:04}");
        frame.save(out.join(format!("{stem}.png")))?;
        let rgb: Vec<u8> = frame.pixels().flat_map(|p| p.0[..3].to_vec()).collect();
        fs::write(out.join(format!("{stem}.rgb")), rgb)?;
        frames.push(json!({"appearance":appearance,"phase":phase,"index":f["index"],
            "file":format!("{stem}.png"),"raw_file":format!("{stem}.rgb"),"pixel_size":[w,h],
            "sample_time_after_trigger":actual,"transition_status":if phase=="transition" {"persistent-surface-resize"} else {"not-applicable"},
            "resize_geometry":if phase=="transition" {geometry} else {Value::Null}}));
    }
    fs::write(
        out.join("render.json"),
        serde_json::to_vec_pretty(&json!({"schema":1,
        "renderer":if cfg!(target_os="macos") {"native-metal"} else {"wgpu-software-fallback"},
        "clock":"GPUI TestDispatcher; measured executor now at draw completion",
        "frames":frames}))?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_resize_measures_real_early_intermediate_and_settled_frames() {
        let mut cx = HeadlessAppContext::with_platform(
            Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
                "Geist",
            )),
            Arc::new(gpui_kit::assets::Assets),
            gpui_platform::current_headless_renderer,
        );
        cx.update(|cx| {
            gpui_kit::install(cx);
            cx.set_reduce_motion(false);
        });
        let measured = Rc::new(Cell::new(([0.0; 4], None)));
        let window = cx
            .open_window(size(px(960.0), px(640.0)), |_, cx: &mut App| {
                cx.new(|_| Fixture {
                    fixture: serde_json::from_str(include_str!(
                        "../../liquid-glass-reference/fixture.json"
                    ))
                    .unwrap(),
                    parameters: json!({}),
                    background: false,
                    dark: false,
                    resize: None,
                    measured: measured.clone(),
                })
            })
            .unwrap();
        let handle: AnyWindowHandle = window.into();
        let capture = |cx: &mut HeadlessAppContext| {
            cx.run_until_parked();
            cx.update_window(handle, |_, w, cx| w.draw(cx).clear(cx))
                .unwrap();
            cx.capture_screenshot(handle).unwrap()
        };
        for scale in [1, 2] {
            for dark in [false, true] {
                cx.update(|cx| {
                    activate_theme(if dark { "studio-dark" } else { "studio-light" }, cx)
                });
                window
                    .update(&mut cx, |view, w, cx| {
                        view.dark = dark;
                        view.resize = None;
                        w.set_scale_factor(scale as f32);
                        cx.notify();
                    })
                    .unwrap();
                for _ in 0..4 {
                    capture(&mut cx);
                }
                let initial = capture(&mut cx);
                assert_eq!(measured.get().0, [64.0, 448.0, 144.0, 48.0]);
                let trigger = cx.background_executor.now();
                window
                    .update(&mut cx, |view, _, cx| {
                        let mut animator = Animator::new(Duration::from_millis(800));
                        animator.play(trigger);
                        view.resize = Some(animator);
                        cx.notify();
                    })
                    .unwrap();
                let crop = |image: &image::RgbaImage| {
                    image::imageops::crop_imm(
                        image,
                        64 * scale,
                        448 * scale,
                        272 * scale,
                        128 * scale,
                    )
                    .to_image()
                };
                let mut previous = crop(&initial);
                let mut elapsed = 0;
                // Explicit independently calculated endpoints and asymmetric
                // intermediate sizes catch stale views and swapped axes.
                for (millis, width, height) in [
                    (80, 156.8, 56.0),
                    (300, 192.0, 78.0),
                    (790, 270.4, 127.0),
                    (800, 272.0, 128.0),
                    (1200, 272.0, 128.0),
                ] {
                    cx.advance_clock(Duration::from_millis(millis - elapsed));
                    elapsed = millis;
                    let image = capture(&mut cx);
                    let (bounds, at) = measured.get();
                    assert_eq!(
                        at.unwrap().duration_since(trigger),
                        Duration::from_millis(millis)
                    );
                    for (actual, expected) in bounds.into_iter().zip([64.0, 448.0, width, height]) {
                        assert!(
                            (actual - expected).abs() <= 0.51 / scale as f32,
                            "{bounds:?}"
                        );
                    }
                    let current = crop(&image);
                    if millis == 1200 {
                        // Use the documented headless per-channel contract;
                        // this test does not establish the cause of rounding.
                        assert!(
                            current
                                .as_raw()
                                .iter()
                                .zip(previous.as_raw())
                                .all(|(a, b)| a.abs_diff(*b) <= 1),
                            "settled pixels changed"
                        );
                    } else {
                        assert_ne!(current, previous, "static output at {millis}ms");
                    }
                    // Apply the same headless contract to unrelated pills/fusion.
                    // Metal CI also showed one-code changes on an unrelated
                    // glyph edge; byte equality is not a portable invariant.
                    let unrelated =
                        image::imageops::crop_imm(&image, 0, 0, 960 * scale, 400 * scale)
                            .to_image();
                    let initial_unrelated =
                        image::imageops::crop_imm(&initial, 0, 0, 960 * scale, 400 * scale)
                            .to_image();
                    assert!(
                        unrelated
                            .as_raw()
                            .iter()
                            .zip(initial_unrelated.as_raw())
                            .all(|(a, b)| a.abs_diff(*b) <= 1),
                        "unrelated fixtures changed beyond one code at {millis}ms, scale {scale}, dark {dark}"
                    );
                    previous = current;
                }
                window
                    .update(&mut cx, |view, _, cx| {
                        view.resize = None;
                        cx.notify();
                    })
                    .unwrap();
                for _ in 0..4 {
                    capture(&mut cx);
                }
                let restored = capture(&mut cx);
                assert_eq!(
                    measured.get().0,
                    [64.0, 448.0, 144.0, 48.0],
                    "compact logical geometry must be restored exactly"
                );
                assert_eq!(restored.dimensions(), initial.dimensions());
                assert!(
                    restored
                        .as_raw()
                        .iter()
                        .zip(initial.as_raw())
                        .all(|(a, b)| a.abs_diff(*b) <= 1),
                    "compact pixels must restore within the headless one-step contract"
                );
                // The full-frame check above includes all unrelated fixtures.
            }
        }
    }

    #[test]
    fn bounded_trial_parameters() {
        for bad in [
            json!([]),
            json!({"unknown": 1}),
            json!({"protect_text": 0.5}),
            json!({"protect_text": true}),
            json!({"regular_gain": "1"}),
            json!({"regular_blur": 65}),
            json!({"regular_saturation": -0.1}),
            json!({"regular_wash": 1.01}),
            json!({"regular_lift": null}),
            json!({"regular_gain": 2.01}),
            json!({"regular_thickness": 64.01}),
            json!({"regular_backdrop_depth": -0.01}),
            json!({"regular_refraction": 1.01}),
            json!({"regular_refractive_index": 0.99}),
        ] {
            assert!(validate_parameters(&bad).is_err(), "{bad}");
        }
        for valid in [
            json!({}),
            json!({"protect_text": 0}),
            json!({"protect_text": 1.0}),
            json!({"regular_blur": 64, "regular_saturation": 3, "regular_wash": 1,
                "regular_gain": 2, "regular_lift": 1}),
            json!({"regular_thickness": 64, "regular_backdrop_depth": 128,
                "regular_refraction": 1, "regular_refractive_index": 2.5}),
        ] {
            validate_parameters(&valid).unwrap();
        }
    }

    #[test]
    fn regular_trials_preserve_clear_and_explicit_blur_wins() {
        let mut cx = HeadlessAppContext::with_platform(
            Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
                "Geist",
            )),
            Arc::new(gpui_kit::assets::Assets),
            gpui_platform::current_headless_renderer,
        );
        cx.update(|cx| {
            gpui_kit::install(cx);
            cx.set_reduce_motion(true);
        });
        let fixture: Value =
            serde_json::from_str(include_str!("../../liquid-glass-reference/fixture.json"))
                .unwrap();
        let window = cx
            .open_window(size(px(960.0), px(640.0)), |_, cx: &mut App| {
                cx.new(|_| Fixture {
                    fixture: fixture.clone(),
                    parameters: json!({}),
                    background: false,
                    dark: false,
                    resize: None,
                    measured: Rc::new(Cell::new(([0.0; 4], None))),
                })
            })
            .unwrap();
        let handle: AnyWindowHandle = window.into();
        for dark in [false, true] {
            cx.update(|cx| {
                activate_theme(if dark { "studio-dark" } else { "studio-light" }, cx);
            });
            let mut capture = |parameters: Value| {
                window
                    .update(&mut cx, |view, w, cx| {
                        view.parameters = parameters;
                        view.dark = dark;
                        w.set_scale_factor(1.0);
                        cx.notify();
                    })
                    .unwrap();
                let mut previous: Option<image::RgbaImage> = None;
                for _ in 0..32 {
                    cx.run_until_parked();
                    cx.update_window(handle, |_, w, cx| {
                        w.draw(cx).clear(cx);
                    })
                    .unwrap();
                    let image = cx.capture_screenshot(handle).unwrap();
                    if previous.as_ref().is_some_and(|p| p == &image) {
                        return image;
                    }
                    previous = Some(image);
                }
                panic!("trial did not settle");
            };
            let baseline = capture(json!({}));
            assert_eq!(baseline, capture(json!({"protect_text": 1})));
            let trial = capture(json!({"regular_blur": 23, "regular_saturation": 0.63,
                "regular_wash": 0.17, "regular_gain": 1.31, "regular_lift": 0.09}));
            for pill in fixture["pills"].as_array().unwrap() {
                let [x, y, w, h] = rect(&pill["rect"]).map(|v| v as u32);
                let crop = |image: &image::RgbaImage| {
                    image::imageops::crop_imm(image, x, y, w, h).to_image()
                };
                if pill["material"] == "clear" {
                    assert_eq!(crop(&baseline), crop(&trial));
                } else {
                    assert_ne!(crop(&baseline), crop(&trial));
                }
            }
            for r in [[64, 304, 268, 72], [448, 304, 320, 72], [64, 448, 144, 48]] {
                let crop = |image: &image::RgbaImage| {
                    image::imageops::crop_imm(image, r[0], r[1], r[2], r[3]).to_image()
                };
                assert_ne!(crop(&baseline), crop(&trial));
            }
            assert_eq!(
                capture(json!({"blur": 7.5})),
                capture(json!({"blur": 7.5, "regular_blur": 23}))
            );
            // Each optical setter must affect actual pixels independently;
            // a combined colour trial could conceal a silently ignored setter.
            for (key, value) in [
                ("regular_refraction", 1.0),
                ("regular_thickness", 9.0),
                ("regular_backdrop_depth", 24.0),
                ("regular_refractive_index", 2.1),
            ] {
                let control = capture(json!({"protect_text": 0}));
                let changed = capture(json!({"protect_text": 0, key: value}));
                let crop = |image: &image::RgbaImage, x| {
                    image::imageops::crop_imm(image, x, 160, 208, 72).to_image()
                };
                assert_ne!(crop(&control, 64), crop(&changed, 64), "{key}");
                assert_eq!(crop(&control, 304), crop(&changed, 304), "Clear: {key}");
            }
            // Protection is a dark-appearance body policy; light can be identical.
            if dark {
                assert!(baseline != capture(json!({"protect_text": 0})));
            }
        }
    }

    struct MaterialCases;

    impl Render for MaterialCases {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let mut root = div().relative().size_full().bg(rgb(0xffffff));
            for i in 0..6 {
                let protect = i % 3 != 1;
                let tinted = i % 3 == 2;
                let body = if i < 3 {
                    let mut glass = Glass::new(format!("single-{i}"))
                        .blur(0.0)
                        .specular(0.0)
                        .protect_text_contrast(protect);
                    if tinted {
                        glass = glass.tint(gpui::hsla(0.0, 1.0, 0.5, 0.25));
                    }
                    glass
                        .child(div().w(px(80.0)).h(px(64.0)))
                        .into_any_element()
                } else {
                    let mut glass = GlassGroup::new(format!("group-{i}"))
                        .blur(0.0)
                        .gap(8.0)
                        .merge(32.0)
                        .protect_text_contrast(protect);
                    if tinted {
                        glass = glass.tint(gpui::hsla(0.0, 1.0, 0.5, 0.25));
                    }
                    glass
                        .pane(format!("pane-a-{i}"), div().w(px(36.0)).h(px(64.0)))
                        .pane(format!("pane-b-{i}"), div().w(px(36.0)).h(px(64.0)))
                        .into_any_element()
                };
                root =
                    root.child(positioned([8.0 + i as f32 * 112.0, 16.0, 80.0, 64.0]).child(body));
            }
            root
        }
    }

    #[test]
    fn material_policy_and_tint_reach_surface_and_fused_bridge_pixels() {
        let mut cx = HeadlessAppContext::with_platform(
            Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
                "Geist",
            )),
            Arc::new(gpui_kit::assets::Assets),
            gpui_platform::current_headless_renderer,
        );
        cx.update(|cx| {
            gpui_kit::install(cx);
            activate_theme("studio-dark", cx);
        });
        let window = cx
            .open_window(size(px(720.0), px(112.0)), |_, cx: &mut App| {
                cx.new(|_| MaterialCases)
            })
            .unwrap();
        let handle: AnyWindowHandle = window.into();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.draw(cx).clear(cx);
        })
        .unwrap();
        let image = cx.capture_screenshot(handle).unwrap();
        for start in [0, 3] {
            let pixel = |i: u32| image.get_pixel((48 + i * 112) * 2, 48 * 2).0;
            let protected = pixel(start);
            let material = pixel(start + 1);
            let tinted = pixel(start + 2);
            assert!(protected[0] < 128, "default body protection: {protected:?}");
            // White source × 1.042 × (1 − 0.45) + 0.075, in encoded RGB.
            assert!(
                (i32::from(material[0]) - 165).abs() <= 2,
                "preset material: {material:?}"
            );
            assert!(
                i32::from(tinted[0]) > i32::from(tinted[1]) + 50,
                "red tint, including bridge: {tinted:?}"
            );
            assert_eq!(tinted[3], 255);
        }
    }

    struct ClippedContents {
        visible: bool,
    }

    impl Render for ClippedContents {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            use gpui_kit_theme::ActiveTheme;
            let body = |width| {
                div()
                    .w(px(width))
                    .h(px(48.0))
                    .when(self.visible, |body| body.bg(rgb(0xff0000)))
            };
            div()
                .relative()
                .size_full()
                .bg(rgb(0xffffff))
                .child(
                    positioned([16.0, 16.0, 80.0, 48.0])
                        .child(Glass::new("clip-single").radius_px(24.0).child(body(80.0))),
                )
                .child(
                    positioned([128.0, 16.0, 104.0, 48.0]).child(
                        GlassGroup::new("clip-group")
                            .radius(Radius::Pill)
                            .gap(8.0)
                            .merge(32.0)
                            .pane("clip-left", body(48.0))
                            .pane("clip-right", body(48.0)),
                    ),
                )
                .child(
                    positioned([16.0, 88.0, 80.0, 48.0]).child(
                        gpui_kit::overlay::surface(
                            "clip-frame",
                            cx.theme(),
                            gpui_kit::overlay::OverlaySurface::MODAL,
                        )
                        .w_full()
                        .h_full()
                        .children([0xff0000, 0x0000ff].map(|color| {
                            div()
                                .w_full()
                                .h(px(24.0))
                                .when(self.visible, |child| child.bg(rgb(color)))
                        })),
                    ),
                )
        }
    }

    #[test]
    fn glass_clips_foreground_without_clipping_surface_shadows_or_fused_bridge() {
        let mut cx = HeadlessAppContext::with_platform(
            Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
                "Geist",
            )),
            Arc::new(gpui_kit::assets::Assets),
            gpui_platform::current_headless_renderer,
        );
        cx.update(|cx| {
            gpui_kit::install(cx);
            cx.set_reduce_motion(true);
            activate_theme("studio-light", cx);
        });
        let window = cx
            .open_window(size(px(256.0), px(160.0)), |_, cx: &mut App| {
                cx.new(|_| ClippedContents { visible: false })
            })
            .unwrap();
        let handle: AnyWindowHandle = window.into();
        let mut capture = |visible| {
            cx.update_window(handle, |view, _, cx| {
                view.downcast::<ClippedContents>()
                    .unwrap()
                    .update(cx, |view, cx| {
                        view.visible = visible;
                        cx.notify();
                    });
            })
            .unwrap();
            let mut previous = None;
            for _ in 0..6 {
                cx.run_until_parked();
                cx.update_window(handle, |_, window, cx| window.draw(cx).clear(cx))
                    .unwrap();
                let image = cx.capture_screenshot(handle).unwrap();
                if previous.as_ref() == Some(&image) {
                    return image;
                }
                previous = Some(image);
            }
            panic!("glass probe and shadows did not settle");
        };
        let empty = capture(false);
        let content = capture(true);
        let at = |image: &image::RgbaImage, x: u32, y: u32| image.get_pixel(x * 2, y * 2).0;
        for (x, y) in [
            (17, 17),
            (94, 17),
            (129, 17),
            (185, 17),
            (180, 40),
            (10, 40),
            (17, 89),
            (94, 134),
        ] {
            assert_eq!(
                at(&content, x, y),
                at(&empty, x, y),
                "outside contents {x},{y}"
            );
        }
        for x in [56, 152, 208] {
            assert_eq!(at(&content, x, 40), [255, 0, 0, 255], "inside {x}");
        }
        assert_eq!(at(&content, 56, 100), [255, 0, 0, 255]);
        assert_eq!(at(&content, 56, 124), [0, 0, 255, 255]);
        assert!(
            at(&empty, 10, 40)[0] < 255,
            "surface shadow must survive outside clip"
        );
    }

    #[test]
    fn asymmetric_rectangle_preserves_axes() {
        assert_eq!(
            super::rect(&serde_json::json!([13, 29, 101, 47])),
            [13.0, 29.0, 101.0, 47.0]
        );
    }
}
