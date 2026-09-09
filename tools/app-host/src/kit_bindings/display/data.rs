//! Structured native display data and its rendering, kept separate from glyphs
//! and small status builders so conversion rules have one local owner.
use super::*;
use gpui_kit::display::{metric::*, sparkline::SparklinePoint};
use gpui_kit::state::{HasPhase, Phase};
use std::time::Duration;

struct SurfacePhase<'a>(&'a Value);
impl HasPhase for SurfacePhase<'_> {
    fn phase(&self) -> Phase {
        match self.0["kind"].as_str() {
            Some("queued") => Phase::Queued,
            Some("blocked") => Phase::Blocked,
            Some("loading") => Phase::Loading,
            Some("refreshing") => Phase::Refreshing,
            Some("ready") => Phase::Ready,
            Some("empty") => Phase::Empty,
            Some("unavailable") => Phase::Unavailable,
            Some("error") => Phase::Error,
            Some("cancelled") => Phase::Cancelled,
            _ => Phase::Idle,
        }
    }
    fn reason(&self) -> Option<&str> {
        self.0["reason"].as_str()
    }
    fn is_stale(&self) -> bool {
        boolean(self.0, "stale")
    }
}
fn heat_axis(v: &Value) -> HeatAxis {
    let mut a = HeatAxis::new(string(v, "id"), string(v, "label"));
    if v.get("group").is_some() {
        a = a.group(string(v, "group"));
    }
    a
}
fn trace_spans(v: &Value) -> Vec<TraceSpan> {
    list(v)
        .map(|s| {
            let mut span = TraceSpan::new(
                string(s, "id"),
                string(s, "label"),
                scalar(s, "start"),
                scalar(s, "end"),
            )
            .depth(integer(s, "depth") as u32)
            .state(match s["state"].as_str() {
                Some("running") => SpanState::Running,
                Some("succeeded") => SpanState::Succeeded,
                Some("failed") => SpanState::Failed,
                _ => SpanState::Pending,
            });
            if s.get("detail").is_some() {
                span = span.detail(string(s, "detail"));
            }
            if s.get("duration").is_some() {
                span = span.duration(string(s, "duration"));
            }
            span
        })
        .collect()
}
fn entries(v: &Value, slots: &KitSlots, w: &mut Window, c: &mut App) -> Vec<TimelineEntry> {
    list(v)
        .map(|e| {
            let mut entry = TimelineEntry::new(string(e, "id"), string(e, "description"))
                .tone(tone(&string(e, "tone")));
            if let Some(time) = e["time"].as_str() {
                entry = entry.time(time.to_owned());
            } else {
                entry = entry.time_unknown();
            }
            if e.get("actor").is_some() {
                entry = entry.actor(string(e, "actor"));
            }
            if let Some(f) = slots.get(&string(e, "id")) {
                entry = entry.detail(f(w, c));
            }
            entry
        })
        .collect()
}
fn metric_reading(v: &Value) -> MetricReading {
    let mut r = MetricReading::new(string(v, "value"));
    if v.get("delta").is_some() {
        r = r.delta(string(v, "delta"), tone(&string(v, "tone")));
    }
    if let Some(d) = v["direction"].as_str() {
        r = r.direction(match d {
            "up" => DeltaDirection::Up,
            "down" => DeltaDirection::Down,
            _ => DeltaDirection::Flat,
        });
    }
    if v.get("trend").is_some() {
        r = r.trend(list(&v["trend"]).map(|p| SparklinePoint::new(scalar(p, "x"), scalar(p, "y"))));
    }
    r
}
fn duration(v: &Value) -> Duration {
    Duration::from_secs_f64(v.as_f64().unwrap_or_default() / 1000.)
}
fn timing(v: &Value) -> gpui::FrameTimingSummary {
    let optional = |key: &str| v.get(key).filter(|v| !v.is_null()).map(duration);
    gpui::FrameTimingSummary {
        sample_count: integer(v, "sampleCount"),
        frames_per_second: v["framesPerSecond"].as_f64().unwrap_or_default(),
        frame_budget: duration(&v["frameBudgetMs"]),
        mean_draw_duration: duration(&v["meanDrawMs"]),
        p95_draw_duration: duration(&v["p95DrawMs"]),
        over_budget_fraction: v["overBudgetFraction"].as_f64().unwrap_or_default(),
        mean_invalidations: v["meanInvalidations"].as_f64().unwrap_or_default(),
        mean_dirty_to_draw_duration: optional("meanDirtyToDrawMs"),
        mean_submission_duration: duration(&v["meanSubmissionMs"]),
        mean_dirty_to_submission_duration: optional("meanDirtyToSubmissionMs"),
        mean_input_to_submission_duration: optional("meanInputToSubmissionMs"),
        mean_input_events: v["meanInputEvents"].as_f64().unwrap_or_default(),
        draw_durations: list(&v["drawDurationsMs"]).map(duration).collect(),
        submission_durations: list(&v["submissionDurationsMs"]).map(duration).collect(),
    }
}

pub(super) fn render_data(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
) -> AnyElement {
    let id = node.id.clone();
    let p = Value::Object(node.props.clone().into_iter().collect());
    let action = |name: &str| {
        if flag(node, "disabled") {
            None
        } else {
            node.events.get(name).cloned()
        }
    };
    match node.component.as_deref().unwrap_or_default() {
        "AnimatedNumber" => {
            let mut b = AnimatedNumber::new(id, p["value"].as_f64().unwrap_or_default());
            if let Some(f) = p.get("format") {
                let decimals = integer(f, "decimals");
                let prefix = string(f, "prefix");
                let suffix = string(f, "suffix");
                b = b.format(move |v| format!("{prefix}{v:.decimals$}{suffix}"));
            }
            if let Some(s) = p.get("spec") {
                let curve = &s["curve"];
                let spec = if let Some(spring) = s.get("spring") {
                    gpui_kit::motion::MotionSpec::sprung(gpui_kit::motion::Spring::new(
                        scalar(spring, "stiffness"),
                        scalar(spring, "damping"),
                        scalar(spring, "mass"),
                    ))
                } else {
                    gpui_kit::motion::MotionSpec::new(
                        integer(s, "durationMs") as u64,
                        gpui_kit::motion::CubicBezier::new(
                            scalar(curve, "x1"),
                            scalar(curve, "y1"),
                            scalar(curve, "x2"),
                            scalar(curve, "y2"),
                        ),
                    )
                };
                b = b.spec(spec.with_delay(integer(s, "delayMs") as u64));
            }
            if let Some(s) = p["typeScale"].as_str() {
                use gpui_kit_theme::TypeScale;
                b = b.type_scale(match s {
                    "caption" => TypeScale::Caption,
                    "label" => TypeScale::Label,
                    "body" => TypeScale::Body,
                    "strong" => TypeScale::Strong,
                    "subtitle" => TypeScale::Subtitle,
                    "code" => TypeScale::Code,
                    _ => TypeScale::Title,
                });
            }
            b.into_any_element()
        }
        "DescriptionList" => {
            let mut b = DescriptionList::new(id).items(list(&p["items"]).map(|i| {
                let v = &i["value"];
                let value = match v["kind"].as_str() {
                    Some("text") => DescriptionValue::text(string(v, "text")),
                    Some("redacted") => DescriptionValue::redacted(string(v, "text")),
                    Some("notApplicable") => DescriptionValue::NotApplicable,
                    _ => DescriptionValue::Unknown,
                };
                DescriptionItem::new(string(i, "id"), string(i, "term"), value)
                    .copyable(boolean(i, "copyable") && !flag(node, "disabled"))
            }));
            if p.get("columns").is_some() {
                b = b.columns(integer(&p, "columns"));
            }
            if let Some(a) = action("copy") {
                b = b.on_copy(move |id, _, _| emit(&a, json!(id.as_ref())));
            }
            b.into_any_element()
        }
        "Heatmap" => {
            let mut b = Heatmap::new(id, text(node, "label"))
                .rows(list(&p["rows"]).map(heat_axis))
                .columns(list(&p["columns"]).map(heat_axis))
                .cells(list(&p["cells"]).map(|c| {
                    let mut cell =
                        HeatCell::new(string(c, "id"), string(c, "row"), string(c, "column"))
                            .label(string(c, "label"))
                            .value(string(c, "value"));
                    if let Some(level) = c["level"].as_u64() {
                        cell = cell.level(level as u8);
                    }
                    cell
                }));
            if let Some(c) = p.get("tint") {
                b = b.tint(tint(c));
            }
            if let Some(s) = p.get("state") {
                b = b.state(match s["kind"].as_str() {
                    Some("loading") => HeatmapState::Loading,
                    Some("empty") => HeatmapState::Empty,
                    Some("unavailable") => HeatmapState::Unavailable(string(s, "reason").into()),
                    Some("error") => HeatmapState::Error(string(s, "reason").into()),
                    _ => HeatmapState::Ready,
                });
            }
            slotted(b, &slots).into_any_element()
        }
        "MetricCard" => {
            let s = &p["state"];
            let state = match s["kind"].as_str() {
                Some("loading") => MetricState::Loading,
                Some("empty") => MetricState::Empty,
                Some("unavailable") => MetricState::Unavailable(string(s, "reason").into()),
                Some("error") => MetricState::Error(string(s, "reason").into()),
                Some("stale") => MetricState::Stale {
                    reading: metric_reading(&s["data"]),
                    reason: string(s, "reason").into(),
                },
                _ => MetricState::Ready(metric_reading(&s["data"])),
            };
            let mut b = MetricCard::new(id, text(node, "label"), state);
            if let Some(c) = p.get("tint") {
                b = b.tint(tint(c));
            }
            slotted(b, &slots).into_any_element()
        }
        "PerformanceHud" => {
            let s = &p["state"];
            let state = match s["kind"].as_str() {
                Some("ready") => PerformanceHudState::Ready(timing(&s["data"])),
                Some("unavailable") => PerformanceHudState::Unavailable(string(s, "reason").into()),
                _ => PerformanceHudState::Waiting,
            };
            let mut b = PerformanceHud::new(id, state).expanded(flag(node, "expanded"));
            if let Some(a) = action("expanded") {
                b = b.on_expanded(move |v, _, _| emit(&a, json!(v)));
            }
            b.into_any_element()
        }
        "StageProgress" => StageProgress::new(id)
            .stages(list(&p["stages"]).map(|s| {
                ProgressStage::new(
                    string(s, "id"),
                    string(s, "label"),
                    match s["status"].as_str() {
                        Some("active") => StageStatus::Active,
                        Some("done") => StageStatus::Done,
                        Some("failed") => StageStatus::Failed,
                        _ => StageStatus::Pending,
                    },
                )
            }))
            .into_any_element(),
        "StateView" => {
            let mut b = if boolean(&p, "fromAsync") {
                use gpui_kit::state::{AsyncStatus, AsyncValue};
                let s = &p["state"];
                let status = match s["kind"].as_str() {
                    Some("loading") => AsyncStatus::Loading,
                    Some("refreshing") => AsyncStatus::Refreshing,
                    Some("ready") => AsyncStatus::Ready,
                    Some("empty") => AsyncStatus::Empty,
                    Some("unavailable") => AsyncStatus::Unavailable(string(s, "reason")),
                    Some("error") => AsyncStatus::Error(string(s, "reason")),
                    _ => AsyncStatus::Idle,
                };
                let value = AsyncValue {
                    value: slots.contains_key("content").then_some(()),
                    status,
                    attempts: 0,
                };
                StateView::from_async(id, &value, |_| {
                    slots.get("content").expect("content slot")(window, cx)
                })
            } else {
                let mut b = StateView::new(id, SurfacePhase(&p["state"]));
                if let Some(f) = slots.get("content") {
                    b = b.content(f(window, cx));
                }
                b
            };
            if let Some(ms) = p.get("elapsedMs") {
                b = b.elapsed(duration(ms));
            }
            slotted(b, &slots).into_any_element()
        }
        "Timeline" => {
            let mut b = Timeline::new(id);
            if p.get("entries").is_some() {
                b = b.entries(entries(&p["entries"], &slots, window, cx));
            }
            if p.get("groups").is_some() {
                b = b.groups(
                    list(&p["groups"])
                        .map(|g| {
                            TimelineGroup::new(string(g, "id"), string(g, "label"))
                                .entries(entries(&g["entries"], &slots, window, cx))
                        })
                        .collect::<Vec<_>>(),
                );
            }
            b.into_any_element()
        }
        "TraceView" | "SpanTimeline" => {
            macro_rules! trace {
                ($type:ident) => {{
                    let mut b = $type::new(id, text(node, "label")).spans(trace_spans(&p["spans"]));
                    if let Some(a) = p.get("axis") {
                        b = b.axis(string(a, "start"), string(a, "end"));
                    }
                    if let Some(t) = p.get("ticks") {
                        b = b.ticks(list(t).map(|t| (scalar(t, "position"), string(t, "label"))));
                    }
                    if p.get("current").is_some() {
                        b = b.current(text(node, "current"));
                    }
                    if let Some(a) = action("select") {
                        b = b.on_select(move |id, _, _| emit(&a, json!(id.as_ref())));
                    }
                    slotted(b, &slots).into_any_element()
                }};
            }
            if node.component.as_deref() == Some("TraceView") {
                trace!(TraceView)
            } else {
                trace!(SpanTimeline)
            }
        }
        _ => unreachable!("display membership dispatch"),
    }
}
