//! Motion driven through a real window, where frames and the clock are
//! simulated rather than assumed.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{TestAppContext, div, prelude::*, px};
use gpui_kit::motion::{
    CubicBezier, Easing, Flipping, Keyframe, Keyframes, MotionSpec, Presence, Sequence, Spring,
    Transition, flip, shared_flip, tracked_ids,
};
use gpui_kit::prelude::*;
use gpui_kit::prelude::{AnimatedNumber, grouped};
use gpui_kit::theme::Theme;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_testkit::harness::Harness;
use gpui_kit_testkit::present;

fn linear(duration_ms: u64) -> MotionSpec {
    MotionSpec::new(duration_ms, CubicBezier::new(0.0, 0.0, 1.0, 1.0))
}

/// An underdamped spring, so overshoot and carried speed are both observable.
fn sprung() -> MotionSpec {
    MotionSpec::sprung(Spring::new(400.0, 28.0, 1.0))
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
fn a_sprung_width_retargeted_mid_flight_keeps_moving(cx: &mut TestAppContext) {
    let transition = Rc::new(RefCell::new(Transition::new(0.0_f32, sprung())));
    let mut harness = width_scene(cx, transition.clone());

    harness.update(|_, cx| {
        transition.borrow_mut().set(100.0);
        cx.refresh_windows();
    });
    harness.advance(Duration::from_millis(40));
    let interrupted = harness.bounds("panel").expect("laid out").size.width;
    assert!(interrupted > px(0.0) && interrupted < px(100.0));

    harness.update(|_, cx| {
        transition.borrow_mut().set(200.0);
        cx.refresh_windows();
    });
    // The same retarget from a standing start, to have something to be faster
    // than.
    let mut from_rest = Transition::new(f32::from(interrupted), sprung());
    from_rest.set(200.0);
    from_rest.advance(Duration::from_millis(32));

    harness.advance(Duration::from_millis(32));
    let moved = harness.bounds("panel").expect("laid out").size.width;
    assert!(
        moved > interrupted,
        "the width stalled at {interrupted:?} instead of continuing"
    );
    assert!(
        moved > px(from_rest.value()),
        "a retarget threw away the speed the width had: {moved:?} against {:?}",
        px(from_rest.value())
    );

    harness.advance(Duration::from_secs(2));
    assert_eq!(
        harness.bounds("panel").expect("laid out").size.width,
        px(200.0)
    );
}

#[gpui::test]
fn reduced_motion_lands_a_retargeted_spring_at_once(cx: &mut TestAppContext) {
    let transition = Rc::new(RefCell::new(Transition::new(0.0_f32, sprung())));
    let mut harness = width_scene(cx, transition.clone());
    harness.update(|_, cx| cx.set_reduce_motion(true));

    harness.update(|_, cx| {
        transition.borrow_mut().set(100.0);
        cx.refresh_windows();
    });
    harness.advance(Duration::ZERO);
    harness.update(|_, cx| {
        transition.borrow_mut().set(200.0);
        cx.refresh_windows();
    });
    harness.advance(Duration::ZERO);

    assert_eq!(
        harness.bounds("panel").expect("laid out").size.width,
        px(200.0)
    );
}

#[gpui::test]
fn a_keyframed_path_passes_through_the_stops_it_was_given(_cx: &mut TestAppContext) {
    let theme = Theme::studio_dark();
    let path = Keyframes::new(
        &theme,
        linear(200),
        [
            Keyframe::new(1.0, 0.0),
            Keyframe::new(0.5, 12.0),
            Keyframe::new(0.0, 0.0),
        ],
    )
    .expect("stops were given");

    assert_eq!(path.offsets(), vec![0.0, 0.5, 1.0]);
    assert_eq!(path.sample(0.0), 0.0);
    assert!((path.sample(0.5) - 12.0).abs() < 1e-4, "the peak is a stop");
    assert_eq!(path.sample(1.0), 0.0);
    assert_eq!(path.sample(4.0), 0.0, "a path does not extrapolate");
}

#[gpui::test]
fn a_keyframe_can_be_reached_on_a_curve_of_its_own(_cx: &mut TestAppContext) {
    let theme = Theme::studio_dark();
    let path = |easing: Option<Easing>| {
        let reached = Keyframe::new(1.0, 10.0);
        Keyframes::new(
            &theme,
            linear(200),
            [
                Keyframe::new(0.0, 0.0),
                match easing {
                    Some(easing) => reached.eased(easing),
                    None => reached,
                },
            ],
        )
        .expect("stops were given")
    };

    let straight = path(None).sample(0.5);
    let held_back = path(Some(Easing::EaseIn)).sample(0.5);
    assert!(
        held_back < straight,
        "a slow-starting stop must lag the specification's curve, {held_back} against {straight}"
    );
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

/// A flexible row wrapped in a position-only flip.
///
/// `flex_1` only reaches layout if the wrapper hands layout the row's own
/// node, so a row that fills the strip is the witness that `flip` took no
/// layout node of its own.
#[gpui::test]
fn a_position_flip_takes_no_layout_node_of_its_own(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let handle = flip("stretch.row", cx);
        div()
            .flex()
            .flex_row()
            .w(px(300.0))
            .child(
                div()
                    .flex_1()
                    .h(px(20.0))
                    .semantic_in(cx, NodeSpec::new("stretch.row", Role::Group))
                    .flip(&handle, window, cx),
            )
            .into_any_element()
    });

    assert_eq!(
        harness.bounds("stretch.row").expect("laid out").size.width,
        px(300.0),
        "a wrapper with a box of its own would have swallowed flex_1"
    );
    assert_eq!(
        harness.update(|_, cx| flip("stretch.row", cx).size()),
        None,
        "a position-only flip never measures a size"
    );
}

/// A box whose size the test changes, above a marker that is not flipped.
///
/// The marker is the witness for the other half of the bargain: unlike a
/// slide, a size animation is a real layout change, so the marker does move
/// with it.
fn resize_scene(cx: &mut TestAppContext, grown: Rc<Cell<bool>>, pushed: Rc<Cell<bool>>) -> Harness {
    Harness::new(cx, gpui_kit::install, move |window, cx| {
        let handle = flip("panel", cx);
        let (width, height) = if grown.get() {
            (200.0, 80.0)
        } else {
            (100.0, 40.0)
        };
        let mut column = div().flex().flex_col().items_start();
        if pushed.get() {
            column = column.child(div().w(px(10.0)).h(px(30.0)));
        }
        column
            .child(
                div()
                    .w(px(width))
                    .h(px(height))
                    .semantic_in(cx, NodeSpec::new("panel", Role::Group))
                    .flip_size(&handle, window, cx),
            )
            .child(
                div()
                    .w(px(100.0))
                    .h(px(20.0))
                    .semantic_in(cx, NodeSpec::new("marker", Role::Group)),
            )
            .into_any_element()
    })
}

fn grow(harness: &mut Harness, grown: &Rc<Cell<bool>>) {
    let grown = grown.clone();
    harness.update(move |_, cx| {
        grown.set(true);
        cx.refresh_windows();
    });
}

#[gpui::test]
fn a_resized_element_starts_at_its_old_size_and_lands_on_the_new_one(cx: &mut TestAppContext) {
    let grown = Rc::new(Cell::new(false));
    let mut harness = resize_scene(cx, grown.clone(), Rc::new(Cell::new(false)));
    assert_eq!(
        harness.update(|_, cx| flip("panel", cx).size()),
        Some(gpui::size(px(100.0), px(40.0))),
        "a first measurement is drawn at once"
    );

    grow(&mut harness, &grown);
    assert_eq!(
        harness.update(|_, cx| flip("panel", cx).size()),
        Some(gpui::size(px(100.0), px(40.0))),
        "the box starts the frame at the size it had"
    );
    assert_eq!(
        harness.update(|_, cx| flip("panel", cx).target_size()),
        Some(gpui::size(px(200.0), px(80.0))),
        "and it already knows where it is going"
    );

    harness.advance(Duration::from_millis(500));
    assert_eq!(
        harness.update(|_, cx| flip("panel", cx).size()),
        Some(gpui::size(px(200.0), px(80.0))),
        "a resize lands exactly on the new size"
    );
}

#[gpui::test]
fn a_resize_moves_the_siblings_a_slide_would_not(cx: &mut TestAppContext) {
    let grown = Rc::new(Cell::new(false));
    let mut harness = resize_scene(cx, grown.clone(), Rc::new(Cell::new(false)));
    let before = harness.bounds("marker").expect("laid out").origin.y;

    grow(&mut harness, &grown);
    let during = harness.bounds("marker").expect("laid out").origin.y;
    assert_eq!(
        during, before,
        "the frame a resize starts on is still the old layout"
    );

    harness.advance(Duration::from_millis(60));
    let midway = harness.bounds("marker").expect("laid out").origin.y;
    assert!(
        midway > before && midway < before + px(40.0),
        "a size animation is a real layout change and takes its siblings with \
         it: the marker was at {before:?} and is at {midway:?}"
    );

    harness.advance(Duration::from_millis(500));
    assert_eq!(
        harness.bounds("marker").expect("laid out").origin.y,
        before + px(40.0),
        "the marker ends below the grown box"
    );
}

#[gpui::test]
fn reduced_motion_puts_a_resized_element_straight_at_its_new_size(cx: &mut TestAppContext) {
    let grown = Rc::new(Cell::new(false));
    let mut harness = resize_scene(cx, grown.clone(), Rc::new(Cell::new(false)));
    let before = harness.bounds("marker").expect("laid out").origin.y;
    harness.update(|_, cx| cx.set_reduce_motion(true));

    grow(&mut harness, &grown);
    assert_eq!(
        harness.update(|_, cx| flip("panel", cx).size()),
        Some(gpui::size(px(200.0), px(80.0))),
        "reduced motion is at the new size on the first frame"
    );
    assert_eq!(
        harness.bounds("marker").expect("laid out").origin.y,
        before + px(40.0),
        "and everything below it is already where it ends up"
    );
}

#[gpui::test]
fn an_element_that_moves_and_grows_does_both(cx: &mut TestAppContext) {
    let grown = Rc::new(Cell::new(false));
    let pushed = Rc::new(Cell::new(false));
    let mut harness = resize_scene(cx, grown.clone(), pushed.clone());

    harness.update({
        let grown = grown.clone();
        let pushed = pushed.clone();
        move |_, cx| {
            grown.set(true);
            pushed.set(true);
            cx.refresh_windows();
        }
    });

    assert_eq!(
        harness.update(|_, cx| flip("panel", cx).offset()).y,
        px(-30.0),
        "the box is drawn where it used to be"
    );
    assert_eq!(
        harness.update(|_, cx| flip("panel", cx).size()),
        Some(gpui::size(px(100.0), px(40.0))),
        "at the size it used to be"
    );

    harness.advance(Duration::from_millis(500));
    assert_eq!(
        harness.update(|_, cx| flip("panel", cx).offset()),
        gpui::Point::default()
    );
    assert_eq!(
        harness.update(|_, cx| flip("panel", cx).size()),
        Some(gpui::size(px(200.0), px(80.0)))
    );
}

/// An element that takes its width from the box it is given, inside a strip
/// the test widens.
///
/// The other scenes watch the box a resize animates; this one watches the
/// element inside it, which is drawn at the animated size because it sizes
/// itself from what it is handed.
fn filling_scene(cx: &mut TestAppContext, wide: Rc<Cell<bool>>) -> Harness {
    Harness::new(cx, gpui_kit::install, move |window, cx| {
        let handle = flip("filler", cx);
        div()
            .flex()
            .flex_col()
            .items_start()
            .w(px(if wide.get() { 300.0 } else { 100.0 }))
            .child(
                div()
                    .w_full()
                    .h(px(40.0))
                    .semantic_in(cx, NodeSpec::new("filler", Role::Group))
                    .flip_size(&handle, window, cx),
            )
            .into_any_element()
    })
}

#[gpui::test]
fn an_element_that_fills_its_box_is_drawn_at_the_animated_size(cx: &mut TestAppContext) {
    let wide = Rc::new(Cell::new(false));
    let mut harness = filling_scene(cx, wide.clone());
    assert_eq!(
        harness.bounds("filler").expect("laid out").size.width,
        px(100.0)
    );

    harness.update({
        let wide = wide.clone();
        move |_, cx| {
            wide.set(true);
            cx.refresh_windows();
        }
    });
    // An element sized from its container learns its new constraints from the
    // frame that applied them, so this animation starts a frame after the one
    // that changed the container.
    harness.advance(Duration::from_millis(16));
    harness.advance(Duration::from_millis(40));
    let midway = harness.bounds("filler").expect("laid out").size.width;
    assert!(
        midway > px(100.0) && midway < px(300.0),
        "the element itself is the animated size, not a picture of the \
         settled one: {midway:?}"
    );

    harness.advance(Duration::from_millis(500));
    assert_eq!(
        harness.bounds("filler").expect("laid out").size.width,
        px(300.0)
    );
}

/// One identity rendered by two different trees.
///
/// `stage` picks which tree renders: the list on its own, the detail panel on
/// its own, both at once, or neither.
#[derive(Clone, Copy, PartialEq)]
enum Stage {
    List,
    Detail,
    Both,
    Neither,
}

fn shared_scene(cx: &mut TestAppContext, stage: Rc<Cell<Stage>>) -> Harness {
    Harness::new(cx, gpui_kit::install, move |window, cx| {
        let mut root = div().flex().flex_col().items_start();
        if matches!(stage.get(), Stage::List | Stage::Both) {
            let handle = shared_flip("item.7", cx);
            root = root.child(
                div()
                    .w(px(80.0))
                    .h(px(20.0))
                    .semantic_in(cx, NodeSpec::new("row", Role::Group))
                    .flip_size(&handle, window, cx),
            );
        }
        if matches!(stage.get(), Stage::Detail | Stage::Both) {
            let handle = shared_flip("item.7", cx);
            root = root.child(div().w(px(10.0)).h(px(50.0))).child(
                div()
                    .w(px(240.0))
                    .h(px(120.0))
                    .semantic_in(cx, NodeSpec::new("detail", Role::Group))
                    .flip_size(&handle, window, cx),
            );
        }
        root.into_any_element()
    })
}

fn stage(harness: &mut Harness, stage: &Rc<Cell<Stage>>, next: Stage) {
    let stage = stage.clone();
    harness.update(move |_, cx| {
        stage.set(next);
        cx.refresh_windows();
    });
}

#[gpui::test]
fn a_shared_id_travels_from_the_tree_that_last_rendered_it(cx: &mut TestAppContext) {
    let showing = Rc::new(Cell::new(Stage::List));
    let mut harness = shared_scene(cx, showing.clone());
    assert_eq!(
        harness.update(|_, cx| flip("item.7", cx).size()),
        Some(gpui::size(px(80.0), px(20.0)))
    );

    stage(&mut harness, &showing, Stage::Detail);
    assert_eq!(
        harness.update(|_, cx| flip("item.7", cx).offset()).y,
        px(-50.0),
        "the panel is drawn where the row was"
    );
    assert_eq!(
        harness.update(|_, cx| flip("item.7", cx).size()),
        Some(gpui::size(px(80.0), px(20.0))),
        "and at the size the row was"
    );

    harness.advance(Duration::from_millis(500));
    assert_eq!(
        harness.update(|_, cx| flip("item.7", cx).offset()),
        gpui::Point::default()
    );
    assert_eq!(
        harness.update(|_, cx| flip("item.7", cx).size()),
        Some(gpui::size(px(240.0), px(120.0))),
        "and arrives as itself"
    );
}

#[gpui::test]
fn two_trees_sharing_an_id_in_one_frame_refuse_to_animate(cx: &mut TestAppContext) {
    let showing = Rc::new(Cell::new(Stage::List));
    let mut harness = shared_scene(cx, showing.clone());

    stage(&mut harness, &showing, Stage::Both);
    assert!(
        harness.update(|_, cx| flip("item.7", cx).is_contended()),
        "two elements in one slot is a collision"
    );
    assert_eq!(
        harness.update(|_, cx| flip("item.7", cx).offset()),
        gpui::Point::default(),
        "a contested id does not animate rather than oscillating"
    );

    harness.frame();
    assert_eq!(
        harness.update(|_, cx| flip("item.7", cx).offset()),
        gpui::Point::default(),
        "and it keeps not animating while the collision lasts"
    );

    stage(&mut harness, &showing, Stage::Detail);
    harness.frame();
    assert!(
        !harness.update(|_, cx| flip("item.7", cx).is_contended()),
        "one renderer again is no longer a collision"
    );
    assert_eq!(
        harness.update(|_, cx| flip("item.7", cx).offset()),
        gpui::Point::default(),
        "and it resumes from a rectangle nothing else was writing to"
    );
}

#[gpui::test]
fn a_shared_id_survives_the_gap_between_two_trees(cx: &mut TestAppContext) {
    let showing = Rc::new(Cell::new(Stage::List));
    let mut harness = shared_scene(cx, showing.clone());

    stage(&mut harness, &showing, Stage::Neither);
    for _ in 0..5 {
        harness.frame();
    }
    assert!(
        harness
            .update(|_, cx| tracked_ids(cx))
            .contains(&"item.7".into()),
        "a shared id outlives the frames in which neither tree renders it"
    );

    stage(&mut harness, &showing, Stage::Detail);
    assert_eq!(
        harness.update(|_, cx| flip("item.7", cx).offset()).y,
        px(-50.0),
        "the panel still arrives from where the row was"
    );
}

#[gpui::test]
fn a_shared_id_left_too_long_arrives_already_in_place(cx: &mut TestAppContext) {
    let showing = Rc::new(Cell::new(Stage::List));
    let mut harness = shared_scene(cx, showing.clone());

    stage(&mut harness, &showing, Stage::Neither);
    harness.advance(Duration::from_secs(2));

    stage(&mut harness, &showing, Stage::Detail);
    assert_eq!(
        harness.update(|_, cx| flip("item.7", cx).offset()),
        gpui::Point::default(),
        "a rectangle from long ago is not somewhere to fly in from"
    );
    assert_eq!(
        harness.update(|_, cx| flip("item.7", cx).size()),
        Some(gpui::size(px(240.0), px(120.0))),
        "the panel is simply its own size"
    );
}

/// The other bound on a handoff. An idle window advances the clock without
/// drawing and a busy one draws without the clock moving much, so a gap that
/// runs out of frames ends the handoff just as one that runs out of time does.
#[gpui::test]
fn a_shared_id_left_for_too_many_frames_arrives_already_in_place(cx: &mut TestAppContext) {
    let showing = Rc::new(Cell::new(Stage::List));
    let mut harness = shared_scene(cx, showing.clone());

    stage(&mut harness, &showing, Stage::Neither);
    for _ in 0..40 {
        harness.frame();
    }

    stage(&mut harness, &showing, Stage::Detail);
    assert_eq!(
        harness.update(|_, cx| flip("item.7", cx).offset()),
        gpui::Point::default(),
        "the clock never moved, so only the frames the gap took can have \
         ended the handoff"
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

// Motion spread across the component families.
//
// The two facts every family has to keep are the same: what the tree publishes
// is where things have settled, and reduced motion leaves one static frame.

/// Everything the tree publishes on the frame a change lands on is what it
/// publishes for good, so nothing is still finding its place afterwards.
fn holds_one_frame(harness: &mut Harness) {
    let settled = harness.snapshot().nodes;
    for _ in 0..4 {
        harness.advance(Duration::from_millis(150));
    }
    assert_eq!(
        harness.snapshot().nodes,
        settled,
        "a reduced-motion frame must be the last word"
    );
}

fn choice_scene(cx: &mut TestAppContext, on: Rc<Cell<bool>>) -> Harness {
    Harness::new(cx, gpui_kit::install, move |_, _| {
        let on = on.get();
        div()
            .child(
                Checkbox::new("form.terms")
                    .label("Accept")
                    .checked(on)
                    .on_change(|_, _, _| {}),
            )
            .child(
                Radio::new("form.plan")
                    .label("Monthly")
                    .selected(on)
                    .on_select(|_, _| {}),
            )
            .child(
                Switch::new("form.notify")
                    .label("Notify")
                    .on(on)
                    .on_change(|_, _, _| {}),
            )
            .child(
                Slider::new("form.volume")
                    .range(0.0, 1.0)
                    .value(if on { 0.9 } else { 0.1 })
                    .on_change(|_, _, _| {}),
            )
            .child(
                SegmentedControl::new("form.view")
                    .segments(vec![
                        Segment::new("list", "List"),
                        Segment::new("grid", "Grid"),
                    ])
                    .selected(if on { "grid" } else { "list" })
                    .on_select(|_, _, _| {}),
            )
            .into_any_element()
    })
}

#[gpui::test]
fn reduced_motion_holds_one_frame_across_the_choice_controls(cx: &mut TestAppContext) {
    let on = Rc::new(Cell::new(false));
    let mut harness = choice_scene(cx, on.clone());
    harness.update(|_, cx| cx.set_reduce_motion(true));

    harness.update({
        let on = on.clone();
        move |_, cx| {
            on.set(true);
            cx.refresh_windows();
        }
    });

    assert_eq!(
        harness.update(|_, cx| flip("form.view.selection", cx).offset()),
        gpui::Point::default(),
        "the segmented background must be drawn where it belongs at once"
    );
    holds_one_frame(&mut harness);
}

#[gpui::test]
fn a_segmented_background_slides_to_the_segment_that_was_chosen(cx: &mut TestAppContext) {
    let on = Rc::new(Cell::new(false));
    let mut harness = choice_scene(cx, on.clone());
    assert_eq!(
        harness.update(|_, cx| flip("form.view.selection", cx).offset()),
        gpui::Point::default()
    );

    harness.update({
        let on = on.clone();
        move |_, cx| {
            on.set(true);
            cx.refresh_windows();
        }
    });

    let offset = harness.update(|_, cx| flip("form.view.selection", cx).offset());
    assert!(
        offset.x < px(0.0),
        "the background starts drawn under the segment that used to hold, got {offset:?}"
    );

    harness.advance(Duration::from_millis(600));
    assert_eq!(
        harness.update(|_, cx| flip("form.view.selection", cx).offset()),
        gpui::Point::default(),
        "the slide settles under the segment that holds now"
    );
}

fn navigation_scene(cx: &mut TestAppContext, second: Rc<Cell<bool>>) -> Harness {
    Harness::new(cx, gpui_kit::install, move |_, _| {
        let second = second.get();
        div()
            .child(
                Tabs::new("workspace.tabs")
                    .tabs(vec![
                        TabItem::new("overview", "Overview"),
                        TabItem::new("runs", "Runs"),
                    ])
                    .selected(if second { "runs" } else { "overview" })
                    .on_select(|_, _, _| {}),
            )
            .child(
                Accordion::new("settings.sections")
                    .expanded_ids(if second { &["storage"][..] } else { &[][..] })
                    .on_toggle(|_, _, _, _| {})
                    .section(
                        AccordionSection::new("storage", "Storage")
                            .body(div().h(px(40.0)).child("Where results are kept")),
                    )
                    .section(AccordionSection::new("policy", "Policy")),
            )
            .into_any_element()
    })
}

#[gpui::test]
fn a_tab_indicator_slides_to_the_tab_that_holds_now(cx: &mut TestAppContext) {
    let second = Rc::new(Cell::new(false));
    let mut harness = navigation_scene(cx, second.clone());

    harness.update({
        let second = second.clone();
        move |_, cx| {
            second.set(true);
            cx.refresh_windows();
        }
    });

    let offset = harness.update(|_, cx| flip("workspace.tabs.indicator", cx).offset());
    assert!(
        offset.x < px(0.0),
        "the indicator starts drawn under the tab that used to hold, got {offset:?}"
    );

    harness.advance(Duration::from_millis(600));
    assert_eq!(
        harness.update(|_, cx| flip("workspace.tabs.indicator", cx).offset()),
        gpui::Point::default()
    );
}

#[gpui::test]
fn an_opening_section_pushes_what_is_below_it_over_several_frames(cx: &mut TestAppContext) {
    let second = Rc::new(Cell::new(false));
    let mut harness = navigation_scene(cx, second.clone());
    let closed = harness
        .bounds("settings.sections.policy")
        .expect("laid out");

    harness.update({
        let second = second.clone();
        move |_, cx| {
            second.set(true);
            cx.refresh_windows();
        }
    });
    let opening = harness
        .bounds("settings.sections.policy")
        .expect("laid out");
    assert_eq!(
        opening.origin.y, closed.origin.y,
        "the section below has not been pushed anywhere yet"
    );

    harness.advance(Duration::from_millis(600));
    let opened = harness
        .bounds("settings.sections.policy")
        .expect("laid out");
    assert!(
        opened.origin.y > closed.origin.y,
        "the body ends up occupying real height, {opened:?} against {closed:?}"
    );
}

#[gpui::test]
fn reduced_motion_opens_a_section_at_its_full_height_at_once(cx: &mut TestAppContext) {
    let second = Rc::new(Cell::new(false));
    let mut harness = navigation_scene(cx, second.clone());
    harness.update(|_, cx| cx.set_reduce_motion(true));
    let closed = harness
        .bounds("settings.sections.policy")
        .expect("laid out");

    harness.update({
        let second = second.clone();
        move |_, cx| {
            second.set(true);
            cx.refresh_windows();
        }
    });

    let opened = harness
        .bounds("settings.sections.policy")
        .expect("laid out");
    assert!(
        opened.origin.y > closed.origin.y,
        "the body is at its full height on the frame it opens"
    );
    assert_eq!(
        harness.update(|_, cx| flip("workspace.tabs.indicator", cx).offset()),
        gpui::Point::default()
    );
    holds_one_frame(&mut harness);
}

fn display_scene(cx: &mut TestAppContext, done: Rc<Cell<bool>>) -> Harness {
    Harness::new(cx, gpui_kit::install, move |_, _| {
        let done = done.get();
        div()
            .child(ProgressBar::new("run.progress").fraction(if done { 0.9 } else { 0.1 }))
            .child(Skeleton::new("run.skeleton").rows(2))
            .child(EmptyState::new("run.empty", "Nothing ran yet"))
            .child(Callout::new("Results are kept in the workspace", Tone::Neutral).id("run.note"))
            .into_any_element()
    })
}

#[gpui::test]
fn a_progress_bar_publishes_the_number_it_was_given_while_the_fill_is_moving(
    cx: &mut TestAppContext,
) {
    let done = Rc::new(Cell::new(false));
    let mut harness = display_scene(cx, done.clone());

    harness.update({
        let done = done.clone();
        move |_, cx| {
            done.set(true);
            cx.refresh_windows();
        }
    });

    assert_eq!(
        harness.node("run.progress").expect("published").value_now,
        Some(0.9),
        "a fill in flight is not a fact: the number is published at once"
    );
}

#[gpui::test]
fn reduced_motion_holds_one_frame_across_the_display_family(cx: &mut TestAppContext) {
    let done = Rc::new(Cell::new(false));
    let mut harness = display_scene(cx, done.clone());
    harness.update(|_, cx| cx.set_reduce_motion(true));

    harness.update({
        let done = done.clone();
        move |_, cx| {
            done.set(true);
            cx.refresh_windows();
        }
    });

    holds_one_frame(&mut harness);
}

/// A menu whose panel is open, so the rows are the thing under test.
fn overlay_scene(cx: &mut TestAppContext) -> (Harness, gpui::Entity<Menu>) {
    let slot: Rc<RefCell<Option<gpui::Entity<Menu>>>> = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let menu = build
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    Menu::new("workspace.edit", window, cx)
                        .trigger("Edit")
                        .items(vec![
                            MenuItem::command("undo", "Undo"),
                            MenuItem::command("redo", "Redo"),
                            MenuItem::command("paste", "Paste"),
                        ])
                })
            })
            .clone();
        div()
            .w(px(600.0))
            .h(px(400.0))
            .child(menu)
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("menu was built");
    (harness, entity)
}

#[gpui::test]
fn reduced_motion_holds_one_frame_across_the_overlay_family(cx: &mut TestAppContext) {
    let (mut harness, menu) = overlay_scene(cx);
    harness.update(|_, cx| cx.set_reduce_motion(true));

    harness.update(|window, cx| {
        menu.update(cx, |menu, cx| menu.open(window, cx));
    });

    // Every row is addressable on the frame the panel opens, whatever the wave
    // would otherwise have been doing to it.
    let snapshot = harness.snapshot();
    for id in [
        "workspace.edit.undo",
        "workspace.edit.redo",
        "workspace.edit.paste",
    ] {
        assert!(snapshot.contains(id), "`{id}` must be published at once");
    }
    holds_one_frame(&mut harness);
}

/// Publishes the presence progress as a width, so a test reads how far in or
/// out the element actually is rather than trusting the state machine.
fn presence_scene(cx: &mut TestAppContext, presence: Rc<RefCell<Presence>>) -> Harness {
    Harness::new(cx, gpui_kit::install, move |window, cx| {
        let mut presence = presence.borrow_mut();
        let progress = presence.animate(window, cx);
        let mut root = div();
        if presence.is_rendered() {
            root = root.child(
                div()
                    .w(px(100.0 * progress))
                    .h(px(10.0))
                    .semantic_in(cx, NodeSpec::new("panel", Role::Group)),
            );
        }
        root.into_any_element()
    })
}

/// Enter and exit on a curve that is nowhere near linear, so a reversal that
/// assumed the two timelines were proportional would show up as a jump.
fn curved(duration_ms: u64) -> MotionSpec {
    MotionSpec::new(duration_ms, CubicBezier::new(0.42, 0.0, 0.58, 1.0))
}

#[gpui::test]
fn an_entrance_cancelled_part_way_leaves_from_where_it_was(cx: &mut TestAppContext) {
    let presence = Rc::new(RefCell::new(Presence::hidden(curved(200), curved(100))));
    let mut harness = presence_scene(cx, presence.clone());

    harness.update(|_, cx| {
        presence.borrow_mut().show();
        cx.refresh_windows();
    });
    harness.advance(Duration::from_millis(60));
    let interrupted = harness.bounds("panel").expect("laid out").size.width;
    assert!(
        interrupted > px(2.0) && interrupted < px(40.0),
        "expected an entrance barely under way, got {interrupted:?}"
    );

    harness.update(|_, cx| {
        presence.borrow_mut().hide();
        cx.refresh_windows();
    });
    let reversed = harness.bounds("panel").expect("still rendered").size.width;
    assert!(
        (reversed - interrupted).abs() < px(4.0),
        "the element jumped from {interrupted:?} to {reversed:?} on being cancelled"
    );

    // A full exit is 100ms, and this one had only a fifth of the way to come
    // back: it is gone well before a full exit would have been.
    harness.advance(Duration::from_millis(60));
    assert!(
        present(&harness.snapshot(), "panel").is_err(),
        "a barely started entrance took longer to leave than a full exit"
    );
}

#[gpui::test]
fn a_full_exit_still_takes_the_whole_exit(cx: &mut TestAppContext) {
    let presence = Rc::new(RefCell::new(Presence::visible(curved(200), curved(100))));
    let mut harness = presence_scene(cx, presence.clone());

    harness.update(|_, cx| {
        presence.borrow_mut().hide();
        cx.refresh_windows();
    });
    harness.advance(Duration::from_millis(60));
    assert!(
        present(&harness.snapshot(), "panel").is_ok(),
        "an exit from fully present must use its whole span"
    );
    harness.advance(Duration::from_millis(60));
    assert!(present(&harness.snapshot(), "panel").is_err());
}

#[gpui::test]
fn an_exit_cancelled_part_way_comes_back_the_way_it_went(cx: &mut TestAppContext) {
    let presence = Rc::new(RefCell::new(Presence::visible(curved(200), curved(100))));
    let mut harness = presence_scene(cx, presence.clone());

    harness.update(|_, cx| {
        presence.borrow_mut().hide();
        cx.refresh_windows();
    });
    harness.advance(Duration::from_millis(40));
    let interrupted = harness.bounds("panel").expect("still rendered").size.width;
    assert!(interrupted < px(100.0), "the exit has not started");

    harness.update(|_, cx| {
        presence.borrow_mut().show();
        cx.refresh_windows();
    });
    let reversed = harness.bounds("panel").expect("still rendered").size.width;
    assert!(
        (reversed - interrupted).abs() < px(4.0),
        "the element jumped from {interrupted:?} to {reversed:?} on being cancelled"
    );

    harness.advance(Duration::from_millis(200));
    assert_eq!(
        harness.bounds("panel").expect("present").size.width,
        px(100.0)
    );
}

#[gpui::test]
fn reduced_motion_lands_both_phases_on_the_frame_they_are_asked_for(cx: &mut TestAppContext) {
    let presence = Rc::new(RefCell::new(Presence::hidden(curved(200), curved(100))));
    let mut harness = presence_scene(cx, presence.clone());
    harness.update(|_, cx| cx.set_reduce_motion(true));

    harness.update(|_, cx| {
        presence.borrow_mut().show();
        cx.refresh_windows();
    });
    harness.advance(Duration::ZERO);
    assert_eq!(
        harness.bounds("panel").expect("present at once").size.width,
        px(100.0),
        "an entrance under reduced motion is over before it is drawn"
    );

    harness.update(|_, cx| {
        presence.borrow_mut().hide();
        cx.refresh_windows();
    });
    harness.advance(Duration::ZERO);
    assert!(present(&harness.snapshot(), "panel").is_err());
}

#[gpui::test]
fn a_sequenced_motion_starts_when_its_predecessor_ends(_cx: &mut TestAppContext) {
    let sequence = Sequence::new([linear(200)]).then(linear(100));

    assert_eq!(sequence.start(1), Duration::from_millis(200));
    assert_eq!(sequence.total(), Duration::from_millis(300));
    assert_eq!(
        sequence.step(1).expect("two steps").delay_ms,
        200,
        "the second step waits out the first"
    );

    // Halfway through the group the first step is over and the second has not
    // begun to move.
    assert_eq!(sequence.progress(0, 200.0 / 300.0), 1.0);
    assert!(sequence.progress(1, 200.0 / 300.0) < 1e-6);
    assert!((sequence.progress(1, 250.0 / 300.0) - 0.5).abs() < 0.01);

    // The same composition written on the specification itself.
    assert_eq!(
        linear(100).after(linear(200)).total(),
        sequence.total(),
        "a chained spec and a sequence describe the same run"
    );
}

#[gpui::test]
fn a_reversed_stagger_gives_the_last_item_the_shortest_delay(_cx: &mut TestAppContext) {
    let forward = gpui_kit::motion::Stagger::from_millis(20);
    let backward = forward.reversed();

    assert_eq!(forward.delay(0, 4), Duration::ZERO);
    assert_eq!(backward.delay(3, 4), Duration::ZERO);
    assert!(backward.delay(0, 4) > backward.delay(3, 4));
    assert_eq!(
        backward.total(4, linear(100)),
        forward.total(4, linear(100)),
        "reversing changes who waits, not how long the group takes"
    );
}

#[gpui::test]
fn a_spring_can_be_asked_for_as_a_duration_and_a_bounce(_cx: &mut TestAppContext) {
    let asked = Duration::from_millis(400);
    let critical = Spring::perceptual(asked, 0.0);
    assert!((critical.damping_ratio() - 1.0).abs() < 1e-3);
    assert!(
        critical.perceptual_duration().abs_diff(asked) < Duration::from_millis(1),
        "it reported {:?} for {asked:?}",
        critical.perceptual_duration()
    );

    let peak = |spring: Spring| {
        let settle = spring.settle_time();
        (0..=200)
            .map(|step| spring.value(settle.mul_f32(step as f32 / 200.0)))
            .fold(f32::MIN, f32::max)
    };
    assert!(
        peak(critical) <= 1.001,
        "a bounce of zero must not overshoot"
    );
    assert!(peak(Spring::perceptual(asked, 0.4)) > 1.0);
    assert!(peak(Spring::perceptual(asked, -0.4)) <= 1.001);
}

#[gpui::test]
fn a_token_spring_preset_is_what_it_always_was(_cx: &mut TestAppContext) {
    let theme = Theme::studio_dark();
    for preset in [
        gpui_kit::theme::SpringPreset::Snappy,
        gpui_kit::theme::SpringPreset::Smooth,
        gpui_kit::theme::SpringPreset::Bouncy,
        gpui_kit::theme::SpringPreset::Grab,
    ] {
        let tokens = theme.spring(preset);
        let spring = Spring::preset(&theme, preset);
        assert_eq!(spring.stiffness, tokens.stiffness);
        assert_eq!(spring.damping, tokens.damping);
        assert_eq!(spring.mass, tokens.mass);
    }
}

#[gpui::test]
fn a_row_wave_is_capped_however_long_the_list_is(_cx: &mut TestAppContext) {
    let cap = gpui_kit::motion::ROW_STAGGER_CAP;
    let stagger = gpui_kit::motion::Stagger::rows();
    for count in [3, 12, 400] {
        assert!(
            stagger.delay(count - 1, count).as_millis() <= cap.as_millis(),
            "{count} rows outlasted the cap"
        );
    }
}
