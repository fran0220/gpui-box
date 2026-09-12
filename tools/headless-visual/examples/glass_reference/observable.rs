//! Label-free 640×400 forward comparison protocol. No optical inversion.
use super::{material, positioned, regular, validate_parameters};
use anyhow::{Result, ensure};
use gpui::{
    AnyWindowHandle, App, Context, HeadlessAppContext, IntoElement, Render, Window, div,
    prelude::*, px, rgb, size,
};
use gpui_kit::prelude::{Glass, GlassPreset};
use gpui_kit_theme::activate_theme;
use serde_json::{Value, json};
use std::{collections::HashSet, fs, path::Path, sync::Arc};

const SCHEMA: &str = "gpui-regular-observable-request-1";
const CAPS: [[f32; 4]; 2] = [[148., 100., 144., 40.], [296., 239., 208., 72.]];

fn keys(v: &Value, expected: &[&str]) -> Result<()> {
    let object = v
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("expected object"))?;
    ensure!(
        object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key)),
        "unknown or missing key"
    );
    Ok(())
}

fn validate(request: &Value) -> Result<()> {
    keys(
        request,
        &[
            "schema",
            "run_id",
            "logical_size",
            "scale",
            "capsules",
            "parameters",
            "cases",
        ],
    )?;
    ensure!(
        request["schema"] == SCHEMA
            && request["logical_size"] == json!([640, 400])
            && request["scale"] == 2,
        "unsupported observable fixture"
    );
    ensure!(
        request["capsules"] == json!({"small":[148,100,144,40],"large":[296,239,208,72]}),
        "unsupported capsules"
    );
    ensure!(
        request["run_id"]
            .as_str()
            .is_some_and(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())),
        "invalid run identity"
    );
    validate_parameters(&request["parameters"])?;
    let cases = request["cases"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("cases must be array"))?;
    ensure!(
        !cases.is_empty() && cases.len() <= 324,
        "invalid case count"
    );
    let mut ids = HashSet::new();
    for case in cases {
        keys(
            case,
            &[
                "id",
                "group",
                "appearance",
                "axis",
                "period",
                "mean",
                "amplitude",
                "index",
                "phase",
                "kind",
                "rest",
                "phase_role",
                "codes",
            ],
        )?;
        let id = case["id"].as_str().unwrap_or("");
        ensure!(
            !id.is_empty()
                && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
                && ids.insert(id),
            "invalid/duplicate id"
        );
        ensure!(
            ["light", "dark"].contains(&case["appearance"].as_str().unwrap_or("")),
            "explicit appearance required"
        );
        let axis = case["axis"].as_str().unwrap_or("");
        ensure!(["x", "y"].contains(&axis), "invalid axis");
        let period = case["period"].as_u64().unwrap_or(0);
        let mean = case["mean"].as_f64().unwrap_or(f64::NAN);
        let amplitude = case["amplitude"].as_f64().unwrap_or(f64::NAN);
        let phase = case["phase"].as_f64().unwrap_or(f64::NAN);
        let index = case["index"].as_u64().unwrap_or(100);
        let kind = case["kind"].as_str().unwrap_or("");
        ensure!(
            [32, 64, 128].contains(&period) && phase.is_finite(),
            "invalid period/phase"
        );
        let group = format!(
            "{}-{axis}-p{period}-m{:02}-a{:02}",
            case["appearance"].as_str().unwrap(),
            (mean * 100.).round() as u32,
            (amplitude * 100.).round() as u32
        );
        ensure!(case["group"] == group, "group mismatch");
        if kind == "flat" {
            let rest = case["rest"].as_str().unwrap_or("");
            ensure!(
                axis == "x"
                    && period == 64
                    && mean == 0.5
                    && amplitude == 0.
                    && phase == 0.
                    && index == 0
                    && case["phase_role"] == "flat"
                    && ["before", "after"].contains(&rest),
                "invalid flat control"
            );
            ensure!(
                id == format!("{}-flat-{rest}", case["appearance"].as_str().unwrap()),
                "flat id mismatch"
            );
        } else {
            ensure!(index < 10 && case["rest"].is_null(), "invalid phase index");
            let expected = if index < 8 {
                index as f64 * std::f64::consts::PI / 4.
            } else {
                (1 + (index - 8) * 8) as f64 * std::f64::consts::PI / 8.
            };
            ensure!(
                (phase - expected).abs() < 1e-14
                    && case["phase_role"] == if index < 8 { "fit" } else { "heldout" },
                "phase/split mismatch"
            );
            ensure!(
                (kind == "primary" && mean == 0.5 && [0.06, 0.12].contains(&amplitude))
                    || (kind == "mean-control"
                        && period == 64
                        && [0.35, 0.65].contains(&mean)
                        && amplitude == 0.06),
                "invalid operating point"
            );
            ensure!(id == format!("{group}-{index}"), "case id mismatch");
        }
        let codes = case["codes"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("codes must be array"))?;
        ensure!(
            codes.len() == if axis == "x" { 1280 } else { 800 },
            "stripe length mismatch"
        );
        for (i, code) in codes.iter().enumerate() {
            let q = (i as f64 + 0.5) / 2.;
            let expected = (255.
                * (mean
                    + amplitude * (2. * std::f64::consts::PI * q / period as f64 + phase).cos()))
            .round_ties_even() as u64;
            ensure!(
                code.as_u64().is_some_and(|v| v <= 255 && v == expected),
                "encoded stripe mismatch at {i}"
            );
        }
    }
    // A producer request cannot silently omit difficult fit or held-out phases.
    for case in cases.iter().filter(|c| c["kind"] != "flat") {
        let indices: HashSet<_> = cases
            .iter()
            .filter(|c| c["group"] == case["group"])
            .map(|c| c["index"].as_u64().unwrap())
            .collect();
        ensure!(indices == (0..10).collect(), "partial phase group");
    }
    Ok(())
}

struct Fixture {
    case: Value,
    parameters: Value,
    background: bool,
}

impl Render for Fixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let codes: Vec<_> = self.case["codes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_u64().unwrap() as u32)
            .collect();
        let horizontal = self.case["axis"] == "x";
        let mut root = div()
            .relative()
            .w(px(640.))
            .h(px(400.))
            .overflow_hidden()
            .child(
                gpui::canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        // One device-pixel stripe at 2x. No image filtering, transfer
                        // function conversion, labels, or CPU material simulation.
                        for (i, code) in codes.iter().enumerate() {
                            let r = if horizontal {
                                [i as f32 / 2., 0., 0.5, 400.]
                            } else {
                                [0., i as f32 / 2., 640., 0.5]
                            };
                            window.paint_quad(gpui::fill(
                                gpui::Bounds::new(
                                    gpui::point(px(r[0]), px(r[1])),
                                    size(px(r[2]), px(r[3])),
                                ),
                                rgb(code * 0x010101),
                            ));
                        }
                        for (x, y, c) in [
                            (8., 8., 0xff0000),
                            (616., 8., 0x00ff00),
                            (8., 376., 0x0000ff),
                        ] {
                            window.paint_quad(gpui::fill(
                                gpui::Bounds::new(
                                    gpui::point(px(x), px(y)),
                                    size(px(16.), px(16.)),
                                ),
                                rgb(c),
                            ));
                        }
                    },
                )
                .absolute()
                .size_full(),
            );
        if !self.background {
            for (name, r) in ["small", "large"].into_iter().zip(CAPS) {
                root = root.child(
                    positioned(r).child(regular(
                        material(
                            Glass::new(format!("observable-{name}"))
                                .preset(GlassPreset::Liquid)
                                .radius_px(r[3] / 2.),
                            &self.parameters,
                        )
                        .child(div().w(px(r[2])).h(px(r[3]))),
                        &self.parameters,
                    )),
                );
            }
        }
        root
    }
}

pub(super) fn run(path: &Path, out: &Path) -> Result<()> {
    let text = fs::read_to_string(path)?;
    let request: Value = serde_json::from_str(&text)?;
    validate(&request)?;
    ensure!(
        out.is_dir() && !out.join("render.json").exists(),
        "fresh output required"
    );
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
    let window = cx.open_window(size(px(640.), px(400.)), |_, cx: &mut App| {
        cx.new(|_| Fixture {
            case: request["cases"][0].clone(),
            parameters: request["parameters"].clone(),
            background: true,
        })
    })?;
    let handle: AnyWindowHandle = window.into();
    let mut frames = Vec::new();
    for (ordinal, case) in request["cases"].as_array().unwrap().iter().enumerate() {
        cx.update(|cx| {
            activate_theme(
                if case["appearance"] == "dark" {
                    "studio-dark"
                } else {
                    "studio-light"
                },
                cx,
            )
        });
        for state in ["background", "glass", "repeat"] {
            window.update(&mut cx, |view, w, cx| {
                view.case = case.clone();
                view.background = state == "background";
                w.set_scale_factor(2.);
                cx.notify();
            })?;
            let mut previous = None;
            let mut result = None;
            for _ in 0..32 {
                cx.run_until_parked();
                cx.update_window(handle, |_, w, cx| w.draw(cx).clear(cx))?;
                let image = cx.capture_screenshot(handle)?;
                ensure!(
                    image.dimensions() == (1280, 800),
                    "unexpected readback extent"
                );
                if previous.as_ref() == Some(&image) {
                    result = Some(image);
                    break;
                }
                previous = Some(image);
            }
            let image = result.ok_or_else(|| anyhow::anyhow!("observable frame did not settle"))?;
            let stem = format!("case-{ordinal:03}-{state}");
            ensure!(
                !out.join(format!("{stem}.png")).exists()
                    && !out.join(format!("{stem}.rgb")).exists(),
                "stale output file"
            );
            image.save(out.join(format!("{stem}.png")))?;
            fs::write(
                out.join(format!("{stem}.rgb")),
                image
                    .pixels()
                    .flat_map(|p| p.0[..3].to_vec())
                    .collect::<Vec<_>>(),
            )?;
            frames.push(json!({"id":case["id"],"state":state,"file":format!("{stem}.png"),"raw_file":format!("{stem}.rgb"),"pixel_size":[1280,800]}));
        }
    }
    fs::write(
        out.join("render.json"),
        serde_json::to_vec_pretty(
            &json!({"schema":"gpui-regular-observable-render-1","request_text":text,"renderer":if cfg!(target_os="macos") {"native-metal"} else {"wgpu-software-fallback"},"frames":frames}),
        )?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_request() -> Value {
        let cases: Vec<_> = (0..10)
            .map(|i| {
                let phase = if i < 8 {
                    i as f64 * std::f64::consts::PI / 4.
                } else {
                    (1 + (i - 8) * 8) as f64 * std::f64::consts::PI / 8.
                };
                let codes: Vec<_> = (0..800)
                    .map(|p| {
                        (255.
                            * (0.5
                                + 0.12
                                    * (2. * std::f64::consts::PI * (p as f64 + 0.5) / 2. / 32.
                                        + phase)
                                        .cos()))
                        .round_ties_even() as u64
                    })
                    .collect();
                json!({"id":format!("dark-y-p32-m50-a12-{i}"),"group":"dark-y-p32-m50-a12",
                "appearance":"dark","axis":"y","period":32,"mean":0.5,"amplitude":0.12,
                "index":i,"phase":phase,"kind":"primary","rest":null,
                "phase_role":if i < 8 {"fit"} else {"heldout"},"codes":codes})
            })
            .collect();
        json!({"schema":SCHEMA,"run_id":"0".repeat(32),"logical_size":[640,400],"scale":2,
            "capsules":{"small":[148,100,144,40],"large":[296,239,208,72]},"parameters":{},"cases":cases})
    }

    #[test]
    fn observable_protocol_validates_encoded_axis_boundaries_and_split() {
        let good = valid_request();
        validate(&good).unwrap();
        let mut bad = good.clone();
        bad["cases"][8]["phase_role"] = json!("fit");
        assert!(validate(&bad).is_err());
        let mut bad = good.clone();
        bad["cases"].as_array_mut().unwrap().pop();
        assert!(validate(&bad).is_err());
        for index in [0, 799] {
            let mut bad = good.clone();
            bad["cases"][1]["codes"][index] = json!(0);
            assert!(validate(&bad).is_err());
        }
        let mut bad = good;
        bad["cases"][0]["axis"] = json!("x");
        assert!(validate(&bad).is_err());
    }

    #[test]
    fn observable_protocol_rejects_old_geometry_and_partial_requests() {
        assert!(validate(&json!({"schema":1,"scale":2})).is_err());
        assert!(validate(&json!({"schema":SCHEMA,"run_id":"0".repeat(32),"logical_size":[960,640],"scale":2,"capsules":{},"parameters":{},"cases":[]})).is_err());
    }
}
