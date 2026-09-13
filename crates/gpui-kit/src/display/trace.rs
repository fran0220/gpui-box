//! A caller-owned execution trace as a waterfall of spans.
//!
//! Legacy `start` and `end` are normalized positions. [`TraceSpan::time`] and
//! [`TraceView::time_viewport`] accept raw UTC milliseconds through the shared
//! chart scale. The host owns the viewport, formatted labels, duration and
//! hierarchy; the component proposes changes and mounts only its row window.

mod index;
mod range;

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, relative, uniform_list,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};
use web_time::Instant;

use crate::display::badge::Tone;
use crate::display::chart::scale::{NumericScale, ScaleError, ScaleKind};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::status::StatusDot;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{FocusRing, Ident, StyledExt};
use crate::layout::measure;
use crate::motion;
use crate::motion::Flipping;
use crate::overlay::tooltip::Tooltipped;
use crate::strings::{ActiveStrings, StringKey};

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type ViewportHandler = Rc<dyn Fn(NumericScale, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(SharedString, bool, &mut Window, &mut App)>;
type TimeFormatter = Rc<dyn Fn(f64) -> SharedString>;
type WheelHandler = Rc<dyn Fn(&gpui::ScrollWheelEvent, &mut Window, &mut App)>;
type TimeSelectionHandler =
    Rc<dyn Fn(crate::interaction::range::RangeEvent, &mut Window, &mut App)>;

/// Shared temporal presentation, never an independent scale engine. Hierarchy
/// uses caller-supplied preorder and depth; collapsed descendants are omitted.
#[derive(Clone)]
struct TraceOptions {
    viewport: NumericScale,
    formatter: Option<TimeFormatter>,
    on_viewport: Option<ViewportHandler>,
    collapsed: HashSet<SharedString>,
    on_toggle: Option<ToggleHandler>,
    visible_rows: usize,
    animate_viewport: bool,
    animate_layout: bool,
    layout_animation: Option<motion::MotionSpec>,
    selected_time: Option<[f64; 2]>,
    on_time_selection: Option<TimeSelectionHandler>,
}

impl Default for TraceOptions {
    fn default() -> Self {
        Self {
            viewport: NumericScale::new(ScaleKind::Time, [0., 1.]).expect("unit time domain"),
            formatter: None,
            on_viewport: None,
            collapsed: HashSet::new(),
            on_toggle: None,
            visible_rows: 12,
            animate_viewport: true,
            animate_layout: true,
            layout_animation: None,
            selected_time: None,
            on_time_selection: None,
        }
    }
}

struct ViewportMotion {
    start: motion::Transition<f64>,
    end: motion::Transition<f64>,
}

/// Publication, not geometry: survivors move through FLIP before new rows are
/// mounted in the space they vacate. Remember only the mounted row window.
#[derive(Default)]
struct RowPublication {
    mounted: HashMap<SharedString, usize>,
    waiting: Option<(Instant, HashSet<SharedString>)>,
    scroll: Option<gpui::Point<gpui::Pixels>>,
    spec: Option<motion::MotionSpec>,
    selected: Option<SharedString>,
}

impl RowPublication {
    fn prepare(
        &mut self,
        hierarchy: &index::Hierarchy,
        scroll: gpui::Point<gpui::Pixels>,
        now: Instant,
        animate: bool,
        spec: motion::MotionSpec,
    ) -> bool {
        let scrolled = self.scroll.is_some_and(|previous| previous != scroll);
        self.scroll = Some(scroll);
        let moved = self
            .mounted
            .iter()
            .any(|(id, previous)| hierarchy.position(id).is_some_and(|rank| rank != *previous));
        if !animate || scrolled {
            self.waiting = None;
        } else if moved || (self.waiting.is_some() && self.spec != Some(spec)) {
            self.waiting = Some((now + spec.total(), self.mounted.keys().cloned().collect()));
        } else if self
            .waiting
            .as_ref()
            .is_some_and(|(until, _)| now >= *until)
        {
            self.waiting = None;
        }
        self.spec = Some(spec);
        self.mounted.clear();
        !scrolled && animate
    }

    fn publish(&mut self, id: &SharedString, rank: usize) -> bool {
        if self
            .waiting
            .as_ref()
            .is_some_and(|(_, allowed)| !allowed.contains(id))
        {
            return false;
        }
        self.mounted.insert(id.clone(), rank);
        true
    }
}

impl ViewportMotion {
    fn new(viewport: NumericScale, spec: motion::MotionSpec) -> Self {
        let [start, end] = viewport.domain();
        Self {
            start: motion::Transition::new(start, spec),
            end: motion::Transition::new(end, spec),
        }
    }

    fn target(&mut self, viewport: NumericScale, spec: motion::MotionSpec, snap: bool) {
        let [start, end] = viewport.domain();
        // Reversing orientation would pass through a zero-width domain. Publish
        // it directly rather than inventing a transient expanded time extent.
        let reversed = (self.start.target() < self.end.target()) != (start < end);
        self.start = self.start.spec(spec);
        self.end = self.end.spec(spec);
        if snap || reversed {
            self.start.snap(start);
            self.end.snap(end);
        } else {
            self.start.set(start);
            self.end.set(end);
        }
    }

    fn animate(
        &mut self,
        viewport: NumericScale,
        window: &mut Window,
        cx: &mut App,
    ) -> NumericScale {
        self.start.animate(window, cx);
        self.end.animate(window, cx);
        self.sampled_viewport(viewport)
    }

    fn sampled_viewport(&mut self, viewport: NumericScale) -> NumericScale {
        let start = self.start.value();
        let end = self.end.value();
        // Independent endpoints must never invert the requested orientation.
        // Overshoot can cross them; adjacent values can also round together.
        // Neither is a caller-requested reversed or expanded constant domain.
        if start != end
            && (start < end) == (viewport.domain()[0] < viewport.domain()[1])
            && let Ok(shown) = NumericScale::new(ScaleKind::Time, [start, end])
        {
            return shown;
        }
        self.start.snap(viewport.domain()[0]);
        self.end.snap(viewport.domain()[1]);
        viewport
    }
}

fn interval(span: &TraceSpan, viewport: NumericScale) -> Option<(f32, f32)> {
    let [start, end] = span.time.unwrap_or([span.start as f64, span.end as f64]);
    if end < start {
        return None;
    }
    let a = viewport.map(start)?;
    let b = viewport.map(end)?;
    let (a, b) = (a.min(b), a.max(b));
    if b < 0. || a > 1. {
        return None;
    }
    Some((a.max(0.) as f32, b.min(1.) as f32))
}

/// The label gutter of [`TraceView`]. Wide enough for a nested name, closed by
/// a hairline so the space left over reads as a column and not as a gap.
const LABEL_WIDTH: f32 = 148.0;
const DURATION_WIDTH: f32 = 64.0;
const ROW_HEIGHT: f32 = 28.0;
const BAR_HEIGHT: f32 = 18.0;
const AXIS_HEIGHT: f32 = 22.0;
const TICK_HEIGHT: f32 = 4.0;
const TICK_LABEL_WIDTH: f32 = 64.0;
/// The room kept outside the two ends of the track.
///
/// Every tick label is centred on its own gridline, including the ones at the
/// ends, so a reading always names the line under it. Without a gutter the
/// label at `0` would need half its width from outside the component, and the
/// track would begin at the frame edge — which is what made the first bar
/// look as though it started before the axis did.
const AXIS_GUTTER: f32 = TICK_LABEL_WIDTH / 2.0;
/// Where the grid is drawn when the host names no ticks of its own. Quarters
/// of a normalized axis are true without inventing a clock, so they carry a
/// line and no text; only a host-supplied tick may carry wording.
const DEFAULT_TICKS: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
/// A bar that ends past this much of the axis has its name drawn before it,
/// because there is no room left after it.
const NAME_AFTER_LIMIT: f32 = 0.7;
/// Alpha is part of the trace's data encoding: outline means pending, solid
/// means observed, and the current span is emphasized within either state.
/// These values are local together because no other component speaks this
/// waterfall vocabulary.
const PENDING_ALPHA: f32 = 0.16;
const PENDING_CURRENT_ALPHA: f32 = 0.24;
const OBSERVED_ALPHA: f32 = 0.72;
const OBSERVED_CURRENT_ALPHA: f32 = 0.92;

/// One position on the host's axis, with the host's exact wording for it.
#[derive(Debug, Clone, PartialEq)]
struct AxisTick {
    at: f32,
    label: Option<SharedString>,
}

fn resolved_ticks(
    ticks: &[AxisTick],
    start: Option<&SharedString>,
    end: Option<&SharedString>,
) -> Vec<AxisTick> {
    let mut resolved: Vec<AxisTick> = if ticks.is_empty() {
        DEFAULT_TICKS
            .iter()
            .map(|at| AxisTick {
                at: *at,
                label: None,
            })
            .collect()
    } else {
        ticks
            .iter()
            .map(|tick| AxisTick {
                at: tick.at.clamp(0.0, 1.0),
                label: tick.label.clone(),
            })
            .collect()
    };
    for (at, label) in [(0.0, start), (1.0, end)] {
        let Some(label) = label else { continue };
        match resolved.iter_mut().find(|tick| tick.at == at) {
            Some(tick) => tick.label = Some(label.clone()),
            None => resolved.push(AxisTick {
                at,
                label: Some(label.clone()),
            }),
        }
    }
    resolved.sort_by(|left, right| {
        left.at
            .partial_cmp(&right.at)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    resolved
}

/// What a span is doing, as the host already knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpanState {
    #[default]
    Pending,
    Running,
    Succeeded,
    Failed,
}

impl SpanState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    fn tone(self) -> Tone {
        match self {
            Self::Pending => Tone::Neutral,
            Self::Running => Tone::Info,
            Self::Succeeded => Tone::Success,
            Self::Failed => Tone::Danger,
        }
    }

    fn label(self, cx: &App) -> SharedString {
        cx.strings().text(match self {
            Self::Pending => StringKey::TracePending,
            Self::Running => StringKey::TraceRunning,
            Self::Succeeded => StringKey::TraceSucceeded,
            Self::Failed => StringKey::TraceFailed,
        })
    }
}

/// One already-timed interval on a unit axis.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceSpan {
    pub id: SharedString,
    pub label: SharedString,
    /// Inclusive start on the host's normalized axis, in `0..=1`.
    pub start: f32,
    /// Exclusive end on the same axis. Values outside the unit interval are
    /// clamped when drawn, never rewritten.
    pub end: f32,
    pub depth: u32,
    pub state: SpanState,
    pub detail: Option<SharedString>,
    /// The host's exact wording for how long this span took. The component
    /// never derives it from `start` and `end`: those are positions on a unit
    /// interval and know nothing about the clock behind them.
    pub duration: Option<SharedString>,
    /// Optional raw Unix millisecond interval. Overrides normalized positions
    /// only for geometry; labels and duration remain caller-owned.
    pub time: Option<[f64; 2]>,
}

impl TraceSpan {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        start: f32,
        end: f32,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            start,
            end,
            depth: 0,
            state: SpanState::Pending,
            detail: None,
            duration: None,
            time: None,
        }
    }

    pub fn depth(mut self, depth: u32) -> Self {
        self.depth = depth;
        self
    }

    pub fn state(mut self, state: SpanState) -> Self {
        self.state = state;
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// The already-formatted length of this span, shown beside its bar.
    pub fn duration(mut self, duration: impl Into<SharedString>) -> Self {
        self.duration = Some(duration.into());
        self
    }

    /// Raw-time entry point preserving sub-millisecond precision until mapping.
    /// Nonfinite or reversed intervals are retained but have no painted bar.
    pub fn time(mut self, start_ms: f64, end_ms: f64) -> Self {
        self.time = Some([start_ms, end_ms]);
        self
    }
}

/// Hierarchical labels beside a waterfall of caller-owned spans.
#[derive(IntoElement)]
pub struct TraceView {
    ident: Ident,
    label: SharedString,
    spans: Rc<Vec<TraceSpan>>,
    options: TraceOptions,
    axis_start: Option<SharedString>,
    axis_end: Option<SharedString>,
    ticks: Vec<AxisTick>,
    current: Option<SharedString>,
    on_select: Option<SelectHandler>,
    slots: Slots,
}

impl TraceView {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            spans: Rc::new(Vec::new()),
            options: TraceOptions::default(),
            axis_start: None,
            axis_end: None,
            ticks: Vec::new(),
            current: None,
            on_select: None,
            slots: Slots::default(),
        }
    }

    pub fn spans(mut self, spans: impl IntoIterator<Item = TraceSpan>) -> Self {
        self.spans = Rc::new(spans.into_iter().collect());
        self
    }

    /// Reuse caller-owned input across frames without cloning every span.
    pub fn shared_spans(mut self, spans: Rc<Vec<TraceSpan>>) -> Self {
        self.spans = spans;
        self
    }

    /// Caller-owned time window. The default remains the normalized [0, 1].
    pub fn time_viewport(mut self, domain: [f64; 2]) -> Result<Self, ScaleError> {
        self.options.viewport = NumericScale::new(ScaleKind::Time, domain)?;
        Ok(self)
    }

    /// Animate caller-applied time windows. Reduced motion and orientation
    /// reversal settle immediately; proposals always use the current caller window.
    pub fn animate_viewport(mut self, animate: bool) -> Self {
        self.options.animate_viewport = animate;
        self
    }

    /// Animate surviving mounted rows after caller-applied hierarchy changes.
    /// Enabled by default, independently of time viewport motion. Disabled or
    /// reduced motion settles immediately; removed rows have no exit animation.
    pub fn animate_layout(mut self, animate: bool) -> Self {
        self.options.animate_layout = animate;
        self
    }

    /// Override Tracking timing for row positions, not the time viewport.
    /// Retargeting starts at displayed geometry. New rows wait for survivors
    /// to settle, then mount in place without hidden handlers or semantics.
    pub fn layout_animation(mut self, spec: motion::MotionSpec) -> Self {
        self.options.layout_animation = Some(spec);
        self
    }

    /// Persistent caller-owned ascending raw time range. None clears it.
    /// A range outside the viewport remains authoritative but cannot be resized
    /// until the caller brings it into view; it is never silently normalized.
    pub fn selected_time(
        mut self,
        value: Option<[f64; 2]>,
    ) -> Result<Self, crate::interaction::range::RangeError> {
        range::validate(value)?;
        self.options.selected_time = value;
        Ok(self)
    }

    /// Shows a range strip and proposes Create/Resize/Move Update, Commit and
    /// Cancel events. Cancellation never rolls back previously accepted values.
    pub fn on_time_selection(
        mut self,
        handler: impl Fn(crate::interaction::range::RangeEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.options.on_time_selection = Some(Rc::new(handler));
        self
    }

    /// Formats raw-time ticks and human-facing range/span readouts. The caller
    /// owns locale, timezone and precision; event/selection values stay raw.
    pub fn format_time(mut self, format: impl Fn(f64) -> SharedString + 'static) -> Self {
        self.options.formatter = Some(Rc::new(format));
        self
    }

    /// Ctrl-wheel zooms at the pointer; shift-wheel pans. The host may refuse.
    pub fn on_viewport(
        mut self,
        handler: impl Fn(NumericScale, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.options.on_viewport = Some(Rc::new(handler));
        self
    }

    /// Collapsed identities in the caller's depth-first preorder span list.
    pub fn collapsed(mut self, ids: impl IntoIterator<Item = SharedString>) -> Self {
        self.options.collapsed = ids.into_iter().collect();
        self
    }

    /// Reports desired expansion, without changing caller-owned hierarchy.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.options.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Maximum viewport height in rows; GPUI may measure one additional row.
    /// Input scanning remains linear and is not a paint-work claim.
    pub fn visible_rows(mut self, count: usize) -> Self {
        self.options.visible_rows = count.max(1);
        self
    }

    /// Exact wording for the two ends of the host's time axis.
    pub fn axis(mut self, start: impl Into<SharedString>, end: impl Into<SharedString>) -> Self {
        self.axis_start = Some(start.into());
        self.axis_end = Some(end.into());
        self
    }

    /// Host-owned gridlines: a position on the same unit axis the spans use,
    /// and the exact text under it. Without any, the grid falls back to
    /// quarters of the axis, which carry a line and no wording.
    pub fn ticks<S: Into<SharedString>>(
        mut self,
        ticks: impl IntoIterator<Item = (f32, S)>,
    ) -> Self {
        self.ticks = ticks
            .into_iter()
            .map(|(at, label)| AxisTick {
                at,
                label: Some(label.into()),
            })
            .collect();
        self
    }

    pub fn current(mut self, id: impl Into<SharedString>) -> Self {
        self.current = Some(id.into());
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Slotted for TraceView {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for TraceView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ticks = resolved_ticks(
            &self.ticks,
            self.axis_start.as_ref(),
            self.axis_end.as_ref(),
        );
        waterfall(
            &self.ident,
            self.label,
            self.spans,
            &ticks,
            self.current.as_ref(),
            self.on_select,
            &self.slots,
            true,
            self.options,
            window,
            cx,
        )
    }
}

/// The waterfall alone: bars and a time axis, with each span named at its own
/// bar instead of in a gutter.
#[derive(IntoElement)]
pub struct SpanTimeline {
    ident: Ident,
    label: SharedString,
    spans: Rc<Vec<TraceSpan>>,
    options: TraceOptions,
    axis_start: Option<SharedString>,
    axis_end: Option<SharedString>,
    ticks: Vec<AxisTick>,
    current: Option<SharedString>,
    on_select: Option<SelectHandler>,
    slots: Slots,
}

impl SpanTimeline {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            spans: Rc::new(Vec::new()),
            options: TraceOptions::default(),
            axis_start: None,
            axis_end: None,
            ticks: Vec::new(),
            current: None,
            on_select: None,
            slots: Slots::default(),
        }
    }

    pub fn spans(mut self, spans: impl IntoIterator<Item = TraceSpan>) -> Self {
        self.spans = Rc::new(spans.into_iter().collect());
        self
    }

    /// Reuse caller-owned input across frames without cloning every span.
    pub fn shared_spans(mut self, spans: Rc<Vec<TraceSpan>>) -> Self {
        self.spans = spans;
        self
    }

    /// Caller-owned time window. The default remains the normalized [0, 1].
    pub fn time_viewport(mut self, domain: [f64; 2]) -> Result<Self, ScaleError> {
        self.options.viewport = NumericScale::new(ScaleKind::Time, domain)?;
        Ok(self)
    }

    /// Animate caller-applied time windows. Reduced motion and orientation
    /// reversal settle immediately; proposals always use the current caller window.
    pub fn animate_viewport(mut self, animate: bool) -> Self {
        self.options.animate_viewport = animate;
        self
    }

    /// Animate surviving mounted rows after caller-applied hierarchy changes.
    /// Enabled by default, independently of time viewport motion. Disabled or
    /// reduced motion settles immediately; removed rows have no exit animation.
    pub fn animate_layout(mut self, animate: bool) -> Self {
        self.options.animate_layout = animate;
        self
    }

    /// Override Tracking timing for row positions, not the time viewport.
    /// Retargeting starts at displayed geometry. New rows wait for survivors
    /// to settle, then mount in place without hidden handlers or semantics.
    pub fn layout_animation(mut self, spec: motion::MotionSpec) -> Self {
        self.options.layout_animation = Some(spec);
        self
    }

    /// Persistent caller-owned ascending raw time range. None clears it.
    /// Out-of-viewport values remain authoritative rather than being normalized.
    pub fn selected_time(
        mut self,
        value: Option<[f64; 2]>,
    ) -> Result<Self, crate::interaction::range::RangeError> {
        range::validate(value)?;
        self.options.selected_time = value;
        Ok(self)
    }

    /// Shows a range strip and proposes shared controlled range events.
    pub fn on_time_selection(
        mut self,
        handler: impl Fn(crate::interaction::range::RangeEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.options.on_time_selection = Some(Rc::new(handler));
        self
    }

    /// Formats raw-time ticks and human-facing range/span readouts. The caller
    /// owns locale, timezone and precision; event/selection values stay raw.
    pub fn format_time(mut self, format: impl Fn(f64) -> SharedString + 'static) -> Self {
        self.options.formatter = Some(Rc::new(format));
        self
    }

    /// Ctrl-wheel zooms at the pointer; shift-wheel pans. The host may refuse.
    pub fn on_viewport(
        mut self,
        handler: impl Fn(NumericScale, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.options.on_viewport = Some(Rc::new(handler));
        self
    }

    /// Collapsed identities in the caller's depth-first preorder span list.
    pub fn collapsed(mut self, ids: impl IntoIterator<Item = SharedString>) -> Self {
        self.options.collapsed = ids.into_iter().collect();
        self
    }

    /// Reports desired expansion, without changing caller-owned hierarchy.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.options.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Maximum viewport height in rows; GPUI may measure one additional row.
    /// Input scanning remains linear and is not a paint-work claim.
    pub fn visible_rows(mut self, count: usize) -> Self {
        self.options.visible_rows = count.max(1);
        self
    }

    pub fn axis(mut self, start: impl Into<SharedString>, end: impl Into<SharedString>) -> Self {
        self.axis_start = Some(start.into());
        self.axis_end = Some(end.into());
        self
    }

    /// Host-owned gridlines: a position on the same unit axis the spans use,
    /// and the exact text under it. Without any, the grid falls back to
    /// quarters of the axis, which carry a line and no wording.
    pub fn ticks<S: Into<SharedString>>(
        mut self,
        ticks: impl IntoIterator<Item = (f32, S)>,
    ) -> Self {
        self.ticks = ticks
            .into_iter()
            .map(|(at, label)| AxisTick {
                at,
                label: Some(label.into()),
            })
            .collect();
        self
    }

    pub fn current(mut self, id: impl Into<SharedString>) -> Self {
        self.current = Some(id.into());
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Slotted for SpanTimeline {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for SpanTimeline {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ticks = resolved_ticks(
            &self.ticks,
            self.axis_start.as_ref(),
            self.axis_end.as_ref(),
        );
        waterfall(
            &self.ident,
            self.label,
            self.spans,
            &ticks,
            self.current.as_ref(),
            self.on_select,
            &self.slots,
            false,
            self.options,
            window,
            cx,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn waterfall(
    ident: &Ident,
    label: SharedString,
    spans: Rc<Vec<TraceSpan>>,
    ticks: &[AxisTick],
    current: Option<&SharedString>,
    on_select: Option<SelectHandler>,
    slots: &Slots,
    with_label: bool,
    mut options: TraceOptions,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let theme = cx.theme().clone();
    let empty = spans.is_empty();
    let requested_viewport = options.viewport;
    let held_viewport = range::prepare(
        &ident.child("time-selection"),
        requested_viewport,
        options.selected_time,
        options.on_time_selection.clone(),
        window,
        cx,
    );
    let animation_target = held_viewport.unwrap_or(requested_viewport);
    let viewport_motion = motion::keyed::slot::<Option<ViewportMotion>>(
        &ident.child("time-motion").semantic_id(),
        window.window_handle().window_id(),
        cx,
    );
    let policy = motion::MotionPolicy::resolve(motion::MotionRole::Navigation, cx);
    {
        let mut state = viewport_motion.borrow_mut();
        let transition =
            state.get_or_insert_with(|| ViewportMotion::new(requested_viewport, policy.spec()));
        transition.target(
            animation_target,
            policy.spec(),
            held_viewport.is_some() || !options.animate_viewport || !policy.animates(),
        );
        options.viewport = transition.animate(animation_target, window, cx);
    }
    let indices = crate::motion::keyed::slot::<index::Hierarchy>(
        &ident.child("hierarchy").semantic_id(),
        window.window_handle().window_id(),
        cx,
    );
    indices.borrow_mut().update(&spans, &options.collapsed);
    let with_duration = indices.borrow().with_duration;
    let measured = measure::cell(&ident.child("measure").semantic_id(), window, cx);
    let left_gutter = AXIS_GUTTER
        + if with_label {
            LABEL_WIDTH + theme.space(Space::Sm)
        } else {
            0.
        };
    let right_gutter = AXIS_GUTTER
        + if with_duration {
            DURATION_WIDTH + theme.space(Space::Sm)
        } else {
            0.
        };
    let width = (f32::from(measured.get().size.width) - left_gutter - right_gutter).max(0.);
    let ticks = viewport_ticks(ticks, &options, width);
    let scroll = crate::data::viewport::scroll_handle(&ident.child("rows"), window, cx);
    let publication = motion::keyed::slot::<RowPublication>(
        &ident.child("row-publication").semantic_id(),
        window.window_handle().window_id(),
        cx,
    );
    let policy = motion::MotionPolicy::resolve(motion::MotionRole::Tracking, cx);
    // Direct navigation must reveal the current caller-selected row without
    // waiting for a decorative rearrangement, just like scrolling does.
    let selection_changed = publication.borrow().selected.as_ref() != current;
    publication.borrow_mut().selected = current.cloned();
    options.animate_layout = publication.borrow_mut().prepare(
        &indices.borrow(),
        scroll.0.borrow().base_handle.offset(),
        cx.background_executor().now(),
        options.animate_layout && policy.animates() && !selection_changed,
        options.layout_animation.unwrap_or(policy.spec()),
    );
    if publication.borrow().waiting.is_some() {
        window.request_animation_frame();
    }
    let wheel: Option<WheelHandler> = options.on_viewport.clone().map(|handler| {
        let measured = measured.clone();
        let viewport = requested_viewport;
        Rc::new(
            move |event: &gpui::ScrollWheelEvent, window: &mut Window, cx: &mut App| {
                if !event.modifiers.control && !event.modifiers.shift {
                    return;
                }
                let bounds = measured.get();
                let width = f32::from(bounds.size.width) - left_gutter - right_gutter;
                if width <= 0. {
                    return;
                }
                let anchor = ((f32::from(event.position.x - bounds.origin.x) - left_gutter)
                    / width)
                    .clamp(0., 1.);
                let delta = match event.delta {
                    gpui::ScrollDelta::Lines(delta) => delta.y * ROW_HEIGHT,
                    gpui::ScrollDelta::Pixels(delta) => f32::from(delta.y),
                };
                if let Ok(next) =
                    viewport_proposal(viewport, anchor, delta, width, event.modifiers.control)
                {
                    handler(next, window, cx);
                    cx.stop_propagation();
                }
            },
        ) as WheelHandler
    });
    let body = if empty {
        slots.or_else(slot::EMPTY, window, cx, |_, cx| {
            marked_empty(
                ident.child("empty"),
                cx.strings().text(StringKey::TraceEmpty),
                EmptyKind::Empty,
            )
            .into_any_element()
        })
    } else {
        let count = indices.borrow().len();
        let row_height = ROW_HEIGHT + theme.space(Space::Xs);
        let render_row = {
            let ident = ident.clone();
            let current = current.cloned();
            let theme = theme.clone();
            let options = options.clone();
            let indices = indices.clone();
            let spans = spans.clone();
            let on_select = on_select.clone();
            let wheel = wheel.clone();
            move |index: usize, window: &mut Window, cx: &mut App| {
                let source_index = indices
                    .borrow()
                    .get(index)
                    .unwrap_or_else(|| unreachable!());
                let span = &spans[source_index];
                if !publication.borrow_mut().publish(&span.id, index) {
                    return div().w_full().h(px(row_height)).into_any_element();
                }
                let branch = spans
                    .get(source_index + 1)
                    .is_some_and(|next| next.depth > span.depth);
                let mut row = div().w_full().h(px(row_height)).child(span_row(
                    &ident,
                    span,
                    with_label,
                    with_duration,
                    current.as_ref() == Some(&span.id),
                    on_select.clone(),
                    &options,
                    branch,
                    &theme,
                    cx,
                ));
                // A row owns modified-wheel time gestures before the list's
                // ordinary vertical scrolling. Unmodified wheels bubble on.
                if let Some(wheel) = &wheel {
                    let wheel = wheel.clone();
                    row = row.on_scroll_wheel(move |event, window, cx| wheel(event, window, cx));
                }
                // Only mounted business identities retain geometry. FLIP excludes
                // ambient scroll offsets and moves semantics and hit targets with
                // paint; the hierarchy remains the sole visible-rank authority.
                let flip = motion::flip(
                    ident
                        .child("row-motion")
                        .child(span.id.clone())
                        .semantic_id(),
                    window,
                    cx,
                );
                let row = row.flip(&flip, window, cx).animate(options.animate_layout);
                match options.layout_animation {
                    Some(spec) => row.animation(spec),
                    None => row,
                }
                .into_any_element()
            }
        };
        let rows = if count > options.visible_rows {
            uniform_list(
                ident.child("rows").element_id(),
                count,
                move |range, window, cx| {
                    range
                        .map(|index| render_row(index, window, cx))
                        .collect::<Vec<_>>()
                },
            )
            .track_scroll(&scroll)
            .w_full()
            .h(px(row_height * options.visible_rows as f32))
            .into_any_element()
        } else {
            div()
                .column()
                .w_full()
                .children((0..count).map(|index| render_row(index, window, cx)))
                .into_any_element()
        };
        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .child(
                div()
                    .relative()
                    .column()
                    .w_full()
                    // First, so every line is painted under the bars and the
                    // names rather than across them.
                    .child(grid_layer(&ticks, with_label, with_duration, &theme))
                    .child(rows),
            )
            .child(axis_row(&ticks, with_label, with_duration, &theme))
            .into_any_element()
    };

    let mut frame = div()
        .on_children_prepainted({
            let measured = measured.clone();
            move |bounds, window, _| {
                if let Some(body) = bounds.get(1) {
                    measure::record(&measured, *body, window);
                }
            }
        })
        .id(ident.element_id())
        .column()
        .w_full()
        .gap_token(&theme, Space::Xs)
        .child(
            div()
                .type_scale(&theme, TypeScale::Label)
                .text_color(theme.colors.text)
                .child(label.clone()),
        )
        .child(body);
    if on_select.is_some() || options.on_toggle.is_some() {
        let focus = crate::motion::keyed::slot::<Option<gpui::FocusHandle>>(
            &ident.child("focus").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let focus = focus
            .borrow_mut()
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        let selected = current.cloned();
        let toggle = options.on_toggle.clone();
        let collapsed = options.collapsed.clone();
        frame = frame
            .tab_index(0)
            .track_focus(&focus)
            .focus_ring(&theme)
            .on_key_down(move |event, window, cx| {
                let at = selected
                    .as_ref()
                    .and_then(|id| indices.borrow().position(id));
                let next = match event.keystroke.key.as_str() {
                    "down" => Some(at.map_or(0, |at| {
                        (at + 1).min(indices.borrow().len().saturating_sub(1))
                    })),
                    "up" => Some(at.unwrap_or(0).saturating_sub(1)),
                    "home" => Some(0),
                    "end" => Some(indices.borrow().len().saturating_sub(1)),
                    "left" | "right" => {
                        if let Some(at) = at {
                            let index = indices.borrow().get(at).unwrap_or_else(|| unreachable!());
                            let span = &spans[index];
                            let branch = spans
                                .get(index + 1)
                                .is_some_and(|next| next.depth > span.depth);
                            let expanded = event.keystroke.key == "right";
                            if branch && expanded == collapsed.contains(&span.id) {
                                if let Some(toggle) = &toggle {
                                    toggle(span.id.clone(), expanded, window, cx);
                                    cx.stop_propagation();
                                }
                                None
                            } else if expanded {
                                branch.then_some(at + 1)
                            } else {
                                indices.borrow().parent(index)
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                let source_index = next.and_then(|next| indices.borrow().get(next));
                if let Some(next) = next
                    && let Some(index) = source_index
                    && let Some(select) = &on_select
                {
                    select(spans[index].id.clone(), window, cx);
                    // The previously focused row may leave the virtual window.
                    // Keep keyboard navigation on the persistent container.
                    focus.focus(window, cx);
                    scroll.scroll_to_item(next, gpui::ScrollStrategy::Nearest);
                    window.refresh();
                    cx.stop_propagation();
                }
            });
    }
    if let Some(wheel) = wheel {
        frame = frame
            .tab_index(0)
            .focus_ring(&theme)
            .on_scroll_wheel(move |event, window, cx| wheel(event, window, cx));
    }
    frame = frame.children(range::render(
        &ident.child("time-selection"),
        &options,
        (left_gutter, right_gutter),
        window,
        cx,
    ));
    frame
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::List)
                .text(label)
                .value(if empty { "empty" } else { "ready" }),
        )
        .into_any_element()
}

fn viewport_proposal(
    viewport: NumericScale,
    anchor: f32,
    delta: f32,
    width: f32,
    zoom: bool,
) -> Result<NumericScale, ScaleError> {
    if zoom {
        viewport.zoom(anchor as f64, (delta as f64 / 400.).exp())
    } else {
        viewport.pan(delta as f64 / width as f64)
    }
}

fn viewport_ticks(ticks: &[AxisTick], options: &TraceOptions, width: f32) -> Vec<AxisTick> {
    let mut ticks: Vec<_> = if let Some(format) = &options.formatter {
        options
            .viewport
            .ticks((width / TICK_LABEL_WIDTH).floor() as usize)
            .into_iter()
            .filter_map(|value| {
                Some(AxisTick {
                    at: options.viewport.map(value)? as f32,
                    label: Some(format(value)),
                })
            })
            .collect()
    } else {
        ticks
            .iter()
            .filter_map(|tick| {
                let at = options.viewport.map(tick.at as f64)? as f32;
                (0. ..=1.).contains(&at).then(|| AxisTick {
                    at,
                    label: tick.label.clone(),
                })
            })
            .collect()
    };
    ticks.sort_by(|a, b| a.at.total_cmp(&b.at));
    let mut last = f32::NEG_INFINITY;
    for tick in &mut ticks {
        if tick.label.is_some() {
            let x = tick.at * width;
            if x - last < TICK_LABEL_WIDTH {
                tick.label = None;
            } else {
                last = x;
            }
        }
    }
    ticks
}

fn marked_empty(ident: Ident, label: SharedString, kind: EmptyKind) -> impl IntoElement {
    let mark_ident = ident.child("mark");
    div()
        .id(mark_ident.element_id())
        .child(EmptyState::new(ident, SharedString::default()).kind(kind))
        .tip(mark_ident, label)
}

/// The column every row measures its normalized positions inside: the same
/// flexible width with the same gutter, so a gridline, a bar and a tick label
/// at the same reading land on the same pixel.
fn track_column() -> gpui::Div {
    div().flex_1().min_w_0().px(px(AXIS_GUTTER))
}

fn grid_layer(
    ticks: &[AxisTick],
    with_label: bool,
    with_duration: bool,
    theme: &gpui_kit_theme::Theme,
) -> gpui::AnyElement {
    let lines = ticks.iter().map(|tick| {
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative(tick.at))
            .w(px(theme.borders.hairline))
            .rounded_full()
            .bg(theme.colors.divider.opacity(theme.opacity.muted))
    });
    let mut layer = div().absolute().inset_0().flex().flex_row();
    layer = layer.gap_token(theme, Space::Sm);
    if with_label {
        // The gutter is closed by one line down the whole plot. Drawn as a
        // border on each row it breaks at every gap and reads as a dashed rule.
        layer = layer.child(
            div().w(px(LABEL_WIDTH)).flex_none().relative().child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(px(theme.borders.hairline))
                    .rounded_full()
                    .bg(theme.colors.divider.opacity(theme.opacity.muted)),
            ),
        );
    }
    layer = layer.child(track_column().child(div().relative().size_full().children(lines)));
    if with_duration {
        layer = layer.child(div().w(px(DURATION_WIDTH)).flex_none());
    }
    layer.into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn span_row(
    parent: &Ident,
    span: &TraceSpan,
    with_label: bool,
    with_duration: bool,
    current: bool,
    on_select: Option<SelectHandler>,
    options: &TraceOptions,
    branch: bool,
    theme: &gpui_kit_theme::Theme,
    cx: &App,
) -> gpui::AnyElement {
    let ident = parent.child(span.id.as_ref());
    let visible = interval(span, options.viewport);
    let (start, end) = visible.unwrap_or((0., 0.));
    let width = end - start;
    let tone = span.state.tone();
    let color = tone.mark_color(None, theme);
    let status = span.state.label(cx);
    let mark = || {
        let mut mark = StatusDot::new(tone);
        if span.state == SpanState::Running {
            mark = mark
                .busy(ident.child("running"))
                .activity(motion::Activity::Advancing);
        }
        mark
    };

    // Work that has not started yet is a quieter tonal bar: its lower fill
    // does not claim that the interval happened, and no outline is needed.
    let pending = span.state == SpanState::Pending;
    let mut fill = div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(relative(start))
        .w(relative(width))
        .min_w(px(1.))
        .radius(theme, Radius::Small);
    fill = if pending {
        fill.bg(color.opacity(if current {
            PENDING_CURRENT_ALPHA
        } else {
            PENDING_ALPHA
        }))
    } else {
        fill.bg(color.opacity(if current {
            OBSERVED_CURRENT_ALPHA
        } else {
            OBSERVED_ALPHA
        }))
    };

    let fill = fill.semantic_in(
        cx,
        NodeSpec::new(ident.child("interval").semantic_id(), Role::Image)
            .parent(ident.semantic_id())
            .text(span.label.clone())
            .value(span.state.name()),
    );
    let mut track = div()
        .relative()
        .size_full()
        .overflow_hidden()
        .children(visible.map(|_| fill));
    if !with_label {
        // The waterfall names each span at its own bar. After the bar while
        // there is room after it, before it once there is not.
        let after = end <= NAME_AFTER_LIMIT;
        let mut name = div()
            .absolute()
            .top_0()
            .bottom_0()
            .row()
            .gap_token(theme, Space::Xs)
            .type_scale(theme, TypeScale::Caption)
            .text_color(theme.colors.text)
            .child(mark())
            .child(div().truncate().child(span.label.clone()));
        name = if !after && start < 1.0 - NAME_AFTER_LIMIT {
            // A span covering most/all of the viewport has no outside seat.
            // Keep its identity inside the track rather than clipping it off.
            name.left(relative(start)).pl(px(theme.space(Space::Xs)))
        } else if after {
            name.left(relative(end)).pl(px(theme.space(Space::Xs)))
        } else {
            name.right(relative(1.0 - start))
                .pr(px(theme.space(Space::Xs)))
        };
        track = track.child(name);
    }
    let track = track_column().h(px(BAR_HEIGHT)).child(track);

    let mut row = div()
        .id(ident.element_id())
        .relative()
        .row()
        .w_full()
        .items_center()
        .gap_token(theme, Space::Sm)
        .h(px(ROW_HEIGHT));
    if with_label {
        row = row.child(
            div()
                .row()
                .items_center()
                .w(px(LABEL_WIDTH))
                .h_full()
                .flex_none()
                .gap_token(theme, Space::Xs)
                .pl(px(span.depth as f32 * theme.space(Space::Md)
                    + if options.on_toggle.is_some() {
                        theme.space(Space::Md)
                    } else {
                        0.
                    }))
                .pr(px(theme.space(Space::Sm)))
                .child(mark())
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .type_scale(theme, TypeScale::Caption)
                        .text_color(theme.colors.text)
                        .child(span.label.clone()),
                ),
        );
    }
    row = row.child(track);
    if with_duration {
        row = row.child(
            div()
                .w(px(DURATION_WIDTH))
                .flex_none()
                .truncate()
                .type_scale(theme, TypeScale::Caption)
                .text_align(gpui::TextAlign::Right)
                .text_color(theme.colors.text_muted)
                .children(span.duration.clone()),
        );
    }
    if let Some(handler) = on_select {
        let click_handler = Rc::clone(&handler);
        let click_id = span.id.clone();
        let key_id = span.id.clone();
        row = row
            .cursor_pointer()
            .tab_index(0)
            .focus_ring(theme)
            .on_click(move |_, window, cx| click_handler(click_id.clone(), window, cx))
            .on_key_down(move |event, window, cx| {
                let target = match event.keystroke.key.as_str() {
                    "enter" | "space" => Some(key_id.clone()),
                    _ => None,
                };
                if let Some(id) = target {
                    handler(id, window, cx);
                    cx.stop_propagation();
                }
            });
    }
    let expanded = !options.collapsed.contains(&span.id);
    if branch && let Some(handler) = &options.on_toggle {
        let handler = handler.clone();
        let id = span.id.clone();
        let toggle = ident.child("toggle");
        let click_handler = handler.clone();
        let click_id = id.clone();
        row = row.child(
            div()
                .id(toggle.element_id())
                .absolute()
                .left_0()
                .top_0()
                .h_full()
                .row()
                .items_center()
                .tab_index(0)
                .focus_ring(theme)
                .cursor_pointer()
                .child(crate::display::icon::Icon::new(if expanded {
                    gpui_kit_assets::Icon::AltArrowDown
                } else {
                    gpui_kit_assets::Icon::AltArrowRight
                }))
                .on_click(move |_, window, cx| {
                    click_handler(click_id.clone(), !expanded, window, cx);
                    cx.stop_propagation();
                })
                .on_key_down(move |event, window, cx| {
                    let desired = match event.keystroke.key.as_str() {
                        "left" => Some(false),
                        "right" => Some(true),
                        "enter" | "space" => Some(!expanded),
                        _ => None,
                    };
                    if let Some(desired) = desired {
                        handler(id.clone(), desired, window, cx);
                        cx.stop_propagation();
                    }
                })
                .semantic_in(
                    cx,
                    NodeSpec::new(toggle.semantic_id(), Role::Button)
                        .text(span.label.clone())
                        .expanded(expanded),
                ),
        );
    }

    let mut spec = NodeSpec::new(ident.semantic_id(), Role::TreeItem)
        .parent(parent.semantic_id())
        .text(span.label.clone())
        .value(span.state.name())
        .selected(current)
        .level(span.depth.saturating_add(1));
    if branch {
        spec = spec.expanded(expanded);
    }
    let readout = exact_readout(span, &status, options.formatter.as_ref(), cx.strings());
    spec = spec.description(readout.clone());
    row.tip(ident, readout)
        .semantic_in(cx, spec)
        .into_any_element()
}

fn format_endpoint(
    value: f64,
    formatter: Option<&TimeFormatter>,
    strings: &crate::strings::Strings,
) -> SharedString {
    formatter.map_or_else(
        || strings.format(StringKey::TimeUtcValue, &[&value.to_string()]),
        |format| format(value),
    )
}

fn exact_readout(
    span: &TraceSpan,
    status: &str,
    formatter: Option<&TimeFormatter>,
    strings: &crate::strings::Strings,
) -> SharedString {
    let interval = if let Some([start, end]) = span.time {
        strings.format(
            StringKey::TraceInterval,
            &[
                &format_endpoint(start, formatter, strings),
                &format_endpoint(end, formatter, strings),
            ],
        )
    } else {
        strings.format(
            StringKey::TraceNormalizedInterval,
            &[&span.start.to_string(), &span.end.to_string()],
        )
    };
    let mut text = strings
        .format(StringKey::TraceReadout, &[&span.label, status, &interval])
        .to_string();
    if let Some(duration) = &span.duration {
        text.push('\n');
        text.push_str(&strings.format(StringKey::TraceDuration, &[duration]));
    }
    if let Some(detail) = &span.detail {
        text.push_str(&format!("\n{detail}"));
    }
    text.into()
}

fn axis_row(
    ticks: &[AxisTick],
    with_label: bool,
    with_duration: bool,
    theme: &gpui_kit_theme::Theme,
) -> gpui::AnyElement {
    let marks = ticks.iter().map(|tick| {
        div()
            .absolute()
            .top_0()
            .left(relative(tick.at))
            .w(px(theme.borders.hairline))
            .h(px(TICK_HEIGHT))
            .rounded_full()
            .bg(theme.colors.divider.opacity(theme.opacity.muted))
    });
    let labels = ticks.iter().filter_map(|tick| {
        let text = tick.label.clone()?;
        // Every reading is centred on the line it names, the ends included.
        // Aligned to the ends of the track instead, the two outermost
        // readings sat beside their own gridlines while every reading between
        // them sat on one, and a reader had to know which rule applied where.
        Some(
            div()
                .absolute()
                .top(px(TICK_HEIGHT + theme.borders.hairline))
                .left(relative(tick.at))
                .w_0()
                .h_0()
                .child(
                    div()
                        .absolute()
                        .left(px(-TICK_LABEL_WIDTH / 2.0))
                        .w(px(TICK_LABEL_WIDTH))
                        .truncate()
                        .text_align(gpui::TextAlign::Center)
                        .type_scale(theme, TypeScale::Caption)
                        .text_color(theme.colors.text_faint)
                        .child(text),
                ),
        )
    });

    let baseline = div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(theme.borders.hairline))
        .rounded_full()
        .bg(theme.colors.divider.opacity(theme.opacity.muted));

    let mut row = div()
        .flex()
        .flex_row()
        .w_full()
        .h(px(AXIS_HEIGHT))
        .gap_token(theme, Space::Sm);
    if with_label {
        row = row.child(div().w(px(LABEL_WIDTH)).flex_none());
    }
    row = row.child(
        track_column().child(
            div()
                .relative()
                .size_full()
                .child(baseline)
                .children(marks)
                .children(labels),
        ),
    );
    if with_duration {
        row = row.child(div().w(px(DURATION_WIDTH)).flex_none());
    }
    row.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_motion_keeps_epoch_precision_retargets_continuously_and_snaps_reversal() {
        let epoch = 1_700_000_000_000.;
        let scale = |a, b| {
            NumericScale::new(ScaleKind::Time, [epoch + a, epoch + b]).expect("fixture domain")
        };
        let spec = motion::MotionSpec::new(1000, motion::CubicBezier::new(0., 0., 1., 1.));
        let mut transition = ViewportMotion::new(scale(0., 100.), spec);
        transition.target(scale(17., 151.), spec, false);
        transition
            .start
            .advance(std::time::Duration::from_millis(500));
        transition
            .end
            .advance(std::time::Duration::from_millis(500));
        assert_eq!(transition.start.value(), epoch + 8.5);
        assert_eq!(transition.end.value(), epoch + 125.5);
        transition.target(scale(-11., 79.), spec, false);
        assert_eq!(transition.start.value(), epoch + 8.5);
        assert_eq!(transition.end.value(), epoch + 125.5);
        transition.target(scale(200., 50.), spec, false);
        assert_eq!(transition.start.value(), epoch + 200.);
        assert_eq!(transition.end.value(), epoch + 50.);
        transition.target(scale(0., 100.), spec, true);
        assert_eq!(transition.start.value(), epoch);
        assert_eq!(transition.end.value(), epoch + 100.);
    }

    #[test]
    fn viewport_navigation_policy_preserves_orientation_during_extreme_narrowing_and_retarget() {
        use gpui_kit_theme::Theme;

        for theme in [Theme::studio_light(), Theme::studio_dark()] {
            let spec = motion::MotionPolicy::spec(motion::MotionRole::Navigation, &theme);
            assert!(!spec.is_sprung());
            for reversed in [false, true] {
                let scale = |mut domain: [f64; 2]| {
                    if reversed {
                        domain.reverse();
                    }
                    NumericScale::new(ScaleKind::Time, domain).expect("finite narrowing fixture")
                };
                let mut target = scale([1530.125, 1530.25]);
                let mut state = ViewportMotion::new(scale([-2e9, 7e5]), spec);
                state.target(target, spec, false);
                for step in 0..400 {
                    if step == 7 {
                        target = scale([-95.75, -95.5]);
                        state.target(target, spec, false);
                    }
                    state.start.advance(std::time::Duration::from_millis(5));
                    state.end.advance(std::time::Duration::from_millis(5));
                    // Check the policy itself, before the safety guard can snap.
                    assert_ne!(state.start.value(), state.end.value());
                    assert_eq!(state.start.value() > state.end.value(), reversed);
                    let shown = state.sampled_viewport(target).domain();
                    assert_eq!(shown[0] > shown[1], reversed);
                }
                assert_eq!(state.sampled_viewport(target), target);
            }
        }
    }

    #[test]
    fn viewport_spring_endpoint_crossing_snaps_to_target_orientation() {
        let spec = motion::MotionSpec::sprung(motion::Spring::new(170., 2., 1.));
        for reversed in [false, true] {
            let scale = |mut domain: [f64; 2]| {
                if reversed {
                    domain.reverse();
                }
                NumericScale::new(ScaleKind::Time, domain).expect("finite spring fixture")
            };
            let mut target = scale([1530.125, 1530.25]);
            let mut state = ViewportMotion::new(scale([-2e9, 7e5]), spec);
            state.target(target, spec, false);
            let mut crossed = false;
            for step in 0..400 {
                if step == 6 {
                    target = scale([-95.75, -95.5]);
                    state.target(target, spec, false);
                }
                state.start.advance(std::time::Duration::from_millis(5));
                state.end.advance(std::time::Duration::from_millis(5));
                let inverted = (state.start.value() > state.end.value()) != reversed;
                crossed |= inverted;
                let shown = state.sampled_viewport(target).domain();
                assert_eq!(shown[0] > shown[1], reversed);
                assert_ne!(shown[0], shown[1]);
                if inverted {
                    assert_eq!(shown, target.domain());
                    assert!(!state.start.is_animating());
                    assert!(!state.end.is_animating());
                }
            }
            assert!(crossed, "fixture must exercise actual spring inversion");
        }
    }

    #[test]
    fn readout_preserves_exact_raw_times_and_caller_duration_instead_of_tick_rounding() {
        let span = TraceSpan::new("id", "Decode", 0., 1.)
            .time(1_700_000_000_025.125, 1_700_000_000_099.875)
            .duration("74.75 ms")
            .detail("fixture");
        assert_eq!(
            exact_readout(
                &span,
                "Succeeded",
                None,
                &crate::strings::Strings::default()
            )
            .as_ref(),
            "Decode · Succeeded\nStart: 1700000000025.125 ms UTC; End: 1700000000099.875 ms UTC\nDuration: 74.75 ms\nfixture"
        );
        assert_eq!(
            exact_readout(
                &TraceSpan::new("id", "Legacy", 0.125, 0.375),
                "Pending",
                None,
                &crate::strings::Strings::default()
            )
            .as_ref(),
            "Legacy · Pending\nStart: 0.125; End: 0.375 (normalized)"
        );
    }

    pub(super) fn visible_indices(
        spans: &[TraceSpan],
        collapsed: &HashSet<SharedString>,
    ) -> Vec<usize> {
        let mut hidden_below = None;
        spans
            .iter()
            .enumerate()
            .filter_map(|(index, span)| {
                if hidden_below.is_some_and(|depth| span.depth > depth) {
                    return None;
                }
                hidden_below = collapsed.contains(&span.id).then_some(span.depth);
                Some(index)
            })
            .collect()
    }

    #[test]
    fn raw_intervals_clip_and_reverse_without_losing_epoch_precision() {
        let epoch = 1_700_000_000_000.;
        let span = TraceSpan::new("raw", "raw", 0., 1.).time(epoch + 25., epoch + 75.);
        let forward =
            NumericScale::new(ScaleKind::Time, [epoch, epoch + 200.]).expect("forward domain");
        let reverse =
            NumericScale::new(ScaleKind::Time, [epoch + 200., epoch]).expect("reverse domain");
        assert_eq!(interval(&span, forward), Some((0.125, 0.375)));
        assert_eq!(interval(&span, reverse), Some((0.625, 0.875)));
        assert_eq!(
            interval(&span.clone().time(epoch - 20., epoch + 50.), forward),
            Some((0., 0.25))
        );
        assert_eq!(
            interval(&span.clone().time(epoch + 210., epoch + 250.), forward),
            None
        );
        assert_eq!(
            interval(&span.clone().time(epoch + 50., epoch + 20.), forward),
            None
        );
        assert_eq!(interval(&span.time(f64::NAN, epoch + 20.), forward), None);
    }

    #[test]
    fn collapse_hides_only_descendants_and_keeps_nested_collapse() {
        let spans: Vec<_> = [("a", 0), ("b", 1), ("c", 2), ("d", 1), ("e", 0), ("f", 1)]
            .into_iter()
            .map(|(id, depth)| TraceSpan::new(id, id, 0., 1.).depth(depth))
            .collect();
        assert_eq!(
            visible_indices(&spans, &HashSet::from(["b".into()])),
            vec![0, 1, 3, 4, 5]
        );
        assert_eq!(
            visible_indices(&spans, &HashSet::from(["a".into(), "b".into()])),
            vec![0, 4, 5]
        );
    }

    #[test]
    fn pointer_zoom_and_pan_propose_exact_time_windows() {
        let viewport = NumericScale::new(ScaleKind::Time, [100., 900.]).expect("valid domain");
        let zoomed =
            viewport_proposal(viewport, 0.25, 400. * 2f32.ln(), 640., true).expect("valid zoom");
        assert!((zoomed.invert(0.25).expect("finite anchor") - 300.).abs() < 0.001);
        let [start, end] = zoomed.domain();
        assert!((start - 200.).abs() < 0.001 && (end - 600.).abs() < 0.001);
        assert_eq!(
            viewport_proposal(viewport, 0.9, 80., 640., false)
                .expect("valid pan")
                .domain(),
            [0., 800.]
        );
        assert_eq!(viewport.domain(), [100., 900.]); // the proposal does not mutate authoritative state.
    }

    #[test]
    fn tick_labels_are_bounded_by_measured_width() {
        let options = TraceOptions {
            viewport: NumericScale::new(ScaleKind::Time, [1000., 9000.]).expect("valid domain"),
            formatter: Some(Rc::new(|value| format!("{value}").into())),
            ..TraceOptions::default()
        };
        for width in [80., 192., 640.] {
            let ticks = viewport_ticks(&[], &options, width);
            let labels: Vec<_> = ticks.iter().filter(|tick| tick.label.is_some()).collect();
            assert!(labels.len() <= (width / TICK_LABEL_WIDTH) as usize + 1);
            for pair in labels.windows(2) {
                assert!((pair[1].at - pair[0].at) * width >= TICK_LABEL_WIDTH - 0.001);
            }
            assert!(ticks.iter().all(|tick| (0. ..=1.).contains(&tick.at)));
        }
    }

    #[test]
    #[ignore = "explicit trace preprocessing workload; mount/paint measured separately"]
    fn trace_workloads() {
        use std::time::Instant;
        for count in [1000, 10000, 100000] {
            let begin = Instant::now();
            let spans: Vec<_> = (0..count)
                .map(|i| {
                    TraceSpan::new(format!("span.{i}"), "Work", 0., 1.)
                        .depth(if i % 16 == 0 { 0 } else { 1 })
                        .time(1000. + i as f64, 1017. + i as f64)
                })
                .collect();
            let spans = Rc::new(spans);
            let input = begin.elapsed();
            let begin = Instant::now();
            let mut indices = index::Hierarchy::default();
            indices.update(&spans, &HashSet::new());
            let hierarchy = begin.elapsed();
            let begin = Instant::now();
            indices.update(&spans, &HashSet::new());
            let retained = begin.elapsed();
            let begin = Instant::now();
            indices.update(&spans, &HashSet::from(["span.0".into()]));
            let collapse = begin.elapsed();
            assert_eq!(indices.len(), count - 15);
            assert_eq!(indices.get(1), Some(16));
            indices.update(&spans, &HashSet::new());
            let begin = Instant::now();
            let viewport =
                NumericScale::new(ScaleKind::Time, [1000., 1200.]).expect("valid domain");
            let mapped: Vec<_> = (0..12)
                .map(|index| interval(&spans[indices.get(index).expect("visible row")], viewport))
                .collect();
            assert_eq!(mapped.len(), 12);
            assert_eq!(mapped[0], Some((0., 0.085)));
            eprintln!(
                "trace spans={count} input={input:?} hierarchy_build={hierarchy:?} retained_update={retained:?} collapse_branch={collapse:?} twelve_row_mapping={:?}; mount/paint=not measured",
                begin.elapsed()
            );
        }
    }

    #[test]
    fn a_span_keeps_the_host_interval_and_names_its_state() {
        let span = TraceSpan::new("gen", "Generate", 0.1, 0.4)
            .depth(1)
            .state(SpanState::Running)
            .detail("model.reply");
        assert_eq!(span.id.as_ref(), "gen");
        assert_eq!(span.start, 0.1);
        assert_eq!(span.end, 0.4);
        assert_eq!(span.state.name(), "running");
        assert_ne!(SpanState::Succeeded, SpanState::Failed);
    }
}
