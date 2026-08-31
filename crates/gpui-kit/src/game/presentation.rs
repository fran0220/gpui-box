//! Complete presentation of caller-owned party, objective, ability, and reward facts.

use std::fmt;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_kit_assets::{Icon, icon as glyph};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, TextTone, TypeScale,
};

use crate::agent::{AgentActivityLine, AgentId, PersonaPortrait};
use crate::controls::button::Button;
use crate::display::badge::{Badge, Tone};
use crate::display::progress::ProgressBar;
use crate::display::status::Callout;
use crate::effects::{EffectParticles, EffectPlan};
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{CardVariant, FocusRing, Ident, Pressable, Selectable, Sizable, StyledExt};
use crate::motion::{Animated, Entrance};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey, Strings};

use super::model::{
    Ability, AbilityId, AbilitySet, AbilitySetIssue, AbilityState, Objective, ObjectiveId,
    ObjectiveIssue, ObjectiveSnapshot, ObjectiveState, PartyGauge, PartyGaugeState, PartyIssue,
    PartyMember, PartySnapshot, RewardId, RewardItem, RewardSnapshot, RewardState,
};

/// What a party roster reports without changing its caller-owned selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyRosterEvent {
    SelectMember(AgentId),
}

type PartyHandler = Rc<dyn Fn(&PartyRosterEvent, &mut Window, &mut App)>;

/// A responsive party roster over standard agent and persona presentation.
#[derive(IntoElement)]
pub struct PartyRoster {
    ident: Ident,
    party: PartySnapshot,
    selected: Option<AgentId>,
    on_event: Option<PartyHandler>,
}

impl fmt::Debug for PartyRoster {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PartyRoster")
            .field("ident", &self.ident)
            .field("members", &self.party.members.len())
            .field("selected", &self.selected)
            .field("has_handler", &self.on_event.is_some())
            .finish()
    }
}

impl PartyRoster {
    pub fn new(ident: impl Into<Ident>, party: PartySnapshot) -> Self {
        Self {
            ident: ident.into(),
            party,
            selected: None,
            on_event: None,
        }
    }

    pub fn selected(mut self, selected: impl Into<AgentId>) -> Self {
        self.selected = Some(selected.into());
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(&PartyRosterEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PartyRoster {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let issues = self.party.issues();
        if !issues.is_empty() {
            return div()
                .child(
                    Callout::new(party_issue_text(&issues, cx.strings()), Tone::Danger)
                        .id(self.ident.child("issues")),
                )
                .into_any_element();
        }

        let count = self.party.members.len();
        let cards = self.party.members.into_iter().map(|member| {
            party_member(
                &self.ident,
                member,
                self.selected.as_ref(),
                self.on_event.as_ref(),
                direction,
                &theme,
                cx,
            )
        });
        div()
            .row_reading(direction)
            .items_stretch()
            .flex_wrap()
            .gap_token(&theme, Space::Md)
            .children(cards)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::List)
                    .value(cx.numbers().count(count)),
            )
            .into_any_element()
    }
}

fn party_member(
    owner: &Ident,
    member: PartyMember,
    selected: Option<&AgentId>,
    handler: Option<&PartyHandler>,
    direction: crate::foundation::LayoutDirection,
    theme: &gpui_kit_theme::Theme,
    cx: &mut App,
) -> AnyElement {
    let id = member.agent.descriptor.id.clone();
    let ident = owner.child(id.as_str());
    let chosen = selected == Some(&id);
    let interactive = handler.is_some();
    let mut portrait = PersonaPortrait::new(ident.child("portrait"), member.agent.clone())
        .expression(member.expression)
        .size(54.0);
    if let Some(image) = member.image {
        portrait = portrait.image(image);
    }
    if let Some(tint) = member.tint {
        portrait = portrait.tint(tint);
    }

    let gauges = member
        .gauges
        .into_iter()
        .map(|gauge| party_gauge(&ident, gauge, theme, cx));
    let body = div()
        .column()
        .min_w_0()
        .flex_1()
        .gap_token(theme, Space::Xs)
        .child(
            div()
                .type_scale(theme, TypeScale::Label)
                .text_tone(theme, TextTone::Primary)
                .child(member.agent.descriptor.name.clone()),
        )
        .children(member.agent.descriptor.role.clone().map(|role| {
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_tone(theme, TextTone::Muted)
                .child(role)
        }))
        .child(AgentActivityLine::new(
            ident.child("activity"),
            member.agent.execution.clone(),
        ))
        .children(gauges);

    let mut card = div()
        .id(ident.element_id())
        .row_reading(direction)
        .items_start()
        .gap_token(theme, Space::Sm)
        .w(px(286.0))
        .p_token(theme, Space::Sm)
        .card_surface(theme, CardVariant::Filled)
        .bg(if chosen {
            theme.colors.selected
        } else {
            theme.colors.panel
        })
        .child(portrait)
        .child(body);
    if let Some(handler) = handler.cloned() {
        let click_id = id.clone();
        let key_id = id.clone();
        let key_handler = handler.clone();
        card = card
            .cursor_pointer()
            .tab_index(0)
            .pressable(cx)
            .focus_ring(theme)
            .on_click(move |_, window, cx| {
                handler(
                    &PartyRosterEvent::SelectMember(click_id.clone()),
                    window,
                    cx,
                )
            })
            .on_key_down(move |event, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    key_handler(&PartyRosterEvent::SelectMember(key_id.clone()), window, cx);
                    cx.stop_propagation();
                }
            });
    }

    card.semantic_in(
        cx,
        NodeSpec::new(
            ident.semantic_id(),
            if interactive { Role::Button } else { Role::Row },
        )
        .text(member.agent.descriptor.name)
        .value(member.agent.execution.as_str())
        .selected(chosen)
        .busy(member.agent.execution.busy())
        .parent(owner.semantic_id()),
    )
    .into_any_element()
}

fn party_gauge(
    owner: &Ident,
    gauge: PartyGauge,
    theme: &gpui_kit_theme::Theme,
    cx: &mut App,
) -> AnyElement {
    let ident = owner.child("gauge").child(gauge.id.as_ref());
    match gauge.state {
        PartyGaugeState::Known { fraction, display } => ProgressBar::new(ident)
            .label(gauge.label)
            .fraction(fraction.get())
            .display(display)
            .into_any_element(),
        PartyGaugeState::Unknown => {
            let unknown = cx.strings().text(StringKey::GameGaugeUnknown);
            div()
                .row()
                .justify_between()
                .gap_token(theme, Space::Sm)
                .type_scale(theme, TypeScale::Caption)
                .text_tone(theme, TextTone::Muted)
                .child(gauge.label.clone())
                .child(unknown.clone())
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Status)
                        .text(gauge.label)
                        .value(unknown),
                )
                .into_any_element()
        }
        PartyGaugeState::Unavailable(reason) => {
            let unavailable = cx.strings().text(StringKey::GameGaugeUnavailable);
            div()
                .column()
                .gap_token(theme, Space::Xs)
                .type_scale(theme, TypeScale::Caption)
                .child(
                    div()
                        .row()
                        .justify_between()
                        .gap_token(theme, Space::Sm)
                        .child(
                            div()
                                .text_tone(theme, TextTone::Muted)
                                .child(gauge.label.clone()),
                        )
                        .child(
                            div()
                                .text_color(theme.colors.warning)
                                .child(unavailable.clone()),
                        ),
                )
                .child(
                    div()
                        .text_tone(theme, TextTone::Muted)
                        .child(reason.clone()),
                )
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Status)
                        .text(gauge.label)
                        .value("unavailable")
                        .description(reason)
                        .invalid(true),
                )
                .into_any_element()
        }
    }
}

fn party_issue_text(issues: &[PartyIssue], strings: &Strings) -> SharedString {
    issues
        .iter()
        .map(|issue| match issue {
            PartyIssue::DuplicateMember(id) => {
                strings.format(StringKey::GamePartyDuplicateMember, &[id.as_str()])
            }
            PartyIssue::DuplicateGauge { member, gauge } => strings.format(
                StringKey::GamePartyDuplicateGauge,
                &[member.as_str(), gauge.as_ref()],
            ),
        })
        .collect::<Vec<_>>()
        .join("; ")
        .into()
}

/// What an objective tracker reports without selecting or completing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectiveTrackerEvent {
    Select(ObjectiveId),
}

type ObjectiveHandler = Rc<dyn Fn(&ObjectiveTrackerEvent, &mut Window, &mut App)>;

/// A hierarchical objective tracker with strict identity validation.
#[derive(IntoElement)]
pub struct ObjectiveTracker {
    ident: Ident,
    snapshot: ObjectiveSnapshot,
    selected: Option<ObjectiveId>,
    on_event: Option<ObjectiveHandler>,
}

impl fmt::Debug for ObjectiveTracker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObjectiveTracker")
            .field("ident", &self.ident)
            .field("objectives", &self.snapshot.objectives.len())
            .field("selected", &self.selected)
            .finish()
    }
}

impl ObjectiveTracker {
    pub fn new(ident: impl Into<Ident>, snapshot: ObjectiveSnapshot) -> Self {
        Self {
            ident: ident.into(),
            snapshot,
            selected: None,
            on_event: None,
        }
    }

    pub fn selected(mut self, selected: impl Into<ObjectiveId>) -> Self {
        self.selected = Some(selected.into());
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(&ObjectiveTrackerEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ObjectiveTracker {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let issues = self.snapshot.issues();
        if !issues.is_empty() {
            return div()
                .child(
                    Callout::new(objective_issue_text(&issues, cx.strings()), Tone::Danger)
                        .id(self.ident.child("issues")),
                )
                .into_any_element();
        }

        let rows = self.snapshot.objectives.iter().map(|objective| {
            let depth = self.snapshot.depth(objective);
            objective_row(
                &self.ident,
                objective,
                depth,
                self.selected.as_ref(),
                self.on_event.as_ref(),
                direction,
                &theme,
                cx,
            )
        });
        div()
            .column()
            .gap_token(&theme, Space::Sm)
            .children(rows)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::List)
                    .value(cx.numbers().count(self.snapshot.objectives.len())),
            )
            .into_any_element()
    }
}

#[allow(clippy::too_many_arguments)]
fn objective_row(
    owner: &Ident,
    objective: &Objective,
    depth: usize,
    selected: Option<&ObjectiveId>,
    handler: Option<&ObjectiveHandler>,
    direction: crate::foundation::LayoutDirection,
    theme: &gpui_kit_theme::Theme,
    cx: &mut App,
) -> AnyElement {
    let ident = owner.child(objective.id.as_str());
    let chosen = selected == Some(&objective.id);
    let (state_label, tone, reason) = objective_state(&objective.state, cx.strings());
    let interactive = handler.is_some();
    let mut row = div()
        .id(ident.element_id())
        .column()
        .gap_token(theme, Space::Xs)
        // A child objective is inset on both edges rather than only the one
        // reading starts at. Indented on one side alone it keeps the column's
        // far edge, so the card reads as a card of the wrong width sitting
        // under the one above it instead of as a card nested inside it.
        .ms(direction, px(depth as f32 * theme.space(Space::Lg)))
        .me(direction, px(depth as f32 * theme.space(Space::Lg)))
        .p_token(theme, Space::Sm)
        .card_surface(theme, CardVariant::Filled)
        .bg(if chosen {
            theme.colors.selected
        } else {
            theme.colors.panel
        })
        .child(
            div()
                .row_reading(direction)
                .justify_between()
                .gap_token(theme, Space::Md)
                .child(
                    div()
                        .min_w_0()
                        .type_scale(theme, TypeScale::Label)
                        .text_tone(theme, TextTone::Primary)
                        .child(objective.title.clone()),
                )
                .child(Badge::new(state_label).id(ident.child("state")).tone(tone)),
        )
        .children(objective.detail.clone().map(|detail| {
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_tone(theme, TextTone::Muted)
                .child(detail)
        }))
        .children(reason.map(|reason| {
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(tone_color(tone, theme))
                .child(reason)
        }))
        .children(objective.progress.map(|progress| {
            ProgressBar::new(ident.child("progress"))
                .fraction(progress.get())
                .display(cx.numbers().percent(progress.get()))
        }));
    if let Some(handler) = handler.cloned() {
        let click_id = objective.id.clone();
        let key_id = objective.id.clone();
        let key_handler = handler.clone();
        row = row
            .cursor_pointer()
            .tab_index(0)
            .pressable(cx)
            .focus_ring(theme)
            .on_click(move |_, window, cx| {
                handler(&ObjectiveTrackerEvent::Select(click_id.clone()), window, cx)
            })
            .on_key_down(move |event, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    key_handler(&ObjectiveTrackerEvent::Select(key_id.clone()), window, cx);
                    cx.stop_propagation();
                }
            });
    }
    row.semantic_in(
        cx,
        NodeSpec::new(
            ident.semantic_id(),
            if interactive { Role::Button } else { Role::Row },
        )
        .text(objective.title.clone())
        .value(objective.state.name())
        .selected(chosen)
        .invalid(matches!(
            objective.state,
            ObjectiveState::Failed(_) | ObjectiveState::Unavailable(_)
        ))
        .parent(owner.semantic_id()),
    )
    .into_any_element()
}

fn objective_state(
    state: &ObjectiveState,
    strings: &Strings,
) -> (SharedString, Tone, Option<SharedString>) {
    match state {
        ObjectiveState::Locked => (
            strings.text(StringKey::GameObjectiveLocked),
            Tone::Neutral,
            None,
        ),
        ObjectiveState::Active => (
            strings.text(StringKey::GameObjectiveActive),
            Tone::Info,
            None,
        ),
        ObjectiveState::Completed => (
            strings.text(StringKey::GameObjectiveCompleted),
            Tone::Success,
            None,
        ),
        ObjectiveState::Failed(reason) => (
            strings.text(StringKey::GameObjectiveFailed),
            Tone::Danger,
            Some(reason.clone()),
        ),
        ObjectiveState::Unavailable(reason) => (
            strings.text(StringKey::GameObjectiveUnavailable),
            Tone::Warning,
            Some(reason.clone()),
        ),
    }
}

fn objective_issue_text(issues: &[ObjectiveIssue], strings: &Strings) -> SharedString {
    issues
        .iter()
        .map(|issue| match issue {
            ObjectiveIssue::DuplicateId(id) => {
                strings.format(StringKey::GameObjectiveDuplicate, &[id.as_str()])
            }
            ObjectiveIssue::DanglingParent { objective, parent } => strings.format(
                StringKey::GameObjectiveDanglingParent,
                &[objective.as_str(), parent.as_str()],
            ),
            ObjectiveIssue::ParentCycle(id) => {
                strings.format(StringKey::GameObjectiveCycle, &[id.as_str()])
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
        .into()
}

/// What an ability bar reports without applying cooldown, charge, or cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbilityBarEvent {
    Activate(AbilityId),
}

type AbilityHandler = Rc<dyn Fn(&AbilityBarEvent, &mut Window, &mut App)>;

/// Standard ability controls with cooldown, charge, cost, shortcut, and
/// refusal presentation.
#[derive(IntoElement)]
pub struct AbilityBar {
    ident: Ident,
    abilities: AbilitySet,
    selected: Option<AbilityId>,
    on_event: Option<AbilityHandler>,
}

impl fmt::Debug for AbilityBar {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AbilityBar")
            .field("ident", &self.ident)
            .field("abilities", &self.abilities.abilities.len())
            .field("selected", &self.selected)
            .finish()
    }
}

impl AbilityBar {
    pub fn new(ident: impl Into<Ident>, abilities: AbilitySet) -> Self {
        Self {
            ident: ident.into(),
            abilities,
            selected: None,
            on_event: None,
        }
    }

    pub fn selected(mut self, selected: impl Into<AbilityId>) -> Self {
        self.selected = Some(selected.into());
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(&AbilityBarEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for AbilityBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let issues = self.abilities.issues();
        if !issues.is_empty() {
            return div()
                .child(
                    Callout::new(ability_issue_text(&issues, cx.strings()), Tone::Danger)
                        .id(self.ident.child("issues")),
                )
                .into_any_element();
        }

        let count = self.abilities.abilities.len();
        let abilities = self.abilities.abilities.into_iter().map(|ability| {
            ability_control(
                &self.ident,
                ability,
                self.selected.as_ref(),
                self.on_event.as_ref(),
                &theme,
                cx,
            )
        });
        div()
            .row_reading(direction)
            .items_start()
            .flex_wrap()
            .gap_token(&theme, Space::Sm)
            .children(abilities)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Toolbar)
                    .value(cx.numbers().count(count)),
            )
            .into_any_element()
    }
}

fn ability_control(
    owner: &Ident,
    ability: Ability,
    selected: Option<&AbilityId>,
    handler: Option<&AbilityHandler>,
    theme: &gpui_kit_theme::Theme,
    cx: &mut App,
) -> AnyElement {
    let ident = owner.child(ability.id.as_str());
    let ready = matches!(ability.state, AbilityState::Ready);
    let chosen = selected == Some(&ability.id);
    let (status, tone, reason) = ability_state(&ability.state, cx.strings());
    let action = if ready {
        let mut button = Button::new(ident.clone())
            .label(ability.label.clone())
            .secondary()
            .control_size(ControlSize::Sm)
            .full_width(true)
            .selected(chosen);
        if let Some(icon) = ability.icon {
            button = button.icon(icon);
        }
        if let Some(description) = reason.clone().or(ability.detail.clone()) {
            button = button.accessible_description(description);
        }
        if let Some(handler) = handler.cloned() {
            let id = ability.id.clone();
            button = button.on_click(move |window, cx| {
                handler(&AbilityBarEvent::Activate(id.clone()), window, cx)
            });
        }
        button.into_any_element()
    } else {
        // Cooldown, disabled, and unavailable are still identities the player
        // must scan. A generic disabled button mutes its whole label by design;
        // this static header keeps the ability readable while publishing a
        // disabled button and installing no action handler.
        let mut header = div()
            .id(ident.element_id())
            .row()
            .items_center()
            .gap_token(theme, Space::Xs)
            .w_full()
            .h(px(theme.control.sm.height))
            .px_token(theme, Space::Sm)
            .radius(theme, Radius::Control)
            .bg(if chosen {
                theme.colors.selected
            } else {
                theme.colors.raised
            })
            .type_scale(theme, TypeScale::Label)
            .text_tone(theme, TextTone::Muted);
        if let Some(icon) = ability.icon {
            header = header.child(
                glyph(icon)
                    .size(px(theme.control.sm.icon_size))
                    .text_color(theme.colors.text_muted),
            );
        }
        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Button)
            .parent(owner.semantic_id())
            .text(ability.label.clone())
            .disabled(true)
            .selected(chosen);
        if let Some(description) = reason.clone().or(ability.detail.clone()) {
            spec = spec.description(description);
        }
        header
            .child(ability.label.clone())
            .semantic_in(cx, spec)
            .into_any_element()
    };

    // Every card lays out the same slots in the same order — status and
    // shortcut on one line, then the numbers, then the reason, then the
    // meter — so four abilities in four states still read as one control.
    let status_row = div()
        .row()
        .items_center()
        .justify_between()
        .gap_token(theme, Space::Xs)
        .child(Badge::new(status).id(ident.child("state")).tone(tone))
        .children(ability.shortcut.map(|shortcut| {
            Badge::new(shortcut)
                .id(ident.child("shortcut"))
                .tone(Tone::Neutral)
        }));

    let has_charges = ability.charges.is_some();
    let has_cost = ability.cost.is_some();
    let numbers = div()
        .row()
        .flex_wrap()
        .gap_token(theme, Space::Xs)
        .children(ability.charges.map(|charges| {
            let current = cx
                .numbers()
                .count_of_total(charges.current(), charges.maximum());
            Badge::new(
                cx.strings()
                    .format(StringKey::GameAbilityCharges, &[current.as_ref()]),
            )
            .id(ident.child("charges"))
            .tone(if charges.current() == 0 {
                Tone::Warning
            } else {
                Tone::Neutral
            })
        }))
        .children(ability.cost.map(|cost| {
            Badge::new(
                cx.strings()
                    .format(StringKey::GameAbilityCost, &[cost.as_ref()]),
            )
            .id(ident.child("cost"))
            .tone(Tone::Neutral)
        }));
    let has_numbers = has_charges || has_cost;

    let meter = match ability.state {
        AbilityState::CoolingDown {
            remaining_fraction, ..
        } => {
            Some(ProgressBar::new(ident.child("cooldown")).fraction(1.0 - remaining_fraction.get()))
        }
        _ => None,
    };

    div()
        .column()
        .gap_token(theme, Space::Xs)
        .w(px(174.0))
        .p_token(theme, Space::Sm)
        .card_surface(theme, CardVariant::Filled)
        .child(action)
        .child(status_row)
        .children(has_numbers.then_some(numbers))
        .children(reason.map(|reason| {
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(tone_color(tone, theme))
                .child(reason)
        }))
        .children(meter)
        .into_any_element()
}

fn ability_state(
    state: &AbilityState,
    strings: &Strings,
) -> (SharedString, Tone, Option<SharedString>) {
    match state {
        AbilityState::Ready => (
            strings.text(StringKey::GameAbilityReady),
            Tone::Success,
            None,
        ),
        AbilityState::CoolingDown { remaining, .. } => (
            strings.format(StringKey::GameAbilityCooldown, &[remaining.as_ref()]),
            Tone::Info,
            None,
        ),
        AbilityState::Disabled(reason) => (
            strings.text(StringKey::GameAbilityDisabled),
            Tone::Warning,
            Some(reason.clone()),
        ),
        AbilityState::Unavailable(reason) => (
            strings.text(StringKey::GameAbilityUnavailable),
            Tone::Danger,
            Some(reason.clone()),
        ),
    }
}

fn ability_issue_text(issues: &[AbilitySetIssue], strings: &Strings) -> SharedString {
    issues
        .iter()
        .map(|issue| match issue {
            AbilitySetIssue::DuplicateId(id) => {
                strings.format(StringKey::GameAbilityDuplicate, &[id.as_str()])
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
        .into()
}

/// What a reward reveal reports without revealing or claiming anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewardRevealEvent {
    RevealRequested(RewardId),
    ClaimRequested(RewardId),
}

type RewardHandler = Rc<dyn Fn(&RewardRevealEvent, &mut Window, &mut App)>;

/// A complete reward reveal composition with optional policy-resolved effects.
#[derive(IntoElement)]
pub struct RewardReveal {
    ident: Ident,
    reward: RewardSnapshot,
    effect: Option<EffectPlan>,
    sample_at: Option<Duration>,
    on_event: Option<RewardHandler>,
}

impl fmt::Debug for RewardReveal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RewardReveal")
            .field("ident", &self.ident)
            .field("reward", &self.reward.id)
            .field("state", &self.reward.state)
            .field("has_effect", &self.effect.is_some())
            .finish()
    }
}

impl RewardReveal {
    pub fn new(ident: impl Into<Ident>, reward: RewardSnapshot) -> Self {
        Self {
            ident: ident.into(),
            reward,
            effect: None,
            sample_at: None,
            on_event: None,
        }
    }

    pub fn effect(mut self, effect: EffectPlan) -> Self {
        self.effect = Some(effect);
        self
    }

    pub fn sample_at(mut self, elapsed: Duration) -> Self {
        self.sample_at = Some(elapsed);
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(&RewardRevealEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for RewardReveal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let issues = self.reward.issues();
        if !issues.is_empty() {
            let message = issues
                .iter()
                .map(|id| {
                    cx.strings()
                        .format(StringKey::GameRewardDuplicateItem, &[id.as_str()])
                })
                .collect::<Vec<_>>()
                .join("; ");
            return div()
                .child(Callout::new(message, Tone::Danger).id(self.ident.child("issues")))
                .into_any_element();
        }

        let state = self.reward.state.clone();
        let reward_id = self.reward.id.clone();
        let visible_items = matches!(state, RewardState::Revealed | RewardState::Claimed);
        let state_label = match &state {
            RewardState::Hidden => cx.strings().text(StringKey::GameRewardHidden),
            RewardState::Revealed => cx.strings().text(StringKey::GameRewardRevealed),
            RewardState::Claimed => cx.strings().text(StringKey::GameRewardClaimed),
            RewardState::Unavailable(_) => cx.strings().text(StringKey::GameRewardUnavailable),
        };
        let state_reason = match &state {
            RewardState::Unavailable(reason) => Some(reason.clone()),
            _ => None,
        };
        let state_tone = match state {
            RewardState::Hidden => Tone::Neutral,
            RewardState::Revealed => Tone::Info,
            RewardState::Claimed => Tone::Success,
            RewardState::Unavailable(_) => Tone::Warning,
        };

        let action = match (&state, self.on_event.clone()) {
            (RewardState::Hidden, Some(handler)) => {
                let id = reward_id.clone();
                Some(
                    Button::new(self.ident.child("reveal"))
                        .label(cx.strings().text(StringKey::GameRewardReveal))
                        .primary()
                        .on_click(move |window, cx| {
                            handler(&RewardRevealEvent::RevealRequested(id.clone()), window, cx)
                        }),
                )
            }
            (RewardState::Revealed, Some(handler)) => {
                let id = reward_id.clone();
                Some(
                    Button::new(self.ident.child("claim"))
                        .label(cx.strings().text(StringKey::GameRewardClaim))
                        .primary()
                        .on_click(move |window, cx| {
                            handler(&RewardRevealEvent::ClaimRequested(id.clone()), window, cx)
                        }),
                )
            }
            _ => None,
        };

        let items = visible_items.then(|| {
            div()
                .row_reading(direction)
                .items_stretch()
                .flex_wrap()
                .gap_token(&theme, Space::Sm)
                .children(
                    self.reward
                        .items
                        .into_iter()
                        .enumerate()
                        .map(|(index, item)| reward_item(&self.ident, item, index, &theme, cx)),
                )
        });
        let effect = (visible_items && matches!(state, RewardState::Revealed))
            .then_some(self.effect)
            .flatten()
            .map(|plan| {
                let particles = EffectParticles::new(plan);
                let particles = match self.sample_at {
                    Some(elapsed) => particles.sample_at(elapsed),
                    None => particles,
                };
                // Celebration has a visual address of its own. Letting it
                // cover the entire title row put glow over the reward's
                // explanation, so the effect read as damaged text instead of
                // as a reveal.
                div()
                    .relative()
                    .size(px(52.0))
                    .flex_none()
                    .rounded_full()
                    .overflow_hidden()
                    .bg(theme.colors.sunken)
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .opacity(theme.opacity.muted)
                            .child(particles),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                glyph(Icon::Archive)
                                    .size(px(theme.control.md.icon_size))
                                    .text_color(theme.colors.text_muted),
                            ),
                    )
            });

        div()
            .relative()
            .column()
            .gap_token(&theme, Space::Md)
            .w_full()
            .p_token(&theme, Space::Lg)
            .card_surface(&theme, CardVariant::Filled)
            .overflow_hidden()
            .child(
                div()
                    .relative()
                    .row_reading(direction)
                    .items_center()
                    .justify_between()
                    .gap_token(&theme, Space::Md)
                    .children(effect)
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .column()
                            .gap_token(&theme, Space::Xs)
                            .child(
                                div()
                                    .type_scale(&theme, TypeScale::Subtitle)
                                    .text_tone(&theme, TextTone::Primary)
                                    .child(self.reward.title.clone()),
                            )
                            .children(self.reward.detail.map(|detail| {
                                div()
                                    .type_scale(&theme, TypeScale::Caption)
                                    .text_tone(&theme, TextTone::Muted)
                                    .child(detail)
                            })),
                    )
                    .child(
                        div().flex_none().child(
                            Badge::new(state_label)
                                .id(self.ident.child("state"))
                                .tone(state_tone),
                        ),
                    ),
            )
            .children(items.map(|items| div().relative().child(items)))
            .children(state_reason.map(|reason| {
                div()
                    .relative()
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.warning)
                    .child(reason)
            }))
            .children(action.map(|action| {
                div()
                    .relative()
                    .row_reading(direction)
                    .justify_end()
                    .child(div().flex_none().child(action))
            }))
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .text(self.reward.title)
                    .value(state.name())
                    .invalid(matches!(state, RewardState::Unavailable(_))),
            )
            .into_any_element()
    }
}

fn reward_item(
    owner: &Ident,
    item: RewardItem,
    index: usize,
    theme: &gpui_kit_theme::Theme,
    cx: &mut App,
) -> AnyElement {
    let ident = owner.child("item").child(item.id.as_str());
    let quantity = cx.numbers().count(item.quantity);
    let fallback_icon = (item.image.is_none() && item.icon.is_none()).then_some(Icon::Widget);
    let art = div()
        .size(px(42.0))
        .flex_none()
        .items_center()
        .justify_center()
        .rounded_full()
        .overflow_hidden()
        .bg(theme.colors.sunken)
        .children(item.image.map(|image| gpui::img(image).size_full()))
        .children(item.icon.or(fallback_icon).map(|icon| {
            glyph(icon)
                .size(px(20.0))
                .text_color(theme.colors.text_muted)
        }));
    div()
        .row()
        .items_center()
        .gap_token(theme, Space::Sm)
        .w(px(174.0))
        .p_token(theme, Space::Sm)
        .radius(theme, Radius::Card)
        .frame(theme, Surface::Raised, Elevation::Raised)
        .child(art)
        .child(
            div()
                .min_w_0()
                .flex_1()
                .column()
                .gap_token(theme, Space::Xs)
                .child(
                    div()
                        .type_scale(theme, TypeScale::Label)
                        .text_tone(theme, TextTone::Primary)
                        .child(item.label.clone()),
                )
                .children(item.detail.map(|detail| {
                    div()
                        .type_scale(theme, TypeScale::Caption)
                        .text_tone(theme, TextTone::Muted)
                        .child(detail)
                })),
        )
        .children((item.quantity != 1).then(|| {
            Badge::new(
                cx.strings()
                    .format(StringKey::GameRewardQuantity, &[quantity.as_ref()]),
            )
            .id(ident.child("quantity"))
            .tone(Tone::Neutral)
        }))
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Group)
                .text(item.label)
                .value(quantity),
        )
        .animate_in_staggered(ident.child("in").element_id(), cx, Entrance::Fade, index, 8)
        .into_any_element()
}

fn tone_color(tone: Tone, theme: &gpui_kit_theme::Theme) -> gpui::Hsla {
    match tone {
        Tone::Neutral => theme.colors.text_muted,
        Tone::Accent => theme.colors.accent_strong,
        Tone::Success => theme.colors.success,
        Tone::Warning => theme.colors.warning,
        Tone::Danger => theme.colors.danger,
        Tone::Info => theme.colors.info,
    }
}
