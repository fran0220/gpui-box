//! Typing, wrapping, and refusal behaviour of `TextArea`, driven through
//! simulated key and mouse input rather than by calling editing methods.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, Focusable, IntoElement, TestAppContext, div, prelude::*, px};
use gpui_kit::controls::textarea::{TextArea, TextAreaEvent};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;
use unicode_segmentation::UnicodeSegmentation;

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

macro_rules! native_id {
    ($node:expr) => {
        gpui::accesskit::NodeId(
            $node["accesskit_id"]
                .as_str()
                .expect("native node id")
                .parse()
                .expect("numeric native node id"),
        )
    };
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

#[gpui::test]
fn multiline_text_reaches_the_accesskit_text_pattern(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.text("e\u{301}👩‍💻\nאב"));
    let tree = harness.accessibility_tree();
    let nodes = tree["nodes"].as_object().expect("nodes");
    let field = nodes
        .values()
        .find(|node| {
            node["element_id"] == "Name(\"form.notes\")"
                && node["aria"]["role"] == "MultilineTextInput"
        })
        .expect("native multiline input");
    assert_eq!(field["aria"]["role"], "MultilineTextInput");
    let actions = field["aria"]["on_action"].as_array().expect("actions");
    assert!(actions.iter().any(|action| action == "SetValue"));
    assert!(actions.iter().any(|action| action == "SetTextSelection"));
    let first_run_id = field["children"][0].as_str().expect("first text run id");
    let second_run_id = field["children"][1].as_str().expect("second text run id");
    let first_run = &nodes[first_run_id];
    let second_run = &nodes[second_run_id];
    assert_eq!(first_run["aria"]["role"], "TextRun");
    assert_eq!(first_run["aria"]["value"], "e\u{301}👩‍💻\n");
    assert_eq!(first_run["aria"]["character_lengths"][0], 3);
    assert_eq!(first_run["aria"]["character_lengths"][1], 11);
    assert_eq!(first_run["aria"]["character_lengths"][2], 1);
    assert_eq!(first_run["aria"]["text_direction"], "LeftToRight");
    assert!(first_run["aria"].get("next_on_line").is_none());
    assert_eq!(second_run["aria"]["value"], "אב");
    assert_eq!(second_run["aria"]["text_direction"], "RightToLeft");
    assert!(second_run["aria"].get("previous_on_line").is_none());

    let field_id = gpui::accesskit::NodeId(
        field["accesskit_id"]
            .as_str()
            .expect("field node id")
            .parse()
            .expect("numeric field id"),
    );
    let first_node_id = gpui::accesskit::NodeId(
        first_run["accesskit_id"]
            .as_str()
            .expect("first run node id")
            .parse()
            .expect("numeric first run id"),
    );
    let second_node_id = gpui::accesskit::NodeId(
        second_run["accesskit_id"]
            .as_str()
            .expect("second run node id")
            .parse()
            .expect("numeric second run id"),
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
                        node: first_node_id,
                        character_index: 1,
                    },
                    focus: gpui::accesskit::TextPosition {
                        node: second_node_id,
                        character_index: 1,
                    },
                },
            )),
        },
    );
    let entity = slot.borrow().clone().expect("area entity");
    assert_eq!(
        harness.update(|_, cx| entity.read(cx).selected_range()),
        3..17
    );
}

#[gpui::test]
fn wrapped_text_runs_follow_visual_rows_and_keep_logical_order(cx: &mut TestAppContext) {
    let text = format!("{} אב {}", "alpha ".repeat(80), "omega ".repeat(80));
    assert!(text.graphemes(true).count() > 255);
    let (mut harness, _slot) = area(cx, move |area| area.text(text.clone()).rows(20));
    let tree = harness.accessibility_tree();
    let nodes = tree["nodes"].as_object().expect("nodes");
    let field = nodes
        .values()
        .find(|node| {
            node["element_id"] == "Name(\"form.notes\")"
                && node["aria"]["role"] == "MultilineTextInput"
        })
        .expect("area");
    let children = field["children"].as_array().expect("visual text runs");
    assert!(children.len() > 2, "narrow text must span visual rows");
    let published = children
        .iter()
        .map(|id| {
            nodes[id.as_str().expect("run")]["aria"]["value"]
                .as_str()
                .expect("text run value")
        })
        .collect::<String>();
    assert_eq!(
        published,
        format!("{} אב {}", "alpha ".repeat(80), "omega ".repeat(80))
    );
    assert!(children.iter().any(|id| {
        let run = &nodes[id.as_str().expect("run")];
        run["aria"]["value"]
            .as_str()
            .is_some_and(|value| value.contains("אב"))
            && run["aria"]["text_direction"] == "RightToLeft"
    }));
    let first = &nodes[children[0].as_str().expect("first visual row")];
    assert!(
        first["aria"].get("next_on_line").is_none(),
        "the first wrapped row must not link into the next visual row"
    );
}

#[gpui::test]
fn trailing_line_breaks_have_a_distinct_empty_line_and_round_trip(cx: &mut TestAppContext) {
    for text in ["a\n", "a\r\n"] {
        let expected_newline = text.len() - if text.ends_with("\r\n") { 2 } else { 1 };
        let owned = text.to_string();
        let (mut harness, slot) = area(cx, move |area| area.text(owned.clone()));
        let tree = harness.accessibility_tree();
        let nodes = tree["nodes"].as_object().expect("nodes");
        let field = nodes
            .values()
            .find(|node| {
                node["element_id"] == "Name(\"form.notes\")"
                    && node["aria"]["role"] == "MultilineTextInput"
            })
            .expect("area");
        let children = field["children"].as_array().expect("runs");
        assert_eq!(children.len(), 2);
        let line = &nodes[children[0].as_str().expect("line")];
        let empty = &nodes[children[1].as_str().expect("empty line")];
        assert_eq!(line["aria"]["value"], text);
        assert_eq!(empty["aria"]["value"], "");

        let field_id = native_id!(field);
        let line_id = native_id!(line);
        let empty_id = native_id!(empty);
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
                            node: empty_id,
                            character_index: 0,
                        },
                        focus: gpui::accesskit::TextPosition {
                            node: line_id,
                            character_index: 1,
                        },
                    },
                )),
            },
        );
        let entity = slot.borrow().clone().expect("area");
        assert_eq!(
            harness.update(|_, cx| entity.read(cx).selected_range()),
            expected_newline..text.len()
        );
    }
}

#[gpui::test]
fn stale_selection_and_disabled_state_are_rechecked_at_dispatch(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.text("before"));
    harness.click("form.notes");
    let entity = slot.borrow().clone().expect("area");
    let tree = harness.accessibility_tree();
    let nodes = tree["nodes"].as_object().expect("nodes");
    let field = nodes
        .values()
        .find(|node| {
            node["element_id"] == "Name(\"form.notes\")"
                && node["aria"]["role"] == "MultilineTextInput"
        })
        .expect("field");
    let field_id = native_id!(field);
    let run = &nodes[field["children"][0].as_str().expect("run")];
    let run_id = native_id!(run);

    // Change state without allowing a rebuilt accessibility tree. The old
    // asynchronous request must not be interpreted against the new source.
    harness
        .context()
        .update(|_, cx| entity.update(cx, |area, cx| area.set_value("after", cx)));
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
                        node: run_id,
                        character_index: 0,
                    },
                    focus: gpui::accesskit::TextPosition {
                        node: run_id,
                        character_index: 2,
                    },
                },
            )),
        },
    );
    assert_eq!(
        harness.update(|_, cx| entity.read(cx).selected_range()),
        5..5
    );

    let tree = harness.accessibility_tree();
    let nodes = tree["nodes"].as_object().expect("nodes");
    let field = nodes
        .values()
        .find(|node| {
            node["element_id"] == "Name(\"form.notes\")"
                && node["aria"]["role"] == "MultilineTextInput"
        })
        .expect("rebuilt field");
    let after_run = &nodes[field["children"][0].as_str().expect("rebuilt run")];
    let after_run_id = native_id!(after_run);
    harness.update(|_, cx| entity.update(cx, |area, cx| area.set_value("latest", cx)));
    harness.context().dispatch_accessibility_action(
        window,
        gpui::accesskit::ActionRequest {
            action: gpui::accesskit::Action::SetTextSelection,
            target_tree: gpui::accesskit::TreeId::ROOT,
            target_node: field_id,
            data: Some(gpui::accesskit::ActionData::SetTextSelection(
                gpui::accesskit::TextSelection {
                    anchor: gpui::accesskit::TextPosition {
                        node: after_run_id,
                        character_index: 0,
                    },
                    focus: gpui::accesskit::TextPosition {
                        node: after_run_id,
                        character_index: 2,
                    },
                },
            )),
        },
    );
    assert_eq!(
        harness.update(|_, cx| entity.read(cx).selected_range()),
        6..6,
        "run ids from a superseded activated tree must be rejected"
    );

    harness.click("form.notes");
    harness.update(|_, cx| entity.update(cx, |area, cx| area.set_disabled(true, cx)));
    assert!(!harness.update(|window, cx| entity.focus_handle(cx).is_focused(window)));
    harness.keystrokes("x");
    assert_eq!(value(&mut harness, &slot), "latest");
    let tree = harness.accessibility_tree();
    let field = tree["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"form.notes\")"
                    && node["aria"]["role"] == "MultilineTextInput"
            })
        })
        .expect("disabled area");
    let actions = field["aria"]["on_action"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for action in ["Focus", "SetValue", "SetTextSelection"] {
        assert!(!actions.iter().any(|candidate| candidate == action));
    }
}

#[gpui::test]
fn unrepresentable_graphemes_omit_the_native_selection_pattern(cx: &mut TestAppContext) {
    let text = format!("a{}", "\u{301}".repeat(128));
    let (mut harness, slot) = area(cx, move |area| area.text(text.clone()).max_length(4));
    let tree = harness.accessibility_tree();
    let field = tree["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"form.notes\")"
                    && node["aria"]["role"] == "MultilineTextInput"
            })
        })
        .expect("area");
    assert!(field.get("children").is_none());
    assert!(
        field["aria"]["on_action"]
            .as_array()
            .is_none_or(|actions| !actions.iter().any(|action| action == "SetTextSelection"))
    );
    assert!(
        field["aria"]["on_action"]
            .as_array()
            .is_some_and(|actions| actions.iter().any(|action| action == "SetValue"))
    );
    let field_id = native_id!(field);
    let window = harness.window();
    harness.context().dispatch_accessibility_action(
        window,
        gpui::accesskit::ActionRequest {
            action: gpui::accesskit::Action::SetValue,
            target_tree: gpui::accesskit::TreeId::ROOT,
            target_node: field_id,
            data: Some(gpui::accesskit::ActionData::Value("ab\r\ncd".into())),
        },
    );
    assert_eq!(value(&mut harness, &slot), "ab\nc");
}
