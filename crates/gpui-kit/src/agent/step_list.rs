//! An ordered run of steps, each with its own state and its own body.
//!
//! # A run of unknown length has no progress
//!
//! [`ProgressBar`] already draws the
//! line this component has to keep: a bar reports a position only when the
//! extent of the work is known, and an unknown extent is drawn as an unknown
//! extent rather than as a bar that happens to be part full. A run whose steps
//! are still arriving is exactly that case — nobody has counted the work — so
//! [`RunLength::Unknown`] makes the summary indeterminate, publishes how many
//! steps are done, and publishes no fraction at all. Inventing one would mean
//! dividing by a total nobody established.
//!
//! # The list applies nothing
//!
//! Which step is running, which failed, and why one was skipped are the
//! caller's facts. A step's reason is the host's own words, shown verbatim,
//! and a step may carry any body the caller builds — usually a
//! [`ToolCallCard`](crate::agent::tool_call::ToolCallCard).

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, Theme, TypeScale};

use crate::display::badge::Tone;
use crate::display::progress::ProgressBar;
use crate::display::status::StatusDot;
use crate::foundation::{Ident, StyledExt};
use crate::strings::{ActiveStrings, StringKey};

/// How wide the column holding the rail and its dots is.
const RAIL: f32 = 16.0;

/// Where one step has got to.
///
/// A step that did not run and a step that ran and failed are different
/// sentences, and both carry the host's reason rather than a word this crate
/// chose.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StepState {
    #[default]
    Pending,
    Running,
    Done,
    /// The step ran and did not succeed, in the host's own words.
    Failed(SharedString),
    /// The step never ran, and this is why.
    Skipped(SharedString),
}

impl StepState {
    /// The name the semantic node publishes.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed(_) => "failed",
            Self::Skipped(_) => "skipped",
        }
    }

    /// The host's words, where the host said any.
    pub fn reason(&self) -> Option<&SharedString> {
        match self {
            Self::Failed(reason) | Self::Skipped(reason) => Some(reason),
            _ => None,
        }
    }

    pub fn tone(&self) -> Tone {
        match self {
            Self::Pending => Tone::Neutral,
            Self::Running => Tone::Accent,
            Self::Done => Tone::Success,
            Self::Failed(_) => Tone::Danger,
            Self::Skipped(_) => Tone::Warning,
        }
    }

    fn key(&self) -> StringKey {
        match self {
            Self::Pending => StringKey::AgentPending,
            Self::Running => StringKey::AgentRunning,
            Self::Done => StringKey::AgentDone,
            Self::Failed(_) => StringKey::AgentFailed,
            Self::Skipped(_) => StringKey::AgentSkipped,
        }
    }
}

/// One step of a run.
pub struct Step {
    id: SharedString,
    title: SharedString,
    state: StepState,
    body: Option<AnyElement>,
}

impl std::fmt::Debug for Step {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Step")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("state", &self.state)
            .field("has_body", &self.body.is_some())
            .finish()
    }
}

impl Step {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            state: StepState::Pending,
            body: None,
        }
    }

    pub fn state(mut self, state: StepState) -> Self {
        self.state = state;
        self
    }

    /// What the step is made of, usually a
    /// [`ToolCallCard`](crate::agent::tool_call::ToolCallCard).
    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// Whether the caller has counted the run.
///
/// This is [`PageTotal`](crate::navigation::pagination::PageTotal) for a run:
/// a host streaming steps as they are decided knows only that more are coming,
/// so that is all the list claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunLength {
    /// Every step in the run is in this list.
    #[default]
    Known,
    /// More steps will arrive and nobody knows how many.
    Unknown,
}

impl RunLength {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Known => "known",
            Self::Unknown => "unknown",
        }
    }
}

/// An ordered run of steps with a summary of how far it has got.
#[derive(IntoElement)]
pub struct StepList {
    ident: Ident,
    steps: Vec<Step>,
    length: RunLength,
}

impl std::fmt::Debug for StepList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StepList")
            .field("ident", &self.ident)
            .field("steps", &self.steps.len())
            .field("length", &self.length)
            .finish()
    }
}

impl StepList {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            steps: Vec::new(),
            length: RunLength::Known,
        }
    }

    pub fn step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    pub fn steps(mut self, steps: impl IntoIterator<Item = Step>) -> Self {
        self.steps.extend(steps);
        self
    }

    /// Whether every step of the run is already in this list.
    pub fn length(mut self, length: RunLength) -> Self {
        self.length = length;
        self
    }

    fn done(&self) -> usize {
        self.steps
            .iter()
            .filter(|step| matches!(step.state, StepState::Done))
            .count()
    }
}

impl RenderOnce for StepList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();
        let total = self.steps.len();
        let done = self.done();

        // A counted run reports a position; an uncounted one reports only what
        // has finished, and the bar stays indeterminate rather than dividing
        // by a total nobody has.
        let summary = ProgressBar::new(ident.child("progress"));
        let summary = match self.length {
            RunLength::Known => summary.count(done, total),
            RunLength::Unknown => summary.display(if done == 1 {
                cx.strings().text(StringKey::AgentStepsDoneOne)
            } else {
                cx.strings()
                    .format(StringKey::AgentStepsDoneMany, &[&done.to_string()])
            }),
        };

        let last = total.saturating_sub(1);
        let mut run = div().w_full().column().gap_token(&theme, Space::Md);
        for (index, step) in self.steps.into_iter().enumerate() {
            run = run.child(step_element(&ident, &theme, step, index < last, cx));
        }

        div()
            .w_full()
            .column()
            .gap_token(&theme, Space::Md)
            .child(summary)
            .child(run)
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::List)
                    // The count is what the list holds, which under an unknown
                    // length is not the same as what the run contains.
                    .value(total.to_string())
                    .busy(matches!(self.length, RunLength::Unknown)),
            )
    }
}

fn step_element(
    list: &Ident,
    theme: &Theme,
    step: Step,
    continues: bool,
    cx: &mut App,
) -> AnyElement {
    let ident = list.child(step.id.as_ref());
    let running = matches!(step.state, StepState::Running);

    let rail = div()
        .w(px(RAIL))
        .flex_none()
        .column()
        .items_center()
        .child(div().mt(px(4.0)).child(StatusDot::new(step.state.tone())))
        .when(continues, |element| {
            element.child(
                div()
                    .mt(px(4.0))
                    .w(px(theme.borders.hairline))
                    .flex_1()
                    .min_h(px(theme.space(Space::Md)))
                    .bg(theme.colors.hairline),
            )
        });

    let reason = step.state.reason().cloned().map(|reason| {
        let state = step.state.as_str();
        div()
            .type_scale(theme, TypeScale::Caption)
            .text_color(step.state.tone().color(theme))
            .child(reason.clone())
            .semantic_in(
                cx,
                NodeSpec::new(ident.child("reason").semantic_id(), Role::Status)
                    .parent(ident.semantic_id())
                    // The host's words, and the state they belong to, so a
                    // skipped step is never read as a failed one.
                    .text(reason)
                    .value(state),
            )
    });

    div()
        .row()
        .items_start()
        .w_full()
        .gap_token(theme, Space::Sm)
        .child(rail)
        .child(
            div()
                .column()
                .flex_1()
                .min_w_0()
                .gap(px(2.0))
                .child(
                    div()
                        .row()
                        .gap_token(theme, Space::Sm)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .type_scale(theme, TypeScale::Label)
                                .text_color(theme.colors.text)
                                .child(step.title.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .type_scale(theme, TypeScale::Caption)
                                .text_color(theme.colors.text_faint)
                                .child(cx.strings().text(step.state.key())),
                        ),
                )
                .children(reason)
                .children(
                    step.body
                        .map(|body| div().mt_token(theme, Space::Xs).child(body)),
                ),
        )
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Row)
                .parent(list.semantic_id())
                .text(step.title.clone())
                .value(step.state.as_str())
                .busy(running),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_publishes_its_own_name() {
        let names = [
            StepState::Pending.as_str(),
            StepState::Running.as_str(),
            StepState::Done.as_str(),
            StepState::Failed("boom".into()).as_str(),
            StepState::Skipped("nothing to do".into()).as_str(),
        ];
        let mut unique = names.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len());
    }

    #[test]
    fn only_a_state_the_host_explained_carries_a_reason() {
        assert!(StepState::Done.reason().is_none());
        assert_eq!(
            StepState::Skipped("nothing to do".into())
                .reason()
                .map(SharedString::to_string),
            Some("nothing to do".to_string())
        );
    }
}
