//! The vocabulary an agent application needs and a conversation does not.
//!
//! A run made of agents and steps, the permission it asks for, and what it
//! costs. Runtime transports stay outside this module: [`model`] is the
//! caller-owned snapshot components read, not a second agent runtime.
pub mod approval;
pub mod cost;
pub mod model;
pub mod offering_catalog;
pub mod permission;
pub mod server_list;
pub mod step_list;
pub mod thinking;
pub mod tool_call;

pub use approval::{AlwaysScope, ApprovalDecision, ApprovalEvent, ApprovalPrompt, ApprovalStatus};
pub use cost::{Basis, ContextGauge, CostLine, CostMeter, LastVerified, Limit, Quantity, Reading};
pub use model::{
    AgentActivity, AgentDescriptor, AgentExecutionState, AgentId, AgentModelIssue, AgentOutcome,
    AgentPresence, AgentRunSnapshot, AgentSnapshot, AgentTaskSnapshot, AgentUiAction,
    AggregationSnapshot, InvocationId, RunId, RunLink, RunLinkId, RunLinkKind, RunSubjectId,
    TaskId, VisualEventId, WaitReason,
};
pub use offering_catalog::{
    OfferingCatalog, OfferingIdentity, OfferingSource, OfferingSourceState, SearchableOffering,
};
pub use permission::{
    PermissionAction, PermissionChange, PermissionEntry, PermissionMatrix, PermissionSource,
    PermissionState, PermissionSubject,
};
pub use server_list::{Catalog, Offering, OfferingKind, ServerEntry, ServerList, ServerState};
pub use step_list::{RunLength, Step, StepList, StepState};
pub use thinking::{Reasoning, ThinkingBlock};
pub use tool_call::{Elapsed, ToolBody, ToolCallCard, ToolCallState, ToolOutput};
