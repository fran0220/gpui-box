use gpui::{TestAppContext, div, prelude::*};
use gpui_kit_testkit::harness::Harness;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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
