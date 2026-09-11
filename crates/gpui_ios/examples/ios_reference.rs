//! Interactive mobile reference app; separate from both native smoke fixtures.
#[cfg(any(target_os = "ios", feature = "platform-check"))]
#[cfg_attr(
    not(target_os = "ios"),
    expect(dead_code, reason = "type-check native entry without linking UIKit")
)]
mod native {
    use anyhow::{Context as _, Result};
    use gpui::{AppLifecyclePhase, Application, Platform, WindowOptions};
    use gpui_box_mobile_reference::{mount, mount_checkpoint, state::Checkpoint};
    use std::{
        borrow::Cow,
        ffi::{CStr, c_char},
        fs,
        io::Write,
        path::{Path, PathBuf},
        rc::Rc,
    };

    unsafe extern "C" {
        fn gpui_ios_copy_application_support_path() -> *mut c_char;
        fn gpui_ios_free_string(text: *mut c_char);
    }

    fn checkpoint_path() -> Result<PathBuf> {
        let pointer = unsafe { gpui_ios_copy_application_support_path() };
        anyhow::ensure!(
            !pointer.is_null(),
            "iOS refused application-support directory"
        );
        let path = unsafe { CStr::from_ptr(pointer) }
            .to_str()
            .map(PathBuf::from);
        unsafe { gpui_ios_free_string(pointer) };
        Ok(path?.join("mobile-reference.json"))
    }

    fn load(path: &Path) -> Result<Option<Checkpoint>> {
        match fs::read(path) {
            Ok(bytes) => Ok(Some(
                serde_json::from_slice(&bytes).context("invalid reference checkpoint")?,
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).context("read reference checkpoint"),
        }
    }

    fn save(path: &Path, checkpoint: &Checkpoint) -> Result<()> {
        let bytes = serde_json::to_vec(checkpoint)?;
        let temporary = path.with_extension("json.next");
        let mut file = fs::File::create(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(temporary, path)?;
        Ok(())
    }

    pub fn run() -> Result<()> {
        let path = checkpoint_path()?;
        let saved = load(&path)?;
        let platform = Rc::new(gpui_ios::IosPlatform::new(
            "Geist",
            gpui_kit::assets::font_bytes()
                .into_iter()
                .map(Cow::Borrowed)
                .collect(),
        )?);
        platform.verify_next_frame(|completion| match completion {
            Ok(()) => println!("IOS_REFERENCE_FRAME_COMPLETED"),
            Err(error) => {
                eprintln!("IOS_REFERENCE_FRAME_FAILED {error:#}");
                std::process::exit(1);
            }
        })?;
        Application::with_platform(platform.clone())
            .with_assets(gpui_kit::assets::Assets)
            .run(move |cx| {
                gpui_kit::install(cx);
                let window = cx
                    .open_window(WindowOptions::default(), move |window, cx| match saved {
                        Some(saved) => {
                            mount_checkpoint(saved, window, cx).unwrap_or_else(|error| {
                                eprintln!("IOS_REFERENCE_RESTORE_REFUSED {error}");
                                std::process::exit(1);
                            })
                        }
                        None => mount(window, cx),
                    })
                    .expect("mount mobile reference");
                let mut async_cx = cx.to_async();
                platform.on_app_lifecycle(Box::new(move |phase| {
                    if phase != AppLifecyclePhase::Background {
                        return;
                    }
                    let result = window
                        .update(&mut async_cx, |app, window, cx| {
                            app.prepare_background(window, cx)
                        })
                        .and_then(|checkpoint| save(&path, &checkpoint));
                    if let Err(error) = result {
                        eprintln!("IOS_REFERENCE_CHECKPOINT_FAILED {error:#}");
                        if let Err(error) = window.update(&mut async_cx, |app, _, cx| {
                            app.state.notice =
                                "Checkpoint save failed; changes were not saved".into();
                            cx.notify();
                        }) {
                            eprintln!("IOS_REFERENCE_NOTICE_FAILED {error:#}");
                        }
                    }
                }));
                println!("IOS_REFERENCE_MOUNTED");
            });
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::{load, save};
        use gpui_box_mobile_reference::state::FixtureState;
        use std::fs;

        #[test]
        fn checkpoint_replacement_preserves_unicode_and_failed_write_keeps_last_value() {
            let directory = tempfile::tempdir().expect("disposable checkpoint directory");
            let path = directory.path().join("mobile-reference.json");
            assert!(load(&path).expect("missing checkpoint").is_none());
            let mut checkpoint = FixtureState::default().checkpoint();
            checkpoint.name = "林😀".into();
            checkpoint.draft = "第一行\nsecond line".into();
            save(&path, &checkpoint).expect("first save");
            let loaded = load(&path)
                .expect("read checkpoint")
                .expect("saved checkpoint");
            assert_eq!(loaded.name, "林😀");
            assert_eq!(loaded.draft, "第一行\nsecond line");
            checkpoint.draft = "replacement".into();
            save(&path, &checkpoint).expect("replace checkpoint");
            fs::create_dir(path.with_extension("json.next")).expect("block temporary writer");
            checkpoint.draft = "must not replace persisted value".into();
            assert!(save(&path, &checkpoint).is_err());
            assert_eq!(
                load(&path)
                    .expect("last checkpoint")
                    .expect("retained")
                    .draft,
                "replacement"
            );
        }

        #[test]
        fn invalid_checkpoint_and_read_failure_are_not_fresh_state() {
            let directory = tempfile::tempdir().expect("disposable checkpoint directory");
            let path = directory.path().join("mobile-reference.json");
            fs::write(&path, b"invalid-json").expect("write invalid checkpoint");
            assert!(load(&path).is_err());
            assert!(load(directory.path()).is_err());
        }

        #[test]
        fn legacy_and_future_checkpoint_versions_are_refused() {
            let directory = tempfile::tempdir().expect("disposable checkpoint directory");
            let path = directory.path().join("mobile-reference.json");
            let mut value = serde_json::to_value(FixtureState::default().checkpoint())
                .expect("checkpoint JSON");
            assert_eq!(value["version"], 1);
            value
                .as_object_mut()
                .expect("checkpoint object")
                .remove("version");
            fs::write(&path, serde_json::to_vec(&value).expect("legacy bytes"))
                .expect("write legacy checkpoint");
            assert!(load(&path).is_err());
            value["version"] = 2.into();
            fs::write(&path, serde_json::to_vec(&value).expect("future bytes"))
                .expect("write future checkpoint");
            let checkpoint = load(&path)
                .expect("valid JSON shape")
                .expect("checkpoint exists");
            assert!(matches!(
                FixtureState::restore(checkpoint),
                Err("Unsupported checkpoint version")
            ));
        }
    }
}

#[cfg(target_os = "ios")]
fn main() {
    if let Err(error) = native::run() {
        eprintln!("IOS_REFERENCE_START_FAILED {error:#}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("The iOS mobile reference requires native UIKit; no desktop substitute.");
    std::process::exit(2);
}
