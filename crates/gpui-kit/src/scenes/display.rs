//! Surfaces that report a value without taking one.

use super::support::*;

pub(super) fn performance_hud(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let summary = gpui::FrameTimingSummary {
        sample_count: 8,
        frames_per_second: 58.7,
        frame_budget: Duration::from_micros(16_667),
        mean_draw_duration: Duration::from_micros(8_425),
        p95_draw_duration: Duration::from_micros(18_900),
        over_budget_fraction: 0.125,
        mean_invalidations: 1.4,
        mean_dirty_to_draw_duration: Some(Duration::from_micros(10_600)),
        draw_durations: [7_200, 7_800, 8_100, 7_600, 9_300, 18_900, 8_000, 8_500]
            .map(Duration::from_micros)
            .into(),
    };
    stack(&theme)
        .w(px(620.0))
        .child(caption(
            &theme,
            "The framework observes existing draws; the controlled HUD only presents the caller's latest summary",
        ))
        .child(
            PerformanceHud::new(
                "scene.performance.ready",
                PerformanceHudState::Ready(summary),
            )
            .expanded(true)
            .on_expanded(|_, _, _| {}),
        )
        .child(
            div()
                .row()
                .items_stretch()
                .gap_token(&theme, Space::Md)
                .child(
                    div().flex_1().min_w_0().child(PerformanceHud::new(
                        "scene.performance.waiting",
                        PerformanceHudState::Waiting,
                    )),
                )
                .child(
                    div().flex_1().min_w_0().child(PerformanceHud::new(
                        "scene.performance.unavailable",
                        PerformanceHudState::Unavailable(
                            "Frame tracing is disabled by this host.".into(),
                        ),
                    )),
                ),
        )
        .into_any_element()
}

pub(super) fn rating(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(caption(
            &theme,
            "A numeric rating is distinct from helpful or unhelpful feedback",
        ))
        .child(
            Rating::new("scene.rating.precise")
                .label("Quality")
                .value(Some(3.5))
                .precision(RatingPrecision::Half)
                .clearable(true)
                .on_change(|_, _, _| {}),
        )
        .child(
            Rating::new("scene.rating.unrated")
                .label("Not reviewed")
                .maximum(10)
                .value(None)
                .on_change(|_, _, _| {}),
        )
        .child(
            Rating::new("scene.rating.disabled")
                .label("Managed score")
                .value(Some(4.0))
                .disabled(true)
                .on_change(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn bubble(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let message = |id: &'static str, label: &'static str, body: &'static str, placement| {
        Bubble::new(id, label)
            .placement(placement)
            .content(crate::foundation::text(&theme, TypeScale::Body, body))
            .actions([Button::new(format!("{id}.action")).label("Reply").ghost()])
    };
    stack(&theme)
        .w(px(620.0))
        .child(caption(
            &theme,
            "Placement and grouping are visual policy; the message and actions belong to the caller",
        ))
        .child(message(
            "scene.bubble.start",
            "Incoming message",
            "The gateway is ready for the next run.",
            BubblePlacement::Start,
        ))
        .child(message(
            "scene.bubble.end",
            "Outgoing message",
            "Start the run when the inputs are verified.",
            BubblePlacement::End,
        ))
        .child(
            Bubble::new("scene.bubble.grouped", "Grouped message")
                .grouped(true)
                .content(crate::foundation::text(
                    &theme,
                    TypeScale::Body,
                    "A grouped follow-up shares the control radius.",
                )),
        )
        .into_any_element()
}

pub(super) fn badge(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .child(caption(&theme, "Tones, which report rather than decorate"))
        .child(
            row(&theme)
                .child(Badge::new("Neutral").neutral().id("scene.badge.neutral"))
                .child(Badge::new("Accent").accent().id("scene.badge.accent"))
                .child(Badge::new("Success").success().id("scene.badge.success"))
                .child(Badge::new("Warning").warning().id("scene.badge.warning"))
                .child(Badge::new("Danger").danger().id("scene.badge.danger"))
                .child(Badge::new("Info").info().id("scene.badge.info")),
        )
        .child(caption(
            &theme,
            "A glyph or a mark, for a badge whose claim has a picture",
        ))
        .child(
            row(&theme)
                .child(
                    Badge::new("Passed")
                        .success()
                        .icon(Icon::Check)
                        .id("scene.badge.icon"),
                )
                .child(Badge::new("Live").success().dot(true).id("scene.badge.dot"))
                .child(
                    Badge::new("Refused")
                        .danger()
                        .icon(Icon::CloseCircle)
                        .id("scene.badge.refused"),
                ),
        )
        .child(caption(
            &theme,
            "Outlined, for a badge landing on a surface that is already washed",
        ))
        .child(
            row(&theme)
                .child(
                    Badge::new("Warning")
                        .warning()
                        .outlined(true)
                        .id("scene.badge.outlined-warning"),
                )
                .child(
                    Badge::new("Neutral")
                        .outlined(true)
                        .id("scene.badge.outlined-neutral"),
                ),
        )
        .child(caption(&theme, "The size ramp"))
        .child(
            row(&theme)
                .child(Badge::new("Extra small").xs().id("scene.badge.xs"))
                .child(Badge::new("Small").small().id("scene.badge.sm"))
                .child(Badge::new("Medium").medium().id("scene.badge.md"))
                .child(Badge::new("Large").large().id("scene.badge.lg")),
        )
        // A tint says whose the badge is. The colours come from the theme's
        // own palette rather than from literals, so a retinted document
        // retints these too, and the tone underneath still reports itself.
        .child(caption(
            &theme,
            "An identity tint, which says whose and not how it is going",
        ))
        .child(
            row(&theme)
                .child(
                    Badge::new("Ada")
                        .tint(identity_tint(&theme, "agent.external"))
                        .id("scene.badge.tinted-neutral"),
                )
                .child(
                    Badge::new("Grace")
                        .tint(identity_tint(&theme, "agent.shell"))
                        .warning()
                        .id("scene.badge.tinted-warning"),
                ),
        )
        .child(caption(
            &theme,
            "The shared tiers, resolved against a palette colour",
        ))
        .children(["grape", "cyan"].map(|group| {
            row(&theme).children(
                [
                    Variant::Filled,
                    Variant::Light,
                    Variant::Outline,
                    Variant::Subtle,
                ]
                .map(|tier| {
                    Badge::new(tier.name())
                        .variant(tier)
                        .color(SharedString::from(group))
                        .id(format!("scene.badge.{group}.{}", tier.name()))
                }),
            )
        }))
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
                    // The pairing the rows above never put together, which is
                    // how a card with no plane came to be lifted off one.
                    Card::new()
                        .id("scene.card.actionable-ghost")
                        .variant(CardVariant::Ghost)
                        .padding(Space::Lg)
                        .on_click(|_window, _cx| {})
                        .header(CardHeader::new("Open the line").subtitle("A ghost card acts too"))
                        .child(caption("No plane to rise off, so the pointer gets a wash.")),
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
    // A dot on its own says a thing has a state without saying which; the
    // name beside it is what the row is for.
    let named = |label: &'static str, dot: StatusDot| {
        div()
            .row()
            .items_center()
            .gap(px(theme.space(Space::Xs)))
            .child(dot)
            .child(caption(&theme, label))
    };
    stack(&theme)
        .w(px(560.0))
        .child(caption(&theme, "The smallest mark, and what each one claims"))
        .child(
            row(&theme)
                .gap(px(theme.space(Space::Lg)))
                .child(named("ready", StatusDot::new(Tone::Success)))
                .child(named("stale", StatusDot::new(Tone::Warning)))
                .child(named("refused", StatusDot::new(Tone::Danger)))
                .child(named("off", StatusDot::new(Tone::Neutral)))
                // The same dot wearing an identity colour: a state the six
                // severities cannot name, still reporting the severity it
                // claims through the surface around it.
                .child(named(
                    "Ada",
                    StatusDot::new(Tone::Neutral).tint(identity_tint(&theme, "agent.external")),
                )),
        )
        .child(caption(&theme, "Work that is still going"))
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
        .child(caption(&theme, "The same claim, named"))
        .child(
            StatusLine::new("Connected", Tone::Success).id("scene.status.line"),
        )
        .child(
            StatusLine::new("Ada · reviewing", Tone::Neutral)
                .tint(identity_tint(&theme, "agent.external"))
                .busy("scene.status.tinted")
                .id("scene.status.tinted"),
        )
        .child(caption(
            &theme,
            "A report: a rail and a glyph carry the severity, so two of them differ \
             by meaning and never by weight",
        ))
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
                    "Inline wait",
                    Spinner::new("scene.loading.inline")
                        .label("Inline wait")
                        .into_any_element(),
                )),
        )
        .child(
            row(&theme)
                .gap_token(&theme, Space::Md)
                .child(indicator(
                    "Region filling in",
                    BarLoader::new("scene.loading.bar")
                        .label("Region filling in")
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
                    "List tails",
                ))
                .child(LoadMore::new("scene.loading.more-idle").on_more(|_, _| {}))
                .child(LoadMore::new("scene.loading.more-loading").state(LoadMoreState::Loading))
                .child(LoadMore::new("scene.loading.more-end").state(LoadMoreState::Exhausted)),
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
    // on a second attempt; refusals are shown by ToolCall instead.
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

pub(super) fn plot(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let accent = theme.colors.accent;
    let custom_marks = vec![
        PlotMark::new(
            "first",
            "First stage",
            "18 jobs",
            bounds(point(0.08, 0.58), size(0.18, 0.28)),
        ),
        PlotMark::new(
            "middle",
            "Middle stage",
            "34 jobs",
            bounds(point(0.40, 0.30), size(0.18, 0.56)),
        ),
        PlotMark::new(
            "last",
            "Last stage",
            "26 jobs",
            bounds(point(0.72, 0.44), size(0.18, 0.42)),
        ),
    ];
    let painted_marks = custom_marks.clone();
    let candles = [
        Candlestick::new("monday", 0.10, 0.32, 0.58, 0.20, 0.50, "Monday", "32–50"),
        Candlestick::new("tuesday", 0.30, 0.52, 0.66, 0.38, 0.44, "Tuesday", "52–44"),
        Candlestick::new(
            "wednesday",
            0.50,
            0.45,
            0.80,
            0.40,
            0.72,
            "Wednesday",
            "45–72",
        ),
        Candlestick::new(
            "thursday", 0.70, 0.70, 0.76, 0.34, 0.42, "Thursday", "70–42",
        ),
        Candlestick::new("friday", 0.90, 0.44, 0.88, 0.36, 0.82, "Friday", "44–82"),
    ];
    let source_tint = identity_tint(&theme, "agent.read");
    let target_tint = identity_tint(&theme, "agent.write");
    let sankey = SankeyData::new(
        [
            SankeyNode::new(
                "queued",
                "Queued",
                "48 jobs",
                bounds(point(0.03, 0.20), size(0.08, 0.58)),
            )
            .tint(source_tint),
            SankeyNode::new(
                "running",
                "Running",
                "34 jobs",
                bounds(point(0.46, 0.28), size(0.08, 0.42)),
            ),
            SankeyNode::new(
                "completed",
                "Completed",
                "26 jobs",
                bounds(point(0.89, 0.12), size(0.08, 0.32)),
            )
            .tint(target_tint),
            SankeyNode::new(
                "deferred",
                "Deferred",
                "8 jobs",
                bounds(point(0.89, 0.62), size(0.08, 0.18)),
            )
            .tint(theme.colors.warning),
        ],
        [
            SankeyLink::new(
                "queue-running",
                "queued",
                "running",
                "Queued to running",
                "34 jobs",
                point(0.11, 0.48),
                point(0.46, 0.49),
                0.30,
            )
            .tint(source_tint),
            SankeyLink::new(
                "running-completed",
                "running",
                "completed",
                "Running to completed",
                "26 jobs",
                point(0.54, 0.44),
                point(0.89, 0.28),
                0.24,
            )
            .tint(target_tint),
            SankeyLink::new(
                "running-deferred",
                "running",
                "deferred",
                "Running to deferred",
                "8 jobs",
                point(0.54, 0.62),
                point(0.89, 0.71),
                0.10,
            )
            .tint(theme.colors.warning),
        ],
    );

    let column = || div().column().flex_1().min_w_0();
    stack(&theme)
        .w(px(920.0))
        .child(caption(
            &theme,
            "normalized geometry and exact wording remain caller-owned",
        ))
        .child(
            div()
                .row()
                .items_start()
                .w_full()
                .gap(px(theme.space(Space::Lg)))
                .child(
                    column().child(
                        Plot::new(
                            "scene.plot.custom",
                            "Pipeline",
                            PlotState::Ready(custom_marks),
                        )
                        .current("middle")
                        .on_current(|_, _, _| {})
                        .paint(move |frame, window, _| {
                            for mark in &painted_marks {
                                window
                                    .paint_quad(gpui::fill(frame.mark_bounds(mark.bounds), accent));
                            }
                        }),
                    ),
                )
                .child(
                    column().child(
                        CandlestickChart::new(
                            "scene.plot.candles",
                            "Daily range",
                            PlotState::Ready(candles.into()),
                        )
                        .current("wednesday")
                        .on_current(|_, _, _| {}),
                    ),
                )
                .child(
                    column().child(
                        SankeyChart::new("scene.plot.sankey", "Run flow", PlotState::Ready(sankey))
                            .current("node.running")
                            .on_current(|_, _, _| {}),
                    ),
                ),
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
    // Three columns rather than one tall stack. The family is eleven surfaces
    // once truthful empty/stale states are included, and even two columns run
    // the final plots below the fixed review frame.
    let column = || {
        div()
            .column()
            .flex_1()
            .min_w_0()
            .gap(px(theme.space(Space::Md)))
    };
    // The frame this is reviewed in is 920 wide. Every column flexes inside
    // that width so the third one earns vertical room without escaping at the
    // right edge.
    stack(&theme)
        .w(px(920.0))
        .child(caption(
            &theme,
            "host-owned series, host-owned axis wording, no invented scale",
        ))
        .child(
            div()
                .row()
                .items_start()
                .w_full()
                .gap(px(theme.space(Space::Lg)))
                .child(
                    column()
                        .child(
                            LineChart::new(
                                "scene.chart.ready",
                                "Fixture load",
                                ChartState::Stale {
                                    series: vec![
                                        ChartSeries::new("cpu", "CPU")
                                            .points(cpu)
                                            .tint(identity_tint(&theme, "agent.read")),
                                        ChartSeries::new("memory", "Memory")
                                            .points(memory)
                                            .tint(identity_tint(&theme, "agent.shell")),
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
                                ChartState::Ready(vec![ChartSeries::new("share", "Share").points(
                                    [
                                        ChartPoint::new("alpha", 0.0, 0.35, "Alpha", "35 jobs"),
                                        ChartPoint::new("beta", 0.33, 0.70, "Beta", "70 jobs"),
                                        ChartPoint::new("gamma", 0.66, 0.45, "Gamma", "45 jobs"),
                                        ChartPoint::new("delta", 1.0, 0.90, "Delta", "90 jobs"),
                                    ],
                                )]),
                            )
                            .axes(ChartAxes::default().y_ends("0", "max")),
                        )
                        .child(
                            PieChart::new(
                                "scene.chart.pie",
                                "Fixture share",
                                ChartState::Ready(vec![ChartSeries::new("share", "Share").points(
                                    [
                                        ChartPoint::new("alpha", 0.0, 0.25, "Alpha", "25%"),
                                        ChartPoint::new("beta", 0.25, 0.35, "Beta", "35%"),
                                        ChartPoint::new("gamma", 0.60, 0.20, "Gamma", "20%"),
                                        ChartPoint::new("delta", 0.80, 0.20, "Delta", "20%"),
                                    ],
                                )]),
                            )
                            .donut(),
                        ),
                )
                .child(
                    column()
                        .child(
                            ChartLegend::new(
                                "scene.chart.legend",
                                [
                                    ChartSeries::new("cpu", "CPU")
                                        .tint(identity_tint(&theme, "agent.read")),
                                    ChartSeries::new("memory", "Memory")
                                        .tint(identity_tint(&theme, "agent.shell")),
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
                                        .tint(identity_tint(&theme, "agent.read")),
                                ]),
                            )
                            .axes(ChartAxes::default().y_ends("0", "max"))
                            .crosshair(),
                        )
                        .child(
                            ScatterChart::new(
                                "scene.chart.scatter",
                                "Fixture samples",
                                ChartState::Ready(vec![
                                    ChartSeries::new("samples", "Samples").points([
                                        ChartPoint::new("a", 0.15, 0.30, "A", "30").weight(0.2),
                                        ChartPoint::new("b", 0.40, 0.62, "B", "62").weight(0.6),
                                        ChartPoint::new("c", 0.72, 0.48, "C", "48").weight(0.35),
                                        ChartPoint::new("d", 0.88, 0.80, "D", "80").weight(0.9),
                                    ]),
                                ]),
                            )
                            .crosshair()
                            .current("samples", "b")
                            .on_current(|_, _, _| {})
                            .axes(ChartAxes::default().y_ends("0", "max")),
                        ),
                )
                .child(
                    column()
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
                                        .tint(identity_tint(&theme, "agent.read")),
                                    ChartSeries::new("memory", "Memory")
                                        .points([
                                            ChartPoint::new("alpha", 0.0, 0.25, "Alpha", "25%"),
                                            ChartPoint::new("beta", 0.5, 0.40, "Beta", "40%"),
                                        ])
                                        .tint(identity_tint(&theme, "agent.shell")),
                                ]),
                            )
                            .axes(ChartAxes::default().y_ends("0", "max")),
                        )
                        .child(RadarChart::new(
                            "scene.chart.radar",
                            "Fixture profile",
                            ChartState::Ready(vec![ChartSeries::new("profile", "Profile").points(
                                [
                                    ChartPoint::new("clarity", 0.0, 0.80, "Clarity", "80"),
                                    ChartPoint::new("speed", 0.0, 0.55, "Speed", "55"),
                                    ChartPoint::new("coverage", 0.0, 0.70, "Coverage", "70"),
                                    ChartPoint::new("cost", 0.0, 0.40, "Cost", "40"),
                                ],
                            )]),
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
                        )),
                ),
        )
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
            .duration("216 ms")
            .detail("planner.finish"),
        TraceSpan::new("generate", "Generate", 0.18, 0.62)
            .depth(1)
            .state(SpanState::Running)
            .duration("528 ms")
            .detail("model.reply"),
        TraceSpan::new("tool", "Search", 0.40, 0.55)
            .depth(2)
            .duration("180 ms")
            .state(SpanState::Succeeded),
        TraceSpan::new("wait", "Review", 0.62, 0.80)
            .depth(1)
            .duration("216 ms")
            .state(SpanState::Pending),
        TraceSpan::new("fail", "Publish", 0.80, 1.00)
            .duration("240 ms")
            .state(SpanState::Failed),
    ];
    // The host owns every reading on the axis; the component only places them.
    let ticks = [(0.25, "300 ms"), (0.5, "600 ms"), (0.75, "900 ms")];
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
                .ticks(ticks)
                .current("generate")
                .on_select(|_, _, _| {}),
        )
        .child(
            SpanTimeline::new("scene.trace.timeline", "Fixture waterfall")
                .spans(spans)
                .axis("0 ms", "1.2 s")
                .ticks(ticks)
                .current("generate")
                .on_select(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn heatmap(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    // A quarter of weekdays, which is the shape this component is actually
    // asked for. Four columns of five said nothing about how the grid reads
    // once it carries a period somebody would look at.
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri"];
    // Week-starting dates. Each column is joined on by its own key and prints
    // the day of the month, which is what fits over a cell; the month it came
    // from is the group header over the run of columns that share it, because
    // `3 10 17 24` three times over names nothing on its own.
    let starts = [6, 13, 20, 27, 3, 10, 17, 24, 3, 10, 17, 24];
    let month_of = |column: usize| ["January", "February", "March"][column / 4];
    let weeks: Vec<HeatAxis> = starts
        .iter()
        .enumerate()
        .map(|(column, day)| {
            HeatAxis::new(format!("w{column}"), day.to_string()).group(month_of(column))
        })
        .collect();
    let mut cells = Vec::new();
    for (row, day) in days.iter().enumerate() {
        for (column, week) in weeks.iter().enumerate() {
            let month = month_of(column);
            let mut cell = HeatCell::new(format!("{day}-{}", week.id), *day, week.id.clone())
                .label(format!("{day}, {month} {}", week.label));
            // Deterministic, and holed in two places so the difference between
            // "nothing was measured" and "zero was measured" has somewhere to
            // show itself.
            let sample = (row * 7 + column * 3 + (column * column) % 5) % 11;
            if sample != 4 && sample != 9 {
                let level = (sample % 5) as u8;
                cell = cell.level(level).value(format!("{level} runs"));
            }
            cells.push(cell);
        }
    }
    stack(&theme)
        .w(px(560.0))
        .child(caption(
            &theme,
            "January to March, by weekday: five intensity steps, and a missing \
             cell is not a measured zero",
        ))
        .child(
            Heatmap::new("scene.heatmap.ready", "Fixture activity")
                .rows(days)
                // The ramp is neutral until a caller says whose quantity it
                // is. This matrix is the run activity the rest of the frame
                // is already reading in the accent, so it hands that over.
                .tint(theme.colors.accent)
                .columns(weeks.clone())
                .cells(cells),
        )
        .child(caption(&theme, "Nothing measured in the period at all"))
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
    // A second metric is a second shape. Two readings that draw the same
    // curve prove only that both were handed the same fixture.
    let queue = [
        SparklinePoint::new(0.0, 0.72),
        SparklinePoint::new(0.14, 0.64),
        SparklinePoint::new(0.28, 0.80),
        SparklinePoint::new(0.42, 0.45),
        SparklinePoint::new(0.57, 0.38),
        SparklinePoint::new(0.71, 0.52),
        SparklinePoint::new(0.85, 0.30),
        SparklinePoint::new(1.0, 0.34),
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
        .child(caption(
            &theme,
            "a reading that is no longer verified draws quieter, and its latest \
             sample is a ring rather than a mark",
        ))
        .child(Sparkline::new(
            "scene.sparkline.stale",
            "Fixture queue depth",
            SparklineState::Stale {
                reading: SparklineReading::new(queue, "34 jobs", "8 jobs", "41 jobs"),
                reason: "The latest sample is unavailable; the verified reading remains.".into(),
            },
        ))
        .child(caption(
            &theme,
            "a series the caller has already spent a colour on",
        ))
        .child(
            Sparkline::new(
                "scene.sparkline.tinted",
                "Fixture tokens",
                SparklineState::Ready(SparklineReading::new(points, "1.2k", "0.4k", "1.4k")),
            )
            .tint(identity_tint(&theme, "agent.read")),
        )
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
        .child(caption(
            &theme,
            "Inset, for a rule inside a padded container that should not reach its \
             corners",
        ))
        .child(
            div()
                .column()
                .w_full()
                .py(px(theme.space(Space::Sm)))
                .radius(&theme, Radius::Card)
                .surface(&theme, Surface::Panel)
                .child(
                    div()
                        .px(px(theme.space(Space::Md)))
                        .py(px(theme.space(Space::Xs)))
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Label,
                            "Workspace",
                        )),
                )
                .child(Divider::new().id("scene.divider.inset").inset(Space::Md))
                .child(
                    div()
                        .px(px(theme.space(Space::Md)))
                        .py(px(theme.space(Space::Xs)))
                        .child(caption(&theme, "Two rows, one rule between them")),
                ),
        )
        .child(caption(&theme, "Standing up, between two columns"))
        .child(
            div()
                .row()
                .items_stretch()
                .h(px(64.0))
                .w_full()
                .gap(px(theme.space(Space::Md)))
                .child(div().flex_1().child(caption(&theme, "Left column")))
                .child(Divider::new().id("scene.divider.vertical").vertical())
                .child(div().flex_1().child(caption(&theme, "Right column"))),
        )
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
                        .tint(identity_tint(&theme, "agent.external"))
                        .on_remove(|_, _| {}),
                )
                .child(
                    Tag::new("scene.tag.grace", "Grace")
                        .tint(identity_tint(&theme, "agent.shell"))
                        .on_remove(|_, _| {}),
                ),
        )
        .child(caption(
            &theme,
            "Read-only: nothing offers to remove it, because no handler was given",
        ))
        .child(row(&theme).child(Tag::new("scene.tag.plain", "read-only")))
        .child(caption(
            &theme,
            "Disabled: the host refuses to act on it at all, and it dims to say so",
        ))
        .child(
            row(&theme).child(
                Tag::new("scene.tag.pinned", "pinned")
                    .disabled(true)
                    .on_remove(|_, _| {}),
            ),
        )
        .child(caption(
            &theme,
            "Singled out by the keyboard, which is what the next keystroke acts on",
        ))
        .child(
            row(&theme).child(
                Tag::new("scene.tag.selected", "selected")
                    .selected(true)
                    .on_remove(|_, _| {}),
            ),
        )
        .child(caption(
            &theme,
            "The shared tiers, resolved against a palette colour",
        ))
        .child(
            row(&theme).children(
                [
                    Variant::Filled,
                    Variant::Light,
                    Variant::Outline,
                    Variant::Subtle,
                ]
                .map(|tier| {
                    Tag::new(format!("scene.tag.lime.{}", tier.name()), tier.name())
                        .variant(tier)
                        .color("lime")
                        .on_remove(|_, _| {})
                }),
            ),
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
            "An identity tint the caller owns, and no name at all — which is drawn \
             as its own state rather than as a picture that failed to arrive",
        ))
        .child(
            row(&theme)
                .child(
                    Avatar::new("Grace Hopper")
                        .tint(identity_tint(&theme, "agent.shell"))
                        .id("scene.avatar.tinted"),
                )
                .child(Avatar::new("").id("scene.avatar.anonymous")),
        )
        .child(caption(
            &theme,
            "Presence, which the host knows and the mark does not derive. No dot at \
             all is not the same claim as offline",
        ))
        .child(
            row(&theme)
                .gap(px(theme.space(Space::Md)))
                .child(
                    Avatar::new("Ada Lovelace")
                        .presence(AvatarPresence::Online)
                        .id("scene.avatar.online"),
                )
                .child(
                    Avatar::new("Grace Hopper")
                        .presence(AvatarPresence::Away)
                        .id("scene.avatar.away"),
                )
                .child(
                    Avatar::new("Katherine Johnson")
                        .presence(AvatarPresence::Busy)
                        .id("scene.avatar.busy"),
                )
                .child(
                    Avatar::new("Ada Lovelace")
                        .presence(AvatarPresence::Offline)
                        .id("scene.avatar.offline"),
                ),
        )
        .child(caption(
            &theme,
            "A stack, where each mark is cut out of the one behind it and the \
             remainder is a count the host supplied",
        ))
        .child(
            row(&theme).child(
                AvatarGroup::new()
                    .id("scene.avatar.group")
                    .size(32.0)
                    .members([
                        Avatar::new("Ada Lovelace").presence(AvatarPresence::Online),
                        Avatar::new("Grace Hopper").presence(AvatarPresence::Away),
                        Avatar::new("Katherine Johnson"),
                    ])
                    .overflow("+4"),
            ),
        )
        .child(caption(&theme, "Sizes"))
        .child(
            row(&theme)
                .items_center()
                .gap(px(theme.space(Space::Md)))
                .child(
                    Avatar::new("Ada Lovelace")
                        .size(20.0)
                        .id("scene.avatar.small"),
                )
                .child(Avatar::new("Ada Lovelace").id("scene.avatar.default"))
                .child(
                    Avatar::new("Ada Lovelace")
                        .size(40.0)
                        .id("scene.avatar.medium"),
                )
                .child(
                    Avatar::new("Ada Lovelace")
                        .size(56.0)
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
    // An empty state fills the region whose contents are missing. Drawn on
    // bare canvas it reads as loose centred text with no edge to be centred
    // in, which is the one thing it never is in a product.
    let region = |state: EmptyState| {
        div()
            .w_full()
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .child(state)
    };
    stack(&theme)
        .w(px(560.0))
        .child(caption(&theme, "Nothing has been started yet"))
        .child(region(
            EmptyState::new("scene.empty-state.unstarted", "No runs yet")
                .kind(EmptyKind::Unstarted)
                .detail("A run appears here once one has been started."),
        ))
        .child(caption(&theme, "The host refused, and says so"))
        .child(region(
            EmptyState::new(
                "scene.empty-state.unavailable",
                "The host refused the request",
            )
            .kind(EmptyKind::Unavailable)
            .detail("Approval is required for this workspace.")
            .action(
                Button::new("scene.empty-state.retry")
                    .label("Try again")
                    .on_click(|_, _| {}),
            ),
        ))
        .child(caption(&theme, "A collection that really is empty"))
        .child(region(
            EmptyState::new("scene.empty-state.empty", "No runs match “failing”")
                .kind(EmptyKind::Empty)
                .detail("Clear the filter to see every run."),
        ))
        .child(caption(&theme, "It was tried, and it failed"))
        .child(region(
            EmptyState::new("scene.empty-state.failed", "The run could not be read")
                .kind(EmptyKind::Failed)
                .detail("The snapshot on disk is from a newer version of the format.")
                .action(
                    Button::new("scene.empty-state.reload")
                        .label("Reload")
                        .on_click(|_, _| {}),
                ),
        ))
        .child(caption(
            &theme,
            "The host refused because the reader is not allowed",
        ))
        .child(region(
            EmptyState::new("scene.empty-state.unauthorized", "This workspace is locked")
                .kind(EmptyKind::Unauthorized)
                .detail("Ask an owner to grant access."),
        ))
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
        .child(caption(
            &theme,
            "A report the reader may put away, because the caller gave it somewhere to go",
        ))
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
        .child(caption(
            &theme,
            "A refusal the caller did not offer a way out of, so no dismiss appears",
        ))
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
                .detail("The three that failed are still in the queue.")
                .action(
                    Button::new("scene.outcome.review")
                        .label("Review the three")
                        .secondary()
                        .small()
                        .on_click(|_, _| {}),
                ),
        )
        .child(caption(
            &theme,
            "Did not finish. What the reader can do about it is a control, not a \
             sentence about a control",
        ))
        .child(
            OutcomePanel::new("scene.outcome.failed", OutcomeKind::Failed)
                .detail("The host closed the connection before any file landed.")
                .action(
                    Button::new("scene.outcome.retry")
                        .label("Import again")
                        .small()
                        .on_click(|_, _| {}),
                ),
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

    // A readout is a reading, so it is drawn as one: the same card, caption
    // and hierarchy a KPI gets. A number floating on the canvas beside its
    // label is a debug print of the value, not a report of it.
    let readout = |label: &'static str, detail: &'static str, number: AnimatedNumber| {
        div()
            .column()
            .flex_1()
            .min_w_0()
            .gap(px(theme.space(Space::Xs)))
            .p(px(theme.space(Space::Md)))
            .card_surface(&theme, CardVariant::Outlined)
            .child(
                crate::foundation::text(&theme, TypeScale::Caption, label)
                    .text_tone(&theme, TextTone::Muted),
            )
            .child(number)
            .child(
                crate::foundation::text(&theme, TypeScale::Caption, detail)
                    .text_tone(&theme, TextTone::Faint),
            )
    };

    stack(&theme)
        .w(px(520.0))
        .child(caption(
            &theme,
            "the published value is the target, from the frame it changes",
        ))
        .child(
            div()
                .row()
                .items_stretch()
                .w_full()
                .gap(px(theme.space(Space::Md)))
                .child(readout(
                    "Runs this week",
                    "counted by the host",
                    AnimatedNumber::new("scene.number.runs", runs)
                        .format(grouped)
                        .type_scale(TypeScale::Title),
                ))
                .child(readout(
                    "Median duration",
                    "one decimal, the host's choice",
                    AnimatedNumber::new("scene.number.seconds", seconds)
                        .format(|value| format!("{value:.1}s"))
                        .type_scale(TypeScale::Title),
                )),
        )
        .child(
            div().row().child(
                Button::new("scene.number.recount")
                    .label("Recount")
                    .on_click(|_, cx| {
                        cx.update_global::<SceneCounts, ()>(|counts, _| {
                            counts.runs += 318.0;
                            counts.seconds += 4.7;
                        });
                        cx.refresh_windows();
                    }),
            ),
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
        // The facts are a section of a detail page rather than prose loose on
        // the canvas, and the list is drawn the way a detail page would hold
        // it: on a surface with an edge the two columns are read against.
        .child(
            div()
                .column()
                .card_surface(&theme, CardVariant::Elevated)
                .p_token(&theme, Space::Md)
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
                ),
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
        .child(caption(
            &theme,
            "unknown, stalled, and paused work remain different facts",
        ))
        .child(
            row(&theme)
                .gap(px(theme.spacing.lg))
                .child(
                    div()
                        .column()
                        .items_center()
                        .gap_token(&theme, Space::Xs)
                        .child(
                            ProgressCircle::new("scene.progress-circle.stalled")
                                .label("Upload stalled")
                                .stalled(true),
                        )
                        .child(caption(&theme, "Stalled")),
                )
                .child(
                    div()
                        .column()
                        .items_center()
                        .gap_token(&theme, Space::Xs)
                        .child(
                            ProgressCircle::new("scene.progress-circle.paused")
                                .label("Upload paused")
                                .paused(true),
                        )
                        .child(caption(&theme, "Paused")),
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
        // At the size a glyph is actually drawn, a stroke two steps apart on
        // the text ramp is a few pixels of difference nobody can hold side by
        // side. Each tone is shown once large enough to be compared and once
        // at the size it is used, on a panel so the comparison is against one
        // ground rather than against the canvas at three different insets.
        .child(
            row(&theme)
                .gap(px(theme.space(Space::Md)))
                .children(tones.map(|(label, tone)| {
                    div()
                        .column()
                        .items_center()
                        .gap(px(theme.space(Space::Xs)))
                        .w(px(96.0))
                        .p(px(theme.space(Space::Sm)))
                        .radius(&theme, Radius::Card)
                        .surface(&theme, Surface::Panel)
                        .child(
                            IconView::named(format!("scene.icon.tone.{label}"), Icon::Info, label)
                                .tone(tone)
                                .large(),
                        )
                        .child(
                            div()
                                .row()
                                .gap(px(theme.space(Space::Xs)))
                                .child(IconView::new(Icon::Check).tone(tone))
                                .child(IconView::new(Icon::Danger).tone(tone))
                                .child(IconView::new(Icon::Refresh).tone(tone)),
                        )
                        .child(
                            crate::foundation::text(&theme, TypeScale::Caption, label)
                                .text_tone(&theme, TextTone::Muted),
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
