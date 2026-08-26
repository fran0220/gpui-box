//! Multi-agent presentation consumes host facts and reports requests without
//! becoming a second agent runtime.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, ParentElement, TestAppContext, div};
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
fn activity_lines_publish_distinct_execution_truth_without_relying_on_colour(
    cx: &mut TestAppContext,
) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .column()
            .child(AgentActivityLine::new(
                "activity.active",
                AgentExecutionState::Active(AgentActivity::Thinking),
            ))
            .child(AgentActivityLine::new(
                "activity.done",
                AgentExecutionState::Completed(AgentOutcome::Succeeded),
            ))
            .into_any_element()
    });

    let active = harness.node("activity.active").expect("active status");
    let done = harness.node("activity.done").expect("completed status");
    assert_eq!((active.role, done.role), (Role::Status, Role::Status));
    assert!(active.busy);
    assert!(!done.busy);
    assert_ne!(active.text, done.text);
    assert_ne!(active.value, done.value);
}

#[gpui::test]
fn run_issues_report_malformed_structure_and_are_absent_for_a_valid_run(cx: &mut TestAppContext) {
    let valid = run();
    let mut invalid = valid.clone();
    invalid.agents.push(agent(
        "child",
        AgentExecutionState::Completed(AgentOutcome::Failed("duplicate fixture".into())),
        "review",
    ));
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .column()
            .child(AgentRunIssues::from_run("issues.invalid", &invalid))
            .child(AgentRunIssues::from_run("issues.valid", &valid))
            .into_any_element()
    });

    let issues = harness
        .node("issues.invalid")
        .expect("malformed run is reported");
    assert_eq!(issues.role, Role::Status);
    assert!(
        issues
            .text
            .as_deref()
            .expect("issue wording")
            .contains("child")
    );
    assert!(
        harness.node("issues.invalid.issue-0").is_some(),
        "individual faults remain addressable"
    );
    assert!(
        harness.node("issues.valid").is_none(),
        "a valid run does not invent a notice"
    );
}

#[gpui::test]
fn artifact_preview_publishes_every_state_and_honours_state_slots(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .column()
            .child(
                ArtifactPreview::new("artifact.ready", "patch.rs")
                    .kind(ArtifactKind::Code)
                    .body("fn ready() {}")
                    .state(ArtifactPreviewState::Ready),
            )
            .child(
                ArtifactPreview::new("artifact.loading", "notes.md")
                    .state(ArtifactPreviewState::Loading),
            )
            .child(
                ArtifactPreview::new("artifact.empty", "notes.md")
                    .state(ArtifactPreviewState::Empty),
            )
            .child(
                ArtifactPreview::new("artifact.unavailable", "notes.md")
                    .state(ArtifactPreviewState::Unavailable("host refused".into())),
            )
            .child(
                ArtifactPreview::new("artifact.error", "notes.md")
                    .state(ArtifactPreviewState::Error("parse failed".into()))
                    .slot(slot::FAILED, |_, _| {
                        Callout::new("Custom artifact failure", Tone::Danger)
                            .id("artifact.error.custom")
                            .into_any_element()
                    }),
            )
            .into_any_element()
    });

    for (id, state) in [
        ("artifact.ready", "ready"),
        ("artifact.loading", "loading"),
        ("artifact.empty", "empty"),
        ("artifact.unavailable", "unavailable"),
        ("artifact.error", "error"),
    ] {
        let node = harness.node(id).expect("artifact state");
        assert_eq!(node.role, Role::Region);
        assert_eq!(node.value.as_deref(), Some(state));
    }
    assert!(harness.node("artifact.error.custom").is_some());
}

#[gpui::test]
fn feedback_votes_and_tags_share_pointer_keyboard_and_disabled_contracts(cx: &mut TestAppContext) {
    let votes = Rc::new(RefCell::new(Vec::new()));
    let tags = Rc::new(RefCell::new(Vec::new()));
    let vote_sink = Rc::clone(&votes);
    let tag_sink = Rc::clone(&tags);
    let disabled_vote_sink = Rc::clone(&votes);
    let disabled_tag_sink = Rc::clone(&tags);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let vote_sink = Rc::clone(&vote_sink);
        let tag_sink = Rc::clone(&tag_sink);
        let disabled_vote_sink = Rc::clone(&disabled_vote_sink);
        let disabled_tag_sink = Rc::clone(&disabled_tag_sink);
        div()
            .column()
            .child(
                FeedbackRating::new("feedback")
                    .vote(Some(FeedbackVote::Up))
                    .tags([("accurate", "Accurate")])
                    .current_tag("accurate")
                    .on_vote(move |vote, _, _| vote_sink.borrow_mut().push(vote))
                    .on_tag(move |tag, _, _| tag_sink.borrow_mut().push(tag)),
            )
            .child(
                FeedbackRating::new("feedback.disabled")
                    .tags([("accurate", "Accurate")])
                    .disabled(true)
                    .on_vote(move |vote, _, _| disabled_vote_sink.borrow_mut().push(vote))
                    .on_tag(move |tag, _, _| disabled_tag_sink.borrow_mut().push(tag)),
            )
            .into_any_element()
    });

    assert_eq!(
        harness.node("feedback.up").expect("vote").parent.as_deref(),
        Some("feedback")
    );
    harness.click("feedback.up");
    votes.borrow_mut().clear();
    harness.keystrokes("space");
    assert_eq!(votes.borrow().as_slice(), [FeedbackVote::Up]);

    harness.click("feedback.tag.accurate");
    tags.borrow_mut().clear();
    harness.keystrokes("enter");
    assert_eq!(tags.borrow().as_slice(), ["accurate"]);

    assert!(
        harness
            .node("feedback.disabled.up")
            .expect("disabled vote")
            .disabled
    );
    harness.click("feedback.disabled.up");
    harness.click("feedback.disabled.tag.accurate");
    assert_eq!(votes.borrow().as_slice(), [FeedbackVote::Up]);
    assert_eq!(tags.borrow().as_slice(), ["accurate"]);
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
