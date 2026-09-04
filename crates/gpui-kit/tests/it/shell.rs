//! The application shell: a tree of splits, panels in regions, a status strip,
//! and a field that records a keystroke. None of them applies anything.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Entity, IntoElement, Modifiers, MouseButton, Pixels, Point, SharedString, TestAppContext,
    WindowControlArea, div, prelude::*, px,
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

// --------------------------------------------------------- desktop titlebar

#[gpui::test]
fn a_titlebar_keeps_caller_content_in_the_client_area(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<()>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        div()
            .w(px(720.0))
            .child(
                DesktopTitlebar::new("shell.titlebar", "Workspace")
                    .subtitle("main.rs")
                    .right(
                        Button::new("shell.titlebar.command")
                            .label("Command palette")
                            .small()
                            .on_click(move |_, _| sink.borrow_mut().push(())),
                    ),
            )
            .into_any_element()
    });

    let titlebar = harness.node("shell.titlebar").expect("published");
    assert_eq!(titlebar.role, Role::Toolbar);
    assert_eq!(titlebar.text.as_deref(), Some("Workspace"));
    let title = harness
        .node("shell.titlebar.title")
        .expect("the title is independently addressable");
    assert_eq!(title.role, Role::Heading);
    assert_eq!(title.text.as_deref(), Some("Workspace"));
    assert_eq!(
        harness
            .node("shell.titlebar.subtitle")
            .expect("the caller-owned subtitle is published")
            .text
            .as_deref(),
        Some("main.rs")
    );

    let title_point = harness.point_in("shell.titlebar.title");
    assert_eq!(
        harness.update(|window, _| window.window_control_area_at(title_point)),
        Some(WindowControlArea::Drag),
        "the title remains draggable"
    );
    let command_point = harness.point_in("shell.titlebar.command");
    assert_eq!(
        harness.update(|window, _| window.window_control_area_at(command_point)),
        Some(WindowControlArea::Client),
        "caller content overrides its enclosing drag strip"
    );
    harness.click("shell.titlebar.command");
    assert_eq!(
        calls.borrow().len(),
        1,
        "caller content remains interactive"
    );
}

#[cfg(not(any(target_os = "macos", target_family = "wasm")))]
#[gpui::test]
fn titlebar_controls_report_requests_and_close_can_be_refused(cx: &mut TestAppContext) {
    let (events, sink) = recorder::<DesktopTitlebarEvent>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        div()
            .w(px(720.0))
            .child(
                DesktopTitlebar::new("shell.titlebar", "Workspace")
                    .button_layout(gpui::WindowButtonLayout {
                        left: [None; gpui::MAX_BUTTONS_PER_SIDE],
                        right: [
                            Some(gpui::WindowButton::Minimize),
                            Some(gpui::WindowButton::Maximize),
                            Some(gpui::WindowButton::Close),
                        ],
                    })
                    .on_event(move |event, window, _| {
                        sink.borrow_mut().push(event);
                        if event == DesktopTitlebarEvent::Close {
                            window.request_close();
                        }
                    }),
            )
            .into_any_element()
    });
    let close_checks = Rc::new(RefCell::new(0));
    harness.update({
        let close_checks = close_checks.clone();
        move |window, cx| {
            window.on_window_should_close(cx, move |_, _| {
                *close_checks.borrow_mut() += 1;
                false
            });
        }
    });

    for (id, area) in [
        ("shell.titlebar.minimize", WindowControlArea::Min),
        ("shell.titlebar.maximize", WindowControlArea::Max),
        ("shell.titlebar.close", WindowControlArea::Close),
    ] {
        let node = harness
            .node(id)
            .unwrap_or_else(|| panic!("`{id}` published"));
        assert_eq!(node.role, Role::Button);
        let point = harness.point_in(id);
        assert_eq!(
            harness.update(|window, _| window.window_control_area_at(point)),
            Some(area),
            "`{id}` keeps its native hit-test identity"
        );
        harness.click(id);
    }

    assert_eq!(
        events.borrow().as_slice(),
        [
            DesktopTitlebarEvent::Minimize,
            DesktopTitlebarEvent::ToggleMaximize,
            DesktopTitlebarEvent::Close,
        ],
        "the component reports intent without owning window state"
    );
    assert_eq!(
        *close_checks.borrow(),
        1,
        "a host-applied close request preserves the refusal callback"
    );
    assert!(
        harness.node("shell.titlebar.close").is_some(),
        "the refused window remains open"
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

// ----------------------------------------------------------- keymap editor

struct KeymapCase {
    harness: Harness,
    editor: Entity<KeymapEditor>,
    events: Calls<KeymapEditorEvent>,
}

fn keymap_commands() -> Vec<KeymapCommand> {
    vec![
        KeymapCommand::new("workspace.open", "Open workspace")
            .context("Workspace")
            .defaults(["cmd-o"])
            .bindings([
                KeymapBinding::new("user-primary", "cmd-shift-o")
                    .conflict("Already opens the recent list")
                    .provenance("User keymap"),
                KeymapBinding::new("workspace-secondary", "ctrl-o").provenance("Workspace keymap"),
            ])
            .searchable("open a folder or project", ["folder", "project"]),
        KeymapCommand::new("terminal.toggle", "Toggle terminal")
            .context("Terminal")
            .defaults(["ctrl-`"])
            .bindings([KeymapBinding::new("default", "ctrl-`")])
            .searchable("show the integrated terminal", ["panel", "console"]),
        KeymapCommand::new("policy.locked", "Managed shortcut")
            .defaults(["cmd-l"])
            .bindings([KeymapBinding::new("managed", "cmd-l")])
            .refused("Managed by the host"),
    ]
}

fn keymap(cx: &mut TestAppContext, query: &'static str, disabled: bool) -> KeymapCase {
    let (events, sink) = recorder::<KeymapEditorEvent>();
    let slot: Rc<RefCell<Option<Entity<KeymapEditor>>>> = Rc::new(RefCell::new(None));
    let held = slot.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let sink = sink.clone();
        let editor = held
            .borrow_mut()
            .get_or_insert_with(|| {
                let editor = cx.new(|cx| {
                    KeymapEditor::new("keymap", window, cx)
                        .commands(keymap_commands())
                        .query(query)
                        .disabled(disabled)
                });
                cx.subscribe(&editor, move |_, event: &KeymapEditorEvent, _| {
                    sink.borrow_mut().push(event.clone());
                })
                .detach();
                editor
            })
            .clone();
        div().w(px(720.0)).child(editor).into_any_element()
    });
    let editor = slot.borrow().clone().expect("the editor was built");
    KeymapCase {
        harness,
        editor,
        events,
    }
}

#[gpui::test]
fn a_keymap_editor_publishes_caller_owned_metadata_and_multiple_bindings(cx: &mut TestAppContext) {
    let mut case = keymap(cx, "", false);

    let row = case.harness.node("keymap.workspace.open").expect("row");
    assert_eq!(row.role, Role::Row);
    assert_eq!(row.text.as_deref(), Some("Open workspace"));
    assert_eq!(row.description.as_deref(), Some("Workspace"));
    assert_eq!(
        case.harness
            .node("keymap.workspace.open.effective")
            .expect("effective bindings")
            .value
            .as_deref(),
        Some("cmd-shift-o, ctrl-o")
    );
    assert_eq!(
        case.harness
            .node("keymap.workspace.open.defaults")
            .expect("defaults")
            .value
            .as_deref(),
        Some("cmd-o")
    );
    assert_eq!(
        case.harness
            .node("keymap.workspace.open.binding.user-primary.conflict")
            .expect("caller conflict")
            .text
            .as_deref(),
        Some("Already opens the recent list")
    );
    assert_eq!(
        case.harness
            .node("keymap.workspace.open.binding.user-primary.provenance")
            .expect("caller provenance")
            .text
            .as_deref(),
        Some("User keymap")
    );
    assert!(
        case.harness
            .node("keymap.workspace.open.binding.workspace-secondary")
            .is_some(),
        "bindings are addressed by caller identity, not position"
    );
}

#[gpui::test]
fn filtering_uses_the_supplied_query_text_and_keywords(cx: &mut TestAppContext) {
    let mut case = keymap(cx, "CONSOLE", false);

    assert!(case.harness.node("keymap.workspace.open").is_none());
    assert!(case.harness.node("keymap.terminal.toggle").is_some());
    assert_eq!(
        case.harness
            .node("keymap.status")
            .expect("filtered count")
            .value
            .as_deref(),
        Some("1")
    );

    case.harness.context().update(|_, cx| {
        case.editor
            .update(cx, |editor, cx| editor.set_query("folder", cx));
    });
    assert!(case.harness.node("keymap.workspace.open").is_some());
    assert!(case.harness.node("keymap.terminal.toggle").is_none());
}

#[gpui::test]
fn one_shared_recorder_switches_targets_and_reports_without_applying(cx: &mut TestAppContext) {
    let mut case = keymap(cx, "", false);

    case.harness.click("keymap.workspace.open.add");
    assert!(
        case.harness
            .node("keymap.recorder")
            .expect("the shared recorder")
            .busy
    );
    case.harness.click("keymap.terminal.toggle.add");
    case.harness.keystrokes("ctrl-j");

    assert_eq!(
        case.events.borrow().as_slice(),
        [
            KeymapEditorEvent::RecordingCancelled {
                command_id: "workspace.open".into(),
            },
            KeymapEditorEvent::AddCaptured {
                command_id: "terminal.toggle".into(),
                keystroke: "ctrl-j".into(),
            },
        ]
    );
    assert!(case.harness.node("keymap.recorder").is_none());
    assert_eq!(
        case.harness
            .node("keymap.terminal.toggle.effective")
            .expect("caller value remains")
            .value
            .as_deref(),
        Some("ctrl-`"),
        "capturing reports an intent; it does not mutate the keymap"
    );
}

#[gpui::test]
fn remove_reset_refusal_and_disabled_actions_are_truthful(cx: &mut TestAppContext) {
    let mut case = keymap(cx, "", false);

    case.harness
        .click("keymap.workspace.open.binding.user-primary.remove");
    case.harness.click("keymap.workspace.open.reset");
    assert_eq!(
        case.events.borrow().as_slice(),
        [
            KeymapEditorEvent::Remove {
                command_id: "workspace.open".into(),
                binding_id: "user-primary".into(),
            },
            KeymapEditorEvent::Reset {
                command_id: "workspace.open".into(),
            },
        ]
    );
    assert!(case.harness.node("keymap.terminal.toggle.reset").is_none());
    assert_eq!(
        case.harness
            .node("keymap.policy.locked.refusal")
            .expect("refusal")
            .text
            .as_deref(),
        Some("Managed by the host")
    );
    assert!(case.harness.node("keymap.policy.locked.add").is_none());
    assert!(
        case.harness
            .node("keymap.policy.locked.binding.managed.remove")
            .is_none()
    );

    let mut disabled = keymap(cx, "", true);
    assert!(disabled.harness.node("keymap.workspace.open.add").is_none());
    assert!(
        disabled
            .harness
            .node("keymap.workspace.open.binding.user-primary.remove")
            .is_none()
    );
    assert_eq!(
        disabled
            .harness
            .node("keymap.workspace.open.effective")
            .expect("disabled keeps values")
            .value
            .as_deref(),
        Some("cmd-shift-o, ctrl-o")
    );
}

#[gpui::test]
fn filtering_away_an_active_row_cancels_its_invisible_recorder(cx: &mut TestAppContext) {
    let mut case = keymap(cx, "", false);
    case.harness.click("keymap.workspace.open.add");

    case.harness.context().update(|_, cx| {
        case.editor
            .update(cx, |editor, cx| editor.set_query("terminal", cx));
    });

    assert!(case.harness.node("keymap.recorder").is_none());
    assert_eq!(
        case.events.borrow().last(),
        Some(&KeymapEditorEvent::RecordingCancelled {
            command_id: "workspace.open".into(),
        })
    );
    case.harness.context().update(|_, cx| {
        assert!(case.editor.read(cx).active_command().is_none());
    });
}
