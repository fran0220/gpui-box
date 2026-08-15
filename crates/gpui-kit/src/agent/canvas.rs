//! A run topology projected onto the generic node canvas.
//!
//! Hosts provide [`AgentRunSnapshot`] facts rather than rebuilding graph
//! nodes, relationship labels, execution-state mappings, or layered positions.
//! Selection, viewport, and optional arranged positions remain controlled:
//! this component only reports proposed changes through [`AgentRunCanvasEvent`].

use std::{collections::HashMap, rc::Rc};

use gpui::{App, IntoElement, Point, RenderOnce, SharedString, Window, point};

use crate::canvas::{
    GraphEdge, GraphInteraction, GraphNode, GraphViewport, NodeGraph, NodeGraphEvent, NodeState,
};
use crate::foundation::direction::ActiveDirection;
use crate::foundation::{Ident, Selectable};
use crate::strings::{ActiveStrings, StringKey, Strings};

use super::model::{
    AgentExecutionState, AgentOutcome, AgentRunSnapshot, RunLink, RunLinkKind, RunSubjectId,
};
use super::presentation::{AgentRunIssues, execution_label};

const NODE_WIDTH: f32 = 216.0;
// Relationship wording lives between columns rather than over the cards. The
// gap is intentionally wider than NodeGraph's generic examples because agent
// relations such as "Aggregation" and "Delegation" must stay readable.
const COLUMN_GAP: f32 = 160.0;
const ROW_GAP: f32 = 52.0;
const ESTIMATED_NODE_HEIGHT: f32 = 132.0;
const INSET: f32 = 40.0;

type EventHandler = Rc<dyn Fn(&AgentRunCanvasEvent, &mut Window, &mut App)>;

/// The reading axis used by the built-in deterministic run layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentRunLayout {
    /// Roots precede dependants in reading order. Right-to-left locales mirror
    /// the columns without changing caller-owned identities or relationships.
    #[default]
    LayeredHorizontal,
    /// Roots sit above dependants. Nodes within a layer follow snapshot order.
    LayeredVertical,
}

/// A controlled change proposed by [`AgentRunCanvas`].
#[derive(Debug, Clone, PartialEq)]
pub enum AgentRunCanvasEvent {
    /// Proposes the complete caller-owned subject selection.
    SelectionChanged(Vec<RunSubjectId>),
    /// Proposes a new pan or zoom value.
    ViewportChanged(GraphViewport),
    /// Proposes an arranged world-space position for one subject.
    PositionChanged {
        subject: RunSubjectId,
        position: Point<f32>,
    },
}

/// A typed, read-mostly view of an [`AgentRunSnapshot`] on a node canvas.
///
/// The default inspection mode supports pan, zoom, and selection but never
/// installs topology editing or deletion. [`AgentRunCanvas::arrangeable`]
/// additionally reports node moves while still keeping links immutable.
#[derive(IntoElement)]
pub struct AgentRunCanvas {
    ident: Ident,
    run: AgentRunSnapshot,
    layout: AgentRunLayout,
    positions: Vec<(RunSubjectId, Point<f32>)>,
    selected: Vec<RunSubjectId>,
    viewport: GraphViewport,
    arrangeable: bool,
    on_event: Option<EventHandler>,
}

impl std::fmt::Debug for AgentRunCanvas {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentRunCanvas")
            .field("ident", &self.ident)
            .field("run", &self.run.id)
            .field("layout", &self.layout)
            .field("arrangeable", &self.arrangeable)
            .finish_non_exhaustive()
    }
}

impl AgentRunCanvas {
    pub fn new(ident: impl Into<Ident>, run: AgentRunSnapshot) -> Self {
        Self {
            ident: ident.into(),
            run,
            layout: AgentRunLayout::default(),
            positions: Vec::new(),
            selected: Vec::new(),
            viewport: GraphViewport::default(),
            arrangeable: false,
            on_event: None,
        }
    }

    /// Chooses one of the built-in deterministic layered arrangements.
    pub fn layout(mut self, layout: AgentRunLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Overrides one built-in position with caller-owned world coordinates.
    /// A later override for the same subject wins.
    pub fn position(mut self, subject: RunSubjectId, x: f32, y: f32) -> Self {
        if x.is_finite() && y.is_finite() {
            self.positions.push((subject, point(x, y)));
        }
        self
    }

    /// Supplies the complete caller-owned subject selection.
    pub fn selected(mut self, selected: impl IntoIterator<Item = RunSubjectId>) -> Self {
        self.selected = selected.into_iter().collect();
        self
    }

    /// Supplies the caller-owned pan and zoom value.
    pub fn viewport(mut self, viewport: GraphViewport) -> Self {
        self.viewport = viewport;
        self
    }

    /// Allows position proposals without enabling deletion or topology edits.
    pub fn arrangeable(mut self, arrangeable: bool) -> Self {
        self.arrangeable = arrangeable;
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(&AgentRunCanvasEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for AgentRunCanvas {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let issues = self.run.issues();
        if !issues.is_empty() {
            return AgentRunIssues::new(self.ident.child("issues"), issues).into_any_element();
        }

        let strings = cx.strings().clone();
        let subjects = run_subjects(&self.run);
        let positions = layered_positions(
            &subjects,
            &self.run.links,
            self.layout,
            cx.layout_direction().is_rtl(),
        );
        let ids: Vec<(SharedString, RunSubjectId)> = subjects
            .iter()
            .map(|subject| (subject_ident(&self.ident, subject), subject.clone()))
            .collect();

        let mut graph = NodeGraph::new(self.ident.clone())
            .viewport(self.viewport)
            .interaction(if self.arrangeable {
                GraphInteraction::Arrange
            } else {
                GraphInteraction::Inspect
            });

        for (index, subject) in subjects.iter().enumerate() {
            let id = &ids[index].0;
            let mut node = subject_node(id.clone(), subject, &self.run, &strings)
                .selected(self.selected.contains(subject));
            if matches!(subject, RunSubjectId::Agent(agent) if agent == &self.run.root)
                && let Some(aggregation) = &self.run.aggregation
            {
                node = node.metric(
                    strings.text(StringKey::AgentRunCanvasResults),
                    format!("{} / {}", aggregation.received, aggregation.expected),
                );
                if aggregation.conflicts > 0 {
                    node = node.metric(
                        strings.text(StringKey::AgentRunCanvasConflicts),
                        aggregation.conflicts.to_string(),
                    );
                }
            }
            let position = self
                .positions
                .iter()
                .rev()
                .find_map(|(candidate, position)| (candidate == subject).then_some(*position))
                .unwrap_or(positions[index]);
            graph = graph.node(node, position.x, position.y);
        }

        for link in &self.run.links {
            let from = subject_ident(&self.ident, &link.from);
            let to = subject_ident(&self.ident, &link.to);
            let mut edge = GraphEdge::new(from, to)
                .id(link_ident(&self.ident, link))
                .label(link_label(link, &strings));
            if link.kind == RunLinkKind::Retry {
                edge = edge.feedback();
            }
            graph = graph.edge(edge);
        }

        if let Some(report) = self.on_event {
            graph = graph.on_event(move |event, window, cx| match event {
                NodeGraphEvent::ViewportChanged(viewport) => {
                    report(&AgentRunCanvasEvent::ViewportChanged(*viewport), window, cx);
                }
                NodeGraphEvent::SelectionChanged { ids: selected } => {
                    let subjects = selected
                        .iter()
                        .filter_map(|selected| {
                            ids.iter()
                                .find_map(|(id, subject)| (id == selected).then(|| subject.clone()))
                        })
                        .collect();
                    report(&AgentRunCanvasEvent::SelectionChanged(subjects), window, cx);
                }
                NodeGraphEvent::NodeMoved { id, position } => {
                    if let Some(subject) = ids
                        .iter()
                        .find_map(|(candidate, subject)| (candidate == id).then(|| subject.clone()))
                    {
                        report(
                            &AgentRunCanvasEvent::PositionChanged {
                                subject,
                                position: *position,
                            },
                            window,
                            cx,
                        );
                    }
                }
                NodeGraphEvent::NodeDeleted { .. }
                | NodeGraphEvent::ConnectionRequested { .. }
                | NodeGraphEvent::DisconnectRequested { .. } => {}
            });
        }

        graph.into_any_element()
    }
}

fn run_subjects(run: &AgentRunSnapshot) -> Vec<RunSubjectId> {
    let mut subjects: Vec<RunSubjectId> = run
        .agents
        .iter()
        .map(|agent| RunSubjectId::Agent(agent.descriptor.id.clone()))
        .chain(
            run.tasks
                .iter()
                .map(|task| RunSubjectId::Task(task.id.clone())),
        )
        .collect();
    for endpoint in run.links.iter().flat_map(|link| [&link.from, &link.to]) {
        if matches!(endpoint, RunSubjectId::Invocation(_)) && !subjects.contains(endpoint) {
            subjects.push(endpoint.clone());
        }
    }
    subjects
}

fn subject_ident(ident: &Ident, subject: &RunSubjectId) -> SharedString {
    let kind = match subject {
        RunSubjectId::Agent(_) => "agent",
        RunSubjectId::Task(_) => "task",
        RunSubjectId::Invocation(_) => "invocation",
    };
    ident.child(kind).child(subject.as_str()).semantic_id()
}

fn link_ident(ident: &Ident, link: &RunLink) -> SharedString {
    ident.child("link").child(link.id.as_str()).semantic_id()
}

fn subject_node(
    semantic_id: SharedString,
    subject: &RunSubjectId,
    run: &AgentRunSnapshot,
    strings: &Strings,
) -> GraphNode {
    match subject {
        RunSubjectId::Agent(agent_id) => {
            let Some(agent) = run
                .agents
                .iter()
                .find(|agent| &agent.descriptor.id == agent_id)
            else {
                unreachable!()
            };
            GraphNode::new(semantic_id, agent.descriptor.name.clone())
                .width(NODE_WIDTH)
                .state(node_state(&agent.execution))
                .action(execution_label(&agent.execution, strings))
                .metric(
                    strings.text(StringKey::AgentRunCanvasKind),
                    strings.text(StringKey::AgentRunCanvasAgent),
                )
        }
        RunSubjectId::Task(task_id) => {
            let Some(task) = run.tasks.iter().find(|task| &task.id == task_id) else {
                unreachable!()
            };
            GraphNode::new(semantic_id, task.label.clone())
                .width(NODE_WIDTH)
                .state(node_state(&task.execution))
                .action(execution_label(&task.execution, strings))
                .metric(
                    strings.text(StringKey::AgentRunCanvasKind),
                    strings.text(StringKey::AgentRunCanvasTask),
                )
        }
        RunSubjectId::Invocation(invocation) => GraphNode::new(
            semantic_id,
            strings.format(StringKey::AgentRunCanvasInvocation, &[invocation.as_str()]),
        )
        .width(NODE_WIDTH)
        .state(NodeState::Pending)
        .action(strings.text(StringKey::AgentRunCanvasInvocationPending))
        .metric(
            strings.text(StringKey::AgentRunCanvasKind),
            strings.text(StringKey::AgentRunCanvasInvocationKind),
        ),
    }
}

fn node_state(execution: &AgentExecutionState) -> NodeState {
    match execution {
        AgentExecutionState::Idle => NodeState::Idle,
        AgentExecutionState::Queued => NodeState::Queued,
        AgentExecutionState::Starting => NodeState::Starting,
        AgentExecutionState::Active(_) => NodeState::Running,
        AgentExecutionState::Waiting(_) => NodeState::Waiting,
        AgentExecutionState::Blocked(_) => NodeState::Blocked,
        AgentExecutionState::Cancelling => NodeState::Cancelling,
        AgentExecutionState::Completed(outcome) => match outcome {
            AgentOutcome::Succeeded => NodeState::Succeeded,
            AgentOutcome::Partial(_) => NodeState::Partial,
            AgentOutcome::Failed(_) => NodeState::Failed,
            AgentOutcome::Refused(_) => NodeState::Refused,
            AgentOutcome::Cancelled => NodeState::Cancelled,
            AgentOutcome::TimedOut(_) => NodeState::TimedOut,
        },
        AgentExecutionState::Unavailable(_) => NodeState::Unavailable,
    }
}

fn link_label(link: &RunLink, strings: &Strings) -> SharedString {
    let relation = strings.text(match link.kind {
        RunLinkKind::Spawn => StringKey::AgentRunCanvasSpawn,
        RunLinkKind::Delegation => StringKey::AgentRunCanvasDelegation,
        RunLinkKind::Dependency => StringKey::AgentRunCanvasDependency,
        RunLinkKind::Handoff => StringKey::AgentRunCanvasHandoff,
        RunLinkKind::Report => StringKey::AgentRunCanvasReport,
        RunLinkKind::Aggregation => StringKey::AgentRunCanvasAggregation,
        RunLinkKind::Retry => StringKey::AgentRunCanvasRetry,
    });
    match &link.label {
        Some(label) => strings.format(
            StringKey::AgentRunCanvasRelationLabel,
            &[relation.as_ref(), label.as_ref()],
        ),
        None => relation,
    }
}

fn contributes_to_layers(kind: RunLinkKind) -> bool {
    !matches!(kind, RunLinkKind::Report | RunLinkKind::Retry)
}

fn layered_positions(
    subjects: &[RunSubjectId],
    links: &[RunLink],
    layout: AgentRunLayout,
    rtl: bool,
) -> Vec<Point<f32>> {
    let indices: HashMap<RunSubjectId, usize> = subjects
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, subject)| (subject, index))
        .collect();
    let mut outgoing = vec![Vec::new(); subjects.len()];
    let mut incoming = vec![Vec::new(); subjects.len()];
    let mut indegree = vec![0usize; subjects.len()];
    for link in links.iter().filter(|link| contributes_to_layers(link.kind)) {
        let (Some(&from), Some(&to)) = (indices.get(&link.from), indices.get(&link.to)) else {
            continue;
        };
        outgoing[from].push(to);
        incoming[to].push(from);
        indegree[to] += 1;
    }

    let mut depth = vec![0usize; subjects.len()];
    let mut placed = vec![false; subjects.len()];
    for _ in 0..subjects.len() {
        let next = (0..subjects.len())
            .find(|&index| !placed[index] && indegree[index] == 0)
            // A cycle has no zero-indegree member. Breaking it at the first
            // stable subject keeps layout finite without rewriting the link.
            .or_else(|| (0..subjects.len()).find(|&index| !placed[index]));
        let Some(index) = next else { break };
        if indegree[index] > 0 {
            depth[index] = incoming[index]
                .iter()
                .filter(|&&parent| placed[parent])
                .map(|&parent| depth[parent] + 1)
                .max()
                .unwrap_or(0);
        }
        placed[index] = true;
        for &target in &outgoing[index] {
            if !placed[target] {
                indegree[target] = indegree[target].saturating_sub(1);
                depth[target] = depth[target].max(depth[index] + 1);
            }
        }
    }

    let max_depth = depth.iter().copied().max().unwrap_or(0);
    let mut rows = vec![0usize; max_depth + 1];
    subjects
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let layer = depth[index];
            let row = rows[layer];
            rows[layer] += 1;
            match layout {
                AgentRunLayout::LayeredHorizontal => {
                    let visual_layer = if rtl { max_depth - layer } else { layer };
                    point(
                        INSET + visual_layer as f32 * (NODE_WIDTH + COLUMN_GAP),
                        INSET + row as f32 * (ESTIMATED_NODE_HEIGHT + ROW_GAP),
                    )
                }
                AgentRunLayout::LayeredVertical => point(
                    INSET + row as f32 * (NODE_WIDTH + COLUMN_GAP),
                    INSET + layer as f32 * (ESTIMATED_NODE_HEIGHT + ROW_GAP),
                ),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use gpui::SharedString;

    use super::*;
    use crate::agent::model::{
        AgentActivity, AgentDescriptor, AgentSnapshot, AgentTaskSnapshot, RunLinkId, TaskId,
        WaitReason,
    };

    fn subject_agent(id: &'static str) -> RunSubjectId {
        RunSubjectId::Agent(id.into())
    }

    fn link(id: &'static str, from: RunSubjectId, to: RunSubjectId, kind: RunLinkKind) -> RunLink {
        RunLink::new(id, from, to, kind)
    }

    #[test]
    fn every_agent_execution_state_maps_without_collapsing() {
        let cases = [
            (AgentExecutionState::Idle, NodeState::Idle),
            (AgentExecutionState::Queued, NodeState::Queued),
            (AgentExecutionState::Starting, NodeState::Starting),
            (
                AgentExecutionState::Active(AgentActivity::Thinking),
                NodeState::Running,
            ),
            (
                AgentExecutionState::Waiting(WaitReason::Approval),
                NodeState::Waiting,
            ),
            (
                AgentExecutionState::Blocked("policy".into()),
                NodeState::Blocked,
            ),
            (AgentExecutionState::Cancelling, NodeState::Cancelling),
            (
                AgentExecutionState::Completed(AgentOutcome::Succeeded),
                NodeState::Succeeded,
            ),
            (
                AgentExecutionState::Completed(AgentOutcome::Partial("some".into())),
                NodeState::Partial,
            ),
            (
                AgentExecutionState::Completed(AgentOutcome::Failed("failed".into())),
                NodeState::Failed,
            ),
            (
                AgentExecutionState::Completed(AgentOutcome::Refused("no".into())),
                NodeState::Refused,
            ),
            (
                AgentExecutionState::Completed(AgentOutcome::Cancelled),
                NodeState::Cancelled,
            ),
            (
                AgentExecutionState::Completed(AgentOutcome::TimedOut("late".into())),
                NodeState::TimedOut,
            ),
            (
                AgentExecutionState::Unavailable("offline".into()),
                NodeState::Unavailable,
            ),
        ];
        for (execution, expected) in cases {
            assert_eq!(node_state(&execution), expected, "{}", execution.as_str());
        }
    }

    #[test]
    fn relationship_wording_keeps_kind_when_the_host_adds_a_label() {
        let strings = Strings::new();
        let kinds = [
            (RunLinkKind::Spawn, "Spawn"),
            (RunLinkKind::Delegation, "Delegation"),
            (RunLinkKind::Dependency, "Dependency"),
            (RunLinkKind::Handoff, "Handoff"),
            (RunLinkKind::Report, "Report"),
            (RunLinkKind::Aggregation, "Aggregation"),
            (RunLinkKind::Retry, "Retry"),
        ];
        for (kind, expected) in kinds {
            let relation = link("edge", subject_agent("a"), subject_agent("b"), kind);
            assert_eq!(link_label(&relation, &strings), expected);
            assert_eq!(
                link_label(&relation.label("round two"), &strings),
                format!("{expected}: round two")
            );
        }
    }

    #[test]
    fn layered_layout_is_deterministic_finite_and_mirrors_horizontal_rtl() {
        let subjects = vec![
            subject_agent("root"),
            subject_agent("a"),
            subject_agent("b"),
        ];
        let links = vec![
            link(
                "root-a",
                subject_agent("root"),
                subject_agent("a"),
                RunLinkKind::Delegation,
            ),
            link(
                "a-b",
                subject_agent("a"),
                subject_agent("b"),
                RunLinkKind::Handoff,
            ),
            link(
                "b-a",
                subject_agent("b"),
                subject_agent("a"),
                RunLinkKind::Retry,
            ),
        ];
        let ltr = layered_positions(&subjects, &links, AgentRunLayout::LayeredHorizontal, false);
        assert_eq!(
            ltr,
            layered_positions(&subjects, &links, AgentRunLayout::LayeredHorizontal, false)
        );
        assert!(ltr[0].x < ltr[1].x && ltr[1].x < ltr[2].x);
        let rtl = layered_positions(&subjects, &links, AgentRunLayout::LayeredHorizontal, true);
        assert!(rtl[0].x > rtl[1].x && rtl[1].x > rtl[2].x);

        let cycle = vec![
            link(
                "a-b",
                subject_agent("a"),
                subject_agent("b"),
                RunLinkKind::Delegation,
            ),
            link(
                "b-a",
                subject_agent("b"),
                subject_agent("a"),
                RunLinkKind::Handoff,
            ),
        ];
        assert!(
            layered_positions(&subjects, &cycle, AgentRunLayout::LayeredVertical, false)
                .iter()
                .all(|position| position.x.is_finite() && position.y.is_finite())
        );
    }

    #[test]
    fn subjects_namespace_agents_tasks_and_invocations() {
        let ident = Ident::new("run");
        let raw = "same";
        let ids = [
            subject_ident(&ident, &RunSubjectId::Agent(raw.into())),
            subject_ident(&ident, &RunSubjectId::Task(raw.into())),
            subject_ident(&ident, &RunSubjectId::Invocation(raw.into())),
        ];
        assert_eq!(ids[0], SharedString::from("run.agent.same"));
        assert_eq!(ids[1], SharedString::from("run.task.same"));
        assert_eq!(ids[2], SharedString::from("run.invocation.same"));
    }

    #[test]
    fn invocations_are_projected_once_without_inventing_snapshot_detail() {
        let invocation = RunSubjectId::Invocation("call".into());
        let run = AgentRunSnapshot::new("run", "root")
            .agents([AgentSnapshot::new(AgentDescriptor::new("root", "Root"))])
            .tasks([AgentTaskSnapshot::new("task", "Task")])
            .links([
                link(
                    "call-task",
                    invocation.clone(),
                    RunSubjectId::Task(TaskId::new("task")),
                    RunLinkKind::Report,
                ),
                link(
                    "root-call",
                    subject_agent("root"),
                    invocation.clone(),
                    RunLinkKind::Delegation,
                ),
            ]);
        assert_eq!(
            run_subjects(&run)
                .iter()
                .filter(|subject| **subject == invocation)
                .count(),
            1
        );
        assert_eq!(run.links[0].id, RunLinkId::new("call-task"));
    }
}
