//! The complete machine-readable developer catalog.
//!
//! `api-index.json` deliberately specializes in Kit components. This index
//! keeps that exact component contract and adds the rest of GPUI Box: package
//! identities and features, public Rust declarations, tokens and themes,
//! guides and guide sections, assets, compatibility, and scenes. It is source
//! data for MCP resources, not a second handwritten API description.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use quote::ToTokens;
use serde_json::{Value, json};
use syn::{Attribute, Expr, Item, Lit, Meta, Type, Visibility};

use crate::dependencies;

pub(crate) fn build(root: &Path, api_index: &str) -> Result<String> {
    let authority = dependencies::authority(root)?;
    let workspace_packages = authority
        .package
        .iter()
        .filter_map(|package| {
            package
                .lib
                .as_ref()
                .map(|lib| (lib.clone(), package.name.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut packages = Vec::new();
    let mut symbols = Vec::new();

    for package in authority.package {
        let manifest_path = root.join(&package.manifest);
        let manifest: toml::Value = toml::from_str(&fs::read_to_string(&manifest_path)?)?;
        let package_table = manifest
            .get("package")
            .and_then(toml::Value::as_table)
            .context("Cargo manifest has no [package] table")?;
        let crate_name = package.lib.clone().unwrap_or_else(|| {
            package
                .name
                .trim_start_matches("gpui-box-")
                .replace('-', "_")
        });
        let crate_root = manifest_path
            .parent()
            .context("package manifest has no directory")?;
        let source_root = crate_root.join("src");
        let mut package_symbols = Vec::new();
        if source_root.is_dir() {
            let mut source_files = Vec::new();
            collect_rs(&source_root, &mut source_files)?;
            source_files.sort();
            for source in source_files {
                read_symbols(
                    root,
                    &source_root,
                    &source,
                    &package.name,
                    &crate_name,
                    &mut package_symbols,
                )?;
            }
        }
        let mut package_symbols = merge_symbol_declarations(package_symbols);
        package_symbols.sort_by(|a, b| string(a, "id").cmp(string(b, "id")));
        let symbol_count = package_symbols.len();
        symbols.extend(package_symbols);

        let features = manifest
            .get("features")
            .cloned()
            .map(toml_to_json)
            .transpose()?
            .unwrap_or_else(|| json!({}));
        let dependencies = dependencies_from_manifest(&manifest, &workspace_packages);
        packages.push(json!({
            "name": package.name,
            "crate": crate_name,
            "manifest": package.manifest,
            "cohort": package.cohort,
            "layer": package.layer,
            "version": package.version,
            "license": package.license,
            "publish": package.publish,
            "description": package_table.get("description").and_then(toml::Value::as_str),
            "readme": package_table.get("readme").and_then(toml::Value::as_str),
            "features": features,
            "dependencies": dependencies,
            "symbolCount": symbol_count,
        }));
    }
    packages.sort_by(|a, b| string(a, "name").cmp(string(b, "name")));
    symbols.sort_by(|a, b| string(a, "id").cmp(string(b, "id")));

    let guides = guides(root)?;
    let recipes = recipes(root, &guides)?;
    let themes = themes(root)?;
    let assets = assets(root)?;
    let compatibility: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("compatibility.toml"))?)?;
    let api: Value = serde_json::from_str(api_index)?;

    let index = json!({
        "schema": 1,
        "project": {
            "name": "GPUI Box",
            "repository": "https://github.com/fran0220/gpui-box",
            "releaseVersion": compatibility.get("release_version").and_then(toml::Value::as_str),
            "releaseStatus": compatibility.get("release_status").and_then(toml::Value::as_str),
            "rustVersion": compatibility.get("rust_version").and_then(toml::Value::as_str),
            "edition": compatibility.get("edition").and_then(toml::Value::as_str),
        },
        "packages": packages,
        "symbols": symbols,
        "components": api.get("components").cloned().unwrap_or_else(|| json!([])),
        "types": api.get("types").cloned().unwrap_or_else(|| json!([])),
        "themes": themes,
        "guides": guides,
        "recipes": recipes,
        "scenes": api.get("scenes").cloned().unwrap_or_else(|| json!([])),
        "assets": assets,
        "compatibility": toml_to_json(compatibility)?,
    });
    Ok(format!("{}\n", serde_json::to_string_pretty(&index)?))
}

fn collect_rs(directory: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rs(&path, out)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn read_symbols(
    root: &Path,
    source_root: &Path,
    source: &Path,
    package: &str,
    crate_name: &str,
    out: &mut Vec<Value>,
) -> Result<()> {
    let body = fs::read_to_string(source)?;
    let parsed = match syn::parse_file(&body) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!(
                "developer index: skipped unparseable source {}: {error}",
                source.display()
            );
            return Ok(());
        }
    };
    let module = module_path(source_root, source);
    let relative = source
        .strip_prefix(root)
        .unwrap_or(source)
        .to_string_lossy()
        .replace('\\', "/");
    let mut reexport_index = 0usize;

    for item in parsed.items {
        match item {
            Item::Struct(item) if public(&item.vis) => push_symbol(
                out,
                package,
                crate_name,
                &module,
                &relative,
                &item.ident.to_string(),
                "struct",
                docs(&item.attrs),
                cfgs(&item.attrs),
                item.to_token_stream().to_string(),
            ),
            Item::Enum(item) if public(&item.vis) => push_symbol(
                out,
                package,
                crate_name,
                &module,
                &relative,
                &item.ident.to_string(),
                "enum",
                docs(&item.attrs),
                cfgs(&item.attrs),
                item.to_token_stream().to_string(),
            ),
            Item::Trait(item) if public(&item.vis) => push_symbol(
                out,
                package,
                crate_name,
                &module,
                &relative,
                &item.ident.to_string(),
                "trait",
                docs(&item.attrs),
                cfgs(&item.attrs),
                item.to_token_stream().to_string(),
            ),
            Item::Fn(item) if public(&item.vis) => push_symbol(
                out,
                package,
                crate_name,
                &module,
                &relative,
                &item.sig.ident.to_string(),
                "function",
                docs(&item.attrs),
                cfgs(&item.attrs),
                format!("pub {}", item.sig.to_token_stream()),
            ),
            Item::Type(item) if public(&item.vis) => push_symbol(
                out,
                package,
                crate_name,
                &module,
                &relative,
                &item.ident.to_string(),
                "type",
                docs(&item.attrs),
                cfgs(&item.attrs),
                item.to_token_stream().to_string(),
            ),
            Item::Const(item) if public(&item.vis) => push_symbol(
                out,
                package,
                crate_name,
                &module,
                &relative,
                &item.ident.to_string(),
                "constant",
                docs(&item.attrs),
                cfgs(&item.attrs),
                format!("pub const {}: {}", item.ident, item.ty.to_token_stream()),
            ),
            Item::Static(item) if public(&item.vis) => push_symbol(
                out,
                package,
                crate_name,
                &module,
                &relative,
                &item.ident.to_string(),
                "static",
                docs(&item.attrs),
                cfgs(&item.attrs),
                format!("pub static {}: {}", item.ident, item.ty.to_token_stream()),
            ),
            Item::Union(item) if public(&item.vis) => push_symbol(
                out,
                package,
                crate_name,
                &module,
                &relative,
                &item.ident.to_string(),
                "union",
                docs(&item.attrs),
                cfgs(&item.attrs),
                item.to_token_stream().to_string(),
            ),
            Item::Use(item) if public(&item.vis) => {
                reexport_index += 1;
                push_symbol_with_id(
                    out,
                    package,
                    crate_name,
                    &module,
                    &relative,
                    &format!("reexport-{reexport_index}"),
                    &item.tree.to_token_stream().to_string(),
                    "reexport",
                    docs(&item.attrs),
                    cfgs(&item.attrs),
                    item.to_token_stream().to_string(),
                );
            }
            Item::Macro(item)
                if item
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("macro_export")) =>
            {
                if let Some(ident) = item.ident {
                    push_symbol(
                        out,
                        package,
                        crate_name,
                        &module,
                        &relative,
                        &ident.to_string(),
                        "macro",
                        docs(&item.attrs),
                        cfgs(&item.attrs),
                        format!("macro_rules! {ident}"),
                    );
                }
            }
            Item::Impl(item) if item.trait_.is_none() => {
                let Some(owner) = type_name(&item.self_ty) else {
                    continue;
                };
                let target = normalize_tokens(&item.self_ty.to_token_stream().to_string());
                for member in item.items {
                    let syn::ImplItem::Fn(method) = member else {
                        continue;
                    };
                    if !public(&method.vis) {
                        continue;
                    }
                    let name = method.sig.ident.to_string();
                    push_symbol_with_target(
                        out,
                        package,
                        crate_name,
                        &module,
                        &relative,
                        &format!("{owner}::{name}"),
                        &name,
                        "method",
                        docs(&method.attrs),
                        cfgs(&method.attrs),
                        format!("pub {}", method.sig.to_token_stream()),
                        Some(&target),
                    );
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_symbol(
    out: &mut Vec<Value>,
    package: &str,
    crate_name: &str,
    module: &str,
    source: &str,
    name: &str,
    kind: &str,
    docs: String,
    cfg: Vec<String>,
    signature: String,
) {
    push_symbol_with_id(
        out, package, crate_name, module, source, name, name, kind, docs, cfg, signature,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_symbol_with_id(
    out: &mut Vec<Value>,
    package: &str,
    crate_name: &str,
    module: &str,
    source: &str,
    id_suffix: &str,
    name: &str,
    kind: &str,
    docs: String,
    cfg: Vec<String>,
    signature: String,
) {
    push_symbol_with_target(
        out, package, crate_name, module, source, id_suffix, name, kind, docs, cfg, signature, None,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_symbol_with_target(
    out: &mut Vec<Value>,
    package: &str,
    crate_name: &str,
    module: &str,
    source: &str,
    id_suffix: &str,
    name: &str,
    kind: &str,
    docs: String,
    cfg: Vec<String>,
    signature: String,
    target: Option<&str>,
) {
    let path = if module.is_empty() {
        crate_name.to_string()
    } else {
        format!("{crate_name}::{module}")
    };
    let mut symbol = json!({
        "id": format!("{path}::{id_suffix}"),
        "name": name,
        "kind": kind,
        "package": package,
        "crate": crate_name,
        "module": module,
        "path": path,
        "source": source,
        "summary": first_paragraph(&docs),
        "documentation": docs,
        "signature": normalize_tokens(&signature),
        "cfg": cfg,
    });
    if let Some(target) = target {
        symbol["target"] = Value::String(target.to_string());
    }
    out.push(symbol);
}

/// A Rust path can have more than one declaration when implementations are
/// selected by `cfg` or supplied for distinct concrete generic targets. The
/// path remains the stable lookup key; declarations preserve every exact
/// source signature instead of making lookup order observable.
fn merge_symbol_declarations(symbols: Vec<Value>) -> Vec<Value> {
    let mut groups = BTreeMap::<String, Vec<Value>>::new();
    for symbol in symbols {
        groups
            .entry(string(&symbol, "id").to_string())
            .or_default()
            .push(symbol);
    }
    groups
        .into_values()
        .map(|mut declarations| {
            if declarations.len() == 1 {
                return declarations.pop().expect("one declaration");
            }
            declarations.sort_by_key(|declaration| {
                (
                    string(declaration, "source").to_string(),
                    string(declaration, "target").to_string(),
                    string(declaration, "signature").to_string(),
                    serde_json::to_string(&declaration["cfg"]).unwrap_or_default(),
                )
            });
            let first = &declarations[0];
            let summary = declarations
                .iter()
                .map(|declaration| string(declaration, "summary"))
                .find(|summary| !summary.is_empty())
                .unwrap_or_default();
            let variants = declarations
                .iter()
                .map(|declaration| {
                    let mut variant = json!({
                        "source": declaration["source"],
                        "documentation": declaration["documentation"],
                        "signature": declaration["signature"],
                        "cfg": declaration["cfg"],
                    });
                    if let Some(target) = declaration.get("target") {
                        variant["target"] = target.clone();
                    }
                    variant
                })
                .collect::<Vec<_>>();
            json!({
                "id": first["id"],
                "name": first["name"],
                "kind": first["kind"],
                "package": first["package"],
                "crate": first["crate"],
                "module": first["module"],
                "path": first["path"],
                "summary": summary,
                "declarations": variants,
            })
        })
        .collect()
}

fn public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn type_name(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn module_path(source_root: &Path, source: &Path) -> String {
    let relative = source.strip_prefix(source_root).unwrap_or(source);
    let mut parts = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let Some(last) = parts.pop() else {
        return String::new();
    };
    let stem = last.trim_end_matches(".rs");
    if !matches!(stem, "lib" | "main" | "mod") {
        parts.push(stem.to_string());
    }
    parts.join("::")
}

fn docs(attributes: &[Attribute]) -> String {
    attributes
        .iter()
        .filter_map(|attribute| {
            if !attribute.path().is_ident("doc") {
                return None;
            }
            let Meta::NameValue(value) = &attribute.meta else {
                return None;
            };
            let Expr::Lit(expression) = &value.value else {
                return None;
            };
            let Lit::Str(value) = &expression.lit else {
                return None;
            };
            Some(value.value().trim().to_string())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn cfgs(attributes: &[Attribute]) -> Vec<String> {
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg"))
        .map(|attribute| normalize_tokens(&attribute.meta.to_token_stream().to_string()))
        .collect()
}

fn normalize_tokens(value: &str) -> String {
    value
        .replace(" :: ", "::")
        .replace(" ,", ",")
        .replace(" ;", ";")
        .replace(" ( ", "(")
        .replace(" )", ")")
        .replace(" < ", "<")
        .replace(" >", ">")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn dependencies_from_manifest(
    manifest: &toml::Value,
    workspace_packages: &BTreeMap<String, String>,
) -> Vec<Value> {
    let mut result = Vec::new();
    for (table, kind) in [
        ("dependencies", "normal"),
        ("build-dependencies", "build"),
        ("dev-dependencies", "dev"),
    ] {
        let Some(dependencies) = manifest.get(table).and_then(toml::Value::as_table) else {
            continue;
        };
        for (alias, dependency) in dependencies {
            let package = dependency
                .get("package")
                .and_then(toml::Value::as_str)
                .or_else(|| workspace_packages.get(alias).map(String::as_str))
                .unwrap_or(alias);
            result.push(json!({
                "alias": alias,
                "package": package,
                "kind": kind,
                "optional": dependency.get("optional").and_then(toml::Value::as_bool).unwrap_or(false),
                "features": dependency.get("features").and_then(toml::Value::as_array).map(|values| values.iter().filter_map(toml::Value::as_str).collect::<Vec<_>>()).unwrap_or_default(),
            }));
        }
    }
    result.sort_by(|a, b| string(a, "alias").cmp(string(b, "alias")));
    result
}

fn guides(root: &Path) -> Result<Vec<Value>> {
    let mut paths = fs::read_dir(root.join("docs"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.retain(|path| path.extension().is_some_and(|extension| extension == "md"));
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let body = fs::read_to_string(&path)?;
            let slug = path
                .file_stem()
                .context("guide has no file stem")?
                .to_string_lossy();
            let title = body
                .lines()
                .find_map(|line| line.strip_prefix("# "))
                .unwrap_or(&slug);
            Ok(json!({
                "slug": slug,
                "title": title,
                "path": path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/"),
                "summary": markdown_summary(&body),
            }))
        })
        .collect()
}

fn recipes(root: &Path, guides: &[Value]) -> Result<Vec<Value>> {
    let mut result = Vec::new();
    let mut ids = BTreeSet::new();
    for guide in guides {
        let slug = string(guide, "slug");
        let body = fs::read_to_string(root.join("docs").join(format!("{slug}.md")))?;
        for title in body.lines().filter_map(|line| line.strip_prefix("## ")) {
            let base = format!("{slug}-{}", slugify(title));
            let mut id = base.clone();
            let mut suffix = 2;
            while !ids.insert(id.clone()) {
                id = format!("{base}-{suffix}");
                suffix += 1;
            }
            result.push(json!({
                "id": id,
                "title": title,
                "guide": slug,
                "anchor": slugify(title),
            }));
        }
    }
    Ok(result)
}

fn themes(root: &Path) -> Result<Vec<Value>> {
    let directory = root.join("crates/gpui-kit-tokens/tokens");
    let mut paths = fs::read_dir(&directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "json")
            && path.file_name().is_some_and(|name| name != "schema.json")
    });
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let name = path
                .file_stem()
                .context("theme has no file stem")?
                .to_string_lossy();
            let value: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
            Ok(json!({
                "name": name,
                "path": path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/"),
                "tokens": value,
            }))
        })
        .collect()
}

fn assets(root: &Path) -> Result<Value> {
    let asset_root = root.join("crates/gpui-kit-assets/assets");
    let mut icons = names_with_extension(&asset_root.join("icons"), "svg")?;
    let mut fonts = names_with_extension(&asset_root.join("fonts"), "ttf")?;
    icons.sort();
    fonts.sort();
    Ok(json!({ "icons": icons, "fonts": fonts }))
}

fn names_with_extension(directory: &Path, extension: &str) -> Result<Vec<String>> {
    Ok(fs::read_dir(directory)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|value| value == extension))
        .filter_map(|path| {
            path.file_stem()
                .map(|name| name.to_string_lossy().to_string())
        })
        .collect())
}

fn markdown_summary(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .skip_while(|line| line.is_empty() || line.starts_with('#'))
        .take_while(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn first_paragraph(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .skip_while(|line| line.is_empty())
        .take_while(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn slugify(value: &str) -> String {
    let mut result = String::new();
    let mut dash = false;
    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            result.push(character);
            dash = false;
        } else if !dash && !result.is_empty() {
            result.push('-');
            dash = true;
        }
    }
    result.trim_end_matches('-').to_string()
}

fn toml_to_json(value: toml::Value) -> Result<Value> {
    Ok(serde_json::to_value(value)?)
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_paths_follow_rust_files() {
        let root = Path::new("crate/src");
        assert_eq!(module_path(root, Path::new("crate/src/lib.rs")), "");
        assert_eq!(
            module_path(root, Path::new("crate/src/window.rs")),
            "window"
        );
        assert_eq!(
            module_path(root, Path::new("crate/src/input/mod.rs")),
            "input"
        );
        assert_eq!(
            module_path(root, Path::new("crate/src/input/key.rs")),
            "input::key"
        );
    }

    #[test]
    fn recipe_slugs_are_stable() {
        assert_eq!(slugify("Per-frame semantics"), "per-frame-semantics");
        assert_eq!(slugify("RTL / LTR"), "rtl-ltr");
    }

    #[test]
    fn duplicate_symbol_paths_keep_every_declaration() {
        let symbols = vec![
            json!({
                "id": "demo::run", "name": "run", "kind": "function",
                "package": "demo", "crate": "demo", "module": "", "path": "demo",
                "source": "src/lib.rs", "summary": "Run it.", "documentation": "Run it.",
                "signature": "pub fn run()", "cfg": ["cfg(unix)"]
            }),
            json!({
                "id": "demo::run", "name": "run", "kind": "function",
                "package": "demo", "crate": "demo", "module": "", "path": "demo",
                "source": "src/lib.rs", "summary": "Run it.", "documentation": "Run it.",
                "signature": "pub fn run()", "cfg": ["cfg(windows)"]
            }),
        ];
        let merged = merge_symbol_declarations(symbols);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0]["id"], "demo::run");
        assert_eq!(
            merged[0]["declarations"]
                .as_array()
                .expect("merged declarations")
                .len(),
            2
        );
    }
}
