//! Motion driven through a real window, where frames and the clock are
//! simulated rather than assumed.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{TestAppContext, div, prelude::*, px};
use gpui_kit::motion::{CubicBezier, MotionSpec, Presence, Transition};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_testkit::harness::Harness;
use gpui_kit_testkit::present;

fn linear(duration_ms: u64) -> MotionSpec {
    MotionSpec::new(duration_ms, CubicBezier::new(0.0, 0.0, 1.0, 1.0))
}

/// Publishes the animated width as a semantic value, so the test reads what the
/// window actually laid out.
fn width_scene(cx: &mut TestAppContext, transition: Rc<RefCell<Transition<f32>>>) -> Harness {
    Harness::new(cx, gpui_kit::install, move |window, cx| {
        let width = transition.borrow_mut().animate(window, cx);
        div()
            .w(px(width))
            .h(px(10.0))
            .semantic_in(cx, NodeSpec::new("panel", Role::Group))
            .into_any_element()
    })
}

#[gpui::test]
fn a_transition_animates_across_frames_and_settles(cx: &mut TestAppContext) {
    let transition = Rc::new(RefCell::new(Transition::new(0.0_f32, linear(200))));
    let mut harness = width_scene(cx, transition.clone());

    assert_eq!(
        harness.bounds("panel").expect("laid out").size.width,
        px(0.0)
    );

    harness.update(|_, cx| {
        transition.borrow_mut().set(100.0);
        cx.refresh_windows();
    });
    harness.advance(Duration::from_millis(100));
    let midpoint = harness.bounds("panel").expect("laid out").size.width;
    assert!(
        midpoint > px(30.0) && midpoint < px(70.0),
        "expected the halfway width, got {midpoint:?}"
    );

    harness.advance(Duration::from_millis(100));
    assert_eq!(
        harness.bounds("panel").expect("laid out").size.width,
        px(100.0)
    );
    assert!(!transition.borrow().is_animating());
}

#[gpui::test]
fn reduced_motion_skips_a_transition_to_its_target(cx: &mut TestAppContext) {
    let transition = Rc::new(RefCell::new(Transition::new(0.0_f32, linear(200))));
    let mut harness = width_scene(cx, transition.clone());
    harness.update(|_, cx| cx.set_reduce_motion(true));

    harness.update(|_, cx| {
        transition.borrow_mut().set(100.0);
        cx.refresh_windows();
    });
    harness.advance(Duration::ZERO);

    assert_eq!(
        harness.bounds("panel").expect("laid out").size.width,
        px(100.0)
    );
}

#[gpui::test]
fn an_exiting_element_stays_in_the_tree_until_its_exit_finishes(cx: &mut TestAppContext) {
    let presence = Rc::new(RefCell::new(Presence::visible(linear(100), linear(100))));
    let scene = presence.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let mut presence = scene.borrow_mut();
        let progress = presence.animate(window, cx);
        let mut root = div();
        if presence.is_rendered() {
            root = root.child(
                div()
                    .w(px(50.0))
                    .h(px(10.0))
                    .opacity(progress)
                    .semantic_in(cx, NodeSpec::new("toast", Role::Status)),
            );
        }
        root.into_any_element()
    });

    assert!(present(&harness.snapshot(), "toast").is_ok());

    harness.update(|_, cx| {
        presence.borrow_mut().hide();
        cx.refresh_windows();
    });
    harness.advance(Duration::from_millis(50));
    assert!(
        present(&harness.snapshot(), "toast").is_ok(),
        "the element must survive its own exit animation"
    );

    harness.advance(Duration::from_millis(50));
    assert!(present(&harness.snapshot(), "toast").is_err());
}

#[gpui::test]
fn reduced_motion_removes_an_exiting_element_immediately(cx: &mut TestAppContext) {
    let presence = Rc::new(RefCell::new(Presence::visible(linear(100), linear(100))));
    let scene = presence.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let mut presence = scene.borrow_mut();
        presence.animate(window, cx);
        let mut root = div();
        if presence.is_rendered() {
            root = root.child(
                div()
                    .w(px(50.0))
                    .h(px(10.0))
                    .semantic_in(cx, NodeSpec::new("toast", Role::Status)),
            );
        }
        root.into_any_element()
    });
    harness.update(|_, cx| cx.set_reduce_motion(true));

    harness.update(|_, cx| {
        presence.borrow_mut().hide();
        cx.refresh_windows();
    });
    harness.advance(Duration::ZERO);
    assert!(present(&harness.snapshot(), "toast").is_err());
}
