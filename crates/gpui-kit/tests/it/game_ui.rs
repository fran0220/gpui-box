//! Game compositions preserve caller authority while owning reusable visual
//! mappings, topology validation, semantics, RTL, and reduced-motion behavior.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{IntoElement, ParentElement, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit::semantics::Role;
use gpui_kit_testkit::harness::Harness;

fn agent(id: &'static str, name: &'static str) -> AgentSnapshot {
    AgentSnapshot::new(AgentDescriptor::new(id, name).role("Companion"))
        .presence(AgentPresence::Online)
        .execution(AgentExecutionState::Idle)
}

fn fraction(value: f32) -> GameFraction {
    GameFraction::new(value).expect("normalized fixture")
}

fn party() -> PartySnapshot {
    PartySnapshot::new([
        PartyMember::new(agent("lyra", "Lyra"))
            .expression(PersonaExpression::Warm)
            .gauge(PartyGauge::known(
                "resolve",
                "Resolve",
                fraction(0.72),
                "72 / 100",
            )),
        PartyMember::new(agent("orin", "Orin")).gauge(PartyGauge::unknown("bond", "Bond")),
    ])
}

fn objectives() -> ObjectiveSnapshot {
    ObjectiveSnapshot::new([
        Objective::new("beacon", "Restore the beacon")
            .state(ObjectiveState::Active)
            .progress(fraction(0.64)),
        Objective::new("crystals", "Align the crystals")
            .parent("beacon")
            .state(ObjectiveState::Completed),
        Objective::new("signal", "Trace the signal")
            .parent("beacon")
            .state(ObjectiveState::Locked),
    ])
}

fn abilities() -> AbilitySet {
    AbilitySet::new([
        Ability::new("dash", "Phase dash")
            .charges(AbilityCharges::new(2, 3).expect("valid charges"))
            .shortcut("Q"),
        Ability::new("nova", "Nova pulse")
            .state(AbilityState::CoolingDown {
                remaining_fraction: fraction(0.4),
                remaining: "2.4s".into(),
            })
            .cost("30 focus"),
        Ability::new("gate", "Open gate")
            .state(AbilityState::Unavailable("Route is not verified".into())),
    ])
}

fn reward(state: RewardState) -> RewardSnapshot {
    RewardSnapshot::new("vault", "Beacon cache")
        .state(state)
        .item(RewardItem::new("shard", "Prism shard").quantity(3))
        .item(RewardItem::new("key", "Signal key").detail("Opens one relay"))
}

#[test]
fn normalized_game_values_reject_invalid_facts_instead_of_clamping() {
    assert_eq!(
        GameFraction::new(f32::NAN),
        Err(GameFractionError::NonFinite)
    );
    assert_eq!(GameFraction::new(1.01), Err(GameFractionError::OutOfRange));
    assert_eq!(
        AbilityCharges::new(4, 3),
        Err(AbilityChargesError::AboveMaximum)
    );
    assert_eq!(
        AbilityCharges::new(0, 0),
        Err(AbilityChargesError::ZeroMaximum)
    );
}

#[test]
fn every_ambiguous_or_dangling_identity_is_reported_once() {
    let duplicate_party = PartySnapshot::new([
        PartyMember::new(agent("lyra", "Lyra")),
        PartyMember::new(agent("lyra", "Duplicate")),
        PartyMember::new(agent("lyra", "Third")),
    ]);
    assert_eq!(
        duplicate_party.issues(),
        vec![PartyIssue::DuplicateMember("lyra".into())]
    );

    let duplicate_gauge = PartySnapshot::new([PartyMember::new(agent("lyra", "Lyra"))
        .gauge(PartyGauge::unknown("resolve", "Resolve"))
        .gauge(PartyGauge::unknown("resolve", "Other resolve"))
        .gauge(PartyGauge::unknown("resolve", "Third resolve"))]);
    assert_eq!(
        duplicate_gauge.issues(),
        vec![PartyIssue::DuplicateGauge {
            member: "lyra".into(),
            gauge: "resolve".into(),
        }]
    );

    assert_eq!(
        AbilitySet::new([
            Ability::new("dash", "Dash"),
            Ability::new("dash", "Duplicate"),
            Ability::new("dash", "Third"),
        ])
        .issues(),
        vec![AbilitySetIssue::DuplicateId("dash".into())]
    );
    assert_eq!(
        RewardSnapshot::new("vault", "Cache")
            .state(RewardState::Revealed)
            .item(RewardItem::new("key", "Key"))
            .item(RewardItem::new("key", "Duplicate"))
            .item(RewardItem::new("key", "Third"))
            .issues(),
        vec![RewardItemId::from("key")]
    );
}

#[test]
fn objective_topology_reports_duplicates_dangling_parents_and_cycles() {
    let snapshot = ObjectiveSnapshot::new([
        Objective::new("duplicate", "First"),
        Objective::new("duplicate", "Second").parent("missing"),
        Objective::new("dangling", "Dangling").parent("missing"),
        Objective::new("cycle-a", "Cycle A").parent("cycle-b"),
        Objective::new("cycle-b", "Cycle B").parent("cycle-a"),
    ]);
    let issues = snapshot.issues();
    assert_eq!(
        issues
            .iter()
            .filter(|issue| matches!(issue, ObjectiveIssue::DuplicateId(_)))
            .count(),
        1
    );
    assert!(issues.contains(&ObjectiveIssue::DanglingParent {
        objective: "dangling".into(),
        parent: "missing".into(),
    }));
    assert!(
        !issues.iter().any(|issue| matches!(
            issue,
            ObjectiveIssue::DanglingParent { objective, .. } if objective.as_str() == "duplicate"
        )),
        "an ambiguous identity is not projected into parent traversal"
    );
    assert!(issues.contains(&ObjectiveIssue::ParentCycle("cycle-a".into())));
    assert!(issues.contains(&ObjectiveIssue::ParentCycle("cycle-b".into())));
}

#[gpui::test]
fn party_selection_reports_identity_without_applying_selection(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = calls.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        PartyRoster::new("game.party", party())
            .selected("lyra")
            .on_event(move |event, _, _| sink.borrow_mut().push(event.clone()))
            .into_any_element()
    });

    assert!(harness.node("game.party.lyra").expect("member").selected);
    assert_eq!(
        harness.node("game.party.orin").expect("member").role,
        Role::Button
    );
    harness.click("game.party.orin");
    assert_eq!(
        calls.borrow().as_slice(),
        [PartyRosterEvent::SelectMember("orin".into())]
    );
    assert!(
        harness
            .node("game.party.lyra")
            .expect("caller selection remains")
            .selected
    );
    assert!(
        !harness
            .node("game.party.orin")
            .expect("request is not applied")
            .selected
    );
}

#[gpui::test]
fn malformed_party_and_objective_topology_render_issues_not_believable_rows(
    cx: &mut TestAppContext,
) {
    let invalid_party = PartySnapshot::new([
        PartyMember::new(agent("same", "First")),
        PartyMember::new(agent("same", "Second")),
    ]);
    let invalid_objectives = ObjectiveSnapshot::new([
        Objective::new("a", "A").parent("b"),
        Objective::new("b", "B").parent("a"),
    ]);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        gpui::div()
            .column()
            .child(PartyRoster::new("game.party", invalid_party.clone()))
            .child(ObjectiveTracker::new(
                "game.objectives",
                invalid_objectives.clone(),
            ))
            .into_any_element()
    });

    assert_eq!(
        harness.node("game.party.issues").expect("issue").role,
        Role::Status
    );
    assert_eq!(
        harness.node("game.objectives.issues").expect("issue").role,
        Role::Status
    );
    assert!(harness.node("game.party.same").is_none());
    assert!(harness.node("game.objectives.a").is_none());
}

#[gpui::test]
fn objective_indent_moves_to_reading_start_in_rtl(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        ObjectiveTracker::new("game.objectives", objectives()).into_any_element()
    });
    let root_ltr = harness.node("game.objectives.beacon").expect("root").bounds;
    let child_ltr = harness
        .node("game.objectives.crystals")
        .expect("child")
        .bounds;
    assert!(child_ltr.x > root_ltr.x);

    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));
    let root_rtl = harness.node("game.objectives.beacon").expect("root").bounds;
    let child_rtl = harness
        .node("game.objectives.crystals")
        .expect("child")
        .bounds;
    assert!(child_rtl.x + child_rtl.width < root_rtl.x + root_rtl.width);
}

#[gpui::test]
fn ability_bar_activates_only_ready_abilities_and_keeps_state_caller_owned(
    cx: &mut TestAppContext,
) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = calls.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        AbilityBar::new("game.abilities", abilities())
            .selected("dash")
            .on_event(move |event, _, _| sink.borrow_mut().push(event.clone()))
            .into_any_element()
    });

    assert!(
        harness
            .node("game.abilities.dash")
            .expect("ready ability")
            .checked
            .expect("selected button")
    );
    assert!(
        harness
            .node("game.abilities.nova")
            .expect("cooldown ability")
            .disabled
    );
    assert!(
        harness
            .node("game.abilities.gate")
            .expect("unavailable ability")
            .disabled
    );

    harness.click("game.abilities.dash");
    harness.click("game.abilities.nova");
    harness.click("game.abilities.gate");
    assert_eq!(
        calls.borrow().as_slice(),
        [AbilityBarEvent::Activate("dash".into())]
    );
    assert!(
        harness
            .node("game.abilities.dash")
            .expect("component did not consume charge")
            .checked
            .expect("selection remains caller-owned")
    );
}

#[gpui::test]
fn reward_reveal_reports_requests_and_exact_effect_samples_own_no_clock(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = calls.clone();
    let hidden = reward(RewardState::Hidden);
    let mut hidden_harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        RewardReveal::new("game.reward", hidden.clone())
            .on_event(move |event, _, _| sink.borrow_mut().push(event.clone()))
            .into_any_element()
    });
    hidden_harness.click("game.reward.reveal");
    assert_eq!(
        calls.borrow().as_slice(),
        [RewardRevealEvent::RevealRequested("vault".into())]
    );
    assert_eq!(
        hidden_harness
            .node("game.reward")
            .expect("hidden reward remains hidden")
            .value
            .as_deref(),
        Some("hidden")
    );

    let mut planner = EffectPlanner::new(EffectPolicy::new(EffectQuality::Cinematic));
    let effect = planner.plan(
        EffectEvent::new("vault-reveal", "game-ui", "vault", VisualCue::Reward),
        1,
        false,
    );
    let revealed = reward(RewardState::Revealed);
    let mut reveal_harness = Harness::new(cx, gpui_kit::install, move |_, cx| {
        cx.set_reduce_motion(true);
        RewardReveal::new("game.reward", revealed.clone())
            .effect(effect.clone())
            .sample_at(Duration::from_millis(620))
            .into_any_element()
    });
    assert!(
        reveal_harness.node("game.reward.item.shard").is_some(),
        "items appear only after the host supplies revealed state"
    );
    assert_eq!(
        reveal_harness.update(|window, cx| window.simulate_next_frame(cx)),
        0,
        "reduced motion and exact samples schedule no continuing animation"
    );
}
