//! A caller-owned KPI reading.
//!
//! The value and the delta are finished host strings. An optional sparkline
//! is already normalized. A refresh failure keeps the last verified reading.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, TypeScale};

use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::loading::PulseLoader;
use crate::display::sparkline::{Sparkline, SparklinePoint, SparklineReading, SparklineState};
use crate::display::status::StatusDot;
use crate::foundation::{Ident, StyledExt};
use crate::strings::{ActiveStrings, StringKey};

/// One verified KPI the host already formatted.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricReading {
    pub value: SharedString,
    pub delta: Option<SharedString>,
    pub tone: Tone,
    pub trend: Vec<SparklinePoint>,
}

impl MetricReading {
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            delta: None,
            tone: Tone::Neutral,
            trend: Vec::new(),
        }
    }

    pub fn delta(mut self, delta: impl Into<SharedString>, tone: Tone) -> Self {
        self.delta = Some(delta.into());
        self.tone = tone;
        self
    }

    pub fn trend(mut self, points: impl IntoIterator<Item = SparklinePoint>) -> Self {
        self.trend = points.into_iter().collect();
        self
    }
}

/// How the reading was asked for.
#[derive(Debug, Clone, PartialEq)]
pub enum MetricState {
    Loading,
    Ready(MetricReading),
    Empty,
    Unavailable(SharedString),
    Error(SharedString),
    Stale {
        reading: MetricReading,
        reason: SharedString,
    },
}

impl MetricState {
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

/// A compact KPI card.
#[derive(IntoElement)]
pub struct MetricCard {
    ident: Ident,
    label: SharedString,
    state: MetricState,
}

impl MetricCard {
    pub fn new(
        ident: impl Into<Ident>,
        label: impl Into<SharedString>,
        state: MetricState,
    ) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            state,
        }
    }
}

impl RenderOnce for MetricCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (inner, spec): (AnyElement, NodeSpec) = match &self.state {
            MetricState::Loading => (
                PulseLoader::new(self.ident.child("loading"))
                    .label(cx.strings().text(StringKey::Loading))
                    .into_any_element(),
                spec_for(&self.ident, &self.label, &self.state),
            ),
            MetricState::Empty => (
                EmptyState::new(
                    self.ident.child("empty"),
                    cx.strings().text(StringKey::MetricEmpty),
                )
                .kind(EmptyKind::Empty)
                .into_any_element(),
                spec_for(&self.ident, &self.label, &self.state),
            ),
            MetricState::Unavailable(reason) => (
                EmptyState::new(
                    self.ident.child("unavailable"),
                    cx.strings().text(StringKey::MetricUnavailable),
                )
                .kind(EmptyKind::Unavailable)
                .detail(reason.clone())
                .into_any_element(),
                spec_for(&self.ident, &self.label, &self.state),
            ),
            MetricState::Error(reason) => (
                EmptyState::new(
                    self.ident.child("error"),
                    cx.strings().text(StringKey::MetricError),
                )
                .kind(EmptyKind::Failed)
                .detail(reason.clone())
                .into_any_element(),
                spec_for(&self.ident, &self.label, &self.state),
            ),
            MetricState::Ready(reading) | MetricState::Stale { reading, .. } => {
                let stale = match &self.state {
                    MetricState::Stale { reason, .. } => Some(reason.clone()),
                    _ => None,
                };
                (
                    ready_metric(&self.ident, &self.label, reading, stale, &theme, cx),
                    spec_for(&self.ident, &self.label, &self.state),
                )
            }
        };
        div()
            .w_full()
            .min_w(px(180.0))
            .child(inner)
            .semantic_in(cx, spec)
    }
}

fn spec_for(ident: &Ident, label: &SharedString, state: &MetricState) -> NodeSpec {
    NodeSpec::new(ident.semantic_id(), Role::Status)
        .text(label.clone())
        .value(state.name())
}

fn ready_metric(
    ident: &Ident,
    label: &SharedString,
    reading: &MetricReading,
    stale: Option<SharedString>,
    theme: &gpui_kit_theme::Theme,
    _cx: &mut App,
) -> AnyElement {
    let trend = (!reading.trend.is_empty()).then(|| {
        Sparkline::new(
            ident.child("trend"),
            label.clone(),
            SparklineState::Ready(SparklineReading::new(
                reading.trend.clone(),
                reading.value.clone(),
                reading.value.clone(),
                reading.value.clone(),
            )),
        )
        .into_any_element()
    });
    div()
        .column()
        .gap_token(theme, Space::Xs)
        .p_token(theme, Space::Md)
        .radius(theme, Radius::Card)
        .surface(theme, Surface::Panel)
        .child(
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_muted)
                .child(label.clone()),
        )
        .child(
            div()
                .type_scale(theme, TypeScale::Title)
                .child(reading.value.clone()),
        )
        .children(reading.delta.as_ref().map(|delta| {
            div()
                .row()
                .items_center()
                .gap_token(theme, Space::Xs)
                .child(StatusDot::new(reading.tone))
                .child(
                    div()
                        .type_scale(theme, TypeScale::Caption)
                        .text_color(theme.colors.text_muted)
                        .child(delta.clone()),
                )
        }))
        .children(stale.map(|reason| {
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.warning)
                .child(reason)
        }))
        .children(trend)
        .into_any_element()
}
