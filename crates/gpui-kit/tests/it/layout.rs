//! Split, scroll area, and toolbar report what the typist asked the layout to
//! be. None of them decides that it happened.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Entity, IntoElement, Modifiers, MouseButton, Pixels, Point, SharedString, TestAppContext, div,
    prelude::*, px,
};
use gpui_kit::prelude::*;
use gpui_kit_assets::Icon;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Calls<T> = Rc<RefCell<Vec<T>>>;

fn recorder<T: 'static>() -> (Calls<T>, Calls<T>) {
    let calls: Calls<T> = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

/// Delivers the frame that publishes what the first one measured.
///
/// A divider's travel and a scrollbar's reach are extents only layout knows,
/// so the frame that measures them asks for one more to publish them. Tests
/// have no platform frame loop, so they deliver it themselves.
fn settle(harness: &mut Harness) {
    harness.advance(std::time::Duration::from_millis(16));
}

/// A pane with enough copy in it to overflow a short viewport.
fn pane(title: &'static str, lines: usize) -> gpui::Div {
    let mut content = div().flex().flex_col().child(title);
    for line in 0..lines {
        content = content.child(SharedString::from(format!("line {line}")));
    }
    content
}

// ---------------------------------------------------------------- split pane

struct SplitCase {
    harness: Harness,
    resizes: Calls<f32>,
    collapses: Calls<SplitSide>,
}

fn split(cx: &mut TestAppContext, ratio: f32, frozen: bool) -> SplitCase {
    split_axis(cx, ratio, frozen, SplitAxis::Horizontal)
}

fn split_axis(cx: &mut TestAppContext, ratio: f32, frozen: bool, axis: SplitAxis) -> SplitCase {
    let (resizes, resize_sink) = recorder::<f32>();
    let (collapses, collapse_sink) = recorder::<SplitSide>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let resize_sink = resize_sink.clone();
        let collapse_sink = collapse_sink.clone();
        div()
            .w(px(600.0))
            .h(px(400.0))
            .child(
                SplitPane::new("workspace.split")
                    .axis(axis)
                    .ratio(ratio)
                    .min_sizes(150.0, 150.0)
                    .collapsible(true)
                    .step(60.0)
                    .disabled(frozen)
                    .start(pane("Files", 2))
                    .end(pane("Editor", 2))
                    .on_resize(move |next, _, _| resize_sink.borrow_mut().push(next))
                    .on_collapse(move |side, _, _| collapse_sink.borrow_mut().push(side)),
            )
            .into_any_element()
    });
    settle(&mut harness);
    SplitCase {
        harness,
        resizes,
        collapses,
    }
}

/// Presses on the divider, moves to `x`, and releases, the way a drag runs.
fn drag_divider(harness: &mut Harness, x: f32) {
    let start = harness.point_in("workspace.split.divider");
    let target: Point<Pixels> = gpui::point(px(x), start.y);
    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    harness.context().run_until_parked();
}

#[gpui::test]
fn pointer_cancel_clears_split_resize_and_the_next_press_works(cx: &mut TestAppContext) {
    let mut case = split(cx, 0.5, false);
    let start = case.harness.point_in("workspace.split.divider");
    case.harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    case.harness
        .context()
        .simulate_event(gpui::MouseCancelEvent);
    case.harness
        .context()
        .simulate_event(gpui::MouseCancelEvent);
    case.harness.context().simulate_mouse_move(
        gpui::point(px(400.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    case.harness
        .context()
        .simulate_mouse_up(start, MouseButton::Left, Modifiers::none());
    assert!(case.resizes.borrow().is_empty());
    drag_divider(&mut case.harness, 400.0);
    assert!(!case.resizes.borrow().is_empty());
}

#[gpui::test]
fn dragging_the_divider_reports_a_ratio_without_moving_the_panes(cx: &mut TestAppContext) {
    let mut case = split(cx, 0.5, false);

    drag_divider(&mut case.harness, 400.0);

    let reported = *case.resizes.borrow().last().expect("a ratio was reported");
    assert!(
        (reported - 2.0 / 3.0).abs() < 0.02,
        "a drag two thirds across reports two thirds, got {reported}"
    );
    // The caller owns the ratio, so the split still draws the one it was given.
    assert_eq!(
        case.harness
            .node("workspace.split")
            .expect("published")
            .value
            .as_deref(),
        Some("0.500")
    );
}

#[gpui::test]
fn a_pane_at_its_minimum_stops_instead_of_reporting_past_it(cx: &mut TestAppContext) {
    let mut case = split(cx, 0.5, false);

    drag_divider(&mut case.harness, 5.0);

    let reported = *case.resizes.borrow().last().expect("a ratio was reported");
    // 150px of 600px is a quarter, and nothing may be reported below it.
    assert!(
        (reported - 0.25).abs() < 0.01,
        "a drag past the minimum reports the minimum, got {reported}"
    );
}

#[gpui::test]
fn the_divider_publishes_the_room_the_minimums_leave(cx: &mut TestAppContext) {
    let mut case = split(cx, 0.5, false);
    let divider = case
        .harness
        .node("workspace.split.divider")
        .expect("published");

    assert_eq!(divider.role, Role::Splitter);
    assert_eq!(divider.parent.as_deref(), Some("workspace.split"));
    let low = divider.value_min.expect("a separator carries its range");
    let high = divider.value_max.expect("a separator carries its range");
    let now = divider.value_now.expect("a separator carries its range");
    assert!((low - 0.25).abs() < 0.01, "low was {low}");
    assert!((high - 0.75).abs() < 0.01, "high was {high}");
    assert!((now - 0.5).abs() < 0.01, "now was {now}");
}

#[gpui::test]
fn native_splitter_contract_matches_axis_and_dispatches_resize(cx: &mut TestAppContext) {
    for (axis, expected, low, high, next) in [
        (SplitAxis::Horizontal, "Vertical", 0.25, 0.75, 0.6),
        (SplitAxis::Vertical, "Horizontal", 0.375, 0.625, 0.625),
    ] {
        let mut case = split_axis(cx, 0.5, false, axis);
        let tree = case.harness.accessibility_tree();
        let splitter = tree["nodes"]
            .as_object()
            .and_then(|nodes| {
                nodes
                    .values()
                    .find(|node| node["element_id"] == "Name(\"workspace.split.divider\")")
            })
            .unwrap_or_else(|| panic!("stable native splitter id: {tree}"));
        assert_eq!(splitter["aria"]["role"], "Splitter");
        assert_eq!(splitter["aria"]["orientation"], expected);
        assert_eq!(splitter["aria"]["min_numeric_value"], low);
        assert_eq!(splitter["aria"]["max_numeric_value"], high);
        assert_eq!(splitter["aria"]["numeric_value"], 0.5);
        let actions = splitter["aria"]["on_action"].as_array().expect("actions");
        assert!(actions.iter().any(|action| action == "Increment"));
        assert!(actions.iter().any(|action| action == "Decrement"));
        let node_id = splitter["accesskit_id"]
            .as_str()
            .expect("raw node id")
            .parse()
            .expect("numeric node id");
        let window = case.harness.window();
        case.harness.context().dispatch_accessibility_action(
            window,
            gpui::accesskit::ActionRequest {
                action: gpui::accesskit::Action::Increment,
                target_tree: gpui::accesskit::TreeId::ROOT,
                target_node: gpui::accesskit::NodeId(node_id),
                data: None,
            },
        );
        let reported = *case.resizes.borrow().last().expect("resize report");
        assert!((reported - next).abs() < 0.01, "reported {reported}");
    }
}

#[gpui::test]
fn arrow_keys_move_the_divider_and_home_and_end_reach_the_limits(cx: &mut TestAppContext) {
    let mut case = split(cx, 0.5, false);

    case.harness.click("workspace.split.divider");
    case.harness.keystrokes("right");
    case.harness.keystrokes("home");
    case.harness.keystrokes("end");

    let reported = case.resizes.borrow().clone();
    assert_eq!(reported.len(), 3, "three keystrokes, three reports");
    assert!(
        (reported[0] - 0.6).abs() < 0.01,
        "a 60px step of 600px is a tenth, got {}",
        reported[0]
    );
    assert!(
        (reported[1] - 0.25).abs() < 0.01,
        "home reaches the low limit"
    );
    assert!(
        (reported[2] - 0.75).abs() < 0.01,
        "end reaches the high limit"
    );
}

#[gpui::test]
fn a_double_click_reports_collapsing_the_smaller_pane(cx: &mut TestAppContext) {
    let mut case = split(cx, 0.3, false);
    let center = case.harness.point_in("workspace.split.divider");

    case.harness.context().simulate_event(gpui::MouseDownEvent {
        position: center,
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    case.harness.context().simulate_event(gpui::MouseUpEvent {
        position: center,
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 2,
    });
    case.harness.context().run_until_parked();

    assert_eq!(*case.collapses.borrow(), vec![SplitSide::Start]);
}

#[gpui::test]
fn a_frozen_split_reports_nothing(cx: &mut TestAppContext) {
    let mut case = split(cx, 0.5, true);

    drag_divider(&mut case.harness, 400.0);
    case.harness.click("workspace.split.divider");
    case.harness.keystrokes("right");

    assert!(case.resizes.borrow().is_empty());
    assert!(case.collapses.borrow().is_empty());
    assert!(
        case.harness
            .node("workspace.split.divider")
            .expect("published")
            .disabled
    );
}

#[gpui::test]
fn a_pane_with_no_share_of_the_split_is_not_drawn(cx: &mut TestAppContext) {
    let mut case = split(cx, 0.0, false);

    assert!(
        case.harness.node("workspace.split.start").is_none(),
        "a collapsed pane leaves nothing addressable behind"
    );
    assert!(case.harness.node("workspace.split.end").is_some());
}

// --------------------------------------------------------------- scroll area

fn scroll(cx: &mut TestAppContext, lines: usize) -> Harness {
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(400.0))
            .child(
                ScrollArea::new("run.output")
                    .label("Run output")
                    .vertical()
                    .height(120.0)
                    .child(pane("Output", lines)),
            )
            .into_any_element()
    });
    settle(&mut harness);
    harness
}

#[gpui::test]
fn a_region_with_nothing_more_publishes_no_scrollbar(cx: &mut TestAppContext) {
    let mut harness = scroll(cx, 1);

    assert!(harness.node("run.output").is_some());
    assert!(
        harness.node("run.output.scrollbar.vertical").is_none(),
        "an absent scrollbar is how a test reads that there is nothing more"
    );
}

#[gpui::test]
fn a_region_with_more_off_screen_publishes_how_far_it_reaches(cx: &mut TestAppContext) {
    let mut harness = scroll(cx, 60);
    let bar = harness
        .node("run.output.scrollbar.vertical")
        .expect("a scrollbar is published while there is more to reach");

    assert_eq!(bar.role, Role::Scrollbar);
    assert_eq!(bar.parent.as_deref(), Some("run.output"));
    let low = bar.value_min.expect("a scrollbar carries its range");
    let high = bar.value_max.expect("a scrollbar carries its range");
    let now = bar.value_now.expect("a scrollbar carries its range");
    assert_eq!(low, 0.0);
    assert!(high > 0.0, "there is more below than the viewport shows");
    assert_eq!(now, 0.0, "nothing has scrolled yet");
}

#[gpui::test]
fn the_scrollbar_does_not_cover_the_content_it_reports(cx: &mut TestAppContext) {
    let mut harness = scroll(cx, 60);
    let content = harness.bounds("run.output.content").expect("published");
    let bar = harness
        .bounds("run.output.scrollbar.vertical")
        .expect("published");

    assert!(
        content.right() <= bar.left() + px(0.5),
        "the gutter is reserved beside the content, not over it"
    );
}

// -------------------------------------------------------------------- toolbar

fn item(id: &'static str, label: &'static str, glyph: Icon) -> ToolbarItem {
    ToolbarItem::new(
        id,
        label,
        Button::new(format!("editor.toolbar.{id}"))
            .label(label)
            .ghost()
            .small()
            .on_click(|_, _| {}),
    )
    .icon(glyph)
}

fn toolbar(cx: &mut TestAppContext, limit: Option<usize>, with_menu: bool) -> Harness {
    toolbar_at(cx, 600.0, limit, with_menu)
}

fn toolbar_at(
    cx: &mut TestAppContext,
    width: f32,
    limit: Option<usize>,
    with_menu: bool,
) -> Harness {
    let slot: Rc<RefCell<Option<Entity<Menu>>>> = Rc::new(RefCell::new(None));
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let menu = slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| Menu::new("editor.overflow", window, cx).trigger("More"))
            })
            .clone();
        let mut bar = Toolbar::new("editor.toolbar")
            .label("Editor actions")
            .group(
                "actions",
                [
                    item("editor.undo", "Undo", Icon::ArrowLeft),
                    item("editor.redo", "Redo", Icon::ArrowRight),
                    item("editor.share", "Share", Icon::Copy),
                    item("editor.publish", "Publish", Icon::ArchiveUp),
                ],
            );
        if let Some(limit) = limit {
            bar = bar.overflow_after(limit);
        }
        if with_menu {
            bar = bar.overflow_menu(menu);
        }
        div().w(px(width)).child(bar).into_any_element()
    });
    harness.snapshot();
    harness
}

#[gpui::test]
fn every_action_that_fits_is_drawn_and_the_rest_move_to_the_menu(cx: &mut TestAppContext) {
    let mut harness = toolbar(cx, Some(2), true);

    assert!(harness.node("editor.toolbar.editor.undo").is_some());
    assert!(harness.node("editor.toolbar.editor.redo").is_some());
    assert!(
        harness.node("editor.toolbar.editor.share").is_none(),
        "an overflowed action is not drawn twice"
    );
    let trigger = harness
        .node("editor.toolbar.overflow")
        .expect("an overflow trigger is published while actions are hidden");
    assert_eq!(
        trigger.value.as_deref(),
        Some("2"),
        "the trigger says how many actions it stands for"
    );
}

#[gpui::test]
fn an_overflowed_action_is_still_reachable_from_the_menu(cx: &mut TestAppContext) {
    let mut harness = toolbar(cx, Some(2), true);

    harness.click("editor.overflow.trigger");

    let row = harness
        .node("editor.overflow.editor.share")
        .expect("an overflowed action is offered by the menu");
    assert_eq!(row.text.as_deref(), Some("Share"));
}

#[gpui::test]
fn with_nowhere_to_move_them_every_action_stays_on_the_bar(cx: &mut TestAppContext) {
    let mut harness = toolbar(cx, Some(2), false);

    // Without a menu to hold them, hiding actions would lose them, so the
    // toolbar keeps drawing every one.
    assert!(harness.node("editor.toolbar.editor.share").is_some());
    assert!(harness.node("editor.toolbar.editor.publish").is_some());
    assert!(harness.node("editor.toolbar.overflow").is_none());
}

/// With no declared cut, the bar draws everything on the frame that produces
/// the measurements and cuts on the next one. The items are moved into the
/// menu, never dropped, which is what the trigger's count has to agree with.
#[gpui::test]
fn a_bar_with_no_room_measures_its_own_cut(cx: &mut TestAppContext) {
    let mut harness = toolbar_at(cx, 96.0, None, true);

    harness.frame();
    harness.snapshot();

    let trigger = harness
        .node("editor.toolbar.overflow")
        .expect("a bar too narrow for its actions publishes a trigger without being told to");
    let hidden: usize = trigger
        .value
        .as_deref()
        .expect("the trigger says how many actions it stands for")
        .parse()
        .expect("a count");
    assert!(hidden > 0, "a 96px bar cannot hold four controls");

    let drawn = [
        "editor.undo",
        "editor.redo",
        "editor.share",
        "editor.publish",
    ]
    .into_iter()
    .filter(|id| harness.node(&format!("editor.toolbar.{id}")).is_some())
    .count();
    assert_eq!(
        drawn + hidden,
        4,
        "every action is either drawn or in the menu, and never both or neither"
    );
}

#[gpui::test]
fn a_bar_with_room_to_spare_cuts_nothing(cx: &mut TestAppContext) {
    let mut harness = toolbar_at(cx, 900.0, None, true);

    harness.frame();
    harness.snapshot();

    assert!(
        harness.node("editor.toolbar.overflow").is_none(),
        "a bar that fits publishes no trigger"
    );
    assert!(harness.node("editor.toolbar.editor.publish").is_some());
}

#[gpui::test]
fn a_toolbar_publishes_how_many_actions_it_holds(cx: &mut TestAppContext) {
    let mut harness = toolbar(cx, None, true);
    let bar = harness.node("editor.toolbar").expect("published");

    assert_eq!(bar.role, Role::Toolbar);
    assert_eq!(bar.text.as_deref(), Some("Editor actions"));
    assert_eq!(bar.value.as_deref(), Some("4"));
}

// ---------------------------------------------------------------- scroll fade

/// A faded region wrapping a bounded scroll area, told which edges hide
/// something.
fn scroll_fade(cx: &mut TestAppContext, edges: FadeEdges) -> Harness {
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(400.0))
            .h(px(240.0))
            .child(
                ScrollFade::new("run.output.fade").edges(edges).child(
                    ScrollArea::new("run.output")
                        .label("Run output")
                        .vertical()
                        .height(200.0)
                        .child(pane("Output", 40)),
                ),
            )
            .into_any_element()
    });
    settle(&mut harness);
    harness
}

#[gpui::test]
fn a_faded_region_publishes_which_edges_hide_something(cx: &mut TestAppContext) {
    let mut harness = scroll_fade(cx, FadeEdges::vertical());
    let region = harness.node("run.output.fade").expect("published");

    assert_eq!(region.role, Role::Region);
    assert_eq!(region.value.as_deref(), Some("top bottom"));
    assert!(
        harness.node("run.output").is_some(),
        "a fade wraps the region it fades and hides nothing from the tree"
    );
}

#[gpui::test]
fn a_region_that_hides_nothing_says_no_edge_fades(cx: &mut TestAppContext) {
    let mut harness = scroll_fade(cx, FadeEdges::default());
    let region = harness.node("run.output.fade").expect("published");

    // An absent fade is a statement — there is no more content past either
    // end — so it is published rather than left to be inferred from pixels.
    assert_eq!(region.value.as_deref(), Some("none"));
}
