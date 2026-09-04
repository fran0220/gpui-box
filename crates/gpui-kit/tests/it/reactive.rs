//! What a bound control and the caller's signal agree about.
//!
//! The unit tests next to `gpui_kit::reactive` cover the signal, the lens,
//! and the form on their own. These drive a real control through the window
//! harness, because the thing worth proving about `.bind` is that a value
//! travels the whole way in both directions and does not travel back.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, IntoElement, Subscription, TestAppContext};
use gpui_kit::controls::input::TextInput;
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

/// Everything the build closure creates once and every test then reads.
#[derive(Default)]
struct Bound {
    input: Option<Entity<TextInput>>,
    signal: Option<Signal<String>>,
    subscriptions: Vec<Subscription>,
}

fn bound_input(cx: &mut TestAppContext, initial: &str) -> (Harness, Rc<RefCell<Bound>>) {
    let state: Rc<RefCell<Bound>> = Rc::default();
    let initial = initial.to_string();
    let build_state = state.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let existing = build_state.borrow().input.clone();
        let input = match existing {
            Some(input) => input,
            None => {
                let input = cx.new(|cx| TextInput::new("form.email", window, cx));
                let signal = Signal::new(cx, initial.clone());
                let subscriptions = TextInput::bind(&input, &signal, cx);
                let mut state = build_state.borrow_mut();
                state.input = Some(input.clone());
                state.signal = Some(signal);
                state.subscriptions = subscriptions;
                input
            }
        };
        input.into_any_element()
    });
    (harness, state)
}

fn typed(harness: &mut Harness, state: &Rc<RefCell<Bound>>) -> String {
    let input = state.borrow().input.clone().expect("the input was built");
    harness.update(|_, cx| input.read(cx).value().to_string())
}

fn held(harness: &mut Harness, state: &Rc<RefCell<Bound>>) -> String {
    let signal = state.borrow().signal.clone().expect("the signal was built");
    harness.update(|_, cx| signal.get(cx))
}

#[gpui::test]
fn a_bound_input_opens_holding_what_the_signal_holds(cx: &mut TestAppContext) {
    let (mut harness, state) = bound_input(cx, "ada@example.com");
    assert_eq!(typed(&mut harness, &state), "ada@example.com");
    assert_eq!(held(&mut harness, &state), "ada@example.com");
}

#[gpui::test]
fn typing_into_a_bound_input_writes_the_signal(cx: &mut TestAppContext) {
    let (mut harness, state) = bound_input(cx, "");
    harness.click("form.email");
    harness.keystrokes("h e l l o");

    assert_eq!(typed(&mut harness, &state), "hello");
    assert_eq!(held(&mut harness, &state), "hello");
}

#[gpui::test]
fn writing_the_signal_puts_the_text_in_a_bound_input(cx: &mut TestAppContext) {
    let (mut harness, state) = bound_input(cx, "");
    let signal = state.borrow().signal.clone().expect("the signal was built");
    harness.update(|_, cx| signal.set(cx, String::from("grace")));

    assert_eq!(typed(&mut harness, &state), "grace");
    let node = harness
        .node("form.email")
        .expect("the input publishes itself");
    assert_eq!(node.value.as_deref(), Some("grace"));
}

#[gpui::test]
fn typing_into_a_bound_input_leaves_the_caret_where_it_was(cx: &mut TestAppContext) {
    // The signal writing the field back is what would move it, so this is
    // the observable form of "the value does not travel round the loop".
    let (mut harness, state) = bound_input(cx, "");
    harness.click("form.email");
    harness.keystrokes("a b c left left");
    let input = state.borrow().input.clone().expect("the input was built");
    let before = harness.update(|_, cx| input.read(cx).cursor_offset());

    harness.keystrokes("x");

    assert_eq!(typed(&mut harness, &state), "axbc");
    assert_eq!(held(&mut harness, &state), "axbc");
    assert_eq!(
        harness.update(|_, cx| input.read(cx).cursor_offset()),
        before + 1,
        "the caret advanced by the character that was typed, and nothing else moved it"
    );
}
