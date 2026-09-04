//! Work that moves through named stages the host already owns.

use gpui::{
    App, Div, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use gpui_kit_assets::Icon as Glyph;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TextTone, Theme, TypeScale};

use crate::display::badge::Tone;
use crate::display::icon::paint;
use crate::foundation::{Ident, StyledExt};
use crate::overlay::tooltip::Tooltipped;
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// The diameter of a stage node, and therefore the width of the rail the
/// connectors run down.
const NODE: f32 = 22.0;

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

    /// The words the node publishes as hover help.
    fn wording(self) -> StringKey {
        match self {
            Self::Pending => StringKey::StagePending,
            Self::Active => StringKey::StageActive,
            Self::Done => StringKey::StageDone,
            Self::Failed => StringKey::StageFailed,
        }
    }

    /// What the node draws instead of its number. A stage that is over says
    /// how it ended; one that is not yet keeps its place in the count.
    fn glyph(self) -> Option<Glyph> {
        match self {
            Self::Done => Some(Glyph::Check),
            Self::Failed => Some(Glyph::Close),
            Self::Pending | Self::Active => None,
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

        let last = self.stages.len().saturating_sub(1);
        div()
            .column()
            .w_full()
            .children(self.stages.iter().enumerate().map(|(index, stage)| {
                let ident = self.ident.child(stage.id.as_ref());
                let status = stage.status;
                let color = status.tone().color(&theme);
                let reached = matches!(status, StageStatus::Done | StageStatus::Failed);
                // A connector says the run got this far, so the segment above
                // a node is the previous stage's claim and the segment below
                // is this one's. Nothing about the rail is decorative: a grey
                // segment is work that has not happened.
                let above = (index > 0)
                    .then(|| self.stages[index - 1].status)
                    .map(|previous| {
                        connector(
                            &theme,
                            matches!(previous, StageStatus::Done | StageStatus::Failed),
                        )
                        .top_0()
                        .h(px(NODE / 2.0))
                    });
                let below = (index < last)
                    .then(|| connector(&theme, reached).top(px(NODE / 2.0)).bottom_0());
                let numeral = cx.numbers().count(index + 1);
                let wording = cx.strings().text(status.wording());
                let mark_ident = ident.child("mark");

                div()
                    .row()
                    .items_stretch()
                    .w_full()
                    .gap_token(&theme, Space::Sm)
                    .child(
                        div()
                            .id(mark_ident.element_id())
                            .relative()
                            .flex_none()
                            .w(px(NODE))
                            .children(above)
                            .children(below)
                            .child(node(&theme, status, color, numeral))
                            .tip(mark_ident, wording),
                    )
                    .child(
                        div()
                            .column()
                            .flex_1()
                            .min_w_0()
                            .pb(px(theme.space(Space::Md)))
                            .child(
                                div()
                                    .type_scale(
                                        &theme,
                                        if status == StageStatus::Active {
                                            TypeScale::Strong
                                        } else {
                                            TypeScale::Label
                                        },
                                    )
                                    .text_tone(
                                        &theme,
                                        if status == StageStatus::Pending {
                                            TextTone::Muted
                                        } else {
                                            TextTone::Primary
                                        },
                                    )
                                    .child(stage.label.clone()),
                            ),
                    )
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

/// One segment of the rail, drawn behind the nodes it joins.
fn connector(theme: &Theme, reached: bool) -> Div {
    div()
        .absolute()
        .left(px((NODE - theme.borders.thick) / 2.0))
        .w(px(theme.borders.thick))
        .rounded_full()
        .bg(if reached {
            theme.colors.success.opacity(theme.opacity.muted)
        } else {
            theme.colors.divider.opacity(theme.opacity.muted)
        })
}

/// The stage marker: its number until it has an outcome to report instead.
fn node(theme: &Theme, status: StageStatus, color: Hsla, numeral: SharedString) -> Div {
    let filled = matches!(status, StageStatus::Done | StageStatus::Failed);
    let mark = div()
        .relative()
        .flex_none()
        .size(px(NODE))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(if filled {
            color
        } else if status == StageStatus::Active {
            color.opacity(theme.effects.semantic_wash_strong_alpha)
        } else {
            theme.colors.sunken
        });
    match status.glyph() {
        Some(glyph) => mark.child(paint(
            glyph,
            theme.control.xs.icon_size,
            theme.colors.text_on_accent,
            false,
        )),
        None => mark.child(
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(match status {
                    StageStatus::Pending => theme.colors.text_faint,
                    _ => color,
                })
                .child(numeral),
        ),
    }
}
