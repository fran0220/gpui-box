//! The action family: a glyph that says what it does, a run of buttons that
//! share a frame but not their answers, and a default action beside the ones
//! it stands in for.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, IntoElement, ParentElement, TestAppContext, div};
use gpui_kit::overlay::MenuEvent;
use gpui_kit::prelude::*;
use gpui_kit_assets::Icon;
use gpui_kit_testkit::harness::Harness;

/// Counts how often a handler was called, the way an owning host would.
type Clicks = Rc<RefCell<Vec<&'static str>>>;

// -- IconButton -----------------------------------------------------------

fn icon_buttons(cx: &mut TestAppContext) -> (Harness, Clicks) {
    let clicks: Clicks = Rc::new(RefCell::new(Vec::new()));
    let sink = clicks.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |_window, _cx| {
        let copy = sink.clone();
        let archive = sink.clone();
        div()
            .row()
            .child(
                IconButton::new("run.copy", Icon::Copy, "Copy run id")
                    .on_click(move |_, _| copy.borrow_mut().push("copy")),
            )
            .child(
                IconButton::new("run.archive", Icon::Archive, "Archive run")
                    .disabled(true)
                    .on_click(move |_, _| archive.borrow_mut().push("archive")),
            )
            .into_any_element()
    });
    (harness, clicks)
}

#[gpui::test]
fn an_icon_button_is_addressed_by_what_it_does(cx: &mut TestAppContext) {
    let (mut harness, _clicks) = icon_buttons(cx);
    let node = harness.node("run.copy").expect("published");

    assert_eq!(node.role, gpui_kit::semantics::Role::Button);
    assert_eq!(
        node.text.as_deref(),
        Some("Copy run id"),
        "a glyph with no name is a button nobody can name"
    );
}

#[gpui::test]
fn an_icon_button_acts_when_it_is_clicked(cx: &mut TestAppContext) {
    let (mut harness, clicks) = icon_buttons(cx);
    harness.click("run.copy");
    assert_eq!(*clicks.borrow(), vec!["copy"]);
}

#[gpui::test]
fn a_refused_icon_button_installs_no_handler(cx: &mut TestAppContext) {
    let (mut harness, clicks) = icon_buttons(cx);
    assert!(harness.node("run.archive").expect("published").disabled);

    harness.click("run.archive");
    assert!(
        clicks.borrow().is_empty(),
        "a refused control does not act when it is clicked anyway"
    );
}

// -- ButtonGroup ----------------------------------------------------------

fn group(cx: &mut TestAppContext, disabled: bool) -> (Harness, Clicks) {
    let clicks: Clicks = Rc::new(RefCell::new(Vec::new()));
    let sink = clicks.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |_window, _cx| {
        let day = sink.clone();
        let week = sink.clone();
        let month = sink.clone();
        ButtonGroup::new("range")
            .children([
                Button::new("range.day")
                    .label("Day")
                    .secondary()
                    .on_click(move |_, _| day.borrow_mut().push("day")),
                Button::new("range.week")
                    .label("Week")
                    .secondary()
                    .selected(true)
                    .on_click(move |_, _| week.borrow_mut().push("week")),
                Button::new("range.month")
                    .label("Month")
                    .secondary()
                    .on_click(move |_, _| month.borrow_mut().push("month")),
            ])
            .disabled(disabled)
            .into_any_element()
    });
    (harness, clicks)
}

#[gpui::test]
fn a_group_names_the_actions_inside_it_without_answering_for_them(cx: &mut TestAppContext) {
    let (mut harness, _clicks) = group(cx, false);

    assert_eq!(
        harness.node("range").expect("published").role,
        gpui_kit::semantics::Role::Toolbar
    );
    for id in ["range.day", "range.week", "range.month"] {
        assert_eq!(
            harness.node(id).expect("published").parent.as_deref(),
            Some("range"),
            "every action in the run is reachable through the run"
        );
    }
    assert_eq!(
        harness.node("range.week").expect("published").checked,
        Some(true)
    );
}

#[gpui::test]
fn every_action_in_a_group_still_reports_itself(cx: &mut TestAppContext) {
    let (mut harness, clicks) = group(cx, false);
    harness.click("range.month");
    harness.click("range.day");
    assert_eq!(*clicks.borrow(), vec!["month", "day"]);
}

#[gpui::test]
fn refusing_the_group_refuses_every_action_in_it(cx: &mut TestAppContext) {
    let (mut harness, clicks) = group(cx, true);

    for id in ["range.day", "range.week", "range.month"] {
        assert!(harness.node(id).expect("published").disabled);
        harness.click(id);
    }
    assert!(clicks.borrow().is_empty());
}

// -- SplitButton ----------------------------------------------------------

fn split(
    cx: &mut TestAppContext,
    configure: impl Fn(SplitButton) -> SplitButton + 'static,
) -> (Harness, Entity<SplitButton>, Clicks) {
    let clicks: Clicks = Rc::new(RefCell::new(Vec::new()));
    let sink = clicks.clone();
    let slot: Rc<RefCell<Option<Entity<SplitButton>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let sink = sink.clone();
        build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    configure(
                        SplitButton::new("publish", window, cx)
                            .label("Publish")
                            .primary()
                            .on_click(move |_, _| sink.borrow_mut().push("publish"))
                            .items(
                                [
                                    MenuItem::command("publish.draft", "Save as draft"),
                                    MenuItem::command("publish.export", "Export only"),
                                ],
                                cx,
                            ),
                    )
                })
            })
            .clone()
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("split button was built");
    (harness, entity, clicks)
}

#[gpui::test]
fn the_action_and_the_arrow_are_separate_targets(cx: &mut TestAppContext) {
    let (mut harness, _entity, clicks) = split(cx, |split| split);

    assert_eq!(
        harness
            .node("publish.action")
            .expect("published")
            .text
            .as_deref(),
        Some("Publish")
    );
    assert_eq!(
        harness
            .node("publish.menu.trigger")
            .expect("published")
            .text
            .as_deref(),
        Some("More actions")
    );

    harness.click("publish.action");
    assert_eq!(*clicks.borrow(), vec!["publish"]);
    assert!(
        !harness.node("publish.menu.list").is_some(),
        "acting is not offering: the default action does not open the menu"
    );
}

#[gpui::test]
fn the_arrow_offers_the_alternatives_without_taking_one(cx: &mut TestAppContext) {
    let (mut harness, entity, clicks) = split(cx, |split| split);
    let events: Rc<RefCell<Vec<MenuEvent>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let menu = harness.update({
        let entity = entity.clone();
        move |_, cx| entity.read(cx).menu().clone()
    });
    harness.update(move |_, cx| {
        cx.subscribe(&menu, move |_, event: &MenuEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });

    harness.click("publish.menu.trigger");
    assert!(harness.node("publish.menu.publish.draft").is_some());
    assert!(clicks.borrow().is_empty(), "opening a menu acts on nothing");

    harness.click("publish.menu.publish.draft");
    assert_eq!(
        *events.borrow(),
        vec![
            MenuEvent::Opened,
            MenuEvent::Invoked("publish.draft".into()),
            MenuEvent::Closed,
        ],
        "the menu says what it did, and taking an alternative closes it"
    );
    assert!(*clicks.borrow() == Vec::<&str>::new());
}

#[gpui::test]
fn refusing_the_default_action_leaves_the_alternatives_reachable(cx: &mut TestAppContext) {
    let (mut harness, _entity, clicks) = split(cx, |split| split.default_disabled(true));

    assert!(harness.node("publish.action").expect("published").disabled);
    harness.click("publish.action");
    assert!(clicks.borrow().is_empty());

    assert!(
        !harness
            .node("publish.menu.trigger")
            .expect("published")
            .disabled
    );
    harness.click("publish.menu.trigger");
    assert!(
        harness.node("publish.menu.publish.export").is_some(),
        "when the usual thing cannot be done, the alternatives matter most"
    );
}

#[gpui::test]
fn refusing_the_whole_control_refuses_both_halves(cx: &mut TestAppContext) {
    let (mut harness, _entity, clicks) = split(cx, |split| split.disabled(true));

    assert!(harness.node("publish").expect("published").disabled);
    assert!(harness.node("publish.action").expect("published").disabled);
    assert!(
        harness
            .node("publish.menu.trigger")
            .expect("published")
            .disabled,
        "the arrow is still shown, so what is unavailable is visible rather than missing"
    );

    harness.click("publish.action");
    harness.click("publish.menu.trigger");
    assert!(clicks.borrow().is_empty());
    assert!(harness.node("publish.menu.publish.draft").is_none());
}
