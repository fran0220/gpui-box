use gpui::{TestAppContext, div, prelude::*};
use gpui_kit_testkit::harness::Harness;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[test]
fn back_preview_cancel_commit_and_stale_history_are_distinct() {
    use gpui_kit::navigation::{BackTransition, NavHistory};
    let mut history = NavHistory::new("root");
    assert!(BackTransition::begin(&history).is_none());
    history.push("details");
    let before = history.clone();
    let mut preview = BackTransition::begin(&history).expect("details can go back");
    preview.update(0.73);
    assert_eq!(preview.progress(), 0.73);
    preview.update(f32::NAN);
    assert_eq!(preview.progress(), 0.73);
    preview.update(-0.2);
    assert_eq!(preview.progress(), 0.0);
    preview.update(1.2);
    assert_eq!(preview.progress(), 1.0);
    preview.cancel();
    assert_eq!(history, before);
    let stale = BackTransition::begin(&history).expect("details can go back");
    history.pop();
    history.forward();
    assert!(!stale.commit(&mut history));
    assert_eq!(history.current(), "details");
    assert!(
        BackTransition::begin(&history)
            .expect("details can go back")
            .commit(&mut history)
    );
    assert_eq!(history.current(), "root");
    assert!(history.can_forward());
}

#[gpui::test]
fn cancelled_back_keeps_active_page_and_focus(cx: &mut TestAppContext) {
    use gpui_kit::navigation::{BackTransition, NavHistory, NavStack};
    let mut history = NavHistory::new("root");
    history.push("detail");
    let mut transition = BackTransition::begin(&history).expect("detail can go back");
    transition.update(0.6);
    let transition = Rc::new(RefCell::new(Some(transition)));
    let focus = cx.update(|cx| cx.focus_handle());
    let mut harness = Harness::new(cx, gpui_kit::install, {
        let transition = transition.clone();
        let focus = focus.clone();
        move |_, _| {
            let mut stack = NavStack::new(
                "preview",
                &history,
                "Detail",
                focus.clone(),
                div().child("Detail"),
            );
            if let Some(preview) = transition.borrow().as_ref() {
                stack = stack.back_transition(preview);
            }
            stack.into_any_element()
        }
    });
    harness.update(|window, cx| focus.focus(window, cx));
    assert!(harness.node("preview.root").is_none());
    assert!(harness.node("preview.detail").is_some());
    transition
        .borrow_mut()
        .take()
        .expect("preview is active")
        .cancel();
    harness.update(|_, cx| cx.refresh_windows());
    assert!(harness.update(|window, _| focus.is_focused(window)));
    assert!(harness.node("preview.detail").is_some());
    assert!(harness.node("preview.root").is_none());
}

#[gpui::test]
fn nav_stack_restores_descendant_focus_and_does_not_steal_external_focus(cx: &mut TestAppContext) {
    use gpui_kit::navigation::nav_stack::{NavHistory, NavStack};
    let history = Rc::new(RefCell::new(NavHistory::new("first")));
    let show_child = Rc::new(Cell::new(true));
    let first = cx.update(|cx| cx.focus_handle());
    let child = cx.update(|cx| cx.focus_handle());
    let second = cx.update(|cx| cx.focus_handle());
    let external = cx.update(|cx| cx.focus_handle());
    let mut harness = Harness::new(cx, gpui_kit::install, {
        let history = history.clone();
        let show_child = show_child.clone();
        let first = first.clone();
        let child = child.clone();
        let second = second.clone();
        let external = external.clone();
        move |_, _| {
            let history = history.borrow();
            let is_first = history.current().as_ref() == "first";
            let content = if is_first && show_child.get() {
                div()
                    .id("first-control")
                    .track_focus(&child)
                    .child("First control")
            } else {
                div().id("second-control").child("Second page")
            };
            div()
                .child(NavStack::new(
                    "nav",
                    &history,
                    "Active page",
                    if is_first {
                        first.clone()
                    } else {
                        second.clone()
                    },
                    content,
                ))
                .child(
                    div()
                        .id("external")
                        .track_focus(&external)
                        .child("Outside navigation"),
                )
                .into_any_element()
        }
    });
    harness.update(|window, cx| {
        cx.set_reduce_motion(true);
        child.focus(window, cx);
    });
    harness.update(|_, cx| {
        assert!(history.borrow_mut().push("second"));
        cx.refresh_windows();
    });
    assert!(harness.update(|window, _| second.is_focused(window)));
    assert!(harness.node("nav.first").is_none());
    assert!(harness.node("nav.second").is_some());
    harness.update(|_, cx| {
        assert!(history.borrow_mut().pop());
        cx.refresh_windows();
    });
    assert!(harness.update(|window, _| child.is_focused(window)));
    harness.update(|window, cx| external.focus(window, cx));
    harness.update(|_, cx| {
        assert!(history.borrow_mut().forward());
        cx.refresh_windows();
    });
    assert!(harness.update(|window, _| external.is_focused(window)));
    harness.update(|window, cx| second.focus(window, cx));
    harness.update(|_, cx| {
        show_child.set(false);
        assert!(history.borrow_mut().pop());
        cx.refresh_windows();
    });
    harness.frame();
    assert!(
        harness.update(|window, _| first.is_focused(window)),
        "removed remembered control falls back to the mounted page root"
    );
}
