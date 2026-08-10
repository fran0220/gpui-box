use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

fn row(index: usize) -> TreeGridRow {
    let mut row = TreeGridRow::new(format!("node-{index:04}"), if index == 0 { 1 } else { 2 })
        .text(format!("Node {index}"))
        .cell(
            "name",
            Cell::new(format!("Node {index}"))
                .text(format!("Node {index}"))
                .published(true),
        )
        .cell("value", format!("Value {index}"));
    if index == 0 {
        row = row.branch(true);
    } else {
        row = row.parent("node-0000");
    }
    row
}

fn tree_grid(cx: &mut TestAppContext, disabled: bool) -> (Harness, Rc<RefCell<Vec<String>>>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = calls.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let select = sink.clone();
        let expand = sink.clone();
        TreeGrid::new("data.tree-grid", 1000, |index, _, _| row(index))
            .columns([
                GridColumn::new("name", "Name").flex(1.0),
                GridColumn::new("value", "Value").fixed(120.0),
            ])
            .selected("node-0001")
            .visible_rows(6)
            .disabled(disabled)
            .on_select(move |id, _, _| select.borrow_mut().push(format!("select:{id}")))
            .on_expand(move |id, open, _, _| {
                expand.borrow_mut().push(format!("expand:{id}:{open}"))
            })
            .into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn publishes_bounded_treegrid_hierarchy(cx: &mut TestAppContext) {
    let (mut harness, _) = tree_grid(cx, false);
    let root = harness.node("data.tree-grid").unwrap();
    assert_eq!(root.role, Role::TreeGrid);
    assert_eq!(root.value.as_deref(), Some("1000"));
    let rows = harness
        .snapshot()
        .children_of("data.tree-grid")
        .into_iter()
        .filter(|n| n.role == Role::Row)
        .count();
    assert!((1..24).contains(&rows));
    let row = harness.node("data.tree-grid.node-0001").unwrap();
    assert_eq!(row.level, Some(2));
    assert_eq!(
        harness.node("data.tree-grid.node-0001.name").unwrap().role,
        Role::GridCell
    );
    assert!(harness.node("data.tree-grid.node-0900").is_none());
}

#[gpui::test]
fn logical_keys_emit_caller_owned_intents_and_disabled_rows_do_not(cx: &mut TestAppContext) {
    let (mut harness, calls) = tree_grid(cx, false);
    harness.click("data.tree-grid.node-0001");
    harness.keystrokes("left");
    assert!(calls.borrow().iter().any(|call| call == "select:node-0000"));

    let (mut disabled, disabled_calls) = tree_grid(cx, true);
    disabled.click("data.tree-grid.node-0001");
    disabled.keystrokes("left right down");
    assert!(disabled_calls.borrow().is_empty());
}
