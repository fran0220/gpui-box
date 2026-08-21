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
use gpui_kit::content::Document;
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;
use gpui_kit_theme::SyntaxColor;

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
fn a_code_view_lays_its_lines_out_one_below_the_next(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        CodeView::new("hunk", code_lines())
            .language("rust")
            .into_any_element()
    });

    let added = harness.bounds("hunk.lines.line-41").expect("laid out");
    let removed = harness.bounds("hunk.lines.line-42").expect("laid out");

    assert!(
        added.size.height > gpui::px(0.0),
        "a line that is on screen occupies space"
    );
    assert!(
        removed.origin.y >= added.origin.y + added.size.height,
        "line 42 sits below line 41 rather than on top of it"
    );
}

#[gpui::test]
fn a_code_view_with_nothing_in_it_copies_nothing(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        CodeView::new("hunk", []).into_any_element()
    });

    assert!(harness.node("hunk.copy").expect("published").disabled);
    assert!(harness.node("hunk.empty").is_some());
}

// -------------------------------------------------------------- developer data

fn fixture_logs(count: usize) -> Vec<LogEntry> {
    (0..count)
        .map(|index| {
            LogEntry::new(
                format!("entry-{index:04}"),
                format!("Fixture log message {index:04}"),
            )
            .timestamp(format!("09:41:{:02}", index % 60))
            .level("INFO", Tone::Info)
            .source("fixture")
        })
        .collect()
}

#[gpui::test]
fn a_log_stream_virtualizes_stable_entries_and_reports_intents(cx: &mut TestAppContext) {
    let (selected, select_sink) = recorder::<String>();
    let (copied, copy_sink) = recorder::<String>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let select_sink = select_sink.clone();
        let copy_sink = copy_sink.clone();
        LogStream::new("logs", fixture_logs(1000))
            .visible_rows(6)
            .selected("entry-0999")
            .on_select(move |id, _, _| select_sink.borrow_mut().push(id.to_string()))
            .on_copy(move |id, _, _| copy_sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });

    assert_eq!(
        harness.node("logs.entries").expect("published").value,
        Some("1000".into())
    );
    assert!(harness.node("logs.entries.entry-0999").is_some());
    assert!(
        harness.node("logs.entries.entry-0000").is_none(),
        "following opens on the newest fixed rows"
    );

    harness.click("logs.entries.entry-0998");
    harness.click("logs.copy");
    assert_eq!(*selected.borrow(), vec!["entry-0998".to_string()]);
    assert_eq!(*copied.borrow(), vec!["entry-0999".to_string()]);
    assert!(
        harness
            .node("logs.entries.entry-0999")
            .expect("published")
            .selected,
        "selection remains the caller's"
    );
}

#[gpui::test]
fn pausing_a_log_changes_only_transient_follow_state(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        LogStream::new("logs", fixture_logs(8))
            .visible_rows(4)
            .into_any_element()
    });

    let following = harness.node("logs.mode").expect("published");
    assert_eq!(following.value.as_deref(), Some("following"));
    assert_eq!(
        harness.node("logs.follow").expect("published").checked,
        Some(true)
    );

    harness.click("logs.follow");

    let paused = harness.node("logs.mode").expect("published");
    assert_eq!(paused.value.as_deref(), Some("paused"));
    assert_eq!(
        harness.node("logs.follow").expect("published").checked,
        Some(false)
    );
    assert_eq!(
        harness.node("logs.entries").expect("still present").value,
        Some("8".into())
    );
}

#[gpui::test]
fn log_states_do_not_collapse_and_stale_keeps_verified_entries(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .column()
            .children([
                LogStream::new("loading", []).state(LogStreamState::Loading),
                LogStream::new("empty", []).state(LogStreamState::Empty),
                LogStream::new("unavailable", []).state(LogStreamState::Unavailable(
                    "The fixture source is offline.".into(),
                )),
                LogStream::new("error", []).state(LogStreamState::Error(
                    "The fixture response was unreadable.".into(),
                )),
                LogStream::new("stale", fixture_logs(2))
                    .state(LogStreamState::Stale("The fixture refresh failed.".into())),
            ])
            .into_any_element()
    });

    for (id, state) in [
        ("loading", "loading"),
        ("empty", "empty"),
        ("unavailable", "unavailable"),
        ("error", "error"),
        ("stale", "stale"),
    ] {
        assert_eq!(
            harness.node(id).expect("published").value.as_deref(),
            Some(state)
        );
    }
    assert!(harness.node("loading").expect("published").busy);
    assert!(harness.node("error").expect("published").invalid);
    assert!(harness.node("stale.entries.entry-0001").is_some());
    assert_eq!(
        harness
            .node("stale.stale")
            .expect("published")
            .text
            .as_deref(),
        Some("The fixture refresh failed.")
    );
}

#[gpui::test]
fn log_search_hits_are_caller_ranges_and_payloads_are_not_published(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        LogStream::new(
            "logs",
            [LogEntry::new("match", "fixture ready")
                .level("INFO", Tone::Info)
                .search_hits([0..7, 40..50])],
        )
        .into_any_element()
    });

    assert_eq!(
        harness
            .node("logs.entries.match.hits")
            .expect("published")
            .value,
        Some("1".into())
    );
    assert_eq!(
        harness
            .node("logs.entries.match")
            .expect("published")
            .text
            .as_deref(),
        Some("INFO"),
        "the log payload does not enter diagnostic snapshots"
    );
}

fn fixture_diff(lines: usize) -> Vec<DiffFile> {
    let lines = (0..lines).map(|index| {
        let line = DiffLine::new(
            format!("line-{index:04}"),
            format!("fixture line {index:04}"),
        );
        match index % 3 {
            0 => line.old_number(index + 10).new_number(index + 10),
            1 => DiffLine::added(
                format!("line-{index:04}"),
                format!("fixture line {index:04}"),
            )
            .new_number(index + 10),
            _ => DiffLine::removed(
                format!("line-{index:04}"),
                format!("fixture line {index:04}"),
            )
            .old_number(index + 10),
        }
    });
    vec![DiffFile::new(
        "report",
        "fixture/report.rs",
        [DiffHunk::new("body", "@@ fixture @@", lines)],
    )]
}

#[gpui::test]
fn a_diff_reports_file_hunk_and_line_identity_without_applying_anything(cx: &mut TestAppContext) {
    let (events, sink) = recorder::<DiffViewEvent>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        DiffView::new("review", fixture_diff(3))
            .visible_rows(5)
            .on_event(move |event, _, _| sink.borrow_mut().push(event))
            .into_any_element()
    });

    harness.click("review.rows.file.report");
    harness.click("review.rows.file.report.hunk.body");
    harness.click("review.rows.file.report.hunk.body.line.line-0001");

    assert_eq!(
        events.borrow().as_slice(),
        [
            DiffViewEvent::FileActivated {
                file_id: "report".into(),
            },
            DiffViewEvent::HunkActivated {
                file_id: "report".into(),
                hunk_id: "body".into(),
            },
            DiffViewEvent::LineActivated {
                file_id: "report".into(),
                hunk_id: "body".into(),
                line_id: "line-0001".into(),
            },
        ]
    );
    assert_eq!(
        harness.node("review").expect("published").value.as_deref(),
        Some("unified")
    );
    assert!(harness.node("review").expect("published").read_only);
}

#[gpui::test]
fn split_diff_uses_the_same_stable_rows_and_large_diffs_stay_virtual(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        DiffView::new("review", fixture_diff(1000))
            .presentation(DiffPresentation::Split)
            .visible_rows(6)
            .on_event(|_, _, _| {})
            .into_any_element()
    });

    assert_eq!(
        harness.node("review").expect("published").value.as_deref(),
        Some("split")
    );
    assert_eq!(
        harness.node("review.rows").expect("published").value,
        Some("1002".into())
    );
    assert!(
        harness
            .node("review.rows.file.report.hunk.body.line.line-0000")
            .is_some()
    );
    assert!(
        harness
            .node("review.rows.file.report.hunk.body.line.line-0900")
            .is_none(),
        "a far line is neither laid out nor published"
    );
}

#[gpui::test]
fn sparkline_publishes_the_callers_exact_reading_and_range(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        Sparkline::new(
            "throughput",
            "Fixture throughput",
            SparklineState::Ready(SparklineReading::new(
                [
                    SparklinePoint::new(0.0, 0.2),
                    SparklinePoint::new(0.5, 0.7),
                    SparklinePoint::new(1.0, 0.4),
                ],
                "40 req/s",
                "20 req/s",
                "70 req/s",
            )),
        )
        .into_any_element()
    });

    let node = harness.node("throughput").expect("published");
    assert_eq!(node.role, Role::Image);
    assert_eq!(node.text.as_deref(), Some("Fixture throughput"));
    assert_eq!(node.value.as_deref(), Some("40 req/s"));
    assert_eq!(
        node.description.as_deref(),
        Some("Minimum 20 req/s; maximum 70 req/s")
    );
    assert!(node.read_only);
}

#[gpui::test]
fn sparkline_states_are_distinct_and_stale_keeps_the_verified_reading(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .column()
            .children([
                Sparkline::new("loading", "Loading metric", SparklineState::Loading),
                Sparkline::new("empty", "Empty metric", SparklineState::Empty),
                Sparkline::new(
                    "unavailable",
                    "Unavailable metric",
                    SparklineState::Unavailable("The fixture has no sampler.".into()),
                ),
                Sparkline::new(
                    "error",
                    "Failed metric",
                    SparklineState::Error("The fixture response was unreadable.".into()),
                ),
                Sparkline::new(
                    "stale",
                    "Stale metric",
                    SparklineState::Stale {
                        reading: SparklineReading::new(
                            [SparklinePoint::new(0.0, 0.2), SparklinePoint::new(1.0, 0.8)],
                            "8 jobs",
                            "2 jobs",
                            "9 jobs",
                        ),
                        reason: "The fixture refresh failed.".into(),
                    },
                ),
            ])
            .into_any_element()
    });

    assert!(harness.node("loading").expect("published").busy);
    assert_eq!(
        harness.node("loading").expect("published").value.as_deref(),
        Some("loading")
    );
    assert_eq!(
        harness.node("empty").expect("published").value.as_deref(),
        Some("empty")
    );
    assert_eq!(
        harness
            .node("unavailable")
            .expect("published")
            .value
            .as_deref(),
        Some("unavailable")
    );
    let error = harness.node("error").expect("published");
    assert!(error.invalid);
    assert_eq!(error.value.as_deref(), Some("error"));
    assert_eq!(
        error.description.as_deref(),
        Some("The fixture response was unreadable.")
    );
    assert_eq!(
        harness.node("stale").expect("published").value.as_deref(),
        Some("8 jobs")
    );
    assert_eq!(
        harness
            .node("stale.stale")
            .expect("published")
            .value
            .as_deref(),
        Some("stale")
    );
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

// --------------------------------------------------- selection across a document

/// Three text blocks in reading order, each with its own semantic node, which
/// is what lets a test point at one and drag to another.
fn selectable_document(cx: &mut TestAppContext) -> Harness {
    Harness::new(cx, gpui_kit::install, |_, _| {
        AgentDocument::new("report")
            .block(AgentDocumentBlock::text("first", "alpha bravo charlie"))
            .block(AgentDocumentBlock::text("second", "delta echo foxtrot"))
            .block(AgentDocumentBlock::text("third", "golf hotel india"))
            .into_any_element()
    })
}

/// Presses at one fraction across a block and releases at a fraction across
/// another.
///
/// A block is as wide as the document, so its centre is well past the end of
/// a short line: the leading edge is the start of the text and the trailing
/// edge is its end. Selecting both blocks whole therefore means aiming at the
/// outer edges of the pair, whichever way the drag runs.
fn drag_between_blocks(harness: &mut Harness, from: (&str, f32), to: (&str, f32)) {
    let start = harness.point_across(from.0, from.1);
    harness.context().simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: start,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    harness.context().run_until_parked();
    let end = harness.point_across(to.0, to.1);
    harness.drag_to(end);
    harness.drop_here();
}

#[gpui::test]
fn a_drag_across_blocks_selects_every_block_it_passed(cx: &mut TestAppContext) {
    let mut harness = selectable_document(cx);

    drag_between_blocks(
        &mut harness,
        ("report.block.first", 0.02),
        ("report.block.third", 0.98),
    );

    let copy = harness
        .update(|window, _| window.document_selection_text())
        .expect("a drag across three blocks selects something");
    assert_eq!(
        copy.participants, 3,
        "the block between the two ends is part of the selection without being pressed"
    );
    assert!(
        copy.text.contains("delta echo foxtrot"),
        "the middle block is copied in full, not sampled: {:?}",
        copy.text
    );
    assert!(
        copy.complete,
        "every participant was mounted, so the copy is not a partial reading"
    );
}

#[gpui::test]
fn nested_markdown_keeps_its_place_between_document_blocks(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        AgentDocument::new("report")
            .block(AgentDocumentBlock::text("first", "opening block"))
            .block(AgentDocumentBlock::markdown(
                "middle",
                "**second** and *third* plus `fourth` after",
            ))
            .block(AgentDocumentBlock::text("last", "closing block"))
            .into_any_element()
    });

    drag_between_blocks(
        &mut harness,
        ("report.block.first", 0.02),
        ("report.block.last", 0.98),
    );

    let copy = harness
        .update(|window, _| window.document_selection_text())
        .expect("the mixed document is selected");
    let markdown_end = copy
        .text
        .find("after")
        .expect("the final Markdown run was copied");
    let closing = copy
        .text
        .find("closing block")
        .expect("the closing block was copied");
    assert!(
        markdown_end < closing,
        "all nested Markdown runs stay before the sibling block that follows them: {:?}",
        copy.text
    );
}

#[gpui::test]
fn a_backwards_drag_reads_the_same_span_as_a_forwards_one(cx: &mut TestAppContext) {
    // The same two points, pressed in each order. A selection is a span, not a
    // direction, so the reading is the same either way.
    let forwards = {
        let mut harness = selectable_document(cx);
        drag_between_blocks(
            &mut harness,
            ("report.block.first", 0.05),
            ("report.block.third", 0.05),
        );
        harness
            .update(|window, _| window.document_selection_text())
            .expect("a downward drag selects")
    };

    let backwards = {
        let mut harness = selectable_document(cx);
        drag_between_blocks(
            &mut harness,
            ("report.block.third", 0.05),
            ("report.block.first", 0.05),
        );
        harness
            .update(|window, _| window.document_selection_text())
            .expect("an upward drag selects")
    };

    assert_eq!(forwards, backwards);
    assert!(
        forwards.text.contains("delta echo foxtrot"),
        "the block between the two ends is read whole in both directions: {:?}",
        forwards.text
    );
}

#[gpui::test]
fn selecting_all_reaches_the_whole_document(cx: &mut TestAppContext) {
    let mut harness = selectable_document(cx);

    // A press is what gives the text focus; the shortcut then acts on the
    // document rather than on the pressed block.
    harness.drag_start("report.block.second");
    harness.drop_here();
    harness.keystrokes("cmd-a");

    let copy = harness
        .update(|window, _| window.document_selection_text())
        .expect("select all selects");
    assert_eq!(copy.participants, 3);
    assert!(copy.text.contains("alpha bravo charlie"));
    assert!(copy.text.contains("golf hotel india"));
}

#[gpui::test]
fn a_selection_is_dismissed_by_a_press_that_lands_on_no_text(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w_full()
            .h(gpui::px(600.0))
            .child(
                AgentDocument::new("report")
                    .block(AgentDocumentBlock::text("first", "alpha bravo charlie"))
                    .block(AgentDocumentBlock::text("second", "delta echo foxtrot")),
            )
            .into_any_element()
    });

    drag_between_blocks(
        &mut harness,
        ("report.block.first", 0.02),
        ("report.block.second", 0.98),
    );
    assert!(harness.update(|window, _| window.document_selection_text().is_some()));

    // Well below the last block, where no participant sits.
    let empty = harness.point_in("report.block.second") + gpui::point(gpui::px(0.), gpui::px(300.));
    harness.context().simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: empty,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    harness.context().run_until_parked();

    assert!(
        harness.update(|window, _| window.document_selection().is_empty()),
        "a press with nothing under it puts the selection down"
    );
}

#[gpui::test]
fn a_drag_inside_an_overlay_does_not_reach_the_page_behind_it(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w_full()
            .child(AgentDocument::new("page").block(AgentDocumentBlock::text(
                "body",
                "page text nobody selected",
            )))
            .child(
                Overlay::modal("dialog").child(
                    AgentDocument::new("dialog.document")
                        .block(AgentDocumentBlock::text("top", "dialog first line"))
                        .block(AgentDocumentBlock::text("bottom", "dialog second line")),
                ),
            )
            .into_any_element()
    });

    drag_between_blocks(
        &mut harness,
        ("dialog.document.block.top", 0.02),
        ("dialog.document.block.bottom", 0.98),
    );

    let copy = harness
        .update(|window, _| window.document_selection_text())
        .expect("the dialog's own text is selectable");
    assert_eq!(
        copy.participants, 2,
        "only the dialog's blocks took part: {:?}",
        copy.text
    );
    assert!(
        !copy.text.contains("page text nobody selected"),
        "the page behind a dialog is a different document: {:?}",
        copy.text
    );
}

/// An answer with four top-level blocks: prose, a fence, a list, and prose.
const LONG_ANSWER: &str = "The opening paragraph of a long answer.\n\n\
```rust\nfn main() {}\n```\n\n\
- one\n- two\n\n\
The closing paragraph of a long answer.\n";

/// What the document calls the row it drew the block starting at `start` in.
///
/// The name is the offset rather than the ordinal, so this is how a caller
/// finds a row too: from the ranges the parser cut.
fn part_row(block: &str, start: usize) -> String {
    format!("report.block.{block}.part-at-{start}")
}

/// Where each top-level block of the answer begins.
fn answer_starts() -> Vec<usize> {
    Document::block_ranges(LONG_ANSWER)
        .expect("the fixture answer is cuttable")
        .into_iter()
        .map(|range| range.start)
        .collect()
}

#[gpui::test]
fn a_virtualized_document_draws_a_long_answer_as_a_row_per_block(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        AgentDocument::new("report")
            .block(AgentDocumentBlock::markdown("answer", LONG_ANSWER))
            .virtualized(8)
            .into_any_element()
    });

    assert!(
        harness.node("report.block.answer").is_none(),
        "a split block is published as the rows it was drawn as, not as itself as well"
    );
    let starts = answer_starts();
    assert_eq!(starts.len(), 4, "the fixture answer has four blocks");
    for start in &starts {
        assert!(
            harness.node(&part_row("answer", *start)).is_some(),
            "every top-level block of the answer is its own row"
        );
    }
    assert!(
        harness
            .node(&part_row("answer", LONG_ANSWER.len()))
            .is_none(),
        "and there are no rows the answer does not have blocks for"
    );
}

#[gpui::test]
fn only_the_row_a_stream_is_writing_into_is_still_arriving(cx: &mut TestAppContext) {
    // The blocks before the one being written closed the moment the next one
    // opened. Reporting all of them as busy would say the whole answer is in
    // motion when one paragraph of it is.
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        AgentDocument::new("report")
            .block(AgentDocumentBlock::markdown("answer", LONG_ANSWER).streaming(true))
            .virtualized(8)
            .into_any_element()
    });

    let starts = answer_starts();
    assert!(
        !harness
            .node(&part_row("answer", starts[0]))
            .expect("drawn")
            .busy
    );
    assert!(
        harness
            .node(&part_row("answer", starts[3]))
            .expect("drawn")
            .busy
    );
}

#[gpui::test]
fn a_drag_down_a_split_answer_reads_it_in_the_order_it_was_written(cx: &mut TestAppContext) {
    // Each row is its own Markdown document, and a document draws its runs
    // from its own reading order. Without a partition per row they would all
    // start at zero and a copy would come back interleaved.
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        AgentDocument::new("report")
            .block(AgentDocumentBlock::markdown("answer", LONG_ANSWER))
            .virtualized(8)
            .into_any_element()
    });

    let starts = answer_starts();
    drag_between_blocks(
        &mut harness,
        (&part_row("answer", starts[0]), 0.0),
        (&part_row("answer", starts[3]), 0.98),
    );

    let copy = harness
        .update(|window, _| window.document_selection_text())
        .expect("the rows of a split answer are selectable");
    assert_eq!(
        copy.participants, 5,
        "every run of every row between the two ends took part: {:?}",
        copy.text
    );
    let opening = copy
        .text
        .find("The opening paragraph")
        .expect("the first row was copied");
    let closing = copy
        .text
        .find("The closing paragraph")
        .expect("the last row was copied");
    assert!(
        opening < closing,
        "the rows are read in document order: {:?}",
        copy.text
    );
}

#[gpui::test]
fn a_copy_across_a_virtualized_log_says_it_could_not_read_it_all(cx: &mut TestAppContext) {
    // Long messages, so the message column reaches across the row and a
    // pointer aimed at the row is aimed at its text.
    let entries: Vec<LogEntry> = (0..1000)
        .map(|index| {
            LogEntry::new(
                format!("entry-{index:04}"),
                format!(
                    "Fixture log message {index:04} with enough words after it to fill the row it is drawn in"
                ),
            )
            .level("INFO", Tone::Info)
        })
        .collect();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        LogStream::new("logs", entries.clone())
            .visible_rows(6)
            .into_any_element()
    });

    // The stream opens on its newest rows, which are the ones actually laid
    // out and therefore the only ones a pointer can reach.
    // A row is wider than its message column, which starts after the level
    // and timestamp, so the drag aims inside that column rather than at the
    // row's own edges.
    drag_between_blocks(
        &mut harness,
        ("logs.entries.entry-0996", 0.3),
        ("logs.entries.entry-0999", 0.45),
    );

    let copy = harness
        .update(|window, _| window.document_selection_text())
        .expect("the mounted rows are selectable");
    assert!(
        copy.text.contains("Fixture log message 0997"),
        "the rows between the two ends are read whole: {:?}",
        copy.text
    );
    assert!(
        !copy.complete,
        "a virtualized stream cannot promise it read every row it spanned"
    );
}

#[gpui::test]
fn colouring_code_moves_nothing_on_the_page(cx: &mut TestAppContext) {
    // Highlighting is paint and only paint. The same glyphs at the same size
    // land in the same places whether or not the scanner ran, so a block that
    // is being coloured while it streams never reflows under the reader.
    let mut plain = Harness::new(cx, gpui_kit::install, |_, _| {
        CodeView::new("hunk", code_lines()).into_any_element()
    });
    let unlit = plain.bounds("hunk.lines.line-41").expect("laid out");

    let mut lit = Harness::new(cx, gpui_kit::install, |_, _| {
        CodeView::new("hunk", code_lines())
            .language("rust")
            .into_any_element()
    });
    let coloured = lit.bounds("hunk.lines.line-41").expect("laid out");

    assert_eq!(unlit, coloured);
}

#[gpui::test]
fn a_language_nobody_here_can_read_is_still_the_language_it_was_called(cx: &mut TestAppContext) {
    // The name is the caller's claim, so it is published whether or not this
    // crate has a table for it. What it does not get is a guess at its
    // grammar.
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        CodeView::new("hunk", code_lines())
            .language("cobol")
            .into_any_element()
    });

    let view = harness.node("hunk").expect("published");
    assert_eq!(view.text.as_deref(), Some("cobol"));
    assert!(harness.node("hunk.lines.line-41").is_some());
}

#[gpui::test]
fn a_folded_file_keeps_only_its_header_and_says_what_it_holds(cx: &mut TestAppContext) {
    // A review of many files is unreadable as all their rows at once, and the
    // fold is only useful if the header that remains says enough to choose by.
    let (events, sink) = recorder::<DiffViewEvent>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        let files: Vec<DiffFile> = fixture_diff(9)
            .into_iter()
            .map(|file| file.folded(true))
            .collect();
        DiffView::new("review", files)
            .visible_rows(5)
            .on_event(move |event, _, _| sink.borrow_mut().push(event))
            .into_any_element()
    });

    assert!(
        harness.node("review.rows.file.report").is_some(),
        "the file itself is still there"
    );
    assert!(
        harness.node("review.rows.file.report.hunk.body").is_none(),
        "and everything under it is not"
    );

    harness.click("review.rows.file.report");
    assert_eq!(
        events.borrow().as_slice(),
        [DiffViewEvent::UnfoldFile {
            file_id: "report".into(),
        }],
        "a folded header asks to be opened rather than reporting a visit"
    );
}

#[gpui::test]
fn a_file_says_what_happened_to_it_above_its_lines(cx: &mut TestAppContext) {
    // A reader cannot tell a new file from a large addition to an old one, or
    // a rename from a deletion and a creation, by reading lines: those are
    // facts about the file, and the file is where they are said.
    let mut files = fixture_diff(3);
    files.push(DiffFile::new("logo", "assets/logo.png", []).notes([
        DiffNote::Added,
        DiffNote::Binary,
        // A second note of a kind the file already carries is dropped
        // rather than drawn under the same name as the first.
        DiffNote::Binary,
    ]));
    let folded: Vec<DiffFile> = files
        .iter()
        .cloned()
        .map(|file| file.folded(true))
        .collect();

    let mut harness = Harness::new(cx, gpui_kit::install, {
        let files = files.clone();
        move |_, _| {
            DiffView::new("review", files.clone())
                .visible_rows(12)
                .into_any_element()
        }
    });
    assert!(
        harness.node("review.rows.file.logo.note.added").is_some(),
        "a file that did not exist before says so"
    );
    assert!(harness.node("review.rows.file.logo.note.binary").is_some());
    assert_eq!(
        harness
            .node("review.rows.file.logo.note.binary")
            .expect("published")
            .text
            .as_deref(),
        Some("File note"),
        "the sentence itself is the crate's to translate, so the diagnostic \
         name is the kind"
    );

    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        DiffView::new("review", folded.clone())
            .visible_rows(12)
            .into_any_element()
    });
    assert!(
        harness.node("review.rows.file.logo").is_some(),
        "the folded file is still listed"
    );
    assert!(
        harness.node("review.rows.file.logo.note.added").is_none(),
        "a fold that left the notes standing would not be a fold"
    );
}

#[gpui::test]
fn stepping_to_a_change_marks_it_and_puts_it_on_screen(cx: &mut TestAppContext) {
    // A large diff is walked, not scrolled. Without the cursor the line below
    // is neither laid out nor published, which is the whole point of the
    // virtualized list — and the whole reason a step has to move the viewport
    // rather than only recolour a row.
    let far = "review.rows.file.report.hunk.body.line.line-0900";
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        DiffView::new("review", fixture_diff(1000))
            .visible_rows(6)
            .into_any_element()
    });
    assert!(harness.node(far).is_none());

    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        DiffView::new("review", fixture_diff(1000))
            .visible_rows(6)
            .cursor(DiffCursor::Line {
                file_id: "report".into(),
                hunk_id: "body".into(),
                line_id: "line-0900".into(),
            })
            .into_any_element()
    });
    let stepped = harness.node(far).expect("the stepped-to line is on screen");
    assert!(
        stepped.selected,
        "and says it is the one the reader asked for"
    );
}

#[gpui::test]
fn two_views_of_one_shared_diff_keep_their_own_arrangement(cx: &mut TestAppContext) {
    // A shared diff is flattened once per version instead of once per frame,
    // and the answer belongs to the view that asked: two views over one
    // allocation arrange it differently, and neither may be handed the other's
    // rows.
    let shared = std::sync::Arc::new(fixture_diff(3));
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let shared = std::sync::Arc::clone(&shared);
        div()
            .child(
                DiffView::shared("unified", std::sync::Arc::clone(&shared))
                    .visible_rows(8)
                    .presentation(DiffPresentation::Unified),
            )
            .child(
                DiffView::shared("split", shared)
                    .visible_rows(8)
                    .presentation(DiffPresentation::Split),
            )
            .into_any_element()
    });

    assert_eq!(
        harness.node("unified").expect("published").value.as_deref(),
        Some("unified")
    );
    assert_eq!(
        harness.node("split").expect("published").value.as_deref(),
        Some("split")
    );
    for view in ["unified", "split"] {
        assert!(
            harness
                .node(&format!("{view}.rows.file.report.hunk.body.line.line-0000"))
                .is_some(),
            "{view} draws the shared diff's own rows"
        );
    }
}

#[gpui::test]
fn naming_the_language_colours_only_the_lines_that_arrived_uncoloured(cx: &mut TestAppContext) {
    // The same rule `CodeView` keeps: the caller's grammar outranks the
    // scanner, and a replacement keeps the more specific claim about which
    // words changed.
    let plain = DiffLine::added("added", "fn main() {}");
    let claimed = DiffLine::added("claimed", "let held = 1;").spans([CodeSpan {
        range: 0..3,
        role: SyntaxColor::Removed,
    }]);
    let file = DiffFile::new(
        "src",
        "src/main.rs",
        [DiffHunk::new("body", "@@ @@", [plain, claimed])],
    );

    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        DiffView::new("review", [file.clone()])
            .language("rust")
            .visible_rows(6)
            .into_any_element()
    });

    // Highlighting is paint, so what a test can hold it to is that the rows
    // still say exactly what they said: the text, the marks and the identities
    // do not move because a colour was applied.
    let added = harness
        .node("review.rows.file.src.hunk.body.line.added")
        .expect("the added line is published");
    assert_eq!(added.text.as_deref(), Some("Added"));
    assert!(
        harness
            .node("review.rows.file.src.hunk.body.line.claimed")
            .is_some(),
        "and so is the line whose spans the caller supplied"
    );
}

#[gpui::test]
fn a_wrapping_diff_keeps_every_row_addressable(cx: &mut TestAppContext) {
    // Measured rows are the other half of the fixed-height trade. What must
    // not change is which rows exist and what they report.
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        DiffView::new("review", fixture_diff(6))
            .wrapping(true)
            .visible_rows(8)
            .into_any_element()
    });

    for index in 0..6 {
        assert!(
            harness
                .node(&format!(
                    "review.rows.file.report.hunk.body.line.line-{index:04}"
                ))
                .is_some(),
            "line {index} went missing once rows were measured"
        );
    }
}

#[gpui::test]
fn a_virtualized_document_lays_out_a_screenful_rather_than_a_conversation(cx: &mut TestAppContext) {
    // The cost of drawing the newest block must not grow with everything said
    // before it, which is what a plain column makes it.
    let built = Rc::new(RefCell::new(0usize));
    let counter = built.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let counter = counter.clone();
        AgentDocument::new("thread")
            .virtualized(4)
            .blocks((0..200).map(move |index| {
                let counter = counter.clone();
                AgentDocumentBlock::code(format!("block-{index:03}"), move |_, _| {
                    *counter.borrow_mut() += 1;
                    div().child(format!("block {index}")).into_any_element()
                })
            }))
            .into_any_element()
    });

    assert!(
        harness.node("thread.block.block-000").is_some(),
        "the first blocks are drawn"
    );
    assert!(
        harness.node("thread.block.block-199").is_none(),
        "and the two-hundredth, which nobody can see, is not"
    );
    let drawn = *built.borrow();
    assert!(
        drawn < 40,
        "only a screenful of blocks was built, but {drawn} were"
    );
    assert_eq!(
        harness.node("thread").expect("published").value.as_deref(),
        Some("ready:200"),
        "the document still says how much it holds, drawn or not"
    );
}

#[gpui::test]
fn a_block_that_grew_does_not_disturb_the_blocks_around_it(cx: &mut TestAppContext) {
    // A streaming reply arrives a token at a time. If each token discarded
    // every height the list had measured, the scrollbar would shudder for as
    // long as the answer took.
    let answer = Rc::new(RefCell::new(String::from("The")));
    let text = answer.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let text = text.borrow().clone();
        AgentDocument::new("thread")
            .virtualized(6)
            .block(AgentDocumentBlock::text(
                "ask",
                "What did the freeze cover?",
            ))
            .block(AgentDocumentBlock::markdown("reply", text).streaming(true))
            .block(AgentDocumentBlock::text("after", "Anything else?"))
            .into_any_element()
    });

    let before = harness
        .bounds("thread.block.ask")
        .expect("the question is drawn");

    for token in [" freeze", " covered", " everything", " after the tag."] {
        answer.borrow_mut().push_str(token);
        harness.frame();
    }

    assert_eq!(
        harness.bounds("thread.block.ask").expect("still drawn"),
        before,
        "the question moved while the answer below it was still arriving"
    );
    assert!(
        harness.node("thread.block.reply").expect("drawn").busy,
        "and the block that is still arriving says so"
    );
}
