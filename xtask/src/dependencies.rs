use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use serde_json::Value as Json;
use sha2::{Digest, Sha256};
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
    check_resolved_layers(&root_meta, &a)?;
    check_resolved_layers(&head_meta, &a)?;
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
        if let Some(owned) = a.package.iter().find(|owned| p["name"] == owned.name) {
            ensure!(
                p["source"].is_null() && path == canonical(root.join(&owned.manifest)),
                "resolved internal package {} is not its local authority",
                owned.name
            );
        }
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
    check_vendored_block_patch(root)?;
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

fn check_vendored_block_patch(root: &Path) -> Result<()> {
    for (manifest, expected_path) in [
        ("Cargo.toml", "vendor/block"),
        ("tools/headless-visual/Cargo.toml", "../../vendor/block"),
    ] {
        let manifest_path = root.join(manifest);
        let value = read_toml(&manifest_path)?;
        let patch = value
            .get("patch")
            .and_then(Value::as_table)
            .with_context(|| format!("{} has no [patch] table", manifest_path.display()))?;
        ensure!(
            patch.len() == 1 && patch.contains_key("crates-io"),
            "{} may patch only crates.io",
            manifest_path.display()
        );
        let crates_io = patch["crates-io"].as_table().with_context(|| {
            format!(
                "{} [patch.crates-io] is not a table",
                manifest_path.display()
            )
        })?;
        ensure!(
            crates_io.len() == 1,
            "{} may contain only the audited block patch",
            manifest_path.display()
        );
        let block = crates_io
            .get("block")
            .and_then(Value::as_table)
            .with_context(|| format!("{} has no block patch", manifest_path.display()))?;
        ensure!(
            block.len() == 1 && block.get("path").and_then(Value::as_str) == Some(expected_path),
            "{} block patch must resolve from {expected_path}",
            manifest_path.display()
        );
    }

    let vendored = read_toml(&root.join("vendor/block/Cargo.toml"))?;
    ensure!(
        vendored["package"]["name"].as_str() == Some("block")
            && vendored["package"]["version"].as_str() == Some("0.1.6")
            && vendored["package"]["license"].as_str() == Some("MIT"),
        "vendored block identity must remain block 0.1.6 under MIT"
    );
    let source = fs::read(root.join("vendor/block/src/lib.rs"))?;
    ensure!(
        format!("{:x}", Sha256::digest(source))
            == "51e54353cee1cc853e567d140d35b4a74e27d5cbdbcbe68e979269f39209906a",
        "vendored block source differs from its audited receipt"
    );
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
        let inherited;
        let mut dependency_dir = owner_dir.to_path_buf();
        let dependency = if dependency.get("workspace").and_then(Value::as_bool) == Some(true) {
            let (directory, workspace) = owner_dir
                .ancestors()
                .find_map(|directory| {
                    let manifest = read_toml(&directory.join("Cargo.toml")).ok()?;
                    manifest
                        .get("workspace")
                        .cloned()
                        .map(|workspace| (directory, workspace))
                })
                .context("inherited dependency has no workspace")?;
            inherited = workspace
                .get("dependencies")
                .and_then(|deps| deps.get(alias))
                .with_context(|| format!("workspace has no dependency {alias}"))?
                .clone();
            dependency_dir = directory.to_path_buf();
            &inherited
        } else {
            dependency
        };
        let name = dependency
            .get("package")
            .and_then(Value::as_str)
            .unwrap_or(alias);
        let named = a.package.iter().find(|package| package.name == name);
        if named.is_some() {
            ensure!(
                dependency.get("path").and_then(Value::as_str).is_some(),
                "{} internal dependency {alias} must use local authority, not a registry",
                owner.name
            );
        }
        let Some(dependency) = dependency.as_table() else {
            continue;
        };
        let Some(path) = dependency.get("path").and_then(Value::as_str) else {
            continue;
        };
        let target = canonical(dependency_dir.join(path));
        if let Some(named) = named {
            ensure!(
                target
                    == canonical(
                        root.join(&named.manifest)
                            .parent()
                            .context("authority manifest parent")?
                    ),
                "{} internal dependency {alias} has the wrong local path",
                owner.name
            );
        }
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

fn check_resolved_layers(metadata: &Json, a: &Authority) -> Result<()> {
    let packages = metadata["packages"]
        .as_array()
        .context("metadata packages missing")?;
    let owned = |id: &Json| {
        let package = packages.iter().find(|package| package["id"] == *id)?;
        a.package.iter().find(|owned| package["name"] == owned.name)
    };
    for node in metadata["resolve"]["nodes"]
        .as_array()
        .context("resolved dependency edges missing")?
    {
        let Some(owner) = owned(&node["id"]) else {
            continue;
        };
        for edge in node["deps"]
            .as_array()
            .context("resolved node deps missing")?
        {
            if let Some(dependency) = owned(&edge["pkg"]) {
                ensure!(
                    owner.layer >= dependency.layer,
                    "resolved edge {} -> {} crosses into a higher layer",
                    owner.name,
                    dependency.name
                );
            }
        }
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
    ensure!(
        c["schema_version"] == 3 && c["mode"] == "frozen-historical-import",
        "Zed provenance config must be a frozen historical import"
    );
    let packages = c["packages"]
        .as_object()
        .context("historical Zed import config has no packages object")?;
    for (m, n) in packages {
        let p = a
            .package
            .iter()
            .find(|p| &p.manifest == m)
            .with_context(|| format!("historical import package {m} absent from authority"))?;
        ensure!(
            p.name
                == n.as_str().with_context(|| format!(
                    "historical package identity for {m} is not a string"
                ))?,
            "historical import identity differs for {m}"
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
    // A gated platform is proven in one of two lanes: `authority` runs inside
    // `xtask gate` on the Linux orb at every commit, `on-demand` is the
    // dispatched `Platforms` workflow for renderers that cannot run there.
    // Either way the commands that produce the evidence must be written down.
    for record in platform {
        let name = record.get("name").and_then(Value::as_str).unwrap_or("?");
        let lane = record
            .get("gate_lane")
            .and_then(Value::as_str)
            .with_context(|| format!("compatibility.toml platform {name} needs gate_lane"))?;
        ensure!(
            matches!(lane, "authority" | "on-demand"),
            "compatibility.toml platform {name} has gate_lane {lane:?}; expected authority or on-demand"
        );
        ensure!(
            record
                .get("ci_commands")
                .and_then(Value::as_array)
                .is_some_and(|commands| !commands.is_empty()),
            "compatibility.toml platform {name} needs non-empty ci_commands"
        );
    }

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
        .get("historical_source")
        .context("provenance.toml needs [historical_source]")?;
    for (record, source) in [
        ("official_url", "official_url"),
        ("official_baseline", "official_baseline"),
        ("bootstrap_url", "bootstrap_url"),
        ("bootstrap_revision", "bootstrap_revision"),
    ] {
        ensure!(
            upstream.get(record).and_then(Value::as_str) == config[source].as_str(),
            "provenance.toml historical source {record} differs from import config"
        );
    }
    ensure!(
        upstream.get("mode").and_then(Value::as_str) == Some("frozen-historical-import")
            && upstream
                .get("cargo_git_dependency")
                .and_then(Value::as_bool)
                == Some(false)
            && upstream.get("official_project").and_then(Value::as_bool) == Some(false)
            && upstream.get("relationship").and_then(Value::as_str)
                == Some("frozen-filtered-import"),
        "provenance.toml must describe an independent frozen source import"
    );
    let sync = provenance
        .get("historical_import")
        .context("provenance.toml needs [historical_import]")?;
    ensure!(
        state.get("schema_version").and_then(Json::as_i64) == Some(3)
            && state.get("mode").and_then(Json::as_str) == Some("frozen-historical-import")
            && state.get("tool_version").and_then(Json::as_str) == Some("3.0.0")
            && sync
                .get("filter_schema_version")
                .and_then(Value::as_integer)
                == config["filter_schema_version"].as_i64()
            && sync.get("history_algorithm").and_then(Value::as_str)
                == config["history_algorithm"].as_str(),
        "historical Zed import mode or algorithm differs across records"
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
            "provenance.toml {key} differs from historical import receipt"
        );
    }
    let overlay = provenance
        .get("historical_overlay")
        .context("provenance.toml needs [historical_overlay]")?;
    let state_overlay = state["fork_overlay"]
        .as_object()
        .context("historical import state needs fork_overlay")?;
    for key in [
        "algorithm",
        "source_url",
        "base_revision",
        "source_tip",
        "filter_digest_sha256",
        "base_vendor_tip",
        "vendor_ref",
        "vendor_tip",
        "integration_commit",
    ] {
        ensure!(
            overlay.get(key).and_then(Value::as_str) == state_overlay[key].as_str(),
            "provenance.toml historical overlay {key} differs from receipt"
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
    fn inherited_dependencies_cannot_bypass_layers_or_local_authority() -> Result<()> {
        let root =
            std::env::temp_dir().join(format!("gpui-dependency-fixture-{}", std::process::id()));
        fs::create_dir_all(root.join("low"))?;
        fs::create_dir_all(root.join("high"))?;
        let low = Package {
            manifest: "low/Cargo.toml".into(),
            name: "low".into(),
            lib: None,
            cohort: "framework".into(),
            version: "0.1.0".into(),
            license: "MIT".into(),
            publish: false,
            layer: 0,
        };
        let high = Package {
            manifest: "high/Cargo.toml".into(),
            name: "high".into(),
            layer: 1,
            ..low.clone()
        };
        let a = Authority {
            schema: 1,
            package: vec![low.clone(), high],
        };
        let mut graph = serde_json::json!({
            "packages": [{"id":"local-low", "name":"low"}, {"id":"local-high", "name":"high"}],
            "resolve": {"nodes": [{"id":"local-low", "deps":[{"name":"renamed", "pkg":"local-high"}]}]}
        });
        assert!(check_resolved_layers(&graph, &a).is_err());
        graph["resolve"]["nodes"][0] =
            serde_json::json!({"id":"local-high", "deps":[{"pkg":"local-low"}]});
        check_resolved_layers(&graph, &a)?;
        fs::write(
            root.join("low/Cargo.toml"),
            "[dependencies]\nrenamed = { workspace = true }\n",
        )?;
        fs::write(
            root.join("Cargo.toml"),
            "[workspace.dependencies]\nrenamed = { package = 'high', path = 'high', version = '0.1.0' }\n",
        )?;
        assert!(
            check_internal_declarations(&root, &low, &a)
                .expect_err("inherited higher-layer dependency must fail")
                .to_string()
                .contains("higher layer")
        );
        let owner = Package { layer: 2, ..low };
        check_internal_declarations(&root, &owner, &a)?;
        fs::write(
            root.join("Cargo.toml"),
            "[workspace.dependencies]\nrenamed = { package = 'high', version = '0.1.0' }\n",
        )?;
        assert!(
            check_internal_declarations(&root, &owner, &a)
                .expect_err("inherited registry dependency must fail")
                .to_string()
                .contains("registry")
        );
        fs::write(
            root.join("low/Cargo.toml"),
            "[target.'cfg(windows)'.build-dependencies]\nhigh = '0.1.0'\n",
        )?;
        assert!(
            check_internal_declarations(&root, &owner, &a)
                .expect_err("target registry dependency must fail")
                .to_string()
                .contains("registry")
        );
        fs::remove_dir_all(root)?;
        Ok(())
    }

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
