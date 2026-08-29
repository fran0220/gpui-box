//! Generic normalized plots and non-Cartesian chart presentations.
//!
//! [`Plot`] owns a measured frame, stable semantic marks, keyboard traversal,
//! and truthful non-ready states. Callers own normalized geometry, exact
//! visible text, and painting. [`CandlestickChart`] and [`SankeyChart`] are
//! presentations over that same boundary; neither infers a domain or lays out
//! caller data.

use std::collections::HashSet;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Hsla, InteractiveElement, IntoElement, ParentElement, PathBuilder,
    Pixels, Point, RenderOnce, SharedString, Styled, Window, bounds, canvas, div, fill, point,
    prelude::FluentBuilder, px, relative, size,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, Theme, TypeScale};

use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::loading::PulseLoader;
use crate::display::status::StatusDot;
use crate::foundation::{FocusRing, Ident, StyledExt};
use crate::motion::keyed;
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

const PLOT_HEIGHT: f32 = 220.0;
/// Flow ribbons remain behind the nodes and are intentionally translucent.
const SANKEY_LINK_ALPHA: f32 = 0.42;

/// Loading and refresh truth for arbitrary caller-owned plot data.
#[derive(Debug, Clone, PartialEq)]
pub enum PlotState<T> {
    Loading,
    Ready(T),
    Empty,
    Unavailable(SharedString),
    Error(SharedString),
    /// A refresh failed while the last verified data remains drawable.
    Stale {
        data: T,
        reason: SharedString,
    },
}

impl<T> PlotState<T> {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready(_) => "ready",
            Self::Empty => "empty",
            Self::Unavailable(_) => "unavailable",
            Self::Error(_) => "error",
            Self::Stale { .. } => "stale",
        }
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> PlotState<U> {
        match self {
            Self::Loading => PlotState::Loading,
            Self::Ready(data) => PlotState::Ready(map(data)),
            Self::Empty => PlotState::Empty,
            Self::Unavailable(reason) => PlotState::Unavailable(reason),
            Self::Error(reason) => PlotState::Error(reason),
            Self::Stale { data, reason } => PlotState::Stale {
                data: map(data),
                reason,
            },
        }
    }

    fn visible(&self) -> Option<(&T, Option<&SharedString>)> {
        match self {
            Self::Ready(data) => Some((data, None)),
            Self::Stale { data, reason } => Some((data, Some(reason))),
            _ => None,
        }
    }
}

impl<T> HasPhase for PlotState<T> {
    fn phase(&self) -> Phase {
        match self {
            Self::Loading => Phase::Loading,
            Self::Ready(_) => Phase::Ready,
            Self::Empty => Phase::Empty,
            Self::Unavailable(_) => Phase::Unavailable,
            Self::Error(_) | Self::Stale { .. } => Phase::Error,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(reason) | Self::Error(reason) | Self::Stale { reason, .. } => {
                Some(reason.as_ref())
            }
            _ => None,
        }
    }

    fn is_stale(&self) -> bool {
        matches!(self, Self::Stale { .. })
    }
}

/// One semantic target inside a plot.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotMark {
    pub id: SharedString,
    pub label: SharedString,
    pub value: SharedString,
    /// Normalized top-left bounds in the plot's `0..=1` square.
    pub bounds: Bounds<f32>,
}

impl PlotMark {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        bounds: Bounds<f32>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: value.into(),
            bounds,
        }
    }

    pub fn is_bounded(&self) -> bool {
        let end = self.bounds.origin + self.bounds.size.into();
        self.bounds.origin.x.is_finite()
            && self.bounds.origin.y.is_finite()
            && self.bounds.size.width.is_finite()
            && self.bounds.size.height.is_finite()
            && self.bounds.origin.x >= 0.0
            && self.bounds.origin.y >= 0.0
            && self.bounds.size.width > 0.0
            && self.bounds.size.height > 0.0
            && end.x <= 1.0
            && end.y <= 1.0
    }
}

/// The measured pixel frame handed to a plot painter.
#[derive(Debug, Clone, Copy)]
pub struct PlotFrame {
    bounds: Bounds<Pixels>,
}

impl PlotFrame {
    pub fn new(bounds: Bounds<Pixels>) -> Self {
        Self { bounds }
    }

    pub fn bounds(self) -> Bounds<Pixels> {
        self.bounds
    }

    /// Maps a normalized top-left point into the measured frame.
    pub fn point(self, normalized: Point<f32>) -> Point<Pixels> {
        point(
            self.bounds.origin.x + self.bounds.size.width * normalized.x,
            self.bounds.origin.y + self.bounds.size.height * normalized.y,
        )
    }

    pub fn mark_bounds(self, normalized: Bounds<f32>) -> Bounds<Pixels> {
        bounds(
            self.point(normalized.origin),
            size(
                self.bounds.size.width * normalized.size.width,
                self.bounds.size.height * normalized.size.height,
            ),
        )
    }
}

type PlotPainter = Rc<dyn Fn(PlotFrame, &mut Window, &mut App)>;
type CurrentHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// A measured plot with caller-painted content and semantic marks.
#[derive(IntoElement)]
pub struct Plot {
    ident: Ident,
    label: SharedString,
    state: PlotState<Vec<PlotMark>>,
    painter: Option<PlotPainter>,
    current: Option<SharedString>,
    on_current: Option<CurrentHandler>,
}

impl std::fmt::Debug for Plot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Plot")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("state", &self.state)
            .field("has_painter", &self.painter.is_some())
            .field("current", &self.current)
            .field("has_handler", &self.on_current.is_some())
            .finish()
    }
}

impl Plot {
    pub fn new(
        ident: impl Into<Ident>,
        label: impl Into<SharedString>,
        state: PlotState<Vec<PlotMark>>,
    ) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            state,
            painter: None,
            current: None,
            on_current: None,
        }
    }

    pub fn paint(mut self, painter: impl Fn(PlotFrame, &mut Window, &mut App) + 'static) -> Self {
        self.painter = Some(Rc::new(painter));
        self
    }

    pub fn current(mut self, mark_id: impl Into<SharedString>) -> Self {
        self.current = Some(mark_id.into());
        self
    }

    pub fn on_current(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_current = Some(Rc::new(handler));
        self
    }
}

#[derive(Debug, Default)]
struct PlotInteraction {
    initialized: bool,
    declared: Option<SharedString>,
    current: Option<SharedString>,
}

impl RenderOnce for Plot {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state_name = self.state.name();
        let (body, spec) = match self.state {
            PlotState::Ready(marks) => {
                let marks = valid_marks(marks);
                let count = marks.len();
                (
                    ready_plot(
                        &self.ident,
                        &self.label,
                        marks,
                        None,
                        self.painter,
                        self.current,
                        self.on_current,
                        &theme,
                        window,
                        cx,
                    ),
                    NodeSpec::new(self.ident.semantic_id(), Role::Group)
                        .text(self.label.clone())
                        .value(state_name)
                        .range(0.0, count as f32, count as f32),
                )
            }
            PlotState::Stale { data, reason } => {
                let marks = valid_marks(data);
                let count = marks.len();
                (
                    ready_plot(
                        &self.ident,
                        &self.label,
                        marks,
                        Some(reason.clone()),
                        self.painter,
                        self.current,
                        self.on_current,
                        &theme,
                        window,
                        cx,
                    ),
                    NodeSpec::new(self.ident.semantic_id(), Role::Group)
                        .text(self.label.clone())
                        .description(reason)
                        .value(state_name)
                        .range(0.0, count as f32, count as f32),
                )
            }
            state => non_ready_plot(&self.ident, &self.label, state, &theme, cx),
        };
        div()
            .w_full()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .child(body)
            .semantic_in(cx, spec)
    }
}

fn valid_marks(marks: Vec<PlotMark>) -> Vec<PlotMark> {
    let mut ids = HashSet::new();
    marks
        .into_iter()
        .filter(|mark| mark.is_bounded() && ids.insert(mark.id.clone()))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn ready_plot(
    ident: &Ident,
    label: &SharedString,
    marks: Vec<PlotMark>,
    stale: Option<SharedString>,
    painter: Option<PlotPainter>,
    declared: Option<SharedString>,
    report: Option<CurrentHandler>,
    theme: &Theme,
    window: &Window,
    cx: &mut App,
) -> AnyElement {
    let interaction = keyed::slot::<PlotInteraction>(
        &ident.child("interaction").semantic_id(),
        window.window_handle().window_id(),
        cx,
    );
    {
        let mut state = interaction.borrow_mut();
        if !state.initialized || state.declared != declared {
            state.current = declared.clone();
            state.declared = declared;
            state.initialized = true;
        }
        if state
            .current
            .as_ref()
            .is_none_or(|id| !marks.iter().any(|mark| &mark.id == id))
        {
            state.current = marks.first().map(|mark| mark.id.clone());
        }
    }
    let current = interaction.borrow().current.clone();
    let readout = current
        .as_ref()
        .and_then(|id| marks.iter().find(|mark| &mark.id == id))
        .map(|mark| {
            div()
                .row()
                .gap_token(theme, Space::Xs)
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_muted)
                .child(mark.label.clone())
                .child(mark.value.clone())
        });

    let plot_id = ident.child("plot");
    let mut plot = div()
        .id(plot_id.element_id())
        .relative()
        .w_full()
        .h(px(PLOT_HEIGHT))
        .overflow_hidden()
        .radius(theme, Radius::Small)
        .surface(theme, Surface::Canvas)
        .children(painter.map(|painter| {
            canvas(
                |_, _, _| {},
                move |frame, _, window, cx| painter(PlotFrame::new(frame), window, cx),
            )
            .size_full()
        }))
        .children(marks.iter().map(|mark| {
            let selected = current.as_ref() == Some(&mark.id);
            div()
                .absolute()
                .left(relative(mark.bounds.origin.x))
                .top(relative(mark.bounds.origin.y))
                .w(relative(mark.bounds.size.width))
                .h(relative(mark.bounds.size.height))
                .semantic_in(
                    cx,
                    NodeSpec::new(
                        plot_id.child("mark").child(mark.id.as_ref()).semantic_id(),
                        Role::Image,
                    )
                    .parent(plot_id.semantic_id())
                    .text(mark.label.clone())
                    .value(mark.value.clone())
                    .selected(selected)
                    .read_only(true),
                )
        }));
    if !marks.is_empty() {
        let key_marks = Rc::new(marks);
        let key_state = Rc::clone(&interaction);
        plot = plot
            .tab_index(0)
            .focus_ring(theme)
            .on_key_down(move |event, window, cx| {
                let current = key_state.borrow().current.clone();
                if let Some(next) = step_mark(&key_marks, current.as_ref(), &event.keystroke.key) {
                    let changed = key_state.borrow().current.as_ref() != Some(&next);
                    if changed {
                        key_state.borrow_mut().current = Some(next.clone());
                        if let Some(report) = &report {
                            report(next, window, cx);
                        }
                        window.refresh();
                    }
                    cx.stop_propagation();
                }
            });
    }
    let plot = plot.semantic_in(
        cx,
        NodeSpec::new(plot_id.semantic_id(), Role::List)
            .parent(ident.semantic_id())
            .text(label.clone())
            .value(current.unwrap_or_default()),
    );

    div()
        .column()
        .w_full()
        .gap_token(theme, Space::Xs)
        .child(
            div()
                .row()
                .items_baseline()
                .justify_between()
                .gap_token(theme, Space::Sm)
                .child(
                    div()
                        .type_scale(theme, TypeScale::Label)
                        .text_color(theme.colors.text)
                        .child(label.clone()),
                )
                .children(readout),
        )
        .children(stale.map(|reason| {
            div()
                .row()
                .items_center()
                .gap_token(theme, Space::Xs)
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.warning)
                .child(StatusDot::new(Tone::Warning))
                .child(reason.clone())
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child("stale").semantic_id(), Role::Status)
                        .parent(ident.semantic_id())
                        .text(reason)
                        .value("stale"),
                )
        }))
        .child(plot)
        .into_any_element()
}

fn step_mark(
    marks: &[PlotMark],
    current: Option<&SharedString>,
    key: &str,
) -> Option<SharedString> {
    if marks.is_empty() {
        return None;
    }
    let place = current
        .and_then(|id| marks.iter().position(|mark| &mark.id == id))
        .unwrap_or_default();
    let next = match key {
        "left" | "up" => place.saturating_sub(1),
        "right" | "down" => (place + 1).min(marks.len() - 1),
        "home" => 0,
        "end" => marks.len() - 1,
        _ => return None,
    };
    Some(marks[next].id.clone())
}

fn non_ready_plot<T>(
    ident: &Ident,
    label: &SharedString,
    state: PlotState<T>,
    theme: &Theme,
    cx: &mut App,
) -> (AnyElement, NodeSpec) {
    let name = state.name();
    let (body, description, invalid): (AnyElement, Option<SharedString>, bool) = match state {
        PlotState::Loading => (
            PulseLoader::new(ident.child("loading"))
                .label(cx.strings().text(StringKey::Loading))
                .into_any_element(),
            None,
            false,
        ),
        PlotState::Empty => (
            EmptyState::new(
                ident.child("empty"),
                cx.strings().text(StringKey::ChartEmpty),
            )
            .kind(EmptyKind::Empty)
            .into_any_element(),
            None,
            false,
        ),
        PlotState::Unavailable(reason) => (
            EmptyState::new(ident.child("unavailable"), reason.clone())
                .kind(EmptyKind::Unavailable)
                .into_any_element(),
            Some(reason),
            false,
        ),
        PlotState::Error(reason) => (
            EmptyState::new(ident.child("error"), reason.clone())
                .kind(EmptyKind::Failed)
                .into_any_element(),
            Some(reason),
            true,
        ),
        PlotState::Ready(_) | PlotState::Stale { .. } => unreachable!("ready handled above"),
    };
    let mut spec = NodeSpec::new(ident.semantic_id(), Role::Status)
        .text(label.clone())
        .value(name)
        .invalid(invalid);
    if let Some(description) = description {
        spec = spec.description(description);
    }
    (
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
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(PLOT_HEIGHT))
                    .radius(theme, Radius::Small)
                    .surface(theme, Surface::Canvas)
                    .child(body),
            )
            .into_any_element(),
        spec,
    )
}

/// One caller-normalized OHLC reading.
#[derive(Debug, Clone, PartialEq)]
pub struct Candlestick {
    pub id: SharedString,
    pub x: f32,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub label: SharedString,
    pub value: SharedString,
}

impl Candlestick {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<SharedString>,
        x: f32,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            x,
            open,
            high,
            low,
            close,
            label: label.into(),
            value: value.into(),
        }
    }

    pub fn is_bounded(&self) -> bool {
        [self.x, self.open, self.high, self.low, self.close]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
            && self.low <= self.open.min(self.close)
            && self.open.max(self.close) <= self.high
    }
}

/// Candlesticks over caller-owned normalized OHLC readings.
#[derive(IntoElement)]
pub struct CandlestickChart {
    ident: Ident,
    label: SharedString,
    state: PlotState<Vec<Candlestick>>,
    body_width: f32,
    rising: Option<Hsla>,
    falling: Option<Hsla>,
    current: Option<SharedString>,
    on_current: Option<CurrentHandler>,
}

impl std::fmt::Debug for CandlestickChart {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CandlestickChart")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("state", &self.state)
            .field("body_width", &self.body_width)
            .field("rising", &self.rising)
            .field("falling", &self.falling)
            .field("current", &self.current)
            .field("has_handler", &self.on_current.is_some())
            .finish()
    }
}

impl CandlestickChart {
    pub fn new(
        ident: impl Into<Ident>,
        label: impl Into<SharedString>,
        state: PlotState<Vec<Candlestick>>,
    ) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            state,
            body_width: 0.06,
            rising: None,
            falling: None,
            current: None,
            on_current: None,
        }
    }

    pub fn body_width(mut self, normalized: f32) -> Self {
        self.body_width = normalized.clamp(0.005, 0.25);
        self
    }

    pub fn rising_tint(mut self, tint: Hsla) -> Self {
        self.rising = Some(tint);
        self
    }

    pub fn falling_tint(mut self, tint: Hsla) -> Self {
        self.falling = Some(tint);
        self
    }

    pub fn current(mut self, id: impl Into<SharedString>) -> Self {
        self.current = Some(id.into());
        self
    }

    pub fn on_current(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_current = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for CandlestickChart {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let visible = self.state.visible().map(|(data, _)| valid_candles(data));
        let marks = self
            .state
            .map(|data| candle_marks(&valid_candles(&data), self.body_width));
        let candles = visible.unwrap_or_default();
        let width = self.body_width;
        let rising = self.rising.unwrap_or(theme.colors.success);
        let falling = self.falling.unwrap_or(theme.colors.danger);
        let hairline = theme.borders.hairline;
        Plot::new(self.ident, self.label, marks)
            .paint(move |frame, window, _| {
                for candle in &candles {
                    let tint = if candle.close >= candle.open {
                        rising
                    } else {
                        falling
                    };
                    let x = candle.x;
                    let high = frame.point(point(x, 1.0 - candle.high));
                    let low = frame.point(point(x, 1.0 - candle.low));
                    let mut wick = PathBuilder::stroke(px(hairline));
                    wick.move_to(high);
                    wick.line_to(low);
                    if let Ok(path) = wick.build() {
                        window.paint_path(path, tint);
                    }
                    let top = 1.0 - candle.open.max(candle.close);
                    let height = (candle.open - candle.close).abs();
                    let body = frame.mark_bounds(bounds(
                        point((x - width / 2.0).clamp(0.0, 1.0 - width), top),
                        size(width, height.max(1.0 / PLOT_HEIGHT)),
                    ));
                    window.paint_quad(fill(body, tint));
                }
            })
            .when_some(self.current, |plot, current| plot.current(current))
            .when_some(self.on_current, |plot, report| {
                plot.on_current(move |id, window, cx| report(id, window, cx))
            })
    }
}

fn valid_candles(candles: &[Candlestick]) -> Vec<Candlestick> {
    let mut ids = HashSet::new();
    candles
        .iter()
        .filter(|candle| candle.is_bounded() && ids.insert(candle.id.clone()))
        .cloned()
        .collect()
}

fn candle_marks(candles: &[Candlestick], width: f32) -> Vec<PlotMark> {
    candles
        .iter()
        .map(|candle| {
            PlotMark::new(
                candle.id.clone(),
                candle.label.clone(),
                candle.value.clone(),
                bounds(
                    point(
                        (candle.x - width / 2.0).clamp(0.0, 1.0 - width),
                        1.0 - candle.high,
                    ),
                    size(width, (candle.high - candle.low).max(1.0 / PLOT_HEIGHT)),
                ),
            )
        })
        .collect()
}

/// One caller-laid-out Sankey node.
#[derive(Debug, Clone, PartialEq)]
pub struct SankeyNode {
    pub id: SharedString,
    pub label: SharedString,
    pub value: SharedString,
    pub bounds: Bounds<f32>,
    pub color: Option<Hsla>,
}

impl SankeyNode {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        bounds: Bounds<f32>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: value.into(),
            bounds,
            color: None,
        }
    }

    pub fn tint(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// One caller-laid-out flow ribbon.
#[derive(Debug, Clone, PartialEq)]
pub struct SankeyLink {
    pub id: SharedString,
    pub source: SharedString,
    pub target: SharedString,
    pub label: SharedString,
    pub value: SharedString,
    pub start: Point<f32>,
    pub end: Point<f32>,
    pub start_width: f32,
    pub end_width: f32,
    pub color: Option<Hsla>,
}

impl SankeyLink {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<SharedString>,
        source: impl Into<SharedString>,
        target: impl Into<SharedString>,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        start: Point<f32>,
        end: Point<f32>,
        width: f32,
    ) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            label: label.into(),
            value: value.into(),
            start,
            end,
            start_width: width,
            end_width: width,
            color: None,
        }
    }

    pub fn widths(mut self, start: f32, end: f32) -> Self {
        self.start_width = start;
        self.end_width = end;
        self
    }

    pub fn tint(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// Caller-owned node and ribbon geometry for a Sankey presentation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SankeyData {
    pub nodes: Vec<SankeyNode>,
    pub links: Vec<SankeyLink>,
}

impl SankeyData {
    pub fn new(
        nodes: impl IntoIterator<Item = SankeyNode>,
        links: impl IntoIterator<Item = SankeyLink>,
    ) -> Self {
        Self {
            nodes: nodes.into_iter().collect(),
            links: links.into_iter().collect(),
        }
    }
}

/// Sankey ribbons over caller-owned normalized layout.
#[derive(IntoElement)]
pub struct SankeyChart {
    ident: Ident,
    label: SharedString,
    state: PlotState<SankeyData>,
    current: Option<SharedString>,
    on_current: Option<CurrentHandler>,
}

impl std::fmt::Debug for SankeyChart {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SankeyChart")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("state", &self.state)
            .field("current", &self.current)
            .field("has_handler", &self.on_current.is_some())
            .finish()
    }
}

impl SankeyChart {
    pub fn new(
        ident: impl Into<Ident>,
        label: impl Into<SharedString>,
        state: PlotState<SankeyData>,
    ) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            state,
            current: None,
            on_current: None,
        }
    }

    pub fn current(mut self, id: impl Into<SharedString>) -> Self {
        self.current = Some(id.into());
        self
    }

    pub fn on_current(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_current = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SankeyChart {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let visible = self.state.visible().map(|(data, _)| valid_sankey(data));
        let marks = self.state.map(|data| sankey_marks(&valid_sankey(&data)));
        let data = visible.unwrap_or_default();
        let accent = theme.colors.accent;
        Plot::new(self.ident, self.label, marks)
            .paint(move |frame, window, _| paint_sankey(frame, &data, accent, window))
            .when_some(self.current, |plot, current| plot.current(current))
            .when_some(self.on_current, |plot, report| {
                plot.on_current(move |id, window, cx| report(id, window, cx))
            })
    }
}

fn valid_sankey(data: &SankeyData) -> SankeyData {
    let mut node_ids = HashSet::new();
    let nodes = data
        .nodes
        .iter()
        .filter(|node| {
            PlotMark::new("node", "node", "node", node.bounds).is_bounded()
                && node_ids.insert(node.id.clone())
        })
        .cloned()
        .collect::<Vec<_>>();
    let node_ids = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    let mut link_ids = HashSet::new();
    let links = data
        .links
        .iter()
        .filter(|link| {
            link_ids.insert(link.id.clone())
                && node_ids.contains(&link.source)
                && node_ids.contains(&link.target)
                && link.start.x.is_finite()
                && link.start.y.is_finite()
                && link.end.x.is_finite()
                && link.end.y.is_finite()
                && (0.0..=1.0).contains(&link.start.x)
                && (0.0..=1.0).contains(&link.start.y)
                && (0.0..=1.0).contains(&link.end.x)
                && (0.0..=1.0).contains(&link.end.y)
                && link.start.x <= link.end.x
                && link.start_width.is_finite()
                && link.end_width.is_finite()
                && link.start_width > 0.0
                && link.end_width > 0.0
                && link.start_width <= 1.0
                && link.end_width <= 1.0
        })
        .cloned()
        .collect();
    SankeyData { nodes, links }
}

fn sankey_marks(data: &SankeyData) -> Vec<PlotMark> {
    let links = data.links.iter().map(|link| {
        let half_start = link.start_width / 2.0;
        let half_end = link.end_width / 2.0;
        let top = (link.start.y - half_start)
            .min(link.end.y - half_end)
            .max(0.0);
        let bottom = (link.start.y + half_start)
            .max(link.end.y + half_end)
            .min(1.0);
        PlotMark::new(
            Ident::new("link").child(link.id.as_ref()).semantic_id(),
            link.label.clone(),
            link.value.clone(),
            bounds(
                point(link.start.x, top),
                size(
                    (link.end.x - link.start.x).max(0.001),
                    (bottom - top).max(0.001),
                ),
            ),
        )
    });
    let nodes = data.nodes.iter().map(|node| {
        PlotMark::new(
            Ident::new("node").child(node.id.as_ref()).semantic_id(),
            node.label.clone(),
            node.value.clone(),
            node.bounds,
        )
    });
    links.chain(nodes).collect()
}

fn paint_sankey(frame: PlotFrame, data: &SankeyData, accent: Hsla, window: &mut Window) {
    for link in &data.links {
        let tint = link.color.unwrap_or(accent).opacity(SANKEY_LINK_ALPHA);
        let start_top = frame.point(point(link.start.x, link.start.y - link.start_width / 2.0));
        let start_bottom = frame.point(point(link.start.x, link.start.y + link.start_width / 2.0));
        let end_top = frame.point(point(link.end.x, link.end.y - link.end_width / 2.0));
        let end_bottom = frame.point(point(link.end.x, link.end.y + link.end_width / 2.0));
        let middle = (start_top.x + end_top.x) / 2.0;
        let mut ribbon = PathBuilder::fill();
        ribbon.move_to(start_top);
        ribbon.cubic_bezier_to(
            end_top,
            point(middle, start_top.y),
            point(middle, end_top.y),
        );
        ribbon.line_to(end_bottom);
        ribbon.cubic_bezier_to(
            start_bottom,
            point(middle, end_bottom.y),
            point(middle, start_bottom.y),
        );
        ribbon.close();
        if let Ok(path) = ribbon.build() {
            window.paint_path(path, tint);
        }
    }
    for node in &data.nodes {
        window.paint_quad(fill(
            frame.mark_bounds(node.bounds),
            node.color.unwrap_or(accent),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_maps_the_normalized_square_into_measured_pixels() {
        let frame = PlotFrame::new(bounds(
            point(px(10.0), px(20.0)),
            size(px(200.0), px(100.0)),
        ));
        assert_eq!(frame.point(point(0.25, 0.5)), point(px(60.0), px(70.0)));
        assert_eq!(
            frame.mark_bounds(bounds(point(0.1, 0.2), size(0.5, 0.4))),
            bounds(point(px(30.0), px(40.0)), size(px(100.0), px(40.0)))
        );
    }

    #[test]
    fn duplicate_and_unbounded_marks_are_not_published() {
        let marks = valid_marks(vec![
            PlotMark::new("a", "A", "1", bounds(point(0.0, 0.0), size(0.2, 0.2))),
            PlotMark::new("a", "A again", "2", bounds(point(0.2, 0.2), size(0.2, 0.2))),
            PlotMark::new(
                "outside",
                "Outside",
                "3",
                bounds(point(0.9, 0.9), size(0.2, 0.2)),
            ),
        ]);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].id.as_ref(), "a");
    }

    #[test]
    fn keyboard_traversal_stops_at_both_ends() {
        let marks = vec![
            PlotMark::new("a", "A", "1", bounds(point(0.0, 0.0), size(0.2, 0.2))),
            PlotMark::new("b", "B", "2", bounds(point(0.4, 0.4), size(0.2, 0.2))),
        ];
        assert_eq!(
            step_mark(&marks, Some(&"a".into()), "left").as_deref(),
            Some("a")
        );
        assert_eq!(
            step_mark(&marks, Some(&"a".into()), "end").as_deref(),
            Some("b")
        );
        assert_eq!(
            step_mark(&marks, Some(&"b".into()), "right").as_deref(),
            Some("b")
        );
    }

    #[test]
    fn candlesticks_refuse_impossible_ohlc_ordering() {
        assert!(Candlestick::new("a", 0.5, 0.4, 0.8, 0.2, 0.7, "A", "40–70").is_bounded());
        assert!(!Candlestick::new("b", 0.5, 0.4, 0.6, 0.2, 0.7, "B", "bad").is_bounded());
    }

    #[test]
    fn sankey_links_must_name_retained_nodes() {
        let data = SankeyData::new(
            [SankeyNode::new(
                "source",
                "Source",
                "10",
                bounds(point(0.0, 0.2), size(0.1, 0.3)),
            )],
            [SankeyLink::new(
                "missing",
                "source",
                "target",
                "Missing target",
                "10",
                point(0.1, 0.35),
                point(0.9, 0.35),
                0.2,
            )],
        );
        assert!(valid_sankey(&data).links.is_empty());
    }
}
