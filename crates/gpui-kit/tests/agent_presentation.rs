//! Multi-agent presentation consumes host facts and reports requests without
//! becoming a second agent runtime.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, ParentElement, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit::semantics::Role;
use gpui_kit_testkit::harness::Harness;

fn agent(id: &'static str, execution: AgentExecutionState, task: &'static str) -> AgentSnapshot {
    AgentSnapshot::new(AgentDescriptor::new(id, id).role(format!("{id} role")))
        .presence(AgentPresence::Online)
        .execution(execution)
        .current_task(task)
}

fn run() -> AgentRunSnapshot {
    AgentRunSnapshot::new("run", "root")
        .agents([
            agent(
                "root",
                AgentExecutionState::Active(AgentActivity::Thinking),
                "plan",
            ),
            agent(
                "child",
                AgentExecutionState::Waiting(WaitReason::Approval),
                "review",
            ),
            agent(
                "done",
                AgentExecutionState::Completed(AgentOutcome::Succeeded),
                "verify",
            ),
        ])
        .tasks([
            AgentTaskSnapshot::new("plan", "Plan").owner("root"),
            AgentTaskSnapshot::new("review", "Review").owner("child"),
            AgentTaskSnapshot::new("verify", "Verify").owner("done"),
        ])
        .links([
            RunLink::new(
                "root-child",
                RunSubjectId::Agent("root".into()),
                RunSubjectId::Agent("child".into()),
                RunLinkKind::Spawn,
            ),
            RunLink::new(
                "child-done",
                RunSubjectId::Agent("child".into()),
                RunSubjectId::Agent("done".into()),
                RunLinkKind::Spawn,
            ),
        ])
}

#[gpui::test]
fn avatar_publishes_identity_presence_execution_and_busy_state(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        AgentAvatar::new(
            "agent.avatar",
            agent(
                "root",
                AgentExecutionState::Active(AgentActivity::Thinking),
                "plan",
            ),
        )
        .into_any_element()
    });

    let avatar = harness.node("agent.avatar").expect("published");
    assert_eq!(avatar.role, Role::Image);
    assert_eq!(avatar.text.as_deref(), Some("root"));
    assert_eq!(avatar.value.as_deref(), Some("online:active"));
    assert!(avatar.busy);
}

#[gpui::test]
fn agent_card_is_keyboard_operable_only_when_it_reports_selection(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = calls.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        AgentCard::new(
            "agent.card",
            agent("root", AgentExecutionState::Idle, "plan"),
        )
        .on_action(move |action, _, _| sink.borrow_mut().push(action))
        .into_any_element()
    });

    assert_eq!(harness.node("agent.card").expect("card").role, Role::Button);
    harness.click("agent.card");
    calls.borrow_mut().clear();
    harness.keystrokes("enter");
    assert_eq!(
        calls.borrow().as_slice(),
        [AgentUiAction::SelectAgent("root".into())]
    );
}

#[gpui::test]
fn roster_reports_business_identity_without_applying_selection(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = calls.clone();
    let snapshot = run();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        AgentRoster::from_run("agent.roster", &snapshot)
            .selected("root")
            .visible_rows(3)
            .on_action(move |action, _, _| sink.borrow_mut().push(action))
            .into_any_element()
    });

    assert_eq!(
        harness
            .node("agent.roster")
            .expect("roster")
            .value
            .as_deref(),
        Some("3")
    );
    assert!(
        harness
            .node("agent.roster.root")
            .expect("root row")
            .selected
    );

    harness.click("agent.roster.child");
    assert_eq!(
        calls.borrow().as_slice(),
        [AgentUiAction::SelectAgent("child".into())]
    );
    assert!(
        harness
            .node("agent.roster.root")
            .expect("selection remains caller-owned")
            .selected
    );
    assert!(
        !harness
            .node("agent.roster.child")
            .expect("request is not applied")
            .selected
    );
}

#[gpui::test]
fn a_roster_without_a_handler_is_read_only(cx: &mut TestAppContext) {
    let snapshot = run();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        AgentRoster::from_run("agent.roster", &snapshot)
            .visible_rows(3)
            .into_any_element()
    });

    assert_eq!(
        harness
            .node("agent.roster.child")
            .expect("row remains visible")
            .role,
        Role::Row
    );
    harness.click("agent.roster.child");
    assert!(
        !harness
            .node("agent.roster.child")
            .expect("read-only row")
            .selected
    );
}

#[gpui::test]
fn duplicate_agent_identity_is_reported_instead_of_collapsed(cx: &mut TestAppContext) {
    let mut snapshot = run();
    snapshot.agents.push(agent(
        "child",
        AgentExecutionState::Completed(AgentOutcome::Failed("duplicate fixture".into())),
        "review",
    ));
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        gpui::div()
            .column()
            .child(AgentRoster::from_run("agent.roster", &snapshot))
            .child(SubagentTree::new("agent.tree", snapshot.clone()))
            .into_any_element()
    });

    for id in ["agent.roster.issues", "agent.tree.issues"] {
        let issue = harness.node(id).expect("structural issue is visible");
        assert_eq!(issue.role, Role::Status);
        assert!(
            issue
                .text
                .as_deref()
                .expect("issue has wording")
                .contains("child")
        );
    }
    assert!(
        harness.node("agent.roster.child").is_none(),
        "duplicate rows were not collapsed into one believable row"
    );
    assert!(
        harness.node("agent.tree.child").is_none(),
        "an invalid topology was not presented as a valid tree"
    );
}

#[gpui::test]
fn subagent_tree_uses_only_spawn_links_and_keeps_disclosure_caller_owned(cx: &mut TestAppContext) {
    let toggles = Rc::new(RefCell::new(Vec::new()));
    let sink = toggles.clone();
    let mut snapshot = run();
    snapshot.links.push(RunLink::new(
        "report",
        RunSubjectId::Agent("done".into()),
        RunSubjectId::Agent("root".into()),
        RunLinkKind::Report,
    ));
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        SubagentTree::new("agent.tree", snapshot.clone())
            .expanded(["root".into(), "child".into()])
            .selected("child")
            .on_toggle(move |id, expanded, _, _| sink.borrow_mut().push((id, expanded)))
            .into_any_element()
    });

    let child = harness.node("agent.tree.child").expect("spawn child");
    let done = harness.node("agent.tree.done").expect("nested spawn child");
    assert_eq!(child.level, Some(2));
    assert_eq!(done.level, Some(3));
    assert!(child.selected);

    harness.click("agent.tree.root.toggle");
    assert_eq!(toggles.borrow().as_slice(), [("root".into(), false)]);
    assert!(
        harness
            .node("agent.tree.root")
            .expect("the component does not apply its toggle request")
            .expanded
            .expect("root is a branch")
    );
}

#[gpui::test]
fn agent_group_reverses_reading_order_in_rtl(cx: &mut TestAppContext) {
    let snapshot = run();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        AgentGroup::new("agent.group", snapshot.agents.clone()).into_any_element()
    });

    let root_ltr = harness.node("agent.group.root").expect("root").bounds.x;
    let done_ltr = harness.node("agent.group.done").expect("done").bounds.x;
    assert!(root_ltr < done_ltr);

    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));

    let root_rtl = harness.node("agent.group.root").expect("root").bounds.x;
    let done_rtl = harness.node("agent.group.done").expect("done").bounds.x;
    assert!(root_rtl > done_rtl);
}
