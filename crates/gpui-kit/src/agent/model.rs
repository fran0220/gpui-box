//! Product-neutral facts for a run involving one or more agents.
//!
//! These types are a view-model contract, not a transport. An A2A, AG-UI,
//! MCP, or application-specific runtime maps its own events into a snapshot;
//! components read the snapshot and report [`AgentUiAction`]s without spawning,
//! cancelling, retrying, or approving anything themselves.
//!
//! # Identity is not list position
//!
//! Agents, tasks, invocations, links, runs, and transient visual events each
//! carry their own business identity. [`AgentRunSnapshot::issues`] reports
//! duplicate and dangling identities instead of silently deduplicating them:
//! deciding whether two records are the same fact is host policy.

use std::collections::HashSet;

use gpui::SharedString;
use serde::{Deserialize, Serialize};

/// The stable identity of an agent, independent of its role or display name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(SharedString);

impl AgentId {
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<&'static str> for AgentId {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl From<String> for AgentId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<SharedString> for AgentId {
    fn from(value: SharedString) -> Self {
        Self::new(value)
    }
}

/// The stable identity of one run.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(SharedString);

impl RunId {
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<&'static str> for RunId {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl From<String> for RunId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<SharedString> for RunId {
    fn from(value: SharedString) -> Self {
        Self::new(value)
    }
}

/// The stable identity of one task in a run.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(SharedString);

impl TaskId {
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<&'static str> for TaskId {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TaskId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<SharedString> for TaskId {
    fn from(value: SharedString) -> Self {
        Self::new(value)
    }
}

/// The stable identity of one tool or capability invocation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InvocationId(SharedString);

impl InvocationId {
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<&'static str> for InvocationId {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl From<String> for InvocationId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<SharedString> for InvocationId {
    fn from(value: SharedString) -> Self {
        Self::new(value)
    }
}

/// The stable identity of a topology relationship.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunLinkId(SharedString);

impl RunLinkId {
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<&'static str> for RunLinkId {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl From<String> for RunLinkId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<SharedString> for RunLinkId {
    fn from(value: SharedString) -> Self {
        Self::new(value)
    }
}

/// The stable identity of a transient visual event.
///
/// An effect player uses this to avoid replaying a celebration after a stream
/// reconnects, and as the deterministic seed for any decorative particles.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VisualEventId(SharedString);

impl VisualEventId {
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<&'static str> for VisualEventId {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl From<String> for VisualEventId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<SharedString> for VisualEventId {
    fn from(value: SharedString) -> Self {
        Self::new(value)
    }
}

/// What kind of thing one topology endpoint identifies.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "kebab-case")]
pub enum RunSubjectId {
    Agent(AgentId),
    Task(TaskId),
    Invocation(InvocationId),
}

impl RunSubjectId {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Agent(id) => id.as_str(),
            Self::Task(id) => id.as_str(),
            Self::Invocation(id) => id.as_str(),
        }
    }
}

/// Identity and caller-owned description of one agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDescriptor {
    pub id: AgentId,
    pub name: SharedString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<SharedString>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<SharedString>,
}

impl AgentDescriptor {
    pub fn new(id: impl Into<AgentId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            role: None,
            capabilities: Vec::new(),
        }
    }

    pub fn role(mut self, role: impl Into<SharedString>) -> Self {
        self.role = Some(role.into());
        self
    }

    pub fn capabilities(
        mut self,
        capabilities: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.capabilities
            .extend(capabilities.into_iter().map(Into::into));
        self
    }
}

/// Whether the host can presently reach an agent.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "kebab-case")]
pub enum AgentPresence {
    #[default]
    Unknown,
    Online,
    Away,
    Offline,
    Unavailable(SharedString),
}

impl AgentPresence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Online => "online",
            Self::Away => "away",
            Self::Offline => "offline",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

/// What useful work an active agent is visibly doing.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "activity", content = "detail", rename_all = "kebab-case")]
pub enum AgentActivity {
    #[default]
    Idle,
    Planning,
    Thinking,
    UsingTool(SharedString),
    Speaking,
    Aggregating,
    Custom(SharedString),
}

impl AgentActivity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Planning => "planning",
            Self::Thinking => "thinking",
            Self::UsingTool(_) => "using-tool",
            Self::Speaking => "speaking",
            Self::Aggregating => "aggregating",
            Self::Custom(_) => "custom",
        }
    }
}

/// Why an agent or task is waiting rather than making progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", content = "detail", rename_all = "kebab-case")]
pub enum WaitReason {
    UserInput,
    Approval,
    Dependency(TaskId),
    RemoteAgent(AgentId),
    RateLimit,
    Custom(SharedString),
}

impl WaitReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserInput => "user-input",
            Self::Approval => "approval",
            Self::Dependency(_) => "dependency",
            Self::RemoteAgent(_) => "remote-agent",
            Self::RateLimit => "rate-limit",
            Self::Custom(_) => "custom",
        }
    }
}

/// The terminal result of an agent, task, or run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", content = "reason", rename_all = "kebab-case")]
pub enum AgentOutcome {
    Succeeded,
    Partial(SharedString),
    Failed(SharedString),
    Refused(SharedString),
    Cancelled,
    TimedOut(SharedString),
}

impl AgentOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Partial(_) => "partial",
            Self::Failed(_) => "failed",
            Self::Refused(_) => "refused",
            Self::Cancelled => "cancelled",
            Self::TimedOut(_) => "timed-out",
        }
    }
}

/// Mutually exclusive execution state for an agent, task, or run.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "state", content = "detail", rename_all = "kebab-case")]
pub enum AgentExecutionState {
    #[default]
    Idle,
    Queued,
    Starting,
    Active(AgentActivity),
    Waiting(WaitReason),
    Blocked(SharedString),
    Cancelling,
    Completed(AgentOutcome),
    Unavailable(SharedString),
}

impl AgentExecutionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Queued => "queued",
            Self::Starting => "starting",
            Self::Active(_) => "active",
            Self::Waiting(_) => "waiting",
            Self::Blocked(_) => "blocked",
            Self::Cancelling => "cancelling",
            Self::Completed(outcome) => outcome.as_str(),
            Self::Unavailable(_) => "unavailable",
        }
    }

    pub fn busy(&self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Starting | Self::Active(_) | Self::Cancelling
        )
    }

    pub fn terminal(&self) -> bool {
        matches!(self, Self::Completed(_))
    }
}

/// One agent at one observed point in a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub descriptor: AgentDescriptor,
    #[serde(default)]
    pub presence: AgentPresence,
    #[serde(default)]
    pub execution: AgentExecutionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_task: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<f32>,
}

impl AgentSnapshot {
    pub fn new(descriptor: AgentDescriptor) -> Self {
        Self {
            descriptor,
            presence: AgentPresence::Unknown,
            execution: AgentExecutionState::Idle,
            current_task: None,
            progress: None,
        }
    }

    pub fn presence(mut self, presence: AgentPresence) -> Self {
        self.presence = presence;
        self
    }

    pub fn execution(mut self, execution: AgentExecutionState) -> Self {
        self.execution = execution;
        self
    }

    pub fn current_task(mut self, task: impl Into<TaskId>) -> Self {
        self.current_task = Some(task.into());
        self
    }

    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = progress.is_finite().then(|| progress.clamp(0.0, 1.0));
        self
    }
}

/// One caller-owned task in a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTaskSnapshot {
    pub id: TaskId,
    pub label: SharedString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<AgentId>,
    #[serde(default)]
    pub execution: AgentExecutionState,
}

impl AgentTaskSnapshot {
    pub fn new(id: impl Into<TaskId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            owner: None,
            execution: AgentExecutionState::Idle,
        }
    }

    pub fn owner(mut self, owner: impl Into<AgentId>) -> Self {
        self.owner = Some(owner.into());
        self
    }

    pub fn execution(mut self, execution: AgentExecutionState) -> Self {
        self.execution = execution;
        self
    }
}

/// The meaning of a topology relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RunLinkKind {
    Spawn,
    Delegation,
    Dependency,
    Handoff,
    Report,
    Aggregation,
    Retry,
}

impl RunLinkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spawn => "spawn",
            Self::Delegation => "delegation",
            Self::Dependency => "dependency",
            Self::Handoff => "handoff",
            Self::Report => "report",
            Self::Aggregation => "aggregation",
            Self::Retry => "retry",
        }
    }
}

/// One typed relationship between two run subjects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunLink {
    pub id: RunLinkId,
    pub from: RunSubjectId,
    pub to: RunSubjectId,
    pub kind: RunLinkKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<SharedString>,
}

impl RunLink {
    pub fn new(
        id: impl Into<RunLinkId>,
        from: RunSubjectId,
        to: RunSubjectId,
        kind: RunLinkKind,
    ) -> Self {
        Self {
            id: id.into(),
            from,
            to,
            kind,
            label: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// A caller-owned count of the results being combined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregationSnapshot {
    pub expected: usize,
    pub received: usize,
    #[serde(default)]
    pub conflicts: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<AgentOutcome>,
}

impl AggregationSnapshot {
    pub fn new(expected: usize, received: usize) -> Self {
        Self {
            expected,
            received,
            conflicts: 0,
            outcome: None,
        }
    }

    pub fn conflicts(mut self, conflicts: usize) -> Self {
        self.conflicts = conflicts;
        self
    }

    pub fn outcome(mut self, outcome: AgentOutcome) -> Self {
        self.outcome = Some(outcome);
        self
    }
}

/// A complete observed run. The caller replaces or updates it as facts change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRunSnapshot {
    pub id: RunId,
    pub root: AgentId,
    #[serde(default)]
    pub execution: AgentExecutionState,
    #[serde(default)]
    pub agents: Vec<AgentSnapshot>,
    #[serde(default)]
    pub tasks: Vec<AgentTaskSnapshot>,
    #[serde(default)]
    pub links: Vec<RunLink>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregation: Option<AggregationSnapshot>,
}

impl AgentRunSnapshot {
    pub fn new(id: impl Into<RunId>, root: impl Into<AgentId>) -> Self {
        Self {
            id: id.into(),
            root: root.into(),
            execution: AgentExecutionState::Idle,
            agents: Vec::new(),
            tasks: Vec::new(),
            links: Vec::new(),
            aggregation: None,
        }
    }

    pub fn execution(mut self, execution: AgentExecutionState) -> Self {
        self.execution = execution;
        self
    }

    pub fn agents(mut self, agents: impl IntoIterator<Item = AgentSnapshot>) -> Self {
        self.agents.extend(agents);
        self
    }

    pub fn tasks(mut self, tasks: impl IntoIterator<Item = AgentTaskSnapshot>) -> Self {
        self.tasks.extend(tasks);
        self
    }

    pub fn links(mut self, links: impl IntoIterator<Item = RunLink>) -> Self {
        self.links.extend(links);
        self
    }

    pub fn aggregation(mut self, aggregation: AggregationSnapshot) -> Self {
        self.aggregation = Some(aggregation);
        self
    }

    /// Reports malformed identities without changing the caller's data.
    pub fn issues(&self) -> Vec<AgentModelIssue> {
        let mut issues = Vec::new();
        let mut agents = HashSet::new();
        for agent in &self.agents {
            if !agents.insert(agent.descriptor.id.clone()) {
                issues.push(AgentModelIssue::DuplicateAgent(agent.descriptor.id.clone()));
            }
        }
        if !agents.contains(&self.root) {
            issues.push(AgentModelIssue::MissingRoot(self.root.clone()));
        }

        let mut tasks = HashSet::new();
        for task in &self.tasks {
            if !tasks.insert(task.id.clone()) {
                issues.push(AgentModelIssue::DuplicateTask(task.id.clone()));
            }
            if let Some(owner) = &task.owner
                && !agents.contains(owner)
            {
                issues.push(AgentModelIssue::MissingTaskOwner {
                    task: task.id.clone(),
                    owner: owner.clone(),
                });
            }
        }

        let mut links = HashSet::new();
        for link in &self.links {
            if !links.insert(link.id.clone()) {
                issues.push(AgentModelIssue::DuplicateLink(link.id.clone()));
            }
            if link.from == link.to {
                issues.push(AgentModelIssue::SelfLink(link.id.clone()));
            }
            for endpoint in [&link.from, &link.to] {
                let present = match endpoint {
                    RunSubjectId::Agent(id) => agents.contains(id),
                    RunSubjectId::Task(id) => tasks.contains(id),
                    // Invocation detail can arrive after its topology link;
                    // invocations are not a required snapshot collection.
                    RunSubjectId::Invocation(_) => true,
                };
                if !present {
                    issues.push(AgentModelIssue::MissingLinkEndpoint {
                        link: link.id.clone(),
                        endpoint: endpoint.clone(),
                    });
                }
            }
        }
        issues
    }
}

/// A structural problem found in an [`AgentRunSnapshot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentModelIssue {
    MissingRoot(AgentId),
    DuplicateAgent(AgentId),
    DuplicateTask(TaskId),
    DuplicateLink(RunLinkId),
    MissingTaskOwner {
        task: TaskId,
        owner: AgentId,
    },
    MissingLinkEndpoint {
        link: RunLinkId,
        endpoint: RunSubjectId,
    },
    SelfLink(RunLinkId),
}

/// An action a multi-agent surface asks the host to consider.
///
/// The component does not apply the action. In particular, requesting
/// cancellation is not the same thing as observing `Cancelling` or
/// `Cancelled` in a later snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentUiAction {
    SelectAgent(AgentId),
    OpenTask(TaskId),
    RequestCancel(RunSubjectId),
    RequestRetry(RunSubjectId),
    FocusResult(RunSubjectId),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(id: &'static str) -> AgentSnapshot {
        AgentSnapshot::new(AgentDescriptor::new(id, id))
    }

    #[test]
    fn execution_states_keep_waiting_refusal_and_failure_distinct() {
        let waiting = AgentExecutionState::Waiting(WaitReason::Approval);
        let refused = AgentExecutionState::Completed(AgentOutcome::Refused("denied".into()));
        let failed = AgentExecutionState::Completed(AgentOutcome::Failed("crashed".into()));

        assert_eq!(waiting.as_str(), "waiting");
        assert_eq!(refused.as_str(), "refused");
        assert_eq!(failed.as_str(), "failed");
        assert!(!waiting.terminal());
        assert!(refused.terminal());
    }

    #[test]
    fn snapshot_reports_duplicates_and_dangling_facts_without_deduplicating() {
        let snapshot = AgentRunSnapshot::new("run", "root")
            .agents([agent("root"), agent("root")])
            .tasks([AgentTaskSnapshot::new("task", "Inspect").owner("missing")])
            .links([RunLink::new(
                "link",
                RunSubjectId::Agent("root".into()),
                RunSubjectId::Task("absent".into()),
                RunLinkKind::Delegation,
            )]);

        assert_eq!(snapshot.agents.len(), 2);
        assert!(
            snapshot
                .issues()
                .contains(&AgentModelIssue::DuplicateAgent("root".into()))
        );
        assert!(snapshot.issues().iter().any(|issue| matches!(
            issue,
            AgentModelIssue::MissingTaskOwner { owner, .. } if owner.as_str() == "missing"
        )));
        assert!(snapshot.issues().iter().any(|issue| matches!(
            issue,
            AgentModelIssue::MissingLinkEndpoint { endpoint, .. }
                if endpoint.as_str() == "absent"
        )));
    }

    #[test]
    fn visual_event_identity_round_trips_through_json() {
        let id = VisualEventId::new("event-7");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"event-7\"");
        assert_eq!(serde_json::from_str::<VisualEventId>(&json).unwrap(), id);
    }
}
