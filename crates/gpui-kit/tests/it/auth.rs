//! Sensitive authentication controls, driven through the same input and
//! native accessibility paths a host uses.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, ClipboardItem, Entity, Focusable, IntoElement, Modifiers, TestAppContext,
    Window,
};
use gpui_kit::controls::auth::{
    OneTimeCodeInput, OneTimeCodeInputEvent, PasswordInput, PasswordInputEvent,
};
use gpui_kit::controls::input::TextInputEvent;
use gpui_kit::foundation::direction::{LayoutDirection, set_layout_direction};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

fn password(
    cx: &mut TestAppContext,
    configure: impl Fn(PasswordInput) -> PasswordInput + 'static,
) -> (Harness, Rc<RefCell<Option<Entity<PasswordInput>>>>) {
    let slot = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let entity = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| configure(PasswordInput::new("auth.password", window, cx)))
            })
            .clone();
        entity.into_any_element()
    });
    (harness, slot)
}

fn code(
    cx: &mut TestAppContext,
    configure: impl Fn(OneTimeCodeInput) -> OneTimeCodeInput + 'static,
) -> (Harness, Rc<RefCell<Option<Entity<OneTimeCodeInput>>>>) {
    let slot = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let entity = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| configure(OneTimeCodeInput::new("auth.code", window, cx)))
            })
            .clone();
        entity.into_any_element()
    });
    (harness, slot)
}

fn password_value(
    harness: &mut Harness,
    slot: &Rc<RefCell<Option<Entity<PasswordInput>>>>,
) -> String {
    let entity = slot.borrow().clone().expect("password was built");
    harness.update(|_, cx| entity.read(cx).value(cx).to_string())
}

fn code_value(
    harness: &mut Harness,
    slot: &Rc<RefCell<Option<Entity<OneTimeCodeInput>>>>,
) -> String {
    let entity = slot.borrow().clone().expect("code was built");
    harness.update(|_, cx| entity.read(cx).value(cx).to_string())
}

fn primary(key: &str) -> String {
    format!(
        "{}-{key}",
        if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        }
    )
}

fn semantic_text(harness: &mut Harness) -> String {
    let snapshot = harness.snapshot();
    snapshot
        .nodes
        .iter()
        .flat_map(|node| {
            [
                node.text.as_deref(),
                node.description.as_deref(),
                node.value.as_deref(),
                node.placeholder.as_deref(),
            ]
        })
        .flatten()
        .collect::<Vec<_>>()
        .join("\n")
}

#[gpui::test]
fn password_typing_reports_caller_events_and_never_formats_the_value(cx: &mut TestAppContext) {
    let (mut harness, slot) = password(cx, |password| {
        password.name("Password").required(true).invalid(true)
    });
    let changes = Rc::new(RefCell::new(Vec::new()));
    let submits = Rc::new(RefCell::new(0));
    let entity = slot.borrow().clone().expect("password entity");
    harness.update(|_, cx| {
        let changes = changes.clone();
        let submits = submits.clone();
        cx.subscribe(
            &entity,
            move |_, event: &PasswordInputEvent, _| match event {
                PasswordInputEvent::Change(value) => changes.borrow_mut().push(value.to_string()),
                PasswordInputEvent::Submit => *submits.borrow_mut() += 1,
                _ => {}
            },
        )
        .detach();
    });

    harness.click("auth.password");
    harness.keystrokes("s e c r e t enter");

    assert_eq!(password_value(&mut harness, &slot), "secret");
    assert_eq!(changes.borrow().last().map(String::as_str), Some("secret"));
    assert_eq!(*submits.borrow(), 1);
    let node = harness.node("auth.password").expect("password node");
    assert_eq!(node.value.as_deref(), Some("[REDACTED]"));
    assert!(node.required);
    assert!(node.invalid);
    let debug = harness.update(|_, cx| format!("{:?}", entity.read(cx)));
    assert!(!debug.contains("secret"));
    assert!(!format!("{:?}", PasswordInputEvent::Change("secret".into())).contains("secret"));
    assert!(!format!("{:?}", TextInputEvent::Change("secret".into())).contains("secret"));
}

#[gpui::test]
fn reveal_changes_only_visual_state_and_preserves_every_redaction_boundary(
    cx: &mut TestAppContext,
) {
    let needle = "credential-needle";
    let (mut harness, slot) = password(cx, move |password| password.name("Password").text(needle));
    let entity = slot.borrow().clone().expect("password entity");

    assert!(!semantic_text(&mut harness).contains(needle));
    let before = harness.accessibility_tree();
    assert!(!before.to_string().contains(needle));
    let native_id = before["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"auth.password\")"
                    && node["aria"]["role"] == "PasswordInput"
            })
        })
        .expect("native password input")["accesskit_id"]
        .clone();
    harness.click("auth.password");
    harness.keystrokes("home right shift-right");
    let reveal = before["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes
                .values()
                .find(|node| node["element_id"] == "Name(\"auth.password.reveal\")")
        })
        .expect("native reveal action");
    let reveal_id = gpui::accesskit::NodeId(
        reveal["accesskit_id"]
            .as_str()
            .expect("reveal node id")
            .parse()
            .expect("numeric reveal node id"),
    );
    let window = harness.window();
    harness.context().dispatch_accessibility_action(
        window,
        gpui::accesskit::ActionRequest {
            action: gpui::accesskit::Action::Focus,
            target_tree: gpui::accesskit::TreeId::ROOT,
            target_node: reveal_id,
            data: None,
        },
    );
    harness.keystrokes("enter");

    assert!(harness.update(|_, cx| entity.read(cx).is_revealed()));
    assert_eq!(
        harness.update(|_, cx| entity.read(cx).selected_range(cx)),
        1..2
    );
    assert_eq!(password_value(&mut harness, &slot), needle);
    assert!(!semantic_text(&mut harness).contains(needle));
    let tree = harness.accessibility_tree();
    assert!(!tree.to_string().contains(needle));
    let field = tree["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"auth.password\")"
                    && node["aria"]["role"] == "PasswordInput"
            })
        })
        .expect("one native password input");
    assert_eq!(
        field["accesskit_id"], native_id,
        "reveal must keep one editor"
    );
    assert!(
        field.get("children").is_none(),
        "a secret has no native text runs"
    );
    let debug = harness.update(|_, cx| format!("{:?}", entity.read(cx)));
    assert!(!debug.contains(needle));
}

#[gpui::test]
fn revealed_password_copy_and_cut_leave_clipboard_and_value_untouched(cx: &mut TestAppContext) {
    let (mut harness, slot) = password(cx, |password| password.text("keep-this"));
    harness.click("auth.password.reveal");
    harness.click("auth.password");
    harness.keystrokes(&primary("a"));
    harness.update(|_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("sentinel".to_string()));
    });

    harness.keystrokes(&primary("c"));
    let copied = harness.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert_eq!(copied.as_deref(), Some("sentinel"));
    harness.keystrokes(&primary("x"));
    let cut = harness.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert_eq!(cut.as_deref(), Some("sentinel"));
    assert_eq!(password_value(&mut harness, &slot), "keep-this");
}

#[gpui::test]
fn disabled_password_installs_neither_editor_nor_reveal_actions(cx: &mut TestAppContext) {
    let (mut harness, slot) = password(cx, |password| password.text("held").disabled(true));
    harness.click("auth.password");
    harness.keystrokes("x");
    harness.click("auth.password.reveal");

    let entity = slot.borrow().clone().expect("password entity");
    assert_eq!(password_value(&mut harness, &slot), "held");
    assert!(!harness.update(|_, cx| entity.read(cx).is_revealed()));
    let snapshot = harness.snapshot();
    assert!(snapshot.find("auth.password").expect("password").disabled);
    assert!(
        snapshot
            .find("auth.password.reveal")
            .expect("reveal")
            .disabled
    );
    let tree = harness.accessibility_tree();
    let nodes = tree["nodes"].as_object().expect("native nodes");
    for id in ["auth.password", "auth.password.reveal"] {
        let element = format!("Name(\"{id}\")");
        let node = nodes
            .values()
            .find(|node| node["element_id"] == element)
            .unwrap_or_else(|| panic!("native {id}"));
        let actions = node["aria"]["on_action"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(actions.is_empty(), "disabled {id} advertised {actions:?}");
    }
}

#[gpui::test]
fn read_only_password_keeps_its_value_visible_to_the_owner_but_refuses_edits(
    cx: &mut TestAppContext,
) {
    let (mut harness, slot) = password(cx, |password| password.text("held").read_only(true));
    let entity = slot.borrow().clone().expect("password entity");
    harness.click("auth.password");
    harness.keystrokes("home right shift-right x backspace");

    assert_eq!(password_value(&mut harness, &slot), "held");
    assert_eq!(
        harness.update(|_, cx| entity.read(cx).selected_range(cx)),
        1..2,
        "read-only keeps keyboard navigation and selection"
    );
    assert!(harness.update(|window, cx| entity.focus_handle(cx).is_focused(window)));
    let node = harness.node("auth.password").expect("read-only password");
    assert!(node.read_only);
    assert_eq!(node.value.as_deref(), Some("[REDACTED]"));
}

#[gpui::test]
fn imperative_values_before_first_render_win_over_builder_seeds(cx: &mut TestAppContext) {
    let password_slot = Rc::new(RefCell::new(None));
    let build_password = password_slot.clone();
    let mut password_harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let entity = build_password
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    let mut password =
                        PasswordInput::new("auth.password", window, cx).text("builder-password");
                    password.set_value("imperative-password", cx);
                    password
                })
            })
            .clone();
        entity.into_any_element()
    });
    assert_eq!(
        password_value(&mut password_harness, &password_slot),
        "imperative-password"
    );

    let code_slot = Rc::new(RefCell::new(None));
    let build_code = code_slot.clone();
    let mut code_harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let entity = build_code
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    let mut code = OneTimeCodeInput::new("auth.code", window, cx)
                        .slots(4)
                        .text("seed");
                    code.set_value("命令🙂x", cx);
                    code
                })
            })
            .clone();
        entity.into_any_element()
    });
    assert_eq!(code_value(&mut code_harness, &code_slot), "命令🙂x");
}

#[gpui::test]
fn whole_code_paste_counts_unicode_graphemes_and_replaces_the_selection(cx: &mut TestAppContext) {
    let (mut harness, slot) = code(cx, |code| code.name("Verification code").slots(4));
    harness.update(|_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("Ae\u{301}👩‍💻ZQ".to_string()));
    });
    harness.click("auth.code");
    harness.keystrokes(&primary("v"));

    assert_eq!(code_value(&mut harness, &slot), "Ae\u{301}👩‍💻Z");
    let entity = slot.borrow().clone().expect("code entity");
    assert_eq!(harness.update(|_, cx| entity.read(cx).len(cx)), 4);
    assert!(harness.update(|_, cx| entity.read(cx).is_complete(cx)));
    assert!(harness.update(|window, cx| entity.focus_handle(cx).is_focused(window)));

    harness.keystrokes(&primary("a"));
    harness.keystrokes("x y");
    assert_eq!(code_value(&mut harness, &slot), "xy");
    assert_eq!(
        harness
            .node("auth.code")
            .expect("code node")
            .description
            .as_deref(),
        Some("2 of 4")
    );
}

#[gpui::test]
fn a_full_code_accepts_a_joining_mark_without_reporting_rejected_input(cx: &mut TestAppContext) {
    let (mut harness, slot) = code(cx, |code| code.slots(1).text("a"));
    let entity = slot.borrow().clone().expect("code entity");
    let changes = Rc::new(RefCell::new(Vec::new()));
    harness.update(|_, cx| {
        let changes = changes.clone();
        cx.subscribe(&entity, move |_, event: &OneTimeCodeInputEvent, _| {
            if let OneTimeCodeInputEvent::Change(value) = event {
                changes.borrow_mut().push(value.to_string());
            }
        })
        .detach();
    });

    harness.click("auth.code");
    harness.context().simulate_input("\u{301}");
    assert_eq!(code_value(&mut harness, &slot), "a\u{301}");
    assert_eq!(changes.borrow().as_slice(), ["a\u{301}"]);

    harness.keystrokes("x");
    assert_eq!(code_value(&mut harness, &slot), "a\u{301}");
    assert_eq!(
        changes.borrow().len(),
        1,
        "rejected over-capacity input is not a change"
    );
}

#[gpui::test]
fn one_time_code_arrows_backspace_delete_and_submit_share_one_editor(cx: &mut TestAppContext) {
    let (mut harness, slot) = code(cx, |code| code.slots(4).text("abcd"));
    let submits = Rc::new(RefCell::new(0));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let entity = slot.borrow().clone().expect("code entity");
    harness.update(|_, cx| {
        let submits = submits.clone();
        let changes = changes.clone();
        cx.subscribe(
            &entity,
            move |_, event: &OneTimeCodeInputEvent, _| match event {
                OneTimeCodeInputEvent::Change(value) => {
                    changes.borrow_mut().push(value.to_string())
                }
                OneTimeCodeInputEvent::Submit => *submits.borrow_mut() += 1,
            },
        )
        .detach();
    });

    harness.click("auth.code");
    harness.keystrokes("end left backspace");
    assert_eq!(code_value(&mut harness, &slot), "abd");
    harness.keystrokes("home right delete");
    assert_eq!(code_value(&mut harness, &slot), "ad");
    harness.keystrokes("enter");

    assert_eq!(*submits.borrow(), 1);
    assert_eq!(changes.borrow().last().map(String::as_str), Some("ad"));
    assert!(!format!("{:?}", OneTimeCodeInputEvent::Change("ad".into())).contains("ad"));
    assert!(harness.update(|window, cx| entity.focus_handle(cx).is_focused(window)));
}

#[gpui::test]
fn one_time_code_publishes_only_redacted_length_shape_to_both_trees(cx: &mut TestAppContext) {
    let needle = "code-needle";
    let (mut harness, _slot) = code(cx, move |code| {
        code.name("Verification code").slots(11).text(needle)
    });
    let node = harness.node("auth.code").expect("one code input");
    assert_eq!(node.value.as_deref(), Some("[REDACTED]"));
    assert_eq!(node.description.as_deref(), Some("11 of 11"));
    assert!(!semantic_text(&mut harness).contains(needle));

    let tree = harness.accessibility_tree();
    assert!(!tree.to_string().contains(needle));
    let native = tree["nodes"]
        .as_object()
        .expect("native nodes")
        .values()
        .filter(|node| {
            node["element_id"] == "Name(\"auth.code\")" && node["aria"]["role"] == "PasswordInput"
        })
        .collect::<Vec<_>>();
    assert_eq!(native.len(), 1, "segments must remain one native input");
    assert!(
        native[0].get("children").is_none(),
        "a code has no native text runs"
    );
}

#[gpui::test]
fn disabled_and_read_only_code_states_refuse_edits_and_publish_state(cx: &mut TestAppContext) {
    let (mut disabled, disabled_slot) = code(cx, |code| {
        code.text("held")
            .disabled(true)
            .invalid(true)
            .required(true)
    });
    disabled.click("auth.code");
    disabled.keystrokes("x backspace enter");
    assert_eq!(code_value(&mut disabled, &disabled_slot), "held");
    let node = disabled.node("auth.code").expect("disabled code");
    assert!(node.disabled);
    assert!(node.invalid);
    assert!(node.required);
    let tree = disabled.accessibility_tree();
    let native = tree["nodes"]
        .as_object()
        .expect("native nodes")
        .values()
        .find(|node| node["element_id"] == "Name(\"auth.code\")")
        .expect("native code");
    assert!(
        native["aria"]["on_action"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .is_empty(),
        "disabled code must install no actions"
    );

    let (mut read_only, read_only_slot) = code(cx, |code| code.text("kept").read_only(true));
    let read_only_entity = read_only_slot.borrow().clone().expect("code entity");
    read_only.click("auth.code");
    read_only.keystrokes("home right shift-right x backspace");
    assert_eq!(code_value(&mut read_only, &read_only_slot), "kept");
    assert!(read_only.update(|window, cx| read_only_entity.focus_handle(cx).is_focused(window)));
    assert!(
        read_only
            .node("auth.code")
            .expect("read-only code")
            .read_only
    );
    read_only.update(|_, cx| {
        read_only_entity.update(cx, |code, cx| code.set_read_only(false, cx));
    });
    read_only.keystrokes("x");
    assert_eq!(
        code_value(&mut read_only, &read_only_slot),
        "kxpt",
        "selection made while read-only survives until editing resumes"
    );
}

#[gpui::test]
fn one_time_code_hit_testing_and_arrows_follow_rtl_slot_geometry(cx: &mut TestAppContext) {
    let (mut harness, slot) = code(cx, |code| code.slots(4).text("ab"));
    let entity = slot.borrow().clone().expect("code entity");
    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));

    let physical_left = harness.point_across("auth.code", 0.1);
    harness
        .context()
        .simulate_click(physical_left, Modifiers::none());
    harness.keystrokes("x");
    assert_eq!(code_value(&mut harness, &slot), "abx");

    harness.update(|_, cx| entity.update(cx, |code, cx| code.set_value("ab", cx)));
    let physical_right = harness.point_across("auth.code", 0.9);
    harness
        .context()
        .simulate_click(physical_right, Modifiers::none());
    harness.keystrokes("y");
    assert_eq!(code_value(&mut harness, &slot), "yab");

    harness.update(|_, cx| entity.update(cx, |code, cx| code.set_value("ab", cx)));
    harness.keystrokes("home left z");
    assert_eq!(
        code_value(&mut harness, &slot),
        "azb",
        "physical Left advances through the visually RTL slots"
    );
}

#[gpui::test]
fn one_time_code_copy_and_cut_never_export_or_delete_the_code(cx: &mut TestAppContext) {
    let (mut harness, slot) = code(cx, |code| code.text("abcdef"));
    harness.click("auth.code");
    harness.keystrokes(&primary("a"));
    harness.update(|_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("sentinel".to_string()));
    });
    harness.keystrokes(&primary("c"));
    harness.keystrokes(&primary("x"));

    assert_eq!(code_value(&mut harness, &slot), "abcdef");
    let clipboard = harness.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert_eq!(clipboard.as_deref(), Some("sentinel"));
}

fn _unused(_: &mut Window, _: &mut App) {}
