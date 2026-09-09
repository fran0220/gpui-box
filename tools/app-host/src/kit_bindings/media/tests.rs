use super::*;
use gpui::{ParentElement, Styled, TestAppContext, div, px};
use gpui_kit_testkit::harness::Harness;
use std::rc::Rc;

fn fixtures() -> Vec<Node> {
    serde_json::from_str(include_str!("fixture/nodes.json")).expect("media fixture data")
}

#[gpui::test]
fn native_media_reports_authority_absence_and_parser_refusal(cx: &mut TestAppContext) {
    for node in fixtures() {
        super::super::validate_descriptor(&node).expect("closed media descriptor");
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
                    Rc::new(|_, _| panic!("unavailable native media emitted success")),
                ))
                .into_any_element()
        });
        let root = harness.node(&id).expect("native media semantic root");
        if id == "review.audio" || id == "review.video" {
            assert_eq!(root.value.as_deref(), Some("no-transport"));
        }
        if id == "review.waveform" {
            assert_eq!(root.value.as_deref(), Some("ready"));
        }
        if id == "review.model" {
            assert_ne!(root.value.as_deref(), Some("ready"));
        }
    }
}

#[test]
fn capture_native_media_review() {
    let Ok(path) = std::env::var("GPUI_MEDIA_CAPTURE") else {
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
        .open_window(size(px(980.), px(1100.)), |_, cx| {
            cx.new(|_| Review(fixtures()))
        })
        .expect("headless media window")
        .into();
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .expect("native draw");
    }
    cx.capture_screenshot(window)
        .expect("native media capture")
        .save(std::path::Path::new(&path))
        .expect("save media capture");
}
