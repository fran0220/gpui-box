//! Real native pointer release through the isolated worker, never a synthetic drop action.
use super::*;
use gpui::{
    HeadlessAppContext, InputEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    point,
};
use gpui_kit::interaction::dnd::DropDecision;
use std::{path::PathBuf, sync::Arc, time::Instant};

fn find(node: &Node, predicate: &impl Fn(&Node) -> bool) -> Option<Node> {
    if predicate(node) {
        return Some(node.clone());
    }
    node.children
        .iter()
        .chain(node.slots.values().flatten())
        .find_map(|child| find(child, predicate))
}

fn wait(
    cx: &mut HeadlessAppContext,
    host: gpui::WindowHandle<Host>,
    predicate: impl Fn(&Node) -> bool,
) -> Result<Node> {
    let until = Instant::now() + Duration::from_secs(12);
    loop {
        capture::pump(cx, host)?;
        if let Some(node) = cx.update(|cx| {
            host.update(cx, |host, _, _| {
                host.frame
                    .as_ref()
                    .and_then(|frame| find(&frame.tree, &predicate))
            })
        })? {
            return Ok(node);
        }
        ensure!(Instant::now() < until, "worker/native frame deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "requires native renderer and real OS-isolated Node worker"]
fn native_pointer_release_resolves_isolated_predicates_without_stale_mutation() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for (component, mode) in [
        ("List", "accept"),
        ("List", "refuse"),
        ("List", "invalid"),
        ("List", "stale"),
        ("List", "removed"),
        ("List", "timeout"),
        ("Tabs", "accept"),
        ("Tree", "accept"),
        ("Tree", "refuse"),
    ] {
        let temp = std::env::temp_dir().join(format!(
            "gpui-drop-e2e-{}-{component}-{mode}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp)?;
        std::fs::write(
            temp.join("app.json"),
            serde_json::to_vec(
                &json!({"schema":1,"id":"deferred-e2e","version":"1.0.0","entry":"main.mjs","permissions":[],"dependencies":{},"contributes":{"commands":[],"keymaps":[],"panels":[]}}),
            )?,
        )?;
        std::fs::write(
            temp.join("main.mjs"),
            include_str!("deferred-fixture/main.mjs"),
        )?;
        std::fs::write(
            temp.join("case.mjs"),
            format!("export const component={component:?}, mode={mode:?};"),
        )?;
        let bridge = Bridge::start(vec![
            root.join("runner.mjs").display().to_string(),
            temp.display().to_string(),
            "--data-dir".into(),
            temp.join("data").display().to_string(),
        ])?;
        let mut cx = HeadlessAppContext::with_platform(
            gpui_platform::test_text_system("Geist"),
            Arc::new(gpui_kit::assets::Assets),
            gpui_platform::current_headless_renderer,
        );
        cx.update(gpui_kit::install);
        let _armed = cx.update(|cx| SemanticCoordinator::global(cx).arm());
        let host = cx.open_window(size(px(980.), px(760.)), |window, cx| {
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
                    frame: None,
                    rendered_revision: Default::default(),
                    error: None,
                    focus,
                    kit: Default::default(),
                    clipboard,
                    resource_store,
                    references: Default::default(),
                    deferred: Default::default(),
                }
            })
        })?;
        let surface = wait(&mut cx, host, |node| {
            node.component.as_deref() == Some(component)
        })?;
        let controller = cx
            .update(|cx| {
                host.update(cx, |host, _, _| {
                    host.deferred
                        .controller(&surface, host.rendered_revision.get())
                })
            })?
            .context("live native controller")?
            .0;
        let snapshot = cx
            .update(|cx| SemanticCoordinator::global(cx).snapshot(host.window_id()))
            .context("native semantics")?;
        let bounds = |suffix: &str| {
            snapshot
                .nodes
                .iter()
                .find(|node| node.id == format!("{}.{suffix}", surface.id))
                .map(|node| node.bounds)
                .with_context(|| {
                    format!(
                        "{component} {suffix}: {:?}",
                        snapshot
                            .nodes
                            .iter()
                            .map(|node| &node.id)
                            .collect::<Vec<_>>()
                    )
                })
        };
        let from = bounds("gamma")?;
        let to = bounds("alpha")?;
        let start = point(
            px(from.x + from.width * 0.5),
            px(from.y + from.height * 0.5),
        );
        let end = point(
            px(to.x + to.width * if component == "Tabs" { 0.2 } else { 0.5 }),
            px(to.y + to.height * 0.2),
        );
        cx.update_window(host.into(), |_, w, c| {
            w.dispatch_event(
                MouseDownEvent {
                    position: start,
                    modifiers: gpui::Modifiers::none(),
                    button: MouseButton::Left,
                    click_count: 1,
                    first_mouse: false,
                }
                .to_platform_input(),
                c,
            );
        })?;
        for step in 1..=16 {
            cx.update_window(host.into(), |_, w, c| {
                w.dispatch_event(
                    MouseMoveEvent {
                        position: if step == 16 {
                            end
                        } else {
                            start + (end - start) * (step as f32 / 16.)
                        },
                        pressed_button: Some(MouseButton::Left),
                        modifiers: gpui::Modifiers::none(),
                    }
                    .to_platform_input(),
                    c,
                );
            })?;
            capture::pump(&mut cx, host)?;
        }
        assert!(
            controller.status().is_none(),
            "{component}/{mode}: never evaluate during hover"
        );
        assert!(
            cx.update(|cx| cx.has_active_drag()),
            "{component}: active native drag before release"
        );
        cx.update_window(host.into(), |_, w, c| {
            w.dispatch_event(
                MouseUpEvent {
                    position: end,
                    modifiers: gpui::Modifiers::none(),
                    button: MouseButton::Left,
                    click_count: 1,
                }
                .to_platform_input(),
                c,
            );
        })?;
        assert!(
            matches!(controller.status(), Some((_, DropDecision::Pending))),
            "{component}/{mode}: native release must become pending: {:?}",
            controller.status()
        );
        if mode == "accept" {
            wait(&mut cx, host, |node| node.text == "commits:1")?;
            wait(&mut cx, host, |node| node.text == "gamma,alpha,beta")?;
        } else {
            let until =
                Instant::now() + Duration::from_millis(if mode == "timeout" { 3800 } else { 550 });
            while Instant::now() < until {
                capture::pump(&mut cx, host)?;
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(
                matches!(controller.status(), Some((_, DropDecision::Refused(_)))),
                "{component}/{mode}: refused or revoked: {:?}",
                controller.status()
            );
            wait(&mut cx, host, |node| node.text == "commits:0")?;
            wait(&mut cx, host, |node| node.text == "alpha,beta,gamma")?;
        }
        if component == "List" && matches!(mode, "accept" | "refuse" | "stale") {
            let artifacts = root.join("../../.amp/in/artifacts");
            std::fs::create_dir_all(&artifacts)?;
            cx.capture_screenshot(host.into())?
                .save(artifacts.join(format!("deferred-{mode}.png")))?;
        }
        println!(
            "isolated native {component}/{mode}: pointer-release pending -> {:?}; caller order verified",
            controller.status().map(|(_, decision)| decision)
        );
        cx.update_window(host.into(), |_, w, _| w.remove_window())?;
        drop(cx);
        std::fs::remove_dir_all(temp)?;
    }
    Ok(())
}
