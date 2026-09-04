//! Sidebar, pagination, and drawer report where the typist wants to go. None
//! of them decides that they got there.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{Entity, TestAppContext, div, prelude::*, px};
use gpui_kit::prelude::*;
use gpui_kit_assets::Icon;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Calls<T> = Rc<RefCell<Vec<T>>>;

fn recorder<T: 'static>() -> (Calls<T>, Calls<T>) {
    let calls: Calls<T> = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

// ------------------------------------------------------------------ sidebar

fn places() -> Vec<SidebarSection> {
    vec![
        SidebarSection::new("work").title("Work").items([
            SidebarItem::new("runs", "Runs")
                .icon(Icon::List)
                .badge("12")
                .children([SidebarItem::new("runs.active", "Active").icon(Icon::Refresh)]),
            SidebarItem::new("files", "Files").icon(Icon::Folder),
            SidebarItem::new("claude", "Claude Code").image("agents/claude.svg"),
        ]),
        SidebarSection::new("admin")
            .title("Administration")
            .items([SidebarItem::new("policy", "Managed by policy")
                .icon(Icon::Key)
                .disabled(true)]),
    ]
}

fn sidebar(cx: &mut TestAppContext, collapsed: bool) -> (Harness, Calls<String>) {
    let (calls, sink) = recorder::<String>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Sidebar::new("workspace.rail")
            .sections(places())
            .active("runs.active")
            .collapsed(collapsed)
            .footer(div().child("Fixture workspace"))
            .on_select(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn clicking_a_place_reports_it_without_moving_the_selection(cx: &mut TestAppContext) {
    let (mut harness, calls) = sidebar(cx, false);

    harness.click("workspace.rail.files");

    assert_eq!(*calls.borrow(), vec!["files".to_string()]);
    // The caller owns where the typist is, so nothing moved.
    assert!(
        harness
            .node("workspace.rail.runs.active")
            .expect("published")
            .selected
    );
    assert!(
        !harness
            .node("workspace.rail.files")
            .expect("published")
            .selected
    );
}

#[gpui::test]
fn a_nested_place_publishes_the_depth_it_sits_at(cx: &mut TestAppContext) {
    let (mut harness, _calls) = sidebar(cx, false);

    let parent = harness.node("workspace.rail.runs").expect("published");
    let child = harness
        .node("workspace.rail.runs.active")
        .expect("published");

    assert_eq!(parent.role, Role::Link);
    assert_eq!(parent.level, Some(1));
    assert_eq!(child.level, Some(2));
    assert_eq!(child.parent.as_deref(), Some("workspace.rail"));
    assert_eq!(parent.value.as_deref(), Some("12"), "a badge is published");
}

#[gpui::test]
fn a_place_kept_by_policy_reports_nothing(cx: &mut TestAppContext) {
    let (mut harness, calls) = sidebar(cx, false);

    harness.click("workspace.rail.policy");

    assert!(calls.borrow().is_empty());
    assert!(
        harness
            .node("workspace.rail.policy")
            .expect("published")
            .disabled
    );
}

#[gpui::test]
fn a_collapsed_rail_narrows_the_drawing_and_keeps_every_name(cx: &mut TestAppContext) {
    let (mut expanded, _) = sidebar(cx, false);
    let wide = expanded.bounds("workspace.rail").expect("published");
    let named = expanded.node("workspace.rail.files").expect("published");

    let (mut collapsed, calls) = sidebar(cx, true);
    let narrow = collapsed.bounds("workspace.rail").expect("published");

    assert!(
        narrow.size.width < wide.size.width,
        "collapsing narrows the rail"
    );
    assert_eq!(
        collapsed
            .node("workspace.rail.files")
            .expect("published")
            .text,
        named.text,
        "a glyph-only rail still says what each place is"
    );
    assert!(
        collapsed.node("workspace.rail.work").is_none(),
        "a caption with no room for it is not drawn"
    );
    assert_eq!(
        collapsed
            .node("workspace.rail")
            .expect("published")
            .expanded,
        Some(false)
    );

    collapsed.click("workspace.rail.files");
    assert_eq!(*calls.borrow(), vec!["files".to_string()]);
}

#[gpui::test]
fn an_image_place_still_publishes_its_name(cx: &mut TestAppContext) {
    let (mut harness, calls) = sidebar(cx, false);
    let node = harness.node("workspace.rail.claude").expect("published");

    assert_eq!(node.text.as_deref(), Some("Claude Code"));
    assert_eq!(node.level, Some(1));

    harness.click("workspace.rail.claude");
    assert_eq!(*calls.borrow(), vec!["claude".to_string()]);
}

// --------------------------------------------------------------- pagination

fn pagination(cx: &mut TestAppContext, page: usize, total: PageTotal) -> (Harness, Calls<usize>) {
    let (calls, sink) = recorder::<usize>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        div()
            .w(px(700.0))
            .child(
                Pagination::new("runs.pages")
                    .page(page)
                    .total(total)
                    .on_select(move |target, _, _| sink.borrow_mut().push(target)),
            )
            .into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn a_page_control_reports_the_page_it_was_asked_for(cx: &mut TestAppContext) {
    let (mut harness, calls) = pagination(cx, 9, PageTotal::Known(20));

    harness.click("runs.pages.next");
    harness.click("runs.pages.last");
    harness.click("runs.pages.page-1");

    assert_eq!(*calls.borrow(), vec![10, 20, 1]);
    // The caller owns which page is showing, so it did not move.
    assert_eq!(
        harness
            .node("runs.pages.status")
            .expect("published")
            .text
            .as_deref(),
        Some("Page 9 of 20")
    );
}

#[gpui::test]
fn a_step_with_nowhere_to_go_reports_nothing(cx: &mut TestAppContext) {
    let (mut harness, calls) = pagination(cx, 1, PageTotal::Known(3));

    harness.click("runs.pages.previous");
    harness.click("runs.pages.first");

    assert!(calls.borrow().is_empty());
    assert!(
        harness
            .node("runs.pages.previous")
            .expect("published")
            .disabled
    );
    assert!(
        harness
            .node("runs.pages.first")
            .expect("published")
            .disabled
    );
}

#[gpui::test]
fn an_elided_range_says_how_many_pages_it_stands_for(cx: &mut TestAppContext) {
    let (mut harness, _calls) = pagination(cx, 9, PageTotal::Known(20));

    let gap = harness
        .node("runs.pages.gap-10-20")
        .expect("an elided run is published");
    assert_eq!(gap.value.as_deref(), Some("9"));
    assert!(
        harness.node("runs.pages.page-1").is_some(),
        "the first page is always offered"
    );
    assert!(
        harness.node("runs.pages.page-20").is_some(),
        "the last page is always offered"
    );
    assert!(harness.node("runs.pages.page-15").is_none());
}

#[gpui::test]
fn an_unknown_total_offers_no_last_page_and_claims_no_count(cx: &mut TestAppContext) {
    let (mut harness, calls) = pagination(cx, 3, PageTotal::Unknown { has_next: true });

    assert!(
        harness.node("runs.pages.last").is_none(),
        "a host that cannot count has no last page to go to"
    );
    assert!(
        harness.node("runs.pages.page-3").is_none(),
        "a page count nobody counted is not drawn as numbers"
    );
    assert_eq!(
        harness
            .node("runs.pages.status")
            .expect("published")
            .text
            .as_deref(),
        Some("Page 3"),
        "the copy states no total it does not have"
    );
    assert_eq!(harness.node("runs.pages").expect("published").value, None);

    harness.click("runs.pages.next");
    assert_eq!(*calls.borrow(), vec![4]);
}

#[gpui::test]
fn a_last_known_page_reports_nothing_forward(cx: &mut TestAppContext) {
    let (mut harness, calls) = pagination(cx, 3, PageTotal::Unknown { has_next: false });

    harness.click("runs.pages.next");

    assert!(calls.borrow().is_empty());
    assert!(harness.node("runs.pages.next").expect("published").disabled);
}

// ------------------------------------------------------------------- drawer

fn drawer(cx: &mut TestAppContext, dismissable: bool) -> (Harness, Entity<Drawer>) {
    let slot: Rc<RefCell<Option<Entity<Drawer>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let drawer = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    Drawer::new("runs.filters", window, cx)
                        .edge(Edge::Right)
                        .size(320.0)
                        .title("Filter runs")
                        .dismissable(dismissable)
                        .content(|_, _| div().child("Failed runs only").into_any_element())
                })
            })
            .clone();
        div()
            .w(px(800.0))
            .h(px(600.0))
            .child(drawer)
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("drawer was built");
    (harness, entity)
}

fn drawer_events(harness: &mut Harness, drawer: &Entity<Drawer>) -> Calls<DrawerEvent> {
    let (seen, sink) = recorder::<DrawerEvent>();
    harness.update({
        let drawer = drawer.clone();
        move |_, cx| {
            cx.subscribe(&drawer, move |_, event: &DrawerEvent, _| {
                sink.borrow_mut().push(*event);
            })
            .detach();
        }
    });
    seen
}

fn open_drawer(harness: &mut Harness, drawer: &Entity<Drawer>) {
    let drawer = drawer.clone();
    harness.update(move |window, cx| {
        drawer.update(cx, |drawer, cx| {
            drawer.open(window, cx);
            drawer.settle(cx);
        });
    });
}

#[gpui::test]
fn a_closed_drawer_publishes_nothing(cx: &mut TestAppContext) {
    let (mut harness, _drawer) = drawer(cx, true);

    assert!(harness.node("runs.filters").is_none());
}

#[gpui::test]
fn an_open_drawer_hangs_from_the_edge_it_was_given(cx: &mut TestAppContext) {
    let (mut harness, entity) = drawer(cx, true);
    open_drawer(&mut harness, &entity);

    let node = harness.node("runs.filters").expect("published");
    assert_eq!(node.role, Role::Dialog);
    assert_eq!(node.text.as_deref(), Some("Filter runs"));
    assert_eq!(node.expanded, Some(true));

    let panel = harness.bounds("runs.filters").expect("published");
    // The surface draws a hairline inside the width it was given.
    assert!(
        (f32::from(panel.size.width) - 320.0).abs() < 4.0,
        "a side drawer is as wide as it was told to be, got {}",
        f32::from(panel.size.width)
    );
    assert!(
        f32::from(panel.size.height) > 300.0,
        "a side drawer stretches down the edge it hangs from"
    );
}

#[gpui::test]
fn escape_reports_a_dismissal_and_the_panel_stays_until_the_slide_finishes(
    cx: &mut TestAppContext,
) {
    let (mut harness, entity) = drawer(cx, true);
    let seen = drawer_events(&mut harness, &entity);
    open_drawer(&mut harness, &entity);

    harness.keystrokes("escape");

    assert!(
        seen.borrow().contains(&DrawerEvent::Dismissed),
        "escape reports the wave-away"
    );
    assert!(
        !seen.borrow().contains(&DrawerEvent::Closed),
        "closing is not reported while the panel is still sliding out"
    );
    assert!(
        harness.node("runs.filters").is_some(),
        "an element cannot animate after it is dropped, so it stays until the slide ends"
    );

    for _ in 0..10 {
        harness.advance(Duration::from_millis(40));
    }

    assert!(seen.borrow().contains(&DrawerEvent::Closed));
    assert!(harness.node("runs.filters").is_none());
}

#[gpui::test]
fn a_drawer_that_cannot_be_dismissed_installs_no_escape_handler(cx: &mut TestAppContext) {
    let (mut harness, entity) = drawer(cx, false);
    let seen = drawer_events(&mut harness, &entity);
    open_drawer(&mut harness, &entity);

    harness.keystrokes("escape");

    assert!(
        seen.borrow()
            .iter()
            .all(|event| *event != DrawerEvent::Dismissed)
    );
    assert!(harness.node("runs.filters").is_some());
}

// ------------------------------------------------------------ undo history

fn revisions() -> Vec<HistoryEntry> {
    vec![
        HistoryEntry::new("opened", "Opened document")
            .source("Fixture host")
            .time("10:14"),
        HistoryEntry::new("managed", "Applied managed template")
            .unavailable("The template is no longer available."),
        HistoryEntry::new("current", "Renamed document").description("Current document"),
        HistoryEntry::new("draft", "Drafted conclusion")
            .source("Alex")
            .time("10:23"),
        HistoryEntry::new("saved", "Saved document"),
    ]
}

fn undo_history(cx: &mut TestAppContext, disabled: bool) -> (Harness, Calls<String>) {
    let (calls, sink) = recorder::<String>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        UndoHistory::new("document.history", "Document undo history")
            .entries(revisions())
            .current("current")
            .disabled(disabled)
            .on_jump(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn undo_history_entries_publish_business_identity_and_caller_metadata(cx: &mut TestAppContext) {
    let (mut harness, _calls) = undo_history(cx, false);

    let list = harness.node("document.history").expect("published");
    assert_eq!(list.role, Role::List);
    assert_eq!(list.text.as_deref(), Some("Document undo history"));
    assert_eq!(list.value.as_deref(), Some("5"));

    let entry = harness.node("document.history.opened").expect("published");
    assert_eq!(entry.role, Role::Row);
    assert_eq!(entry.text.as_deref(), Some("Opened document"));
    assert_eq!(entry.parent.as_deref(), Some("document.history"));
    assert_eq!(
        harness
            .node("document.history.opened.source")
            .expect("published")
            .text
            .as_deref(),
        Some("Fixture host")
    );
    assert_eq!(
        harness
            .node("document.history.opened.time")
            .expect("published")
            .text
            .as_deref(),
        Some("10:14")
    );
}

#[gpui::test]
fn undo_history_jump_reports_an_entry_and_moves_no_history(cx: &mut TestAppContext) {
    let (mut harness, calls) = undo_history(cx, false);

    harness.click("document.history.opened");

    assert_eq!(*calls.borrow(), vec!["opened".to_string()]);
    assert!(
        harness
            .node("document.history.current")
            .expect("published")
            .selected,
        "the caller still owns the current revision"
    );
    assert!(
        !harness
            .node("document.history.opened")
            .expect("published")
            .selected
    );
}

#[gpui::test]
fn undo_history_current_entry_reports_no_redundant_jump(cx: &mut TestAppContext) {
    let (mut harness, calls) = undo_history(cx, false);

    harness.click("document.history.current");
    harness.keystrokes("enter");

    assert!(calls.borrow().is_empty());
}

#[gpui::test]
fn undo_history_unavailable_revision_keeps_its_reason_and_installs_no_action(
    cx: &mut TestAppContext,
) {
    let (mut harness, calls) = undo_history(cx, false);

    harness.click("document.history.managed");

    assert!(calls.borrow().is_empty());
    assert!(
        harness
            .node("document.history.managed")
            .expect("published")
            .disabled
    );
    assert_eq!(
        harness
            .node("document.history.managed.reason")
            .expect("published")
            .text
            .as_deref(),
        Some("The template is no longer available.")
    );
}

#[gpui::test]
fn undo_history_keyboard_skips_refusals_and_the_current_revision(cx: &mut TestAppContext) {
    let (mut harness, calls) = undo_history(cx, false);

    harness.click("document.history.saved");
    calls.borrow_mut().clear();
    harness.keystrokes("up");
    assert_eq!(*calls.borrow(), vec!["opened".to_string()]);

    calls.borrow_mut().clear();
    harness.keystrokes("down");
    assert_eq!(*calls.borrow(), vec!["draft".to_string()]);

    calls.borrow_mut().clear();
    harness.keystrokes("home");
    harness.keystrokes("end");
    assert_eq!(
        *calls.borrow(),
        vec!["opened".to_string(), "saved".to_string()]
    );
}

#[gpui::test]
fn undo_history_disabled_state_installs_no_jump_handlers(cx: &mut TestAppContext) {
    let (mut harness, calls) = undo_history(cx, true);

    harness.click("document.history.opened");
    harness.keystrokes("end enter");

    assert!(calls.borrow().is_empty());
}

#[gpui::test]
fn undo_history_entries_meet_so_the_timeline_reads_as_one_thread(cx: &mut TestAppContext) {
    let (mut harness, _calls) = undo_history(cx, false);

    let ids = [
        "document.history.opened",
        "document.history.managed",
        "document.history.current",
        "document.history.draft",
        "document.history.saved",
    ];
    let boxes: Vec<_> = ids
        .iter()
        .map(|id| harness.bounds(id).expect("laid out"))
        .collect();

    for pair in boxes.windows(2) {
        let (above, below) = (pair[0], pair[1]);
        // The rail is drawn inside each entry, so a gap between two entries is
        // a gap in the thread. Every entry has to start where the one above it
        // ended for the spine to be continuous.
        assert_eq!(
            above.bottom(),
            below.origin.y,
            "the entries left a gap for the timeline to break in"
        );
        assert_eq!(
            above.size.width, below.size.width,
            "entries disagreed about how wide a row is"
        );
    }
}
