//! A deliberately narrow trend reading, not a charting framework.
//!
//! A sparkline has no axes, ticks, legend, tooltip, locale or scale policy.
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
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, PathBuilder, RenderOnce,
    SharedString, Styled, Window, canvas, div, point, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, TypeScale};

use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::status::StatusDot;
use crate::foundation::{Ident, StyledExt};
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
    Ready(SparklineReading),
    Empty,
    Unavailable(SharedString),
    /// The reading is the last verified value; the text says why it is stale.
    Stale {
        reading: SparklineReading,
        reason: SharedString,
    },
}

impl SparklineState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ready(_) => "ready",
            Self::Empty => "empty",
            Self::Unavailable(_) => "unavailable",
            Self::Stale { .. } => "stale",
        }
    }
}

/// A compact, accessible trend reading.
#[derive(Debug, IntoElement)]
pub struct Sparkline {
    ident: Ident,
    label: SharedString,
    state: SparklineState,
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
        }
    }
}

impl RenderOnce for Sparkline {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, spec): (AnyElement, NodeSpec) = match &self.state {
            SparklineState::Ready(reading) => (
                reading_body(&self.ident, &self.label, reading, None, cx),
                reading_spec(&self.ident, &self.label, reading, cx),
            ),
            SparklineState::Stale { reading, reason } => (
                reading_body(&self.ident, &self.label, reading, Some(reason.clone()), cx),
                reading_spec(&self.ident, &self.label, reading, cx),
            ),
            SparklineState::Empty => (
                EmptyState::new(
                    self.ident.child("empty"),
                    cx.strings().text(StringKey::SparklineEmpty),
                )
                .kind(EmptyKind::Empty)
                .into_any_element(),
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.label.clone())
                    .value("empty")
                    .read_only(true),
            ),
            SparklineState::Unavailable(reason) => (
                EmptyState::new(
                    self.ident.child("unavailable"),
                    cx.strings().text(StringKey::SparklineUnavailable),
                )
                .kind(EmptyKind::Unavailable)
                .detail(reason.clone())
                .into_any_element(),
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.label.clone())
                    .value("unavailable")
                    .read_only(true),
            ),
        };
        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .p_token(&theme, Space::Sm)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Raised, Elevation::Raised)
            .child(body)
            .semantic_in(cx, spec)
    }
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
    let points: Vec<SparklinePoint> = reading
        .points
        .iter()
        .copied()
        .filter(|point| point.is_bounded())
        .collect();
    let stroke = theme.borders.thick;
    let color = theme.colors.accent;

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
        .child(spark_canvas(points, stroke, color))
        .child(
            div()
                .row()
                .justify_between()
                .type_scale(&theme, TypeScale::Caption)
                .text_color(theme.colors.text_faint)
                .child(minimum)
                .child(maximum),
        )
        .into_any_element()
}

fn spark_canvas(points: Vec<SparklinePoint>, stroke: f32, color: gpui::Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
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
            let mut builder = PathBuilder::stroke(px(stroke));
            builder.move_to(at(points[0]));
            for sample in points.iter().copied().skip(1) {
                builder.line_to(at(sample));
            }
            if let Ok(path) = builder.build() {
                window.paint_path(path, color);
            }
        },
    )
    .w_full()
    .h(px(72.0))
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
