//! Taking an edit back and putting it again, driven through simulated input
//! rather than by calling the history directly.
//!
//! What is asserted here is the boundary a reader would recognise: a run of
//! typing is one step, a word is where it stops, a deletion is its own step,
//! and a field holding a credential has no way back at all.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, IntoElement, TestAppContext, div, prelude::*, px};
use gpui_kit::controls::input::TextInput;
use gpui_kit::controls::textarea::TextArea;
use gpui_kit_testkit::harness::Harness;

/// The platform's undo and redo keystrokes, as this repository binds them.
const UNDO: &str = if cfg!(target_os = "macos") {
    "cmd-z"
} else {
    "ctrl-z"
};
const REDO: &str = if cfg!(target_os = "macos") {
    "cmd-shift-z"
} else {
    "ctrl-shift-z"
};

fn input(
    cx: &mut TestAppContext,
    configure: impl Fn(TextInput) -> TextInput + 'static,
) -> (Harness, Rc<RefCell<Option<Entity<TextInput>>>>) {
    let slot: Rc<RefCell<Option<Entity<TextInput>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let entity = build_slot
            .borrow_mut()
            .get_or_insert_with(|| cx.new(|cx| configure(TextInput::new("form.token", window, cx))))
            .clone();
        entity.into_any_element()
    });
    (harness, slot)
}

fn value(harness: &mut Harness, slot: &Rc<RefCell<Option<Entity<TextInput>>>>) -> String {
    let entity = slot.borrow().clone().expect("input was built");
    harness.update(|_, cx| entity.read(cx).value().to_string())
}

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
        div().w(px(200.0)).child(entity).into_any_element()
    });
    (harness, slot)
}

fn area_value(harness: &mut Harness, slot: &Rc<RefCell<Option<Entity<TextArea>>>>) -> String {
    let entity = slot.borrow().clone().expect("area was built");
    harness.update(|_, cx| entity.read(cx).value().to_string())
}

#[gpui::test]
fn a_run_of_typing_is_taken_back_as_one_word(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input);
    harness.click("form.token");
    harness.keystrokes("h e l l o");
    assert_eq!(value(&mut harness, &slot), "hello");

    harness.keystrokes(UNDO);
    assert_eq!(
        value(&mut harness, &slot),
        "",
        "typing a word is one thing the reader did, not five"
    );
}

#[gpui::test]
fn undo_stops_between_words_rather_than_clearing_the_field(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input);
    harness.click("form.token");
    harness.keystrokes("o n e space t w o");
    assert_eq!(value(&mut harness, &slot), "one two");

    harness.keystrokes(UNDO);
    assert_eq!(value(&mut harness, &slot), "one ");
}

#[gpui::test]
fn redo_puts_back_what_undo_took(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input);
    harness.click("form.token");
    harness.keystrokes("h i");
    harness.keystrokes(UNDO);
    assert_eq!(value(&mut harness, &slot), "");

    harness.keystrokes(REDO);
    assert_eq!(value(&mut harness, &slot), "hi");
}

#[gpui::test]
fn a_deletion_is_its_own_step(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input);
    harness.click("form.token");
    harness.keystrokes("a b c backspace");
    assert_eq!(value(&mut harness, &slot), "ab");

    harness.keystrokes(UNDO);
    assert_eq!(
        value(&mut harness, &slot),
        "abc",
        "the deletion came back before the typing did"
    );
    harness.keystrokes(UNDO);
    assert_eq!(value(&mut harness, &slot), "");
}

#[gpui::test]
fn a_secret_field_keeps_no_way_back_to_what_it_held(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input.secret(true));
    harness.click("form.token");
    harness.keystrokes("h u n t e r");
    assert_eq!(value(&mut harness, &slot), "hunter");

    harness.keystrokes(UNDO);
    assert_eq!(
        value(&mut harness, &slot),
        "hunter",
        "a credential that could be undone back into view would outlive the moment it was replaced"
    );
    let node = harness.node("form.token").expect("published");
    assert_eq!(node.value.as_deref(), Some("[REDACTED]"));
}

#[gpui::test]
fn a_value_the_host_set_cannot_be_undone_back_out(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input);
    harness.click("form.token");
    harness.keystrokes("t y p e d");

    let entity = slot.borrow().clone().expect("input entity");
    harness.update(|_, cx| {
        entity.update(cx, |input, cx| input.set_value("from the host", cx));
    });
    assert_eq!(value(&mut harness, &slot), "from the host");

    harness.keystrokes(UNDO);
    assert_eq!(
        value(&mut harness, &slot),
        "from the host",
        "the steps that described the old value no longer describe anything"
    );
}

#[gpui::test]
fn a_read_only_field_offers_nothing_to_take_back(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input.text("fixed").read_only(true));
    harness.click("form.token");
    harness.keystrokes("x");
    harness.keystrokes(UNDO);

    assert_eq!(value(&mut harness, &slot), "fixed");
}

#[gpui::test]
fn an_area_takes_back_a_line_at_a_time(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area);
    harness.click("form.notes");
    harness.keystrokes("o n e enter t w o");
    assert_eq!(area_value(&mut harness, &slot), "one\ntwo");

    harness.keystrokes(UNDO);
    assert_eq!(area_value(&mut harness, &slot), "one\n");
    harness.keystrokes(UNDO);
    assert_eq!(
        area_value(&mut harness, &slot),
        "one",
        "the newline is its own step, because it is where a reader would stop"
    );
}
