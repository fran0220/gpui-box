//! Deterministic structural performance authority for large Kit surfaces.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use anyhow::{Context as _, Result, bail};
use gpui::{
    AnyElement, AppContext as _, IntoElement, ParentElement as _, Styled as _, TestAppContext, div,
};
use gpui_kit::controls::editor::{Editor, EditorSyntax};
use gpui_kit::controls::textarea::{TextArea, TextAreaWrap};
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
static HEAP_REQUESTED_BYTES: AtomicU64 = AtomicU64::new(0);

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

// SAFETY: every operation delegates to `System` with the exact layout and
// pointer it received. The atomic bookkeeping neither allocates nor changes
// allocator behavior.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation(layout.size());
        // SAFETY: `layout` is forwarded unchanged to the system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count_allocation(layout.size());
        // SAFETY: `layout` is forwarded unchanged to the system allocator.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` and `layout` came from the delegated system allocator.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count_allocation(new_size);
        // SAFETY: all arguments are forwarded unchanged to the system allocator.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

fn count_allocation(bytes: usize) {
    if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
        HEAP_ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        HEAP_REQUESTED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    }
}

fn begin_allocation_measurement() {
    COUNT_ALLOCATIONS.store(false, Ordering::Relaxed);
    HEAP_ALLOCATIONS.store(0, Ordering::Relaxed);
    HEAP_REQUESTED_BYTES.store(0, Ordering::Relaxed);
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
    for columns in [1_000, 10_000] {
        for rtl in [false, true] {
            for report in run_wide_grid(columns, rtl)? {
                if columns == 10_000 && report["destination"].as_u64().is_some_and(|at| at <= 703) {
                    let small = reports
                        .iter()
                        .find(|small| {
                            small["name"] == "wide-data-grid"
                                && small["dataset_columns"] == 1_000
                                && small["rtl"] == rtl
                                && small["destination"] == report["destination"]
                        })
                        .expect("matching smaller viewport");
                    for path in [
                        "/cell_builder_calls",
                        "/sample/builder_calls",
                        "/sample/frame/request_layout_calls",
                        "/sample/frame/prepaint_calls",
                        "/sample/frame/paint_calls",
                        "/sample/frame/semantic_nodes",
                    ] {
                        let small = small
                            .pointer(path)
                            .and_then(serde_json::Value::as_u64)
                            .context("small wide metric unavailable")?;
                        let large = report
                            .pointer(path)
                            .and_then(serde_json::Value::as_u64)
                            .context("large wide metric unavailable")?;
                        if large > small {
                            bail!("wide-grid viewport work {path} grew with column count");
                        }
                    }
                }
                reports.push(report);
            }
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
    for items in [1_000, 10_000] {
        eprintln!("measuring Markdown history: {items}");
        let mut measured = run_markdown_history(items)?;
        for syntax in [false, true] {
            eprintln!("measuring editable document: {items}, syntax={syntax}");
            measured.extend(run_editable_document(items, syntax)?);
        }
        for report in measured {
            if items == 10_000 {
                let small = reports
                    .iter()
                    .find(|small| {
                        small["name"] == report["name"] && small["phase"] == report["phase"]
                    })
                    .expect("smaller document fixture");
                for path in [
                    "/checked_frame/sample/frame/request_layout_calls",
                    "/checked_frame/sample/frame/prepaint_calls",
                    "/checked_frame/sample/frame/paint_calls",
                    "/checked_frame/sample/frame/semantic_nodes",
                    "/shaped_lines",
                    "/shaped_bytes",
                    "/parser_input_bytes_offered",
                    "/parser_passes",
                    "/parsed_bytes",
                    "/copied_bytes",
                    "/planned_rows",
                ] {
                    anyhow::ensure!(
                        report.pointer(path) == small.pointer(path),
                        "{} {} work {path} changed with dataset size",
                        report["name"],
                        report["phase"]
                    );
                }
            }
            reports.push(report);
        }
    }
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

#[test]
fn mounted_markdown_history_work() {
    for items in [1_000, 10_000] {
        run_markdown_history(items).expect("mounted Markdown budgets");
    }
}

fn check_document_frame(harness: &mut Harness) -> Result<PerformanceReport> {
    PerformanceBudget::new("document-viewport")
        .limit(PerformanceMetric::RequestLayoutCalls, 1_500)
        .limit(PerformanceMetric::PrepaintCalls, 1_500)
        .limit(PerformanceMetric::PaintCalls, 1_500)
        .limit(PerformanceMetric::SemanticNodes, 350)
        .enforce(PerformanceSample::new(harness.frame_stats()))
        .map_err(|error| anyhow::anyhow!(error))
}

fn run_markdown_history(items: usize) -> Result<Vec<serde_json::Value>> {
    let blocks = Rc::new(RefCell::new(
        (0..items)
            .map(|index| {
                (
                    gpui::SharedString::from(format!("message-{index}")),
                    gpui::SharedString::from("**Asymmetric** history with `code`.\n\n"),
                )
            })
            .collect::<Vec<_>>(),
    ));
    let input = blocks.clone();
    let conversions = Rc::new(Cell::new(0usize));
    let converted = conversions.clone();
    let mut cx = TestAppContext::single();
    let mut harness = Harness::new(&mut cx, gpui_kit::install, move |_, _| {
        AgentDocument::new("perf.markdown")
            .blocks(input.borrow().iter().map(|(id, source)| {
                converted.set(converted.get() + 1);
                AgentDocumentBlock::markdown(id.clone(), source.clone()).streaming(true)
            }))
            .virtualized(VISIBLE_ROWS)
            .into_any_element()
    });
    harness.scroll("perf.markdown", 1_000_000_000.0);
    harness.frame();
    harness.frame();
    let mut reports = Vec::new();
    for phase in ["static", "append", "stream"] {
        let before =
            harness.update(|window, cx| AgentDocument::work(&"perf.markdown".into(), window, cx));
        let delta = "Streamed **delta**.\n\n";
        if phase == "append" {
            blocks
                .borrow_mut()
                .push(("new-message".into(), delta.into()));
        } else if phase == "stream" {
            blocks.borrow_mut().last_mut().expect("tail").1 = format!("{delta}{delta}").into();
        }
        conversions.set(0);
        begin_allocation_measurement();
        harness.frame();
        if phase != "static" {
            harness.scroll("perf.markdown", 1_000_000_000.0);
            harness.frame();
        }
        let allocations = end_allocation_measurement();
        let requested_bytes = HEAP_REQUESTED_BYTES.load(Ordering::Acquire);
        let after =
            harness.update(|window, cx| AgentDocument::work(&"perf.markdown".into(), window, cx));
        let passes = after.parser.parser_passes - before.parser.parser_passes;
        let parsed = after.parser.parsed_bytes - before.parser.parsed_bytes;
        let copied = after.parser.copied_bytes - before.parser.copied_bytes;
        let planned = after.planned_rows - before.planned_rows;
        if phase == "static" {
            anyhow::ensure!(
                before.parser.parser_passes == items && before.planned_rows == items,
                "initial history was not parsed and planned exactly once"
            );
            anyhow::ensure!(
                after.input_checks - before.input_checks == items,
                "history input comparisons changed"
            );
            anyhow::ensure!(
                (passes, parsed, copied, planned) == (0, 0, 0, 0),
                "static Markdown repeated work"
            );
        } else {
            anyhow::ensure!(
                passes == 1
                    && copied == delta.len()
                    && parsed == delta.len() * if phase == "stream" { 2 } else { 1 }
                    && planned == if phase == "stream" { 2 } else { 1 },
                "Markdown append work: {passes}/{parsed}/{copied}/{planned}"
            );
            anyhow::ensure!(
                harness
                    .current_snapshot()
                    .nodes
                    .iter()
                    .any(|node| node.id.starts_with("perf.markdown.block.new-message.")),
                "streamed message was not mounted"
            );
        }
        reports.push(serde_json::json!({"name":"markdown-history", "phase":phase,
            "dataset_items":items, "parser_passes":passes, "parsed_bytes":parsed,
            "copied_bytes":copied, "planned_rows":planned,
            "input_checks":after.input_checks-before.input_checks,
            "caller_input_conversions":conversions.get(), "heap_allocations":allocations,
            "measurement_scope":"update, tail reveal, and settled redraw; static is one redraw",
            "heap_requested_bytes":requested_bytes,
            "checked_frame":check_document_frame(&mut harness)?, "total_work_bounded":false}));
    }
    Ok(reports)
}

fn run_editable_document(items: usize, syntax: bool) -> Result<Vec<serde_json::Value>> {
    // Caller source preparation precedes mounting and every measured operation.
    let row = "{\"asymmetric\":\"界\",\"value\":13},\n";
    let source = format!("[\n{}{{\"tail\":7}}\n]", row.repeat(items));
    let source_bytes = source.len();
    let mut cx = TestAppContext::single();
    let area_slot = Rc::new(RefCell::new(None::<gpui::Entity<TextArea>>));
    let editor_slot = Rc::new(RefCell::new(None::<gpui::Entity<Editor>>));
    let area_build = area_slot.clone();
    let editor_build = editor_slot.clone();
    eprintln!("  mounting");
    let mut harness = Harness::new(&mut cx, gpui_kit::install, move |window, cx| {
        let child = if syntax {
            let entity = editor_build
                .borrow_mut()
                .get_or_insert_with(|| {
                    cx.new(|cx| {
                        Editor::new("perf.editor", "Source", source.clone(), window, cx)
                            .rows(8)
                            .syntax(EditorSyntax::json())
                    })
                })
                .clone();
            *area_build.borrow_mut() = Some(entity.read(cx).text_area().clone());
            entity.into_any_element()
        } else {
            area_build
                .borrow_mut()
                .get_or_insert_with(|| {
                    cx.new(|cx| {
                        TextArea::new("perf.area", window, cx)
                            .text(source.clone())
                            .rows(8)
                            .wrap(TextAreaWrap::None)
                    })
                })
                .clone()
                .into_any_element()
        };
        div().w(gpui::px(640.0)).child(child).into_any_element()
    });
    harness.frame();
    harness.frame();
    let area = area_slot.borrow().clone().expect("mounted area");
    let mut reports = Vec::new();
    for phase in ["static", "edit", "scroll", "select-all"] {
        eprintln!("  {phase}");
        let before_scroll = (phase == "scroll")
            .then(|| harness.update(|_, cx| area.read(cx).caret_bounds().expect("caret").top()));
        begin_allocation_measurement();
        match phase {
            "edit" => harness.update(|_, cx| {
                area.update(cx, |area, cx| {
                    // Change one digit near the start; preserving byte length also
                    // makes the independent select-all expectation unambiguous.
                    let offset = 2 + row.find("13").expect("number");
                    assert_eq!(
                        area.replace_range(offset..offset + 1, "2", cx),
                        Some(offset..offset + 1)
                    );
                })
            }),
            "scroll" => harness.scroll(
                if syntax {
                    "perf.editor.input"
                } else {
                    "perf.area"
                },
                173.0,
            ),
            "select-all" => harness.update(|_, cx| {
                area.update(cx, |area, cx| area.set_selected_range(0..source_bytes, cx))
            }),
            _ => {}
        }
        let operation_allocations = end_allocation_measurement();
        let operation_bytes = HEAP_REQUESTED_BYTES.load(Ordering::Acquire);
        begin_allocation_measurement();
        harness.frame();
        let frame_allocations = end_allocation_measurement();
        let frame_bytes = HEAP_REQUESTED_BYTES.load(Ordering::Acquire);
        let work = harness.update(|_, cx| {
            let area = area.read(cx);
            assert_eq!(area.document().len(), source_bytes);
            if phase != "static" {
                let offset = 2 + row.find("13").expect("number");
                assert_eq!(
                    area.document().slice(offset..offset + 2).as_deref(),
                    Some("23")
                );
            }
            if phase == "select-all" {
                assert_eq!(area.selected_range(), 0..source_bytes);
            }
            area.shaping_work().expect("mounted text layout")
        });
        anyhow::ensure!(
            work.shaped_lines > 0 && work.shaped_lines <= 9 && work.shaped_bytes <= 9 * row.len(),
            "{phase} shaped whole document: {work:?}"
        );
        if let Some(before) = before_scroll {
            let after = harness.update(|_, cx| area.read(cx).caret_bounds().expect("caret").top());
            anyhow::ensure!(
                after == before - gpui::px(173.0),
                "wheel did not scroll the viewport"
            );
        }
        let parser = if syntax && phase == "edit" {
            Some(harness.update(|_, cx| {
                editor_slot
                    .borrow()
                    .as_ref()
                    .expect("editor")
                    .read(cx)
                    .syntax_state()
                    .expect("syntax")
                    .work()
            }))
        } else {
            None
        };
        if let Some(parser) = parser {
            anyhow::ensure!(
                parser.incremental && parser.input_bytes_offered < 65_536,
                "incremental parser regressed: {parser:?}"
            );
        }
        reports.push(serde_json::json!({"name":if syntax {"editor-json"} else {"textarea-no-wrap"},
            "phase":phase, "dataset_items":items, "caller_source_bytes":source_bytes,
            "operation_heap_allocations":operation_allocations, "frame_heap_allocations":frame_allocations,
            "operation_heap_requested_bytes":operation_bytes, "frame_heap_requested_bytes":frame_bytes,
            "shaped_lines":work.shaped_lines, "shaped_bytes":work.shaped_bytes,
            "parser_input_bytes_offered":parser.map(|p| p.input_bytes_offered),
            "parser_input_requests":parser.map(|p| p.input_requests),
            "checked_frame":check_document_frame(&mut harness)?, "total_work_bounded":false}));
    }
    drop(area);
    area_slot.borrow_mut().take();
    editor_slot.borrow_mut().take();
    Ok(reports)
}

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
    report["caller_eager_cell_conversions"] = if matches!(name, "data-grid" | "tree-grid") {
        calls.get() * 3
    } else {
        0
    }
    .into();
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
        "wide-data-grid" => 12_140,
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

/// The caller retains column descriptors and supplies cells by key. Descriptor
/// cloning is still dataset-sized input work; cell building must not be.
fn run_wide_grid(column_count: usize, rtl: bool) -> Result<Vec<serde_json::Value>> {
    let columns: Vec<_> = (0..column_count)
        .map(|index| {
            let column = GridColumn::new(format!("field-{index}"), format!("Field {index}"))
                .pinned(index == 0)
                .sortable(true)
                .resizable(true)
                .editable(true);
            if index % 5 == 0 {
                column.flex(2.0).min_width(180.0)
            } else {
                column.fixed(180.0 + (index % 7) as f32 * 31.0)
            }
        })
        .collect();
    let rows = Rc::new(Cell::new(0));
    let cells = Rc::new(Cell::new(0));
    let target = Rc::new(Cell::new(0));
    let mut cx = TestAppContext::single();
    let mut harness = Harness::new(
        &mut cx,
        move |cx| {
            gpui_kit::install(cx);
            gpui_kit::prelude::set_layout_direction(
                if rtl {
                    gpui_kit::prelude::LayoutDirection::RightToLeft
                } else {
                    gpui_kit::prelude::LayoutDirection::LeftToRight
                },
                cx,
            );
        },
        {
            let rows = Rc::clone(&rows);
            let cells = Rc::clone(&cells);
            let target = Rc::clone(&target);
            move |_, _| {
                let rows = Rc::clone(&rows);
                let cells = Rc::clone(&cells);
                div()
                    .w(gpui::px(870.0))
                    .child(
                        DataGrid::new("perf.wide-grid", DATASET_ITEMS, move |row, _, _| {
                            rows.set(rows.get() + 1);
                            let cells = Rc::clone(&cells);
                            GridRow::new(format!("row-{row}")).cells_with(move |key, _, _| {
                                cells.set(cells.get() + 1);
                                gpui_kit::prelude::Cell::new(format!("{row}/{key}")).published(true)
                            })
                        })
                        .columns(columns.iter().cloned())
                        .row_height(31.5)
                        .visible_rows(VISIBLE_ROWS)
                        .on_sort(|_, _, _, _| {})
                        .on_resize(|_, _, _, _| {})
                        .on_edit_request(|_, _, _, _| {})
                        .footer_cell("field-0", "Summary")
                        .scroll_to_cell(target.get(), format!("field-{}", target.get())),
                    )
                    .into_any_element()
            }
        },
    );
    let mut positions = Vec::new();
    for destination in [0, 703, column_count - 1] {
        target.set(destination);
        harness.frame();
        harness.frame();
        rows.set(0);
        cells.set(0);
        begin_allocation_measurement();
        harness.frame();
        let heap = end_allocation_measurement();
        let snapshot = harness.current_snapshot();
        let mounted_rows = snapshot
            .nodes
            .iter()
            .filter(|node| node.role == Role::Row)
            .count();
        let sample = PerformanceSample::new(harness.frame_stats())
            .heap_allocations(heap)
            .mounted_items(mounted_rows as u64)
            .builder_calls(rows.get());
        let report = budget("wide-data-grid")
            .enforce(sample)
            .map_err(|error| anyhow::anyhow!(error))?;
        if cells.get() > 192 {
            bail!("wide-grid built {} cells (limit 192)", cells.get());
        }
        let mut report = serde_json::to_value(report)?;
        report["cell_builder_calls"] = cells.get().into();
        report["cell_builder_limit"] = 192.into();
        report["destination"] = destination.into();
        report["dataset_rows"] = DATASET_ITEMS.into();
        report["dataset_columns"] = column_count.into();
        report["rtl"] = rtl.into();
        report["caller_column_conversions"] = column_count.into();
        report["caller_eager_cell_conversions"] = 0.into();
        positions.push(report);
    }
    Ok(positions)
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
