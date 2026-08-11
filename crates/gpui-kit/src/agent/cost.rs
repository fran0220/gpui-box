//! What a run has cost, and how much of a context window it has used.
//!
//! # An unlabelled estimate cannot be built
//!
//! [`Quantity`] has no constructor that takes a number on its own. Every one
//! of them names the [`Basis`] in the same call —
//! [`Quantity::measured`] or [`Quantity::estimated`] — and the wording is
//! derived from that basis, so a number reaches a screen with the fact that it
//! was estimated attached or it does not reach a screen at all. The label is
//! in the drawn text, in the mark beside it, and in the node's published
//! value, because a reader who saw the number on one surface and not the other
//! would draw the wrong conclusion on the second one.
//!
//! # Unavailable is not zero
//!
//! [`Reading::Unavailable`] is a state, not a quantity. Nothing about it is
//! drawn as a number, no proportion is computed from it, and the node
//! publishes the refusal rather than a value.
//!
//! # An unknown limit draws no proportion
//!
//! [`Limit::Unknown`] means nobody stated a ceiling. A proportion of an
//! unknown total is invented, so [`ContextGauge`] draws no fill and publishes
//! no range, exactly as [`ProgressBar`](crate::display::progress::ProgressBar) refuses
//! to claim a position for work whose extent is unknown. It does not fall back
//! to the indeterminate sweep either: that sweep means "in flight", and a
//! reading of what has been used so far is not in flight.
//!
//! # Numbers are the caller's
//!
//! Currency, token counts, grouping and the position of a unit are locale
//! work, which this crate does not do — the same reason
//! [`Timeline`](crate::display::timeline::Timeline) takes times as finished strings. A
//! [`Quantity`] therefore carries both the caller's already-formatted wording
//! *and*, separately, the bare number, which is used for one thing only:
//! working out the proportion of a known limit. Nothing here ever turns a
//! number into text.

use gpui::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, TextTone, TypeScale};

use crate::display::badge::{Badge, Tone};
use crate::foundation::{Ident, StyledExt, text};
use crate::strings::{ActiveStrings, StringKey};

/// How a number was arrived at.
///
/// There is no third variant meaning "unspecified": a caller that cannot say
/// which of these two it has does not have a number, it has
/// [`Reading::Unavailable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// Counted by whoever owns the meter.
    Measured,
    /// Worked out from something else, and said so everywhere it appears.
    Estimated,
}

impl Basis {
    /// The stable name published in the semantic tree.
    pub fn name(self) -> &'static str {
        match self {
            Self::Measured => "measured",
            Self::Estimated => "estimated",
        }
    }

    pub fn is_estimate(self) -> bool {
        matches!(self, Self::Estimated)
    }
}

/// A number, the caller's wording for it, and how it was arrived at.
#[derive(Debug, Clone, PartialEq)]
pub struct Quantity {
    basis: Basis,
    amount: f64,
    text: SharedString,
}

impl Quantity {
    /// A counted number. `text` is the caller's own formatting of it.
    pub fn measured(amount: f64, text: impl Into<SharedString>) -> Self {
        Self {
            basis: Basis::Measured,
            amount,
            text: text.into(),
        }
    }

    /// An estimate. Every rendering of it says so.
    pub fn estimated(amount: f64, text: impl Into<SharedString>) -> Self {
        Self {
            basis: Basis::Estimated,
            amount,
            text: text.into(),
        }
    }

    pub fn basis(&self) -> Basis {
        self.basis
    }

    /// The bare number, used only for proportions against a known limit.
    pub fn amount(&self) -> f64 {
        self.amount
    }

    /// The caller's wording, without the estimate label.
    pub fn text(&self) -> &SharedString {
        &self.text
    }

    /// What a reader reads and what the node publishes: the caller's wording,
    /// carrying its basis when the basis is one a reader must know about.
    pub fn display(&self, cx: &App) -> SharedString {
        cx.strings().format(
            match self.basis {
                Basis::Measured => StringKey::CostMeasured,
                Basis::Estimated => StringKey::CostEstimated,
            },
            &[&self.text],
        )
    }
}

/// What is known about a number right now.
#[derive(Debug, Clone, PartialEq)]
pub enum Reading {
    /// A number, with its basis.
    Known(Quantity),
    /// Nobody could say. Never drawn as zero, and never counted into a
    /// proportion.
    Unavailable { reason: Option<SharedString> },
}

impl Reading {
    pub fn measured(amount: f64, text: impl Into<SharedString>) -> Self {
        Self::Known(Quantity::measured(amount, text))
    }

    pub fn estimated(amount: f64, text: impl Into<SharedString>) -> Self {
        Self::Known(Quantity::estimated(amount, text))
    }

    pub fn unavailable() -> Self {
        Self::Unavailable { reason: None }
    }

    /// Unavailable, in the host's own words, which are shown verbatim.
    pub fn unavailable_because(reason: impl Into<SharedString>) -> Self {
        Self::Unavailable {
            reason: Some(reason.into()),
        }
    }

    pub fn quantity(&self) -> Option<&Quantity> {
        match self {
            Self::Known(quantity) => Some(quantity),
            Self::Unavailable { .. } => None,
        }
    }

    /// The stable name published in the semantic tree.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Known(quantity) => quantity.basis().name(),
            Self::Unavailable { .. } => "unavailable",
        }
    }

    fn display(&self, cx: &App) -> SharedString {
        match self {
            Self::Known(quantity) => quantity.display(cx),
            Self::Unavailable { reason } => reason
                .clone()
                .unwrap_or_else(|| cx.strings().text(StringKey::CostUnavailable)),
        }
    }

    fn is_estimate(&self) -> bool {
        self.quantity()
            .is_some_and(|quantity| quantity.basis().is_estimate())
    }
}

/// The ceiling a gauge measures against.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Limit {
    /// A stated ceiling, which may itself be an estimate.
    Known(Quantity),
    /// Nobody stated one. No proportion is drawn.
    #[default]
    Unknown,
}

impl Limit {
    pub fn measured(amount: f64, text: impl Into<SharedString>) -> Self {
        Self::Known(Quantity::measured(amount, text))
    }

    pub fn estimated(amount: f64, text: impl Into<SharedString>) -> Self {
        Self::Known(Quantity::estimated(amount, text))
    }

    pub fn quantity(&self) -> Option<&Quantity> {
        match self {
            Self::Known(quantity) => Some(quantity),
            Self::Unknown => None,
        }
    }

    pub fn is_known(&self) -> bool {
        matches!(self, Self::Known(_))
    }
}

/// When a reading was last verified, for a reading that is no longer current.
///
/// Marking a value stale and saying when it was from is one call, because a
/// value marked stale without a date is a warning nobody can act on. The
/// wording of the date is the caller's, as in
/// [`Timeline`](crate::display::timeline::Timeline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastVerified(SharedString);

impl LastVerified {
    pub fn at(when: impl Into<SharedString>) -> Self {
        Self(when.into())
    }

    fn sentence(&self, cx: &App) -> SharedString {
        cx.strings().format(StringKey::CostLastVerified, &[&self.0])
    }
}

/// One line of a [`CostMeter`].
#[derive(Debug, Clone, PartialEq)]
pub struct CostLine {
    id: SharedString,
    label: SharedString,
    reading: Reading,
    stale: Option<LastVerified>,
}

impl CostLine {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        reading: Reading,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            reading,
            stale: None,
        }
    }

    /// The refresh that should have replaced this reading did not. The value
    /// stays on screen and says when it was from.
    pub fn stale(mut self, verified: LastVerified) -> Self {
        self.stale = Some(verified);
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn reading(&self) -> &Reading {
        &self.reading
    }
}

/// What a run has cost so far, line by line.
#[derive(Debug, Clone, IntoElement)]
pub struct CostMeter {
    ident: Ident,
    label: Option<SharedString>,
    lines: Vec<CostLine>,
}

impl CostMeter {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            lines: Vec::new(),
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn line(mut self, line: CostLine) -> Self {
        self.lines.push(line);
        self
    }

    pub fn lines(mut self, lines: impl IntoIterator<Item = CostLine>) -> Self {
        self.lines.extend(lines);
        self
    }
}

impl RenderOnce for CostMeter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rows: Vec<_> = self
            .lines
            .iter()
            .map(|line| {
                let ident = self.ident.child(line.id.as_ref());
                let display = line.reading.display(cx);
                let unavailable = line.reading.quantity().is_none();

                div()
                    .column()
                    .w_full()
                    .gap_token(&theme, Space::Xs)
                    .child(
                        div()
                            .row()
                            .w_full()
                            .justify_between()
                            .gap_token(&theme, Space::Sm)
                            .child(
                                text(&theme, TypeScale::Label, line.label.clone())
                                    .text_tone(&theme, TextTone::Muted),
                            )
                            .child(
                                div()
                                    .row()
                                    .gap_token(&theme, Space::Sm)
                                    .child(
                                        text(&theme, TypeScale::Label, display.clone()).text_tone(
                                            &theme,
                                            if unavailable {
                                                TextTone::Faint
                                            } else {
                                                TextTone::Primary
                                            },
                                        ),
                                    )
                                    .when(line.reading.is_estimate(), |element| {
                                        element.child(estimate_mark(&ident, cx))
                                    }),
                            ),
                    )
                    .children(
                        line.stale
                            .as_ref()
                            .map(|verified| stale_line(&ident, &theme, verified.sentence(cx), cx)),
                    )
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.semantic_id(), Role::Status)
                            .text(line.label.clone())
                            .value(display)
                            .parent(self.ident.semantic_id()),
                    )
            })
            .collect();

        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .p_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .when_some(self.label.clone(), |element, label| {
                element.child(
                    text(&theme, TypeScale::Caption, label).text_tone(&theme, TextTone::Faint),
                )
            })
            .children(rows)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .when_label(self.label)
                    .value(SharedString::from(self.lines.len().to_string())),
            )
    }
}

/// How much of a context window a run has used.
///
/// The gauge draws a proportion only when it has both a reading and a limit,
/// and says which of the two it is missing when it does not.
#[derive(Debug, Clone, IntoElement)]
pub struct ContextGauge {
    ident: Ident,
    label: Option<SharedString>,
    used: Reading,
    limit: Limit,
    stale: Option<LastVerified>,
}

impl ContextGauge {
    /// `used` carries its own basis, so a gauge cannot be given a bare number.
    pub fn new(ident: impl Into<Ident>, used: Reading) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            used,
            limit: Limit::Unknown,
            stale: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn limit(mut self, limit: Limit) -> Self {
        self.limit = limit;
        self
    }

    /// The reading is the last verified one, from the moment stated.
    pub fn stale(mut self, verified: LastVerified) -> Self {
        self.stale = Some(verified);
        self
    }

    /// The proportion, when there is one to draw.
    ///
    /// `None` whenever the limit is unknown or the reading is unavailable: a
    /// proportion of an unknown total, or of a number nobody has, is invented.
    pub fn fraction(&self) -> Option<f32> {
        let used = self.used.quantity()?;
        let limit = self.limit.quantity()?;
        (limit.amount() > 0.0).then(|| (used.amount() / limit.amount()).clamp(0.0, 1.0) as f32)
    }
}

impl RenderOnce for ContextGauge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let fraction = self.fraction();
        let used_display = self.used.display(cx);
        let unavailable = self.used.quantity().is_none();
        let estimated = self.used.is_estimate()
            || self
                .limit
                .quantity()
                .is_some_and(|limit| limit.basis().is_estimate());

        let reading = match self.limit.quantity() {
            Some(limit) => cx.strings().format(
                StringKey::CountOfTotal,
                &[&used_display, &limit.display(cx)],
            ),
            None => used_display.clone(),
        };

        let spec = match fraction {
            Some(fraction) => NodeSpec::new(self.ident.semantic_id(), Role::Progress)
                .range(0.0, 1.0, fraction)
                .value(reading.clone()),
            // No range, so no `Progress`: a range role without a range is a
            // node claiming a position it does not have.
            None => NodeSpec::new(self.ident.semantic_id(), Role::Status)
                .value(reading.clone())
                .invalid(false),
        };
        let spec = match self.label.clone() {
            Some(label) => spec.text(label),
            None => spec,
        };

        let limit_note = (!self.limit.is_known()).then(|| {
            let words = cx.strings().text(StringKey::ContextUnknownLimit);
            text(&theme, TypeScale::Caption, words.clone())
                .text_tone(&theme, TextTone::Faint)
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("limit").semantic_id(), Role::Text)
                        .text(words)
                        .value(SharedString::new_static("unknown-limit"))
                        .parent(self.ident.semantic_id()),
                )
        });

        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .child(
                div()
                    .row()
                    .w_full()
                    .justify_between()
                    .gap_token(&theme, Space::Sm)
                    .children(self.label.clone().map(|label| {
                        text(&theme, TypeScale::Label, label).text_tone(&theme, TextTone::Muted)
                    }))
                    .child(
                        div()
                            .row()
                            .gap_token(&theme, Space::Sm)
                            .child(text(&theme, TypeScale::Label, reading).text_tone(
                                &theme,
                                if unavailable {
                                    TextTone::Faint
                                } else {
                                    TextTone::Primary
                                },
                            ))
                            .when(estimated, |element| {
                                element.child(estimate_mark(&self.ident, cx))
                            }),
                    ),
            )
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(4.0))
                    .rounded_full()
                    .overflow_hidden()
                    .bg(theme.colors.hairline_strong)
                    // Only a known proportion is drawn. An unknown limit gets
                    // the empty track and the note beneath it, not a sweep:
                    // a sweep would say the number is being worked out.
                    .when_some(fraction, |element, fraction| {
                        element.child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .bottom_0()
                                .rounded_full()
                                .bg(theme.colors.accent)
                                .w(relative(fraction)),
                        )
                    }),
            )
            .children(limit_note)
            .children(
                self.stale
                    .as_ref()
                    .map(|verified| stale_line(&self.ident, &theme, verified.sentence(cx), cx)),
            )
            .semantic_in(cx, spec)
    }
}

/// The mark that says a number was not counted, drawn beside every estimate.
fn estimate_mark(ident: &Ident, cx: &App) -> Badge {
    Badge::new(cx.strings().text(StringKey::CostEstimateMark))
        .tone(Tone::Info)
        .id(ident.child("estimate"))
}

fn stale_line(
    ident: &Ident,
    theme: &gpui_kit_theme::Theme,
    sentence: SharedString,
    cx: &App,
) -> impl IntoElement {
    text(theme, TypeScale::Caption, sentence.clone())
        .text_color(theme.colors.warning)
        .semantic_in(
            cx,
            NodeSpec::new(ident.child("stale").semantic_id(), Role::Status)
                .text(sentence)
                .value(SharedString::new_static("stale"))
                .parent(ident.semantic_id()),
        )
}

trait NodeSpecExt {
    fn when_label(self, label: Option<SharedString>) -> Self;
}

impl NodeSpecExt for NodeSpec {
    fn when_label(self, label: Option<SharedString>) -> Self {
        match label {
            Some(label) => self.text(label),
            None => self,
        }
    }
}
