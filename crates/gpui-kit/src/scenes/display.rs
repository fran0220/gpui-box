//! Surfaces that report a value without taking one.

use super::support::*;

pub(super) fn badge(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(
            row(&theme)
                .child(Badge::new("Neutral").neutral().id("scene.badge.neutral"))
                .child(Badge::new("Accent").accent().id("scene.badge.accent"))
                .child(Badge::new("Success").success().id("scene.badge.success"))
                .child(Badge::new("Warning").warning().id("scene.badge.warning"))
                .child(Badge::new("Danger").danger().id("scene.badge.danger"))
                .child(Badge::new("Info").info().id("scene.badge.info")),
        )
        // A tint says whose the badge is. The colours come from the theme's
        // own palette rather than from literals, so a retinted document
        // retints these too, and the tone underneath still reports itself.
        .child(
            row(&theme)
                .child(
                    Badge::new("Ada")
                        .tint(identity_tint(&theme, "loader.pink"))
                        .id("scene.badge.tinted-neutral"),
                )
                .child(
                    Badge::new("Grace")
                        .tint(identity_tint(&theme, "loader.orange"))
                        .warning()
                        .id("scene.badge.tinted-warning"),
                ),
        )
        .into_any_element()
}

/// A palette entry as an identity colour, falling back to the accent when a
/// theme has not named that scale.
pub(super) fn identity_tint(theme: &Theme, path: &str) -> gpui::Hsla {
    theme.palette_color(path).unwrap_or(theme.colors.accent)
}

/// Every region, variant and state a card can hold, at the sizes a caller
/// actually builds them: a titled list, the three variants side by side, a
/// card that is one action, and a card holding prose.
pub(super) fn card(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let label = |content: &'static str| {
        div()
            .flex_1()
            .min_w_0()
            .child(crate::foundation::text(&theme, TypeScale::Label, content))
    };
    let caption = |content: &'static str| {
        crate::foundation::text(&theme, TypeScale::Caption, content)
            .text_tone(&theme, TextTone::Muted)
    };
    let column = |card: Card| div().flex_1().min_w_0().child(card);

    stack(&theme)
        .child(
            Card::new()
                .id("scene.card")
                .header(
                    CardHeader::new("Workspace services")
                        .subtitle("Four checked a moment ago")
                        .action(|_window, _cx| {
                            Button::new("scene.card.refresh")
                                .label("Refresh")
                                .secondary()
                                .small()
                                .into_any_element()
                        }),
                )
                .divided(true)
                .child(
                    ListRow::new()
                        .id("scene.card.runtime")
                        .leading(StatusDot::new(Tone::Success))
                        .child(label("Native runtime"))
                        .trailing(Badge::new("Ready").success()),
                )
                .child(
                    ListRow::new()
                        .id("scene.card.catalog")
                        .leading(StatusDot::new(Tone::Warning))
                        .child(label("Model catalog"))
                        .trailing(Badge::new("Stale").warning()),
                )
                .child(
                    ListRow::new()
                        .id("scene.card.index")
                        .selected(true)
                        .leading(StatusDot::new(Tone::Success))
                        .child(label("Search index"))
                        .trailing(Badge::new("Ready").success()),
                )
                .child(
                    ListRow::new()
                        .id("scene.card.telemetry")
                        .disabled(true)
                        .leading(StatusDot::new(Tone::Neutral))
                        .child(label("Telemetry export"))
                        .trailing(Badge::new("Off")),
                )
                .footer(|_window, cx| {
                    let theme = cx.theme().clone();
                    div()
                        .row()
                        .w_full()
                        .gap(px(theme.spacing.sm))
                        .child(
                            Button::new("scene.card.restart")
                                .label("Restart all")
                                .secondary()
                                .small(),
                        )
                        .child(div().flex_1())
                        .child(
                            crate::foundation::text(&theme, TypeScale::Caption, "4 services")
                                .text_tone(&theme, TextTone::Faint),
                        )
                        .into_any_element()
                }),
        )
        .child(
            row(&theme)
                .items_start()
                .child(column(
                    Card::new()
                        .id("scene.card.elevated")
                        .variant(CardVariant::Elevated)
                        .padding(Space::Lg)
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Strong,
                            "Elevated",
                        ))
                        .child(caption("A shadow. One card on a page.")),
                ))
                .child(column(
                    Card::new()
                        .id("scene.card.outlined")
                        .variant(CardVariant::Outlined)
                        .padding(Space::Lg)
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Strong,
                            "Outlined",
                        ))
                        .child(caption("A hairline. A grid of them.")),
                ))
                .child(column(
                    Card::new()
                        .id("scene.card.ghost")
                        .variant(CardVariant::Ghost)
                        .padding(Space::Lg)
                        .child(crate::foundation::text(&theme, TypeScale::Strong, "Ghost"))
                        .child(caption("Neither. Structure without a plane.")),
                )),
        )
        .child(
            row(&theme)
                .items_start()
                .child(column(
                    Card::new()
                        .id("scene.card.actionable")
                        .padding(Space::Lg)
                        .on_click(|_window, _cx| {})
                        .header(CardHeader::new("Open the run").subtitle("The whole card acts"))
                        .child(caption("Enter and space reach it from the keyboard.")),
                ))
                .child(column(
                    Card::new()
                        .id("scene.card.chosen")
                        .padding(Space::Lg)
                        .selected(true)
                        .header(CardHeader::new("Selected").subtitle("A wash and a rail"))
                        .child(caption("Both are drawn inside, so choosing moves nothing.")),
                ))
                .child(column(
                    Card::new()
                        .id("scene.card.unavailable")
                        .padding(Space::Lg)
                        .disabled(true)
                        .on_click(|_window, _cx| {})
                        .header(CardHeader::new("Unavailable").subtitle("Says so, and stays read"))
                        .child(caption("A disabled card installs no handler at all.")),
                )),
        )
        .into_any_element()
}

pub(super) fn status(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(
            row(&theme)
                .child(StatusDot::new(Tone::Success))
                .child(StatusDot::new(Tone::Warning))
                .child(StatusDot::new(Tone::Danger))
                .child(StatusDot::new(Tone::Neutral))
                // The same dot wearing an identity colour: a state the six
                // severities cannot name, still reporting the severity it
                // claims through the surface around it.
                .child(StatusDot::new(Tone::Neutral).tint(identity_tint(&theme, "loader.pink"))),
        )
        .child(
            row(&theme)
                .child(
                    StatusDot::new(Tone::Accent)
                        .busy("scene.status.deliberating")
                        .activity(crate::motion::Activity::Deliberating),
                )
                .child(
                    StatusDot::new(Tone::Info)
                        .busy("scene.status.working")
                        .activity(crate::motion::Activity::Working),
                )
                .child(
                    StatusDot::new(Tone::Success)
                        .busy("scene.status.advancing")
                        .activity(crate::motion::Activity::Advancing),
                ),
        )
        .child(
            StatusLine::new("Connected", Tone::Success).id("scene.status.line"),
        )
        .child(
            StatusLine::new("Ada · reviewing", Tone::Neutral)
                .tint(identity_tint(&theme, "loader.pink"))
                .busy("scene.status.tinted")
                .id("scene.status.tinted"),
        )
        .child(
            Callout::new(
                "The host refused this action. The refusal is shown, not converted to an empty state.",
                Tone::Danger,
            )
            .id("scene.status.refusal"),
        )
        .child(
            Callout::new("Refreshing failed. The last verified value remains visible.", Tone::Warning)
                .id("scene.status.stale"),
        )
        .into_any_element()
}

pub(super) fn loading(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let indicator = |title: &'static str, loader: AnyElement| {
        div()
            .column()
            .items_center()
            .justify_center()
            .gap_token(&theme, Space::Md)
            .w(px(220.0))
            .h(px(112.0))
            .p_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .surface(&theme, gpui_kit_theme::Surface::Panel)
            .child(crate::foundation::text(&theme, TypeScale::Label, title))
            .child(loader)
    };
    stack(&theme)
        .w_full()
        .max_w(px(520.0))
        .child(
            row(&theme)
                .gap_token(&theme, Space::Md)
                .child(indicator(
                    "Loading providers",
                    PulseLoader::new("scene.loading.pulse")
                        .label("Loading providers")
                        .into_any_element(),
                ))
                .child(indicator(
                    "Contacting host",
                    GradientSpinner::new("scene.loading.spinner")
                        .label("Contacting host")
                        .into_any_element(),
                )),
        )
        .child(
            row(&theme)
                .gap_token(&theme, Space::Md)
                .child(indicator(
                    "Inline wait",
                    Spinner::new("scene.loading.inline")
                        .label("Inline wait")
                        .into_any_element(),
                ))
                .child(indicator(
                    "Refreshing in place",
                    RefreshVeil::new(
                        "scene.loading.veil",
                        crate::foundation::text(&theme, TypeScale::Body, "Last verified list"),
                    )
                    .label("Refreshing")
                    .into_any_element(),
                )),
        )
        .child(
            div()
                .column()
                .gap_token(&theme, Space::Sm)
                .w_full()
                .child(crate::foundation::text(
                    &theme,
                    TypeScale::Label,
                    "Loading list",
                ))
                .child(
                    Skeleton::new("scene.loading.skeleton")
                        .rows(3)
                        .label("Loading list"),
                ),
        )
        .child(
            div()
                .column()
                .gap_token(&theme, Space::Sm)
                .w_full()
                .child(crate::foundation::text(
                    &theme,
                    TypeScale::Label,
                    "Card and paragraph placeholders",
                ))
                .child(
                    Skeleton::new("scene.loading.shapes")
                        .shapes([
                            SkeletonShape::Card,
                            SkeletonShape::Paragraph { lines: 3 },
                            SkeletonShape::Circle { size: 28.0 },
                        ])
                        .label("Loading card"),
                ),
        )
        .into_any_element()
}

pub(super) fn failure_panel(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    // A failure, not a refusal. Retrying a refusal would only be refused
    // again, so the retry here is offered against something that can succeed
    // on a second attempt; refusals are shown by ToolCallCard instead.
    let failed: Result<(), &str> = Err("The runs service did not respond.");
    stack(&theme)
        .w(px(560.0))
        .child(caption(
            &theme,
            "the host's own words, kept on screen; the retry belongs to the host",
        ))
        .children(
            FailurePanel::from_result("scene.failure.query", &failed).map(|panel| {
                panel
                    .title("Runs")
                    .detail("The connection timed out after 30 seconds.")
                    .attempts(3)
                    .on_retry(|_, _| {})
            }),
        )
        .into_any_element()
}

pub(super) fn chart(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let cpu = [
        ChartPoint::new("00:00", 0.0, 0.20, "00:00", "20%"),
        ChartPoint::new("00:15", 0.25, 0.45, "00:15", "45%"),
        ChartPoint::new("00:30", 0.5, 0.38, "00:30", "38%"),
        ChartPoint::new("00:45", 0.75, 0.70, "00:45", "70%"),
        ChartPoint::new("01:00", 1.0, 0.62, "01:00", "62%"),
    ];
    let memory = [
        ChartPoint::new("00:00", 0.0, 0.40, "00:00", "40%"),
        ChartPoint::new("00:15", 0.25, 0.42, "00:15", "42%"),
        ChartPoint::new("00:30", 0.5, 0.55, "00:30", "55%"),
        ChartPoint::new("00:45", 0.75, 0.58, "00:45", "58%"),
        ChartPoint::new("01:00", 1.0, 0.61, "01:00", "61%"),
    ];
    stack(&theme)
        .w(px(520.0))
        .child(caption(
            &theme,
            "host-owned series, host-owned axis wording, no invented scale",
        ))
        .child(
            LineChart::new(
                "scene.chart.ready",
                "Fixture load",
                ChartState::Stale {
                    series: vec![
                        ChartSeries::new("cpu", "CPU")
                            .points(cpu)
                            .tint(identity_tint(&theme, "loader.blue")),
                        ChartSeries::new("memory", "Memory")
                            .points(memory)
                            .tint(identity_tint(&theme, "loader.orange")),
                    ],
                    reason: "Refresh failed; showing last verified sample".into(),
                },
            )
            .area()
            .crosshair()
            .current("cpu", "00:45")
            .on_current(|_, _, _| {})
            .axes(
                ChartAxes::default()
                    .x_label("Time")
                    .y_label("Utilization")
                    .x_ends("00:00", "01:00")
                    .y_ends("0%", "100%"),
            ),
        )
        .child(LineChart::new(
            "scene.chart.empty",
            "Fixture load",
            ChartState::Empty,
        ))
        .child(
            BarChart::new(
                "scene.chart.bars",
                "Fixture share",
                ChartState::Ready(vec![ChartSeries::new("share", "Share").points([
                    ChartPoint::new("alpha", 0.0, 0.35, "Alpha", "35 jobs"),
                    ChartPoint::new("beta", 0.33, 0.70, "Beta", "70 jobs"),
                    ChartPoint::new("gamma", 0.66, 0.45, "Gamma", "45 jobs"),
                    ChartPoint::new("delta", 1.0, 0.90, "Delta", "90 jobs"),
                ])]),
            )
            .axes(ChartAxes::default().y_ends("0", "max")),
        )
        .child(
            PieChart::new(
                "scene.chart.pie",
                "Fixture share",
                ChartState::Ready(vec![ChartSeries::new("share", "Share").points([
                    ChartPoint::new("alpha", 0.0, 0.25, "Alpha", "25%"),
                    ChartPoint::new("beta", 0.25, 0.35, "Beta", "35%"),
                    ChartPoint::new("gamma", 0.60, 0.20, "Gamma", "20%"),
                    ChartPoint::new("delta", 0.80, 0.20, "Delta", "20%"),
                ])]),
            )
            .donut(),
        )
        .child(
            StackedBarChart::new(
                "scene.chart.stacked",
                "Fixture mix",
                ChartState::Ready(vec![
                    ChartSeries::new("cpu", "CPU")
                        .points([
                            ChartPoint::new("alpha", 0.0, 0.30, "Alpha", "30%"),
                            ChartPoint::new("beta", 0.5, 0.20, "Beta", "20%"),
                        ])
                        .tint(identity_tint(&theme, "loader.blue")),
                    ChartSeries::new("memory", "Memory")
                        .points([
                            ChartPoint::new("alpha", 0.0, 0.25, "Alpha", "25%"),
                            ChartPoint::new("beta", 0.5, 0.40, "Beta", "40%"),
                        ])
                        .tint(identity_tint(&theme, "loader.orange")),
                ]),
            )
            .axes(ChartAxes::default().y_ends("0", "max")),
        )
        .child(
            ChartLegend::new(
                "scene.chart.legend",
                [
                    ChartSeries::new("cpu", "CPU").tint(identity_tint(&theme, "loader.blue")),
                    ChartSeries::new("memory", "Memory")
                        .tint(identity_tint(&theme, "loader.orange")),
                ],
            )
            .on_toggle(|_, _, _, _| {}),
        )
        .child(
            AreaChart::new(
                "scene.chart.area",
                "Fixture tokens",
                ChartState::Ready(vec![
                    ChartSeries::new("tokens", "Tokens")
                        .points([
                            ChartPoint::new("00:00", 0.0, 0.20, "00:00", "20%"),
                            ChartPoint::new("00:15", 0.25, 0.45, "00:15", "45%"),
                            ChartPoint::new("00:30", 0.5, 0.38, "00:30", "38%"),
                            ChartPoint::new("00:45", 0.75, 0.70, "00:45", "70%"),
                            ChartPoint::new("01:00", 1.0, 0.62, "01:00", "62%"),
                        ])
                        .tint(identity_tint(&theme, "loader.blue")),
                ]),
            )
            .axes(ChartAxes::default().y_ends("0", "max"))
            .crosshair(),
        )
        .child(
            ScatterChart::new(
                "scene.chart.scatter",
                "Fixture samples",
                ChartState::Ready(vec![ChartSeries::new("samples", "Samples").points([
                    ChartPoint::new("a", 0.15, 0.30, "A", "30").weight(0.2),
                    ChartPoint::new("b", 0.40, 0.62, "B", "62").weight(0.6),
                    ChartPoint::new("c", 0.72, 0.48, "C", "48").weight(0.35),
                    ChartPoint::new("d", 0.88, 0.80, "D", "80").weight(0.9),
                ])]),
            )
            .crosshair()
            .current("samples", "b")
            .on_current(|_, _, _| {})
            .axes(ChartAxes::default().y_ends("0", "max")),
        )
        .child(RadarChart::new(
            "scene.chart.radar",
            "Fixture profile",
            ChartState::Ready(vec![ChartSeries::new("profile", "Profile").points([
                ChartPoint::new("clarity", 0.0, 0.80, "Clarity", "80"),
                ChartPoint::new("speed", 0.0, 0.55, "Speed", "55"),
                ChartPoint::new("coverage", 0.0, 0.70, "Coverage", "70"),
                ChartPoint::new("cost", 0.0, 0.40, "Cost", "40"),
            ])]),
        ))
        .child(RadarChart::new(
            "scene.chart.radar-empty",
            "Fixture profile",
            ChartState::Empty,
        ))
        .child(GaugeChart::new(
            "scene.chart.gauge",
            "Fixture occupancy",
            ChartState::Ready(vec![
                ChartSeries::new("occupancy", "Occupancy")
                    .points([ChartPoint::new("now", 0.0, 0.72, "Now", "72%")]),
            ]),
        ))
        .into_any_element()
}

pub(super) fn metric_card(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(420.0))
        .child(caption(
            &theme,
            "a KPI keeps the last verified reading when a refresh fails",
        ))
        .child(MetricCard::new(
            "scene.metric.ready",
            "Tokens",
            MetricState::Ready(
                MetricReading::new("12.4k")
                    .delta("+8%", Tone::Success)
                    .trend([
                        SparklinePoint::new(0.0, 0.30),
                        SparklinePoint::new(0.35, 0.55),
                        SparklinePoint::new(0.70, 0.48),
                        SparklinePoint::new(1.0, 0.72),
                    ]),
            ),
        ))
        .child(MetricCard::new(
            "scene.metric.loading",
            "Tokens",
            MetricState::Loading,
        ))
        .child(MetricCard::new(
            "scene.metric.empty",
            "Tokens",
            MetricState::Empty,
        ))
        .child(MetricCard::new(
            "scene.metric.unavailable",
            "Tokens",
            MetricState::Unavailable("The meter host is offline.".into()),
        ))
        .child(MetricCard::new(
            "scene.metric.error",
            "Tokens",
            MetricState::Error("The meter returned an invalid reading.".into()),
        ))
        .child(MetricCard::new(
            "scene.metric.stale",
            "Tokens",
            MetricState::Stale {
                reading: MetricReading::new("12.4k").delta("+8%", Tone::Warning),
                reason: "Refresh failed; showing last verified reading".into(),
            },
        ))
        .into_any_element()
}

pub(super) fn trace(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let spans = [
        TraceSpan::new("plan", "Plan", 0.00, 0.18)
            .state(SpanState::Succeeded)
            .detail("planner.finish"),
        TraceSpan::new("generate", "Generate", 0.18, 0.62)
            .depth(1)
            .state(SpanState::Running)
            .detail("model.reply"),
        TraceSpan::new("tool", "Search", 0.40, 0.55)
            .depth(2)
            .state(SpanState::Succeeded),
        TraceSpan::new("wait", "Review", 0.62, 0.80)
            .depth(1)
            .state(SpanState::Pending),
        TraceSpan::new("fail", "Publish", 0.80, 1.00).state(SpanState::Failed),
    ];
    stack(&theme)
        .w(px(640.0))
        .child(caption(
            &theme,
            "normalized intervals, host-owned labels, a refusal stays a failure",
        ))
        .child(
            TraceView::new("scene.trace.view", "Fixture run")
                .spans(spans.clone())
                .axis("0 ms", "1.2 s")
                .current("generate")
                .on_select(|_, _, _| {}),
        )
        .child(
            SpanTimeline::new("scene.trace.timeline", "Fixture waterfall")
                .spans(spans)
                .axis("0 ms", "1.2 s")
                .current("generate")
                .on_select(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn heatmap(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri"];
    let weeks = ["W1", "W2", "W3", "W4"];
    let levels = [
        [Some(0), Some(1), Some(2), None],
        [Some(1), Some(3), Some(2), Some(1)],
        [Some(4), Some(2), Some(0), Some(3)],
        [None, Some(1), Some(4), Some(2)],
        [Some(2), None, Some(1), Some(0)],
    ];
    let mut cells = Vec::new();
    for (row, day) in days.iter().enumerate() {
        for (column, week) in weeks.iter().enumerate() {
            let mut cell =
                HeatCell::new(format!("{day}-{week}"), *day, *week).label(format!("{day} {week}"));
            if let Some(level) = levels[row][column] {
                cell = cell.level(level).value(format!("{level}"));
            }
            cells.push(cell);
        }
    }
    stack(&theme)
        .w(px(420.0))
        .child(caption(
            &theme,
            "five intensity steps; a missing cell is not a measured zero",
        ))
        .child(
            Heatmap::new("scene.heatmap.ready", "Fixture activity")
                .rows(days)
                .columns(weeks)
                .cells(cells),
        )
        .child(Heatmap::new("scene.heatmap.empty", "Fixture activity").state(HeatmapState::Empty))
        .into_any_element()
}

pub(super) fn sparkline(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let points = [
        SparklinePoint::new(0.0, 0.20),
        SparklinePoint::new(0.14, 0.34),
        SparklinePoint::new(0.28, 0.29),
        SparklinePoint::new(0.42, 0.56),
        SparklinePoint::new(0.57, 0.48),
        SparklinePoint::new(0.71, 0.74),
        SparklinePoint::new(0.85, 0.66),
        SparklinePoint::new(1.0, 0.82),
    ];
    stack(&theme)
        .w(px(520.0))
        .child(caption(
            &theme,
            "normalized geometry with caller-formatted current, minimum and maximum",
        ))
        .child(Sparkline::new(
            "scene.sparkline.rate",
            "Fixture throughput",
            SparklineState::Ready(SparklineReading::new(
                points, "82 req/s", "20 req/s", "82 req/s",
            )),
        ))
        .child(Sparkline::new(
            "scene.sparkline.stale",
            "Fixture queue depth",
            SparklineState::Stale {
                reading: SparklineReading::new(points, "34 jobs", "8 jobs", "41 jobs"),
                reason: "The latest sample is unavailable; the verified reading remains.".into(),
            },
        ))
        .into_any_element()
}

/// Progress that is counted, and progress that is not.
pub(super) fn progress_bar(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(420.0))
        .child(caption(&theme, "A count the host reported"))
        .child(
            ProgressBar::new("scene.progress-bar.counted")
                .label("Indexing workspace")
                .count(3, 12),
        )
        .child(
            ProgressBar::new("scene.progress-bar.nearly")
                .label("Uploading")
                .count(11, 12),
        )
        .child(caption(
            &theme,
            "No count yet. The bar says it is working, and does not invent a fraction",
        ))
        .child(ProgressBar::new("scene.progress-bar.unknown").label("Contacting host"))
        .child(caption(&theme, "Stopped, without inventing a fraction"))
        .child(
            ProgressBar::new("scene.progress-bar.stalled")
                .label("Indexing workspace")
                .count(3, 12)
                .stalled(true),
        )
        .child(
            ProgressBar::new("scene.progress-bar.paused")
                .label("Uploading")
                .count(6, 12)
                .paused(true)
                .on_cancel(|_, _| {}),
        )
        .into_any_element()
}

/// A rule between groups, with and without a name for what it separates.
pub(super) fn divider(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(420.0))
        .child(caption(&theme, "Plain"))
        .child(Divider::new().id("scene.divider.plain"))
        .child(caption(&theme, "Labelled, which names the group below it"))
        .child(Divider::new().id("scene.divider.labelled").label("Filters"))
        .child(Divider::new().id("scene.divider.archive").label("Archive"))
        .into_any_element()
}

/// A removable token, in every tone a caller can give one.
pub(super) fn tag(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(caption(&theme, "Tones, which report rather than decorate"))
        .child(
            row(&theme)
                .child(Tag::new("scene.tag.rust", "rust").on_remove(|_, _| {}))
                .child(
                    Tag::new("scene.tag.failing", "failing")
                        .tone(Tone::Danger)
                        .on_remove(|_, _| {}),
                )
                .child(
                    Tag::new("scene.tag.passing", "passing")
                        .tone(Tone::Success)
                        .on_remove(|_, _| {}),
                )
                .child(
                    Tag::new("scene.tag.review", "needs review")
                        .tone(Tone::Warning)
                        .on_remove(|_, _| {}),
                ),
        )
        .child(caption(
            &theme,
            "An identity tint, which is a fact about who and not about severity",
        ))
        .child(
            row(&theme)
                .child(
                    Tag::new("scene.tag.ada", "Ada")
                        .tint(identity_tint(&theme, "loader.pink"))
                        .on_remove(|_, _| {}),
                )
                .child(
                    Tag::new("scene.tag.grace", "Grace")
                        .tint(identity_tint(&theme, "loader.orange"))
                        .on_remove(|_, _| {}),
                ),
        )
        .child(caption(
            &theme,
            "Not removable, and disabled. Neither installs a handler",
        ))
        .child(
            row(&theme)
                .child(Tag::new("scene.tag.plain", "read-only"))
                .child(Tag::new("scene.tag.pinned", "pinned").disabled(true)),
        )
        .into_any_element()
}

/// An identity, with a name to derive from and without one.
pub(super) fn avatar(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(caption(&theme, "Derived from the name it was given"))
        .child(
            row(&theme)
                .child(Avatar::new("Ada Lovelace").id("scene.avatar.ada"))
                .child(Avatar::new("Grace Hopper").id("scene.avatar.grace"))
                .child(Avatar::new("Katherine Johnson").id("scene.avatar.katherine")),
        )
        .child(caption(
            &theme,
            "An identity tint the caller owns, and no name at all",
        ))
        .child(
            row(&theme)
                .child(
                    Avatar::new("Grace Hopper")
                        .tint(identity_tint(&theme, "loader.orange"))
                        .id("scene.avatar.tinted"),
                )
                .child(Avatar::new("").id("scene.avatar.anonymous")),
        )
        .child(caption(&theme, "Sizes"))
        .child(
            row(&theme)
                .items_center()
                .child(
                    Avatar::new("Ada Lovelace")
                        .size(24.0)
                        .id("scene.avatar.small"),
                )
                .child(Avatar::new("Ada Lovelace").id("scene.avatar.default"))
                .child(
                    Avatar::new("Ada Lovelace")
                        .size(48.0)
                        .id("scene.avatar.large"),
                ),
        )
        .into_any_element()
}

/// Nothing to show, and the four different reasons for it.
///
/// These are separate states because they are separate facts. A host that
/// refused is not a collection that is empty, and neither is a query that
/// matched nothing; collapsing them is how a refusal gets shown as an absence.
pub(super) fn empty_state(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .child(caption(&theme, "Nothing has been started yet"))
        .child(
            EmptyState::new("scene.empty-state.unstarted", "No runs yet")
                .kind(EmptyKind::Unstarted)
                .detail("A run appears here once one has been started."),
        )
        .child(caption(&theme, "The host refused, and says so"))
        .child(
            EmptyState::new(
                "scene.empty-state.unavailable",
                "The host refused the request",
            )
            .kind(EmptyKind::Unavailable)
            .detail("Approval is required for this workspace.")
            .action(
                Button::new("scene.empty-state.retry")
                    .label("Try again")
                    .secondary()
                    .on_click(|_, _| {}),
            ),
        )
        .child(caption(&theme, "A collection that really is empty"))
        .child(
            EmptyState::new("scene.empty-state.empty", "No runs match “failing”")
                .kind(EmptyKind::Empty)
                .detail("Clear the filter to see every run."),
        )
        .child(caption(&theme, "It was tried, and it failed"))
        .child(
            EmptyState::new("scene.empty-state.failed", "The run could not be read")
                .kind(EmptyKind::Failed)
                .detail("The snapshot on disk is from a newer version of the format.")
                .action(
                    Button::new("scene.empty-state.reload")
                        .label("Reload")
                        .secondary()
                        .on_click(|_, _| {}),
                ),
        )
        .child(caption(
            &theme,
            "The host refused because the reader is not allowed",
        ))
        .child(
            EmptyState::new("scene.empty-state.unauthorized", "This workspace is locked")
                .kind(EmptyKind::Unauthorized)
                .detail("Ask an owner to grant access."),
        )
        .into_any_element()
}

pub(super) fn state_ladder(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let pane = |title: &'static str, view: StateView| {
        div()
            .column()
            .gap_token(&theme, Space::Xs)
            .w(px(240.0))
            .child(crate::foundation::text(&theme, TypeScale::Caption, title))
            .child(view)
    };
    stack(&theme)
        .w_full()
        .child(caption(
            &theme,
            "Ten phases, one surface. A refusal is not an empty list.",
        ))
        .child(
            row(&theme)
                .child(pane(
                    "Idle",
                    StateView::new("scene.state.idle", Phase::Idle),
                ))
                .child(pane(
                    "Queued",
                    StateView::new("scene.state.queued", Phase::Queued),
                ))
                .child(pane(
                    "Blocked",
                    StateView::new("scene.state.blocked", Phase::Blocked),
                )),
        )
        .child(
            row(&theme)
                .child(pane(
                    "Loading",
                    StateView::new("scene.state.loading", Phase::Loading),
                ))
                .child(pane(
                    "Empty",
                    StateView::new("scene.state.empty", Phase::Empty),
                ))
                .child(pane(
                    "Cancelled",
                    StateView::new("scene.state.cancelled", Phase::Cancelled).content(div()),
                )),
        )
        .child(
            row(&theme)
                .child(pane(
                    "Unavailable",
                    StateView::new(
                        "scene.state.unavailable",
                        Loadable::<(), String>::Unavailable("the host refused".into()),
                    ),
                ))
                .child(pane(
                    "Error",
                    StateView::new(
                        "scene.state.error",
                        AsyncValue::<(), String>::error("the index is still building".into()),
                    ),
                ))
                .child(pane(
                    "Ready",
                    StateView::from_async(
                        "scene.state.ready",
                        &AsyncValue::<_, String>::ready("12 runs"),
                        |value| {
                            crate::foundation::text(&theme, TypeScale::Body, *value)
                                .into_any_element()
                        },
                    ),
                )),
        )
        .child(pane("Refreshing, last value kept", {
            let mut value = AsyncValue::<_, String>::ready("12 runs");
            value.refresh();
            StateView::from_async("scene.state.refreshing", &value, |text| {
                crate::foundation::text(&theme, TypeScale::Body, *text).into_any_element()
            })
        }))
        .child(
            StaleMark::new(
                "scene.state.stale",
                "Refreshing failed. The last verified value remains.",
            )
            .updated("a moment ago"),
        )
        .into_any_element()
}

pub(super) fn banner(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .child(caption(&theme, "A page-level report, dismissable"))
        .child(
            Banner::new(
                "scene.banner.warning",
                "The last refresh failed. The verified list is still on screen.",
                Tone::Warning,
            )
            .title("Stale workspace")
            .action(
                Button::new("scene.banner.retry")
                    .label("Try again")
                    .secondary()
                    .small(),
            )
            .on_dismiss(|_, _| {}),
        )
        .child(
            Banner::new(
                "scene.banner.danger",
                "The host refused this action.",
                Tone::Danger,
            )
            .title("Refused"),
        )
        .into_any_element()
}

pub(super) fn outcome_panel(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .child(caption(&theme, "Finished"))
        .child(
            OutcomePanel::new("scene.outcome.success", OutcomeKind::Success)
                .title("Import finished")
                .count("50 files imported"),
        )
        .child(caption(&theme, "Finished, with failures the host numbered"))
        .child(
            OutcomePanel::new("scene.outcome.partial", OutcomeKind::Partial)
                .title("Import finished, with failures")
                .count("47 succeeded, 3 failed")
                .detail("The three that failed are still in the queue."),
        )
        .child(caption(&theme, "Did not finish"))
        .child(
            OutcomePanel::new("scene.outcome.failed", OutcomeKind::Failed)
                .detail("The host closed the connection before any file landed."),
        )
        .into_any_element()
}

pub(super) fn stage_progress(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(420.0))
        .child(caption(&theme, "Download, then verify, then install"))
        .child(StageProgress::new("scene.stage.running").stages([
            ProgressStage::new("download", "Download", StageStatus::Done),
            ProgressStage::new("verify", "Verify", StageStatus::Active),
            ProgressStage::new("install", "Install", StageStatus::Pending),
        ]))
        .child(caption(&theme, "A stage that failed keeps its name"))
        .child(StageProgress::new("scene.stage.failed").stages([
            ProgressStage::new("download", "Download", StageStatus::Done),
            ProgressStage::new("verify", "Verify", StageStatus::Failed),
            ProgressStage::new("install", "Install", StageStatus::Pending),
        ]))
        .into_any_element()
}

/// The counts the animated readout scene is currently showing.
#[derive(Debug)]
pub(super) struct SceneCounts {
    runs: f64,
    seconds: f64,
}

impl Global for SceneCounts {}

pub(super) fn animated_number(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneCounts>() {
        cx.set_global(SceneCounts {
            runs: 1204.0,
            seconds: 18.4,
        });
    }
    let counts = cx.global::<SceneCounts>();
    let (runs, seconds) = (counts.runs, counts.seconds);
    let theme = cx.theme().clone();

    let readout = |label: &'static str, number: AnimatedNumber| {
        div()
            .column()
            .gap(px(theme.spacing.xs))
            .child(
                crate::foundation::text(&theme, TypeScale::Caption, label)
                    .text_tone(&theme, TextTone::Muted),
            )
            .child(number)
    };

    stack(&theme)
        .child(
            row(&theme)
                .gap(px(theme.spacing.xl))
                .items_start()
                .child(readout(
                    "Runs this week",
                    AnimatedNumber::new("scene.number.runs", runs).format(grouped),
                ))
                .child(readout(
                    "Median duration",
                    AnimatedNumber::new("scene.number.seconds", seconds)
                        .format(|value| format!("{value:.1}s")),
                )),
        )
        .child(
            row(&theme).child(
                Button::new("scene.number.recount")
                    .label("Recount")
                    .secondary()
                    .on_click(|_, cx| {
                        cx.update_global::<SceneCounts, ()>(|counts, _| {
                            counts.runs += 318.0;
                            counts.seconds += 4.7;
                        });
                        cx.refresh_windows();
                    }),
            ),
        )
        .child(
            crate::foundation::text(
                &theme,
                TypeScale::Body,
                "The published value is the target, from the frame it changes.",
            )
            .text_tone(&theme, TextTone::Muted),
        )
        .into_any_element()
}

pub(super) fn detail(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(640.0))
        .child(caption(
            &theme,
            "unknown, not applicable, and redacted are three facts",
        ))
        .child(
            DescriptionList::new("scene.detail.facts")
                .columns(2)
                .items([
                    DescriptionItem::new("id", "Run", "run-4821"),
                    DescriptionItem::new("owner", "Owner", "fixture-owner"),
                    DescriptionItem::new("finished", "Finished", DescriptionValue::Unknown),
                    DescriptionItem::new("artifact", "Artifact", DescriptionValue::NotApplicable),
                    DescriptionItem::new(
                        "token",
                        "Access token",
                        DescriptionValue::redacted("51 characters"),
                    )
                    .copyable(true),
                ])
                .on_copy(|_, _, _| {}),
        )
        .child(caption(
            &theme,
            "what happened, in the words the host chose",
        ))
        .child(
            Timeline::new("scene.detail.activity")
                .group(
                    TimelineGroup::new("today", "Today")
                        .entry(
                            TimelineEntry::new("queued", "Run queued")
                                .time("09:12")
                                .actor("fixture-owner")
                                .tone(Tone::Neutral),
                        )
                        .entry(
                            TimelineEntry::new("started", "Indexing started")
                                .time("09:13")
                                .actor("scheduler")
                                .tone(Tone::Info),
                        )
                        .entry(
                            TimelineEntry::new("failed", "Indexing failed")
                                .time("09:41")
                                .actor("scheduler")
                                .tone(Tone::Danger)
                                .detail(crate::foundation::text(
                                    &theme,
                                    TypeScale::Body,
                                    SharedString::new_static(
                                        "The host refused the request. The refusal is shown as it \
                                     arrived.",
                                    ),
                                ).text_tone(&theme, TextTone::Muted)),
                        ),
                )
                .group(
                    TimelineGroup::new("earlier", "Earlier").entry(
                        TimelineEntry::new("imported", "Workspace imported")
                            .time_unknown()
                            .actor("fixture-owner")
                            .tone(Tone::Neutral),
                    ),
                ),
        )
        .into_any_element()
}

pub(super) fn progress_circle(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(caption(
            &theme,
            "a position exists only when the extent is known",
        ))
        .child(
            row(&theme)
                .gap(px(theme.spacing.lg))
                .child(
                    ProgressCircle::new("scene.progress-circle.upload")
                        .count(3, 12)
                        .label("Uploading artifacts")
                        .centre("25%"),
                )
                .child(
                    ProgressCircle::new("scene.progress-circle.verify")
                        .fraction(0.72)
                        .label("Verifying checksums")
                        .display("72%")
                        .centre("72%"),
                )
                .child(
                    ProgressCircle::new("scene.progress-circle.contact")
                        .label("Contacting the host"),
                ),
        )
        .child(caption(&theme, "the size ramp"))
        .child(
            row(&theme)
                .gap(px(theme.spacing.lg))
                .child(
                    ProgressCircle::new("scene.progress-circle.xs")
                        .fraction(0.4)
                        .label("Extra small")
                        .xs(),
                )
                .child(
                    ProgressCircle::new("scene.progress-circle.sm")
                        .fraction(0.4)
                        .label("Small")
                        .small(),
                )
                .child(
                    ProgressCircle::new("scene.progress-circle.md")
                        .fraction(0.4)
                        .label("Medium")
                        .medium(),
                )
                .child(
                    ProgressCircle::new("scene.progress-circle.lg")
                        .fraction(0.4)
                        .label("Large")
                        .large(),
                ),
        )
        .into_any_element()
}

/// The glyph catalog, at the sizes and tones a caller can ask for.
///
/// An icon appears inside a dozen other scenes, which is how it went so long
/// without one of its own: it was recognisable everywhere and reviewed
/// nowhere, so a change to a tone or to the direction rule would have moved a
/// tab strip and a sidebar and been read as a change to those.
pub(super) fn icon(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let tones = [
        ("primary", IconTone::Primary),
        ("muted", IconTone::Muted),
        ("faint", IconTone::Faint),
        ("accent", IconTone::Accent),
        ("accent strong", IconTone::AccentStrong),
        ("success", IconTone::Success),
        ("warning", IconTone::Warning),
        ("danger", IconTone::Danger),
        ("info", IconTone::Info),
    ];

    stack(&theme)
        .child(caption(
            &theme,
            "The whole catalog, so a glyph that goes missing is visible here",
        ))
        .child(
            row(&theme)
                .gap(px(theme.space(Space::Md)))
                .children(Icon::ALL.iter().map(|glyph| {
                    let name = SharedString::from(format!("{glyph:?}"));
                    IconView::named(format!("scene.icon.glyph.{name}"), *glyph, name.clone())
                        .into_any_element()
                })),
        )
        .child(caption(
            &theme,
            "Every tone. These are facts about what a glyph reports, not decoration",
        ))
        .child(
            row(&theme)
                .gap(px(theme.space(Space::Lg)))
                .children(tones.map(|(label, tone)| {
                    div()
                        .column()
                        .items_center()
                        .gap(px(theme.space(Space::Xs)))
                        .child(
                            IconView::named(format!("scene.icon.tone.{label}"), Icon::Info, label)
                                .tone(tone),
                        )
                        .child(
                            crate::foundation::text(&theme, TypeScale::Caption, label)
                                .text_tone(&theme, TextTone::Faint),
                        )
                        .into_any_element()
                })),
        )
        .child(caption(
            &theme,
            "A glyph that means a direction turns with the reading order; one that \
             means a thing does not",
        ))
        .child(
            row(&theme).gap(px(theme.space(Space::Md))).children(
                [
                    (Icon::ArrowRight, "arrow-right"),
                    (Icon::Return, "return"),
                    (Icon::Copy, "copy"),
                    (Icon::Check, "check"),
                    (Icon::Settings, "settings"),
                ]
                .map(|(glyph, name)| {
                    IconView::named(format!("scene.icon.direction.{name}"), glyph, name)
                        .follow_direction(true)
                        .into_any_element()
                }),
            ),
        )
        .into_any_element()
}
