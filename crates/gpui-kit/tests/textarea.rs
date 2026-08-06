//! Typing, wrapping, and refusal behaviour of `TextArea`, driven through
//! simulated key and mouse input rather than by calling editing methods.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, IntoElement, TestAppContext, div, prelude::*, px};
use gpui_kit::controls::textarea::{TextArea, TextAreaEvent};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

/// The width every area under test is given, narrow enough that a sentence
/// wraps onto several visual rows.
const WIDTH: f32 = 200.0;

/// Opens a window holding one text area, and hands back the entity so a test
/// can read the committed value the way an owning view would.
fn area(
    cx: &mut TestAppContext,
    configure: impl Fn(TextArea) -> TextArea + 'static,
) -> (Harness, Rc<RefCell<Option<Entity<TextArea>>>>) {
    let slot: Rc<RefCell<Option<Entity<TextArea>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let entity = build_slot
            .borrow_mut()
            .get_or_insert_with(|| cx.new(|cx| configure(TextArea::new("form.notes", window, cx))))
            .clone();
        div().w(px(WIDTH)).child(entity).into_any_element()
    });
    (harness, slot)
}

fn value(harness: &mut Harness, slot: &Rc<RefCell<Option<Entity<TextArea>>>>) -> String {
    let entity = slot.borrow().clone().expect("area was built");
    harness.update(|_, cx| entity.read(cx).value().to_string())
}

fn caret(harness: &mut Harness, slot: &Rc<RefCell<Option<Entity<TextArea>>>>) -> usize {
    let entity = slot.borrow().clone().expect("area was built");
    harness.update(|_, cx| entity.read(cx).cursor_offset())
}

fn caret_row(harness: &mut Harness, slot: &Rc<RefCell<Option<Entity<TextArea>>>>) -> usize {
    let entity = slot.borrow().clone().expect("area was built");
    harness.update(|_, cx| entity.read(cx).cursor_row())
}

fn primary(chord: &str) -> String {
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    format!("{modifier}-{chord}")
}

#[gpui::test]
fn typing_after_focus_changes_the_value(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.placeholder("What changed, and why"));
    harness.click("form.notes");
    harness.keystrokes("h e l l o");

    assert_eq!(value(&mut harness, &slot), "hello");
    let node = harness.node("form.notes").expect("area publishes itself");
    assert_eq!(node.value.as_deref(), Some("hello"));
    assert_eq!(node.placeholder.as_deref(), Some("What changed, and why"));
}

#[gpui::test]
fn enter_inserts_a_line_instead_of_submitting(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.placeholder("Notes"));
    let submits = Rc::new(RefCell::new(0usize));
    let counter = submits.clone();
    let entity = slot.borrow().clone().expect("area was built");
    harness.update(move |_, cx| {
        cx.subscribe(&entity, move |_, event: &TextAreaEvent, _| {
            if matches!(event, TextAreaEvent::Submit) {
                *counter.borrow_mut() += 1;
            }
        })
        .detach();
    });

    harness.click("form.notes");
    harness.keystrokes("a enter b");

    assert_eq!(value(&mut harness, &slot), "a\nb");
    assert_eq!(*submits.borrow(), 0);
}

#[gpui::test]
fn the_submit_chord_reports_a_submission(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.placeholder("Notes"));
    let submits = Rc::new(RefCell::new(0usize));
    let counter = submits.clone();
    let entity = slot.borrow().clone().expect("area was built");
    harness.update(move |_, cx| {
        cx.subscribe(&entity, move |_, event: &TextAreaEvent, _| {
            if matches!(event, TextAreaEvent::Submit) {
                *counter.borrow_mut() += 1;
            }
        })
        .detach();
    });

    harness.click("form.notes");
    harness.keystrokes("h i");
    harness.keystrokes(&primary("enter"));

    assert_eq!(*submits.borrow(), 1);
    assert_eq!(value(&mut harness, &slot), "hi");
}

#[gpui::test]
fn up_and_down_move_by_visual_row_across_a_wrap(cx: &mut TestAppContext) {
    let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa";
    let (mut harness, slot) = area(cx, move |area| area.text(text).rows(6));
    harness.click("form.notes");
    harness.keystrokes(&primary("end"));

    let end = caret(&mut harness, &slot);
    let last_row = caret_row(&mut harness, &slot);
    assert_eq!(end, text.len());
    assert!(
        last_row > 0,
        "the sample text must wrap for this test to mean anything"
    );

    harness.keystrokes("up");
    let above = caret(&mut harness, &slot);
    assert_eq!(caret_row(&mut harness, &slot), last_row - 1);
    assert!(
        above > 0 && above < end,
        "a visual row up must land inside the text, not at its start"
    );

    // No hard break exists, so returning to the same offset can only come from
    // a preserved goal column.
    harness.keystrokes("down");
    assert_eq!(caret_row(&mut harness, &slot), last_row);
    assert_eq!(caret(&mut harness, &slot), end);
}

#[gpui::test]
fn up_and_down_cross_a_hard_break(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.text("one\ntwo\nthree").rows(4));
    harness.click("form.notes");
    harness.keystrokes(&primary("end"));
    assert_eq!(caret_row(&mut harness, &slot), 2);

    harness.keystrokes("up up");
    assert_eq!(caret_row(&mut harness, &slot), 0);
    assert_eq!(caret(&mut harness, &slot), 3);
}

#[gpui::test]
fn home_and_end_stop_at_the_visual_row(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.text("one\ntwo\nthree").rows(4));
    harness.click("form.notes");
    harness.keystrokes(&primary("end"));
    harness.keystrokes("home");
    assert_eq!(caret(&mut harness, &slot), 8);

    harness.keystrokes(&primary("home"));
    assert_eq!(caret(&mut harness, &slot), 0);

    harness.keystrokes("end");
    assert_eq!(caret(&mut harness, &slot), 3);
}

#[gpui::test]
fn a_disabled_area_refuses_typing(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.placeholder("Notes").disabled(true));
    harness.click("form.notes");
    harness.keystrokes("n o enter");

    assert_eq!(value(&mut harness, &slot), "");
    assert!(harness.node("form.notes").expect("published").disabled);
}

#[gpui::test]
fn a_length_limit_truncates_instead_of_rejecting(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.placeholder("Notes").max_length(3));
    harness.click("form.notes");
    harness.keystrokes("a b c d e");

    assert_eq!(value(&mut harness, &slot), "abc");
}

#[gpui::test]
fn select_all_then_typing_replaces_everything(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.text("first line\nsecond line"));
    harness.click("form.notes");
    harness.keystrokes(&primary("a"));
    harness.keystrokes("n");

    assert_eq!(value(&mut harness, &slot), "n");
}

#[gpui::test]
fn an_invalid_required_area_says_so(cx: &mut TestAppContext) {
    let (mut harness, _slot) = area(cx, |area| {
        area.placeholder("Notes").invalid(true).required(true)
    });
    let node = harness.node("form.notes").expect("published");
    assert!(node.invalid);
    assert!(node.required);
}

#[gpui::test]
fn a_host_can_replace_the_value(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.text("first"));
    let entity = slot.borrow().clone().expect("area was built");
    harness.update(move |_, cx| {
        entity.update(cx, |area, cx| area.set_value("second\nthird", cx));
    });

    assert_eq!(value(&mut harness, &slot), "second\nthird");
    assert_eq!(
        harness
            .node("form.notes")
            .expect("published")
            .value
            .as_deref(),
        Some("second\nthird")
    );
}

#[gpui::test]
fn the_area_grows_with_the_text_up_to_its_limit(cx: &mut TestAppContext) {
    let (mut harness, _slot) = area(cx, |area| area.placeholder("Notes").rows(1).max_rows(3));
    let start = harness.bounds("form.notes").expect("published").size.height;

    harness.click("form.notes");
    harness.keystrokes("a enter b");
    let grown = harness.bounds("form.notes").expect("published").size.height;
    assert!(grown > start, "two lines must be taller than one");

    harness.keystrokes("enter c enter d enter e");
    let capped = harness.bounds("form.notes").expect("published").size.height;
    assert!(
        capped < grown * 2.0,
        "growth must stop at the row limit rather than follow the text"
    );
}
