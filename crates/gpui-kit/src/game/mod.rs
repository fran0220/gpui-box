//! Product-neutral game and character compositions.
//!
//! The host owns rules, outcomes, inventory, persistence, and input mapping.
//! This module consumes snapshots and reports typed intent; it owns the
//! reusable party, objective, ability, reward, motion, and effect presentation
//! that downstream character and game surfaces would otherwise rebuild.

mod model;
mod presentation;

pub use model::{
    Ability, AbilityCharges, AbilityChargesError, AbilityId, AbilitySet, AbilitySetIssue,
    AbilityState, GameFraction, GameFractionError, Objective, ObjectiveId, ObjectiveIssue,
    ObjectiveSnapshot, ObjectiveState, PartyGauge, PartyGaugeState, PartyIssue, PartyMember,
    PartySnapshot, RewardId, RewardItem, RewardItemId, RewardSnapshot, RewardState,
};
pub use presentation::{
    AbilityBar, AbilityBarEvent, ObjectiveTracker, ObjectiveTrackerEvent, PartyRoster,
    PartyRosterEvent, RewardReveal, RewardRevealEvent,
};
