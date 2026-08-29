//! Source editing contracts of `Editor`.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AppContext as _, Entity, HighlightStyle, IntoElement, TestAppContext, div, prelude::*, px,
};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

type EditorSlot = Rc<RefCell<Option<Entity<Editor>>>>;

fn editor(
    cx: &mut TestAppContext,
    text: &'static str,
    configure: impl Fn(Editor) -> Editor + 'static,
) -> (Harness, EditorSlot) {
    let slot = EditorSlot::default();
    let build_slot = slot.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let editor = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| configure(Editor::new("source", "Source", text, window, cx)))
            })
            .clone();
        div()
            .w(px(420.0))
            .h(px(280.0))
            .child(editor)
            .into_any_element()
    });
    (harness, slot)
}

#[gpui::test]
fn typing_uses_the_shared_text_area_and_reports_revisioned_edits(cx: &mut TestAppContext) {
    let (mut harness, slot) = editor(cx, "", |editor| editor);
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();
    let entity = slot.borrow().clone().expect("editor was built");
    harness.update(move |_, cx| {
        cx.subscribe(&entity, move |_, event: &EditorEvent, _| {
            seen.borrow_mut().push(event.clone());
        })
        .detach();
    });

    harness.click("source.input");
    harness.keystrokes("l e t space x");

    let snapshot = harness.update(|_, cx| {
        slot.borrow()
            .as_ref()
            .expect("editor")
            .read(cx)
            .snapshot(cx)
    });
    assert_eq!(snapshot.text.as_ref(), "let x");
    assert_eq!(snapshot.revision, 5);
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| matches!(event, EditorEvent::Edited(_)))
            .count(),
        5
    );
    assert_eq!(
        harness
            .node("source.input")
            .expect("the shared input is published")
            .value
            .as_deref(),
        Some("let x")
    );
}

#[gpui::test]
fn no_wrap_geometry_publishes_one_row_per_hard_line(cx: &mut TestAppContext) {
    let (mut harness, slot) = editor(
        cx,
        "first line that is deliberately far wider than the viewport can hold\nsecond\nthird",
        |editor| editor.rows(4),
    );
    harness.click("source.input");
    harness.keystrokes(if cfg!(target_os = "macos") {
        "cmd-end"
    } else {
        "ctrl-end"
    });
    harness.frame();

    let geometry = harness
        .update(|_, cx| {
            slot.borrow()
                .as_ref()
                .expect("editor")
                .read(cx)
                .geometry(cx)
        })
        .expect("a painted editor publishes geometry");
    assert_eq!(geometry.revision, 0);
    assert_eq!(
        geometry
            .lines
            .iter()
            .map(|line| line.line)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(geometry.lines[1].range, 69..76);
}

#[gpui::test]
fn highlights_must_match_the_revision_and_form_disjoint_boundaries(cx: &mut TestAppContext) {
    let (mut harness, slot) = editor(cx, "let βanswer = 42;", |editor| editor);
    let style = HighlightStyle {
        color: Some(gpui::red()),
        ..Default::default()
    };
    let entity = slot.borrow().clone().expect("editor was built");

    assert!(harness.update(|_, cx| {
        entity.update(cx, |editor, cx| {
            editor.set_highlights(
                EditorHighlights::new(0, [EditorHighlight::new(0..3, style)]),
                cx,
            )
        })
    }));
    assert!(!harness.update(|_, cx| {
        entity.update(cx, |editor, cx| {
            editor.set_highlights(
                EditorHighlights::new(1, [EditorHighlight::new(0..3, style)]),
                cx,
            )
        })
    }));
    assert!(!harness.update(|_, cx| {
        entity.update(cx, |editor, cx| {
            editor.set_highlights(
                EditorHighlights::new(
                    0,
                    [
                        EditorHighlight::new(0..3, style),
                        EditorHighlight::new(2..6, style),
                    ],
                ),
                cx,
            )
        })
    }));
    assert!(!harness.update(|_, cx| {
        entity.update(cx, |editor, cx| {
            editor.set_highlights(
                EditorHighlights::new(0, [EditorHighlight::new(4..5, style)]),
                cx,
            )
        })
    }));
}

#[gpui::test]
fn tab_uses_one_synchronous_caller_owned_replacement(cx: &mut TestAppContext) {
    let requests = Rc::new(RefCell::new(Vec::new()));
    let captured = requests.clone();
    let (mut harness, slot) = editor(cx, "value", move |editor| {
        let captured = captured.clone();
        editor.indent_with(move |request| {
            captured.borrow_mut().push(request.clone());
            let caret = request.selection.end;
            Some(EditorIndentation::new(caret..caret, "    ").selection(caret + 4..caret + 4))
        })
    });

    harness.click("source.input");
    harness.keystrokes("tab");

    let snapshot = harness.update(|_, cx| {
        slot.borrow()
            .as_ref()
            .expect("editor")
            .read(cx)
            .snapshot(cx)
    });
    assert_eq!(snapshot.text.as_ref(), "value    ");
    assert_eq!(snapshot.revision, 1);
    assert_eq!(requests.borrow().len(), 1);
    assert_eq!(requests.borrow()[0].snapshot.revision, 0);
    assert_eq!(
        requests.borrow()[0].direction,
        EditorIndentDirection::Indent
    );
}

#[gpui::test]
fn ordinary_text_areas_keep_their_literal_tab_behavior(cx: &mut TestAppContext) {
    let slot: Rc<RefCell<Option<Entity<TextArea>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let area = build_slot
            .borrow_mut()
            .get_or_insert_with(|| cx.new(|cx| TextArea::new("notes", window, cx).text("ordinary")))
            .clone();
        div().w(px(420.0)).child(area).into_any_element()
    });

    harness.click("notes");
    harness.keystrokes("tab");

    assert_eq!(
        harness.update(|_, cx| { slot.borrow().as_ref().expect("area").read(cx).snapshot() }),
        TextAreaSnapshot {
            revision: 1,
            text: "ordinary\t".into(),
        }
    );
}
