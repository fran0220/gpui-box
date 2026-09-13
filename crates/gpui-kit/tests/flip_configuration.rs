//! Configured FLIP publishes and picks its displayed geometry.
use gpui::{TestAppContext, div, point, prelude::*, px};
use gpui_kit::motion::{CubicBezier, Flipping, MotionSpec, Spring, flip};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_testkit::harness::Harness;
use std::{cell::Cell, rc::Rc, time::Duration};

#[gpui::test]
fn configured_timing_preserves_geometry_and_input_through_disable_and_retarget(
    cx: &mut TestAppContext,
) {
    let tween = MotionSpec::new(1000, CubicBezier::new(0.0, 0.0, 1.0, 1.0)).with_delay(100);
    let state = Rc::new(Cell::new((0.0, 40.0, true, tween)));
    let hits = Rc::new(Cell::new(0));
    let input = state.clone();
    let clicked = hits.clone();
    let mut h = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let (x, width, enabled, spec) = input.get();
        let clicked = clicked.clone();
        let handle = flip("configured", window, cx);
        div()
            .flex()
            .flex_col()
            .items_start()
            .w(px(400.0))
            .pl(px(x))
            .child(
                div()
                    .w(px(width))
                    .h(px(30.0))
                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                        clicked.set(clicked.get() + 1)
                    })
                    .semantic_in(cx, NodeSpec::new("configured.child", Role::Button))
                    .flip_size(&handle, window, cx)
                    .animate(enabled)
                    .animation(spec),
            )
            .into_any_element()
    });
    let first = h.bounds("configured.child").expect("initial child bounds");
    state.set((100.0, 80.0, true, tween));
    assert_eq!(
        h.bounds("configured.child").expect("updated child bounds"),
        first
    );
    h.advance(Duration::from_millis(100));
    assert_eq!(
        h.bounds("configured.child").expect("delayed child bounds"),
        first,
        "custom delay holds position and size"
    );
    h.advance(Duration::from_millis(400));
    let middle = h
        .bounds("configured.child")
        .expect("intermediate child bounds");
    assert!((f32::from(middle.origin.x) - 40.0).abs() <= 0.5);
    assert!((f32::from(middle.size.width) - 56.0).abs() <= 0.5);
    // The displayed rectangle hits; the settled rectangle does not.
    h.context().simulate_mouse_down(
        point(px(45.0), px(10.0)),
        gpui::MouseButton::Left,
        gpui::Modifiers::none(),
    );
    assert_eq!(hits.get(), 1);
    h.context().simulate_mouse_down(
        point(px(170.0), px(10.0)),
        gpui::MouseButton::Left,
        gpui::Modifiers::none(),
    );
    assert_eq!(hits.get(), 1);
    let fast = MotionSpec::new(200, CubicBezier::new(0.0, 0.0, 1.0, 1.0));
    state.set((100.0, 80.0, true, fast));
    assert_eq!(
        h.bounds("configured.child").expect("retimed child bounds"),
        middle,
        "timing change is continuous"
    );
    h.advance(Duration::from_millis(100));
    let retargeted = h
        .bounds("configured.child")
        .expect("retargeted child bounds");
    assert!((f32::from(retargeted.origin.x) - 70.0).abs() <= 0.5);
    assert!((f32::from(retargeted.size.width) - 68.0).abs() <= 0.5);
    state.set((100.0, 80.0, false, fast));
    let settled = h.bounds("configured.child").expect("settled child bounds");
    assert_eq!(settled.origin.x, px(100.0));
    assert_eq!(settled.size.width, px(80.0));
    state.set((100.0, 80.0, true, tween));
    assert_eq!(
        h.bounds("configured.child")
            .expect("reenabled child bounds"),
        settled,
        "no replay on re-enable"
    );
    let spring = MotionSpec::sprung(Spring::new(400.0, 28.0, 1.0));
    state.set((20.0, 40.0, true, spring));
    assert_eq!(
        h.bounds("configured.child").expect("spring start bounds"),
        settled
    );
    h.advance(Duration::from_millis(20));
    let sprung = h
        .bounds("configured.child")
        .expect("spring intermediate bounds");
    assert!(sprung.origin.x > px(20.0) && sprung.origin.x < px(100.0));
    assert!(sprung.size.width > px(40.0) && sprung.size.width < px(80.0));
    h.update(|_, cx| cx.set_reduce_motion(true));
    let reduced = h.bounds("configured.child").expect("reduced motion bounds");
    assert_eq!(reduced.origin.x, px(20.0));
    assert_eq!(reduced.size.width, px(40.0));
    h.update(|_, cx| cx.set_reduce_motion(false));
    state.set((20.0, 0.0, true, spring));
    for _ in 0..30 {
        h.advance(Duration::from_millis(20));
        h.snapshot();
        h.update(|window, cx| {
            let displayed = flip("configured", window, cx)
                .size()
                .expect("measured FLIP size");
            assert!(displayed.width >= px(0.0));
            assert_eq!(displayed.height, px(30.0));
        });
    }
}
