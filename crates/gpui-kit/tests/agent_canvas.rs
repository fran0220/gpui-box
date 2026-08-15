//! AgentRunCanvas owns the run-to-graph projection while leaving interaction
//! state controlled by the host.

use std::{cell::RefCell, rc::Rc};

use gpui::{Modifiers, MouseButton, TestAppContext, div, point, prelude::*, px};
use gpui_kit::foundation::{LayoutDirection, set_layout_direction};
use gpui_kit::prelude::*;
use gpui_kit::semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Calls = Rc<RefCell<Vec<AgentRunCanvasEvent>>>;

fn agent(id: &'static str, execution: AgentExecutionState) -> AgentSnapshot {
    AgentSnapshot::new(AgentDescriptor::new(id, id)).execution(execution)
}

fn run() -> AgentRunSnapshot {
    AgentRunSnapshot::new("run", "root")
        .agents([
            agent(
                "root",
                AgentExecutionState::Active(AgentActivity::Aggregating),
            ),
            agent("worker", AgentExecutionState::Waiting(WaitReason::Approval)),
            agent(
                "reviewer",
                AgentExecutionState::Completed(AgentOutcome::Refused("policy".into())),
            ),
        ])
        .tasks([
            AgentTaskSnapshot::new("draft", "Draft").execution(AgentExecutionState::Starting),
            AgentTaskSnapshot::new("review", "Review").execution(AgentExecutionState::Completed(
                AgentOutcome::Partial("one concern".into()),
            )),
        ])
        .links([
            RunLink::new(
                "spawn",
                RunSubjectId::Agent("root".into()),
                RunSubjectId::Agent("worker".into()),
                RunLinkKind::Spawn,
            ),
            RunLink::new(
                "delegate",
                RunSubjectId::Agent("root".into()),
                RunSubjectId::Task("draft".into()),
                RunLinkKind::Delegation,
            ),
            RunLink::new(
                "depends",
                RunSubjectId::Task("draft".into()),
                RunSubjectId::Task("review".into()),
                RunLinkKind::Dependency,
            ),
            RunLink::new(
                "handoff",
                RunSubjectId::Agent("worker".into()),
                RunSubjectId::Agent("reviewer".into()),
                RunLinkKind::Handoff,
            ),
            RunLink::new(
                "report",
                RunSubjectId::Invocation("tool-call".into()),
                RunSubjectId::Agent("worker".into()),
                RunLinkKind::Report,
            ),
            RunLink::new(
                "aggregate",
                RunSubjectId::Task("review".into()),
                RunSubjectId::Agent("root".into()),
                RunLinkKind::Aggregation,
            ),
            RunLink::new(
                "retry",
                RunSubjectId::Agent("reviewer".into()),
                RunSubjectId::Agent("worker".into()),
                RunLinkKind::Retry,
            )
            .label("attempt 2"),
        ])
        .aggregation(AggregationSnapshot::new(3, 2).conflicts(1))
}

fn edge_id(id: &str) -> String {
    let id = format!("agent-run.link.{id}");
    format!("graph-edge:{}:{}", id.len(), id)
}

fn canvas(cx: &mut TestAppContext, arrangeable: bool) -> (Harness, Calls) {
    let calls = Calls::default();
    let sink = Rc::clone(&calls);
    let run = run();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        div()
            .w(px(980.0))
            .h(px(520.0))
            .child(
                AgentRunCanvas::new("agent-run", run.clone())
                    .arrangeable(arrangeable)
                    .selected([RunSubjectId::Agent("root".into())])
                    .on_event(move |event, _, _| sink.borrow_mut().push(event.clone())),
            )
            .into_any_element()
    });
    harness.frame();
    (harness, calls)
}

#[gpui::test]
fn canvas_projects_typed_subjects_states_and_every_relationship(cx: &mut TestAppContext) {
    let (mut harness, _) = canvas(cx, false);

    let root = harness.node("agent-run.agent.root").expect("root agent");
    assert_eq!(root.value.as_deref(), Some("running"));
    assert!(root.selected);
    assert_eq!(
        harness
            .node("agent-run.agent.worker")
            .expect("worker")
            .value
            .as_deref(),
        Some("waiting")
    );
    assert_eq!(
        harness
            .node("agent-run.agent.reviewer")
            .expect("reviewer")
            .value
            .as_deref(),
        Some("refused")
    );
    assert_eq!(
        harness
            .node("agent-run.task.review")
            .expect("task")
            .value
            .as_deref(),
        Some("partial")
    );
    assert_eq!(
        harness
            .node("agent-run.invocation.tool-call")
            .expect("invocation placeholder")
            .value
            .as_deref(),
        Some("pending")
    );

    for (id, label) in [
        ("spawn", "Spawn"),
        ("delegate", "Delegation"),
        ("depends", "Dependency"),
        ("handoff", "Handoff"),
        ("report", "Report"),
        ("aggregate", "Aggregation"),
        ("retry", "Retry: attempt 2"),
    ] {
        let edge = harness.node(&edge_id(id)).expect("typed edge is published");
        assert_eq!(edge.role, Role::Group);
        assert_eq!(edge.text.as_deref(), Some(label), "{id}");
    }
}

#[gpui::test]
fn canvas_reports_typed_selection_but_inspection_never_reports_arrangement(
    cx: &mut TestAppContext,
) {
    let (mut harness, calls) = canvas(cx, false);

    harness.click("agent-run.agent.worker");
    assert!(calls.borrow().iter().any(|event| matches!(
        event,
        AgentRunCanvasEvent::SelectionChanged(subjects)
            if subjects == &[RunSubjectId::Agent("worker".into())]
    )));

    calls.borrow_mut().clear();
    harness.keystrokes("delete");
    let start = harness.point_in("agent-run.agent.worker");
    let end = start + point(px(75.0), px(45.0));
    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    assert!(
        calls
            .borrow()
            .iter()
            .all(|event| !matches!(event, AgentRunCanvasEvent::PositionChanged { .. }))
    );
}

#[gpui::test]
fn arranging_reports_business_subject_without_applying_the_position(cx: &mut TestAppContext) {
    let (mut harness, calls) = canvas(cx, true);
    let before = harness
        .bounds("agent-run.agent.worker")
        .expect("worker bounds");
    let start = before.center();
    let end = start + point(px(80.0), px(35.0));
    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(end, MouseButton::Left, Modifiers::none());

    assert!(calls.borrow().iter().any(|event| matches!(
        event,
        AgentRunCanvasEvent::PositionChanged {
            subject: RunSubjectId::Agent(id),
            ..
        } if id.as_str() == "worker"
    )));
    assert_eq!(
        harness
            .bounds("agent-run.agent.worker")
            .expect("controlled worker bounds"),
        before
    );
}

#[gpui::test]
fn horizontal_layout_follows_reading_direction(cx: &mut TestAppContext) {
    let run = AgentRunSnapshot::new("direction", "root")
        .agents([
            agent("root", AgentExecutionState::Idle),
            agent("child", AgentExecutionState::Idle),
        ])
        .links([RunLink::new(
            "spawn",
            RunSubjectId::Agent("root".into()),
            RunSubjectId::Agent("child".into()),
            RunLinkKind::Spawn,
        )]);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(720.0))
            .h(px(280.0))
            .child(AgentRunCanvas::new("direction", run.clone()))
            .into_any_element()
    });
    let root_ltr = harness.bounds("direction.agent.root").expect("root").left();
    let child_ltr = harness
        .bounds("direction.agent.child")
        .expect("child")
        .left();
    assert!(root_ltr < child_ltr);

    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));
    let root_rtl = harness.bounds("direction.agent.root").expect("root").left();
    let child_rtl = harness
        .bounds("direction.agent.child")
        .expect("child")
        .left();
    assert!(root_rtl > child_rtl);
}

#[gpui::test]
fn malformed_snapshot_reports_issues_instead_of_collapsing_duplicate_ids(cx: &mut TestAppContext) {
    let malformed = AgentRunSnapshot::new("bad", "same").agents([
        agent("same", AgentExecutionState::Idle),
        agent("same", AgentExecutionState::Queued),
    ]);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        AgentRunCanvas::new("bad", malformed.clone()).into_any_element()
    });

    let issue = harness.node("bad.issues").expect("structure issue");
    assert_eq!(issue.role, Role::Status);
    assert!(
        issue
            .text
            .as_deref()
            .expect("issue wording")
            .contains("duplicated")
    );
    assert!(harness.node("bad.agent.same").is_none());
}
