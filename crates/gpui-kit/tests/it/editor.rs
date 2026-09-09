//! Source editing contracts of `Editor`.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AppContext as _, Entity, Focusable as _, HighlightStyle, InputEvent as _, IntoElement,
    Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase, div, point, prelude::*,
    px,
};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

type EditorSlot = Rc<RefCell<Option<Entity<Editor>>>>;

#[gpui::test]
fn retained_options_update_editor_authority_and_remove_caller_policies(cx: &mut TestAppContext) {
    let (mut harness, slot) = editor(cx, "évalue", |editor| editor.language_services(true));
    let entity = slot.borrow().clone().expect("editor");
    let area = harness.update(|_, cx| entity.read(cx).text_area().clone());
    harness.click("source.input");
    harness.keystrokes("x");
    let before = harness.update(|_, cx| entity.read(cx).snapshot(cx));
    harness.update(|_, cx| {
        entity.update(cx, |editor, cx| {
            editor.set_label("Updated source", cx);
            editor.set_rows(3, cx);
            editor.set_line_numbers(false, cx);
            editor.set_indent_with(Some(Rc::new(|_| panic!("removed policy must not run"))), cx);
            editor.set_indent_with(None, cx);
            editor.set_read_only(true, cx);
            editor.set_disabled(true, cx);
            editor.set_language_services(false, cx);
            #[cfg(feature = "syntax")]
            {
                editor.set_syntax(Some(gpui_kit::controls::editor::EditorSyntax::json()), cx);
                editor.set_syntax(None, cx);
                assert!(editor.syntax_state().is_none());
            }
        })
    });
    harness.frame();
    harness.update(|_, cx| {
        let editor = entity.read(cx);
        assert_eq!(editor.text_area(), &area);
        assert!(editor.is_disabled());
        assert!(editor.is_read_only());
        assert!(area.read(cx).is_disabled());
        assert!(area.read(cx).is_read_only());
        assert_eq!(editor.snapshot(cx), before);
    });
    harness.update(|_, cx| {
        entity.update(cx, |editor, cx| {
            editor.set_disabled(false, cx);
            editor.set_read_only(false, cx);
        })
    });
    harness.frame();
    harness.update(|window, cx| window.focus(&area.read(cx).focus_handle(cx), cx));
    harness.keystrokes("tab");
    harness
        .update(|_, cx| assert_eq!(area.read(cx).value().as_ref(), format!("{}\t", before.text)));
}

#[gpui::test]
fn folded_rows_keep_source_identity_and_expand_for_hidden_navigation(cx: &mut TestAppContext) {
    use gpui_kit::controls::editor::EditorFold;
    let source = "header\n界 hidden\nאבג hidden\nlast hidden\ntail😀\n";
    let (mut harness, slot) = editor(cx, source, |editor| editor.rows(4));
    let entity = slot.borrow().clone().expect("editor");
    harness.update(|_, cx| {
        entity.update(cx, |editor, cx| {
            editor.set_selections([(0..0, false)], cx);
            assert!(editor.set_folds(
                0,
                vec![EditorFold {
                    id: "body".into(),
                    lines: 0..4
                }],
                cx
            ));
            assert!(editor.set_fold_collapsed("body", true, cx));
            assert!(!editor.set_folds(9, Vec::new(), cx));
        })
    });
    harness.frame();
    harness.update(|_, cx| {
        let editor = entity.read(cx);
        let geometry = editor.geometry(cx).expect("folded geometry");
        assert_eq!(
            geometry
                .lines
                .iter()
                .map(|line| line.line)
                .collect::<Vec<_>>(),
            [1, 5, 6]
        );
        assert_eq!(editor.snapshot(cx).text.as_ref(), source);
        let work = editor.text_area().read(cx).shaping_work().expect("work");
        assert!(work.shaped_lines <= 3, "{work:?}");
    });
    harness.click("source.fold.body");
    harness.update(|_, cx| assert!(!entity.read(cx).is_fold_collapsed("body")));
    harness.click("source.fold.body");
    harness.update(|_, cx| {
        entity.update(cx, |editor, cx| {
            assert!(editor.is_fold_collapsed("body"));
            editor.set_selections([(8..8, false)], cx);
        })
    });
    harness.frame();
    harness.update(|_, cx| {
        assert!(!entity.read(cx).is_fold_collapsed("body"));
        assert_eq!(entity.read(cx).snapshot(cx).text.as_ref(), source);
    });
    harness.update(|window, cx| {
        let area = entity.read(cx).text_area().clone();
        window.focus(&area.read(cx).focus_handle(cx), cx);
        area.update(cx, |area, cx| area.set_selected_range(0..0, cx));
    });
    harness.keystrokes("ctrl-alt-f");
    harness.update(|_, cx| assert!(entity.read(cx).is_fold_collapsed("body")));
    harness.keystrokes("down");
    harness.update(|_, cx| {
        let area = entity.read(cx).text_area().read(cx);
        assert_eq!(area.document().line_at(area.cursor_offset()), 4);
    });
    harness.keystrokes("x");
    harness.update(|_, cx| {
        assert!(!entity.read(cx).is_fold_collapsed("body"));
    });
    harness.keystrokes(if cfg!(target_os = "macos") {
        "cmd-z"
    } else {
        "ctrl-z"
    });
    harness.update(|_, cx| assert_eq!(entity.read(cx).snapshot(cx).text.as_ref(), source));
}

fn completion() -> AsyncValue<EditorServiceResult, gpui::SharedString> {
    AsyncValue::ready(EditorServiceResult::Items(vec![EditorServiceItem {
        id: "complete-unicode".into(),
        label: "界_value".into(),
        detail: None,
        effect: EditorServiceEffect::Edits(vec![EditorReplacement {
            range: 3..5,
            text: "界_value".into(),
        }]),
    }]))
}

#[gpui::test]
fn language_completion_keyboard_accepts_one_transaction_and_rejects_late_replies(
    cx: &mut TestAppContext,
) {
    let (mut harness, slot) = editor(cx, "// é tail", |editor| editor.language_services(true));
    let entity = slot.borrow().clone().expect("editor");
    let requests = Rc::new(RefCell::new(Vec::new()));
    let received = requests.clone();
    let _subscription = harness.update(|_, cx| {
        cx.subscribe(&entity, move |_, event, _| {
            if let EditorEvent::ServiceRequested(request) = event {
                received.borrow_mut().push(request.clone());
            }
        })
    });
    harness.click("source.input");
    harness.update(|_, cx| {
        entity
            .read(cx)
            .text_area()
            .clone()
            .update(cx, |area, cx| area.set_selected_range(5..5, cx));
    });
    harness.keystrokes("ctrl-space");
    let request = requests.borrow().last().cloned().expect("keyboard request");
    assert_eq!(request.kind, EditorServiceKind::Completion);
    assert_eq!(request.position, 5);
    harness.update(|_, cx| {
        entity.update(cx, |editor, cx| {
            assert!(editor.set_service_result(request.id, completion(), cx));
        })
    });
    harness.frame();
    harness.keystrokes("enter");
    harness.update(|_, cx| {
        assert_eq!(
            entity.read(cx).snapshot(cx).text.as_ref(),
            "// 界_value tail"
        );
        entity.update(cx, |editor, cx| {
            assert!(!editor.set_service_result(request.id, completion(), cx))
        });
    });
    harness.keystrokes(if cfg!(target_os = "macos") {
        "cmd-z"
    } else {
        "ctrl-z"
    });
    harness.update(|_, cx| assert_eq!(entity.read(cx).snapshot(cx).text.as_ref(), "// é tail"));
}

#[gpui::test]
fn service_refresh_retains_verified_values_but_refuses_acceptance_until_ready(
    cx: &mut TestAppContext,
) {
    let (mut harness, slot) = editor(cx, "// é tail", |editor| editor.language_services(true));
    let entity = slot.borrow().clone().expect("editor");
    harness.update(|_, cx| {
        entity.update(cx, |editor, cx| {
            let first = editor
                .request_service(EditorServiceKind::Completion, 5, cx)
                .expect("first");
            assert!(editor.set_service_result(first.id, completion(), cx));
            let second = editor
                .request_service(EditorServiceKind::Completion, 5, cx)
                .expect("second");
            assert!(!editor.set_service_result(first.id, completion(), cx));
            assert!(editor.set_service_result(
                second.id,
                AsyncValue::error("Host refused refresh".into()),
                cx
            ));
            assert!(!editor.accept_service_item("complete-unicode", cx));
        })
    });
    harness.frame();
    assert!(harness.bounds("source.service.complete-unicode").is_some());
    assert!(harness.bounds("source.service.status").is_some());
    harness.update(|_, cx| assert_eq!(entity.read(cx).snapshot(cx).text.as_ref(), "// é tail"));
}

#[gpui::test]
fn service_batches_validate_unicode_overlap_revision_and_host_owned_effects(
    cx: &mut TestAppContext,
) {
    let (mut harness, slot) = editor(cx, "// é tail", |editor| editor.language_services(true));
    let entity = slot.borrow().clone().expect("editor");
    let events = Rc::new(RefCell::new(Vec::new()));
    let received = events.clone();
    let _subscription = harness.update(|_, cx| {
        cx.subscribe(&entity, move |_, event, _| {
            received.borrow_mut().push(event.clone())
        })
    });
    harness.update(|_, cx| {
        entity.update(cx, |editor, cx| {
            assert!(
                editor
                    .request_service(EditorServiceKind::Completion, 4, cx)
                    .is_none()
            );
            let request = editor
                .request_service(EditorServiceKind::Completion, 5, cx)
                .expect("request");
            let mut invalid = completion();
            if let Some(EditorServiceResult::Items(items)) = &mut invalid.value {
                items[0].effect = EditorServiceEffect::Edits(vec![EditorReplacement {
                    range: 4..5,
                    text: "bad".into(),
                }]);
            }
            assert!(!editor.set_service_result(request.id, invalid, cx));
            assert!(!editor.set_semantic_tokens(9, vec![], cx));
            assert!(editor.set_semantic_tokens(
                0,
                vec![EditorSemanticToken {
                    range: 3..5,
                    class: gpui_kit_theme::SyntaxColor::StringLiteral
                }],
                cx
            ));
            assert!(!editor.set_diagnostics(
                0,
                vec![EditorDiagnostic {
                    id: "bad-byte".into(),
                    range: 4..5,
                    message: "invalid".into(),
                    severity: EditorDiagnosticSeverity::Error
                }],
                cx
            ));
            let request = editor
                .request_service(EditorServiceKind::Definition, 5, cx)
                .expect("definition");
            assert!(editor.set_service_result(
                request.id,
                AsyncValue::ready(EditorServiceResult::Items(vec![EditorServiceItem {
                    id: "definition".into(),
                    label: "Declaration".into(),
                    detail: None,
                    effect: EditorServiceEffect::Definition {
                        target: "caller-document".into(),
                        range: 37..41
                    }
                }])),
                cx
            ));
            assert!(editor.accept_service_item("definition", cx));
            let request = editor
                .request_service(EditorServiceKind::CodeActions, 5, cx)
                .expect("actions");
            assert!(editor.set_service_result(
                request.id,
                AsyncValue::ready(EditorServiceResult::Items(vec![EditorServiceItem {
                    id: "fix".into(),
                    label: "Host fix".into(),
                    detail: None,
                    effect: EditorServiceEffect::Action("caller-fix".into())
                }])),
                cx
            ));
            assert!(editor.accept_service_item("fix", cx));
            assert_eq!(editor.snapshot(cx).text.as_ref(), "// é tail");
        })
    });
    assert!(events.borrow().iter().any(|event| matches!(event, EditorEvent::DefinitionRequested { target, range } if target == "caller-document" && *range == (37..41))));
    assert!(events.borrow().iter().any(
        |event| matches!(event, EditorEvent::CodeActionRequested(action) if action == "caller-fix")
    ));
}

#[gpui::test]
fn changed_events_retain_lazy_snapshots_instead_of_eager_whole_values(cx: &mut TestAppContext) {
    let (mut harness, slot) = editor(cx, "old😀tail", |editor| editor.rows(3));
    let entity = slot.borrow().clone().expect("editor");
    let snapshots = Rc::new(RefCell::new(Vec::new()));
    let output = snapshots.clone();
    let _subscription = harness.update(|_, cx| {
        cx.subscribe(&entity, move |_, event, _| {
            if let EditorEvent::Changed(snapshot) = event {
                assert_eq!(snapshot.materialized_bytes(), 0);
                output.borrow_mut().push(snapshot.clone());
            }
        })
    });
    harness.update(|_, cx| {
        let area = entity.read(cx).text_area().clone();
        area.update(cx, |area, cx| {
            area.replace_range(3..7, "界", cx);
        });
    });
    harness.frame();
    let snapshots = snapshots.borrow();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].materialized_bytes(), 0);
    assert_eq!(
        snapshots[0].slice(0..snapshots[0].len()).as_deref(),
        Some("old界tail")
    );
    assert_eq!(snapshots[0].materialized_bytes(), 0);
    assert_eq!(snapshots[0].text().as_ref(), "old界tail");
    assert_eq!(snapshots[0].materialized_bytes(), "old界tail".len());
}

#[gpui::test]
fn multicursor_typing_deletion_and_history_share_the_real_input_surface(cx: &mut TestAppContext) {
    let (mut harness, slot) = editor(cx, "é middle 😀 end", |editor| editor.rows(3));
    let entity = slot.borrow().clone().expect("editor");
    harness.click("source.input");
    harness.update(|_, cx| {
        let area = entity.read(cx).text_area().clone();
        area.update(cx, |area, cx| {
            assert!(area.set_selections([(10..14, true), (0..2, false)], cx))
        });
    });
    harness.keystrokes("q");
    harness.update(|_, cx| {
        let area = entity.read(cx).text_area().read(cx);
        assert_eq!(area.value().as_ref(), "q middle q end");
        assert_eq!(area.selections(), vec![(10..10, false), (1..1, false)]);
    });
    harness.keystrokes("backspace");
    harness.update(|_, cx| assert_eq!(entity.read(cx).snapshot(cx).text.as_ref(), " middle  end"));
    harness.keystrokes(if cfg!(target_os = "macos") {
        "cmd-z cmd-z"
    } else {
        "ctrl-z ctrl-z"
    });
    harness.update(|_, cx| {
        let area = entity.read(cx).text_area().read(cx);
        assert_eq!(area.value().as_ref(), "é middle 😀 end");
        assert_eq!(area.selections(), vec![(10..14, true), (0..2, false)]);
    });
}

#[gpui::test]
fn rectangular_selection_uses_painted_columns_and_clamps_short_rows(cx: &mut TestAppContext) {
    let (mut harness, slot) = editor(cx, "abcdef\nxy\n123456", |editor| editor.rows(4));
    let entity = slot.borrow().clone().expect("editor");
    harness.click("source.input");
    harness.update(|_, cx| {
        let area = entity.read(cx).text_area().clone();
        let anchor = area.read(cx).bounds_for_range(1..2).expect("b")[0];
        let focus = area.read(cx).bounds_for_range(14..15).expect("5")[0];
        let short_column = area.read(cx).bounds_for_range(8..9).expect("y")[0];
        assert_eq!(
            anchor.left(),
            short_column.left(),
            "monospaced columns: {anchor:?}, {short_column:?}"
        );
        area.update(cx, |area, cx| {
            assert!(area.select_rectangle(
                point(anchor.left(), anchor.center().y),
                point(focus.left(), focus.center().y),
                cx
            ))
        });
        assert_eq!(
            area.read(cx).selections(),
            vec![(11..14, false), (8..9, false), (1..4, false)]
        );
    });
    harness.keystrokes("x");
    harness
        .update(|_, cx| assert_eq!(entity.read(cx).snapshot(cx).text.as_ref(), "axef\nxx\n1x56"));
    harness.keystrokes(if cfg!(target_os = "macos") {
        "cmd-z"
    } else {
        "ctrl-z"
    });
    harness.update(|_, cx| {
        let area = entity.read(cx).text_area().read(cx);
        assert_eq!(area.value().as_ref(), "abcdef\nxy\n123456");
        assert_eq!(
            area.selections(),
            vec![(11..14, false), (8..9, false), (1..4, false)]
        );
    });
}

#[gpui::test]
fn wheel_returns_unused_native_axes_to_the_parent(cx: &mut TestAppContext) {
    let slot = EditorSlot::default();
    let build_slot = slot.clone();
    let residuals = Rc::new(RefCell::new(Vec::new()));
    let seen = residuals.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let entity = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    Editor::new(
                        "source",
                        "Source",
                        "one\ntwo\nthree\nfour\nfive\nsix\nseven",
                        window,
                        cx,
                    )
                    .rows(2)
                })
            })
            .clone();
        let seen = seen.clone();
        div()
            .w(px(420.0))
            .h(px(280.0))
            .on_scroll_wheel(move |event, _, _| {
                let ScrollDelta::Lines(delta) = event.delta else {
                    panic!("residual must retain native line units");
                };
                seen.borrow_mut().push(delta);
            })
            .child(entity)
            .into_any_element()
    });
    let entity = slot.borrow().clone().expect("editor");
    harness.update(|_, cx| {
        let area = entity.read(cx).text_area().clone();
        area.update(cx, |area, cx| area.set_selected_range(0..0, cx));
    });
    let at = harness.bounds("source.input").expect("bounds").center();
    let line_height = harness.update(|_, cx| {
        let geometry = entity.read(cx).geometry(cx).expect("geometry");
        let line_height = geometry.lines[0].bounds.size.height;
        assert_eq!(geometry.lines.len(), 7);
        assert!(
            geometry.viewport.size.height < line_height * 7.0,
            "{geometry:?}"
        );
        assert!(
            geometry.viewport.contains(&at),
            "event {at:?}, viewport {geometry:?}"
        );
        line_height
    });
    // Vertical motion fits; the editor has no horizontal overflow. A parent
    // must receive all three native horizontal units and none of the two Y.
    harness.update(|window, cx| {
        window.dispatch_event(
            ScrollWheelEvent {
                position: at,
                delta: ScrollDelta::Lines(point(-3.0, -2.0)),
                modifiers: Modifiers::none(),
                touch_phase: TouchPhase::Moved,
            }
            .to_platform_input(),
            cx,
        );
    });
    assert_eq!(*residuals.borrow(), vec![point(-3.0, 0.0)]);
    residuals.borrow_mut().clear();
    harness.update(|window, cx| {
        assert_eq!(
            entity
                .read(cx)
                .geometry(cx)
                .expect("geometry")
                .vertical_scroll,
            line_height * 2.0
        );
        // Returning past the top consumes exactly two units, leaving three.
        window.dispatch_event(
            ScrollWheelEvent {
                position: at,
                delta: ScrollDelta::Lines(point(1.0, 5.0)),
                modifiers: Modifiers::none(),
                touch_phase: TouchPhase::Moved,
            }
            .to_platform_input(),
            cx,
        );
    });
    assert_eq!(*residuals.borrow(), vec![point(1.0, 3.0)]);
    harness.update(|_, cx| {
        assert_eq!(
            entity
                .read(cx)
                .geometry(cx)
                .expect("geometry")
                .vertical_scroll,
            px(0.0)
        );
    });
}

#[gpui::test]
fn wheel_browses_independently_and_preserves_horizontal_width(cx: &mut TestAppContext) {
    let (mut harness, slot) = editor(cx, "", |editor| editor.rows(3));
    let entity = slot.borrow().clone().expect("editor");
    harness.update(|_, cx| {
        entity.update(cx, |editor, cx| {
            editor.set_value(
                format!("{}\n{}", "wide ".repeat(60), "short\n".repeat(80)),
                cx,
            );
        });
    });
    harness.click("source.input");
    harness.keystrokes(if cfg!(target_os = "macos") {
        "cmd-home"
    } else {
        "ctrl-home"
    });
    let at = harness.bounds("source.input").expect("bounds").center();
    harness.update(|window, cx| {
        window.dispatch_event(
            ScrollWheelEvent {
                position: at,
                delta: ScrollDelta::Pixels(point(px(-37.0), px(-83.0))),
                modifiers: Modifiers::none(),
                touch_phase: TouchPhase::Moved,
            }
            .to_platform_input(),
            cx,
        );
    });
    harness.frame();
    harness.frame();
    harness.update(|_, cx| {
        let editor = entity.read(cx);
        let geometry = editor.geometry(cx).expect("geometry");
        assert_eq!(geometry.horizontal_scroll, px(37.0));
        assert_eq!(geometry.vertical_scroll, px(83.0));
        let area = editor.text_area().read(cx);
        assert_eq!(area.cursor_offset(), 0, "wheel must not move the caret");
        assert!(
            area.shaping_work().expect("layout").shaped_lines <= 4,
            "offscreen caret must not demand another shaped row"
        );
    });
    harness.keystrokes("right");
    harness.update(|_, cx| {
        let editor = entity.read(cx);
        let geometry = editor.geometry(cx).expect("geometry");
        assert_eq!(
            geometry.vertical_scroll,
            px(0.0),
            "navigation reveals again"
        );
        assert!(geometry.horizontal_scroll < px(37.0));
        assert_eq!(editor.text_area().read(cx).cursor_offset(), 1);
    });
    harness.scroll("source.input", 91.0);
    harness.keystrokes("x");
    harness.update(|_, cx| {
        let editor = entity.read(cx);
        assert_eq!(
            editor.geometry(cx).expect("geometry").vertical_scroll,
            px(0.0)
        );
        assert!(editor.snapshot(cx).text.starts_with("wxide"));
    });
}

#[cfg(feature = "syntax")]
#[gpui::test]
fn syntax_tracks_real_area_edits_and_reports_parse_errors(cx: &mut TestAppContext) {
    let (mut harness, slot) = editor(cx, "{\"é\":13}", |editor| {
        editor.syntax(EditorSyntax::json())
    });
    let entity = slot.borrow().clone().expect("editor");
    harness.update(|_, cx| {
        let syntax = entity.read(cx).syntax_state().expect("parser");
        assert_eq!(syntax.revision(), Some(0));
        assert!(!syntax.work().incremental);
        assert!(syntax.errors().is_empty());
    });
    harness.update(|_, cx| {
        let area = entity.read(cx).text_area().clone();
        area.update(cx, |area, cx| {
            assert_eq!(area.replace_range(6..8, "271", cx), Some(6..9));
        });
    });
    harness.update(|_, cx| {
        let syntax = entity.read(cx).syntax_state().expect("parser");
        assert_eq!(syntax.revision(), Some(1));
        assert!(syntax.work().incremental);
        assert!(
            syntax
                .captures(0..10)
                .expect("captures")
                .iter()
                .any(|capture| { capture.name.as_ref() == "number" && capture.range == (6..9) })
        );
    });
    harness.click("source.input");
    harness.keystrokes(if cfg!(target_os = "macos") {
        "cmd-end x"
    } else {
        "ctrl-end x"
    });
    harness.update(|_, cx| {
        let syntax = entity.read(cx).syntax_state().expect("parser");
        assert_eq!(syntax.revision(), Some(2));
        assert!(!syntax.errors().is_empty());
    });
}

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

#[gpui::test]
fn source_viewport_shapes_only_visible_rows_after_edits_and_navigation(cx: &mut TestAppContext) {
    let (mut harness, slot) = editor(cx, "", |editor| editor.rows(4));
    let text = "let asymmetric = '界';\n".repeat(512) + "tail😀";
    let expected_len = text.len();
    let entity = slot.borrow().clone().expect("editor");
    harness.update(|_, cx| entity.update(cx, |editor, cx| editor.set_value(text, cx)));
    harness.frame();
    harness.update(|_, cx| {
        let area = entity.read(cx).text_area().read(cx);
        assert_eq!(area.cursor_row(), 512);
        let work = area.shaping_work().expect("measured layout");
        assert!(work.shaped_lines <= 5, "{work:?}");
        assert!(work.shaped_bytes <= 5 * 24, "{work:?}");
        assert_eq!(area.document().len(), expected_len);
    });
    harness.click("source.input");
    harness.keystrokes(if cfg!(target_os = "macos") {
        "cmd-home"
    } else {
        "ctrl-home"
    });
    harness.keystrokes("down down down down down");
    harness.update(|_, cx| {
        let area = entity.read(cx).text_area().read(cx);
        assert_eq!(area.cursor_row(), 5);
        assert!(area.shaping_work().expect("layout").shaped_lines <= 5);
    });
    harness.keystrokes(if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    });
    harness.update(|_, cx| {
        let area = entity.read(cx).text_area().read(cx);
        assert_eq!(area.selected_range(), 0..expected_len);
        assert!(
            area.shaping_work().expect("layout").shaped_lines <= 5,
            "select-all must not shape all source rows"
        );
    });
}
