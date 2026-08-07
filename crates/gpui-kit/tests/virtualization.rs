//! A large-data surface holds a viewport, not a data set.
//!
//! These tests are about cost and about honesty: how many rows a surface
//! builds when it is handed ten thousand, which rows it builds after the
//! reader scrolls, and how a snapshot tells a row that is off screen from one
//! that is not there at all. The semantic node of a virtualized surface
//! carries the total, so absent and unrendered never look the same.

use std::cell::{Cell as Counter, RefCell};
use std::rc::Rc;

use gpui::{IntoElement, ParentElement, SharedString, TestAppContext, div};
use gpui_kit::prelude::*;
use gpui_kit_semantics::{Node, Role};
use gpui_kit_testkit::harness::Harness;

/// How many rows a surface asked its caller to build.
///
/// The closure belongs to the test, so counting a build needs nothing from the
/// component beyond the API a caller already uses.
#[derive(Clone, Default)]
struct Builds(Rc<Counter<usize>>);

impl Builds {
    fn bump(&self) {
        self.0.set(self.0.get() + 1);
    }

    fn since_reset(&self) -> usize {
        self.0.get()
    }

    fn reset(&self) {
        self.0.set(0);
    }
}

const LARGE: usize = 10_000;
const VIEWPORT: usize = 8;
/// A viewport's worth of rows, a partial row at either edge, the row a
/// `uniform_list` measures to learn its height, and room for the handful of
/// frames a test settles through. Anything within this is a viewport; ten
/// thousand is not.
const BOUND: usize = VIEWPORT * 8;

fn key(index: usize) -> SharedString {
    SharedString::from(format!("record-{index:05}"))
}

fn label(index: usize) -> SharedString {
    SharedString::from(format!("Fixture record {index:05}"))
}

fn rows_under(harness: &mut Harness, parent: &str, role: Role) -> Vec<Node> {
    harness
        .snapshot()
        .descendants_of(parent)
        .into_iter()
        .filter(|node| node.role == role)
        .cloned()
        .collect()
}

fn ids_under(harness: &mut Harness, parent: &str, role: Role) -> Vec<String> {
    rows_under(harness, parent, role)
        .into_iter()
        .map(|node| node.id)
        .collect()
}

/// Every node of `role` whose id sits under `prefix`, found by name rather
/// than by walking parents.
///
/// A virtualized tree can draw a node whose parent has scrolled off the top.
/// The node still reports the parent it has, because that is true, but a walk
/// down from the tree cannot reach it — the link points at something nothing
/// published. Naming is what a test uses instead.
fn ids_named(harness: &mut Harness, prefix: &str, role: Role) -> Vec<String> {
    harness
        .snapshot()
        .under(prefix)
        .into_iter()
        .filter(|node| node.role == role)
        .map(|node| node.id.clone())
        .collect()
}

// ------------------------------------------------------------------ the list

fn list(cx: &mut TestAppContext, count: usize) -> (Harness, Builds) {
    let builds = Builds::default();
    let counted = builds.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let counted = counted.clone();
        List::new("v.list", count, move |index, _, _| {
            counted.bump();
            ListItem::new(key(index), label(index)).text(label(index))
        })
        .visible_rows(VIEWPORT)
        .selected(key(0))
        .on_select(|_, _, _| {})
        .into_any_element()
    });
    builds.reset();
    (harness, builds)
}

#[gpui::test]
fn a_list_of_ten_thousand_builds_a_viewport_and_not_a_data_set(cx: &mut TestAppContext) {
    let (mut harness, builds) = list(cx, LARGE);

    harness.frame();

    let built = builds.since_reset();
    assert!(built > 0, "the list must still build the rows it shows");
    assert!(
        built <= BOUND,
        "a viewport of {VIEWPORT} rows must not build {LARGE}, built {built}"
    );
    assert!(
        rows_under(&mut harness, "v.list", Role::Row).len() <= BOUND,
        "a row that was never built cannot publish a node"
    );
}

#[gpui::test]
fn a_list_reports_its_total_so_an_off_screen_row_is_not_an_absent_one(cx: &mut TestAppContext) {
    let (mut harness, _builds) = list(cx, LARGE);

    assert_eq!(
        harness.node("v.list").expect("published").value.as_deref(),
        Some("10000"),
        "the surface reports the size of the data set it was given"
    );
    assert!(harness.node("v.list.record-00000").is_some());
    assert!(
        harness.node("v.list.record-09000").is_none(),
        "an unrendered row publishes nothing, and the total is what says it exists"
    );
}

#[gpui::test]
fn scrolling_a_list_changes_which_rows_it_builds(cx: &mut TestAppContext) {
    let (mut harness, _builds) = list(cx, LARGE);

    let before = ids_under(&mut harness, "v.list", Role::Row);
    harness.scroll("v.list", 1200.0);
    let after = ids_under(&mut harness, "v.list", Role::Row);

    assert!(!before.is_empty() && !after.is_empty());
    assert_ne!(before, after, "a scroll must move the viewport");
    assert!(
        before.iter().all(|id| !after.contains(id)),
        "a scroll of many viewports must leave none of the old rows behind"
    );
    assert_eq!(
        harness.node("v.list").expect("published").value.as_deref(),
        Some("10000"),
        "scrolling changes what is drawn, never how much there is"
    );
}

#[gpui::test]
fn the_keyboard_reaches_a_list_row_that_was_never_built(cx: &mut TestAppContext) {
    let reported: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = reported.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        List::new("v.list", LARGE, |index, _, _| {
            ListItem::new(key(index), label(index)).text(label(index))
        })
        .visible_rows(VIEWPORT)
        .selected(key(0))
        .on_select(move |id, _, _| sink.borrow_mut().push(id.to_string()))
        .into_any_element()
    });

    harness.click("v.list.record-00000");
    reported.borrow_mut().clear();
    harness.keystrokes("end");

    assert_eq!(*reported.borrow(), vec!["record-09999".to_string()]);
    assert!(
        harness.node("v.list.record-09999").is_some(),
        "the row the list named must be one the typist can see"
    );
    assert!(harness.node("v.list.record-00000").is_none());
}

// ----------------------------------------------------------------- the table

fn table_columns() -> Vec<Column> {
    vec![
        Column::new("name", "Record").flex(2.0),
        Column::new("state", "State").fixed(110.0),
    ]
}

fn table_row(index: usize) -> Row {
    Row::new(key(index))
        .text(label(index))
        .cell(
            "name",
            Cell::new(label(index)).text(label(index)).published(true),
        )
        .cell("state", "Ready")
}

fn table(cx: &mut TestAppContext, count: usize, bounded: bool) -> (Harness, Builds) {
    let builds = Builds::default();
    let counted = builds.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let counted = counted.clone();
        let mut element = Table::new("v.table")
            .columns(table_columns())
            .selected(key(0))
            .rows_from(count, move |index, _, _| {
                counted.bump();
                table_row(index)
            })
            .on_select(|_, _, _| {});
        if bounded {
            element = element.visible_rows(VIEWPORT);
        }
        element.into_any_element()
    });
    builds.reset();
    (harness, builds)
}

#[gpui::test]
fn a_table_built_from_a_source_builds_only_the_rows_its_viewport_holds(cx: &mut TestAppContext) {
    let (mut harness, builds) = table(cx, LARGE, true);

    harness.frame();

    let built = builds.since_reset();
    assert!(built > 0, "the table must still build the rows it shows");
    assert!(
        built <= BOUND,
        "a viewport of {VIEWPORT} rows must not build {LARGE}, built {built}"
    );
    assert_eq!(
        harness.node("v.table").expect("published").value.as_deref(),
        Some("10000"),
        "the table reports the size of the collection, not the size of the viewport"
    );
    assert!(harness.node("v.table.record-00000").is_some());
    assert!(
        harness.node("v.table.record-09000").is_none(),
        "a row outside the viewport is not an assertion target"
    );
}

#[gpui::test]
fn scrolling_a_table_changes_which_rows_it_builds(cx: &mut TestAppContext) {
    let (mut harness, _builds) = table(cx, LARGE, true);

    let before = ids_under(&mut harness, "v.table", Role::Row);
    harness.scroll("v.table", 1200.0);
    let after = ids_under(&mut harness, "v.table", Role::Row);

    assert!(!before.is_empty() && !after.is_empty());
    assert!(
        before.iter().all(|id| !after.contains(id)),
        "a scroll of many viewports must leave none of the old rows behind"
    );
}

/// A table reports a row only when it is clicked, so a caller that moves the
/// selection somewhere the viewport has never drawn is the one that has to
/// bring it into view. Naming the body is how it does that without owning a
/// GPUI handle.
#[gpui::test]
fn a_table_row_far_down_becomes_real_once_it_is_revealed(cx: &mut TestAppContext) {
    let (mut harness, _builds) = table(cx, LARGE, true);

    assert!(
        harness.node("v.table.record-05000").is_none(),
        "the row starts well outside the viewport"
    );

    harness
        .update(|_, cx| gpui_kit::data::reveal_row(&Ident::new("v.table").child("body"), 5000, cx));
    harness.frame();

    assert!(
        harness.node("v.table.record-05000").is_some(),
        "a revealed row is built, laid out, and addressable"
    );
    assert!(harness.node("v.table.record-00000").is_none());
}

/// The two ways in have to agree, or a caller who moved to a source to survive
/// a large collection would be changing the surface as well as its cost.
#[gpui::test]
fn a_small_table_publishes_the_same_thing_either_way_it_is_given_its_rows(cx: &mut TestAppContext) {
    let described = |harness: &mut Harness| -> Vec<(String, Option<String>, bool)> {
        rows_under(harness, "v.table", Role::Row)
            .into_iter()
            .map(|node| (node.id, node.text, node.selected))
            .collect()
    };

    let mut sourced = Harness::new(cx, gpui_kit::install, |_, _| {
        Table::new("v.table")
            .columns(table_columns())
            .selected(key(0))
            .rows_from(3, |index, _, _| table_row(index))
            .visible_rows(VIEWPORT)
            .on_select(|_, _, _| {})
            .into_any_element()
    });
    let from_source = described(&mut sourced);
    let sourced_total = sourced.node("v.table").expect("published").value;
    let sourced_cell = sourced
        .node("v.table.record-00001.name")
        .expect("published");

    let mut materialized = Harness::new(cx, gpui_kit::install, |_, _| {
        Table::new("v.table")
            .columns(table_columns())
            .selected(key(0))
            .rows((0..3).map(table_row))
            .visible_rows(VIEWPORT)
            .on_select(|_, _, _| {})
            .into_any_element()
    });
    let from_rows = described(&mut materialized);
    let materialized_cell = materialized
        .node("v.table.record-00001.name")
        .expect("published");

    assert_eq!(from_rows.len(), 3);
    assert_eq!(from_source, from_rows);
    assert_eq!(
        sourced_total,
        materialized.node("v.table").expect("published").value
    );
    assert_eq!(sourced_cell.text, materialized_cell.text);
    assert_eq!(sourced_cell.parent, materialized_cell.parent);
}

// ------------------------------------------------------------------ the tree

/// A hierarchy of one shallow branch per hundred leaves, so a bounded tree has
/// far more disclosed rows than it can show.
fn forest(branches: usize, leaves: usize) -> Vec<TreeNode> {
    (0..branches)
        .map(|branch| {
            TreeNode::new(
                SharedString::from(format!("branch-{branch:04}")),
                SharedString::from(format!("Branch {branch:04}")),
            )
            .children((0..leaves).map(move |leaf| {
                let index = branch * leaves + leaf;
                TreeNode::new(key(index), label(index))
            }))
        })
        .collect()
}

fn expanded(branches: usize) -> Vec<SharedString> {
    (0..branches)
        .map(|branch| SharedString::from(format!("branch-{branch:04}")))
        .collect()
}

fn tree(cx: &mut TestAppContext, branches: usize, leaves: usize, bounded: bool) -> Harness {
    Harness::new(cx, gpui_kit::install, move |_, _| {
        let mut element = Tree::new("v.tree")
            .nodes(forest(branches, leaves))
            .expanded(expanded(branches))
            .selected("branch-0000")
            .on_select(|_, _, _| {})
            .on_toggle(|_, _, _, _| {});
        if bounded {
            element = element.visible_rows(VIEWPORT);
        }
        element.into_any_element()
    })
}

#[gpui::test]
fn a_bounded_tree_draws_a_viewport_and_counts_what_it_disclosed(cx: &mut TestAppContext) {
    // A hundred branches of a hundred leaves, all open: 10,100 disclosed rows.
    let mut harness = tree(cx, 100, 100, true);

    let node = harness.node("v.tree").expect("published");
    assert_eq!(
        node.value.as_deref(),
        Some("10100"),
        "the tree reports how many rows it disclosed, which is what makes an \
         unrendered row different from a collapsed one"
    );

    let drawn = ids_named(&mut harness, "v.tree.", Role::TreeItem);
    assert!(!drawn.is_empty());
    assert!(
        drawn.len() <= BOUND,
        "a viewport of {VIEWPORT} rows must not publish 10100 nodes, published {}",
        drawn.len()
    );
    assert!(harness.node("v.tree.branch-0000").is_some());
    assert!(
        harness.node("v.tree.branch-0099").is_none(),
        "a disclosed node outside the viewport is counted, not published"
    );
}

#[gpui::test]
fn scrolling_a_tree_changes_which_nodes_it_draws(cx: &mut TestAppContext) {
    let mut harness = tree(cx, 100, 100, true);

    let before = ids_named(&mut harness, "v.tree.", Role::TreeItem);
    harness.scroll("v.tree", 1200.0);
    let after = ids_named(&mut harness, "v.tree.", Role::TreeItem);

    assert!(!before.is_empty() && !after.is_empty());
    assert!(
        before.iter().all(|id| !after.contains(id)),
        "a scroll of many viewports must leave none of the old rows behind"
    );
    assert_eq!(
        harness.node("v.tree").expect("published").value.as_deref(),
        Some("10100"),
        "scrolling changes what is drawn, never what is disclosed"
    );
}

#[gpui::test]
fn the_keyboard_reaches_a_tree_node_the_viewport_has_never_drawn(cx: &mut TestAppContext) {
    let reported: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = reported.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Tree::new("v.tree")
            .nodes(forest(10, 100))
            .expanded(expanded(10))
            .selected("branch-0000")
            .visible_rows(VIEWPORT)
            .on_select(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .on_toggle(|_, _, _, _| {})
            .into_any_element()
    });

    harness.click("v.tree.branch-0000");
    reported.borrow_mut().clear();
    harness.keystrokes("end");

    // The last disclosed row is the last leaf of the last branch.
    assert_eq!(*reported.borrow(), vec![key(999).to_string()]);
    assert!(
        harness.node(&format!("v.tree.{}", key(999))).is_some(),
        "the node the tree named must be one the typist can see"
    );
    assert!(harness.node("v.tree.branch-0000").is_none());
}

#[gpui::test]
fn a_small_tree_publishes_the_same_thing_bounded_or_not(cx: &mut TestAppContext) {
    let mut bounded = tree(cx, 2, 3, true);
    let with_bound = ids_named(&mut bounded, "v.tree.", Role::TreeItem);
    let bounded_leaf = bounded
        .node(&format!("v.tree.{}", key(4)))
        .expect("published");

    let mut unbounded = tree(cx, 2, 3, false);
    let without_bound = ids_named(&mut unbounded, "v.tree.", Role::TreeItem);
    let unbounded_leaf = unbounded
        .node(&format!("v.tree.{}", key(4)))
        .expect("published");

    assert_eq!(with_bound.len(), 8, "two branches and six leaves");
    assert_eq!(with_bound, without_bound);
    assert_eq!(bounded_leaf.level, unbounded_leaf.level);
    assert_eq!(bounded_leaf.parent, unbounded_leaf.parent);
    assert_eq!(
        bounded.node("v.tree").expect("published").value,
        unbounded.node("v.tree").expect("published").value
    );
}

// -------------------------------------------------------------- the datagrid

fn grid_columns() -> Vec<GridColumn> {
    vec![
        GridColumn::new("name", "Record").flex(2.0).min_width(120.0),
        GridColumn::new("owner", "Owner")
            .fixed(140.0)
            .editable(true),
    ]
}

fn grid_row(index: usize) -> GridRow {
    GridRow::new(key(index))
        .text(label(index))
        .cell(
            "name",
            Cell::new(label(index)).text(label(index)).published(true),
        )
        .cell("owner", SharedString::from(format!("owner-{index:05}")))
}

#[gpui::test]
fn a_grid_of_ten_thousand_builds_a_viewport_and_not_a_data_set(cx: &mut TestAppContext) {
    let builds = Builds::default();
    let counted = builds.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let counted = counted.clone();
        DataGrid::new("v.grid", LARGE, move |index, _, _| {
            counted.bump();
            grid_row(index)
        })
        .columns(grid_columns())
        .visible_rows(VIEWPORT)
        .selection_mode(SelectionMode::Multiple)
        .on_select(|_, _, _| {})
        .into_any_element()
    });
    builds.reset();

    harness.frame();

    let built = builds.since_reset();
    assert!(built > 0, "the grid must still build the rows it shows");
    assert!(
        built <= BOUND,
        "a viewport of {VIEWPORT} rows must not build {LARGE}, built {built}"
    );
    assert_eq!(
        harness.node("v.grid").expect("published").value.as_deref(),
        Some("10000")
    );
    assert!(
        harness.node("v.grid.record-09000").is_none(),
        "a row outside the viewport is not an assertion target"
    );
}

/// The field lives with the grid, not with the row, so scrolling the cell out
/// of sight and back finds the same field holding what was typed into it. A
/// grid that rebuilt the editor would silently drop an unfinished edit.
#[gpui::test]
fn an_open_editor_survives_a_scroll_that_takes_its_cell_off_screen(cx: &mut TestAppContext) {
    let edits: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = edits.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        DataGrid::new("v.grid", 400, |index, _, _| grid_row(index))
            .columns(grid_columns())
            .visible_rows(VIEWPORT)
            .editing(Some(EditingCell::new(key(1), "owner", "owner-00001")))
            .on_edit(move |intent, _, _| {
                sink.borrow_mut()
                    .push(format!("{}:{:?}", intent.value, intent.outcome));
            })
            .into_any_element()
    });

    harness.keystrokes("x");
    assert_eq!(
        harness
            .node("v.grid.edit")
            .expect("the cell is a field")
            .value
            .as_deref(),
        Some("owner-00001x")
    );

    harness.scroll("v.grid", 2000.0);
    assert!(
        harness.node("v.grid.edit").is_none(),
        "a cell that is off screen draws nothing, field or not"
    );
    assert!(
        edits.borrow().is_empty(),
        "scrolling is not a commit and not a revert"
    );

    harness.scroll("v.grid", -4000.0);
    assert_eq!(
        harness
            .node("v.grid.edit")
            .expect("the field is back with the cell")
            .value
            .as_deref(),
        Some("owner-00001x"),
        "the field kept what was typed into it across the scroll"
    );
}

/// A detail region drawn over the slots beneath an opened row is still there
/// after the row is scrolled away and back, for the same reason.
#[gpui::test]
fn a_grid_row_that_scrolls_away_and_back_is_the_same_row(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        DataGrid::new("v.grid", 400, |index, _, _| grid_row(index))
            .columns(grid_columns())
            .visible_rows(VIEWPORT)
            .expanded(vec![Expanded::new(key(1), 1)])
            .detail_rows(2)
            .detail(|id, _, _| {
                div()
                    .child(SharedString::from(format!("Detail {id}")))
                    .into_any_element()
            })
            .into_any_element()
    });

    let before = harness
        .node(&format!("v.grid.{}", key(1)))
        .expect("published");
    harness.scroll("v.grid", 2000.0);
    assert!(harness.node(&format!("v.grid.{}", key(1))).is_none());

    harness.scroll("v.grid", -4000.0);
    let after = harness
        .node(&format!("v.grid.{}", key(1)))
        .expect("published");
    assert_eq!(before.id, after.id);
    assert_eq!(before.text, after.text);
    assert_eq!(before.expanded, after.expanded);
}
