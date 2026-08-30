//! One invocation of one tool, kept quieter than the answer it supports.
//!
//! # A refusal is not an absence and not an error
//!
//! Five states, five renderings, and one published name each. A host that
//! declined to run a tool made a decision, so [`ToolCallState::Refused`]
//! carries the host's reason and reads as a decision. It is not
//! [`ToolCallState::Failed`], which blames the tool for something it did; and
//! it is not [`ToolOutput::Silent`], which claims the tool ran and returned
//! nothing. A row that rendered any of those three the same way would be
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
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::Icon as Glyph;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, AgentColor, ControlSize, Radius, Space, TextTone, Theme, TypeScale,
};

use crate::controls::button::Button;
use crate::display::badge::Tone;
use crate::display::icon::{Icon as IconView, IconTone};
use crate::display::status::StatusDot;
use crate::foundation::{FocusRing, Ident, Pressable, Sizable, StyledExt, text};
use crate::motion;
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type RetryHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// The family of work one tool performs.
///
/// Family is required at construction so every transcript gets the same
/// low-saturation edge cue instead of each host inventing a colour map. It is
/// classification only: failure and refusal still use their severity colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolFamily {
    Read,
    Network,
    Shell,
    Edit,
    External,
}

impl ToolFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Network => "network",
            Self::Shell => "shell",
            Self::Edit => "edit",
            Self::External => "external",
        }
    }

    fn color(self, theme: &Theme) -> gpui::Hsla {
        theme.colors.agent.get(match self {
            Self::Read => AgentColor::Read,
            Self::Network => AgentColor::Network,
            Self::Shell => AgentColor::Shell,
            Self::Edit => AgentColor::Edit,
            Self::External => AgentColor::External,
        })
    }
}

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
    const DEFAULT_MAX_LINES: usize = 4;

    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            max_lines: Some(Self::DEFAULT_MAX_LINES),
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

    /// Draws the whole body. The transcript default is four lines; callers
    /// opt into an unbounded body only where the surrounding surface already
    /// owns scrolling or another bound.
    pub fn all_lines(mut self) -> Self {
        self.max_lines = None;
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

    pub fn remaining_line_count(&self) -> usize {
        self.line_count() - self.shown_line_count()
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
                &[
                    cx.numbers().count(self.shown_line_count()).as_ref(),
                    cx.numbers().count(total).as_ref(),
                ],
            );
        }
        cx.strings().format_plural(
            StringKey::AgentLinesOne,
            StringKey::AgentLinesMany,
            cx.numbers().plural(total),
            &[cx.numbers().count(total).as_ref()],
        )
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
    /// row reported rather than the colour it painted.
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
}

impl HasPhase for ToolCallState {
    fn phase(&self) -> Phase {
        match self {
            Self::PendingApproval => Phase::Blocked,
            Self::Running => Phase::Loading,
            Self::Succeeded { .. } => Phase::Ready,
            Self::Failed { .. } => Phase::Error,
            Self::Refused { .. } => Phase::Unavailable,
        }
    }

    fn reason(&self) -> Option<&str> {
        ToolCallState::reason(self).map(|reason| reason.as_ref())
    }
}

impl ToolCallState {
    /// Whether anything ran, which is what makes an elapsed time meaningful.
    fn ran(&self) -> bool {
        matches!(
            self,
            Self::Running | Self::Succeeded { .. } | Self::Failed { .. }
        )
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

/// One quiet evidence row for a tool invocation.
///
/// The row is collapsed by default. Arguments, output and an optional diff
/// appear only when the caller advances `expanded`; the component reports the
/// requested next state through `on_toggle` and runs no tool itself.
#[derive(IntoElement)]
pub struct ToolCall {
    ident: Ident,
    family: ToolFamily,
    tool: SharedString,
    summary: Option<SharedString>,
    arguments: Option<ToolBody>,
    state: ToolCallState,
    elapsed: Elapsed,
    diff: Option<AnyElement>,
    expanded: bool,
    on_toggle: Option<ToggleHandler>,
    on_retry: Option<RetryHandler>,
}

impl std::fmt::Debug for ToolCall {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToolCall")
            .field("ident", &self.ident)
            .field("family", &self.family)
            .field("tool", &self.tool)
            .field("state", &self.state)
            .field("elapsed", &self.elapsed)
            .field("expanded", &self.expanded)
            .field("has_arguments", &self.arguments.is_some())
            .field("has_toggle_handler", &self.on_toggle.is_some())
            .field("has_retry_handler", &self.on_retry.is_some())
            .finish()
    }
}

impl ToolCall {
    /// `tool` is the short verb or tool name the host wants the transcript to
    /// show. Kit owns no tool catalogue and invents no display name.
    pub fn new(ident: impl Into<Ident>, family: ToolFamily, tool: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            family,
            tool: tool.into(),
            summary: None,
            arguments: None,
            state: ToolCallState::PendingApproval,
            elapsed: Elapsed::Unknown,
            diff: None,
            expanded: false,
            on_toggle: None,
            on_retry: None,
        }
    }

    /// A concise, display-safe key argument such as a path, command or query.
    ///
    /// Unlike [`ToolBody`], this line is published to the semantic tree so a
    /// screen reader can hear the same summary. Do not pass credentials or
    /// unredacted user content; keep those in the redacted body.
    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// What the tool was called with. Hidden while the row is collapsed.
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

    /// A caller-built intra-change view, typically a small [`DiffView`](crate::content::DiffView).
    pub fn diff(mut self, diff: impl IntoElement) -> Self {
        self.diff = Some(diff.into_any_element());
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Offers a quiet inline retry action on a failed row only.
    pub fn on_retry(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(Rc::new(handler));
        self
    }

    fn has_details(&self) -> bool {
        self.arguments.is_some()
            || self.diff.is_some()
            || matches!(
                self.state,
                ToolCallState::Succeeded {
                    output: ToolOutput::Body(_)
                }
            )
    }
}

impl RenderOnce for ToolCall {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();
        let has_details = self.has_details();
        let open = self.expanded && has_details;
        let actionable = has_details && self.on_toggle.is_some();
        let family_color = self.family.color(&theme);
        let retryable = matches!(self.state, ToolCallState::Failed { .. });

        let mut dot = match self.state {
            ToolCallState::PendingApproval | ToolCallState::Running => {
                StatusDot::new(Tone::Accent).tint(family_color)
            }
            ToolCallState::Succeeded { .. } => StatusDot::new(Tone::Neutral).tint(family_color),
            ToolCallState::Failed { .. } => StatusDot::new(Tone::Danger),
            ToolCallState::Refused { .. } => StatusDot::new(Tone::Warning),
        };
        if matches!(self.state, ToolCallState::Running) {
            dot = dot
                .busy(ident.child("state.mark"))
                .activity(motion::Activity::Advancing);
        }

        let status = match &self.state {
            ToolCallState::PendingApproval => Some((
                ident.child("state"),
                cx.strings().text(StringKey::AgentPendingApproval),
                theme.colors.text_faint,
                "pending-approval",
            )),
            ToolCallState::Running => Some((
                ident.child("state"),
                cx.strings().text(StringKey::AgentRunning),
                theme.colors.text_faint,
                "running",
            )),
            ToolCallState::Succeeded {
                output: ToolOutput::Silent,
            } => Some((
                ident.child("result"),
                cx.strings().text(StringKey::AgentNoOutput),
                theme.colors.text_faint,
                "nothing",
            )),
            ToolCallState::Succeeded { .. } => None,
            ToolCallState::Failed { error } => Some((
                ident.child("error"),
                error.clone(),
                theme.colors.danger,
                "failed",
            )),
            ToolCallState::Refused { reason } => Some((
                ident.child("refusal"),
                reason.clone(),
                theme.colors.warning,
                "refused",
            )),
        };

        let summary = self.summary.clone();
        let semantic_text = match &summary {
            Some(summary) => SharedString::from(format!("{} {}", self.tool, summary)),
            None => self.tool.clone(),
        };
        let mut line_core = div()
            .id(ident.child("toggle").element_id())
            .row()
            .min_w_0()
            .flex_1()
            .items_center()
            .gap_token(&theme, Space::Sm)
            .py(px(theme.space(Space::Xxs)))
            .child(dot)
            .child(
                // Family lives on the mark, not on the name. Colouring every
                // tool name turned a column of rows into one hue per row,
                // which is a rainbow standing in for five categories and
                // leaves nothing neutral for a severity colour to stand out
                // against.
                text(&theme, TypeScale::Caption, self.tool.clone())
                    .flex_none()
                    .mono(&theme)
                    .text_tone(&theme, TextTone::Primary),
            )
            .children(summary.map(|summary| {
                text(&theme, TypeScale::Caption, summary)
                    .min_w_0()
                    .truncate()
                    .mono(&theme)
                    .text_tone(&theme, TextTone::Faint)
            }))
            .children(status.map(|(status_ident, words, color, value)| {
                // The status is part of the sentence the row reads as, so it
                // takes the width it needs and no more. Stretched, it pushed
                // the duration to the far edge and opened a gutter wide
                // enough to lose the two apart.
                text(&theme, TypeScale::Caption, words.clone())
                    .min_w_0()
                    .truncate()
                    .mono(&theme)
                    .text_color(color)
                    .semantic_in(
                        cx,
                        NodeSpec::new(status_ident.semantic_id(), Role::Status)
                            .parent(ident.semantic_id())
                            .text(words)
                            .value(value),
                    )
            }))
            .children(
                self.state
                    .ran()
                    .then_some(&self.elapsed)
                    .and_then(|elapsed| match elapsed {
                        Elapsed::Took(took) => Some(took.clone()),
                        Elapsed::Unknown => None,
                    })
                    .map(|elapsed| {
                        text(&theme, TypeScale::Caption, elapsed.clone())
                            .flex_none()
                            .mono(&theme)
                            .text_tone(&theme, TextTone::Faint)
                            .semantic_in(
                                cx,
                                NodeSpec::new(ident.child("elapsed").semantic_id(), Role::Text)
                                    .parent(ident.semantic_id())
                                    .text(elapsed.clone())
                                    .value(elapsed),
                            )
                    }),
            )
            .children(actionable.then(|| {
                IconView::new(if open {
                    Glyph::AltArrowDown
                } else {
                    Glyph::AltArrowRight
                })
                .small()
                .tone(IconTone::Faint)
            }))
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .focus_ring(&theme)
            });

        if let Some(handler) = self.on_toggle.clone().filter(|_| actionable) {
            let key_handler = Rc::clone(&handler);
            line_core = line_core
                .on_click(move |_, window, cx| handler(!open, window, cx))
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        key_handler(!open, window, cx);
                        cx.stop_propagation();
                    }
                });
        }
        let line_core = line_core.semantic_in(
            cx,
            NodeSpec::new(
                ident.child("toggle").semantic_id(),
                if actionable { Role::Button } else { Role::Text },
            )
            .parent(ident.semantic_id())
            .text(semantic_text.clone())
            .value(self.family.as_str())
            .expanded(open),
        );

        let retry = self
            .on_retry
            .filter(|_| retryable)
            .map(|handler| inline_retry(&ident, handler, cx));
        let line = div()
            .row()
            .w_full()
            .min_w_0()
            .items_center()
            .gap_token(&theme, Space::Sm)
            .child(line_core)
            .children(retry);

        let arguments = self.arguments.filter(|_| open).map(|body| {
            evidence_body(
                &ident.child("arguments"),
                &ident,
                &theme,
                cx.strings().text(StringKey::AgentArguments),
                &body,
                cx,
            )
        });
        let result = match &self.state {
            ToolCallState::Succeeded {
                output: ToolOutput::Body(body),
            } if open => Some(evidence_body(
                &ident.child("result"),
                &ident,
                &theme,
                cx.strings().text(StringKey::AgentResult),
                body,
                cx,
            )),
            _ => None,
        };

        // Arguments and result are two blocks, not one block with a seam:
        // at the tighter step they met close enough to read as a single
        // panel that had been cut in half.
        div()
            .w_full()
            .column()
            .gap_token(&theme, Space::Sm)
            .child(line)
            .children(arguments)
            .children(
                open.then_some(self.diff)
                    .flatten()
                    .map(|diff| div().w_full().pl(px(theme.space(Space::Lg))).child(diff)),
            )
            .children(result)
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Group)
                    .text(semantic_text)
                    .description(self.family.as_str())
                    .value(self.state.as_str())
                    .busy(matches!(self.state, ToolCallState::Running)),
            )
    }
}

fn inline_retry(call: &Ident, handler: RetryHandler, cx: &mut App) -> AnyElement {
    let ident = call.child("retry");
    let label = cx.strings().text(StringKey::TryAgain);
    // Retry is an ordinary action, so it uses the ordinary control instead
    // of maintaining a second, caption-sized implementation of button focus,
    // keyboard activation, press response, and semantic state in this row.
    Button::new(ident)
        .label(label)
        .secondary()
        .control_size(ControlSize::Xs)
        .semantic_parent(call.semantic_id())
        .on_click(move |window, cx| handler(window, cx))
        .into_any_element()
}

/// One unlabelled visual block of caller-owned text. Its semantic node keeps
/// the arguments/result distinction and publishes only the shape.
fn evidence_body(
    ident: &Ident,
    call: &Ident,
    theme: &Theme,
    label: SharedString,
    body: &ToolBody,
    cx: &mut App,
) -> AnyElement {
    let shape = body.shape(cx);
    let remaining = body.remaining_line_count();
    let more = (remaining > 0).then(|| {
        cx.strings().format_plural(
            StringKey::AgentMoreLineOne,
            StringKey::AgentMoreLineMany,
            cx.numbers().plural(remaining),
            &[cx.numbers().count(remaining).as_ref()],
        )
    });
    div()
        .w_full()
        .column()
        .gap(px(theme.space(Space::Xxs)))
        .pl(px(theme.space(Space::Lg)))
        .child(
            div()
                .w_full()
                .column()
                .px_token(theme, Space::Sm)
                .py_token(theme, Space::Xs)
                .radius(theme, Radius::Small)
                .bg(theme.colors.agent.evidence_wash)
                .mono(theme)
                .children(body.shown_lines().into_iter().map(|line| {
                    text(theme, TypeScale::Caption, line)
                        .mono(theme)
                        .text_tone(theme, TextTone::Muted)
                }))
                .children(more.map(|more| {
                    text(theme, TypeScale::Caption, more)
                        .mono(theme)
                        .text_tone(theme, TextTone::Faint)
                })),
        )
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Text)
                .parent(call.semantic_id())
                .text(label)
                .value(shape),
        )
        .into_any_element()
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
    fn bodies_are_bounded_by_default_and_can_opt_out() {
        let body = ToolBody::new("one\ntwo\nthree\nfour\nfive");
        assert_eq!(body.shown_line_count(), 4);
        assert_eq!(body.remaining_line_count(), 1);

        let whole = body.all_lines();
        assert_eq!(whole.shown_line_count(), 5);
        assert_eq!(whole.remaining_line_count(), 0);
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

#[cfg(test)]
mod tool_call_phase_tests {
    use super::*;

    #[test]
    fn approval_is_blocked_and_a_refusal_is_unavailable() {
        assert_eq!(ToolCallState::PendingApproval.phase(), Phase::Blocked);
        assert_eq!(ToolCallState::Running.phase(), Phase::Loading);
        assert_eq!(ToolCallState::succeeded_silently().phase(), Phase::Ready);
        assert_eq!(ToolCallState::failed("boom").phase(), Phase::Error);
        let refused = ToolCallState::refused("not allowed");
        assert_eq!(refused.phase(), Phase::Unavailable);
        assert_eq!(HasPhase::reason(&refused), Some("not allowed"));
    }
}
