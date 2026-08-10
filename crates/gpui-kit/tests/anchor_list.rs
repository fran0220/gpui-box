//! Synthetic fixtures for caller-owned in-page anchor navigation.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{AppContext as _, Entity, IntoElement, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Calls = Rc<RefCell<Vec<String>>>;

fn fixture() -> Vec<Anchor> {
    vec![
        Anchor::new("summary", "Fixture summary"),
        Anchor::new("inputs", "Fixture inputs"),
        Anchor::new("policy", "Fixture policy").disabled(true),
        Anchor::new("result", "Fixture result"),
    ]
}

fn list(cx: &mut TestAppContext, active: &'static str) -> (Harness, Calls) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = calls.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        AnchorList::new("fixture.anchors")
            .anchors(fixture())
            .active(active)
            .on_navigate(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn click_reports_intent_without_changing_caller_owned_active(cx: &mut TestAppContext) {
    let (mut harness, calls) = list(cx, "inputs");
    harness.click("fixture.anchors.summary");

    assert_eq!(*calls.borrow(), vec!["summary"]);
    assert_eq!(harness.node("fixture.anchors").unwrap().role, Role::List);
    let active = harness.node("fixture.anchors.inputs").unwrap();
    assert_eq!(active.role, Role::Link);
    assert!(active.selected);
    assert_eq!(active.parent.as_deref(), Some("fixture.anchors"));
    assert!(!harness.node("fixture.anchors.summary").unwrap().selected);
}

#[gpui::test]
fn business_ids_stay_stable_when_the_caller_reorders_anchors(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        AnchorList::new("fixture.anchors")
            .anchors([
                Anchor::new("result", "Fixture result"),
                Anchor::new("summary", "Fixture summary"),
            ])
            .active("result")
            .into_any_element()
    });

    assert_eq!(
        harness
            .node("fixture.anchors.result")
            .unwrap()
            .text
            .as_deref(),
        Some("Fixture result")
    );
    assert!(harness.node("fixture.anchors.0").is_none());
}

#[gpui::test]
fn disabled_anchor_and_list_install_no_actions(cx: &mut TestAppContext) {
    let (mut anchor_harness, anchor_calls) = list(cx, "inputs");
    anchor_harness.click("fixture.anchors.policy");
    assert!(anchor_calls.borrow().is_empty());
    assert!(
        anchor_harness
            .node("fixture.anchors.policy")
            .unwrap()
            .disabled
    );

    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = calls.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        AnchorList::new("fixture.anchors")
            .anchors(fixture())
            .active("inputs")
            .disabled(true)
            .on_navigate(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });
    harness.click("fixture.anchors.policy");
    harness.click("fixture.anchors.summary");
    harness.keystrokes("right");

    assert!(calls.borrow().is_empty());
    assert!(harness.node("fixture.anchors.policy").unwrap().disabled);
    assert!(harness.node("fixture.anchors.summary").unwrap().disabled);
}

#[gpui::test]
fn disabling_an_overflowing_list_closes_the_menu_and_keeps_every_anchor_inert(
    cx: &mut TestAppContext,
) {
    let disabled = Rc::new(Cell::new(false));
    let render_disabled = disabled.clone();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = calls.clone();
    let slot: Rc<RefCell<Option<Entity<Menu>>>> = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let sink = sink.clone();
        let menu = build
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| Menu::new("fixture.anchor-menu", window, cx).trigger("More"))
            })
            .clone();
        AnchorList::new("fixture.anchors")
            .anchors(fixture())
            .active("inputs")
            .overflow_after(2)
            .overflow_menu(menu)
            .disabled(render_disabled.get())
            .on_navigate(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });

    harness.click("fixture.anchor-menu.trigger");
    let menu = slot.borrow().clone().expect("overflow menu");
    assert!(menu.read_with(cx, |menu, _| menu.is_open()));

    harness.update(move |window, _| {
        disabled.set(true);
        window.refresh();
    });
    assert!(!menu.read_with(cx, |menu, _| menu.is_open()));
    assert!(harness.node("fixture.anchors.overflow").is_none());
    assert!(harness.node("fixture.anchor-menu.trigger").is_none());
    for id in ["summary", "inputs", "policy", "result"] {
        assert!(
            harness
                .node(&format!("fixture.anchors.{id}"))
                .expect("disabled inline anchor")
                .disabled
        );
    }
    harness.click("fixture.anchors.result");
    harness.keystrokes("right");
    assert!(calls.borrow().is_empty());
}

#[gpui::test]
fn arrows_follow_ltr_and_rtl_reading_order_and_skip_disabled(cx: &mut TestAppContext) {
    let (mut harness, calls) = list(cx, "inputs");
    harness.click("fixture.anchors.inputs");
    calls.borrow_mut().clear();
    harness.keystrokes("right");
    assert_eq!(*calls.borrow(), vec!["result"]);

    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));
    harness.click("fixture.anchors.inputs");
    calls.borrow_mut().clear();
    harness.keystrokes("right");
    assert_eq!(*calls.borrow(), vec!["summary"]);
    calls.borrow_mut().clear();
    harness.keystrokes("left");
    assert_eq!(*calls.borrow(), vec!["result"]);
}

#[gpui::test]
fn home_and_end_choose_the_enabled_edges(cx: &mut TestAppContext) {
    let (mut harness, calls) = list(cx, "inputs");
    harness.click("fixture.anchors.inputs");
    calls.borrow_mut().clear();
    harness.keystrokes("home");
    assert_eq!(*calls.borrow(), vec!["summary"]);
    calls.borrow_mut().clear();
    harness.keystrokes("end");
    assert_eq!(*calls.borrow(), vec!["result"]);
}

#[gpui::test]
fn overflow_without_a_menu_keeps_every_anchor_inline(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        AnchorList::new("fixture.anchors")
            .anchors(fixture())
            .overflow_after(2)
            .into_any_element()
    });
    assert!(harness.node("fixture.anchors.result").is_some());
    assert!(harness.node("fixture.anchors.overflow").is_none());
}

#[gpui::test]
fn overflow_relocates_rows_preserving_ids_state_and_count(cx: &mut TestAppContext) {
    let slot: Rc<RefCell<Option<Entity<Menu>>>> = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let menu = build
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| Menu::new("fixture.anchor-menu", window, cx).trigger("More"))
            })
            .clone();
        AnchorList::new("fixture.anchors")
            .anchors(fixture())
            .active("result")
            .overflow_after(2)
            .overflow_menu(menu)
            .on_navigate(|_, _, _| {})
            .into_any_element()
    });

    assert!(harness.node("fixture.anchors.policy").is_none());
    assert!(harness.node("fixture.anchors.result").is_none());
    assert_eq!(
        harness
            .node("fixture.anchors.overflow")
            .unwrap()
            .value
            .as_deref(),
        Some("2")
    );
    assert_eq!(
        harness.node("fixture.anchors").unwrap().value.as_deref(),
        Some("4")
    );

    harness.click("fixture.anchor-menu.trigger");
    let policy = harness.node("fixture.anchor-menu.policy").unwrap();
    assert_eq!(policy.text.as_deref(), Some("Fixture policy"));
    assert!(policy.disabled);
    let result = harness.node("fixture.anchor-menu.result").unwrap();
    assert_eq!(result.text.as_deref(), Some("Fixture result"));
    assert_eq!(result.checked, Some(true));
}

#[gpui::test]
fn keyboard_walks_into_overflowed_anchors(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = calls.clone();
    let slot: Rc<RefCell<Option<Entity<Menu>>>> = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let sink = sink.clone();
        let menu = build
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| Menu::new("fixture.anchor-menu", window, cx).trigger("More"))
            })
            .clone();
        AnchorList::new("fixture.anchors")
            .anchors(fixture())
            .active("inputs")
            .overflow_after(2)
            .overflow_menu(menu)
            .on_navigate(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });
    harness.click("fixture.anchors.inputs");
    calls.borrow_mut().clear();
    harness.keystrokes("right");
    assert_eq!(*calls.borrow(), vec!["result"]);
}
