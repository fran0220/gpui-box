//! Separate from the visual gate: opt-in, bounded renderer timing evidence.
use anyhow::{Context, Result, ensure};
use gpui::{
    Background, BorderStyle, Bounds, ContentMask, Corners, DevicePixels, Edges, GpuExecutionTime,
    Hsla, Quad, RendererFrameTiming, ScaledPixels, Scene, point, size,
};
use serde_json::{Value, json};
use std::time::Instant;

const WARMUP: usize = 8;
const SAMPLES: usize = 30;
const WARMUP_SECONDS: f64 = 0.5;

fn workload(count: u32) -> Scene {
    let mut scene = Scene::default();
    let viewport = Bounds {
        origin: point(ScaledPixels(0.0), ScaledPixels(0.0)),
        size: size(ScaledPixels(640.0), ScaledPixels(480.0)),
    };
    for index in 0..count {
        scene.insert_primitive(Quad {
            order: index,
            border_style: BorderStyle::default(),
            bounds: Bounds {
                origin: point(
                    ScaledPixels((index * 37 % 560) as f32),
                    ScaledPixels((index * 19 % 400) as f32),
                ),
                size: size(ScaledPixels(80.0), ScaledPixels(80.0)),
            },
            content_mask: ContentMask { bounds: viewport },
            background: Background::from(Hsla {
                h: (index % 31) as f32 / 31.0,
                s: 0.6,
                l: 0.5,
                a: 0.8,
            }),
            border_color: Hsla::transparent_black(),
            border_widths: Edges::default(),
            corner_radii: Corners::default(),
        });
    }
    scene.finish();
    scene
}

fn sample(timing: RendererFrameTiming) -> Value {
    let gpu = match timing.gpu_execution {
        GpuExecutionTime::Measured(duration) => {
            json!({"status": "measured", "seconds": duration.as_secs_f64()})
        }
        GpuExecutionTime::Unsupported(reason) => json!({"status": "unsupported", "reason": reason}),
    };
    json!({
        "submission_id": timing.submission_id,
        "cpu_encode_submit": timing.cpu_encode_submit.as_secs_f64(),
        "submit_to_completion": timing.submit_to_completion.as_secs_f64(),
        "gpu_execution": gpu,
        "timestamp_readback": timing.timestamp_readback.map(|time| time.as_secs_f64()),
    })
}

fn main() -> Result<()> {
    let session = std::env::var("GPUI_TIMING_SESSION")
        .context("set GPUI_TIMING_SESSION to one same-host comparison session ID")?;
    ensure!(!session.is_empty(), "empty host session");
    let started = Instant::now();
    let mut renderer = gpui_platform::current_headless_renderer()
        .context("native headless renderer unavailable")?;
    let renderer_initialization = started.elapsed().as_secs_f64();
    let extent = size(DevicePixels(640), DevicePixels(480));
    let mut workloads = serde_json::Map::new();
    // Two independently compared workloads, never one divided by the other:
    // a renderer-wide regression must not normalize itself away.
    for (name, count) in [("small_quads", 32), ("quads", 1024)] {
        let scene = workload(count);
        let first_frame = sample(renderer.measure_scene(&scene, extent)?);
        let warming = Instant::now();
        let mut warmup_samples = 0;
        while warmup_samples < WARMUP || warming.elapsed().as_secs_f64() < WARMUP_SECONDS {
            ensure!(warmup_samples < 4096, "warmup frame bound exceeded");
            renderer.measure_scene(&scene, extent)?;
            warmup_samples += 1;
        }
        let warmup_elapsed = warming.elapsed().as_secs_f64();
        let mut samples = Vec::with_capacity(SAMPLES);
        for _ in 0..SAMPLES {
            let mut measured = sample(renderer.measure_scene(&scene, extent)?);
            // A *separate* capture, not the measured submission's GPU time.
            // This API includes a new render, pixel transfer and CPU copies.
            let capture_started = Instant::now();
            let image = renderer.render_scene_to_image(&scene, extent)?;
            let capture_total = capture_started.elapsed().as_secs_f64();
            ensure!(
                image.dimensions() == (640, 480),
                "capture dimensions changed"
            );
            ensure!(
                image.pixels().any(|pixel| pixel.0[..3] != [0, 0, 0]),
                "workload rendered no colored pixels"
            );
            std::hint::black_box(image);
            measured["separate_capture_total"] = json!(capture_total);
            samples.push(measured);
        }
        workloads.insert(
            name.into(),
            json!({"primitive_count": count, "first_frame": first_frame, "warmup_samples": warmup_samples, "warmup_elapsed": warmup_elapsed, "samples": samples}),
        );
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema": 1,
            "session": session,
            "adapter": renderer.timing_identity(),
            "renderer_class": if cfg!(target_os = "macos") { "native-metal" } else { "wgpu-software-fallback" },
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "debug_assertions": cfg!(debug_assertions),
            "workload_version": "quads-v1-640x480",
            "warmup": WARMUP,
            "warmup_seconds": WARMUP_SECONDS,
            "sample_count": SAMPLES,
            "renderer_initialization": renderer_initialization,
            "workloads": workloads,
            "excluded": ["input-to-display", "presentation", "native Windows D3D", "application startup", "pixel-readback-only duration"],
        }))?
    );
    Ok(())
}
