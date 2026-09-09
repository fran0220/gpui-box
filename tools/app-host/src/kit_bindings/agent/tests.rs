use super::*;
use gpui::{ParentElement, Styled, TestAppContext, div, px};
use gpui_kit_testkit::harness::Harness;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

fn node(component: &str, id: &str, props: Value, events: Value) -> Node {
    serde_json::from_value(
        json!({"kind":"kit","component":component,"id":id,"props":props,"events":events}),
    )
    .expect("agent fixture node")
}

#[gpui::test]
fn every_agent_fixture_builds_native_semantics(cx: &mut TestAppContext) {
    let nodes: Vec<Node> =
        serde_json::from_str(include_str!("fixture/cases.json")).expect("agent fixtures");
    for node in nodes {
        super::super::validate_descriptor(&node).expect("valid agent descriptor");
        let name = node.component.clone().expect("fixture component");
        let state = State::default();
        let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
            state.render(&node, KitSlots::new(), window, cx, Rc::new(|_, _| {}))
        });
        assert!(!harness.snapshot().nodes.is_empty(), "{name}");
        harness.remount(|_, _| div().into_any_element());
    }
}

#[test]
fn native_queries_preserve_unknown_and_asymmetric_facts() {
    let plan = node(
        "AgentPlan",
        "plan",
        json!({"items":[
        {"id":"a","label":"One","state":"done"},
        {"id":"b","label":"Two","state":"blocked","reason":"Needs review"},
        {"id":"c","label":"Three","state":"doing"}]}),
        json!({}),
    );
    assert_eq!(
        invoke(&plan, "done", &json!({}), true).expect("done query"),
        json!(1)
    );
    assert!(invoke(&plan, "done", &json!({}), false).is_err());
    let mut gauge = node(
        "ContextGauge",
        "gauge",
        json!({"used":{"basis":"measured","amount":23,"text":"23"},"limit":{"basis":"estimated","amount":80,"text":"80"},"stale":"09:13"}),
        json!({}),
    );
    let result = invoke(&gauge, "fraction", &json!({}), true)
        .expect("fraction query")
        .as_f64()
        .expect("known fraction");
    assert!((result - 0.2875).abs() < 1e-6);
    gauge.props.remove("limit");
    assert_eq!(
        invoke(&gauge, "fraction", &json!({}), true).expect("unknown limit query"),
        Value::Null
    );
    gauge.props.insert(
        "used".into(),
        json!({"basis":"unavailable","reason":"Refused"}),
    );
    assert_eq!(
        invoke(&gauge, "fraction", &json!({}), true).expect("unavailable reading query"),
        Value::Null
    );
}

#[gpui::test]
fn native_agent_intents_preserve_exact_caller_payloads(cx: &mut TestAppContext) {
    for (index, target, action, expected) in [
        (0, "plan.publish", "pick", json!("publish")),
        (
            17,
            "servers.fixture-server.retry",
            "retry",
            json!("fixture-server"),
        ),
        (
            19,
            "permissions.fixture-subject.read",
            "change",
            json!({"subject":"fixture-subject","action":"read","next":"allowed"}),
        ),
        (
            22,
            "dialogue.choice.inspect",
            "choice",
            json!({"turnId":"fixture-turn","choiceId":"inspect"}),
        ),
    ] {
        let mut nodes: Vec<Node> =
            serde_json::from_str(include_str!("fixture/cases.json")).expect("agent fixtures");
        let node = nodes.remove(index);
        let output = Rc::new(RefCell::new(Vec::new()));
        let recorded = output.clone();
        let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
            let recorded = recorded.clone();
            render(
                &node,
                KitSlots::new(),
                window,
                cx,
                Rc::new(move |name, value| recorded.borrow_mut().push((name.to_owned(), value))),
            )
        });
        harness.click(target);
        assert_eq!(&*output.borrow(), &[(action.into(), expected)], "{target}");
        if index == 22 {
            harness.click("dialogue.choice.execute");
            assert_eq!(
                output.borrow().len(),
                1,
                "unavailable choice refuses handler"
            );
        }
        harness.remount(|_, _| div().into_any_element());
    }
}

#[gpui::test]
fn native_fallback_slots_build_fresh_elements_on_each_frame(cx: &mut TestAppContext) {
    for (component, slot, props) in [
        (
            "PromptBuilder",
            "empty",
            json!({"label":"Fixture","state":{"kind":"empty"}}),
        ),
        (
            "ArtifactPreview",
            "loading",
            json!({"title":"Fixture","state":{"kind":"loading"}}),
        ),
        (
            "ArtifactPreview",
            "empty",
            json!({"title":"Fixture","state":{"kind":"empty"}}),
        ),
        (
            "ArtifactPreview",
            "failed",
            json!({"title":"Fixture","state":{"kind":"error","reason":"Caller error"}}),
        ),
        ("ServerList", "empty", json!({"servers":[]})),
    ] {
        let node = node(component, "slotted", props, json!({}));
        let builds = Rc::new(std::cell::Cell::new(0));
        let counted = builds.clone();
        let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
            let counted = counted.clone();
            let mut slots = KitSlots::new();
            slots.insert(
                slot.into(),
                Rc::new(move |_, _| {
                    counted.set(counted.get() + 1);
                    Button::new("slot.child")
                        .label("Caller slot")
                        .into_any_element()
                }),
            );
            render(&node, slots, window, cx, Rc::new(|_, _| {}))
        });
        assert!(harness.node("slot.child").is_some(), "{component}.{slot}");
        harness.frame();
        assert!(builds.get() >= 2, "slot creates fresh elements");
        harness.remount(|_, _| div().into_any_element());
    }
}

#[gpui::test]
fn native_feedback_and_prompt_emit_caller_data_without_status_changes(cx: &mut TestAppContext) {
    let output = Rc::new(RefCell::new(Vec::new()));
    let recorded = output.clone();
    let nodes = [
        node(
            "FeedbackRating",
            "feedback",
            json!({"vote":"up","tags":[{"id":"wrong-source","label":"Wrong source"}]}),
            json!({"vote":"vote","tag":"tag"}),
        ),
        node(
            "FeedbackRating",
            "disabled",
            json!({"disabled":true}),
            json!({"vote":"forbidden"}),
        ),
        node(
            "PromptBuilder",
            "prompt",
            json!({"label":"Explicit fixture","state":{"kind":"ready"},"slots":[{"id":"language","name":"Language","value":"Rust"}]}),
            json!({"slot":"slot"}),
        ),
    ];
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let recorded = recorded.clone();
        let emit: Emit =
            Rc::new(move |action, value| recorded.borrow_mut().push((action.to_owned(), value)));
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .children(
                nodes
                    .iter()
                    .map(|node| render(node, BTreeMap::new(), window, cx, emit.clone())),
            )
            .into_any_element()
    });
    harness.click("feedback.down");
    harness.click("feedback.tag.wrong-source");
    harness.click("disabled.up");
    harness.click("prompt.slot.language");
    assert_eq!(
        &*output.borrow(),
        &[
            ("vote".into(), json!("down")),
            ("tag".into(), json!("wrong-source")),
            (
                "slot".into(),
                json!({"id":"language","name":"Language","value":"Rust"})
            ),
        ]
    );
    assert!(
        harness
            .node("disabled.up")
            .expect("disabled feedback")
            .disabled
    );
}

#[gpui::test]
fn retained_requests_run_every_method_and_refuse_resolved_or_unoffered_actions(
    cx: &mut TestAppContext,
) {
    let state = Rc::new(State::default());
    let build_state = state.clone();
    let output = Rc::new(RefCell::new(Vec::new()));
    let recorded = output.clone();
    let approval = node(
        "ApprovalPrompt",
        "approval",
        json!({"action":"Explicit fixture request","status":{"kind":"pending"},"always":[{"kind":"tool","subject":"inspect"}]}),
        json!({"approve":"approved","decline":"declined"}),
    );
    let question = node(
        "ClarificationPanel",
        "question",
        json!({"question":"Which fixture?","multiple":true,"skippable":true,"status":{"kind":"pending"},"options":[{"id":"west","label":"West","detail":"One"},{"id":"east","label":"East","unavailable":"Host refused"},{"id":"north","label":"North"}]}),
        json!({"answer":"answered","skip":"skipped"}),
    );
    let build_nodes = [approval.clone(), question.clone()];
    let mut harness =
        Harness::new(cx, gpui_kit::install, move |window, cx| {
            let recorded = recorded.clone();
            let emit: Emit =
                Rc::new(move |name, value| recorded.borrow_mut().push((name.to_owned(), value)));
            div()
                .flex()
                .flex_col()
                .children(build_nodes.iter().map(|node| {
                    build_state.render(node, KitSlots::new(), window, cx, emit.clone())
                }))
                .into_any_element()
        });
    harness.update(|window, cx| {
        assert_eq!(state.invoke(&approval, "current_status", &json!({}), true, window, cx).expect("initial status"), json!({"kind":"pending"}));
        assert!(state.invoke(&approval, "approve", &json!({"decision":{"kind":"always","scope":{"kind":"host","subject":"not-offered"}}}), false, window, cx).is_err());
        state.invoke(&approval, "approve", &json!({"decision":{"kind":"always","scope":{"kind":"tool","subject":"inspect"}}}), false, window, cx).expect("offered approval");
        state.invoke(&approval, "decline", &json!({}), false, window, cx).expect("decline intent");
        assert_eq!(state.invoke(&approval, "current_status", &json!({}), true, window, cx).expect("intent preserves status"), json!({"kind":"pending"}));
        state.invoke(&approval, "set_status", &json!({"status":{"kind":"expired"}}), false, window, cx).expect("caller expired status");
        assert!(state.invoke(&approval, "decline", &json!({}), false, window, cx).is_err());
        assert_eq!(state.invoke(&approval, "current_status", &json!({}), true, window, cx).expect("expired query"), json!({"kind":"expired"}));
        assert_eq!(state.invoke(&question, "candidates", &json!({}), true, window, cx).expect("candidates query"), question.props["options"]);
        assert!(state.invoke(&question, "answer", &json!({}), false, window, cx).is_err());
        assert!(state.invoke(&question, "choose", &json!({"id":"east"}), false, window, cx).is_err());
        state.invoke(&question, "choose", &json!({"id":"north"}), false, window, cx).expect("choose north");
        state.invoke(&question, "choose", &json!({"id":"west"}), false, window, cx).expect("choose west");
        assert_eq!(state.invoke(&question, "chosen", &json!({}), true, window, cx).expect("chosen query"), json!(["north","west"]));
        state.invoke(&question, "answer", &json!({}), false, window, cx).expect("answer intent");
        state.invoke(&question, "skip", &json!({}), false, window, cx).expect("skip intent");
        state.invoke(&question, "set_status", &json!({"status":{"kind":"withdrawn","reason":"New question"}}), false, window, cx).expect("caller withdrawal");
        assert_eq!(state.invoke(&question, "current_status", &json!({}), true, window, cx).expect("withdrawal query"), json!({"kind":"withdrawn","reason":"New question"}));
        assert!(state.invoke(&question, "choose", &json!({"id":"west"}), false, window, cx).is_err());
    });
    harness.frame();
    assert_eq!(
        &*output.borrow(),
        &[
            (
                "approved".into(),
                json!({"kind":"always","scope":{"kind":"tool","subject":"inspect"}})
            ),
            ("declined".into(), Value::Null),
            ("answered".into(), json!(["north", "west"])),
            ("skipped".into(), Value::Null),
        ]
    );
    let retained = state
        .retained
        .borrow()
        .get(&(0, "question".into()))
        .expect("retained question")
        .clone();
    harness.frame();
    harness.update(|window, cx| {
        assert!(Rc::ptr_eq(
            &retained,
            state
                .retained
                .borrow()
                .get(&(0, "question".into()))
                .expect("same retained question")
        ));
        let root: Node =
            serde_json::from_value(json!({"kind":"column","id":"empty","children":[]}))
                .expect("empty root");
        state.reconcile(&root, cx);
        assert!(
            state
                .invoke(&question, "chosen", &json!({}), true, window, cx)
                .is_err()
        );
    });
    drop(retained);
    harness.remount(|_, _| div().into_any_element());
    harness.update(|_, cx| {
        let root: Node =
            serde_json::from_value(json!({"kind":"column","id":"empty"})).expect("empty root");
        state.reconcile(&root, cx);
    });
    harness.frame();
}

#[gpui::test]
fn custom_image_sources_reach_native_agent_and_game_renderers(cx: &mut TestAppContext) {
    use gpui_kit::game::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let hits: Arc<[AtomicUsize; 7]> = Arc::new(std::array::from_fn(|_| AtomicUsize::new(0)));
    let counts = hits.clone();
    let source = move |index: usize| {
        let counts = counts.clone();
        gpui::ImageSource::Custom(Arc::new(move |_, _| {
            counts[index].fetch_add(1, Ordering::SeqCst);
            None
        }))
    };
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let agent = || AgentSnapshot::new(AgentDescriptor::new("fixture-agent", "Fixture"));
        div()
            .flex()
            .flex_col()
            .children([
                AgentAvatar::new("avatar", agent())
                    .image_source(source(0))
                    .into_any_element(),
                AgentCard::new("card", agent())
                    .image_source(source(1))
                    .into_any_element(),
                AgentGroup::new("group", [agent()])
                    .appearances([AgentAppearance::new("fixture-agent").image_source(source(2))])
                    .into_any_element(),
                PersonaPortrait::new("portrait", agent())
                    .image_source(source(3))
                    .into_any_element(),
                PersonaDialogue::new(
                    "dialogue",
                    DialogueTurn::new("turn", agent(), "Fixture body"),
                )
                .image_source(source(4))
                .into_any_element(),
                PartyRoster::new(
                    "party",
                    PartySnapshot::new([PartyMember::new(agent()).image_source(source(5))]),
                )
                .into_any_element(),
                RewardReveal::new(
                    "reward",
                    RewardSnapshot::new("reward-data", "Fixture reward")
                        .state(RewardState::Revealed)
                        .item(RewardItem::new("reward-item", "Art").image_source(source(6))),
                )
                .into_any_element(),
            ])
            .into_any_element()
    });
    harness.frame();
    for (index, count) in hits.iter().enumerate() {
        assert!(
            count.load(Ordering::SeqCst) > 0,
            "native image branch {index} invokes its own custom loader"
        );
    }
}
