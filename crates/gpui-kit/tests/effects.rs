//! Semantic visual events choose policy-owned recipes and never make callers
//! hand-write accessibility, replay, or budget fallbacks.

use std::time::Duration;

use gpui::{IntoElement, ParentElement, Styled, TestAppContext, div, px};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

fn animated_plan(id: &'static str, cue: VisualCue) -> EffectPlan {
    let mut planner = EffectPlanner::new(EffectPolicy::new(EffectQuality::Cinematic));
    planner.plan(
        EffectEvent::new(id, "effects-test", "target", cue),
        1,
        false,
    )
}

#[gpui::test]
fn installed_policy_defaults_balanced_and_can_be_replaced(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| div().into_any_element());
    assert_eq!(
        harness.update(|_, cx| effect_policy(cx).quality),
        EffectQuality::Balanced
    );

    harness.update(|_, cx| set_effect_policy(EffectPolicy::new(EffectQuality::Off), cx));
    assert_eq!(
        harness.update(|_, cx| effect_policy(cx).quality),
        EffectQuality::Off
    );
    let plan = harness.update(|_, cx| {
        plan_effect(
            EffectEvent::new("arrival", "conversation", "agent", VisualCue::Arrival),
            cx,
        )
    });
    assert_eq!(
        plan.presentation,
        EffectPresentation::Static(EffectFallback::Quality)
    );
}

#[gpui::test]
fn active_reduced_motion_and_replay_policy_are_applied_centrally(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| div().into_any_element());
    harness.update(|_, cx| {
        set_effect_policy(EffectPolicy::new(EffectQuality::Cinematic), cx);
        cx.set_reduce_motion(true);
    });
    let static_plan = harness.update(|_, cx| {
        plan_effect(
            EffectEvent::new("reward", "game", "player", VisualCue::Reward),
            cx,
        )
    });
    assert_eq!(
        static_plan.presentation,
        EffectPresentation::Static(EffectFallback::ReducedMotion)
    );

    harness.update(|_, cx| cx.set_reduce_motion(false));
    let replay = harness.update(|_, cx| {
        plan_effect(
            EffectEvent::new("reward", "game", "player", VisualCue::Reward),
            cx,
        )
    });
    assert_eq!(
        replay.presentation,
        EffectPresentation::Suppressed(EffectSuppression::Replay),
        "changing accessibility policy does not replay an event"
    );
}

#[gpui::test]
fn particle_layer_owns_frame_scheduling_and_stops_for_exact_samples(cx: &mut TestAppContext) {
    let plan = animated_plan("animated", VisualCue::Reward);
    let animated_plan = plan.clone();
    let mut animated = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(240.0))
            .h(px(160.0))
            .child(EffectParticles::new(animated_plan.clone()))
            .into_any_element()
    });
    assert!(
        animated.update(|window, cx| window.simulate_next_frame(cx)) > 0,
        "a live policy-owned recipe requests its own frame"
    );

    let mut sampled = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(240.0))
            .h(px(160.0))
            .child(EffectParticles::new(plan.clone()).sample_at(Duration::from_millis(620)))
            .into_any_element()
    });
    assert_eq!(
        sampled.update(|window, cx| window.simulate_next_frame(cx)),
        0,
        "an exact replay sample owns no timeline"
    );
}

#[gpui::test]
fn active_reduced_motion_stops_an_already_animated_particle_plan(cx: &mut TestAppContext) {
    let plan = animated_plan("reduce-running", VisualCue::Success);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(240.0))
            .h(px(160.0))
            .child(EffectParticles::new(plan.clone()))
            .into_any_element()
    });
    harness.update(|_, cx| cx.set_reduce_motion(true));
    harness.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    assert_eq!(
        harness.update(|window, cx| window.simulate_next_frame(cx)),
        0,
        "the component rechecks accessibility instead of trusting a stale animated plan"
    );
}

#[test]
fn every_agent_event_maps_to_a_semantic_cue_and_names_typed_endpoints() {
    let mappings = [
        (AgentVisualEventKind::AgentSpawned, VisualCue::Arrival),
        (
            AgentVisualEventKind::DelegationStarted,
            VisualCue::Delegation,
        ),
        (AgentVisualEventKind::HandoffCommitted, VisualCue::Handoff),
        (
            AgentVisualEventKind::ResultAggregated,
            VisualCue::Aggregation,
        ),
        (AgentVisualEventKind::AgentSucceeded, VisualCue::Success),
        (AgentVisualEventKind::AgentRefused, VisualCue::Refusal),
        (AgentVisualEventKind::AgentFailed, VisualCue::Failure),
        (AgentVisualEventKind::RewardGranted, VisualCue::Reward),
    ];
    for (kind, expected) in mappings {
        let event = AgentVisualEvent::new(
            format!("event-{}", expected.importance() as u8),
            "run-canvas",
            kind,
            RunSubjectId::Task("same".into()),
        )
        .origin(RunSubjectId::Agent("same".into()))
        .effect_event();
        assert_eq!(event.cue, expected);
        assert_eq!(event.target.as_ref(), "4:task:same");
        assert_eq!(event.origin.as_deref(), Some("5:agent:same"));
        assert_ne!(event.origin.as_deref(), Some(event.target.as_ref()));
    }
}

#[test]
fn agent_visual_event_round_trips_without_an_effect_recipe() {
    let event = AgentVisualEvent::new(
        "handoff-7",
        "run-canvas",
        AgentVisualEventKind::HandoffCommitted,
        RunSubjectId::Agent("reviewer".into()),
    )
    .origin(RunSubjectId::Agent("researcher".into()));
    let json = serde_json::to_string(&event).expect("semantic event serializes");
    assert!(json.contains("handoff-committed"));
    assert!(!json.contains("trace"));
    assert!(!json.contains("particle"));
    assert_eq!(
        serde_json::from_str::<AgentVisualEvent>(&json).expect("semantic event deserializes"),
        event
    );
}
