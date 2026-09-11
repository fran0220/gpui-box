//! Android example-host policy for the shared reference app. Persistence stays
//! in ReferenceActivity; GPUI and Kit never own its checkpoint storage.

use gpui::{Application, AsyncApp, WindowHandle, WindowOptions};
use gpui_box_mobile_reference::{ReferenceApp, mount, mount_checkpoint, state::Checkpoint};
use jni::{
    JNIEnv,
    objects::{JObject, JString},
    sys::jstring,
};
use std::cell::RefCell;

thread_local! {
    static REFERENCE: RefCell<Option<(WindowHandle<ReferenceApp>, AsyncApp)>> = const { RefCell::new(None) };
}

fn boundary<T: Default>(
    env: &mut JNIEnv<'_>,
    operation: impl FnOnce(&mut JNIEnv<'_>) -> anyhow::Result<T>,
) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(env))) {
        Ok(Ok(value)) => value,
        result => {
            let message = match result {
                Ok(Err(error)) => format!("Android reference host: {error:#}"),
                _ => "Android reference host panicked".into(),
            };
            if !env.exception_check().unwrap_or(true) {
                let _ = env.throw_new("java/lang/IllegalStateException", message);
            }
            T::default()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_example_ReferenceActivity_nativeCreate(
    mut env: JNIEnv,
    activity: JObject,
    checkpoint: JString,
) {
    boundary(&mut env, |env| {
        let saved: Option<Checkpoint> = if checkpoint.is_null() {
            None
        } else {
            let text: String = env.get_string(&checkpoint)?.into();
            Some(serde_json::from_str(&text)?)
        };
        REFERENCE.with(|reference| reference.take());
        gpui_android::initialize(env, activity, |platform| {
            Application::with_platform(platform)
                .with_assets(gpui_kit::assets::Assets)
                .run_embedded(move |cx| {
                    gpui_kit::install(cx);
                    let handle = cx
                        .open_window(WindowOptions::default(), move |window, cx| match saved {
                            Some(saved) => mount_checkpoint(saved, window, cx)
                                .expect("reference checkpoint restoration refused"),
                            None => mount(window, cx),
                        })
                        .expect("open Android reference window");
                    let mut app = cx.to_async();
                    handle
                        .update(cx, |_, window, _| {
                            // Keep the fixture open at root so its refusal/root
                            // notice can be reviewed; Home leaves the Activity.
                            window.set_back_enabled(true);
                            window.set_back_handler(move || {
                                let _ = handle.update(&mut app, |reference, window, cx| {
                                    reference.request_back(window, cx);
                                });
                            });
                        })
                        .expect("install reference back policy");
                    REFERENCE.with(|reference| reference.replace(Some((handle, cx.to_async()))));
                })
        })
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_example_ReferenceActivity_nativeCheckpoint(
    mut env: JNIEnv,
    _: JObject,
) -> jstring {
    boundary(&mut env, |env| {
        REFERENCE.with(|reference| {
            let mut reference = reference.borrow_mut();
            let Some((window, cx)) = reference.as_mut() else {
                // Activity can stop before its first Surface opens a window.
                // Null preserves the previously stored checkpoint unchanged.
                return Ok(std::ptr::null_mut());
            };
            let checkpoint =
                window.update(cx, |app, window, cx| app.prepare_background(window, cx))?;
            let json = serde_json::to_string(&checkpoint)?;
            Ok(env.new_string(json)?.into_raw())
        })
    })
}
