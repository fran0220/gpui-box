//! The document vocabulary: tabs that say whether they are written down, a
//! search that says how sure it is of its count, a notification centre that
//! outlives the toasts it holds, a panel that failed rather than emptied, code
//! nobody can edit, and files on their way somewhere.
//!
//! Every assertion goes through the public API and the semantic tree, and
//! every interaction is simulated input rather than a method call standing in
//! for one.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext as _, Entity, IntoElement, Modifiers, MouseButton, MouseDownEvent, SharedString,
    TestAppContext, div, prelude::*,
};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

type Calls<T> = Rc<RefCell<Vec<T>>>;

fn recorder<T: 'static>() -> (Calls<T>, Calls<T>) {
    let calls: Calls<T> = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

/// Presses the middle button over a node, which no harness gesture covers
/// because only this component reads it.
fn middle_click(harness: &mut Harness, id: &str) {
    let at = harness.point_in(id);
    harness.context().simulate_event(MouseDownEvent {
        button: MouseButton::Middle,
        position: at,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    harness.context().run_until_parked();
}

// ---------------------------------------------------------------- document tabs

fn document_tabs(
    cx: &mut TestAppContext,
    closable: bool,
) -> (Harness, Calls<String>, Calls<String>) {
    let (selected, select_sink) = recorder::<String>();
    let (closed, close_sink) = recorder::<String>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let select_sink = select_sink.clone();
        let close_sink = close_sink.clone();
        Tabs::new("editor.tabs")
            .tabs([
                TabItem::new("readme", "README.md").closable(closable),
                TabItem::new("main", "main.rs").dirty().closable(closable),
                TabItem::new("theme", "theme.json")
                    .saving()
                    .closable(closable),
                TabItem::new("notes", "notes.md")
                    .save_failed("The workspace is read-only.")
                    .closable(closable),
            ])
            .selected("main")
            .on_select(move |id, _, _| select_sink.borrow_mut().push(id.to_string()))
            .on_close(move |id, _, _| close_sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });
    (harness, selected, closed)
}

#[gpui::test]
fn a_clean_tab_says_nothing_and_the_other_three_say_different_things(cx: &mut TestAppContext) {
    let (mut harness, _selected, _closed) = document_tabs(cx, true);

    // Silence is the message: a written-down tab wears no mark at all, which
    // is what makes a mark's presence mean something.
    assert!(harness.node("editor.tabs.readme.save").is_none());

    let dirty = harness.node("editor.tabs.main.save").expect("published");
    let saving = harness.node("editor.tabs.theme.save").expect("published");
    let failed = harness.node("editor.tabs.notes.save").expect("published");

    assert_eq!(dirty.value.as_deref(), Some("dirty"));
    assert_eq!(saving.value.as_deref(), Some("saving"));
    assert_eq!(failed.value.as_deref(), Some("save-failed"));

    // A save in flight is busy and a save that failed is invalid; neither is
    // the other, and neither is clean.
    assert!(saving.busy && !saving.invalid);
    assert!(failed.invalid && !failed.busy);
    assert!(!dirty.busy && !dirty.invalid);

    // The host's own reason survives to the tree rather than being replaced
    // by a catalogue word for failure.
    assert_eq!(failed.text.as_deref(), Some("The workspace is read-only."));
}

#[gpui::test]
fn closing_a_tab_does_not_also_switch_to_it(cx: &mut TestAppContext) {
    let (mut harness, selected, closed) = document_tabs(cx, true);

    harness.click("editor.tabs.readme.close");

    assert_eq!(*closed.borrow(), vec!["readme".to_string()]);
    // The close control stops the click travelling, so the gesture that means
    // "switch to this tab" never lands.
    assert!(selected.borrow().is_empty());
}

#[gpui::test]
fn a_middle_click_on_a_tab_puts_it_away(cx: &mut TestAppContext) {
    let (mut harness, selected, closed) = document_tabs(cx, true);

    middle_click(&mut harness, "editor.tabs.theme");

    assert_eq!(*closed.borrow(), vec!["theme".to_string()]);
    assert!(selected.borrow().is_empty());
}

#[gpui::test]
fn a_tab_nobody_can_close_offers_no_control(cx: &mut TestAppContext) {
    let (mut harness, _selected, _closed) = document_tabs(cx, false);

    assert!(harness.node("editor.tabs.readme.close").is_none());
    assert!(harness.node("editor.tabs.main.close").is_none());
}

#[gpui::test]
fn overflowed_tabs_are_relocated_and_not_dropped(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |window, cx| {
        let overflow = cx.new(|cx| Menu::new("editor.tabs.menu", window, cx).trigger("More"));
        Tabs::new("editor.tabs")
            .tabs([
                TabItem::new("one", "adapter.rs"),
                TabItem::new("two", "catalog.rs"),
                TabItem::new("three", "harness.rs"),
                TabItem::new("four", "registry.rs"),
                TabItem::new("five", "transport.rs"),
            ])
            .selected("two")
            .overflow_after(3)
            .overflow_menu(overflow)
            .on_select(|_, _, _| {})
            .into_any_element()
    });

    // Three are drawn.
    assert!(harness.node("editor.tabs.three").is_some());
    assert!(harness.node("editor.tabs.four").is_none());

    // The strip still counts every tab the caller declared, because the
    // keyboard reaches all of them; the overflow group says how many moved.
    assert_eq!(
        harness.node("editor.tabs").expect("published").value,
        Some("5".into())
    );
    let overflow = harness.node("editor.tabs.overflow").expect("published");
    assert_eq!(overflow.value.as_deref(), Some("2"));
}

#[gpui::test]
fn a_hidden_tab_can_still_be_reached_from_the_keyboard(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<String>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let sink = sink.clone();
        let overflow = cx.new(|cx| Menu::new("editor.tabs.menu", window, cx).trigger("More"));
        Tabs::new("editor.tabs")
            .tabs([
                TabItem::new("one", "adapter.rs"),
                TabItem::new("two", "catalog.rs"),
                TabItem::new("three", "harness.rs"),
                TabItem::new("four", "registry.rs"),
            ])
            // The last drawn tab, so the next step lands on a hidden one.
            .selected("three")
            .overflow_after(3)
            .overflow_menu(overflow)
            .on_select(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });

    harness.click("editor.tabs.three");
    calls.borrow_mut().clear();
    harness.keystrokes("right");

    // Arrowing past the last drawn tab reaches the tab that overflowed, which
    // is the whole reason the strip keeps them in its keyboard order.
    assert_eq!(*calls.borrow(), vec!["four".to_string()]);
}

// ---------------------------------------------------------------------- search

fn search(cx: &mut TestAppContext, count: HitCount) -> (Harness, Entity<SearchField>) {
    let held: Rc<RefCell<Option<Entity<SearchField>>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let field = sink
            .borrow_mut()
            .get_or_insert_with(|| {
                let count = count.clone();
                cx.new(|cx| {
                    let mut field = SearchField::new("find", window, cx);
                    field.set_count(count, cx);
                    field
                })
            })
            .clone();
        field.into_any_element()
    });
    let field = held.borrow().clone().expect("built");
    (harness, field)
}

#[gpui::test]
fn counting_is_not_none_and_too_many_is_not_a_total(cx: &mut TestAppContext) {
    let (mut counting, _) = search(cx, HitCount::Counting);
    assert_eq!(
        counting.node("find.count").expect("published").value,
        Some("counting".into())
    );

    let (mut none, _) = search(cx, HitCount::None);
    assert_eq!(
        none.node("find.count").expect("published").value,
        Some("none".into())
    );

    let (mut many, _) = search(cx, HitCount::TooMany { counted: 500 });
    let node = many.node("find.count").expect("published");
    assert_eq!(node.value.as_deref(), Some("too-many"));
    // "At least 500" is published as its own state rather than as the total
    // 500, which nothing has established.
    assert_ne!(node.value.as_deref(), Some("known"));
}

#[gpui::test]
fn a_search_that_found_nothing_cannot_be_stepped_through(cx: &mut TestAppContext) {
    let (mut harness, _field) = search(cx, HitCount::None);

    // A step with nowhere to go installs no handler at all.
    assert!(harness.node("find.next").expect("published").disabled);
    assert!(harness.node("find.previous").expect("published").disabled);
}

#[gpui::test]
fn a_host_that_could_not_search_says_so_in_its_own_words(cx: &mut TestAppContext) {
    let (mut harness, _field) = search(
        cx,
        HitCount::Unavailable("The index is still building.".into()),
    );
    let node = harness.node("find.count").expect("published");

    assert_eq!(node.value.as_deref(), Some("unavailable"));
    assert_eq!(node.text.as_deref(), Some("The index is still building."));
}

#[gpui::test]
fn stepping_reports_and_moves_nothing(cx: &mut TestAppContext) {
    let (mut harness, field) = search(
        cx,
        HitCount::Known {
            total: 12,
            current: Some(2),
        },
    );
    let (calls, sink) = recorder::<SearchFieldEvent>();
    harness.update(move |_, cx| {
        cx.subscribe(&field, move |_, event: &SearchFieldEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });

    harness.click("find.next");

    assert_eq!(*calls.borrow(), vec![SearchFieldEvent::Next]);
    // The count did not move: the host owns where the caret is.
    assert_eq!(
        harness
            .node("find.count")
            .expect("published")
            .text
            .as_deref(),
        Some("3 of 12")
    );
}

// -------------------------------------------------------------- find & replace

/// A find-and-replace surface built once and held, so a subscription taken
/// after the first frame is still watching the entity the controls report to.
fn find_replace(cx: &mut TestAppContext, count: HitCount) -> (Harness, Calls<FindReplaceEvent>) {
    let held: Rc<RefCell<Option<Entity<FindReplace>>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let count = count.clone();
        sink.borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    let mut surface = FindReplace::new("replace", window, cx);
                    surface.set_count(count, cx);
                    surface
                })
            })
            .clone()
            .into_any_element()
    });
    let surface = held.borrow().clone().expect("built");

    let (calls, events) = recorder::<FindReplaceEvent>();
    harness.update(move |_, cx| {
        cx.subscribe(&surface, move |_, event: &FindReplaceEvent, _| {
            events.borrow_mut().push(event.clone());
        })
        .detach();
    });
    (harness, calls)
}

#[gpui::test]
fn replace_all_states_its_count_before_it_is_taken(cx: &mut TestAppContext) {
    let (mut harness, calls) = find_replace(
        cx,
        HitCount::Known {
            total: 12,
            current: Some(0),
        },
    );

    let control = harness.node("replace.replace-all").expect("published");
    // The number is on the control itself, so nobody agrees to a change
    // whose size they were told afterwards.
    assert_eq!(control.text.as_deref(), Some("Replace all 12"));

    harness.click("replace.replace-all");

    assert_eq!(
        *calls.borrow(),
        vec![FindReplaceEvent::ReplaceAll { count: 12 }]
    );
}

#[gpui::test]
fn replace_all_refuses_a_count_nobody_established(cx: &mut TestAppContext) {
    let (mut harness, calls) = find_replace(cx, HitCount::TooMany { counted: 500 });

    // A count that stopped early is not a count, so the control that would
    // change that many installs no handler.
    assert!(
        harness
            .node("replace.replace-all")
            .expect("published")
            .disabled
    );
    // And the surface says why rather than leaving a dead control unexplained.
    assert!(harness.node("replace.replace-all.reason").is_some());

    harness.click("replace.replace-all");
    assert!(calls.borrow().is_empty());
}

// ------------------------------------------------------------------- highlight

#[gpui::test]
fn a_range_naming_nothing_real_costs_its_mark_and_not_the_line(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        HighlightedText::new("The transport reports what it did.")
            .id("line")
            .hits([4..13, 400..500])
            .current(0)
            .into_any_element()
    });

    let node = harness.node("line").expect("published");
    // The text is intact.
    assert_eq!(
        node.text.as_deref(),
        Some("The transport reports what it did.")
    );
    // And the tree publishes the mark that was drawn, not the two asked for.
    assert_eq!(node.value.as_deref(), Some("1"));
}

// ---------------------------------------------------------- notification centre

fn centre(cx: &mut TestAppContext) -> (Harness, Entity<NotificationCenter>) {
    let held: Rc<RefCell<Option<Entity<NotificationCenter>>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |_, cx| {
        let centre = sink
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    let mut centre = NotificationCenter::new("notifications", cx);
                    centre.record(
                        Notification::new("run.exported", "Theme exported to disk")
                            .tone(Tone::Success)
                            .read(true),
                        cx,
                    );
                    centre.record(
                        Notification::new("run.refused", "The host refused to publish this run")
                            .tone(Tone::Danger),
                        cx,
                    );
                    centre
                })
            })
            .clone();
        centre.into_any_element()
    });
    let centre = held.borrow().clone().expect("built");
    (harness, centre)
}

#[gpui::test]
fn the_badge_counts_only_what_the_centre_still_holds(cx: &mut TestAppContext) {
    let (mut harness, centre) = centre(cx);

    assert_eq!(
        harness.node("notifications").expect("published").value,
        Some("1".into())
    );

    harness.update(|_, cx| {
        centre.update(cx, |centre, cx| centre.mark_all_read(cx));
    });

    // Nothing unread, so nothing claimed.
    assert_eq!(
        harness.node("notifications").expect("published").value,
        Some("0".into())
    );
}

#[gpui::test]
fn a_centre_that_dropped_records_stops_claiming_an_exact_count(cx: &mut TestAppContext) {
    let held: Rc<RefCell<Option<Entity<NotificationCenter>>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, cx| {
        sink.borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| NotificationCenter::new("notifications", cx).capacity(2))
            })
            .clone()
            .into_any_element()
    });
    let centre = held.borrow().clone().expect("built");

    harness.update(|_, cx| {
        centre.update(cx, |centre, cx| {
            for index in 0..3 {
                centre.record(
                    Notification::new(format!("run.{index}"), format!("Run {index} finished")),
                    cx,
                );
            }
        });
    });

    // Two are held and both are unread, but a third was dropped and nobody
    // knows whether it had been read, so the badge says "at least".
    assert_eq!(
        harness.update(|_, cx| centre.read(cx).unread()),
        UnreadCount::AtLeast(2)
    );
    assert_eq!(
        harness.node("notifications").expect("published").value,
        Some("2+".into())
    );
}

#[gpui::test]
fn dismissing_one_is_not_clearing_them_all(cx: &mut TestAppContext) {
    let (mut harness, centre) = centre(cx);

    harness.click("run.refused.dismiss");

    assert!(harness.update(|_, cx| !centre.read(cx).holds("run.refused")));
    // The other one is untouched.
    assert!(harness.update(|_, cx| centre.read(cx).holds("run.exported")));

    harness.click("notifications.clear-all");
    assert!(harness.update(|_, cx| centre.read(cx).is_empty()));
}

#[gpui::test]
fn a_notification_publishes_its_severity_and_whether_it_was_read(cx: &mut TestAppContext) {
    let (mut harness, _centre) = centre(cx);

    let read = harness.node("run.exported").expect("published");
    let unread = harness.node("run.refused").expect("published");

    assert_eq!(read.checked, Some(true));
    assert_eq!(unread.checked, Some(false));
    assert_eq!(read.value.as_deref(), Some("success"));
    assert_eq!(unread.value.as_deref(), Some("danger"));
}

// --------------------------------------------------------------- failure panel

#[gpui::test]
fn a_failed_panel_is_not_an_empty_one(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<()>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        let held: Result<(), &str> = Err("The workspace refused this query.");
        div()
            .children(FailurePanel::from_result("runs", &held).map(|panel| {
                panel
                    .title("Runs")
                    .attempts(3)
                    .on_retry(move |_, _| sink.borrow_mut().push(()))
            }))
            .into_any_element()
    });

    let panel = harness.node("runs").expect("published");
    assert_eq!(panel.value.as_deref(), Some("failed"));
    assert!(panel.invalid);

    // The host's sentence survives word for word.
    assert_eq!(
        harness
            .node("runs.reason")
            .expect("published")
            .text
            .as_deref(),
        Some("The workspace refused this query.")
    );
    // And a retry that has never once worked says how many times it has been
    // tried rather than looking like a first attempt.
    assert_eq!(
        harness.node("runs.attempts").expect("published").value,
        Some("3".into())
    );

    harness.click("runs.retry");
    // Retrying belongs to the host: the panel reports and does nothing.
    assert_eq!(calls.borrow().len(), 1);
}

#[gpui::test]
fn a_panel_with_nothing_to_retry_offers_no_control(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        FailurePanel::new("runs", "The workspace refused this query.").into_any_element()
    });

    assert!(harness.node("runs.retry").is_none());
}

// ------------------------------------------------------------------- code view

fn code_lines() -> Vec<CodeLine> {
    vec![
        CodeLine::new(40, "fn report(&self) -> Outcome {"),
        CodeLine::new(41, "    let verified = self.check();").mark(LineMark::Added),
        CodeLine::new(42, "    let stale = self.cached();").mark(LineMark::Removed),
        CodeLine::new(43, "}").mark(LineMark::Error),
    ]
}

#[gpui::test]
fn a_code_view_keeps_the_line_numbers_the_file_has(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        CodeView::new("hunk", code_lines())
            .language("rust")
            .into_any_element()
    });

    // Numbered by the file, not by position in the slice that was handed in,
    // so a review comment on line 41 finds line 41.
    assert!(harness.node("hunk.lines.line-41").is_some());
    assert!(harness.node("hunk.lines.line-1").is_none());
    assert_eq!(
        harness.node("hunk").expect("published").value,
        Some("4".into())
    );
}

#[gpui::test]
fn only_a_marked_line_is_an_assertion_target(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        CodeView::new("hunk", code_lines()).into_any_element()
    });

    // An unmarked line publishes nothing: a thousand of them would bury every
    // other node under rows that only repeat their own text.
    assert!(harness.node("hunk.lines.line-40").is_none());

    let added = harness.node("hunk.lines.line-41").expect("published");
    let removed = harness.node("hunk.lines.line-42").expect("published");
    let failing = harness.node("hunk.lines.line-43").expect("published");

    assert_eq!(added.value.as_deref(), Some("added"));
    assert_eq!(removed.value.as_deref(), Some("removed"));
    assert_eq!(failing.value.as_deref(), Some("error"));
    // Only the error is a fault; a removed line is a difference.
    assert!(failing.invalid && !removed.invalid);
}

#[gpui::test]
fn a_code_view_with_nothing_in_it_copies_nothing(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        CodeView::new("hunk", []).into_any_element()
    });

    assert!(harness.node("hunk.copy").expect("published").disabled);
    assert!(harness.node("hunk.empty").is_some());
}

// ----------------------------------------------------------------- upload list

fn uploads() -> Vec<Upload> {
    vec![
        Upload::new("brief", "brief.pdf").done(),
        Upload::new("capture", "capture.png").uploading(0.4),
        Upload::new("archive", "archive.zip").failed("The connection dropped."),
        Upload::new("installer", "installer.exe").refused("This zone does not take programs."),
    ]
}

fn upload_list(cx: &mut TestAppContext) -> (Harness, Calls<SharedString>) {
    let (calls, sink) = recorder::<SharedString>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        UploadList::new("attachments")
            .uploads(uploads())
            .on_retry(move |id, _, _| sink.borrow_mut().push(id))
            .on_cancel(|_, _, _| {})
            .on_remove(|_, _, _| {})
            .into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn a_refusal_is_not_a_failure(cx: &mut TestAppContext) {
    let (mut harness, _calls) = upload_list(cx);

    let failed = harness.node("attachments.archive").expect("published");
    let refused = harness.node("attachments.installer").expect("published");

    assert_eq!(failed.value.as_deref(), Some("failed"));
    assert_eq!(refused.value.as_deref(), Some("refused"));
    // A file that broke is invalid; a file the host declined is not broken.
    assert!(failed.invalid);
    assert!(!refused.invalid && refused.disabled);
}

#[gpui::test]
fn only_a_failure_is_offered_a_retry(cx: &mut TestAppContext) {
    let (mut harness, calls) = upload_list(cx);

    // Trying the same file against the same rule cannot end differently, so
    // no control exists at all rather than one that could not work.
    assert!(harness.node("attachments.installer.retry").is_none());
    assert!(harness.node("attachments.brief.retry").is_none());

    harness.click("attachments.archive.retry");
    assert_eq!(*calls.borrow(), vec![SharedString::from("archive")]);
}

#[gpui::test]
fn a_file_in_flight_can_be_stopped_and_a_settled_one_removed(cx: &mut TestAppContext) {
    let (mut harness, _calls) = upload_list(cx);

    assert!(harness.node("attachments.capture.cancel").is_some());
    assert!(harness.node("attachments.capture.remove").is_none());

    assert!(harness.node("attachments.brief.remove").is_some());
    assert!(harness.node("attachments.brief.cancel").is_none());
}

#[gpui::test]
fn a_batch_with_an_unknown_extent_shows_no_percentage(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        UploadList::new("attachments")
            .uploads([
                Upload::new("a", "a.bin").uploading(None),
                Upload::new("b", "b.bin").done(),
            ])
            .into_any_element()
    });

    let bar = harness.node("attachments.overall").expect("published");
    // Indeterminate rather than a number assembled out of a file count.
    assert_eq!(bar.value_now, None);
}

#[gpui::test]
fn a_disabled_list_installs_no_handler_at_all(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<SharedString>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        UploadList::new("attachments")
            .uploads(uploads())
            .on_retry(move |id, _, _| sink.borrow_mut().push(id))
            .disabled(true)
            .into_any_element()
    });

    assert!(harness.node("attachments").expect("published").disabled);
    assert!(harness.node("attachments.archive.retry").is_none());
    assert!(calls.borrow().is_empty());
}
