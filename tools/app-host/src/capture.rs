//! Offscreen review uses the same native Host and the real process supervisor.
use super::*;
use gpui::{HeadlessAppContext, InputEvent, MouseButton, MouseDownEvent, MouseUpEvent, point};
use std::{path::Path, sync::Arc, time::Instant};

pub(super) fn run(bridge: Bridge, path: &str) -> Result<()> {
    let mut frame = None;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let next = bridge.incoming.recv_timeout(Duration::from_secs(1))??;
        let ready = !next.tree.text.contains("Loading");
        frame = Some(next);
        if ready {
            // Drain startup/activation frames before reviewing the retained tree.
            std::thread::sleep(Duration::from_millis(500));
            while let Ok(next) = bridge.incoming.try_recv() {
                frame = Some(next?);
            }
            break;
        }
    }
    ensure!(frame.is_some(), "runtime did not render");
    let mut cx = HeadlessAppContext::with_platform(
        gpui_platform::test_text_system("Geist"),
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
            Host {
                bridge,
                frame,
                rendered_revision: Default::default(),
                focus,
                error: None,
                kit: Default::default(),
                clipboard,
            }
        })
    })?;
    let window = handle.into();
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))?;
    }
    cx.capture_screenshot(window)?.save(Path::new(path))?;
    // Optional actual native mouse event, useful for both template state and
    // host-owned permission controls. Coordinates are reviewed from the image.
    if let Ok(click) = std::env::var("GPUI_CAPTURE_CLICK") {
        let values: Vec<f32> = click
            .split(',')
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()?;
        ensure!(values.len() == 2, "GPUI_CAPTURE_CLICK needs x,y");
        let at = point(px(values[0]), px(values[1]));
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
            window.dispatch_event(
                MouseUpEvent {
                    position: at,
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
                            host.invoke_request(request, window, cx);
                        }
                    }
                    Ok::<_, anyhow::Error>(())
                })
            })??;
            cx.run_until_parked();
            cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))?;
            std::thread::sleep(Duration::from_millis(10));
        }
        cx.update(|cx| {
            handle.update(cx, |host, _, cx| {
                let mut changed = received;
                while let Ok(next) = host.bridge.incoming.try_recv() {
                    host.frame = Some(next?);
                    changed = true;
                }
                ensure!(changed, "native click produced no runtime frame");
                if let Ok(expected) = std::env::var("GPUI_CAPTURE_EXPECT") {
                    fn contains(node: &Node, expected: &str) -> bool {
                        node.text == expected
                            || node
                                .children
                                .iter()
                                .chain(node.slots.values().flatten())
                                .any(|child| contains(child, expected))
                    }
                    ensure!(
                        host.frame
                            .as_ref()
                            .is_some_and(|frame| contains(&frame.tree, &expected)),
                        "native click did not produce expected text"
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
