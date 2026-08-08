//! Typing, selection, and refusal behaviour of `TextInput`, driven through
//! simulated key and mouse input rather than by calling editing methods.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{App, AppContext as _, Entity, IntoElement, TestAppContext, Window};
use gpui_kit::controls::input::{TextInput, TextInputEvent};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

/// Opens a window holding one input, and hands back the entity so a test can
/// read the committed value the way an owning view would.
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

#[gpui::test]
fn typing_after_focus_changes_the_value(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input.placeholder("sk-..."));
    harness.click("form.token");
    harness.keystrokes("h e l l o");

    assert_eq!(value(&mut harness, &slot), "hello");
    let node = harness.node("form.token").expect("input publishes itself");
    assert_eq!(node.value.as_deref(), Some("hello"));
    assert_eq!(node.placeholder.as_deref(), Some("sk-..."));
}

#[gpui::test]
fn an_unfocused_input_ignores_typing(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input);
    harness.keystrokes("h i");
    assert_eq!(value(&mut harness, &slot), "");
}

#[gpui::test]
fn a_disabled_input_refuses_typing(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input.disabled(true));
    harness.click("form.token");
    harness.keystrokes("n o");

    assert_eq!(value(&mut harness, &slot), "");
    assert!(harness.node("form.token").expect("published").disabled);
}

#[gpui::test]
fn backspace_removes_one_grapheme_at_a_time(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input.text("héllo"));
    harness.click("form.token");
    harness.keystrokes("end backspace backspace");

    assert_eq!(value(&mut harness, &slot), "hél");
}

#[gpui::test]
fn select_all_then_typing_replaces_everything(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input.text("old value"));
    harness.click("form.token");
    let select_all = if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    };
    harness.keystrokes(select_all);
    harness.keystrokes("n");

    assert_eq!(value(&mut harness, &slot), "n");
}

#[gpui::test]
fn a_length_limit_truncates_instead_of_rejecting(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input.max_length(3));
    harness.click("form.token");
    harness.keystrokes("a b c d e");

    assert_eq!(value(&mut harness, &slot), "abc");
}

#[gpui::test]
fn a_secret_never_reaches_the_semantic_tree(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input.secret(true));
    harness.click("form.token");
    harness.keystrokes("s e c r e t");

    assert_eq!(value(&mut harness, &slot), "secret");
    let node = harness.node("form.token").expect("published");
    assert_eq!(node.value.as_deref(), Some("[REDACTED]"));
}

#[gpui::test]
fn an_invalid_required_field_says_so(cx: &mut TestAppContext) {
    let (mut harness, _slot) = input(cx, |input| input.invalid(true).required(true));
    let node = harness.node("form.token").expect("published");
    assert!(node.invalid);
    assert!(node.required);
}

#[gpui::test]
fn the_primary_key_reports_a_submission(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input);
    let submits = Rc::new(RefCell::new(0usize));
    let counter = submits.clone();
    let entity = slot.borrow().clone().expect("input was built");
    harness.update(move |_, cx| {
        cx.subscribe(&entity, move |_, event: &TextInputEvent, _| {
            if matches!(event, TextInputEvent::Submit) {
                *counter.borrow_mut() += 1;
            }
        })
        .detach();
    });

    harness.click("form.token");
    harness.keystrokes("h i enter");

    assert_eq!(*submits.borrow(), 1);
    assert_eq!(value(&mut harness, &slot), "hi");
}

#[gpui::test]
fn a_host_can_replace_the_value(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input.text("first"));
    let entity = slot.borrow().clone().expect("input was built");
    harness.update(move |_, cx| {
        entity.update(cx, |input, cx| input.set_value("second", cx));
    });

    assert_eq!(value(&mut harness, &slot), "second");
    assert_eq!(
        harness
            .node("form.token")
            .expect("published")
            .value
            .as_deref(),
        Some("second")
    );
}

#[gpui::test]
fn editable_text_reaches_the_accesskit_text_pattern(cx: &mut TestAppContext) {
    let (mut harness, slot) = input(cx, |input| input.name("Token").text("a😀é"));
    let tree = harness.accessibility_tree();
    let nodes = tree["nodes"].as_object().expect("nodes");
    let field = nodes
        .values()
        .find(|node| {
            node["element_id"] == "Name(\"form.token\")" && node["aria"]["role"] == "TextInput"
        })
        .expect("native text input");
    assert_eq!(field["aria"]["role"], "TextInput");
    assert_eq!(field["aria"]["label"], "Token");
    let actions = field["aria"]["on_action"].as_array().expect("actions");
    assert!(actions.iter().any(|action| action == "SetValue"));
    assert!(actions.iter().any(|action| action == "SetTextSelection"));

    let run_id = field["children"][0].as_str().expect("text run id");
    let run = &nodes[run_id];
    assert_eq!(run["aria"]["role"], "TextRun");
    assert_eq!(run["aria"]["value"], "a😀é");
    assert_eq!(run["aria"]["character_lengths"][0], 1);
    assert_eq!(run["aria"]["character_lengths"][1], 4);
    assert_eq!(run["aria"]["character_lengths"][2], 2);
    assert_eq!(
        field["aria"]["text_selection"]["anchor"]["character_index"],
        3
    );
    assert_eq!(
        field["aria"]["text_selection"]["focus"]["character_index"],
        3
    );

    let field_id = gpui::accesskit::NodeId(
        field["accesskit_id"]
            .as_str()
            .expect("field node id")
            .parse()
            .expect("numeric field id"),
    );
    let run_node_id = gpui::accesskit::NodeId(
        run["accesskit_id"]
            .as_str()
            .expect("run node id")
            .parse()
            .expect("numeric run id"),
    );
    let window = harness.window();
    harness.context().dispatch_accessibility_action(
        window,
        gpui::accesskit::ActionRequest {
            action: gpui::accesskit::Action::SetTextSelection,
            target_tree: gpui::accesskit::TreeId::ROOT,
            target_node: field_id,
            data: Some(gpui::accesskit::ActionData::SetTextSelection(
                gpui::accesskit::TextSelection {
                    anchor: gpui::accesskit::TextPosition {
                        node: run_node_id,
                        character_index: 1,
                    },
                    focus: gpui::accesskit::TextPosition {
                        node: run_node_id,
                        character_index: 2,
                    },
                },
            )),
        },
    );
    let entity = slot.borrow().clone().expect("input entity");
    assert_eq!(
        harness.update(|_, cx| entity.read(cx).selected_range()),
        1..5
    );
    harness.context().dispatch_accessibility_action(
        window,
        gpui::accesskit::ActionRequest {
            action: gpui::accesskit::Action::SetValue,
            target_tree: gpui::accesskit::TreeId::ROOT,
            target_node: field_id,
            data: Some(gpui::accesskit::ActionData::Value("updated".into())),
        },
    );
    assert_eq!(value(&mut harness, &slot), "updated");
}

#[gpui::test]
fn password_and_read_only_native_contracts_are_explicit(cx: &mut TestAppContext) {
    let (mut password, _slot) = input(cx, |input| {
        input.name("Password").text("secret").secret(true)
    });
    let tree = password.accessibility_tree();
    let field = tree["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"form.token\")"
                    && node["aria"]["role"] == "PasswordInput"
            })
        })
        .expect("native password input");
    assert_eq!(field["aria"]["role"], "PasswordInput");
    assert!(
        field.get("children").is_none(),
        "password text must not become text runs"
    );

    let (mut read_only, _slot) = input(cx, |input| input.text("verified").read_only(true));
    let tree = read_only.accessibility_tree();
    let field = tree["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"form.token\")" && node["aria"]["role"] == "TextInput"
            })
        })
        .expect("native read-only input");
    assert_eq!(field["aria"]["read_only"], true);
    let actions = field["aria"]["on_action"]
        .as_array()
        .expect("selection action");
    assert!(actions.iter().any(|action| action == "SetTextSelection"));
    assert!(!actions.iter().any(|action| action == "SetValue"));
}

fn _unused(_: &mut Window, _: &mut App) {}
