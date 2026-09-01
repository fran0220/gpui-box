//! What an agent proposes, spends, and is permitted to do.

use super::display::identity_tint;
use super::support::*;

pub(super) fn scene_agent_run() -> AgentRunSnapshot {
    AgentRunSnapshot::new("release-review", "orchestrator")
        .execution(AgentExecutionState::Active(AgentActivity::Aggregating))
        .agents([
            AgentSnapshot::new(
                AgentDescriptor::new("orchestrator", "Atlas").role("Run coordinator"),
            )
            .presence(AgentPresence::Online)
            .execution(AgentExecutionState::Active(AgentActivity::Aggregating))
            .current_task("synthesis"),
            AgentSnapshot::new(
                AgentDescriptor::new("researcher", "Nova").role("Repository research"),
            )
            .presence(AgentPresence::Online)
            .execution(AgentExecutionState::Active(AgentActivity::UsingTool(
                "repository search".into(),
            )))
            .current_task("evidence"),
            AgentSnapshot::new(AgentDescriptor::new("reviewer", "Sage").role("Independent review"))
                .presence(AgentPresence::Away)
                .execution(AgentExecutionState::Waiting(WaitReason::Approval))
                .current_task("review"),
            AgentSnapshot::new(AgentDescriptor::new("verifier", "Bolt").role("Test verification"))
                .presence(AgentPresence::Online)
                .execution(AgentExecutionState::Completed(AgentOutcome::Succeeded))
                .current_task("tests"),
        ])
        .tasks([
            AgentTaskSnapshot::new("synthesis", "Synthesis").owner("orchestrator"),
            AgentTaskSnapshot::new("evidence", "Evidence").owner("researcher"),
            AgentTaskSnapshot::new("review", "Review").owner("reviewer"),
            AgentTaskSnapshot::new("tests", "Tests").owner("verifier"),
        ])
        .links([
            RunLink::new(
                "spawn-researcher",
                RunSubjectId::Agent("orchestrator".into()),
                RunSubjectId::Agent("researcher".into()),
                RunLinkKind::Spawn,
            ),
            RunLink::new(
                "spawn-reviewer",
                RunSubjectId::Agent("orchestrator".into()),
                RunSubjectId::Agent("reviewer".into()),
                RunLinkKind::Spawn,
            ),
            RunLink::new(
                "spawn-verifier",
                RunSubjectId::Agent("researcher".into()),
                RunSubjectId::Agent("verifier".into()),
                RunLinkKind::Spawn,
            ),
        ])
}

/// One agent's identity and one agent's sentence, in the states they report.
///
/// Both were only ever drawn inside `AgentCard` inside `agent-roster`, so
/// nobody had seen a presence mark next to the other presence marks, and a
/// change to either moved a picture of a roster.
pub(super) fn agent_avatar(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let identity = |id: &'static str, name: &'static str, presence: AgentPresence| {
        AgentSnapshot::new(AgentDescriptor::new(id, name).role("Repository research"))
            .presence(presence)
    };
    let executions = [
        (
            "active",
            AgentExecutionState::Active(AgentActivity::UsingTool("repository search".into())),
        ),
        (
            "waiting",
            AgentExecutionState::Waiting(WaitReason::Approval),
        ),
        (
            "succeeded",
            AgentExecutionState::Completed(AgentOutcome::Succeeded),
        ),
        (
            "failed",
            AgentExecutionState::Completed(AgentOutcome::Failed(
                "the verifier could not reach the test host".into(),
            )),
        ),
        (
            "refused",
            AgentExecutionState::Completed(AgentOutcome::Refused(
                "this workspace requires approval".into(),
            )),
        ),
    ];

    stack(&theme)
        .child(caption(
            &theme,
            "Presence is a separate mark, so it does not compete with the identity tint",
        ))
        .child(
            row(&theme)
                .items_center()
                .gap_token(&theme, Space::Md)
                .child(AgentAvatar::new(
                    "scene.agent-avatar.online",
                    identity("nova", "Nova", AgentPresence::Online),
                ))
                .child(AgentAvatar::new(
                    "scene.agent-avatar.away",
                    identity("sage", "Sage", AgentPresence::Away),
                ))
                .child(AgentAvatar::new(
                    "scene.agent-avatar.offline",
                    identity("bolt", "Bolt", AgentPresence::Offline),
                ))
                .child(
                    AgentAvatar::new(
                        "scene.agent-avatar.tinted",
                        identity("atlas", "Atlas", AgentPresence::Online),
                    )
                    .tint(identity_tint(&theme, "agent.external")),
                ),
        )
        .child(caption(&theme, "Sizes"))
        .child(
            row(&theme)
                .items_center()
                .gap_token(&theme, Space::Md)
                .child(
                    AgentAvatar::new(
                        "scene.agent-avatar.small",
                        identity("nova", "Nova", AgentPresence::Online),
                    )
                    .size(20.0),
                )
                .child(AgentAvatar::new(
                    "scene.agent-avatar.default",
                    identity("nova", "Nova", AgentPresence::Online),
                ))
                .child(
                    AgentAvatar::new(
                        "scene.agent-avatar.large",
                        identity("nova", "Nova", AgentPresence::Online),
                    )
                    .size(44.0),
                ),
        )
        .child(caption(
            &theme,
            "The execution sentence, which carries a non-colour mark because \
             colour alone is not a report",
        ))
        .child(
            div()
                .column()
                .gap_token(&theme, Space::Sm)
                .children(executions.map(|(id, execution)| {
                    AgentActivityLine::new(format!("scene.agent-avatar.line.{id}"), execution)
                        .into_any_element()
                })),
        )
        .into_any_element()
}

/// What a malformed run snapshot looks like when it is reported instead of
/// drawn over.
///
/// This component renders nothing at all when the snapshot is well formed,
/// which is why it needs a scene of its own: it was declared against
/// `agent-run-canvas`, where the fixture is valid and no pixel of it ever
/// appeared. A picture of a component that draws nothing is not a review.
pub(super) fn agent_run_issues(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    // Deliberately broken: the root names an agent that is not in the list,
    // one agent is listed twice, a task is owned by nobody, and a link ends
    // at an agent that does not exist.
    let malformed = AgentRunSnapshot::new("release-review", "missing-coordinator")
        .execution(AgentExecutionState::Active(AgentActivity::Aggregating))
        .agents([
            AgentSnapshot::new(AgentDescriptor::new("researcher", "Nova"))
                .presence(AgentPresence::Online),
            AgentSnapshot::new(AgentDescriptor::new("researcher", "Nova"))
                .presence(AgentPresence::Online),
        ])
        .tasks([AgentTaskSnapshot::new("review", "Review").owner("reviewer")])
        .links([RunLink::new(
            "spawn-verifier",
            RunSubjectId::Agent("researcher".into()),
            RunSubjectId::Agent("verifier".into()),
            RunLinkKind::Spawn,
        )]);

    stack(&theme)
        .w(px(560.0))
        .child(caption(
            &theme,
            "A snapshot the host sent that does not describe a run",
        ))
        .child(AgentRunIssues::from_run(
            "scene.agent-run-issues.notice",
            &malformed,
        ))
        .child(caption(
            &theme,
            "A well-formed snapshot, where the component draws nothing at all",
        ))
        .child(AgentRunIssues::from_run(
            "scene.agent-run-issues.silent",
            &scene_agent_run(),
        ))
        .into_any_element()
}

pub(super) fn agent_roster(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let direction = cx.layout_direction();
    let run = scene_agent_run();
    let root = run
        .agents
        .first()
        .cloned()
        .expect("the canonical multi-agent scene has a root agent");

    stack(&theme)
        .w(px(720.0))
        .child(
            div()
                .row_reading(direction)
                .items_center()
                .justify_between()
                .child(
                    div()
                        .column()
                        .gap_token(&theme, Space::Xs)
                        .child(
                            div()
                                .type_scale(&theme, TypeScale::Title)
                                .text_tone(&theme, TextTone::Primary)
                                .child("Release review"),
                        )
                        .child(AgentActivityLine::new(
                            "scene.agent-roster.run-state",
                            run.execution.clone(),
                        )),
                )
                .child(
                    AgentGroup::new("scene.agent-roster.group", run.agents.clone()).max_visible(3),
                ),
        )
        .child(
            AgentCard::new("scene.agent-roster.root", root)
                .task_label("Synthesis")
                .selected(true)
                .on_action(|_, _, _| {}),
        )
        .child(
            div()
                .row_reading(direction)
                .items_start()
                .gap_token(&theme, Space::Lg)
                .child(
                    div().w(px(420.0)).child(
                        AgentRoster::from_run("scene.agent-roster.list", &run)
                            .selected("researcher")
                            .visible_rows(4)
                            .on_action(|_, _, _| {}),
                    ),
                )
                .child(
                    div().w(px(270.0)).child(
                        SubagentTree::new("scene.agent-roster.tree", run)
                            .expanded(["orchestrator".into(), "researcher".into()])
                            .selected("researcher")
                            .visible_rows(4)
                            .on_toggle(|_, _, _, _| {})
                            .on_action(|_, _, _| {}),
                    ),
                ),
        )
        .into_any_element()
}

pub(super) fn persona(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let speaking =
        AgentSnapshot::new(AgentDescriptor::new("guide", "Lyra").role("Expedition guide"))
            .presence(AgentPresence::Online)
            .execution(AgentExecutionState::Active(AgentActivity::Speaking));
    let listening =
        AgentSnapshot::new(AgentDescriptor::new("listener", "Moss").role("World companion"))
            .presence(AgentPresence::Online)
            .execution(AgentExecutionState::Waiting(WaitReason::UserInput));
    let speaking_voice =
        VoiceSample::new(VoiceState::Speaking, 0.76, 0.58).expect("scene voice is normalized");
    let listening_voice =
        VoiceSample::new(VoiceState::Listening, 0.32, 0.48).expect("scene voice is normalized");
    let idle = AgentSnapshot::new(AgentDescriptor::new("idle", "Ori").role("Archivist"))
        .presence(AgentPresence::Away)
        .execution(AgentExecutionState::Idle);
    let thinking = AgentSnapshot::new(AgentDescriptor::new("thinking", "Sable").role("Tactician"))
        .presence(AgentPresence::Online)
        .execution(AgentExecutionState::Active(AgentActivity::Thinking));
    let celebrating = AgentSnapshot::new(AgentDescriptor::new("victor", "Kite").role("Pathfinder"))
        .presence(AgentPresence::Online)
        .execution(AgentExecutionState::Completed(AgentOutcome::Succeeded));
    let unavailable_voice = VoiceSample::new(
        VoiceState::Unavailable("Voice input was not granted.".into()),
        0.0,
        0.0,
    )
    .expect("scene voice is normalized");

    let content = div()
        .column()
        .w(px(680.0))
        .gap_token(&theme, Space::Md)
        .child(
            div()
                .row_reading(cx.layout_direction())
                .items_center()
                .gap_token(&theme, Space::Xl)
                .child(
                    PersonaPortrait::new("scene.persona.portrait-speaking", speaking.clone())
                        .expression(PersonaExpression::Warm)
                        .voice(speaking_voice.clone())
                        .sample_at(Duration::from_millis(640)),
                )
                .child(
                    PersonaPortrait::new("scene.persona.portrait-listening", listening.clone())
                        .expression(PersonaExpression::Focused)
                        .voice(listening_voice.clone())
                        .sample_at(Duration::from_millis(640)),
                )
                .child(
                    div()
                        .column()
                        .gap_token(&theme, Space::Xs)
                        .child(
                            div()
                                .type_scale(&theme, TypeScale::Label)
                                .text_tone(&theme, TextTone::Primary)
                                .child("Voice-reactive"),
                        )
                        .child(
                            VoiceReactive::new("scene.persona.voice", speaking_voice)
                                .sample_at(Duration::from_millis(640)),
                        ),
                ),
        )
        .child(
            PersonaDialogue::new(
                "scene.persona.dialogue",
                DialogueTurn::markdown(
                    "crossroads",
                    speaking,
                    "The signal splits at the **crystal bridge.** I can scout either route without advancing the story for you.",
                )
                .streaming(true)
                .choice(DialogueChoice::new("bridge", "Cross the bridge").selected(true))
                .choice(DialogueChoice::new("cavern", "Enter the cavern"))
                .choice(
                    DialogueChoice::new("gate", "Open the sealed gate")
                        .unavailable("The gate key has not been verified."),
                ),
            )
            .expression(PersonaExpression::Warm)
            .voice(listening_voice)
            .on_event(|_, _, _| {}),
        )
        .child(
            div()
                .row_reading(cx.layout_direction())
                .items_center()
                .gap_token(&theme, Space::Xl)
                .child(
                    div()
                        .column()
                        .items_center()
                        .gap_token(&theme, Space::Xs)
                        .child(
                            PersonaPortrait::new("scene.persona.idle", idle)
                                .expression(PersonaExpression::Neutral)
                                .size(68.0),
                        )
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Caption,
                            "Idle · neutral",
                        )),
                )
                .child(
                    div()
                        .column()
                        .items_center()
                        .gap_token(&theme, Space::Xs)
                        .child(
                            PersonaPortrait::new("scene.persona.thinking", thinking)
                                .expression(PersonaExpression::Focused)
                                .size(68.0),
                        )
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Caption,
                            "Thinking · focused",
                        )),
                )
                .child(
                    div()
                        .column()
                        .items_center()
                        .gap_token(&theme, Space::Xs)
                        .child(
                            PersonaPortrait::new("scene.persona.celebrating", celebrating)
                                .expression(PersonaExpression::Celebrating)
                                .size(68.0),
                        )
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Caption,
                            "Succeeded · celebrating",
                        )),
                )
                .child(
                    div()
                        .column()
                        .items_center()
                        .gap_token(&theme, Space::Xs)
                        .child(VoiceReactive::new(
                            "scene.persona.voice-unavailable",
                            unavailable_voice,
                        ))
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Caption,
                            "Unavailable",
                        )),
                ),
        )
        ;

    stack(&theme)
        .size_full()
        .items_center()
        .justify_center()
        .child(content)
        .into_any_element()
}

pub(super) fn agent_run_canvas(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let run = AgentRunSnapshot::new("release-topology", "orchestrator")
        .execution(AgentExecutionState::Active(AgentActivity::Aggregating))
        .agents([
            AgentSnapshot::new(
                AgentDescriptor::new("orchestrator", "Atlas").role("Run coordinator"),
            )
            .execution(AgentExecutionState::Active(AgentActivity::Aggregating)),
            AgentSnapshot::new(
                AgentDescriptor::new("researcher", "Nova").role("Repository research"),
            )
            .execution(AgentExecutionState::Active(AgentActivity::UsingTool(
                "repository search".into(),
            ))),
            AgentSnapshot::new(AgentDescriptor::new("reviewer", "Sage").role("Independent review"))
                .execution(AgentExecutionState::Waiting(WaitReason::Approval)),
        ])
        .tasks([
            AgentTaskSnapshot::new("evidence", "Evidence bundle")
                .execution(AgentExecutionState::Completed(AgentOutcome::Succeeded)),
            AgentTaskSnapshot::new("synthesis", "Release synthesis").execution(
                AgentExecutionState::Completed(AgentOutcome::Partial("awaiting approval".into())),
            ),
        ])
        .links([
            RunLink::new(
                "spawn-researcher",
                RunSubjectId::Agent("orchestrator".into()),
                RunSubjectId::Agent("researcher".into()),
                RunLinkKind::Spawn,
            ),
            RunLink::new(
                "delegate-evidence",
                RunSubjectId::Agent("orchestrator".into()),
                RunSubjectId::Task("evidence".into()),
                RunLinkKind::Delegation,
            ),
            RunLink::new(
                "evidence-before-synthesis",
                RunSubjectId::Task("evidence".into()),
                RunSubjectId::Task("synthesis".into()),
                RunLinkKind::Dependency,
            ),
            RunLink::new(
                "handoff-review",
                RunSubjectId::Agent("researcher".into()),
                RunSubjectId::Agent("reviewer".into()),
                RunLinkKind::Handoff,
            ),
            RunLink::new(
                "search-report",
                RunSubjectId::Invocation("search-42".into()),
                RunSubjectId::Agent("researcher".into()),
                RunLinkKind::Report,
            ),
            RunLink::new(
                "aggregate-review",
                RunSubjectId::Agent("reviewer".into()),
                RunSubjectId::Task("synthesis".into()),
                RunLinkKind::Aggregation,
            ),
            RunLink::new(
                "retry-search",
                RunSubjectId::Agent("reviewer".into()),
                RunSubjectId::Agent("researcher".into()),
                RunLinkKind::Retry,
            )
            .label("bounded attempt 2"),
        ])
        .aggregation(AggregationSnapshot::new(3, 2).conflicts(1));

    // Laid out down the frame rather than across it, and drawn at 1:1. Four
    // node columns and the relationship wording between them are wider than
    // any frame this is reviewed in, and the zoom that made them fit put the
    // node bodies and the edge labels below the size at which they are text
    // at all: the picture said the canvas was empty and its nodes unreadable,
    // which is a claim about the component that was not true.
    stack(&theme)
        .w(px(920.0))
        .child(
            div()
                .w_full()
                .h(px(830.0))
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    AgentRunCanvas::new("scene.agent-run-canvas", run)
                        .layout(AgentRunLayout::LayeredVertical)
                        .selected([RunSubjectId::Agent("researcher".into())])
                        // Panned far enough right for the retry loop, which
                        // routes around the outside of the column it returns
                        // to, to bring its own wording with it.
                        .viewport(GraphViewport::new(gpui::point(80.0, 0.0), 1.0))
                        .on_event(|_, _, _| {}),
                ),
        )
        .into_any_element()
}

pub(super) struct SceneApprovals {
    pending: Entity<ApprovalPrompt>,
    declined: Entity<ApprovalPrompt>,
    expired: Entity<ApprovalPrompt>,
    superseded: Entity<ApprovalPrompt>,
}

impl Global for SceneApprovals {}

pub(super) fn approval(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneApprovals>() {
        let pending = cx.new(|cx| {
            ApprovalPrompt::new(
                "scene.approval.pending",
                "Write to /work/report/summary.md",
                window,
                cx,
            )
            .details([
                DescriptionItem::new("tool", "Tool", "write-file"),
                DescriptionItem::new("path", "Path", "/work/report/summary.md"),
                DescriptionItem::new("bytes", "Size", "4 KB"),
            ])
            .always(AlwaysScope::Session)
            .always(AlwaysScope::path("/work/report"))
            .always(AlwaysScope::tool("write-file"))
        });
        let declined = cx.new(|cx| {
            ApprovalPrompt::new(
                "scene.approval.declined",
                "Delete /work/report/draft.md",
                window,
                cx,
            )
            .status(ApprovalStatus::Declined)
        });
        let expired = cx.new(|cx| {
            ApprovalPrompt::new(
                "scene.approval.expired",
                "Open a connection to build.internal:8443",
                window,
                cx,
            )
            .status(ApprovalStatus::Expired)
        });
        let superseded = cx.new(|cx| {
            ApprovalPrompt::new(
                "scene.approval.superseded",
                "Run the test suite in /work",
                window,
                cx,
            )
            .status(ApprovalStatus::Superseded {
                by: "a later request covering the whole workspace".into(),
            })
        });
        cx.set_global(SceneApprovals {
            pending,
            declined,
            expired,
            superseded,
        });
    }
    let prompts = cx.global::<SceneApprovals>();
    let pending = prompts.pending.clone();
    let declined = prompts.declined.clone();
    let expired = prompts.expired.clone();
    let superseded = prompts.superseded.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(560.0))
        .child(pending)
        .child(
            Divider::new()
                .id("scene.approval.rule.resolved")
                .label("Answered, expired, and replaced are three different things"),
        )
        .child(declined)
        .child(expired)
        .child(superseded)
        .into_any_element()
}

pub(super) struct SceneClarifications {
    one: Entity<ClarificationPanel>,
    several: Entity<ClarificationPanel>,
    answered: Entity<ClarificationPanel>,
    skipped: Entity<ClarificationPanel>,
    withdrawn: Entity<ClarificationPanel>,
    superseded: Entity<ClarificationPanel>,
    empty: Entity<ClarificationPanel>,
}

impl Global for SceneClarifications {}

/// A question the agent cannot go on without: its own guesses at what was
/// meant, one answer or several, and the four different things that can
/// become of it.
pub(super) fn clarification(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneClarifications>() {
        let candidates = || {
            [
                ClarificationOption::new("src", "src/camera/follow.ts")
                    .detail("Changed in this run"),
                ClarificationOption::new("test", "src/camera/follow.test.ts")
                    .detail("Covers the module above"),
                ClarificationOption::new("docs", "docs/camera.md")
                    .detail("Describes the behaviour"),
            ]
        };
        let one = cx.new(|cx| {
            ClarificationPanel::new(
                "scene.clarification.one",
                "Three files match \"follow\". Which one did you mean?",
                window,
                cx,
            )
            .options(candidates())
        });
        let several = cx.new(|cx| {
            ClarificationPanel::new(
                "scene.clarification.several",
                "Which of these should the release note cover?",
                window,
                cx,
            )
            .multiple()
            .skippable()
            .option(
                ClarificationOption::new("radius", "The node card radius fix")
                    .detail("Landed in 2e79cbea"),
            )
            .option(
                ClarificationOption::new("cjk", "The bundled Chinese face")
                    .detail("Landed in 56c400bc"),
            )
            // A candidate the agent offered and can no longer take. It keeps
            // its place and says why, rather than leaving a list that silently
            // shrank between two frames.
            .option(
                ClarificationOption::new("glass", "The glass radius override")
                    .unavailable("Not released yet, so it has no note to cover."),
            )
        });
        let answered = cx.new(|cx| {
            ClarificationPanel::new(
                "scene.clarification.answered",
                "Three files match \"follow\". Which one did you mean?",
                window,
                cx,
            )
            .options(candidates())
            .status(ClarificationStatus::Answered(vec!["test".into()]))
        });
        let skipped = cx.new(|cx| {
            ClarificationPanel::new(
                "scene.clarification.skipped",
                "Should the summary quote the failing output in full?",
                window,
                cx,
            )
            .skippable()
            .option(ClarificationOption::new("full", "Quote all of it"))
            .option(ClarificationOption::new("head", "Quote the first failure"))
            .status(ClarificationStatus::Skipped)
        });
        let withdrawn = cx.new(|cx| {
            ClarificationPanel::new(
                "scene.clarification.withdrawn",
                "Which branch should the fix land on?",
                window,
                cx,
            )
            .option(ClarificationOption::new("main", "main"))
            .option(ClarificationOption::new("release", "release/0.1"))
            .status(ClarificationStatus::Withdrawn(
                "the repository has only one branch".into(),
            ))
        });
        let superseded = cx.new(|cx| {
            ClarificationPanel::new(
                "scene.clarification.superseded",
                "Which test suite should run first?",
                window,
                cx,
            )
            .option(ClarificationOption::new("unit", "The unit tests"))
            .option(ClarificationOption::new("visual", "The visual gate"))
            .status(ClarificationStatus::Superseded {
                by: "a later question covering the whole gate".into(),
            })
        });
        let empty = cx.new(|cx| {
            ClarificationPanel::new(
                "scene.clarification.empty",
                "Which of the cached results should be reused?",
                window,
                cx,
            )
        });
        cx.set_global(SceneClarifications {
            one,
            several,
            answered,
            skipped,
            withdrawn,
            superseded,
            empty,
        });
    }
    let panels = cx.global::<SceneClarifications>();
    let one = panels.one.clone();
    let several = panels.several.clone();
    let answered = panels.answered.clone();
    let skipped = panels.skipped.clone();
    let withdrawn = panels.withdrawn.clone();
    let superseded = panels.superseded.clone();
    let empty = panels.empty.clone();
    let theme = cx.theme().clone();

    let asking = div()
        .column()
        .w(px(430.0))
        .gap(px(theme.spacing.md))
        .child(caption(
            &theme,
            "One answer, one gesture: picking is answering, so there is no confirm",
        ))
        .child(one)
        .child(caption(
            &theme,
            "Several accumulate, so these get a confirming control — and one candidate the agent can no longer take",
        ))
        .child(several)
        .child(caption(
            &theme,
            "A question with nothing to pick from says so, rather than drawing an empty form",
        ))
        .child(empty);

    let settled = div()
        .column()
        .w(px(430.0))
        .gap(px(theme.spacing.md))
        .child(caption(
            &theme,
            "Answered, left to the agent, withdrawn, and replaced are four different things",
        ))
        .child(answered)
        .child(skipped)
        .child(withdrawn)
        .child(superseded);

    stack(&theme)
        .child(
            div()
                .row()
                .items_start()
                .gap(px(theme.spacing.md))
                .child(asking)
                .child(settled),
        )
        .into_any_element()
}

pub(super) fn permission_matrix(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let actions = [
        PermissionAction::new("read", "Read files"),
        PermissionAction::new("write", "Write files"),
        PermissionAction::new("network", "Reach the network"),
    ];
    let subjects = [
        PermissionSubject::new("workspace", "This workspace")
            .cell("read", PermissionEntry::new(PermissionState::Allowed))
            .cell("write", PermissionEntry::new(PermissionState::Ask))
            .cell(
                "network",
                PermissionEntry::inherited(PermissionState::Denied, "the organisation policy"),
            ),
        PermissionSubject::new("scratch", "The scratch directory")
            .cell(
                "read",
                PermissionEntry::inherited(PermissionState::Allowed, "this workspace"),
            )
            .cell("write", PermissionEntry::new(PermissionState::Allowed))
            .cell(
                "network",
                PermissionEntry::inherited(PermissionState::Denied, "the organisation policy"),
            ),
        // A calculator has no files to read, which is not the same as being
        // refused them.
        PermissionSubject::new("calculator", "The calculator tool")
            .cell("network", PermissionEntry::new(PermissionState::Denied)),
    ];

    stack(&theme)
        .w(px(720.0))
        .child(
            PermissionMatrix::new("scene.permission.editable")
                .actions(actions.clone())
                .subjects(subjects.clone())
                .on_change(|_change, _window, _cx| {}),
        )
        .child(
            Divider::new()
                .id("scene.permission.rule.read-only")
                .label("The same permissions, shown rather than offered"),
        )
        .child(
            PermissionMatrix::new("scene.permission.read-only")
                .actions(actions)
                .subjects(subjects),
        )
        .into_any_element()
}

pub(super) fn cost_meter(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(
            CostMeter::new("scene.cost.meter")
                .label("This run")
                .line(CostLine::new(
                    "spend",
                    "Spend",
                    Reading::measured(1.24, "1.24 credits"),
                ))
                .line(CostLine::new(
                    "projected",
                    "Projected at this rate",
                    Reading::estimated(4.0, "4.00 credits"),
                ))
                .line(
                    CostLine::new(
                        "account",
                        "Account balance",
                        Reading::measured(112.0, "112.00 credits"),
                    )
                    .stale(LastVerified::at("09:41 today")),
                )
                .line(CostLine::new(
                    "storage",
                    "Storage",
                    Reading::unavailable_because("The billing host refused the request."),
                )),
        )
        .child(
            Divider::new()
                .id("scene.cost.rule.gauge")
                .label("A limit that is known, and one that is not"),
        )
        .child(
            ContextGauge::new(
                "scene.cost.context.known",
                Reading::measured(48_000.0, "48,000 tokens"),
            )
            .label("Context used")
            .limit(Limit::measured(128_000.0, "128,000 tokens")),
        )
        .child(
            ContextGauge::new(
                "scene.cost.context.unknown",
                Reading::estimated(48_000.0, "48,000 tokens"),
            )
            .label("Context used"),
        )
        .child(
            ContextGauge::new("scene.cost.context.unavailable", Reading::unavailable())
                .label("Context used")
                .limit(Limit::measured(128_000.0, "128,000 tokens")),
        )
        .into_any_element()
}

/// Every state a call can be in, side by side.
///
/// Two columns because a captured image is one screen: stacked, the refusal
/// fell below the fold, and a state nobody can see in the snapshot is a state
/// the snapshot does not guard.
pub(super) fn tool_call(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    div()
        .flex()
        .gap(px(theme.spacing.lg))
        .child(
            stack(&theme).w(px(460.0)).child(
            ToolCall::new("scene.tool.pending", ToolFamily::External, "mcp.dispatch")
                .summary("designer review")
                .arguments("{ \"target\": \"designer-review\" }")
                .state(ToolCallState::PendingApproval),
        )
        .child(
            ToolCall::new("scene.tool.running", ToolFamily::Network, "web.fetch")
                .summary("ampcode.com/manual")
                .arguments("{ \"url\": \"https://ampcode.com/manual\" }")
                .state(ToolCallState::Running)
                .elapsed("4.2 s"),
        )
        .child(
            ToolCall::new("scene.tool.succeeded", ToolFamily::Read, "workspace.read")
                .summary("README.md")
                .arguments("{ \"path\": \"README.md\" }")
                .state(ToolCallState::succeeded(
                    ToolBody::new(
                        "# GPUI Box\n\nProduct-neutral components.\n\nEvery word is replaceable.",
                    )
                    .max_lines(2),
                ))
                .expanded(true)
                .on_toggle(|_, _, _| {})
                .elapsed("0.3 s"),
        )
        .child(
            ToolCall::new("scene.tool.silent", ToolFamily::Edit, "workspace.touch")
                .summary("notes.md")
                .arguments("{ \"path\": \"notes.md\" }")
                .state(ToolCallState::succeeded_silently())
                .elapsed("0.1 s"),
        ),
        )
        .child(
            stack(&theme)
                .w(px(460.0))
                .child(
                    ToolCall::new("scene.tool.failed", ToolFamily::Edit, "workspace.write")
                        .summary("notes.md")
                        .arguments("{ \"path\": \"/read-only/notes.md\" }")
                        .state(ToolCallState::failed("Read only."))
                        .elapsed("0.2 s")
                        .on_retry(|_, _| {}),
                )
                // A refusal reads as a decision: it is not the failure above
                // it, and not the silent success in the other column.
                .child(
                    ToolCall::new("scene.tool.refused", ToolFamily::Shell, "shell.run")
                        .summary("rm -rf build")
                        .arguments("{ \"command\": \"rm -rf build\" }")
                        .state(ToolCallState::refused("Shell commands are not allowed.")),
                ),
        )
        .into_any_element()
}

pub(super) fn thinking(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(640.0))
        .child(
            ThinkingBlock::new(
                "scene.thinking.collapsed",
                Reasoning::present("The brief asks for two files, so read both before answering."),
            )
            .elapsed("2.1 s"),
        )
        // Reasoning that has finished and reasoning still arriving are the
        // same words in the same place, so the difference has to be shown.
        .child(
            ThinkingBlock::new(
                "scene.thinking.working",
                Reasoning::present("Reading the second file before answering."),
            )
            .thinking(true),
        )
        .child(
            ThinkingBlock::new(
                "scene.thinking.open",
                Reasoning::present(
                    "The brief asks for two files.\nRead both before answering.\nThen summarise.",
                ),
            )
            .expanded(true)
            .elapsed("8.4 s")
            .on_toggle(|_, _, _| {}),
        )
        // Withheld and absent are two different facts, and neither is the
        // collapsed block above.
        .child(ThinkingBlock::new(
            "scene.thinking.withheld",
            Reasoning::withheld("This connection does not hand over reasoning."),
        ))
        .child(ThinkingBlock::new(
            "scene.thinking.absent",
            Reasoning::Absent,
        ))
        .into_any_element()
}

// ------------------------------------------------- structured data and agents

pub(super) fn server_list(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .child(
            // Only the one with a catalogue is opened. Expanding the two empty
            // ones as well pushed disconnected, failed, and disabled out of
            // the captured screen, and a state no image shows is a state no
            // image guards.
            ServerList::new("scene.servers")
                .expanded_ids(&["workspace"])
                .selected("workspace")
                .servers([
                    ServerEntry::new("workspace", "Workspace tools")
                        .detail("Running beside this window")
                        .state(ServerState::Connected)
                        .offers([
                            Offering::tool("read", "Read a file")
                                .summary("Returns the contents of one file")
                                .qualifier("path, max_bytes"),
                            Offering::tool("write", "Write a file")
                                .summary("Replaces the contents of one file"),
                            Offering::skill("review", "Review a change")
                                .summary("Reads a diff and reports what it finds"),
                            Offering::resource("changelog", "Changelog")
                                .qualifier("workspace:/CHANGELOG.md"),
                        ]),
                    ServerEntry::new("index", "Search index")
                        .detail("Answered, and the answer was empty")
                        .state(ServerState::Connected)
                        .offers([]),
                    ServerEntry::new("archive", "Archive")
                        .detail("Nobody has asked it anything yet")
                        .state(ServerState::Connected),
                    ServerEntry::new("build", "Build runner")
                        .state(ServerState::Connecting)
                        .catalog(Catalog::Asking),
                    ServerEntry::new("notes", "Notes").state(ServerState::Disconnected),
                    ServerEntry::new("deploy", "Deployment").state(ServerState::Failed {
                        reason: "The connection was refused after three attempts.".into(),
                    }),
                    ServerEntry::new("telemetry", "Telemetry").state(ServerState::Disabled {
                        reason: Some("You turned this one off.".into()),
                    }),
                ])
                .on_select(|_, _, _| {})
                .on_retry(|_, _, _| {})
                .on_toggle(|_, _, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn offering_catalog(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let workspace = OfferingSource::new(
        "workspace",
        "Workspace tools",
        OfferingSourceState::Ready(vec![
            SearchableOffering::new(
                Offering::tool("read", "Read a file")
                    .summary("Returns the contents of one file")
                    .qualifier("path, max_bytes"),
                "read file contents path workspace",
            ),
            SearchableOffering::new(
                Offering::skill("review", "Review a change")
                    .summary("Reads a diff and reports what it finds"),
                "review change diff findings",
            ),
        ]),
    );
    let archive = OfferingSource::new(
        "archive",
        "Archive tools",
        OfferingSourceState::Stale {
            offerings: vec![
                SearchableOffering::new(
                    Offering::tool("read", "Read a file").summary("Reads one archived file"),
                    "read archived file contents",
                ),
                SearchableOffering::new(
                    Offering::resource("changelog", "Changelog").qualifier("archive:/CHANGELOG.md"),
                    "changelog changes release history",
                ),
            ],
            reason: "Archive refresh failed; showing the last verified results.".into(),
        },
    );
    stack(&theme)
        .w(px(560.0))
        .child(
            OfferingCatalog::new("scene.offering-catalog")
                .sources([workspace, archive])
                .selected(OfferingIdentity::new("workspace", "read"))
                .on_activate(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn prompt_builder(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(caption(
            &theme,
            "host-owned slots; activating one reports identity, never fills the prompt",
        ))
        .child(
            PromptBuilder::new("scene.prompt.loading", "Review template")
                .state(PromptBuilderState::Loading),
        )
        .child(
            PromptBuilder::new("scene.prompt.ready", "Review template")
                .body("Review {path} for {concern} and return a verdict.")
                .slots([
                    PromptSlot::new("path", "path").value("src/lib.rs"),
                    PromptSlot::new("concern", "concern"),
                ])
                .on_slot(|_, _, _| {}),
        )
        .child(
            PromptBuilder::new("scene.prompt.empty", "Review template")
                .state(PromptBuilderState::Empty),
        )
        .child(
            PromptBuilder::new("scene.prompt.unavailable", "Review template").state(
                PromptBuilderState::Unavailable("The template host refused the request.".into()),
            ),
        )
        .child(
            PromptBuilder::new("scene.prompt.error", "Review template").state(
                PromptBuilderState::Error("The template request failed.".into()),
            ),
        )
        .into_any_element()
}

pub(super) fn feedback_rating(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(420.0))
        .child(caption(
            &theme,
            "vote and reason tags are host-owned; a disabled control installs no handler",
        ))
        .child(
            FeedbackRating::new("scene.feedback.ready")
                .vote(Some(FeedbackVote::Up))
                .tags([("accurate", "Accurate"), ("incomplete", "Incomplete")])
                .current_tag("accurate")
                .on_vote(|_, _, _| {})
                .on_tag(|_, _, _| {}),
        )
        .child(FeedbackRating::new("scene.feedback.idle").on_vote(|_, _, _| {}))
        .child(FeedbackRating::new("scene.feedback.disabled").disabled(true))
        .into_any_element()
}

pub(super) fn artifact_preview(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(480.0))
        .child(caption(
            &theme,
            "a generated artifact is shown as the kind it is; a refusal stays a refusal",
        ))
        .child(
            ArtifactPreview::new("scene.artifact.ready", "patch.rs")
                .kind(ArtifactKind::Code)
                .language(Language::Rust)
                .body(
                    "/// Whether the run may be published.\n\
                     fn ready(state: &State) -> bool {\n    \
                         state.checks == 12 && !state.blocked\n\
                     }",
                )
                .state(ArtifactPreviewState::Ready),
        )
        .child(
            ArtifactPreview::new("scene.artifact.loading", "notes.md")
                .state(ArtifactPreviewState::Loading),
        )
        .child(
            ArtifactPreview::new("scene.artifact.empty", "notes.md")
                .state(ArtifactPreviewState::Empty),
        )
        .child(
            ArtifactPreview::new("scene.artifact.unavailable", "notes.md").state(
                ArtifactPreviewState::Unavailable("The artifact host is offline.".into()),
            ),
        )
        .child(
            ArtifactPreview::new("scene.artifact.error", "notes.md").state(
                ArtifactPreviewState::Error("The artifact parser rejected this result.".into()),
            ),
        )
        .into_any_element()
}

/// The plan a run is working through, at the size it is actually read: a run
/// of marks, and words spent once on the thing in flight.
pub(super) fn agent_plan(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(420.0))
        .child(caption(
            &theme,
            "Behind, in flight, ahead — and the two a plan cannot be honest without",
        ))
        .child(
            AgentPlan::new("scene.plan.running")
                .item(PlanItem::new("brief", "Read the brief").done())
                .item(PlanItem::new("search", "Search the workspace").done())
                .item(PlanItem::new("summarise", "Summarise what was found").doing())
                .item(PlanItem::new("publish", "Publish the summary"))
                .item(PlanItem::new("notify", "Notify the reviewers"))
                .on_pick(|_, _, _| {}),
        )
        .child(caption(
            &theme,
            "A plan that stopped, and one the agent decided against",
        ))
        .child(
            AgentPlan::new("scene.plan.stopped")
                .item(PlanItem::new("brief", "Read the brief").done())
                .item(
                    PlanItem::new("notify", "Notify the reviewers")
                        .blocked("The notification service did not respond."),
                )
                .item(
                    PlanItem::new("archive", "Archive the previous run")
                        .dropped("Nothing had been published yet."),
                )
                .item(PlanItem::new("close", "Close the run")),
        )
        .child(caption(&theme, "Nothing in flight, so nothing is said"))
        .child(
            AgentPlan::new("scene.plan.settled")
                .item(PlanItem::new("brief", "Read the brief").done())
                .item(PlanItem::new("close", "Close the run").done()),
        )
        .into_any_element()
}
