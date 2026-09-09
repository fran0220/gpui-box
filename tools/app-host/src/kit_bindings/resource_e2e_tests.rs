//! Integration-only harness: real isolated worker/supervisor/native Host protocol.
//! Register as a test sibling of content in kit_bindings after runtime integration.
//! No central component registry or production transport is replaced by this test.
use crate::*;
use gpui::{EffectOwner, HeadlessAppContext};
use std::{path::PathBuf, rc::Rc, sync::Arc, time::Instant};

fn find(node: &Node, predicate: &impl Fn(&Node) -> bool) -> Option<Node> {
    if predicate(node) {
        return Some(node.clone());
    }
    node.children
        .iter()
        .chain(node.slots.values().flatten())
        .find_map(|node| find(node, predicate))
}

fn pump(
    cx: &mut HeadlessAppContext,
    host: gpui::WindowHandle<Host>,
    registrations: &mut usize,
) -> Result<()> {
    cx.update(|cx| {
        host.update(cx, |host, window, cx| {
            while let Ok(frame) = host.bridge.incoming.try_recv() {
                host.frame = Some(frame?);
                cx.notify();
            }
            if host
                .frame
                .as_ref()
                .is_some_and(|frame| frame.revision == host.rendered_revision.get())
            {
                while let Ok(request) = host.bridge.requests.try_recv() {
                    if matches!(request, HostRequest::Resource(_)) {
                        *registrations += 1;
                    }
                    host.handle_request(request, window, cx);
                }
            }
            Ok::<_, anyhow::Error>(())
        })
    })??;
    cx.run_until_parked();
    cx.update_window(host.into(), |_, window, cx| window.draw(cx).clear(cx))?;
    Ok(())
}

fn wait(
    cx: &mut HeadlessAppContext,
    host: gpui::WindowHandle<Host>,
    registrations: &mut usize,
    predicate: impl Fn(&Node) -> bool,
) -> Result<Node> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        pump(cx, host, registrations)?;
        let found = cx.update(|cx| {
            host.update(cx, |host, _, _| {
                host.frame
                    .as_ref()
                    .and_then(|frame| find(&frame.tree, &predicate))
            })
        })?;
        if let Some(node) = found {
            return Ok(node);
        }
        ensure!(
            Instant::now() < deadline,
            "isolated worker/native host state deadline exceeded"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn act(cx: &mut HeadlessAppContext, host: gpui::WindowHandle<Host>, node: &Node) -> Result<()> {
    cx.update(|cx| host.update(cx, |host, _, _| {
        host.bridge.outgoing.send(json!({"kind":"event","generation":0,"revision":host.frame.as_ref().context("mounted frame")?.revision,"action":node.action.as_ref().context("enabled native action")?}))?;
        Ok::<_, anyhow::Error>(())
    }))??;
    Ok(())
}

struct Review {
    owner: EffectOwner,
    node: Node,
}
impl Render for Review {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SemanticCoordinator::global(cx).begin_frame(window);
        let content = cx.with_effect_owner(Some(self.owner), |cx| {
            super::content::render(
                &self.node,
                super::KitSlots::new(),
                window,
                cx,
                Rc::new(|_, _| {}),
            )
        });
        div()
            .size_full()
            .p(px(20.))
            .child(gpui::effect_owner(self.owner, content))
    }
}

#[test]
#[ignore = "requires integrated runtime/resource bridge and Linux OS sandbox"]
fn isolated_worker_registers_native_image_and_reload_revokes_it() -> Result<()> {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let data = std::env::temp_dir().join(format!("gpui-resource-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&data)?;
    let bridge = Bridge::start(vec![
        crate_root.join("runner.mjs").display().to_string(),
        crate_root
            .join("src/resource-fixture")
            .display()
            .to_string(),
        "--data-dir".into(),
        data.display().to_string(),
    ])?;
    // No --trust-local switch: runner must launch the real native OS sandbox.
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
            }
        })
    })?;
    let mut registrations = 0;
    let unsafe_action = wait(&mut cx, host, &mut registrations, |node| {
        node.text == "Reject unsafe resource"
    })?;
    act(&mut cx, host, &unsafe_action)?;
    wait(&mut cx, host, &mut registrations, |node| {
        node.text == "Unsafe resource rejected"
    })?;
    assert_eq!(
        registrations, 0,
        "unsafe paths must not reach native registration"
    );

    let register = wait(&mut cx, host, &mut registrations, |node| {
        node.text == "Register fixture pixels"
    })?;
    act(&mut cx, host, &register)?;
    let deny = wait(&mut cx, host, &mut registrations, |node| {
        node.id.ends_with(".resources.deny")
    })?;
    act(&mut cx, host, &deny)?;
    wait(&mut cx, host, &mut registrations, |node| {
        node.text.starts_with("Registration refused:")
    })?;
    assert_eq!(
        registrations, 0,
        "permission denial must not call native resource store"
    );

    let register = wait(&mut cx, host, &mut registrations, |node| {
        node.text == "Register fixture pixels"
    })?;
    act(&mut cx, host, &register)?;
    let allow = wait(&mut cx, host, &mut registrations, |node| {
        node.id.ends_with(".resources.allow")
    })?;
    act(&mut cx, host, &allow)?;
    let ready = wait(&mut cx, host, &mut registrations, |node| {
        node.text == "Ready: pixels"
    })?;
    assert_eq!(
        registrations, 1,
        "one actual native registration acknowledged to worker"
    );
    let owner = cx
        .update(|cx| host.update(cx, |host, _, _| host.clipboard.owner_of(&ready)))?
        .context("worker status mount owner")?;
    let node: Node = serde_json::from_value(
        json!({"kind":"kit","id":"resource.e2e.image","instance":ready.instance,"component":"ImageViewer","props":{"frames":[{"id":"pixels","label":"Isolated worker fixture pixels","state":"ready","width":32,"height":16,"resource":{"key":"pixels"}}],"height":180}}),
    )?;
    super::content::validate_descriptor(&node)?;
    let review = cx.open_window(size(px(700.), px(300.)), |_, cx| {
        cx.new(|_| Review { owner, node })
    })?;
    let draw = |cx: &mut HeadlessAppContext| -> Result<()> {
        cx.update(|cx| review.update(cx, |_, _, cx| cx.notify()))?;
        cx.run_until_parked();
        cx.update_window(review.into(), |_, window, cx| window.draw(cx).clear(cx))?;
        Ok(())
    };
    draw(&mut cx)?;
    let pixels = cx.capture_screenshot(review.into())?;
    assert!(pixels.pixels().any(|pixel| pixel.0 == [255, 0, 0, 255]));
    assert!(pixels.pixels().any(|pixel| pixel.0 == [0, 64, 255, 255]));
    let semantics = cx
        .update(|cx| SemanticCoordinator::global(cx).snapshot(review.window_id()))
        .context("ready viewer semantics")?;
    assert!(semantics.nodes.iter().any(|node| node.role == Role::Image
        && node.visible
        && node.value.as_deref() == Some("ready")));
    let reload = wait(&mut cx, host, &mut registrations, |node| {
        node.text == "Reload app"
    })?;
    act(&mut cx, host, &reload)?;
    wait(&mut cx, host, &mut registrations, |node| {
        node.text == "Awaiting registration" && node.instance != ready.instance
    })?;
    draw(&mut cx)?;
    let revoked = cx.capture_screenshot(review.into())?;
    assert!(!revoked.pixels().any(|pixel| pixel.0 == [255, 0, 0, 255]));
    let semantics = cx
        .update(|cx| SemanticCoordinator::global(cx).snapshot(review.window_id()))
        .context("revoked viewer semantics")?;
    assert!(semantics.nodes.iter().any(|node| node.role == Role::Image
        && node.visible
        && node.value.as_deref() == Some("unavailable")));
    cx.update(|cx| {
        cx.with_effect_owner(Some(owner), |cx| {
            assert!(
                resources::Resources::image(
                    &resources::ResourceRef {
                        key: "pixels".into()
                    },
                    cx
                )
                .is_err()
            )
        })
    });
    if let Ok(directory) = std::env::var("GPUI_RESOURCE_ARTIFACTS") {
        std::fs::create_dir_all(&directory)?;
        pixels.save(format!("{directory}/resources-worker-ready.png"))?;
        revoked.save(format!("{directory}/resources-worker-reloaded.png"))?;
    }
    drop(cx);
    std::fs::remove_dir_all(data)?;
    Ok(())
}
