//! Rows, columns, and hierarchies of caller-owned records.

use super::support::*;

pub(super) fn diagnostics_list(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let diagnostics = [
        Diagnostic::new(
            "fixture-error",
            DiagnosticSeverity::Error,
            DiagnosticLocation::new("fixture.rs:18"),
            "Fixture diagnostic: incompatible value",
        )
        .action(DiagnosticAction::new("inspect", "Inspect fixture")),
        Diagnostic::new(
            "fixture-warning",
            DiagnosticSeverity::Warning,
            DiagnosticLocation::new("fixture.rs:31"),
            "Fixture diagnostic: unused declaration",
        ),
        Diagnostic::new(
            "fixture-information",
            DiagnosticSeverity::Information,
            DiagnosticLocation::new("fixture.rs:47"),
            "Fixture diagnostic: a simpler form is available",
        ),
        Diagnostic::new(
            "fixture-hint",
            DiagnosticSeverity::Hint,
            DiagnosticLocation::new("fixture.rs:64"),
            "Fixture diagnostic: consider a descriptive name",
        ),
    ];
    stack(&theme)
        .w(px(760.0))
        .child(caption(
            &theme,
            "synthetic diagnostics; filters, selection, and actions are caller-owned intents",
        ))
        .child(
            DiagnosticsList::new("scene.diagnostics", Loadable::Ready(diagnostics.to_vec()))
                .selected("fixture-warning")
                .visible_rows(5)
                .on_filter(|_, _, _| {})
                .on_select(|_, _, _| {})
                .on_action(|_, _, _, _| {}),
        )
        .into_any_element()
}

/// How many records the list fixture claims to hold.
///
/// The count exists to make the difference visible: the list publishes all of
/// it, and renders only the handful the viewport can show.
pub(super) const FIXTURE_RECORDS: usize = 240;

/// One synthetic record. Nothing here stands for a product: the identity is a
/// fixture key and the label says so.
pub(super) fn fixture_record(index: usize) -> (SharedString, SharedString) {
    (
        SharedString::from(format!("record-{index:04}")),
        SharedString::from(format!("Fixture record {index:04}")),
    )
}

pub(super) fn list(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(420.0))
        .child(
            crate::foundation::text(
                &theme,
                TypeScale::Caption,
                SharedString::from(format!(
                    "{FIXTURE_RECORDS} fixture records; only the rendered ones publish"
                )),
            )
            .text_tone(&theme, TextTone::Muted),
        )
        .child(
            div()
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .py_token(&theme, Space::Sm)
                .overflow_hidden()
                .child(
                    List::new("scene.list.records", FIXTURE_RECORDS, |index, _, _| {
                        let (id, label) = fixture_record(index);
                        ListItem::new(id, label.clone()).text(label)
                    })
                    .selected(fixture_record(2).0)
                    .visible_rows(8)
                    .on_select(|_, _, _| {}),
                ),
        )
        .into_any_element()
}

/// What one flow row holds: a label, and prose whose length decides how tall
/// the row turns out to be.
fn fixture_entry(index: usize) -> (SharedString, SharedString) {
    let body = match index % 3 {
        0 => "A short entry.".to_string(),
        1 => "An entry long enough to wrap onto a second line, which is what \
              makes this row taller than the one above it."
            .to_string(),
        _ => "An entry longer still. Nothing here is a slot: the row is as \
              tall as the prose it holds, and the flow measures it once, when \
              it first comes into view, then keeps that measurement while the \
              rows around it change."
            .to_string(),
    };
    (
        SharedString::from(format!("entry-{index:04}")),
        SharedString::from(body),
    )
}

const FLOW_VISIBLE_ROWS: usize = 5;

pub(super) fn flow(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .child(caption(
            &theme,
            "rows drawn by the caller, laid out only where the viewport reaches; \
             a row is as tall as what it holds",
        ))
        .child(
            div()
                .column()
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    // A row is as tall as its prose, so the viewport's edge
                    // falls wherever it falls — through a line of text as
                    // often as between two. The fade is what turns that from
                    // a glyph sliced in half into a statement that the entries
                    // carry on past the edge.
                    ScrollFade::new("scene.flow.edge")
                        .bottom(true)
                        .fit_height()
                        .child({
                            let theme = theme.clone();
                            div()
                                .w_full()
                                .child(
                                    Flow::new(
                                        "scene.flow.entries",
                                        FIXTURE_RECORDS,
                                        move |index, _, cx| {
                                            let (id, body) = fixture_entry(index);
                                            // A flow publishes nothing of its own, so the rows say
                                            // what they are — which is the arrangement any caller
                                            // drawing its own rows ends up with.
                                            div()
                                                .w_full()
                                                .column()
                                                .gap_token(&theme, Space::Xs)
                                                .px_token(&theme, Space::Md)
                                                .py_token(&theme, Space::Sm)
                                                .child(
                                                    crate::foundation::text(
                                                        &theme,
                                                        TypeScale::Caption,
                                                        id.clone(),
                                                    )
                                                    .text_tone(&theme, TextTone::Muted),
                                                )
                                                .child(crate::foundation::text(
                                                    &theme,
                                                    TypeScale::Body,
                                                    body.clone(),
                                                ))
                                                .semantic_in(
                                                    cx,
                                                    NodeSpec::new(
                                                        format!("scene.flow.entries.{id}"),
                                                        Role::Row,
                                                    )
                                                    .parent("scene.flow.entries")
                                                    .text(body),
                                                )
                                                .into_any_element()
                                        },
                                    )
                                    .estimate(72.0)
                                    .visible_rows(FLOW_VISIBLE_ROWS),
                                )
                                .semantic_in(
                                    cx,
                                    NodeSpec::new("scene.flow.entries", Role::List)
                                        .value(FIXTURE_RECORDS.to_string()),
                                )
                        }),
                )
                .child(rule(&theme))
                // How much is past the edge, and something to do about it. A
                // count on its own states the overflow and leaves the reader
                // to find the rest by dragging.
                .child(
                    div()
                        .row()
                        .w_full()
                        .gap_token(&theme, Space::Sm)
                        .px_token(&theme, Space::Md)
                        .py_token(&theme, Space::Xs)
                        .child(caption(
                            &theme,
                            SharedString::from(format!(
                                "{} more below",
                                FIXTURE_RECORDS - FLOW_VISIBLE_ROWS
                            )),
                        ))
                        .child(div().flex_1())
                        .child(
                            Button::new("scene.flow.end")
                                .label("Go to the end")
                                .ghost()
                                .small()
                                .on_click(|window, cx| {
                                    crate::data::glide_to_row(
                                        &Ident::from("scene.flow.entries"),
                                        FIXTURE_RECORDS - 1,
                                        window,
                                        cx,
                                    );
                                }),
                        ),
                ),
        )
        .into_any_element()
}

pub(super) fn table(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let state = |label: &'static str, tone: Tone| {
        Cell::new(Badge::new(label).tone(tone))
            .text(label)
            .published(true)
    };
    let columns = [
        Column::new("name", "Run").flex(2.0).sortable(true),
        Column::new("state", "State").fixed(110.0),
        Column::new("duration", "Duration")
            .fixed(96.0)
            .align(Align::End)
            .sortable(true),
    ];
    stack(&theme)
        .w(px(600.0))
        .child(
            Table::new("scene.table.runs")
                .columns(columns.clone())
                .sorted_by("duration", SortDirection::Descending)
                .selected("run-b12")
                .rows([
                    Row::new("run-a04")
                        .text("Indexing")
                        .cell("name", "Indexing")
                        .cell("state", state("Ready", Tone::Success))
                        .cell("duration", "4m 12s"),
                    Row::new("run-b12")
                        .text("Verifying")
                        .cell("name", "Verifying")
                        .cell("state", state("Stale", Tone::Warning))
                        .cell("duration", "2m 08s"),
                    Row::new("run-c31")
                        .text("Publishing")
                        .cell("name", "Publishing")
                        .cell("state", state("Refused", Tone::Danger))
                        .cell("duration", "1m 44s"),
                    Row::new("run-d02")
                        .text("Archiving")
                        .disabled(true)
                        .cell("name", "Archiving")
                        .cell("state", state("Managed", Tone::Neutral))
                        .cell("duration", "0m 51s"),
                ])
                .visible_rows(6)
                .on_sort(|_, _, _, _| {})
                .on_select(|_, _, _| {}),
        )
        .child(
            Table::new("scene.table.stale")
                .columns(columns.clone())
                .rows([Row::new("run-a04")
                    .text("Indexing")
                    .cell("name", "Indexing")
                    .cell("state", state("Ready", Tone::Success))
                    .cell("duration", "4m 12s")])
                .failure("The host refused the refresh")
                .on_select(|_, _, _| {}),
        )
        .child(
            Table::new("scene.table.empty")
                .columns(columns)
                .on_select(|_, _, _| {}),
        )
        .into_any_element()
}

/// How many rows the fixture host has handed over, and how many exist behind
/// it. The two numbers differ on purpose: that gap is what the select-all box
/// and the bulk bar have to be honest about.
pub(super) const FIXTURE_JOBS_LOADED: usize = 240;

pub(super) const FIXTURE_JOBS_TOTAL: usize = 12_000;

/// One synthetic job. Nothing here stands for a product: the identity is a
/// fixture key and the label says so.
pub(super) fn fixture_job(
    index: usize,
) -> (SharedString, SharedString, SharedString, SharedString) {
    const PHASES: [&str; 4] = ["Indexing", "Verifying", "Publishing", "Archiving"];
    const OWNERS: [&str; 3] = ["fixture-a", "fixture-b", "fixture-c"];
    (
        SharedString::from(format!("job-{index:04}")),
        SharedString::from(format!("{} {index:04}", PHASES[index % PHASES.len()])),
        SharedString::from(OWNERS[index % OWNERS.len()]),
        SharedString::from(format!("{}m {:02}s", index % 9 + 1, index * 7 % 60)),
    )
}

pub(super) fn fixture_job_tone(index: usize) -> (&'static str, Tone) {
    match index % 4 {
        0 => ("Ready", Tone::Success),
        1 => ("Stale", Tone::Warning),
        2 => ("Refused", Tone::Danger),
        _ => ("Managed", Tone::Neutral),
    }
}

pub(super) fn grid_columns() -> [GridColumn; 4] {
    [
        // Declared second, drawn first: a pinned column holds the left edge
        // whatever order the caller puts the columns in.
        GridColumn::new("owner", "Owner")
            .fixed(120.0)
            .reorderable(true)
            .editable(true),
        GridColumn::new("name", "Job")
            .flex(2.0)
            .min_width(140.0)
            .pinned(true)
            .sortable(true)
            .resizable(true),
        GridColumn::new("state", "State")
            .fixed(110.0)
            .reorderable(true),
        GridColumn::new("duration", "Duration")
            .fixed(104.0)
            .align(Align::End)
            .sortable(true)
            .resizable(true)
            .reorderable(true),
    ]
}

pub(super) fn grid_row(index: usize) -> GridRow {
    let (id, name, owner, duration) = fixture_job(index);
    let (label, tone) = fixture_job_tone(index);
    GridRow::new(id)
        .text(name.clone())
        .cell("name", Cell::new(name.clone()).text(name).published(true))
        .cell("owner", Cell::new(owner.clone()).text(owner))
        .cell(
            "state",
            Cell::new(Badge::new(label).tone(tone))
                .text(label)
                .published(true),
        )
        .cell("duration", duration)
}

pub(super) fn grid_detail(theme: &Theme, id: SharedString) -> AnyElement {
    div()
        .column()
        .gap(px(theme.spacing.xs))
        .child(
            crate::foundation::text(
                theme,
                TypeScale::Caption,
                SharedString::from(format!("Fixture detail for {id}")),
            )
            .text_tone(theme, TextTone::Muted),
        )
        .child(
            crate::foundation::text(
                theme,
                TypeScale::Body,
                SharedString::from(
                    "Only an opened row builds this region; the rest never ask for it.",
                ),
            )
            .text_tone(theme, TextTone::Muted),
        )
        .into_any_element()
}

pub(super) fn data_grid(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let detail_theme = theme.clone();
    stack(&theme)
        .w(px(760.0))
        .child(
            BulkBar::new("scene.data-grid.bulk", 2)
                .total(FIXTURE_JOBS_TOTAL)
                .action(
                    Button::new("scene.data-grid.bulk.retry")
                        .label("Retry")
                        .secondary()
                        .small()
                        .on_click(|_, _| {}),
                )
                .action(
                    Button::new("scene.data-grid.bulk.archive")
                        .label("Archive")
                        .secondary()
                        .small()
                        .on_click(|_, _| {}),
                )
                .on_select_all(|_, _| {})
                .on_dismiss(|_, _| {}),
        )
        .child(
            DataGrid::new(
                "scene.data-grid.jobs",
                FIXTURE_JOBS_LOADED,
                |index, _, _| grid_row(index),
            )
            .total(FIXTURE_JOBS_TOTAL)
            .columns(grid_columns())
            // The columns a group names have to be adjacent in the order the
            // caller declared, which is what makes the group a bracket over
            // them rather than a caption over one of them.
            .group(ColumnGroup::new("identity", "Identity").columns(["name", "owner"]))
            .footer_cell("duration", "4.2s")
            .sorted_by("duration", SortDirection::Descending)
            .selection_mode(SelectionMode::Multiple)
            .selected(["job-0001", "job-0003"])
            .expanded([Expanded::new("job-0002", 2)])
            .detail_rows(2)
            .detail(move |id, _, _| grid_detail(&detail_theme, id))
            .visible_rows(9)
            .on_sort(|_, _, _, _| {})
            .on_select(|_, _, _| {})
            .on_resize(|_, _, _, _| {})
            .on_fit(|_, _, _| {})
            .on_reorder(|_, _, _| {})
            .on_expand(|_, _, _, _| {})
            .on_edit_request(|_, _, _, _| {})
            .on_edit(|_, _, _| {})
            .range(Some(CellRange::new(
                "job-0001", "name", "job-0003", "state",
            )))
            .on_range_change(|_, _, _| {})
            .on_copy(|_, _, _| {}),
        )
        .child(
            crate::foundation::text(
                &theme,
                TypeScale::Caption,
                SharedString::from(format!(
                    "{FIXTURE_JOBS_LOADED} rows loaded of {FIXTURE_JOBS_TOTAL}; only the drawn \
                     ones publish"
                )),
            )
            .text_tone(&theme, TextTone::Muted),
        )
        .into_any_element()
}

pub(super) fn data_grid_editing(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(760.0))
        .child(
            DataGrid::new("scene.data-grid-editing.jobs", 6, |index, _, _| {
                grid_row(index)
            })
            .columns(grid_columns())
            .sorted_by("duration", SortDirection::Descending)
            .selection_mode(SelectionMode::Single)
            .selected(["job-0001"])
            .editing(Some(EditingCell::new("job-0001", "owner", "fixture-b")))
            .visible_rows(6)
            .on_sort(|_, _, _, _| {})
            .on_select(|_, _, _| {})
            .on_resize(|_, _, _, _| {})
            .on_edit_request(|_, _, _, _| {})
            .on_edit(|_, _, _| {}),
        )
        .child(
            crate::foundation::text(
                &theme,
                TypeScale::Caption,
                SharedString::from(
                    "Escape reverts, enter commits, tab commits and moves on. The grid never \
                     writes the value.",
                ),
            )
            .text_tone(&theme, TextTone::Muted),
        )
        .into_any_element()
}

pub(super) fn tree_grid(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    // Whether a branch is open is structure, and the disclosure already says
    // it. What belongs in a state column is what is true of the record — the
    // same vocabulary, in the same pills, that `table` and `data-grid` use.
    let rows = [
        (
            "workspace",
            "Workspace",
            1,
            true,
            true,
            None,
            "Ready",
            Tone::Success,
        ),
        (
            "src",
            "src",
            2,
            true,
            true,
            Some("workspace"),
            "Ready",
            Tone::Success,
        ),
        (
            "components",
            "components",
            3,
            false,
            false,
            Some("src"),
            "Stale",
            Tone::Warning,
        ),
        (
            "lib",
            "lib.rs",
            3,
            false,
            false,
            Some("src"),
            "Ready",
            Tone::Success,
        ),
        (
            "docs",
            "docs",
            2,
            true,
            false,
            Some("workspace"),
            "Refused",
            Tone::Danger,
        ),
    ];
    stack(&theme)
        .w(px(720.0))
        .child(
            TreeGrid::new("scene.tree-grid.files", rows.len(), move |index, _, _| {
                let (id, name, level, branch, expanded, parent, state, tone) = rows[index];
                let mut row = TreeGridRow::new(id, level)
                    .text(name)
                    .cell("name", Cell::new(name).text(name).published(true))
                    .cell("kind", if branch { "Folder" } else { "File" })
                    .cell(
                        "state",
                        Cell::new(Badge::new(state).tone(tone))
                            .text(state)
                            .published(true),
                    );
                if branch {
                    row = row.branch(expanded);
                }
                if let Some(parent) = parent {
                    row = row.parent(parent);
                }
                row
            })
            .columns([
                GridColumn::new("name", "Name").flex(2.0),
                GridColumn::new("kind", "Kind").fixed(120.0),
                GridColumn::new("state", "State").flex(1.0),
            ])
            .selected("components")
            .visible_rows(5)
            .on_select(|_, _, _| {})
            .on_expand(|_, _, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn tree(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(360.0))
        .child(
            Tree::new("scene.tree.workspace")
                .selected("tokens")
                .nodes([
                    TreeNode::new("workspace", "workspace")
                        .icon(Icon::Folder)
                        .children([
                            TreeNode::new("crates", "crates")
                                .icon(Icon::Folder)
                                .children([
                                    TreeNode::new("kit", "gpui-kit").icon(Icon::Document),
                                    TreeNode::new("tokens", "gpui-kit-tokens").icon(Icon::Document),
                                ]),
                            TreeNode::new("docs", "docs")
                                .icon(Icon::Folder)
                                .children([TreeNode::new("components", "components.md")
                                    .icon(Icon::Document)]),
                        ]),
                    TreeNode::new("target", "target")
                        .icon(Icon::Archive)
                        .disabled(true)
                        .children([TreeNode::new("debug", "debug").icon(Icon::Folder)]),
                    TreeNode::new("remote", "remote")
                        .icon(Icon::Folder)
                        .branch(BranchState::Loading),
                ])
                .expanded_ids(&["workspace", "crates", "remote"])
                .on_toggle(|_, _, _, _| {})
                .on_select(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn drag_list(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let carried = fixture_record(4);
    let anchor = fixture_record(1);
    // A capture cannot photograph a gesture, so the drag is placed by hand.
    // Staging fixes the ghost, the indicator, and the open slot, and takes the
    // pointer and the spring out of the picture.
    dnd::stage(
        StagedDrag::new(DragItem::new(
            "scene.drag.records",
            carried.0.clone(),
            carried.1.clone(),
        ))
        .landing(
            "scene.drag.records",
            DropPosition::Before(anchor.0.clone()),
            Some(1),
            true,
        ),
        cx,
    );

    stack(&theme)
        .w(px(420.0))
        .child(caption(
            &theme,
            SharedString::from(format!("{} moving before {}", carried.1, anchor.1)),
        ))
        .child(
            div()
                .relative()
                .child(
                    div()
                        .surface(&theme, Surface::Panel)
                        .radius(&theme, Radius::Card)
                        .py_token(&theme, Space::Xs)
                        .overflow_hidden()
                        .child(
                            List::new("scene.drag.records", 6, |index, _, _| {
                                let (id, label) = fixture_record(index);
                                ListItem::new(id, label.clone()).text(label)
                            })
                            // A row slides without its layout slot moving, so the
                            // viewport is one row taller than the rows it holds and
                            // the open slot has somewhere to be.
                            .visible_rows(7)
                            .reorderable(true)
                            .on_select(|_, _, _| {})
                            .on_reorder(|_, _, _| {}),
                        ),
                )
                // The ghost is what the pointer is holding, and the pointer is
                // beside the rows rather than on top of them: staged over a
                // row it would cut the label it is meant to be leaving.
                .children(
                    dnd::staged_ghost(cx)
                        .map(|ghost| div().absolute().left(px(248.0)).top(px(52.0)).child(ghost)),
                ),
        )
        .into_any_element()
}

pub(super) fn drag_tree(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    dnd::stage(
        StagedDrag::new(
            DragItem::new("scene.drag.workspace", "kit", "gpui-kit").icon(Icon::Document),
        )
        .landing(
            "scene.drag.workspace",
            DropPosition::Into(SharedString::new_static("docs")),
            None,
            true,
        ),
        cx,
    );

    stack(&theme)
        .w(px(360.0))
        .child(caption(&theme, "gpui-kit moving into docs"))
        .child(
            div()
                .relative()
                .child(
                    Tree::new("scene.drag.workspace")
                        .expanded_ids(&["workspace", "crates", "docs"])
                        .nodes([TreeNode::new("workspace", "workspace")
                            .icon(Icon::Folder)
                            .children([
                                TreeNode::new("crates", "crates")
                                    .icon(Icon::Folder)
                                    .children([
                                        TreeNode::new("kit", "gpui-kit").icon(Icon::Document),
                                        TreeNode::new("tokens", "gpui-kit-tokens")
                                            .icon(Icon::Document),
                                    ]),
                                TreeNode::new("docs", "docs")
                                    .icon(Icon::Folder)
                                    .children([TreeNode::new("components", "components.md")
                                        .icon(Icon::Document)]),
                            ])])
                        .reorderable(true)
                        .on_toggle(|_, _, _, _| {})
                        .on_select(|_, _, _| {})
                        .on_move(|_, _, _| {}),
                )
                .children(
                    dnd::staged_ghost(cx)
                        // Beside the row it is landing on, and level with it:
                        // a ghost drawn over the target hides the one thing
                        // the picture is about.
                        .map(|ghost| div().absolute().left(px(300.0)).top(px(155.0)).child(ghost)),
                ),
        )
        .into_any_element()
}

pub(super) fn kanban(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(640.0))
        .child(caption(
            &theme,
            "columns and cards are host-owned; a held card can report a move",
        ))
        .child(
            KanbanBoard::new("scene.kanban.ready")
                .columns([
                    KanbanColumn::new("inbox", "Inbox"),
                    KanbanColumn::new("doing", "Doing").limit(2),
                    KanbanColumn::new("done", "Done"),
                ])
                .cards([
                    KanbanCard::new("triage", "Triage fixture", "inbox")
                        .detail("Waiting on review"),
                    KanbanCard::new("draw", "Draw scene", "doing"),
                    KanbanCard::new("caption", "Write captions", "doing"),
                    KanbanCard::new("audit", "Audit tokens", "doing"),
                    KanbanCard::new("ship", "Ship notes", "done"),
                ])
                .held("triage")
                .on_card(|_, _, _| {})
                .on_move(|_, _, _, _| {})
                .on_add(|_, _, _| {}),
        )
        .child(KanbanBoard::new("scene.kanban.empty").state(KanbanState::Empty))
        .child(
            KanbanBoard::new("scene.kanban.unavailable").state(KanbanState::Unavailable(
                "The board host refused the request.".into(),
            )),
        )
        .into_any_element()
}
