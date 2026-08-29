//! A controlled presentation of caller-owned frame timing diagnostics.
//!
//! [`gpui::FrameTimingMonitor`] owns collection and bounded history at the
//! framework boundary. This component owns neither a monitor nor a clock and
//! never requests another frame. The caller polls on its own workload policy,
//! stores the resulting state, and decides whether the detail rows are open.

use std::{rc::Rc, time::Duration};

use gpui::{
    AnyElement, App, FrameTimingSummary, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TextTone, TypeScale};

use crate::controls::button::Button;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::loading::PulseLoader;
use crate::display::sparkline::{Sparkline, SparklinePoint, SparklineReading, SparklineState};
use crate::foundation::{CardVariant, Ident, Sizable, StyledExt};
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type ExpandedHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// The diagnostics fact a host currently has.
#[derive(Debug, Clone, PartialEq)]
pub enum PerformanceHudState {
    /// Tracing is available but fewer than two target-window draws exist.
    Waiting,
    /// A framework-produced summary over the caller's chosen window/history.
    Ready(FrameTimingSummary),
    /// The host cannot provide frame timings, with its exact reason.
    Unavailable(SharedString),
}

impl PerformanceHudState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Ready(_) => "ready",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

impl HasPhase for PerformanceHudState {
    fn phase(&self) -> Phase {
        match self {
            Self::Waiting => Phase::Loading,
            Self::Ready(_) => Phase::Ready,
            Self::Unavailable(_) => Phase::Unavailable,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(reason) => Some(reason.as_ref()),
            _ => None,
        }
    }
}

/// A compact or expanded frame diagnostics surface.
///
/// `expanded` is controlled. Activating its button only reports the next
/// value through `on_expanded`; it never mutates caller state.
#[derive(IntoElement)]
pub struct PerformanceHud {
    ident: Ident,
    state: PerformanceHudState,
    expanded: bool,
    on_expanded: Option<ExpandedHandler>,
}

impl std::fmt::Debug for PerformanceHud {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PerformanceHud")
            .field("ident", &self.ident)
            .field("state", &self.state)
            .field("expanded", &self.expanded)
            .field("has_expanded_handler", &self.on_expanded.is_some())
            .finish()
    }
}

impl PerformanceHud {
    pub fn new(ident: impl Into<Ident>, state: PerformanceHudState) -> Self {
        Self {
            ident: ident.into(),
            state,
            expanded: false,
            on_expanded: None,
        }
    }

    /// The detail state the caller currently owns.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Reports the detail state requested by the expand/collapse action.
    pub fn on_expanded(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_expanded = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PerformanceHud {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let title = cx.strings().text(StringKey::PerformanceTitle);
        let root_id = self.ident.semantic_id();
        let body = match &self.state {
            PerformanceHudState::Waiting => div()
                .flex()
                .items_center()
                .justify_center()
                .min_h(px(92.0))
                .child(
                    PulseLoader::new(self.ident.child("waiting"))
                        .label(cx.strings().text(StringKey::PerformanceWaiting)),
                )
                .into_any_element(),
            PerformanceHudState::Unavailable(reason) => EmptyState::new(
                self.ident.child("unavailable"),
                cx.strings().text(StringKey::PerformanceUnavailable),
            )
            .kind(EmptyKind::Unavailable)
            .detail(reason.clone())
            .into_any_element(),
            PerformanceHudState::Ready(summary) => {
                ready_body(&self.ident, summary, self.expanded, cx)
            }
        };

        let expand = self.on_expanded.map(|handler| {
            let next = !self.expanded;
            let label = if self.expanded {
                cx.strings().text(StringKey::PerformanceCollapse)
            } else {
                cx.strings().text(StringKey::PerformanceExpand)
            };
            Button::new(self.ident.child("expanded"))
                .label(label)
                .ghost()
                .small()
                .checked_state(self.expanded)
                .semantic_parent(root_id.clone())
                .on_click(move |window, cx| handler(next, window, cx))
        });

        div()
            .column()
            .w_full()
            .min_w(px(280.0))
            .gap_token(&theme, Space::Sm)
            .p_token(&theme, Space::Md)
            .card_surface(&theme, CardVariant::Outlined)
            .child(
                div()
                    .row()
                    .items_center()
                    .justify_between()
                    .gap_token(&theme, Space::Sm)
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Label)
                            .text_tone(&theme, TextTone::Primary)
                            .child(title.clone()),
                    )
                    .children(expand),
            )
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(root_id, Role::Status)
                    .text(title)
                    .value(self.state.name())
                    .busy(matches!(self.state, PerformanceHudState::Waiting)),
            )
    }
}

fn ready_body(ident: &Ident, summary: &FrameTimingSummary, expanded: bool, cx: &App) -> AnyElement {
    let theme = cx.theme().clone();
    let parent = ident.semantic_id();
    let fps = format_fps(summary.frames_per_second, cx);
    let mean_draw = format_duration(summary.mean_draw_duration, cx);
    let points = normalized_draws(summary);
    let maximum = summary
        .draw_durations
        .iter()
        .copied()
        .max()
        .unwrap_or(summary.frame_budget)
        .max(summary.frame_budget);
    let zero = format_duration(Duration::ZERO, cx);
    let maximum = format_duration(maximum, cx);

    let mut body = div()
        .column()
        .w_full()
        .gap_token(&theme, Space::Sm)
        .child(
            div()
                .row()
                .items_end()
                .justify_between()
                .gap_token(&theme, Space::Md)
                .child(
                    stat(
                        ident.child("fps"),
                        &parent,
                        cx.strings().text(StringKey::PerformanceFps),
                        fps.clone(),
                        true,
                        cx,
                    )
                    .flex_1(),
                )
                .child(
                    stat(
                        ident.child("mean-draw"),
                        &parent,
                        cx.strings().text(StringKey::PerformanceMeanDraw),
                        mean_draw,
                        true,
                        cx,
                    )
                    .flex_1(),
                ),
        )
        .child(
            div().w_full().h(px(56.0)).child(
                Sparkline::new(
                    ident.child("draw-history"),
                    cx.strings().text(StringKey::PerformanceDrawHistory),
                    SparklineState::Ready(SparklineReading::new(points, fps, zero, maximum)),
                )
                .embedded(),
            ),
        );

    if expanded {
        let mut details = div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .pt(px(theme.space(Space::Xs)))
            .border_t_1()
            .border_color(theme.colors.divider)
            .child(stat(
                ident.child("p95-draw"),
                &parent,
                cx.strings().text(StringKey::PerformanceP95Draw),
                format_duration(summary.p95_draw_duration, cx),
                false,
                cx,
            ))
            .child(stat(
                ident.child("budget"),
                &parent,
                cx.strings().text(StringKey::PerformanceFrameBudget),
                format_duration(summary.frame_budget, cx),
                false,
                cx,
            ))
            .child(stat(
                ident.child("over-budget"),
                &parent,
                cx.strings().text(StringKey::PerformanceOverBudget),
                cx.numbers().percent(summary.over_budget_fraction as f32),
                false,
                cx,
            ))
            .child(stat(
                ident.child("invalidations"),
                &parent,
                cx.strings().text(StringKey::PerformanceInvalidations),
                cx.numbers().decimal(summary.mean_invalidations, 1),
                false,
                cx,
            ))
            .child(stat(
                ident.child("samples"),
                &parent,
                cx.strings().text(StringKey::PerformanceSamples),
                cx.numbers().count(summary.sample_count),
                false,
                cx,
            ));
        if let Some(latency) = summary.mean_dirty_to_draw_duration {
            details = details.child(stat(
                ident.child("dirty-to-draw"),
                &parent,
                cx.strings().text(StringKey::PerformanceDirtyToDraw),
                format_duration(latency, cx),
                false,
                cx,
            ));
        }
        body = body.child(details);
    }
    body.into_any_element()
}

fn stat(
    ident: Ident,
    parent: &SharedString,
    label: SharedString,
    value: SharedString,
    prominent: bool,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let theme = cx.theme().clone();
    div()
        .row()
        .items_baseline()
        .justify_between()
        .gap_token(&theme, Space::Sm)
        .child(
            div()
                .type_scale(&theme, TypeScale::Caption)
                .text_tone(&theme, TextTone::Muted)
                .child(label.clone()),
        )
        .child(
            div()
                .type_scale(
                    &theme,
                    if prominent {
                        TypeScale::Title
                    } else {
                        TypeScale::Code
                    },
                )
                .text_tone(&theme, TextTone::Primary)
                .child(value.clone()),
        )
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Text)
                .parent(parent.clone())
                .text(label)
                .value(value),
        )
}

fn normalized_draws(summary: &FrameTimingSummary) -> Vec<SparklinePoint> {
    let maximum = summary
        .draw_durations
        .iter()
        .copied()
        .max()
        .unwrap_or(summary.frame_budget)
        .max(summary.frame_budget)
        .as_secs_f32()
        .max(f32::EPSILON);
    let denominator = summary.draw_durations.len().saturating_sub(1).max(1) as f32;
    summary
        .draw_durations
        .iter()
        .enumerate()
        .map(|(index, duration)| {
            SparklinePoint::new(
                index as f32 / denominator,
                (duration.as_secs_f32() / maximum).clamp(0.0, 1.0),
            )
        })
        .collect()
}

fn format_fps(value: f64, cx: &App) -> SharedString {
    cx.strings().format(
        StringKey::PerformanceFpsValue,
        &[cx.numbers().decimal(value, 1).as_ref()],
    )
}

fn format_duration(duration: Duration, cx: &App) -> SharedString {
    cx.strings().format(
        StringKey::PerformanceMilliseconds,
        &[cx.numbers()
            .decimal(duration.as_secs_f64() * 1_000.0, 1)
            .as_ref()],
    )
}
