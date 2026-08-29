//! Cartesian readings over host-owned series.
//!
//! A chart does not invent a domain, a tick, a locale, an aggregation, or a
//! colour. The host supplies normalized coordinates and exact visible text.
//! This module owns the reusable presentation work downstream applications
//! should not have to redraw: axes, legends, keyed data motion, area fills,
//! crosshair interaction, and truthful loading and refresh states.
//!
//! Geometry follows caller-owned business ids. A point that moves keeps its
//! [`Transition`] while a point that enters or leaves runs a [`Presence`]
//! lifecycle. Semantic values always come from the newest caller input; only
//! pixels interpolate. Reduced motion settles both systems immediately.
//!
//! [`Transition`]: crate::motion::Transition
//! [`Presence`]: crate::motion::Presence

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, CursorStyle, Hsla, InteractiveElement, IntoElement, ParentElement,
    PathBuilder, Pixels, Point, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, canvas, div, linear_color_stop, linear_gradient_stops, point, prelude::FluentBuilder,
    px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, TypeScale};

use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::loading::PulseLoader;
use crate::display::sparkline::SparklinePoint;
use crate::display::status::StatusDot;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{FocusRing, Ident, StyledExt};
use crate::layout::measure;
use crate::motion::{self, MotionPolicy, MotionRole, Presence, Stagger, Transition, keyed};
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

/// One host-owned point in a chart series.
///
/// `id` is business identity and remains stable when the point moves or its
/// text changes. `label` and `value` are exact host-formatted strings: the
/// crosshair swaps them atomically and never interpolates between strings.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartPoint {
    pub id: SharedString,
    pub position: SparklinePoint,
    pub label: SharedString,
    pub value: SharedString,
    pub weight: Option<f32>,
}

impl ChartPoint {
    pub fn new(
        id: impl Into<SharedString>,
        x: f32,
        y: f32,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            position: SparklinePoint::new(x, y),
            label: label.into(),
            value: value.into(),
            weight: None,
        }
    }

    /// Normalized bubble radius for a scatter reading. The host already
    /// decided the mapping; this is only how large the mark is drawn.
    pub fn weight(mut self, weight: f32) -> Self {
        self.weight = Some(weight.clamp(0.0, 1.0));
        self
    }

    pub fn is_bounded(&self) -> bool {
        self.position.is_bounded()
    }
}

/// One named series. Identity is the caller's, never the draw order.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartSeries {
    pub id: SharedString,
    pub label: SharedString,
    pub points: Vec<ChartPoint>,
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

    pub fn points(mut self, points: impl IntoIterator<Item = ChartPoint>) -> Self {
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
    /// A refresh failed, so the last verified series remain visible with the
    /// host's exact explanation instead of being replaced by an empty chart.
    Stale {
        series: Vec<ChartSeries>,
        reason: SharedString,
    },
}

impl ChartState {
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

    fn visible_series(&self) -> Option<(&[ChartSeries], Option<&SharedString>)> {
        match self {
            Self::Ready(series) => Some((series, None)),
            Self::Stale { series, reason } => Some((series, Some(reason))),
            _ => None,
        }
    }
}

impl HasPhase for ChartState {
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

/// The business identity of the point under the crosshair.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChartSelection {
    pub series_id: SharedString,
    pub point_id: SharedString,
}

impl ChartSelection {
    pub fn new(series_id: impl Into<SharedString>, point_id: impl Into<SharedString>) -> Self {
        Self {
            series_id: series_id.into(),
            point_id: point_id.into(),
        }
    }
}

type CurrentHandler = Rc<dyn Fn(ChartSelection, &mut Window, &mut App)>;

/// A bar chart over one host-owned series of categorized values.
#[derive(Debug, IntoElement)]
pub struct BarChart {
    ident: Ident,
    label: SharedString,
    axes: ChartAxes,
    state: ChartState,
    slots: Slots,
}

impl BarChart {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>, state: ChartState) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            axes: ChartAxes::default(),
            state,
            slots: Slots::default(),
        }
    }

    pub fn axes(mut self, axes: ChartAxes) -> Self {
        self.axes = axes;
        self
    }
}

impl Slotted for BarChart {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for BarChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, spec): (AnyElement, NodeSpec) = match self.state.visible_series() {
            Some((series, stale)) => {
                let active = active_points(series, &theme)
                    .into_iter()
                    .filter(|point| point.series_order == 0)
                    .collect::<Vec<_>>();
                let count = active.len();
                (
                    ready_bars(
                        &self.ident,
                        &self.label,
                        &self.axes,
                        active,
                        stale.cloned(),
                        &theme,
                        window,
                        cx,
                    ),
                    chart_spec(&self.ident, &self.label, &self.state, count, stale),
                )
            }
            None => line_like_state(
                &self.ident,
                &self.label,
                &self.state,
                &self.slots,
                window,
                cx,
            ),
        };
        div()
            .w_full()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .child(body)
            .semantic_in(cx, spec)
    }
}

fn chart_spec(
    ident: &Ident,
    label: &SharedString,
    state: &ChartState,
    count: usize,
    stale: Option<&SharedString>,
) -> NodeSpec {
    let mut spec = NodeSpec::new(ident.semantic_id(), Role::Group)
        .text(label.clone())
        .value(state.name())
        .range(0.0, count as f32, count as f32);
    if let Some(reason) = stale {
        spec = spec.description(reason.clone());
    }
    spec
}

fn line_like_state(
    ident: &Ident,
    label: &SharedString,
    state: &ChartState,
    slots: &Slots,
    window: &mut Window,
    cx: &mut App,
) -> (AnyElement, NodeSpec) {
    let theme = cx.theme().clone();
    match state {
        ChartState::Loading => (
            slots.or_else(slot::LOADING, window, cx, |_, cx| {
                non_ready_body(
                    label,
                    PulseLoader::new(ident.child("loading"))
                        .label(cx.strings().text(StringKey::Loading)),
                    &theme,
                )
            }),
            NodeSpec::new(ident.semantic_id(), Role::Status)
                .text(label.clone())
                .busy(true)
                .value(state.name()),
        ),
        ChartState::Empty => (
            slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                non_ready_body(
                    label,
                    EmptyState::new(
                        ident.child("empty"),
                        cx.strings().text(StringKey::ChartEmpty),
                    )
                    .kind(EmptyKind::Empty),
                    &theme,
                )
            }),
            NodeSpec::new(ident.semantic_id(), Role::Status)
                .text(label.clone())
                .value(state.name()),
        ),
        ChartState::Unavailable(reason) => (
            slots.or_else(slot::EMPTY, window, cx, |_, _| {
                non_ready_body(
                    label,
                    EmptyState::new(ident.child("unavailable"), reason.clone())
                        .kind(EmptyKind::Unavailable),
                    &theme,
                )
            }),
            NodeSpec::new(ident.semantic_id(), Role::Status)
                .text(label.clone())
                .description(reason.clone())
                .value(state.name()),
        ),
        ChartState::Error(reason) => (
            slots.or_else(slot::FAILED, window, cx, |_, _| {
                non_ready_body(
                    label,
                    EmptyState::new(ident.child("error"), reason.clone()).kind(EmptyKind::Failed),
                    &theme,
                )
            }),
            NodeSpec::new(ident.semantic_id(), Role::Status)
                .text(label.clone())
                .description(reason.clone())
                .invalid(true)
                .value(state.name()),
        ),
        ChartState::Ready(_) | ChartState::Stale { .. } => {
            unreachable!("visible series are drawn by the caller")
        }
    }
}

fn non_ready_body(
    label: &SharedString,
    body: impl IntoElement,
    theme: &gpui_kit_theme::Theme,
) -> AnyElement {
    div()
        .column()
        .w_full()
        .gap_token(theme, Space::Xs)
        .child(chart_heading(label, None, theme))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w_full()
                .h(px(PLOT_HEIGHT))
                .overflow_hidden()
                .radius(theme, Radius::Small)
                .surface(theme, Surface::Canvas)
                .child(body),
        )
        .into_any_element()
}

/// A line chart over one or more host-owned series.
#[derive(IntoElement)]
pub struct LineChart {
    ident: Ident,
    label: SharedString,
    axes: ChartAxes,
    state: ChartState,
    area: bool,
    smooth: bool,
    crosshair: bool,
    current: Option<ChartSelection>,
    on_current: Option<CurrentHandler>,
    slots: Slots,
}

impl LineChart {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>, state: ChartState) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            axes: ChartAxes::default(),
            state,
            area: false,
            smooth: false,
            crosshair: false,
            current: None,
            on_current: None,
            slots: Slots::default(),
        }
    }

    pub fn axes(mut self, axes: ChartAxes) -> Self {
        self.axes = axes;
        self
    }

    /// Fills every series down to the normalized baseline with a three-stop
    /// gradient derived from that series' caller-owned colour.
    pub fn area(mut self) -> Self {
        self.area = true;
        self
    }

    /// Interpolates each series with a Catmull-Rom spline instead of
    /// polyline segments.
    pub fn smooth(mut self) -> Self {
        self.smooth = true;
        self
    }

    /// Enables pointer and keyboard crosshair navigation.
    pub fn crosshair(mut self) -> Self {
        self.crosshair = true;
        self
    }

    /// Seeds or controls the current crosshair by business identity.
    pub fn current(
        mut self,
        series_id: impl Into<SharedString>,
        point_id: impl Into<SharedString>,
    ) -> Self {
        self.current = Some(ChartSelection::new(series_id, point_id));
        self.crosshair = true;
        self
    }

    /// Reports the nearest point immediately when pointer or keyboard input
    /// changes the crosshair. The component keeps only this transient visual
    /// current; the host decides whether the reported point changes anything.
    pub fn on_current(
        mut self,
        handler: impl Fn(ChartSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_current = Some(Rc::new(handler));
        self.crosshair = true;
        self
    }
}

impl Slotted for LineChart {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for LineChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, spec): (AnyElement, NodeSpec) = match self.state.visible_series() {
            Some((series, stale)) => {
                let active = active_points(series, &theme);
                let count = active.len();
                (
                    ready_chart(
                        &self.ident,
                        &self.label,
                        &self.axes,
                        series,
                        active,
                        stale.cloned(),
                        self.area,
                        self.smooth,
                        false,
                        self.crosshair,
                        self.current,
                        self.on_current,
                        &theme,
                        window,
                        cx,
                    ),
                    chart_spec(&self.ident, &self.label, &self.state, count, stale),
                )
            }
            None => line_like_state(
                &self.ident,
                &self.label,
                &self.state,
                &self.slots,
                window,
                cx,
            ),
        };

        div()
            .w_full()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .child(body)
            .semantic_in(cx, spec)
    }
}

/// A filled area chart over one or more host-owned series.
///
/// Geometry is the same unit interval [`LineChart`] uses. The fill and the
/// optional Catmull-Rom stroke are presentation; they invent no domain.
#[derive(IntoElement)]
pub struct AreaChart {
    ident: Ident,
    label: SharedString,
    axes: ChartAxes,
    state: ChartState,
    smooth: bool,
    crosshair: bool,
    current: Option<ChartSelection>,
    on_current: Option<CurrentHandler>,
    slots: Slots,
}

impl AreaChart {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>, state: ChartState) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            axes: ChartAxes::default(),
            state,
            smooth: true,
            crosshair: false,
            current: None,
            on_current: None,
            slots: Slots::default(),
        }
    }

    pub fn axes(mut self, axes: ChartAxes) -> Self {
        self.axes = axes;
        self
    }

    pub fn polyline(mut self) -> Self {
        self.smooth = false;
        self
    }

    pub fn crosshair(mut self) -> Self {
        self.crosshair = true;
        self
    }

    pub fn current(
        mut self,
        series_id: impl Into<SharedString>,
        point_id: impl Into<SharedString>,
    ) -> Self {
        self.current = Some(ChartSelection::new(series_id, point_id));
        self.crosshair = true;
        self
    }

    pub fn on_current(
        mut self,
        handler: impl Fn(ChartSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_current = Some(Rc::new(handler));
        self.crosshair = true;
        self
    }
}

impl Slotted for AreaChart {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for AreaChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, spec): (AnyElement, NodeSpec) = match self.state.visible_series() {
            Some((series, stale)) => {
                let active = active_points(series, &theme);
                let count = active.len();
                (
                    ready_chart(
                        &self.ident,
                        &self.label,
                        &self.axes,
                        series,
                        active,
                        stale.cloned(),
                        true,
                        self.smooth,
                        false,
                        self.crosshair,
                        self.current,
                        self.on_current,
                        &theme,
                        window,
                        cx,
                    ),
                    chart_spec(&self.ident, &self.label, &self.state, count, stale),
                )
            }
            None => line_like_state(
                &self.ident,
                &self.label,
                &self.state,
                &self.slots,
                window,
                cx,
            ),
        };
        div()
            .w_full()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .child(body)
            .semantic_in(cx, spec)
    }
}

/// Scatter or bubble marks over one or more host-owned series.
///
/// Each point's optional [`ChartPoint::weight`] is a normalized radius. The
/// host already mapped the quantity; this only sizes the mark.
#[derive(IntoElement)]
pub struct ScatterChart {
    ident: Ident,
    label: SharedString,
    axes: ChartAxes,
    state: ChartState,
    crosshair: bool,
    current: Option<ChartSelection>,
    on_current: Option<CurrentHandler>,
    slots: Slots,
}

impl ScatterChart {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>, state: ChartState) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            axes: ChartAxes::default(),
            state,
            crosshair: false,
            current: None,
            on_current: None,
            slots: Slots::default(),
        }
    }

    pub fn axes(mut self, axes: ChartAxes) -> Self {
        self.axes = axes;
        self
    }

    pub fn crosshair(mut self) -> Self {
        self.crosshair = true;
        self
    }

    pub fn current(
        mut self,
        series_id: impl Into<SharedString>,
        point_id: impl Into<SharedString>,
    ) -> Self {
        self.current = Some(ChartSelection::new(series_id, point_id));
        self.crosshair = true;
        self
    }

    pub fn on_current(
        mut self,
        handler: impl Fn(ChartSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_current = Some(Rc::new(handler));
        self.crosshair = true;
        self
    }
}

impl Slotted for ScatterChart {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for ScatterChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, spec): (AnyElement, NodeSpec) = match self.state.visible_series() {
            Some((series, stale)) => {
                let active = active_points(series, &theme);
                let count = active.len();
                (
                    ready_chart(
                        &self.ident,
                        &self.label,
                        &self.axes,
                        series,
                        active,
                        stale.cloned(),
                        false,
                        false,
                        true,
                        self.crosshair,
                        self.current,
                        self.on_current,
                        &theme,
                        window,
                        cx,
                    ),
                    chart_spec(&self.ident, &self.label, &self.state, count, stale),
                )
            }
            None => line_like_state(
                &self.ident,
                &self.label,
                &self.state,
                &self.slots,
                window,
                cx,
            ),
        };
        div()
            .w_full()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .child(body)
            .semantic_in(cx, spec)
    }
}

/// How tall every plot region in this module is.
const PLOT_HEIGHT: f32 = 160.0;

/// How far the current-point readout stands off the corner of the plot.
const TOOLTIP_MARGIN: f32 = 8.0;
const RADAR_LABEL_WIDTH: f32 = 72.0;
const RADAR_LABEL_Y_OFFSET: f32 = -8.0;

#[derive(Debug, Clone)]
struct ActivePoint {
    selection: ChartSelection,
    series_label: SharedString,
    point: ChartPoint,
    color: Hsla,
    /// Whether `color` is the caller's own choice. A chart that divides one
    /// series into categories reaches for the categorical scale instead, and
    /// may only do so when nobody has already spent a colour here.
    tinted: bool,
    series_order: usize,
    point_order: usize,
}

fn active_points(series: &[ChartSeries], theme: &gpui_kit_theme::Theme) -> Vec<ActivePoint> {
    let mut series_ids = HashSet::new();
    let mut active = Vec::new();
    for (series_order, series) in series.iter().enumerate() {
        if !series_ids.insert(series.id.clone()) {
            continue;
        }
        let mut point_ids = HashSet::new();
        for (point_order, sample) in series.points.iter().enumerate() {
            if !sample.is_bounded() || !point_ids.insert(sample.id.clone()) {
                continue;
            }
            active.push(ActivePoint {
                selection: ChartSelection::new(series.id.clone(), sample.id.clone()),
                series_label: series.label.clone(),
                point: sample.clone(),
                color: series.color.unwrap_or(theme.colors.accent),
                tinted: series.color.is_some(),
                series_order,
                point_order,
            });
        }
    }
    active
}

fn point_key(selection: &ChartSelection) -> SharedString {
    Ident::new("series")
        .child(selection.series_id.as_ref())
        .child("point")
        .child(selection.point_id.as_ref())
        .semantic_id()
}

struct AnimatedPoint {
    selection: ChartSelection,
    color: Hsla,
    series_order: usize,
    point_order: usize,
    position: Transition<Point<f32>>,
    presence: Presence,
}

#[derive(Default)]
struct ChartMotion {
    points: HashMap<SharedString, AnimatedPoint>,
}

#[derive(Debug, Clone)]
struct PaintPoint {
    selection: ChartSelection,
    color: Hsla,
    series_order: usize,
    point_order: usize,
    position: Point<f32>,
    opacity: f32,
}

/// Theme-owned paint shared by every Cartesian chart canvas. Geometry and
/// data interpolation remain local; opacity and line weight do not.
#[derive(Debug, Clone, Copy)]
struct CartesianPaint {
    crosshair_color: Hsla,
    crosshair_width: f32,
    crosshair_primary_alpha: f32,
    crosshair_secondary_alpha: f32,
    area_start_alpha: f32,
    area_middle_alpha: f32,
}

impl CartesianPaint {
    fn from_theme(theme: &gpui_kit_theme::Theme) -> Self {
        Self {
            crosshair_color: theme.colors.hairline_strong,
            crosshair_width: theme.borders.hairline,
            crosshair_primary_alpha: theme.effects.accent_border_strong_alpha,
            crosshair_secondary_alpha: theme.effects.semantic_border_alpha,
            area_start_alpha: theme.effects.area_wash_alpha,
            area_middle_alpha: theme.effects.semantic_wash_alpha,
        }
    }
}

fn sync_motion(
    ident: &Ident,
    active: &[ActivePoint],
    theme: &gpui_kit_theme::Theme,
    window: &mut Window,
    cx: &mut App,
) -> Vec<PaintPoint> {
    let cell = keyed::slot::<ChartMotion>(
        &ident.child("motion").semantic_id(),
        window.window_handle().window_id(),
        cx,
    );
    let mut motion_state = cell.borrow_mut();
    let active_keys = active
        .iter()
        .map(|point| point_key(&point.selection))
        .collect::<HashSet<_>>();

    for (key, point) in &mut motion_state.points {
        if !active_keys.contains(key) {
            point.presence.hide();
        }
    }

    let mut new_keys = active
        .iter()
        .map(|point| point_key(&point.selection))
        .filter(|key| !motion_state.points.contains_key(key))
        .collect::<Vec<_>>();
    new_keys.sort();
    let new_ranks = new_keys
        .iter()
        .enumerate()
        .map(|(rank, key)| (key.clone(), rank))
        .collect::<HashMap<_, _>>();
    let stagger = Stagger::rows(theme);

    for target in active {
        let key = point_key(&target.selection);
        let target_position = point(target.point.position.x, target.point.position.y);
        if let Some(animated) = motion_state.points.get_mut(&key) {
            animated.color = target.color;
            animated.series_order = target.series_order;
            animated.point_order = target.point_order;
            animated.position = animated
                .position
                .spec(MotionPolicy::spec(MotionRole::Resize, theme));
            animated.position.set(target_position);
            animated.presence.show();
        } else {
            let rank = new_ranks.get(&key).copied().unwrap_or_default();
            let enter = stagger.spec(
                rank,
                new_keys.len(),
                MotionPolicy::spec(MotionRole::Entrance, theme),
            );
            let mut presence = Presence::hidden(enter, MotionPolicy::spec(MotionRole::Exit, theme));
            presence.show();
            motion_state.points.insert(
                key,
                AnimatedPoint {
                    selection: target.selection.clone(),
                    color: target.color,
                    series_order: target.series_order,
                    point_order: target.point_order,
                    position: Transition::new(
                        target_position,
                        MotionPolicy::spec(MotionRole::Resize, theme),
                    ),
                    presence,
                },
            );
        }
    }

    let mut painted = Vec::new();
    for animated in motion_state.points.values_mut() {
        let progress = animated.presence.animate(window, cx).clamp(0.0, 1.0);
        if !animated.presence.is_rendered() {
            continue;
        }
        let position = animated.position.animate(window, cx);
        painted.push(PaintPoint {
            selection: animated.selection.clone(),
            color: animated.color,
            series_order: animated.series_order,
            point_order: animated.point_order,
            position: point(position.x, position.y * progress),
            opacity: progress,
        });
    }
    motion_state
        .points
        .retain(|_, point| point.presence.is_rendered());
    painted.sort_by_key(|point| (point.series_order, point.point_order));
    painted
}

#[derive(Default)]
struct ChartInteraction {
    initialized: bool,
    declared: Option<ChartSelection>,
    current: Option<ChartSelection>,
}

fn current_cell(
    ident: &Ident,
    declared: Option<ChartSelection>,
    active: &[ActivePoint],
    window: &Window,
    cx: &mut App,
) -> Rc<std::cell::RefCell<ChartInteraction>> {
    let cell = keyed::slot::<ChartInteraction>(
        &ident.child("interaction").semantic_id(),
        window.window_handle().window_id(),
        cx,
    );
    {
        let mut state = cell.borrow_mut();
        if !state.initialized || state.declared != declared {
            state.current = declared.clone();
            state.declared = declared;
            state.initialized = true;
        }
        if state
            .current
            .as_ref()
            .is_none_or(|selection| !active.iter().any(|point| &point.selection == selection))
        {
            state.current = active.first().map(|point| point.selection.clone());
        }
    }
    cell
}

fn update_current(
    cell: &Rc<std::cell::RefCell<ChartInteraction>>,
    next: ChartSelection,
    report: Option<&CurrentHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let changed = cell.borrow().current.as_ref() != Some(&next);
    if !changed {
        return;
    }
    cell.borrow_mut().current = Some(next.clone());
    if let Some(report) = report {
        report(next, window, cx);
    }
    window.refresh();
}

fn nearest_point(
    active: &[ActivePoint],
    bounds: Bounds<Pixels>,
    pointer: Point<Pixels>,
) -> Option<ChartSelection> {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    active
        .iter()
        .min_by(|left, right| {
            let distance = |point: &ActivePoint| {
                let x = f32::from(pointer.x - bounds.origin.x) - point.point.position.x * width;
                let y = f32::from(pointer.y - bounds.origin.y)
                    - (1.0 - point.point.position.y) * height;
                x * x + y * y
            };
            distance(left)
                .partial_cmp(&distance(right))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|point| point.selection.clone())
}

fn step_point(
    active: &[ActivePoint],
    current: Option<&ChartSelection>,
    key: &str,
) -> Option<ChartSelection> {
    let current = current
        .and_then(|selection| active.iter().find(|point| &point.selection == selection))
        .or_else(|| active.first())?;
    let mut same_series = active
        .iter()
        .filter(|point| point.selection.series_id == current.selection.series_id)
        .collect::<Vec<_>>();
    same_series.sort_by(|left, right| {
        left.point
            .position
            .x
            .partial_cmp(&right.point.position.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.point_order.cmp(&right.point_order))
    });
    let place = same_series
        .iter()
        .position(|point| point.selection == current.selection)
        .unwrap_or_default();
    match key {
        "left" => same_series.get(place.saturating_sub(1)).copied(),
        "right" => same_series
            .get((place + 1).min(same_series.len() - 1))
            .copied(),
        "home" => same_series.first().copied(),
        "end" => same_series.last().copied(),
        "up" | "down" => {
            let offset = if key == "up" { -1 } else { 1 };
            let next_series = (current.series_order as isize + offset)
                .clamp(0, active.last()?.series_order as isize)
                as usize;
            active
                .iter()
                .filter(|point| point.series_order == next_series)
                .min_by(|left, right| {
                    (left.point.position.x - current.point.position.x)
                        .abs()
                        .partial_cmp(&(right.point.position.x - current.point.position.x).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        }
        _ => return None,
    }
    .map(|point| point.selection.clone())
}

#[allow(clippy::too_many_arguments)]
fn ready_chart(
    ident: &Ident,
    label: &SharedString,
    axes: &ChartAxes,
    series: &[ChartSeries],
    active: Vec<ActivePoint>,
    stale: Option<SharedString>,
    area: bool,
    smooth: bool,
    scatter: bool,
    crosshair: bool,
    declared: Option<ChartSelection>,
    report: Option<CurrentHandler>,
    theme: &gpui_kit_theme::Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let stroke = theme.borders.thick;
    let painted = sync_motion(ident, &active, theme, window, cx);
    let measured = measure::cell(&ident.child("plot-bounds").semantic_id(), window, cx);
    let interaction = crosshair.then(|| current_cell(ident, declared, &active, window, cx));
    let current = interaction.as_ref().and_then(|state| {
        let selection = state.borrow().current.clone()?;
        active
            .iter()
            .find(|point| point.selection == selection)
            .cloned()
    });
    let crosshair_position = current.as_ref().map(|current| {
        motion::tracked(
            &ident.child("crosshair").semantic_id(),
            point(current.point.position.x, current.point.position.y),
            MotionPolicy::spec(MotionRole::Tracking, theme),
            window,
            cx,
        )
    });

    let plot_spec = if crosshair {
        let mut spec = NodeSpec::new(ident.child("plot").semantic_id(), Role::Slider)
            .parent(ident.semantic_id())
            .text(label.clone())
            .range(
                0.0,
                active.len().saturating_sub(1) as f32,
                current
                    .as_ref()
                    .and_then(|current| {
                        active
                            .iter()
                            .position(|point| point.selection == current.selection)
                    })
                    .unwrap_or_default() as f32,
            );
        if let Some(current) = &current {
            spec = spec.value(current.point.value.clone());
        }
        spec
    } else {
        NodeSpec::new(ident.child("plot").semantic_id(), Role::Image)
            .parent(ident.semantic_id())
            .text(label.clone())
            .read_only(true)
    };

    let weights: HashMap<ChartSelection, f32> = active
        .iter()
        .filter_map(|point| {
            point
                .point
                .weight
                .map(|weight| (point.selection.clone(), weight))
        })
        .collect();
    let paint = CartesianPaint::from_theme(theme);
    let chart_canvas = if scatter {
        scatter_canvas(painted, weights, stroke, crosshair_position, paint).into_any_element()
    } else {
        line_canvas(painted, stroke, area, smooth, crosshair_position, paint).into_any_element()
    };
    let current_tip = current.as_ref().map(|current| {
        current_tooltip(ident, current, tooltip_anchor(&active, current), theme, cx)
    });
    let mut plot = div()
        .on_children_prepainted({
            let measured = Rc::clone(&measured);
            move |bounds, window, _| {
                if let Some(first) = bounds.first() {
                    measure::record(&measured, *first, window);
                }
            }
        })
        .id(ident.child("plot").element_id())
        .relative()
        .flex_1()
        .min_w_0()
        .h(px(PLOT_HEIGHT))
        .overflow_hidden()
        .radius(theme, Radius::Small)
        .surface(theme, Surface::Canvas)
        // Behind the data: a rule is the reading the axis already names, put
        // where the label points, not a mark competing with the series.
        .children(
            value_rules(axes)
                .into_iter()
                .map(|fraction| value_rule(fraction, PLOT_HEIGHT, theme)),
        )
        .child(chart_canvas)
        .children(current_tip);
    if let Some(interaction) = interaction {
        plot = plot
            .tab_index(0)
            .focus_ring(theme)
            .cursor(CursorStyle::Crosshair);
        let pointer_points = Rc::new(active.clone());
        let pointer_bounds = Rc::clone(&measured);
        let pointer_state = Rc::clone(&interaction);
        let pointer_report = report.clone();
        plot = plot.on_mouse_move(move |event, window, cx| {
            if let Some(next) = nearest_point(&pointer_points, pointer_bounds.get(), event.position)
            {
                update_current(&pointer_state, next, pointer_report.as_ref(), window, cx);
            }
        });
        let key_points = Rc::new(active.clone());
        let key_state = Rc::clone(&interaction);
        let key_report = report;
        plot = plot.on_key_down(move |event, window, cx| {
            let current = key_state.borrow().current.clone();
            if let Some(next) = step_point(&key_points, current.as_ref(), &event.keystroke.key) {
                update_current(&key_state, next, key_report.as_ref(), window, cx);
                cx.stop_propagation();
            }
        });
    }
    let plot = plot.semantic_in(cx, plot_spec);

    let has_y_axis = axes.y_start.is_some() || axes.y_end.is_some();

    div()
        .column()
        .w_full()
        .gap_token(theme, Space::Xs)
        .child(chart_heading(label, axes.y_label.clone(), theme))
        .children(stale.map(|reason| stale_warning(ident, reason, theme, cx)))
        .child(
            div()
                .row()
                .items_start()
                .gap_token(theme, Space::Xs)
                .children(y_axis(axes, theme))
                .child(plot),
        )
        .child(x_axis(axes, has_y_axis, theme))
        .child(series_legend(ident, series, theme, cx))
        .into_any_element()
}

fn chart_heading(
    label: &SharedString,
    y_label: Option<SharedString>,
    theme: &gpui_kit_theme::Theme,
) -> AnyElement {
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
        .children(y_label.map(|label| {
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_muted)
                .child(label)
        }))
        .into_any_element()
}

fn stale_warning(
    ident: &Ident,
    reason: SharedString,
    theme: &gpui_kit_theme::Theme,
    cx: &App,
) -> AnyElement {
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
        .into_any_element()
}

fn y_axis(axes: &ChartAxes, theme: &gpui_kit_theme::Theme) -> Option<AnyElement> {
    (axes.y_start.is_some() || axes.y_end.is_some()).then(|| {
        div()
            .column()
            .flex_none()
            .w(px(44.0))
            .h(px(PLOT_HEIGHT))
            .justify_between()
            .type_scale(theme, TypeScale::Caption)
            .text_color(theme.colors.text_faint)
            .children(axes.y_end.clone())
            .children(axes.y_start.clone())
            .into_any_element()
    })
}

/// The value-axis readings this chart already puts a label against, as
/// normalized heights.
///
/// A chart here owns no tick model — it invents no scale, so it computes no
/// intermediate reading to hang a rule on. The two ends are the readings the
/// caller named, so those are the two a rule may be drawn at, and a caller
/// that named neither gets none.
fn value_rules(axes: &ChartAxes) -> Vec<f32> {
    let mut rules = Vec::new();
    if axes.y_start.is_some() {
        rules.push(0.0);
    }
    if axes.y_end.is_some() {
        rules.push(1.0);
    }
    rules
}

/// One rule across a plot region of `height`, at a normalized reading.
fn value_rule(fraction: f32, height: f32, theme: &gpui_kit_theme::Theme) -> gpui::Div {
    let weight = theme.borders.hairline;
    let top =
        ((1.0 - fraction.clamp(0.0, 1.0)) * height - weight / 2.0).clamp(0.0, height - weight);
    div()
        .absolute()
        .left_0()
        .right_0()
        .top(px(top))
        .h(px(weight))
        .bg(theme.colors.hairline)
}

fn axis_offset(axes: &ChartAxes, theme: &gpui_kit_theme::Theme) -> f32 {
    if axes.y_start.is_some() || axes.y_end.is_some() {
        44.0 + theme.space(Space::Xs)
    } else {
        0.0
    }
}

fn x_axis(axes: &ChartAxes, has_y_axis: bool, theme: &gpui_kit_theme::Theme) -> AnyElement {
    div()
        .column()
        .ml(px(if has_y_axis {
            axis_offset(axes, theme)
        } else {
            0.0
        }))
        .gap_token(theme, Space::Xs)
        .child(
            div()
                // Cartesian endpoints are physical data coordinates, not a
                // reading-order row: x=0 remains left and x=1 remains right.
                .row()
                .justify_between()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_faint)
                .children(axes.x_start.clone())
                .children(axes.x_end.clone()),
        )
        .children(axes.x_label.clone().map(|label| {
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_align(gpui::TextAlign::Center)
                .text_color(theme.colors.text_muted)
                .child(label)
        }))
        .into_any_element()
}

fn series_legend(
    ident: &Ident,
    series: &[ChartSeries],
    theme: &gpui_kit_theme::Theme,
    cx: &App,
) -> AnyElement {
    let mut ids = HashSet::new();
    div()
        .row()
        .flex_wrap()
        .gap_token(theme, Space::Sm)
        .children(series.iter().filter_map(|series| {
            if !ids.insert(series.id.clone()) {
                return None;
            }
            let color = series.color.unwrap_or(theme.colors.accent);
            Some(
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
                            ident
                                .child("series")
                                .child(series.id.as_ref())
                                .semantic_id(),
                            Role::Status,
                        )
                        .parent(ident.semantic_id())
                        .text(series.label.clone())
                        .value(series.id.clone()),
                    ),
            )
        }))
        .into_any_element()
}

/// Which corner of the plot the current-point readout stands in, as
/// `(against the leading edge, against the floor)`.
///
/// The half of the plot the sample is not in, and then the side of that half
/// the series is furthest from. A fixed corner covered whatever happened to
/// be drawn there — including the very point it was naming — and the sample's
/// own height says nothing about the height of the series at the other end of
/// the plot, so the data in the half the readout is going to is what decides.
fn tooltip_anchor(active: &[ActivePoint], current: &ActivePoint) -> (bool, bool) {
    let leading = current.point.position.x > 0.5;
    let (total, count) = active
        .iter()
        .filter(|point| (point.point.position.x <= 0.5) == leading)
        .fold((0.0f32, 0usize), |(total, count), point| {
            (total + point.point.position.y.clamp(0.0, 1.0), count + 1)
        });
    let height = if count == 0 {
        0.5
    } else {
        total / count as f32
    };
    (leading, height >= 0.5)
}

fn current_tooltip(
    ident: &Ident,
    current: &ActivePoint,
    (leading, floor): (bool, bool),
    theme: &gpui_kit_theme::Theme,
    cx: &App,
) -> AnyElement {
    let semantic_id = ident
        .child("series")
        .child(current.selection.series_id.as_ref())
        .child("point")
        .child(current.selection.point_id.as_ref())
        .semantic_id();
    div()
        .absolute()
        .map(|tip| {
            if leading {
                tip.left(px(TOOLTIP_MARGIN))
            } else {
                tip.right(px(TOOLTIP_MARGIN))
            }
        })
        .map(|tip| {
            if floor {
                tip.bottom(px(TOOLTIP_MARGIN))
            } else {
                tip.top(px(TOOLTIP_MARGIN))
            }
        })
        .column()
        .gap(px(theme.space(Space::Xxs) / 2.0))
        .px_token(theme, Space::Xs)
        .py(px(theme.space(Space::Xs)))
        .radius(theme, Radius::Small)
        .frame(theme, Surface::Overlay, Elevation::Overlay)
        .child(
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_faint)
                .child(current.series_label.clone()),
        )
        .child(
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_muted)
                .child(current.point.label.clone()),
        )
        .child(
            div()
                .type_scale(theme, TypeScale::Label)
                .text_color(theme.colors.text)
                .child(current.point.value.clone()),
        )
        .semantic_in(
            cx,
            NodeSpec::new(semantic_id, Role::Status)
                .parent(
                    ident
                        .child("series")
                        .child(current.selection.series_id.as_ref())
                        .semantic_id(),
                )
                .text(current.point.label.clone())
                .value(current.point.value.clone())
                .selected(true),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn ready_bars(
    ident: &Ident,
    label: &SharedString,
    axes: &ChartAxes,
    active: Vec<ActivePoint>,
    stale: Option<SharedString>,
    theme: &gpui_kit_theme::Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let painted = sync_motion(ident, &active, theme, window, cx);
    let active_by_key = active
        .iter()
        .map(|point| (point_key(&point.selection), point))
        .collect::<HashMap<_, _>>();
    let bars = painted
        .iter()
        .map(|point| {
            let key = point_key(&point.selection);
            // A bar chart divides one series into categories, so what a
            // reader has to tell apart is a category and not a series. Left
            // uncoloured they take the categorical scale in order; a caller
            // that already spent a colour on the series keeps it.
            let base = match active_by_key.get(&key) {
                Some(active) if active.tinted => point.color,
                _ => theme.colors.sequence.get(point.point_order),
            };
            let bar = div()
                .flex_1()
                .h(relative(point.position.y.clamp(0.0, 1.0)))
                .rounded_t(px(theme.radii.small))
                .bg(linear_gradient_stops(
                    180.0,
                    [
                        linear_color_stop(base.opacity(point.opacity), 0.0),
                        linear_color_stop(base.opacity(point.opacity * theme.opacity.muted), 1.0),
                    ],
                ));
            match active_by_key.get(&key) {
                Some(active) => bar
                    .semantic_in(
                        cx,
                        NodeSpec::new(
                            ident
                                .child("series")
                                .child(active.selection.series_id.as_ref())
                                .child("point")
                                .child(active.selection.point_id.as_ref())
                                .semantic_id(),
                            Role::Status,
                        )
                        .parent(ident.semantic_id())
                        .text(active.point.label.clone())
                        .value(active.point.value.clone()),
                    )
                    .into_any_element(),
                None => bar.into_any_element(),
            }
        })
        .collect::<Vec<_>>();

    let offset = axis_offset(axes, theme);
    div()
        .column()
        .w_full()
        .gap_token(theme, Space::Xs)
        .child(chart_heading(label, axes.y_label.clone(), theme))
        .children(stale.map(|reason| stale_warning(ident, reason, theme, cx)))
        .child(
            div()
                .row()
                .items_end()
                .w_full()
                .gap_token(theme, Space::Xs)
                .children(y_axis(axes, theme))
                .child(
                    div()
                        .relative()
                        .row()
                        .items_end()
                        .flex_1()
                        .min_w_0()
                        .gap(px(theme.space(Space::Xs) + theme.space(Space::Xxs)))
                        .h(px(PLOT_HEIGHT))
                        .children(
                            value_rules(axes)
                                .into_iter()
                                .map(|fraction| value_rule(fraction, PLOT_HEIGHT, theme)),
                        )
                        .children(bars),
                ),
        )
        .child(
            div()
                .row()
                .ml(px(offset))
                .gap(px(theme.space(Space::Xs) + theme.space(Space::Xxs)))
                .children(active.iter().map(|point| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .type_scale(theme, TypeScale::Caption)
                        .text_align(gpui::TextAlign::Center)
                        .text_color(theme.colors.text_faint)
                        .child(point.point.label.clone())
                })),
        )
        .child(x_axis(axes, offset > 0.0, theme))
        .into_any_element()
}

fn trace_series(
    builder: &mut PathBuilder,
    samples: &[Point<f32>],
    smooth: bool,
    at: &impl Fn(Point<f32>) -> Point<Pixels>,
) {
    if samples.is_empty() {
        return;
    }
    builder.line_to(at(samples[0]));
    if !smooth || samples.len() < 3 {
        for sample in samples.iter().skip(1) {
            builder.line_to(at(*sample));
        }
        return;
    }
    for index in 0..samples.len() - 1 {
        let first = samples[index.saturating_sub(1)];
        let second = samples[index];
        let third = samples[index + 1];
        let fourth = samples[usize::min(index + 2, samples.len() - 1)];
        let control_a = point(
            second.x + (third.x - first.x) / 6.0,
            second.y + (third.y - first.y) / 6.0,
        );
        let control_b = point(
            third.x - (fourth.x - second.x) / 6.0,
            third.y - (fourth.y - second.y) / 6.0,
        );
        builder.cubic_bezier_to(at(third), at(control_a), at(control_b));
    }
}

fn scatter_canvas(
    points: Vec<PaintPoint>,
    weights: HashMap<ChartSelection, f32>,
    stroke: f32,
    crosshair: Option<Point<f32>>,
    paint: CartesianPaint,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let inset = stroke / 2.0;
            let width = (f32::from(bounds.size.width) - stroke).max(0.0);
            let height = (f32::from(bounds.size.height) - stroke).max(0.0);
            if width <= 0.0 || height <= 0.0 {
                return;
            }
            let at = |sample: Point<f32>| {
                point(
                    bounds.origin.x + px(inset + sample.x * width),
                    bounds.origin.y + px(inset + (1.0 - sample.y) * height),
                )
            };
            for sample in &points {
                let centre = at(sample.position);
                let radius = 3.0 + weights.get(&sample.selection).copied().unwrap_or(0.0) * 8.0;
                let mut builder = PathBuilder::fill();
                builder.move_to(point(centre.x + px(radius), centre.y));
                builder.arc_to(
                    point(px(radius), px(radius)),
                    px(0.0),
                    false,
                    true,
                    point(centre.x - px(radius), centre.y),
                );
                builder.arc_to(
                    point(px(radius), px(radius)),
                    px(0.0),
                    false,
                    true,
                    point(centre.x + px(radius), centre.y),
                );
                builder.close();
                if let Ok(path) = builder.build() {
                    window.paint_path(path, sample.color.opacity(sample.opacity));
                }
            }
            if let Some(crosshair) = crosshair {
                paint_crosshair(window, bounds, at(crosshair), paint);
            }
        },
    )
    .w_full()
    .h_full()
}

fn line_canvas(
    points: Vec<PaintPoint>,
    stroke: f32,
    area: bool,
    smooth: bool,
    crosshair: Option<Point<f32>>,
    paint: CartesianPaint,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let inset = stroke / 2.0;
            let width = (f32::from(bounds.size.width) - stroke).max(0.0);
            let height = (f32::from(bounds.size.height) - stroke).max(0.0);
            if width <= 0.0 || height <= 0.0 {
                return;
            }
            let at = |sample: Point<f32>| {
                point(
                    bounds.origin.x + px(inset + sample.x * width),
                    bounds.origin.y + px(inset + (1.0 - sample.y) * height),
                )
            };
            let mut grouped: HashMap<SharedString, Vec<&PaintPoint>> = HashMap::new();
            for sample in &points {
                grouped
                    .entry(sample.selection.series_id.clone())
                    .or_default()
                    .push(sample);
            }
            let mut grouped = grouped.into_values().collect::<Vec<_>>();
            grouped.sort_by_key(|series| series[0].series_order);

            if area {
                // Two fills at the weight one fill wants stack into a third
                // colour that belongs to neither series and matches no entry
                // in the key. The more series overlap, the less each fill is
                // allowed to say, and the line stays the thing that carries
                // the identity.
                let crowding = 1.0 / (grouped.len().max(1) as f32).sqrt();
                for series in &grouped {
                    if series.len() < 2 {
                        continue;
                    }
                    let opacity = crowding * series.iter().map(|point| point.opacity).sum::<f32>()
                        / series.len() as f32;
                    let color = series[0].color;
                    let samples: Vec<Point<f32>> =
                        series.iter().map(|sample| sample.position).collect();
                    let mut builder = PathBuilder::fill();
                    builder.move_to(at(point(samples[0].x, 0.0)));
                    trace_series(&mut builder, &samples, smooth, &at);
                    builder.line_to(at(point(
                        samples.last().expect("series is nonempty").x,
                        0.0,
                    )));
                    builder.close();
                    if let Ok(path) = builder.build() {
                        window.paint_path(
                            path,
                            linear_gradient_stops(
                                180.0,
                                [
                                    linear_color_stop(
                                        color.opacity(paint.area_start_alpha * opacity),
                                        0.0,
                                    ),
                                    linear_color_stop(
                                        color.opacity(paint.area_middle_alpha * opacity),
                                        0.58,
                                    ),
                                    linear_color_stop(color.opacity(0.0), 1.0),
                                ],
                            ),
                        );
                    }
                }
            }

            for series in grouped {
                if series.len() < 2 {
                    continue;
                }
                let opacity =
                    series.iter().map(|point| point.opacity).sum::<f32>() / series.len() as f32;
                let samples: Vec<Point<f32>> =
                    series.iter().map(|sample| sample.position).collect();
                let mut builder = PathBuilder::stroke(px(stroke));
                builder.move_to(at(samples[0]));
                trace_series(&mut builder, &samples, smooth, &at);
                if let Ok(path) = builder.build() {
                    window.paint_path(path, series[0].color.opacity(opacity));
                }
            }

            if let Some(crosshair) = crosshair {
                paint_crosshair(window, bounds, at(crosshair), paint);
            }
        },
    )
    .w_full()
    .h_full()
}

fn paint_crosshair(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    target: Point<Pixels>,
    paint: CartesianPaint,
) {
    let mut vertical = PathBuilder::stroke(px(paint.crosshair_width));
    vertical.move_to(point(target.x, bounds.top()));
    vertical.line_to(point(target.x, bounds.bottom()));
    if let Ok(path) = vertical.build() {
        window.paint_path(
            path,
            paint.crosshair_color.opacity(paint.crosshair_primary_alpha),
        );
    }

    let mut horizontal = PathBuilder::stroke(px(paint.crosshair_width));
    horizontal.move_to(point(bounds.left(), target.y));
    horizontal.line_to(point(bounds.right(), target.y));
    if let Ok(path) = horizontal.build() {
        window.paint_path(
            path,
            paint
                .crosshair_color
                .opacity(paint.crosshair_secondary_alpha),
        );
    }
}

/// A pie or donut over one host-owned series of shares.
///
/// Each point's `y` is the share of the circle the host already computed.
/// Shares are not renormalised here.
#[derive(Debug, IntoElement)]
pub struct PieChart {
    ident: Ident,
    label: SharedString,
    state: ChartState,
    donut: bool,
    slots: Slots,
}

impl PieChart {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>, state: ChartState) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            state,
            donut: false,
            slots: Slots::default(),
        }
    }

    pub fn donut(mut self) -> Self {
        self.donut = true;
        self
    }
}

impl Slotted for PieChart {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for PieChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, spec): (AnyElement, NodeSpec) = match self.state.visible_series() {
            Some((series, stale)) => {
                let active = active_points(series, &theme)
                    .into_iter()
                    .filter(|point| point.series_order == 0)
                    .collect::<Vec<_>>();
                let count = active.len();
                (
                    ready_pie(
                        &self.ident,
                        &self.label,
                        active,
                        stale.cloned(),
                        self.donut,
                        &theme,
                        window,
                        cx,
                    ),
                    chart_spec(&self.ident, &self.label, &self.state, count, stale),
                )
            }
            None => line_like_state(
                &self.ident,
                &self.label,
                &self.state,
                &self.slots,
                window,
                cx,
            ),
        };
        div()
            .w_full()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .child(body)
            .semantic_in(cx, spec)
    }
}

/// Bars stacked from the same category identity across series.
#[derive(Debug, IntoElement)]
pub struct StackedBarChart {
    ident: Ident,
    label: SharedString,
    axes: ChartAxes,
    state: ChartState,
    slots: Slots,
}

impl StackedBarChart {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>, state: ChartState) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            axes: ChartAxes::default(),
            state,
            slots: Slots::default(),
        }
    }

    pub fn axes(mut self, axes: ChartAxes) -> Self {
        self.axes = axes;
        self
    }
}

impl Slotted for StackedBarChart {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for StackedBarChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, spec): (AnyElement, NodeSpec) = match self.state.visible_series() {
            Some((series, stale)) => {
                let active = active_points(series, &theme);
                let count = active.len();
                (
                    ready_stacked(
                        &self.ident,
                        &self.label,
                        &self.axes,
                        series,
                        &active,
                        stale.cloned(),
                        &theme,
                        window,
                        cx,
                    ),
                    chart_spec(&self.ident, &self.label, &self.state, count, stale),
                )
            }
            None => line_like_state(
                &self.ident,
                &self.label,
                &self.state,
                &self.slots,
                window,
                cx,
            ),
        };
        div()
            .w_full()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .child(body)
            .semantic_in(cx, spec)
    }
}

type LegendToggle = Rc<dyn Fn(SharedString, bool, &mut Window, &mut App)>;

/// A standalone legend that reports series hide and show.
#[derive(IntoElement)]
pub struct ChartLegend {
    ident: Ident,
    series: Vec<ChartSeries>,
    hidden: Vec<SharedString>,
    on_toggle: Option<LegendToggle>,
}

impl std::fmt::Debug for ChartLegend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChartLegend")
            .field("ident", &self.ident)
            .field("series", &self.series.len())
            .field("hidden", &self.hidden)
            .finish()
    }
}

impl ChartLegend {
    pub fn new(ident: impl Into<Ident>, series: impl IntoIterator<Item = ChartSeries>) -> Self {
        Self {
            ident: ident.into(),
            series: series.into_iter().collect(),
            hidden: Vec::new(),
            on_toggle: None,
        }
    }

    pub fn hidden(mut self, ids: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.hidden = ids.into_iter().map(Into::into).collect();
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ChartLegend {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let label = cx.strings().text(StringKey::ChartLegend);
        let mut ids = HashSet::new();
        div()
            .id(self.ident.element_id())
            .row()
            .flex_wrap()
            .gap_token(&theme, Space::Sm)
            .children(self.series.iter().filter_map(|series| {
                if !ids.insert(series.id.clone()) {
                    return None;
                }
                let hidden = self.hidden.iter().any(|id| id == &series.id);
                let color = series.color.unwrap_or(theme.colors.accent);
                let ident = self.ident.child(series.id.as_ref());
                let mut row = div()
                    .id(ident.element_id())
                    .row()
                    .items_center()
                    .gap_token(&theme, Space::Xs)
                    .opacity(if hidden { theme.opacity.muted } else { 1.0 })
                    .child(div().size(px(8.0)).rounded_full().bg(color))
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Caption)
                            .text_color(theme.colors.text_muted)
                            .child(series.label.clone()),
                    );
                if let Some(handler) = self.on_toggle.clone() {
                    let id = series.id.clone();
                    let next = hidden;
                    row = row.cursor_pointer().on_click(move |_, window, cx| {
                        handler(id.clone(), next, window, cx);
                    });
                }
                Some(
                    row.semantic_in(
                        cx,
                        NodeSpec::new(ident.semantic_id(), Role::Button)
                            .parent(self.ident.semantic_id())
                            .text(series.label.clone())
                            .value(if hidden { "hidden" } else { "shown" })
                            .selected(!hidden),
                    ),
                )
            }))
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group).text(label),
            )
    }
}

#[allow(clippy::too_many_arguments)]
fn ready_pie(
    ident: &Ident,
    label: &SharedString,
    active: Vec<ActivePoint>,
    stale: Option<SharedString>,
    donut: bool,
    theme: &gpui_kit_theme::Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let painted = sync_motion(ident, &active, theme, window, cx);
    // A pie divides one series into its parts, so the thing a reader has to
    // tell apart is a slice and not a series. Painted in the series colour
    // they are one disc with hairline seams in it; uncoloured they take the
    // categorical scale, and a caller that coloured the series keeps that
    // colour and steps down it instead. Either way the key beside them names
    // the slices rather than the one series they all came from.
    let count = painted.len().max(1);
    let tinted = active.first().is_some_and(|point| point.tinted);
    let slice_color = |color: Hsla, index: usize| {
        if tinted {
            slice_step(color, index, count)
        } else {
            theme.colors.sequence.get(index)
        }
    };
    let slices = painted
        .iter()
        .enumerate()
        .map(|(index, point)| {
            (
                slice_color(point.color, index),
                point.position.y.clamp(0.0, 1.0),
                point.opacity,
            )
        })
        .collect::<Vec<_>>();
    let key: Vec<ChartSeries> = active
        .iter()
        .enumerate()
        .map(|(index, point)| ChartSeries {
            id: point.selection.point_id.clone(),
            label: point.point.label.clone(),
            points: Vec::new(),
            color: Some(slice_color(point.color, index)),
        })
        .collect();
    div()
        .column()
        .w_full()
        .gap_token(theme, Space::Xs)
        .child(chart_heading(label, None, theme))
        .children(stale.map(|reason| stale_warning(ident, reason, theme, cx)))
        .child(div().w_full().h(px(180.0)).child(pie_canvas(slices, donut)))
        .child(series_legend(ident, &key, theme, cx))
        .into_any_element()
}

/// One slice's step down the series colour.
///
/// The ramp runs from the full colour to a little under half of it, which is
/// the widest range that keeps the palest slice legible against the surface
/// behind it in both appearances.
fn slice_step(color: Hsla, index: usize, count: usize) -> Hsla {
    let span = 0.55;
    let position = if count <= 1 {
        0.0
    } else {
        index as f32 / (count - 1) as f32
    };
    color.opacity(1.0 - span * position)
}

fn pie_canvas(slices: Vec<(Hsla, f32, f32)>, donut: bool) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            let radius = width.min(height) / 2.0 - 2.0;
            if radius <= 0.0 {
                return;
            }
            let center = bounds.center();
            let inner = if donut { radius * 0.55 } else { 0.0 };
            let mut angle = -std::f32::consts::FRAC_PI_2;
            for (color, share, opacity) in &slices {
                let span = *share * std::f32::consts::TAU;
                let a0 = angle;
                let a1 = angle + span;
                let mut builder = PathBuilder::fill();
                let steps = 28;
                for step in 0..=steps {
                    let t = a0 + (a1 - a0) * (step as f32 / steps as f32);
                    let at = point(
                        center.x + px(radius * t.cos()),
                        center.y + px(radius * t.sin()),
                    );
                    if step == 0 {
                        if inner <= 0.0 {
                            builder.move_to(center);
                            builder.line_to(at);
                        } else {
                            builder.move_to(at);
                        }
                    } else {
                        builder.line_to(at);
                    }
                }
                if inner > 0.0 {
                    for step in (0..=steps).rev() {
                        let t = a0 + (a1 - a0) * (step as f32 / steps as f32);
                        builder.line_to(point(
                            center.x + px(inner * t.cos()),
                            center.y + px(inner * t.sin()),
                        ));
                    }
                }
                builder.close();
                if let Ok(path) = builder.build() {
                    window.paint_path(path, color.opacity(*opacity));
                }
                angle = a1;
            }
        },
    )
    .w_full()
    .h_full()
}

#[allow(clippy::too_many_arguments)]
fn ready_stacked(
    ident: &Ident,
    label: &SharedString,
    axes: &ChartAxes,
    series: &[ChartSeries],
    active: &[ActivePoint],
    stale: Option<SharedString>,
    theme: &gpui_kit_theme::Theme,
    _window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let mut categories: Vec<SharedString> = Vec::new();
    for point in series.iter().flat_map(|series| series.points.iter()) {
        if !categories.iter().any(|id| id == &point.id) {
            categories.push(point.id.clone());
        }
    }
    let offset = axis_offset(axes, theme);
    let columns = categories
        .iter()
        .map(|category| {
            let mut stack = div()
                .flex_1()
                .h_full()
                .column()
                .justify_end()
                .overflow_hidden();
            let mut used = 0.0;
            for (order, series) in series.iter().enumerate() {
                let Some(point) = series.points.iter().find(|point| &point.id == category) else {
                    continue;
                };
                let share = point.position.y.clamp(0.0, 1.0 - used);
                if share <= 0.0 {
                    continue;
                }
                used += share;
                let color = series.color.unwrap_or(theme.colors.accent);
                stack = stack.child(
                    div().w_full().h(relative(share)).bg(color).semantic_in(
                        cx,
                        NodeSpec::new(
                            ident
                                .child("series")
                                .child(series.id.as_ref())
                                .child("point")
                                .child(point.id.as_ref())
                                .semantic_id(),
                            Role::Status,
                        )
                        .parent(ident.semantic_id())
                        .text(point.label.clone())
                        .value(point.value.clone()),
                    ),
                );
                let _ = (order, active);
            }
            stack
        })
        .collect::<Vec<_>>();
    div()
        .column()
        .w_full()
        .gap_token(theme, Space::Xs)
        .child(chart_heading(label, axes.y_label.clone(), theme))
        .children(stale.map(|reason| stale_warning(ident, reason, theme, cx)))
        .child(
            div()
                .row()
                .items_end()
                .w_full()
                .gap_token(theme, Space::Xs)
                .children(y_axis(axes, theme))
                .child(
                    div()
                        .relative()
                        .row()
                        .items_end()
                        .flex_1()
                        .min_w_0()
                        .gap(px(theme.space(Space::Xs) + theme.space(Space::Xxs)))
                        .h(px(PLOT_HEIGHT))
                        .children(
                            value_rules(axes)
                                .into_iter()
                                .map(|fraction| value_rule(fraction, PLOT_HEIGHT, theme)),
                        )
                        .children(columns),
                ),
        )
        .child(
            div()
                .row()
                .ml(px(offset))
                .gap(px(theme.space(Space::Xs) + theme.space(Space::Xxs)))
                .children(categories.iter().map(|category| {
                    let label = series
                        .iter()
                        .flat_map(|series| series.points.iter())
                        .find(|point| &point.id == category)
                        .map(|point| point.label.clone())
                        .unwrap_or_else(|| category.clone());
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .type_scale(theme, TypeScale::Caption)
                        .text_align(gpui::TextAlign::Center)
                        .text_color(theme.colors.text_faint)
                        .child(label)
                })),
        )
        .child(x_axis(axes, offset > 0.0, theme))
        .child(series_legend(ident, series, theme, cx))
        .into_any_element()
}

/// A polar reading over host-owned axes.
///
/// Each point is an axis: `id` is the axis, `y` is the already-normalized
/// radius, and `label` is the host's wording. Angular order is the host's
/// point order. `x` is ignored.
#[derive(IntoElement)]
pub struct RadarChart {
    ident: Ident,
    label: SharedString,
    state: ChartState,
    slots: Slots,
}

impl RadarChart {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>, state: ChartState) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            state,
            slots: Slots::default(),
        }
    }
}

impl Slotted for RadarChart {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for RadarChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, spec): (AnyElement, NodeSpec) = match self.state.visible_series() {
            Some((series, stale)) => {
                let count = series.iter().map(|item| item.points.len()).sum();
                (
                    ready_radar(&self.ident, &self.label, series, stale.cloned(), &theme, cx),
                    chart_spec(&self.ident, &self.label, &self.state, count, stale),
                )
            }
            None => match &self.state {
                ChartState::Empty => (
                    self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                        non_ready_body(
                            &self.label,
                            EmptyState::new(
                                self.ident.child("empty"),
                                cx.strings().text(StringKey::RadarEmpty),
                            )
                            .kind(EmptyKind::Empty),
                            &theme,
                        )
                    }),
                    NodeSpec::new(self.ident.semantic_id(), Role::Status)
                        .text(self.label.clone())
                        .value("empty"),
                ),
                _ => line_like_state(
                    &self.ident,
                    &self.label,
                    &self.state,
                    &self.slots,
                    window,
                    cx,
                ),
            },
        };
        div().w_full().child(body).semantic_in(cx, spec)
    }
}

fn ready_radar(
    ident: &Ident,
    label: &SharedString,
    series: &[ChartSeries],
    stale: Option<SharedString>,
    theme: &gpui_kit_theme::Theme,
    cx: &mut App,
) -> AnyElement {
    let axes = series
        .first()
        .map(|first| first.points.as_slice())
        .unwrap_or(&[]);
    let axis_count = axes.len().max(3);
    let rings = [0.25, 0.5, 0.75, 1.0];
    let accent = theme.colors.accent;
    let faint = theme.colors.text_faint;
    let area_wash_alpha = theme.effects.area_wash_alpha;
    let plotted = series.to_vec();
    let axis_labels = axes
        .iter()
        .enumerate()
        .map(|(index, axis)| {
            let angle = radar_angle(index, axis_count);
            let horizontal = 0.5 + angle.cos() * 0.42;
            let vertical = 0.5 + angle.sin() * 0.42;
            div()
                .absolute()
                .left(relative(horizontal))
                .top(relative(vertical))
                .ml(px(-RADAR_LABEL_WIDTH / 2.0))
                .mt(px(RADAR_LABEL_Y_OFFSET))
                .w(px(RADAR_LABEL_WIDTH))
                .type_scale(theme, TypeScale::Caption)
                .text_align(gpui::TextAlign::Center)
                .text_color(theme.colors.text_muted)
                .child(axis.label.clone())
                .semantic_in(
                    cx,
                    NodeSpec::new(
                        ident.child("axis").child(axis.id.as_ref()).semantic_id(),
                        Role::Status,
                    )
                    .text(axis.label.clone())
                    .value(axis.value.clone()),
                )
        })
        .collect::<Vec<_>>();
    div()
        .column()
        .w_full()
        .gap_token(theme, Space::Xs)
        .child(chart_heading(label, None, theme))
        .children(stale.map(|reason| stale_warning(ident, reason, theme, cx)))
        .child(
            div()
                .relative()
                .w_full()
                .h(px(220.0))
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let width = f32::from(bounds.size.width);
                            let height = f32::from(bounds.size.height);
                            if width <= 0.0 || height <= 0.0 {
                                return;
                            }
                            let center = point(
                                bounds.origin.x + px(width / 2.0),
                                bounds.origin.y + px(height / 2.0),
                            );
                            // Leave a real label lane around the web. Labels
                            // collected in a row under the plot made angular
                            // order impossible to read without counting.
                            let radius = width.min(height) * 0.32;
                            for ring in rings {
                                let mut web = PathBuilder::stroke(px(1.0));
                                for index in 0..=axis_count {
                                    let angle = radar_angle(index % axis_count, axis_count);
                                    let spot = radar_point(center, radius * ring, angle);
                                    if index == 0 {
                                        web.move_to(spot);
                                    } else {
                                        web.line_to(spot);
                                    }
                                }
                                if let Ok(path) = web.build() {
                                    window.paint_path(path, faint);
                                }
                            }
                            for series in &plotted {
                                let color = series.color.unwrap_or(accent);
                                let mut fill = PathBuilder::fill();
                                let mut stroke = PathBuilder::stroke(px(1.5));
                                for (index, point) in series.points.iter().enumerate() {
                                    let angle = radar_angle(index, axis_count);
                                    let spot = radar_point(
                                        center,
                                        radius * point.position.y.clamp(0.0, 1.0),
                                        angle,
                                    );
                                    if index == 0 {
                                        fill.move_to(spot);
                                        stroke.move_to(spot);
                                    } else {
                                        fill.line_to(spot);
                                        stroke.line_to(spot);
                                    }
                                }
                                fill.close();
                                stroke.close();
                                if let Ok(path) = fill.build() {
                                    window.paint_path(path, color.opacity(area_wash_alpha));
                                }
                                if let Ok(path) = stroke.build() {
                                    window.paint_path(path, color);
                                }
                            }
                        },
                    )
                    .absolute()
                    .inset_0(),
                )
                .children(axis_labels),
        )
        .into_any_element()
}

fn radar_angle(index: usize, count: usize) -> f32 {
    -std::f32::consts::FRAC_PI_2 + (index as f32) * std::f32::consts::TAU / count as f32
}

fn radar_point(center: Point<Pixels>, radius: f32, angle: f32) -> Point<Pixels> {
    point(
        center.x + px(radius * angle.cos()),
        center.y + px(radius * angle.sin()),
    )
}

/// A single already-normalized reading on a semicircle.
///
/// `value` is a host-formatted string. `amount` is the needle, already in
/// `0..=1`. An unknown amount draws the scale and no needle.
#[derive(IntoElement)]
pub struct GaugeChart {
    ident: Ident,
    label: SharedString,
    state: ChartState,
    slots: Slots,
}

impl GaugeChart {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>, state: ChartState) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            state,
            slots: Slots::default(),
        }
    }
}

impl Slotted for GaugeChart {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for GaugeChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, spec): (AnyElement, NodeSpec) = match self.state.visible_series() {
            Some((series, stale)) => {
                let point = series
                    .first()
                    .and_then(|series| series.points.first())
                    .cloned();
                let color = series
                    .first()
                    .and_then(|series| series.color)
                    .unwrap_or(theme.colors.accent);
                (
                    ready_gauge(
                        &self.ident,
                        &self.label,
                        point.as_ref(),
                        color,
                        stale.cloned(),
                        &theme,
                        cx,
                    ),
                    chart_spec(
                        &self.ident,
                        &self.label,
                        &self.state,
                        usize::from(point.is_some()),
                        stale,
                    ),
                )
            }
            None => line_like_state(
                &self.ident,
                &self.label,
                &self.state,
                &self.slots,
                window,
                cx,
            ),
        };
        div().w_full().child(body).semantic_in(cx, spec)
    }
}

fn ready_gauge(
    ident: &Ident,
    label: &SharedString,
    reading: Option<&ChartPoint>,
    color: Hsla,
    stale: Option<SharedString>,
    theme: &gpui_kit_theme::Theme,
    cx: &mut App,
) -> AnyElement {
    let amount = reading.map(|point| point.position.y.clamp(0.0, 1.0));
    let track_alpha = theme.effects.soft_contrast_alpha;
    let wording = reading
        .map(|point| point.value.clone())
        .unwrap_or_else(|| cx.strings().text(StringKey::GaugeEmpty));
    div()
        .column()
        .w_full()
        .gap_token(theme, Space::Xs)
        .child(chart_heading(label, None, theme))
        .children(stale.map(|reason| stale_warning(ident, reason, theme, cx)))
        .child(
            div()
                .relative()
                .w_full()
                .h(px(120.0))
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let width = f32::from(bounds.size.width);
                            let height = f32::from(bounds.size.height);
                            if width <= 0.0 || height <= 0.0 {
                                return;
                            }
                            let center = point(
                                bounds.origin.x + px(width / 2.0),
                                bounds.origin.y + px(height * 0.86),
                            );
                            let radius = width.min(height * 1.6) * 0.42;
                            let mut track = PathBuilder::stroke(px(10.0));
                            gauge_arc(&mut track, center, radius, 0.0, 1.0);
                            if let Ok(path) = track.build() {
                                window.paint_path(path, color.opacity(track_alpha));
                            }
                            if let Some(amount) = amount {
                                let mut fill = PathBuilder::stroke(px(10.0));
                                gauge_arc(&mut fill, center, radius, 0.0, amount);
                                if let Ok(path) = fill.build() {
                                    window.paint_path(path, color);
                                }
                            }
                        },
                    )
                    .absolute()
                    .inset_0(),
                )
                // The reading belongs inside the scale's open centre. A
                // separate row beneath the canvas looked like a second metric
                // with no stated relationship to the arc.
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(16.0))
                        .type_scale(theme, TypeScale::Subtitle)
                        .text_align(gpui::TextAlign::Center)
                        .child(wording)
                        .semantic_in(
                            cx,
                            NodeSpec::new(ident.child("reading").semantic_id(), Role::Status)
                                .text(label.clone())
                                .value(
                                    reading.map(|point| point.value.clone()).unwrap_or_default(),
                                ),
                        ),
                ),
        )
        .into_any_element()
}

fn gauge_arc(builder: &mut PathBuilder, center: Point<Pixels>, radius: f32, start: f32, end: f32) {
    let steps = 24;
    for index in 0..=steps {
        let t = start + (end - start) * (index as f32 / steps as f32);
        let angle = std::f32::consts::PI * (1.0 - t);
        let spot = point(
            center.x + px(radius * angle.cos()),
            center.y - px(radius * angle.sin()),
        );
        if index == 0 {
            builder.move_to(spot);
        } else {
            builder.line_to(spot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point_at(id: &str, x: f32, y: f32) -> ChartPoint {
        ChartPoint::new(id, x, y, id, format!("{id}-value"))
    }

    #[test]
    fn points_keep_business_identity_and_exact_text() {
        let point = ChartPoint::new("minute-42", 0.4, 0.7, "12:42", "73.2%");
        let series = ChartSeries::new("cpu", "CPU").points([point.clone()]);
        assert_eq!(series.id.as_ref(), "cpu");
        assert_eq!(series.points[0], point);
        assert_eq!(series.points[0].value.as_ref(), "73.2%");
    }

    #[test]
    fn stale_keeps_verified_series_and_names_itself() {
        let state = ChartState::Stale {
            series: vec![ChartSeries::new("cpu", "CPU").points([point_at("now", 1.0, 0.5)])],
            reason: "refresh failed".into(),
        };
        let (series, reason) = state.visible_series().expect("verified data remains");
        assert_eq!(series[0].points[0].id.as_ref(), "now");
        assert_eq!(reason.expect("stale reason").as_ref(), "refresh failed");
        assert_eq!(state.name(), "stale");
    }

    #[test]
    fn invalid_and_duplicate_business_ids_are_not_published() {
        let theme = gpui_kit_theme::Theme::studio_dark();
        let series = vec![
            ChartSeries::new("cpu", "CPU").points([
                point_at("same", 0.0, 0.2),
                point_at("same", 0.4, 0.4),
                point_at("outside", 1.2, 0.4),
            ]),
            ChartSeries::new("cpu", "duplicate series").points([point_at("other", 1.0, 0.8)]),
        ];
        let active = active_points(&series, &theme);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].selection, ChartSelection::new("cpu", "same"));
    }

    #[test]
    fn nearest_uses_the_rendered_two_dimensional_distance() {
        let theme = gpui_kit_theme::Theme::studio_dark();
        let active = active_points(
            &[ChartSeries::new("cpu", "CPU")
                .points([point_at("low", 0.2, 0.2), point_at("high", 0.8, 0.8)])],
            &theme,
        );
        let bounds = Bounds::new(point(px(10.0), px(20.0)), gpui::size(px(200.0), px(100.0)));
        assert_eq!(
            nearest_point(&active, bounds, point(px(174.0), px(38.0))),
            Some(ChartSelection::new("cpu", "high"))
        );
    }

    #[test]
    fn keyboard_steps_by_geometry_without_replacing_business_ids() {
        let theme = gpui_kit_theme::Theme::studio_dark();
        let active = active_points(
            &[
                ChartSeries::new("cpu", "CPU")
                    .points([point_at("late", 0.8, 0.2), point_at("early", 0.2, 0.2)]),
                ChartSeries::new("memory", "Memory").points([point_at("nearest", 0.75, 0.6)]),
            ],
            &theme,
        );
        assert_eq!(
            step_point(&active, Some(&ChartSelection::new("cpu", "early")), "right"),
            Some(ChartSelection::new("cpu", "late"))
        );
        assert_eq!(
            step_point(&active, Some(&ChartSelection::new("cpu", "late")), "down"),
            Some(ChartSelection::new("memory", "nearest"))
        );
    }

    #[test]
    fn the_readout_stands_clear_of_the_sample_and_of_the_series_beside_it() {
        let theme = gpui_kit_theme::Theme::studio_dark();
        let rising = active_points(
            &[ChartSeries::new("cpu", "CPU").points([
                point_at("start", 0.0, 0.20),
                point_at("mid", 0.5, 0.30),
                point_at("now", 0.75, 0.70),
            ])],
            &theme,
        );
        let current = rising
            .iter()
            .find(|point| point.selection.point_id.as_ref() == "now")
            .expect("the sample is published")
            .clone();
        // The sample is on the trailing half, so the readout crosses to the
        // leading one; the series is low over there, so it takes the ceiling.
        assert_eq!(tooltip_anchor(&rising, &current), (true, false));

        let falling = active_points(
            &[ChartSeries::new("cpu", "CPU").points([
                point_at("now", 0.25, 0.30),
                point_at("later", 0.75, 0.80),
                point_at("last", 1.0, 0.90),
            ])],
            &theme,
        );
        let current = falling
            .iter()
            .find(|point| point.selection.point_id.as_ref() == "now")
            .expect("the sample is published")
            .clone();
        assert_eq!(tooltip_anchor(&falling, &current), (false, true));
    }
}

#[cfg(test)]
mod chart_phase_tests {
    use super::*;

    #[test]
    fn stale_projects_as_error_and_keeps_the_verified_series() {
        let state = ChartState::Stale {
            series: Vec::new(),
            reason: "offline".into(),
        };
        assert_eq!(state.phase(), Phase::Error);
        assert!(state.is_stale());
        assert_eq!(state.reason(), Some("offline"));
    }
}
