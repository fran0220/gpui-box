//! A deliberately narrow trend reading.
//!
//! A sparkline has no axes, ticks, legend, tooltip, locale or scale policy.
//! Broader cartesian surfaces belong in this library as later primitives;
//! they are not this component.
//! The caller supplies points already normalized into the inclusive `0..=1`
//! square, plus the exact label and current, minimum and maximum text a reader
//! should receive. `x = 0` is the leading edge, `x = 1` the trailing edge,
//! `y = 0` the bottom and `y = 1` the top. Points outside that square or with
//! non-finite coordinates are skipped rather than clamped into a value the
//! caller did not supply.
//!
//! GPUI's existing stroked path is used directly. The path has no motion,
//! locale lookup or layout-dependent sampling, so the same normalized points
//! produce the same geometry for the same bounds.

use gpui::{
    AnyElement, App, Hsla, InteractiveElement, IntoElement, ParentElement, PathBuilder, RenderOnce,
    SharedString, Styled, Window, canvas, div, point, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, TypeScale};

use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::loading::PulseLoader;
use crate::display::status::StatusDot;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Ident, StyledExt};
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

/// One point already normalized by the caller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SparklinePoint {
    pub x: f32,
    pub y: f32,
}

impl SparklinePoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn is_bounded(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && (0.0..=1.0).contains(&self.x)
            && (0.0..=1.0).contains(&self.y)
    }
}

/// The plotted points and the exact text that makes them accessible.
#[derive(Debug, Clone, PartialEq)]
pub struct SparklineReading {
    pub points: Vec<SparklinePoint>,
    pub current: SharedString,
    pub minimum: SharedString,
    pub maximum: SharedString,
}

impl SparklineReading {
    pub fn new(
        points: impl IntoIterator<Item = SparklinePoint>,
        current: impl Into<SharedString>,
        minimum: impl Into<SharedString>,
        maximum: impl Into<SharedString>,
    ) -> Self {
        Self {
            points: points.into_iter().collect(),
            current: current.into(),
            minimum: minimum.into(),
            maximum: maximum.into(),
        }
    }

    /// How many supplied points are inside the documented normalized bounds.
    pub fn published_points(&self) -> usize {
        self.points
            .iter()
            .filter(|point| point.is_bounded())
            .count()
    }
}

/// The complete state of one trend reading.
#[derive(Debug, Clone, PartialEq)]
pub enum SparklineState {
    Loading,
    Ready(SparklineReading),
    Empty,
    Unavailable(SharedString),
    Error(SharedString),
    /// The reading is the last verified value; the text says why it is stale.
    Stale {
        reading: SparklineReading,
        reason: SharedString,
    },
}

impl SparklineState {
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
}

impl HasPhase for SparklineState {
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

/// A compact, accessible trend reading.
#[derive(Debug, IntoElement)]
pub struct Sparkline {
    ident: Ident,
    label: SharedString,
    state: SparklineState,
    tint: Option<Hsla>,
    embedded: bool,
    embedded_stale: bool,
    slots: Slots,
}

impl Sparkline {
    pub fn new(
        ident: impl Into<Ident>,
        label: impl Into<SharedString>,
        state: SparklineState,
    ) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            state,
            tint: None,
            embedded: false,
            embedded_stale: false,
            slots: Slots::default(),
        }
    }

    /// The colour this series is known by.
    ///
    /// A trend is neutral by default, because one line in a box does not need
    /// a colour to be told apart from the lines that are not there. A caller
    /// that has already spent a colour on this series — a legend, a chart it
    /// sits beside — passes it here so the two agree.
    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    /// The plot alone, for a surface that already has a frame and a label.
    ///
    /// An embedded sparkline publishes no node: the card it is inside already
    /// announces the reading, and a plot that repeats it says the value
    /// twice. It draws no minimum or maximum either, which is what stops a
    /// host that only had a current value from being made to state a range it
    /// never supplied.
    pub fn embedded(mut self) -> Self {
        self.embedded = true;
        self
    }

    /// Whether an embedded plot is drawing a reading that is no longer
    /// verified. The framed form takes this from its own state instead.
    pub fn stale(mut self, stale: bool) -> Self {
        self.embedded_stale = stale;
        self
    }
}

impl Slotted for Sparkline {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for Sparkline {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        if self.embedded {
            let points = match &self.state {
                SparklineState::Ready(reading) | SparklineState::Stale { reading, .. } => {
                    bounded(reading)
                }
                _ => Vec::new(),
            };
            let stale = self.embedded_stale || matches!(self.state, SparklineState::Stale { .. });
            return div()
                .w_full()
                .h_full()
                .child(plot(&theme, points, self.tint, stale))
                .into_any_element();
        }
        let (body, spec): (AnyElement, NodeSpec) = match &self.state {
            SparklineState::Loading => (
                self.slots.or_else(slot::LOADING, window, cx, |_, cx| {
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w_full()
                        .p(px(24.0))
                        .child(
                            PulseLoader::new(self.ident.child("loading"))
                                .label(cx.strings().text(StringKey::Loading)),
                        )
                        .into_any_element()
                }),
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.label.clone())
                    .value("loading")
                    .busy(true)
                    .read_only(true),
            ),
            SparklineState::Ready(reading) => (
                reading_body(&self.ident, &self.label, reading, None, self.tint, cx),
                reading_spec(&self.ident, &self.label, reading, cx),
            ),
            SparklineState::Stale { reading, reason } => (
                reading_body(
                    &self.ident,
                    &self.label,
                    reading,
                    Some(reason.clone()),
                    self.tint,
                    cx,
                ),
                reading_spec(&self.ident, &self.label, reading, cx),
            ),
            SparklineState::Empty => (
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        self.ident.child("empty"),
                        cx.strings().text(StringKey::SparklineEmpty),
                    )
                    .kind(EmptyKind::Empty)
                    .into_any_element()
                }),
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.label.clone())
                    .value("empty")
                    .read_only(true),
            ),
            SparklineState::Unavailable(reason) => (
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        self.ident.child("unavailable"),
                        cx.strings().text(StringKey::SparklineUnavailable),
                    )
                    .kind(EmptyKind::Unavailable)
                    .detail(reason.clone())
                    .into_any_element()
                }),
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.label.clone())
                    .value("unavailable")
                    .read_only(true),
            ),
            SparklineState::Error(reason) => (
                self.slots.or_else(slot::FAILED, window, cx, |_, cx| {
                    EmptyState::new(
                        self.ident.child("error"),
                        cx.strings().text(StringKey::SparklineError),
                    )
                    .kind(EmptyKind::Failed)
                    .detail(reason.clone())
                    .into_any_element()
                }),
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.label.clone())
                    .value("error")
                    .description(reason.clone())
                    .invalid(true)
                    .read_only(true),
            ),
        };
        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .p_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Raised, Elevation::Raised)
            .child(body)
            .semantic_in(cx, spec)
            .into_any_element()
    }
}

/// The supplied points that are inside the documented normalized square.
fn bounded(reading: &SparklineReading) -> Vec<SparklinePoint> {
    reading
        .points
        .iter()
        .copied()
        .filter(|point| point.is_bounded())
        .collect()
}

fn reading_spec(
    ident: &Ident,
    label: &SharedString,
    reading: &SparklineReading,
    cx: &App,
) -> NodeSpec {
    let range = cx.strings().format(
        StringKey::SparklineRange,
        &[reading.minimum.as_ref(), reading.maximum.as_ref()],
    );
    NodeSpec::new(ident.semantic_id(), Role::Image)
        .text(label.clone())
        .value(reading.current.clone())
        .description(range)
        .read_only(true)
}

fn reading_body(
    ident: &Ident,
    label: &SharedString,
    reading: &SparklineReading,
    stale: Option<SharedString>,
    tint: Option<Hsla>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme().clone();
    let current = cx
        .strings()
        .format(StringKey::SparklineCurrent, &[reading.current.as_ref()]);
    let minimum = cx
        .strings()
        .format(StringKey::SparklineMinimum, &[reading.minimum.as_ref()]);
    let maximum = cx
        .strings()
        .format(StringKey::SparklineMaximum, &[reading.maximum.as_ref()]);
    let points = bounded(reading);
    let is_stale = stale.is_some();

    div()
        .column()
        .w_full()
        .gap_token(&theme, Space::Xs)
        .child(
            div()
                .row()
                .items_baseline()
                .justify_between()
                .gap_token(&theme, Space::Sm)
                .child(
                    div()
                        .type_scale(&theme, TypeScale::Label)
                        .text_color(theme.colors.text)
                        .child(label.clone()),
                )
                .child(
                    div()
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_muted)
                        .child(current),
                ),
        )
        .children(stale.map(|reason| {
            div()
                .row()
                .items_center()
                .gap_token(&theme, Space::Xs)
                .type_scale(&theme, TypeScale::Caption)
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
        // The plot and the two extremes it reached, side by side. Read against
        // the top and the bottom of the box they describe, a maximum and a
        // minimum are a scale; stranded on a line of their own underneath,
        // they are two more numbers.
        .child(
            div()
                .row()
                .items_stretch()
                .w_full()
                .gap_token(&theme, Space::Sm)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h(px(PLOT_HEIGHT))
                        .child(plot(&theme, points, tint, is_stale)),
                )
                .child(
                    div()
                        .column()
                        .flex_none()
                        .justify_between()
                        .h(px(PLOT_HEIGHT))
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_faint)
                        .child(maximum)
                        .child(minimum),
                ),
        )
        .into_any_element()
}

/// How tall a framed plot is.
const PLOT_HEIGHT: f32 = 72.0;

/// The band across the middle of the plot, as a fraction of its height.
///
/// A line with nothing behind it cannot be read as high or low, only as a
/// shape. One rule, at the middle of the box the caller normalized into, is
/// the least a reader needs to place it — and is the caller's own midpoint
/// rather than an axis this component invented.
const MIDLINE: f32 = 0.5;

/// The mark itself: a ground, a midline, the filled area, the line and the
/// point the reading is currently at.
fn plot(
    theme: &gpui_kit_theme::Theme,
    points: Vec<SparklinePoint>,
    tint: Option<Hsla>,
    stale: bool,
) -> impl IntoElement {
    // A trend is data, so it is neutral until a caller says whose it is; a
    // reading that is no longer verified draws quieter than one that is, which
    // is the difference the mark itself was missing.
    let line = match (tint, stale) {
        (Some(tint), false) => tint,
        (Some(tint), true) => tint.opacity(0.45),
        (None, false) => theme.colors.text,
        (None, true) => theme.colors.text_faint,
    };
    let stroke = theme.borders.thick;
    div()
        .relative()
        .w_full()
        .h_full()
        .overflow_hidden()
        .radius(theme, Radius::Small)
        .surface(theme, Surface::Sunken)
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(relative(MIDLINE))
                .h(px(theme.borders.hairline))
                .bg(theme.colors.divider),
        )
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    paint_series(bounds, window, &points, line, stroke, stale);
                },
            )
            .absolute()
            .inset_0(),
        )
}

/// The filled area, the line over it, and the mark on the last sample.
fn paint_series(
    bounds: gpui::Bounds<gpui::Pixels>,
    window: &mut Window,
    points: &[SparklinePoint],
    color: Hsla,
    stroke: f32,
    stale: bool,
) {
    if points.len() < 2 {
        return;
    }
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
    let floor = bounds.origin.y + px(inset + height);

    // The area first, so the line sits on top of its own fill rather than
    // being half covered by it.
    let mut area = PathBuilder::fill();
    area.move_to(point(at(points[0]).x, floor));
    for sample in points.iter().copied() {
        area.line_to(at(sample));
    }
    area.line_to(point(at(points[points.len() - 1]).x, floor));
    area.close();
    if let Ok(path) = area.build() {
        window.paint_path(path, color.opacity(if stale { 0.06 } else { 0.14 }));
    }

    let mut line = PathBuilder::stroke(px(stroke));
    line.move_to(at(points[0]));
    for sample in points.iter().copied().skip(1) {
        line.line_to(at(sample));
    }
    if let Ok(path) = line.build() {
        window.paint_path(path, color);
    }

    // Where the reading is now. A verified sample is a solid mark and an
    // unverified one is a ring, so the state is legible in the plot and not
    // only in the sentence beside it.
    let last = at(points[points.len() - 1]);
    let radius = stroke * 1.8;
    let mut mark = if stale {
        PathBuilder::stroke(px(stroke))
    } else {
        PathBuilder::fill()
    };
    let sides = 12;
    for step in 0..=sides {
        let angle = std::f32::consts::TAU * (step as f32) / (sides as f32);
        let vertex = point(
            last.x + px(radius * angle.cos()),
            last.y + px(radius * angle.sin()),
        );
        if step == 0 {
            mark.move_to(vertex);
        } else {
            mark.line_to(vertex);
        }
    }
    mark.close();
    if let Ok(path) = mark.build() {
        window.paint_path(path, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_outside_the_documented_square_are_not_published() {
        let reading = SparklineReading::new(
            [
                SparklinePoint::new(0.0, 0.2),
                SparklinePoint::new(0.5, 1.2),
                SparklinePoint::new(1.0, 0.8),
            ],
            "8 req/s",
            "2 req/s",
            "9 req/s",
        );
        assert_eq!(reading.published_points(), 2);
    }

    #[test]
    fn non_finite_points_are_not_bounded() {
        assert!(!SparklinePoint::new(f32::NAN, 0.5).is_bounded());
        assert!(!SparklinePoint::new(0.5, f32::INFINITY).is_bounded());
    }
}

#[cfg(test)]
mod sparkline_phase_tests {
    use super::*;

    #[test]
    fn stale_projects_as_error_and_keeps_the_verified_reading() {
        let state = SparklineState::Stale {
            reading: SparklineReading {
                points: Vec::new(),
                current: SharedString::from("0"),
                minimum: SharedString::from("0"),
                maximum: SharedString::from("0"),
            },
            reason: "offline".into(),
        };
        assert_eq!(state.phase(), Phase::Error);
        assert!(state.is_stale());
        assert_eq!(state.reason(), Some("offline"));
    }
}
