//! Presentation a game shell needs and an application does not.

use super::support::*;

pub(super) fn game_ui(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let direction = cx.layout_direction();
    let normalized = |value| GameFraction::new(value).expect("scene fraction is normalized");
    let lyra = AgentSnapshot::new(AgentDescriptor::new("lyra", "Lyra").role("Pathfinder"))
        .presence(AgentPresence::Online)
        .execution(AgentExecutionState::Active(AgentActivity::UsingTool(
            "prism scanner".into(),
        )));
    let orin = AgentSnapshot::new(AgentDescriptor::new("orin", "Orin").role("Warden"))
        .presence(AgentPresence::Online)
        .execution(AgentExecutionState::Waiting(WaitReason::Dependency(
            "beacon".into(),
        )));
    let party = PartySnapshot::new([
        PartyMember::new(lyra)
            .expression(PersonaExpression::Focused)
            .gauge(PartyGauge::known(
                "resolve",
                "Resolve",
                normalized(0.72),
                "72 / 100",
            )),
        PartyMember::new(orin)
            .expression(PersonaExpression::Neutral)
            .gauge(PartyGauge::unknown("bond", "Bond"))
            .gauge(PartyGauge::unavailable(
                "aegis",
                "Aegis",
                "Telemetry was not verified",
            )),
    ]);
    let objectives = ObjectiveSnapshot::new([
        Objective::new("beacon", "Restore the beacon")
            .detail("Bring the relay online without inventing an outcome.")
            .state(ObjectiveState::Active)
            .progress(normalized(0.64)),
        Objective::new("crystals", "Align the crystals")
            .parent("beacon")
            .state(ObjectiveState::Completed),
        Objective::new("signal", "Trace the signal")
            .parent("beacon")
            .state(ObjectiveState::Locked),
        Objective::new("archive", "Recover the archive")
            .parent("beacon")
            .state(ObjectiveState::Failed(
                "The archive host refused access.".into(),
            )),
    ]);
    let abilities = AbilitySet::new([
        Ability::new("dash", "Phase dash")
            .icon(Icon::ArrowRight)
            .shortcut("Q")
            .charges(AbilityCharges::new(2, 3).expect("scene charges are valid")),
        Ability::new("nova", "Nova pulse")
            .icon(Icon::AddCircle)
            .shortcut("W")
            .cost("30 focus")
            .state(AbilityState::CoolingDown {
                remaining_fraction: normalized(0.4),
                remaining: "2.4s".into(),
            }),
        Ability::new("ward", "Signal ward")
            .icon(Icon::Key)
            .shortcut("E")
            .state(AbilityState::Disabled("No open target".into())),
        Ability::new("gate", "Open gate")
            .icon(Icon::Global)
            .shortcut("R")
            .state(AbilityState::Unavailable(
                "The route has not been verified".into(),
            )),
    ]);
    let reward = RewardSnapshot::new("beacon-cache", "Beacon cache")
        .detail("The host has confirmed this reveal; claiming remains a request.")
        .state(RewardState::Revealed)
        .item(
            RewardItem::new("shard", "Prism shard")
                .icon(Icon::Widget)
                .quantity(3),
        )
        .item(
            RewardItem::new("key", "Signal key")
                .icon(Icon::Key)
                .detail("Opens one relay"),
        );
    let mut planner = EffectPlanner::new(EffectPolicy::new(EffectQuality::Cinematic));
    let reward_effect = planner.plan(
        EffectEvent::new(
            "beacon-cache-revealed",
            "game-ui",
            "beacon-cache",
            VisualCue::Reward,
        ),
        1,
        false,
    );
    let heading = |text: &'static str| {
        div()
            .type_scale(&theme, TypeScale::Subtitle)
            .text_tone(&theme, TextTone::Primary)
            .child(text)
    };

    stack(&theme)
        .size_full()
        .items_center()
        .justify_center()
        .child(
            div()
                .column()
                .w(px(820.0))
                .gap_token(&theme, Space::Lg)
                .child(
                    div()
                        .column()
                        .gap_token(&theme, Space::Xs)
                        .child(
                            div()
                                .type_scale(&theme, TypeScale::Title)
                                .text_tone(&theme, TextTone::Primary)
                                .child("Expedition interface"),
                        )
                        .child(
                            div()
                                .type_scale(&theme, TypeScale::Caption)
                                .text_tone(&theme, TextTone::Muted)
                                .child(
                                    "Typed facts in, typed requests out; progression stays with the host.",
                                ),
                        ),
                )
                .child(
                    div()
                        .column()
                        .gap_token(&theme, Space::Sm)
                        .child(heading("Party"))
                        .child(
                            PartyRoster::new("scene.game-ui.party", party)
                                .selected("lyra")
                                .on_event(|_, _, _| {}),
                        ),
                )
                .child(
                    div()
                        .row_reading(direction)
                        .items_start()
                        .gap_token(&theme, Space::Lg)
                        .child(
                            div()
                                .column()
                                .w(px(400.0))
                                .gap_token(&theme, Space::Sm)
                                .child(heading("Objectives"))
                                .child(
                                    ObjectiveTracker::new(
                                        "scene.game-ui.objectives",
                                        objectives,
                                    )
                                    .selected("beacon")
                                    .on_event(|_, _, _| {}),
                                ),
                        )
                        .child(
                            div()
                                .column()
                                .w(px(400.0))
                                .gap_token(&theme, Space::Sm)
                                .child(heading("Reward"))
                                .child(
                                    RewardReveal::new("scene.game-ui.reward", reward)
                                        .effect(reward_effect)
                                        .sample_at(Duration::from_millis(620))
                                        .on_event(|_, _, _| {}),
                                ),
                        ),
                )
                .child(
                    div()
                        .column()
                        .gap_token(&theme, Space::Sm)
                        .child(heading("Abilities"))
                        .child(
                            AbilityBar::new("scene.game-ui.abilities", abilities)
                                .selected("dash")
                                .on_event(|_, _, _| {}),
                        ),
                ),
        )
        .into_any_element()
}
