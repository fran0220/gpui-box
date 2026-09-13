//! V7 CPU/structural evidence using the real Cartesian element and GPUI input.
//! Harness executes layout/prepaint/paint, not a GPU benchmark. Timings are
//! advisory. New-data projection, hit construction and motion still scan all n;
//! a sparse viewport bounds semantic mounting, not total work.

use super::*;
use gpui::{Modifiers, MouseMoveEvent, ScrollDelta, ScrollWheelEvent, TouchPhase, point, px};
use gpui_kit::display::chart::{
    ChartSelection,
    cartesian::{CartesianChart, CartesianEvent},
    data::{ChartScale, ChartValue, RawPoint, RawSeries, SeriesMark, ValueAxis, project},
    scale::{NumericScale, ScaleKind},
};
use std::time::Instant;

const ID: &str = "perf.cartesian";
const SERIES: &str = "observations";
const VISIBLE: usize = 24;

// Identity is the original numeric x, deliberately unrelated to vector offset.
fn raw_x(key: usize) -> f64 {
    10_000. + key as f64 * 10.
}

fn observation(key: usize, target: usize) -> RawPoint {
    let x = raw_x(key);
    let y = if key == target {
        91.25
    } else {
        20. + (key % 37) as f64
    };
    RawPoint::new(format!("x-{x}"), ChartValue::Number(x), Some(y))
        .text(format!("Observation at {x}"), format!("{y} exact units"))
}

fn measured<T>(operation: impl FnOnce() -> T) -> (T, serde_json::Value) {
    begin_allocation_measurement();
    let start = Instant::now();
    let result = operation();
    let elapsed = start.elapsed();
    let allocations = end_allocation_measurement();
    let bytes = HEAP_REQUESTED_BYTES.load(Ordering::Acquire);
    (
        result,
        serde_json::json!({
            "elapsed_ns_advisory": elapsed.as_nanos() as u64,
            "heap_allocations": allocations, "heap_requested_bytes": bytes,
        }),
    )
}

fn frame_report(
    harness: &mut Harness,
    mut measurement: serde_json::Value,
    builds: u64,
) -> serde_json::Value {
    let snapshot = harness.current_snapshot();
    let mounted = snapshot
        .nodes
        .iter()
        .filter(|node| {
            node.id
                .starts_with("perf.cartesian.series.observations.point.")
        })
        .count();
    measurement["sample"] = serde_json::to_value(
        PerformanceSample::new(harness.frame_stats())
            .heap_allocations(
                measurement["heap_allocations"]
                    .as_u64()
                    .expect("measured allocations"),
            )
            .mounted_items(mounted as u64)
            .builder_calls(builds),
    )
    .expect("serializable structural counters");
    measurement["frame_scope"] =
        "last completed frame; allocation/time cover the whole named operation".into();
    measurement
}

pub(super) fn run() -> Result<Vec<serde_json::Value>> {
    let mut reports = Vec::new();
    for (items, dense) in [
        (1_000, false),
        (10_000, false),
        (100_000, false),
        (1_000, true),
    ] {
        for animate in [false, true] {
            eprintln!("measuring Cartesian chart: {items}, full_domain={dense}, animate={animate}");
            reports.extend(run_case(items, dense, animate)?);
        }
    }
    Ok(reports)
}

fn run_case(items: usize, dense: bool, animate: bool) -> Result<Vec<serde_json::Value>> {
    let first = items / 2;
    let target = first + 11;
    let domain = if dense {
        [raw_x(0) - 5., raw_x(items - 1) + 5.]
    } else {
        [raw_x(first) - 5., raw_x(first + VISIBLE - 1) + 5.]
    };
    let scale = NumericScale::new(ScaleKind::Linear, domain).expect("finite fixture domain");
    let axes = vec![ValueAxis {
        id: "units".into(),
        label: "Units".into(),
        scale: NumericScale::new(ScaleKind::Linear, [0., 100.]).expect("finite value domain"),
    }];
    // Reverse storage order to catch accidental source-index identities. Input
    // construction is reported independently, never hidden in projection time.
    let (series, input_measurement) = measured(|| {
        RawSeries::new(SERIES, "units", SeriesMark::Scatter)
            .points((0..items).rev().map(|key| observation(key, target)))
    });
    let (projected, mut projection_measurement) = measured(|| {
        project(
            std::slice::from_ref(&series),
            &ChartScale::Numeric(scale),
            &axes,
            &[],
        )
    });
    let projected = projected.map_err(|error| anyhow::anyhow!("projection: {error:?}"))?;
    anyhow::ensure!(
        projected[0].points.len() == items,
        "projection lost raw inputs"
    );
    for p in projected[0].points.iter().flatten() {
        let raw = &series.points[p.source];
        anyhow::ensure!(
            ChartScale::Numeric(scale).map(&raw.x) == Some(p.x)
                && axes[0].scale.map(raw.y.expect("present value")) == Some(p.y),
            "projection changed source correspondence"
        );
    }
    projection_measurement["projected_points"] = items.into();
    projection_measurement["input_points_offered"] = items.into();
    drop(projected);
    let series = Rc::new(RefCell::new(Rc::new(vec![series])));
    let viewport = Rc::new(Cell::new(scale));
    let selected = Rc::new(RefCell::new(None));
    let hovered = Rc::new(RefCell::new(None));
    let events = Rc::new(RefCell::new(Vec::new()));
    let builds = Rc::new(Cell::new(0u64));
    let mut cx = TestAppContext::single();
    let mut harness = Harness::new(&mut cx, gpui_kit::install, |_, _| div().into_any_element());
    let build = {
        let (series, viewport, selected, hovered, events, builds) = (
            series.clone(),
            viewport.clone(),
            selected.clone(),
            hovered.clone(),
            events.clone(),
            builds.clone(),
        );
        move |_: &mut gpui::Window, _: &mut gpui::App| {
            builds.set(builds.get() + 1);
            let events = events.clone();
            div()
                .w(px(800.))
                .child(
                    CartesianChart::new(
                        ID,
                        "Deterministic raw observations",
                        ChartScale::Numeric(viewport.get()),
                        axes.clone(),
                    )
                    .shared_series(series.borrow().clone())
                    .animate(animate)
                    .selected(selected.borrow().clone())
                    .hovered(hovered.borrow().clone())
                    .height(240.)
                    .on_event(move |event, _, _| events.borrow_mut().push(event)),
                )
                .into_any_element()
        }
    };
    let (_, mount) = measured(|| harness.remount(build));
    let mut phases = vec![
        ("input-preparation", input_measurement),
        ("projection", projection_measurement),
        ("mount", frame_report(&mut harness, mount, builds.get())),
    ];
    harness.frame();
    harness.frame();
    builds.set(0);
    let (_, settled) = measured(|| harness.frame());
    phases.push((
        "settled-redraw",
        frame_report(&mut harness, settled, builds.get()),
    ));
    let mounted = phases.last().expect("settled").1["sample"]["mounted_items"]
        .as_u64()
        .expect("mounted count");
    anyhow::ensure!(
        mounted == if dense { items as u64 } else { VISIBLE as u64 },
        "unexpected visible marks: {mounted}"
    );
    if !dense {
        let allocations = phases.last().expect("settled").1["heap_allocations"]
            .as_u64()
            .expect("allocation count");
        anyhow::ensure!(
            allocations <= 2_500,
            "sparse chart cloned/formatted full input on redraw: {allocations} allocations"
        );
    }

    // Appending a new original x outside this viewport keeps the sparse mounted
    // count fixed while forcing the current public API to process n+1 inputs.
    builds.set(0);
    let (_, append) = measured(|| {
        Rc::make_mut(&mut series.borrow_mut())[0]
            .points
            .push(observation(items, target));
        harness.frame();
    });
    phases.push(("append", frame_report(&mut harness, append, builds.get())));
    let expected_raw = observation(target, target);
    let expected = ChartSelection::new(SERIES, expected_raw.id.clone());
    let mark_id = format!("{ID}.series.{SERIES}.point.{}", expected_raw.id);
    let mark = harness
        .current_snapshot()
        .find(&mark_id)
        .cloned()
        .context("original mark mounted")?;
    anyhow::ensure!(
        mark.value.as_deref() == Some(expected_raw.formatted.as_ref()),
        "original value changed"
    );
    let center = mark.bounds.center();
    let at = point(px(center.0), px(center.1));
    events.borrow_mut().clear();
    builds.set(0);
    let (_, click) = measured(|| {
        harness.context().simulate_click(at, Modifiers::none());
        harness.context().run_until_parked();
    });
    anyhow::ensure!(
        events
            .borrow()
            .contains(&CartesianEvent::Select(Some(expected.clone()))),
        "click did not report exact original identity"
    );
    phases.push((
        "selection-input",
        frame_report(&mut harness, click, builds.get()),
    ));
    *selected.borrow_mut() = Some(expected.clone());
    builds.set(0);
    let (_, redraw) = measured(|| harness.frame());
    phases.push((
        "selected-redraw",
        frame_report(&mut harness, redraw, builds.get()),
    ));
    let snapshot = harness.current_snapshot();
    let mark = snapshot.find(&mark_id).context("selected original mark")?;
    anyhow::ensure!(
        mark.selected && mark.value.as_deref() == Some(expected_raw.formatted.as_ref()),
        "selection lost exact value"
    );

    events.borrow_mut().clear();
    builds.set(0);
    let (_, hover) = measured(|| {
        harness.context().simulate_event(MouseMoveEvent {
            position: at,
            ..Default::default()
        });
        harness.context().run_until_parked();
    });
    anyhow::ensure!(
        events
            .borrow()
            .contains(&CartesianEvent::Hover(Some(expected.clone()))),
        "hover did not report exact original identity"
    );
    phases.push((
        "hover-input",
        frame_report(&mut harness, hover, builds.get()),
    ));
    *hovered.borrow_mut() = Some(expected);
    builds.set(0);
    let (_, redraw) = measured(|| harness.frame());
    phases.push((
        "hovered-redraw",
        frame_report(&mut harness, redraw, builds.get()),
    ));
    anyhow::ensure!(
        harness
            .current_snapshot()
            .find(&mark_id)
            .and_then(|n| n.value.as_deref())
            == Some(expected_raw.formatted.as_ref()),
        "hover lost raw value"
    );

    let plot = harness
        .bounds("perf.cartesian.plot")
        .context("plot bounds")?;
    let anchor = f64::from(f32::from(plot.center().x - plot.origin.x) / f32::from(plot.size.width));
    let expected_viewport = scale
        .zoom(anchor, (-30_f64 / 300.).exp())
        .expect("valid zoom");
    events.borrow_mut().clear();
    builds.set(0);
    let (_, change) = measured(|| {
        harness.context().simulate_event(ScrollWheelEvent {
            position: plot.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(30.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
        harness.context().run_until_parked();
    });
    anyhow::ensure!(
        events
            .borrow()
            .contains(&CartesianEvent::Viewport(expected_viewport)),
        "wrong viewport proposal"
    );
    anyhow::ensure!(viewport.get() == scale, "chart mutated caller viewport");
    phases.push((
        "viewport-input",
        frame_report(&mut harness, change, builds.get()),
    ));
    viewport.set(expected_viewport);
    builds.set(0);
    let (_, redraw) = measured(|| harness.frame());
    phases.push((
        "viewport-change",
        frame_report(&mut harness, redraw, builds.get()),
    ));
    let snapshot = harness.current_snapshot();
    let moved = snapshot
        .find(&mark_id)
        .context("original mark after zoom")?;
    let plot = snapshot
        .find("perf.cartesian.plot")
        .context("zoomed plot")?;
    let expected_center = plot.bounds.x
        + expected_viewport.map(raw_x(target)).expect("target maps") as f32 * plot.bounds.width;
    anyhow::ensure!(
        (moved.bounds.center().0 - expected_center).abs() <= 1.
            && moved.value.as_deref() == Some(expected_raw.formatted.as_ref()),
        "accepted viewport did not render original data at proposed coordinates"
    );

    Ok(phases.into_iter().map(|(phase, mut report)| {
        report["name"] = "cartesian-chart".into();
        report["phase"] = phase.into();
        report["dataset_items"] = items.into();
        report["full_domain"] = dense.into();
        report["animate"] = animate.into();
        report["shared_input"] = true.into();
        report["raw_items_after_append"] = (items + 1).into();
        let offered = if matches!(phase, "input-preparation" | "projection" | "mount" | "settled-redraw") { items } else { items + 1 };
        report["raw_items_in_phase"] = offered.into();
        report["caller_cloned_points"] = if phase == "append" { items } else { 0 }.into();
        report["total_work_bounded"] = false.into();
        report["evidence_scope"] = "Harness CPU layout/prepaint/paint and real input dispatch; not GPU/FPS evidence; shared immutable input reuses projection, but hit construction and enabled motion still scan all inputs".into();
        report["verified_original"] = serde_json::json!({"series": SERIES, "id": expected_raw.id.as_ref(), "x": raw_x(target), "y": expected_raw.y, "formatted": expected_raw.formatted.as_ref()});
        report["verified_viewport_proposal"] = serde_json::json!(expected_viewport.domain());
        report
    }).collect())
}

/// Explicit fast selector; the default entry point still runs every old budget.
pub(super) fn selected_run() -> Result<bool> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) != Some("--charts") {
        return Ok(false);
    }
    let output = match args.as_slice() {
        [_] => PathBuf::from("target/performance/charts.json"),
        [_, flag, path] if flag == "--output" => PathBuf::from(path),
        _ => bail!("usage: gpui-box-performance --charts [--output <report.json>]"),
    };
    let document = serde_json::json!({"schema_version": 3, "selector": "charts", "dataset_sizes": [1000, 10000, 100000], "reports": run()?});
    let encoded = serde_json::to_string_pretty(&document)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, format!("{encoded}\n"))?;
    println!("{encoded}");
    eprintln!("chart performance report written to {}", output.display());
    Ok(true)
}
