//! Work that moves through named stages the host already owns.

use gpui::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};

use crate::display::badge::Tone;
use crate::display::status::StatusDot;
use crate::foundation::{Ident, StyledExt};
use crate::state::{HasPhase, Phase};

/// Where one stage stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageStatus {
    Pending,
    Active,
    Done,
    Failed,
}

impl StageStatus {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }

    fn tone(self) -> Tone {
        match self {
            Self::Pending => Tone::Neutral,
            Self::Active => Tone::Accent,
            Self::Done => Tone::Success,
            Self::Failed => Tone::Danger,
        }
    }
}

impl HasPhase for StageStatus {
    fn phase(&self) -> Phase {
        match self {
            Self::Pending => Phase::Queued,
            Self::Active => Phase::Loading,
            Self::Done => Phase::Ready,
            Self::Failed => Phase::Error,
        }
    }
}

/// One host-owned stage of a longer piece of work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressStage {
    pub id: SharedString,
    pub label: SharedString,
    pub status: StageStatus,
}

impl ProgressStage {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        status: StageStatus,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status,
        }
    }
}

/// A run of named stages, all of them supplied by the caller.
#[derive(Debug, IntoElement)]
pub struct StageProgress {
    ident: Ident,
    stages: Vec<ProgressStage>,
}

impl StageProgress {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            stages: Vec::new(),
        }
    }

    pub fn stages(mut self, stages: impl IntoIterator<Item = ProgressStage>) -> Self {
        self.stages = stages.into_iter().collect();
        self
    }
}

impl RenderOnce for StageProgress {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let busy = self
            .stages
            .iter()
            .any(|stage| stage.status == StageStatus::Active);
        let failed = self
            .stages
            .iter()
            .any(|stage| stage.status == StageStatus::Failed);
        let value = if failed {
            "failed"
        } else if busy {
            "active"
        } else if self
            .stages
            .iter()
            .all(|stage| stage.status == StageStatus::Done)
        {
            "done"
        } else {
            "pending"
        };
        let done = self
            .stages
            .iter()
            .filter(|stage| stage.status == StageStatus::Done)
            .count() as f32;
        let total = self.stages.len() as f32;

        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .children(self.stages.iter().enumerate().map(|(index, stage)| {
                let ident = self.ident.child(stage.id.as_ref());
                let mut dot = StatusDot::new(stage.status.tone());
                if stage.status == StageStatus::Active {
                    dot = dot.busy(ident.child("busy"));
                }
                div()
                    .row()
                    .items_center()
                    .gap_token(&theme, Space::Sm)
                    .child(dot)
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Label)
                            .text_color(theme.colors.text)
                            .child(stage.label.clone()),
                    )
                    .when(index + 1 < self.stages.len(), |element| {
                        element.child(
                            div()
                                .flex_1()
                                .h(px(theme.borders.hairline))
                                .bg(theme.colors.divider)
                                .radius(&theme, Radius::Small),
                        )
                    })
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.semantic_id(), Role::Status)
                            .parent(self.ident.semantic_id())
                            .text(stage.label.clone())
                            .value(stage.status.name())
                            .busy(stage.status == StageStatus::Active),
                    )
            }))
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Progress)
                    .value(value)
                    .range(0.0, total, done)
                    .busy(busy),
            )
    }
}
