//! Motion driven through a real window, where frames and the clock are
//! simulated rather than assumed.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{TestAppContext, div, prelude::*, px};
use gpui_kit::motion::{
    CubicBezier, Flipping, MotionSpec, Presence, Transition, flip, tracked_ids,
};
use gpui_kit::prelude::*;
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
