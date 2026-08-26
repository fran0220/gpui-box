//! What a run is allowed to claim: `ToolCall`, `StepList`, `ThinkingBlock`.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, ParentElement, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Calls = Rc<RefCell<Vec<String>>>;

const SECRET: &str = "sk-live-not-a-real-key";

// ----------------------------------------------------------- tool call card

fn five_states(cx: &mut TestAppContext) -> Harness {
    Harness::new(cx, gpui_kit::install, |_, _| {
        gpui::div()
            .child(
                ToolCall::new("call.pending", ToolFamily::Read, "workspace.search")
                    .state(ToolCallState::PendingApproval),
            )
            .child(
                ToolCall::new("call.running", ToolFamily::Read, "workspace.index")
                    .state(ToolCallState::Running)
                    .elapsed("4.2 s"),
            )
            .child(
                ToolCall::new("call.succeeded", ToolFamily::Read, "workspace.read")
                    .state(ToolCallState::succeeded("one line"))
                    .elapsed("0.3 s"),
            )
            .child(
                ToolCall::new("call.silent", ToolFamily::Edit, "workspace.touch")
                    .state(ToolCallState::succeeded_silently())
                    .elapsed("0.1 s"),
            )
            .child(
                ToolCall::new("call.failed", ToolFamily::Edit, "workspace.write")
                    .state(ToolCallState::failed("The path is read only."))
                    .elapsed("0.2 s"),
            )
            .child(
                ToolCall::new("call.refused", ToolFamily::Shell, "shell.run")
                    .state(ToolCallState::refused("This workspace allows no shell.")),
            )
            .into_any_element()
    })
}

#[gpui::test]
fn each_of_the_five_states_presents_as_itself(cx: &mut TestAppContext) {
    let mut harness = five_states(cx);

    let states = [
        ("call.pending", "pending-approval"),
        ("call.running", "running"),
        ("call.succeeded", "succeeded"),
        ("call.failed", "failed"),
        ("call.refused", "refused"),
    ];
    for (id, expected) in states {
        let card = harness.node(id).expect("published");
        assert_eq!(card.role, Role::Group);
        assert_eq!(
            card.value.as_deref(),
            Some(expected),
            "{id} published the wrong state"
        );
    }

    assert!(
        harness.node("call.running").expect("published").busy,
        "a call in flight says so"
    );
    assert!(
        !harness.node("call.pending").expect("published").busy,
        "nothing is in flight while approval is outstanding"
    );

    assert_eq!(
        harness
            .node("call.pending.state")
            .expect("pending is named")
            .value
            .as_deref(),
        Some("pending-approval")
    );
    assert_eq!(
        harness
            .node("call.running.state")
            .expect("running is named")
            .value
            .as_deref(),
        Some("running")
    );
    assert!(
        harness.node("call.succeeded.state").is_none(),
        "a completed row is quiet; its root still publishes the exact state"
    );
}

#[gpui::test]
fn a_refusal_is_not_an_error_and_not_an_empty_result(cx: &mut TestAppContext) {
    let mut harness = five_states(cx);

    let refusal = harness.node("call.refused.refusal").expect("published");
    assert_eq!(
        refusal.text.as_deref(),
        Some("This workspace allows no shell."),
        "the host's words are shown as the host wrote them"
    );
    assert!(
        harness.node("call.refused.error").is_none(),
        "a refusal is not an error"
    );
    assert!(
        harness.node("call.refused.result").is_none(),
        "a refusal is not a result, empty or otherwise"
    );
    assert!(
        harness.node("call.refused.elapsed").is_none(),
        "nothing ran, so there is no duration to claim"
    );

    // The call that did run and returned nothing is a different row in a
    // different state, which is the confusion this component exists to stop.
    let silent = harness.node("call.silent").expect("published");
    assert_eq!(silent.value.as_deref(), Some("succeeded"));
    assert_eq!(
        harness
            .node("call.silent.result")
            .expect("published")
            .value
            .as_deref(),
        Some("nothing")
    );
    assert!(harness.node("call.refused").expect("published").value != silent.value);
}

#[gpui::test]
fn a_failed_call_keeps_its_error_on_screen(cx: &mut TestAppContext) {
    let mut harness = five_states(cx);

    let error = harness.node("call.failed.error").expect("published");
    assert_eq!(error.text.as_deref(), Some("The path is read only."));
    assert!(
        harness.node("call.failed.refusal").is_none(),
        "a failure is not a refusal"
    );
    assert_eq!(
        harness
            .node("call.failed.elapsed")
            .expect("published")
            .value
            .as_deref(),
        Some("0.2 s"),
        "a failure keeps everything else it knows too"
    );
}

#[gpui::test]
fn a_duration_nobody_stated_says_so_rather_than_reading_zero(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ToolCall::new("call.running", ToolFamily::Read, "workspace.index")
            .state(ToolCallState::Running)
            .into_any_element()
    });

    assert!(
        harness.node("call.running.elapsed").is_none(),
        "an unstated duration renders no zero or invented estimate"
    );
}

#[gpui::test]
fn a_truncated_body_says_how_much_it_left_out(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        gpui::div()
            .child(
                ToolCall::new("call.cut", ToolFamily::Read, "workspace.read")
                    .arguments(ToolBody::new("one\ntwo\nthree\nfour").max_lines(2))
                    .state(ToolCallState::succeeded(
                        ToolBody::new("alpha\nbeta\ngamma").max_lines(1),
                    ))
                    .expanded(true),
            )
            .child(
                ToolCall::new("call.whole", ToolFamily::Read, "workspace.read")
                    .arguments(ToolBody::new("one\ntwo"))
                    .state(ToolCallState::succeeded("only this"))
                    .expanded(true),
            )
            .into_any_element()
    });

    assert_eq!(
        harness
            .node("call.cut.arguments")
            .expect("published")
            .value
            .as_deref(),
        Some("2 of 4 lines shown")
    );
    assert_eq!(
        harness
            .node("call.cut.result")
            .expect("published")
            .value
            .as_deref(),
        Some("1 of 3 lines shown")
    );
    // A body that was not cut states its size rather than staying silent, so
    // "there is more" is read off the same line every time.
    assert_eq!(
        harness
            .node("call.whole.arguments")
            .expect("published")
            .value
            .as_deref(),
        Some("2 lines")
    );
    assert_eq!(
        harness
            .node("call.whole.result")
            .expect("published")
            .value
            .as_deref(),
        Some("1 line")
    );
}

#[gpui::test]
fn no_argument_or_result_content_reaches_the_semantic_tree(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ToolCall::new("call.secret", ToolFamily::Network, "workspace.connect")
            .arguments(format!("{{ \"token\": \"{SECRET}\" }}"))
            .state(ToolCallState::succeeded(format!("connected with {SECRET}")))
            .expanded(true)
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    let leaked = snapshot.nodes.iter().any(|node| {
        [
            node.text.as_deref(),
            node.value.as_deref(),
            node.labels.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|text| text.contains(SECRET))
    });
    assert!(
        !leaked,
        "caller-owned content must never reach a snapshot: only its shape"
    );
    assert_eq!(
        harness
            .node("call.secret.arguments")
            .expect("published")
            .value
            .as_deref(),
        Some("1 line"),
        "the shape is what is published instead"
    );
}

#[gpui::test]
fn only_a_failed_call_installs_the_retry_handler(cx: &mut TestAppContext) {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let failed = Rc::clone(&sink);
        let succeeded = Rc::clone(&sink);
        let refused = Rc::clone(&sink);
        gpui::div()
            .child(
                ToolCall::new("call.failed", ToolFamily::Edit, "workspace.write")
                    .state(ToolCallState::failed("The path is read only."))
                    .on_retry(move |_, _| failed.borrow_mut().push("failed".to_string())),
            )
            .child(
                ToolCall::new("call.succeeded", ToolFamily::Read, "workspace.read")
                    .state(ToolCallState::succeeded("one line"))
                    .on_retry(move |_, _| succeeded.borrow_mut().push("succeeded".to_string())),
            )
            .child(
                ToolCall::new("call.refused", ToolFamily::Shell, "shell.run")
                    .state(ToolCallState::refused("No shell here."))
                    .on_retry(move |_, _| refused.borrow_mut().push("refused".to_string())),
            )
            .into_any_element()
    });

    assert!(
        harness.node("call.succeeded.retry").is_none(),
        "there is nothing to try again about a call that worked"
    );
    assert!(
        harness.node("call.refused.retry").is_none(),
        "a decision is not retried by the card that reports it"
    );
    harness.click("call.failed.retry");
    assert_eq!(*calls.borrow(), vec!["failed".to_string()]);

    calls.borrow_mut().clear();
    harness.keystrokes("enter");
    assert_eq!(
        *calls.borrow(),
        vec!["failed".to_string()],
        "the standard retry control answers the keyboard as well as the pointer"
    );
}

#[gpui::test]
fn a_tool_row_reports_its_family_summary_and_requested_disclosure(cx: &mut TestAppContext) {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        ToolCall::new("call.search", ToolFamily::Read, "workspace.search")
            .summary("docs · validation")
            .arguments("{ \"query\": \"validation\" }")
            .on_toggle(move |open, _, _| sink.borrow_mut().push(format!("open:{open}")))
            .into_any_element()
    });

    let call = harness.node("call.search").expect("published");
    assert_eq!(call.description.as_deref(), Some("read"));
    assert_eq!(
        call.text.as_deref(),
        Some("workspace.search docs · validation")
    );
    let toggle = harness.node("call.search.toggle").expect("published");
    assert_eq!(toggle.role, Role::Button);
    assert_eq!(toggle.expanded, Some(false));
    assert!(harness.node("call.search.arguments").is_none());

    harness.click("call.search.toggle");
    assert_eq!(*calls.borrow(), vec!["open:true".to_string()]);
}

// ------------------------------------------------------------------ step list

fn run(length: RunLength) -> StepList {
    StepList::new("run.steps")
        .length(length)
        .step(Step::new("read", "Read the brief").state(StepState::Done))
        .step(Step::new("search", "Search the workspace").state(StepState::Running))
        .step(Step::new("summarise", "Summarise what was found"))
        .step(
            Step::new("publish", "Publish the summary")
                .state(StepState::Skipped("Publishing is turned off.".into())),
        )
        .step(
            Step::new("notify", "Notify the reviewers")
                .state(StepState::Failed("The service refused the request.".into())),
        )
}

#[gpui::test]
fn each_step_publishes_its_own_state_under_its_own_identity(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        run(RunLength::Known).into_any_element()
    });

    let expected = [
        ("run.steps.read", "done"),
        ("run.steps.search", "running"),
        ("run.steps.summarise", "pending"),
        ("run.steps.publish", "skipped"),
        ("run.steps.notify", "failed"),
    ];
    for (id, state) in expected {
        let step = harness.node(id).expect("published");
        assert_eq!(step.role, Role::Row);
        assert_eq!(step.value.as_deref(), Some(state));
    }
    assert!(harness.node("run.steps.search").expect("published").busy);
    assert_eq!(
        harness
            .node("run.steps")
            .expect("published")
            .value
            .as_deref(),
        Some("5")
    );
}

#[gpui::test]
fn a_skipped_step_and_a_failed_one_are_two_different_sentences(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        run(RunLength::Known).into_any_element()
    });

    let skipped = harness.node("run.steps.publish.reason").expect("published");
    assert_eq!(skipped.text.as_deref(), Some("Publishing is turned off."));
    assert_eq!(skipped.value.as_deref(), Some("skipped"));

    let failed = harness.node("run.steps.notify.reason").expect("published");
    assert_eq!(
        failed.text.as_deref(),
        Some("The service refused the request.")
    );
    assert_eq!(failed.value.as_deref(), Some("failed"));

    assert!(
        harness.node("run.steps.read.reason").is_none(),
        "a step that simply finished has nothing to explain"
    );
}

#[gpui::test]
fn a_run_of_unknown_length_renders_no_determinate_progress(cx: &mut TestAppContext) {
    let mut counted = Harness::new(cx, gpui_kit::install, |_, _| {
        run(RunLength::Known).into_any_element()
    });
    let known = counted.node("run.steps.progress").expect("published");
    assert_eq!(known.role, Role::Progress);
    assert_eq!(known.value_now, Some(0.2), "one of five steps is done");
    assert_eq!(known.value.as_deref(), Some("1 of 5"));

    let mut open = Harness::new(cx, gpui_kit::install, |_, _| {
        run(RunLength::Unknown).into_any_element()
    });
    let unknown = open.node("run.steps.progress").expect("published");
    assert!(unknown.busy, "the run is still going");
    assert_eq!(
        unknown.value_now, None,
        "a run nobody counted has no fraction to report"
    );
    assert_eq!(
        unknown.value.as_deref(),
        Some("1 step done"),
        "what is known is reported; what is not is not invented"
    );
}

#[gpui::test]
fn a_step_carries_the_tool_call_it_is_made_of(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        StepList::new("run.steps")
            .step(
                Step::new("search", "Search the workspace")
                    .state(StepState::Running)
                    .body(
                        ToolCall::new(
                            "run.steps.search.call",
                            ToolFamily::Read,
                            "workspace.search",
                        )
                        .arguments("{ \"query\": \"budget\" }")
                        .state(ToolCallState::Running),
                    ),
            )
            .into_any_element()
    });

    let call = harness.node("run.steps.search.call").expect("published");
    assert_eq!(call.value.as_deref(), Some("running"));
    assert_eq!(call.text.as_deref(), Some("workspace.search"));
}

// --------------------------------------------------------------- thinking

fn reasoning(cx: &mut TestAppContext) -> Harness {
    Harness::new(cx, gpui_kit::install, |_, _| {
        gpui::div()
            .child(ThinkingBlock::new(
                "turn.shut",
                Reasoning::present("Read both files before answering."),
            ))
            .child(
                ThinkingBlock::new(
                    "turn.open",
                    Reasoning::present("Read both files before answering."),
                )
                .expanded(true),
            )
            .child(ThinkingBlock::new(
                "turn.withheld",
                Reasoning::withheld("This connection does not hand over reasoning."),
            ))
            .child(ThinkingBlock::new("turn.absent", Reasoning::Absent))
            .into_any_element()
    })
}

#[gpui::test]
fn withheld_collapsed_and_absent_are_three_presentations(cx: &mut TestAppContext) {
    let mut harness = reasoning(cx);

    let shut = harness.node("turn.shut").expect("published");
    assert_eq!(shut.value.as_deref(), Some("present"));
    assert_eq!(shut.expanded, Some(false), "collapsed by default");
    assert!(
        harness.node("turn.shut.body").is_none(),
        "a closed block renders no body, so nothing invisible stays addressable"
    );

    let open = harness.node("turn.open").expect("published");
    assert_eq!(open.value.as_deref(), Some("present"));
    assert_eq!(open.expanded, Some(true));
    assert!(harness.node("turn.open.body").is_some());

    let withheld = harness.node("turn.withheld").expect("published");
    assert_eq!(withheld.value.as_deref(), Some("withheld"));
    assert_eq!(
        harness
            .node("turn.withheld.withheld")
            .expect("published")
            .text
            .as_deref(),
        Some("This connection does not hand over reasoning."),
        "whoever withheld it says why, in their own words"
    );

    let absent = harness.node("turn.absent").expect("published");
    assert_eq!(absent.value.as_deref(), Some("absent"));
    assert!(
        harness.node("turn.absent.withheld").is_none(),
        "reasoning nobody produced was not withheld by anybody"
    );

    let mut names = vec![
        shut.value.clone(),
        withheld.value.clone(),
        absent.value.clone(),
    ];
    names.dedup();
    assert_eq!(names.len(), 3, "three facts, three published names");
}

#[gpui::test]
fn the_type_keeps_the_two_absences_apart(cx: &mut TestAppContext) {
    // The API takes a `Reasoning`, never an `Option`, so "withheld" cannot be
    // reached by forgetting: a caller holding nothing has to say which of the
    // two nothings it holds. What a test can check is that the two are not
    // equal and do not render the same, and that reasoning which exists but is
    // empty is still reasoning that exists.
    assert_ne!(Reasoning::withheld("policy"), Reasoning::Absent);
    assert_ne!(Reasoning::present(String::new()), Reasoning::Absent);
    assert_eq!(Reasoning::present(String::new()).as_str(), "present");

    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ThinkingBlock::new("turn.empty", Reasoning::present(String::new()))
            .expanded(true)
            .into_any_element()
    });
    assert_eq!(
        harness
            .node("turn.empty")
            .expect("published")
            .value
            .as_deref(),
        Some("present"),
        "an empty answer is an answer"
    );
}

#[gpui::test]
fn a_block_with_nothing_to_open_installs_no_handler(cx: &mut TestAppContext) {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let withheld = Rc::clone(&sink);
        let absent = Rc::clone(&sink);
        let present = Rc::clone(&sink);
        gpui::div()
            .child(
                ThinkingBlock::new(
                    "turn.withheld",
                    Reasoning::withheld("This connection does not hand over reasoning."),
                )
                .on_toggle(move |next, _, _| {
                    withheld.borrow_mut().push(format!("withheld:{next}"))
                }),
            )
            .child(
                ThinkingBlock::new("turn.absent", Reasoning::Absent).on_toggle(
                    move |next, _, _| absent.borrow_mut().push(format!("absent:{next}")),
                ),
            )
            .child(
                ThinkingBlock::new("turn.present", Reasoning::present("Because.")).on_toggle(
                    move |next, _, _| present.borrow_mut().push(format!("present:{next}")),
                ),
            )
            .into_any_element()
    });

    assert_eq!(
        harness.node("turn.withheld").expect("published").role,
        Role::Text,
        "there is nothing to operate, so it is not a button"
    );
    assert_eq!(
        harness.node("turn.absent").expect("published").role,
        Role::Text
    );
    assert_eq!(
        harness.node("turn.present").expect("published").role,
        Role::Button
    );

    harness.click("turn.withheld");
    harness.click("turn.absent");
    assert!(
        calls.borrow().is_empty(),
        "a block that cannot open installs no handler at all"
    );

    harness.click("turn.present");
    assert_eq!(*calls.borrow(), vec!["present:true".to_string()]);
}

#[gpui::test]
fn reasoning_text_stays_out_of_the_snapshot(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ThinkingBlock::new(
            "turn.open",
            Reasoning::present(format!("The token is {SECRET}, so use it.")),
        )
        .expanded(true)
        .into_any_element()
    });

    let snapshot = harness.snapshot();
    assert!(
        !snapshot
            .nodes
            .iter()
            .any(|node| [node.text.as_deref(), node.value.as_deref()]
                .into_iter()
                .flatten()
                .any(|text| text.contains(SECRET))),
        "a model's reasoning is caller-owned content and never published"
    );
}
