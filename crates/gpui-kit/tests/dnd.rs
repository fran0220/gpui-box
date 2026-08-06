//! Drag and drop, driven through a simulated pointer.
//!
//! Every assertion here reads the published semantic tree or a value the host
//! was handed. Nothing reads component state, and nothing sleeps.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{SharedString, TestAppContext, div, prelude::*, px};
use gpui_kit::interaction::dnd::{self, DRAG_NODE_ID, DragItem, DropIntent, DropPosition};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

/// Everything a host was told during one test.
type Reports = Rc<RefCell<Vec<String>>>;

fn reports() -> Reports {
    Rc::new(RefCell::new(Vec::new()))
}

fn record(reports: &Reports, intent: &DropIntent) {
    reports
        .borrow_mut()
        .push(format!("{} {}", intent.item.id, intent.position));
}

const ROWS: [&str; 4] = ["alpha", "beta", "gamma", "delta"];

fn row_label(index: usize) -> SharedString {
    SharedString::from(format!("Row {}", ROWS[index]))
}

fn list_harness(cx: &mut TestAppContext, reports: Reports) -> Harness {
    Harness::new(cx, gpui_kit::install, move |_, _| {
        let reports = Rc::clone(&reports);
        div()
            .w(px(320.0))
            .child(
                List::new("queue", ROWS.len(), |index, _, _| {
                    ListItem::new(ROWS[index], row_label(index)).text(row_label(index))
                })
                .row_height(32.0)
                .reorderable(true)
                .on_select(|_, _, _| {})
                .on_reorder(move |intent, _, _| record(&reports, intent)),
            )
            .into_any_element()
    })
}

#[gpui::test]
fn a_drag_publishes_what_it_carries_and_where_it_would_land(cx: &mut TestAppContext) {
    let mut harness = list_harness(cx, reports());

    assert!(
        harness.node(DRAG_NODE_ID).is_none(),
        "nothing is being dragged yet"
    );

    harness.drag_start("queue.gamma");
    let carried = harness.node(DRAG_NODE_ID).expect("a drag publishes itself");
    assert_eq!(carried.text.as_deref(), Some("Row gamma"));
    assert_eq!(carried.value.as_deref(), Some("gamma none"));

    let over = harness.point_down("queue.alpha", 0.2);
    harness.drag_to(over);
    let landing = harness
        .node(DRAG_NODE_ID)
        .expect("the drag is still in flight");
    assert_eq!(landing.value.as_deref(), Some("gamma before:alpha"));
    assert!(!landing.invalid, "alpha takes the payload");

    let below = harness.point_down("queue.alpha", 0.8);
    harness.drag_to(below);
    assert_eq!(
        harness.node(DRAG_NODE_ID).and_then(|node| node.value),
        Some("gamma after:alpha".into())
    );
}

#[gpui::test]
fn a_drop_reports_the_intent_and_moves_nothing(cx: &mut TestAppContext) {
    let reports = reports();
    let mut harness = list_harness(cx, Rc::clone(&reports));
    let order = |harness: &mut Harness| {
        harness
            .snapshot()
            .children_of("queue")
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>()
    };
    let before = order(&mut harness);

    harness.drag_start("queue.gamma");
    let over = harness.point_down("queue.alpha", 0.2);
    harness.drag_to(over);
    harness.drop_here();

    assert_eq!(reports.borrow().as_slice(), ["gamma before:alpha"]);
    assert_eq!(
        order(&mut harness),
        before,
        "the list shows the order the host still says is true"
    );
    assert!(
        harness.node(DRAG_NODE_ID).is_none(),
        "a finished drag publishes nothing"
    );
}

#[gpui::test]
fn escape_cancels_a_drag_silently(cx: &mut TestAppContext) {
    let reports = reports();
    let mut harness = list_harness(cx, Rc::clone(&reports));

    harness.drag_start("queue.gamma");
    let over = harness.point_down("queue.alpha", 0.2);
    harness.drag_to(over);
    assert!(harness.node(DRAG_NODE_ID).is_some());

    harness.cancel_drag();
    assert!(
        harness.node(DRAG_NODE_ID).is_none(),
        "a cancelled drag stops publishing"
    );

    harness.drop_here();
    assert!(
        reports.borrow().is_empty(),
        "a cancelled drag reports nothing"
    );
}

#[gpui::test]
fn a_row_offers_no_slot_against_itself(cx: &mut TestAppContext) {
    let reports = reports();
    let mut harness = list_harness(cx, Rc::clone(&reports));

    harness.drag_start("queue.gamma");
    harness.drag_over("queue.gamma");
    assert_eq!(
        harness.node(DRAG_NODE_ID).and_then(|node| node.value),
        Some("gamma none".into())
    );
    harness.drop_here();
    assert!(reports.borrow().is_empty());
}

#[gpui::test]
fn a_refused_target_looks_refused_and_reports_nothing(cx: &mut TestAppContext) {
    let reports = reports();
    let handler = Rc::clone(&reports);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let handler = Rc::clone(&handler);
        div()
            .w(px(320.0))
            .child(
                List::new("queue", ROWS.len(), |index, _, _| {
                    ListItem::new(ROWS[index], row_label(index)).text(row_label(index))
                })
                .row_height(32.0)
                .reorderable(true)
                // The host allows a move only onto the first row.
                .accepts(|_, position| position.anchor().as_ref() == "alpha")
                .on_select(|_, _, _| {})
                .on_reorder(move |intent, _, _| record(&handler, intent)),
            )
            .into_any_element()
    });

    harness.drag_start("queue.gamma");
    let over = harness.point_down("queue.delta", 0.2);
    harness.drag_to(over);
    let refused = harness.node(DRAG_NODE_ID).expect("the drag is in flight");
    assert_eq!(refused.value.as_deref(), Some("gamma before:delta"));
    assert!(refused.invalid, "a refusal is published as a refusal");

    harness.drop_here();
    assert!(
        reports.borrow().is_empty(),
        "dropping on a refusing target reports nothing"
    );
}

fn tree_nodes() -> Vec<TreeNode> {
    vec![
        TreeNode::new("workspace", "workspace").children([
            TreeNode::new("crates", "crates").children([TreeNode::new("kit", "gpui-kit")]),
            TreeNode::new("docs", "docs").children([TreeNode::new("guide", "guide.md")]),
        ]),
        TreeNode::new("target", "target").children([TreeNode::new("debug", "debug")]),
    ]
}

fn tree_harness(cx: &mut TestAppContext, reports: Reports) -> Harness {
    Harness::new(cx, gpui_kit::install, move |_, _| {
        let reports = Rc::clone(&reports);
        div()
            .w(px(320.0))
            .child(
                Tree::new("files")
                    .nodes(tree_nodes())
                    .expanded_ids(&["workspace", "crates", "docs", "target"])
                    .reorderable(true)
                    .on_select(|_, _, _| {})
                    .on_toggle(|_, _, _, _| {})
                    .on_move(move |intent, _, _| record(&reports, intent)),
            )
            .into_any_element()
    })
}

#[gpui::test]
fn a_tree_refuses_a_drop_into_its_own_descendant(cx: &mut TestAppContext) {
    let reports = reports();
    let mut harness = tree_harness(cx, Rc::clone(&reports));

    harness.drag_start("files.workspace");
    let into_child = harness.point_down("files.crates", 0.5);
    harness.drag_to(into_child);

    let landing = harness.node(DRAG_NODE_ID).expect("the drag is in flight");
    assert_eq!(landing.value.as_deref(), Some("workspace into:crates"));
    assert!(
        landing.invalid,
        "a node cannot be moved inside something it contains"
    );

    harness.drop_here();
    assert!(reports.borrow().is_empty());
}

#[gpui::test]
fn a_tree_moves_a_node_into_another_branch(cx: &mut TestAppContext) {
    let reports = reports();
    let mut harness = tree_harness(cx, Rc::clone(&reports));

    harness.drag_start("files.kit");
    let into_docs = harness.point_down("files.docs", 0.5);
    harness.drag_to(into_docs);
    assert_eq!(
        harness.node(DRAG_NODE_ID).and_then(|node| node.value),
        Some("kit into:docs".into())
    );
    harness.drop_here();
    assert_eq!(reports.borrow().as_slice(), ["kit into:docs"]);
}

#[gpui::test]
fn a_leaf_offers_the_slots_beside_it_rather_than_itself(cx: &mut TestAppContext) {
    let reports = reports();
    let mut harness = tree_harness(cx, Rc::clone(&reports));

    harness.drag_start("files.kit");
    // The middle of a leaf still has to ask for something, so it splits in two
    // instead of offering a branch it does not have.
    let middle = harness.point_down("files.guide", 0.5);
    harness.drag_to(middle);
    assert_eq!(
        harness.node(DRAG_NODE_ID).and_then(|node| node.value),
        Some("kit after:guide".into())
    );
    harness.drop_here();
    assert_eq!(reports.borrow().as_slice(), ["kit after:guide"]);
}

#[gpui::test]
fn tabs_report_where_a_dragged_tab_should_go(cx: &mut TestAppContext) {
    let reports = reports();
    let handler = Rc::clone(&reports);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let handler = Rc::clone(&handler);
        div()
            .w(px(520.0))
            .child(
                Tabs::new("panes")
                    .tabs([
                        TabItem::new("overview", "Overview"),
                        TabItem::new("runs", "Runs"),
                        TabItem::new("logs", "Logs"),
                    ])
                    .selected("runs")
                    .reorderable(true)
                    .on_select(|_, _, _| {})
                    .on_reorder(move |intent, _, _| record(&handler, intent)),
            )
            .into_any_element()
    });

    let order = |harness: &mut Harness| {
        harness
            .snapshot()
            .children_of("panes")
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>()
    };
    let before = order(&mut harness);

    harness.drag_start("panes.logs");
    let left_of_overview = harness.point_across("panes.overview", 0.2);
    harness.drag_to(left_of_overview);
    assert_eq!(
        harness.node(DRAG_NODE_ID).and_then(|node| node.value),
        Some("logs before:overview".into())
    );
    harness.drop_here();

    assert_eq!(reports.borrow().as_slice(), ["logs before:overview"]);
    assert_eq!(
        order(&mut harness),
        before,
        "the strip keeps the order the host still says is true"
    );
}

#[gpui::test]
fn a_dropzone_tells_its_three_states_apart(cx: &mut TestAppContext) {
    let dropped = reports();
    let handler = Rc::clone(&dropped);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let handler = Rc::clone(&handler);
        div()
            .w(px(420.0))
            .column()
            .child(
                List::new("queue", ROWS.len(), |index, _, _| {
                    ListItem::new(ROWS[index], row_label(index)).text(row_label(index))
                })
                .row_height(32.0)
                .reorderable(true)
                .on_select(|_, _, _| {})
                .on_reorder(|_, _, _| {}),
            )
            .child(
                Dropzone::new("attachments", "Drop rows here")
                    .accepts([gpui_kit::interaction::ROW_KIND])
                    .on_drop(move |item, _, _| handler.borrow_mut().push(item.id.to_string())),
            )
            .child(
                Dropzone::new("files", "Drop files here")
                    .refusal("Only files can be attached here.")
                    .on_drop(|_, _, _| {}),
            )
            .into_any_element()
    });

    assert_eq!(
        harness.node("attachments").and_then(|node| node.value),
        Some("idle".into())
    );

    harness.drag_start("queue.gamma");
    harness.drag_over("attachments");
    let accepting = harness.node("attachments").expect("the zone is on screen");
    assert_eq!(accepting.value.as_deref(), Some("accepting"));
    assert!(!accepting.invalid);
    assert_eq!(
        harness.node("files").and_then(|node| node.value),
        Some("idle".into()),
        "a zone the pointer is not over is idle, not refusing"
    );

    harness.drag_over("files");
    let refusing = harness.node("files").expect("the zone is on screen");
    assert_eq!(refusing.value.as_deref(), Some("refusing"));
    assert!(refusing.invalid);
    assert_eq!(
        refusing.text.as_deref(),
        Some("Only files can be attached here."),
        "a refusal says why, and never reads as idle"
    );

    harness.drop_here();
    assert!(
        dropped.borrow().is_empty(),
        "the refusing zone is not the accepting one"
    );

    harness.drag_start("queue.gamma");
    harness.drag_over("attachments");
    harness.drop_here();
    assert_eq!(dropped.borrow().as_slice(), ["gamma"]);
}

#[gpui::test]
fn a_pinned_dropzone_shows_the_state_it_was_pinned_to(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w(px(320.0))
            .child(
                Dropzone::new("refusing", "Drop files here")
                    .refusal("Only files can be attached here.")
                    .state(DropzoneState::Refusing)
                    .on_drop(|_, _, _| {}),
            )
            .into_any_element()
    });
    let node = harness.node("refusing").expect("the zone is on screen");
    assert_eq!(node.value.as_deref(), Some("refusing"));
    assert!(node.invalid);
}

#[gpui::test]
fn a_list_scrolls_when_a_drag_reaches_its_edge(cx: &mut TestAppContext) {
    const COUNT: usize = 40;
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w(px(320.0))
            .child(
                List::new("records", COUNT, |index, _, _| {
                    let id = SharedString::from(format!("record-{index:04}"));
                    let label = SharedString::from(format!("Fixture record {index:04}"));
                    ListItem::new(id, label.clone()).text(label)
                })
                .row_height(32.0)
                .visible_rows(6)
                .reorderable(true)
                .on_select(|_, _, _| {})
                .on_reorder(|_, _, _| {}),
            )
            .into_any_element()
    });

    assert!(
        harness.node("records.record-0008").is_none(),
        "the eighth row is outside the viewport"
    );

    harness.drag_start("records.record-0000");
    let list = harness.bounds("records").expect("the list is on screen");
    let edge = gpui::point(
        list.origin.x + list.size.width / 2.0,
        list.bottom() - px(4.0),
    );
    for _ in 0..3 {
        harness.drag_to(edge);
        harness.frame();
    }

    assert!(
        harness.node("records.record-0008").is_some(),
        "a drag at the bottom edge brings the next rows to the pointer"
    );
    harness.cancel_drag();
}

#[gpui::test]
fn a_staged_drag_renders_without_a_pointer(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, cx| {
        dnd::stage(
            StagedDrag::new(DragItem::new("queue", "gamma", "Row gamma")).landing(
                "queue",
                DropPosition::Before(SharedString::new_static("alpha")),
                Some(0),
                true,
            ),
            cx,
        );
        div()
            .w(px(320.0))
            .child(
                List::new("queue", ROWS.len(), |index, _, _| {
                    ListItem::new(ROWS[index], row_label(index)).text(row_label(index))
                })
                .row_height(32.0)
                .reorderable(true)
                .on_select(|_, _, _| {})
                .on_reorder(|_, _, _| {}),
            )
            .children(dnd::staged_ghost(cx))
            .into_any_element()
    });

    let ghost = harness.node(DRAG_NODE_ID).expect("a staged drag publishes");
    assert_eq!(ghost.value.as_deref(), Some("gamma before:alpha"));
}
