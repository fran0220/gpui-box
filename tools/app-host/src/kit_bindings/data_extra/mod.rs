//! Native data surfaces. JSON rows are retained as data; viewport callbacks
//! construct rows and cells, and slot factories are never consumed ahead of time.
use super::*;
use gpui::{ParentElement, div};
use gpui_kit::foundation::slot::Slotted;
use gpui_kit::layout::Breakpoint;
use gpui_kit::state::Loadable;
use gpui_kit_theme::Space;

pub(super) const COMPONENTS: &[&str] = &[
    "BulkBar",
    "DataGrid",
    "DiagnosticsList",
    "Flow",
    "ImageList",
    "KanbanBoard",
    "Masonry",
    "Table",
    "Tree",
    "TreeGrid",
];

pub(super) fn validate_props(node: &Node) -> anyhow::Result<()> {
    use anyhow::ensure;
    let p = Value::Object(node.props.clone());
    let component = node.component.as_deref().unwrap_or_default();
    if component == "Tree" {
        fn visit(
            nodes: &[Value],
            ids: &mut std::collections::HashSet<String>,
        ) -> anyhow::Result<()> {
            for v in nodes {
                ensure!(ids.insert(string(v, "id")), "duplicate tree identity");
                visit(&values(v, "children"), ids)?;
            }
            Ok(())
        }
        visit(&values(&p, "nodes"), &mut Default::default())?;
    }
    if matches!(component, "DataGrid" | "TreeGrid" | "Table") {
        let columns = values(&p, "columns")
            .into_iter()
            .map(|v| string(&v, "id"))
            .collect::<std::collections::HashSet<_>>();
        let names = values(&p, "slotNames")
            .into_iter()
            .map(|v| string(&v, "id"))
            .collect::<std::collections::HashSet<_>>();
        let mut rows = std::collections::HashSet::new();
        for row in values(&p, "rows") {
            if component == "TreeGrid" && row["parent"].is_string() {
                ensure!(
                    rows.contains(&string(&row, "parent")),
                    "treegrid parent must precede child"
                );
            }
            rows.insert(string(&row, "id"));
            for cell in values(&row, "cells") {
                ensure!(
                    columns.contains(&string(&cell, "id")),
                    "unknown cell column"
                );
                if cell["slot"].is_string() {
                    ensure!(
                        names.contains(&string(&cell, "slot")),
                        "undeclared cell slot"
                    );
                }
            }
        }
        for v in values(&p, "expanded") {
            ensure!(
                p["rows"]
                    .get(v["index"].as_u64().unwrap_or(u64::MAX) as usize)
                    .is_some_and(|r| r["id"] == v["id"]),
                "expanded identity/index mismatch"
            );
        }
        for v in values(&p, "columns") {
            ensure!(
                !(v["fixed"].is_number() && v["flex"].is_number()),
                "column width is either fixed or flex"
            );
        }
    }
    Ok(())
}

fn string(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_owned()
}
fn values(v: &Value, key: &str) -> Vec<Value> {
    v[key].as_array().cloned().unwrap_or_default()
}
fn strings(v: &Value) -> Vec<SharedString> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|s| s.to_owned().into())
        .collect()
}
fn slotted<T: Slotted>(mut control: T, slots: &KitSlots) -> T {
    for &name in T::SLOTS {
        if let Some(factory) = slots.get(name).cloned() {
            control = control.slot(name, move |window, cx| factory(window, cx));
        }
    }
    control
}
fn content(
    slots: &KitSlots,
    name: &str,
    label: String,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    slots
        .get(name)
        .map_or_else(|| div().child(label).into_any_element(), |f| f(window, cx))
}
fn empty(node: &Node, slots: &KitSlots, window: &mut Window, cx: &mut App) -> EmptyState {
    let v = node.props.get("empty").unwrap_or(&Value::Null);
    let mut c = EmptyState::new(format!("{}.empty", node.id), string(v, "title")).kind(
        match v["kind"].as_str() {
            Some("unstarted") => EmptyKind::Unstarted,
            Some("queued") => EmptyKind::Queued,
            Some("blocked") => EmptyKind::Blocked,
            Some("cancelled") => EmptyKind::Cancelled,
            Some("unavailable") => EmptyKind::Unavailable,
            Some("failed") => EmptyKind::Failed,
            Some("unauthorized") => EmptyKind::Unauthorized,
            _ => EmptyKind::Empty,
        },
    );
    if v["detail"].is_string() {
        c = c.detail(string(v, "detail"));
    }
    if v["icon"].is_object() {
        c = c.icon(super::icon::resolve(&v["icon"]).expect("validated built-in icon"));
    }
    if let Some(f) = slots.get("empty_action") {
        c = c.action(f(window, cx));
    }
    c
}
fn cell(value: &Value, slots: &KitSlots, window: &mut Window, cx: &mut App) -> Cell {
    let label = string(value, "text");
    Cell::new(content(
        slots,
        &string(value, "slot"),
        label.clone(),
        window,
        cx,
    ))
    .text(label)
    .published(value["published"].as_bool().unwrap_or(true))
}
fn align(value: &Value) -> Align {
    match value.as_str() {
        Some("center") => Align::Center,
        Some("end") => Align::End,
        _ => Align::Start,
    }
}
fn lines(node: &Node) -> GridLines {
    match text(node, "lines").as_str() {
        "rows" => GridLines::Rows,
        _ => GridLines::None,
    }
}
fn direction(value: &Value) -> SortDirection {
    if value.as_str() == Some("descending") {
        SortDirection::Descending
    } else {
        SortDirection::Ascending
    }
}
fn severity(v: &Value) -> DiagnosticSeverity {
    match v.as_str() {
        Some("error") => DiagnosticSeverity::Error,
        Some("warning") => DiagnosticSeverity::Warning,
        Some("hint") => DiagnosticSeverity::Hint,
        _ => DiagnosticSeverity::Information,
    }
}
fn breakpoint(v: &Value) -> Breakpoint {
    match v.as_str() {
        Some("sm") => Breakpoint::Small,
        Some("md") => Breakpoint::Medium,
        Some("lg") => Breakpoint::Large,
        _ => Breakpoint::ExtraLarge,
    }
}
fn space(v: &Value) -> Space {
    match v.as_str() {
        Some("xxs") => Space::Xxs,
        Some("xs") => Space::Xs,
        Some("sm") => Space::Sm,
        Some("lg") => Space::Lg,
        Some("xl") => Space::Xl,
        Some("xxl") => Space::Xxl,
        _ => Space::Md,
    }
}
fn column(v: &Value) -> GridColumn {
    let mut c = GridColumn::new(string(v, "id"), string(v, "header"))
        .align(align(&v["align"]))
        .sortable(v["sortable"].as_bool().unwrap_or(false))
        .resizable(v["resizable"].as_bool().unwrap_or(false))
        .reorderable(v["reorderable"].as_bool().unwrap_or(false))
        .pinned(v["pinned"].as_bool().unwrap_or(false))
        .editable(v["editable"].as_bool().unwrap_or(false));
    if let Some(n) = v["fixed"].as_f64() {
        c = c.fixed(n as f32);
    }
    if let Some(n) = v["flex"].as_f64() {
        c = c.flex(n as f32);
    }
    if let Some(n) = v["minWidth"].as_f64() {
        c = c.min_width(n as f32);
    }
    c
}
fn grid_row(v: Value, slots: Rc<KitSlots>) -> GridRow {
    let cells: BTreeMap<String, Value> = values(&v, "cells")
        .into_iter()
        .map(|c| (string(&c, "id"), c))
        .collect();
    GridRow::new(string(&v, "id"))
        .text(string(&v, "label"))
        .disabled(v["disabled"].as_bool().unwrap_or(false))
        .cells_with(move |key, window, cx| {
            cell(
                cells.get(key.as_ref()).unwrap_or(&Value::Null),
                &slots,
                window,
                cx,
            )
        })
}
fn range(v: &Value) -> CellRange {
    CellRange::new(
        string(v, "startRow"),
        string(v, "startColumn"),
        string(v, "endRow"),
        string(v, "endColumn"),
    )
}
fn range_value(v: &CellRange) -> Value {
    json!({"startRow":v.start_row.as_ref(),"startColumn":v.start_column.as_ref(),"endRow":v.end_row.as_ref(),"endColumn":v.end_column.as_ref()})
}
pub(super) fn drop_value(v: &DropIntent) -> Value {
    let mut value = super::drag_item_payload(&v.item);
    value["anchor"] = json!(v.position.anchor().as_ref());
    value["position"] = json!(v.position.verb());
    value["velocity"] = json!({"x":v.velocity.x,"y":v.velocity.y});
    value
}
fn selection(v: &SelectionChange) -> Value {
    match v {
        SelectionChange::Replace(id) => json!({"kind":"replace","id":id.as_ref()}),
        SelectionChange::Toggle(id) => json!({"kind":"toggle","id":id.as_ref()}),
        SelectionChange::Range { anchor, to } => {
            json!({"kind":"range","anchor":anchor.as_ref(),"to":to.as_ref()})
        }
        SelectionChange::Loaded => json!({"kind":"loaded"}),
        SelectionChange::Everything => json!({"kind":"everything"}),
        SelectionChange::Clear => json!({"kind":"clear"}),
    }
}
fn tree_node(v: &Value) -> TreeNode {
    let mut node = TreeNode::new(string(v, "id"), string(v, "label"))
        .disabled(v["disabled"].as_bool().unwrap_or(false))
        .branch(match v["branch"].as_str() {
            Some("loading") => BranchState::Loading,
            Some("unavailable") => BranchState::Unavailable(string(v, "reason").into()),
            Some("failed") => BranchState::Failed(string(v, "reason").into()),
            _ => BranchState::Ready,
        })
        .children(values(v, "children").iter().map(tree_node));
    if v["icon"].is_object() {
        node = node.icon(super::icon::resolve(&v["icon"]).expect("validated built-in icon"));
    }
    node
}

pub(super) fn render(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
) -> AnyElement {
    render_deferred(node, slots, window, cx, emit, None)
}

pub(super) fn render_deferred(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
    deferred: Option<(gpui_kit::interaction::dnd::DeferredDrop, u64)>,
) -> AnyElement {
    let p = Value::Object(node.props.clone());
    let event = |name: &str| {
        if flag(node, "disabled") {
            None
        } else {
            node.events.get(name).cloned()
        }
    };
    match node.component.as_deref().unwrap_or_default() {
        "TreeGrid" => {
            let rows = values(&p, "rows");
            let row_slots = Rc::new(slots.clone());
            let mut c = TreeGrid::new(node.id.clone(), rows.len(), move |i, _, _| {
                let v = &rows[i];
                let cells: BTreeMap<String, Value> = values(v, "cells")
                    .into_iter()
                    .map(|v| (string(&v, "id"), v))
                    .collect();
                let slots = row_slots.clone();
                let mut r =
                    TreeGridRow::new(string(v, "id"), v["level"].as_u64().unwrap_or(1) as u32)
                        .text(string(v, "label"))
                        .disabled(v["disabled"].as_bool().unwrap_or(false))
                        .cells_with(move |key, w, cx| {
                            cell(
                                cells.get(key.as_ref()).unwrap_or(&Value::Null),
                                &slots,
                                w,
                                cx,
                            )
                        });
                if v["hasChildren"].as_bool().unwrap_or(false) {
                    r = r.branch(v["expanded"].as_bool().unwrap_or(false));
                }
                if v["parent"].is_string() {
                    r = r.parent(string(v, "parent"));
                }
                r
            })
            .columns(values(&p, "columns").iter().map(column))
            .lines(lines(node))
            .disabled(flag(node, "disabled"))
            .loading(flag(node, "loading"));
            if p["empty"].is_object() {
                c = c.empty(empty(node, &slots, window, cx));
            }
            if node.props.contains_key("selected") {
                c = c.selected(text(node, "selected"));
            }
            if node.props.contains_key("failure") {
                c = c.failure(text(node, "failure"));
            }
            if node.props.contains_key("unavailable") {
                c = c.unavailable(text(node, "unavailable"));
            }
            if node.props.contains_key("visibleRows") {
                c = c.visible_rows(number(node, "visibleRows", 10.) as usize);
            }
            if node.props.contains_key("rowHeight") {
                c = c.row_height(number(node, "rowHeight", 32.));
            }
            if let Some(a) = event("select") {
                let e = emit.clone();
                c = c.on_select(move |id, _, _| e(&a, json!(id.as_ref())));
            }
            if let Some(a) = event("expand") {
                c = c.on_expand(move |id, expanded, _, _| {
                    emit(&a, json!({"id":id.as_ref(),"expanded":expanded}))
                });
            }
            slotted(c, &slots).into_any_element()
        }
        "DiagnosticsList" => {
            let diagnostics = values(&p, "diagnostics")
                .iter()
                .map(|v| {
                    Diagnostic::new(
                        string(v, "id"),
                        severity(&v["severity"]),
                        DiagnosticLocation::new(string(v, "location")),
                        string(v, "message"),
                    )
                    .disabled(v["disabled"].as_bool().unwrap_or(false))
                    .actions(values(v, "actions").iter().map(|a| {
                        DiagnosticAction::new(string(a, "id"), string(a, "label"))
                            .disabled(a["disabled"].as_bool().unwrap_or(false))
                    }))
                })
                .collect();
            let state = match p["state"].as_str() {
                Some("idle") => Loadable::Idle,
                Some("loading") => Loadable::Loading,
                Some("empty") => Loadable::Empty,
                Some("unavailable") => Loadable::Unavailable(text(node, "reason")),
                Some("error") => Loadable::Error(text(node, "reason").into()),
                _ => Loadable::Ready(diagnostics),
            };
            let mut c = DiagnosticsList::new(node.id.clone(), state)
                .disabled(flag(node, "disabled"))
                .control_size(size(node));
            if node.props.contains_key("filter") {
                c = c.filter(DiagnosticFilter::from_severities(
                    values(&p, "filter").iter().map(severity),
                ));
            }
            if node.props.contains_key("selected") {
                c = c.selected(text(node, "selected"));
            }
            if node.props.contains_key("visibleRows") {
                c = c.visible_rows(number(node, "visibleRows", 10.) as usize);
            }
            if let Some(a) = event("filter") {
                let e = emit.clone();
                c = c.on_filter(move |v, _, _| {
                    e(
                        &a,
                        json!(v.severities().map(|v| v.name()).collect::<Vec<_>>()),
                    )
                });
            }
            if let Some(a) = event("select") {
                let e = emit.clone();
                c = c.on_select(move |id, _, _| e(&a, json!(id.as_ref())));
            }
            if let Some(a) = event("action") {
                let e = emit.clone();
                c = c.on_action(move |id, action, _, _| {
                    e(&a, json!({"id":id.as_ref(),"action":action.as_ref()}))
                });
            }
            if let Some(a) = event("retry") {
                c = c.on_retry(move |_, _| emit(&a, Value::Null));
            }
            slotted(c, &slots).into_any_element()
        }
        "KanbanBoard" => {
            let mut c = KanbanBoard::new(node.id.clone())
                .disabled(flag(node, "disabled"))
                .columns(values(&p, "columns").iter().map(|v| {
                    let mut c = KanbanColumn::new(string(v, "id"), string(v, "title"));
                    if let Some(n) = v["limit"].as_u64() {
                        c = c.limit(n as usize);
                    }
                    c
                }))
                .cards(values(&p, "cards").iter().map(|v| {
                    KanbanCard::new(string(v, "id"), string(v, "title"), string(v, "column"))
                        .detail(string(v, "detail"))
                }))
                .state(match p["state"].as_str() {
                    Some("loading") => KanbanState::Loading,
                    Some("empty") => KanbanState::Empty,
                    Some("unavailable") => KanbanState::Unavailable(text(node, "reason").into()),
                    Some("error") => KanbanState::Error(text(node, "reason").into()),
                    _ => KanbanState::Ready,
                });
            if node.props.contains_key("held") {
                c = c.held(text(node, "held"));
            }
            if let Some(a) = event("card") {
                let e = emit.clone();
                c=c.on_card(move |v,_,_|e(&a,json!({"id":v.id.as_ref(),"title":v.title.as_ref(),"detail":v.detail.as_ref(),"column":v.column.as_ref()})));
            }
            if let Some(a) = event("move") {
                let e = emit.clone();
                c=c.on_move(move |v,col,_,_|e(&a,json!({"card":{"id":v.id.as_ref(),"title":v.title.as_ref(),"detail":v.detail.as_ref(),"column":v.column.as_ref()},"column":col.as_ref()})));
            }
            if let Some(a) = event("add") {
                c = c.on_add(move |id, _, _| emit(&a, json!(id.as_ref())));
            }
            slotted(c, &slots).into_any_element()
        }
        "ImageList" => {
            let mut c = ImageList::new(node.id.clone())
                .disabled(flag(node, "disabled"))
                .items(values(&p, "items").iter().map(|v| {
                    ImageListItem::new(
                        string(v, "id"),
                        string(v, "label"),
                        content(&slots, &string(v, "id"), String::new(), window, cx),
                    )
                    .disabled(v["disabled"].as_bool().unwrap_or(false))
                }));
            if node.props.contains_key("columns") {
                c = c.columns(number(node, "columns", 1.) as usize);
            }
            if node.props.contains_key("gap") {
                c = c.gap(space(&p["gap"]));
            }
            for v in values(&p, "columnsAt") {
                c = c.columns_at(
                    breakpoint(&v["breakpoint"]),
                    v["columns"].as_u64().unwrap_or(1) as usize,
                );
            }
            if node.props.contains_key("selected") {
                c = c.selected(text(node, "selected"));
            }
            if let Some(a) = event("select") {
                c = c.on_select(move |id, _, _| emit(&a, json!(id.as_ref())));
            }
            c.into_any_element()
        }
        "Masonry" => {
            let mut c = Masonry::new(node.id.clone()).items(values(&p, "items").iter().map(|v| {
                MasonryItem::new(
                    string(v, "id"),
                    content(&slots, &string(v, "id"), String::new(), window, cx),
                    v["height"].as_f64().unwrap_or(32.) as f32,
                )
            }));
            if node.props.contains_key("columns") {
                c = c.columns(number(node, "columns", 1.) as usize);
            }
            if node.props.contains_key("gap") {
                c = c.gap(space(&p["gap"]));
            }
            for v in values(&p, "columnsAt") {
                c = c.columns_at(
                    breakpoint(&v["breakpoint"]),
                    v["columns"].as_u64().unwrap_or(1) as usize,
                );
            }
            c.into_any_element()
        }
        "BulkBar" => {
            let mut c = BulkBar::new(node.id.clone(), number(node, "count", 0.) as usize);
            if node.props.contains_key("total") {
                c = c.total(number(node, "total", 0.) as usize);
            }
            if node.props.contains_key("noun") {
                c = c.noun(text(node, "noun"));
            }
            if let Some(f) = slots.get("actions") {
                c = c.action(f(window, cx));
            }
            if let Some(a) = event("selectAll") {
                let e = emit.clone();
                c = c.on_select_all(move |_, _| e(&a, Value::Null));
            }
            if let Some(a) = event("dismiss") {
                c = c.on_dismiss(move |_, _| emit(&a, Value::Null));
            }
            c.into_any_element()
        }
        "Flow" => {
            let rows = values(&p, "rows");
            let keys = rows.iter().map(|v| string(v, "id")).collect::<Vec<_>>();
            let revisions = rows
                .iter()
                .map(|v| v["revision"].as_u64().unwrap_or(0))
                .collect();
            let mut c = Flow::new(node.id.clone(), rows.len(), move |i, w, cx| {
                content(
                    &slots,
                    &string(&rows[i], "id"),
                    string(&rows[i], "label"),
                    w,
                    cx,
                )
            })
            .keys(keys)
            .revisions(revisions);
            if node.props.contains_key("estimate") {
                c = c.estimate(number(node, "estimate", 32.));
            }
            if node.props.contains_key("visibleRows") {
                c = c.visible_rows(number(node, "visibleRows", 10.) as usize);
            }
            if flag(node, "anchoredToEnd") {
                c = c.anchored_to_end();
            }
            if flag(node, "fills") {
                c = c.fills();
            }
            if node.props.contains_key("inset") {
                c = c.content_inset(
                    p["inset"]["top"].as_f64().unwrap_or(0.) as f32,
                    p["inset"]["bottom"].as_f64().unwrap_or(0.) as f32,
                );
            }
            c.into_any_element()
        }
        "DataGrid" => {
            let rows = values(&p, "rows");
            let row_slots = Rc::new(slots.clone());
            let mut c = DataGrid::new(node.id.clone(), rows.len(), move |i, _, _| {
                grid_row(rows[i].clone(), row_slots.clone())
            })
            .columns(values(&p, "columns").iter().map(column))
            .lines(lines(node))
            .disabled(flag(node, "disabled"))
            .control_size(size(node))
            .loading(flag(node, "loading"))
            .selected(strings(&p["selected"]))
            .selection_mode(match p["selectionMode"].as_str() {
                Some("single") => SelectionMode::Single,
                Some("multiple") => SelectionMode::Multiple,
                _ => SelectionMode::None,
            });
            if p["empty"].is_object() {
                c = c.empty(empty(node, &slots, window, cx));
            }
            if node.props.contains_key("total") {
                c = c.total(number(node, "total", 0.) as usize);
            }
            if node.props.contains_key("failure") {
                c = c.failure(text(node, "failure"));
            }
            if node.props.contains_key("visibleRows") {
                c = c.visible_rows(number(node, "visibleRows", 10.) as usize);
            }
            if node.props.contains_key("rowHeight") {
                c = c.row_height(number(node, "rowHeight", 32.));
            }
            if p["sort"].is_object() {
                c = c.sorted_by(
                    string(&p["sort"], "column"),
                    direction(&p["sort"]["direction"]),
                );
            }
            c = c.groups(values(&p, "groups").iter().map(|v| {
                ColumnGroup::new(string(v, "id"), string(v, "label"))
                    .columns(strings(&v["columns"]))
            }));
            for v in values(&p, "footer") {
                c = c.footer_cell(string(&v, "id"), string(&v, "text"));
            }
            c = c.expanded(values(&p, "expanded").iter().map(|v| {
                Expanded::new(string(v, "id"), v["index"].as_u64().unwrap_or(0) as usize)
            }));
            if node.props.contains_key("detailRows") {
                c = c.detail_rows(number(node, "detailRows", 1.) as usize);
            }
            let detail_slots = slots.clone();
            c = c.detail(move |id, w, cx| {
                content(&detail_slots, &format!("detail:{id}"), String::new(), w, cx)
            });
            if p["editing"].is_object() {
                let v = &p["editing"];
                c = c.editing(Some(EditingCell::new(
                    string(v, "row"),
                    string(v, "column"),
                    string(v, "value"),
                )));
            }
            if p["range"].is_object() {
                c = c.range(Some(range(&p["range"])));
            }
            if p["scrollToCell"].is_object() {
                c = c.scroll_to_cell(
                    p["scrollToCell"]["row"].as_u64().unwrap_or(0) as usize,
                    string(&p["scrollToCell"], "column"),
                );
            }
            if let Some(a) = event("sort") {
                let e = emit.clone();
                c = c.on_sort(move |id, d, _, _| {
                    e(&a, json!({"column":id.as_ref(),"direction":d.as_str()}))
                });
            }
            if let Some(a) = event("select") {
                let e = emit.clone();
                c = c.on_select(move |v, _, _| e(&a, selection(v)));
            }
            if let Some(a) = event("resize") {
                let e = emit.clone();
                c = c.on_resize(move |id, width, _, _| {
                    e(&a, json!({"column":id.as_ref(),"width":width}))
                });
            }
            if let Some(a) = event("fit") {
                let e = emit.clone();
                c = c.on_fit(move |id, _, _| e(&a, json!(id.as_ref())));
            }
            if let Some(a) = event("expand") {
                let e = emit.clone();
                c = c.on_expand(move |id, expanded, _, _| {
                    e(&a, json!({"id":id.as_ref(),"expanded":expanded}))
                });
            }
            if let Some(a) = event("rangeChange") {
                let e = emit.clone();
                c = c.on_range_change(move |r, _, _| e(&a, range_value(r)));
            }
            if let Some(a) = event("copy") {
                let e = emit.clone();
                c = c.on_copy(move |v, _, _| e(&a, json!(v.as_ref())));
            }
            if let Some(a) = event("editRequest") {
                let e = emit.clone();
                c = c.on_edit_request(move |r, col, _, _| {
                    e(&a, json!({"row":r.as_ref(),"column":col.as_ref()}))
                });
            }
            if let Some(a) = event("edit") {
                let e = emit.clone();
                c=c.on_edit(move |v,_,_|e(&a,json!({"row":v.row.as_ref(),"column":v.column.as_ref(),"value":v.value.as_ref(),"outcome":v.outcome.as_str(),"next":v.next.as_ref().map(|(r,c)|json!({"row":r.as_ref(),"column":c.as_ref()}))})));
            }
            if let Some(a) = event("reorder") {
                c = c.on_reorder(move |v, _, _| emit(&a, drop_value(v)));
            }
            slotted(c, &slots).into_any_element()
        }
        "Table" => {
            let rows = values(&p, "rows");
            let row_slots = slots.clone();
            let mut c = Table::new(node.id.clone())
                .rows_from(rows.len(), move |i, w, cx| {
                    let v = &rows[i];
                    let mut r = Row::new(string(v, "id"))
                        .text(string(v, "label"))
                        .disabled(v["disabled"].as_bool().unwrap_or(false));
                    for v in values(v, "cells") {
                        r = r.cell(string(&v, "id"), cell(&v, &row_slots, w, cx));
                    }
                    r
                })
                .columns(values(&p, "columns").iter().map(|v| {
                    let mut c = Column::new(string(v, "id"), string(v, "header"))
                        .align(align(&v["align"]))
                        .sortable(v["sortable"].as_bool().unwrap_or(false));
                    if let Some(n) = v["fixed"].as_f64() {
                        c = c.fixed(n as f32);
                    }
                    if let Some(n) = v["flex"].as_f64() {
                        c = c.flex(n as f32);
                    }
                    c
                }))
                .lines(lines(node))
                .disabled(flag(node, "disabled"))
                .loading(flag(node, "loading"))
                .control_size(size(node));
            if p["empty"].is_object() {
                c = c.empty(empty(node, &slots, window, cx));
            }
            if node.props.contains_key("selected") {
                c = c.selected(text(node, "selected"));
            }
            if node.props.contains_key("failure") {
                c = c.failure(text(node, "failure"));
            }
            if node.props.contains_key("visibleRows") {
                c = c.visible_rows(number(node, "visibleRows", 10.) as usize);
            }
            if node.props.contains_key("rowHeight") {
                c = c.row_height(number(node, "rowHeight", 32.));
            }
            if p["sort"].is_object() {
                c = c.sorted_by(
                    string(&p["sort"], "column"),
                    direction(&p["sort"]["direction"]),
                );
            }
            if let Some(a) = event("sort") {
                let e = emit.clone();
                c = c.on_sort(move |id, d, _, _| {
                    e(&a, json!({"column":id.as_ref(),"direction":d.as_str()}))
                });
            }
            if let Some(a) = event("select") {
                c = c.on_select(move |id, _, _| emit(&a, json!(id.as_ref())));
            }
            slotted(c, &slots).into_any_element()
        }
        "Tree" => {
            let mut c = Tree::new(node.id.clone())
                .nodes(values(&p, "nodes").iter().map(tree_node))
                .disabled(flag(node, "disabled"))
                .loading(flag(node, "loading"))
                .reorderable(flag(node, "reorderable"))
                .control_size(size(node));
            if p["empty"].is_object() {
                c = c.empty(empty(node, &slots, window, cx));
            }
            if node.props.contains_key("expanded") {
                c = c.expanded(strings(&p["expanded"]));
            }
            if node.props.contains_key("selected") {
                c = c.selected(text(node, "selected"));
            }
            if node.props.contains_key("failure") {
                c = c.failure(text(node, "failure"));
            }
            if node.props.contains_key("visibleRows") {
                c = c.visible_rows(number(node, "visibleRows", 10.) as usize);
            }
            if let Some(a) = event("toggle") {
                let e = emit.clone();
                c = c.on_toggle(move |id, expanded, _, _| {
                    e(&a, json!({"id":id.as_ref(),"expanded":expanded}))
                });
            }
            if let Some(a) = event("select") {
                let e = emit.clone();
                c = c.on_select(move |id, _, _| e(&a, json!(id.as_ref())));
            }
            if let Some(a) = event("move") {
                c = c.on_move(move |v, _, _| emit(&a, drop_value(v)));
            }
            if let Some((controller, revision)) = deferred {
                c = c.deferred_acceptance(controller, revision);
            }
            slotted(c, &slots).into_any_element()
        }
        _ => unreachable!("family registration validated"),
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests;
