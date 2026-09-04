//! Persona components keep voice, dialogue, and portrait state caller-owned.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{IntoElement, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit::semantics::Role;
use gpui_kit_testkit::harness::Harness;

fn agent(execution: AgentExecutionState) -> AgentSnapshot {
    AgentSnapshot::new(AgentDescriptor::new("guide", "Lyra").role("Guide"))
        .presence(AgentPresence::Online)
        .execution(execution)
}

fn speaking() -> VoiceSample {
    VoiceSample::new(VoiceState::Speaking, 0.75, 0.55).expect("normalized fixture")
}

#[gpui::test]
fn voice_meter_publishes_the_host_sample_and_exact_replay_owns_no_clock(cx: &mut TestAppContext) {
    let sample = speaking();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        VoiceReactive::new("persona.voice", sample.clone())
            .sample_at(Duration::from_millis(420))
            .into_any_element()
    });

    let voice = harness.node("persona.voice").expect("voice meter");
    assert_eq!(voice.role, Role::Progress);
    assert_eq!(voice.value.as_deref(), Some("speaking"));
    assert_eq!(voice.value_min, Some(0.0));
    assert_eq!(voice.value_max, Some(1.0));
    assert_eq!(voice.value_now, Some(0.75));
    assert!(voice.busy);
    assert_eq!(
        harness.update(|window, cx| window.simulate_next_frame(cx)),
        0,
        "an exact voice sample schedules no animation"
    );
}

#[gpui::test]
fn reduced_motion_interrupts_a_live_voice_timeline(cx: &mut TestAppContext) {
    let sample = speaking();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        VoiceReactive::new("persona.voice", sample.clone()).into_any_element()
    });
    assert!(harness.update(|window, cx| window.simulate_next_frame(cx)) > 0);

    harness.update(|_, cx| cx.set_reduce_motion(true));
    harness.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    assert_eq!(
        harness.update(|window, cx| window.simulate_next_frame(cx)),
        0
    );
}

#[gpui::test]
fn portrait_keeps_expression_voice_and_execution_as_separate_facts(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        PersonaPortrait::new(
            "persona.portrait",
            agent(AgentExecutionState::Active(AgentActivity::Speaking)),
        )
        .expression(PersonaExpression::Warm)
        .voice(speaking())
        .sample_at(Duration::from_millis(420))
        .into_any_element()
    });

    let portrait = harness.node("persona.portrait").expect("portrait");
    assert_eq!(portrait.role, Role::Group);
    assert_eq!(portrait.text.as_deref(), Some("Lyra"));
    assert_eq!(portrait.value.as_deref(), Some("warm:speaking"));
    assert!(portrait.busy);
    assert_eq!(
        harness
            .node("persona.portrait.avatar")
            .expect("standard avatar remains the identity image")
            .value
            .as_deref(),
        Some("online:active")
    );
}

#[gpui::test]
fn dialogue_reports_available_choices_and_keeps_selection_caller_owned(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = calls.clone();
    let turn = DialogueTurn::markdown("turn-7", agent(AgentExecutionState::Idle), "Choose.")
        .choice(DialogueChoice::new("scout", "Scout").selected(true))
        .choice(DialogueChoice::new("sealed", "Open gate").unavailable("Key not verified"));
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        PersonaDialogue::new("persona.dialogue", turn.clone())
            .on_event(move |event, _, _| sink.borrow_mut().push(event.clone()))
            .into_any_element()
    });

    assert!(
        harness
            .node("persona.dialogue.choice.scout")
            .expect("available choice")
            .checked
            .expect("caller-owned selected choice publishes checked")
    );
    assert!(
        harness
            .node("persona.dialogue.choice.sealed")
            .expect("refused choice")
            .disabled
    );

    harness.click("persona.dialogue.choice.scout");
    assert_eq!(
        calls.borrow().as_slice(),
        [PersonaDialogueEvent::ChoiceRequested {
            turn_id: "turn-7".into(),
            choice_id: "scout".into(),
        }]
    );
    assert!(
        harness
            .node("persona.dialogue.choice.scout")
            .expect("request did not apply state")
            .checked
            .expect("selection remains caller-owned")
    );

    harness.click("persona.dialogue.choice.sealed");
    assert_eq!(
        calls.borrow().len(),
        1,
        "a refused choice installs no action"
    );
}

#[gpui::test]
fn dialogue_forwards_safe_markdown_requests_with_turn_identity(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = calls.clone();
    let turn = DialogueTurn::markdown(
        "turn-image",
        agent(AgentExecutionState::Idle),
        "![Map](world/map.png)",
    );
    let _harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        PersonaDialogue::new("persona.dialogue", turn.clone())
            .on_event(move |event, _, _| sink.borrow_mut().push(event.clone()))
            .into_any_element()
    });

    assert_eq!(
        calls.borrow().as_slice(),
        [PersonaDialogueEvent::Markdown {
            turn_id: "turn-image".into(),
            event: MarkdownEvent::ImageRequested {
                src: "world/map.png".into(),
                alt: "Map".into(),
            },
        }]
    );
}

#[gpui::test]
fn dialogue_places_the_portrait_at_reading_start_in_both_directions(cx: &mut TestAppContext) {
    let turn = DialogueTurn::markdown("turn", agent(AgentExecutionState::Idle), "Hello");
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        PersonaDialogue::new("persona.dialogue", turn.clone()).into_any_element()
    });

    let portrait_ltr = harness
        .node("persona.dialogue.portrait")
        .expect("portrait")
        .bounds
        .x;
    let body_ltr = harness
        .node("persona.dialogue.body")
        .expect("body")
        .bounds
        .x;
    assert!(portrait_ltr < body_ltr);

    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));
    let portrait_rtl = harness
        .node("persona.dialogue.portrait")
        .expect("portrait")
        .bounds
        .x;
    let body_rtl = harness
        .node("persona.dialogue.body")
        .expect("body")
        .bounds
        .x;
    assert!(portrait_rtl > body_rtl);
}
