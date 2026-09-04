//! Typing, wrapping, and refusal behaviour of `TextArea`, driven through
//! simulated key and mouse input rather than by calling editing methods.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, Focusable, IntoElement, TestAppContext, div, prelude::*, px};
use gpui_kit::controls::textarea::{Frame, TextArea, TextAreaEvent, TextAreaWrap};
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
    let positions = first_run["aria"]["character_positions"]
        .as_array()
        .expect("painted grapheme positions");
    let widths = first_run["aria"]["character_widths"]
        .as_array()
        .expect("painted grapheme widths");
    assert_eq!(positions.len(), 3);
    assert_eq!(widths.len(), 3);
    assert!(positions[1].as_f64().expect("second position") > 0.0);
    assert!(widths[0].as_f64().expect("first width") > 0.0);
    assert!(widths[1].as_f64().expect("second width") > 0.0);
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

/// Collects everything an area reports, so a test reads the sequence rather
/// than one flag per event kind.
fn reported(
    harness: &mut Harness,
    slot: &Rc<RefCell<Option<Entity<TextArea>>>>,
) -> Rc<RefCell<Vec<TextAreaEvent>>> {
    let events: Rc<RefCell<Vec<TextAreaEvent>>> = Rc::new(RefCell::new(Vec::new()));
    let into = events.clone();
    let entity = slot.borrow().clone().expect("area was built");
    harness.update(move |_, cx| {
        cx.subscribe(&entity, move |_, event: &TextAreaEvent, _| {
            into.borrow_mut().push(event.clone());
        })
        .detach();
    });
    events
}

#[gpui::test]
fn a_composer_submits_on_enter_and_opens_a_line_on_shift_enter(cx: &mut TestAppContext) {
    // The other convention, for text that is a message rather than a value.
    let (mut harness, slot) = area(cx, |area| area.enter(Enter::Submits));
    let events = reported(&mut harness, &slot);

    harness.click("form.notes");
    harness.keystrokes("a enter");
    assert_eq!(
        value(&mut harness, &slot),
        "a",
        "enter sent it rather than adding to it"
    );
    assert!(events.borrow().contains(&TextAreaEvent::Submit));

    harness.keystrokes("shift-enter b");
    assert_eq!(
        value(&mut harness, &slot),
        "a\nb",
        "and the second line is still reachable"
    );
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| **event == TextAreaEvent::Submit)
            .count(),
        1,
        "shift-enter is not a second submission"
    );
}

#[gpui::test]
fn the_arrows_can_be_claimed_by_whatever_is_listing_over_the_area(cx: &mut TestAppContext) {
    // A menu drawn over the composer cannot take the arrows for itself: a
    // bound key never reaches a raw listener. So the area hands them over, and
    // the caret stays where it is while they are gone.
    let (mut harness, slot) = area(cx, |area| area.text("first\nsecond\nthird"));
    let events = reported(&mut harness, &slot);
    let entity = slot.borrow().clone().expect("area was built");

    harness.click("form.notes");
    harness.keystrokes("up");
    let moved = caret_row(&mut harness, &slot);

    let claimed = entity.clone();
    harness.update(move |_, cx| {
        claimed.update(cx, |area, _| area.set_arrows_claimed(true));
    });
    harness.keystrokes("up up");

    assert_eq!(
        caret_row(&mut harness, &slot),
        moved,
        "the caret did not move while the arrows belonged to something else"
    );
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| **event == TextAreaEvent::MoveUp)
            .count(),
        2,
        "and both keystrokes were reported"
    );

    let released = entity.clone();
    harness.update(move |_, cx| {
        released.update(cx, |area, _| area.set_arrows_claimed(false));
    });
    harness.keystrokes("down");
    assert_ne!(
        caret_row(&mut harness, &slot),
        moved,
        "taking them back moves the caret again"
    );
}

#[gpui::test]
fn completion_claims_accept_dismiss_and_arrows_without_changing_submit_chords(
    cx: &mut TestAppContext,
) {
    let (mut harness, slot) = area(cx, |area| area.text("query"));
    let events = reported(&mut harness, &slot);
    let entity = slot.borrow().clone().expect("area was built");

    harness.click("form.notes");
    let completion = entity.clone();
    harness.update(move |_, cx| {
        completion.update(cx, |area, cx| {
            area.set_completion_claimed(true);
            cx.notify();
        });
    });
    harness.keystrokes("up down enter escape");
    harness.keystrokes(&primary("enter"));

    let events = events.borrow();
    assert!(events.contains(&TextAreaEvent::MoveUp));
    assert!(events.contains(&TextAreaEvent::MoveDown));
    assert!(events.contains(&TextAreaEvent::AcceptCompletion));
    assert!(events.contains(&TextAreaEvent::DismissCompletion));
    assert!(events.contains(&TextAreaEvent::Submit));
    assert!(!events.contains(&TextAreaEvent::Cancel));
    drop(events);
    assert_eq!(value(&mut harness, &slot), "query");
}

#[gpui::test]
fn a_completion_replacement_is_unicode_safe_and_one_undo_step(cx: &mut TestAppContext) {
    let original = "Hello @ál";
    let (mut harness, slot) = area(cx, move |area| area.text(original));
    let events = reported(&mut harness, &slot);
    let entity = slot.borrow().clone().expect("area was built");

    harness.click("form.notes");
    let replacement = entity.clone();
    let replaced = harness.update(move |_, cx| {
        replacement.update(cx, |area, cx| {
            area.set_selected_range(0..0, cx);
            area.replace_range(6..original.len(), "@Ada", cx)
        })
    });

    assert_eq!(replaced, Some(6..10));
    assert_eq!(value(&mut harness, &slot), "Hello @Ada");
    assert!(
        events
            .borrow()
            .contains(&TextAreaEvent::SelectionChanged(10..10))
    );
    harness.keystrokes(&primary("z"));
    assert_eq!(value(&mut harness, &slot), original);
}

#[gpui::test]
fn range_geometry_is_only_published_for_the_shaped_value(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.text("alpha beta"));
    let events = reported(&mut harness, &slot);
    let entity = slot.borrow().clone().expect("area was built");
    harness.frame();

    let (caret, word) = harness.update(|_, cx| {
        let area = entity.read(cx);
        (area.caret_bounds(), area.bounds_for_range(0..5))
    });
    assert!(caret.is_some());
    assert!(word.is_some_and(|bounds| !bounds.is_empty()));

    let changed = entity.clone();
    let stale = harness.update(move |_, cx| {
        changed.update(cx, |area, cx| {
            area.set_value("next", cx);
            area.caret_bounds()
        })
    });
    assert!(stale.is_none());
    harness.frame();
    assert!(
        harness
            .update(|_, cx| entity.read(cx).caret_bounds())
            .is_some()
    );
    assert!(events.borrow().contains(&TextAreaEvent::GeometryChanged));
}

#[gpui::test]
fn a_paste_that_is_not_text_is_reported_rather_than_dropped(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area);
    let events = reported(&mut harness, &slot);

    let image = gpui::Image::from_bytes(gpui::ImageFormat::Png, vec![1, 2, 3]);
    harness.update({
        let image = image.clone();
        move |_, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_image(&image))
    });
    harness.click("form.notes");
    harness.keystrokes(&primary("v"));

    assert_eq!(
        value(&mut harness, &slot),
        "",
        "an image is not written into the text as if somebody typed it"
    );
    assert!(
        events
            .borrow()
            .contains(&TextAreaEvent::Pasted(Pasted::Images(vec![image]))),
        "the host is told what arrived: {:?}",
        events.borrow()
    );
}

#[gpui::test]
fn a_drop_lands_at_the_caret_as_one_undoable_step(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.text("before AFTER"));
    let entity = slot.borrow().clone().expect("area was built");

    harness.click("form.notes");
    let dropped = entity.clone();
    harness.update(move |_, cx| {
        dropped.update(cx, |area, cx| {
            area.set_value("before AFTER", cx);
            area.insert("drop", cx);
        });
    });

    assert_eq!(value(&mut harness, &slot), "before AFTERdrop");
    harness.keystrokes(&primary("z"));
    assert_eq!(
        value(&mut harness, &slot),
        "before AFTER",
        "one drop is one undo"
    );
}

#[gpui::test]
fn what_the_area_measured_says_what_a_narrower_frame_would_have_to_hold(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| {
        area.text("A sentence long enough that this frame has to wrap it more than once.")
            .rows(1)
            .max_rows(4)
    });
    let entity = slot.borrow().clone().expect("area was built");
    harness.frame();

    let measured = harness
        .update(|_, cx| entity.read(cx).measured())
        .expect("the area has been laid out");
    assert!(
        measured.wrapped > px(0.0) && measured.wrapped < px(WIDTH),
        "the text was wrapped against the area's own width, inside the {WIDTH}px          frame: {measured:?}"
    );
    assert!(
        measured.text > measured.wrapped,
        "the unwrapped text is wider than what wrapped it: {measured:?}"
    );
    assert!(measured.height > px(0.0), "the wrapped text has a height");

    let before = measured.pass;
    harness.click("form.notes");
    harness.keystrokes("x");
    let after = harness
        .update(|_, cx| entity.read(cx).measured())
        .expect("still laid out");
    assert!(
        after.pass > before,
        "a later layout is a later pass, so a host can tell it apart from the one it acted on"
    );
}

#[gpui::test]
fn a_short_line_measures_narrower_than_the_frame_it_sits_in(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.text("Short.").rows(1));
    let entity = slot.borrow().clone().expect("area was built");
    harness.frame();

    let measured = harness
        .update(|_, cx| entity.read(cx).measured())
        .expect("the area has been laid out");
    assert!(
        measured.text < measured.wrapped,
        "text that fits reports that it fits, which is what keeps a pill a pill: {measured:?}"
    );
}

#[gpui::test]
fn a_new_placeholder_does_not_disturb_what_was_typed(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.placeholder("Ask anything"));
    let entity = slot.borrow().clone().expect("area was built");

    harness.click("form.notes");
    harness.keystrokes("h i");
    let changed = entity.clone();
    harness.update(move |_, cx| {
        changed.update(cx, |area, cx| {
            area.set_placeholder("Answer the question", cx)
        });
    });

    assert_eq!(
        value(&mut harness, &slot),
        "hi",
        "the area was told what to suggest when it is empty, not to empty itself"
    );
    assert_eq!(
        harness
            .node("form.notes")
            .and_then(|node| node.placeholder.clone()),
        Some("Answer the question".into()),
        "and it suggests the new one"
    );
}

#[gpui::test]
fn an_area_in_a_host_frame_still_edits(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.frame(Frame::Host).text("draft"));

    harness.click("form.notes");
    harness.keystrokes(&primary("a"));
    harness.keystrokes("s e n t");

    assert_eq!(
        value(&mut harness, &slot),
        "sent",
        "the host took over the frame, not the editing"
    );
}

#[gpui::test]
fn an_area_built_without_a_window_reports_focus_once_it_is_shown(cx: &mut TestAppContext) {
    let slot: Rc<RefCell<Option<Entity<TextArea>>>> = Rc::new(RefCell::new(None));
    let events: Rc<RefCell<Vec<TextAreaEvent>>> = Rc::new(RefCell::new(Vec::new()));
    let build_slot = slot.clone();
    let build_events = events.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, cx| {
        let entity = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                // No window: this is the position a host is in inside a
                // subscription, a task, or a test that never opened one.
                let area = cx.new(|cx| TextArea::detached("form.notes", cx));
                let seen = build_events.clone();
                cx.subscribe(&area, move |_, event: &TextAreaEvent, _| {
                    seen.borrow_mut().push(event.clone());
                })
                .detach();
                area
            })
            .clone();
        div().w(px(WIDTH)).child(entity).into_any_element()
    });

    harness.frame();
    harness.frame();
    harness.click("form.notes");
    harness.keystrokes("h i");

    assert_eq!(
        value(&mut harness, &slot),
        "hi",
        "an area that started without a window is an ordinary area once it is shown"
    );
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| matches!(event, TextAreaEvent::Change(_)))
            .count(),
        2,
        "and it reports each change once: repeated frames do not subscribe it twice: {:?}",
        events.borrow()
    );
}

#[gpui::test]
fn no_wrap_keeps_one_hard_line_and_reveals_its_caret(cx: &mut TestAppContext) {
    let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa";
    let (mut harness, slot) = area(cx, move |area| {
        area.text(text).rows(2).wrap(TextAreaWrap::None)
    });

    harness.frame();
    harness.frame();
    let entity = slot.borrow().clone().expect("area was built");
    let (row, offset, caret) = harness.update(|_, cx| {
        let area = entity.read(cx);
        (
            area.cursor_row(),
            area.horizontal_scroll_offset(),
            area.caret_bounds(),
        )
    });

    assert_eq!(row, 0, "a hard line must not become visual rows");
    assert!(
        offset > px(0.0),
        "the viewport must reveal the ending caret"
    );
    assert!(
        caret.is_some(),
        "the revealed caret keeps published geometry"
    );
}

#[gpui::test]
fn edits_and_undo_publish_monotonic_revisioned_replacements(cx: &mut TestAppContext) {
    let (mut harness, slot) = area(cx, |area| area.text("alpha"));
    let events = reported(&mut harness, &slot);
    let entity = slot.borrow().clone().expect("area was built");

    let changed = entity.clone();
    harness.update(move |_, cx| {
        changed.update(cx, |area, cx| {
            assert_eq!(area.replace_range(1..4, "β", cx), Some(1..3));
        });
    });
    assert_eq!(value(&mut harness, &slot), "aβa");
    assert_eq!(
        harness.update(|_, cx| entity.read(cx).snapshot()),
        gpui_kit::controls::textarea::TextAreaSnapshot {
            revision: 1,
            text: "aβa".into(),
        }
    );
    assert!(events.borrow().contains(&TextAreaEvent::Edited(
        gpui_kit::controls::textarea::TextAreaEdit {
            revision: 1,
            replaced: 1..4,
            inserted: "β".into(),
        }
    )));

    harness.click("form.notes");
    harness.keystrokes(&primary("z"));
    assert_eq!(value(&mut harness, &slot), "alpha");
    assert!(events.borrow().contains(&TextAreaEvent::Edited(
        gpui_kit::controls::textarea::TextAreaEdit {
            revision: 2,
            replaced: 1..3,
            inserted: "lph".into(),
        }
    )));
}
