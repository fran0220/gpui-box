//! Renders the scene catalog without a window system and compares the pixels.
//!
//! This is the visual gate on supported macOS, Linux, and Windows hosts. GPUI
//! draws each scene into an offscreen texture at an exact device-pixel size and
//! the pixels are read straight back, so no window, display, menu bar, dock, or
//! compositor takes part. Text is shaped by cosmic-text from the fonts this
//! repository bundles, and time is simulated, which together make the same
//! scene produce the same bytes on any machine running the same renderer.
//!
//! Asking the renderer for the size is the point. A real window negotiates its
//! size with the display it opens on, so the same catalog captured on two Macs
//! produced two incompatible baseline sets; that is what this harness exists to
//! prevent.
//!
//! Active baselines live in
//! `snapshots/headless/{macos,linux,windows}/scenes`, one set per renderer.

use anyhow::Result;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next();
    let mut scenes = Vec::new();
    let mut shard = None;
    while let Some(argument) = args.next() {
        if argument == "--shard" {
            anyhow::ensure!(shard.is_none(), "--shard may be specified only once");
            let value = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("--shard requires INDEX/COUNT"))?;
            shard = Some(Shard::parse(&value)?);
        } else if argument.starts_with('-') {
            anyhow::bail!("unknown option `{argument}`");
        } else {
            scenes.push(argument);
        }
    }
    match command.as_deref() {
        Some("capture") => imp::capture(&scenes, shard),
        Some("check") => imp::check(&scenes, shard),
        Some("check-order") => {
            anyhow::ensure!(
                shard.is_none(),
                "check-order compares the complete catalog, not a shard"
            );
            imp::check_order(&scenes)
        }
        Some("serve") => {
            anyhow::ensure!(
                scenes.is_empty() && shard.is_none(),
                "serve reads line-delimited JSON from stdin and takes no scene arguments"
            );
            serve::run()
        }
        _ => anyhow::bail!(
            "usage: headless-visual <capture|check|serve> [--shard INDEX/COUNT] [scene...]; check-order TARGET [selected-scenes...]"
        ),
    }
}

mod serve;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Shard {
    index: usize,
    count: usize,
}

impl Shard {
    fn parse(value: &str) -> Result<Self> {
        let (index, count) = value
            .split_once('/')
            .ok_or_else(|| anyhow::anyhow!("shard `{value}` must be INDEX/COUNT"))?;
        let index = index
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("shard index `{index}` is not a non-negative integer"))?;
        let count = count
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("shard count `{count}` is not a positive integer"))?;
        anyhow::ensure!(count > 0, "shard count must be greater than zero");
        anyhow::ensure!(
            index < count,
            "shard index {index} is outside a shard count of {count}"
        );
        Ok(Self { index, count })
    }

    fn includes(self, scene_index: usize) -> bool {
        scene_index % self.count == self.index
    }
}

mod imp {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use anyhow::{Context as _, Result, bail};
    use gpui::{
        AnyWindowHandle, App, Context, HeadlessAppContext, IntoElement, Render, Window, div,
        prelude::*, px, size,
    };
    use gpui_kit::prelude::set_layout_direction;
    use gpui_kit_semantics::SemanticCoordinator;
    use gpui_kit_theme::{Theme, activate_theme};

    use crate::Shard;

    /// Accepts what `check` would have rejected, and leaves the rest alone.
    ///
    /// A baseline the new frame agrees with to within one step is not rewritten.
    /// `check` already treats those as matching, so rewriting them recorded a
    /// change nothing had made: a full run's sprite atlas has accumulated
    /// different state by its ninetieth scene, which moves an antialiased pixel
    /// or two in a handful of scenes that nobody touched. Every full pass then
    /// had to be hand-sorted, and the reviewer had to tell one-step noise from
    /// the change under review before committing — three consecutive passes on
    /// two platforms did exactly that, by hand, for the same three scenes.
    ///
    /// Writing only what moved further than the gate's own tolerance makes the
    /// diff of a capture mean what a reader assumes it means.
    pub fn capture(only: &[String], shard: Option<Shard>) -> Result<()> {
        let directory = snapshots();
        fs::create_dir_all(&directory)
            .with_context(|| format!("create {}", directory.display()))?;
        let mut kept = 0usize;
        let mut written = 0usize;
        let count = capture_frames(only, shard, |name, frame| {
            let path = directory.join(name);
            if path.exists() {
                let expected = image::open(&path)
                    .with_context(|| format!("read {}", path.display()))?
                    .into_rgba8();
                if within_one_step(&expected, frame) {
                    kept += 1;
                    return Ok(());
                }
            }
            written += 1;
            frame
                .save(&path)
                .with_context(|| format!("write {}", path.display()))
        })?;
        println!(
            "captured {count} images into {}: {written} written, {kept} already within tolerance",
            directory.display()
        );
        Ok(())
    }

    /// Captures into a scratch directory and reports every image that differs
    /// from the committed one.
    ///
    /// The comparison allows one step per channel, which is what the native
    /// gate has always allowed. Exactness was tried first and does not hold:
    /// capturing `frost` on its own and capturing it as part of the whole
    /// catalog differ by one pixel at one step, because the sprite atlas has
    /// accumulated different state by the time the ninetieth scene draws.
    /// Scoped runs agree with each other to the byte, so the tolerance buys a
    /// scoped check that means the same thing as the full one. Anything a
    /// component actually changed moves far further than one step.
    pub fn check(only: &[String], shard: Option<Shard>) -> Result<()> {
        let committed = snapshots();
        let scratch = repo_root().join("target").join("headless-scene-check");
        if scratch.exists() {
            fs::remove_dir_all(&scratch).with_context(|| format!("clear {}", scratch.display()))?;
        }
        let mut differing = Vec::new();
        let mut missing = Vec::new();
        let count = capture_frames(only, shard, |name, frame| {
            let old = committed.join(name);
            if !old.exists() {
                missing.push(name.to_owned());
            } else {
                let expected = image::open(&old)
                    .with_context(|| format!("read {}", old.display()))?
                    .into_rgba8();
                if within_one_step(&expected, frame) {
                    return Ok(());
                }
                differing.push(name.to_owned());
            }
            fs::create_dir_all(&scratch)
                .with_context(|| format!("create {}", scratch.display()))?;
            let actual = scratch.join(name);
            frame
                .save(&actual)
                .with_context(|| format!("write {}", actual.display()))
        })?;
        differing.sort();
        missing.sort();

        if differing.is_empty() && missing.is_empty() {
            println!("{count} images match {}", committed.display());
            return Ok(());
        }
        for name in &missing {
            println!("new     {name}");
        }
        for name in &differing {
            println!("changed {name}");
        }
        bail!(
            "{} changed and {} new image(s) under {}; review them, then run \
             `headless-visual capture` to accept",
            differing.len(),
            missing.len(),
            scratch.display()
        );
    }

    /// Strict pixel equality between a full catalog and selected catalog order.
    /// Uses the native platform renderer in both runs (Metal or software WGPU),
    /// never rewrites baselines, and deliberately fails even on one-step atlas
    /// rounding. This diagnostic is stricter than the ordinary visual gate.
    pub fn check_order(selected: &[String]) -> Result<()> {
        let target = selected
            .first()
            .context("check-order requires a target scene")?;
        for scene in selected {
            if gpui_kit::scenes::find(scene).is_none() {
                bail!("unknown scene `{scene}`");
            }
        }
        let names =
            gpui_kit::tokens::bundled().map(|theme| format!("{target}-{}.png", theme.meta.id));
        let output = repo_root().join("target/headless-order-check");
        fs::create_dir_all(output.join("full"))?;
        fs::create_dir_all(output.join("scoped"))?;
        fs::create_dir_all(output.join("diff-x64"))?;
        let mut full = std::collections::BTreeMap::new();
        capture_frames(&[], None, |name, frame| {
            if names.iter().any(|expected| expected == name) {
                frame.save(output.join("full").join(name))?;
                full.insert(name.to_owned(), frame.clone());
            }
            Ok(())
        })?;
        let mut exact = true;
        capture_frames(selected, None, |name, frame| {
            if !names.iter().any(|expected| expected == name) {
                return Ok(());
            }
            frame.save(output.join("scoped").join(name))?;
            let expected = full.get(name).context("full catalog omitted target")?;
            anyhow::ensure!(
                expected.dimensions() == frame.dimensions(),
                "{name} changed size"
            );
            let mut count = 0usize;
            let mut over_one = 0usize;
            let mut maximum = 0u8;
            let mut bounds = [frame.width(), frame.height(), 0, 0];
            let mut difference = image::RgbaImage::from_pixel(
                frame.width(),
                frame.height(),
                image::Rgba([0, 0, 0, 255]),
            );
            for ((x, y, a), b) in expected.enumerate_pixels().zip(frame.pixels()) {
                let delta =
                    a.0.iter()
                        .zip(b.0)
                        .map(|(&a, b)| a.abs_diff(b))
                        .max()
                        .expect("RGBA has four channels");
                let intensity = delta.saturating_mul(64);
                difference.put_pixel(x, y, image::Rgba([intensity, intensity, intensity, 255]));
                if delta != 0 {
                    count += 1;
                    over_one += usize::from(delta > 1);
                    maximum = maximum.max(delta);
                    bounds = [
                        bounds[0].min(x),
                        bounds[1].min(y),
                        bounds[2].max(x),
                        bounds[3].max(y),
                    ];
                }
            }
            difference.save(output.join("diff-x64").join(name))?;
            exact &= count == 0;
            println!(
                "{}",
                serde_json::json!({
                    "frame": name, "byte_identical": count == 0,
                    "different_pixels": count, "pixels_over_one": over_one,
                    "max_channel_step": maximum,
                    "inclusive_bounds": (count != 0).then_some(bounds),
                })
            );
            Ok(())
        })?;
        anyhow::ensure!(
            exact,
            "target pixels depend on capture order; inspect {} (no tolerance applied)",
            output.display()
        );
        Ok(())
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the manifest sits two levels under the repository root")
            .to_path_buf()
    }

    /// One baseline set per renderer, because Metal, llvmpipe, and WARP land
    /// antialiased edges differently.
    fn snapshots() -> PathBuf {
        let renderer = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        };
        repo_root()
            .join("snapshots")
            .join("headless")
            .join(renderer)
            .join("scenes")
    }

    /// Whether two captures agree to within one step on every channel.
    ///
    /// A differing size is never within tolerance: the harness asks the
    /// renderer for an exact size, so a different one means the harness itself
    /// changed rather than the picture.
    fn within_one_step(left: &image::RgbaImage, right: &image::RgbaImage) -> bool {
        if left.dimensions() != right.dimensions() {
            return false;
        }
        left.as_raw()
            .iter()
            .zip(right.as_raw())
            .all(|(left, right)| left.abs_diff(*right) <= 1)
    }

    /// Which scene the host shows.
    ///
    /// The host is rendered by GPUI, not called by this module, so the choice
    /// travels through a static the render function reads.
    static SCENE: Mutex<Option<&'static str>> = Mutex::new(None);

    struct Host;

    impl Render for Host {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            SemanticCoordinator::global(cx).begin_frame(window);
            let theme = Theme::get(cx).clone();
            let root = div().size_full().bg(theme.colors.canvas);
            let Some(name) = *SCENE.lock().expect("scene name is never poisoned") else {
                return root;
            };
            let scene = gpui_kit::scenes::find(name).expect("scene is registered");
            root.child((scene.build)(window, cx))
        }
    }

    /// Drives one headless window over the whole catalog.
    fn capture_frames(
        only: &[String],
        shard: Option<Shard>,
        mut accept: impl FnMut(&str, &image::RgbaImage) -> Result<()>,
    ) -> Result<usize> {
        for name in only {
            if gpui_kit::scenes::find(name).is_none() {
                bail!("unknown scene `{name}`");
            }
        }

        let initialization_started = Instant::now();
        // Only the bundled fonts take part. Loading the machine's own fonts
        // would shape text differently from one machine to the next, and the
        // exact per-adapter comparison above depends on there being no such
        // difference.
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
            // A capture is a still frame, so an animation in flight would put
            // an arbitrary phase into the file. Reduced motion settles a
            // one-shot at its end and holds a repeating one at its start.
            cx.set_reduce_motion(true);
        });
        let _diagnostics = cx.update(|cx| SemanticCoordinator::global(cx).arm());
        let window: AnyWindowHandle = cx
            .open_window(size(px(920.0), px(1000.0)), |_, cx: &mut App| {
                cx.new(|_| Host)
            })?
            .into();
        println!(
            "headless renderer initialized in {:.2?}",
            initialization_started.elapsed()
        );

        let wanted = |name: &str| only.is_empty() || only.iter().any(|only| only == name);
        let rendering_started = Instant::now();
        let mut count = 0;
        // Scene outside, theme inside, matching the macOS gallery: a scene may
        // install state on its first build, so its images are taken next to
        // each other rather than a whole catalog apart.
        for (scene_index, scene) in gpui_kit::scenes::catalog().into_iter().enumerate() {
            if !wanted(scene.name) || shard.is_some_and(|shard| !shard.includes(scene_index)) {
                continue;
            }
            let scene_started = Instant::now();
            for theme in gpui_kit::tokens::bundled() {
                let id = theme.meta.id.clone();
                let known = cx.update(|cx| {
                    let known = activate_theme(&id, cx);
                    if known {
                        set_layout_direction(gpui_kit::scenes::direction(scene.name), cx);
                    }
                    known
                });
                if !known {
                    bail!("unknown theme `{id}`");
                }
                *SCENE.lock().expect("scene name is never poisoned") = Some(scene.name);
                let frame = settled_image(&mut cx, window)
                    .with_context(|| format!("capture scene `{}` in `{id}`", scene.name))?;
                let name = format!("{}-{id}.png", scene.name);
                accept(&name, &frame)?;
                count += 1;
            }
            println!(
                "rendered scene `{}` in {:.2?}",
                scene.name,
                scene_started.elapsed()
            );
        }
        println!(
            "rendered and compared {count} images in {:.2?}",
            rendering_started.elapsed()
        );
        Ok(count)
    }

    /// Draws until two consecutive frames agree, then returns the agreed one.
    ///
    /// A scene may still be arranging itself across its first few draws, such
    /// as an editor that takes focus a frame after it appears. Time here is
    /// simulated, so settling is pumping the dispatcher and drawing again, not
    /// sleeping; the bound exists because a scene that never stops moving
    /// should fail loudly rather than hold the gate open.
    fn settled_image(
        cx: &mut HeadlessAppContext,
        window: AnyWindowHandle,
    ) -> Result<image::RgbaImage> {
        let mut previous: Option<image::RgbaImage> = None;
        for _ in 0..32 {
            cx.run_until_parked();
            cx.update_window(window, |_, window, cx| {
                window.draw(cx).clear(cx);
            })?;
            let frame = cx.capture_screenshot(window)?;
            if previous
                .as_ref()
                .is_some_and(|previous| previous.as_raw() == frame.as_raw())
            {
                return Ok(frame);
            }
            previous = Some(frame);
        }
        bail!("the scene did not settle within 32 draws");
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use image::{Rgba, RgbaImage};

        fn save_test_frame(name: &str, frame: &RgbaImage) -> Result<PathBuf> {
            let directory = repo_root().join("target");
            fs::create_dir_all(&directory)?;
            let path = directory.join(name);
            frame
                .save(&path)
                .with_context(|| format!("save {}", path.display()))?;
            Ok(path)
        }

        #[test]
        fn unclipped_deferred_pixels_keep_scale_and_restore_ancestor_masks() -> Result<()> {
            use gpui::{Bounds, ContentMask, Corners, RoundedClip, canvas, point};
            fn inner<R>(w: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
                let bounds = Bounds::new(point(px(10.), px(8.)), size(px(80.), px(60.)));
                w.with_content_mask(Some(ContentMask { bounds }), |w| {
                    w.with_rounded_content_mask(
                        RoundedClip::new(
                            bounds,
                            Corners {
                                top_left: px(12.),
                                ..Corners::default()
                            },
                        ),
                        f,
                    )
                })
            }
            fn outer<R>(w: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
                w.with_visual_scale(1.5, point(px(-10.), px(-5.)), |w| {
                    w.with_content_mask(
                        Some(ContentMask {
                            bounds: Bounds::new(point(px(0.), px(0.)), size(px(45.), px(35.))),
                        }),
                        |w| {
                            w.with_rounded_content_mask(
                                RoundedClip::new(
                                    Bounds::new(point(px(0.), px(0.)), size(px(60.), px(40.))),
                                    Corners {
                                        top_left: px(20.),
                                        ..Corners::default()
                                    },
                                ),
                                f,
                            )
                        },
                    )
                })
            }
            struct Host {
                mode: u8,
            }
            impl Render for Host {
                fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                    let mode = self.mode;
                    div().size_full().bg(gpui::black()).child(
                        canvas(
                            move |bounds, w, cx| {
                                let mut child = div()
                                    .size_full()
                                    .child(
                                        canvas(
                                            |_, _, _| {},
                                            |bounds, _, w, _| {
                                                w.without_content_masks(|w| {
                                                    inner(w, |w| {
                                                        w.paint_quad(gpui::fill(
                                                            bounds,
                                                            gpui::blue(),
                                                        ))
                                                    })
                                                });
                                            },
                                        )
                                        .absolute()
                                        .size_full(),
                                    )
                                    .child(
                                        canvas(
                                            |_, _, _| {},
                                            |bounds, _, w, _| {
                                                w.paint_quad(gpui::fill(bounds, gpui::red()))
                                            },
                                        )
                                        .absolute()
                                        .size_full(),
                                    );
                                if mode != 0 {
                                    let deferred = gpui::deferred(
                                        canvas(
                                            |_, _, _| {},
                                            |bounds, _, w, _| {
                                                inner(w, |w| {
                                                    w.paint_quad(gpui::fill(bounds, gpui::green()))
                                                })
                                            },
                                        )
                                        .absolute()
                                        .size_full(),
                                    )
                                    .preserve_accessibility();
                                    child = child.child(if mode == 2 {
                                        deferred.unclipped()
                                    } else {
                                        deferred
                                    });
                                }
                                let mut child = child.into_any_element();
                                child.layout_as_root(
                                    bounds.size.map(gpui::AvailableSpace::Definite),
                                    w,
                                    cx,
                                );
                                outer(w, |w| child.prepaint(w, cx));
                                child
                            },
                            |_, mut child, w, cx| outer(w, |w| child.paint(w, cx)),
                        )
                        .size_full(),
                    )
                }
            }
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
                activate_theme("studio-dark", cx);
            });
            let handle = cx.open_window(size(px(180.), px(130.)), |_, cx| {
                cx.new(|_| Host { mode: 0 })
            })?;
            let direct = settled_image(&mut cx, handle.into())?;
            let pixel = |image: &RgbaImage, x: u32, y: u32| {
                *image.get_pixel(x * image.width() / 180, y * image.height() / 130)
            };
            assert_eq!(
                pixel(&direct, 80, 40),
                Rgba([0, 0, 255, 255]),
                "rectangular escape and sibling restoration"
            );
            assert_eq!(
                pixel(&direct, 110, 60),
                Rgba([0, 0, 255, 255]),
                "rounded escape"
            );
            handle.update(&mut cx, |v, _, cx| {
                v.mode = 1;
                cx.notify();
            })?;
            let normal = settled_image(&mut cx, handle.into())?;
            assert_eq!(
                pixel(&normal, 110, 60),
                Rgba([0, 0, 255, 255]),
                "default deferred retains rounded ancestor"
            );
            handle.update(&mut cx, |v, _, cx| {
                v.mode = 2;
                cx.notify();
            })?;
            let escaped = settled_image(&mut cx, handle.into())?;
            assert_eq!(
                pixel(&escaped, 110, 60),
                Rgba([0, 128, 0, 255]),
                "opt-in paints at transformed point outside ancestor"
            );
            assert_eq!(
                pixel(&escaped, 21, 15),
                Rgba([255, 0, 0, 255]),
                "own rounded corner exposes sibling"
            );
            assert_eq!(
                pixel(&escaped, 145, 60),
                Rgba([0, 0, 0, 255]),
                "own rectangle remains"
            );
            assert_eq!(
                escaped,
                settled_image(&mut cx, handle.into())?,
                "repeated frame stable"
            );
            let mut comparison = RgbaImage::new(escaped.width() * 3, escaped.height());
            for (i, image) in [&direct, &normal, &escaped].into_iter().enumerate() {
                image::imageops::replace(
                    &mut comparison,
                    image,
                    (i as u32 * image.width()) as i64,
                    0,
                );
            }
            save_test_frame("unclipped-deferred.png", &comparison)?;
            Ok(())
        }

        #[test]
        fn visual_scale_rasterizes_glyph_and_svg_at_effective_resolution() -> Result<()> {
            use gpui::{Bounds, TransformationMatrix, canvas, point};
            struct Host {
                mode: u8,
            }
            impl Render for Host {
                fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                    let mode = self.mode;
                    div().size_full().bg(gpui::black()).child(canvas(
                        move |bounds, window, _| {
                            // Shape once at the original public font size in all
                            // modes. The reference below changes raster size only.
                            let line = window.text_system().shape_line("R".into(), px(20.), &[window.text_style().to_run(1)], None);
                            let run = &line.runs[0];
                            assert_eq!(bounds.size, size(px(260.), px(110.)), "layout stays fixed");
                            (run.font_id, run.glyphs[0].id)
                        },
                        move |_, (font, glyph), window, cx| {
                            let paint = |window: &mut Window| {
                                let (origin, font_size, svg_bounds) = if mode == 2 {
                                    (point(px(40.), px(70.)), px(30.), Bounds::new(point(px(130.), px(34.)), size(px(30.), px(18.))))
                                } else {
                                    (point(px(30.), px(50.)), px(20.), Bounds::new(point(px(90.), px(26.)), size(px(20.), px(12.))))
                                };
                                window.paint_glyph(origin, font, glyph, font_size, gpui::white()).expect("glyph paint");
                                window.paint_svg(svg_bounds, "scale-fixture.svg".into(), Some(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 12"><path d="M0 0h4v8h16v4H0z"/></svg>"#), TransformationMatrix::unit(), gpui::white(), cx).expect("svg paint");
                            };
                            if mode == 1 {
                                window.with_visual_scale(1.5, point(px(10.), px(10.)), paint);
                            } else { paint(window); }
                        },
                    ).size_full())
                }
            }
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
                activate_theme("studio-dark", cx);
            });
            let handle = cx.open_window(size(px(260.), px(110.)), |_, cx| {
                cx.new(|_| Host { mode: 0 })
            })?;
            let normal = settled_image(&mut cx, handle.into())?;
            handle.update(&mut cx, |view, _, cx| {
                view.mode = 1;
                cx.notify();
            })?;
            let scaled = settled_image(&mut cx, handle.into())?;
            handle.update(&mut cx, |view, _, cx| {
                view.mode = 2;
                cx.notify();
            })?;
            let reference = settled_image(&mut cx, handle.into())?;
            assert_ne!(normal, scaled, "actual foreground pixels move and grow");
            assert!(
                within_one_step(&scaled, &reference),
                "scale must rerasterize, not enlarge old glyph/SVG pixels"
            );
            handle.update(&mut cx, |view, _, cx| {
                view.mode = 0;
                cx.notify();
            })?;
            assert_eq!(
                normal,
                settled_image(&mut cx, handle.into())?,
                "release restores original foreground"
            );
            save_test_frame("headless-visual-scale.png", &scaled)?;
            Ok(())
        }

        /// Runs through the platform's actual headless renderer (Metal on macOS),
        /// not a window or a baseline. Paths must use the same once-only clip as
        /// quads, and nested optics must retain their original source pixels.
        #[test]
        fn framework_rounded_subtree_clip_pixels() -> Result<()> {
            use gpui::{
                BackdropGlass, Background, Bounds, ClipChain, ClipId, ContentMask, Corners,
                DevicePixels, GlassMaterial, Hsla, Path, Quad, RoundedClip, ScaledPixels, Scene,
                point,
            };
            let mut renderer = gpui_platform::current_headless_renderer()
                .expect("platform headless renderer required");
            let bounds = Bounds::new(
                point(ScaledPixels(0.), ScaledPixels(0.)),
                size(ScaledPixels(128.), ScaledPixels(128.)),
            );
            let quad = Quad {
                order: 0,
                border_style: Default::default(),
                bounds,
                content_mask: ContentMask { bounds },
                background: Background::from(Hsla::black()),
                border_color: Hsla::transparent_black(),
                corner_radii: Corners::default(),
                border_widths: Default::default(),
                clip_id: ClipId::NONE,
            };
            let mut chain = ClipChain::default();
            chain.push(RoundedClip::new(
                Bounds::new(point(px(16.), px(16.)), size(px(96.), px(96.))),
                Corners {
                    top_left: px(40.),
                    top_right: px(0.),
                    bottom_right: px(24.),
                    bottom_left: px(0.),
                },
            ));
            for _ in 0..80 {
                chain.push(RoundedClip::new(
                    Bounds::new(point(px(17.), px(17.)), size(px(94.), px(94.))),
                    Corners::default(),
                ));
            }
            let extent = size(DevicePixels(128), DevicePixels(128));
            let mut reference = None;
            for path in [false, true] {
                let mut scene = Scene::default();
                scene.insert_primitive(quad);
                scene.with_clip_chain(&chain, 1., |scene| {
                    if path {
                        let mut path = Path::new(point(px(0.), px(0.)));
                        path.line_to(point(px(128.), px(0.)));
                        path.line_to(point(px(128.), px(128.)));
                        path.line_to(point(px(0.), px(128.)));
                        let mut path = path.scale(1.);
                        path.content_mask = quad.content_mask;
                        path.color = Background::from(Hsla::white());
                        scene.insert_primitive(path);
                    } else {
                        scene.insert_primitive(Quad {
                            background: Background::from(Hsla::white()),
                            ..quad
                        });
                    }
                });
                scene.finish();
                let frame = renderer.render_scene_to_image(&scene, extent)?;
                for (x, y) in [(18, 18), (109, 109), (8, 60)] {
                    assert_eq!(frame.get_pixel(x, y).0, [0, 0, 0, 255]);
                }
                for (x, y) in [(64, 64), (108, 20), (20, 108)] {
                    assert_eq!(frame.get_pixel(x, y).0, [255; 4]);
                }
                if let Some(reference) = reference.as_ref() {
                    assert!(
                        within_one_step(reference, &frame),
                        "path clip applies once, in window coordinates"
                    );
                } else {
                    reference = Some(frame);
                }
            }
            let mut images = Vec::new();
            let mut probes = Vec::new();
            for clipped in [false, true] {
                let mut scene = Scene::default();
                scene.insert_primitive(Quad {
                    background: Background::from(gpui::hsla(0., 0., 0.3, 1.)),
                    ..quad
                });
                let glass = BackdropGlass {
                    order: 0,
                    bounds,
                    content_mask: quad.content_mask,
                    corner_radii: Corners::default(),
                    material: GlassMaterial {
                        blur_radius: ScaledPixels(12.),
                        wash: gpui::Rgba {
                            r: 0.,
                            g: 0.,
                            b: 0.,
                            a: 0.4,
                        },
                        probe: 0,
                        ..GlassMaterial::clear()
                    },
                    lobes: Default::default(),
                    lobe_count: 0,
                    clip_id: ClipId::NONE,
                };
                scene.insert_backdrop_glass(glass);
                let paint = |scene: &mut Scene| {
                    scene.insert_backdrop_glass(BackdropGlass {
                        material: GlassMaterial {
                            probe: 1,
                            ..glass.material
                        },
                        ..glass
                    })
                };
                if clipped {
                    scene.with_clip_chain(&chain, 1., paint);
                } else {
                    paint(&mut scene);
                }
                scene.finish();
                images.push(renderer.render_scene_to_image(&scene, extent)?);
                probes.push(
                    renderer
                        .backdrop_luminance(1)
                        .expect("nested probe completes"),
                );
            }
            assert_eq!(probes[0], probes[1], "clip never filters probe source");
            assert_eq!(images[0].get_pixel(64, 64), images[1].get_pixel(64, 64));
            assert!(
                images[1].get_pixel(18, 18)[0] > images[0].get_pixel(18, 18)[0] + 10,
                "rejected rounded corner retains earlier glass rather than applying later glass"
            );
            Ok(())
        }

        #[test]
        fn glass_focus_is_an_inner_report_even_after_budget_refusal() -> Result<()> {
            use gpui::rgb;
            use gpui_kit::foundation::ThemeOverlay;
            use gpui_kit::overlay::{OverlaySurface, surface};
            use gpui_kit::prelude::{Glass, GlassPreset};
            use gpui_kit_theme::{Elevation, Radius};

            const COUNT: usize = gpui::MAX_BACKDROP_GLASS_SURFACES_PER_FRAME + 12;
            struct FocusHost;
            impl Render for FocusHost {
                fn render(
                    &mut self,
                    window: &mut Window,
                    cx: &mut Context<Self>,
                ) -> impl IntoElement {
                    SemanticCoordinator::global(cx).begin_frame(window);
                    div()
                        .size_full()
                        .bg(rgb(0x181818))
                        .children((0..COUNT).map(|slot| {
                            let theme = Theme::studio_dark()
                                .modify(|theme| {
                                    theme.colors.focus = rgb(0xff0088).into();
                                    theme.effects.focus_ring_width = 4.;
                                })
                                .with_reduce_transparency(slot % 4 == 3);
                            let preset = match slot % 4 {
                                0 => GlassPreset::Liquid,
                                1 | 3 => GlassPreset::Clear,
                                _ => GlassPreset::Frosted,
                            };
                            // Deliberately paint over the top edge: the report
                            // must be painted after this child.
                            let content = div().relative().w(px(120.)).h(px(48.)).child(
                                div()
                                    .absolute()
                                    .left(px(40.))
                                    .top_0()
                                    .w(px(40.))
                                    .h(px(8.))
                                    .bg(rgb(0x00ff00)),
                            );
                            let ident = format!("test.focus.slot.{slot}");
                            let pane = if slot == 1 {
                                surface(ident, &theme, OverlaySurface::MEDIA_CAPTION)
                                    .focused(true)
                                    .child(content)
                                    .into_any_element()
                            } else {
                                ThemeOverlay::theme(
                                    theme,
                                    Glass::new(ident)
                                        .preset(preset)
                                        .dimmed(true)
                                        .focused(true)
                                        .radius(Radius::Pill)
                                        .elevation(Elevation::Flat)
                                        .child(content),
                                )
                                .into_any_element()
                            };
                            div()
                                .absolute()
                                .left(px(12. + (slot % 7) as f32 * 144.))
                                .top(px(12. + (slot / 7) as f32 * 72.))
                                .child(pane)
                        }))
                }
            }
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
                activate_theme("studio-dark", cx);
            });
            let window: AnyWindowHandle = cx
                .open_window(size(px(1020.), px(310.)), |_, cx| cx.new(|_| FocusHost))?
                .into();
            let frame = settled_image(&mut cx, window)?;
            save_test_frame("headless-glass-focus.png", &frame)?;
            let scale = frame.width() as f32 / 1020.;
            let sample = |x: f32, y: f32| *frame.get_pixel((x * scale) as u32, (y * scale) as u32);
            for slot in 0..COUNT {
                let x = 12. + (slot % 7) as f32 * 144.;
                let y = 12. + (slot / 7) as f32 * 72.;
                assert_eq!(
                    sample(x + 60., y + 3.),
                    Rgba([255, 0, 136, 255]),
                    "slot {slot}: token-width report after child"
                );
                assert_eq!(
                    sample(x + 60., y - 2.),
                    Rgba([24, 24, 24, 255]),
                    "slot {slot}: no external halo"
                );
                assert_eq!(
                    sample(x, y),
                    Rgba([24, 24, 24, 255]),
                    "slot {slot}: fitted rounded corner"
                );
            }
            Ok(())
        }

        #[test]
        fn cover_image_rounds_all_corners_in_a_clipped_media_card() -> Result<()> {
            use gpui::{DevicePixels, ObjectFit, RenderImage, img, rgb};
            use gpui_kit::prelude::{Glass, GlassPreset};
            use gpui_kit_theme::{Elevation, Radius};

            struct ImageHost {
                width: f32,
            }
            impl Render for ImageHost {
                fn render(
                    &mut self,
                    window: &mut Window,
                    cx: &mut Context<Self>,
                ) -> impl IntoElement {
                    SemanticCoordinator::global(cx).begin_frame(window);
                    let media = Arc::new(
                        RenderImage::from_rgba(
                            size(DevicePixels(480), DevicePixels(144)),
                            [40, 180, 90, 255].repeat(480 * 144),
                        )
                        .expect("fixture dimensions"),
                    );
                    div().size_full().bg(rgb(0x101010)).child(
                        div()
                            .relative()
                            .w(px(self.width))
                            .h(px(220.))
                            .rounded(px(12.))
                            .overflow_hidden()
                            .child(
                                img(media)
                                    .size_full()
                                    .rounded(px(12.))
                                    .object_fit(ObjectFit::Cover),
                            )
                            .child(
                                div().absolute().left_0().right_0().bottom_0().child(
                                    Glass::new("test.image-caption")
                                        .preset(GlassPreset::Clear)
                                        .dimmed(true)
                                        .radius(Radius::Card)
                                        .elevation(Elevation::Flat)
                                        .child(div().w_full().h(px(60.)).child("Media caption")),
                                ),
                            ),
                    )
                }
            }
            for width in [480., 880.] {
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
                    activate_theme("studio-dark", cx);
                });
                let window: AnyWindowHandle = cx
                    .open_window(size(px(width + 20.), px(240.)), |_, cx| {
                        cx.new(|_| ImageHost { width })
                    })?
                    .into();
                let frame = settled_image(&mut cx, window)?;
                save_test_frame(&format!("headless-rounded-cover-image-{width}.png"), &frame)?;
                let scale = frame.width() as f32 / (width + 20.);
                let sample =
                    |x: f32, y: f32| *frame.get_pixel((x * scale) as u32, (y * scale) as u32);
                for (x, y) in [(1., 1.), (width - 2., 1.), (1., 218.), (width - 2., 218.)] {
                    assert_eq!(
                        sample(x, y),
                        sample(width + 10., 230.),
                        "width {width}: outside rounded corner ({x}, {y})"
                    );
                }
                assert_eq!(sample(240., 110.), Rgba([40, 180, 90, 255]));
            }
            Ok(())
        }

        #[test]
        fn glass_press_changes_optics_without_moving_semantic_bounds() -> Result<()> {
            use gpui::{
                MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, point,
                rgb,
            };
            use gpui_kit::prelude::{Glass, GlassPreset};
            use gpui_kit_theme::Elevation;

            struct PressHost;
            impl Render for PressHost {
                fn render(
                    &mut self,
                    window: &mut Window,
                    cx: &mut Context<Self>,
                ) -> impl IntoElement {
                    SemanticCoordinator::global(cx).begin_frame(window);
                    div()
                        .size_full()
                        .bg(rgb(0x202020))
                        .children((0..32).map(|x| {
                            div()
                                .absolute()
                                .left(px(x as f32 * 8.))
                                .top_0()
                                .h_full()
                                .w(px(2.))
                                .bg(rgb(0xe0e0e0))
                        }))
                        .child(
                            div().absolute().left(px(40.)).top(px(32.)).child(
                                Glass::new("test.glass.press")
                                    .preset(GlassPreset::Lens)
                                    .elevation(Elevation::Flat)
                                    .radius_px(32.)
                                    .refraction(1.)
                                    .thickness(18.)
                                    .backdrop_depth(24.)
                                    .pressable(true)
                                    .child(div().w(px(168.)).h(px(112.))),
                            ),
                        )
                }
            }
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
                activate_theme("studio-dark", cx);
            });
            let coordinator = cx.update(|cx| SemanticCoordinator::global(cx));
            let _diagnostics = coordinator.arm();
            let window: AnyWindowHandle = cx
                .open_window(size(px(256.), px(176.)), |_, cx| cx.new(|_| PressHost))?
                .into();
            let initial = settled_image(&mut cx, window)?;
            let bounds = || {
                coordinator
                    .snapshot(window.window_id())
                    .unwrap()
                    .find("test.glass.press")
                    .unwrap()
                    .bounds
            };
            let original_bounds = bounds();
            assert!(original_bounds.width > 0. && original_bounds.height > 0.);
            let position = point(
                px(original_bounds.x + original_bounds.width / 2.),
                px(original_bounds.y + original_bounds.height / 2.),
            );
            cx.update_window(window, |_, window, cx| {
                window.dispatch_event(
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position,
                        ..Default::default()
                    }),
                    cx,
                );
                window.dispatch_event(
                    PlatformInput::MouseDown(MouseDownEvent {
                        position,
                        button: MouseButton::Left,
                        click_count: 1,
                        ..Default::default()
                    }),
                    cx,
                );
            })?;
            let pressed = settled_image(&mut cx, window)?;
            assert_eq!(bounds(), original_bounds);
            let changed = initial
                .pixels()
                .zip(pressed.pixels())
                .filter(|(a, b)| a.0[0].abs_diff(b.0[0]) > 8)
                .count();
            assert!(
                changed > 100,
                "press must displace real backdrop pixels, changed={changed}"
            );
            cx.update_window(window, |_, window, cx| {
                window.dispatch_event(
                    PlatformInput::MouseUp(MouseUpEvent {
                        position,
                        button: MouseButton::Left,
                        click_count: 1,
                        ..Default::default()
                    }),
                    cx,
                );
            })?;
            let released = settled_image(&mut cx, window)?;
            assert_eq!(bounds(), original_bounds);
            assert!(
                within_one_step(&initial, &released),
                "release restores the original optics"
            );
            save_test_frame("headless-glass-released.png", &initial)?;
            save_test_frame("headless-glass-pressed.png", &pressed)?;
            Ok(())
        }

        #[test]
        fn glass_foreground_press_renders_about_each_logical_center() -> Result<()> {
            use gpui::{
                MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, point,
                rgb,
            };
            use gpui_kit::{
                motion::{MotionPolicy, MotionRole},
                prelude::{Glass, GlassGroup, GlassPreset, ThemeOverlay},
            };
            use gpui_kit_theme::Elevation;
            use std::time::Duration;

            struct Host {
                theme: Theme,
                group: bool,
            }
            fn content() -> impl IntoElement {
                div()
                    .relative()
                    .w(px(200.))
                    .h(px(120.))
                    .child(
                        div()
                            .absolute()
                            .left(px(23.))
                            .top(px(19.))
                            .w(px(37.))
                            .h(px(21.))
                            .bg(rgb(0xff0000)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(131.))
                            .top(px(76.))
                            .w(px(43.))
                            .h(px(17.))
                            .bg(rgb(0x00ff00)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(67.))
                            .top(px(45.))
                            .text_size(px(24.))
                            .text_color(rgb(0xffffff))
                            .child("Ag7"),
                    )
            }
            impl Render for Host {
                fn render(
                    &mut self,
                    window: &mut Window,
                    cx: &mut Context<Self>,
                ) -> impl IntoElement {
                    SemanticCoordinator::global(cx).begin_frame(window);
                    let surface = if self.group {
                        GlassGroup::new("press.outer")
                            .preset(GlassPreset::Clear)
                            .radius(gpui_kit_theme::Radius::Card)
                            .pressable(true)
                            .pane("press.a", content())
                            .pane("press.b", content())
                            .into_any_element()
                    } else {
                        Glass::new("press.outer")
                            .preset(GlassPreset::Clear)
                            .elevation(Elevation::Flat)
                            .radius_px(12.)
                            .pressable(true)
                            .child(content())
                            .into_any_element()
                    };
                    div().size_full().bg(rgb(0x202020)).child(
                        div()
                            .absolute()
                            .left(px(30.))
                            .top(px(30.))
                            .child(ThemeOverlay::theme(self.theme.clone(), surface)),
                    )
                }
            }
            // An exaggerated response makes wrong pivots and integer rounding visible;
            // the real token value is separately rendered for human review.
            for group in [false, true] {
                for (scale_override, reduced) in
                    [(Some(1.24), false), (Some(1.24), true), (None, false)]
                {
                    let theme = Theme::studio_dark().modify(|theme| {
                        if let Some(scale) = scale_override {
                            theme.effects.glass_press_scale = scale;
                        }
                    });
                    let scale = theme.effects.glass_press_scale;
                    let spring = MotionPolicy::spec(MotionRole::Tracking, &theme)
                        .spring()
                        .expect("press tracks a spring");
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
                        cx.set_reduce_motion(reduced);
                    });
                    let coordinator = cx.update(|cx| SemanticCoordinator::global(cx));
                    let _diagnostics = coordinator.arm();
                    let window: AnyWindowHandle = cx
                        .open_window(size(px(500.), px(180.)), |_, cx| {
                            cx.new(|_| Host { theme, group })
                        })?
                        .into();
                    let initial = settled_image(&mut cx, window)?;
                    let ids = if group {
                        vec!["press.outer", "press.a", "press.b"]
                    } else {
                        vec!["press.outer"]
                    };
                    let bounds = || {
                        let snapshot = coordinator.snapshot(window.window_id()).unwrap();
                        ids.iter()
                            .map(|id| snapshot.find(id).unwrap().bounds)
                            .collect::<Vec<_>>()
                    };
                    let original = bounds();
                    let panes = if group { &original[1..] } else { &original[..] };
                    let position = point(px(panes[0].x + 100.), px(panes[0].y + 60.));
                    cx.update_window(window, |_, window, cx| {
                        window.dispatch_event(
                            PlatformInput::MouseMove(MouseMoveEvent {
                                position,
                                ..Default::default()
                            }),
                            cx,
                        );
                        window.dispatch_event(
                            PlatformInput::MouseDown(MouseDownEvent {
                                position,
                                button: MouseButton::Left,
                                click_count: 1,
                                ..Default::default()
                            }),
                            cx,
                        );
                    })?;
                    // Establish the target at t=0; settled_image intentionally never advances time.
                    let _ = settled_image(&mut cx, window)?;
                    let mut elapsed = 0;
                    let mut held = initial.clone();
                    for millis in [16, 96, 2000] {
                        cx.advance_clock(Duration::from_millis(millis - elapsed));
                        elapsed = millis;
                        held = settled_image(&mut cx, window)?;
                        assert_eq!(
                            bounds(),
                            original,
                            "layout/outer bounds: group={group}, t={millis}"
                        );
                        let progress = if millis == 2000 {
                            1.
                        } else {
                            spring.value(Duration::from_millis(millis))
                        };
                        let factor = if reduced {
                            1.
                        } else {
                            1. + (scale - 1.) * progress
                        };
                        let device_scale = held.width() as f32 / 500.;
                        for pane in panes {
                            for (color, rect) in [
                                ([255, 0, 0, 255], [23., 19., 60., 40.]),
                                ([0, 255, 0, 255], [131., 76., 174., 93.]),
                            ] {
                                let mut actual = [u32::MAX, u32::MAX, 0, 0];
                                for (x, y, pixel) in held.enumerate_pixels() {
                                    if pixel.0 == color
                                        && (x as f32 / device_scale) >= pane.x
                                        && (x as f32 / device_scale) < pane.x + pane.width
                                    {
                                        actual[0] = actual[0].min(x);
                                        actual[1] = actual[1].min(y);
                                        actual[2] = actual[2].max(x + 1);
                                        actual[3] = actual[3].max(y + 1);
                                    }
                                }
                                // Independent affine geometry, not production transform helpers.
                                let expected = [
                                    pane.x + 100. + (rect[0] - 100.) * factor,
                                    pane.y + 60. + (rect[1] - 60.) * factor,
                                    pane.x + 100. + (rect[2] - 100.) * factor,
                                    pane.y + 60. + (rect[3] - 60.) * factor,
                                ];
                                for edge in 0..4 {
                                    assert!(
                                        (actual[edge] as f32 - expected[edge] * device_scale).abs()
                                            <= 1.1,
                                        "group={group}, reduced={reduced}, t={millis}, factor={factor}, edge={edge}: {actual:?} != {expected:?}"
                                    );
                                }
                            }
                        }
                        if reduced {
                            assert!(
                                within_one_step(&initial, &held),
                                "reduced motion must leave foreground unscaled"
                            );
                        }
                    }
                    if !reduced {
                        let glyph_changes = initial
                            .pixels()
                            .zip(held.pixels())
                            .filter(|(a, b)| {
                                (a.0[..3].iter().all(|v| *v > 220)
                                    || b.0[..3].iter().all(|v| *v > 220))
                                    && a.0[..3]
                                        .iter()
                                        .zip(&b.0[..3])
                                        .any(|(a, b)| a.abs_diff(*b) > 8)
                            })
                            .count();
                        assert!(glyph_changes > 20, "glyphs must scale too: {glyph_changes}");
                    }
                    // Material perimeter remains at rest: only the interior is transformed.
                    for (x, y, pixel) in initial.enumerate_pixels() {
                        let ds = initial.width() as f32 / 500.;
                        if !panes.iter().any(|p| {
                            x as f32 / ds > p.x + 2.
                                && (x as f32 / ds) < p.x + p.width - 2.
                                && y as f32 / ds > p.y + 2.
                                && (y as f32 / ds) < p.y + p.height - 2.
                        }) {
                            assert!(
                                pixel
                                    .0
                                    .iter()
                                    .zip(&held.get_pixel(x, y).0)
                                    .all(|(a, b)| a.abs_diff(*b) <= 1),
                                "fixed material perimeter at {x},{y}"
                            );
                        }
                    }
                    cx.update_window(window, |_, window, cx| {
                        window.dispatch_event(
                            PlatformInput::MouseUp(MouseUpEvent {
                                position,
                                button: MouseButton::Left,
                                click_count: 1,
                                ..Default::default()
                            }),
                            cx,
                        );
                    })?;
                    let _ = settled_image(&mut cx, window)?;
                    cx.advance_clock(Duration::from_secs(2));
                    let released = settled_image(&mut cx, window)?;
                    assert_eq!(bounds(), original);
                    assert!(
                        within_one_step(&initial, &released),
                        "release restores foreground"
                    );
                    if scale_override.is_none() {
                        let directory = repo_root().join(".amp/in/artifacts");
                        std::fs::create_dir_all(&directory)?;
                        for (name, image) in [
                            ("before", &initial),
                            ("held", &held),
                            ("released", &released),
                        ] {
                            image.save(directory.join(format!(
                                "glass-foreground-default-group-{group}-{name}.png"
                            )))?;
                        }
                    }
                }
            }
            Ok(())
        }

        #[test]
        fn clear_pill_dims_media_inside_a_clipped_card() -> Result<()> {
            use gpui::{DevicePixels, ObjectFit, RenderImage, img};
            use gpui_kit::foundation::Sizable;
            use gpui_kit::prelude::{Button, ControlSize, Glass, GlassPreset};
            use gpui_kit_theme::{Elevation, Radius};

            struct PillHost;
            impl Render for PillHost {
                fn render(
                    &mut self,
                    window: &mut Window,
                    cx: &mut Context<Self>,
                ) -> impl IntoElement {
                    SemanticCoordinator::global(cx).begin_frame(window);
                    let media = Arc::new(
                        RenderImage::from_rgba(
                            size(DevicePixels(4), DevicePixels(4)),
                            [100, 80, 60, 255].repeat(16),
                        )
                        .expect("fixture image dimensions"),
                    );
                    div().size_full().overflow_hidden().child(
                        div()
                            .relative()
                            .w(px(280.))
                            .h(px(130.))
                            .rounded(px(16.))
                            .overflow_hidden()
                            .child(img(media).size_full().object_fit(ObjectFit::Cover))
                            .child(
                                div().absolute().right(px(20.)).bottom(px(20.)).child(
                                    Glass::new("test.clear-pill")
                                        .preset(GlassPreset::Clear)
                                        .dimmed(true)
                                        .radius(Radius::Pill)
                                        .elevation(Elevation::Flat)
                                        .child(
                                            div().w(px(200.)).h(px(41.)).child(
                                                Button::new("test.clear-pill.action")
                                                    .ghost()
                                                    .control_size(ControlSize::Xs)
                                                    .label("Generate image"),
                                            ),
                                        ),
                                ),
                            ),
                    )
                }
            }

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
                activate_theme("studio-light", cx);
            });
            let window: AnyWindowHandle = cx
                .open_window(size(px(300.), px(150.)), |_, cx| cx.new(|_| PillHost))?
                .into();
            let frame = settled_image(&mut cx, window)?;
            let output = save_test_frame("headless-clear-pill.png", &frame)?;
            let scale = frame.width() as f32 / 300.;
            let sample = |x: f32, y: f32| frame.get_pixel((x * scale) as u32, (y * scale) as u32);
            let raw = sample(150., 40.);
            let inside = sample(150., 100.);
            // Uniform media excludes lens displacement. At the flat interior,
            // Clear applies 35% black, gain 1.042, then white lift 0.075.
            // Flat elevation isolates the material from shadow compositing.
            // This checks actual paint ordering as well as Pill radius fitting.
            for channel in 0..3 {
                let expected = (raw[channel] as f32 * 0.65 * 1.042 + 255. * 0.075).round() as i16;
                assert!(
                    (i16::from(inside[channel]) - expected).abs() <= 2,
                    "channel {channel}: raw={raw:?}, inside={inside:?}, expected={expected}; {}",
                    output.display()
                );
            }
            assert!(
                sample(150., 69.5)[0] > inside[0] + 10,
                "Pill retains its rim"
            );
            assert_eq!(sample(61., 70.), raw, "outside the fitted arc stays media");
            Ok(())
        }

        #[test]
        fn comparison_allows_one_channel_step() {
            let expected = RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 255]));
            let actual = RgbaImage::from_pixel(1, 1, Rgba([11, 19, 30, 254]));

            assert!(within_one_step(&expected, &actual));
        }

        #[test]
        fn comparison_rejects_larger_changes_and_sizes() {
            let expected = RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 255]));
            let changed = RgbaImage::from_pixel(1, 1, Rgba([12, 20, 30, 255]));
            let resized = RgbaImage::from_pixel(2, 1, Rgba([10, 20, 30, 255]));

            assert!(!within_one_step(&expected, &changed));
            assert!(!within_one_step(&expected, &resized));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shard_assigns_every_scene_to_exactly_one_worker() -> Result<()> {
        let shards = (0..8)
            .map(|index| Shard::parse(&format!("{index}/8")))
            .collect::<Result<Vec<_>>>()?;

        for scene in 0..gpui_kit::scenes::catalog().len() {
            assert_eq!(
                shards.iter().filter(|shard| shard.includes(scene)).count(),
                1
            );
        }
        Ok(())
    }

    #[test]
    fn shard_rejects_invalid_coordinates() {
        for invalid in ["", "1", "x/4", "1/x", "0/0", "4/4", "1/2/3"] {
            assert!(Shard::parse(invalid).is_err(), "accepted `{invalid}`");
        }
    }
}
