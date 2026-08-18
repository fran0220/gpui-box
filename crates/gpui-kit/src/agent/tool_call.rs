//! One invocation of one tool: what was called, with what, what came back,
//! and how long it took.
//!
//! # A refusal is not an absence and not an error
//!
//! Five states, five renderings, and one published name each. A host that
//! declined to run a tool made a decision, so [`ToolCallState::Refused`]
//! carries the host's reason and reads as a decision. It is not
//! [`ToolCallState::Failed`], which blames the tool for something it did; and
//! it is not [`ToolOutput::Silent`], which claims the tool ran and returned
//! nothing. A card that rendered any of those three the same way would be
//! telling the reader something nobody established.
//!
//! # Nothing here is this crate's data
//!
//! Arguments, results, errors and refusal reasons are the caller's, may be
//! long, and may be secret. So a [`ToolBody`] publishes its *shape* — how many
//! lines it holds, and how many of them are on screen — and never its text,
//! the rule [`DescriptionValue::Redacted`](crate::display::description_list::DescriptionValue)
//! already keeps. Truncation is stated in the same words it publishes: a body
//! cut to three of twelve lines says so where the cut happens, rather than
//! fading out and leaving the reader to guess how much is missing.
//!
//! An elapsed time is a string the caller already wrote, for the reason
//! [`Timeline`](crate::display::timeline::Timeline) takes one: turning a
//! duration into words is locale work this crate does not do.

use std::rc::Rc;

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};
use gpui_kit_assets::Icon as Glyph;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, TextTone, Theme, TypeScale};

use crate::controls::button::Button;
use crate::display::badge::{Badge, Tone};
use crate::display::icon::{Icon as IconView, IconTone};
use crate::display::status::Callout;
use crate::foundation::{CardVariant, Ident, Sizable, StyledExt, text};
use crate::strings::{ActiveStrings, StringKey};

type RetryHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A block of caller-owned text — the arguments a tool was called with, or
/// what it returned — with an optional limit on how much of it is drawn.
///
/// The body keeps the whole text so the component can say how much it left
/// out, and draws only what the limit allows. Nothing but the measurement
/// reaches the semantic tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolBody {
    text: SharedString,
    max_lines: Option<usize>,
}

impl ToolBody {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            max_lines: None,
        }
    }

    /// Draws at most this many lines and states how many there are in total.
    ///
    /// A limit of zero is one line: a body that drew nothing at all and called
    /// itself truncated would be indistinguishable from a body nobody passed.
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.max_lines = Some(lines.max(1));
        self
    }

    /// The text as the caller gave it. It is the caller's own data coming
    /// back, and the component never publishes it.
    pub fn text(&self) -> &SharedString {
        &self.text
    }

    pub fn line_count(&self) -> usize {
        self.text.lines().count().max(1)
    }

    pub fn shown_line_count(&self) -> usize {
        match self.max_lines {
            Some(limit) => limit.min(self.line_count()),
            None => self.line_count(),
        }
    }

    pub fn is_truncated(&self) -> bool {
        self.shown_line_count() < self.line_count()
    }

    /// The lines that are actually drawn.
    fn shown_lines(&self) -> Vec<SharedString> {
        self.text
            .lines()
            .take(self.shown_line_count())
            .map(|line| SharedString::from(line.to_string()))
            .collect()
    }

    /// The measurement a reader is shown and a node publishes: never the text.
    pub fn shape(&self, cx: &App) -> SharedString {
        let total = self.line_count();
        if self.is_truncated() {
            return cx.strings().format(
                StringKey::AgentTruncated,
                &[&self.shown_line_count().to_string(), &total.to_string()],
            );
        }
        if total == 1 {
            cx.strings().text(StringKey::AgentLinesOne)
        } else {
            cx.strings()
                .format(StringKey::AgentLinesMany, &[&total.to_string()])
        }
    }
}

impl From<SharedString> for ToolBody {
    fn from(value: SharedString) -> Self {
        Self::new(value)
    }
}

impl From<&'static str> for ToolBody {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ToolBody {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// What a tool that ran to completion gave back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolOutput {
    Body(ToolBody),
    /// The tool ran and returned nothing. This is a fact about a completed
    /// call, which is why it can only be reached through
    /// [`ToolCallState::Succeeded`] and never stands in for a refusal.
    Silent,
}

/// Where one invocation has got to.
///
/// The five are a closed set rather than flags, so a card cannot be running
/// and failed at once, and cannot be refused without a reason: the host that
/// declined has to say what it declined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCallState {
    /// Nothing has run. Somebody has to allow it first.
    PendingApproval,
    Running,
    Succeeded {
        output: ToolOutput,
    },
    /// The tool ran and did not succeed, in the host's own words.
    Failed {
        error: SharedString,
    },
    /// The host declined to run it, in the host's own words. Nothing ran, so
    /// there is no result and no error — only a decision.
    Refused {
        reason: SharedString,
    },
}

impl ToolCallState {
    pub fn succeeded(output: impl Into<ToolBody>) -> Self {
        Self::Succeeded {
            output: ToolOutput::Body(output.into()),
        }
    }

    /// A call that ran, succeeded, and returned nothing.
    pub fn succeeded_silently() -> Self {
        Self::Succeeded {
            output: ToolOutput::Silent,
        }
    }

    pub fn failed(error: impl Into<SharedString>) -> Self {
        Self::Failed {
            error: error.into(),
        }
    }

    pub fn refused(reason: impl Into<SharedString>) -> Self {
        Self::Refused {
            reason: reason.into(),
        }
    }

    /// The name the semantic node publishes, so a test asserts the state the
    /// card reported rather than the colour it painted.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PendingApproval => "pending-approval",
            Self::Running => "running",
            Self::Succeeded { .. } => "succeeded",
            Self::Failed { .. } => "failed",
            Self::Refused { .. } => "refused",
        }
    }

    /// Why this call did not produce a result, when that is a thing the host
    /// said. A failure's error and a refusal's reason are different sentences
    /// and are never rendered in the same place.
    pub fn reason(&self) -> Option<&SharedString> {
        match self {
            Self::Failed { error } => Some(error),
            Self::Refused { reason } => Some(reason),
            _ => None,
        }
    }

    /// Whether anything ran, which is what makes an elapsed time meaningful.
    fn ran(&self) -> bool {
        matches!(
            self,
            Self::Running | Self::Succeeded { .. } | Self::Failed { .. }
        )
    }

    fn tone(&self) -> Tone {
        match self {
            Self::PendingApproval => Tone::Info,
            Self::Running => Tone::Accent,
            Self::Succeeded { .. } => Tone::Success,
            Self::Failed { .. } => Tone::Danger,
            Self::Refused { .. } => Tone::Warning,
        }
    }

    fn glyph(&self) -> Glyph {
        match self {
            Self::PendingApproval => Glyph::Key,
            Self::Running => Glyph::Refresh,
            Self::Succeeded { .. } => Glyph::Check,
            Self::Failed { .. } => Glyph::Danger,
            Self::Refused { .. } => Glyph::CloseCircle,
        }
    }

    fn key(&self) -> StringKey {
        match self {
            Self::PendingApproval => StringKey::AgentPendingApproval,
            Self::Running => StringKey::AgentRunning,
            Self::Succeeded { .. } => StringKey::AgentSucceeded,
            Self::Failed { .. } => StringKey::AgentFailed,
            Self::Refused { .. } => StringKey::AgentDeclined,
        }
    }
}

/// How long a call took, as far as anyone has said.
///
/// A duration nobody stated is a state, not a zero — the rule
/// [`TransportDuration`](crate::content::transport::TransportDuration) keeps
/// for the same reason.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Elapsed {
    /// A duration the caller has already put into words.
    Took(SharedString),
    #[default]
    Unknown,
}

impl Elapsed {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Took(_) => "known",
            Self::Unknown => "unknown",
        }
    }

    fn shown(&self, cx: &App) -> SharedString {
        match self {
            Self::Took(took) => took.clone(),
            Self::Unknown => cx.strings().text(StringKey::AgentElapsedUnknown),
        }
    }
}

impl From<SharedString> for Elapsed {
    fn from(value: SharedString) -> Self {
        Self::Took(value)
    }
}

impl From<&'static str> for Elapsed {
    fn from(value: &'static str) -> Self {
        Self::Took(SharedString::new_static(value))
    }
}

impl From<String> for Elapsed {
    fn from(value: String) -> Self {
        Self::Took(SharedString::from(value))
    }
}

/// One tool invocation, in whichever of its five states holds.
#[derive(IntoElement)]
pub struct ToolCallCard {
    ident: Ident,
    tool: SharedString,
    arguments: Option<ToolBody>,
    state: ToolCallState,
    elapsed: Elapsed,
    diff: Option<AnyElement>,
    on_retry: Option<RetryHandler>,
}

impl std::fmt::Debug for ToolCallCard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToolCallCard")
            .field("ident", &self.ident)
            .field("tool", &self.tool)
            .field("state", &self.state)
            .field("elapsed", &self.elapsed)
            .field("has_arguments", &self.arguments.is_some())
            .field("has_handler", &self.on_retry.is_some())
            .finish()
    }
}

impl ToolCallCard {
    /// `tool` is the name of the tool as the host knows it. This crate has no
    /// catalogue of tools and invents no display name for one.
    pub fn new(ident: impl Into<Ident>, tool: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            tool: tool.into(),
            arguments: None,
            state: ToolCallState::PendingApproval,
            elapsed: Elapsed::Unknown,
            diff: None,
            on_retry: None,
        }
    }

    /// What the tool was called with.
    pub fn arguments(mut self, arguments: impl Into<ToolBody>) -> Self {
        self.arguments = Some(arguments.into());
        self
    }

    pub fn state(mut self, state: ToolCallState) -> Self {
        self.state = state;
        self
    }

    pub fn elapsed(mut self, elapsed: impl Into<Elapsed>) -> Self {
        self.elapsed = elapsed.into();
        self
    }

    /// A caller-built intra-change view, typically a small [`DiffView`].
    pub fn diff(mut self, diff: impl IntoElement) -> Self {
        self.diff = Some(diff.into_any_element());
        self
    }

    /// Offers one control that reports the call should be tried again.
    ///
    /// It exists only on a failed call, and nothing is retried here: the card
    /// runs no tool, so a host that refuses the request simply keeps showing
    /// the failure that still holds.
    pub fn on_retry(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(Rc::new(handler));
        self
    }

    fn retryable(&self) -> bool {
        matches!(self.state, ToolCallState::Failed { .. }) && self.on_retry.is_some()
    }
}

impl RenderOnce for ToolCallCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();
        let tone = self.state.tone();
        let retryable = self.retryable();

        let header = div()
            .row()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .child({
                let mark = IconView::new(self.state.glyph())
                    .small()
                    .tone(icon_tone(tone));
                // A running call turns. The glyph is a rotation arrow, and
                // a still one reads as a call that has jammed.
                match self.state {
                    ToolCallState::Running => mark.spinning(ident.child("state.mark")),
                    _ => mark,
                }
            })
            .child(
                text(&theme, TypeScale::Code, self.tool.clone())
                    .flex_1()
                    .min_w_0()
                    .font_family(theme.typography.mono.clone()),
            )
            .child(
                Badge::new(cx.strings().text(self.state.key()))
                    .tone(tone)
                    .id(ident.child("state")),
            )
            .children(self.state.ran().then(|| {
                let words = self.elapsed.shown(cx);
                text(&theme, TypeScale::Caption, words.clone())
                    .flex_none()
                    .text_tone(
                        &theme,
                        match self.elapsed {
                            Elapsed::Took(_) => TextTone::Muted,
                            Elapsed::Unknown => TextTone::Faint,
                        },
                    )
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.child("elapsed").semantic_id(), Role::Text)
                            .parent(ident.semantic_id())
                            .text(words)
                            .value(match &self.elapsed {
                                Elapsed::Took(took) => took.clone(),
                                Elapsed::Unknown => SharedString::new_static("unknown"),
                            }),
                    )
            }));

        let arguments = self.arguments.map(|body| {
            block(
                &ident.child("arguments"),
                &ident,
                &theme,
                cx.strings().text(StringKey::AgentArguments),
                &body,
                cx,
            )
        });

        // Each state renders its own consequence, and only its own: a failure
        // shows an error, a refusal shows a decision, and neither is ever
        // drawn as the other or as an absence of output.
        let outcome = match &self.state {
            ToolCallState::PendingApproval | ToolCallState::Running => None,
            ToolCallState::Succeeded { output } => Some(match output {
                ToolOutput::Body(body) => block(
                    &ident.child("result"),
                    &ident,
                    &theme,
                    cx.strings().text(StringKey::AgentResult),
                    body,
                    cx,
                ),
                ToolOutput::Silent => {
                    let words = cx.strings().text(StringKey::AgentNoOutput);
                    text(&theme, TypeScale::Caption, words.clone())
                        .text_tone(&theme, TextTone::Muted)
                        .semantic_in(
                            cx,
                            NodeSpec::new(ident.child("result").semantic_id(), Role::Text)
                                .parent(ident.semantic_id())
                                .text(words)
                                .value("nothing"),
                        )
                        .into_any_element()
                }
            }),
            ToolCallState::Failed { error } => Some(
                Callout::new(error.clone(), Tone::Danger)
                    .id(ident.child("error"))
                    .into_any_element(),
            ),
            ToolCallState::Refused { reason } => Some(
                Callout::new(reason.clone(), Tone::Warning)
                    .id(ident.child("refusal"))
                    .into_any_element(),
            ),
        };

        let retry = self.on_retry.filter(|_| retryable).map(|handler| {
            Button::new(ident.child("retry"))
                .label(cx.strings().text(StringKey::TryAgain))
                .secondary()
                .small()
                .semantic_parent(ident.semantic_id())
                .on_click(move |window, cx| handler(window, cx))
        });

        div()
            .w_full()
            .column()
            .gap_token(&theme, Space::Sm)
            .p_token(&theme, Space::Md)
            .card_surface(&theme, CardVariant::Elevated)
            .child(header)
            .children(arguments)
            .children(self.diff)
            .children(outcome)
            .children(retry.map(|retry| div().row().child(retry)))
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Group)
                    .text(self.tool.clone())
                    .value(self.state.as_str())
                    .busy(matches!(self.state, ToolCallState::Running)),
            )
    }
}

/// One labelled block of caller-owned text, drawn to its limit and publishing
/// only its measurement.
fn block(
    ident: &Ident,
    card: &Ident,
    theme: &Theme,
    label: SharedString,
    body: &ToolBody,
    cx: &mut App,
) -> gpui::AnyElement {
    let shape = body.shape(cx);
    div()
        .w_full()
        .column()
        .gap(px(2.0))
        .child(
            div()
                .row()
                .justify_between()
                .gap_token(theme, Space::Sm)
                .child(
                    text(theme, TypeScale::Caption, label.clone())
                        .text_tone(theme, TextTone::Faint),
                )
                // The measurement is stated whether or not anything was cut,
                // so "there is more" is read off the same line every time
                // rather than appearing only when it is bad news.
                .child(
                    text(theme, TypeScale::Caption, shape.clone())
                        .text_tone(theme, TextTone::Faint),
                ),
        )
        .child(
            div()
                .w_full()
                .px_token(theme, Space::Sm)
                .py(px(2.0))
                .radius(theme, Radius::Small)
                .surface(theme, Surface::Raised)
                .font_family(theme.typography.mono.clone())
                // One element per line, the way a fenced block is drawn: a
                // single element would run the whole body together.
                .children(body.shown_lines().into_iter().map(|line| {
                    text(theme, TypeScale::Code, line).text_tone(theme, TextTone::Muted)
                })),
        )
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Text)
                .parent(card.semantic_id())
                .text(label)
                // The shape, never the text: this is somebody's data and may
                // be a credential.
                .value(shape),
        )
        .into_any_element()
}

fn icon_tone(tone: Tone) -> IconTone {
    match tone {
        Tone::Neutral => IconTone::Muted,
        Tone::Accent => IconTone::Accent,
        Tone::Success => IconTone::Success,
        Tone::Warning => IconTone::Warning,
        Tone::Danger => IconTone::Danger,
        Tone::Info => IconTone::Info,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_within_its_limit_is_not_truncated() {
        let body = ToolBody::new("one\ntwo").max_lines(4);
        assert_eq!(body.line_count(), 2);
        assert_eq!(body.shown_line_count(), 2);
        assert!(!body.is_truncated());
        assert_eq!(body.shown_lines(), vec!["one", "two"]);
    }

    #[test]
    fn a_body_past_its_limit_keeps_the_whole_count() {
        let body = ToolBody::new("one\ntwo\nthree").max_lines(1);
        assert!(body.is_truncated());
        assert_eq!(body.shown_line_count(), 1);
        assert_eq!(body.line_count(), 3);
        assert_eq!(body.shown_lines(), vec!["one"]);
        assert_eq!(
            body.text().as_ref(),
            "one\ntwo\nthree",
            "the caller's data comes back whole; only the drawing is cut"
        );
    }

    #[test]
    fn a_limit_of_zero_still_draws_a_line() {
        let body = ToolBody::new("one\ntwo").max_lines(0);
        assert_eq!(body.shown_line_count(), 1);
    }

    #[test]
    fn every_state_publishes_its_own_name() {
        let names = [
            ToolCallState::PendingApproval.as_str(),
            ToolCallState::Running.as_str(),
            ToolCallState::succeeded_silently().as_str(),
            ToolCallState::failed("boom").as_str(),
            ToolCallState::refused("no").as_str(),
        ];
        let mut unique = names.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len());
    }

    #[test]
    fn only_a_call_that_ran_has_a_duration_to_report() {
        assert!(!ToolCallState::PendingApproval.ran());
        assert!(!ToolCallState::refused("declined").ran());
        assert!(ToolCallState::Running.ran());
        assert!(ToolCallState::succeeded_silently().ran());
        assert!(ToolCallState::failed("boom").ran());
    }
}
