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
    let rest = args.collect::<Vec<_>>();
    match command.as_deref() {
        Some("capture") => imp::capture(&rest),
        Some("check") => imp::check(&rest),
        _ => anyhow::bail!("usage: headless-visual <capture|check> [scene...]"),
    }
}

mod imp {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use anyhow::{Context as _, Result, bail};
    use gpui::{
        AnyWindowHandle, App, Context, HeadlessAppContext, IntoElement, Render, Window, div,
        prelude::*, px, size,
    };
    use gpui_kit::prelude::set_layout_direction;
    use gpui_kit_semantics::SemanticRegistry;
    use gpui_kit_theme::{Theme, activate_theme};

    pub fn capture(only: &[String]) -> Result<()> {
        let directory = snapshots();
        fs::create_dir_all(&directory)
            .with_context(|| format!("create {}", directory.display()))?;
        let count = capture_frames(only, |name, frame| {
            let path = directory.join(name);
            frame
                .save(&path)
                .with_context(|| format!("write {}", path.display()))
        })?;
        println!("captured {count} images into {}", directory.display());
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
    pub fn check(only: &[String]) -> Result<()> {
        let committed = snapshots();
        let scratch = repo_root().join("target").join("headless-scene-check");
        if scratch.exists() {
            fs::remove_dir_all(&scratch).with_context(|| format!("clear {}", scratch.display()))?;
        }
        let mut differing = Vec::new();
        let mut missing = Vec::new();
        let count = capture_frames(only, |name, frame| {
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
            SemanticRegistry::global(cx).begin_frame();
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
        mut accept: impl FnMut(&str, &image::RgbaImage) -> Result<()>,
    ) -> Result<usize> {
        for name in only {
            if gpui_kit::scenes::find(name).is_none() {
                bail!("unknown scene `{name}`");
            }
        }

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
        let window: AnyWindowHandle = cx
            .open_window(size(px(920.0), px(1000.0)), |_, cx: &mut App| {
                cx.new(|_| Host)
            })?
            .into();

        let wanted = |name: &str| only.is_empty() || only.iter().any(|only| only == name);
        let mut count = 0;
        // Scene outside, theme inside, matching the macOS gallery: a scene may
        // install state on its first build, so its images are taken next to
        // each other rather than a whole catalog apart.
        for scene in gpui_kit::scenes::catalog() {
            if !wanted(scene.name) {
                continue;
            }
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
        }
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
