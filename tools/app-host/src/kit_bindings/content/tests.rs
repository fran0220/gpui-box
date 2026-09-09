use super::*;
use gpui::{ParentElement, Styled, TestAppContext, div, px};
use gpui_kit_testkit::harness::Harness;
use std::rc::Rc;

fn fixtures() -> Vec<Node> {
    serde_json::from_str(include_str!("fixture/nodes.json")).expect("content fixture data")
}

#[gpui::test]
fn code_copy_refusal_keeps_clipboard_and_granted_copy_uses_actual_caller_text(
    cx: &mut TestAppContext,
) {
    let owner = gpui::EffectOwner::new();
    let node = fixtures().remove(1);
    let mut harness = Harness::new(
        cx,
        move |cx| {
            gpui_kit::install(cx);
            gpui_kit::foundation::register_owner_state(owner, cx);
        },
        move |window, cx| {
            gpui::effect_owner(
                owner,
                render(&node, KitSlots::new(), window, cx, Rc::new(|_, _| {})),
            )
            .into_any_element()
        },
    );
    harness.update(|_, cx| {
        cx.set_clipboard_policy(move |candidate, operation| {
            candidate == owner && operation == gpui::ClipboardOperation::Read
        });
    });
    harness.click("review.code.copy");
    assert!(
        harness
            .node("review.code.copy.refusal")
            .expect("truthful copy refusal")
            .invalid
    );
    harness.update(|_, cx| cx.set_clipboard_policy(move |candidate, _| candidate == owner));
    harness.click("review.code.copy");
    assert!(harness.node("review.code.copy.refusal").is_none());
    harness.update(|_, cx| {
        cx.with_effect_owner(Some(owner), |cx| {
            let copied = cx
                .try_read_from_clipboard()
                .expect("authorized clipboard read")
                .expect("copied item");
            assert_eq!(
                copied.text().as_deref(),
                Some("let café = 7;\nprintln!(\"{café}\");")
            );
        })
    });
}

#[gpui::test]
fn document_slot_factory_recurses_without_borrowing_state(cx: &mut TestAppContext) {
    let state = Rc::new(State::default());
    let weak = Rc::downgrade(&state);
    let held = state.clone();
    let builds = Rc::new(std::cell::Cell::new(0));
    let counted = builds.clone();
    let descriptor:Node=serde_json::from_value(json!({"kind":"kit","id":"slotted","component":"AgentDocument","props":{"blocks":[{"id":"code","kind":"code"}],"virtualized":2},"slots":{"code":[{"kind":"kit","id":"nested.code","component":"CodeView","props":{"text":"nested caller text"}}]}})).expect("slotted document");
    super::super::validate_descriptor(&descriptor).expect("valid typed slot");
    let child = descriptor.slots["code"][0].clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let weak = weak.clone();
        let child = child.clone();
        let counted = counted.clone();
        let mut slots = KitSlots::new();
        slots.insert(
            "code".into(),
            Rc::new(move |window, cx| {
                counted.set(counted.get() + 1);
                weak.upgrade().expect("live adapter state").render(
                    &child,
                    KitSlots::new(),
                    window,
                    cx,
                    Rc::new(|_, _| {}),
                )
            }),
        );
        held.render(&descriptor, slots, window, cx, Rc::new(|_, _| {}))
    });
    assert!(harness.node("nested.code").is_some());
    harness.snapshot();
    assert!(
        builds.get() >= 2,
        "slots build fresh native elements on each frame"
    );
}

fn image_store(owner: gpui::EffectOwner, cx: &mut App) -> crate::resources::ResourceStore {
    use base64::Engine as _;
    let mut store = Resources::install(cx);
    store.reconcile(&std::collections::HashMap::from([(owner, owner)]), cx);
    let bytes = [
        32u32.to_le_bytes().as_slice(),
        16u32.to_le_bytes().as_slice(),
        &(0..512)
            .flat_map(|pixel| {
                if pixel % 32 < 11 {
                    [255, 0, 0, 255]
                } else {
                    [0, 64, 255, 255]
                }
            })
            .collect::<Vec<_>>(),
    ]
    .concat();
    store
        .register(
            owner,
            crate::resources::Registration {
                key: "pixels".into(),
                mime: "image/x.gpui-rgba8".into(),
                data: base64::engine::general_purpose::STANDARD.encode(bytes),
            },
        )
        .expect("approved bounded pixels");
    store
}

fn image_node() -> Node {
    serde_json::from_value(json!({"kind":"kit","id":"resource.image","component":"ImageViewer","props":{"frames":[{"id":"pixels","label":"Approved raw image resource","state":"ready","width":32,"height":16,"resource":{"key":"pixels"}}],"height":180}})).expect("resource image node")
}

#[gpui::test]
fn native_image_resources_require_owner_and_refuse_after_revocation(cx: &mut TestAppContext) {
    let owner = gpui::EffectOwner::new();
    let scope = Rc::new(std::cell::Cell::new(Some(owner)));
    let current = scope.clone();
    let node = image_node();
    super::super::validate_descriptor(&node).expect("closed resource descriptor");
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        cx.with_effect_owner(current.get(), |cx| {
            let element = render(&node, KitSlots::new(), window, cx, Rc::new(|_, _| {}));
            if let Some(owner) = current.get() {
                gpui::effect_owner(owner, element).into_any_element()
            } else {
                element
            }
        })
    });
    assert_eq!(
        harness
            .node("resource.image.frame")
            .expect("frame")
            .value
            .as_deref(),
        Some("unavailable")
    );
    let mut store = harness.update(|_, cx| image_store(owner, cx));
    assert_eq!(
        harness
            .node("resource.image.frame")
            .expect("frame")
            .value
            .as_deref(),
        Some("ready")
    );
    scope.set(Some(gpui::EffectOwner::new()));
    assert_eq!(
        harness
            .node("resource.image.frame")
            .expect("foreign frame")
            .value
            .as_deref(),
        Some("unavailable")
    );
    scope.set(None);
    assert_eq!(
        harness
            .node("resource.image.frame")
            .expect("unowned frame")
            .value
            .as_deref(),
        Some("unavailable")
    );
    scope.set(Some(owner));
    harness.update(|_, cx| store.revoke(owner, cx));
    assert_eq!(
        harness
            .node("resource.image.frame")
            .expect("revoked frame")
            .value
            .as_deref(),
        Some("unavailable")
    );
    harness.update(|_, cx| store.clear(cx));
}

#[gpui::test]
fn terminal_retains_partial_ansi_stream_and_releases_owned_emulator(cx: &mut TestAppContext) {
    let state = Rc::new(State::default());
    let held = state.clone();
    let node = Rc::new(RefCell::new(fixtures().remove(9)));
    let source = node.clone();
    node.borrow_mut()
        .props
        .insert("text".into(), json!("\u{1b}[3"));
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let node = source.borrow().clone();
        div()
            .w(px(600.))
            .h(px(120.))
            .child(held.render(&node, KitSlots::new(), window, cx, Rc::new(|_, _| {})))
            .into_any_element()
    });
    harness.snapshot();
    let emulator = state
        .terminals
        .borrow()
        .values()
        .next()
        .expect("retained emulator")
        .clone();
    node.borrow_mut()
        .props
        .insert("text".into(), json!("\u{1b}[32mHello ANSI"));
    harness.snapshot();
    assert!(
        emulator
            .borrow()
            .emulator
            .row_text(0)
            .starts_with("Hello ANSI")
    );
    // Native gesture changes retained selection rather than rebuilding the ANSI fold.
    harness.drag_start("review.terminal");
    harness.drag_to(gpui::point(px(25.), px(12.)));
    harness.drop_here();
    assert!(emulator.borrow().emulator.has_selection());
    harness.snapshot();
    assert!(emulator.borrow().emulator.has_selection());
    node.borrow_mut().props.remove("text");
    node.borrow_mut()
        .props
        .insert("state".into(), json!("error"));
    harness.snapshot();
    assert!(
        emulator
            .borrow()
            .emulator
            .row_text(0)
            .starts_with("Hello ANSI")
    );
    let weak = Rc::downgrade(&emulator);
    drop(emulator);
    harness.remount(|_, _| div().into_any_element());
    harness.update(|_, cx| {
        state.reconcile(
            &serde_json::from_value(json!({"kind":"column","id":"empty"})).expect("empty root"),
            cx,
        )
    });
    assert!(state.terminals.borrow().is_empty());
    assert!(weak.upgrade().is_none(), "adapter-owned emulator released");
}

#[gpui::test]
fn every_content_adapter_renders_native_semantics(cx: &mut TestAppContext) {
    let nodes = fixtures();
    assert_eq!(nodes.len(), COMPONENTS.len());
    for node in nodes {
        super::super::validate_descriptor(&node).expect("closed fixture descriptor");
        let id = node.id.clone();
        let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
            div()
                .w(px(720.))
                .h(px(540.))
                .child(render(
                    &node,
                    KitSlots::new(),
                    window,
                    cx,
                    Rc::new(|_, _| {}),
                ))
                .into_any_element()
        });
        assert!(harness.node(&id).is_some(), "native root missing: {id}");
    }
}

#[gpui::test]
fn markdown_copy_obeys_owner_policy_and_reports_refusal_without_payload(cx: &mut TestAppContext) {
    let owner = gpui::EffectOwner::new();
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let node = fixtures().remove(0);
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let output = output.clone();
        gpui::effect_owner(
            owner,
            render(
                &node,
                KitSlots::new(),
                window,
                cx,
                Rc::new(move |_, event| output.borrow_mut().push(event)),
            ),
        )
        .into_any_element()
    });
    let copy = harness
        .snapshot()
        .nodes
        .iter()
        .find(|node| node.id.contains("copy"))
        .expect("native markdown copy control")
        .id
        .to_string();
    harness.update(|_, cx| cx.set_clipboard_policy(|_, _| false));
    harness.click(&copy);
    let refused = events
        .borrow()
        .iter()
        .find(|event| event["kind"] == "codeCopyRefused")
        .cloned()
        .expect("copy refusal");
    assert_eq!(refused["reason"], "denied");
    assert!(refused.get("text").is_none());
    events.borrow_mut().clear();
    harness.update(move |_, cx| cx.set_clipboard_policy(move |candidate, _| candidate == owner));
    harness.click(&copy);
    // Native Markdown's parser removes the fence's final newline.
    assert!(
        events
            .borrow()
            .iter()
            .any(|event| event["kind"] == "codeCopied" && event["text"] == "let answer = 42;"),
        "events: {:?}",
        events.borrow()
    );
}

#[gpui::test]
fn mounted_queries_stream_reconciliation_and_teardown(cx: &mut TestAppContext) {
    let state = Rc::new(State::default());
    let held = state.clone();
    let nodes = Rc::new(RefCell::new(fixtures()));
    let source = nodes.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let nodes = source.borrow().clone();
        div()
            .w(px(700.))
            .children([1, 2].map(|index| {
                held.render(
                    &nodes[index],
                    KitSlots::new(),
                    window,
                    cx,
                    Rc::new(|_, _| {}),
                )
            }))
            .into_any_element()
    });
    harness.update(|window, cx| {
        assert_eq!(
            state
                .invoke(&nodes.borrow()[1], "text", &json!({}), true, window, cx)
                .expect("mounted code query"),
            json!("let café = 7;\nprintln!(\"{café}\");")
        );
        assert_eq!(
            state
                .invoke(
                    &nodes.borrow()[2],
                    "duplicate_ids",
                    &json!({}),
                    true,
                    window,
                    cx
                )
                .expect("duplicate identity query"),
            json!([])
        );
        assert!(
            state
                .invoke(&nodes.borrow()[2], "work", &json!({}), true, window, cx)
                .expect("native parser work")["parser"]["parser_passes"]
                .as_u64()
                .expect("bounded parser counter")
                > 0
        );
        assert!(
            state
                .invoke(
                    &nodes.borrow()[2],
                    "remeasure_block",
                    &json!({"block":"missing"}),
                    false,
                    window,
                    cx
                )
                .is_err()
        );
        state
            .invoke(
                &nodes.borrow()[2],
                "remeasure_block",
                &json!({"block":"answer"}),
                false,
                window,
                cx,
            )
            .expect("known block remeasurement");
    });
    nodes.borrow_mut()[1].props.insert(
        "lines".into(),
        json!([{"number":41,"text":"updated native value"}]),
    );
    nodes.borrow_mut()[2].props["blocks"][0]["text"] =
        json!("## Streamed answer\n\nFirst verified paragraph. Continued.");
    nodes.borrow_mut()[2].props["blocks"][0]["revision"] = json!(3);
    harness.snapshot();
    harness.update(|window, cx| {
        assert_eq!(
            state
                .invoke(&nodes.borrow()[1], "text", &json!({}), true, window, cx)
                .expect("updated native query"),
            json!("updated native value")
        );
        let empty: Node = serde_json::from_value(json!({"kind":"column","id":"empty"}))
            .expect("empty fixture root");
        state.reconcile(&empty, cx);
        assert!(
            state
                .invoke(&nodes.borrow()[1], "text", &json!({}), true, window, cx)
                .is_err()
        );
    });
}

#[test]
fn capture_native_content_review() {
    let resources = std::env::var("GPUI_RESOURCE_CAPTURE").ok();
    let Some(path) = resources
        .clone()
        .or_else(|| std::env::var("GPUI_CONTENT_CAPTURE").ok())
    else {
        return;
    };
    use gpui::{AppContext, HeadlessAppContext, size};
    let mut cx = HeadlessAppContext::with_platform(
        gpui_platform::test_text_system("Geist"),
        std::sync::Arc::new(gpui_kit::assets::Assets),
        gpui_platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::install(cx);
        cx.set_reduce_motion(true);
    });
    let owner = gpui::EffectOwner::new();
    let mut store = resources
        .as_ref()
        .map(|_| cx.update(|cx| image_store(owner, cx)));
    let nodes = if resources.is_some() {
        vec![image_node(), serde_json::from_value(json!({"kind":"kit","id":"resource.markdown","component":"Markdown","props":{"source":"# Authorized caller image\n\n![Asymmetric red and blue pixels](picture)","images":[{"src":"picture","resource":{"key":"pixels"}}]}})).expect("markdown image descriptor")]
    } else {
        fixtures().into_iter().take(4).collect()
    };
    struct Review(Vec<Node>, gpui::EffectOwner);
    impl gpui::Render for Review {
        fn render(
            &mut self,
            window: &mut Window,
            cx: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            let element =
                cx.with_effect_owner(Some(self.1), |cx| {
                    div()
                        .w_full()
                        .h_full()
                        .flex()
                        .flex_col()
                        .gap(px(16.))
                        .p(px(20.))
                        .children(self.0.iter().map(|node| {
                            render(node, KitSlots::new(), window, cx, Rc::new(|_, _| {}))
                        }))
                });
            gpui::effect_owner(self.1, element)
        }
    }
    let window = cx
        .open_window(size(px(980.), px(1000.)), |_, cx| {
            cx.new(|_| Review(nodes, owner))
        })
        .expect("headless content window")
        .into();
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .expect("native draw");
    }
    cx.capture_screenshot(window)
        .expect("native content capture")
        .save(std::path::Path::new(&path))
        .expect("save content capture");
    if let Some(store) = store.as_mut() {
        let image = cx.capture_screenshot(window).expect("approved pixels");
        assert!(image.pixels().any(|pixel| pixel.0 == [255, 0, 0, 255]));
        assert!(image.pixels().any(|pixel| pixel.0 == [0, 64, 255, 255]));
        cx.update(|cx| store.revoke(owner, cx));
        for _ in 0..3 {
            cx.run_until_parked();
            cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
                .expect("revoked draw");
        }
        let image = cx.capture_screenshot(window).expect("revoked pixels");
        assert!(!image.pixels().any(|pixel| pixel.0 == [255, 0, 0, 255]));
        image
            .save(format!("{path}.revoked.png"))
            .expect("save revoked capture");
    }
}
