use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use toml::Value;

const ZED_REPOSITORY: &str = "https://github.com/fran0220/zed";

#[derive(Debug, Eq, PartialEq)]
struct Pin {
    repository: String,
    revision: String,
}

pub fn check(root: &Path, args: &[String]) -> Result<()> {
    let expected_revision = match args {
        [] => None,
        [flag, revision] if flag == "--expected-zed-revision" => Some(revision.as_str()),
        _ => bail!(
            "usage: cargo xtask dependencies check \
             [--expected-zed-revision <40-character-sha>]"
        ),
    };

    let root_manifest = read_manifest(&root.join("Cargo.toml"))?;
    let headless_manifest = read_manifest(&root.join("tools/headless-visual/Cargo.toml"))?;
    let authority = dependency_pin(&root_manifest, &["workspace", "dependencies"], "gpui")?;

    ensure!(
        authority.repository == ZED_REPOSITORY,
        "workspace GPUI must come from {ZED_REPOSITORY}, found {}",
        authority.repository
    );
    ensure!(
        is_full_git_revision(&authority.revision),
        "workspace GPUI revision must be a 40-character lowercase Git SHA, found {}",
        authority.revision
    );
    if let Some(expected_revision) = expected_revision {
        ensure!(
            authority.revision == expected_revision,
            "workspace pins Zed revision {}, expected {expected_revision}",
            authority.revision
        );
    }

    let pins = [
        (
            "root gpui_platform",
            dependency_pin(
                &root_manifest,
                &["workspace", "dependencies"],
                "gpui_platform",
            )?,
        ),
        (
            "headless gpui",
            dependency_pin(&headless_manifest, &["dependencies"], "gpui")?,
        ),
        (
            "headless gpui_platform",
            dependency_pin(
                &headless_manifest,
                &[
                    "target",
                    "cfg(any(target_os = \"linux\", target_os = \"windows\"))",
                    "dependencies",
                ],
                "gpui_platform",
            )?,
        ),
        (
            "headless gpui_wgpu",
            dependency_pin(
                &headless_manifest,
                &[
                    "target",
                    "cfg(any(target_os = \"linux\", target_os = \"windows\"))",
                    "dependencies",
                ],
                "gpui_wgpu",
            )?,
        ),
    ];
    for (label, pin) in pins {
        ensure!(
            pin == authority,
            "{label} uses {pin:?}, but the workspace authority is {authority:?}"
        );
    }
    ensure!(
        headless_manifest.get("patch").is_none(),
        "tools/headless-visual must use the workspace Zed pin directly, not a [patch]"
    );

    check_lockfile(&root.join("Cargo.lock"), &authority)?;
    check_lockfile(&root.join("tools/headless-visual/Cargo.lock"), &authority)?;

    for relative in [
        "README.md",
        "docs/compatibility.md",
        "PROVENANCE.md",
        "THIRD_PARTY_NOTICES",
    ] {
        let path = root.join(relative);
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("read dependency declaration {}", path.display()))?;
        ensure!(
            contents.contains(&authority.repository) && contents.contains(&authority.revision),
            "{relative} must declare the authoritative Zed repository and revision"
        );
    }

    println!(
        "dependency pins agree on {}@{}",
        authority.repository, authority.revision
    );
    Ok(())
}

fn read_manifest(path: &Path) -> Result<Value> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("read manifest {}", path.display()))?;
    toml::from_str(&contents).with_context(|| format!("parse manifest {}", path.display()))
}

fn dependency_pin(manifest: &Value, table_path: &[&str], dependency: &str) -> Result<Pin> {
    let mut table = manifest;
    for component in table_path {
        table = table
            .get(component)
            .with_context(|| format!("missing manifest table component `{component}`"))?;
    }
    let dependency = table
        .get(dependency)
        .with_context(|| format!("missing dependency `{dependency}`"))?;
    let repository = dependency
        .get("git")
        .and_then(Value::as_str)
        .context("Git dependency is missing `git`")?;
    let revision = dependency
        .get("rev")
        .and_then(Value::as_str)
        .context("Git dependency is missing `rev`")?;
    Ok(Pin {
        repository: repository.to_owned(),
        revision: revision.to_owned(),
    })
}

fn is_full_git_revision(revision: &str) -> bool {
    revision.len() == 40
        && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        && revision.bytes().all(|byte| !byte.is_ascii_uppercase())
}

fn check_lockfile(path: &Path, pin: &Pin) -> Result<()> {
    let contents = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let expected = format!(
        "source = \"git+{}?rev={}#{}\"",
        pin.repository, pin.revision, pin.revision
    );
    let sources = contents
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            line.starts_with("source = \"git+")
                && (line.contains("/zed?") || line.contains("/zed#") || line.contains("/zed.git"))
        })
        .collect::<Vec<_>>();
    ensure!(
        !sources.is_empty(),
        "{} contains no locked Zed packages",
        path.display()
    );
    for source in sources {
        ensure!(
            source.trim() == expected,
            "{} contains a divergent Zed source: {source}",
            path.display()
        );
    }
    Ok(())
}
