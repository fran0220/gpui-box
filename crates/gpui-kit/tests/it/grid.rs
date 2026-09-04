//! `DataGrid` holds a viewport, not a data set, and applies nothing. It
//! reports what was operated and renders exactly what the caller says is true.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    IntoElement, Modifiers, ParentElement, ScrollDelta, ScrollWheelEvent, SharedString, Styled,
    TestAppContext, TouchPhase, div, point, px,
};
use gpui_kit::prelude::*;
use gpui_kit_semantics::{Node, Role};
use gpui_kit_testkit::harness::Harness;

type Calls<T> = Rc<RefCell<Vec<T>>>;

/// One synthetic job. The identity is a fixture key, never a position.
fn job(index: usize) -> (SharedString, SharedString, SharedString) {
    (
        SharedString::from(format!("job-{index:04}")),
        SharedString::from(format!("Fixture job {index:04}")),
        SharedString::from(format!("owner-{}", index % 3)),
    )
}

fn columns() -> Vec<GridColumn> {
    vec![
        GridColumn::new("name", "Job")
            .flex(2.0)
            .min_width(120.0)
            .pinned(true)
            .sortable(true),
        GridColumn::new("owner", "Owner")
            .fixed(140.0)
            .reorderable(true)
            .editable(true),
        GridColumn::new("duration", "Duration")
            .fixed(120.0)
            .min_width(60.0)
            .sortable(true)
            .resizable(true),
    ]
}

fn row(index: usize) -> GridRow {
    let (id, name, owner) = job(index);
    GridRow::new(id)
        .text(name.clone())
        .cell("name", Cell::new(name.clone()).text(name).published(true))
        .cell("owner", Cell::new(owner.clone()).text(owner))
        .cell("duration", format!("{}m", index % 9 + 1))
}

/// Everything a test hands the grid, so a single builder covers every case.
#[derive(Clone, Default)]
struct Given {
    count: usize,
    total: Option<usize>,
    sort: Option<(SharedString, SortDirection)>,
    selected: Vec<SharedString>,
    mode: SelectionMode,
    failure: Option<SharedString>,
    editing: Option<EditingCell>,
    expanded: Vec<Expanded>,
}

impl Given {
    fn rows(count: usize) -> Self {
        Self {
            count,
            mode: SelectionMode::Multiple,
            ..Self::default()
        }
    }
}

#[derive(Default)]
struct Reports {
    sorts: Calls<(String, String)>,
    selections: Calls<String>,
    widths: Calls<(String, f32)>,
    fits: Calls<String>,
    orders: Calls<String>,
    expansions: Calls<(String, bool)>,
    edit_requests: Calls<(String, String)>,
    edits: Calls<(String, String, String, String)>,
}

fn grid(cx: &mut TestAppContext, given: Given) -> (Harness, Rc<Reports>) {
    let reports = Rc::new(Reports::default());
    let sinks = Rc::clone(&reports);
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sinks = Rc::clone(&sinks);
        let given = given.clone();
        let mut element = DataGrid::new("data.grid", given.count, |index, _, _| row(index))
            .columns(columns())
            .sort(given.sort.clone())
            .selection_mode(given.mode)
            .selected(given.selected.clone())
            .expanded(given.expanded.clone())
            .detail_rows(2)
            .detail(|id, _, _| {
                gpui::div()
                    .child(SharedString::from(format!("Detail for {id}")))
                    .into_any_element()
            })
            .editing(given.editing.clone())
            .visible_rows(8)
            .on_sort({
                let sink = Rc::clone(&sinks);
                move |key, direction, _, _| {
                    sink.sorts
                        .borrow_mut()
                        .push((key.to_string(), direction.as_str().to_string()));
                }
            })
            .on_select({
                let sink = Rc::clone(&sinks);
                move |change, _, _| {
                    let described = match change {
                        SelectionChange::Replace(id) => format!("replace:{id}"),
                        SelectionChange::Toggle(id) => format!("toggle:{id}"),
                        SelectionChange::Range { anchor, to } => format!("range:{anchor}..{to}"),
                        SelectionChange::Loaded => "loaded".to_string(),
                        SelectionChange::Everything => "everything".to_string(),
                        SelectionChange::Clear => "clear".to_string(),
                    };
                    sink.selections.borrow_mut().push(described);
                }
            })
            .on_resize({
                let sink = Rc::clone(&sinks);
                move |key, width, _, _| sink.widths.borrow_mut().push((key.to_string(), width))
            })
            .on_fit({
                let sink = Rc::clone(&sinks);
                move |key, _, _| sink.fits.borrow_mut().push(key.to_string())
            })
            .on_reorder({
                let sink = Rc::clone(&sinks);
                move |intent, _, _| {
                    sink.orders
                        .borrow_mut()
                        .push(format!("{} {}", intent.item.id, intent.position));
                }
            })
            .on_expand({
                let sink = Rc::clone(&sinks);
                move |id, open, _, _| sink.expansions.borrow_mut().push((id.to_string(), open))
            })
            .on_edit_request({
                let sink = Rc::clone(&sinks);
                move |row, column, _, _| {
                    sink.edit_requests
                        .borrow_mut()
                        .push((row.to_string(), column.to_string()));
                }
            })
            .on_edit({
                let sink = Rc::clone(&sinks);
                move |intent, _, _| {
                    sink.edits.borrow_mut().push((
                        intent.row.to_string(),
                        intent.column.to_string(),
                        intent.value.to_string(),
                        intent.outcome.as_str().to_string(),
                    ));
                }
            });
        if let Some(total) = given.total {
            element = element.total(total);
        }
        if let Some(failure) = given.failure.clone() {
            element = element.failure(failure);
        }
        element.into_any_element()
    });
    (harness, reports)
}

/// A real double click, which `simulate_click` cannot produce: it always
/// reports a click count of one.
fn double_click(harness: &mut Harness, id: &str) {
    let position = harness.point_in(id);
    harness.context().simulate_event(gpui::MouseDownEvent {
        position,
        button: gpui::MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    harness.context().simulate_event(gpui::MouseUpEvent {
        position,
        button: gpui::MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 2,
    });
    harness.context().run_until_parked();
}

fn rows_of(harness: &mut Harness) -> Vec<Node> {
    harness
        .snapshot()
        .children_of("data.grid")
        .into_iter()
        .filter(|node| node.role == Role::Row)
        .cloned()
        .collect()
}

#[gpui::test]
fn a_virtualized_grid_publishes_its_total_and_only_the_rows_it_drew(cx: &mut TestAppContext) {
    let (mut harness, _reports) = grid(cx, Given::rows(1000));

    let node = harness.node("data.grid").expect("published");
    assert_eq!(node.role, Role::Table);
    assert_eq!(
        node.value.as_deref(),
        Some("1,000"),
        "the grid must report the size of the data set it was given"
    );

    let drawn = rows_of(&mut harness);
    assert!(
        (1..24).contains(&drawn.len()),
        "a viewport of eight rows must not publish a thousand nodes, drew {}",
        drawn.len()
    );
    assert!(harness.node("data.grid.job-0000").is_some());
    assert!(
        harness.node("data.grid.job-0900").is_none(),
        "a row outside the viewport must not be addressable"
    );
    assert!(
        harness.node("data.grid.job-0000.name").is_some(),
        "a published cell is an assertion target"
    );
    assert!(
        harness.node("data.grid.job-0000.duration").is_none(),
        "an unmarked cell must not add a node to the tree"
    );
}

#[gpui::test]
fn the_keyboard_reports_a_row_the_viewport_has_never_drawn(cx: &mut TestAppContext) {
    let (mut harness, reports) = grid(cx, Given::rows(1000));

    harness.click("data.grid.job-0000");
    reports.selections.borrow_mut().clear();
    harness.keystrokes("end");

    assert_eq!(*reports.selections.borrow(), vec!["replace:job-0999"]);
    // The grid scrolled to what it reported, so the row is now on screen.
    assert!(harness.node("data.grid.job-0999").is_some());
    assert!(harness.node("data.grid.job-0000").is_none());
}

#[gpui::test]
fn a_sortable_header_reports_the_next_direction_and_sorts_nothing(cx: &mut TestAppContext) {
    let (mut harness, reports) = grid(
        cx,
        Given {
            sort: Some((SharedString::from("duration"), SortDirection::Ascending)),
            ..Given::rows(40)
        },
    );

    let header = harness
        .node("data.grid.header.duration")
        .expect("published");
    assert_eq!(header.role, Role::Button);
    assert_eq!(header.value.as_deref(), Some("ascending"));
    assert_eq!(
        harness
            .node("data.grid.header.name")
            .expect("published")
            .value
            .as_deref(),
        Some("unsorted")
    );
    assert_eq!(
        harness
            .node("data.grid.header.owner")
            .expect("published")
            .role,
        Role::Cell,
        "a header that does not sort is not a button"
    );

    let before: Vec<String> = rows_of(&mut harness)
        .into_iter()
        .map(|row| row.id.to_string())
        .collect();
    harness.click("data.grid.header.duration");

    assert_eq!(
        *reports.sorts.borrow(),
        vec![("duration".to_string(), "descending".to_string())]
    );
    let after: Vec<String> = rows_of(&mut harness)
        .into_iter()
        .map(|row| row.id.to_string())
        .collect();
    assert_eq!(before, after, "the grid renders the order it was given");
}

#[gpui::test]
fn dragging_a_column_edge_reports_a_width_and_resizes_nothing(cx: &mut TestAppContext) {
    let (mut harness, reports) = grid(cx, Given::rows(40));

    let handle = harness
        .node("data.grid.header.duration.resize")
        .expect("a resizable column publishes its handle");
    assert_eq!(handle.role, Role::Separator);
    let before = harness
        .bounds("data.grid.header.duration")
        .expect("measured");

    harness.drag_start("data.grid.header.duration.resize");
    let anchor = harness.pointer();
    harness.drag_to(anchor - gpui::point(px(20.0), px(0.0)));
    let near = reports.widths.borrow().last().cloned();
    harness.drag_to(anchor - gpui::point(px(50.0), px(0.0)));
    let far = reports.widths.borrow().last().cloned();
    harness.drop_here();

    let (key, near_width) = near.expect("a drag reports a width");
    let (_, far_width) = far.expect("a drag reports a width");
    assert_eq!(key, "duration");
    assert!(
        far_width < near_width,
        "dragging further left must ask for a narrower column, {near_width} then {far_width}"
    );

    let after = harness
        .bounds("data.grid.header.duration")
        .expect("measured");
    assert!(
        (after.size.width - before.size.width).abs() < px(0.5),
        "the caller owns the width, so nothing moved"
    );
}

#[gpui::test]
fn a_column_edge_dragged_past_its_minimum_reports_the_minimum(cx: &mut TestAppContext) {
    let (mut harness, reports) = grid(cx, Given::rows(40));

    harness.drag_start("data.grid.header.duration.resize");
    let anchor = harness.pointer();
    harness.drag_to(anchor - gpui::point(px(400.0), px(0.0)));
    harness.drop_here();

    let (_, width) = reports
        .widths
        .borrow()
        .last()
        .cloned()
        .expect("a drag reports a width");
    assert_eq!(width, 60.0, "a column stops at the minimum it declared");
}

#[gpui::test]
fn dropping_a_column_reports_where_it_should_go(cx: &mut TestAppContext) {
    let (mut harness, reports) = grid(cx, Given::rows(40));

    let before: Vec<String> = harness
        .snapshot()
        .under("data.grid.header.")
        .into_iter()
        .map(|node| node.id.to_string())
        .collect();

    harness.drag("data.grid.header.owner", "data.grid.header.duration");

    assert_eq!(*reports.orders.borrow(), vec!["owner after:duration"]);
    let after: Vec<String> = harness
        .snapshot()
        .under("data.grid.header.")
        .into_iter()
        .map(|node| node.id.to_string())
        .collect();
    assert_eq!(before, after, "the grid renders the order it was given");
}

#[gpui::test]
fn a_pinned_column_cannot_be_carried_out_of_the_left_edge(cx: &mut TestAppContext) {
    let (mut harness, reports) = grid(cx, Given::rows(40));

    harness.drag("data.grid.header.owner", "data.grid.header.name");

    assert!(
        reports.orders.borrow().is_empty(),
        "nothing may be dropped across a pinned column"
    );
}

#[gpui::test]
fn a_wide_grid_scrolls_as_one_surface_and_freezes_its_pinned_group(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w(px(520.0))
            .child(
                DataGrid::new("wide-grid", 3, |index, _, _| {
                    GridRow::new(format!("row-{index}"))
                        .cell("name", Cell::new(format!("Pinned {index}")).published(true))
                        .cell("owner", format!("Owner {index}"))
                        .cell(
                            "status",
                            Cell::new(format!("Status {index}")).published(true),
                        )
                        .cell("updated", format!("Updated {index}"))
                })
                .columns([
                    GridColumn::new("name", "Name").fixed(220.0).pinned(true),
                    GridColumn::new("owner", "Owner").fixed(260.0),
                    GridColumn::new("status", "Status").fixed(260.0),
                    GridColumn::new("updated", "Updated").fixed(260.0),
                ])
                .footer_cell("updated", "Summary")
                .visible_rows(3),
            )
            .into_any_element()
    });

    let pinned_before = harness
        .bounds("wide-grid.header.name")
        .expect("pinned header");
    let cell_before = harness
        .bounds("wide-grid.row-0.name")
        .expect("pinned body cell");
    let moving_before = harness
        .bounds("wide-grid.header.status")
        .expect("moving header");
    let moving_cell_before = harness
        .bounds("wide-grid.row-0.status")
        .expect("moving body cell");
    let summary_before = harness
        .bounds("wide-grid.summary.updated")
        .expect("moving summary cell");
    let at = harness.point_in("wide-grid");
    harness.context().simulate_event(ScrollWheelEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(-300.0), px(0.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    harness.context().run_until_parked();

    let pinned_after = harness
        .bounds("wide-grid.header.name")
        .expect("pinned header after scroll");
    let cell_after = harness
        .bounds("wide-grid.row-0.name")
        .expect("pinned body cell after scroll");
    let moving_after = harness
        .bounds("wide-grid.header.status")
        .expect("moving header after scroll");
    let moving_cell_after = harness
        .bounds("wide-grid.row-0.status")
        .expect("moving body cell after scroll");
    let summary_after = harness
        .bounds("wide-grid.summary.updated")
        .expect("moving summary cell after scroll");

    assert_eq!(pinned_after.origin.x, pinned_before.origin.x);
    assert_eq!(cell_after.origin.x, cell_before.origin.x);
    assert!(
        moving_after.origin.x < moving_before.origin.x - px(250.0),
        "the header and body share the horizontal viewport"
    );
    let shift = moving_after.origin.x - moving_before.origin.x;
    assert_eq!(
        moving_cell_after.origin.x - moving_cell_before.origin.x,
        shift
    );
    assert_eq!(summary_after.origin.x - summary_before.origin.x, shift);
}

#[gpui::test]
fn keyboard_focus_reveals_a_moving_header_beyond_the_frozen_group(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w(px(520.0))
            .child(
                DataGrid::new("focus-grid", 1, |_, _, _| {
                    GridRow::new("row")
                        .cell("name", "Pinned")
                        .cell("filler", "Middle")
                        .cell("action", "Focusable")
                })
                .columns([
                    GridColumn::new("name", "Name").fixed(220.0).pinned(true),
                    GridColumn::new("filler", "Middle").fixed(260.0),
                    GridColumn::new("action", "Action")
                        .fixed(260.0)
                        .sortable(true),
                ])
                .visible_rows(1)
                .on_sort(|_, _, _, _| {}),
            )
            .into_any_element()
    });

    let grid = harness.bounds("focus-grid").expect("grid bounds");
    let frozen = harness
        .bounds("focus-grid.header.name")
        .expect("frozen header");
    let before = harness
        .bounds("focus-grid.header.action")
        .expect("offscreen focus target");
    assert!(before.right() > grid.right());

    harness.update(|window, cx| window.focus_next(cx));

    let after = harness
        .bounds("focus-grid.header.action")
        .expect("revealed focus target");
    let frozen_after = harness
        .bounds("focus-grid.header.name")
        .expect("frozen header after reveal");
    assert_eq!(frozen_after.origin.x, frozen.origin.x);
    assert!(
        after.left() >= frozen.right(),
        "focused header {after:?} must clear frozen header {frozen:?}"
    );
    assert!(
        after.right() <= grid.right(),
        "focused header {after:?} must fit viewport {grid:?}; before was {before:?}"
    );
}

#[gpui::test]
fn a_frozen_group_holds_the_right_reading_edge_in_rtl(cx: &mut TestAppContext) {
    let mut harness = Harness::new(
        cx,
        |cx| {
            gpui_kit::install(cx);
            set_layout_direction(LayoutDirection::RightToLeft, cx);
        },
        |_, _| {
            div()
                .w(px(520.0))
                .child(
                    DataGrid::new("rtl-grid", 1, |_, _, _| {
                        GridRow::new("row")
                            .cell("name", Cell::new("Pinned").published(true))
                            .cell("owner", "Owner")
                            .cell("status", "Status")
                            .cell("updated", "Updated")
                    })
                    .columns([
                        GridColumn::new("name", "Name").fixed(220.0).pinned(true),
                        GridColumn::new("owner", "Owner").fixed(260.0),
                        GridColumn::new("status", "Status").fixed(260.0),
                        GridColumn::new("updated", "Updated").fixed(260.0),
                    ])
                    .visible_rows(1),
                )
                .into_any_element()
        },
    );
    assert_eq!(
        harness.update(|_, cx| cx.layout_direction()),
        LayoutDirection::RightToLeft
    );

    let grid = harness.bounds("rtl-grid").expect("grid bounds");
    let pinned_before = harness
        .bounds("rtl-grid.header.name")
        .expect("pinned RTL header");
    let moving_before = harness
        .bounds("rtl-grid.header.status")
        .expect("moving RTL header");
    let owner_before = harness
        .bounds("rtl-grid.header.owner")
        .expect("first moving RTL header");
    let updated_before = harness
        .bounds("rtl-grid.header.updated")
        .expect("last moving RTL header");
    assert!(
        pinned_before.right() <= grid.right(),
        "pinned {pinned_before:?}, grid {grid:?}"
    );
    assert!(
        pinned_before.right() > moving_before.right(),
        "pinned {pinned_before:?}, owner {owner_before:?}, status {moving_before:?}, updated \
         {updated_before:?}, grid {grid:?}"
    );

    let at = harness.point_in("rtl-grid");
    harness.context().simulate_event(ScrollWheelEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(300.0), px(0.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    harness.context().run_until_parked();

    let pinned_after = harness
        .bounds("rtl-grid.header.name")
        .expect("pinned RTL header after scroll");
    let moving_after = harness
        .bounds("rtl-grid.header.status")
        .expect("moving RTL header after scroll");
    assert_eq!(pinned_after.origin.x, pinned_before.origin.x);
    assert!(moving_after.origin.x > moving_before.origin.x + px(250.0));

    harness.update(|_, cx| set_layout_direction(LayoutDirection::LeftToRight, cx));
    assert_eq!(
        harness.update(|_, cx| cx.layout_direction()),
        LayoutDirection::LeftToRight
    );
    harness.advance(Duration::from_secs(1));
    let ltr = harness
        .bounds("rtl-grid.header.name")
        .expect("pinned header after LTR switch");
    assert!(ltr.left() >= grid.left(), "LTR pinned header {ltr:?}");
    assert!(
        ltr.left() < grid.center().x,
        "LTR pinned header after direction switch {ltr:?}"
    );

    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));
    harness.advance(Duration::from_secs(1));
    let rtl_again = harness
        .bounds("rtl-grid.header.name")
        .expect("pinned header after RTL switch");
    assert!(
        rtl_again.right() <= grid.right() && rtl_again.left() > grid.center().x,
        "RTL pinned header after direction switches {rtl_again:?}"
    );
}

#[gpui::test]
fn shift_clicking_reports_the_span_from_the_row_last_operated(cx: &mut TestAppContext) {
    let (mut harness, reports) = grid(cx, Given::rows(40));

    harness.click("data.grid.job-0001");
    assert_eq!(*reports.selections.borrow(), vec!["replace:job-0001"]);
    reports.selections.borrow_mut().clear();

    let target = harness.point_in("data.grid.job-0004");
    harness.context().simulate_click(
        target,
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    harness.context().run_until_parked();

    assert_eq!(
        *reports.selections.borrow(),
        vec!["range:job-0001..job-0004"]
    );
    assert!(
        !harness
            .node("data.grid.job-0004")
            .expect("published")
            .selected,
        "the caller owns the selection, so nothing moved"
    );
}

#[gpui::test]
fn a_modified_click_reports_a_toggle_rather_than_a_replacement(cx: &mut TestAppContext) {
    let (mut harness, reports) = grid(cx, Given::rows(40));

    let target = harness.point_in("data.grid.job-0002");
    harness.context().simulate_click(
        target,
        Modifiers {
            control: true,
            ..Modifiers::none()
        },
    );
    harness.context().run_until_parked();

    assert_eq!(*reports.selections.borrow(), vec!["toggle:job-0002"]);
}

#[gpui::test]
fn the_header_box_says_what_it_can_speak_for(cx: &mut TestAppContext) {
    let loaded = 40;
    let (mut harness, reports) = grid(
        cx,
        Given {
            total: Some(12_000),
            ..Given::rows(loaded)
        },
    );

    let box_node = harness.node("data.grid.select-all").expect("published");
    assert_eq!(box_node.role, Role::Checkbox);
    assert_eq!(box_node.checked, Some(false));
    assert_eq!(
        box_node.value.as_deref(),
        Some("0 of 40 loaded, 12,000 total"),
        "a box in a virtualized grid must not read as though it speaks for the whole data set"
    );

    harness.click("data.grid.select-all");
    assert_eq!(
        *reports.selections.borrow(),
        vec!["loaded"],
        "the box asks for the loaded rows and nothing wider"
    );
    reports.selections.borrow_mut().clear();
    harness.keystrokes("space");
    assert_eq!(
        *reports.selections.borrow(),
        vec!["loaded"],
        "the same loaded-only request is available from the keyboard"
    );

    // Two of the loaded rows selected is neither all nor none.
    let (mut harness, _reports) = grid(
        cx,
        Given {
            total: Some(12_000),
            selected: vec!["job-0000".into(), "job-0001".into()],
            ..Given::rows(loaded)
        },
    );
    let box_node = harness.node("data.grid.select-all").expect("published");
    assert_eq!(box_node.checked, None, "a partial selection is mixed");
    assert_eq!(
        box_node.value.as_deref(),
        Some("2 of 40 loaded, 12,000 total")
    );

    let all: Vec<SharedString> = (0..loaded).map(|index| job(index).0).collect();
    let (mut harness, reports) = grid(
        cx,
        Given {
            total: Some(12_000),
            selected: all,
            ..Given::rows(loaded)
        },
    );
    let box_node = harness.node("data.grid.select-all").expect("published");
    assert_eq!(box_node.checked, Some(true));
    assert_eq!(
        box_node.value.as_deref(),
        Some("40 of 40 loaded, 12,000 total"),
        "every loaded row is selected, and eleven thousand nine hundred and sixty are not"
    );
    harness.click("data.grid.select-all");
    assert_eq!(*reports.selections.borrow(), vec!["clear"]);
}

fn bulk_bar(cx: &mut TestAppContext, count: usize, total: usize) -> (Harness, Rc<Reports>) {
    let reports = Rc::new(Reports::default());
    let sinks = Rc::clone(&reports);
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let widen = Rc::clone(&sinks);
        let clear = Rc::clone(&sinks);
        BulkBar::new("data.bulk", count)
            .total(total)
            .action(
                Button::new("data.bulk.archive")
                    .label("Archive")
                    .secondary()
                    .on_click(|_, _| {}),
            )
            .on_select_all(move |_, _| widen.selections.borrow_mut().push("everything".into()))
            .on_dismiss(move |_, _| clear.selections.borrow_mut().push("clear".into()))
            .into_any_element()
    });
    (harness, reports)
}

#[gpui::test]
fn the_bulk_bar_states_the_selection_it_actually_has(cx: &mut TestAppContext) {
    let (mut harness, reports) = bulk_bar(cx, 40, 12_000);

    let bar = harness.node("data.bulk").expect("published");
    assert_eq!(bar.role, Role::Toolbar);
    assert_eq!(bar.value.as_deref(), Some("40"));
    assert_eq!(bar.text.as_deref(), Some("40 selected"));

    let wider = harness
        .node("data.bulk.select-all")
        .expect("a selection narrower than the data set offers the wider intent");
    assert_eq!(wider.text.as_deref(), Some("Select all 12,000"));

    harness.click("data.bulk.select-all");
    assert_eq!(*reports.selections.borrow(), vec!["everything"]);
    reports.selections.borrow_mut().clear();
    harness.keystrokes("enter");
    assert_eq!(
        *reports.selections.borrow(),
        vec!["everything"],
        "the wider selection is not pointer-only"
    );

    harness.click("data.bulk.dismiss");
    assert_eq!(*reports.selections.borrow(), vec!["everything", "clear"]);
}

#[gpui::test]
fn a_bulk_bar_over_the_whole_data_set_offers_nothing_wider(cx: &mut TestAppContext) {
    let (mut harness, _reports) = bulk_bar(cx, 12_000, 12_000);

    assert_eq!(
        harness
            .node("data.bulk")
            .expect("published")
            .text
            .as_deref(),
        Some("12,000 selected")
    );
    assert!(
        harness.node("data.bulk.select-all").is_none(),
        "there is nothing left to widen to"
    );
}

#[gpui::test]
fn an_empty_selection_shows_no_bulk_bar(cx: &mut TestAppContext) {
    let (mut harness, _reports) = bulk_bar(cx, 0, 12_000);

    assert!(harness.node("data.bulk").is_none());
}

#[gpui::test]
fn an_open_row_is_the_only_one_that_builds_a_detail(cx: &mut TestAppContext) {
    let (mut harness, reports) = grid(
        cx,
        Given {
            expanded: vec![Expanded::new("job-0002", 2)],
            ..Given::rows(40)
        },
    );

    assert!(harness.node("data.grid.job-0002.detail").is_some());
    assert!(harness.node("data.grid.job-0001.detail").is_none());
    assert_eq!(
        harness
            .node("data.grid.job-0002")
            .expect("published")
            .expanded,
        Some(true)
    );

    harness.click("data.grid.job-0001.expand");
    assert_eq!(
        *reports.expansions.borrow(),
        vec![("job-0001".into(), true)]
    );
    assert!(
        harness.node("data.grid.job-0001.detail").is_none(),
        "nothing was applied, so nothing opened"
    );
}

fn editing(cx: &mut TestAppContext, cell: EditingCell) -> (Harness, Rc<Reports>) {
    grid(
        cx,
        Given {
            editing: Some(cell),
            ..Given::rows(20)
        },
    )
}

#[gpui::test]
fn a_cell_asks_to_be_opened_and_opens_nothing_itself(cx: &mut TestAppContext) {
    let (mut harness, reports) = grid(cx, Given::rows(20));

    double_click(&mut harness, "data.grid.job-0001.owner");

    assert_eq!(
        *reports.edit_requests.borrow(),
        vec![("job-0001".to_string(), "owner".to_string())]
    );
    assert!(
        harness.node("data.grid.edit").is_none(),
        "the caller owns which cell is open, so nothing opened"
    );
}

#[gpui::test]
fn enter_commits_an_edit_once_and_writes_nothing(cx: &mut TestAppContext) {
    let (mut harness, reports) = editing(cx, EditingCell::new("job-0001", "owner", "owner-1"));

    let field = harness.node("data.grid.edit").expect("the cell is a field");
    assert_eq!(field.role, Role::Input);

    harness.keystrokes("x");
    harness.keystrokes("enter");

    let edits = reports.edits.borrow().clone();
    assert_eq!(edits.len(), 1, "an edit reports once, not once per frame");
    assert_eq!(edits[0].0, "job-0001");
    assert_eq!(edits[0].1, "owner");
    assert_eq!(edits[0].3, "commit");
    assert_eq!(
        edits[0].2, "owner-1x",
        "the value reported is what the field held"
    );
    assert_eq!(
        harness
            .node("data.grid.job-0001")
            .expect("published")
            .text
            .as_deref(),
        Some("Fixture job 0001"),
        "the grid never writes the value"
    );
}

#[gpui::test]
fn escape_reverts_an_edit_to_the_value_the_grid_was_given(cx: &mut TestAppContext) {
    let (mut harness, reports) = editing(cx, EditingCell::new("job-0001", "owner", "owner-1"));

    harness.keystrokes("z");
    harness.keystrokes("escape");

    let edits = reports.edits.borrow().clone();
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].3, "revert");
    assert_eq!(
        edits[0].2, "owner-1",
        "a revert reports the value that still holds, not the one abandoned"
    );
}

#[gpui::test]
fn tab_commits_and_names_the_cell_it_moves_to(cx: &mut TestAppContext) {
    let (mut harness, reports) = editing(cx, EditingCell::new("job-0001", "owner", "owner-1"));

    harness.keystrokes("tab");

    let edits = reports.edits.borrow().clone();
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].3, "commit");
}

#[gpui::test]
fn a_failed_refresh_keeps_the_rows_that_are_still_true(cx: &mut TestAppContext) {
    let (mut harness, _reports) = grid(
        cx,
        Given {
            failure: Some("The host refused the refresh".into()),
            ..Given::rows(40)
        },
    );

    let drawn = rows_of(&mut harness);
    assert!(
        !drawn.is_empty(),
        "a refresh that failed must not take the last verified rows away"
    );
    assert!(harness.node("data.grid.job-0000").is_some());

    let banner = harness.node("data.grid.failure").expect("published");
    assert_eq!(banner.role, Role::Status);
    assert_eq!(banner.value.as_deref(), Some("stale"));
    assert_eq!(banner.text.as_deref(), Some("The host refused the refresh"));
    assert!(banner.invalid);
    assert!(
        harness.node("data.grid.empty").is_none(),
        "a failure over real rows is not an empty state"
    );
}

#[gpui::test]
fn a_failure_with_nothing_behind_it_takes_the_surface(cx: &mut TestAppContext) {
    let (mut harness, _reports) = grid(
        cx,
        Given {
            failure: Some("The host refused the refresh".into()),
            ..Given::rows(0)
        },
    );

    let empty = harness.node("data.grid.empty").expect("published");
    assert_eq!(empty.value.as_deref(), Some("failed"));
    assert!(rows_of(&mut harness).is_empty());
}

#[gpui::test]
fn a_double_click_on_a_column_edge_asks_for_a_fit_and_measures_nothing(cx: &mut TestAppContext) {
    let (mut harness, reports) = grid(cx, Given::rows(1000));

    double_click(&mut harness, "data.grid.header.duration.resize");

    assert_eq!(
        *reports.fits.borrow(),
        vec!["duration".to_string()],
        "the grid cannot measure the rows it never drew, so it reports the request"
    );
}
