//! Composed raw-data charts. Scales, visibility, hover, selection and viewport
//! remain controlled inputs; callbacks report requests, never mutate data.
//! Paint uses GPUI clipping and pointer capture. No chart-specific input shim.

use super::cartesian_performance::{HitIndex, ProjectionCache, sample_path};
use super::data::*;
use super::scale::NumericScale;
use super::{ChartLegend, ChartSelection, ChartSeries};
use crate::display::state_view::StateView;
use crate::foundation::{FocusRing, Ident, StyledExt};
use crate::layout::measure;
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};
use gpui::{
    App, Bounds, Hsla, MouseButton, PathBuilder, Pixels, Point, SharedString, Window, canvas, div,
    point, prelude::*, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, Surface, TypeScale};
use std::rc::Rc;

#[path = "cartesian_layout.rs"]
mod layout;
#[path = "cartesian_lifecycle.rs"]
mod lifecycle;
#[path = "cartesian_motion.rs"]
mod motion;
#[path = "cartesian_range.rs"]
mod range;
pub use range::CartesianRange;

/// Visualization lifecycle timing resolved through the existing motion engine.
/// Domain/orientation changes remain direct; reduced motion settles immediately.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CartesianMotion {
    pub enter: crate::motion::MotionSpec,
    pub update: crate::motion::MotionSpec,
    pub exit: crate::motion::MotionSpec,
}
impl CartesianMotion {
    pub fn themed(theme: &gpui_kit_theme::Theme) -> Self {
        use crate::motion::{MotionPolicy, MotionRole};
        Self {
            enter: MotionPolicy::spec(MotionRole::Entrance, theme),
            update: MotionPolicy::spec(MotionRole::Resize, theme),
            exit: MotionPolicy::spec(MotionRole::Exit, theme),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChartOrientation {
    #[default]
    Vertical,
    /// Independent x increases downward; value-axis readings increase rightward.
    Horizontal,
}

/// Leading means left for a vertical lane, top for a horizontal lane.
/// Trailing means right or bottom respectively.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisSide {
    Leading,
    Trailing,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CartesianEvent {
    Hover(Option<ChartSelection>),
    Select(Option<ChartSelection>),
    Visibility { series: SharedString, visible: bool },
    Emphasis(Option<SharedString>),
    Viewport(NumericScale),
    Brush([ChartValue; 2]),
    Range(crate::interaction::range::RangeEvent),
    Reset,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TooltipMode {
    #[default]
    Point,
    SharedAxis,
}

/// Current caller data for one rich tooltip row. Values are never interpolated.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartTooltipRow {
    pub series_id: SharedString,
    pub series_label: SharedString,
    pub point: RawPoint,
    pub color: Hsla,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChartTooltipData {
    pub anchor: ChartSelection,
    pub x: ChartValue,
    pub rows: Vec<ChartTooltipRow>,
}

/// Sampling changes paths, never source marks, identities or readouts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PathSampling {
    #[default]
    Exact,
    /// For ordered linear paths, retain per-pixel endpoints and extrema of
    /// both value and area baseline. Missing runs never join. Curves and
    /// unordered paths remain exact; semantic publication remains O(n).
    MinMax,
}

/// A caller-owned tick, including calendar/locale policy where required.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartTick {
    pub value: ChartValue,
    pub label: SharedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TickError {
    UnknownAxis,
    InvalidValue,
    OutsideDomain,
    DuplicateValue,
}

fn explicit_ticks(
    scale: &ChartScale,
    ticks: &[ChartTick],
) -> Result<Vec<(f64, SharedString)>, TickError> {
    let mut resolved = Vec::with_capacity(ticks.len());
    for (index, tick) in ticks.iter().enumerate() {
        let fraction = scale.map(&tick.value).ok_or(TickError::InvalidValue)?;
        if let (ChartScale::Numeric(scale), ChartValue::Number(value)) = (scale, &tick.value) {
            let [a, b] = scale.domain();
            if *value < a.min(b) || *value > a.max(b) {
                return Err(TickError::OutsideDomain);
            }
        }
        if !(0.0..=1.0).contains(&fraction) {
            return Err(TickError::OutsideDomain);
        }
        // Identity is raw, never the (potentially underflowed) projection.
        if ticks[..index]
            .iter()
            .any(|previous| previous.value == tick.value)
        {
            return Err(TickError::DuplicateValue);
        }
        resolved.push((fraction, tick.label.clone()));
    }
    resolved.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(resolved)
}

/// A line or shaded interval on an identified value axis. Its screen direction
/// follows the chart orientation.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartReference {
    pub id: SharedString,
    pub axis: SharedString,
    pub range: [f64; 2],
    pub label: SharedString,
    pub color: Option<Hsla>,
}

type EventHandler = Rc<dyn Fn(CartesianEvent, &mut Window, &mut App)>;
type Formatter = Rc<dyn Fn(&str, f64) -> SharedString>;
type TooltipBuilder = Rc<dyn Fn(&ChartTooltipData, &mut Window, &mut App) -> gpui::AnyElement>;
type MarkPainter = Rc<dyn Fn(Bounds<Pixels>, Hsla, &mut Window, &mut App)>;
type MarkBuilder = Rc<dyn Fn(&RawSeries, &RawPoint) -> Option<CustomMark>>;

/// A caller-painted scatter glyph, centered at the shared projected point.
/// Painting is clipped to this physical-pixel rectangle and the plot. Its exact
/// visible rectangle also owns hit testing and semantics, including during motion.
#[derive(Clone)]
pub struct CustomMark {
    size: [f32; 2],
    paint: MarkPainter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidMarkSize;

impl CustomMark {
    pub fn new(
        width: f32,
        height: f32,
        paint: impl Fn(Bounds<Pixels>, Hsla, &mut Window, &mut App) + 'static,
    ) -> Result<Self, InvalidMarkSize> {
        if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
            return Err(InvalidMarkSize);
        }
        Ok(Self {
            size: [width, height],
            paint: Rc::new(paint),
        })
    }
}

struct ChartStatus {
    phase: Phase,
    reason: Option<SharedString>,
    stale: bool,
}
impl HasPhase for ChartStatus {
    fn phase(&self) -> Phase {
        self.phase
    }
    fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
    fn is_stale(&self) -> bool {
        self.stale
    }
}

/// Shared-coordinate line/area/bar/scatter layers over raw f64 observations.
/// Missing samples break paths. All-zero data is ready, not empty. An invalid
/// dataset is an explicit error. Stale data remains visible with its reason.
#[derive(IntoElement)]
pub struct CartesianChart {
    ident: Ident,
    label: SharedString,
    x: ChartScale,
    axes: Vec<ValueAxis>,
    series: Rc<Vec<RawSeries>>,
    hidden: Vec<SharedString>,
    hovered: Option<ChartSelection>,
    selected: Option<ChartSelection>,
    emphasized: Option<SharedString>,
    tooltip: TooltipMode,
    tooltip_content: Option<TooltipBuilder>,
    floating_tooltip: bool,
    range: Option<CartesianRange>,
    references: Vec<ChartReference>,
    stale: Option<SharedString>,
    status: ChartStatus,
    height: f32,
    animate: bool,
    motion: Option<CartesianMotion>,
    sampling: PathSampling,
    format: Formatter,
    x_ticks: Option<Vec<(f64, SharedString)>>,
    axis_ticks: std::collections::HashMap<SharedString, Vec<(f64, SharedString)>>,
    orientation: ChartOrientation,
    x_side: Option<AxisSide>,
    axis_sides: std::collections::HashMap<SharedString, AxisSide>,
    custom_marks: Option<MarkBuilder>,
    on_event: Option<EventHandler>,
}

impl CartesianChart {
    pub fn new(
        ident: impl Into<Ident>,
        label: impl Into<SharedString>,
        x: ChartScale,
        axes: impl IntoIterator<Item = ValueAxis>,
    ) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            x,
            axes: axes.into_iter().collect(),
            series: Rc::new(Vec::new()),
            hidden: Vec::new(),
            hovered: None,
            selected: None,
            emphasized: None,
            tooltip: TooltipMode::Point,
            tooltip_content: None,
            floating_tooltip: true,
            range: None,
            references: Vec::new(),
            stale: None,
            status: ChartStatus {
                phase: Phase::Ready,
                reason: None,
                stale: false,
            },
            height: 220.,
            animate: true,
            motion: None,
            sampling: PathSampling::Exact,
            format: Rc::new(|_, v| v.to_string().into()),
            x_ticks: None,
            axis_ticks: Default::default(),
            orientation: ChartOrientation::Vertical,
            x_side: None,
            axis_sides: Default::default(),
            custom_marks: None,
            on_event: None,
        }
    }
    pub fn series(mut self, series: impl IntoIterator<Item = RawSeries>) -> Self {
        self.series = Rc::new(series.into_iter().collect());
        self
    }
    /// Reuse immutable caller data across redraws without cloning every point.
    /// Replace the Rc (or use Rc::make_mut) on a data update. Projection is
    /// reused only while data, domains, axes and hidden series are unchanged.
    pub fn shared_series(mut self, series: Rc<Vec<RawSeries>>) -> Self {
        self.series = series;
        self
    }
    /// Animate updates by series/point identity. Values and readouts always
    /// reflect current raw input; only geometry moves. Reduced motion settles
    /// immediately. Axis/topology changes snap to preserve direct manipulation.
    pub fn animate(mut self, enabled: bool) -> Self {
        self.animate = enabled;
        self
    }
    /// Configure keyed arrival, geometry/color updates and paint-only departure.
    /// This does not override `.animate(false)` or reduced-motion policy.
    pub fn motion(mut self, motion: CartesianMotion) -> Self {
        self.motion = Some(motion);
        self
    }
    /// Opt into pixel-column path reduction; the default preserves every
    /// segment. Point glyphs, hit bounds and exact raw values remain intact.
    pub fn sampling(mut self, sampling: PathSampling) -> Self {
        self.sampling = sampling;
        self
    }
    /// Extend scatter rendering without creating another coordinate system.
    /// Return None for the standard glyph. Missing points never invoke this
    /// builder. Compose a scatter series over other marks for custom annotations.
    pub fn custom_marks(
        mut self,
        build: impl Fn(&RawSeries, &RawPoint) -> Option<CustomMark> + 'static,
    ) -> Self {
        self.custom_marks = Some(Rc::new(build));
        self
    }
    /// Orient every layer, axis, semantic bound and gesture together.
    pub fn orientation(mut self, orientation: ChartOrientation) -> Self {
        self.orientation = orientation;
        self
    }
    /// Place the independent axis. Default is bottom (vertical charts) or left
    /// (horizontal charts); Leading means top/left, Trailing bottom/right.
    pub fn x_axis_side(mut self, side: AxisSide) -> Self {
        self.x_side = Some(side);
        self
    }
    /// Place a value axis on either plot edge. Default is left for vertical
    /// charts, bottom for horizontal charts. Multiple lanes reserve real space.
    pub fn axis_side(
        mut self,
        axis: impl Into<SharedString>,
        side: AxisSide,
    ) -> Result<Self, DataError> {
        let axis = axis.into();
        if !self.axes.iter().any(|a| a.id == axis) {
            return Err(DataError::UnknownAxis(axis));
        }
        self.axis_sides.insert(axis, side);
        Ok(self)
    }
    /// Use the shared Phase/AsyncValue state contract. Non-ready states install
    /// no data handlers; stale errors retain the last supplied series.
    pub fn state(mut self, state: impl HasPhase) -> Self {
        self.status = ChartStatus {
            phase: state.phase(),
            reason: state.reason().map(SharedString::from),
            stale: state.is_stale(),
        };
        self.stale = self
            .status
            .stale
            .then(|| self.status.reason.clone().unwrap_or_default());
        self
    }
    pub fn hidden(mut self, ids: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.hidden = ids.into_iter().map(Into::into).collect();
        self
    }
    pub fn hovered(mut self, point: Option<ChartSelection>) -> Self {
        self.hovered = point;
        self
    }
    pub fn selected(mut self, point: Option<ChartSelection>) -> Self {
        self.selected = point;
        self
    }
    /// Dim other series visually without removing hit or semantic targets.
    /// Unknown/hidden identities produce no emphasis. Legend hover only proposes.
    pub fn emphasized(mut self, series: Option<impl Into<SharedString>>) -> Self {
        self.emphasized = series.map(Into::into);
        self
    }
    pub fn tooltip(mut self, mode: TooltipMode) -> Self {
        self.tooltip = mode;
        self
    }
    /// Customize the read-only floating surface using current raw observations.
    /// Window-bounded placement and a retained accessible readout remain owned
    /// by the chart. Interactive controls belong outside this hover surface.
    pub fn tooltip_content(
        mut self,
        build: impl Fn(&ChartTooltipData, &mut Window, &mut App) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.tooltip_content = Some(Rc::new(build));
        self
    }
    /// Hide only the floating surface; the persistent accessible readout stays.
    pub fn floating_tooltip(mut self, visible: bool) -> Self {
        self.floating_tooltip = visible;
        self
    }
    /// Add a controlled persistent numeric selection and overview strip.
    pub fn range(mut self, range: CartesianRange) -> Self {
        self.range = Some(range);
        self
    }
    pub fn references(mut self, references: impl IntoIterator<Item = ChartReference>) -> Self {
        self.references = references.into_iter().collect();
        self
    }
    pub fn stale(mut self, reason: impl Into<SharedString>) -> Self {
        self.stale = Some(reason.into());
        self
    }
    pub fn height(mut self, height: f32) -> Self {
        if height.is_finite() {
            self.height = height.max(80.);
        }
        self
    }
    /// Format numeric ticks by axis ID; the x axis is identified by "x".
    pub fn format_ticks(mut self, format: impl Fn(&str, f64) -> SharedString + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }
    /// Replace automatic x ticks. An empty list hides ticks. Positions must be
    /// unique raw values in the domain; measured label collision suppression
    /// still applies. Tick labels are caller text, not formatted a second time.
    pub fn x_ticks(
        mut self,
        ticks: impl IntoIterator<Item = ChartTick>,
    ) -> Result<Self, TickError> {
        self.x_ticks = Some(explicit_ticks(
            &self.x,
            &ticks.into_iter().collect::<Vec<_>>(),
        )?);
        Ok(self)
    }
    /// Replace ticks/grid positions for one named value axis, without changing
    /// its domain. Unknown axes and invalid/duplicate/out-of-domain values fail.
    pub fn axis_ticks(
        mut self,
        axis: impl Into<SharedString>,
        ticks: impl IntoIterator<Item = ChartTick>,
    ) -> Result<Self, TickError> {
        let id = axis.into();
        let axis = self
            .axes
            .iter()
            .find(|axis| axis.id == id)
            .ok_or(TickError::UnknownAxis)?;
        let ticks = explicit_ticks(
            &ChartScale::Numeric(axis.scale),
            &ticks.into_iter().collect::<Vec<_>>(),
        )?;
        self.axis_ticks.insert(id, ticks);
        Ok(self)
    }
    pub fn on_event(
        mut self,
        handler: impl Fn(CartesianEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

#[derive(Clone)]
struct Hit {
    series: usize,
    point: usize,
    x: f64,
    y: f64,
    rect: [f64; 4],
}

impl Hit {
    fn matches(&self, selection: &ChartSelection, series: &[RawSeries]) -> bool {
        selection.series_id == series[self.series].id
            && selection.point_id == series[self.series].points[self.point].id
    }

    fn selection(&self, series: &[RawSeries]) -> ChartSelection {
        ChartSelection::new(
            series[self.series].id.clone(),
            series[self.series].points[self.point].id.clone(),
        )
    }
}

#[derive(Default)]
struct HitCache {
    projection: Option<Rc<Vec<ProjectedSeries>>>,
    dimensions: [f32; 2],
    custom: bool,
    hits: Rc<Vec<Hit>>,
    index: Option<Rc<HitIndex>>,
}

impl HitCache {
    fn get(
        &mut self,
        projection: &Rc<Vec<ProjectedSeries>>,
        dimensions: [f32; 2],
        custom: bool,
        build: impl FnOnce() -> Vec<Hit>,
    ) -> Rc<Vec<Hit>> {
        let same = !custom
            && !self.custom
            && self.dimensions == dimensions
            && self
                .projection
                .as_ref()
                .is_some_and(|previous| Rc::ptr_eq(previous, projection));
        if !same {
            self.hits = Rc::new(build());
            self.projection = Some(projection.clone());
            self.dimensions = dimensions;
            self.custom = custom;
            self.index = None;
        }
        self.hits.clone()
    }

    fn index(&mut self) -> Rc<HitIndex> {
        self.index
            .get_or_insert_with(|| Rc::new(HitIndex::new(self.hits.iter().map(|hit| hit.rect))))
            .clone()
    }
}

fn mark_center(p: &ProjectedPoint, s: &ProjectedSeries, mark: SeriesMark, band: f64) -> f64 {
    if matches!(mark, SeriesMark::Bar | SeriesMark::Range) {
        p.x + (s.bar_offset + s.bar_width / 2.) * band
    } else {
        p.x
    }
}

fn mark_rect(
    p: &ProjectedPoint,
    s: &ProjectedSeries,
    mark: SeriesMark,
    band: f64,
    width: f32,
    height: f32,
    custom_size: Option<[f32; 2]>,
) -> [f64; 4] {
    let [glyph_width, glyph_height] = custom_size.unwrap_or([6., 6.]);
    let dx = f64::from(glyph_width / 2.) / f64::from(width);
    let dy = f64::from(glyph_height / 2.) / f64::from(height);
    let (mut left, mut right, mut low, mut high) =
        if matches!(mark, SeriesMark::Bar | SeriesMark::Range) {
            let left = p.x + s.bar_offset * band;
            (
                left,
                left + s.bar_width * band,
                p.y.min(p.baseline),
                p.y.max(p.baseline),
            )
        } else {
            (p.x - dx, p.x + dx, p.y - dy, p.y + dy)
        };
    if high == low {
        low -= 0.5 / f64::from(height);
        high += 0.5 / f64::from(height);
    }
    if let Some([a, b]) = p.error {
        low = low.min(a.min(b));
        high = high.max(a.max(b));
        let center = mark_center(p, s, mark, band);
        left = left.min(center - 4. / f64::from(width));
        right = right.max(center + 4. / f64::from(width));
    }
    [left.max(0.), low.max(0.), right.min(1.), high.min(1.)]
}

fn nearest(
    hits: &[Hit],
    index: &HitIndex,
    series: &[RawSeries],
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
    shared: bool,
    orientation: ChartOrientation,
) -> Option<ChartSelection> {
    if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) || !bounds.contains(&position) {
        return None;
    }
    let x = f64::from(f32::from(position.x - bounds.origin.x));
    let y = f64::from(f32::from(position.y - bounds.origin.y));
    let w = f64::from(f32::from(bounds.size.width));
    let h = f64::from(f32::from(bounds.size.height));
    let (position, dimensions) = match orientation {
        ChartOrientation::Vertical => ([x / w, 1. - y / h], [w, h]),
        ChartOrientation::Horizontal => ([y / h, x / w], [h, w]),
    };
    index
        .nearest(position, dimensions, shared)
        .0
        .map(|i| hits[i].selection(series))
}

#[derive(Clone)]
struct Gesture {
    start: f64,
    current: f64,
    bounds: Bounds<Pixels>,
    scale: ChartScale,
    brush: bool,
    orientation: ChartOrientation,
}

impl RenderOnce for CartesianChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let orientation = self.orientation;
        let horizontal = orientation == ChartOrientation::Horizontal;
        if !matches!(self.status.phase, Phase::Ready | Phase::Refreshing) && !self.status.stale {
            return div()
                .column()
                .w_full()
                .child(self.label)
                .child(StateView::new(self.ident, self.status))
                .into_any_element();
        }
        let projection = crate::motion::keyed::slot::<ProjectionCache>(
            &self.ident.child("projection").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let mut projected =
            match projection
                .borrow_mut()
                .get(&self.series, &self.x, &self.axes, &self.hidden)
            {
                Ok(projected) => projected,
                Err(_) => {
                    return div()
                        .child(
                            cx.strings()
                                .format(StringKey::ChartInvalidData, &[&self.label]),
                        )
                        .semantic_in(
                            cx,
                            NodeSpec::new(self.ident.semantic_id(), Role::Status)
                                .text(self.label)
                                .value("error"),
                        )
                        .into_any_element();
                }
            };
        let mut reference_ids = std::collections::HashSet::new();
        for reference in &self.references {
            if !reference_ids.insert(&reference.id)
                || reference.range[0] > reference.range[1]
                || !self.axes.iter().any(|axis| {
                    axis.id == reference.axis
                        && reference.range.iter().all(|v| axis.scale.map(*v).is_some())
                })
            {
                return div()
                    .child(cx.strings().text(StringKey::ChartInvalidReference))
                    .semantic_in(
                        cx,
                        NodeSpec::new(self.ident.semantic_id(), Role::Status)
                            .text(self.label)
                            .value("error"),
                    )
                    .into_any_element();
            }
        }
        let geometry = crate::motion::keyed::slot::<motion::GeometryMotion>(
            &self.ident.child("geometry").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let revision = projected.clone();
        let timing = self
            .motion
            .unwrap_or_else(|| CartesianMotion::themed(&theme));
        geometry.borrow_mut().spec = Some(timing.update);
        if self.animate {
            projected = geometry.borrow_mut().animate(
                projected,
                &self.series,
                (self.x.clone(), self.axes.clone()),
                window,
                cx,
            );
        } else {
            *geometry.borrow_mut() = motion::GeometryMotion::default();
        }
        let lifecycle = crate::motion::keyed::slot::<lifecycle::Lifecycle>(
            &self.ident.child("lifecycle").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let paint_styles = if self.animate {
            lifecycle.borrow_mut().animate(
                self.series.clone(),
                (revision, projected.clone()),
                (self.x.clone(), self.axes.clone(), orientation),
                timing,
                window,
                cx,
            )
        } else {
            *lifecycle.borrow_mut() = lifecycle::Lifecycle::default();
            Rc::new(lifecycle::PaintStyles::default())
        };
        geometry
            .borrow_mut()
            .prune(!paint_styles.retired.is_empty());
        let range = range::build(&self, window, cx);
        let emphasis = lifecycle::emphasis(&self, window, cx);
        let measured = measure::cell(&self.ident.child("plot-bounds").semantic_id(), window, cx);
        let width = f32::from(measured.get().size.width);
        let width = if width > 0. { width } else { 320. };
        let [logical_width, logical_height] = orientation.dimensions(width, self.height);
        let font_size = theme.type_style(TypeScale::Caption).size;
        let measure_text = |text: &SharedString| {
            f32::from(
                window
                    .text_system()
                    .shape_line(
                        text.clone(),
                        px(font_size),
                        &[gpui::TextRun {
                            len: text.len(),
                            font: window.text_style().font(),
                            color: theme.colors.text_muted,
                            ..Default::default()
                        }],
                        None,
                    )
                    .width,
            )
        };
        let lane = |mut ticks: Vec<(f64, SharedString)>,
                    vertical: bool,
                    reverse: bool,
                    thickness: f32,
                    side: AxisSide| {
            if reverse {
                for (fraction, _) in &mut ticks {
                    *fraction = 1. - *fraction;
                }
            }
            ticks.sort_by(|a, b| a.0.total_cmp(&b.0));
            let length = if vertical { self.height } else { width };
            let mut end = f32::NEG_INFINITY;
            let labels = ticks
                .into_iter()
                .filter_map(|(fraction, text)| {
                    let extent = if vertical {
                        font_size
                    } else {
                        measure_text(&text)
                    };
                    let offset = (fraction as f32 * length - extent / 2.)
                        .clamp(0., (length - extent).max(0.));
                    if offset < end {
                        return None;
                    }
                    end = offset + extent + theme.space(Space::Xs);
                    Some(
                        div()
                            .absolute()
                            .when(vertical, |el| {
                                el.top(px(offset))
                                    .when(side == AxisSide::Leading, |el| {
                                        el.right(px(theme.space(Space::Xs)))
                                    })
                                    .when(side == AxisSide::Trailing, |el| {
                                        el.left(px(theme.space(Space::Xs)))
                                    })
                            })
                            .when(!vertical, |el| el.left(px(offset)))
                            .type_scale(&theme, TypeScale::Caption)
                            .text_color(theme.colors.text_muted)
                            .child(text),
                    )
                })
                .collect::<Vec<_>>();
            div()
                .relative()
                .flex_shrink_0()
                .when(vertical, |el| el.w(px(thickness)).h(px(self.height)))
                .when(!vertical, |el| el.w_full().h(px(font_size * 1.8)))
                .children(labels)
                .into_any_element()
        };
        let mut left_axes = Vec::new();
        let mut right_axes = Vec::new();
        let mut top_axes = Vec::new();
        let mut bottom_axes = Vec::new();
        let mut left_margin = 0.;
        let mut right_margin = 0.;
        for axis in &self.axes {
            let ticks = self.axis_ticks.get(&axis.id).cloned().unwrap_or_else(|| {
                axis.scale
                    .ticks((logical_height / if horizontal { 90. } else { 42. }) as usize)
                    .into_iter()
                    .map(|v| {
                        (
                            axis.scale.map(v).expect("tick belongs to axis domain"),
                            (self.format)(&axis.id, v),
                        )
                    })
                    .collect()
            });
            let thickness = ticks
                .iter()
                .map(|(_, label)| label)
                .chain(std::iter::once(&axis.label))
                .map(&measure_text)
                .fold(0., f32::max)
                + theme.space(Space::Sm);
            let side = self
                .axis_sides
                .get(&axis.id)
                .copied()
                .unwrap_or(if horizontal {
                    AxisSide::Trailing
                } else {
                    AxisSide::Leading
                });
            let element = lane(ticks, !horizontal, !horizontal, thickness, side);
            match (horizontal, side) {
                (false, AxisSide::Leading) => {
                    left_margin += thickness;
                    left_axes.push(element);
                }
                (false, AxisSide::Trailing) => {
                    right_margin += thickness;
                    right_axes.push(element);
                }
                (true, AxisSide::Leading) => top_axes.push(element),
                (true, AxisSide::Trailing) => bottom_axes.push(element),
            }
        }
        let x_ticks: Vec<(f64, SharedString)> = self.x_ticks.unwrap_or_else(|| match &self.x {
            ChartScale::Numeric(scale) => scale
                .ticks((logical_width / if horizontal { 42. } else { 90. }) as usize)
                .into_iter()
                .map(|v| {
                    (
                        scale.map(v).expect("tick belongs to x domain"),
                        (self.format)("x", v),
                    )
                })
                .collect(),
            ChartScale::Category(scale) => scale
                .categories()
                .iter()
                .map(|v| (scale.map(v).expect("category belongs to scale"), v.clone()))
                .collect(),
        });
        let thickness = x_ticks
            .iter()
            .map(|(_, label)| measure_text(label))
            .fold(0., f32::max)
            + theme.space(Space::Sm);
        let side = self.x_side.unwrap_or(if horizontal {
            AxisSide::Leading
        } else {
            AxisSide::Trailing
        });
        let element = lane(x_ticks, horizontal, false, thickness, side);
        match (horizontal, side) {
            (true, AxisSide::Leading) => {
                left_margin += thickness;
                left_axes.push(element);
            }
            (true, AxisSide::Trailing) => {
                right_margin += thickness;
                right_axes.push(element);
            }
            (false, AxisSide::Leading) => top_axes.push(element),
            (false, AxisSide::Trailing) => bottom_axes.push(element),
        }
        let band = match &self.x {
            ChartScale::Category(scale) => scale.bandwidth(),
            ChartScale::Numeric(_) => {
                let mut positions = projected
                    .iter()
                    .filter(|s| {
                        matches!(
                            self.series[s.source].mark,
                            SeriesMark::Bar | SeriesMark::Range
                        )
                    })
                    .flat_map(|s| s.points.iter().flatten().map(|p| p.x))
                    .collect::<Vec<_>>();
                positions.sort_by(f64::total_cmp);
                positions.dedup();
                positions.windows(2).map(|p| p[1] - p[0]).fold(1., f64::min)
            }
        };
        let mut custom = std::collections::HashMap::new();
        if let Some(build) = &self.custom_marks {
            for s in projected.iter() {
                let series = &self.series[s.source];
                if series.mark == SeriesMark::Scatter {
                    for p in s.points.iter().flatten() {
                        if let Some(mark) = build(series, &series.points[p.source]) {
                            custom.insert((s.source, p.source), mark);
                        }
                    }
                }
            }
        }
        let custom = Rc::new(custom);
        lifecycle.borrow_mut().custom(custom.clone(), band);
        let custom_size = |series, point| {
            custom
                .get(&(series, point))
                .map(|mark| orientation.dimensions(mark.size[0], mark.size[1]))
        };
        let hit_cache = crate::motion::keyed::slot::<HitCache>(
            &self.ident.child("hit-geometry").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let hits = hit_cache.borrow_mut().get(
            &projected,
            [logical_width, logical_height],
            self.custom_marks.is_some(),
            || {
                projected
                    .iter()
                    .flat_map(|s| {
                        s.points.iter().flatten().map(|p| {
                            let series = &self.series[s.source];
                            Hit {
                                series: s.source,
                                point: p.source,
                                x: mark_center(p, s, series.mark, band),
                                y: p.y,
                                rect: mark_rect(
                                    p,
                                    s,
                                    series.mark,
                                    band,
                                    logical_width,
                                    logical_height,
                                    custom_size(s.source, p.source),
                                ),
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            },
        );
        let mut semantics = Vec::new();
        for s in projected.iter() {
            for p in s.points.iter().flatten() {
                let source = &self.series[s.source];
                let raw = &source.points[p.source];
                let rect = mark_rect(
                    p,
                    s,
                    source.mark,
                    band,
                    logical_width,
                    logical_height,
                    custom_size(s.source, p.source),
                );
                if rect[0] > rect[2] || rect[1] > rect[3] {
                    continue;
                }
                let selected = self.selected.as_ref().is_some_and(|selection| {
                    selection.series_id == source.id && selection.point_id == raw.id
                });
                let rect = orientation.rect(rect);
                semantics.push(
                    div()
                        .absolute()
                        .left(relative(rect[0] as f32))
                        .top(relative(rect[1] as f32))
                        .w(relative((rect[2] - rect[0]) as f32))
                        .h(relative((rect[3] - rect[1]) as f32))
                        .semantic_in(
                            cx,
                            NodeSpec::new(
                                self.ident
                                    .child("series")
                                    .child(source.id.as_ref())
                                    .child("point")
                                    .child(raw.id.as_ref())
                                    .semantic_id(),
                                Role::Status,
                            )
                            .parent(self.ident.semantic_id())
                            .text(raw.label.clone())
                            .value(raw.formatted.clone())
                            .selected(selected),
                        ),
                );
            }
        }
        let painted_series = self.series.clone();
        let painted_axes = self.axes.clone();
        let grid_ticks = self
            .axes
            .first()
            .map(|axis| {
                self.axis_ticks
                    .get(&axis.id)
                    .map(|ticks| {
                        ticks
                            .iter()
                            .map(|(fraction, _)| *fraction)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|| {
                        axis.scale
                            .ticks((logical_height / if horizontal { 90. } else { 42. }) as usize)
                            .into_iter()
                            .map(|v| axis.scale.map(v).expect("tick belongs to axis domain"))
                            .collect()
                    })
            })
            .unwrap_or_default();
        let refs = self.references.clone();
        let paint_theme = theme.clone();
        let current = self.hovered.as_ref().or(self.selected.as_ref());
        let current_hit = current.and_then(|id| {
            hits.iter().find(|p| {
                p.matches(id, &self.series) && p.rect[0] <= p.rect[2] && p.rect[1] <= p.rect[3]
            })
        });
        let crosshair = current_hit.map(|p| p.x);
        let tooltip_data = current_hit.map(|current| {
            let x = self.series[current.series].points[current.point].x.clone();
            let mut rows = Vec::new();
            for (index, series) in self.series.iter().enumerate() {
                if self.hidden.contains(&series.id) {
                    continue;
                }
                for (point_index, raw) in series.points.iter().enumerate() {
                    if if self.tooltip == TooltipMode::SharedAxis {
                        raw.x == x
                    } else {
                        index == current.series && point_index == current.point
                    } {
                        rows.push(ChartTooltipRow {
                            series_id: series.id.clone(),
                            series_label: series.label.clone(),
                            point: raw.clone(),
                            color: raw
                                .color
                                .or(series.color)
                                .unwrap_or(theme.colors.sequence.get(index)),
                        });
                    }
                }
            }
            ChartTooltipData {
                anchor: current.selection(&self.series),
                x,
                rows,
            }
        });
        let readout = tooltip_data.as_ref().map(|data| {
            data.rows
                .iter()
                .map(|row| {
                    format!(
                        "{} · {}: {}",
                        row.series_label, row.point.label, row.point.formatted
                    )
                })
                .collect::<Vec<_>>()
                .join("  |  ")
        });
        let floating = if self.floating_tooltip {
            tooltip_data.as_ref().zip(current_hit).map(|(data, hit)| {
                let content = if let Some(build) = &self.tooltip_content {
                    build(data, window, cx)
                } else {
                    div()
                        .column()
                        .gap_token(&theme, Space::Xs)
                        .children(data.rows.iter().map(|row| {
                            div()
                                .row()
                                .items_center()
                                .gap_token(&theme, Space::Sm)
                                .child(
                                    div()
                                        .size(px(8.))
                                        .flex_shrink_0()
                                        .bg(row.color)
                                        .rounded_full(),
                                )
                                .child(
                                    div()
                                        .column()
                                        .child(row.series_label.clone())
                                        .child(row.point.label.clone()),
                                )
                                .child(div().ml_auto().child(row.point.formatted.clone()))
                        }))
                        .into_any_element()
                };
                let [x, y] = orientation.screen(hit.x, hit.y);
                let above = y > 0.5;
                let before = x > 0.5;
                let anchor = match (above, before) {
                    (false, false) => gpui::Anchor::TopLeft,
                    (false, true) => gpui::Anchor::TopRight,
                    (true, false) => gpui::Anchor::BottomLeft,
                    (true, true) => gpui::Anchor::BottomRight,
                };
                let surface = crate::overlay::surface(
                    self.ident.child("tooltip"),
                    &theme,
                    crate::overlay::OverlaySurface::FLOATING,
                )
                .p(px(theme.space(Space::Sm)))
                .max_w(px((f32::from(window.viewport_size().width)
                    - theme.space(Space::Sm) * 2.)
                    .clamp(1., 360.)))
                .max_h(px((f32::from(window.viewport_size().height)
                    - theme.space(Space::Sm) * 2.)
                    .max(1.)))
                .overflow_hidden()
                .type_scale(&theme, TypeScale::Caption)
                .child(content)
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("tooltip").semantic_id(), Role::Tooltip)
                        .parent(self.ident.semantic_id())
                        .text(self.label.clone())
                        .value(readout.clone().unwrap_or_default()),
                );
                div()
                    .absolute()
                    .left(relative(x.clamp(0., 1.) as f32))
                    .top(relative(y.clamp(0., 1.) as f32))
                    .size_0()
                    .child(
                        gpui::deferred(
                            gpui::anchored()
                                .anchor(anchor)
                                .snap_to_window_with_margin(px(theme.space(Space::Sm)))
                                .child(
                                    div()
                                        .when(before, |el| el.pr(px(theme.space(Space::Sm))))
                                        .when(!before, |el| el.pl(px(theme.space(Space::Sm))))
                                        .when(above, |el| el.pb(px(theme.space(Space::Sm))))
                                        .when(!above, |el| el.pt(px(theme.space(Space::Sm))))
                                        .child(surface),
                                ),
                        )
                        .unclipped()
                        .priority(1),
                    )
                    .into_any_element()
            })
        } else {
            None
        };
        let canvas = canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                let at = |x: f64, y: f64| orientation.at(bounds, x, y);
                let [paint_width, paint_height] = orientation
                    .dimensions(f32::from(bounds.size.width), f32::from(bounds.size.height));
                for y in &grid_ticks {
                    stroke(
                        window,
                        &[at(0., *y), at(1., *y)],
                        paint_theme.colors.hairline,
                        1.,
                    );
                }
                for reference in &refs {
                    let axis = painted_axes
                        .iter()
                        .find(|a| a.id == reference.axis)
                        .expect("reference axis validated before paint");
                    let a = axis
                        .scale
                        .map(reference.range[0])
                        .expect("validated reference endpoint");
                    let b = axis
                        .scale
                        .map(reference.range[1])
                        .expect("validated reference endpoint");
                    let color = reference.color.unwrap_or(paint_theme.colors.accent);
                    if a == b {
                        stroke(window, &[at(0., a), at(1., a)], color, 1.5);
                    } else {
                        fill(
                            window,
                            &[at(0., a), at(1., a), at(1., b), at(0., b)],
                            color.opacity(paint_theme.effects.area_wash_alpha),
                        );
                    }
                }
                for (s, data, custom, band) in paint_styles
                    .retired
                    .iter()
                    .map(|layer| (&layer.projected, &layer.raw, &layer.custom, layer.band))
                    .chain(
                        projected
                            .iter()
                            .map(|s| (s, &painted_series, &custom, band)),
                    )
                {
                    let source = &data[s.source];
                    let base_color = source
                        .color
                        .unwrap_or(paint_theme.colors.sequence.get(s.source));
                    let color = paint_styles.series(source, base_color);
                    let opacity = emphasis.get(&source.id).copied().unwrap_or(1.);
                    for run in s.points.split(Option::is_none) {
                        let run = run.iter().flatten().collect::<Vec<_>>();
                        if run.is_empty() {
                            continue;
                        }
                        if matches!(source.mark, SeriesMark::Line | SeriesMark::Area) {
                            let path_points = if self.sampling == PathSampling::MinMax
                                && source.curve == Curve::Linear
                            {
                                let coordinates = run
                                    .iter()
                                    .map(|p| [p.x, p.y, p.baseline])
                                    .collect::<Vec<_>>();
                                sample_path(&coordinates, paint_width.ceil() as usize)
                                    .into_iter()
                                    .map(|i| run[i])
                                    .collect::<Vec<_>>()
                            } else {
                                run.clone()
                            };
                            let samples =
                                path_points.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>();
                            if source.mark == SeriesMark::Area {
                                let mut path = PathBuilder::fill();
                                path.move_to(at(samples[0][0], samples[0][1]));
                                trace(&mut path, &samples, source.curve, &at);
                                let baseline = path_points
                                    .iter()
                                    .rev()
                                    .map(|p| [p.x, p.baseline])
                                    .collect::<Vec<_>>();
                                path.line_to(at(baseline[0][0], baseline[0][1]));
                                let reverse_curve = match source.curve {
                                    Curve::StepBefore => Curve::StepAfter,
                                    Curve::StepAfter => Curve::StepBefore,
                                    curve => curve,
                                };
                                trace(&mut path, &baseline, reverse_curve, &at);
                                path.close();
                                if let Ok(path) = path.build() {
                                    window.paint_path(
                                        path,
                                        color
                                            .opacity(paint_theme.effects.area_wash_alpha * opacity),
                                    );
                                }
                            }
                            let mut path = PathBuilder::stroke(px(2.));
                            path.move_to(at(samples[0][0], samples[0][1]));
                            trace(&mut path, &samples, source.curve, &at);
                            if let Ok(path) = path.build() {
                                window.paint_path(path, color.opacity(opacity));
                            }
                        }
                        for p in run {
                            let size = custom
                                .get(&(s.source, p.source))
                                .map(|mark| orientation.dimensions(mark.size[0], mark.size[1]));
                            let rect =
                                mark_rect(p, s, source.mark, band, paint_width, paint_height, size);
                            if rect[0] > rect[2] || rect[1] > rect[3] {
                                continue;
                            }
                            let color = paint_styles
                                .point(source, &source.points[p.source], color)
                                .opacity(opacity);
                            if matches!(source.mark, SeriesMark::Bar | SeriesMark::Range) {
                                let a = p.x + s.bar_offset * band;
                                let b = a + s.bar_width * band;
                                if p.y == p.baseline {
                                    stroke(window, &[at(a, p.y), at(b, p.y)], color, 1.);
                                } else {
                                    fill(
                                        window,
                                        &[
                                            at(a, p.baseline),
                                            at(a, p.y),
                                            at(b, p.y),
                                            at(b, p.baseline),
                                        ],
                                        color,
                                    );
                                }
                            }
                            if source.mark == SeriesMark::Scatter || source.mark == SeriesMark::Line
                            {
                                let c = at(p.x, p.y);
                                if let Some(mark) = custom.get(&(s.source, p.source)) {
                                    let bounds = Bounds::new(
                                        point(
                                            c.x - px(mark.size[0] / 2.),
                                            c.y - px(mark.size[1] / 2.),
                                        ),
                                        gpui::size(px(mark.size[0]), px(mark.size[1])),
                                    );
                                    window.with_content_mask(
                                        Some(gpui::ContentMask { bounds }),
                                        |window| (mark.paint)(bounds, color, window, cx),
                                    );
                                } else {
                                    fill(
                                        window,
                                        &[
                                            point(c.x - px(3.), c.y),
                                            point(c.x, c.y - px(3.)),
                                            point(c.x + px(3.), c.y),
                                            point(c.x, c.y + px(3.)),
                                        ],
                                        color,
                                    );
                                }
                            }
                            if let Some([low, high]) = p.error {
                                let center = mark_center(p, s, source.mark, band);
                                let a = at(center, low);
                                let b = at(center, high);
                                stroke(window, &[a, b], color, 1.5);
                                for q in [a, b] {
                                    let cap = if horizontal {
                                        [point(q.x, q.y - px(4.)), point(q.x, q.y + px(4.))]
                                    } else {
                                        [point(q.x - px(4.), q.y), point(q.x + px(4.), q.y)]
                                    };
                                    stroke(window, &cap, color, 1.5);
                                }
                            }
                        }
                    }
                }
                if let Some(x) = crosshair {
                    stroke(
                        window,
                        &[at(x, 0.), at(x, 1.)],
                        paint_theme.colors.text_muted,
                        1.,
                    );
                }
            },
        )
        .w_full()
        .h_full();
        let mut plot = div()
            .on_children_prepainted({
                let measured = measured.clone();
                move |bounds, window, _| {
                    if let Some(bounds) = bounds.first() {
                        measure::record(&measured, *bounds, window);
                    }
                }
            })
            .id(self.ident.child("plot").element_id())
            .relative()
            .flex_1()
            .min_w_0()
            .h(px(self.height))
            .overflow_hidden()
            .surface(&theme, Surface::Canvas)
            .child(canvas)
            .children(semantics)
            .children(floating);
        if let Some(report) = self.on_event.clone() {
            let hit_index = hit_cache.borrow_mut().index();
            let gesture = crate::motion::keyed::slot::<Option<Gesture>>(
                &self.ident.child("gesture").semantic_id(),
                window.window_handle().window_id(),
                cx,
            );
            let cancel = gesture.clone();
            if let Some(g) = gesture.borrow().as_ref().filter(|g| g.brush) {
                let start = g.start.clamp(0., 1.);
                let end = g.current.clamp(0., 1.);
                plot = plot.child(
                    div()
                        .absolute()
                        .when(g.orientation == ChartOrientation::Vertical, |el| {
                            el.top_0()
                                .bottom_0()
                                .left(relative(start.min(end) as f32))
                                .w(relative((end - start).abs() as f32))
                        })
                        .when(g.orientation == ChartOrientation::Horizontal, |el| {
                            el.left_0()
                                .right_0()
                                .top(relative(start.min(end) as f32))
                                .h(relative((end - start).abs() as f32))
                        })
                        .bg(theme.colors.accent.opacity(theme.effects.area_wash_alpha))
                        .semantic_in(
                            cx,
                            NodeSpec::new(
                                self.ident.child("brush-preview").semantic_id(),
                                Role::Status,
                            )
                            .parent(self.ident.semantic_id())
                            .value(format!("{start}..{end}")),
                        ),
                );
            }
            plot = plot
                .child(crate::interaction::on_pointer_cancel(move |_, _| {
                    *cancel.borrow_mut() = None;
                }))
                .tab_index(0)
                .focus_ring(&theme);
            let move_report = report.clone();
            let move_hits = hits.clone();
            let move_index = hit_index.clone();
            let move_series = self.series.clone();
            let move_bounds = measured.clone();
            let moving = gesture.clone();
            let shared = self.tooltip == TooltipMode::SharedAxis;
            plot = plot.on_mouse_move(move |event, window, cx| {
                let proposal = {
                    let mut state = moving.borrow_mut();
                    state.as_mut().and_then(|g| {
                        g.current = g.orientation.fraction(g.bounds, event.position);
                        if g.brush {
                            None
                        } else if let ChartScale::Numeric(scale) = g.scale {
                            scale.pan(g.current - g.start).ok()
                        } else {
                            None
                        }
                    })
                };
                if let Some(scale) = proposal {
                    move_report(CartesianEvent::Viewport(scale), window, cx);
                }
                if moving.borrow().is_some() {
                    window.refresh();
                }
                move_report(
                    CartesianEvent::Hover(nearest(
                        &move_hits,
                        &move_index,
                        &move_series,
                        move_bounds.get(),
                        event.position,
                        shared,
                        orientation,
                    )),
                    window,
                    cx,
                );
            });
            let leave = report.clone();
            plot = plot.on_hover(move |hovered, window, cx| {
                if !hovered {
                    leave(CartesianEvent::Hover(None), window, cx);
                }
            });
            let down = gesture.clone();
            let bounds = measured.clone();
            let scale = self.x.clone();
            plot =
                plot.on_mouse_down_with_pointer_capture(MouseButton::Left, move |event, _, _| {
                    let b = bounds.get();
                    if b.size.width > px(0.) && b.size.height > px(0.) {
                        *down.borrow_mut() = Some(Gesture {
                            start: orientation.fraction(b, event.position),
                            current: orientation.fraction(b, event.position),
                            bounds: b,
                            scale: scale.clone(),
                            brush: event.modifiers.shift,
                            orientation,
                        });
                    }
                });
            let up = gesture.clone();
            let up_report = report.clone();
            let up_hits = hits.clone();
            let up_index = hit_index;
            let up_series = self.series.clone();
            plot = plot.on_mouse_up(MouseButton::Left, move |event, window, cx| {
                if let Some(g) = up.borrow_mut().take() {
                    let b = g.bounds;
                    if b.size.width <= px(0.) {
                        return;
                    }
                    let end = g.orientation.fraction(b, event.position);
                    if (end - g.start).abs() < 0.005 {
                        up_report(
                            CartesianEvent::Select(nearest(
                                &up_hits,
                                &up_index,
                                &up_series,
                                b,
                                event.position,
                                shared,
                                g.orientation,
                            )),
                            window,
                            cx,
                        );
                    } else if g.brush {
                        if let (Some(a), Some(b)) = (
                            g.scale.invert(g.start.clamp(0., 1.)),
                            g.scale.invert(end.clamp(0., 1.)),
                        ) {
                            up_report(CartesianEvent::Brush([a, b]), window, cx);
                        }
                    } else if let ChartScale::Numeric(scale) = g.scale
                        && let Ok(next) = scale.pan(end - g.start)
                    {
                        up_report(CartesianEvent::Viewport(next), window, cx);
                    }
                    window.refresh();
                }
            });
            let wheel = report.clone();
            let bounds = measured.clone();
            let scale = self.x.clone();
            plot = plot.on_scroll_wheel(move |event, window, cx| {
                if let ChartScale::Numeric(scale) = scale {
                    let b = bounds.get();
                    if b.size.width <= px(0.) || b.size.height <= px(0.) {
                        return;
                    }
                    let anchor = orientation.fraction(b, event.position);
                    let delta = f64::from(f32::from(event.delta.pixel_delta(px(20.)).y));
                    if let Ok(next) = scale.zoom(anchor, (-delta / 300.).exp()) {
                        wheel(CartesianEvent::Viewport(next), window, cx);
                        cx.stop_propagation();
                    }
                }
            });
            let key_hits = hits.clone();
            let key_series = self.series.clone();
            let selection = self.selected.clone();
            plot = plot.on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "escape" {
                    *gesture.borrow_mut() = None;
                    window.release_pointer();
                    window.refresh();
                    report(CartesianEvent::Select(None), window, cx);
                } else if key == "home" {
                    report(CartesianEvent::Reset, window, cx);
                } else if matches!(key, "left" | "right" | "up" | "down") && !key_hits.is_empty() {
                    let mut ordered = key_hits
                        .iter()
                        .filter(|p| p.rect[0] <= p.rect[2] && p.rect[1] <= p.rect[3])
                        .collect::<Vec<_>>();
                    if ordered.is_empty() {
                        return;
                    }
                    ordered.sort_by(|a, b| {
                        let a = orientation.screen(a.x, a.y);
                        let b = orientation.screen(b.x, b.y);
                        if matches!(key, "up" | "down") {
                            a[1].total_cmp(&b[1])
                        } else {
                            a[0].total_cmp(&b[0])
                        }
                    });
                    let current = selection
                        .as_ref()
                        .and_then(|id| ordered.iter().position(|p| p.matches(id, &key_series)));
                    let next = match (current, key) {
                        (Some(i), "left" | "up") => i.saturating_sub(1),
                        (Some(i), _) => (i + 1).min(ordered.len() - 1),
                        _ => 0,
                    };
                    report(
                        CartesianEvent::Select(Some(ordered[next].selection(&key_series))),
                        window,
                        cx,
                    );
                } else {
                    return;
                }
                cx.stop_propagation();
            });
        }
        let plot = plot.semantic_in(
            cx,
            NodeSpec::new(self.ident.child("plot").semantic_id(), Role::Group)
                .parent(self.ident.semantic_id())
                .text(self.label.clone()),
        );
        let legend_series = self
            .series
            .iter()
            .enumerate()
            .map(|(i, s)| {
                ChartSeries::new(s.id.clone(), s.label.clone())
                    .tint(s.color.unwrap_or(theme.colors.sequence.get(i)))
            })
            .collect::<Vec<_>>();
        let mut legend =
            ChartLegend::new(self.ident.child("legend"), legend_series).hidden(self.hidden);
        if let Some(report) = self.on_event {
            let hover = report.clone();
            legend = legend
                .on_emphasis(move |series, window, cx| {
                    hover(CartesianEvent::Emphasis(series), window, cx)
                })
                .on_toggle(move |series, visible, window, cx| {
                    report(CartesianEvent::Visibility { series, visible }, window, cx)
                });
        }
        let has_data = !hits.is_empty();
        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .child(
                div()
                    .type_scale(&theme, TypeScale::Label)
                    .child(self.label.clone()),
            )
            .child(
                div()
                    .row()
                    .gap_token(&theme, Space::Sm)
                    .children(self.axes.iter().map(|axis| {
                        div()
                            .type_scale(&theme, TypeScale::Caption)
                            .child(axis.label.clone())
                    })),
            )
            .children(
                self.stale.as_ref().map(|reason| {
                    div().child(cx.strings().format(StringKey::ChartStale, &[reason]))
                }),
            )
            .children((!top_axes.is_empty()).then(|| {
                div()
                    .column()
                    .ml(px(left_margin))
                    .mr(px(right_margin))
                    .children(top_axes)
            }))
            .child(
                div()
                    .row()
                    .w_full()
                    .pt(px(font_size / 2.))
                    .child(div().row().children(left_axes))
                    .child(plot)
                    .child(div().row().children(right_axes)),
            )
            .children((!bottom_axes.is_empty()).then(|| {
                div()
                    .column()
                    .ml(px(left_margin))
                    .mr(px(right_margin))
                    .children(bottom_axes)
            }))
            .children((!has_data).then(|| div().child(cx.strings().text(StringKey::ChartEmpty))))
            .children(readout.map(|text| {
                div()
                    .type_scale(&theme, TypeScale::Caption)
                    .child(text.clone())
                    .semantic_in(
                        cx,
                        NodeSpec::new(self.ident.child("readout").semantic_id(), Role::Status)
                            .parent(self.ident.semantic_id())
                            .text(self.label.clone())
                            .value(text),
                    )
            }))
            .children(self.references.iter().map(|reference| {
                div()
                    .type_scale(&theme, TypeScale::Caption)
                    .child(reference.label.clone())
                    .semantic_in(
                        cx,
                        NodeSpec::new(
                            self.ident
                                .child("reference")
                                .child(reference.id.as_ref())
                                .semantic_id(),
                            Role::Status,
                        )
                        .text(reference.label.clone())
                        .value(format!("{}..{}", reference.range[0], reference.range[1])),
                    )
            }))
            .child(legend)
            .children(range)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .text(self.label)
                    .value(if self.stale.is_some() {
                        "stale"
                    } else if has_data {
                        "ready"
                    } else {
                        "empty"
                    }),
            )
            .into_any_element()
    }
}

fn stroke(window: &mut Window, points: &[Point<Pixels>], color: Hsla, width: f32) {
    let mut path = PathBuilder::stroke(px(width));
    for (i, p) in points.iter().enumerate() {
        if i == 0 {
            path.move_to(*p);
        } else {
            path.line_to(*p);
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
fn fill(window: &mut Window, points: &[Point<Pixels>], color: Hsla) {
    let mut path = PathBuilder::fill();
    for (i, p) in points.iter().enumerate() {
        if i == 0 {
            path.move_to(*p);
        } else {
            path.line_to(*p);
        }
    }
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
fn trace(
    path: &mut PathBuilder,
    samples: &[[f64; 2]],
    curve: Curve,
    at: &impl Fn(f64, f64) -> Point<Pixels>,
) {
    let tangents = if curve == Curve::Monotone {
        monotone_tangents(samples)
    } else {
        None
    };
    for (i, pair) in samples.windows(2).enumerate() {
        let [a, b] = [pair[0], pair[1]];
        match curve {
            Curve::StepBefore => {
                path.line_to(at(a[0], b[1]));
                path.line_to(at(b[0], b[1]));
            }
            Curve::StepAfter => {
                path.line_to(at(b[0], a[1]));
                path.line_to(at(b[0], b[1]));
            }
            Curve::Monotone if tangents.is_some() => {
                let m = tangents.as_ref().expect("monotone guard checked tangents");
                let dx = (b[0] - a[0]) / 3.;
                path.cubic_bezier_to(
                    at(b[0], b[1]),
                    at(a[0] + dx, a[1] + dx * m[i]),
                    at(b[0] - dx, b[1] - dx * m[i + 1]),
                );
            }
            _ => path.line_to(at(b[0], b[1])),
        }
    }
}

#[cfg(test)]
#[path = "cartesian_tests.rs"]
mod tests;
