//! Motion driven by the hand: the speed a gesture was released at, the effects
//! built on it, and values read off a scroll offset rather than off a clock.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{Point, TestAppContext, div, point, prelude::*, px};
use gpui_kit::motion::{
    Flick, MotionSpec, ScrollLink, Spring, Transition, Velocity, flick, rubber_band,
};
use gpui_kit::prelude::*;
use gpui_kit::theme::Theme;
use gpui_kit_testkit::harness::Harness;

const ROWS: [&str; 4] = ["alpha", "beta", "gamma", "delta"];

/// What the host was told about the last drop.
type Released = Rc<RefCell<Option<(gpui::SharedString, Velocity)>>>;

fn list_harness(cx: &mut TestAppContext, released: Released) -> Harness {
    Harness::new(cx, gpui_kit::install, move |_, _| {
        let released = Rc::clone(&released);
        div()
            .w(px(320.0))
            .child(
                List::new("queue", ROWS.len(), |index, _, _| {
                    ListItem::new(ROWS[index], ROWS[index]).text(ROWS[index])
                })
                .row_height(32.0)
                .reorderable(true)
                .on_select(|_, _, _| {})
                .on_reorder(move |intent, _, _| {
                    *released.borrow_mut() = Some((intent.item.id.clone(), intent.velocity));
                }),
            )
            .into_any_element()
    })
}

/// Drags the bottom row to the top of the list in even steps, so the gesture
/// has a speed a tracker can measure rather than one instant of travel.
///
/// Returns where the gesture started, so a test can say how far it went.
fn sweep_to_the_top(harness: &mut Harness) -> Point<gpui::Pixels> {
    harness.drag_start("queue.delta");
    let from = harness.pointer();
    let to = harness.point_down("queue.alpha", 0.2);
    let steps = 6;
    for step in 1..=steps {
        harness
            .context()
            .executor()
            .advance_clock(Duration::from_millis(10));
        let fraction = step as f32 / steps as f32;
        harness.drag_to(point(from.x, from.y + (to.y - from.y) * fraction));
    }
    from
}

#[gpui::test]
fn a_drop_reports_the_speed_the_gesture_was_moving_at(cx: &mut TestAppContext) {
    let released: Released = Rc::new(RefCell::new(None));
    let mut harness = list_harness(cx, Rc::clone(&released));

    let from = sweep_to_the_top(&mut harness);
    let travel = harness.pointer() - from;
    harness.drop_here();

    let (id, velocity) = released.borrow().clone().expect("the drop was reported");
    assert_eq!(id.as_ref(), "delta");
    assert!(
        velocity.y < -800.0,
        "a gesture that crossed the list in sixty milliseconds was moving: {velocity:?}"
    );
    assert_eq!(
        flick(travel, velocity, &Theme::studio_dark()),
        Some(Flick::Up),
        "a fast upward gesture is a flick upward"
    );
}

#[gpui::test]
fn a_drag_that_stopped_before_release_reports_no_velocity(cx: &mut TestAppContext) {
    let released: Released = Rc::new(RefCell::new(None));
    let mut harness = list_harness(cx, Rc::clone(&released));

    let from = sweep_to_the_top(&mut harness);
    let travel = harness.pointer() - from;
    // The hand stops and holds. No moves are delivered, so the pause is only
    // visible against the clock.
    harness
        .context()
        .executor()
        .advance_clock(Duration::from_millis(300));
    harness.drop_here();

    let (_, velocity) = released.borrow().clone().expect("the drop was reported");
    assert_eq!(
        velocity,
        Velocity::ZERO,
        "a drag the user parked must not report the speed it had before the pause"
    );
    assert_eq!(
        flick(travel, velocity, &Theme::studio_dark()),
        None,
        "a parked drag is a placement, not a flick"
    );
}

#[test]
fn a_flick_is_not_a_slow_drag_of_the_same_distance() {
    let theme = Theme::studio_dark();
    let travel = point(px(0.0), px(-140.0));
    let flicked = Velocity::new(0.0, -theme.motion.flick_velocity * 3.0);
    let dragged = Velocity::new(0.0, -theme.motion.flick_velocity / 5.0);

    assert_eq!(flick(travel, flicked, &theme), Some(Flick::Up));
    assert_eq!(flick(travel, dragged, &theme), None);
    assert!(flicked.speed() > dragged.speed());
    assert!(!flicked.is_still() && !Velocity::ZERO.speed().is_nan());
    assert!(Velocity::ZERO.is_still());
}

#[test]
fn a_rubber_band_resists_more_the_harder_it_is_pulled_and_never_gives_way() {
    let theme = Theme::studio_dark();
    let extent = px(240.0);
    let tension = theme.motion.rubber_band_tension;

    let mut previous = px(0.0);
    for pull in [1.0, 10.0, 40.0, 120.0, 400.0, 10_000.0] {
        let shown = rubber_band(px(pull), extent, tension);
        assert!(shown > previous, "the band stopped stretching at {pull}");
        assert!(shown < px(pull), "the band did not resist at {pull}");
        assert!(shown < extent, "the band gave way at {pull}");
        previous = shown;
    }
}

/// An underdamped spring, so what a carried speed adds is visible as overshoot.
fn sprung() -> MotionSpec {
    MotionSpec::sprung(Spring::new(400.0, 28.0, 1.0))
}

#[test]
fn a_value_released_with_speed_travels_further_than_one_released_at_rest() {
    let peak = |mut transition: Transition<f32>| {
        let mut highest = f32::MIN;
        for _ in 0..200 {
            transition.advance(Duration::from_millis(8));
            highest = highest.max(transition.value());
        }
        highest
    };

    let mut flung = Transition::new(0.0_f32, sprung());
    flung.release(10.0, 60.0);
    let mut placed = Transition::new(0.0_f32, sprung());
    placed.set(10.0);

    let (flung, placed) = (peak(flung), peak(placed));
    assert!(
        flung > placed,
        "inertia was thrown away: {flung} against {placed}"
    );
}

#[test]
fn a_released_value_still_settles_exactly_on_its_target() {
    let mut transition = Transition::new(0.0_f32, sprung());
    transition.release(10.0, 90.0);
    transition.advance(Duration::from_secs(5));
    assert_eq!(transition.value(), 10.0);
    assert!(!transition.is_animating());
}

#[test]
fn scroll_progress_is_nothing_before_the_range_all_of_it_after_and_grows_between() {
    let link = ScrollLink::new(px(40.0), px(160.0));
    assert_eq!(link.progress(px(0.0)), 0.0);
    assert_eq!(link.progress(px(40.0)), 0.0);
    assert_eq!(link.progress(px(160.0)), 1.0);
    assert_eq!(link.progress(px(9_000.0)), 1.0);

    let mut previous = 0.0;
    for offset in 0..=200 {
        let progress = link.progress(px(offset as f32));
        assert!(progress >= previous, "progress fell at offset {offset}");
        previous = progress;
    }

    // A collapsing header and a shadow are the two motivating cases, and both
    // are this one call.
    assert_eq!(link.sample(px(100.0), px(96.0), px(48.0)), px(72.0));
}

/// A scroll-linked value is a function of the offset, so nothing about it asks
/// for another frame. The transition beside it does, which is what makes the
/// assertion mean something.
#[gpui::test]
fn a_scroll_linked_value_asks_for_no_animation_frames(cx: &mut TestAppContext) {
    let mut linked = Harness::new(cx, gpui_kit::install, |_, _| {
        let width = ScrollLink::over(px(200.0)).sample(px(80.0), px(10.0), px(90.0));
        div().w(width).h(px(10.0)).into_any_element()
    });
    assert_eq!(
        linked.update(|window, cx| window.simulate_next_frame(cx)),
        0,
        "a value read off an offset has nothing to animate"
    );

    let transition = Rc::new(RefCell::new(Transition::new(0.0_f32, sprung())));
    let driven = Rc::clone(&transition);
    let mut timed = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let width = driven.borrow_mut().animate(window, cx);
        div().w(px(width)).h(px(10.0)).into_any_element()
    });
    timed.update(|_, cx| {
        transition.borrow_mut().set(100.0);
        cx.refresh_windows();
    });
    assert!(
        timed.update(|window, cx| window.simulate_next_frame(cx)) > 0,
        "a transition in flight does ask for the next frame"
    );
}
