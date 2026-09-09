//! Native game facts and visual effects; no game rules, rewards, or services execute here.
use super::{Emit, KitSlots, Node, text};
use gpui::{AnyElement, App, IntoElement, SharedString, Window};
use gpui_kit::{
    agent::*,
    effects::*,
    game::*,
    motion::{Micro, MicroMark},
};
use serde_json::{Value, json};
use std::time::Duration;

pub(super) const COMPONENTS: &[&str] = &[
    "AbilityBar",
    "ObjectiveTracker",
    "PartyRoster",
    "RewardReveal",
    "EffectParticles",
    "CinematicEffect",
    "MicroMark",
];

pub(super) fn validate(node: &Node) -> anyhow::Result<()> {
    super::agent::validate(node)?;
    if node.component.as_deref() == Some("AbilityBar") {
        for ability in list(&node.props["abilities"]) {
            if let Some(charges) = ability.get("charges") {
                anyhow::ensure!(
                    charges["current"].as_u64() <= charges["maximum"].as_u64(),
                    "ability charges exceed maximum"
                );
            }
        }
    }
    Ok(())
}

fn string(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}
fn list(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}
fn fraction(value: &Value) -> GameFraction {
    GameFraction::new(value.as_f64().expect("validated fraction") as f32)
        .expect("bounded game fraction")
}
fn elapsed(value: &Value) -> Duration {
    Duration::from_millis(value.as_u64().expect("validated elapsed milliseconds"))
}
pub(super) fn expression(value: &Value) -> PersonaExpression {
    match value["kind"].as_str() {
        Some("warm") => PersonaExpression::Warm,
        Some("focused") => PersonaExpression::Focused,
        Some("concerned") => PersonaExpression::Concerned,
        Some("celebrating") => PersonaExpression::Celebrating,
        Some("custom") => PersonaExpression::Custom(string(value, "name").into()),
        _ => PersonaExpression::Neutral,
    }
}
pub(super) fn plan(value: &Value) -> EffectPlan {
    let cue = match value["cue"].as_str().expect("validated cue") {
        "arrival" => VisualCue::Arrival,
        "delegation" => VisualCue::Delegation,
        "handoff" => VisualCue::Handoff,
        "aggregation" => VisualCue::Aggregation,
        "success" => VisualCue::Success,
        "reward" => VisualCue::Reward,
        "attention" => VisualCue::Attention,
        "refusal" => VisualCue::Refusal,
        "failure" => VisualCue::Failure,
        _ => unreachable!("closed cue"),
    };
    let recipe = match value["recipe"].as_str().expect("validated recipe") {
        "arrival-halo" => EffectRecipe::ArrivalHalo,
        "delegation-trace" => EffectRecipe::DelegationTrace,
        "handoff-trace" => EffectRecipe::HandoffTrace,
        "aggregation-pulse" => EffectRecipe::AggregationPulse,
        "success-burst" => EffectRecipe::SuccessBurst,
        "reward-celebration" => EffectRecipe::RewardCelebration,
        "attention-pulse" => EffectRecipe::AttentionPulse,
        "refusal-mark" => EffectRecipe::RefusalMark,
        "failure-pulse" => EffectRecipe::FailurePulse,
        _ => unreachable!("closed recipe"),
    };
    let presentation = match value["presentation"]["kind"].as_str() {
        Some("animated") => EffectPresentation::Animated,
        Some("suppressed") => EffectPresentation::Suppressed(EffectSuppression::Replay),
        _ => EffectPresentation::Static(match value["presentation"]["reason"].as_str() {
            Some("quality") => EffectFallback::Quality,
            Some("budget") => EffectFallback::Budget,
            _ => EffectFallback::ReducedMotion,
        }),
    };
    EffectPlan {
        id: string(value, "id").into(),
        surface: string(value, "surface").into(),
        target: string(value, "target").into(),
        origin: value
            .get("origin")
            .and_then(Value::as_str)
            .map(|s| s.to_owned().into()),
        cue,
        recipe,
        presentation,
        seed: value["seed"].as_u64().expect("validated seed"),
    }
}

pub(super) fn render(
    node: &Node,
    _slots: KitSlots,
    _window: &mut Window,
    _cx: &mut App,
    emit: Emit,
) -> AnyElement {
    let id = SharedString::from(node.id.clone());
    match node.component.as_deref().unwrap_or_default() {
        "MicroMark" => MicroMark::new(
            id,
            match text(node, "kind").as_str() {
                "heartbeat" => Micro::Heartbeat,
                "bounce" => Micro::Bounce,
                "wobble" => Micro::Wobble,
                "pop" => Micro::Pop,
                _ => Micro::Sparkle,
            },
            text(node, "label"),
        )
        .into_any_element(),
        "EffectParticles" => {
            let mut control = EffectParticles::new(plan(&node.props["plan"]));
            if let Some(value) = node.props.get("sampleAt") {
                control = control.sample_at(elapsed(value));
            }
            control.into_any_element()
        }
        "CinematicEffect" => {
            let value = &node.props["unavailable"];
            let kind = match value["kind"]
                .as_str()
                .expect("validated cinematic error kind")
            {
                "runtime-unavailable" => DotLottieErrorKind::RuntimeUnavailable,
                "invalid-limits" => DotLottieErrorKind::InvalidLimits,
                "empty-asset" => DotLottieErrorKind::EmptyAsset,
                "encoded-size" => DotLottieErrorKind::EncodedSize,
                "archive-invalid" => DotLottieErrorKind::ArchiveInvalid,
                "archive-entries" => DotLottieErrorKind::ArchiveEntries,
                "archive-entry-size" => DotLottieErrorKind::ArchiveEntrySize,
                "archive-expanded-size" => DotLottieErrorKind::ArchiveExpandedSize,
                "archive-compression-ratio" => DotLottieErrorKind::ArchiveCompressionRatio,
                "animation-count" => DotLottieErrorKind::AnimationCount,
                "state-machine-count" => DotLottieErrorKind::StateMachineCount,
                "image-count" => DotLottieErrorKind::ImageCount,
                "image-size" => DotLottieErrorKind::ImageSize,
                "canvas-size" => DotLottieErrorKind::CanvasSize,
                "frame-rate" => DotLottieErrorKind::FrameRate,
                "frame-count" => DotLottieErrorKind::FrameCount,
                "duration" => DotLottieErrorKind::Duration,
                "unsupported-feature" => DotLottieErrorKind::UnsupportedFeature,
                "invalid-input" => DotLottieErrorKind::InvalidInput,
                "render-failed" => DotLottieErrorKind::RenderFailed,
                _ => unreachable!("closed dotlottie error"),
            };
            let mut control = CinematicEffect::new(id, plan(&node.props["plan"]))
                .unavailable(DotLottieError::new(kind, string(value, "detail")));
            if let Some(value) = node.props.get("sampleAt") {
                control = control.sample_at(elapsed(value));
            }
            control.into_any_element()
        }
        "AbilityBar" => {
            let abilities = list(&node.props["abilities"]).map(|value| {
                let state = &value["state"];
                let mut ability = Ability::new(string(value, "id"), string(value, "label")).state(
                    match state["kind"].as_str() {
                        Some("cooling-down") => AbilityState::CoolingDown {
                            remaining: string(state, "remaining").into(),
                            remaining_fraction: fraction(&state["remainingFraction"]),
                        },
                        Some("disabled") => AbilityState::Disabled(string(state, "reason").into()),
                        Some("unavailable") => {
                            AbilityState::Unavailable(string(state, "reason").into())
                        }
                        _ => AbilityState::Ready,
                    },
                );
                if value.get("detail").is_some() {
                    ability = ability.detail(string(value, "detail"));
                }
                if value.get("shortcut").is_some() {
                    ability = ability.shortcut(string(value, "shortcut"));
                }
                if value.get("cost").is_some() {
                    ability = ability.cost(string(value, "cost"));
                }
                if let Some(icon) = value.get("icon") {
                    ability =
                        ability.icon(super::icon::resolve(icon).expect("validated builtin icon"));
                }
                if let Some(charges) = value.get("charges") {
                    ability = ability.charges(
                        AbilityCharges::new(
                            charges["current"]
                                .as_u64()
                                .expect("validated current charges")
                                as usize,
                            charges["maximum"]
                                .as_u64()
                                .expect("validated maximum charges")
                                as usize,
                        )
                        .expect("validated charge counts"),
                    );
                }
                ability
            });
            let mut control = AbilityBar::new(id, AbilitySet::new(abilities));
            if node.props.contains_key("selected") {
                control = control.selected(text(node, "selected"));
            }
            if let Some(action) = node.events.get("activate").cloned() {
                control = control.on_event(move |AbilityBarEvent::Activate(id), _, _| {
                    emit(&action, json!(id.as_str()))
                });
            }
            control.into_any_element()
        }
        "ObjectiveTracker" => {
            let objectives = list(&node.props["objectives"]).map(|value| {
                let state = &value["state"];
                let mut objective = Objective::new(string(value, "id"), string(value, "title"))
                    .state(match state["kind"].as_str() {
                        Some("locked") => ObjectiveState::Locked,
                        Some("completed") => ObjectiveState::Completed,
                        Some("failed") => ObjectiveState::Failed(string(state, "reason").into()),
                        Some("unavailable") => {
                            ObjectiveState::Unavailable(string(state, "reason").into())
                        }
                        _ => ObjectiveState::Active,
                    });
                if value.get("detail").is_some() {
                    objective = objective.detail(string(value, "detail"));
                }
                if value.get("parent").is_some() {
                    objective = objective.parent(string(value, "parent"));
                }
                if let Some(value) = value.get("progress") {
                    objective = objective.progress(fraction(value));
                }
                objective
            });
            let mut control = ObjectiveTracker::new(id, ObjectiveSnapshot::new(objectives));
            if node.props.contains_key("selected") {
                control = control.selected(text(node, "selected"));
            }
            if let Some(action) = node.events.get("select").cloned() {
                control = control.on_event(move |ObjectiveTrackerEvent::Select(id), _, _| {
                    emit(&action, json!(id.as_str()))
                });
            }
            control.into_any_element()
        }
        "PartyRoster" => {
            let members = list(&node.props["members"]).map(|value| {
                let mut member = PartyMember::new(super::agent::decode(&value["agent"]));
                if let Some(value) = value.get("image") {
                    member = member.image_source(super::agent::image(value, _cx));
                }
                if let Some(value) = value.get("expression") {
                    member = member.expression(expression(value));
                }
                if let Some(value) = value.get("tint") {
                    member = member.tint(super::agent::tint(value));
                }
                for gauge in list(&value["gauges"]) {
                    let state = &gauge["state"];
                    member = member.gauge(match state["kind"].as_str() {
                        Some("known") => PartyGauge::known(
                            string(gauge, "id"),
                            string(gauge, "label"),
                            fraction(&state["fraction"]),
                            string(state, "display"),
                        ),
                        Some("unavailable") => PartyGauge::unavailable(
                            string(gauge, "id"),
                            string(gauge, "label"),
                            string(state, "reason"),
                        ),
                        _ => PartyGauge::unknown(string(gauge, "id"), string(gauge, "label")),
                    });
                }
                member
            });
            let mut control = PartyRoster::new(id, PartySnapshot::new(members));
            if node.props.contains_key("selected") {
                control = control.selected(text(node, "selected"));
            }
            if let Some(action) = node.events.get("selectMember").cloned() {
                control = control.on_event(move |PartyRosterEvent::SelectMember(id), _, _| {
                    emit(&action, json!(id.as_str()))
                });
            }
            control.into_any_element()
        }
        "RewardReveal" => {
            let value = &node.props["reward"];
            let state = &value["state"];
            let mut reward = RewardSnapshot::new(string(value, "id"), string(value, "title"))
                .state(match state["kind"].as_str() {
                    Some("revealed") => RewardState::Revealed,
                    Some("claimed") => RewardState::Claimed,
                    Some("unavailable") => RewardState::Unavailable(string(state, "reason").into()),
                    _ => RewardState::Hidden,
                });
            if value.get("detail").is_some() {
                reward = reward.detail(string(value, "detail"));
            }
            for value in list(&value["items"]) {
                let mut item = RewardItem::new(string(value, "id"), string(value, "label"))
                    .quantity(value["quantity"].as_u64().expect("validated quantity") as usize);
                if let Some(value) = value.get("image") {
                    item = item.image_source(super::agent::image(value, _cx));
                }
                if value.get("detail").is_some() {
                    item = item.detail(string(value, "detail"));
                }
                if let Some(icon) = value.get("icon") {
                    item = item.icon(super::icon::resolve(icon).expect("validated builtin icon"));
                }
                reward = reward.item(item);
            }
            let mut control = RewardReveal::new(id, reward);
            if let Some(value) = node.props.get("effect") {
                control = control.effect(plan(value));
            }
            if let Some(value) = node.props.get("sampleAt") {
                control = control.sample_at(elapsed(value));
            }
            let events = node.events.clone();
            if !events.is_empty() {
                control = control.on_event(move |event, _, _| {
                    let (name, id) = match event {
                        RewardRevealEvent::RevealRequested(id) => ("revealRequested", id),
                        RewardRevealEvent::ClaimRequested(id) => ("claimRequested", id),
                    };
                    if let Some(action) = events.get(name) {
                        emit(action, json!(id.as_str()));
                    }
                });
            }
            control.into_any_element()
        }
        _ => unreachable!("validated game/effects registration"),
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests;
