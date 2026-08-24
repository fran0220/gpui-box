//! Deterministic structural performance authority for large Kit surfaces.

use std::cell::Cell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::{Context as _, Result, bail};
use gpui::{AnyElement, IntoElement, ParentElement as _, TestAppContext, div};
use gpui_kit::prelude::{
    AgentDocument, AgentDocumentBlock, CodeLine, CodeView, DataGrid, GridColumn, GridRow, List,
    ListItem, LogEntry, LogStream, TreeGrid, TreeGridRow,
};
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;
use gpui_kit_testkit::{
    PerformanceBudget, PerformanceMetric, PerformanceReport, PerformanceSample,
};

const DATASET_ITEMS: usize = 10_000;
const VISIBLE_ROWS: usize = 24;

/// What a fixture hands the harness: one view builder, called every frame.
type ViewBuilder = Box<dyn Fn(&mut gpui::Window, &mut gpui::App) -> AnyElement>;

fn main() -> Result<()> {
    let output = output_path()?;
    let reports = vec![
        run("list", list_fixture)?,
        run("data-grid", data_grid_fixture)?,
        run("tree-grid", tree_grid_fixture)?,
        run("code-view", code_view_fixture)?,
        run("log-stream", log_stream_fixture)?,
        run("agent-document", agent_document_fixture)?,
    ];
    prove_unbounded_fixture_fails()?;

    let document = serde_json::json!({
        "schema_version": 1,
        "dataset_items": DATASET_ITEMS,
        "viewport_rows": VISIBLE_ROWS,
        "reports": reports,
        "detector_proof": "unbounded-10k-fixture-refused",
    });
    let encoded = serde_json::to_string_pretty(&document)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create performance report directory {}", parent.display()))?;
    }
    fs::write(&output, format!("{encoded}\n"))
        .with_context(|| format!("write performance report {}", output.display()))?;
    println!("{encoded}");
    eprintln!("performance report written to {}", output.display());
    Ok(())
}

fn output_path() -> Result<PathBuf> {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (None, None, None) => Ok(PathBuf::from("target/performance/report.json")),
        (Some("--output"), Some(path), None) => Ok(path.into()),
        _ => bail!("usage: gpui-box-performance [--output <report.json>]"),
    }
}

type Fixture = fn(Rc<Cell<u64>>) -> ViewBuilder;

fn run(name: &str, fixture: Fixture) -> Result<PerformanceReport> {
    let calls = Rc::new(Cell::new(0));
    let build = fixture(Rc::clone(&calls));
    let mut cx = TestAppContext::single();
    let mut harness = Harness::new(&mut cx, gpui_kit::install, build);

    calls.set(0);
    harness.frame();
    let stats = harness.frame_stats();
    let snapshot = harness.current_snapshot();
    let mounted_rows = snapshot
        .nodes
        .iter()
        .filter(|node| matches!(node.role, Role::Row | Role::TreeItem))
        .count() as u64;
    let builder_calls = calls.get().max(mounted_rows);
    let sample = PerformanceSample::new(stats)
        .mounted_items(mounted_rows)
        .builder_calls(builder_calls);

    budget(name)
        .enforce(sample)
        .map_err(|error| anyhow::anyhow!(error))
}

fn budget(name: &str) -> PerformanceBudget {
    PerformanceBudget::new(name)
        .limit(PerformanceMetric::EntityRenders, 8)
        .limit(PerformanceMetric::RequestLayoutCalls, 1_500)
        .limit(PerformanceMetric::PrepaintCalls, 1_500)
        .limit(PerformanceMetric::PaintCalls, 1_500)
        .limit(PerformanceMetric::Invalidations, 4)
        .limit(PerformanceMetric::SemanticNodes, 350)
        .limit(PerformanceMetric::PlatformViewPlacements, 0)
        .limit(PerformanceMetric::AllocatorDeltaBytes, 0)
        .limit(PerformanceMetric::MountedItems, 96)
        .limit(PerformanceMetric::BuilderCalls, 128)
}

fn list_fixture(calls: Rc<Cell<u64>>) -> ViewBuilder {
    Box::new(move |_, _| {
        let calls = Rc::clone(&calls);
        List::new("perf.list", DATASET_ITEMS, move |index, _, _| {
            calls.set(calls.get().saturating_add(1));
            ListItem::new(
                format!("row-{index}"),
                div().child(format!("List row {index}")),
            )
        })
        .visible_rows(VISIBLE_ROWS)
        .into_any_element()
    })
}

fn data_grid_fixture(calls: Rc<Cell<u64>>) -> ViewBuilder {
    Box::new(move |_, _| {
        let calls = Rc::clone(&calls);
        DataGrid::new("perf.data-grid", DATASET_ITEMS, move |index, _, _| {
            calls.set(calls.get().saturating_add(1));
            GridRow::new(format!("row-{index}"))
                .text(format!("Data row {index}"))
                .cell("name", format!("Record {index}"))
                .cell("state", "Ready")
                .cell("owner", "GPUI Box")
        })
        .columns([
            GridColumn::new("name", "Name"),
            GridColumn::new("state", "State"),
            GridColumn::new("owner", "Owner"),
        ])
        .visible_rows(VISIBLE_ROWS)
        .into_any_element()
    })
}

fn tree_grid_fixture(calls: Rc<Cell<u64>>) -> ViewBuilder {
    Box::new(move |_, _| {
        let calls = Rc::clone(&calls);
        TreeGrid::new("perf.tree-grid", DATASET_ITEMS, move |index, _, _| {
            calls.set(calls.get().saturating_add(1));
            TreeGridRow::new(format!("node-{index}"), 1)
                .text(format!("Tree row {index}"))
                .cell("name", format!("Node {index}"))
                .cell("state", "Ready")
        })
        .columns([
            GridColumn::new("name", "Name"),
            GridColumn::new("state", "State"),
        ])
        .visible_rows(VISIBLE_ROWS)
        .into_any_element()
    })
}

fn code_view_fixture(_calls: Rc<Cell<u64>>) -> ViewBuilder {
    Box::new(move |_, _| {
        CodeView::new(
            "perf.code-view",
            (0..DATASET_ITEMS)
                .map(|index| CodeLine::new(index + 1, format!("let row_{index} = {index};"))),
        )
        .visible_lines(VISIBLE_ROWS)
        .copyable(false)
        .into_any_element()
    })
}

fn log_stream_fixture(_calls: Rc<Cell<u64>>) -> ViewBuilder {
    Box::new(move |_, _| {
        LogStream::new(
            "perf.log-stream",
            (0..DATASET_ITEMS)
                .map(|index| LogEntry::new(format!("entry-{index}"), format!("message {index}"))),
        )
        .visible_rows(VISIBLE_ROWS)
        .into_any_element()
    })
}

fn agent_document_fixture(_calls: Rc<Cell<u64>>) -> ViewBuilder {
    Box::new(move |_, _| {
        AgentDocument::new("perf.agent-document")
            .blocks((0..DATASET_ITEMS).map(|index| {
                AgentDocumentBlock::text(
                    format!("block-{index}"),
                    format!("Agent transcript paragraph {index}"),
                )
            }))
            .virtualized(VISIBLE_ROWS)
            .into_any_element()
    })
}

fn prove_unbounded_fixture_fails() -> Result<()> {
    let mut cx = TestAppContext::single();
    let mut harness = Harness::new(&mut cx, gpui_kit::install, |_, _| {
        div()
            .children((0..DATASET_ITEMS).map(|index| div().child(format!("row {index}"))))
            .into_any_element()
    });
    harness.frame();
    let sample = PerformanceSample::new(harness.frame_stats());
    if budget("unbounded-detector-proof").enforce(sample).is_ok() {
        bail!("the deliberately unbounded 10,000-row fixture unexpectedly passed")
    }
    Ok(())
}
