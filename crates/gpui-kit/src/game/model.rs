//! Caller-owned facts consumed by game presentation components.

use std::collections::{HashMap, HashSet};
use std::fmt;

use gpui::{Hsla, SharedString};
use gpui_kit_assets::Icon;

use crate::agent::{AgentId, AgentSnapshot, PersonaExpression};

macro_rules! identity {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(SharedString);

        impl $name {
            pub fn new(value: impl Into<SharedString>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                self.0.as_ref()
            }
        }

        impl From<&'static str> for $name {
            fn from(value: &'static str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl From<SharedString> for $name {
            fn from(value: SharedString) -> Self {
                Self::new(value)
            }
        }
    };
}

identity!(ObjectiveId);
identity!(AbilityId);
identity!(RewardId);
identity!(RewardItemId);

/// A validated scalar in the closed interval from zero to one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameFraction(f32);

impl GameFraction {
    pub fn new(value: f32) -> Result<Self, GameFractionError> {
        if !value.is_finite() {
            return Err(GameFractionError::NonFinite);
        }
        if !(0.0..=1.0).contains(&value) {
            return Err(GameFractionError::OutOfRange);
        }
        Ok(Self(value))
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

/// Why a game fraction was rejected instead of clamped into a believable
/// progress fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameFractionError {
    NonFinite,
    OutOfRange,
}

impl fmt::Display for GameFractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => formatter.write_str("game fraction must be finite"),
            Self::OutOfRange => formatter.write_str("game fraction must be between 0 and 1"),
        }
    }
}

impl std::error::Error for GameFractionError {}

/// What is known about one party gauge such as health, guard, or mana.
#[derive(Debug, Clone, PartialEq)]
pub enum PartyGaugeState {
    Unknown,
    Known {
        fraction: GameFraction,
        display: SharedString,
    },
    Unavailable(SharedString),
}

/// One stable caller-named gauge on a party member.
#[derive(Debug, Clone, PartialEq)]
pub struct PartyGauge {
    pub(crate) id: SharedString,
    pub(crate) label: SharedString,
    pub(crate) state: PartyGaugeState,
}

impl PartyGauge {
    pub fn known(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        fraction: GameFraction,
        display: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state: PartyGaugeState::Known {
                fraction,
                display: display.into(),
            },
        }
    }

    pub fn unknown(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state: PartyGaugeState::Unknown,
        }
    }

    pub fn unavailable(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        reason: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state: PartyGaugeState::Unavailable(reason.into()),
        }
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// One caller-owned party member over the shared agent identity vocabulary.
#[derive(Debug, Clone, PartialEq)]
pub struct PartyMember {
    pub(crate) agent: AgentSnapshot,
    pub(crate) expression: PersonaExpression,
    pub(crate) image: Option<SharedString>,
    pub(crate) tint: Option<Hsla>,
    pub(crate) gauges: Vec<PartyGauge>,
}

impl PartyMember {
    pub fn new(agent: AgentSnapshot) -> Self {
        Self {
            agent,
            expression: PersonaExpression::Neutral,
            image: None,
            tint: None,
            gauges: Vec::new(),
        }
    }

    pub fn expression(mut self, expression: PersonaExpression) -> Self {
        self.expression = expression;
        self
    }

    pub fn image(mut self, image: impl Into<SharedString>) -> Self {
        self.image = Some(image.into());
        self
    }

    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn gauge(mut self, gauge: PartyGauge) -> Self {
        self.gauges.push(gauge);
        self
    }

    pub fn id(&self) -> &AgentId {
        &self.agent.descriptor.id
    }
}

/// A malformed party fact that must be shown instead of collapsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyIssue {
    DuplicateMember(AgentId),
    DuplicateGauge {
        member: AgentId,
        gauge: SharedString,
    },
}

/// The complete party snapshot consumed by [`PartyRoster`](super::PartyRoster).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PartySnapshot {
    pub(crate) members: Vec<PartyMember>,
}

impl PartySnapshot {
    pub fn new(members: impl IntoIterator<Item = PartyMember>) -> Self {
        Self {
            members: members.into_iter().collect(),
        }
    }

    pub fn issues(&self) -> Vec<PartyIssue> {
        let mut issues = Vec::new();
        let mut members = HashSet::new();
        let mut reported_members = HashSet::new();
        let mut reported_gauges = HashSet::new();
        for member in &self.members {
            let member_id = member.agent.descriptor.id.clone();
            if !members.insert(member_id.clone()) && reported_members.insert(member_id.clone()) {
                issues.push(PartyIssue::DuplicateMember(member_id.clone()));
            }
            let mut gauges = HashSet::new();
            for gauge in &member.gauges {
                if !gauges.insert(gauge.id.clone())
                    && reported_gauges.insert((member_id.clone(), gauge.id.clone()))
                {
                    issues.push(PartyIssue::DuplicateGauge {
                        member: member_id.clone(),
                        gauge: gauge.id.clone(),
                    });
                }
            }
        }
        issues
    }

    pub fn members(&self) -> &[PartyMember] {
        &self.members
    }
}

/// Caller-owned objective state. Rules decide it; the tracker only presents
/// it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ObjectiveState {
    Locked,
    #[default]
    Active,
    Completed,
    Failed(SharedString),
    Unavailable(SharedString),
}

impl ObjectiveState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Locked => "locked",
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Failed(_) => "failed",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

/// One stable objective and its optional parent relationship.
#[derive(Debug, Clone, PartialEq)]
pub struct Objective {
    pub(crate) id: ObjectiveId,
    pub(crate) title: SharedString,
    pub(crate) detail: Option<SharedString>,
    pub(crate) parent: Option<ObjectiveId>,
    pub(crate) state: ObjectiveState,
    pub(crate) progress: Option<GameFraction>,
}

impl Objective {
    pub fn new(id: impl Into<ObjectiveId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            parent: None,
            state: ObjectiveState::default(),
            progress: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn parent(mut self, parent: impl Into<ObjectiveId>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn state(mut self, state: ObjectiveState) -> Self {
        self.state = state;
        self
    }

    pub fn progress(mut self, progress: GameFraction) -> Self {
        self.progress = Some(progress);
        self
    }

    pub fn id(&self) -> &ObjectiveId {
        &self.id
    }
}

/// A malformed objective relationship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectiveIssue {
    DuplicateId(ObjectiveId),
    DanglingParent {
        objective: ObjectiveId,
        parent: ObjectiveId,
    },
    ParentCycle(ObjectiveId),
}

/// A caller-owned objective document with structural validation.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ObjectiveSnapshot {
    pub(crate) objectives: Vec<Objective>,
}

impl ObjectiveSnapshot {
    pub fn new(objectives: impl IntoIterator<Item = Objective>) -> Self {
        Self {
            objectives: objectives.into_iter().collect(),
        }
    }

    pub fn objectives(&self) -> &[Objective] {
        &self.objectives
    }

    pub fn issues(&self) -> Vec<ObjectiveIssue> {
        let mut issues = Vec::new();
        let mut counts = HashMap::<ObjectiveId, usize>::new();
        for objective in &self.objectives {
            *counts.entry(objective.id.clone()).or_default() += 1;
        }
        let mut reported_duplicates = HashSet::new();
        for objective in &self.objectives {
            if counts.get(&objective.id).copied().unwrap_or_default() > 1
                && reported_duplicates.insert(objective.id.clone())
            {
                issues.push(ObjectiveIssue::DuplicateId(objective.id.clone()));
            }
        }

        let known: HashMap<ObjectiveId, &Objective> = self
            .objectives
            .iter()
            .filter(|objective| counts.get(&objective.id) == Some(&1))
            .map(|objective| (objective.id.clone(), objective))
            .collect();
        for objective in &self.objectives {
            if counts.get(&objective.id) != Some(&1) {
                continue;
            }
            if let Some(parent) = &objective.parent
                && !known.contains_key(parent)
            {
                issues.push(ObjectiveIssue::DanglingParent {
                    objective: objective.id.clone(),
                    parent: parent.clone(),
                });
            }
        }

        let mut cycles = Vec::new();
        for objective in &self.objectives {
            if counts.get(&objective.id) != Some(&1) {
                continue;
            }
            let mut path = HashSet::new();
            let mut current = Some(objective.id.clone());
            while let Some(id) = current {
                if !path.insert(id.clone()) {
                    if !cycles.contains(&id) {
                        cycles.push(id);
                    }
                    break;
                }
                current = known.get(&id).and_then(|entry| entry.parent.clone());
            }
        }
        issues.extend(cycles.into_iter().map(ObjectiveIssue::ParentCycle));
        issues
    }

    pub(crate) fn depth(&self, objective: &Objective) -> usize {
        let by_id: HashMap<&ObjectiveId, &Objective> = self
            .objectives
            .iter()
            .map(|item| (&item.id, item))
            .collect();
        let mut depth = 0;
        let mut current = objective.parent.as_ref();
        while let Some(parent) = current {
            depth += 1;
            current = by_id.get(parent).and_then(|item| item.parent.as_ref());
        }
        depth
    }
}

/// Stable identity of an ability.
#[derive(Debug, Clone, PartialEq)]
pub struct Ability {
    pub(crate) id: AbilityId,
    pub(crate) label: SharedString,
    pub(crate) detail: Option<SharedString>,
    pub(crate) icon: Option<Icon>,
    pub(crate) shortcut: Option<SharedString>,
    pub(crate) cost: Option<SharedString>,
    pub(crate) charges: Option<AbilityCharges>,
    pub(crate) state: AbilityState,
}

impl Ability {
    pub fn new(id: impl Into<AbilityId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            icon: None,
            shortcut: None,
            cost: None,
            charges: None,
            state: AbilityState::Ready,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn cost(mut self, cost: impl Into<SharedString>) -> Self {
        self.cost = Some(cost.into());
        self
    }

    pub fn charges(mut self, charges: AbilityCharges) -> Self {
        self.charges = Some(charges);
        self
    }

    pub fn state(mut self, state: AbilityState) -> Self {
        self.state = state;
        self
    }

    pub fn id(&self) -> &AbilityId {
        &self.id
    }
}

/// Remaining and maximum charges for one ability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbilityCharges {
    current: usize,
    maximum: usize,
}

impl AbilityCharges {
    pub fn new(current: usize, maximum: usize) -> Result<Self, AbilityChargesError> {
        if maximum == 0 {
            return Err(AbilityChargesError::ZeroMaximum);
        }
        if current > maximum {
            return Err(AbilityChargesError::AboveMaximum);
        }
        Ok(Self { current, maximum })
    }

    pub fn current(self) -> usize {
        self.current
    }

    pub fn maximum(self) -> usize {
        self.maximum
    }
}

/// Why charge counts were rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityChargesError {
    ZeroMaximum,
    AboveMaximum,
}

impl fmt::Display for AbilityChargesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroMaximum => formatter.write_str("ability charge maximum must be positive"),
            Self::AboveMaximum => {
                formatter.write_str("ability charges cannot exceed their maximum")
            }
        }
    }
}

impl std::error::Error for AbilityChargesError {}

/// Caller-owned ability availability.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AbilityState {
    #[default]
    Ready,
    CoolingDown {
        remaining: SharedString,
        remaining_fraction: GameFraction,
    },
    Disabled(SharedString),
    Unavailable(SharedString),
}

impl AbilityState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::CoolingDown { .. } => "cooling-down",
            Self::Disabled(_) => "disabled",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

/// A duplicate ability fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbilitySetIssue {
    DuplicateId(AbilityId),
}

/// A caller-owned ability set.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AbilitySet {
    pub(crate) abilities: Vec<Ability>,
}

impl AbilitySet {
    pub fn new(abilities: impl IntoIterator<Item = Ability>) -> Self {
        Self {
            abilities: abilities.into_iter().collect(),
        }
    }

    pub fn abilities(&self) -> &[Ability] {
        &self.abilities
    }

    pub fn issues(&self) -> Vec<AbilitySetIssue> {
        let mut seen = HashSet::new();
        let mut reported = HashSet::new();
        let mut issues = Vec::new();
        for ability in &self.abilities {
            if !seen.insert(ability.id.clone()) && reported.insert(ability.id.clone()) {
                issues.push(AbilitySetIssue::DuplicateId(ability.id.clone()));
            }
        }
        issues
    }
}

/// One reward item. Quantity is a count, not a formatted string, so Kit can
/// present it consistently without interpreting the item.
#[derive(Debug, Clone, PartialEq)]
pub struct RewardItem {
    pub(crate) id: RewardItemId,
    pub(crate) label: SharedString,
    pub(crate) detail: Option<SharedString>,
    pub(crate) quantity: usize,
    pub(crate) icon: Option<Icon>,
    pub(crate) image: Option<SharedString>,
}

impl RewardItem {
    pub fn new(id: impl Into<RewardItemId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            quantity: 1,
            icon: None,
            image: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn quantity(mut self, quantity: usize) -> Self {
        self.quantity = quantity;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn image(mut self, image: impl Into<SharedString>) -> Self {
        self.image = Some(image.into());
        self
    }

    pub fn id(&self) -> &RewardItemId {
        &self.id
    }
}

/// Caller-owned reward reveal state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RewardState {
    #[default]
    Hidden,
    Revealed,
    Claimed,
    Unavailable(SharedString),
}

impl RewardState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Revealed => "revealed",
            Self::Claimed => "claimed",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

/// One reward composition and all caller-owned items it may reveal.
#[derive(Debug, Clone, PartialEq)]
pub struct RewardSnapshot {
    pub(crate) id: RewardId,
    pub(crate) title: SharedString,
    pub(crate) detail: Option<SharedString>,
    pub(crate) items: Vec<RewardItem>,
    pub(crate) state: RewardState,
}

impl RewardSnapshot {
    pub fn new(id: impl Into<RewardId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            items: Vec::new(),
            state: RewardState::Hidden,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn item(mut self, item: RewardItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn state(mut self, state: RewardState) -> Self {
        self.state = state;
        self
    }

    pub fn id(&self) -> &RewardId {
        &self.id
    }

    pub fn issues(&self) -> Vec<RewardItemId> {
        let mut seen = HashSet::new();
        let mut reported = HashSet::new();
        let mut issues = Vec::new();
        for item in &self.items {
            if !seen.insert(item.id.clone()) && reported.insert(item.id.clone()) {
                issues.push(item.id.clone());
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentDescriptor;

    #[test]
    fn fractions_and_charges_reject_impossible_facts() {
        assert_eq!(
            GameFraction::new(f32::INFINITY),
            Err(GameFractionError::NonFinite)
        );
        assert_eq!(GameFraction::new(-0.1), Err(GameFractionError::OutOfRange));
        assert_eq!(
            AbilityCharges::new(3, 2),
            Err(AbilityChargesError::AboveMaximum)
        );
        assert_eq!(
            AbilityCharges::new(0, 0),
            Err(AbilityChargesError::ZeroMaximum)
        );
    }

    #[test]
    fn duplicate_and_dangling_game_facts_are_reported() {
        let member = || PartyMember::new(AgentSnapshot::new(AgentDescriptor::new("same", "Same")));
        assert_eq!(
            PartySnapshot::new([member(), member()]).issues(),
            [PartyIssue::DuplicateMember("same".into())]
        );

        let objectives = ObjectiveSnapshot::new([
            Objective::new("same", "One"),
            Objective::new("same", "Two"),
            Objective::new("child", "Child").parent("missing"),
        ]);
        let issues = objectives.issues();
        assert!(issues.contains(&ObjectiveIssue::DuplicateId("same".into())));
        assert!(issues.contains(&ObjectiveIssue::DanglingParent {
            objective: "child".into(),
            parent: "missing".into(),
        }));

        assert_eq!(
            AbilitySet::new([Ability::new("same", "One"), Ability::new("same", "Two")]).issues(),
            [AbilitySetIssue::DuplicateId("same".into())]
        );
        assert_eq!(
            RewardSnapshot::new("reward", "Reward")
                .item(RewardItem::new("same", "One"))
                .item(RewardItem::new("same", "Two"))
                .issues(),
            [RewardItemId::from("same")]
        );
    }

    #[test]
    fn objective_cycles_are_reported() {
        let snapshot = ObjectiveSnapshot::new([
            Objective::new("a", "A").parent("b"),
            Objective::new("b", "B").parent("a"),
        ]);
        assert!(
            snapshot
                .issues()
                .iter()
                .any(|issue| matches!(issue, ObjectiveIssue::ParentCycle(_)))
        );
    }
}
