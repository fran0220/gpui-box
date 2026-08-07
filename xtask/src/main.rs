use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use gpui_kit_tokens::{
    BorderWeight, ControlSize, Density, Elevation, Layer, MotionEasing, OpacityRole, Radius, Space,
    SpringPreset, TokenDocument, bundled, contrast,
};

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let command = (args.next(), args.next());
    let rest = args.collect::<Vec<_>>();
    match (command.0.as_deref(), command.1.as_deref()) {
        (Some("tokens"), Some("generate")) => tokens(false),
        (Some("tokens"), Some("check")) => tokens(true),
        (Some("scenes"), Some("list")) => scenes_list(),
        (Some("scenes"), Some("capture")) => scenes_capture(&rest),
        (Some("scenes"), Some("check")) => scenes_check(&rest),
        (Some("gate"), None) => gate(false),
        (Some("gate"), Some("full")) => gate(true),
        _ => bail!(
            "usage: cargo xtask <tokens generate|tokens check|scenes list|\
             scenes capture [name...]|scenes check [name...]|gate [full]>"
        ),
    }
}

fn scenes_list() -> Result<()> {
    for scene in gpui_kit::scenes::catalog() {
        println!("{}", scene.name);
    }
    Ok(())
}

/// Renders scenes in every bundled theme to reviewable images.
///
/// Naming scenes captures only those, which is what a change to one component
/// needs. Naming none captures the catalog.
fn scenes_capture(only: &[String]) -> Result<()> {
    let directory = snapshots();
    let count = capture_into(&directory, only)?;
    println!("captured {count} images into {}", directory.display());
    Ok(())
}

/// Captures into a scratch directory and reports every image that differs from
/// the committed one.
///
/// This is the visual regression gate. It only means anything because captures
/// are deterministic: the gallery renders with reduced motion, so no animation
/// phase leaks into a file.
///
/// Images are compared as pixels rather than as bytes. The renderer sometimes
/// lands a channel one step either way, and a gate that cried about a
/// difference nobody can see would be a gate nobody reads.
fn scenes_check(only: &[String]) -> Result<()> {
    let committed = snapshots();
    let scratch = root().join("target").join("scene-check");
    if scratch.exists() {
        fs::remove_dir_all(&scratch).with_context(|| format!("clear {}", scratch.display()))?;
    }
    let count = capture_into(&scratch, only)?;

    let mut differing = Vec::new();
    let mut missing = Vec::new();
    for entry in fs::read_dir(&scratch)? {
        let entry = entry?;
        let name = entry.file_name();
        let old = committed.join(&name);
        if !old.exists() {
            missing.push(name.to_string_lossy().into_owned());
            continue;
        }
        let apart = distance(&old, &entry.path())
            .with_context(|| format!("compare {}", name.to_string_lossy()))?;
        if apart > NOISE {
            differing.push((name.to_string_lossy().into_owned(), apart));
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
    for (name, apart) in &differing {
        println!("changed {name} (by {apart})");
    }
    bail!(
        "{} changed and {} new image(s) under {}; review them, then run \
         `cargo run -p xtask -- scenes capture` to accept",
        differing.len(),
        missing.len(),
        scratch.display()
    );
}

/// Runs the checks a change has to pass.
///
/// The short form is what a work-in-progress change wants: it answers in about
/// a minute. The full form adds the two slow proofs, rendered documentation
/// and the visual regression, and is what a commit wants.
fn gate(full: bool) -> Result<()> {
    step("cargo", &["fmt", "--all", "--", "--check"], None)?;
    step("cargo", &["check", "--workspace", "--all-targets"], None)?;
    step("cargo", &["test", "--workspace"], None)?;
    step(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        None,
    )?;
    tokens(true)?;
    if full {
        step(
            "cargo",
            &["doc", "--no-deps", "--workspace"],
            Some(("RUSTDOCFLAGS", "-D warnings")),
        )?;
        scenes_check(&[])?;
    }
    println!("gate passed");
    Ok(())
}

fn step(program: &str, args: &[&str], env: Option<(&str, &str)>) -> Result<()> {
    println!("== {program} {}", args.join(" "));
    let mut command = Command::new(program);
    command.args(args).current_dir(root());
    if let Some((name, value)) = env {
        command.env(name, value);
    }
    let status = command
        .status()
        .with_context(|| format!("run {program} {}", args.join(" ")))?;
    if !status.success() {
        bail!("{program} {} failed", args.join(" "));
    }
    Ok(())
}

fn snapshots() -> PathBuf {
    root().join("snapshots").join(platform()).join("scenes")
}

/// How far one channel may land from the committed image and still count as
/// the same picture.
///
/// One step is the smallest difference the format can hold, and the renderer
/// does land there occasionally on an antialiased edge. Anything a component
/// actually changed moves further than this, so the tolerance costs no
/// coverage while keeping the gate worth reading.
const NOISE: u8 = 1;

/// The largest per-channel difference between two images.
///
/// Images of different sizes are as far apart as it is possible to be, since
/// there is no pixel to compare a missing pixel against.
fn distance(left: &Path, right: &Path) -> Result<u8> {
    let left = decode(left)?;
    let right = decode(right)?;
    if left.0 != right.0 {
        return Ok(u8::MAX);
    }
    Ok(left
        .1
        .iter()
        .zip(right.1.iter())
        .map(|(left, right)| left.abs_diff(*right))
        .max()
        .unwrap_or(0))
}

/// Decodes a PNG into its dimensions and its pixels.
fn decode(path: &Path) -> Result<((u32, u32), Vec<u8>)> {
    let file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder
        .read_info()
        .with_context(|| format!("read {}", path.display()))?;
    let size = reader
        .output_buffer_size()
        .with_context(|| format!("{} is larger than this machine can hold", path.display()))?;
    let mut pixels = vec![0; size];
    let info = reader
        .next_frame(&mut pixels)
        .with_context(|| format!("decode {}", path.display()))?;
    pixels.truncate(info.buffer_size());
    Ok(((info.width, info.height), pixels))
}

/// Drives one gallery process over the whole catalog.
///
/// A GPUI application owns the window system for its lifetime, so the gallery
/// swaps the scene on a live window rather than opening a process per image.
fn capture_into(directory: &Path, only: &[String]) -> Result<usize> {
    let _held = Capturing::claim()?;
    fs::create_dir_all(directory).with_context(|| format!("create {}", directory.display()))?;
    let mut command = Command::new(env!("CARGO"));
    command
        .args(["run", "--quiet", "-p", "gpui-kit-gallery", "--"])
        .arg("--capture-all")
        .arg(directory)
        .current_dir(root());
    if !only.is_empty() {
        for name in only {
            if gpui_kit::scenes::find(name).is_none() {
                bail!("unknown scene `{name}`");
            }
        }
        command.arg("--only").arg(only.join(","));
    }
    let status = command.status().context("run the gallery")?;
    if !status.success() {
        bail!("capturing scenes failed");
    }
    // Naming every image the run owed rather than counting what is there:
    // a run that stopped early would otherwise report as a complete one, and
    // a comparison against images it never wrote would pass. Counting alone
    // cannot tell the difference when the destination already holds the rest
    // of the catalog.
    let wanted = expected_images(only);
    let absent: Vec<&String> = wanted
        .iter()
        .filter(|name| !directory.join(name).exists())
        .collect();
    if !absent.is_empty() {
        bail!(
            "the capture owed {} images and {} never arrived under {}: {}",
            wanted.len(),
            absent.len(),
            directory.display(),
            absent
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(wanted.len())
}

/// The right to be the only capture running on this machine.
///
/// Two galleries cannot capture at once. They take the foreground from each
/// other, and a window nobody is compositing hands back the frame it drew
/// last, so both runs read each other's scenes. That failure looks exactly
/// like a component having changed, which is the one thing this tool exists
/// to report truthfully.
struct Capturing(PathBuf);

impl Capturing {
    fn claim() -> Result<Self> {
        let path = root().join("target").join("capturing.lock");
        fs::create_dir_all(path.parent().expect("target has a parent"))?;
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => Ok(Self(path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => bail!(
                "another capture is running. Wait for it, or delete {} if nothing is.",
                path.display()
            ),
            Err(error) => Err(error).with_context(|| format!("claim {}", path.display())),
        }
    }
}

impl Drop for Capturing {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Every file name a capture of `only` owes, or of the catalog when empty.
fn expected_images(only: &[String]) -> Vec<String> {
    let scenes: Vec<String> = if only.is_empty() {
        gpui_kit::scenes::catalog()
            .iter()
            .map(|scene| scene.name.to_owned())
            .collect()
    } else {
        only.to_vec()
    };
    let themes: Vec<String> = bundled()
        .iter()
        .map(|theme| theme.meta.id.clone())
        .collect();
    scenes
        .iter()
        .flat_map(|scene| {
            themes
                .iter()
                .map(move |theme| format!("{scene}-{theme}.png"))
        })
        .collect()
}

fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

fn tokens(check: bool) -> Result<()> {
    contrast_gate()?;

    let mut output = String::from(
        "<!-- @generated by `cargo xtask tokens generate`; do not edit. -->\n\
         # Token reference\n\n\
         The JSON documents under `tokens/` are the authority. These tables are\n\
         a review aid, and every theme below is validated on each run.\n",
    );
    for document in bundled() {
        theme_section(&mut output, document)?;
    }

    let path = root().join("docs/token-reference.md");
    if check {
        let current = fs::read_to_string(&path)
            .with_context(|| format!("read generated {}", path.display()))?;
        if current != output {
            bail!(
                "{} is stale; run `cargo xtask tokens generate`",
                path.display()
            );
        }
        println!("{} is current", path.display());
    } else {
        fs::write(&path, output).with_context(|| format!("write {}", path.display()))?;
        println!("generated {}", path.display());
    }
    Ok(())
}

/// Fails the task rather than the document, so a contrast regression is caught
/// before it can be committed as a generated table.
fn contrast_gate() -> Result<()> {
    let mut failed = false;
    for document in bundled() {
        for failure in contrast::failures(document) {
            failed = true;
            eprintln!(
                "{}: {} on {} is {:.2}:1, below the {:.1}:1 minimum",
                document.meta.id,
                failure.foreground,
                failure.background,
                failure.ratio,
                failure.minimum
            );
        }
    }
    if failed {
        bail!("contrast requirements are not met");
    }
    Ok(())
}

/// Emits one theme, in a fixed order.
///
/// The table is built from typed accessors rather than by iterating the parsed
/// JSON, because whether `serde_json` preserves document order depends on
/// feature unification across the workspace, and a generated file must not
/// change with an unrelated dependency.
fn theme_section(output: &mut String, tokens: &TokenDocument) -> Result<()> {
    write!(
        output,
        "\n## {} (`{}`)\n\nAppearance: `{:?}`.\n",
        tokens.meta.name, tokens.meta.id, tokens.meta.appearance
    )?;

    output.push_str("\n### Palette\n\n| Token | Value |\n|---|---|\n");
    for (group, steps) in &tokens.color.palette {
        for (step, value) in steps {
            writeln!(output, "| `color.palette.{group}.{step}` | `{value}` |")?;
        }
    }

    output.push_str("\n### Semantic color\n\n| Token | Source | Resolved |\n|---|---|---|\n");
    let color = &tokens.color;
    let sources: Vec<(String, &str)> = vec![
        ("color.surface.canvas".into(), color.surface.canvas.as_str()),
        ("color.surface.panel".into(), color.surface.panel.as_str()),
        ("color.surface.raised".into(), color.surface.raised.as_str()),
        (
            "color.surface.overlay".into(),
            color.surface.overlay.as_str(),
        ),
        ("color.text.primary".into(), color.text.primary.as_str()),
        ("color.text.muted".into(), color.text.muted.as_str()),
        ("color.text.faint".into(), color.text.faint.as_str()),
        ("color.text.onAccent".into(), color.text.on_accent.as_str()),
        (
            "color.interactive.hover".into(),
            color.interactive.hover.as_str(),
        ),
        (
            "color.interactive.active".into(),
            color.interactive.active.as_str(),
        ),
        (
            "color.interactive.selected".into(),
            color.interactive.selected.as_str(),
        ),
        (
            "color.interactive.hairline".into(),
            color.interactive.hairline.as_str(),
        ),
        (
            "color.interactive.hairlineStrong".into(),
            color.interactive.hairline_strong.as_str(),
        ),
        (
            "color.interactive.focus".into(),
            color.interactive.focus.as_str(),
        ),
        (
            "color.semantic.accent".into(),
            color.semantic.accent.as_str(),
        ),
        (
            "color.semantic.accentStrong".into(),
            color.semantic.accent_strong.as_str(),
        ),
        (
            "color.semantic.danger".into(),
            color.semantic.danger.as_str(),
        ),
        (
            "color.semantic.warning".into(),
            color.semantic.warning.as_str(),
        ),
        (
            "color.semantic.success".into(),
            color.semantic.success.as_str(),
        ),
        ("color.semantic.info".into(), color.semantic.info.as_str()),
    ]
    .into_iter()
    .chain(
        color
            .loader
            .gradient
            .iter()
            .enumerate()
            .map(|(index, value)| (format!("color.loader.gradient.{index}"), value.as_str())),
    )
    .collect();

    for (path, source) in sources {
        writeln!(
            output,
            "| `{path}` | `{source}` | `{}` |",
            hex(tokens, source)
        )?;
    }

    output.push_str("\n### Spacing\n\n| Step | Pixels |\n|---|---:|\n");
    for (name, step) in [
        ("xs", Space::Xs),
        ("sm", Space::Sm),
        ("md", Space::Md),
        ("lg", Space::Lg),
        ("xl", Space::Xl),
        ("xxl", Space::Xxl),
    ] {
        writeln!(output, "| `{name}` | {} |", tokens.spacing(step))?;
    }

    output.push_str("\n### Radius\n\n| Step | Pixels |\n|---|---:|\n");
    for (name, step) in [
        ("small", Radius::Small),
        ("control", Radius::Control),
        ("card", Radius::Card),
        ("dialog", Radius::Dialog),
        ("bubble", Radius::Bubble),
        ("pill", Radius::Pill),
    ] {
        writeln!(output, "| `{name}` | {} |", tokens.radius(step))?;
    }

    output.push_str(
        "\n### Control scale\n\n| Step | Height | Padding X | Gap | Font | Icon |\n|---|---:|---:|---:|---:|---:|\n",
    );
    for (name, size) in [
        ("xs", ControlSize::Xs),
        ("sm", ControlSize::Sm),
        ("md", ControlSize::Md),
        ("lg", ControlSize::Lg),
    ] {
        let step = tokens.control(size);
        writeln!(
            output,
            "| `{name}` | {} | {} | {} | {} | {} |",
            step.height, step.padding_x, step.gap, step.font_size, step.icon_size
        )?;
    }

    output.push_str("\n### Density\n\n| Axis | Space | Control | Font |\n|---|---:|---:|---:|\n");
    for (name, density) in [
        ("compact", Density::Compact),
        ("comfortable", Density::Comfortable),
    ] {
        let scale = tokens.density(density);
        writeln!(
            output,
            "| `{name}` | {} | {} | {} |",
            scale.space, scale.control, scale.font
        )?;
    }

    output.push_str(
        "\n### Elevation\n\n| Step | Y | Blur | Spread | Color |\n|---|---:|---:|---:|---|\n",
    );
    for (name, level) in [
        ("flat", Elevation::Flat),
        ("raised", Elevation::Raised),
        ("overlay", Elevation::Overlay),
        ("modal", Elevation::Modal),
    ] {
        let step = tokens.elevation(level);
        writeln!(
            output,
            "| `{name}` | {} | {} | {} | `{}` |",
            step.y,
            step.blur,
            step.spread,
            format_color(step.color)
        )?;
    }

    output.push_str("\n### Layers\n\n| Layer | Z index |\n|---|---:|\n");
    for layer in Layer::ALL {
        writeln!(output, "| `{layer:?}` | {} |", tokens.z_index(layer))?;
    }

    output.push_str("\n### Motion\n\n| Easing | Curve |\n|---|---|\n");
    for easing in MotionEasing::ALL {
        writeln!(
            output,
            "| `{}` | `{:?}` |",
            easing.name(),
            tokens.easing(easing)
        )?;
    }
    output.push_str("\n| Spring | Stiffness | Damping | Mass |\n|---|---:|---:|---:|\n");
    for preset in SpringPreset::ALL {
        let spring = tokens.spring(preset);
        writeln!(
            output,
            "| `{}` | {} | {} | {} |",
            preset.name(),
            spring.stiffness,
            spring.damping,
            spring.mass
        )?;
    }
    output.push_str("\n| Response | Pixels |\n|---|---:|\n");
    for (name, value) in [
        ("motion.pressOffsetPx", tokens.press_offset()),
        ("motion.hoverLiftPx", tokens.hover_lift()),
    ] {
        writeln!(output, "| `{name}` | {value} |")?;
    }

    output.push_str("\n| Gesture | Value |\n|---|---:|\n");
    for (name, value) in [
        ("motion.flickVelocityPxPerSec", tokens.flick_velocity()),
        ("motion.rubberBandTension", tokens.rubber_band_tension()),
    ] {
        writeln!(output, "| `{name}` | {value} |")?;
    }

    output.push_str("\n### Border and opacity\n\n| Token | Value |\n|---|---:|\n");
    for (name, value) in [
        (
            "border.hairline",
            tokens.border_width(BorderWeight::Hairline),
        ),
        ("border.thick", tokens.border_width(BorderWeight::Thick)),
        ("opacity.disabled", tokens.opacity(OpacityRole::Disabled)),
        ("opacity.muted", tokens.opacity(OpacityRole::Muted)),
        ("opacity.scrim", tokens.opacity(OpacityRole::Scrim)),
    ] {
        writeln!(output, "| `{name}` | {value} |")?;
    }

    output.push_str("\n### Effects\n\n| Token | Value |\n|---|---:|\n");
    for (name, value) in [
        ("effect.glassAlphaMacos", tokens.effect.glass_alpha_macos),
        ("effect.backdropBlur", tokens.effect.backdrop_blur),
        ("effect.edgeFadeBand", tokens.effect.edge_fade_band),
        (
            "effect.selectedRingAlpha",
            tokens.effect.selected_ring_alpha,
        ),
        ("effect.focusRingWidth", tokens.effect.focus_ring_width),
        ("effect.focusRingAlpha", tokens.effect.focus_ring_alpha),
    ] {
        writeln!(output, "| `{name}` | {value} |")?;
    }

    output.push_str(
        "\n### Contrast\n\n| Foreground | Background | Ratio | Minimum |\n|---|---|---:|---:|\n",
    );
    for check in contrast::report(tokens) {
        writeln!(
            output,
            "| `{}` | `{}` | {:.2} | {:.1} |",
            check.foreground, check.background, check.ratio, check.minimum
        )?;
    }
    Ok(())
}

fn hex(tokens: &TokenDocument, source: &str) -> String {
    let color = gpui_kit_tokens::Color::resolve("reference", source, &tokens.color.palette)
        .expect("the document is validated before it is documented");
    format_color(color)
}

fn format_color(color: gpui_kit_tokens::Color) -> String {
    let channel = |value: f32| (value * 255.0).round() as u8;
    let rgb = format!(
        "#{:02x}{:02x}{:02x}",
        channel(color.red),
        channel(color.green),
        channel(color.blue)
    );
    if color.alpha >= 1.0 {
        rgb
    } else {
        format!("{rgb}{:02x}", channel(color.alpha))
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives under the repository root")
        .to_path_buf()
}
