//! A caller-owned execution trace as a waterfall of spans.
//!
//! Times are already normalized by the host. This component does not invent a
//! clock, a duration, or a locale: `start` and `end` are positions on a unit
//! interval, and every label is a finished string. Clicking a span reports
//! its identity; the host decides whether a payload is shown.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};

use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::status::StatusDot;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Ident, StyledExt};
use crate::motion;
use crate::strings::{ActiveStrings, StringKey};

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

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
}

/// Hierarchical labels beside a waterfall of caller-owned spans.
#[derive(IntoElement)]
pub struct TraceView {
    ident: Ident,
    label: SharedString,
    spans: Vec<TraceSpan>,
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
            spans: Vec::new(),
            axis_start: None,
            axis_end: None,
            ticks: Vec::new(),
            current: None,
            on_select: None,
            slots: Slots::default(),
        }
    }

    pub fn spans(mut self, spans: impl IntoIterator<Item = TraceSpan>) -> Self {
        self.spans = spans.into_iter().collect();
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
            &self.spans,
            &ticks,
            self.current.as_ref(),
            self.on_select,
            &self.slots,
            true,
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
    spans: Vec<TraceSpan>,
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
            spans: Vec::new(),
            axis_start: None,
            axis_end: None,
            ticks: Vec::new(),
            current: None,
            on_select: None,
            slots: Slots::default(),
        }
    }

    pub fn spans(mut self, spans: impl IntoIterator<Item = TraceSpan>) -> Self {
        self.spans = spans.into_iter().collect();
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
            &self.spans,
            &ticks,
            self.current.as_ref(),
            self.on_select,
            &self.slots,
            false,
            window,
            cx,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn waterfall(
    ident: &Ident,
    label: SharedString,
    spans: &[TraceSpan],
    ticks: &[AxisTick],
    current: Option<&SharedString>,
    on_select: Option<SelectHandler>,
    slots: &Slots,
    with_label: bool,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let theme = cx.theme().clone();
    let empty = spans.is_empty();
    let with_duration = spans.iter().any(|span| span.duration.is_some());
    let body = if empty {
        slots.or_else(slot::EMPTY, window, cx, |_, cx| {
            EmptyState::new(
                ident.child("empty"),
                cx.strings().text(StringKey::TraceEmpty),
            )
            .kind(EmptyKind::Empty)
            .into_any_element()
        })
    } else {
        let rows: Vec<_> = spans
            .iter()
            .map(|span| {
                span_row(
                    ident,
                    span,
                    with_label,
                    with_duration,
                    current == Some(&span.id),
                    on_select.clone(),
                    &theme,
                    cx,
                )
            })
            .collect();
        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .child(
                div()
                    .relative()
                    .column()
                    .w_full()
                    .gap_token(&theme, Space::Xs)
                    // First, so every line is painted under the bars and the
                    // names rather than across them.
                    .child(grid_layer(ticks, with_label, with_duration, &theme))
                    .children(rows),
            )
            .child(axis_row(ticks, with_label, with_duration, &theme))
            .into_any_element()
    };

    div()
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
        .child(body)
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::List)
                .text(label)
                .value(if empty { "empty" } else { "ready" }),
        )
        .into_any_element()
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
            .bg(theme.colors.divider)
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
                    .bg(theme.colors.divider),
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
    theme: &gpui_kit_theme::Theme,
    cx: &App,
) -> gpui::AnyElement {
    let ident = parent.child(span.id.as_ref());
    let start = span.start.clamp(0.0, 1.0);
    let end = span.end.clamp(start, 1.0);
    let width = (end - start).max(0.02);
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

    // Work that has not started yet is drawn as an outline: a filled bar is a
    // claim that the interval happened.
    let pending = span.state == SpanState::Pending;
    let mut fill = div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(relative(start))
        .w(relative(width))
        .radius(theme, Radius::Small);
    fill = if pending {
        fill.bg(color.opacity(if current { 0.24 } else { 0.16 }))
            .border(px(theme.borders.hairline))
            .border_color(color.opacity(if current { 0.82 } else { 0.62 }))
    } else {
        fill.bg(color.opacity(if current { 0.92 } else { 0.72 }))
    };

    let mut track = div().relative().size_full().overflow_hidden().child(fill);
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
        name = if after {
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
                .pl(px(span.depth as f32 * theme.space(Space::Md)))
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
        let id = span.id.clone();
        row = row
            .cursor_pointer()
            .on_click(move |_, window, cx| handler(id.clone(), window, cx));
    }

    let mut spec = NodeSpec::new(ident.semantic_id(), Role::TreeItem)
        .parent(parent.semantic_id())
        .text(span.label.clone())
        .value(span.state.name())
        .selected(current)
        .level(span.depth.saturating_add(1));
    if let Some(detail) = &span.detail {
        spec = spec.description(detail.clone());
    } else {
        spec = spec.description(status);
    }
    row.semantic_in(cx, spec).into_any_element()
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
            .bg(theme.colors.divider)
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
        .bg(theme.colors.divider);

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
