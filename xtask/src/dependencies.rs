use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use serde_json::Value as Json;
use toml::Value;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Authority {
    pub schema: u32,
    pub package: Vec<Package>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Package {
    pub manifest: String,
    pub name: String,
    pub lib: Option<String>,
    pub cohort: String,
    pub version: String,
    pub license: String,
    pub publish: bool,
    pub layer: u8,
}

pub(crate) fn authority(root: &Path) -> Result<Authority> {
    let path = root.join("package-authority.toml");
    let value: Authority = toml::from_str(
        &fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?,
    )?;
    ensure!(
        value.schema == 1,
        "unsupported package authority schema {}",
        value.schema
    );
    validate_authority(&value)?;
    Ok(value)
}

fn validate_authority(a: &Authority) -> Result<()> {
    let mut names = HashSet::new();
    let mut manifests = HashSet::new();
    for p in &a.package {
        ensure!(
            names.insert(&p.name),
            "duplicate authority package {}",
            p.name
        );
        ensure!(
            manifests.insert(&p.manifest),
            "duplicate authority manifest {}",
            p.manifest
        );
        let expected = match p.cohort.as_str() {
            "framework" => 0,
            "kit" => 1,
            "tool" => 2,
            x => bail!("invalid cohort {x} for {}", p.name),
        };
        ensure!(
            p.layer == expected,
            "{} has layer {}, expected {} for {}",
            p.name,
            p.layer,
            expected,
            p.cohort
        );
    }
    Ok(())
}

pub fn check(root: &Path, args: &[String]) -> Result<()> {
    ensure!(
        args.is_empty(),
        "usage: cargo run -p xtask -- dependencies check"
    );
    let a = authority(root)?;
    let root_meta = metadata(&root.join("Cargo.toml"))?;
    let head_meta = metadata(&root.join("tools/headless-visual/Cargo.toml"))?;
    let root_packages = root_meta["packages"]
        .as_array()
        .context("root Cargo metadata has no packages array")?;
    let head_packages = head_meta["packages"]
        .as_array()
        .context("headless Cargo metadata has no packages array")?;
    let mut actual = HashMap::new();
    for p in root_packages.iter().chain(head_packages) {
        if p["source"].as_str().is_some_and(|s| s.starts_with("git+")) {
            bail!("Git package in resolved graph: {}", p["name"]);
        }
        let path = canonical(
            p["manifest_path"]
                .as_str()
                .context("Cargo metadata package has no manifest_path")?,
        );
        actual.insert(path, p);
        ensure!(
            !matches!(
                p["name"].as_str(),
                Some("zlog" | "ztracing" | "ztracing_macro")
            ),
            "forbidden legacy package {}",
            p["name"]
        );
    }
    for p in &a.package {
        let path = canonical(root.join(&p.manifest));
        let m = actual
            .get(&path)
            .with_context(|| format!("authority manifest absent from metadata: {}", p.manifest))?;
        ensure!(
            m["name"] == p.name && m["version"] == p.version && m["license"] == p.license,
            "authority identity differs for {}",
            p.manifest
        );
        ensure!(
            (m["publish"].as_array().is_none()) == p.publish,
            "authority publish differs for {}",
            p.name
        );
        if let Some(lib) = &p.lib {
            let targets = m["targets"]
                .as_array()
                .with_context(|| format!("Cargo metadata has no targets for {}", p.name))?;
            ensure!(
                targets.iter().any(|t| t["name"] == *lib
                    && t["kind"].as_array().is_some_and(|kinds| kinds
                        .iter()
                        .any(|k| k == "lib" || k == "proc-macro"))),
                "{} lacks lib target {}",
                p.name,
                lib
            );
        }
        check_internal_declarations(root, p, &a)?;
    }
    check_single_gpui_library(root_packages, "root graph")?;
    check_single_gpui_library(head_packages, "headless graph")?;
    ensure!(
        read_toml(&root.join("tools/headless-visual/Cargo.toml"))?
            .get("patch")
            .is_none(),
        "headless workspace must not contain [patch]"
    );
    for lock in [
        root.join("Cargo.lock"),
        root.join("tools/headless-visual/Cargo.lock"),
    ] {
        ensure!(
            !fs::read_to_string(&lock)?.contains("source = \"git+"),
            "{} contains a Git source",
            lock.display()
        );
    }
    check_sync(root, &a)?;
    check_release_records(root, &a)?;
    println!("package identities, dependency graphs, compatibility, and provenance records agree");
    Ok(())
}

fn check_internal_declarations(root: &Path, owner: &Package, a: &Authority) -> Result<()> {
    let manifest = read_toml(&root.join(&owner.manifest))?;
    let owner_manifest = root.join(&owner.manifest);
    let owner_dir = owner_manifest
        .parent()
        .context("authority manifest has no parent directory")?;
    for table in ["dependencies", "build-dependencies", "dev-dependencies"] {
        if let Some(deps) = manifest.get(table).and_then(Value::as_table) {
            check_dependency_table(root, owner, a, owner_dir, table, "root", deps)?;
        }
    }
    if let Some(targets) = manifest.get("target").and_then(Value::as_table) {
        for (target, tables) in targets {
            for table in ["dependencies", "build-dependencies", "dev-dependencies"] {
                if let Some(deps) = tables.get(table).and_then(Value::as_table) {
                    check_dependency_table(root, owner, a, owner_dir, table, target, deps)?;
                }
            }
        }
    }
    Ok(())
}

fn check_dependency_table(
    root: &Path,
    owner: &Package,
    a: &Authority,
    owner_dir: &Path,
    table: &str,
    target_context: &str,
    dependencies: &toml::map::Map<String, Value>,
) -> Result<()> {
    for (alias, dependency) in dependencies {
        let Some(dependency) = dependency.as_table() else {
            continue;
        };
        let Some(path) = dependency.get("path").and_then(Value::as_str) else {
            continue;
        };
        let target = canonical(owner_dir.join(path));
        let Some(package) = a.package.iter().find(|package| {
            root.join(&package.manifest)
                .parent()
                .is_some_and(|parent| canonical(parent) == target)
        }) else {
            continue;
        };
        if owner.publish {
            if table == "dev-dependencies" {
                ensure!(
                    dependency.get("version").is_none(),
                    "{} {target_context} dev-dependency {alias} must be path-only so Cargo omits it from the published manifest",
                    owner.name
                );
            } else {
                ensure!(
                    dependency.get("version").and_then(Value::as_str).is_some(),
                    "{} {target_context} dependency {alias} needs path+version",
                    owner.name
                );
            }
        }
        if alias != &package.name {
            ensure!(
                dependency.get("package").and_then(Value::as_str) == Some(package.name.as_str()),
                "{} dependency {alias} needs package = {:?}",
                owner.name,
                package.name
            );
        }
        ensure!(
            owner.layer >= package.layer,
            "{} may not depend on higher layer {}",
            owner.name,
            package.name
        );
    }
    Ok(())
}

fn check_single_gpui_library(packages: &[Json], graph: &str) -> Result<()> {
    let mut owners = Vec::new();
    for package in packages {
        let targets = package["targets"]
            .as_array()
            .with_context(|| format!("{graph} package {} has no targets", package["name"]))?;
        if targets.iter().any(|target| {
            target["name"] == "gpui"
                && target["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind == "lib"))
        }) {
            owners.push(
                package["name"]
                    .as_str()
                    .with_context(|| format!("{graph} has an unnamed gpui library owner"))?,
            );
        }
    }
    ensure!(
        owners == ["gpui-box"],
        "{graph} must contain exactly one lib gpui owned by gpui-box, found {owners:?}"
    );
    Ok(())
}

fn check_sync(root: &Path, a: &Authority) -> Result<()> {
    let c: Json = serde_json::from_str(&fs::read_to_string(
        root.join("scripts/sync-zed/config.json"),
    )?)?;
    let packages = c["packages"]
        .as_object()
        .context("sync-zed config has no packages object")?;
    for (m, n) in packages {
        let p = a
            .package
            .iter()
            .find(|p| &p.manifest == m)
            .with_context(|| format!("sync package {m} absent from authority"))?;
        ensure!(
            p.name
                == n.as_str()
                    .with_context(|| format!("sync package identity for {m} is not a string"))?,
            "sync identity differs for {m}"
        );
    }
    Ok(())
}

fn check_release_records(root: &Path, a: &Authority) -> Result<()> {
    let compatibility = read_toml(&root.join("compatibility.toml"))?;
    ensure!(
        compatibility.get("schema").and_then(Value::as_integer) == Some(1),
        "unsupported compatibility.toml schema"
    );
    ensure!(
        compatibility.get("repository").and_then(Value::as_str)
            == Some("https://github.com/fran0220/gpui-box")
            && compatibility
                .get("type_universe_package")
                .and_then(Value::as_str)
                == Some("gpui-box")
            && compatibility
                .get("type_universe_crate")
                .and_then(Value::as_str)
                == Some("gpui"),
        "compatibility.toml has the wrong public identity"
    );
    let versions = a
        .package
        .iter()
        .filter(|package| package.publish)
        .map(|package| package.version.as_str())
        .collect::<HashSet<_>>();
    ensure!(
        versions.len() == 1,
        "publishable authority must have one compatibility version cohort"
    );
    let version = versions
        .iter()
        .next()
        .copied()
        .context("publishable authority has no version")?;
    let mut parts = version.split('.');
    let cohort = format!(
        "{}.{}.x",
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default()
    );
    ensure!(
        compatibility.get("cohort").and_then(Value::as_str) == Some(&cohort),
        "compatibility.toml cohort must be {cohort}"
    );
    let release_status = compatibility
        .get("release_status")
        .and_then(Value::as_str)
        .context("compatibility.toml needs release_status")?;
    ensure!(
        matches!(release_status, "unreleased" | "released"),
        "compatibility.toml has invalid release_status"
    );
    let platform = compatibility
        .get("platform")
        .and_then(Value::as_array)
        .context("compatibility.toml needs [[platform]] records")?;
    let expected_platforms = ["linux", "macos", "wasm32-unknown-unknown", "windows"]
        .into_iter()
        .collect::<HashSet<_>>();
    let actual_platforms = platform
        .iter()
        .filter_map(|record| record.get("name").and_then(Value::as_str))
        .collect::<HashSet<_>>();
    ensure!(
        actual_platforms == expected_platforms,
        "compatibility.toml platform set is incomplete"
    );
    ensure!(
        platform
            .iter()
            .all(|record| record.get("ci_gated").and_then(Value::as_bool) == Some(true)),
        "every claimed compatibility platform must be CI-gated"
    );

    let provenance = read_toml(&root.join("provenance.toml"))?;
    ensure!(
        provenance.get("schema").and_then(Value::as_integer) == Some(1)
            && provenance.get("repository").and_then(Value::as_str)
                == Some("https://github.com/fran0220/gpui-box"),
        "provenance.toml has the wrong schema or public identity"
    );
    ensure!(
        provenance.get("release_status").and_then(Value::as_str) == Some(release_status),
        "compatibility.toml and provenance.toml release status differ"
    );
    if release_status == "released" {
        let compatibility_date = compatibility
            .get("release_date")
            .and_then(Value::as_str)
            .context("released compatibility.toml needs release_date")?;
        ensure!(
            compatibility.get("release_version").and_then(Value::as_str) == Some(version)
                && provenance.get("release_version").and_then(Value::as_str) == Some(version)
                && provenance.get("release_date").and_then(Value::as_str)
                    == Some(compatibility_date)
                && compatibility_date.len() == 10
                && compatibility_date.as_bytes()[4] == b'-'
                && compatibility_date.as_bytes()[7] == b'-'
                && compatibility_date
                    .bytes()
                    .enumerate()
                    .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit()),
            "released compatibility/provenance records need matching authority version and YYYY-MM-DD date"
        );
    }
    let config: Json = serde_json::from_str(&fs::read_to_string(
        root.join("scripts/sync-zed/config.json"),
    )?)?;
    let state: Json = serde_json::from_str(&fs::read_to_string(
        root.join("scripts/sync-zed/state.json"),
    )?)?;
    let upstream = provenance
        .get("upstream")
        .context("provenance.toml needs [upstream]")?;
    for (record, source) in [
        ("official_url", "official_url"),
        ("official_baseline", "official_baseline"),
        ("bootstrap_url", "bootstrap_url"),
        ("bootstrap_revision", "bootstrap_revision"),
    ] {
        ensure!(
            upstream.get(record).and_then(Value::as_str) == config[source].as_str(),
            "provenance.toml upstream {record} differs from sync config"
        );
    }
    ensure!(
        upstream
            .get("cargo_git_dependency")
            .and_then(Value::as_bool)
            == Some(false)
            && upstream.get("official_project").and_then(Value::as_bool) == Some(false)
            && upstream.get("relationship").and_then(Value::as_str)
                == Some("filtered-imported-source"),
        "provenance.toml must describe an independent filtered source import"
    );
    let sync = provenance
        .get("sync")
        .context("provenance.toml needs [sync]")?;
    ensure!(
        sync.get("filter_schema_version")
            .and_then(Value::as_integer)
            == config["filter_schema_version"].as_i64()
            && sync.get("history_algorithm").and_then(Value::as_str)
                == config["history_algorithm"].as_str(),
        "provenance.toml sync algorithm differs from sync config"
    );
    let history_bootstrapped = state["vendor_tip"].as_str().is_some();
    ensure!(
        sync.get("history_bootstrapped").and_then(Value::as_bool) == Some(history_bootstrapped),
        "provenance.toml sync bootstrap state differs from sync receipt"
    );
    for key in [
        "bootstrap_vendor_tip",
        "vendor_tip",
        "last_synced_sha",
        "integration_commit",
    ] {
        let receipt = state[key].as_str().unwrap_or_default();
        ensure!(
            sync.get(key).and_then(Value::as_str) == Some(receipt),
            "provenance.toml {key} differs from sync receipt"
        );
    }
    let licenses = provenance
        .get("licenses")
        .context("provenance.toml needs [licenses]")?;
    ensure!(
        licenses.get("framework").and_then(Value::as_str) == Some("Apache-2.0")
            && licenses.get("kit").and_then(Value::as_str) == Some("MIT")
            && licenses.get("mcp").and_then(Value::as_str) == Some("MIT"),
        "provenance.toml license cohorts differ from package authority"
    );
    Ok(())
}

fn metadata(manifest: &Path) -> Result<Json> {
    let o = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(manifest)
        .output()?;
    ensure!(
        o.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    Ok(serde_json::from_slice(&o.stdout)?)
}
fn read_toml(path: &Path) -> Result<Value> {
    Ok(toml::from_str(&fs::read_to_string(path)?)?)
}
fn canonical(path: impl AsRef<Path>) -> PathBuf {
    path.as_ref()
        .canonicalize()
        .unwrap_or_else(|_| path.as_ref().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authority_rejects_layers() {
        let a = Authority {
            schema: 1,
            package: vec![Package {
                manifest: "x".into(),
                name: "x".into(),
                lib: None,
                cohort: "kit".into(),
                version: "0.1.0".into(),
                license: "MIT".into(),
                publish: true,
                layer: 0,
            }],
        };
        assert!(validate_authority(&a).is_err())
    }

    #[test]
    fn gpui_library_must_have_one_authoritative_owner() {
        let valid = vec![serde_json::json!({
            "name": "gpui-box",
            "targets": [{"name": "gpui", "kind": ["lib"]}]
        })];
        assert!(check_single_gpui_library(&valid, "test graph").is_ok());
        let impostor = vec![
            valid[0].clone(),
            serde_json::json!({
                "name": "another-package",
                "targets": [{"name": "gpui", "kind": ["lib"]}]
            }),
        ];
        assert!(check_single_gpui_library(&impostor, "test graph").is_err());
    }
}
