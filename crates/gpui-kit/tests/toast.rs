//! Notifications report what happened. A failure is never hidden on a timer,
//! a toast under the pointer keeps its time, and one that leaves finishes its
//! exit before it is dropped from the tree.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{AppContext as _, Entity, Modifiers, MouseButton, TestAppContext, div, prelude::*, px};
use gpui_kit::motion::Phase;
use gpui_kit::overlay::toast;
use gpui_kit::prelude::*;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_testkit::harness::Harness;

/// Longer than any entry or exit animation in the bundled themes.
const SETTLE: Duration = Duration::from_millis(400);

#[gpui::test]
fn a_static_status_is_not_a_live_region(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, cx| {
        div()
            .child("3 selected")
            .semantic_in(
                cx,
                NodeSpec::new("selection.count", Role::Status).text("3 selected"),
            )
            .into_any_element()
    });
    let tree = harness.accessibility_tree();
    let status = tree["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"selection.count\")"
                    && node["aria"]["role"] == "Status"
            })
        })
        .expect("static native status");
    assert!(status["aria"].get("live").is_none());
}

fn scene(cx: &mut TestAppContext, capacity: usize) -> (Harness, Entity<ToastLayer>) {
    let slot: Rc<RefCell<Option<Entity<ToastLayer>>>> = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_window, cx| {
        let layer = build
            .borrow_mut()
            .get_or_insert_with(|| cx.new(|cx| ToastLayer::new(cx).capacity(capacity)))
            .clone();
        div().size_full().child(layer).into_any_element()
    });
    harness.snapshot();
    let layer = slot.borrow().clone().expect("a layer was mounted");
    (harness, layer)
}

fn push(harness: &mut Harness, toast: Toast) -> bool {
    harness.update(move |_, cx| toast::push(cx, toast))
}

fn phase(harness: &mut Harness, layer: &Entity<ToastLayer>, ident: &str) -> Option<Phase> {
    let layer = layer.clone();
    let ident = ident.to_string();
    harness.update(move |_, cx| layer.read(cx).phase(&ident))
}

fn hover(harness: &mut Harness, ident: &str) {
    let point = harness.point_in(ident);
    harness
        .context()
        .simulate_mouse_move(point, None::<MouseButton>, Modifiers::none());
    harness.context().run_until_parked();
}

fn hover_away(harness: &mut Harness) {
    harness.context().simulate_mouse_move(
        gpui::point(px(2.0), px(2.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    harness.context().run_until_parked();
}

#[gpui::test]
fn a_pushed_toast_is_published_with_its_message_and_tone(cx: &mut TestAppContext) {
    let (mut harness, _layer) = scene(cx, 3);

    assert!(push(
        &mut harness,
        Toast::new("run.published", "The run was published")
            .tone(Tone::Success)
            .detail("Three artifacts were written.")
            .action("Show", |_, _| {})
    ));

    let node = harness.node("run.published").expect("published");
    assert_eq!(node.role, Role::Toast);
    assert_eq!(node.text.as_deref(), Some("The run was published"));
    assert_eq!(node.value.as_deref(), Some("success"));
    assert_eq!(node.live, Some(gpui_kit_semantics::LiveRegion::Polite));
    assert!(node.live_atomic);
    assert!(node.visible);

    let tree = harness.accessibility_tree();
    let native = tree["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"run.published\")" && node["aria"]["role"] == "Status"
            })
        })
        .expect("native live status");
    assert_eq!(native["aria"]["live"], "Polite");
    assert_eq!(native["aria"]["live_atomic"], true);

    let snapshot = harness.snapshot();
    let children: Vec<&str> = snapshot
        .children_of("run.published")
        .iter()
        .map(|node| node.id.as_str())
        .collect();
    assert!(children.contains(&"run.published.action"));
    assert!(children.contains(&"run.published.dismiss"));
    assert!(children.contains(&"run.published.detail"));
    assert_eq!(
        snapshot
            .find("run.published.action")
            .expect("published")
            .role,
        Role::Button
    );
}

#[gpui::test]
fn a_danger_toast_is_an_assertive_live_region(cx: &mut TestAppContext) {
    let (mut harness, _layer) = scene(cx, 3);
    assert!(push(
        &mut harness,
        Toast::new("run.failed", "Publishing failed").tone(Tone::Danger)
    ));
    let node = harness.node("run.failed").expect("published");
    assert_eq!(node.live, Some(gpui_kit_semantics::LiveRegion::Assertive));
    let tree = harness.accessibility_tree();
    let native = tree["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"run.failed\")" && node["aria"]["role"] == "Status"
            })
        })
        .expect("assertive native status");
    assert_eq!(native["aria"]["live"], "Assertive");
}

#[gpui::test]
fn updating_one_toast_updates_the_explicit_live_node(cx: &mut TestAppContext) {
    let (mut harness, _layer) = scene(cx, 3);
    assert!(push(
        &mut harness,
        Toast::new("run.progress", "Uploading one file")
    ));
    let before = harness.accessibility_tree();
    let before = before["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"run.progress\")" && node["aria"]["role"] == "Status"
            })
        })
        .expect("initial explicit live node");
    assert_eq!(before["aria"]["label"], "Uploading one file");
    assert_eq!(before["aria"]["live"], "Polite");

    assert!(push(
        &mut harness,
        Toast::new("run.progress", "Upload complete")
    ));
    let after = harness.accessibility_tree();
    let after = after["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"run.progress\")" && node["aria"]["role"] == "Status"
            })
        })
        .expect("updated explicit live node");
    assert_eq!(after["aria"]["label"], "Upload complete");
    assert_eq!(after["aria"]["live"], "Polite");
}

#[gpui::test]
fn a_push_with_no_layer_mounted_reports_that_it_went_nowhere(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_window, _cx| {
        div().size_full().into_any_element()
    });
    harness.snapshot();

    let delivered = harness.update(|_, cx| {
        toast::push(
            cx,
            Toast::new("run.published", "The run was published").tone(Tone::Success),
        )
    });

    assert!(
        !delivered,
        "a library must not pick a window it was not given"
    );
    assert!(harness.node("run.published").is_none());
}

#[gpui::test]
fn a_timed_toast_leaves_after_its_timeout_and_not_before(cx: &mut TestAppContext) {
    let (mut harness, layer) = scene(cx, 3);
    push(
        &mut harness,
        Toast::new("run.published", "The run was published")
            .tone(Tone::Success)
            .timeout(Duration::from_millis(400)),
    );

    harness.advance(Duration::from_millis(300));
    assert!(
        harness.node("run.published").is_some(),
        "the toast left before its time was up"
    );

    harness.advance(Duration::from_millis(200));
    assert_eq!(
        phase(&mut harness, &layer, "run.published"),
        Some(Phase::Exiting)
    );

    harness.advance(SETTLE);
    assert!(harness.node("run.published").is_none());
}

#[gpui::test]
fn a_failure_is_never_hidden_on_a_timer(cx: &mut TestAppContext) {
    let (mut harness, _layer) = scene(cx, 3);
    push(
        &mut harness,
        Toast::new("run.refused", "The host refused to publish this run").tone(Tone::Danger),
    );
    push(
        &mut harness,
        Toast::new("catalog.stale", "Refreshing the catalog failed").tone(Tone::Warning),
    );

    for _ in 0..10 {
        harness.advance(Duration::from_secs(2));
    }

    assert!(
        harness.node("run.refused").is_some(),
        "a failure the typist did not see is a failure that was not reported"
    );
    assert!(harness.node("catalog.stale").is_some());
}

#[gpui::test]
fn a_pointer_resting_on_a_toast_pauses_its_timer(cx: &mut TestAppContext) {
    let (mut harness, _layer) = scene(cx, 3);
    push(
        &mut harness,
        Toast::new("run.published", "The run was published")
            .tone(Tone::Success)
            .timeout(Duration::from_millis(300))
            .action("Show", |_, _| {}),
    );

    hover(&mut harness, "run.published");
    assert!(harness.node("run.published").expect("published").hovered);
    harness.advance(Duration::from_millis(600));
    harness.advance(Duration::from_millis(600));
    assert!(
        harness.node("run.published").is_some(),
        "a toast under the pointer must keep its time"
    );

    hover_away(&mut harness);
    harness.advance(Duration::from_millis(400));
    harness.advance(SETTLE);
    assert!(harness.node("run.published").is_none());
}

#[gpui::test]
fn the_action_reports_and_then_dismisses(cx: &mut TestAppContext) {
    let (mut harness, layer) = scene(cx, 3);
    let taken = Rc::new(RefCell::new(0_usize));
    let sink = taken.clone();
    push(
        &mut harness,
        Toast::new("run.refused", "The host refused to publish this run")
            .tone(Tone::Danger)
            .action("Try again", move |_, _| *sink.borrow_mut() += 1),
    );
    harness.advance(SETTLE);

    harness.click("run.refused.action");

    assert_eq!(*taken.borrow(), 1, "the action reports to its caller");
    assert_eq!(
        phase(&mut harness, &layer, "run.refused"),
        Some(Phase::Exiting)
    );
    harness.advance(SETTLE);
    assert!(harness.node("run.refused").is_none());
}

#[gpui::test]
fn a_dismissed_toast_finishes_its_exit_before_it_leaves_the_tree(cx: &mut TestAppContext) {
    let (mut harness, layer) = scene(cx, 3);
    push(
        &mut harness,
        Toast::new("run.published", "The run was published").tone(Tone::Success),
    );
    harness.advance(SETTLE);
    assert_eq!(
        phase(&mut harness, &layer, "run.published"),
        Some(Phase::Present)
    );

    harness.click("run.published.dismiss");

    assert_eq!(
        phase(&mut harness, &layer, "run.published"),
        Some(Phase::Exiting)
    );
    assert!(
        harness.node("run.published").is_some(),
        "the element must survive its own exit"
    );

    harness.advance(SETTLE);
    assert_eq!(phase(&mut harness, &layer, "run.published"), None);
    assert!(harness.node("run.published").is_none());
}

#[gpui::test]
fn reduced_motion_shows_and_removes_a_toast_in_one_frame(cx: &mut TestAppContext) {
    let (mut harness, layer) = scene(cx, 3);
    harness.update(|_, cx| cx.set_reduce_motion(true));
    push(
        &mut harness,
        Toast::new("run.published", "The run was published").tone(Tone::Success),
    );

    assert_eq!(
        phase(&mut harness, &layer, "run.published"),
        Some(Phase::Present)
    );

    harness.update(|_, cx| toast::dismiss(cx, "run.published"));
    harness.advance(Duration::ZERO);
    assert!(harness.node("run.published").is_none());
}

#[gpui::test]
fn the_stack_evicts_the_oldest_dismissable_toast_and_never_a_persistent_one(
    cx: &mut TestAppContext,
) {
    let (mut harness, layer) = scene(cx, 2);
    push(
        &mut harness,
        Toast::new("run.refused", "The host refused to publish this run").tone(Tone::Danger),
    );
    push(
        &mut harness,
        Toast::new("run.indexed", "Indexing finished").tone(Tone::Success),
    );
    harness.advance(SETTLE);

    push(
        &mut harness,
        Toast::new("run.exported", "Theme exported").tone(Tone::Success),
    );

    assert_eq!(
        phase(&mut harness, &layer, "run.indexed"),
        Some(Phase::Exiting),
        "the oldest dismissable toast makes room"
    );
    assert_eq!(
        phase(&mut harness, &layer, "run.refused"),
        Some(Phase::Present),
        "a failure is never evicted to make room"
    );
    harness.advance(SETTLE);
    assert!(harness.node("run.indexed").is_none());
    assert!(harness.node("run.refused").is_some());
    assert!(harness.node("run.exported").is_some());

    push(
        &mut harness,
        Toast::new("run.archived", "Archiving finished").tone(Tone::Success),
    );
    harness.advance(SETTLE);
    assert!(harness.node("run.exported").is_none());
    assert!(harness.node("run.refused").is_some());
    assert!(harness.node("run.archived").is_some());
}
