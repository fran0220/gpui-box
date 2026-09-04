//! A caller-owned KPI reading.
//!
//! The value and the delta are finished host strings. An optional sparkline
//! is already normalized. A refresh failure keeps the last verified reading.

use gpui::{
    AnyElement, App, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, px,
};
use gpui_kit_assets::Icon as Glyph;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, ColorChoice, Radius, SemanticColor, Space, TextTone, Theme, TypeScale, Variant,
    VariantColors,
};

use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::icon::paint as paint_glyph;
use crate::display::loading::{Skeleton, SkeletonShape};
use crate::display::sparkline::{Sparkline, SparklinePoint, SparklineReading, SparklineState};
use crate::display::status::StatusDot;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{CardVariant, Ident, StyledExt};
use crate::motion;
use crate::overlay::tooltip::Tooltipped;
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

/// The least tall the region under the caption is, whatever is in it.
///
/// A card that loses its shape when the reading does not arrive reports the
/// absence as a different component; every state of one metric is the same
/// object in the same place, so the floor is the card's and not the
/// reading's. It is a floor rather than a fixed height: a state that needs
/// more room takes it, and a row of cards still lines up because they all
/// stand on the same one.
const BODY_HEIGHT: f32 = 76.0;

/// The edge of the arrow beside a delta, in pixels.
///
/// Smaller than any control icon step: it sits inside caption text and is
/// read as part of the number, not as a glyph of its own.
const DELTA_ARROW: f32 = 11.0;

/// The height the trend plot and the placeholder standing in for it share.
const TREND_HEIGHT: f32 = 40.0;

/// Which way a delta points.
///
/// This is direction, never judgement. Whether moving that way is good is the
/// caller's [`Tone`], because only the caller knows the polarity of its own
/// metric — a rising error rate and a rising revenue both point up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DeltaDirection {
    Up,
    Down,
    #[default]
    Flat,
}

impl DeltaDirection {
    /// The direction the host's own delta text already states.
    ///
    /// Only the leading sign is read, and only the characters a formatter
    /// writes for one: nothing is inferred from the magnitude, and text
    /// carrying no sign points nowhere rather than being guessed at.
    pub fn from_text(delta: &str) -> Self {
        match delta.trim_start().chars().next() {
            Some('+') => Self::Up,
            // ASCII hyphen-minus and the typographic minus a locale-aware
            // formatter writes instead.
            Some('-' | '\u{2212}') => Self::Down,
            _ => Self::Flat,
        }
    }

    fn glyph(self) -> Option<Glyph> {
        match self {
            Self::Up => Some(Glyph::ArrowUp),
            Self::Down => Some(Glyph::ArrowDown),
            Self::Flat => None,
        }
    }
}

/// One verified KPI the host already formatted.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricReading {
    pub value: SharedString,
    pub delta: Option<SharedString>,
    pub tone: Tone,
    pub direction: Option<DeltaDirection>,
    pub trend: Vec<SparklinePoint>,
}

impl MetricReading {
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            delta: None,
            tone: Tone::Neutral,
            direction: None,
            trend: Vec::new(),
        }
    }

    pub fn delta(mut self, delta: impl Into<SharedString>, tone: Tone) -> Self {
        self.delta = Some(delta.into());
        self.tone = tone;
        self
    }

    /// The way the delta points, for text whose sign the card cannot read —
    /// a word, a ratio, a locale that writes the sign last.
    pub fn direction(mut self, direction: DeltaDirection) -> Self {
        self.direction = Some(direction);
        self
    }

    /// The direction shown: the caller's when it stated one, otherwise the
    /// one its own delta text already states.
    pub fn resolved_direction(&self) -> DeltaDirection {
        self.direction.unwrap_or_else(|| {
            self.delta
                .as_ref()
                .map(|delta| DeltaDirection::from_text(delta))
                .unwrap_or_default()
        })
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

impl HasPhase for MetricState {
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

/// A compact KPI card.
#[derive(IntoElement)]
pub struct MetricCard {
    ident: Ident,
    label: SharedString,
    state: MetricState,
    tint: Option<Hsla>,
    slots: Slots,
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
            tint: None,
            slots: Slots::default(),
        }
    }

    /// The colour this metric is known by, which its trend is drawn in.
    ///
    /// Without one the trend takes the theme accent. A caller that has
    /// already spent a colour on this series elsewhere — a legend, a chart
    /// the card sits beside — passes it here so the two agree.
    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }
}

impl Slotted for MetricCard {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for MetricCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let spec = spec_for(&self.ident, &self.label, &self.state);
        let body: AnyElement = match &self.state {
            // The placeholder takes the shape of the reading it is standing
            // in for — a value, a delta, a trend — so the card that arrives
            // lands where the wait was rather than replacing it.
            MetricState::Loading => self.slots.or_else(slot::LOADING, window, cx, |_, cx| {
                Skeleton::new(self.ident.child("loading"))
                    .label(cx.strings().text(StringKey::Loading))
                    .shapes([
                        SkeletonShape::Rect {
                            width: 0.42,
                            height: theme.typography.title.line_height,
                        },
                        SkeletonShape::Rect {
                            width: 0.24,
                            height: theme.typography.caption.line_height,
                        },
                        SkeletonShape::Rect {
                            width: 1.0,
                            height: TREND_HEIGHT,
                        },
                    ])
                    .into_any_element()
            }),
            MetricState::Empty => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                marked_empty(
                    self.ident.child("empty"),
                    cx.strings().text(StringKey::MetricEmpty),
                    EmptyKind::Empty,
                    None,
                )
            }),
            MetricState::Unavailable(reason) => {
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    marked_empty(
                        self.ident.child("unavailable"),
                        cx.strings().text(StringKey::MetricUnavailable),
                        EmptyKind::Unavailable,
                        Some(reason.clone()),
                    )
                })
            }
            MetricState::Error(reason) => self.slots.or_else(slot::FAILED, window, cx, |_, cx| {
                marked_empty(
                    self.ident.child("error"),
                    cx.strings().text(StringKey::MetricError),
                    EmptyKind::Failed,
                    Some(reason.clone()),
                )
            }),
            MetricState::Ready(reading) | MetricState::Stale { reading, .. } => {
                let stale = match &self.state {
                    MetricState::Stale { reason, .. } => Some(reason.clone()),
                    _ => None,
                };
                reading_body(
                    &self.ident,
                    &self.label,
                    reading,
                    stale,
                    self.tint.unwrap_or(theme.colors.accent),
                    &theme,
                )
            }
        };

        // Real content replacing a stand-in fades up in the box the stand-in
        // already reserved, which is the one thing the caption above promised
        // and nothing delivered: the card used to cut from a skeleton to a
        // reading between two frames.
        //
        // The key is what makes the swap the animation and not the value. A
        // reading and a stale reading share it, so a refresh that failed keeps
        // the last verified number sitting exactly where it was instead of
        // fading it back in and implying it is new.
        let arrival = self.ident.child("body").child(match &self.state {
            MetricState::Ready(_) | MetricState::Stale { .. } => "reading",
            standing_in => standing_in.name(),
        });

        div()
            .column()
            .w_full()
            .min_w(px(180.0))
            .gap_token(&theme, Space::Sm)
            .p_token(&theme, Space::Md)
            .card_surface(&theme, CardVariant::Filled)
            .child(
                div()
                    .type_scale(&theme, TypeScale::Caption)
                    .text_tone(&theme, TextTone::Muted)
                    .child(self.label.clone()),
            )
            .child(motion::surface_in(
                arrival.element_id(),
                &theme,
                div()
                    .column()
                    .justify_center()
                    .w_full()
                    .min_h(px(BODY_HEIGHT))
                    .child(body),
            ))
            .semantic_in(cx, spec)
    }
}

fn spec_for(ident: &Ident, label: &SharedString, state: &MetricState) -> NodeSpec {
    let mut spec = NodeSpec::new(ident.semantic_id(), Role::Status)
        .text(label.clone())
        .value(state.name());
    if let Some(reason) = state.reason() {
        spec = spec.description(reason);
    }
    spec
}

fn marked_empty(
    ident: Ident,
    label: SharedString,
    kind: EmptyKind,
    detail: Option<SharedString>,
) -> AnyElement {
    let mut empty = EmptyState::new(ident.clone(), SharedString::default()).kind(kind);
    if let Some(detail) = detail {
        empty = empty.detail(detail);
    }
    let mark_ident = ident.child("mark");
    div()
        .id(mark_ident.element_id())
        .child(empty)
        .tip(mark_ident, label)
        .into_any_element()
}

/// The reading itself: the value, what it moved by, and the shape it moved in.
///
/// The trend is drawn embedded, which is what keeps the card one card. A
/// sparkline that brought its own frame in here drew a second panel inside
/// the first and repeated the card's own value back at it as a minimum and a
/// maximum it had never been given.
fn reading_body(
    ident: &Ident,
    label: &SharedString,
    reading: &MetricReading,
    stale: Option<SharedString>,
    tint: Hsla,
    theme: &Theme,
) -> AnyElement {
    let trend = (!reading.trend.is_empty()).then(|| {
        div().h(px(TREND_HEIGHT)).w_full().child(
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
            .tint(tint)
            .embedded()
            .stale(stale.is_some()),
        )
    });
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
                        .type_scale(theme, TypeScale::Title)
                        .text_tone(theme, TextTone::Primary)
                        .child(reading.value.clone()),
                )
                .children(reading.delta.as_ref().map(|delta| {
                    delta_pill(delta, reading.tone, reading.resolved_direction(), theme)
                })),
        )
        .children(trend)
        .children(stale.map(|reason| {
            div()
                .row()
                .items_center()
                .gap_token(theme, Space::Xs)
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.warning)
                .child(StatusDot::new(Tone::Warning))
                .child(reason)
        }))
        .into_any_element()
}

/// The wash and the readable text one delta tier resolves to.
fn delta_colors(tone: Tone, theme: &Theme) -> VariantColors {
    let role = match tone {
        Tone::Success => SemanticColor::Success,
        Tone::Warning => SemanticColor::Warning,
        Tone::Danger => SemanticColor::Danger,
        Tone::Info => SemanticColor::Info,
        Tone::Accent => SemanticColor::Accent,
        // A neutral delta is not claiming anything, so it takes the neutral
        // track rather than a colour a reader would look for a meaning in.
        Tone::Neutral => {
            return VariantColors {
                background: theme.colors.track,
                background_hover: theme.colors.track,
                background_active: theme.colors.track,
                text: theme.colors.text_muted,
            };
        }
    };
    theme.variant_colors(Variant::Light, &ColorChoice::Semantic(role))
}

/// The delta as a chip: the way it moved, then how much.
fn delta_pill(
    delta: &SharedString,
    tone: Tone,
    direction: DeltaDirection,
    theme: &Theme,
) -> AnyElement {
    let colors = delta_colors(tone, theme);
    div()
        .row()
        .items_center()
        .flex_none()
        .gap(px(theme.space(Space::Xxs)))
        .px_token(theme, Space::Xs)
        .py(px(theme.space(Space::Xxs) / 2.0))
        .radius(theme, Radius::Pill)
        .bg(colors.background)
        .children(
            direction
                .glyph()
                .map(|glyph| paint_glyph(glyph, DELTA_ARROW, colors.text, false)),
        )
        .child(
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(colors.text)
                .child(delta.clone()),
        )
        .into_any_element()
}

#[cfg(test)]
mod metric_phase_tests {
    use super::*;

    #[test]
    fn a_delta_points_the_way_its_own_text_says_unless_the_caller_says_otherwise() {
        let inferred = MetricReading::new("12.4k").delta("-3%", Tone::Danger);
        assert_eq!(inferred.resolved_direction(), DeltaDirection::Down);

        // Text carrying no sign points nowhere rather than being guessed at,
        // and the caller can still state the direction itself.
        let unsigned = MetricReading::new("12.4k").delta("steady", Tone::Neutral);
        assert_eq!(unsigned.resolved_direction(), DeltaDirection::Flat);
        assert_eq!(
            unsigned.direction(DeltaDirection::Up).resolved_direction(),
            DeltaDirection::Up
        );
    }

    #[test]
    fn stale_projects_as_error_and_keeps_the_verified_reading() {
        let state = MetricState::Stale {
            reading: MetricReading::new("12"),
            reason: "offline".into(),
        };
        assert_eq!(state.phase(), Phase::Error);
        assert!(state.is_stale());
        assert_eq!(state.reason(), Some("offline"));
    }
}
