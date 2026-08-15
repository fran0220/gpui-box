//! Semantic visual events choose policy-owned recipes and never make callers
//! hand-write accessibility, replay, or budget fallbacks.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    DevicePixels, IntoElement, ParentElement, RenderImage, Styled, TestAppContext, div, px, size,
};
use gpui_kit::prelude::*;
use gpui_kit::semantics::Role;
use gpui_kit_testkit::harness::Harness;

fn animated_plan(id: &'static str, cue: VisualCue) -> EffectPlan {
    let mut planner = EffectPlanner::new(EffectPolicy::new(EffectQuality::Cinematic));
    planner.plan(
        EffectEvent::new(id, "effects-test", "target", cue),
        1,
        false,
    )
}

#[derive(Debug)]
struct RecordingClip {
    samples: Rc<RefCell<Vec<DotLottieSample>>>,
}

impl DotLottieClip for RecordingClip {
    fn metadata(&self) -> DotLottieMetadata {
        DotLottieMetadata {
            width: 1,
            height: 1,
            frame_rate_millihertz: 60_000,
            frame_count: 60,
            duration: Duration::from_secs(1),
            animation_count: 1,
            state_machine_count: 0,
        }
    }

    fn render(&self, sample: DotLottieSample) -> Result<Arc<RenderImage>, DotLottieError> {
        self.samples.borrow_mut().push(sample);
        Ok(Arc::new(
            RenderImage::from_rgba(
                size(DevicePixels(1), DevicePixels(1)),
                vec![70, 210, 245, 255],
            )
            .expect("valid recording frame"),
        ))
    }
}

fn recording_clip() -> (Rc<RefCell<Vec<DotLottieSample>>>, Rc<dyn DotLottieClip>) {
    let samples = Rc::new(RefCell::new(Vec::new()));
    let clip = Rc::new(RecordingClip {
        samples: samples.clone(),
    });
    (samples, clip)
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

#[gpui::test]
fn exact_cinematic_samples_own_no_timeline_and_mirror_direction_in_rtl(cx: &mut TestAppContext) {
    let plan = animated_plan("cinematic-handoff", VisualCue::Handoff);
    let (samples, clip) = recording_clip();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(240.0))
            .h(px(160.0))
            .child(
                CinematicEffect::new("cinematic.handoff", plan.clone())
                    .clip(clip.clone())
                    .sample_at(Duration::from_millis(575)),
            )
            .into_any_element()
    });

    let node = harness.node("cinematic.handoff").expect("semantic image");
    assert_eq!(node.role, Role::Image);
    assert_eq!(node.value.as_deref(), Some("adapter-frame"));
    let sample = *samples.borrow().last().expect("clip sampled");
    assert_eq!(sample.progress_per_mille(), 500);
    assert!(!sample.mirror_x());
    assert_eq!(
        harness.update(|window, cx| window.simulate_next_frame(cx)),
        0,
        "an exact cinematic sample schedules no frame"
    );

    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));
    harness.snapshot();
    let sample = *samples.borrow().last().expect("RTL clip sampled");
    assert_eq!(sample.progress_per_mille(), 500);
    assert!(sample.mirror_x());
}

#[gpui::test]
fn reduced_motion_uses_the_semantic_poster_and_stops_cinematic_playback(cx: &mut TestAppContext) {
    let plan = animated_plan("cinematic-reduced", VisualCue::Reward);
    let (samples, clip) = recording_clip();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(240.0))
            .h(px(160.0))
            .child(CinematicEffect::new("cinematic.poster", plan.clone()).clip(clip.clone()))
            .into_any_element()
    });
    harness.update(|_, cx| cx.set_reduce_motion(true));

    let node = harness.node("cinematic.poster").expect("semantic poster");
    assert_eq!(node.value.as_deref(), Some("poster"));
    assert_eq!(
        samples
            .borrow()
            .last()
            .expect("poster sampled")
            .progress_per_mille(),
        CinematicRecipe::Reward.poster_progress_per_mille()
    );
    harness.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    assert_eq!(
        harness.update(|window, cx| window.simulate_next_frame(cx)),
        0,
        "the poster owns no timeline"
    );
}

#[gpui::test]
fn unavailable_cinematic_assets_publish_a_typed_particle_fallback(cx: &mut TestAppContext) {
    let plan = animated_plan("cinematic-invalid", VisualCue::Success);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(240.0))
            .h(px(160.0))
            .child(
                CinematicEffect::new("cinematic.invalid", plan.clone())
                    .unavailable(DotLottieError::new(
                        DotLottieErrorKind::ArchiveInvalid,
                        "secret host parser detail",
                    ))
                    .sample_at(Duration::from_millis(500)),
            )
            .into_any_element()
    });

    let node = harness
        .node("cinematic.invalid")
        .expect("semantic fallback");
    assert_eq!(node.value.as_deref(), Some("fallback-archive-invalid"));
    assert!(
        !node
            .description
            .as_deref()
            .unwrap_or_default()
            .contains("secret"),
        "host parser details never enter semantic snapshots"
    );
}

#[gpui::test]
fn cinematic_timeline_owns_frames_and_suppressed_replays_render_nothing(cx: &mut TestAppContext) {
    let live_plan = animated_plan("cinematic-live", VisualCue::Arrival);
    let (live_samples, live_clip) = recording_clip();
    let mut live = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(240.0))
            .h(px(160.0))
            .child(
                CinematicEffect::new("cinematic.live", live_plan.clone()).clip(live_clip.clone()),
            )
            .into_any_element()
    });
    assert!(
        live.update(|window, cx| window.simulate_next_frame(cx)) > 0,
        "a live adapter-backed recipe requests its own frame"
    );
    assert!(!live_samples.borrow().is_empty());

    let mut planner = EffectPlanner::new(EffectPolicy::new(EffectQuality::Cinematic));
    let event = EffectEvent::new(
        "cinematic-replay",
        "effects-test",
        "target",
        VisualCue::Reward,
    );
    let _ = planner.plan(event.clone(), 1, false);
    let replay = planner.plan(event, 1, false);
    let (replay_samples, replay_clip) = recording_clip();
    let mut suppressed = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(240.0))
            .h(px(160.0))
            .child(
                CinematicEffect::new("cinematic.replay", replay.clone()).clip(replay_clip.clone()),
            )
            .into_any_element()
    });
    assert!(suppressed.node("cinematic.replay").is_none());
    assert!(replay_samples.borrow().is_empty());
}

#[test]
fn dotlottie_requests_are_typed_intents_and_do_not_claim_outcomes() {
    let input = DotLottieInput::new("mood.intensity", DotLottieInputValue::Number(0.72))
        .expect("bounded input");
    let requests = [
        DotLottieRequest::Play,
        DotLottieRequest::Pause,
        DotLottieRequest::Stop,
        DotLottieRequest::Seek(Duration::from_millis(420)),
        DotLottieRequest::Input(input.clone()),
    ];

    assert_eq!(input.name(), "mood.intensity");
    assert_eq!(input.value(), DotLottieInputValue::Number(0.72));
    assert_eq!(requests.last(), Some(&DotLottieRequest::Input(input)));
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
