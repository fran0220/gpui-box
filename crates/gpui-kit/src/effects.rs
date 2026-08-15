//! Semantic visual events resolved through one quality and budget policy.
//!
//! Callers report what happened — arrival, delegation, handoff, aggregation,
//! success, refusal, failure, or reward — rather than naming particles,
//! colours, shaders, or animation timings. [`plan_effect`] chooses a stable
//! recipe and degrades it to a static presentation when motion is reduced,
//! the active quality tier excludes it, or a frame/surface budget is spent.
//!
//! Event identity is consumed once. Rebuilding a component or reconnecting a
//! stream therefore does not replay a celebration. The history is bounded by
//! [`EffectPolicy::replay_capacity`], and budgets reset on the semantic frame
//! generation installed by [`crate::install`].

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use gpui::{App, Global, SharedString};

use crate::motion::keyed;

/// How much optional visual richness the host permits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EffectQuality {
    /// No animation. Semantic events still resolve to static feedback.
    Off,
    /// Animate only events needed to direct attention or explain failure.
    Essential,
    /// Animate essential and expressive product events, but not spectacle.
    #[default]
    Balanced,
    /// Permit decorative reward and celebration treatments as well.
    Cinematic,
}

/// Why a cue matters when the active quality tier decides how to present it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectImportance {
    Essential,
    Expressive,
    Decorative,
}

/// A product-neutral statement of what changed visually.
///
/// These are meanings, not implementations. In particular `Reward` does not
/// promise particles and `Handoff` does not prescribe a colour or path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VisualCue {
    Arrival,
    Delegation,
    Handoff,
    Aggregation,
    Success,
    Reward,
    Attention,
    Refusal,
    Failure,
}

impl VisualCue {
    pub fn importance(self) -> EffectImportance {
        match self {
            Self::Attention | Self::Refusal | Self::Failure => EffectImportance::Essential,
            Self::Arrival
            | Self::Delegation
            | Self::Handoff
            | Self::Aggregation
            | Self::Success => EffectImportance::Expressive,
            Self::Reward => EffectImportance::Decorative,
        }
    }

    fn recipe(self) -> EffectRecipe {
        match self {
            Self::Arrival => EffectRecipe::ArrivalHalo,
            Self::Delegation => EffectRecipe::DelegationTrace,
            Self::Handoff => EffectRecipe::HandoffTrace,
            Self::Aggregation => EffectRecipe::AggregationPulse,
            Self::Success => EffectRecipe::SuccessBurst,
            Self::Reward => EffectRecipe::RewardCelebration,
            Self::Attention => EffectRecipe::AttentionPulse,
            Self::Refusal => EffectRecipe::RefusalMark,
            Self::Failure => EffectRecipe::FailurePulse,
        }
    }

    fn cost(self) -> EffectCost {
        match self {
            Self::Arrival => EffectCost::new(1, 1, 24, 48_000),
            Self::Delegation => EffectCost::new(1, 1, 18, 56_000),
            Self::Handoff => EffectCost::new(1, 1, 24, 72_000),
            Self::Aggregation => EffectCost::new(1, 1, 32, 64_000),
            Self::Success => EffectCost::new(1, 1, 36, 48_000),
            Self::Reward => EffectCost::new(1, 2, 96, 120_000),
            Self::Attention => EffectCost::new(1, 0, 0, 36_000),
            Self::Refusal => EffectCost::new(1, 0, 0, 36_000),
            Self::Failure => EffectCost::new(1, 1, 12, 48_000),
        }
    }
}

/// One normalized event submitted to the effects policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectEvent {
    pub id: SharedString,
    pub surface: SharedString,
    pub target: SharedString,
    pub origin: Option<SharedString>,
    pub cue: VisualCue,
}

impl EffectEvent {
    pub fn new(
        id: impl Into<SharedString>,
        surface: impl Into<SharedString>,
        target: impl Into<SharedString>,
        cue: VisualCue,
    ) -> Self {
        Self {
            id: id.into(),
            surface: surface.into(),
            target: target.into(),
            origin: None,
            cue,
        }
    }

    /// Names the related source for a transfer or arrival when one exists.
    pub fn origin(mut self, origin: impl Into<SharedString>) -> Self {
        self.origin = Some(origin.into());
        self
    }
}

/// Renderer-facing recipe selected from a semantic cue.
///
/// Recipes carry no arbitrary shader or caller-selected colour. A renderer
/// uses theme semantics, motion tokens, and the [`EffectPresentation`] chosen
/// by policy. Concrete particle and stroke primitives are implemented below
/// this boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectRecipe {
    ArrivalHalo,
    DelegationTrace,
    HandoffTrace,
    AggregationPulse,
    SuccessBurst,
    RewardCelebration,
    AttentionPulse,
    RefusalMark,
    FailurePulse,
}

/// How the chosen recipe is allowed to appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectPresentation {
    /// This identity was already consumed and should not appear again.
    Suppressed(EffectSuppression),
    /// The semantic fallback: visible feedback with no time-based movement.
    Static(EffectFallback),
    /// The complete token-driven animated recipe.
    Animated,
}

/// Why no visual presentation should be emitted at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectSuppression {
    Replay,
}

/// Why the full animated recipe resolved to its static counterpart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectFallback {
    Quality,
    ReducedMotion,
    Budget,
}

/// The complete deterministic answer for one semantic event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectPlan {
    pub id: SharedString,
    pub surface: SharedString,
    pub target: SharedString,
    pub origin: Option<SharedString>,
    pub cue: VisualCue,
    pub recipe: EffectRecipe,
    pub presentation: EffectPresentation,
    /// Stable seed derived from event identity, independent of process hash
    /// randomization and suitable for deterministic particle placement.
    pub seed: u64,
}

impl EffectPlan {
    pub fn animated(&self) -> bool {
        self.presentation == EffectPresentation::Animated
    }

    pub fn visible(&self) -> bool {
        !matches!(self.presentation, EffectPresentation::Suppressed(_))
    }
}

/// Estimated renderer work consumed by one animated recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EffectCost {
    pub events: u16,
    pub emitters: u16,
    pub particles: u32,
    pub animated_pixels: u32,
}

impl EffectCost {
    pub const fn new(events: u16, emitters: u16, particles: u32, animated_pixels: u32) -> Self {
        Self {
            events,
            emitters,
            particles,
            animated_pixels,
        }
    }

    fn fits(self, budget: Self) -> bool {
        self.events <= budget.events
            && self.emitters <= budget.emitters
            && self.particles <= budget.particles
            && self.animated_pixels <= budget.animated_pixels
    }

    fn saturating_add(self, other: Self) -> Self {
        Self {
            events: self.events.saturating_add(other.events),
            emitters: self.emitters.saturating_add(other.emitters),
            particles: self.particles.saturating_add(other.particles),
            animated_pixels: self.animated_pixels.saturating_add(other.animated_pixels),
        }
    }
}

/// Maximum animated work admitted during one frame.
pub type EffectBudget = EffectCost;

/// Quality, replay, and frame/surface budget policy installed by the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectPolicy {
    pub quality: EffectQuality,
    pub global_budget: EffectBudget,
    pub surface_budget: EffectBudget,
    pub replay_capacity: usize,
}

impl EffectPolicy {
    pub fn new(quality: EffectQuality) -> Self {
        let (global_budget, surface_budget) = default_budgets(quality);
        Self {
            quality,
            global_budget,
            surface_budget,
            replay_capacity: 2_048,
        }
    }

    /// Replaces both per-frame limits. Static fallbacks do not spend them.
    pub fn budgets(mut self, global: EffectBudget, per_surface: EffectBudget) -> Self {
        self.global_budget = global;
        self.surface_budget = per_surface;
        self
    }

    /// Bounds reconnect/rebuild suppression history. Zero remembers nothing.
    pub fn replay_capacity(mut self, capacity: usize) -> Self {
        self.replay_capacity = capacity;
        self
    }
}

impl Default for EffectPolicy {
    fn default() -> Self {
        Self::new(EffectQuality::Balanced)
    }
}

fn default_budgets(quality: EffectQuality) -> (EffectBudget, EffectBudget) {
    match quality {
        EffectQuality::Off => (EffectBudget::default(), EffectBudget::default()),
        EffectQuality::Essential => (
            EffectBudget::new(8, 4, 64, 320_000),
            EffectBudget::new(4, 2, 32, 160_000),
        ),
        EffectQuality::Balanced => (
            EffectBudget::new(24, 12, 384, 1_600_000),
            EffectBudget::new(8, 4, 128, 480_000),
        ),
        EffectQuality::Cinematic => (
            EffectBudget::new(64, 32, 2_048, 8_000_000),
            EffectBudget::new(20, 12, 768, 2_400_000),
        ),
    }
}

/// Stateful resolver used by the installed global and available for isolated
/// runtimes or deterministic tests.
#[derive(Debug)]
pub struct EffectPlanner {
    policy: EffectPolicy,
    frame: Option<u64>,
    global_usage: EffectCost,
    surface_usage: HashMap<SharedString, EffectCost>,
    replay_order: VecDeque<SharedString>,
    replay_seen: HashSet<SharedString>,
}

impl EffectPlanner {
    pub fn new(policy: EffectPolicy) -> Self {
        Self {
            policy,
            frame: None,
            global_usage: EffectCost::default(),
            surface_usage: HashMap::new(),
            replay_order: VecDeque::new(),
            replay_seen: HashSet::new(),
        }
    }

    pub fn policy(&self) -> &EffectPolicy {
        &self.policy
    }

    /// Changes future quality and budgets without replaying consumed events.
    pub fn set_policy(&mut self, policy: EffectPolicy) {
        self.policy = policy;
        self.trim_replay_history();
        self.global_usage = EffectCost::default();
        self.surface_usage.clear();
    }

    /// Resolves one event for a known frame and accessibility preference.
    pub fn plan(&mut self, event: EffectEvent, frame: u64, reduce_motion: bool) -> EffectPlan {
        let recipe = event.cue.recipe();
        let seed = stable_seed(event.id.as_ref());
        let replay_key = replay_key(&event.surface, &event.id);
        if self.replay_seen.contains(&replay_key) {
            return EffectPlan {
                id: event.id,
                surface: event.surface,
                target: event.target,
                origin: event.origin,
                cue: event.cue,
                recipe,
                presentation: EffectPresentation::Suppressed(EffectSuppression::Replay),
                seed,
            };
        }
        self.remember(replay_key);

        if self.frame != Some(frame) {
            self.frame = Some(frame);
            self.global_usage = EffectCost::default();
            self.surface_usage.clear();
        }

        let quality_allows = match self.policy.quality {
            EffectQuality::Off => false,
            EffectQuality::Essential => event.cue.importance() == EffectImportance::Essential,
            EffectQuality::Balanced => event.cue.importance() != EffectImportance::Decorative,
            EffectQuality::Cinematic => true,
        };
        let cost = event.cue.cost();
        let surface_usage = self
            .surface_usage
            .get(&event.surface)
            .copied()
            .unwrap_or_default();
        let within_budget = self
            .global_usage
            .saturating_add(cost)
            .fits(self.policy.global_budget)
            && surface_usage
                .saturating_add(cost)
                .fits(self.policy.surface_budget);
        let presentation = if reduce_motion {
            EffectPresentation::Static(EffectFallback::ReducedMotion)
        } else if !quality_allows {
            EffectPresentation::Static(EffectFallback::Quality)
        } else if !within_budget {
            EffectPresentation::Static(EffectFallback::Budget)
        } else {
            self.global_usage = self.global_usage.saturating_add(cost);
            self.surface_usage
                .insert(event.surface.clone(), surface_usage.saturating_add(cost));
            EffectPresentation::Animated
        };

        EffectPlan {
            id: event.id,
            surface: event.surface,
            target: event.target,
            origin: event.origin,
            cue: event.cue,
            recipe,
            presentation,
            seed,
        }
    }

    fn remember(&mut self, id: SharedString) {
        if self.policy.replay_capacity == 0 {
            return;
        }
        while self.replay_order.len() >= self.policy.replay_capacity {
            if let Some(expired) = self.replay_order.pop_front() {
                self.replay_seen.remove(&expired);
            }
        }
        self.replay_seen.insert(id.clone());
        self.replay_order.push_back(id);
    }

    fn trim_replay_history(&mut self) {
        while self.replay_order.len() > self.policy.replay_capacity {
            if let Some(expired) = self.replay_order.pop_front() {
                self.replay_seen.remove(&expired);
            }
        }
        if self.policy.replay_capacity == 0 {
            self.replay_seen.clear();
        }
    }
}

impl Default for EffectPlanner {
    fn default() -> Self {
        Self::new(EffectPolicy::default())
    }
}

#[derive(Clone)]
struct InstalledEffects(Rc<RefCell<EffectPlanner>>);

impl Default for InstalledEffects {
    fn default() -> Self {
        Self(Rc::new(RefCell::new(EffectPlanner::default())))
    }
}

impl Global for InstalledEffects {}

pub(crate) fn install(cx: &mut App) {
    if !cx.has_global::<InstalledEffects>() {
        cx.set_global(InstalledEffects::default());
    }
}

/// Installs a new policy while preserving replay history.
pub fn set_effect_policy(policy: EffectPolicy, cx: &mut App) {
    install(cx);
    cx.global::<InstalledEffects>()
        .0
        .borrow_mut()
        .set_policy(policy);
    cx.refresh_windows();
}

/// Returns the currently installed policy, defaulting to Balanced.
pub fn effect_policy(cx: &App) -> EffectPolicy {
    cx.try_global::<InstalledEffects>()
        .map(|effects| effects.0.borrow().policy().clone())
        .unwrap_or_default()
}

/// Resolves and consumes one semantic event against the active policy.
pub fn plan_effect(event: EffectEvent, cx: &mut App) -> EffectPlan {
    install(cx);
    let frame = keyed::frame_counter(cx).unwrap_or_default();
    let reduce_motion = cx.reduce_motion();
    cx.global::<InstalledEffects>()
        .0
        .borrow_mut()
        .plan(event, frame, reduce_motion)
}

fn stable_seed(id: &str) -> u64 {
    // FNV-1a is deliberately fixed rather than statistically special: this
    // seed needs reproducibility across runs and platforms, not cryptography.
    let mut hash = 0xcbf29ce484222325u64;
    for byte in id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn replay_key(surface: &str, id: &str) -> SharedString {
    format!("{}:{}{}:{}", surface.len(), surface, id.len(), id).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: &'static str, surface: &'static str, cue: VisualCue) -> EffectEvent {
        EffectEvent::new(id, surface, "target", cue)
    }

    #[test]
    fn quality_tiers_degrade_without_losing_static_feedback() {
        let cases = [
            (EffectQuality::Off, false, false, false),
            (EffectQuality::Essential, true, false, false),
            (EffectQuality::Balanced, true, true, false),
            (EffectQuality::Cinematic, true, true, true),
        ];
        for (quality, essential, expressive, decorative) in cases {
            let mut planner = EffectPlanner::new(EffectPolicy::new(quality));
            assert_eq!(
                planner
                    .plan(event("attention", "one", VisualCue::Attention), 1, false)
                    .animated(),
                essential
            );
            assert_eq!(
                planner
                    .plan(event("arrival", "one", VisualCue::Arrival), 1, false)
                    .animated(),
                expressive
            );
            let reward = planner.plan(event("reward", "one", VisualCue::Reward), 1, false);
            assert_eq!(reward.animated(), decorative);
            assert!(reward.visible());
        }
    }

    #[test]
    fn reduced_motion_always_selects_the_static_recipe() {
        let mut planner = EffectPlanner::new(EffectPolicy::new(EffectQuality::Cinematic));
        let plan = planner.plan(event("reward", "one", VisualCue::Reward), 1, true);
        assert_eq!(
            plan.presentation,
            EffectPresentation::Static(EffectFallback::ReducedMotion)
        );
        assert_eq!(plan.recipe, EffectRecipe::RewardCelebration);
    }

    #[test]
    fn replay_is_suppressed_and_seed_is_stable() {
        let mut planner = EffectPlanner::default();
        let first = planner.plan(event("same", "one", VisualCue::Success), 1, false);
        let replay = planner.plan(event("same", "one", VisualCue::Failure), 2, false);
        assert!(first.visible());
        assert_eq!(first.seed, stable_seed("same"));
        assert_eq!(replay.seed, first.seed);
        assert_eq!(
            replay.presentation,
            EffectPresentation::Suppressed(EffectSuppression::Replay)
        );
        assert!(
            planner
                .plan(event("same", "another", VisualCue::Success), 2, false)
                .visible(),
            "event identity is replay-scoped to its surface"
        );
    }

    #[test]
    fn global_and_surface_budgets_degrade_only_excess_work() {
        let one = EffectBudget::new(1, 1, 128, 200_000);
        let mut planner =
            EffectPlanner::new(EffectPolicy::new(EffectQuality::Cinematic).budgets(one, one));
        assert!(
            planner
                .plan(event("a", "left", VisualCue::Success), 7, false)
                .animated()
        );
        assert_eq!(
            planner
                .plan(event("b", "right", VisualCue::Success), 7, false)
                .presentation,
            EffectPresentation::Static(EffectFallback::Budget),
            "the global frame budget is already spent"
        );
        assert!(
            planner
                .plan(event("c", "left", VisualCue::Success), 8, false)
                .animated(),
            "a new frame resets both ledgers"
        );

        let global = EffectBudget::new(4, 4, 512, 800_000);
        let mut planner =
            EffectPlanner::new(EffectPolicy::new(EffectQuality::Cinematic).budgets(global, one));
        assert!(
            planner
                .plan(event("d", "left", VisualCue::Success), 9, false)
                .animated()
        );
        assert_eq!(
            planner
                .plan(event("e", "left", VisualCue::Success), 9, false)
                .presentation,
            EffectPresentation::Static(EffectFallback::Budget)
        );
        assert!(
            planner
                .plan(event("f", "right", VisualCue::Success), 9, false)
                .animated(),
            "another surface has its own allowance"
        );
    }

    #[test]
    fn replay_history_is_bounded_and_can_be_disabled() {
        let mut planner = EffectPlanner::new(EffectPolicy::default().replay_capacity(2));
        planner.plan(event("a", "one", VisualCue::Success), 1, false);
        planner.plan(event("b", "one", VisualCue::Success), 1, false);
        planner.plan(event("c", "one", VisualCue::Success), 1, false);
        assert!(
            planner
                .plan(event("a", "one", VisualCue::Success), 2, false)
                .visible(),
            "the oldest bounded identity can be consumed again"
        );

        let mut planner = EffectPlanner::new(EffectPolicy::default().replay_capacity(0));
        planner.plan(event("same", "one", VisualCue::Success), 1, false);
        assert!(
            planner
                .plan(event("same", "one", VisualCue::Success), 1, false)
                .visible()
        );
    }

    #[test]
    fn event_identity_produces_platform_independent_seeds() {
        assert_eq!(stable_seed("event-7"), 16_957_040_352_285_269_107);
        assert_ne!(stable_seed("event-7"), stable_seed("event-8"));
    }
}
