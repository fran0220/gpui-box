//! The application shell: a tree of splits, panels in regions, a status strip,
//! and a field that records a keystroke. None of them applies anything.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Entity, IntoElement, Modifiers, MouseButton, Pixels, Point, SharedString, TestAppContext, div,
    prelude::*, px,
};
use gpui_kit::assets::Icon;
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Calls<T> = Rc<RefCell<Vec<T>>>;

fn recorder<T: 'static>() -> (Calls<T>, Calls<T>) {
    let calls: Calls<T> = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

/// Delivers the frame that publishes what the first one measured.
fn settle(harness: &mut Harness) {
    harness.advance(std::time::Duration::from_millis(16));
}

fn pane(title: &'static str) -> gpui::Div {
    div().flex().flex_col().child(title)
}

// ------------------------------------------------------------- nested splits

/// A workspace three levels deep: a file tree beside an editor stacked over a
/// terminal, so a drag on the outer divider has to reckon with a leaf two
/// branches away.
fn workspace() -> SplitLayout {
    SplitLayout::horizontal(
        "body",
        0.3,
        SplitLayout::leaf(SplitPaneSpec::new("files").min_width(150.0)),
        SplitLayout::vertical(
            "editing",
            0.6,
            SplitLayout::leaf(SplitPaneSpec::new("editor").min(200.0)),
            SplitLayout::leaf(SplitPaneSpec::new("terminal").min_height(100.0)),
        ),
    )
}

/// What a host would write out: plain fields, with no library type left in
/// them, standing in for whatever row or document it persists.
#[derive(Debug, PartialEq)]
struct Stored {
    id: String,
    parent: Option<String>,
    kind: &'static str,
    ratio: f32,
    min_width: f32,
    min_height: f32,
    collapsed: bool,
}

#[gpui::test]
fn a_nested_layout_round_trips_through_the_public_structure(cx: &mut TestAppContext) {
    let _ = cx;
    let layout = workspace().with_collapsed("files", false);
    let records = layout.to_records();

    let stored: Vec<Stored> = records
        .iter()
        .map(|record| Stored {
            id: record.id.to_string(),
            parent: record.parent.as_ref().map(ToString::to_string),
            kind: record.kind.name(),
            ratio: record.ratio,
            min_width: record.min_width,
            min_height: record.min_height,
            collapsed: record.collapsed,
        })
        .collect();
    assert_eq!(stored[0].id, "body");
    assert_eq!(stored[0].kind, "horizontal");
    assert_eq!(
        stored[1],
        Stored {
            id: "files".into(),
            parent: Some("body".into()),
            kind: "pane",
            ratio: 0.0,
            min_width: 150.0,
            min_height: 0.0,
            collapsed: false,
        }
    );

    let restored = SplitLayout::from_records(&records).expect("the records are a tree");
    assert_eq!(restored, layout);
    assert_eq!(
        restored.to_records(),
        records,
        "the conversion loses nothing"
    );
}

struct TreeCase {
    harness: Harness,
    changes: Calls<SplitChange>,
}

fn tree(cx: &mut TestAppContext, layout: SplitLayout) -> TreeCase {
    let (changes, sink) = recorder::<SplitChange>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        div()
            .w(px(600.0))
            .h(px(400.0))
            .child(
                SplitTree::new("workspace")
                    .layout(layout.clone())
                    .pane("files", pane("Files"))
                    .pane("editor", pane("Editor"))
                    .pane("terminal", pane("Terminal"))
                    .on_change(move |change, _, _| sink.borrow_mut().push(change)),
            )
            .into_any_element()
    });
    settle(&mut harness);
    TreeCase { harness, changes }
}

fn drag_divider(harness: &mut Harness, id: &str, x: f32) {
    let start = harness.point_in(id);
    let target: Point<Pixels> = gpui::point(px(x), start.y);
    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    harness.context().run_until_parked();
}

#[gpui::test]
fn a_drag_that_would_starve_a_distant_leaf_stops_at_its_minimum(cx: &mut TestAppContext) {
    let mut case = tree(cx, workspace());

    // Dragging the outer divider right would squeeze the editor and terminal
    // column, whose own minimum is the larger of two leaves two levels down.
    drag_divider(&mut case.harness, "workspace.body.divider", 590.0);

    let reported = case.changes.borrow().clone();
    let SplitChange::Ratio { split, ratio } = reported.last().expect("a ratio was reported") else {
        panic!("a drag reports a ratio");
    };
    assert_eq!(split.as_ref(), "body");
    // The editing column needs 200px for the editor, so the file tree may take
    // at most 400px of 600px.
    assert!(
        (*ratio - 2.0 / 3.0).abs() < 0.02,
        "the drag stopped where the editor would starve, got {ratio}"
    );

    // The caller owns the ratio, so the tree still draws the one it was given.
    let divider = case
        .harness
        .node("workspace.body.divider")
        .expect("published");
    let high = divider.value_max.expect("a separator carries its range");
    assert!(
        (high - 2.0 / 3.0).abs() < 0.02,
        "the published limit was {high}"
    );
}

#[gpui::test]
fn every_leaf_of_a_nested_layout_is_addressable(cx: &mut TestAppContext) {
    let mut case = tree(cx, workspace());

    for leaf in ["files", "editor", "terminal"] {
        assert!(
            case.harness.node(&format!("workspace.{leaf}")).is_some(),
            "`{leaf}` is published by name"
        );
    }
    assert_eq!(
        case.harness.node("workspace").expect("published").value,
        Some("3".into()),
        "the tree says how many panes it holds"
    );
}

#[gpui::test]
fn a_collapsed_leaf_is_drawn_at_its_rail_and_offers_no_divider(cx: &mut TestAppContext) {
    let layout = SplitLayout::horizontal(
        "body",
        0.3,
        SplitLayout::leaf(
            SplitPaneSpec::new("files")
                .min_width(150.0)
                .rail(44.0)
                .collapsed(true),
        ),
        SplitLayout::leaf(SplitPaneSpec::new("editor").min_width(200.0)),
    );
    let mut case = tree(cx, layout);

    assert!(
        case.harness.node("workspace.body.divider").is_none(),
        "a rail has no ratio to move, so there is nothing to drag"
    );
    let files = case.harness.bounds("workspace.files").expect("published");
    assert!(
        (f32::from(files.size.width) - 44.0).abs() < 1.0,
        "a collapsed pane takes its rail, got {:?}",
        files.size.width
    );
    assert_eq!(
        case.harness
            .node("workspace.files")
            .expect("published")
            .expanded,
        Some(false)
    );
}

// -------------------------------------------------------------------- dock

fn dock(cx: &mut TestAppContext, right_collapsed: bool) -> (Harness, Calls<DockEvent>) {
    let (events, sink) = recorder::<DockEvent>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        div()
            .w(px(900.0))
            .h(px(560.0))
            .child(
                Dock::new("shell")
                    .share(DockRegion::Left, 0.25)
                    .panel(
                        DockRegion::Left,
                        DockPanel::new("files", "Files")
                            .icon(Icon::Folder)
                            .content(pane("Workspace")),
                    )
                    .panel(
                        DockRegion::Left,
                        DockPanel::new("search", "Search").icon(Icon::Magnifier),
                    )
                    .active(DockRegion::Left, "files")
                    .panel(
                        DockRegion::Centre,
                        DockPanel::new("editor", "main.rs").content(pane("fn main()")),
                    )
                    .panel(
                        DockRegion::Right,
                        DockPanel::new("outline", "Outline").icon(Icon::List),
                    )
                    .panel(
                        DockRegion::Right,
                        DockPanel::new("history", "History").icon(Icon::GitBranch),
                    )
                    .collapsed(DockRegion::Right, right_collapsed)
                    .panel(
                        DockRegion::Bottom,
                        DockPanel::new("problems", "Problems").unavailable(
                            "The language server is not running, so problems cannot be listed.",
                        ),
                    )
                    .on_event(move |event, _, _| sink.borrow_mut().push(event)),
            )
            .into_any_element()
    });
    settle(&mut harness);
    (harness, events)
}

#[gpui::test]
fn a_collapsed_region_still_publishes_every_panel_it_holds(cx: &mut TestAppContext) {
    let (mut harness, _) = dock(cx, true);

    let region = harness.node("shell.right").expect("published");
    assert_eq!(region.expanded, Some(false));
    assert_eq!(region.value.as_deref(), Some("2"));

    for (id, title) in [("outline", "Outline"), ("history", "History")] {
        let item = harness
            .node(&format!("shell.right.rail.{id}"))
            .unwrap_or_else(|| panic!("`{id}` is published while the region is narrow"));
        assert_eq!(item.text.as_deref(), Some(title));
    }
}

#[gpui::test]
fn picking_from_a_rail_reports_the_panel_and_the_room_it_needs(cx: &mut TestAppContext) {
    let (mut harness, events) = dock(cx, true);

    harness.click("shell.right.rail.history");

    assert_eq!(
        events.borrow().clone(),
        vec![
            DockEvent::PanelSelected {
                region: DockRegion::Right,
                panel: "history".into()
            },
            DockEvent::RegionCollapsed {
                region: DockRegion::Right,
                collapsed: false
            }
        ]
    );
    // Nothing was applied: the rail is still a rail.
    assert_eq!(
        harness.node("shell.right").expect("published").expanded,
        Some(false)
    );
}

#[gpui::test]
fn a_panel_dragged_to_another_region_reports_a_move_and_moves_nothing(cx: &mut TestAppContext) {
    let (mut harness, events) = dock(cx, false);

    let from = harness.point_in("shell.left.tabs.search");
    let onto = harness.point_in("shell.right.tabs.outline");
    harness
        .context()
        .simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    // GPUI turns a press plus travel into a drag; the first move starts it.
    harness.context().simulate_mouse_move(
        gpui::point(from.x + px(8.0), from.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    harness.context().simulate_mouse_move(
        gpui::point(onto.x - px(8.0), onto.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    harness
        .context()
        .simulate_mouse_move(onto, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(onto, MouseButton::Left, Modifiers::none());
    harness.context().run_until_parked();

    let moves: Vec<DockEvent> = events
        .borrow()
        .iter()
        .filter(|event| matches!(event, DockEvent::PanelMoved { .. }))
        .cloned()
        .collect();
    let Some(DockEvent::PanelMoved {
        panel, to_region, ..
    }) = moves.first()
    else {
        panic!(
            "a drop onto another region reports a move, got {:?}",
            events.borrow()
        );
    };
    assert_eq!(panel.as_ref(), "search");
    assert_eq!(*to_region, DockRegion::Right);

    // The library moved nothing: the panel is still in the region it was in.
    assert!(harness.node("shell.left.tabs.search").is_some());
    assert!(harness.node("shell.right.tabs.search").is_none());
}

#[gpui::test]
fn an_unavailable_panel_states_why_in_place(cx: &mut TestAppContext) {
    let (mut harness, _) = dock(cx, false);

    let body = harness
        .node("shell.bottom.problems")
        .expect("an unavailable panel keeps its place");
    assert_eq!(body.role, Role::TabPanel);
    assert!(body.invalid, "the panel says it cannot be shown");
    assert_eq!(
        body.value.as_deref(),
        Some("The language server is not running, so problems cannot be listed."),
        "the host's reason is carried verbatim"
    );
    // The panel did not disappear: its tab is still there to come back to.
    assert!(harness.node("shell.bottom.tabs.problems").is_some());
}

#[gpui::test]
fn a_region_divider_reports_the_share_the_region_asked_for(cx: &mut TestAppContext) {
    let (mut harness, events) = dock(cx, false);

    drag_divider(&mut harness, "shell.layout.dock.body.divider", 300.0);

    let resizes: Vec<DockEvent> = events
        .borrow()
        .iter()
        .filter(|event| matches!(event, DockEvent::RegionResized { .. }))
        .cloned()
        .collect();
    let Some(DockEvent::RegionResized { region, ratio }) = resizes.last() else {
        panic!("a drag on a region divider reports a share");
    };
    assert_eq!(*region, DockRegion::Left);
    assert!((*ratio - 1.0 / 3.0).abs() < 0.03, "the share was {ratio}");
}

#[gpui::test]
fn a_frozen_dock_reports_nothing(cx: &mut TestAppContext) {
    let (events, sink) = recorder::<DockEvent>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        div()
            .w(px(600.0))
            .h(px(400.0))
            .child(
                Dock::new("shell")
                    .panel(DockRegion::Left, DockPanel::new("files", "Files"))
                    .panel(DockRegion::Left, DockPanel::new("search", "Search"))
                    .panel(DockRegion::Centre, DockPanel::new("editor", "main.rs"))
                    .disabled(true)
                    .on_event(move |event, _, _| sink.borrow_mut().push(event)),
            )
            .into_any_element()
    });
    settle(&mut harness);

    harness.click("shell.left.tabs.search");

    assert!(events.borrow().is_empty());
    assert!(
        harness
            .node("shell.left.tabs.search")
            .expect("published")
            .disabled
    );
}

// -------------------------------------------------------------- status bar

fn status(cx: &mut TestAppContext) -> Harness {
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let mut branch = AsyncValue::<SharedString, String>::ready("main@a1b2c3".into());
        branch.refresh();
        branch.fail_refresh("the host is unreachable".into());
        let fresh = AsyncValue::<SharedString, String>::ready("12 files".into());

        div()
            .w(px(720.0))
            .child(
                StatusBar::new("shell.status")
                    .label("Workspace status")
                    .start([
                        StatusItem::text("branch", "unknown")
                            .icon(Icon::GitBranch)
                            .tracking(&branch),
                        StatusItem::text("indexed", "none").tracking(&fresh),
                        StatusItem::state("build", "Build passing", Tone::Success),
                        StatusItem::text("plain", "Ln 42, Col 7"),
                    ])
                    .centre([StatusItem::progress("index", "Indexing").count(7, 12)])
                    .end([StatusItem::action("encoding", "UTF-8").on_click(|_, _| {})]),
            )
            .into_any_element()
    });
    settle(&mut harness);
    harness
}

#[gpui::test]
fn a_status_item_marked_stale_says_stale_and_keeps_its_last_value(cx: &mut TestAppContext) {
    let mut harness = status(cx);

    let branch = harness.node("shell.status.branch").expect("published");
    assert_eq!(branch.role, Role::Status);
    assert_eq!(
        branch.text.as_deref(),
        Some("main@a1b2c3"),
        "a failed refresh keeps the last verified value on screen"
    );
    assert_eq!(branch.value.as_deref(), Some("stale"));

    let fresh = harness.node("shell.status.indexed").expect("published");
    assert_eq!(fresh.value.as_deref(), Some("ready"));
}

#[gpui::test]
fn a_status_item_the_host_gave_no_state_claims_none(cx: &mut TestAppContext) {
    let mut harness = status(cx);

    assert_eq!(
        harness.node("shell.status.plain").expect("published").value,
        None,
        "the bar does not invent a state for an item that was given none"
    );
    assert_eq!(
        harness
            .node("shell.status.build")
            .expect("published")
            .value
            .as_deref(),
        Some("success"),
        "a state the host chose is published by name, not by colour"
    );
}

#[gpui::test]
fn a_status_bar_publishes_what_it_holds_and_its_parts(cx: &mut TestAppContext) {
    let mut harness = status(cx);

    let bar = harness.node("shell.status").expect("published");
    assert_eq!(bar.role, Role::Toolbar);
    assert_eq!(bar.text.as_deref(), Some("Workspace status"));
    assert_eq!(bar.value.as_deref(), Some("6"));

    let ring = harness
        .node("shell.status.index.progress")
        .expect("a progress item publishes its position");
    assert_eq!(ring.role, Role::Progress);
    let now = ring
        .value_now
        .expect("a ring with a known extent has a position");
    assert!((now - 7.0 / 12.0).abs() < 0.01, "the position was {now}");
    assert_eq!(ring.value_max, Some(1.0));

    let action = harness.node("shell.status.encoding").expect("published");
    assert_eq!(action.role, Role::Button);
}

// ------------------------------------------------------ keybinding recorder

struct RecorderCase {
    harness: Harness,
    recorder: Entity<KeybindingRecorder>,
    events: Calls<KeybindingRecorderEvent>,
}

fn keybinding(
    cx: &mut TestAppContext,
    allow_escape: bool,
    conflict: Option<&'static str>,
) -> RecorderCase {
    let (events, sink) = recorder::<KeybindingRecorderEvent>();
    let slot: Rc<RefCell<Option<Entity<KeybindingRecorder>>>> = Rc::new(RefCell::new(None));
    let held = slot.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let sink = sink.clone();
        let field = held
            .borrow_mut()
            .get_or_insert_with(|| {
                let field = cx.new(|cx| {
                    let mut recorder = KeybindingRecorder::new("settings.palette", window, cx)
                        .label("Command palette")
                        .allow_escape(allow_escape);
                    if let Some(reason) = conflict {
                        recorder = recorder.conflict(Some(reason));
                    }
                    recorder
                });
                cx.subscribe(&field, move |_, event: &KeybindingRecorderEvent, _| {
                    sink.borrow_mut().push(event.clone());
                })
                .detach();
                field
            })
            .clone();
        div().w(px(420.0)).child(field).into_any_element()
    });
    let recorder = slot.borrow().clone().expect("the recorder was built");
    RecorderCase {
        harness,
        recorder,
        events,
    }
}

#[gpui::test]
fn the_recorder_captures_a_keystroke_in_gpui_syntax(cx: &mut TestAppContext) {
    let mut case = keybinding(cx, false, None);

    case.harness.click("settings.palette");
    assert!(
        case.harness
            .node("settings.palette")
            .expect("published")
            .busy,
        "recording is unmistakable in the tree as well as on screen"
    );
    case.harness.keystrokes("cmd-shift-p");

    let captured: Vec<SharedString> = case
        .events
        .borrow()
        .iter()
        .filter_map(|event| match event {
            KeybindingRecorderEvent::Captured(binding) => Some(binding.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(captured.len(), 1);
    let binding = captured[0].clone();

    // The one property that matters: GPUI reads back what the recorder wrote.
    let parsed = gpui::Keystroke::parse(binding.as_ref()).expect("gpui parses its own syntax");
    assert_eq!(parsed.key, "p");
    assert!(parsed.modifiers.platform && parsed.modifiers.shift);
    case.harness.context().update(|_, cx| {
        assert!(
            !Kbd::new(binding.clone()).caps(cx).is_empty(),
            "Kbd draws the same string"
        );
    });

    // A captured binding is reported, not applied.
    case.harness.context().update(|_, cx| {
        assert_eq!(case.recorder.read(cx).current_binding(), None);
        assert!(!case.recorder.read(cx).is_recording());
    });
}

#[gpui::test]
fn a_modifier_alone_does_not_end_recording(cx: &mut TestAppContext) {
    let mut case = keybinding(cx, false, None);

    case.harness.click("settings.palette");
    case.harness.keystrokes("shift");
    case.harness.keystrokes("ctrl");

    assert!(
        case.events
            .borrow()
            .iter()
            .all(|event| matches!(event, KeybindingRecorderEvent::Started)),
        "a hand resting on the keyboard is not a binding, got {:?}",
        case.events.borrow()
    );
    case.harness.context().update(|_, cx| {
        assert!(case.recorder.read(cx).is_recording(), "it is still waiting");
    });
}

#[gpui::test]
fn escape_ends_recording_without_capturing(cx: &mut TestAppContext) {
    let mut case = keybinding(cx, false, None);

    case.harness.click("settings.palette");
    case.harness.keystrokes("escape");

    assert_eq!(
        case.events.borrow().clone(),
        vec![
            KeybindingRecorderEvent::Started,
            KeybindingRecorderEvent::Cancelled
        ]
    );
    case.harness.context().update(|_, cx| {
        assert!(!case.recorder.read(cx).is_recording());
    });
}

#[gpui::test]
fn a_caller_that_needs_escape_can_have_it(cx: &mut TestAppContext) {
    let mut case = keybinding(cx, true, None);

    case.harness.click("settings.palette");
    case.harness.keystrokes("escape");

    assert_eq!(
        case.events.borrow().last(),
        Some(&KeybindingRecorderEvent::Captured("escape".into())),
        "with allow_escape on, escape is a keystroke like any other"
    );
}

#[gpui::test]
fn a_conflict_the_host_declared_is_rendered(cx: &mut TestAppContext) {
    let reason = "cmd-shift-p already opens the command palette.";
    let mut case = keybinding(cx, false, Some(reason));

    let field = case.harness.node("settings.palette").expect("published");
    assert!(field.invalid, "the recorder shows the host's judgement");

    let note = case
        .harness
        .node("settings.palette.conflict")
        .expect("the reason is published, not just painted");
    assert_eq!(note.text.as_deref(), Some(reason));
    assert_eq!(note.role, Role::Status);
}

#[gpui::test]
fn a_recorder_with_no_conflict_declares_none(cx: &mut TestAppContext) {
    let mut case = keybinding(cx, false, None);

    assert!(
        !case
            .harness
            .node("settings.palette")
            .expect("published")
            .invalid,
        "the recorder never guesses at a conflict"
    );
    assert!(case.harness.node("settings.palette.conflict").is_none());
}
