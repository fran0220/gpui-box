//! A dialog asks one question, keeps the keyboard while it is open, and
//! reports the answer. It never decides anything itself.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, FocusHandle, TestAppContext, div, prelude::*, px};
use gpui_kit::prelude::*;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_testkit::harness::Harness;

struct Fixture {
    dialog: Entity<Dialog>,
    trigger: FocusHandle,
}

fn scene(
    cx: &mut TestAppContext,
    dismissable: bool,
    destructive: bool,
) -> (Harness, Entity<Dialog>, FocusHandle) {
    let slot: Rc<RefCell<Option<Rc<Fixture>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let fixture = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                Rc::new(Fixture {
                    dialog: cx.new(|cx| {
                        Dialog::new("workspace.delete", window, cx)
                            .title("Delete this workspace?")
                            .description("Everything in it is removed from this machine.")
                            .content(|_, cx| {
                                div()
                                    .w(px(200.0))
                                    .h(px(20.0))
                                    .child("Nothing else is affected.")
                                    .semantic_in(
                                        cx,
                                        NodeSpec::new("workspace.delete.body", Role::Text)
                                            .text("Nothing else is affected."),
                                    )
                                    .into_any_element()
                            })
                            .dismissable(dismissable)
                            .destructive(destructive)
                            .cancel_label("Keep")
                            .confirm_label("Delete")
                    }),
                    trigger: cx.focus_handle(),
                })
            })
            .clone();

        div()
            .child(
                div()
                    .id("page.delete")
                    .track_focus(&fixture.trigger)
                    .w(px(80.0))
                    .h(px(24.0))
                    .semantic_in(
                        cx,
                        NodeSpec::new("page.delete", Role::Button)
                            .text("Delete workspace")
                            .focus(&fixture.trigger),
                    ),
            )
            .child(fixture.dialog.clone())
            .into_any_element()
    });
    harness.snapshot();
    let fixture = slot.borrow().clone().expect("dialog was built");
    (harness, fixture.dialog.clone(), fixture.trigger.clone())
}

fn events(harness: &mut Harness, dialog: &Entity<Dialog>) -> Rc<RefCell<Vec<DialogEvent>>> {
    let seen: Rc<RefCell<Vec<DialogEvent>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    harness.update({
        let dialog = dialog.clone();
        move |_, cx| {
            cx.subscribe(&dialog, move |_, event: &DialogEvent, _| {
                sink.borrow_mut().push(*event);
            })
            .detach();
        }
    });
    seen
}

fn open(harness: &mut Harness, dialog: &Entity<Dialog>) {
    let dialog = dialog.clone();
    harness.update(move |window, cx| {
        dialog.update(cx, |dialog, cx| dialog.open(window, cx));
    });
}

#[gpui::test]
fn a_closed_dialog_publishes_neither_itself_nor_its_body(cx: &mut TestAppContext) {
    let (mut harness, _dialog, _trigger) = scene(cx, true, false);

    assert!(harness.node("workspace.delete").is_none());
    assert!(
        harness.node("workspace.delete.body").is_none(),
        "a closed dialog must not publish its body"
    );
    assert!(harness.node("workspace.delete.confirm").is_none());
}

#[gpui::test]
fn opening_publishes_the_dialog_its_body_and_its_actions(cx: &mut TestAppContext) {
    let (mut harness, dialog, _trigger) = scene(cx, true, false);
    let seen = events(&mut harness, &dialog);
    open(&mut harness, &dialog);

    let node = harness.node("workspace.delete").expect("published");
    assert_eq!(node.role, Role::Dialog);
    assert_eq!(node.text.as_deref(), Some("Delete this workspace?"));
    assert_eq!(node.expanded, Some(true));
    assert!(node.visible);

    assert_eq!(
        harness
            .node("workspace.delete.description")
            .expect("published")
            .text
            .as_deref(),
        Some("Everything in it is removed from this machine.")
    );
    assert!(harness.node("workspace.delete.body").is_some());
    assert_eq!(
        harness
            .node("workspace.delete.confirm")
            .expect("published")
            .text
            .as_deref(),
        Some("Delete")
    );
    assert_eq!(
        harness
            .node("workspace.delete.cancel")
            .expect("published")
            .text
            .as_deref(),
        Some("Keep")
    );
    assert_eq!(*seen.borrow(), vec![DialogEvent::Opened]);
}

#[gpui::test]
fn escape_dismisses_a_dismissable_dialog(cx: &mut TestAppContext) {
    let (mut harness, dialog, _trigger) = scene(cx, true, false);
    let seen = events(&mut harness, &dialog);
    open(&mut harness, &dialog);

    harness.keystrokes("escape");

    assert_eq!(
        *seen.borrow(),
        vec![
            DialogEvent::Opened,
            DialogEvent::Dismissed,
            DialogEvent::Closed
        ]
    );
    assert!(harness.node("workspace.delete").is_none());
}

#[gpui::test]
fn escape_does_nothing_to_a_dialog_that_cannot_be_dismissed(cx: &mut TestAppContext) {
    let (mut harness, dialog, _trigger) = scene(cx, false, false);
    let seen = events(&mut harness, &dialog);
    open(&mut harness, &dialog);

    harness.keystrokes("escape");

    assert_eq!(*seen.borrow(), vec![DialogEvent::Opened]);
    assert_eq!(
        harness
            .node("workspace.delete")
            .expect("published")
            .expanded,
        Some(true)
    );
}

#[gpui::test]
fn confirming_and_cancelling_report_distinct_outcomes(cx: &mut TestAppContext) {
    let (mut harness, dialog, _trigger) = scene(cx, true, false);
    let seen = events(&mut harness, &dialog);

    open(&mut harness, &dialog);
    harness.click("workspace.delete.confirm");
    assert_eq!(
        *seen.borrow(),
        vec![
            DialogEvent::Opened,
            DialogEvent::Confirmed,
            DialogEvent::Closed
        ]
    );

    seen.borrow_mut().clear();
    open(&mut harness, &dialog);
    harness.click("workspace.delete.cancel");
    assert_eq!(
        *seen.borrow(),
        vec![
            DialogEvent::Opened,
            DialogEvent::Cancelled,
            DialogEvent::Closed
        ]
    );
}

#[gpui::test]
fn clicking_the_scrim_dismisses_a_dismissable_dialog(cx: &mut TestAppContext) {
    let (mut harness, dialog, _trigger) = scene(cx, true, false);
    let seen = events(&mut harness, &dialog);
    open(&mut harness, &dialog);

    // A corner of the window is scrim, never the centered dialog.
    harness
        .context()
        .simulate_click(gpui::point(px(6.0), px(6.0)), gpui::Modifiers::none());
    harness.context().run_until_parked();

    assert!(seen.borrow().contains(&DialogEvent::Dismissed));
    assert!(harness.node("workspace.delete").is_none());
}

#[gpui::test]
fn tabbing_cycles_inside_the_open_dialog(cx: &mut TestAppContext) {
    let (mut harness, dialog, trigger) = scene(cx, true, false);
    harness.update({
        let trigger = trigger.clone();
        move |window, cx| trigger.focus(window, cx)
    });
    open(&mut harness, &dialog);

    assert!(
        harness
            .node("workspace.delete.confirm")
            .expect("published")
            .focused,
        "opening moves the keyboard onto the primary action"
    );

    harness.keystrokes("tab");
    assert!(
        harness
            .node("workspace.delete.cancel")
            .expect("published")
            .focused
    );
    assert!(
        !harness.node("page.delete").expect("published").focused,
        "tab must not walk out of the dialog"
    );

    harness.keystrokes("shift-tab");
    assert!(
        harness
            .node("workspace.delete.confirm")
            .expect("published")
            .focused
    );
}

#[gpui::test]
fn closing_returns_the_keyboard_to_where_it_came_from(cx: &mut TestAppContext) {
    let (mut harness, dialog, trigger) = scene(cx, true, false);
    harness.update({
        let trigger = trigger.clone();
        move |window, cx| trigger.focus(window, cx)
    });
    open(&mut harness, &dialog);
    harness.click("workspace.delete.cancel");

    assert!(
        harness.node("page.delete").expect("published").focused,
        "closing gives focus back to the element that opened the dialog"
    );
}

#[gpui::test]
fn a_destructive_dialog_opens_on_the_action_that_changes_nothing(cx: &mut TestAppContext) {
    let (mut harness, dialog, _trigger) = scene(cx, true, true);
    open(&mut harness, &dialog);

    assert!(
        harness
            .node("workspace.delete.cancel")
            .expect("published")
            .focused
    );
    assert!(
        !harness
            .node("workspace.delete.confirm")
            .expect("published")
            .focused
    );
}
