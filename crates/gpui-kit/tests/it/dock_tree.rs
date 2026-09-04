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
    dock_with_size(cx, topology, 900.0, 620.0)
}

fn dock_with_size(
    cx: &mut TestAppContext,
    topology: DockTopology,
    width: f32,
    height: f32,
) -> (Harness, Rc<RefCell<Vec<DockTreeEvent>>>) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        div()
            .w(px(width))
            .h(px(height))
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
    let restored = DockTopology::from_records(&records).expect("records came from this topology");

    assert_eq!(restored, topology);
    assert_eq!(
        restored.to_records(),
        records,
        "load/dump/load reaches a fixpoint"
    );
    assert_eq!(records.len(), 5);
    assert!(
        restored
            .find_stack("empty")
            .expect("empty stack persists")
            .panels()
            .is_empty()
    );
}

#[test]
fn malformed_records_are_rejected_without_guessing_a_topology() {
    assert_eq!(
        DockTopology::from_records(&[]),
        Err(DockRecordError::NoRoot)
    );

    let mut many_roots = topology().to_records();
    let mut other_root = many_roots
        .iter()
        .find(|record| record.id.as_ref() == "editor")
        .expect("editor stack")
        .clone();
    other_root.id = "other".into();
    other_root.parent = None;
    many_roots.push(other_root);
    assert_eq!(
        DockTopology::from_records(&many_roots),
        Err(DockRecordError::ManyRoots(vec![
            "root".into(),
            "other".into(),
        ]))
    );

    let mut duplicate_id = topology().to_records();
    duplicate_id.push(duplicate_id[0].clone());
    assert_eq!(
        DockTopology::from_records(&duplicate_id),
        Err(DockRecordError::DuplicateId("root".into()))
    );

    let mut missing_parent = topology().to_records();
    missing_parent
        .iter_mut()
        .find(|record| record.id.as_ref() == "editor")
        .expect("editor stack")
        .parent = Some("absent".into());
    assert_eq!(
        DockTopology::from_records(&missing_parent),
        Err(DockRecordError::MissingParent {
            id: "editor".into(),
            parent: "absent".into(),
        })
    );

    let mut wrong_child_count = topology().to_records();
    wrong_child_count.retain(|record| record.id.as_ref() != "empty");
    assert_eq!(
        DockTopology::from_records(&wrong_child_count),
        Err(DockRecordError::WrongChildCount {
            id: "root".into(),
            found: 1,
        })
    );

    let mut stack_with_children = topology().to_records();
    stack_with_children
        .iter_mut()
        .find(|record| record.id.as_ref() == "work")
        .expect("work split")
        .kind = DockRecordKind::Stack;
    assert_eq!(
        DockTopology::from_records(&stack_with_children),
        Err(DockRecordError::StackWithChildren("work".into()))
    );

    let mut split_with_panels = topology().to_records();
    split_with_panels[0].panels.push("rogue".into());
    assert_eq!(
        DockTopology::from_records(&split_with_panels),
        Err(DockRecordError::SplitWithPanels("root".into()))
    );

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

    let mut unreachable = topology().to_records();
    let mut first = unreachable
        .iter()
        .find(|record| record.id.as_ref() == "empty")
        .expect("empty stack")
        .clone();
    first.id = "first".into();
    first.parent = Some("second".into());
    let mut second = first.clone();
    second.id = "second".into();
    second.parent = Some("first".into());
    unreachable.extend([first, second]);
    assert_eq!(
        DockTopology::from_records(&unreachable),
        Err(DockRecordError::Unreachable)
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
    assert_split_ratios(&mut harness, "selecting a tab");
    assert!(
        !events
            .borrow()
            .iter()
            .any(|event| matches!(event, DockTreeEvent::SplitResized { .. }))
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
    assert_split_ratios(&mut harness, "collapsing a stack");
    assert!(
        !events
            .borrow()
            .iter()
            .any(|event| matches!(event, DockTreeEvent::SplitResized { .. }))
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

    let moves: Vec<DockTreeEvent> = events
        .borrow()
        .iter()
        .filter(|event| matches!(event, DockTreeEvent::PanelMoved { .. }))
        .cloned()
        .collect();
    assert_eq!(
        moves,
        [DockTreeEvent::PanelMoved {
            panel: "search".into(),
            to_stack: "empty".into(),
            before: None,
        }],
        "one completed drag reports exactly one caller-owned move intent"
    );
    assert!(harness.node("tree.source.tabs.search").is_some());
    assert!(harness.node("tree.empty.tabs.search").is_none());
}

#[gpui::test]
fn dragging_a_nested_divider_reports_only_that_split(cx: &mut TestAppContext) {
    let (mut harness, events) = dock(cx);
    let start = harness.point_in("tree.layout.work.divider");
    let target = gpui::point(start.x + px(120.0), start.y);
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

    let resized: Vec<DockTreeEvent> = events
        .borrow()
        .iter()
        .filter(|event| matches!(event, DockTreeEvent::SplitResized { .. }))
        .cloned()
        .collect();
    assert!(!resized.is_empty(), "the dragged divider reports a ratio");
    assert!(
        resized.iter().all(|event| matches!(
            event,
            DockTreeEvent::SplitResized { split, .. } if split.as_ref() == "work"
        )),
        "a nested divider must never report its root or a sibling: {resized:?}"
    );
}

#[gpui::test]
fn split_ratios_are_dimensionless_across_container_sizes(cx: &mut TestAppContext) {
    let (mut narrow, _) = dock_with_size(cx, topology(), 600.0, 500.0);
    let narrow_width = narrow
        .node("tree.layout.work")
        .expect("nested split")
        .bounds
        .width;
    let narrow_ratio = narrow
        .node("tree.layout.work.divider")
        .expect("nested divider")
        .value_now
        .expect("splitter publishes its ratio");
    drop(narrow);

    let (mut wide, _) = dock_with_size(cx, topology(), 900.0, 620.0);
    let wide_width = wide
        .node("tree.layout.work")
        .expect("nested split")
        .bounds
        .width;
    let wide_ratio = wide
        .node("tree.layout.work.divider")
        .expect("nested divider")
        .value_now
        .expect("splitter publishes its ratio");

    assert!(wide_width > narrow_width, "the test changed pixel geometry");
    assert!((narrow_ratio - 0.28).abs() < 1e-6);
    assert!((wide_ratio - 0.28).abs() < 1e-6);
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

fn assert_split_ratios(harness: &mut Harness, action: &str) {
    for (split, expected) in [("root", 0.72), ("work", 0.28)] {
        let actual = harness
            .node(&format!("tree.layout.{split}.divider"))
            .unwrap_or_else(|| panic!("{split} divider"))
            .value_now
            .unwrap_or_else(|| panic!("{split} ratio"));
        assert!(
            (actual - expected).abs() < 1e-6,
            "{action} cannot rewrite unrelated split `{split}`: expected {expected}, got {actual}"
        );
    }
}
