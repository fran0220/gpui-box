use crate::dependencies::{Package, authority};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

const LOCAL_REGISTRY_VERSION: &str = "cargo-local-registry 0.2.12";
const PUBLISH_OPT_IN: &str = "GPUI_BOX_PUBLISH";
const CRATES_IO_USER_AGENT: &str = "gpui-box-release/0.1 (https://github.com/fran0220/gpui-box)";
const INITIAL_RELEASE_VERSION: &str = "0.1.0";
const INITIAL_RELEASE_COMMIT: &str = "888369c73c258567664785a761faebdc64d39d4e";
const VENDORED_BLOCK_NAME: &str = "block";
const VENDORED_BLOCK_VERSION: &str = "0.1.6";
const NEW_CRATE_RATE_LIMIT_WAIT: Duration = Duration::from_secs(10 * 60 + 5);
const NEW_CRATE_RATE_LIMIT_RETRIES: usize = 3;

struct PublicationPlan {
    packages: BTreeMap<String, Package>,
    order: Vec<String>,
    external: BTreeSet<String>,
}

pub fn plan(root: &Path) -> Result<()> {
    let publication = publication_plan(root)?;
    for (i, name) in publication.order.iter().enumerate() {
        let p = publication
            .packages
            .get(name)
            .with_context(|| format!("publication order contains unknown package {name}"))?;
        println!("{:2}. {} {} [{}]", i + 1, name, p.version, p.cohort);
    }
    println!(
        "external dependencies: {}",
        publication
            .external
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(())
}

fn publication_plan(root: &Path) -> Result<PublicationPlan> {
    let a = authority(root)?;
    let packages = a
        .package
        .into_iter()
        .filter(|p| p.publish)
        .map(|p| (p.name.clone(), p))
        .collect::<BTreeMap<_, _>>();
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .current_dir(root)
        .output()?;
    ensure!(output.status.success(), "cargo metadata failed");
    let meta: Value = serde_json::from_slice(&output.stdout)?;
    let metadata_packages = meta["packages"]
        .as_array()
        .context("Cargo metadata has no packages array")?;
    let mut edges: BTreeMap<String, BTreeSet<String>> = packages
        .keys()
        .map(|n| (n.clone(), BTreeSet::new()))
        .collect();
    let mut external = BTreeSet::new();
    for p in metadata_packages {
        let Some(owner) = p["name"].as_str().filter(|n| packages.contains_key(*n)) else {
            continue;
        };
        let dependencies = p["dependencies"]
            .as_array()
            .with_context(|| format!("Cargo metadata has no dependencies for {owner}"))?;
        for d in dependencies {
            if !dependency_is_published(d)? {
                continue;
            }
            let name = d["name"]
                .as_str()
                .with_context(|| format!("Cargo metadata has an unnamed dependency for {owner}"))?;
            if packages.contains_key(name) {
                edges
                    .get_mut(owner)
                    .with_context(|| format!("publication graph lacks package {owner}"))?
                    .insert(name.into());
            } else {
                external.insert(name.into());
            }
        }
    }
    let mut order = Vec::new();
    while !edges.is_empty() {
        let ready = edges
            .iter()
            .filter(|(_, d)| d.is_empty())
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>();
        if ready.is_empty() {
            bail!("publish dependency cycle: {:?}", edges)
        }
        for n in ready {
            edges.remove(&n);
            for d in edges.values_mut() {
                d.remove(&n);
            }
            order.push(n)
        }
    }
    Ok(PublicationPlan {
        packages,
        order,
        external,
    })
}

fn dependency_is_published(dependency: &Value) -> Result<bool> {
    if dependency["kind"].as_str() != Some("dev") {
        return Ok(true);
    }
    let requirement = dependency["req"]
        .as_str()
        .context("Cargo metadata dependency has no version requirement")?;
    Ok(!(dependency["path"].as_str().is_some() && requirement == "*"))
}

fn package_patches(root: &Path, packages: &BTreeMap<String, Package>) -> Result<Vec<String>> {
    packages
        .values()
        .map(|package| {
            let manifest = root.join(&package.manifest);
            let parent = manifest
                .parent()
                .with_context(|| format!("package manifest has no parent: {}", package.manifest))?;
            Ok(format!(
                "patch.crates-io.{}.path='{}'",
                package.name,
                parent.display()
            ))
        })
        .collect()
}

fn apply_package_patches(command: &mut Command, patches: &[String]) {
    for patch in patches {
        command.args(["--config", patch]);
    }
}

pub fn check(root: &Path) -> Result<()> {
    let PublicationPlan {
        packages, order, ..
    } = publication_plan(root)?;
    let out = root.join("target/package-check");
    if out.exists() {
        fs::remove_dir_all(&out)?
    }
    fs::create_dir_all(out.join("archives"))?;
    let patches = package_patches(root, &packages)?;
    for name in &order {
        let p = &packages[name];
        let mut cmd = Command::new("cargo");
        cmd.args([
            "package",
            "--allow-dirty",
            "--locked",
            "--no-verify",
            "--manifest-path",
        ])
        .arg(root.join(&p.manifest))
        .env("CARGO_TARGET_DIR", &out);
        apply_package_patches(&mut cmd, &patches);
        let status = cmd.status()?;
        ensure!(status.success(), "cargo package failed for {name}");
        let archive_name = format!("{}-{}.crate", name, p.version);
        let archive = out.join("package").join(&archive_name);
        ensure!(archive.is_file(), "missing {}", archive.display());
        fs::copy(&archive, out.join("archives").join(archive_name))?;
        let listing = Command::new("tar").args(["tzf"]).arg(&archive).output()?;
        ensure!(listing.status.success(), "cannot inspect {name} archive");
        let listing = String::from_utf8(listing.stdout)?;
        for line in listing.lines() {
            ensure!(
                !line.starts_with('/') && !line.split('/').any(|x| x == ".."),
                "unsafe archive member {line}"
            );
        }
        let lower_listing = listing.to_ascii_lowercase();
        check_archive_legal(name, &p.cohort, &lower_listing)?;
        let manifest = Command::new("tar")
            .args(["xOzf"])
            .arg(&archive)
            .arg(format!("{}-{}/Cargo.toml", name, p.version))
            .output()?;
        ensure!(
            manifest.status.success(),
            "normalized Cargo.toml missing for {name}"
        );
        check_normalized(&String::from_utf8(manifest.stdout)?, p, &packages)?;
    }
    // The workspace lock records the audited `block` fork as a path package,
    // so cargo-local-registry cannot download it from crates.io. Package that
    // exact source explicitly instead of relying on a developer's Cargo cache.
    let block_manifest = root.join("vendor/block/Cargo.toml");
    let block: toml::Value = toml::from_str(&fs::read_to_string(&block_manifest)?)?;
    let block_package = &block["package"];
    let block_name = block_package["name"]
        .as_str()
        .context("vendored block manifest has no package name")?;
    let block_version = block_package["version"]
        .as_str()
        .context("vendored block manifest has no package version")?;
    ensure!(
        block_name == VENDORED_BLOCK_NAME && block_version == VENDORED_BLOCK_VERSION,
        "vendored compatibility fork must be {VENDORED_BLOCK_NAME} {VENDORED_BLOCK_VERSION}, found {block_name} {block_version}"
    );
    let block_status = Command::new("cargo")
        .args([
            "package",
            "--allow-dirty",
            "--locked",
            "--no-verify",
            "--manifest-path",
        ])
        .arg(&block_manifest)
        .env("CARGO_TARGET_DIR", &out)
        .status()?;
    ensure!(
        block_status.success(),
        "cargo package failed for {block_name}"
    );
    let block_archive = out
        .join("package")
        .join(format!("{block_name}-{block_version}.crate"));
    ensure!(
        block_archive.is_file(),
        "missing {}",
        block_archive.display()
    );
    let version = Command::new("cargo-local-registry")
        .arg("--version")
        .output();
    ensure!(
        version.as_ref().is_ok_and(|v| v.status.success()
            && String::from_utf8_lossy(&v.stdout).trim() == LOCAL_REGISTRY_VERSION),
        "cargo-local-registry 0.2.12 is required for registry-only consumer verification; install it explicitly with `cargo install cargo-local-registry --version 0.2.12 --locked`"
    );
    let registry = out.join("registry");
    fs::create_dir_all(&registry)?;
    let status = Command::new("cargo-local-registry")
        .args(["sync"])
        .arg(root.join("Cargo.lock"))
        .arg(&registry)
        .status()?;
    ensure!(
        status.success(),
        "local registry sync failed; partial registry retained at {}",
        registry.display()
    );
    // The archives produced above are authoritative, so always replace both
    // bytes and their index records after sync has populated crates.io inputs.
    insert_archive(&registry, &block_archive)?;
    for name in &order {
        let p = &packages[name];
        insert_archive(
            &registry,
            &out.join("archives")
                .join(format!("{}-{}.crate", name, p.version)),
        )?;
    }
    let cargo_home = out.join("cargo-home");
    fs::create_dir_all(&cargo_home)?;
    fs::write(
        cargo_home.join("config.toml"),
        format!(
            "[source.crates-io]\nreplace-with = 'package-check'\n[source.package-check]\nlocal-registry = '{}'\n[net]\noffline = true\n",
            registry.display()
        ),
    )?;
    let consumers = out.join("consumers");
    let framework_version = &packages["gpui-box"].version;
    let kit_version = &packages["gpui-box-kit"].version;
    create_consumer(
        &consumers.join("framework-only"),
        &format!(
            "gpui = {{ package = \"gpui-box\", version = \"={framework_version}\" }}\ngpui_platform = {{ package = \"gpui-box-platform\", version = \"={framework_version}\" }}"
        ),
        "use gpui::Application;\nfn main() { let _: fn() -> Application = gpui_platform::application; }\n",
    )?;
    create_consumer(
        &consumers.join("framework-and-kit"),
        &format!(
            "gpui = {{ package = \"gpui-box\", version = \"={framework_version}\" }}\ngpui_kit = {{ package = \"gpui-box-kit\", version = \"={kit_version}\" }}"
        ),
        "use gpui::{SharedString, div};\nuse gpui_kit::prelude::Button;\nfn main() { let _: SharedString = \"ok\".into(); let _ = div(); let _ = Button::new(\"consumer-button\"); }\n",
    )?;
    create_consumer(
        &consumers.join("framework-property-test"),
        &format!(
            "gpui = {{ package = \"gpui-box\", version = \"={framework_version}\", features = [\"test-support\"] }}"
        ),
        "#[gpui::property_test(config = gpui::proptest::test_runner::Config::with_cases(2))]\nfn identity(#[strategy = 0u8..4] value: u8) -> gpui::proptest::test_runner::TestCaseResult { gpui::proptest::prop_assert_eq!(value, value); Ok(()) }\nfn main() {}\n",
    )?;
    for consumer in [
        consumers.join("framework-only"),
        consumers.join("framework-and-kit"),
    ] {
        let metadata = cargo_consumer(
            &consumer,
            &cargo_home,
            &["metadata", "--format-version", "1", "--offline"],
        )?;
        check_consumer_metadata(&serde_json::from_slice(&metadata)?, root)?;
        cargo_consumer(&consumer, &cargo_home, &["check", "--offline"])?;
    }
    let property_consumer = consumers.join("framework-property-test");
    let metadata = cargo_consumer(
        &property_consumer,
        &cargo_home,
        &["metadata", "--format-version", "1", "--offline"],
    )?;
    check_consumer_metadata(&serde_json::from_slice(&metadata)?, root)?;
    cargo_consumer(&property_consumer, &cargo_home, &["test", "--offline"])?;
    install_registry_mcp(
        &consumers.join("mcp-install"),
        &cargo_home,
        &packages["gpui-box-mcp"].version,
    )?;
    // Consumers, the local registry, and its Cargo home are disposable proof
    // inputs. Retaining them adds several gigabytes to CI cache traversal and
    // can race rust-cache against transient crate test directories. Keep the
    // authoritative archives on success; an early error still preserves the
    // complete package-check tree for diagnosis.
    for temporary in ["cargo-home", "consumers", "package", "registry"] {
        fs::remove_dir_all(out.join(temporary))?;
    }
    Ok(())
}

fn check_archive_legal(name: &str, cohort: &str, listing: &str) -> Result<()> {
    let has = |needle: &str| listing.lines().any(|line| line.contains(needle));
    match cohort {
        "framework" => ensure!(
            has("license-apache") || has("apache-2.0"),
            "{name} archive lacks its Apache-2.0 license file"
        ),
        "kit" | "tool" => ensure!(
            has("license-mit") || has("/license"),
            "{name} archive lacks its MIT license file"
        ),
        other => bail!("{name} has unsupported publish cohort {other}"),
    }
    if name == "gpui-box-kit-assets" {
        ensure!(
            has("third_party_notices"),
            "asset archive lacks THIRD_PARTY_NOTICES"
        );
        for license in ["ofl", "cc-by", "mit"] {
            ensure!(
                has(license),
                "asset archive lacks the {license} license bundle"
            );
        }
    }
    Ok(())
}

pub fn publish(root: &Path, args: &[String]) -> Result<()> {
    ensure!(
        args == ["--execute"],
        "publishing is disabled by default; exact usage is `GPUI_BOX_PUBLISH=1 cargo run -p xtask -- package publish --execute` (this uses --no-verify only because package check archives must already exist)"
    );
    ensure!(
        std::env::var(PUBLISH_OPT_IN).as_deref() == Ok("1"),
        "refusing to publish without GPUI_BOX_PUBLISH=1"
    );
    let PublicationPlan {
        packages, order, ..
    } = publication_plan(root)?;
    let versions: BTreeSet<_> = packages.values().map(|p| p.version.as_str()).collect();
    ensure!(
        versions.len() == 1,
        "publish authority does not have one unified version"
    );
    let version = versions
        .first()
        .copied()
        .context("publish authority has no packages")?;
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()?;
    ensure!(
        status.status.success() && status.stdout.is_empty(),
        "publishing requires a clean worktree"
    );
    let tag_ref = format!("refs/tags/v{version}");
    let tag_type = Command::new("git")
        .args(["cat-file", "-t", &tag_ref])
        .current_dir(root)
        .output()?;
    ensure!(
        tag_type.status.success() && String::from_utf8_lossy(&tag_type.stdout).trim() == "tag",
        "release tag v{version} must exist and be annotated"
    );
    let tag_commit = Command::new("git")
        .args(["rev-list", "-n", "1", &tag_ref])
        .current_dir(root)
        .output()?;
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;
    ensure!(
        tag_commit.status.success() && head.status.success() && tag_commit.stdout == head.stdout,
        "annotated release tag v{version} must point at HEAD"
    );
    if version == INITIAL_RELEASE_VERSION {
        ensure!(
            String::from_utf8_lossy(&head.stdout).trim() == INITIAL_RELEASE_COMMIT,
            "initial release v{version} must remain pinned to {INITIAL_RELEASE_COMMIT}"
        );
    }

    let patches = package_patches(root, &packages)?;
    let reproduction = root.join("target/publish-reproduction");
    if reproduction.exists() {
        fs::remove_dir_all(&reproduction)?;
    }

    for name in order {
        let p = &packages[&name];
        let archive = root
            .join("target/package-check/archives")
            .join(format!("{name}-{}.crate", p.version));
        ensure!(
            archive.is_file(),
            "missing checked archive {}; run `cargo run -p xtask -- package check` in this workflow first",
            archive.display()
        );
        let expected = sha256_file(&archive)?;
        let mut package = Command::new("cargo");
        package
            .args(["package", "--locked", "--no-verify", "--manifest-path"])
            .arg(root.join(&p.manifest))
            .current_dir(root)
            .env("CARGO_TARGET_DIR", &reproduction);
        apply_package_patches(&mut package, &patches);
        let status = package.status()?;
        ensure!(
            status.success(),
            "could not reproduce checked archive for {name}"
        );
        let reproduced = reproduction
            .join("package")
            .join(format!("{name}-{}.crate", p.version));
        ensure!(
            reproduced.is_file() && sha256_file(&reproduced)? == expected,
            "publisher did not reproduce the checked archive for {name}; refusing to upload"
        );
        if remote_version_exists(&name, &p.version)? {
            match remote_archive_checksum(&name, &p.version)? {
                Some(found) => {
                    ensure!(
                        found == expected,
                        "crates.io {name} {} exists with a different archive checksum",
                        p.version
                    );
                    ensure!(
                        remote_index_contains(&name, &p.version)?,
                        "crates.io {name} {} is downloadable but absent or yanked in the sparse index; refusing to continue",
                        p.version
                    );
                }
                None => bail!(
                    "crates.io reports {name} {} exists, but its archive is unavailable; refusing to continue",
                    p.version
                ),
            }
        } else {
            let mut publish = Command::new("cargo");
            publish
                .args(["publish", "--locked", "--no-verify", "--manifest-path"])
                .arg(root.join(&p.manifest))
                .current_dir(root);
            apply_package_patches(&mut publish, &patches);
            let mut accepted = false;
            for attempt in 0..=NEW_CRATE_RATE_LIMIT_RETRIES {
                let output = publish.output()?;
                io::stdout().write_all(&output.stdout)?;
                io::stderr().write_all(&output.stderr)?;
                if output.status.success() || remote_version_exists(&name, &p.version)? {
                    accepted = true;
                    break;
                }
                if publish_was_new_crate_rate_limited(&output.stdout, &output.stderr)
                    && attempt < NEW_CRATE_RATE_LIMIT_RETRIES
                {
                    eprintln!(
                        "crates.io rate-limited new crate {name}; waiting {} seconds before retry {} of {}",
                        NEW_CRATE_RATE_LIMIT_WAIT.as_secs(),
                        attempt + 1,
                        NEW_CRATE_RATE_LIMIT_RETRIES
                    );
                    thread::sleep(NEW_CRATE_RATE_LIMIT_WAIT);
                    continue;
                }
                break;
            }
            ensure!(
                accepted,
                "cargo publish failed for {name}; check crates.io before retrying"
            );
            let mut verified = false;
            for delay in [2, 4, 8, 16, 30, 60] {
                thread::sleep(Duration::from_secs(delay));
                if remote_version_exists(&name, &p.version)?
                    && remote_archive_checksum(&name, &p.version)?.as_deref() == Some(&expected)
                    && remote_index_contains(&name, &p.version)?
                {
                    verified = true;
                    break;
                }
            }
            ensure!(
                verified,
                "published {name} but its exact metadata, archive, and unyanked sparse-index entry did not all become visible within the bounded wait; inspect crates.io before resuming"
            );
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

fn publish_was_new_crate_rate_limited(stdout: &[u8], stderr: &[u8]) -> bool {
    let output = format!(
        "{}\n{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    )
    .to_ascii_lowercase();
    output.contains("429 too many requests")
        && output.contains("published too many new crates in a short period of time")
}

fn validate_remote_coordinate(name: &str, version: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_'),
        "invalid crates.io package name"
    );
    ensure!(
        !version.is_empty()
            && version
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'+')),
        "invalid crates.io version"
    );
    Ok(())
}

fn crate_download_url(name: &str, version: &str) -> Result<String> {
    validate_remote_coordinate(name, version)?;
    Ok(format!(
        "https://static.crates.io/crates/{name}/{name}-{version}.crate"
    ))
}

fn crate_version_url(name: &str, version: &str) -> Result<String> {
    validate_remote_coordinate(name, version)?;
    Ok(format!("https://crates.io/api/v1/crates/{name}/{version}"))
}

fn version_exists_from_status(code: &str, name: &str, version: &str) -> Result<bool> {
    match code {
        "200" => Ok(true),
        "404" => Ok(false),
        _ => bail!(
            "crates.io version lookup returned HTTP {code} for {name} {version}; refusing to publish"
        ),
    }
}

fn remote_version_exists(name: &str, version: &str) -> Result<bool> {
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--output",
            "/dev/null",
            "--user-agent",
            CRATES_IO_USER_AGENT,
            "--write-out",
            "%{http_code}",
            &crate_version_url(name, version)?,
        ])
        .output()?;
    ensure!(
        output.status.success(),
        "crates.io version lookup failed for {name} {version}"
    );
    version_exists_from_status(&String::from_utf8(output.stdout)?, name, version)
}

fn remote_archive_checksum(name: &str, version: &str) -> Result<Option<String>> {
    let temp = std::env::temp_dir().join(format!(
        "gpui-box-download-{}-{name}.crate",
        std::process::id()
    ));
    let output = Command::new("curl")
        .args(["--silent", "--show-error", "--location", "--output"])
        .arg(&temp)
        .args([
            "--user-agent",
            CRATES_IO_USER_AGENT,
            "--write-out",
            "%{http_code}",
            &crate_download_url(name, version)?,
        ])
        .output()?;
    ensure!(
        output.status.success(),
        "crates.io download request failed for {name}"
    );
    let code = String::from_utf8(output.stdout)?;
    let result = match code.as_str() {
        "200" => Some(sha256_file(&temp)?),
        "403" | "404" => None,
        _ => bail!("crates.io download returned HTTP {code} for {name}; refusing to publish"),
    };
    let _ = fs::remove_file(temp);
    Ok(result)
}

fn remote_index_contains(name: &str, version: &str) -> Result<bool> {
    validate_remote_coordinate(name, version)?;
    let url = format!("https://index.crates.io/{}", index_relative_path(name));
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--user-agent",
            CRATES_IO_USER_AGENT,
            "--write-out",
            "\n%{http_code}",
            &url,
        ])
        .output()?;
    ensure!(
        output.status.success(),
        "crates.io sparse-index request failed for {name}"
    );
    let output = String::from_utf8(output.stdout)?;
    let (body, code) = output
        .rsplit_once('\n')
        .context("crates.io sparse-index response has no HTTP status")?;
    match code {
        "404" => Ok(false),
        "200" => {
            for line in body.lines().filter(|line| !line.is_empty()) {
                let entry: Value = serde_json::from_str(line)
                    .with_context(|| format!("invalid crates.io sparse-index entry for {name}"))?;
                if entry["vers"].as_str() == Some(version) {
                    return Ok(entry["yanked"].as_bool() == Some(false));
                }
            }
            Ok(false)
        }
        _ => bail!("crates.io sparse index returned HTTP {code} for {name}"),
    }
}

fn visit_dependencies(v: &toml::Value, target: Option<&str>, out: &mut Vec<Value>) -> Result<()> {
    for (table, kind) in [
        ("dependencies", "normal"),
        ("build-dependencies", "build"),
        ("dev-dependencies", "dev"),
    ] {
        if let Some(ds) = v.get(table).and_then(toml::Value::as_table) {
            for (alias, d) in ds {
                let (req, features, optional, default_features, package) =
                    if let Some(s) = d.as_str() {
                        (s, vec![], false, true, None)
                    } else {
                        let t = d.as_table().with_context(|| {
                            format!("dependency {alias} is not a string or table")
                        })?;
                        (
                            t.get("version")
                                .and_then(toml::Value::as_str)
                                .unwrap_or("*"),
                            t.get("features")
                                .and_then(toml::Value::as_array)
                                .map(|a| a.iter().filter_map(toml::Value::as_str).collect())
                                .unwrap_or_default(),
                            t.get("optional")
                                .and_then(toml::Value::as_bool)
                                .unwrap_or(false),
                            t.get("default-features")
                                .and_then(toml::Value::as_bool)
                                .unwrap_or(true),
                            t.get("package").and_then(toml::Value::as_str),
                        )
                    };
                out.push(json!({"name":alias,"req":req,"features":features,"optional":optional,"default_features":default_features,"target":target,"kind":kind,"registry":null,"package":package}));
            }
        }
    }
    if target.is_none()
        && let Some(ts) = v.get("target").and_then(toml::Value::as_table)
    {
        for (name, tables) in ts {
            visit_dependencies(tables, Some(name), out)?;
        }
    }
    Ok(())
}

fn index_relative_path(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    match n.len() {
        1 => format!("1/{n}"),
        2 => format!("2/{n}"),
        3 => format!("3/{}/{n}", &n[..1]),
        _ => format!("{}/{}/{n}", &n[..2], &n[2..4]),
    }
}

fn index_path(registry: &Path, name: &str) -> PathBuf {
    registry.join("index").join(index_relative_path(name))
}

fn insert_archive(registry: &Path, archive: &Path) -> Result<()> {
    let bytes = fs::read(archive)?;
    let checksum = format!("{:x}", Sha256::digest(&bytes));
    let archive_stem = archive
        .file_stem()
        .context("package archive has no filename stem")?
        .to_string_lossy();
    let output = Command::new("tar")
        .args(["xOzf"])
        .arg(archive)
        .arg(format!("{archive_stem}/Cargo.toml"))
        .output()?;
    ensure!(
        output.status.success(),
        "cannot read manifest from {}",
        archive.display()
    );
    let v: toml::Value = toml::from_str(&String::from_utf8(output.stdout)?)?;
    let package = &v["package"];
    let name = package["name"]
        .as_str()
        .context("normalized archive manifest has no package name")?;
    let vers = package["version"]
        .as_str()
        .context("normalized archive manifest has no package version")?;
    fs::copy(archive, registry.join(format!("{name}-{vers}.crate")))?;
    let mut deps = Vec::new();
    visit_dependencies(&v, None, &mut deps)?;
    let entry = json!({"name":name,"vers":vers,"deps":deps,"cksum":checksum,"features":v.get("features").cloned().unwrap_or(toml::Value::Table(Default::default())),"yanked":false,"links":package.get("links").and_then(toml::Value::as_str)});
    replace_index_entry(&index_path(registry, name), vers, &entry)
}

fn replace_index_entry(path: &Path, vers: &str, entry: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?
    }
    let mut lines = if path.exists() {
        fs::read_to_string(path)?
            .lines()
            .map(str::to_owned)
            .collect()
    } else {
        Vec::new()
    };
    lines.retain(|line| {
        serde_json::from_str::<Value>(line)
            .ok()
            .and_then(|x| x["vers"].as_str().map(str::to_owned))
            .as_deref()
            != Some(vers)
    });
    lines.push(serde_json::to_string(&entry)?);
    lines.sort_by_key(|line| {
        serde_json::from_str::<Value>(line)
            .ok()
            .and_then(|x| x["vers"].as_str().map(str::to_owned))
    });
    fs::write(path, format!("{}\n", lines.join("\n")))?;
    Ok(())
}

fn create_consumer(path: &Path, deps: &str, source: &str) -> Result<()> {
    fs::create_dir_all(path.join("src"))?;
    fs::write(
        path.join("Cargo.toml"),
        format!(
            "[package]\nname='consumer'\nversion='0.0.0'\nedition='2024'\n\n[dependencies]\n{deps}\n\n[workspace]\n"
        ),
    )?;
    fs::write(path.join("src/main.rs"), source)?;
    Ok(())
}
fn cargo_consumer(path: &Path, home: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let o = Command::new("cargo")
        .args(args)
        .current_dir(path)
        .env("CARGO_HOME", home)
        .env("CARGO_TARGET_DIR", path.join("target"))
        .output()?;
    ensure!(
        o.status.success(),
        "consumer cargo {} failed:\n{}\n{}",
        args[0],
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    Ok(o.stdout)
}

fn install_registry_mcp(root: &Path, home: &Path, version: &str) -> Result<()> {
    fs::create_dir_all(root)?;
    let output = Command::new("cargo")
        .args([
            "install",
            "--offline",
            "--locked",
            "--version",
            &format!("={version}"),
            "--root",
        ])
        .arg(root)
        .arg("gpui-box-mcp")
        .env("CARGO_HOME", home)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()?;
    ensure!(
        output.status.success(),
        "registry-only cargo install gpui-box-mcp failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let binary = root.join("bin").join(if cfg!(windows) {
        "gpui-box-mcp.exe"
    } else {
        "gpui-box-mcp"
    });
    ensure!(
        binary.is_file(),
        "registry-only cargo install did not produce {}",
        binary.display()
    );
    let output = Command::new(&binary).arg("--help").output()?;
    ensure!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("gpui-box-mcp"),
        "registry-only gpui-box-mcp --help failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&binary).arg("--version").output()?;
    ensure!(
        output.status.success()
            && String::from_utf8_lossy(&output.stdout).trim() == format!("gpui-box-mcp {version}"),
        "registry-only gpui-box-mcp --version failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn check_consumer_metadata(meta: &Value, root: &Path) -> Result<()> {
    let packages = meta["packages"]
        .as_array()
        .context("consumer Cargo metadata has no packages array")?;
    check_single_gpui_library(packages, "consumer")?;
    for p in packages {
        let source = p["source"].as_str().unwrap_or("");
        ensure!(
            !source.starts_with("git+"),
            "consumer contains git package {}",
            p["name"]
        );
        let manifest = p["manifest_path"]
            .as_str()
            .context("consumer Cargo metadata package has no manifest_path")?;
        let manifest = Path::new(manifest);
        let generated = root.join("target/package-check");
        ensure!(
            !manifest.starts_with(root) || manifest.starts_with(generated),
            "consumer leaked workspace path {}",
            p["manifest_path"]
        );
    }
    Ok(())
}

fn check_single_gpui_library(packages: &[Value], graph: &str) -> Result<()> {
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
        "{graph} must resolve exactly one lib gpui owned by gpui-box, found {owners:?}"
    );
    Ok(())
}

fn check_normalized(text: &str, p: &Package, packages: &BTreeMap<String, Package>) -> Result<()> {
    let v: toml::Value = toml::from_str(text)?;
    let package = &v["package"];
    ensure!(
        package["name"].as_str() == Some(&p.name)
            && package["version"].as_str() == Some(&p.version)
            && package["license"].as_str() == Some(&p.license),
        "normalized identity differs for {}",
        p.name
    );
    for table in ["dependencies", "build-dependencies", "dev-dependencies"] {
        if let Some(ds) = v.get(table).and_then(|x| x.as_table()) {
            for (n, d) in ds {
                ensure!(
                    d.get("path").is_none() && d.get("git").is_none(),
                    "normalized dependency {n} retains path/git"
                );
                if table == "dev-dependencies" {
                    let package = d.get("package").and_then(toml::Value::as_str).unwrap_or(n);
                    ensure!(
                        !packages.contains_key(package),
                        "normalized {} retains internal dev-dependency {package}; make it path-only so crates.io can bootstrap the cohort",
                        p.name
                    );
                }
            }
        }
    }
    if let Some(targets) = v.get("target").and_then(toml::Value::as_table) {
        for tables in targets.values() {
            for table in ["dependencies", "build-dependencies", "dev-dependencies"] {
                if let Some(ds) = tables.get(table).and_then(toml::Value::as_table) {
                    for (n, d) in ds {
                        ensure!(
                            d.get("path").is_none() && d.get("git").is_none(),
                            "normalized target dependency {n} retains path/git"
                        );
                        if table == "dev-dependencies" {
                            let package =
                                d.get("package").and_then(toml::Value::as_str).unwrap_or(n);
                            ensure!(
                                !packages.contains_key(package),
                                "normalized {} retains internal target dev-dependency {package}; make it path-only so crates.io can bootstrap the cohort",
                                p.name
                            );
                        }
                    }
                }
            }
        }
    }
    ensure!(
        package["repository"].as_str().is_some(),
        "{} lacks repository metadata",
        p.name
    );
    if p.name == "gpui-box-mcp" {
        ensure!(
            v.get("bin")
                .and_then(toml::Value::as_array)
                .is_some_and(|bins| bins
                    .iter()
                    .any(|bin| bin["name"].as_str() == Some("gpui-box-mcp"))),
            "gpui-box-mcp normalized manifest lacks its installable binary target"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalized_rejects_path() {
        let p = Package {
            manifest: "x".into(),
            name: "x".into(),
            lib: None,
            cohort: "framework".into(),
            version: "0.1.0".into(),
            license: "MIT".into(),
            publish: true,
            layer: 0,
        };
        let packages = BTreeMap::from([(p.name.clone(), p.clone())]);
        assert!(
            check_normalized(
                "[package]\nname='x'\nversion='0.1.0'\nlicense='MIT'\n[dependencies]\ny={path='y'}",
                &p,
                &packages
            )
            .is_err()
        )
    }

    #[test]
    fn normalized_rejects_internal_dev_dependencies() {
        let p = Package {
            manifest: "x".into(),
            name: "x".into(),
            lib: None,
            cohort: "framework".into(),
            version: "0.1.0".into(),
            license: "MIT".into(),
            publish: true,
            layer: 0,
        };
        let packages = BTreeMap::from([(p.name.clone(), p.clone())]);
        assert!(
            check_normalized(
                "[package]\nname='x'\nversion='0.1.0'\nlicense='MIT'\nrepository='https://example.com'\n[dev-dependencies]\nself-test={package='x',version='0.1.0'}",
                &p,
                &packages
            )
            .is_err()
        )
    }

    #[test]
    fn canonical_index_paths() {
        let root = Path::new("registry");
        assert_eq!(index_path(root, "A"), root.join("index/1/a"));
        assert_eq!(index_path(root, "Ab"), root.join("index/2/ab"));
        assert_eq!(index_path(root, "Abc"), root.join("index/3/a/abc"));
        assert_eq!(index_path(root, "Serde"), root.join("index/se/rd/serde"));
    }

    #[test]
    fn remote_urls_reject_injection_and_use_the_canonical_sparse_path() -> Result<()> {
        assert!(crate_download_url("../../bad", "0.1.0").is_err());
        assert!(crate_download_url("good", "0.1.0?bad").is_err());
        validate_remote_coordinate("gpui-box", "0.1.0-alpha.1+build")?;
        assert_eq!(index_relative_path("gpui-box"), "gp/ui/gpui-box");
        Ok(())
    }

    #[test]
    fn registry_dependencies_include_alias_target_and_kinds() -> Result<()> {
        let value: toml::Value = toml::from_str(
            "[dependencies]\nalias={package='real',version='^1',features=['x'],optional=true,default-features=false}\n[build-dependencies]\nb='2'\n[target.'cfg(unix)'.dev-dependencies]\nd={version='3'}",
        )?;
        let mut dependencies = Vec::new();
        visit_dependencies(&value, None, &mut dependencies)?;
        assert_eq!(dependencies.len(), 3);
        assert!(dependencies.iter().any(|d| d["name"] == "alias"
            && d["package"] == "real"
            && d["optional"] == true
            && d["default_features"] == false));
        assert!(
            dependencies
                .iter()
                .any(|d| d["name"] == "b" && d["kind"] == "build")
        );
        assert!(
            dependencies
                .iter()
                .any(|d| d["name"] == "d" && d["kind"] == "dev" && d["target"] == "cfg(unix)")
        );
        Ok(())
    }

    #[test]
    fn metadata_requires_one_registry_gpui() {
        let root = Path::new("/workspace");
        let valid = json!({"packages":[{"name":"gpui-box","source":"registry+file:///registry","manifest_path":"/cargo/home/registry/gpui/Cargo.toml","targets":[{"name":"gpui","kind":["lib"]}]}]});
        assert!(check_consumer_metadata(&valid, root).is_ok());
        let duplicate = json!({"packages":[
            {"name":"gpui-box","source":"registry+x","manifest_path":"/x","targets":[{"name":"gpui","kind":["lib"]}]},
            {"name":"impostor","source":"registry+x","manifest_path":"/y","targets":[{"name":"gpui","kind":["lib"]}]}
        ]});
        assert!(check_consumer_metadata(&duplicate, root).is_err());
    }

    #[test]
    fn publication_edges_include_versioned_but_not_path_only_dev_dependencies() -> Result<()> {
        assert!(!dependency_is_published(
            &json!({"kind":"dev","name":"local","path":"/workspace/local","req":"*"})
        )?);
        assert!(dependency_is_published(
            &json!({"kind":"dev","name":"published","path":"/workspace/published","req":"^0.1.0"})
        )?);
        assert!(dependency_is_published(
            &json!({"kind":"dev","name":"external","path":null,"req":"*"})
        )?);
        assert!(dependency_is_published(
            &json!({"kind":"build","name":"build","path":"/workspace/build","req":"*"})
        )?);
        Ok(())
    }

    #[test]
    fn index_replacement_retains_versions_and_updates_checksum() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("gpui-box-index-{}", std::process::id()));
        let path = dir.join("index/x");
        replace_index_entry(&path, "1.0.0", &json!({"vers":"1.0.0","cksum":"old"}))?;
        replace_index_entry(&path, "2.0.0", &json!({"vers":"2.0.0","cksum":"two"}))?;
        replace_index_entry(&path, "1.0.0", &json!({"vers":"1.0.0","cksum":"new"}))?;
        let lines: Vec<Value> = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str)
            .collect::<serde_json::Result<_>>()?;
        assert_eq!(lines.len(), 2);
        assert!(
            lines
                .iter()
                .any(|line| line["vers"] == "1.0.0" && line["cksum"] == "new")
        );
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn archive_legal_is_per_crate_and_assets_are_strict() {
        assert!(check_archive_legal("gpui-box", "framework", "x/license-apache").is_ok());
        assert!(check_archive_legal("gpui-box", "framework", "x/readme").is_err());
        assert!(check_archive_legal("gpui-box-kit", "kit", "x/license-mit").is_ok());
        assert!(
            check_archive_legal(
                "gpui-box-kit-assets",
                "kit",
                "x/license-mit\nx/third_party_notices\nx/ofl\nx/cc-by"
            )
            .is_ok()
        );
        assert!(
            check_archive_legal(
                "gpui-box-kit-assets",
                "kit",
                "x/license-mit\nx/third_party_notices\nx/ofl"
            )
            .is_err()
        );
    }

    #[test]
    fn download_url_rejects_path_and_query_injection() -> Result<()> {
        assert_eq!(
            crate_download_url("gpui-box", "0.1.0")?,
            "https://static.crates.io/crates/gpui-box/gpui-box-0.1.0.crate"
        );
        assert!(crate_download_url("../gpui", "0.1.0").is_err());
        assert!(crate_download_url("gpui", "0.1.0?x").is_err());
        Ok(())
    }

    #[test]
    fn version_api_is_the_authority_for_release_presence() -> Result<()> {
        assert_eq!(
            crate_version_url("gpui-box", "0.1.0")?,
            "https://crates.io/api/v1/crates/gpui-box/0.1.0"
        );
        assert!(version_exists_from_status("200", "gpui-box", "0.1.0")?);
        assert!(!version_exists_from_status("404", "gpui-box", "0.1.0")?);
        assert!(version_exists_from_status("403", "gpui-box", "0.1.0").is_err());
        Ok(())
    }

    #[test]
    fn only_the_new_crate_rate_limit_is_retryable() {
        assert!(publish_was_new_crate_rate_limited(
            b"",
            b"status 429 Too Many Requests): You have published too many new crates in a short period of time. Please try again later."
        ));
        assert!(!publish_was_new_crate_rate_limited(
            b"",
            b"status 429 Too Many Requests): generic throttle"
        ));
        assert!(!publish_was_new_crate_rate_limited(
            b"",
            b"status 500: You have published too many new crates in a short period of time"
        ));
    }

    #[test]
    fn publish_requires_exact_execute_argument_before_other_checks() {
        assert!(publish(Path::new("."), &[]).is_err());
        assert!(publish(Path::new("."), &["--execute".into(), "extra".into()]).is_err());
    }
}
