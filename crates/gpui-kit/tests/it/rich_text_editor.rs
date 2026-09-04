//! Keyboard, model, and semantic contracts of `RichTextEditor`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AppContext as _, Entity, EntityInputHandler, IntoElement, TestAppContext, div, prelude::*, px,
};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

type SessionSlot = Rc<RefCell<Option<Entity<RichTextEditSession>>>>;
type EditorSlot = Rc<RefCell<Option<Entity<RichTextEditor>>>>;

fn editor(
    cx: &mut TestAppContext,
    document: RichTextDocument,
    configure: impl Fn(RichTextEditor) -> RichTextEditor + 'static,
) -> (Harness, SessionSlot, EditorSlot) {
    let session_slot = SessionSlot::default();
    let editor_slot = EditorSlot::default();
    let build_session = Rc::clone(&session_slot);
    let build_editor = Rc::clone(&editor_slot);
    let next_id = Rc::new(Cell::new(0_u64));
    let harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let session = build_session
            .borrow_mut()
            .get_or_insert_with(|| cx.new(|_| RichTextEditSession::new(document.clone())))
            .clone();
        let editor = build_editor
            .borrow_mut()
            .get_or_insert_with(|| {
                let next_id = Rc::clone(&next_id);
                cx.new(|cx| {
                    configure(RichTextEditor::new(
                        "form.rich",
                        session,
                        move || {
                            let value = next_id.get().wrapping_add(1);
                            next_id.set(value);
                            RichTextBlockId::new(format!("new-{value}"))
                        },
                        window,
                        cx,
                    ))
                })
            })
            .clone();
        div().w(px(420.0)).child(editor).into_any_element()
    });
    (harness, session_slot, editor_slot)
}

fn primary(chord: &str) -> String {
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    format!("{modifier}-{chord}")
}

fn block_texts(harness: &mut Harness, slot: &SessionSlot) -> Vec<String> {
    let session = slot.borrow().clone().expect("session was built");
    harness.update(|_, cx| {
        session
            .read(cx)
            .document()
            .blocks()
            .iter()
            .map(|block| block.text().to_string())
            .collect()
    })
}

#[gpui::test]
fn enter_is_a_hard_break_and_shift_enter_is_a_soft_break(cx: &mut TestAppContext) {
    let document = RichTextDocument::empty("first").expect("fixture is valid");
    let (mut harness, session, _) = editor(cx, document, |editor| {
        editor.toolbar(false).name("Document")
    });
    harness.click("form.rich");
    harness.keystrokes("a enter b shift-enter c");

    assert_eq!(block_texts(&mut harness, &session), ["a", "b\nc"]);
    assert_eq!(
        harness
            .node("form.rich")
            .expect("editor publishes itself")
            .value
            .as_deref(),
        Some("a\nb\nc")
    );
    assert_eq!(
        harness
            .node("form.rich")
            .expect("published")
            .text
            .as_deref(),
        Some("Document")
    );
}

#[gpui::test]
fn rich_text_publishes_painted_character_geometry(cx: &mut TestAppContext) {
    let document = RichTextDocument::new([
        RichTextBlock::new("first", "a😀"),
        RichTextBlock::new("second", "tail"),
    ])
    .expect("fixture is valid");
    let (mut harness, _, _) = editor(cx, document, |editor| {
        editor.toolbar(false).name("Document")
    });

    let tree = harness.accessibility_tree();
    let nodes = tree["nodes"].as_object().expect("nodes");
    let field = nodes
        .values()
        .find(|node| {
            node["element_id"] == "Name(\"form.rich\")"
                && node["aria"]["role"] == "MultilineTextInput"
        })
        .expect("native rich text input");
    let first_run_id = field["children"][0].as_str().expect("first text run id");
    let first_run = &nodes[first_run_id];
    assert_eq!(first_run["aria"]["value"], "a😀\n");
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
}

#[gpui::test]
fn formatting_shortcuts_change_the_caller_owned_session(cx: &mut TestAppContext) {
    let document = RichTextDocument::plain("first", "quiet evidence").expect("fixture is valid");
    let (mut harness, session, _) = editor(cx, document, |editor| editor.toolbar(false));
    harness.click("form.rich");
    harness.keystrokes(&primary("a"));
    harness.keystrokes(&primary("b"));

    let session = session.borrow().clone().expect("session was built");
    assert!(harness.update(|_, cx| {
        session.read(cx).document().blocks()[0]
            .styles()
            .style_at(0)
            .format(RichTextFormat::Bold)
    }));
}

#[gpui::test]
fn repeated_ime_updates_replace_one_composition_and_undo_as_one_step(cx: &mut TestAppContext) {
    let document = RichTextDocument::empty("first").expect("fixture is valid");
    let (mut harness, session, editor) = editor(cx, document, |editor| editor.toolbar(false));
    let editor = editor.borrow().clone().expect("editor was built");
    harness.update(|window, cx| {
        editor.update(cx, |editor, cx| {
            editor.replace_and_mark_text_in_range(None, "😀", Some(0..2), window, cx);
            editor.replace_and_mark_text_in_range(None, "字", Some(0..1), window, cx);
            editor.unmark_text(window, cx);
        });
    });
    assert_eq!(block_texts(&mut harness, &session), ["字"]);

    harness.click("form.rich");
    harness.keystrokes(&primary("z"));
    assert_eq!(block_texts(&mut harness, &session), [""]);
}

#[gpui::test]
fn multiline_platform_input_gets_stable_blocks_and_one_undo_step(cx: &mut TestAppContext) {
    let document = RichTextDocument::empty("first").expect("fixture is valid");
    let (mut harness, session, editor) = editor(cx, document, |editor| editor.toolbar(false));
    let editor = editor.borrow().clone().expect("editor was built");
    harness.update(|window, cx| {
        editor.update(cx, |editor, cx| {
            editor.replace_text_in_range(None, "one\ntwo\nthree", window, cx);
        });
    });
    assert_eq!(block_texts(&mut harness, &session), ["one", "two", "three"]);

    harness.click("form.rich");
    harness.keystrokes(&primary("z"));
    assert_eq!(block_texts(&mut harness, &session), [""]);
}

#[gpui::test]
fn replacing_a_cross_block_selection_collapses_it_to_one_block(cx: &mut TestAppContext) {
    let document = RichTextDocument::new([
        RichTextBlock::new("first", "one"),
        RichTextBlock::new("second", "two"),
    ])
    .expect("fixture is valid");
    let (mut harness, session, _) = editor(cx, document, |editor| editor.toolbar(false));
    harness.click("form.rich");
    harness.keystrokes(&primary("a"));
    harness.keystrokes("x");

    assert_eq!(block_texts(&mut harness, &session), ["x"]);
}

#[gpui::test]
fn read_only_and_disabled_editors_refuse_input_truthfully(cx: &mut TestAppContext) {
    let document = RichTextDocument::plain("first", "fixed").expect("fixture is valid");
    let (mut read_only, read_only_session, _) = editor(cx, document.clone(), |editor| {
        editor.toolbar(false).read_only(true)
    });
    read_only.click("form.rich");
    read_only.keystrokes("x backspace");
    assert_eq!(block_texts(&mut read_only, &read_only_session), ["fixed"]);
    assert!(read_only.node("form.rich").expect("published").read_only);

    let (mut disabled, disabled_session, _) =
        editor(cx, document, |editor| editor.toolbar(false).disabled(true));
    disabled.click("form.rich");
    disabled.keystrokes("x backspace");
    assert_eq!(block_texts(&mut disabled, &disabled_session), ["fixed"]);
    assert!(disabled.node("form.rich").expect("published").disabled);
}

#[gpui::test]
fn host_replacement_is_the_next_render_and_clears_stale_geometry(cx: &mut TestAppContext) {
    let document = RichTextDocument::plain("first", "before").expect("fixture is valid");
    let (mut harness, session, _) = editor(cx, document, |editor| editor.toolbar(false));
    let session = session.borrow().clone().expect("session was built");
    harness.update(|_, cx| {
        session.update(cx, |session, cx| {
            let document =
                RichTextDocument::plain("replacement", "after").expect("replacement is valid");
            let selection = document.selection_at_end();
            session
                .replace_document(document, selection)
                .expect("replacement selection is valid");
            cx.notify();
        });
    });

    assert_eq!(
        harness
            .node("form.rich")
            .expect("editor publishes itself")
            .value
            .as_deref(),
        Some("after")
    );
}
