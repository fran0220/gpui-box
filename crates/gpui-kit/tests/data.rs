//! List, table, and tree hold a viewport, not a data set. They report what was
//! operated and render exactly what the caller says is true.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, SharedString, Styled, TestAppContext};
use gpui_kit::data::glide_to_row;
use gpui_kit::motion::engage_end;
use gpui_kit::prelude::*;
use gpui_kit_semantics::{Node, NodeSpec, Role, Semantic};
use gpui_kit_testkit::harness::Harness;

type Calls<T> = Rc<RefCell<Vec<T>>>;

fn recorder<T: 'static>() -> (Calls<T>, Calls<T>) {
    let calls: Calls<T> = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

/// A synthetic record set. The identity is a fixture key, not a position.
fn records(count: usize) -> Vec<(SharedString, SharedString)> {
    (0..count)
        .map(|index| {
            (
                SharedString::from(format!("record-{index:04}")),
                SharedString::from(format!("Fixture record {index:04}")),
            )
        })
        .collect()
}

type Data = Rc<RefCell<Vec<(SharedString, SharedString)>>>;

fn list(
    cx: &mut TestAppContext,
    data: Data,
    selected: &'static str,
    refused: &'static [&'static str],
) -> (Harness, Calls<String>) {
    let (calls, sink) = recorder::<String>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        let rows = data.clone();
        let count = rows.borrow().len();
        List::new("data.list", count, move |index, _, _| {
            let (id, label) = rows.borrow()[index].clone();
            let disabled = refused.contains(&id.as_ref());
            ListItem::new(id, label.clone())
                .text(label)
                .disabled(disabled)
        })
        .selected(selected)
        .visible_rows(8)
        .on_select(move |id, _, _| sink.borrow_mut().push(id.to_string()))
        .into_any_element()
    });
    (harness, calls)
}

fn rows_of(harness: &mut Harness, parent: &str) -> Vec<Node> {
    harness
        .snapshot()
        .children_of(parent)
        .into_iter()
        .filter(|node| node.role == Role::Row)
        .cloned()
        .collect()
}

#[gpui::test]
fn a_virtualized_list_publishes_its_total_and_only_the_rows_it_drew(cx: &mut TestAppContext) {
    let data: Data = Rc::new(RefCell::new(records(1000)));
    let (mut harness, _calls) = list(cx, data, "record-0000", &[]);

    let list = harness.node("data.list").expect("published");
    assert_eq!(list.role, Role::List);
    assert_eq!(
        list.value.as_deref(),
        Some("1000"),
        "the list must report the size of the data set it was given"
    );

    let drawn = rows_of(&mut harness, "data.list");
    assert!(
        (1..20).contains(&drawn.len()),
        "a viewport of eight rows must not publish a thousand nodes, drew {}",
        drawn.len()
    );
    assert!(harness.node("data.list.record-0000").is_some());
    assert!(
        harness.node("data.list.record-0900").is_none(),
        "a row outside the viewport must not be addressable"
    );
}

#[gpui::test]
fn a_row_keeps_its_id_when_the_data_is_reordered(cx: &mut TestAppContext) {
    // The whole set fits in the viewport, so a reorder changes the order the
    // rows are drawn in rather than which rows are drawn at all.
    let data: Data = Rc::new(RefCell::new(records(6)));
    let (mut harness, _calls) = list(cx, data.clone(), "record-0000", &[]);

    let before: Vec<String> = rows_of(&mut harness, "data.list")
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(before.len(), 6);
    assert!(before.contains(&"data.list.record-0000".to_string()));

    harness.update(|window, _| {
        data.borrow_mut().reverse();
        window.refresh();
    });

    let after: Vec<String> = rows_of(&mut harness, "data.list")
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_ne!(before, after, "the fixture must actually have reordered");
    let mut sorted_before = before.clone();
    let mut sorted_after = after.clone();
    sorted_before.sort();
    sorted_after.sort();
    assert_eq!(
        sorted_before, sorted_after,
        "an id derived from identity survives a reorder"
    );
}

#[gpui::test]
fn clicking_a_row_reports_it_without_moving_the_selection(cx: &mut TestAppContext) {
    let data: Data = Rc::new(RefCell::new(records(40)));
    let (mut harness, calls) = list(cx, data, "record-0001", &[]);

    harness.click("data.list.record-0003");

    assert_eq!(*calls.borrow(), vec!["record-0003".to_string()]);
    assert!(
        harness
            .node("data.list.record-0001")
            .expect("published")
            .selected,
        "the caller owns the selection, so nothing moved"
    );
    assert!(
        !harness
            .node("data.list.record-0003")
            .expect("published")
            .selected
    );
}

#[gpui::test]
fn a_refused_row_installs_no_handler(cx: &mut TestAppContext) {
    let data: Data = Rc::new(RefCell::new(records(40)));
    let (mut harness, calls) = list(cx, data, "record-0000", &["record-0002"]);

    harness.click("data.list.record-0002");

    assert!(calls.borrow().is_empty());
    assert!(
        harness
            .node("data.list.record-0002")
            .expect("published")
            .disabled
    );
}

#[gpui::test]
fn the_keyboard_moves_through_the_list_and_skips_refusals(cx: &mut TestAppContext) {
    let data: Data = Rc::new(RefCell::new(records(1000)));
    let (mut harness, calls) = list(cx, data, "record-0000", &["record-0001"]);

    harness.click("data.list.record-0000");
    calls.borrow_mut().clear();
    harness.keystrokes("down");
    assert_eq!(
        *calls.borrow(),
        vec!["record-0002".to_string()],
        "a refused row is passed over, not reported"
    );

    calls.borrow_mut().clear();
    harness.keystrokes("up");
    assert!(
        calls.borrow().is_empty(),
        "the selection is still the first row, so up reports nothing"
    );
}

#[gpui::test]
fn end_reports_a_row_the_viewport_has_never_drawn(cx: &mut TestAppContext) {
    let data: Data = Rc::new(RefCell::new(records(1000)));
    let (mut harness, calls) = list(cx, data, "record-0000", &[]);

    harness.click("data.list.record-0000");
    calls.borrow_mut().clear();
    harness.keystrokes("end");

    assert_eq!(*calls.borrow(), vec!["record-0999".to_string()]);
    // The list scrolled to what it reported, so the row is now on screen.
    assert!(harness.node("data.list.record-0999").is_some());
    assert!(harness.node("data.list.record-0000").is_none());
}

fn table(
    cx: &mut TestAppContext,
    sort: Option<(SharedString, SortDirection)>,
) -> (Harness, Calls<(String, String)>, Calls<String>) {
    let (sorts, sort_sink) = recorder::<(String, String)>();
    let (selects, select_sink) = recorder::<String>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sort_sink = sort_sink.clone();
        let select_sink = select_sink.clone();
        Table::new("data.table")
            .columns([
                Column::new("name", "Run").flex(2.0).sortable(true),
                Column::new("state", "State").fixed(110.0),
                Column::new("duration", "Duration")
                    .fixed(96.0)
                    .sortable(true),
            ])
            .sort(sort.clone())
            .selected("run-b12")
            .rows([
                Row::new("run-a04")
                    .text("Indexing")
                    .cell("name", "Indexing")
                    .cell("state", Cell::new("Ready").text("Ready").published(true))
                    .cell("duration", "4m 12s"),
                Row::new("run-b12")
                    .text("Verifying")
                    .cell("name", "Verifying")
                    .cell("state", Cell::new("Stale").text("Stale").published(true))
                    .cell("duration", "2m 08s"),
                Row::new("run-c31")
                    .text("Archiving")
                    .disabled(true)
                    .cell("name", "Archiving")
                    .cell(
                        "state",
                        Cell::new("Managed").text("Managed").published(true),
                    )
                    .cell("duration", "0m 51s"),
            ])
            .on_sort(move |key, direction, _, _| {
                sort_sink
                    .borrow_mut()
                    .push((key.to_string(), direction.as_str().to_string()))
            })
            .on_select(move |id, _, _| select_sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });
    (harness, sorts, selects)
}

#[gpui::test]
fn a_table_publishes_its_row_count_and_one_node_per_row(cx: &mut TestAppContext) {
    let (mut harness, _sorts, _selects) = table(cx, None);

    let node = harness.node("data.table").expect("published");
    assert_eq!(node.role, Role::Table);
    assert_eq!(node.value.as_deref(), Some("3"));

    let rows = rows_of(&mut harness, "data.table");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].id, "data.table.run-a04");
    assert_eq!(rows[0].text.as_deref(), Some("Indexing"));
    assert!(
        harness
            .node("data.table.run-b12")
            .expect("published")
            .selected
    );
}

#[gpui::test]
fn only_a_marked_cell_is_an_assertion_target(cx: &mut TestAppContext) {
    let (mut harness, _sorts, _selects) = table(cx, None);

    let cell = harness.node("data.table.run-a04.state").expect("published");
    assert_eq!(cell.role, Role::Cell);
    assert_eq!(cell.text.as_deref(), Some("Ready"));
    assert_eq!(cell.parent.as_deref(), Some("data.table.run-a04"));
    assert!(
        harness.node("data.table.run-a04.name").is_none(),
        "an unmarked cell must not add a node to the tree"
    );
}

#[gpui::test]
fn a_sortable_header_reports_the_next_direction_and_sorts_nothing(cx: &mut TestAppContext) {
    let (mut harness, sorts, _selects) = table(
        cx,
        Some((SharedString::from("duration"), SortDirection::Ascending)),
    );

    let header = harness
        .node("data.table.header.duration")
        .expect("published");
    assert_eq!(header.role, Role::Button);
    assert_eq!(header.value.as_deref(), Some("ascending"));
    assert_eq!(
        harness
            .node("data.table.header.name")
            .expect("published")
            .value
            .as_deref(),
        Some("unsorted")
    );

    let before: Vec<String> = rows_of(&mut harness, "data.table")
        .into_iter()
        .map(|row| row.id)
        .collect();
    harness.click("data.table.header.duration");

    assert_eq!(
        *sorts.borrow(),
        vec![("duration".to_string(), "descending".to_string())]
    );
    let after: Vec<String> = rows_of(&mut harness, "data.table")
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(before, after, "the table renders the order it was given");
}

#[gpui::test]
fn a_header_that_does_not_sort_is_not_a_button(cx: &mut TestAppContext) {
    let (mut harness, sorts, _selects) = table(cx, None);

    let header = harness.node("data.table.header.state").expect("published");
    assert_eq!(header.role, Role::Cell);

    harness.click("data.table.header.state");
    assert!(sorts.borrow().is_empty());
}

#[gpui::test]
fn an_empty_table_is_not_a_ready_table_with_no_rows(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        Table::new("data.table")
            .columns([Column::new("name", "Run")])
            .into_any_element()
    });

    let empty = harness.node("data.table.empty").expect("published");
    assert_eq!(empty.value.as_deref(), Some("empty"));
    assert_eq!(
        harness
            .node("data.table")
            .expect("published")
            .value
            .as_deref(),
        Some("0")
    );
    assert!(rows_of(&mut harness, "data.table").is_empty());
}

#[gpui::test]
fn a_loading_table_with_no_rows_is_busy_not_empty(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        Table::new("data.table")
            .columns([Column::new("name", "Run")])
            .loading(true)
            .into_any_element()
    });

    let loading = harness.node("data.table.loading").expect("published");
    assert!(loading.busy);
    assert_eq!(loading.value.as_deref(), Some("loading"));
    assert!(harness.node("data.table.empty").is_none());
}

#[gpui::test]
fn a_failed_refresh_keeps_the_table_rows_that_are_still_true(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        Table::new("data.table")
            .columns([Column::new("name", "Run")])
            .rows([Row::new("run-a04")
                .text("Indexing")
                .cell("name", "Indexing")])
            .failure("The host refused the refresh")
            .into_any_element()
    });

    assert!(harness.node("data.table.run-a04").is_some());
    let banner = harness.node("data.table.failure").expect("published");
    assert_eq!(banner.role, Role::Status);
    assert_eq!(banner.value.as_deref(), Some("stale"));
    assert_eq!(banner.text.as_deref(), Some("The host refused the refresh"));
    assert!(banner.invalid);
    assert!(harness.node("data.table.empty").is_none());
}

#[gpui::test]
fn a_table_failure_with_nothing_behind_it_takes_the_surface(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        Table::new("data.table")
            .columns([Column::new("name", "Run")])
            .failure("The host refused the refresh")
            .into_any_element()
    });

    let empty = harness.node("data.table.empty").expect("published");
    assert_eq!(empty.value.as_deref(), Some("failed"));
    assert!(rows_of(&mut harness, "data.table").is_empty());
}

#[gpui::test]
fn a_disabled_table_row_installs_no_handler(cx: &mut TestAppContext) {
    let (mut harness, _sorts, selects) = table(cx, None);

    harness.click("data.table.run-c31");
    assert!(selects.borrow().is_empty());

    harness.click("data.table.run-a04");
    assert_eq!(*selects.borrow(), vec!["run-a04".to_string()]);
    assert!(
        harness
            .node("data.table.run-b12")
            .expect("published")
            .selected,
        "the caller still owns the selection"
    );
}

fn tree(
    cx: &mut TestAppContext,
    expanded: &'static [&'static str],
    selected: &'static str,
) -> (Harness, Calls<(String, bool)>, Calls<String>) {
    let (toggles, toggle_sink) = recorder::<(String, bool)>();
    let (selects, select_sink) = recorder::<String>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let toggle_sink = toggle_sink.clone();
        let select_sink = select_sink.clone();
        Tree::new("data.tree")
            .expanded_ids(expanded)
            .selected(selected)
            .nodes([
                TreeNode::new("workspace", "workspace").children([
                    TreeNode::new("crates", "crates").children([TreeNode::new("kit", "gpui-kit")]),
                    TreeNode::new("docs", "docs"),
                ]),
                TreeNode::new("target", "target").disabled(true),
                TreeNode::new("readme", "README.md"),
            ])
            .on_toggle(move |id, next, _, _| toggle_sink.borrow_mut().push((id.to_string(), next)))
            .on_select(move |id, _, _| select_sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });
    (harness, toggles, selects)
}

#[gpui::test]
fn a_collapsed_node_renders_none_of_its_children(cx: &mut TestAppContext) {
    let (mut harness, _toggles, _selects) = tree(cx, &[], "workspace");

    let node = harness.node("data.tree.workspace").expect("published");
    assert_eq!(node.role, Role::TreeItem);
    assert_eq!(node.expanded, Some(false));
    assert_eq!(node.level, Some(1));
    assert_eq!(node.parent.as_deref(), Some("data.tree"));
    assert!(
        harness.node("data.tree.crates").is_none(),
        "a shut branch must not leave its children addressable"
    );
    assert_eq!(
        harness
            .node("data.tree.readme")
            .expect("published")
            .expanded,
        None,
        "a leaf claims no disclosure state"
    );
}

#[gpui::test]
fn an_open_node_publishes_its_children_one_level_deeper(cx: &mut TestAppContext) {
    let (mut harness, _toggles, _selects) = tree(cx, &["workspace", "crates"], "workspace");

    let child = harness.node("data.tree.kit").expect("published");
    assert_eq!(child.level, Some(3));
    assert_eq!(child.parent.as_deref(), Some("data.tree.crates"));
    assert_eq!(
        harness
            .node("data.tree.crates")
            .expect("published")
            .expanded,
        Some(true)
    );
}

#[gpui::test]
fn the_disclosure_reports_the_state_the_node_should_take(cx: &mut TestAppContext) {
    let (mut harness, toggles, selects) = tree(cx, &[], "workspace");

    harness.click("data.tree.workspace.toggle");

    assert_eq!(*toggles.borrow(), vec![("workspace".to_string(), true)]);
    assert!(
        selects.borrow().is_empty(),
        "a disclosure is not a selection"
    );
    assert_eq!(
        harness
            .node("data.tree.workspace")
            .expect("published")
            .expanded,
        Some(false),
        "nothing was applied, so nothing opened"
    );
}

#[gpui::test]
fn the_disclosure_answers_the_keyboard_it_is_reachable_by(cx: &mut TestAppContext) {
    let (mut harness, toggles, _selects) = tree(cx, &[], "workspace");

    harness.click("data.tree.workspace.toggle");
    toggles.borrow_mut().clear();
    harness.keystrokes("enter");

    assert_eq!(
        *toggles.borrow(),
        vec![("workspace".to_string(), true)],
        "a control tab can reach must answer the keyboard as well as the pointer"
    );
}

#[gpui::test]
fn the_keyboard_walks_only_the_visible_nodes(cx: &mut TestAppContext) {
    let (mut harness, _toggles, selects) = tree(cx, &["workspace"], "workspace");

    harness.click("data.tree.workspace");
    selects.borrow_mut().clear();
    harness.keystrokes("down");
    assert_eq!(
        *selects.borrow(),
        vec!["crates".to_string()],
        "the next visible node is the open branch's first child"
    );

    // `docs` is followed by a refusal, so a move from it lands past `target`.
    let (mut harness, _toggles, selects) = tree(cx, &["workspace"], "docs");
    harness.click("data.tree.docs");
    selects.borrow_mut().clear();
    harness.keystrokes("down");
    assert_eq!(*selects.borrow(), vec!["readme".to_string()]);
}

#[gpui::test]
fn right_opens_a_branch_and_left_ascends_from_a_leaf(cx: &mut TestAppContext) {
    let (mut harness, toggles, _selects) = tree(cx, &[], "workspace");
    harness.click("data.tree.workspace");
    harness.keystrokes("right");
    assert_eq!(*toggles.borrow(), vec![("workspace".to_string(), true)]);

    let (mut harness, _toggles, selects) = tree(cx, &["workspace"], "docs");
    harness.click("data.tree.docs");
    selects.borrow_mut().clear();
    harness.keystrokes("left");
    assert_eq!(*selects.borrow(), vec!["workspace".to_string()]);
}

#[gpui::test]
fn a_refused_node_reports_nothing(cx: &mut TestAppContext) {
    let (mut harness, _toggles, selects) = tree(cx, &[], "workspace");

    harness.click("data.tree.target");

    assert!(selects.borrow().is_empty());
    assert!(
        harness
            .node("data.tree.target")
            .expect("published")
            .disabled
    );
}

#[gpui::test]
fn a_virtualized_row_is_settled_on_the_frame_it_first_renders(cx: &mut TestAppContext) {
    // A row that entered a viewport is not an arrival, it is the same row that
    // was always there, so it gets no entrance and nothing about it is still
    // moving on the next frame.
    let data: Data = Rc::new(RefCell::new(records(4)));
    let (mut harness, _calls) = list(cx, data.clone(), "record-0000", &[]);

    harness.update({
        let data = data.clone();
        move |_, cx| {
            data.borrow_mut()
                .insert(0, ("record-new".into(), "Fixture record new".into()));
            cx.refresh_windows();
        }
    });
    let arrived: Vec<Node> = rows_of(&mut harness, "data.list");

    harness.advance(std::time::Duration::from_millis(400));
    assert_eq!(
        rows_of(&mut harness, "data.list"),
        arrived,
        "a row must be where it belongs on the frame it appears"
    );
}

// --------------------------------------------------------------------- flow

/// A flow of caller-drawn rows: a turn is a card that names itself, and
/// nothing wraps it. Rows differ in height, which is the only reason a flow
/// exists rather than a uniform list.
fn turns(cx: &mut TestAppContext, data: Data) -> Harness {
    Harness::new(cx, gpui_kit::install, move |_, _| {
        let rows = data.clone();
        let count = rows.borrow().len();
        let keys: Vec<SharedString> = rows.borrow().iter().map(|(id, _)| id.clone()).collect();
        Flow::new("data.flow", count, move |index, _, cx| {
            let (id, label) = rows.borrow()[index].clone();
            gpui::div()
                .w_full()
                .h(gpui::px(if index % 2 == 0 { 40.0 } else { 90.0 }))
                .semantic_in(
                    cx,
                    NodeSpec::new(format!("data.flow.{id}"), Role::Row)
                        .parent("data.flow")
                        .text(label),
                )
                .into_any_element()
        })
        .keys(keys)
        .estimate(60.0)
        .anchored_to_end()
        .visible_rows(4)
        .into_any_element()
    })
}

#[gpui::test]
fn a_flow_lays_out_the_rows_its_viewport_reaches_and_no_others(cx: &mut TestAppContext) {
    let data: Data = Rc::new(RefCell::new(records(200)));
    let mut harness = turns(cx, data);
    harness.frame();

    let drawn = (0..200)
        .filter(|index| {
            harness
                .node(&format!("data.flow.record-{index:04}"))
                .is_some()
        })
        .count();
    assert!(
        drawn > 0 && drawn < 40,
        "a viewport four rows tall drew {drawn} of two hundred rows"
    );
}

#[gpui::test]
fn a_flow_is_followed_by_name_like_any_other_surface(cx: &mut TestAppContext) {
    // The point of drawing the rows yourself is that you keep everything else:
    // a surface whose rows nobody else designed is still scrolled, followed
    // and released by naming it.
    let data: Data = Rc::new(RefCell::new(records(200)));
    let mut harness = turns(cx, data.clone());
    harness.frame();

    let flow = Ident::new("data.flow");
    harness.update({
        let flow = flow.clone();
        move |window, cx| engage_end(&flow, window, cx)
    });
    for _ in 0..90 {
        harness.advance(std::time::Duration::from_millis(16));
    }
    assert!(
        harness.node("data.flow.record-0199").is_some(),
        "following the end has to arrive at the end"
    );

    data.borrow_mut()
        .push(("record-new".into(), "Fixture record new".into()));
    for _ in 0..90 {
        harness.advance(std::time::Duration::from_millis(16));
    }
    assert!(
        harness.node("data.flow.record-new").is_some(),
        "a row arriving while the end is held is followed"
    );
}

#[gpui::test]
fn a_glide_travels_to_its_row_rather_than_arriving_at_it(cx: &mut TestAppContext) {
    // The distance is unmeasured — two hundred rows nobody has laid out — so
    // what this asserts is that the travel is spread over frames at all, and
    // that whatever the estimate did on the way, the arrival is exact.
    let data: Data = Rc::new(RefCell::new(records(200)));
    let mut harness = turns(cx, data);
    harness.frame();
    assert!(
        harness.node("data.flow.record-0199").is_some(),
        "a flow anchored to its end opens there"
    );

    let flow = Ident::new("data.flow");
    harness.update({
        let flow = flow.clone();
        move |window, cx| glide_to_row(&flow, 0, window, cx)
    });
    for _ in 0..8 {
        harness.advance(std::time::Duration::from_millis(16));
    }
    assert!(
        harness.node("data.flow.record-0199").is_none(),
        "eight frames into a glide the end must be behind the reader"
    );

    for _ in 0..90 {
        harness.advance(std::time::Duration::from_millis(16));
    }
    assert!(
        harness.node("data.flow.record-0000").is_some(),
        "however the travel went, it ends on the row that was asked for"
    );
}
