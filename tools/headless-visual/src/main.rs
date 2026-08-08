//! Renders the scene catalog without a window system and compares the pixels.
//!
//! This is the visual gate for the platforms the macOS gallery cannot cover.
//! GPUI draws each scene through its wgpu renderer into an offscreen texture
//! and the pixels are read straight back, so no GPU hardware, compositor, or
//! display server takes part. Text is shaped by cosmic-text from the fonts
//! this repository bundles, and time is simulated, which together make the
//! same scene reproducible on every machine. The comparison permits one 8-bit
//! channel step because llvmpipe and WARP can land an antialiased edge one
//! step apart; anything a component changes moves further than that.
//!
//! The baseline lives in `snapshots/headless/scenes`, beside but distinct
//! from the macOS baseline: CoreText and Metal land antialiased edges
//! differently from cosmic-text and a software rasterizer, and pretending
//! those were one picture would make both gates lie.

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

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod imp {
    use anyhow::{Result, bail};

    pub fn capture(_rest: &[String]) -> Result<()> {
        unavailable()
    }

    pub fn check(_rest: &[String]) -> Result<()> {
        unavailable()
    }

    fn unavailable() -> Result<()> {
        bail!(
            "the headless visual gate runs on Linux and Windows; on macOS the native \
             gate is `cargo run -p xtask -- scenes check`"
        );
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
mod imp {
    use std::fs;
    use std::path::{Path, PathBuf};
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
        let count = capture_into(&directory, only)?;
        println!("captured {count} images into {}", directory.display());
        Ok(())
    }

    /// Captures into a scratch directory and reports every image that differs
    /// from the committed one.
    ///
    /// Linux and Windows shape the same bundled fonts, but their software
    /// adapters can still land an antialiased edge one channel step apart.
    /// The gate absorbs only that smallest representable difference.
    pub fn check(only: &[String]) -> Result<()> {
        let committed = snapshots();
        let scratch = repo_root().join("target").join("headless-scene-check");
        if scratch.exists() {
            fs::remove_dir_all(&scratch).with_context(|| format!("clear {}", scratch.display()))?;
        }
        let count = capture_into(&scratch, only)?;

        let mut differing = Vec::new();
        let mut missing = Vec::new();
        for entry in fs::read_dir(&scratch)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let old = committed.join(&name);
            if !old.exists() {
                missing.push(name);
                continue;
            }
            if !same_pixels(&old, &entry.path()).with_context(|| format!("compare {name}"))? {
                differing.push(name);
            }
        }
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

    fn snapshots() -> PathBuf {
        repo_root()
            .join("snapshots")
            .join("headless")
            .join("scenes")
    }

    const CHANNEL_TOLERANCE: u8 = 1;

    fn same_pixels(left: &Path, right: &Path) -> Result<bool> {
        let left = image::open(left)?.into_rgba8();
        let right = image::open(right)?.into_rgba8();
        Ok(left.dimensions() == right.dimensions()
            && left
                .as_raw()
                .iter()
                .zip(right.as_raw())
                .all(|(left, right)| left.abs_diff(*right) <= CHANNEL_TOLERANCE))
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
    fn capture_into(directory: &Path, only: &[String]) -> Result<usize> {
        for name in only {
            if gpui_kit::scenes::find(name).is_none() {
                bail!("unknown scene `{name}`");
            }
        }
        fs::create_dir_all(directory).with_context(|| format!("create {}", directory.display()))?;

        // Only the bundled fonts take part. Loading the machine's own fonts
        // would shape text differently from one machine to the next, far beyond
        // the one-channel renderer tolerance above.
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
                let path = directory.join(format!("{}-{id}.png", scene.name));
                frame
                    .save(&path)
                    .with_context(|| format!("write {}", path.display()))?;
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
}
