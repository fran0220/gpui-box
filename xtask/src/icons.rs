use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const MANIFEST: &str = "crates/gpui-kit-assets/assets/PHOSPHOR.toml";
const GENERATED: &str = "crates/gpui-kit-assets/src/icons.rs";
const ASSETS: &str = "crates/gpui-kit-assets/assets/icons";
const CHECKSUMS: &str = "crates/gpui-kit-assets/assets/icons/SHA256SUMS";

#[derive(Debug, Deserialize)]
struct Catalog {
    source: Source,
    weights: Vec<String>,
    icon: Vec<IconSpec>,
    #[serde(default)]
    alias: Vec<AliasSpec>,
}

#[derive(Debug, Deserialize)]
struct Source {
    url: String,
    revision: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct IconSpec {
    variant: String,
    source: String,
    mirroring: String,
}

#[derive(Debug, Deserialize)]
struct AliasSpec {
    variant: String,
    target: String,
    weight: String,
}

pub fn import(root: &Path, source_root: &Path) -> Result<()> {
    let catalog = read_catalog(root)?;
    validate_catalog(&catalog)?;
    validate_source(&catalog, source_root)?;

    let mut selected = Vec::new();
    for icon in &catalog.icon {
        for weight in &catalog.weights {
            let source = source_path(source_root, &icon.source, weight)?;
            let bytes = fs::read(&source).with_context(|| format!("read {}", source.display()))?;
            validate_svg(&bytes, &source)?;
            selected.push((asset_path(&icon.source, weight)?, bytes));
        }
    }

    let directory = root.join(ASSETS);
    if directory.exists() {
        fs::remove_dir_all(&directory).with_context(|| format!("clear {}", directory.display()))?;
    }
    fs::create_dir_all(&directory).with_context(|| format!("create {}", directory.display()))?;
    for (relative, bytes) in &selected {
        let destination = directory.join(relative);
        fs::create_dir_all(destination.parent().expect("icon weight directory"))?;
        fs::write(&destination, bytes)
            .with_context(|| format!("write {}", destination.display()))?;
    }
    fs::write(root.join(GENERATED), generated_source(&catalog)?)?;
    fs::write(root.join(CHECKSUMS), checksums(&selected))?;
    check(root)?;
    println!(
        "imported {} Phosphor glyph names in {} weights from {}",
        catalog.icon.len(),
        catalog.weights.len(),
        catalog.source.revision
    );
    Ok(())
}

pub fn check(root: &Path) -> Result<()> {
    let catalog = read_catalog(root)?;
    validate_catalog(&catalog)?;

    let generated = root.join(GENERATED);
    let actual =
        fs::read_to_string(&generated).with_context(|| format!("read {}", generated.display()))?;
    let expected = generated_source(&catalog)?;
    if actual != expected {
        bail!(
            "{} is stale; run `cargo run -p xtask -- icons import <phosphor-core-checkout>`",
            generated.display()
        );
    }

    let mut expected_paths = BTreeSet::new();
    let mut selected = Vec::new();
    for icon in &catalog.icon {
        for weight in &catalog.weights {
            let relative = asset_path(&icon.source, weight)?;
            expected_paths.insert(relative.clone());
            let path = root.join(ASSETS).join(&relative);
            let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            validate_svg(&bytes, &path)?;
            selected.push((relative, bytes));
        }
    }
    expected_paths.insert(PathBuf::from("SHA256SUMS"));
    let actual_paths = files_below(&root.join(ASSETS))?;
    if actual_paths != expected_paths {
        bail!(
            "Phosphor asset set differs from {}; expected {:?}, found {:?}",
            MANIFEST,
            expected_paths,
            actual_paths
        );
    }

    let checksum_path = root.join(CHECKSUMS);
    let actual_checksums = fs::read_to_string(&checksum_path)
        .with_context(|| format!("read {}", checksum_path.display()))?;
    let expected_checksums = checksums(&selected);
    if actual_checksums != expected_checksums {
        bail!(
            "{} does not match the selected SVG bytes; re-import from the pinned source",
            checksum_path.display()
        );
    }
    println!(
        "Phosphor catalog matches {} at {}",
        catalog.source.version, catalog.source.revision
    );
    Ok(())
}

fn read_catalog(root: &Path) -> Result<Catalog> {
    let path = root.join(MANIFEST);
    let body = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&body).with_context(|| format!("parse {}", path.display()))
}

fn validate_catalog(catalog: &Catalog) -> Result<()> {
    if catalog.source.revision.len() != 40
        || !catalog
            .source
            .revision
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("Phosphor revision must be one complete Git object id");
    }
    if catalog.source.url != "https://github.com/phosphor-icons/core" {
        bail!("Phosphor source URL must name the authoritative core repository");
    }
    if catalog.weights != ["regular", "fill"] {
        bail!("the Kit icon grammar is exactly Regular plus Fill");
    }
    let mut variants = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for icon in &catalog.icon {
        if !is_pascal_case(&icon.variant) {
            bail!("icon variant `{}` is not PascalCase", icon.variant);
        }
        if !is_kebab_case(&icon.source) {
            bail!(
                "Phosphor name `{}` is not canonical kebab-case",
                icon.source
            );
        }
        if !matches!(icon.mirroring.as_str(), "fixed" | "directional") {
            bail!("icon `{}` has no mirroring decision", icon.variant);
        }
        if !variants.insert(icon.variant.as_str()) {
            bail!("duplicate icon variant `{}`", icon.variant);
        }
        if !sources.insert(icon.source.as_str()) {
            bail!("duplicate Phosphor source `{}`", icon.source);
        }
    }
    for alias in &catalog.alias {
        if !is_pascal_case(&alias.variant) || !variants.insert(alias.variant.as_str()) {
            bail!("invalid or duplicate icon alias `{}`", alias.variant);
        }
        if !catalog.icon.iter().any(|icon| icon.variant == alias.target) {
            bail!("icon alias `{}` has no target", alias.variant);
        }
        if !catalog.weights.contains(&alias.weight) {
            bail!("icon alias `{}` has an unavailable weight", alias.variant);
        }
    }
    Ok(())
}

fn validate_source(catalog: &Catalog, source_root: &Path) -> Result<()> {
    let package_path = source_root.join("package.json");
    let package: serde_json::Value = serde_json::from_slice(
        &fs::read(&package_path).with_context(|| format!("read {}", package_path.display()))?,
    )?;
    if package["name"] != "@phosphor-icons/core" || package["version"] != catalog.source.version {
        bail!(
            "{} is not @phosphor-icons/core {}",
            source_root.display(),
            catalog.source.version
        );
    }
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(source_root)
        .output()
        .context("read the Phosphor checkout revision")?;
    if !output.status.success() {
        bail!("{} is not a Git checkout", source_root.display());
    }
    let revision = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if revision != catalog.source.revision {
        bail!(
            "Phosphor checkout is {revision}, expected {}",
            catalog.source.revision
        );
    }
    Ok(())
}

fn source_path(root: &Path, name: &str, weight: &str) -> Result<PathBuf> {
    let suffix = weight_suffix(weight)?;
    Ok(root
        .join("assets")
        .join(weight)
        .join(format!("{name}{suffix}.svg")))
}

fn asset_path(name: &str, weight: &str) -> Result<PathBuf> {
    let suffix = weight_suffix(weight)?;
    Ok(PathBuf::from(weight).join(format!("{name}{suffix}.svg")))
}

fn weight_suffix(weight: &str) -> Result<&'static str> {
    match weight {
        "regular" => Ok(""),
        "fill" => Ok("-fill"),
        other => bail!("unsupported Phosphor weight `{other}`"),
    }
}

fn validate_svg(bytes: &[u8], path: &Path) -> Result<()> {
    let body =
        std::str::from_utf8(bytes).with_context(|| format!("{} is not UTF-8", path.display()))?;
    if !body.contains("viewBox=\"0 0 256 256\"")
        || !body.contains("currentColor")
        || body.contains("<script")
        || body.contains("href=")
    {
        bail!(
            "{} is not a self-contained Phosphor 256-unit currentColor SVG",
            path.display()
        );
    }
    Ok(())
}

fn generated_source(catalog: &Catalog) -> Result<String> {
    let mut source = String::new();
    writeln!(
        source,
        "//! Generated by `cargo run -p xtask -- icons import`; do not edit."
    )
    .expect("writing to a String cannot fail");
    writeln!(source, "use std::borrow::Cow;").expect("writing to a String cannot fail");
    writeln!(source).expect("writing to a String cannot fail");
    writeln!(
        source,
        "pub const PHOSPHOR_VERSION: &str = {:?};",
        catalog.source.version
    )
    .expect("writing to a String cannot fail");
    writeln!(
        source,
        "pub const PHOSPHOR_REVISION: &str = {:?};",
        catalog.source.revision
    )
    .expect("writing to a String cannot fail");
    writeln!(source).expect("writing to a String cannot fail");
    source.push_str(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub enum Mirroring { Directional, Fixed }\n\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub enum IconWeight { Regular, Fill }\n\n\
         impl IconWeight {\n\
             pub const ALL: &'static [Self] = &[Self::Regular, Self::Fill];\n\
         }\n\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub enum IconName {\n",
    );
    for icon in &catalog.icon {
        writeln!(source, "    {},", icon.variant).expect("writing to a String cannot fail");
    }
    source.push_str("}\n\nimpl IconName {\n");
    source.push_str("    pub const ALL: &'static [Self] = &[\n");
    for icon in &catalog.icon {
        writeln!(source, "        Self::{},", icon.variant)
            .expect("writing to a String cannot fail");
    }
    source.push_str(
        "    ];\n\n    pub const fn source_name(self) -> &'static str {\n        match self {\n",
    );
    for icon in &catalog.icon {
        writeln!(
            source,
            "            Self::{} => {:?},",
            icon.variant, icon.source
        )
        .expect("writing to a String cannot fail");
    }
    source.push_str("        }\n    }\n\n    pub const fn mirroring(self) -> Mirroring {\n        match self {\n");
    for icon in &catalog.icon {
        let mirroring = if icon.mirroring == "directional" {
            "Directional"
        } else {
            "Fixed"
        };
        writeln!(
            source,
            "            Self::{} => Mirroring::{mirroring},",
            icon.variant
        )
        .expect("writing to a String cannot fail");
    }
    source.push_str("        }\n    }\n}\n\n");
    source.push_str(
        "#[derive(Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct Icon { name: IconName, weight: IconWeight }\n\n\
         #[allow(non_upper_case_globals)]\n\
         impl Icon {\n\
             pub const ALL: &'static [Self] = &[\n",
    );
    for icon in &catalog.icon {
        writeln!(source, "        Self::{},", icon.variant)
            .expect("writing to a String cannot fail");
    }
    source.push_str("    ];\n\n");
    for icon in &catalog.icon {
        writeln!(
            source,
            "    pub const {}: Self = Self::new(IconName::{});",
            icon.variant, icon.variant
        )
        .expect("writing to a String cannot fail");
    }
    for alias in &catalog.alias {
        let weight = rust_weight(&alias.weight);
        writeln!(
            source,
            "    pub const {}: Self = Self::{}.with_weight(IconWeight::{});",
            alias.variant, alias.target, weight
        )
        .expect("writing to a String cannot fail");
    }
    source.push_str(
        "\n    pub const fn new(name: IconName) -> Self { Self { name, weight: IconWeight::Regular } }\n\
         pub const fn with_weight(self, weight: IconWeight) -> Self { Self { weight, ..self } }\n\
         pub const fn filled(self) -> Self { self.with_weight(IconWeight::Fill) }\n\
         pub const fn name(self) -> IconName { self.name }\n\
         pub const fn weight(self) -> IconWeight { self.weight }\n\
         pub const fn mirroring(self) -> Mirroring { self.name.mirroring() }\n\
         pub const fn mirrors_in_rtl(self) -> bool { matches!(self.mirroring(), Mirroring::Directional) }\n\n\
         pub const fn path(self) -> &'static str {\n\
             match (self.name, self.weight) {\n",
    );
    for icon in &catalog.icon {
        for weight in &catalog.weights {
            let rust_weight = rust_weight(weight);
            let relative = asset_path(&icon.source, weight).expect("validated weight");
            writeln!(
                source,
                "            (IconName::{}, IconWeight::{}) => {:?},",
                icon.variant,
                rust_weight,
                format!("icons/{}", relative.display())
            )
            .expect("writing to a String cannot fail");
        }
    }
    source.push_str("        }\n    }\n}\n\n");
    source.push_str(
        "impl std::fmt::Debug for Icon {\n\
             fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n\
                 match self.weight {\n\
                     IconWeight::Regular => self.name.fmt(formatter),\n\
                     weight => write!(formatter, \"{:?}.{:?}\", self.name, weight),\n\
                 }\n\
             }\n\
         }\n\n\
         pub(crate) const ALL_PATHS: &[&str] = &[\n",
    );
    for icon in &catalog.icon {
        for weight in &catalog.weights {
            let relative = asset_path(&icon.source, weight).expect("validated weight");
            writeln!(source, "    {:?},", format!("icons/{}", relative.display()))
                .expect("writing to a String cannot fail");
        }
    }
    source.push_str(
        "];\n\n\
         pub(crate) fn load(path: &str) -> Option<Cow<'static, [u8]>> {\n\
             match path {\n",
    );
    for icon in &catalog.icon {
        for weight in &catalog.weights {
            let relative = asset_path(&icon.source, weight).expect("validated weight");
            let asset = format!("icons/{}", relative.display());
            let include = format!("../assets/{asset}");
            writeln!(
                source,
                "        {asset:?} => Some(Cow::Borrowed(include_bytes!({include:?}).as_slice())),"
            )
            .expect("writing to a String cannot fail");
        }
    }
    source.push_str("        _ => None,\n    }\n}\n");
    format_rust(source)
}

fn format_rust(source: String) -> Result<String> {
    let mut child = Command::new("rustfmt")
        .args(["--emit", "stdout", "--edition", "2024"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("start rustfmt for generated icon source")?;
    child
        .stdin
        .take()
        .context("open rustfmt stdin")?
        .write_all(source.as_bytes())
        .context("write generated icon source to rustfmt")?;
    let output = child
        .wait_with_output()
        .context("format generated icon source")?;
    if !output.status.success() {
        bail!(
            "rustfmt rejected generated icon source:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout).context("rustfmt emitted non-UTF-8 icon source")
}

fn rust_weight(weight: &str) -> &'static str {
    match weight {
        "regular" => "Regular",
        "fill" => "Fill",
        _ => unreachable!("catalog validation rejects other weights"),
    }
}

fn checksums(selected: &[(PathBuf, Vec<u8>)]) -> String {
    let mut output = String::new();
    for (path, bytes) in selected {
        let digest = Sha256::digest(bytes);
        for byte in digest {
            write!(output, "{byte:02x}").expect("writing to a String cannot fail");
        }
        writeln!(output, "  {}", path.display()).expect("writing to a String cannot fail");
    }
    output
}

fn files_below(root: &Path) -> Result<BTreeSet<PathBuf>> {
    fn visit(root: &Path, at: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(at).with_context(|| format!("list {}", at.display()))? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files)?;
            } else {
                files.insert(path.strip_prefix(root)?.to_path_buf());
            }
        }
        Ok(())
    }
    let mut files = BTreeSet::new();
    visit(root, root, &mut files)?;
    Ok(files)
}

fn is_pascal_case(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_uppercase())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn is_kebab_case(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
