use super::*;
use gpui::{ParentElement, Styled, TestAppContext, div, px};
use gpui_kit_testkit::harness::Harness;
use std::rc::Rc;

fn fixtures() -> Vec<Node> {
    serde_json::from_str(include_str!("fixture/nodes.json")).expect("content fixture data")
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
    let Ok(path) = std::env::var("GPUI_CONTENT_CAPTURE") else {
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
    let nodes = fixtures();
    struct Review(Vec<Node>);
    impl gpui::Render for Review {
        fn render(
            &mut self,
            window: &mut Window,
            cx: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            div()
                .w_full()
                .h_full()
                .flex()
                .flex_col()
                .gap(px(16.))
                .p(px(20.))
                .children(
                    self.0
                        .iter()
                        .map(|node| render(node, KitSlots::new(), window, cx, Rc::new(|_, _| {}))),
                )
        }
    }
    let window = cx
        .open_window(size(px(980.), px(1000.)), |_, cx| {
            cx.new(|_| Review(nodes.into_iter().take(4).collect()))
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
}
