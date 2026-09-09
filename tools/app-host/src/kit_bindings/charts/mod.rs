//! Caller-owned chart data converted into the actual Kit builders.
use super::{Emit, KitSlots, Node, flag, text};
use anyhow::{Result, bail, ensure};
use gpui::{AnyElement, App, Bounds, Hsla, IntoElement, SharedString, Window, point, size};
use gpui_kit::{
    display::{chart::*, plot::*, sparkline::*},
    foundation::slot::Slotted,
};
use serde_json::{Value, json};

pub(super) const COMPONENTS: &[&str] = &[
    "AreaChart",
    "BarChart",
    "CandlestickChart",
    "ChartLegend",
    "GaugeChart",
    "LineChart",
    "PieChart",
    "Plot",
    "RadarChart",
    "SankeyChart",
    "ScatterChart",
    "Sparkline",
    "StackedBarChart",
];

fn string(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_owned()
}
fn scalar(v: &Value, key: &str) -> f32 {
    v[key].as_f64().unwrap_or_default() as f32
}
fn list(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}
fn color(v: &Value) -> Hsla {
    gpui::hsla(
        scalar(v, "h"),
        scalar(v, "s"),
        scalar(v, "l"),
        scalar(v, "a"),
    )
}
fn bounds(v: &Value) -> Bounds<f32> {
    Bounds::new(
        point(scalar(v, "x"), scalar(v, "y")),
        size(scalar(v, "width"), scalar(v, "height")),
    )
}

pub(super) fn series(value: &Value) -> Vec<ChartSeries> {
    list(value)
        .map(|v| {
            let mut series = ChartSeries::new(string(v, "id"), string(v, "label")).points(
                list(&v["points"]).map(|p| {
                    let mut pnt = ChartPoint::new(
                        string(p, "id"),
                        scalar(p, "x"),
                        scalar(p, "y"),
                        string(p, "label"),
                        string(p, "value"),
                    );
                    if p.get("weight").is_some() {
                        pnt = pnt.weight(scalar(p, "weight"));
                    }
                    pnt
                }),
            );
            if let Some(tint) = v.get("tint") {
                series = series.tint(color(tint));
            }
            series
        })
        .collect()
}

fn chart_state(v: &Value) -> ChartState {
    match v["kind"].as_str().unwrap_or_default() {
        "loading" => ChartState::Loading,
        "empty" => ChartState::Empty,
        "unavailable" => ChartState::Unavailable(string(v, "reason").into()),
        "error" => ChartState::Error(string(v, "reason").into()),
        "stale" => ChartState::Stale {
            series: series(&v["data"]),
            reason: string(v, "reason").into(),
        },
        "ready" => ChartState::Ready(series(&v["data"])),
        _ => unreachable!("validated chart state"),
    }
}
fn plot_state<T>(v: &Value, convert: impl FnOnce(&Value) -> T) -> PlotState<T> {
    match v["kind"].as_str().unwrap_or_default() {
        "loading" => PlotState::Loading,
        "empty" => PlotState::Empty,
        "unavailable" => PlotState::Unavailable(string(v, "reason").into()),
        "error" => PlotState::Error(string(v, "reason").into()),
        "stale" => PlotState::Stale {
            data: convert(&v["data"]),
            reason: string(v, "reason").into(),
        },
        "ready" => PlotState::Ready(convert(&v["data"])),
        _ => unreachable!("validated plot state"),
    }
}
fn axes(v: &Value) -> ChartAxes {
    let get = |key| v[key].as_str().map(|s| SharedString::from(s.to_owned()));
    ChartAxes {
        x_label: get("xLabel"),
        y_label: get("yLabel"),
        x_start: get("xStart"),
        x_end: get("xEnd"),
        y_start: get("yStart"),
        y_end: get("yEnd"),
    }
}
fn slotted<T: Slotted>(mut builder: T, slots: &KitSlots) -> T {
    for &name in T::SLOTS {
        if let Some(factory) = slots.get(name).cloned() {
            builder = builder.slot(name, move |window, cx| factory(window, cx));
        }
    }
    builder
}
fn candles(v: &Value) -> Vec<Candlestick> {
    list(v)
        .map(|c| {
            Candlestick::new(
                string(c, "id"),
                scalar(c, "x"),
                scalar(c, "open"),
                scalar(c, "high"),
                scalar(c, "low"),
                scalar(c, "close"),
                string(c, "label"),
                string(c, "value"),
            )
        })
        .collect()
}
fn marks(v: &Value) -> Vec<PlotMark> {
    list(v)
        .map(|m| {
            PlotMark::new(
                string(m, "id"),
                string(m, "label"),
                string(m, "value"),
                bounds(&m["bounds"]),
            )
        })
        .collect()
}
fn sankey(v: &Value) -> SankeyData {
    SankeyData::new(
        list(&v["nodes"]).map(|n| {
            let mut node = SankeyNode::new(
                string(n, "id"),
                string(n, "label"),
                string(n, "value"),
                bounds(&n["bounds"]),
            );
            if let Some(tint) = n.get("tint") {
                node = node.tint(color(tint));
            }
            node
        }),
        list(&v["links"]).map(|l| {
            let mut link = SankeyLink::new(
                string(l, "id"),
                string(l, "source"),
                string(l, "target"),
                string(l, "label"),
                string(l, "value"),
                point(scalar(&l["start"], "x"), scalar(&l["start"], "y")),
                point(scalar(&l["end"], "x"), scalar(&l["end"], "y")),
                scalar(l, "startWidth"),
            )
            .widths(scalar(l, "startWidth"), scalar(l, "endWidth"));
            if let Some(tint) = l.get("tint") {
                link = link.tint(color(tint));
            }
            link
        }),
    )
}
fn reading(v: &Value) -> SparklineReading {
    SparklineReading::new(
        list(&v["points"]).map(|p| SparklinePoint::new(scalar(p, "x"), scalar(p, "y"))),
        string(v, "current"),
        string(v, "minimum"),
        string(v, "maximum"),
    )
}

/// Additional native invariants after the host's closed field validation.
pub(super) fn validate(node: &Node) -> Result<()> {
    if let Some(paint) = node.props.get("paint") {
        for op in list(paint) {
            if op["kind"] == "rect" {
                ensure!(
                    op.get("bounds").is_some()
                        && op.get("points").is_none()
                        && op.get("width").is_none(),
                    "rectangle paint payload mismatch"
                );
                ensure!(
                    PlotMark::new("paint", "", "", bounds(&op["bounds"])).is_bounded(),
                    "invalid paint bounds"
                );
            } else {
                ensure!(
                    op.get("bounds").is_none()
                        && list(&op["points"]).count()
                            >= if op["kind"] == "polygon" { 3 } else { 2 }
                        && (op.get("width").is_some() == (op["kind"] == "line")),
                    "path paint payload mismatch"
                );
            }
        }
    }
    let Some(state) = node.props.get("state") else {
        return Ok(());
    };
    let kind = state["kind"].as_str().unwrap_or_default();
    ensure!(
        state.get("data").is_some() == matches!(kind, "ready" | "stale"),
        "state data mismatch"
    );
    ensure!(
        state.get("reason").is_some() == matches!(kind, "unavailable" | "error" | "stale"),
        "state reason mismatch"
    );
    if !matches!(kind, "ready" | "stale") {
        return Ok(());
    }
    match node.component.as_deref() {
        Some("CandlestickChart") => ensure!(
            candles(&state["data"]).iter().all(Candlestick::is_bounded),
            "invalid OHLC range"
        ),
        Some("Plot") => ensure!(
            marks(&state["data"]).iter().all(PlotMark::is_bounded),
            "invalid normalized plot bounds"
        ),
        Some("SankeyChart") => {
            let data = sankey(&state["data"]);
            for n in &data.nodes {
                ensure!(
                    PlotMark::new(n.id.clone(), n.label.clone(), n.value.clone(), n.bounds)
                        .is_bounded(),
                    "invalid Sankey node bounds"
                );
            }
            for l in &data.links {
                ensure!(
                    data.nodes.iter().any(|n| n.id == l.source)
                        && data.nodes.iter().any(|n| n.id == l.target),
                    "unknown Sankey endpoint"
                );
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn render(
    node: &Node,
    slots: KitSlots,
    _window: &mut Window,
    _cx: &mut App,
    emit: Emit,
) -> AnyElement {
    let id = node.id.clone();
    let label = text(node, "label");
    let state = node.props.get("state").unwrap_or(&Value::Null);
    let current = node.props.get("current");
    let action = (!flag(node, "disabled"))
        .then(|| node.events.get("current").cloned())
        .flatten();
    let axes = axes(node.props.get("axes").unwrap_or(&Value::Null));
    macro_rules! interactive {
        ($builder:expr) => {{
            let mut builder = $builder;
            if flag(node,"crosshair") { builder = builder.crosshair(); }
            if let Some(current) = current { builder = builder.current(string(current,"seriesId"),string(current,"pointId")); }
            if let Some(action) = action { builder = builder.on_current(move |selection,_,_| emit(&action,json!({"seriesId":selection.series_id.as_ref(),"pointId":selection.point_id.as_ref()}))); }
            slotted(builder,&slots).into_any_element()
        }};
    }
    match node.component.as_deref().unwrap_or_default() {
        "BarChart" => slotted(
            BarChart::new(id, label, chart_state(state)).axes(axes),
            &slots,
        )
        .into_any_element(),
        "StackedBarChart" => slotted(
            StackedBarChart::new(id, label, chart_state(state)).axes(axes),
            &slots,
        )
        .into_any_element(),
        "LineChart" => {
            let mut builder = LineChart::new(id, label, chart_state(state)).axes(axes);
            if flag(node, "area") {
                builder = builder.area();
            }
            if flag(node, "smooth") {
                builder = builder.smooth();
            }
            interactive!(builder)
        }
        "AreaChart" => {
            let mut builder = AreaChart::new(id, label, chart_state(state)).axes(axes);
            if flag(node, "polyline") {
                builder = builder.polyline();
            }
            interactive!(builder)
        }
        "ScatterChart" => interactive!(ScatterChart::new(id, label, chart_state(state)).axes(axes)),
        "PieChart" => {
            let mut builder = PieChart::new(id, label, chart_state(state));
            if flag(node, "donut") {
                builder = builder.donut();
            }
            slotted(builder, &slots).into_any_element()
        }
        "RadarChart" => {
            slotted(RadarChart::new(id, label, chart_state(state)), &slots).into_any_element()
        }
        "GaugeChart" => {
            slotted(GaugeChart::new(id, label, chart_state(state)), &slots).into_any_element()
        }
        "ChartLegend" => {
            let mut builder =
                ChartLegend::new(id, series(node.props.get("series").unwrap_or(&Value::Null)))
                    .hidden(
                        list(node.props.get("hidden").unwrap_or(&Value::Null))
                            .filter_map(Value::as_str)
                            .map(str::to_owned),
                    );
            if !flag(node, "disabled")
                && let Some(action) = node.events.get("toggle").cloned()
            {
                builder = builder.on_toggle(move |id, hidden, _, _| {
                    emit(&action, json!({"id":id.as_ref(),"hidden":hidden}))
                });
            }
            builder.into_any_element()
        }
        "CandlestickChart" => {
            let mut builder = CandlestickChart::new(id, label, plot_state(state, candles));
            if let Some(width) = node.props.get("bodyWidth").and_then(Value::as_f64) {
                builder = builder.body_width(width as f32);
            }
            if let Some(tint) = node.props.get("risingTint") {
                builder = builder.rising_tint(color(tint));
            }
            if let Some(tint) = node.props.get("fallingTint") {
                builder = builder.falling_tint(color(tint));
            }
            if let Some(current) = current.and_then(Value::as_str) {
                builder = builder.current(current.to_owned());
            }
            if let Some(action) = action {
                builder = builder.on_current(move |id, _, _| emit(&action, json!(id.as_ref())));
            }
            builder.into_any_element()
        }
        "SankeyChart" => {
            let mut builder = SankeyChart::new(id, label, plot_state(state, sankey));
            if let Some(current) = current.and_then(Value::as_str) {
                builder = builder.current(current.to_owned());
            }
            if let Some(action) = action {
                builder = builder.on_current(move |id, _, _| emit(&action, json!(id.as_ref())));
            }
            builder.into_any_element()
        }
        "Plot" => {
            let mut builder = Plot::new(id, label, plot_state(state, marks));
            if let Some(paint) = node.props.get("paint") {
                let operations = paint.as_array().cloned().unwrap_or_default();
                builder = builder.paint(move |frame, window, _| {
                    for operation in &operations {
                        let tint = color(&operation["tint"]);
                        if operation["kind"] == "rect" {
                            window.paint_quad(gpui::fill(
                                frame.mark_bounds(bounds(&operation["bounds"])),
                                tint,
                            ));
                        } else {
                            let filled = operation["kind"] == "polygon";
                            let mut path = if filled {
                                gpui::PathBuilder::fill()
                            } else {
                                gpui::PathBuilder::stroke(gpui::px(scalar(operation, "width")))
                            };
                            for (index, p) in list(&operation["points"]).enumerate() {
                                let p = frame.point(point(scalar(p, "x"), scalar(p, "y")));
                                if index == 0 {
                                    path.move_to(p);
                                } else {
                                    path.line_to(p);
                                }
                            }
                            if filled {
                                path.close();
                            }
                            if let Ok(path) = path.build() {
                                window.paint_path(path, tint);
                            }
                        }
                    }
                });
            }
            if let Some(current) = current.and_then(Value::as_str) {
                builder = builder.current(current.to_owned());
            }
            if let Some(action) = action {
                builder = builder.on_current(move |id, _, _| emit(&action, json!(id.as_ref())));
            }
            builder.into_any_element()
        }
        "Sparkline" => {
            let state = match plot_state(state, reading) {
                PlotState::Ready(data) => SparklineState::Ready(data),
                PlotState::Loading => SparklineState::Loading,
                PlotState::Empty => SparklineState::Empty,
                PlotState::Unavailable(reason) => SparklineState::Unavailable(reason),
                PlotState::Error(reason) => SparklineState::Error(reason),
                PlotState::Stale { data, reason } => SparklineState::Stale {
                    reading: data,
                    reason,
                },
            };
            let mut builder = Sparkline::new(id, label, state).stale(flag(node, "stale"));
            if let Some(tint) = node.props.get("tint") {
                builder = builder.tint(color(tint));
            }
            if flag(node, "embedded") {
                builder = builder.embedded();
            }
            slotted(builder, &slots).into_any_element()
        }
        _ => unreachable!("chart dispatch membership"),
    }
}

/// Native data/layout queries, not cached wire echoes. No chart owns commands.
pub(super) fn invoke(
    node: &Node,
    method: &str,
    args: &Value,
    query: bool,
    _window: &mut Window,
    _cx: &mut App,
) -> Result<Value> {
    ensure!(query, "charts have no native commands");
    match (node.component.as_deref(), method) {
        (Some("Sparkline"), "published_points") => {
            let state = &node.props["state"];
            ensure!(
                matches!(state["kind"].as_str(), Some("ready" | "stale")),
                "Sparkline has no verified reading"
            );
            Ok(json!(reading(&state["data"]).published_points()))
        }
        (Some("SankeyChart"), "layout") => {
            let alignment = match args["alignment"].as_str() {
                Some("right") => SankeyAlignment::Right,
                Some("justify") => SankeyAlignment::Justify,
                _ => SankeyAlignment::Left,
            };
            let weights = list(&args["weights"])
                .map(|v| v.as_f64().unwrap_or_default())
                .collect::<Vec<_>>();
            let (data, scale) = sankey(&args["data"])
                .layout(
                    &weights,
                    scalar(args, "nodeWidth"),
                    scalar(args, "gap"),
                    alignment,
                )
                .map_err(|e| anyhow::anyhow!("Sankey layout refused: {e:?}"))?;
            Ok(
                json!({"scale":scale,"nodes":data.nodes.iter().map(|n| json!({"id":n.id.as_ref(),"bounds":{"x":n.bounds.origin.x,"y":n.bounds.origin.y,"width":n.bounds.size.width,"height":n.bounds.size.height}})).collect::<Vec<_>>(),"links":data.links.iter().map(|l|json!({"id":l.id.as_ref(),"start":{"x":l.start.x,"y":l.start.y},"end":{"x":l.end.x,"y":l.end.y},"startWidth":l.start_width,"endWidth":l.end_width})).collect::<Vec<_>>()}),
            )
        }
        _ => bail!("unsupported chart query"),
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests;
