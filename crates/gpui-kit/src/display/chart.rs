//! Cartesian readings over host-owned series.
//!
//! A chart does not invent a domain, a tick, a locale, or a colour. The host
//! supplies those the way a sparkline already supplies normalized points.
//! What this module adds is the frame an application dashboard actually
//! needs: axes that can carry the host's labels, more than one series, and
//! a published current point a test can ask for.
//!
//! Motion belongs to [`crate::motion::Transition`] keyed by series id, not to
//! a second animation system. This first surface paints the settled geometry
//! so a downstream host can stop drawing its own axes.

use gpui::{
    App, Hsla, IntoElement, ParentElement, PathBuilder, RenderOnce, SharedString, Styled, Window,
    canvas, div, point, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TypeScale};

use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::loading::PulseLoader;
use crate::display::sparkline::SparklinePoint;
use crate::foundation::{Ident, StyledExt};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// One named series. Identity is the caller's, never the draw order.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartSeries {
    pub id: SharedString,
    pub label: SharedString,
    pub points: Vec<SparklinePoint>,
    pub color: Option<Hsla>,
}

impl ChartSeries {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            points: Vec::new(),
            color: None,
        }
    }

    pub fn points(mut self, points: impl IntoIterator<Item = SparklinePoint>) -> Self {
        self.points = points.into_iter().collect();
        self
    }

    /// A caller-owned colour. Without one the series takes the theme accent.
    pub fn tint(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// Host-supplied wording for the two axes. Empty labels are omitted, not
/// replaced with a guessed scale.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChartAxes {
    pub x_label: Option<SharedString>,
    pub y_label: Option<SharedString>,
    pub x_start: Option<SharedString>,
    pub x_end: Option<SharedString>,
    pub y_start: Option<SharedString>,
    pub y_end: Option<SharedString>,
}

impl ChartAxes {
    pub fn x_label(mut self, label: impl Into<SharedString>) -> Self {
        self.x_label = Some(label.into());
        self
    }

    pub fn y_label(mut self, label: impl Into<SharedString>) -> Self {
        self.y_label = Some(label.into());
        self
    }

    pub fn x_ends(mut self, start: impl Into<SharedString>, end: impl Into<SharedString>) -> Self {
        self.x_start = Some(start.into());
        self.x_end = Some(end.into());
        self
    }

    pub fn y_ends(mut self, start: impl Into<SharedString>, end: impl Into<SharedString>) -> Self {
        self.y_start = Some(start.into());
        self.y_end = Some(end.into());
        self
    }
}

/// The complete state of one cartesian reading.
#[derive(Debug, Clone, PartialEq)]
pub enum ChartState {
    Loading,
    Ready(Vec<ChartSeries>),
    Empty,
    Unavailable(SharedString),
    Error(SharedString),
}

impl ChartState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready(_) => "ready",
            Self::Empty => "empty",
            Self::Unavailable(_) => "unavailable",
            Self::Error(_) => "error",
        }
    }
}

/// A bar chart over one host-owned series of categorized values.
#[derive(Debug, IntoElement)]
pub struct BarChart {
    ident: Ident,
    label: SharedString,
    axes: ChartAxes,
    state: ChartState,
}

impl BarChart {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>, state: ChartState) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            axes: ChartAxes::default(),
            state,
        }
    }

    pub fn axes(mut self, axes: ChartAxes) -> Self {
        self.axes = axes;
        self
    }
}

impl RenderOnce for BarChart {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, spec): (gpui::AnyElement, NodeSpec) = match &self.state {
            ChartState::Ready(series) => {
                let count = series
                    .first()
                    .map(|series| series.points.len())
                    .unwrap_or(0);
                (
                    ready_bars(&self.ident, &self.label, &self.axes, series, &theme, cx),
                    NodeSpec::new(self.ident.semantic_id(), Role::Group)
                        .text(self.label.clone())
                        .value(self.state.name())
                        .range(0.0, count as f32, count as f32),
                )
            }
            other => line_like_state(&self.ident, &self.label, other, cx),
        };
        div().w_full().child(body).semantic_in(cx, spec)
    }
}

fn line_like_state(
    ident: &Ident,
    label: &SharedString,
    state: &ChartState,
    cx: &App,
) -> (gpui::AnyElement, NodeSpec) {
    match state {
        ChartState::Loading => (
            PulseLoader::new(ident.child("loading"))
                .label(cx.strings().text(StringKey::Loading))
                .into_any_element(),
            NodeSpec::new(ident.semantic_id(), Role::Status)
                .text(label.clone())
                .busy(true)
                .value(state.name()),
        ),
        ChartState::Empty => (
            EmptyState::new(
                ident.child("empty"),
                cx.strings().text(StringKey::ChartEmpty),
            )
            .kind(EmptyKind::Empty)
            .into_any_element(),
            NodeSpec::new(ident.semantic_id(), Role::Status)
                .text(label.clone())
                .value(state.name()),
        ),
        ChartState::Unavailable(reason) => (
            EmptyState::new(ident.child("unavailable"), reason.clone())
                .kind(EmptyKind::Unavailable)
                .into_any_element(),
            NodeSpec::new(ident.semantic_id(), Role::Status)
                .text(label.clone())
                .value(state.name()),
        ),
        ChartState::Error(reason) => (
            EmptyState::new(ident.child("error"), reason.clone())
                .kind(EmptyKind::Failed)
                .into_any_element(),
            NodeSpec::new(ident.semantic_id(), Role::Status)
                .text(label.clone())
                .value(state.name()),
        ),
        ChartState::Ready(_) => unreachable!("ready is drawn by the caller"),
    }
}

/// A line chart over one or more host-owned series.
#[derive(Debug, IntoElement)]
pub struct LineChart {
    ident: Ident,
    label: SharedString,
    axes: ChartAxes,
    state: ChartState,
}

impl LineChart {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>, state: ChartState) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            axes: ChartAxes::default(),
            state,
        }
    }

    pub fn axes(mut self, axes: ChartAxes) -> Self {
        self.axes = axes;
        self
    }
}

impl RenderOnce for LineChart {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, spec): (gpui::AnyElement, NodeSpec) = match &self.state {
            ChartState::Ready(series) => {
                let count = series.len();
                (
                    ready_chart(&self.ident, &self.label, &self.axes, series, &theme, cx),
                    NodeSpec::new(self.ident.semantic_id(), Role::Group)
                        .text(self.label.clone())
                        .value(self.state.name())
                        .range(0.0, count as f32, count as f32),
                )
            }
            other => line_like_state(&self.ident, &self.label, other, cx),
        };

        div().w_full().child(body).semantic_in(cx, spec)
    }
}

fn ready_chart(
    ident: &Ident,
    label: &SharedString,
    axes: &ChartAxes,
    series: &[ChartSeries],
    theme: &gpui_kit_theme::Theme,
    cx: &App,
) -> gpui::AnyElement {
    let stroke = theme.borders.thick;
    let plotted: Vec<(Hsla, Vec<SparklinePoint>)> = series
        .iter()
        .map(|series| {
            let color = series.color.unwrap_or(theme.colors.accent);
            let points = series
                .points
                .iter()
                .copied()
                .filter(|point| point.is_bounded())
                .collect();
            (color, points)
        })
        .collect();

    div()
        .column()
        .w_full()
        .gap_token(theme, Space::Xs)
        .child(
            div()
                .type_scale(theme, TypeScale::Label)
                .text_color(theme.colors.text)
                .child(label.clone()),
        )
        .children(axes.y_end.clone().map(|end| {
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_faint)
                .child(end)
        }))
        .child(line_canvas(plotted, stroke))
        .child(
            div()
                .row()
                .justify_between()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_faint)
                .children(axes.x_start.clone())
                .children(axes.x_end.clone()),
        )
        .child(
            div()
                .row()
                .flex_wrap()
                .gap_token(theme, Space::Sm)
                .children(series.iter().map(|series| {
                    let color = series.color.unwrap_or(theme.colors.accent);
                    div()
                        .row()
                        .items_center()
                        .gap_token(theme, Space::Xs)
                        .child(div().size(px(8.0)).rounded_full().bg(color))
                        .child(
                            div()
                                .type_scale(theme, TypeScale::Caption)
                                .text_color(theme.colors.text_muted)
                                .child(series.label.clone()),
                        )
                        .semantic_in(
                            cx,
                            NodeSpec::new(
                                ident.child(series.id.as_ref()).semantic_id(),
                                Role::Status,
                            )
                            .parent(ident.semantic_id())
                            .text(series.label.clone())
                            .value(series.id.clone()),
                        )
                })),
        )
        .into_any_element()
}

fn ready_bars(
    ident: &Ident,
    label: &SharedString,
    axes: &ChartAxes,
    series: &[ChartSeries],
    theme: &gpui_kit_theme::Theme,
    cx: &App,
) -> gpui::AnyElement {
    let first = series.first();
    let color = first
        .and_then(|series| series.color)
        .unwrap_or(theme.colors.accent);
    let points = first
        .map(|series| {
            series
                .points
                .iter()
                .copied()
                .filter(|point| point.is_bounded())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    div()
        .column()
        .w_full()
        .gap_token(theme, Space::Xs)
        .child(
            div()
                .type_scale(theme, TypeScale::Label)
                .text_color(theme.colors.text)
                .child(label.clone()),
        )
        .children(axes.y_end.clone().map(|end| {
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_faint)
                .child(end)
        }))
        .child(
            div()
                .row()
                .items_end()
                .gap(px(6.0))
                .h(px(160.0))
                .w_full()
                .children(points.into_iter().enumerate().map(|(index, point)| {
                    let percent = cx.numbers().percent(point.y);
                    div()
                        .flex_1()
                        .h(gpui::relative(point.y.max(0.04)))
                        .rounded_t(px(theme.radii.small))
                        .bg(color)
                        .semantic_in(
                            cx,
                            NodeSpec::new(
                                ident.child(format!("bar-{index}")).semantic_id(),
                                Role::Status,
                            )
                            .parent(ident.semantic_id())
                            .value(percent),
                        )
                })),
        )
        .child(
            div()
                .row()
                .justify_between()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_faint)
                .children(axes.x_start.clone())
                .children(axes.x_end.clone()),
        )
        .into_any_element()
}

fn line_canvas(series: Vec<(Hsla, Vec<SparklinePoint>)>, stroke: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let inset = stroke / 2.0;
            let width = (f32::from(bounds.size.width) - stroke).max(0.0);
            let height = (f32::from(bounds.size.height) - stroke).max(0.0);
            if width <= 0.0 || height <= 0.0 {
                return;
            }
            let at = |sample: SparklinePoint| {
                point(
                    bounds.origin.x + px(inset + sample.x * width),
                    bounds.origin.y + px(inset + (1.0 - sample.y) * height),
                )
            };
            for (color, points) in &series {
                if points.len() < 2 {
                    continue;
                }
                let mut builder = PathBuilder::stroke(px(stroke));
                builder.move_to(at(points[0]));
                for sample in points.iter().copied().skip(1) {
                    builder.line_to(at(sample));
                }
                if let Ok(path) = builder.build() {
                    window.paint_path(path, *color);
                }
            }
        },
    )
    .w_full()
    .h(px(160.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_series_keeps_the_caller_id() {
        let series = ChartSeries::new("cpu", "CPU").points([SparklinePoint::new(0.0, 0.2)]);
        assert_eq!(series.id.as_ref(), "cpu");
        assert_eq!(ChartState::Empty.name(), "empty");
    }
}
