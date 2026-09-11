use super::*;
use jni::{
    objects::{JFloatArray, JIntArray, JString},
    sys::{jboolean, jstring},
};

fn boundary<T: Default>(env: &mut JNIEnv<'_>, f: impl FnOnce(&mut JNIEnv<'_>) -> Result<T>) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(env))) {
        Ok(Ok(value)) => value,
        result => {
            let message = match result {
                Ok(Err(error)) => format!("GPUI Android: {error:#}"),
                _ => "GPUI Android panicked; see native diagnostics".into(),
            };
            if !env.exception_check().unwrap_or(true) {
                let _ = env.throw_new("java/lang/IllegalStateException", message);
            }
            T::default()
        }
    }
}
fn window() -> Option<Rc<window::AndroidWindow>> {
    platform().window.borrow().clone()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeSurface(
    mut env: JNIEnv,
    _: JObject,
    surface: JObject,
    width: i32,
    height: i32,
    scale: f32,
) {
    boundary(&mut env, |env| {
        anyhow::ensure!(
            width > 0 && height > 0 && scale.is_finite() && scale > 0.,
            "invalid Android surface metrics"
        );
        let p = platform();
        let native =
            unsafe { native_window_from_surface(env.get_native_interface(), surface.as_raw()) };
        anyhow::ensure!(!native.is_null(), "ANativeWindow_fromSurface failed");
        let surface = NativeSurface(Arc::new(NativeWindow(native as usize)));
        let drawable = size(DevicePixels(width), DevicePixels(height));
        p.size.set(drawable);
        p.scale.set(scale);
        let logical = size(px(width as f32 / scale), px(height as f32 / scale));
        p.display.bounds.set(Bounds {
            origin: Point::default(),
            size: logical,
        });
        if let Some(w) = window() {
            if w.renderer.borrow().device_lost() {
                w.renderer.borrow_mut().recover(&surface)?;
            }
            w.renderer
                .borrow_mut()
                .replace_surface(&surface, surface_config(drawable))?;
            w.surface.replace(Some(surface.clone()));
            if let Some(mut cb) = w.resize.take() {
                cb(logical, scale);
                w.resize.replace(Some(cb));
            }
            if let Some(mut cb) = w.active.take() {
                cb(p.active.get());
                w.active.replace(Some(cb));
            }
        }
        p.surface.replace(Some(surface));
        p.presentation
            .borrow_mut()
            .attach(width as u32, height as u32);
        let launch = p.launch.take();
        if let Some(launch) = launch {
            launch();
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeDetach(mut env: JNIEnv, _: JObject) {
    boundary(&mut env, |_| {
        platform().presentation.borrow_mut().detach();
        if let Some(w) = window() {
            w.cancel_contacts();
            if let Some(mut cb) = w.active.take() {
                cb(false);
                w.active.replace(Some(cb));
            }
            let result = w.renderer.borrow_mut().unconfigure_surface();
            w.surface.take();
            platform().surface.take();
            result?;
        }
        platform().surface.take();
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeFrame(mut env: JNIEnv, _: JObject) {
    boundary(&mut env, |_| {
        let p = platform();
        p.dispatcher.drain();
        if p.presentation.borrow().can_draw()
            && let Some(w) = window()
            && let Some(mut cb) = w.frame.take()
        {
            let force_render = w.renderer.borrow_mut().needs_redraw();
            cb(RequestFrameOptions {
                require_presentation: true,
                force_render,
            });
            w.frame.replace(Some(cb));
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeDrain(mut env: JNIEnv, _: JObject) {
    boundary(&mut env, |_| {
        platform().dispatcher.drain();
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeLifecycle(
    mut env: JNIEnv,
    _: JObject,
    phase: i32,
) {
    boundary(&mut env, |_| {
        let p = platform();
        let phase = match phase {
            0 => AppLifecyclePhase::Foreground,
            1 => AppLifecyclePhase::Active,
            2 => AppLifecyclePhase::Inactive,
            3 => AppLifecyclePhase::Background,
            _ => bail!("invalid lifecycle phase"),
        };
        let active = phase == AppLifecyclePhase::Active;
        p.active.set(active);
        if active {
            p.presentation.borrow_mut().resume();
        } else {
            p.presentation.borrow_mut().pause();
        }
        if let Some(w) = window() {
            if !active {
                w.cancel_contacts();
            }
            if let Some(mut cb) = w.active.take() {
                cb(active);
                w.active.replace(Some(cb));
            }
        }
        if let Some(mut cb) = p.lifecycle.take() {
            cb(phase);
            p.lifecycle.replace(Some(cb));
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeMemory(mut env: JNIEnv, _: JObject) {
    boundary(&mut env, |_| {
        let p = platform();
        if let Some(mut cb) = p.memory.take() {
            cb();
            p.memory.replace(Some(cb));
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeDestroy(mut env: JNIEnv, _: JObject) {
    boundary(&mut env, |_| {
        let p = platform();
        p.presentation.borrow_mut().detach();
        p.active.set(false);
        let mut result = Ok(());
        if let Some(w) = window() {
            w.cancel_contacts();
            result = w.renderer.borrow_mut().destroy();
            w.surface.take();
            if let Some(cb) = w.close.take() {
                cb();
            }
        }
        if let Some(mut cb) = p.quit.take() {
            cb();
        }
        let application = APPLICATION.with(|app| app.take());
        drop(application);
        PLATFORM.with(|p| p.take());
        result
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeTouch(
    mut env: JNIEnv,
    _: JObject,
    action: i32,
    ids: JIntArray,
    xs: JFloatArray,
    ys: JFloatArray,
    pressures: JFloatArray,
) {
    boundary(&mut env, |env| {
        let Some(w) = window() else {
            return Ok(());
        };
        if action & 255 == 3 {
            w.cancel_contacts();
            return Ok(());
        }
        let count = env.get_array_length(&ids)? as usize;
        anyhow::ensure!(
            env.get_array_length(&xs)? as usize == count
                && env.get_array_length(&ys)? as usize == count
                && env.get_array_length(&pressures)? as usize == count,
            "motion arrays differ in length"
        );
        let mut ids_out = vec![0; count];
        let mut x = vec![0.; count];
        let mut y = vec![0.; count];
        let mut force = vec![0.; count];
        env.get_int_array_region(&ids, 0, &mut ids_out)?;
        env.get_float_array_region(&xs, 0, &mut x)?;
        env.get_float_array_region(&ys, 0, &mut y)?;
        env.get_float_array_region(&pressures, 0, &mut force)?;
        for (i, action) in crate::motion_pointers(action, count) {
            let id = ids_out[i] as u64;
            let position = point(
                px(x[i] / platform().scale.get()),
                px(y[i] / platform().scale.get()),
            );
            let phase = match action {
                crate::PointerAction::Down => TouchPhase::Started,
                crate::PointerAction::Move => TouchPhase::Moved,
                crate::PointerAction::Up => TouchPhase::Ended,
                crate::PointerAction::Cancel => TouchPhase::Cancelled,
            };
            if matches!(phase, TouchPhase::Started | TouchPhase::Moved) {
                w.contacts.borrow_mut().insert(id, position);
            } else {
                w.contacts.borrow_mut().remove(&id);
            }
            w.emit(PlatformInput::Touch(TouchEvent {
                id: TouchId(id),
                phase,
                position,
                force: Some(force[i].clamp(0., 1.)),
                predicted_position: None,
            }));
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeInsets(
    mut env: JNIEnv,
    _: JObject,
    values: JIntArray,
) {
    boundary(&mut env, |env| {
        anyhow::ensure!(
            env.get_array_length(&values)? == 8,
            "insets need two edge groups"
        );
        let mut v = [0; 8];
        env.get_int_array_region(&values, 0, &mut v)?;
        if let Some(w) = window() {
            let s = platform().scale.get();
            let edges = |i| Edges {
                left: px(v[i] as f32 / s),
                top: px(v[i + 1] as f32 / s),
                right: px(v[i + 2] as f32 / s),
                bottom: px(v[i + 3] as f32 / s),
            };
            let insets = WindowInsets {
                safe_area: edges(0),
                ime: edges(4),
            };
            w.insets.replace(insets.clone());
            if let Some(mut cb) = w.insets_changed.take() {
                cb(insets);
                w.insets_changed.replace(Some(cb));
            }
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeBack(
    mut env: JNIEnv,
    _: JObject,
) -> jboolean {
    boundary(&mut env, |_| {
        if let Some(w) = window()
            && w.back_enabled.get()
            && let Some(mut cb) = w.back.take()
        {
            cb();
            w.back.replace(Some(cb));
            return Ok(1);
        }
        Ok(0)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeInputSnapshot(
    mut env: JNIEnv,
    _: JObject,
    generation: i64,
) -> jstring {
    boundary(&mut env, |env| {
        let snapshot = window().filter(|w| generation == 0 || generation == w.input_generation.get()).and_then(|w| w.with_input(|input| {
            let selected = input.native_selection()?;
            let length = input.text_length_utf16()?;
            let text = input.text_for_range(0..length, &mut None)?;
            let marked = input.marked_text_range();
            let options = input.text_input_options()?;
            Some(serde_json::json!({"text": text, "start": selected.anchor.utf16_offset, "end": selected.head.utf16_offset,
                "generation": w.input_generation.get(), "purpose": format!("{:?}", options.purpose), "action": format!("{:?}", options.action),
                "secure": options.secure, "multiline": options.multiline,
                "markedStart": marked.as_ref().map_or(-1, |r| r.start as i64), "markedEnd": marked.as_ref().map_or(-1, |r| r.end as i64)}).to_string())
        })).flatten();
        Ok(match snapshot {
            Some(snapshot) => env.new_string(snapshot)?.into_raw(),
            None => std::ptr::null_mut(),
        })
    })
}

/// Commands operate on the authoritative GPUI UTF-16 document, not a shadow
/// text model: 0 commit, 1 compose, 2 selection, 3 finish, 4 composing region,
/// 5 delete range. Java BaseInputConnection calculates deletion ranges.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeEdit(
    mut env: JNIEnv,
    _: JObject,
    command: i32,
    text: JString,
    start: i32,
    end: i32,
    cursor: i32,
    generation: i64,
) -> jboolean {
    boundary(&mut env, |env| {
        let text: String = if text.is_null() {
            String::new()
        } else {
            env.get_string(&text)?.into()
        };
        let result = window()
            .filter(|w| w.input_generation.get() == generation)
            .and_then(|w| {
                w.with_input(|input| -> Option<()> {
                    let length = input.text_length_utf16()?;
                    let native_selection = input.native_selection()?;
                    let anchor = native_selection.anchor.utf16_offset;
                    let head = native_selection.head.utf16_offset;
                    let selected = anchor.min(head)..anchor.max(head);
                    match command {
                        0 | 1 => {
                            let range = input.marked_text_range().unwrap_or(selected);
                            let count = text.encode_utf16().count();
                            let position = crate::insertion_cursor(
                                range.start,
                                count,
                                cursor,
                                length - range.len() + count,
                            );
                            if command == 1 {
                                input.replace_and_mark_text_in_range(Some(range), &text, None);
                            } else {
                                input.replace_text_in_range(Some(range), &text);
                                input.unmark_text();
                            }
                            input.set_selected_text_range(position..position);
                        }
                        2 | 4 | 5 => {
                            if start < 0 || end < 0 {
                                return None;
                            }
                            let range = (start.min(end) as usize)..(start.max(end) as usize);
                            let document = input.text_for_range(0..length, &mut None)?;
                            let bytes = crate::utf16_range(&document, range.clone())?;
                            match command {
                                2 => {
                                    if !input.set_native_selection(crate::android_selection(
                                        &document, start, end,
                                    )?) {
                                        return None;
                                    }
                                }
                                4 => {
                                    input.replace_and_mark_text_in_range(
                                        Some(range),
                                        &document[bytes],
                                        None,
                                    );
                                    if !input.set_native_selection(native_selection) {
                                        return None;
                                    }
                                }
                                _ => {
                                    let selection = crate::native_selection_after_deletion(
                                        native_selection,
                                        range.clone(),
                                    );
                                    let marked = input.marked_text_range().map(|marked| {
                                        crate::selection_after_deletion(marked, range.clone())
                                    });
                                    input.replace_text_in_range(Some(range), "");
                                    if let Some(marked) = marked.filter(|marked| !marked.is_empty())
                                    {
                                        let text =
                                            input.text_for_range(marked.clone(), &mut None)?;
                                        input.replace_and_mark_text_in_range(
                                            Some(marked),
                                            &text,
                                            None,
                                        );
                                    }
                                    if !input.set_native_selection(selection) {
                                        return None;
                                    }
                                }
                            }
                        }
                        3 => input.unmark_text(),
                        6 => {
                            let action = match start {
                                2 => TextInputAction::Go,
                                3 => TextInputAction::Search,
                                4 => TextInputAction::Send,
                                5 => TextInputAction::Next,
                                6 => TextInputAction::Done,
                                7 => TextInputAction::Previous,
                                _ => TextInputAction::Default,
                            };
                            if !input.perform_text_input_action(action) {
                                return None;
                            }
                        }
                        _ => return None,
                    }
                    Some(())
                })
            })
            .flatten()
            .is_some();
        Ok(result.into())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeAccessibility(
    mut env: JNIEnv,
    _: JObject,
    enabled: jboolean,
) {
    boundary(&mut env, |_| {
        if let Some(w) = window() {
            let callbacks = w.a11y_callbacks.take();
            if let Some(callbacks) = callbacks {
                if enabled != 0 {
                    if let Some(update) = (callbacks.activation)() {
                        w.a11y.borrow_mut().update(update);
                        platform()
                            .java
                            .string("updateAccessibility", &w.a11y.borrow().json());
                    }
                } else {
                    (callbacks.deactivation)();
                }
                w.a11y_callbacks.replace(Some(callbacks));
            }
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeAccessibilityAction(
    mut env: JNIEnv,
    _: JObject,
    id: i32,
    action: i32,
    value: JString,
) -> jboolean {
    boundary(&mut env, |env| {
        let value = if value.is_null() {
            None
        } else {
            Some(env.get_string(&value)?.into())
        };
        if let Some(w) = window() {
            let request = w.a11y.borrow().action(id, action, value);
            if let Some(request) = request
                && let Some(callbacks) = w.a11y_callbacks.take()
            {
                (callbacks.action)(request);
                w.a11y_callbacks.replace(Some(callbacks));
                return Ok(1);
            }
        }
        Ok(0)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativeSettingsChanged(
    mut env: JNIEnv,
    _: JObject,
    thermal: jboolean,
) {
    boundary(&mut env, |env| {
        let p = platform();
        if thermal == 0 {
            let appearance = crate::android_appearance(
                env.call_method(p.java.activity.as_obj(), "uiMode", "()I", &[])?
                    .i()?,
            );
            if p.appearance.replace(appearance) != appearance
                && let Some(w) = window()
                && let Some(mut cb) = w.appearance_changed.take()
            {
                cb();
                w.appearance_changed.replace(Some(cb));
            }
        }
        let callbacks = if thermal != 0 {
            &p.thermal_changed
        } else {
            &p.keyboard_changed
        };
        if let Some(mut cb) = callbacks.take() {
            cb();
            callbacks.replace(Some(cb));
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpui_box_GpuiActivity_nativePresentedFrames(
    mut env: JNIEnv,
    _: JObject,
) -> i64 {
    boundary(&mut env, |_| {
        Ok(window().map_or(0, |w| w.presented_frames.get()))
    })
}
