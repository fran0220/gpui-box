//! Offscreen review uses the same native Host and the real process supervisor.
use super::*;
use gpui::{
    HeadlessAppContext, InputEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    point,
};
use std::{path::Path, sync::Arc, time::Instant};

fn coordinates(value: &str, count: usize) -> Result<Vec<f32>> {
    let values = value
        .split(',')
        .map(str::parse)
        .collect::<std::result::Result<Vec<f32>, _>>()?;
    ensure!(
        values.len() == count,
        "native input needs {count} comma-separated coordinates"
    );
    ensure!(
        values.iter().enumerate().all(|(i, value)| value.is_finite()
            && *value >= 0.0
            && *value < if i % 2 == 0 { 980.0 } else { 760.0 }),
        "native input coordinates must be inside the 980x760 viewport"
    );
    Ok(values)
}

fn pump(cx: &mut HeadlessAppContext, handle: gpui::WindowHandle<Host>) -> Result<bool> {
    let mut received = false;
    cx.update(|cx| {
        handle.update(cx, |host, window, cx| {
            while let Ok(frame) = host.bridge.incoming.try_recv() {
                host.frame = Some(frame?);
                received = true;
                cx.notify();
            }
            if host
                .frame
                .as_ref()
                .is_some_and(|frame| frame.revision == host.rendered_revision.get())
            {
                while let Ok(request) = host.bridge.requests.try_recv() {
                    host.handle_request(request, window, cx);
                }
            }
            Ok::<_, anyhow::Error>(())
        })
    })??;
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| window.draw(cx).clear(cx))?;
    Ok(received)
}

fn contains_text(node: &Node, expected: &str) -> bool {
    node.text == expected
        || node
            .children
            .iter()
            .chain(node.slots.values().flatten())
            .any(|child| contains_text(child, expected))
}

fn mounted_app(node: &Node) -> bool {
    (node.instance != 0 && node.id.starts_with("app."))
        || node
            .children
            .iter()
            .chain(node.slots.values().flatten())
            .any(mounted_app)
}

pub(super) fn run(bridge: Bridge, path: &str) -> Result<()> {
    let mut frame = None;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let next = match bridge.incoming.recv_timeout(Duration::from_millis(100)) {
            Ok(next) => next?,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(error) => return Err(error.into()),
        };
        if mounted_app(&next.tree) {
            frame = Some(next);
            break;
        }
    }
    ensure!(
        frame.is_some(),
        "runtime did not mount an app within capture deadline"
    );
    let text_system = gpui_platform::test_text_system("Geist");
    review_fonts::register_emoji_review_font(text_system.as_ref())?;
    let mut cx = HeadlessAppContext::with_platform(
        text_system,
        Arc::new(gpui_kit::assets::Assets),
        gpui_platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::install(cx);
        cx.set_reduce_motion(true);
    });
    let _diagnostics = cx.update(|cx| SemanticCoordinator::global(cx).arm());
    let handle = cx.open_window(size(px(980.0), px(760.0)), |window, cx| {
        cx.new(|cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            let clipboard = clipboard::Policy::install(bridge.outgoing.clone(), cx);
            let resource_store = resources::Resources::install(cx);
            cx.on_release(|host: &mut Host, cx| {
                host.clipboard.release(cx);
                host.resource_store.clear(cx);
            })
            .detach();
            Host {
                bridge,
                frame,
                rendered_revision: Default::default(),
                focus,
                error: None,
                kit: Default::default(),
                clipboard,
                resource_store,
                references: references::Registry::new(),
            }
        })
    })?;
    let window = handle.into();
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))?;
    }
    if let Ok(expected) = std::env::var("GPUI_CAPTURE_READY_TEXT") {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            pump(&mut cx, handle)?;
            let ready = cx.update(|cx| {
                handle.update(cx, |host, _, _| {
                    host.frame
                        .as_ref()
                        .is_some_and(|frame| contains_text(&frame.tree, &expected))
                })
            })?;
            if ready {
                break;
            }
            ensure!(
                Instant::now() < deadline,
                "capture did not reach expected ready text"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    cx.capture_screenshot(window)?.save(Path::new(path))?;
    // Optional actual native mouse event, useful for both template state and
    // host-owned permission controls. Coordinates are reviewed from the image.
    let click = std::env::var("GPUI_CAPTURE_CLICK").ok();
    let drag = std::env::var("GPUI_CAPTURE_DRAG").ok();
    ensure!(
        click.is_none() || drag.is_none(),
        "choose either native click or drag"
    );
    if let Some(input) = drag.as_ref().or(click.as_ref()) {
        let values = coordinates(input, if drag.is_some() { 4 } else { 2 })?;
        let at = point(px(values[0]), px(values[1]));
        let end = if drag.is_some() {
            ensure!(
                (values[2] - values[0]).hypot(values[3] - values[1]) >= 16.0,
                "native drag must travel at least 16 pixels"
            );
            point(px(values[2]), px(values[3]))
        } else {
            at
        };
        cx.update_window(window, |_, window, cx| {
            window.dispatch_event(
                MouseDownEvent {
                    position: at,
                    modifiers: gpui::Modifiers::none(),
                    button: MouseButton::Left,
                    click_count: 1,
                    first_mouse: false,
                }
                .to_platform_input(),
                cx,
            );
        })?;
        if drag.is_some() {
            // Fixed work bound. Draw after each held-button move so threshold
            // activation, hit testing and the native drag preview all run.
            for step in 1..=16 {
                let position = at + (end - at) * (step as f32 / 16.0);
                cx.update_window(window, |_, window, cx| {
                    window.dispatch_event(
                        MouseMoveEvent {
                            position,
                            pressed_button: Some(MouseButton::Left),
                            modifiers: gpui::Modifiers::none(),
                        }
                        .to_platform_input(),
                        cx,
                    );
                })?;
                // Controlled props may round-trip through the worker while
                // dragging. Poll twice per step, with 10ms between polls.
                for _ in 0..2 {
                    std::thread::sleep(Duration::from_millis(10));
                    pump(&mut cx, handle)?;
                }
            }
            if std::env::var("GPUI_CAPTURE_DRAG_PRE_RELEASE").as_deref() == Ok("1") {
                cx.capture_screenshot(window)?
                    .save(Path::new(&format!("{path}.drag.png")))?;
            }
        }
        cx.update_window(window, |_, window, cx| {
            window.dispatch_event(
                MouseUpEvent {
                    position: end,
                    modifiers: gpui::Modifiers::none(),
                    button: MouseButton::Left,
                    click_count: 1,
                }
                .to_platform_input(),
                cx,
            );
        })?;
        // Keep the native request/response pump alive while an isolated callback
        // awaits a native query, rather than sleeping with requests queued.
        let mut received = false;
        for _ in 0..50 {
            received |= pump(&mut cx, handle)?;
            std::thread::sleep(Duration::from_millis(10));
        }
        cx.update(|cx| {
            handle.update(cx, |host, _, cx| {
                let mut changed = received;
                while let Ok(next) = host.bridge.incoming.try_recv() {
                    host.frame = Some(next?);
                    changed = true;
                }
                ensure!(
                    drag.is_some() || changed,
                    "native click produced no runtime frame"
                );
                if let Ok(expected) = std::env::var("GPUI_CAPTURE_EXPECT") {
                    ensure!(
                        host.frame
                            .as_ref()
                            .is_some_and(|frame| contains_text(&frame.tree, &expected)),
                        "native input did not produce expected text"
                    );
                    println!("Native input verified: {expected}");
                }
                cx.notify();
                Ok::<_, anyhow::Error>(())
            })
        })??;
        for _ in 0..3 {
            cx.run_until_parked();
            cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))?;
        }
        cx.capture_screenshot(window)?
            .save(Path::new(&format!("{path}.after.png")))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_column_is_not_a_mounted_app_and_ready_text_reaches_slots() {
        let loading: Node = serde_json::from_value(json!({"kind":"column","id":"host.root","children":[{"kind":"text","id":"host.status","text":"Loading app…"}]})).expect("loading frame");
        assert!(!mounted_app(&loading));
        let ready: Node = serde_json::from_value(json!({"kind":"column","id":"host.root","children":[{"kind":"kit","component":"Accordion","id":"app.section.g1.m1","instance":1,"slots":{"body":[{"kind":"text","id":"app.ready.g1.m2","instance":1,"text":"Fixture ready"}]}}]})).expect("mounted fixture");
        assert!(mounted_app(&ready));
        assert!(contains_text(&ready, "Fixture ready"));
        assert!(!contains_text(&ready, "Missing fixture"));
    }

    #[test]
    fn native_input_coordinates_are_finite_bounded_and_exact_arity() {
        assert_eq!(
            coordinates("0,759,979,1", 4).expect("viewport edges"),
            vec![0.0, 759.0, 979.0, 1.0]
        );
        for input in ["NaN,2", "1,inf", "-1,20", "980,20", "20,760", "1,2,3", "1"] {
            assert!(coordinates(input, 2).is_err(), "{input}");
        }
    }
}
