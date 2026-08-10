//! Cascader reports navigation and choices without taking ownership of data.

use std::{cell::RefCell, rc::Rc};

use gpui::{AppContext as _, Entity, IntoElement, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

fn options() -> Vec<CascaderOption> {
    vec![
        CascaderOption::new("synthetic.branch", "Synthetic branch").children([
            CascaderOption::new("synthetic.leaf-a", "Synthetic leaf A"),
            CascaderOption::new("synthetic.disabled", "Synthetic disabled").disabled(true),
            CascaderOption::new("synthetic.leaf-b", "Synthetic leaf B"),
        ]),
        CascaderOption::new("synthetic.loading", "Synthetic loading").loading_children(),
        CascaderOption::new("synthetic.idle", "Synthetic idle").idle_children(),
        CascaderOption::new("synthetic.empty", "Synthetic empty").empty_children(),
        CascaderOption::new("synthetic.unavailable", "Synthetic unavailable")
            .unavailable_children("Synthetic host refusal"),
        CascaderOption::new("synthetic.error", "Synthetic error")
            .error_children("Synthetic request failure"),
    ]
}

fn cascader(cx: &mut TestAppContext) -> (Harness, Entity<Cascader>) {
    let slot = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        build
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    Cascader::new("synthetic.cascader", window, cx)
                        .name("Synthetic destination")
                        .placeholder("Synthetic choice")
                        .options(options())
                        .selected("synthetic.leaf-a")
                })
            })
            .clone()
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("cascader built");
    (harness, entity)
}

fn events(harness: &mut Harness, entity: &Entity<Cascader>) -> Rc<RefCell<Vec<CascaderEvent>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    let entity = entity.clone();
    harness.update(move |_, cx| {
        cx.subscribe(&entity, move |_, event: &CascaderEvent, _| {
            sink.borrow_mut().push(event.clone())
        })
        .detach();
    });
    seen
}

#[gpui::test]
fn identity_expansion_and_selection_remain_caller_owned(cx: &mut TestAppContext) {
    let (mut harness, entity) = cascader(cx);
    let seen = events(&mut harness, &entity);
    harness.click("synthetic.cascader");
    harness.click("synthetic.cascader.synthetic.branch");

    assert!(
        harness
            .node("synthetic.cascader.synthetic.leaf-a")
            .is_some()
    );
    assert_eq!(
        entity.read_with(cx, |view, _| view.open_path().to_vec()),
        vec!["synthetic.branch"]
    );
    assert!(
        seen.borrow()
            .contains(&CascaderEvent::Expanded("synthetic.branch".into()))
    );

    harness.click("synthetic.cascader.synthetic.leaf-b");
    assert!(
        seen.borrow()
            .contains(&CascaderEvent::Selected("synthetic.leaf-b".into()))
    );
    assert_eq!(
        entity.read_with(cx, |view, _| view.selected_id().cloned()),
        Some("synthetic.leaf-a".into())
    );
    assert_eq!(
        harness
            .node("synthetic.cascader")
            .expect("trigger")
            .value
            .as_deref(),
        Some("Synthetic leaf A")
    );
}

#[gpui::test]
fn async_branch_states_are_distinct_and_retry_is_an_intent(cx: &mut TestAppContext) {
    let (mut harness, entity) = cascader(cx);
    let seen = events(&mut harness, &entity);
    harness.click("synthetic.cascader");

    harness.click("synthetic.cascader.synthetic.loading");
    assert!(
        harness
            .node("synthetic.cascader.synthetic.loading.state")
            .expect("loading")
            .busy
    );
    harness.click("synthetic.cascader.synthetic.idle");
    assert_eq!(
        harness
            .node("synthetic.cascader.synthetic.idle.state")
            .expect("idle")
            .value
            .as_deref(),
        Some("unstarted")
    );
    harness.click("synthetic.cascader.synthetic.empty");
    assert_eq!(
        harness
            .node("synthetic.cascader.synthetic.empty.state")
            .expect("empty")
            .value
            .as_deref(),
        Some("empty")
    );
    harness.click("synthetic.cascader.synthetic.unavailable");
    assert_eq!(
        harness
            .node("synthetic.cascader.synthetic.unavailable.state")
            .expect("unavailable")
            .value
            .as_deref(),
        Some("unavailable")
    );
    harness.click("synthetic.cascader.synthetic.unavailable.state.retry");
    assert!(
        seen.borrow()
            .contains(&CascaderEvent::Retry("synthetic.unavailable".into()))
    );
    harness.click("synthetic.cascader.synthetic.error");
    assert_eq!(
        harness
            .node("synthetic.cascader.synthetic.error.state")
            .expect("error")
            .value
            .as_deref(),
        Some("failed")
    );
}

#[gpui::test]
fn disabled_rows_and_disabled_trigger_install_no_actions(cx: &mut TestAppContext) {
    let (mut harness, entity) = cascader(cx);
    let seen = events(&mut harness, &entity);
    harness.click("synthetic.cascader");
    harness.click("synthetic.cascader.synthetic.branch");
    harness.click("synthetic.cascader.synthetic.disabled");
    assert!(
        !seen
            .borrow()
            .iter()
            .any(|event| matches!(event, CascaderEvent::Selected(_)))
    );
    assert!(
        harness
            .node("synthetic.cascader.synthetic.disabled")
            .expect("row")
            .disabled
    );

    harness.update(move |_, cx| entity.update(cx, |view, cx| view.set_disabled(true, cx)));
    let tree = harness.accessibility_tree();
    let trigger = tree["nodes"]
        .as_object()
        .unwrap()
        .values()
        .find(|node| node["element_id"] == "Name(\"synthetic.cascader\")")
        .expect("native trigger");
    let actions = trigger["aria"]["on_action"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        !actions
            .iter()
            .any(|action| action == "Focus" || action == "Click")
    );
}

#[gpui::test]
fn keyboard_moves_expands_backs_up_and_swaps_horizontal_arrows_in_rtl(cx: &mut TestAppContext) {
    let (mut harness, entity) = cascader(cx);
    harness.click("synthetic.cascader");
    harness.keystrokes("right");
    assert_eq!(
        entity.read_with(cx, |view, _| view.open_path().to_vec()),
        vec!["synthetic.branch"]
    );
    harness.keystrokes("end enter");
    assert_eq!(
        entity.read_with(cx, |view, _| view.selected_id().cloned()),
        Some("synthetic.leaf-a".into())
    );

    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));
    harness.click("synthetic.cascader");
    harness.keystrokes("left");
    assert_eq!(
        entity.read_with(cx, |view, _| view.open_path().to_vec()),
        vec!["synthetic.branch"]
    );
    harness.keystrokes("right escape");
    assert!(!entity.read_with(cx, |view, _| view.is_open()));
}
