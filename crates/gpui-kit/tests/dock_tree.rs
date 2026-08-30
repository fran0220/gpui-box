//! Persistence, rendering, and intent contracts of recursive docks.

use std::{cell::RefCell, rc::Rc};

use gpui::{IntoElement, Modifiers, MouseButton, TestAppContext, div, prelude::*, px};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

fn topology() -> DockTopology {
    DockTopology::vertical(
        "root",
        0.72,
        DockTopology::horizontal(
            "work",
            0.28,
            DockTopology::Stack(DockStack::new("source", ["files", "search"]).active("files")),
            DockTopology::stack("editor", ["main"]),
        ),
        DockTopology::Stack(DockStack::new("empty", std::iter::empty::<&str>()).collapsed(true)),
    )
}

fn dock_with(
    cx: &mut TestAppContext,
    topology: DockTopology,
) -> (Harness, Rc<RefCell<Vec<DockTreeEvent>>>) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        div()
            .w(px(900.0))
            .h(px(620.0))
            .child(
                DockTree::new("tree", topology.clone())
                    .panels([
                        DockPanel::new("files", "Files").content(div().child("Workspace")),
                        DockPanel::new("search", "Search"),
                        DockPanel::new("main", "main.rs").content(div().child("fn main()")),
                    ])
                    .on_event(move |event, _, _| sink.borrow_mut().push(event)),
            )
            .into_any_element()
    });
    (harness, events)
}

fn dock(cx: &mut TestAppContext) -> (Harness, Rc<RefCell<Vec<DockTreeEvent>>>) {
    dock_with(cx, topology())
}

#[test]
fn records_round_trip_nested_splits_and_an_empty_stack() {
    let topology = topology();
    let records = topology.to_records();

    assert_eq!(
        DockTopology::from_records(&records).expect("records came from this topology"),
        topology
    );
    assert_eq!(records.len(), 5);
    assert!(
        DockTopology::from_records(&records)
            .expect("valid")
            .find_stack("empty")
            .expect("empty stack persists")
            .panels()
            .is_empty()
    );
}

#[test]
fn records_reject_duplicate_panel_membership_and_an_absent_active_panel() {
    let mut duplicate = topology().to_records();
    duplicate
        .iter_mut()
        .find(|record| record.id.as_ref() == "empty")
        .expect("empty stack")
        .panels
        .push("files".into());
    assert_eq!(
        DockTopology::from_records(&duplicate),
        Err(DockRecordError::DuplicatePanel("files".into()))
    );

    let mut absent = topology().to_records();
    absent
        .iter_mut()
        .find(|record| record.id.as_ref() == "editor")
        .expect("editor stack")
        .active = Some("missing".into());
    assert_eq!(
        DockTopology::from_records(&absent),
        Err(DockRecordError::MissingActive {
            stack: "editor".into(),
            panel: "missing".into(),
        })
    );
}

#[gpui::test]
fn recursive_stacks_publish_membership_and_keep_empty_drop_targets(cx: &mut TestAppContext) {
    let (mut harness, _) = dock(cx);

    assert_eq!(
        harness.node("tree").expect("dock root").value.as_deref(),
        Some("3")
    );
    assert_eq!(
        harness
            .node("tree.source")
            .expect("source stack")
            .value
            .as_deref(),
        Some("2")
    );
    assert_eq!(
        harness
            .node("tree.empty")
            .expect("empty stack")
            .value
            .as_deref(),
        Some("0")
    );
    assert_eq!(
        harness
            .node("tree.empty.empty")
            .expect("an empty stack remains visible")
            .text
            .as_deref(),
        Some("Drop a panel here")
    );
}

#[gpui::test]
fn selection_is_reported_and_remains_caller_owned(cx: &mut TestAppContext) {
    let (mut harness, events) = dock(cx);
    harness.click("tree.source.tabs.search");

    assert!(events.borrow().contains(&DockTreeEvent::PanelSelected {
        stack: "source".into(),
        panel: "search".into(),
    }));
    assert_eq!(
        harness
            .node("tree.source.tabs.files")
            .expect("files tab")
            .checked,
        Some(true)
    );
}

#[gpui::test]
fn collapsing_a_stack_is_reported_and_remains_caller_owned(cx: &mut TestAppContext) {
    let (mut harness, events) = dock(cx);
    harness.click("tree.source.collapse");

    assert_eq!(
        events.borrow().as_slice(),
        &[DockTreeEvent::StackCollapsed {
            stack: "source".into(),
            collapsed: true,
        }]
    );
    assert_eq!(
        harness.node("tree.source").expect("source stack").expanded,
        Some(true)
    );
}

#[gpui::test]
fn picking_from_a_collapsed_stack_reports_selection_then_expansion(cx: &mut TestAppContext) {
    let collapsed = DockTopology::vertical(
        "root",
        0.3,
        DockTopology::Stack(
            DockStack::new("source", ["files", "search"])
                .active("files")
                .collapsed(true),
        ),
        DockTopology::stack("editor", ["main"]),
    );
    let (mut harness, events) = dock_with(cx, collapsed);
    harness.click("tree.source.rail.search");

    assert_eq!(
        events.borrow().as_slice(),
        &[
            DockTreeEvent::PanelSelected {
                stack: "source".into(),
                panel: "search".into(),
            },
            DockTreeEvent::StackCollapsed {
                stack: "source".into(),
                collapsed: false,
            },
        ]
    );
    assert_eq!(
        harness.node("tree.source").expect("source stack").expanded,
        Some(false)
    );
}

#[gpui::test]
fn a_drop_into_an_empty_stack_reports_a_move_and_changes_nothing(cx: &mut TestAppContext) {
    let (mut harness, events) = dock(cx);
    drag(&mut harness, "tree.source.tabs.search", "tree.empty.body");

    assert!(events.borrow().iter().any(|event| {
        event
            == &DockTreeEvent::PanelMoved {
                panel: "search".into(),
                to_stack: "empty".into(),
                before: None,
            }
    }));
    assert!(harness.node("tree.source.tabs.search").is_some());
    assert!(harness.node("tree.empty.tabs.search").is_none());
}

#[gpui::test]
fn an_edge_drop_reports_split_placement_without_inventing_node_ids(cx: &mut TestAppContext) {
    let (mut harness, events) = dock(cx);
    let from = harness.point_in("tree.source.tabs.search");
    harness
        .context()
        .simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    harness.context().simulate_mouse_move(
        gpui::point(from.x + px(8.0), from.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    harness.context().run_until_parked();

    let edge = harness.point_in("tree.editor.split-right");
    harness
        .context()
        .simulate_mouse_move(edge, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(edge, MouseButton::Left, Modifiers::none());
    harness.context().run_until_parked();

    assert!(events.borrow().iter().any(|event| {
        event
            == &DockTreeEvent::PanelSplit {
                panel: "search".into(),
                target_stack: "editor".into(),
                placement: DockPlacement::Right,
            }
    }));
    assert!(harness.node("tree.source.tabs.search").is_some());
}

fn drag(harness: &mut Harness, from: &str, to: &str) {
    let from = harness.point_in(from);
    let to = harness.point_in(to);
    harness
        .context()
        .simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    harness.context().simulate_mouse_move(
        gpui::point(from.x + px(8.0), from.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    harness
        .context()
        .simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    harness.context().run_until_parked();
}
