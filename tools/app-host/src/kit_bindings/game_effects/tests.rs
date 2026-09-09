use super::*;
use gpui::{TestAppContext, div};
use gpui_kit_testkit::harness::Harness;
use std::{cell::RefCell, rc::Rc};

fn fixtures() -> Vec<Node> {
    serde_json::from_str(include_str!("fixture/cases.json")).expect("game fixture data")
}

/// Opt-in real GPU/software-renderer review, separate from mock-platform tests.
#[test]
#[ignore = "requires native headless renderer; writes review frames to GPUI_FAMILY_REVIEW"]
fn native_motion_and_reward_image_refusal_review() {
    use gpui::{
        AppContext as _, Context, HeadlessAppContext, ParentElement as _, Render, Styled as _, px,
        rgb, size,
    };
    use gpui_kit_semantics::SemanticCoordinator;
    struct Review {
        reward: Node,
        particles: Node,
        micro: Node,
    }
    impl Render for Review {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            SemanticCoordinator::global(cx).begin_frame(window);
            div()
                .size_full()
                .bg(rgb(0x161616))
                .text_color(rgb(0xffffff))
                .p(px(16.))
                .flex()
                .flex_col()
                .child(div().h(px(260.)).flex_none().child(render(
                    &self.reward,
                    KitSlots::new(),
                    window,
                    cx,
                    Rc::new(|_, _| {}),
                )))
                .child(div().h(px(100.)).flex_none().child(render(
                    &self.particles,
                    KitSlots::new(),
                    window,
                    cx,
                    Rc::new(|_, _| {}),
                )))
                .child(render(
                    &self.micro,
                    KitSlots::new(),
                    window,
                    cx,
                    Rc::new(|_, _| {}),
                ))
        }
    }
    let path = std::path::PathBuf::from(
        std::env::var("GPUI_FAMILY_REVIEW").expect("review output directory"),
    );
    std::fs::create_dir_all(&path).expect("review directory");
    let mut cx = HeadlessAppContext::with_platform(
        gpui_platform::test_text_system("Geist"),
        std::sync::Arc::new(gpui_kit::assets::Assets),
        gpui_platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::install(cx);
        cx.set_reduce_motion(false);
    });
    let _armed = cx.update(|cx| SemanticCoordinator::global(cx).arm());
    let mut reward = fixtures().remove(3);
    reward.props.get_mut("reward").expect("reward fixture")["items"][0]["image"] =
        json!({"key":"missing-resource"});
    let mut particles = fixtures().remove(4);
    particles.props.remove("sampleAt");
    particles.props.get_mut("plan").expect("particle plan")["presentation"] =
        json!({"kind":"animated"});
    let micro = fixtures().remove(6);
    let handle = cx
        .open_window(size(px(720.), px(540.)), |_, cx| {
            cx.new(|_| Review {
                reward,
                particles,
                micro,
            })
        })
        .expect("review window");
    let window = handle.into();
    let mut first = None;
    let mut changed = false;
    for index in 0..40 {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .expect("native frame");
        let frame = cx.capture_screenshot(window).expect("native screenshot");
        if let Some(first) = &first {
            changed |= first != &frame;
        } else {
            first = Some(frame.clone());
        }
        frame
            .save(path.join(format!("{index:03}.png")))
            .expect("save motion frame");
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(changed, "live native motion must change rendered pixels");
    cx.update(|cx| {
        let snapshot = SemanticCoordinator::global(cx)
            .snapshot(window.window_id())
            .expect("native semantics");
        assert!(
            snapshot
                .nodes
                .iter()
                .any(|node| node.id.as_str() == "reward.item.fixture-item.image-unavailable"),
            "missing image remains an explicit refusal"
        );
    });
}

#[test]
fn charge_bounds_are_checked_before_native_construction() {
    let mut node = fixtures().remove(0);
    for (current, maximum, valid) in [(3, 3, true), (2, 7, true), (4, 3, false), (0, 0, false)] {
        node.props.get_mut("abilities").expect("fixture abilities")[0]["charges"] =
            json!({"current":current,"maximum":maximum});
        assert_eq!(super::super::validate_descriptor(&node).is_ok(), valid);
    }
}

#[gpui::test]
fn objective_party_reward_events_are_requests_not_state_transitions(cx: &mut TestAppContext) {
    for (index, target, action, expected) in [
        (1, "objectives.investigate", "select", "investigate"),
        (2, "party.fixture-member", "selectMember", "fixture-member"),
        (3, "reward.claim", "claim", "fixture-reward"),
    ] {
        let node = fixtures().remove(index);
        let output = Rc::new(RefCell::new(Vec::new()));
        let recorded = output.clone();
        let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
            let recorded = recorded.clone();
            render(
                &node,
                KitSlots::new(),
                window,
                cx,
                Rc::new(move |name, value| recorded.borrow_mut().push((name.to_owned(), value))),
            )
        });
        harness.click(target);
        assert_eq!(&*output.borrow(), &[(action.into(), json!(expected))]);
        if index == 3 {
            assert_eq!(
                harness
                    .node("reward")
                    .expect("reward evidence")
                    .value
                    .as_deref(),
                Some("revealed")
            );
        }
        harness.remount(|_, _| div().into_any_element());
    }
}

#[gpui::test]
fn every_game_fixture_builds_native_semantics(cx: &mut TestAppContext) {
    for node in fixtures() {
        super::super::validate_descriptor(&node).expect("valid fixture descriptor");
        let name = node.component.clone().expect("fixture component");
        let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
            render(&node, KitSlots::new(), window, cx, Rc::new(|_, _| {}))
        });
        // EffectParticles is a decorative native canvas, intentionally not an
        // interaction/semantic target. All other surfaces expose native facts.
        assert_eq!(
            harness.snapshot().nodes.is_empty(),
            name == "EffectParticles",
            "{name}"
        );
        harness.remount(|_, _| div().into_any_element());
    }
}

#[gpui::test]
fn game_events_preserve_identity_and_disabled_native_actions_are_absent(cx: &mut TestAppContext) {
    let output = Rc::new(RefCell::new(Vec::new()));
    let recorded = output.clone();
    let node = fixtures().remove(0);
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let recorded = recorded.clone();
        render(
            &node,
            KitSlots::new(),
            window,
            cx,
            Rc::new(move |name, value| recorded.borrow_mut().push((name.to_owned(), value))),
        )
    });
    harness.click("abilities.inspect");
    harness.click("abilities.execute");
    harness.click("abilities.restore");
    assert_eq!(&*output.borrow(), &[("activate".into(), json!("inspect"))]);
    assert!(
        harness
            .node("abilities.execute")
            .expect("disabled ability")
            .disabled
    );
    assert!(
        harness
            .node("abilities.restore")
            .expect("cooldown ability")
            .disabled
    );
}
