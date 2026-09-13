//! Validated conserved flow layouts with persistent node/link labels.
use super::support::*;
use crate::display::plot::{SankeyAlignment, SankeyOrder};

pub(super) fn sankey_layout(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let graph = SankeyData::new(
        [
            ("a", "Incoming A", "70"),
            ("b", "Incoming B", "30"),
            ("c", "Destination C", "30"),
            ("d", "Destination D", "70"),
        ]
        .map(|(id, label, value)| SankeyNode::new(id, label, value, Default::default())),
        [
            ("ad", "a", "d", "A → D", "70"),
            ("bc", "b", "c", "B → C", "30"),
        ]
        .map(|(id, source, target, label, value)| {
            SankeyLink::new(
                id,
                source,
                target,
                label,
                value,
                Default::default(),
                Default::default(),
                0.0,
            )
        }),
    );
    stack(&theme)
        .w(px(860.0))
        .child(caption(
            &theme,
            "Synthetic conserved flows · same 70:30 scale · deterministic crossing reduction",
        ))
        .child(
            div().row().gap_token(&theme, Space::Lg).children(
                [
                    (SankeyOrder::Input, "input", "Input order"),
                    (SankeyOrder::Barycenter, "ordered", "Barycenter order"),
                ]
                .map(|(order, id, label)| {
                    let (data, _) = graph
                        .clone()
                        .layout_ordered(&[70.0, 30.0], 0.08, 0.1, SankeyAlignment::Left, order)
                        .expect("valid flows");
                    div().w(px(380.0)).child(
                        SankeyChart::new(
                            Ident::new("scene.sankey").child(id),
                            label,
                            PlotState::Ready(data),
                        )
                        .labels(true)
                        .on_current(|_, _, _| {}),
                    )
                }),
            ),
        )
        .into_any_element()
}

pub(super) fn raw_candlestick(_window: &mut Window, cx: &mut App) -> AnyElement {
    use crate::display::chart::{
        cartesian::CartesianChart,
        data::{ChartScale, ChartValue, RawPoint, RawSeries, SeriesMark, ValueAxis},
        scale::{NumericScale, ScaleKind},
    };
    use crate::display::plot::RawOhlc;
    let theme = cx.theme().clone();
    let day = 86_400_000.0;
    let x = ChartScale::Numeric(
        NumericScale::new(ScaleKind::Time, [0.0, 4.0 * day]).expect("time domain"),
    );
    let candles = RawOhlc::series(
        "ohlc",
        "price",
        [
            RawOhlc::new(
                "day-a",
                day,
                [30.0, 60.0, 20.0, 50.0],
                "Day A",
                "O30 H60 L20 C50",
            ),
            RawOhlc::new(
                "day-b",
                day * 2.0,
                [65.0, 75.0, 35.0, 40.0],
                "Day B",
                "O65 H75 L35 C40",
            ),
            RawOhlc::new(
                "day-c",
                day * 3.0,
                [45.0, 55.0, 30.0, 45.0],
                "Day C · doji",
                "O45 H55 L30 C45",
            ),
        ],
        theme.colors.success,
        theme.colors.danger,
    )
    .expect("validated OHLC");
    let volume = RawSeries::new("volume", "volume", SeriesMark::Bar)
        .tint(theme.colors.accent.opacity(0.22))
        .points(
            [
                ("day-a", day, 120.0),
                ("day-b", 2.0 * day, 260.0),
                ("day-c", 3.0 * day, 80.0),
            ]
            .map(|(id, x, value)| RawPoint::new(id, ChartValue::Number(x), Some(value))),
        );
    let overlay = RawSeries::new("reference", "price", SeriesMark::Line)
        .tint(theme.colors.warning)
        .points(
            [
                ("day-a", day, 42.0),
                ("day-b", 2.0 * day, 48.0),
                ("day-c", 3.0 * day, 46.0),
            ]
            .map(|(id, x, value)| RawPoint::new(id, ChartValue::Number(x), Some(value))),
        );
    stack(&theme).w(px(900.0)).child(caption(&theme, "Synthetic OHLC · shared UTC-duration x scale · independent volume axis · caller overlay"))
        .child(CartesianChart::new("scene.raw-ohlc", "OHLC with volume and overlay", x, [
            ValueAxis { id: "price".into(), label: "Price".into(), scale: NumericScale::new(ScaleKind::Linear, [0.0, 80.0]).expect("price domain") },
            ValueAxis { id: "volume".into(), label: "Volume".into(), scale: NumericScale::new(ScaleKind::Linear, [0.0, 300.0]).expect("volume domain") },
        ]).series([volume, candles, overlay]).format_ticks(move |axis, value| if axis == "x" { format!("D{:.0}", value / day).into() } else { value.to_string().into() }).selected(Some(ChartSelection::new("ohlc", "day-b"))).on_event(|_, _, _| {}))
        .into_any_element()
}
