//! Motion driven through a real window, where frames and the clock are
//! simulated rather than assumed.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{TestAppContext, div, prelude::*, px};
use gpui_kit::motion::{
    CubicBezier, Flipping, MotionSpec, Presence, Transition, flip, tracked_ids,
};
use gpui_kit::prelude::{AnimatedNumber, grouped};
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

/// A column whose two rows swap places, beside a marker that never moves.
///
/// The marker is deliberately not flipped: it is the witness that a slide is
/// painted over the layout rather than inside it.
fn reorder_scene(
    cx: &mut TestAppContext,
    swapped: Rc<Cell<bool>>,
    second: Rc<Cell<bool>>,
) -> Harness {
    Harness::new(cx, gpui_kit::install, move |window, cx| {
        let order: [&str; 2] = if swapped.get() {
            ["b", "a"]
        } else {
            ["a", "b"]
        };
        let mut column = div().flex().flex_col();
        for id in order {
            if id == "b" && !second.get() {
                continue;
            }
            let handle = flip(format!("row.{id}"), cx);
            column = column.child(
                div()
                    .w(px(100.0))
                    .h(px(20.0))
                    .semantic_in(cx, NodeSpec::new(format!("row.{id}"), Role::Group))
                    .flip(&handle, window, cx),
            );
        }
        div()
            .flex()
            .flex_col()
            .child(column)
            .child(
                div()
                    .w(px(100.0))
                    .h(px(20.0))
                    .semantic_in(cx, NodeSpec::new("marker", Role::Group)),
            )
            .into_any_element()
    })
}

#[gpui::test]
fn a_moved_element_starts_offset_and_settles_back_to_zero(cx: &mut TestAppContext) {
    let swapped = Rc::new(Cell::new(false));
    let mut harness = reorder_scene(cx, swapped.clone(), Rc::new(Cell::new(true)));
    let settled = harness.bounds("row.a").expect("laid out").origin;

    harness.update({
        let swapped = swapped.clone();
        move |_, cx| {
            swapped.set(true);
            cx.refresh_windows();
        }
    });

    let offset = harness.update(|_, cx| flip("row.a", cx).offset());
    assert_eq!(
        offset.y,
        px(-20.0),
        "the row starts drawn where it used to be"
    );

    harness.advance(Duration::from_millis(500));
    let offset = harness.update(|_, cx| flip("row.a", cx).offset());
    assert_eq!(offset, gpui::Point::default(), "the slide settles at zero");
    assert_eq!(
        harness.bounds("row.a").expect("laid out").origin.y,
        settled.y + px(20.0),
        "the row ends in its new slot"
    );
}

#[gpui::test]
fn a_slide_does_not_move_a_sibling(cx: &mut TestAppContext) {
    let swapped = Rc::new(Cell::new(false));
    let mut harness = reorder_scene(cx, swapped.clone(), Rc::new(Cell::new(true)));
    let before = harness.bounds("marker").expect("laid out");

    harness.update({
        let swapped = swapped.clone();
        move |_, cx| {
            swapped.set(true);
            cx.refresh_windows();
        }
    });
    assert!(
        harness.update(|_, cx| flip("row.a", cx).is_animating()),
        "the sibling check is only meaningful while a slide is in flight"
    );
    assert_eq!(
        harness.bounds("marker").expect("laid out"),
        before,
        "an offset element must not push anything beside it"
    );
}

#[gpui::test]
fn reduced_motion_puts_a_moved_element_straight_into_its_new_place(cx: &mut TestAppContext) {
    let swapped = Rc::new(Cell::new(false));
    let mut harness = reorder_scene(cx, swapped.clone(), Rc::new(Cell::new(true)));
    harness.update(|_, cx| cx.set_reduce_motion(true));

    harness.update({
        let swapped = swapped.clone();
        move |_, cx| {
            swapped.set(true);
            cx.refresh_windows();
        }
    });

    assert_eq!(
        harness.update(|_, cx| flip("row.a", cx).offset()),
        gpui::Point::default(),
        "reduced motion never offsets the first frame"
    );
}

#[gpui::test]
fn the_flip_global_drops_ids_that_stopped_rendering(cx: &mut TestAppContext) {
    let second = Rc::new(Cell::new(true));
    let mut harness = reorder_scene(cx, Rc::new(Cell::new(false)), second.clone());
    assert!(
        harness
            .update(|_, cx| tracked_ids(cx))
            .contains(&"row.b".into())
    );

    harness.update({
        let second = second.clone();
        move |_, cx| {
            second.set(false);
            cx.refresh_windows();
        }
    });
    harness.frame();
    harness.frame();

    let tracked = harness.update(|_, cx| tracked_ids(cx));
    assert!(
        tracked.contains(&"row.a".into()),
        "a row still on screen keeps its state"
    );
    assert!(
        !tracked.contains(&"row.b".into()),
        "a row that stopped rendering must not be retained forever"
    );
}

#[gpui::test]
fn an_animated_number_publishes_its_target_before_it_finishes_counting(cx: &mut TestAppContext) {
    let total = Rc::new(Cell::new(120.0_f64));
    let scene = total.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _cx| {
        AnimatedNumber::new("run.total", scene.get())
            .format(grouped)
            .into_any_element()
    });
    assert_eq!(
        harness
            .node("run.total")
            .expect("published")
            .value
            .as_deref(),
        Some("120")
    );

    harness.update({
        let total = total.clone();
        move |_, cx| {
            total.set(1204.0);
            cx.refresh_windows();
        }
    });

    assert_eq!(
        harness
            .node("run.total")
            .expect("published")
            .value
            .as_deref(),
        Some("1,204"),
        "a number in flight is not a fact: the target is published at once"
    );
}

#[gpui::test]
fn reduced_motion_shows_an_animated_number_at_its_target(cx: &mut TestAppContext) {
    let total = Rc::new(Cell::new(0.0_f64));
    let scene = total.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _cx| {
        AnimatedNumber::new("run.total", scene.get())
            .format(grouped)
            .into_any_element()
    });
    harness.update(|_, cx| cx.set_reduce_motion(true));
    harness.update({
        let total = total.clone();
        move |_, cx| {
            total.set(1204.0);
            cx.refresh_windows();
        }
    });

    let immediate = harness.node("run.total").expect("published");
    assert_eq!(immediate.value.as_deref(), Some("1,204"));

    harness.advance(Duration::from_millis(500));
    assert_eq!(
        harness.node("run.total").expect("published").bounds,
        immediate.bounds,
        "reduced motion draws the settled glyphs on the first frame"
    );
}
