//! Standard presentation of caller-owned agent snapshots.
//!
//! A host should not have to rebuild an avatar halo, execution wording, a
//! virtualized roster, or a spawn hierarchy merely to show the model it
//! already owns. These components consume [`AgentSnapshot`] and
//! [`AgentRunSnapshot`] directly, and report selection through
//! [`AgentUiAction`]. They never update execution state or topology.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    AnyElement, App, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon as Glyph, icon as glyph};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, Elevation, Radius, SemanticBorder, SemanticWash, Space, Surface, TextTone,
    TypeScale,
};

use crate::agent::model::{
    AgentActivity, AgentExecutionState, AgentId, AgentModelIssue, AgentOutcome, AgentPresence,
    AgentRunSnapshot, AgentSnapshot, AgentTaskSnapshot, AgentUiAction, RunLinkKind, RunSubjectId,
    WaitReason,
};
use crate::data::{List, ListItem, Tree, TreeNode};
use crate::display::avatar::Avatar;
use crate::display::badge::{Badge, Tone};
use crate::display::status::{Callout, StatusDot};
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{FocusRing, Ident, Pressable, SelectedRow, StyledExt};
use crate::motion;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey, Strings};

type ActionHandler = Rc<dyn Fn(AgentUiAction, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(AgentId, bool, &mut Window, &mut App)>;

/// Optional caller-owned art for an agent identity.
///
/// Execution and presence never come from this value. Changing a portrait or
/// role tint therefore cannot accidentally change the state the UI reports.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentAppearance {
    id: AgentId,
    image: Option<SharedString>,
    tint: Option<Hsla>,
}

impl AgentAppearance {
    pub fn new(id: impl Into<AgentId>) -> Self {
        Self {
            id: id.into(),
            image: None,
            tint: None,
        }
    }

    pub fn image(mut self, image: impl Into<SharedString>) -> Self {
        self.image = Some(image.into());
        self
    }

    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }
}

/// A single truthful notice for malformed multi-agent snapshot structure.
///
/// Components use this before building identity-indexed rows or topology, so
/// duplicate and dangling facts are never silently collapsed into a different
/// run. A valid snapshot renders no notice.
#[derive(Debug, IntoElement)]
pub struct AgentRunIssues {
    ident: Ident,
    issues: Vec<AgentModelIssue>,
}

impl AgentRunIssues {
    pub fn new(ident: impl Into<Ident>, issues: impl IntoIterator<Item = AgentModelIssue>) -> Self {
        Self {
            ident: ident.into(),
            issues: issues.into_iter().collect(),
        }
    }

    pub fn from_run(ident: impl Into<Ident>, run: &AgentRunSnapshot) -> Self {
        Self::new(ident, run.issues())
    }
}

impl RenderOnce for AgentRunIssues {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.issues.is_empty() {
            return div().into_any_element();
        }
        let theme = cx.theme().clone();
        // One notice per problem. Joined into a sentence, four independent
        // faults read as one long complaint and none of them can be pointed
        // at on its own.
        let labels: Vec<_> = self
            .issues
            .iter()
            .map(|issue| issue_label(issue, cx.strings()))
            .collect();
        let notices: Vec<_> = labels
            .iter()
            .enumerate()
            .map(|(index, label)| {
                Callout::new(label.clone(), Tone::Danger)
                    .id(self.ident.child(format!("issue-{index}")))
            })
            .collect();
        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .children(notices)
            // The set of faults is itself one report — a host asks whether the
            // snapshot it handed over was well formed, not which notice is
            // third — so the group keeps the identity and each notice hangs
            // under it.
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Status).text(labels.join("; ")),
            )
            .into_any_element()
    }
}

/// A status-coloured agent identity with a separate presence mark.
#[derive(Debug, IntoElement)]
pub struct AgentAvatar {
    ident: Ident,
    agent: AgentSnapshot,
    image: Option<SharedString>,
    tint: Option<Hsla>,
    size: f32,
    mark_inset: f32,
    parent: Option<Ident>,
}

impl AgentAvatar {
    pub fn new(ident: impl Into<Ident>, agent: AgentSnapshot) -> Self {
        Self {
            ident: ident.into(),
            agent,
            image: None,
            tint: None,
            size: 32.0,
            mark_inset: 0.0,
            parent: None,
        }
    }

    /// How far the presence mark is held back from the trailing edge.
    ///
    /// Only a stack sets this, and it sets it to however much the next
    /// identity covers this one: a mark drawn at the edge of an avatar that
    /// something else is standing in front of cuts a bite out of that
    /// neighbour's ring, which reads as damage to the neighbour rather than
    /// as a fact about this agent.
    pub(crate) fn mark_inset(mut self, inset: f32) -> Self {
        if inset.is_finite() {
            self.mark_inset = inset.max(0.0);
        }
        self
    }

    pub fn image(mut self, image: impl Into<SharedString>) -> Self {
        self.image = Some(image.into());
        self
    }

    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        if size.is_finite() {
            self.size = size.max(16.0);
        }
        self
    }

    /// Establishes the semantic group this avatar belongs to.
    pub fn parent(mut self, parent: impl Into<Ident>) -> Self {
        self.parent = Some(parent.into());
        self
    }
}

impl RenderOnce for AgentAvatar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let tone = execution_tone(&self.agent.execution);
        let color = tone.mark_color(self.tint, &theme);
        let busy = self.agent.execution.busy();
        let presence = presence_tone(&self.agent.presence);

        let mut avatar = Avatar::new(self.agent.descriptor.name.clone()).size(self.size - 4.0);
        if let Some(image) = self.image {
            avatar = avatar.image(image);
        }
        if let Some(tint) = self.tint {
            avatar = avatar.tint(tint);
        }

        let disc = div()
            .size(px(self.size))
            .flex_none()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(theme.color_wash(color, SemanticWash::Strong))
            .border(px(theme.borders.thick))
            .border_color(theme.color_border(color, SemanticBorder::Target))
            .child(avatar);
        let disc = if busy {
            motion::breathe(disc, self.ident.child("activity").element_id(), &theme, cx)
        } else {
            disc.into_any_element()
        };

        // The presence mark is drawn here rather than taken from `StatusDot`
        // because that dot is one fixed size: at the smallest avatar it stood
        // proud of the disc it belongs to. Offline gets a filled mark in the
        // muted tone, so "not here" is legible instead of nearly the panel.
        let dot_edge = presence_dot_edge(self.size);
        let presence_color = match &self.agent.presence {
            AgentPresence::Offline => theme.colors.text_muted,
            AgentPresence::Unknown => theme.colors.hairline_strong,
            _ => presence.mark_color(None, &theme),
        };
        let mark = div()
            .absolute()
            .bottom_0()
            .when(direction.is_ltr(), |element| {
                element.right(px(self.mark_inset))
            })
            .when(direction.is_rtl(), |element| {
                element.left(px(self.mark_inset))
            })
            .size(px(presence_mark_edge(self.size, theme.borders.thick)))
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(theme.colors.panel)
            .child(
                div()
                    .size(px(dot_edge))
                    .flex_none()
                    .rounded_full()
                    .bg(presence_color),
            );
        let state_mark = execution_glyph(&self.agent.execution).map(|state_glyph| {
            glyph(state_glyph)
                .absolute()
                .top_0()
                .when(direction.is_ltr(), |element| element.left_0())
                .when(direction.is_rtl(), |element| element.right_0())
                .size(px((self.size * 0.34).clamp(9.0, 14.0)))
                .p(px(theme.space(Space::Xxs) / 2.0))
                .rounded_full()
                .bg(theme.colors.panel)
                .text_color(color)
        });

        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Image)
            .text(self.agent.descriptor.name.clone())
            .value(format!(
                "{}:{}",
                self.agent.presence.as_str(),
                self.agent.execution.as_str()
            ))
            .busy(busy);
        if let Some(parent) = self.parent {
            spec = spec.parent(parent.semantic_id());
        }

        div()
            .relative()
            .size(px(self.size))
            .flex_none()
            .child(disc)
            .child(mark)
            .children(state_mark)
            .semantic_in(cx, spec)
    }
}

/// The localized execution sentence and its non-colour status mark.
#[derive(Debug, IntoElement)]
pub struct AgentActivityLine {
    ident: Ident,
    execution: AgentExecutionState,
    tint: Option<Hsla>,
}

impl AgentActivityLine {
    pub fn new(ident: impl Into<Ident>, execution: AgentExecutionState) -> Self {
        Self {
            ident: ident.into(),
            execution,
            tint: None,
        }
    }

    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }
}

impl RenderOnce for AgentActivityLine {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let label = execution_label(&self.execution, cx.strings());
        let tone = execution_tone(&self.execution);
        let busy = self.execution.busy();
        let color = tone.mark_color(self.tint, &theme);

        // Waiting for approval and a refusal are both cautions, so a coloured
        // dot draws them the same. The mark is the execution's own glyph
        // wherever it has one, which is the only channel that separates two
        // states sharing a severity.
        let inner = match execution_glyph(&self.execution) {
            Some(state_glyph) => {
                crate::display::icon::paint(state_glyph, theme.control.sm.icon_size, color, false)
                    .into_any_element()
            }
            None => {
                let mut dot = StatusDot::new(tone);
                if let Some(tint) = self.tint {
                    dot = dot.tint(tint);
                }
                dot.into_any_element()
            }
        };
        let mark = div()
            .flex_none()
            .h(px(theme.typography.label.line_height))
            .row()
            .items_center()
            .child(inner);
        let mark = if busy {
            motion::breathe(mark, self.ident.child("busy").element_id(), &theme, cx)
        } else {
            mark.into_any_element()
        };

        div()
            .row()
            .items_start()
            .gap_token(&theme, Space::Sm)
            .type_scale(&theme, TypeScale::Label)
            .text_color(theme.colors.text_muted)
            .child(mark)
            .child(div().min_w_0().child(label.clone()))
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Status)
                    .text(label)
                    .value(tone.name())
                    .busy(busy),
            )
    }
}

/// A complete summary of one agent and its current caller-owned task.
#[derive(IntoElement)]
pub struct AgentCard {
    ident: Ident,
    agent: AgentSnapshot,
    task_label: Option<SharedString>,
    image: Option<SharedString>,
    tint: Option<Hsla>,
    selected: bool,
    on_action: Option<ActionHandler>,
}

impl std::fmt::Debug for AgentCard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentCard")
            .field("ident", &self.ident)
            .field("agent", &self.agent.descriptor.id)
            .field("selected", &self.selected)
            .field("has_handler", &self.on_action.is_some())
            .finish()
    }
}

impl AgentCard {
    pub fn new(ident: impl Into<Ident>, agent: AgentSnapshot) -> Self {
        Self {
            ident: ident.into(),
            agent,
            task_label: None,
            image: None,
            tint: None,
            selected: false,
            on_action: None,
        }
    }

    pub fn task_label(mut self, label: impl Into<SharedString>) -> Self {
        self.task_label = Some(label.into());
        self
    }

    pub fn image(mut self, image: impl Into<SharedString>) -> Self {
        self.image = Some(image.into());
        self
    }

    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_action(
        mut self,
        handler: impl Fn(AgentUiAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for AgentCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let agent_id = self.agent.descriptor.id.clone();
        let interactive = self.on_action.is_some();
        let mut avatar = AgentAvatar::new(self.ident.child("avatar"), self.agent.clone())
            .size(40.0)
            .parent(self.ident.clone());
        if let Some(image) = self.image {
            avatar = avatar.image(image);
        }
        if let Some(tint) = self.tint {
            avatar = avatar.tint(tint);
        }

        let identity = div()
            .min_w_0()
            .flex_1()
            .column()
            .child(
                div()
                    .type_scale(&theme, TypeScale::Body)
                    .text_tone(&theme, TextTone::Primary)
                    .child(self.agent.descriptor.name.clone()),
            )
            .children(self.agent.descriptor.role.clone().map(|role| {
                div()
                    .type_scale(&theme, TypeScale::Caption)
                    .text_tone(&theme, TextTone::Muted)
                    .child(role)
            }))
            .child(AgentActivityLine::new(
                self.ident.child("activity"),
                self.agent.execution.clone(),
            ));

        let body = div()
            .row_reading(direction)
            .items_center()
            .gap_token(&theme, Space::Md)
            .child(avatar)
            .child(identity)
            .children(self.task_label.map(|label| {
                Badge::new(label)
                    .tone(Tone::Neutral)
                    .id(self.ident.child("task"))
            }));

        let mut card = div()
            .id(self.ident.element_id())
            .w_full()
            .p_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .child(body)
            .selected_row(&theme, direction, self.selected);
        if let Some(handler) = self.on_action {
            let keyboard_handler = handler.clone();
            let keyboard_id = agent_id.clone();
            card = card
                .cursor_pointer()
                .tab_index(0)
                .pressable(cx)
                .focus_ring(&theme)
                .on_click(move |_, window, cx| {
                    handler(AgentUiAction::SelectAgent(agent_id.clone()), window, cx)
                })
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        keyboard_handler(
                            AgentUiAction::SelectAgent(keyboard_id.clone()),
                            window,
                            cx,
                        );
                        cx.stop_propagation();
                    }
                });
        }

        card.semantic_in(
            cx,
            NodeSpec::new(
                self.ident.semantic_id(),
                if interactive {
                    Role::Button
                } else {
                    Role::Group
                },
            )
            .text(self.agent.descriptor.name)
            .value(self.agent.execution.as_str())
            .selected(self.selected)
            .busy(self.agent.execution.busy()),
        )
    }
}

/// A virtualized list of agents with standard identity and activity rows.
#[derive(IntoElement)]
pub struct AgentRoster {
    ident: Ident,
    agents: Vec<AgentSnapshot>,
    tasks: Vec<AgentTaskSnapshot>,
    appearances: Vec<AgentAppearance>,
    selected: Option<AgentId>,
    visible_rows: Option<usize>,
    on_action: Option<ActionHandler>,
}

impl std::fmt::Debug for AgentRoster {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentRoster")
            .field("ident", &self.ident)
            .field("agents", &self.agents.len())
            .field("selected", &self.selected)
            .field("visible_rows", &self.visible_rows)
            .field("has_handler", &self.on_action.is_some())
            .finish()
    }
}

impl AgentRoster {
    pub fn new(ident: impl Into<Ident>, agents: impl IntoIterator<Item = AgentSnapshot>) -> Self {
        Self {
            ident: ident.into(),
            agents: agents.into_iter().collect(),
            tasks: Vec::new(),
            appearances: Vec::new(),
            selected: None,
            visible_rows: None,
            on_action: None,
        }
    }

    pub fn from_run(ident: impl Into<Ident>, run: &AgentRunSnapshot) -> Self {
        Self::new(ident, run.agents.clone()).tasks(run.tasks.clone())
    }

    pub fn tasks(mut self, tasks: impl IntoIterator<Item = AgentTaskSnapshot>) -> Self {
        self.tasks.extend(tasks);
        self
    }

    pub fn appearances(mut self, appearances: impl IntoIterator<Item = AgentAppearance>) -> Self {
        self.appearances.extend(appearances);
        self
    }

    pub fn selected(mut self, selected: impl Into<AgentId>) -> Self {
        self.selected = Some(selected.into());
        self
    }

    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = Some(rows.max(1));
        self
    }

    pub fn on_action(
        mut self,
        handler: impl Fn(AgentUiAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for AgentRoster {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let issues = duplicate_identity_issues(&self.agents, &self.tasks);
        if !issues.is_empty() {
            return AgentRunIssues::new(self.ident.child("issues"), issues).into_any_element();
        }
        let agents = Rc::new(self.agents);
        let tasks: Rc<HashMap<_, _>> = Rc::new(
            self.tasks
                .into_iter()
                .map(|task| (task.id, task.label))
                .collect(),
        );
        let appearances: Rc<HashMap<_, _>> = Rc::new(
            self.appearances
                .into_iter()
                .map(|appearance| (appearance.id.clone(), appearance))
                .collect(),
        );
        let row_ident = self.ident.clone();
        let rows = Rc::clone(&agents);
        let labels = Rc::clone(&tasks);
        let art = Rc::clone(&appearances);
        let mut list = List::new(self.ident.clone(), agents.len(), move |index, _, cx| {
            let theme = cx.theme().clone();
            let agent = rows[index].clone();
            let id = shared_agent_id(&agent.descriptor.id);
            let task_label = agent
                .current_task
                .as_ref()
                .and_then(|task| labels.get(task))
                .cloned();
            let row = roster_row(
                &row_ident,
                agent.clone(),
                task_label,
                art.get(&agent.descriptor.id),
                &theme,
            );
            ListItem::new(id, row).text(agent.descriptor.name)
        })
        .row_height(56.0);
        if let Some(rows) = self.visible_rows {
            list = list.visible_rows(rows);
        }
        if let Some(selected) = self.selected {
            list = list.selected(shared_agent_id(&selected));
        }
        if let Some(handler) = self.on_action {
            list = list.on_select(move |id, window, cx| {
                handler(AgentUiAction::SelectAgent(AgentId::new(id)), window, cx)
            });
        }
        list.into_any_element()
    }
}

fn roster_row(
    roster: &Ident,
    agent: AgentSnapshot,
    task_label: Option<SharedString>,
    appearance: Option<&AgentAppearance>,
    theme: &gpui_kit_theme::Theme,
) -> AnyElement {
    let ident = roster.child(agent.descriptor.id.as_str());
    let mut avatar = AgentAvatar::new(ident.child("avatar"), agent.clone())
        .size(36.0)
        .parent(ident.clone());
    if let Some(image) = appearance.and_then(|appearance| appearance.image.clone()) {
        avatar = avatar.image(image);
    }
    if let Some(tint) = appearance.and_then(|appearance| appearance.tint) {
        avatar = avatar.tint(tint);
    }
    div()
        .h_full()
        .row()
        .items_center()
        .gap_token(theme, Space::Sm)
        .child(avatar)
        .child(
            div()
                .min_w_0()
                .flex_1()
                .column()
                .child(
                    div()
                        .type_scale(theme, TypeScale::Body)
                        .text_tone(theme, TextTone::Primary)
                        .child(agent.descriptor.name),
                )
                .child(AgentActivityLine::new(
                    ident.child("activity"),
                    agent.execution,
                )),
        )
        .children(task_label.map(|label| {
            div()
                .flex_none()
                .row()
                .justify_end()
                .child(Badge::new(label).neutral())
        }))
        .into_any_element()
}

/// A compact overlapping group of agent identities with an overflow count.
#[derive(IntoElement)]
pub struct AgentGroup {
    ident: Ident,
    agents: Vec<AgentSnapshot>,
    appearances: Vec<AgentAppearance>,
    max_visible: usize,
    size: f32,
}

impl std::fmt::Debug for AgentGroup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentGroup")
            .field("ident", &self.ident)
            .field("agents", &self.agents.len())
            .field("max_visible", &self.max_visible)
            .finish()
    }
}

impl AgentGroup {
    pub fn new(ident: impl Into<Ident>, agents: impl IntoIterator<Item = AgentSnapshot>) -> Self {
        Self {
            ident: ident.into(),
            agents: agents.into_iter().collect(),
            appearances: Vec::new(),
            max_visible: 4,
            size: 30.0,
        }
    }

    pub fn max_visible(mut self, max_visible: usize) -> Self {
        self.max_visible = max_visible.max(1);
        self
    }

    pub fn appearances(mut self, appearances: impl IntoIterator<Item = AgentAppearance>) -> Self {
        self.appearances.extend(appearances);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        if size.is_finite() {
            self.size = size.max(16.0);
        }
        self
    }
}

impl RenderOnce for AgentGroup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let issues = duplicate_identity_issues(&self.agents, &[]);
        if !issues.is_empty() {
            return AgentRunIssues::new(self.ident.child("issues"), issues).into_any_element();
        }
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let count = self.agents.len();
        let visible = count.min(self.max_visible);
        let appearances: HashMap<_, _> = self
            .appearances
            .into_iter()
            .map(|appearance| (appearance.id.clone(), appearance))
            .collect();
        let overlap = group_overlap(self.size);
        let mut stack: Vec<AnyElement> = Vec::new();
        for (index, agent) in self.agents.into_iter().take(visible).enumerate() {
            let appearance = appearances.get(&agent.descriptor.id);
            let mut avatar =
                AgentAvatar::new(self.ident.child(agent.descriptor.id.as_str()), agent)
                    .size(self.size)
                    // The last identity in the stack has nothing in front of
                    // it, so its mark keeps the edge.
                    .mark_inset(if index + 1 < visible { overlap } else { 0.0 })
                    .parent(self.ident.clone());
            if let Some(image) = appearance.and_then(|appearance| appearance.image.clone()) {
                avatar = avatar.image(image);
            }
            if let Some(tint) = appearance.and_then(|appearance| appearance.tint) {
                avatar = avatar.tint(tint);
            }
            stack.push(
                div()
                    .flex_none()
                    .when(index > 0, |element| element.ms(direction, px(-overlap)))
                    .child(avatar)
                    .into_any_element(),
            );
        }
        // Painted back to front so each identity covers the *leading* edge of
        // the one behind it. In reading order the newer avatar landed on the
        // older one's presence mark and cut it in half.
        stack.reverse();
        let overflow = (count > visible).then(|| {
            div()
                .flex_none()
                .ms(direction, px(theme.space(Space::Xs)))
                .child(
                    Badge::new(cx.numbers().positive_count(count - visible))
                        .neutral()
                        .id(self.ident.child("overflow")),
                )
                .into_any_element()
        });
        let mut row = div().flex().items_center();
        row = if direction.is_ltr() {
            row.flex_row_reverse()
        } else {
            row.flex_row()
        };
        row.children(overflow)
            .children(stack)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .value(cx.numbers().count(count)),
            )
            .into_any_element()
    }
}

/// A caller-controlled hierarchy derived from `Spawn` links in a run.
#[derive(IntoElement)]
pub struct SubagentTree {
    ident: Ident,
    run: AgentRunSnapshot,
    expanded: Vec<AgentId>,
    selected: Option<AgentId>,
    visible_rows: Option<usize>,
    on_toggle: Option<ToggleHandler>,
    on_action: Option<ActionHandler>,
}

impl std::fmt::Debug for SubagentTree {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SubagentTree")
            .field("ident", &self.ident)
            .field("run", &self.run.id)
            .field("agents", &self.run.agents.len())
            .field("expanded", &self.expanded.len())
            .field("selected", &self.selected)
            .finish()
    }
}

impl SubagentTree {
    pub fn new(ident: impl Into<Ident>, run: AgentRunSnapshot) -> Self {
        Self {
            ident: ident.into(),
            run,
            expanded: Vec::new(),
            selected: None,
            visible_rows: None,
            on_toggle: None,
            on_action: None,
        }
    }

    pub fn expanded(mut self, expanded: impl IntoIterator<Item = AgentId>) -> Self {
        self.expanded.extend(expanded);
        self
    }

    pub fn selected(mut self, selected: impl Into<AgentId>) -> Self {
        self.selected = Some(selected.into());
        self
    }

    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = Some(rows.max(1));
        self
    }

    /// Reports the next caller-owned disclosure state without applying it.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(AgentId, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    pub fn on_action(
        mut self,
        handler: impl Fn(AgentUiAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SubagentTree {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let issues = self.run.issues();
        if !issues.is_empty() {
            return AgentRunIssues::new(self.ident.child("issues"), issues).into_any_element();
        }
        let nodes = topology_nodes(&self.run, cx.strings());
        let mut tree = Tree::new(self.ident.clone())
            .nodes(nodes)
            .expanded(self.expanded.iter().map(shared_agent_id));
        if let Some(selected) = self.selected {
            tree = tree.selected(shared_agent_id(&selected));
        }
        if let Some(rows) = self.visible_rows {
            tree = tree.visible_rows(rows);
        }
        if let Some(handler) = self.on_toggle {
            tree = tree.on_toggle(move |id, expanded, window, cx| {
                handler(AgentId::new(id), expanded, window, cx)
            });
        }
        if let Some(handler) = self.on_action {
            tree = tree.on_select(move |id, window, cx| {
                handler(AgentUiAction::SelectAgent(AgentId::new(id)), window, cx)
            });
        }
        tree.into_any_element()
    }
}

fn duplicate_identity_issues(
    agents: &[AgentSnapshot],
    tasks: &[AgentTaskSnapshot],
) -> Vec<AgentModelIssue> {
    let mut issues = Vec::new();
    let mut agent_ids = HashSet::new();
    for agent in agents {
        if !agent_ids.insert(agent.descriptor.id.clone()) {
            issues.push(AgentModelIssue::DuplicateAgent(agent.descriptor.id.clone()));
        }
    }
    let mut task_ids = HashSet::new();
    for task in tasks {
        if !task_ids.insert(task.id.clone()) {
            issues.push(AgentModelIssue::DuplicateTask(task.id.clone()));
        }
    }
    issues
}

fn topology_nodes(run: &AgentRunSnapshot, strings: &Strings) -> Vec<TreeNode> {
    let agents: HashMap<_, _> = run
        .agents
        .iter()
        .map(|agent| (agent.descriptor.id.clone(), agent))
        .collect();
    let mut children: HashMap<AgentId, Vec<AgentId>> = HashMap::new();
    let mut has_parent = HashSet::new();
    for link in &run.links {
        if link.kind != RunLinkKind::Spawn {
            continue;
        }
        if let (RunSubjectId::Agent(parent), RunSubjectId::Agent(child)) = (&link.from, &link.to) {
            children
                .entry(parent.clone())
                .or_default()
                .push(child.clone());
            has_parent.insert(child.clone());
        }
    }

    let mut roots = Vec::new();
    if agents.contains_key(&run.root) {
        roots.push(run.root.clone());
    }
    roots.extend(
        run.agents
            .iter()
            .map(|agent| agent.descriptor.id.clone())
            .filter(|id| id != &run.root && !has_parent.contains(id)),
    );

    let mut visited = HashSet::new();
    let mut nodes = roots
        .into_iter()
        .filter_map(|root| agent_tree_node(&root, &agents, &children, strings, &mut visited))
        .collect::<Vec<_>>();
    // A cycle or a parent missing from this snapshot must not hide an agent.
    for agent in &run.agents {
        if let Some(node) = agent_tree_node(
            &agent.descriptor.id,
            &agents,
            &children,
            strings,
            &mut visited,
        ) {
            nodes.push(node);
        }
    }
    nodes
}

fn agent_tree_node(
    id: &AgentId,
    agents: &HashMap<AgentId, &AgentSnapshot>,
    children: &HashMap<AgentId, Vec<AgentId>>,
    strings: &Strings,
    visited: &mut HashSet<AgentId>,
) -> Option<TreeNode> {
    if !visited.insert(id.clone()) {
        return None;
    }
    let agent = agents.get(id)?;
    let label = SharedString::from(format!(
        "{} — {}",
        agent.descriptor.name,
        execution_label(&agent.execution, strings)
    ));
    let mut node = TreeNode::new(shared_agent_id(id), label).icon(Glyph::GitBranch);
    if let Some(descendants) = children.get(id) {
        for child in descendants {
            if let Some(child) = agent_tree_node(child, agents, children, strings, visited) {
                node = node.child(child);
            }
        }
    }
    Some(node)
}

fn shared_agent_id(id: &AgentId) -> SharedString {
    SharedString::from(id.as_str().to_string())
}

/// The presence dot's own edge on an avatar of `size`.
///
/// It scales rather than holding a floor with an area of its own: the dot sits
/// in a cut-out two borders wider than itself, and at the smallest avatar a
/// six-pixel floor put that cut-out over the middle of the disc, so the mark
/// covered the initial it was a fact about.
fn presence_dot_edge(size: f32) -> f32 {
    (size * 0.24).clamp(3.5, 10.0)
}

/// The whole presence mark, cut-out included.
fn presence_mark_edge(size: f32, thick: f32) -> f32 {
    presence_dot_edge(size) + thick * 2.0
}

/// How much of one identity in a stack the next one covers.
fn group_overlap(size: f32) -> f32 {
    size * 0.28
}

fn presence_tone(presence: &AgentPresence) -> Tone {
    match presence {
        AgentPresence::Unknown | AgentPresence::Offline => Tone::Neutral,
        AgentPresence::Online => Tone::Success,
        AgentPresence::Away => Tone::Warning,
        AgentPresence::Unavailable(_) => Tone::Danger,
    }
}

fn execution_tone(execution: &AgentExecutionState) -> Tone {
    match execution {
        AgentExecutionState::Idle => Tone::Neutral,
        AgentExecutionState::Queued | AgentExecutionState::Starting => Tone::Info,
        AgentExecutionState::Active(_) => Tone::Accent,
        AgentExecutionState::Waiting(_) | AgentExecutionState::Cancelling => Tone::Warning,
        AgentExecutionState::Blocked(_) => Tone::Danger,
        AgentExecutionState::Completed(outcome) => match outcome {
            AgentOutcome::Succeeded => Tone::Success,
            AgentOutcome::Partial(_) => Tone::Warning,
            AgentOutcome::Failed(_) | AgentOutcome::TimedOut(_) => Tone::Danger,
            AgentOutcome::Refused(_) => Tone::Warning,
            AgentOutcome::Cancelled => Tone::Neutral,
        },
        AgentExecutionState::Unavailable(_) => Tone::Neutral,
    }
}

fn execution_glyph(execution: &AgentExecutionState) -> Option<Glyph> {
    match execution {
        AgentExecutionState::Idle => None,
        AgentExecutionState::Queued
        | AgentExecutionState::Starting
        | AgentExecutionState::Active(_) => Some(Glyph::Refresh),
        AgentExecutionState::Waiting(_) => Some(Glyph::Info),
        AgentExecutionState::Blocked(_) => Some(Glyph::Danger),
        AgentExecutionState::Cancelling => Some(Glyph::Close),
        AgentExecutionState::Completed(outcome) => Some(match outcome {
            AgentOutcome::Succeeded => Glyph::Check,
            AgentOutcome::Partial(_) | AgentOutcome::TimedOut(_) => Glyph::Danger,
            AgentOutcome::Failed(_) | AgentOutcome::Cancelled => Glyph::Close,
            // Refusal is a decision not to run, which is the one thing the
            // stop square does not say: it is the mark a running thing is
            // stopped with, and a run that never started wore it too.
            AgentOutcome::Refused(_) => Glyph::Forbidden,
        }),
        AgentExecutionState::Unavailable(_) => Some(Glyph::CloseCircle),
    }
}

pub(crate) fn execution_label(execution: &AgentExecutionState, strings: &Strings) -> SharedString {
    match execution {
        AgentExecutionState::Idle => strings.text(StringKey::AgentIdle),
        AgentExecutionState::Queued => strings.text(StringKey::AgentQueued),
        AgentExecutionState::Starting => strings.text(StringKey::AgentStarting),
        AgentExecutionState::Active(activity) => activity_label(activity, strings),
        AgentExecutionState::Waiting(reason) => wait_label(reason, strings),
        AgentExecutionState::Blocked(reason) => {
            strings.format(StringKey::AgentBlockedBecause, &[reason.as_ref()])
        }
        AgentExecutionState::Cancelling => strings.text(StringKey::AgentCancelling),
        AgentExecutionState::Completed(outcome) => match outcome {
            AgentOutcome::Succeeded => strings.text(StringKey::AgentSucceeded),
            AgentOutcome::Partial(reason) => {
                strings.format(StringKey::AgentPartialBecause, &[reason.as_ref()])
            }
            AgentOutcome::Failed(reason) => {
                strings.format(StringKey::AgentFailedBecause, &[reason.as_ref()])
            }
            AgentOutcome::Refused(reason) => {
                strings.format(StringKey::AgentRefusedBecause, &[reason.as_ref()])
            }
            AgentOutcome::Cancelled => strings.text(StringKey::AgentCancelled),
            AgentOutcome::TimedOut(reason) => {
                strings.format(StringKey::AgentTimedOutBecause, &[reason.as_ref()])
            }
        },
        AgentExecutionState::Unavailable(reason) => {
            strings.format(StringKey::AgentUnavailableBecause, &[reason.as_ref()])
        }
    }
}

fn activity_label(activity: &AgentActivity, strings: &Strings) -> SharedString {
    match activity {
        AgentActivity::Idle => strings.text(StringKey::AgentIdle),
        AgentActivity::Planning => strings.text(StringKey::AgentPlanning),
        AgentActivity::Thinking => strings.text(StringKey::AgentReasoningThinking),
        AgentActivity::UsingTool(tool) => {
            strings.format(StringKey::AgentUsingTool, &[tool.as_ref()])
        }
        AgentActivity::Speaking => strings.text(StringKey::AgentSpeaking),
        AgentActivity::Aggregating => strings.text(StringKey::AgentAggregating),
        AgentActivity::Custom(label) => label.clone(),
    }
}

fn wait_label(reason: &WaitReason, strings: &Strings) -> SharedString {
    match reason {
        WaitReason::UserInput => strings.text(StringKey::AgentWaitingInput),
        WaitReason::Approval => strings.text(StringKey::AgentPendingApproval),
        WaitReason::Dependency(_) => strings.text(StringKey::AgentWaitingDependency),
        WaitReason::RemoteAgent(_) => strings.text(StringKey::AgentWaitingAgent),
        WaitReason::RateLimit => strings.text(StringKey::AgentWaitingRateLimit),
        WaitReason::Custom(label) => label.clone(),
    }
}

fn issue_label(issue: &AgentModelIssue, strings: &Strings) -> SharedString {
    match issue {
        AgentModelIssue::MissingRoot(root) => {
            strings.format(StringKey::AgentIssueMissingRoot, &[root.as_str()])
        }
        AgentModelIssue::DuplicateAgent(agent) => {
            strings.format(StringKey::AgentIssueDuplicateAgent, &[agent.as_str()])
        }
        AgentModelIssue::DuplicateTask(task) => {
            strings.format(StringKey::AgentIssueDuplicateTask, &[task.as_str()])
        }
        AgentModelIssue::DuplicateLink(link) => {
            strings.format(StringKey::AgentIssueDuplicateLink, &[link.as_str()])
        }
        AgentModelIssue::MissingTaskOwner { task, owner } => strings.format(
            StringKey::AgentIssueMissingTaskOwner,
            &[task.as_str(), owner.as_str()],
        ),
        AgentModelIssue::MissingLinkEndpoint { link, endpoint } => strings.format(
            StringKey::AgentIssueMissingLinkEndpoint,
            &[link.as_str(), endpoint.as_str()],
        ),
        AgentModelIssue::SelfLink(link) => {
            strings.format(StringKey::AgentIssueSelfLink, &[link.as_str()])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::model::{AgentDescriptor, RunLink, RunLinkId};

    fn agent(id: &'static str) -> AgentSnapshot {
        AgentSnapshot::new(AgentDescriptor::new(id, id))
    }

    #[test]
    fn spawn_links_form_a_tree_and_non_spawn_links_do_not_reparent() {
        let run = AgentRunSnapshot::new("run", "root")
            .agents([agent("root"), agent("child"), agent("reporter")])
            .links([
                RunLink::new(
                    RunLinkId::new("spawn"),
                    RunSubjectId::Agent("root".into()),
                    RunSubjectId::Agent("child".into()),
                    RunLinkKind::Spawn,
                ),
                RunLink::new(
                    RunLinkId::new("report"),
                    RunSubjectId::Agent("child".into()),
                    RunSubjectId::Agent("reporter".into()),
                    RunLinkKind::Report,
                ),
            ]);

        let nodes = topology_nodes(&run, &Strings::default());
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn every_execution_state_has_visible_wording() {
        let strings = Strings::default();
        for state in [
            AgentExecutionState::Idle,
            AgentExecutionState::Queued,
            AgentExecutionState::Starting,
            AgentExecutionState::Waiting(WaitReason::Approval),
            AgentExecutionState::Blocked("blocked".into()),
            AgentExecutionState::Cancelling,
            AgentExecutionState::Completed(AgentOutcome::Cancelled),
            AgentExecutionState::Unavailable("offline".into()),
        ] {
            assert!(!execution_label(&state, &strings).trim().is_empty());
        }
    }

    #[test]
    fn host_failure_and_refusal_reasons_are_not_summarized_away() {
        let strings = Strings::default();
        for (state, reason) in [
            (
                AgentExecutionState::Blocked("policy blocked this run".into()),
                "policy blocked this run",
            ),
            (
                AgentExecutionState::Completed(AgentOutcome::Failed("tests failed".into())),
                "tests failed",
            ),
            (
                AgentExecutionState::Completed(AgentOutcome::Refused(
                    "approval was refused".into(),
                )),
                "approval was refused",
            ),
            (
                AgentExecutionState::Unavailable("runner is offline".into()),
                "runner is offline",
            ),
        ] {
            assert!(execution_label(&state, &strings).contains(reason));
        }
    }

    #[test]
    fn the_presence_mark_never_reaches_the_middle_of_the_avatar() {
        // The initial, or the portrait, is in the middle. A mark whose
        // cut-out reaches the centre of the disc hides the identity it is a
        // fact about, which is what a fixed floor did at the smallest size.
        for size in [16.0_f32, 20.0, 24.0, 32.0, 44.0, 88.0] {
            assert!(
                presence_mark_edge(size, 2.0) < size / 2.0,
                "the mark covers the middle at {size}"
            );
        }
    }

    #[test]
    fn a_stacked_identity_keeps_its_mark_off_the_next_one() {
        // In a stack every identity but the last is covered by the one in
        // front of it, so its mark is held back by exactly that much and ends
        // where the neighbour begins.
        for size in [16.0_f32, 20.0, 30.0, 44.0] {
            let inset = group_overlap(size);
            assert!(
                presence_mark_edge(size, 2.0) + inset <= size,
                "the mark leaves its own avatar at {size}"
            );
        }
    }

    #[test]
    fn refusal_and_stopping_are_different_marks() {
        // A refusal is a decision not to run; the stop square is what a
        // running thing is stopped with. One glyph for both said the run had
        // been halted when nothing had started.
        assert_eq!(
            execution_glyph(&AgentExecutionState::Completed(AgentOutcome::Refused(
                "policy".into()
            ))),
            Some(Glyph::Forbidden)
        );
        assert_ne!(
            execution_glyph(&AgentExecutionState::Completed(AgentOutcome::Refused(
                "policy".into()
            ))),
            execution_glyph(&AgentExecutionState::Completed(AgentOutcome::Failed(
                "tests".into()
            )))
        );
    }
}
