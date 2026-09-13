//! Original asymmetric raw-data fixtures, not production telemetry.
use super::support::*;
use crate::display::chart::ChartSelection;
use crate::display::chart::cartesian::{
    AxisSide, CartesianChart, CartesianEvent, ChartOrientation, ChartReference, ChartTick,
    CustomMark, PathSampling, TooltipMode,
};
use crate::display::chart::data::*;
use crate::display::chart::scale::*;

pub(super) fn cartesian(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let scale =
        |domain| NumericScale::new(ScaleKind::Linear, domain).expect("finite fixture domain");
    let axis = |id: &str, label: &str, domain| ValueAxis {
        id: id.to_string().into(),
        label: label.to_string().into(),
        scale: scale(domain),
    };
    let points = |values: &[Option<f64>]| {
        [
            ("intake", 0.),
            ("validation", 1.),
            ("routing", 2.),
            ("processing", 3.),
            ("delivery", 4.),
        ]
        .into_iter()
        .zip(values)
        .map(|((id, x), y)| RawPoint::new(id, ChartValue::Number(x), *y))
        .collect::<Vec<_>>()
    };
    let mixed = CartesianChart::new(
        "scene.cartesian.mixed",
        "Mixed units · error intervals",
        ChartScale::Numeric(scale([-0.5, 4.5])),
        [
            axis("count", "Requests", [-20., 80.]),
            axis("latency", "Latency (ms)", [0., 400.]),
        ],
    )
    .x_ticks(
        [
            (0., "Intake"),
            (1., "Validate"),
            (2., "Route"),
            (3., "Process"),
            (4., "Deliver"),
        ]
        .into_iter()
        .map(|(value, label)| ChartTick {
            value: ChartValue::Number(value),
            label: label.into(),
        }),
    )
    .expect("caller process ticks in domain")
    .series([
        RawSeries::new("requests", "count", SeriesMark::Bar).points(points(&[
            Some(31.),
            Some(-12.),
            Some(64.),
            Some(48.),
            Some(23.),
        ])),
        RawSeries::new("latency", "latency", SeriesMark::Line)
            .curve(Curve::Monotone)
            .points(points(&[
                Some(80.),
                Some(230.),
                Some(160.),
                Some(310.),
                Some(280.),
            ])),
        RawSeries::new("observed", "count", SeriesMark::Scatter).points([RawPoint::new(
            "audit",
            ChartValue::Number(2.2),
            Some(55.),
        )
        .error([44., 69.])
        .text("Audit", "55 verified")]),
    ])
    .references([ChartReference {
        id: "capacity".into(),
        axis: "count".into(),
        range: [60., 70.],
        label: "Capacity band".into(),
        color: None,
    }])
    .height(180.)
    .selected(Some(ChartSelection::new("observed", "audit")))
    .tooltip(TooltipMode::Point);
    let categories = CategoryScale::new(["West district", "East district", "North district"])
        .expect("distinct fixture categories");
    let categorical =
        |id: &str, values: [f64; 3], percent| {
            RawSeries::new(id, "amount", SeriesMark::Bar)
                .stack(if percent {
                    Stack::Percent("group".into())
                } else {
                    Stack::Absolute("group".into())
                })
                .points(categories.categories().iter().zip(values).map(|(x, y)| {
                    RawPoint::new(x.clone(), ChartValue::Category(x.clone()), Some(y))
                }))
        };
    let stacks = CartesianChart::new(
        "scene.cartesian.stacks",
        "Diverging stacks · independent sign totals",
        ChartScale::Category(categories.clone()),
        [axis("amount", "Amount", [-50., 100.])],
    )
    .series([
        categorical("First", [37., -23., 52.], false),
        categorical("Second", [21., -17., 33.], false),
    ])
    .height(180.);
    let gaps = CartesianChart::new(
        "scene.cartesian.gaps",
        "Missing is not zero · retained stale data",
        ChartScale::Numeric(scale([0., 4.])),
        [axis("value", "Reading", [-10., 50.])],
    )
    .series([
        RawSeries::new("area", "value", SeriesMark::Area)
            .curve(Curve::StepAfter)
            .points(points(&[Some(12.), Some(36.), None, Some(-6.), Some(21.)])),
        RawSeries::new("line", "value", SeriesMark::Line)
            .curve(Curve::StepBefore)
            .points(points(&[Some(5.), Some(20.), None, Some(13.), Some(30.)])),
    ])
    .stale("Fixture refresh refused")
    .height(150.);
    let percent = CartesianChart::new(
        "scene.cartesian.percent",
        "Percent stacks · exact raw readout",
        ChartScale::Category(categories.clone()),
        [axis("amount", "Share (%)", [-100., 100.])],
    )
    .series([
        categorical("First", [37., -23., 52.], true),
        categorical("Second", [21., -17., 33.], true),
    ])
    .height(180.)
    .selected(Some(ChartSelection::new("First", "East district")))
    .tooltip(TooltipMode::SharedAxis);
    stack(&theme).w(px(920.)).child(caption(&theme,"Original fixtures · raw f64 data; shared coordinates, category identity and exact readouts"))
        .child(div().row().items_start().gap_token(&theme,Space::Lg).child(div().flex_1().min_w_0().child(mixed)).child(div().flex_1().min_w_0().child(stacks)))
        .child(div().row().items_start().gap_token(&theme,Space::Lg).child(div().flex_1().min_w_0().child(gaps)).child(div().flex_1().min_w_0().child(percent)))
        .into_any_element()
}

pub(super) fn cartesian_layout(window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let changed = crate::motion::keyed::slot::<bool>(
        &"scene.cartesian-layout.readings".into(),
        window.window_handle().window_id(),
        cx,
    );
    let revised = *changed.borrow();
    let mut charts = Vec::new();
    for (id, orientation) in [
        ("vertical", ChartOrientation::Vertical),
        ("horizontal", ChartOrientation::Horizontal),
    ] {
        let axis = |id: &str, domain| ValueAxis {
            id: id.to_owned().into(),
            label: id.to_owned().into(),
            scale: NumericScale::new(ScaleKind::Linear, domain).expect("fixture value domain"),
        };
        let mut points = [("West", 63., 17.), ("East", 29., 74.), ("North", 42., 42.)]
            .into_iter()
            .map(|(id, y, baseline)| {
                let y = if revised { y + 13.125 } else { y };
                RawPoint::new(id, ChartValue::Category(id.into()), Some(y))
                    .baseline(baseline)
                    .error([y.min(baseline) - 9., y.max(baseline) + 7.])
                    .text(id, format!("{baseline} → {y} units"))
            })
            .collect::<Vec<_>>();
        if revised {
            points.reverse();
        }
        let report = changed.clone();
        charts.push(
            div().flex_1().min_w_0().child(
                CartesianChart::new(
                    format!("scene.cartesian-layout.{id}"),
                    format!("{id} · ranges, independent axes"),
                    ChartScale::Category(
                        CategoryScale::new(["West", "East", "North"])
                            .expect("fixture category identities"),
                    ),
                    [axis("Units", [-10., 100.]), axis("Percent", [0., 100.])],
                )
                .orientation(orientation)
                .x_axis_side(if orientation == ChartOrientation::Vertical {
                    AxisSide::Leading
                } else {
                    AxisSide::Trailing
                })
                .axis_side(
                    "Percent",
                    if orientation == ChartOrientation::Vertical {
                        AxisSide::Trailing
                    } else {
                        AxisSide::Leading
                    },
                )
                .expect("fixture axis")
                .axis_ticks(
                    "Units",
                    [-10., 17., 63., 100.].map(|value| ChartTick {
                        value: ChartValue::Number(value),
                        label: format!("{value} u").into(),
                    }),
                )
                .expect("explicit asymmetric value ticks")
                .series([
                    RawSeries::new("Interval", "Units", SeriesMark::Range).points(points),
                    RawSeries::new("Forecast", "Percent", SeriesMark::Scatter).points([
                        RawPoint::new(
                            "East",
                            ChartValue::Category("East".into()),
                            Some(if revised { 71.25 } else { 36.5 }),
                        )
                        .text("East forecast", if revised { "71.25%" } else { "36.5%" }),
                    ]),
                ])
                .custom_marks(|_, _| {
                    Some(
                        CustomMark::new(22., 8., |bounds, color, window, _| {
                            window.paint_quad(gpui::fill(bounds, color))
                        })
                        .expect("positive custom fixture mark"),
                    )
                })
                .selected(Some(ChartSelection::new("Interval", "West")))
                .height(230.)
                .on_event(move |event, window, _| {
                    if matches!(event, CartesianEvent::Select(Some(_))) {
                        let next = !*report.borrow();
                        *report.borrow_mut() = next;
                        window.refresh();
                    }
                }),
            ),
        );
    }
    stack(&theme).w(px(920.)).child(caption(&theme,"Original fixtures · opposite axis lanes, caller ticks and custom glyphs; click a mark to update/reorder raw readings"))
        .child(div().row().items_start().gap_token(&theme,Space::Lg).children(charts)).into_any_element()
}

pub(super) fn cartesian_states(_window: &mut Window, cx: &mut App) -> AnyElement {
    use crate::display::chart::{GaugeChart, PieChart, RadarChart};
    use crate::state::AsyncValue;
    let theme = cx.theme().clone();
    let scale = NumericScale::new(ScaleKind::Linear, [0., 100.]).expect("finite fixture domain");
    let point = |id: &str, y| RawPoint::new(id, ChartValue::Number(0.), Some(y));
    let pie = PieChart::from_raw(
        "scene.raw.pie",
        "Raw shares · 13 / 29 / 7",
        RawSeries::new("shares", "", SeriesMark::Bar).points([
            point("West", 13.),
            point("East", 29.),
            point("North", 7.),
        ]),
    )
    .expect("valid raw pie fixture")
    .donut();
    let axes = ["Speed", "Coverage", "Quality"].map(|id| ValueAxis {
        id: id.into(),
        label: id.into(),
        scale,
    });
    let radar = RadarChart::from_raw(
        "scene.raw.radar",
        "Explicit radial domains",
        [RawSeries::new("score", "", SeriesMark::Line).points([
            point("Quality", 91.),
            point("Speed", 38.),
            point("Coverage", 67.),
        ])],
        &axes,
    )
    .expect("valid raw radar fixture");
    let gauge = GaugeChart::from_raw(
        "scene.raw.gauge",
        "Raw reading · domain 0–100",
        point("verified", 73.125).text("Verified", "73.125"),
        scale,
    )
    .expect("in-domain gauge fixture");
    let states: [(&str, AsyncValue<(), String>); 4] = [
        ("Loading", AsyncValue::loading()),
        ("Empty", AsyncValue::empty()),
        (
            "Unavailable",
            AsyncValue::refused("Fixture capability refused"),
        ),
        ("Error", AsyncValue::error("Fixture input failed".into())),
    ];
    stack(&theme)
        .w(px(920.))
        .child(caption(
            &theme,
            "Raw polar fixtures and truthful non-ready Cartesian states",
        ))
        .child(
            div()
                .row()
                .items_start()
                .gap_token(&theme, Space::Lg)
                .child(div().flex_1().child(pie))
                .child(div().flex_1().child(radar))
                .child(div().flex_1().child(gauge)),
        )
        .child(
            div()
                .row()
                .items_start()
                .flex_wrap()
                .gap_token(&theme, Space::Lg)
                .children(states.into_iter().map(|(label, state)| {
                    div().w(px(430.)).child(
                        CartesianChart::new(
                            format!("scene.raw.state.{label}"),
                            label,
                            ChartScale::Numeric(scale),
                            [ValueAxis {
                                id: "y".into(),
                                label: "Value".into(),
                                scale,
                            }],
                        )
                        .state(state),
                    )
                })),
        )
        .into_any_element()
}

/// The scene owns the linked state exactly as a product would; charts emit
/// requests and never communicate through a chart-global synchronization bus.
pub(super) fn cartesian_linked(window: &mut Window, cx: &mut App) -> AnyElement {
    use crate::display::chart::cartesian::CartesianEvent;
    #[derive(Default)]
    struct Linked {
        scale: Option<NumericScale>,
        selected: Option<ChartSelection>,
        hovered: Option<ChartSelection>,
        hidden: Vec<SharedString>,
    }
    let theme = cx.theme().clone();
    let state = crate::motion::keyed::slot::<Linked>(
        &"scene.cartesian.linked.host".into(),
        window.window_handle().window_id(),
        cx,
    );
    let initial = NumericScale::new(ScaleKind::Time, [1_700_000_000_000., 1_700_000_600_000.])
        .expect("finite fixture time domain");
    let current = state.borrow();
    let x = current.scale.unwrap_or(initial);
    let mut charts = Vec::new();
    for (id, width, mark) in [
        ("overview", 600., SeriesMark::Line),
        ("detail", 280., SeriesMark::Range),
    ] {
        let report = state.clone();
        let samples = [
            (1_700_000_060_000_u64, 12.),
            (1_700_000_180_000, 43.),
            (1_700_000_300_000, 29.),
            (1_700_000_420_000, 67.),
            (1_700_000_540_000, 51.),
        ]
        .into_iter()
        .map(|(timestamp, y)| {
            let p = RawPoint::new(
                format!("reading-{timestamp}"),
                ChartValue::Number(timestamp as f64),
                Some(y),
            );
            if mark == SeriesMark::Range {
                p.baseline(y - 17.).error([y - 23., y + 9.])
            } else {
                p
            }
        });
        charts.push(
            div().w(px(width)).child(
                CartesianChart::new(
                    format!("scene.linked.{id}"),
                    id,
                    ChartScale::Numeric(x),
                    [ValueAxis {
                        id: "y".into(),
                        label: "Reading units".into(),
                        scale: NumericScale::new(ScaleKind::Linear, [-20., 100.])
                            .expect("finite fixture domain"),
                    }],
                )
                .series([RawSeries::new("readings", "y", mark)
                    .points(samples)
                    .curve(Curve::Monotone)])
                .selected(current.selected.clone())
                .hovered(current.hovered.clone())
                .hidden(current.hidden.clone())
                .height(210.)
                .format_ticks(|axis, v| {
                    if axis == "x" {
                        format!("{:.0} s", (v - 1_700_000_000_000.) / 1000.).into()
                    } else {
                        v.to_string().into()
                    }
                })
                .on_event(move |event, window, _| {
                    let mut state = report.borrow_mut();
                    match event {
                        CartesianEvent::Hover(value) => state.hovered = value,
                        CartesianEvent::Select(value) => state.selected = value,
                        CartesianEvent::Viewport(scale) => state.scale = Some(scale),
                        CartesianEvent::Visibility { series, visible } => {
                            state.hidden.retain(|id| id != &series);
                            if !visible {
                                state.hidden.push(series);
                            }
                        }
                        CartesianEvent::Brush([ChartValue::Number(a), ChartValue::Number(b)]) => {
                            state.scale =
                                NumericScale::new(ScaleKind::Time, [a.min(b), a.max(b)]).ok();
                        }
                        CartesianEvent::Reset => state.scale = None,
                        _ => {}
                    }
                    window.refresh();
                }),
            ),
        );
    }
    drop(current);
    stack(&theme).w(px(920.)).child(caption(&theme,"Linked caller-owned time viewport · drag to pan, Shift-drag brush, wheel zoom; arrows select, Home resets"))
        .child(div().row().items_start().gap_token(&theme,Space::Lg).children(charts)).into_any_element()
}

pub(super) fn cartesian_dense(window: &mut Window, cx: &mut App) -> AnyElement {
    #[derive(Default)]
    struct Readings {
        series: Option<Rc<Vec<RawSeries>>>,
        selected: Option<ChartSelection>,
    }
    let theme = cx.theme().clone();
    let state = crate::motion::keyed::slot::<Readings>(
        &"scene.cartesian.dense-input".into(),
        window.window_handle().window_id(),
        cx,
    );
    let mut state_value = state.borrow_mut();
    if state_value.series.is_none() {
        state_value.selected = Some(ChartSelection::new("signal", "time-620000ms"));
    }
    let series = state_value
        .series
        .get_or_insert_with(|| {
            Rc::new(vec![
                RawSeries::new("signal", "value", SeriesMark::Area).points((0..6000).map(
                    |sample| {
                        let time = sample as f64 * 0.5;
                        let value = if (450.0..550.0).contains(&time) {
                            None
                        } else {
                            Some(if time == 620. {
                                73.125
                            } else if time == 1800. {
                                -29.
                            } else {
                                5. + (time / 90.).sin() * 9.
                            })
                        };
                        RawPoint::new(
                            format!("time-{}ms", (time * 1000.) as u64),
                            ChartValue::Number(time),
                            value,
                        )
                        .text(
                            format!("t={time}"),
                            value.map(|v| format!("{v:.3} units")).unwrap_or_default(),
                        )
                    },
                )),
            ])
        })
        .clone();
    let selected = state_value.selected.clone();
    drop(state_value);
    let scale =
        |domain| NumericScale::new(ScaleKind::Linear, domain).expect("finite dense fixture domain");
    let mut charts = Vec::new();
    for (name, label, width, sampling) in [
        (
            "exact",
            "Exact path · 6,000 raw observations",
            550.,
            PathSampling::Exact,
        ),
        (
            "sampled",
            "Pixel extrema · same raw identities",
            290.,
            PathSampling::MinMax,
        ),
    ] {
        let report = state.clone();
        charts.push(
            div().w(px(width)).min_w_0().child(
                CartesianChart::new(
                    format!("scene.cartesian.dense.{name}"),
                    label,
                    ChartScale::Numeric(scale([0., 2999.5])),
                    [ValueAxis {
                        id: "value".into(),
                        label: "Units".into(),
                        scale: scale([-40., 85.]),
                    }],
                )
                .shared_series(series.clone())
                .animate(false)
                .sampling(sampling)
                .height(220.)
                .selected(selected.clone())
                .on_event(move |event, window, _| {
                    if let CartesianEvent::Select(value) = event {
                        report.borrow_mut().selected = value;
                        window.refresh();
                    }
                }),
            ),
        );
    }
    stack(&theme).w(px(920.))
        .child(caption(&theme, "Original fixture · path reduction preserves positive/negative spikes and missing runs; selection and exact values remain caller-owned"))
        .child(div().row().items_start().gap_token(&theme, Space::Lg).children(charts))
        .into_any_element()
}
