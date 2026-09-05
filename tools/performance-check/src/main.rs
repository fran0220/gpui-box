//! Deterministic structural performance authority for large Kit surfaces.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use anyhow::{Context as _, Result, bail};
use gpui::{AnyElement, IntoElement, ParentElement as _, Styled as _, TestAppContext, div};
use gpui_kit::foundation::Selectable as _;
use gpui_kit::prelude::{
    AgentDocument, AgentDocumentBlock, Badge, Button, CodeLine, CodeView, ColorChoice, DataGrid,
    GraphInteraction, GraphNode, GridColumn, GridRow, List, ListItem, LogEntry, LogStream,
    NodeGraph, TreeGrid, TreeGridRow, Variant,
};
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;
use gpui_kit_testkit::{
    PerformanceBudget, PerformanceMetric, PerformanceReport, PerformanceSample,
};

const DATASET_ITEMS: usize = 10_000;
const VISIBLE_ROWS: usize = 24;
/// A full visible board rather than a virtualized row fixture: half ordinary
/// pseudo-glass, half promoted glass requests.
const MATERIAL_NODES: usize = 64;
const PROMOTED_NODES: usize = 32;
const THEME_SEMANTIC_NODES: usize = 128;
const _: () = assert!(PROMOTED_NODES > gpui::MAX_BACKDROP_GLASS_SURFACES_PER_FRAME);

struct CountingAllocator;

static COUNT_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static HEAP_ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

// SAFETY: every operation delegates to `System` with the exact layout and
// pointer it received. The atomic bookkeeping neither allocates nor changes
// allocator behavior.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        // SAFETY: `layout` is forwarded unchanged to the system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        // SAFETY: `layout` is forwarded unchanged to the system allocator.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` and `layout` came from the delegated system allocator.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count_allocation();
        // SAFETY: all arguments are forwarded unchanged to the system allocator.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

fn count_allocation() {
    if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
        HEAP_ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    }
}

fn begin_allocation_measurement() {
    COUNT_ALLOCATIONS.store(false, Ordering::Relaxed);
    HEAP_ALLOCATIONS.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
}

fn end_allocation_measurement() -> u64 {
    COUNT_ALLOCATIONS.store(false, Ordering::Release);
    HEAP_ALLOCATIONS.load(Ordering::Acquire)
}

/// What a fixture hands the harness: one view builder, called every frame.
type ViewBuilder = Box<dyn Fn(&mut gpui::Window, &mut gpui::App) -> AnyElement>;

fn main() -> Result<()> {
    let output = output_path()?;
    let mut reports = Vec::new();
    for items in [1_000, DATASET_ITEMS] {
        for (name, fixture) in [
            ("list", list_fixture as Fixture),
            ("data-grid", data_grid_fixture),
            ("tree-grid", tree_grid_fixture),
            ("code-view", code_view_fixture),
            ("log-stream", log_stream_fixture),
            ("agent-document", agent_document_fixture),
        ] {
            let report = run(name, fixture, items)?;
            if items == DATASET_ITEMS {
                let small = reports
                    .iter()
                    .find(|small: &&serde_json::Value| small["name"] == name)
                    .expect("smaller fixture ran first");
                for path in [
                    "/sample/mounted_items",
                    "/sample/builder_calls",
                    "/sample/frame/request_layout_calls",
                    "/sample/frame/prepaint_calls",
                    "/sample/frame/paint_calls",
                    "/sample/frame/semantic_nodes",
                ] {
                    let large_count = report
                        .pointer(path)
                        .and_then(serde_json::Value::as_u64)
                        .context("large fixture metric unavailable")?;
                    let small_count = small
                        .pointer(path)
                        .and_then(serde_json::Value::as_u64)
                        .context("small fixture metric unavailable")?;
                    if large_count > small_count {
                        bail!("{name} viewport work {path} grew with dataset size");
                    }
                }
            }
            reports.push(report);
        }
    }
    reports.push(run(
        "node-graph-material",
        node_graph_material_fixture,
        MATERIAL_NODES,
    )?);
    reports.push(run(
        "theme-semantics",
        theme_semantics_fixture,
        THEME_SEMANTIC_NODES,
    )?);
    reports.push(serde_json::to_value(run_idle_frame()?)?);
    prove_unbounded_fixture_fails()?;

    let document = serde_json::json!({
        "schema_version": 3,
        "dataset_sizes": [1_000, DATASET_ITEMS],
        "dataset_items": DATASET_ITEMS,
        "viewport_rows": VISIBLE_ROWS,
        "node_graph_nodes": MATERIAL_NODES,
        "node_graph_promoted": PROMOTED_NODES,
        "theme_semantic_nodes": THEME_SEMANTIC_NODES,
        "backdrop_glass_admission": gpui::MAX_BACKDROP_GLASS_SURFACES_PER_FRAME,
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

type Fixture = fn(Rc<Cell<u64>>, usize) -> ViewBuilder;

fn run(name: &str, fixture: Fixture, items: usize) -> Result<serde_json::Value> {
    let calls = Rc::new(Cell::new(0));
    let build = fixture(Rc::clone(&calls), items);
    let mut cx = TestAppContext::single();
    let mut harness = Harness::new(&mut cx, gpui_kit::install, build);

    harness.frame();
    harness.frame();
    calls.set(0);
    begin_allocation_measurement();
    harness.frame();
    let heap_allocations = end_allocation_measurement();
    let stats = harness.frame_stats();
    let snapshot = harness.current_snapshot();
    let mounted_rows = snapshot
        .nodes
        .iter()
        .filter(|node| matches!(node.role, Role::Row | Role::TreeItem))
        .count() as u64;
    // Eager collection APIs have no caller row-builder callback. Their input
    // conversions are real dataset-sized work, not mounted rows in disguise.
    let eager = matches!(name, "code-view" | "log-stream" | "agent-document");
    let builder_calls = if eager { 0 } else { calls.get() };
    let sample = PerformanceSample::new(stats)
        .heap_allocations(heap_allocations)
        .mounted_items(mounted_rows)
        .builder_calls(builder_calls);

    let report = budget(name)
        .enforce(sample)
        .map_err(|error| anyhow::anyhow!(error))?;
    let mut report = serde_json::to_value(report)?;
    report["dataset_items"] = items.into();
    report["caller_input_conversions"] = if eager { calls.get() } else { 0 }.into();
    report["has_row_builder_callback"] = matches!(name, "list" | "data-grid" | "tree-grid").into();
    Ok(report)
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
        .limit(
            PerformanceMetric::HeapAllocations,
            heap_allocation_limit(name),
        )
        .limit(PerformanceMetric::MountedItems, 96)
        .limit(PerformanceMetric::BuilderCalls, 128)
}

fn heap_allocation_limit(name: &str) -> u64 {
    match name {
        // Baseline plus a 10% integer ceiling. These are ratchets, not generic
        // capacity targets: lower the matching value when an optimization
        // removes steady-state frame allocations.
        "list" => 1_330,
        "data-grid" => 3_723,
        "tree-grid" => 4_452,
        "code-view" => 6_096,
        "log-stream" => 6_485,
        "agent-document" => 6_940,
        "node-graph-material" => 12_105,
        "theme-semantics" => 7_251,
        "idle-frame" => 0,
        "unbounded-detector-proof" => u64::MAX,
        _ => panic!("fixture `{name}` has no heap-allocation ratchet"),
    }
}

fn run_idle_frame() -> Result<PerformanceReport> {
    let mut cx = TestAppContext::single();
    let mut harness = Harness::new(&mut cx, gpui_kit::install, |_, _| div().into_any_element());
    harness.frame();
    harness.frame();
    let before = harness.frame_stats();
    begin_allocation_measurement();
    harness.context().run_until_parked();
    let heap_allocations = end_allocation_measurement();
    let after = harness.frame_stats();
    if after.frame_index != before.frame_index {
        bail!(
            "idle fixture drew frame {} after settling frame {} without invalidation",
            after.frame_index,
            before.frame_index
        );
    }

    let interval = gpui::FrameStats {
        frame_index: after.frame_index,
        allocator_delta_bytes: Some(0),
        ..Default::default()
    };
    PerformanceBudget::new("idle-frame")
        .limit(PerformanceMetric::EntityRenders, 0)
        .limit(PerformanceMetric::RequestLayoutCalls, 0)
        .limit(PerformanceMetric::PrepaintCalls, 0)
        .limit(PerformanceMetric::PaintCalls, 0)
        .limit(PerformanceMetric::Invalidations, 0)
        .limit(PerformanceMetric::SemanticNodes, 0)
        .limit(PerformanceMetric::PlatformViewPlacements, 0)
        .limit(PerformanceMetric::AllocatorDeltaBytes, 0)
        .limit(
            PerformanceMetric::HeapAllocations,
            heap_allocation_limit("idle-frame"),
        )
        .limit(PerformanceMetric::MountedItems, 0)
        .limit(PerformanceMetric::BuilderCalls, 0)
        .enforce(PerformanceSample::new(interval).heap_allocations(heap_allocations))
        .map_err(|error| anyhow::anyhow!(error))
}

fn list_fixture(calls: Rc<Cell<u64>>, items: usize) -> ViewBuilder {
    Box::new(move |_, _| {
        let calls = Rc::clone(&calls);
        List::new("perf.list", items, move |index, _, _| {
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

fn data_grid_fixture(calls: Rc<Cell<u64>>, items: usize) -> ViewBuilder {
    Box::new(move |_, _| {
        let calls = Rc::clone(&calls);
        DataGrid::new("perf.data-grid", items, move |index, _, _| {
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

fn tree_grid_fixture(calls: Rc<Cell<u64>>, items: usize) -> ViewBuilder {
    Box::new(move |_, _| {
        let calls = Rc::clone(&calls);
        TreeGrid::new("perf.tree-grid", items, move |index, _, _| {
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

fn code_view_fixture(calls: Rc<Cell<u64>>, items: usize) -> ViewBuilder {
    let lines = (0..items)
        .map(|index| CodeLine::new(index + 1, format!("let row_{index} = {index};")))
        .collect::<Vec<_>>();
    Box::new(move |_, _| {
        CodeView::new(
            "perf.code-view",
            lines.iter().map(|line| {
                calls.set(calls.get() + 1);
                line.clone()
            }),
        )
        .visible_lines(VISIBLE_ROWS)
        .copyable(false)
        .into_any_element()
    })
}

fn log_stream_fixture(calls: Rc<Cell<u64>>, items: usize) -> ViewBuilder {
    let entries = (0..items)
        .map(|index| LogEntry::new(format!("entry-{index}"), format!("message {index}")))
        .collect::<Vec<_>>();
    Box::new(move |_, _| {
        LogStream::new(
            "perf.log-stream",
            entries.iter().map(|entry| {
                calls.set(calls.get() + 1);
                entry.clone()
            }),
        )
        .visible_rows(VISIBLE_ROWS)
        .into_any_element()
    })
}

fn agent_document_fixture(calls: Rc<Cell<u64>>, items: usize) -> ViewBuilder {
    let data = (0..items)
        .map(|index| {
            (
                gpui::SharedString::from(format!("block-{index}")),
                gpui::SharedString::from(format!("Agent transcript paragraph {index}")),
            )
        })
        .collect::<Vec<_>>();
    Box::new(move |_, _| {
        AgentDocument::new("perf.agent-document")
            .blocks(data.iter().map(|(id, text)| {
                calls.set(calls.get() + 1);
                AgentDocumentBlock::text(id.clone(), text.clone())
            }))
            .virtualized(VISIBLE_ROWS)
            .into_any_element()
    })
}

fn node_graph_material_fixture(calls: Rc<Cell<u64>>, _items: usize) -> ViewBuilder {
    // Deliberately cross the renderer's admission ceiling. The admitted panes
    // carry real Frosted snapshots; requests beyond it keep their material
    // fill through the framework fallback. This is the high-state-density
    // case the split material policy must bound, not only its easy rest case.
    Box::new(move |_, _| {
        let mut graph = NodeGraph::new("perf.node-graph-material")
            .interaction(GraphInteraction::Inspect)
            .grid(false);
        for index in 0..MATERIAL_NODES {
            calls.set(calls.get().saturating_add(1));
            let column = index % 8;
            let row = index / 8;
            graph = graph.node(
                GraphNode::new(format!("material-node-{index}"), format!("Node {index}"))
                    .width(84.0)
                    .selected(index < PROMOTED_NODES),
                10.0 + column as f32 * 96.0,
                10.0 + row as f32 * 64.0,
            );
        }
        div().size_full().child(graph).into_any_element()
    })
}

fn theme_semantics_fixture(_calls: Rc<Cell<u64>>, _items: usize) -> ViewBuilder {
    Box::new(move |_, _| {
        div()
            .flex()
            .flex_wrap()
            .children((0..THEME_SEMANTIC_NODES).map(|index| {
                let color = ColorChoice::Palette("teal".into());
                if index % 2 == 0 {
                    Button::new(format!("perf.theme.button-{index}"))
                        .label(format!("Button {index}"))
                        .variant(Variant::Subtle)
                        .color(color)
                        .into_any_element()
                } else {
                    Badge::new(format!("Badge {index}"))
                        .id(format!("perf.theme.badge-{index}"))
                        .variant(Variant::Subtle)
                        .color(color)
                        .into_any_element()
                }
            }))
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
