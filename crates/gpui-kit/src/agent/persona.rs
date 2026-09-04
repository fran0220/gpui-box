//! Persona presentation over caller-owned agent, dialogue, and voice facts.
//!
//! These components make expressive assistants and game characters possible
//! without becoming a character runtime. The host resolves portrait assets,
//! samples audio, owns dialogue progression, and decides what an action does.
//! Kit owns the reusable crop, expression treatment, voice topology, Markdown
//! composition, motion, accessibility fallbacks, and semantic targets.
//!
//! Voice input is deliberately a bounded scalar snapshot, never a microphone
//! or recognizer. Dialogue reuses [`Markdown`], so it
//! executes no HTML, opens no link, and fetches no image.

use std::fmt;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, ContrastTint, ControlSize, Elevation, Radius, Space, Surface, TextTone, TypeScale,
};
use web_time::Instant;

use crate::agent::model::AgentSnapshot;
use crate::agent::presentation::{AgentActivityLine, AgentAvatar};
use crate::content::{CodeBlock, CodeSpan, ImageRequest, Markdown, MarkdownEvent, MessageBody};
use crate::controls::button::Button;
use crate::display::badge::Tone;
use crate::display::status::StatusDot;
use crate::effects::{EffectParticles, EffectPlan};
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{Disableable, Ident, Selectable, Sizable, StyledExt};
use crate::motion::{Activity, MotionPolicy, MotionRole, ResolvedMotion, keyed};
use crate::strings::{ActiveStrings, StringKey};

const EXPRESSION_MARK_RADIUS: f32 = 1.0;

/// A caller-observed expression, independent of execution state.
///
/// An agent may be focused while idle or warm while speaking. Keeping this
/// fact separate prevents portrait styling from changing what the execution
/// surface reports.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PersonaExpression {
    #[default]
    Neutral,
    Warm,
    Focused,
    Concerned,
    Celebrating,
    /// A host-owned expression name. It receives the neutral visual treatment
    /// rather than guessing what the name means.
    Custom(SharedString),
}

impl PersonaExpression {
    pub fn name(&self) -> &str {
        match self {
            Self::Neutral => "neutral",
            Self::Warm => "warm",
            Self::Focused => "focused",
            Self::Concerned => "concerned",
            Self::Celebrating => "celebrating",
            Self::Custom(name) => name,
        }
    }
}

/// Whether a host-observed voice channel is active.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum VoiceState {
    #[default]
    Silent,
    Listening,
    Speaking,
    Unavailable(SharedString),
}

impl VoiceState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Silent => "silent",
            Self::Listening => "listening",
            Self::Speaking => "speaking",
            Self::Unavailable(_) => "unavailable",
        }
    }

    fn active(&self) -> bool {
        matches!(self, Self::Listening | Self::Speaking)
    }
}

/// Which normalized voice field failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceField {
    Level,
    Envelope,
}

/// Why a voice sample was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceSampleErrorKind {
    NonFinite,
    OutOfRange,
}

/// A rejected voice sample. Invalid values are never clamped into plausible
/// audio facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceSampleError {
    pub field: VoiceField,
    pub kind: VoiceSampleErrorKind,
}

impl fmt::Display for VoiceSampleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let field = match self.field {
            VoiceField::Level => "level",
            VoiceField::Envelope => "envelope",
        };
        let reason = match self.kind {
            VoiceSampleErrorKind::NonFinite => "must be finite",
            VoiceSampleErrorKind::OutOfRange => "must be between 0 and 1",
        };
        write!(formatter, "voice {field} {reason}")
    }
}

impl std::error::Error for VoiceSampleError {}

/// One bounded, caller-sampled voice observation.
///
/// `level` is the current normalized energy and `envelope` is the host's
/// normalized smoothed energy. Kit turns both into bars and pulses; callers do
/// not pass bar heights, colours, smoothing constants, or animation timing.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceSample {
    state: VoiceState,
    level: f32,
    envelope: f32,
}

impl VoiceSample {
    pub fn new(state: VoiceState, level: f32, envelope: f32) -> Result<Self, VoiceSampleError> {
        validate_voice_value(VoiceField::Level, level)?;
        validate_voice_value(VoiceField::Envelope, envelope)?;
        Ok(Self {
            state,
            level,
            envelope,
        })
    }

    pub fn silent() -> Self {
        Self {
            state: VoiceState::Silent,
            level: 0.0,
            envelope: 0.0,
        }
    }

    pub fn state(&self) -> &VoiceState {
        &self.state
    }

    pub fn level(&self) -> f32 {
        self.level
    }

    pub fn envelope(&self) -> f32 {
        self.envelope
    }
}

impl Default for VoiceSample {
    fn default() -> Self {
        Self::silent()
    }
}

fn validate_voice_value(field: VoiceField, value: f32) -> Result<(), VoiceSampleError> {
    if !value.is_finite() {
        return Err(VoiceSampleError {
            field,
            kind: VoiceSampleErrorKind::NonFinite,
        });
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(VoiceSampleError {
            field,
            kind: VoiceSampleErrorKind::OutOfRange,
        });
    }
    Ok(())
}

/// A standard voice-reactive visualization from a normalized host sample.
///
/// Live samples animate from one component-owned timeline. [`Self::sample_at`]
/// freezes that timeline at an absolute instant for replay and headless
/// rendering. Reduced motion shows the current envelope as a static symmetric
/// meter and schedules no frames.
#[derive(Debug, IntoElement)]
pub struct VoiceReactive {
    ident: Ident,
    sample: VoiceSample,
    sample_at: Option<Duration>,
}

impl VoiceReactive {
    pub fn new(ident: impl Into<Ident>, sample: VoiceSample) -> Self {
        Self {
            ident: ident.into(),
            sample,
            sample_at: None,
        }
    }

    pub fn sample_at(mut self, elapsed: Duration) -> Self {
        self.sample_at = Some(elapsed);
        self
    }
}

impl RenderOnce for VoiceReactive {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let motion = voice_motion(&self.sample.state, cx);
        let elapsed = voice_elapsed(
            &self.ident,
            &self.sample,
            self.sample_at,
            motion,
            window,
            cx,
        );
        let bars = voice_levels(&self.sample, elapsed, motion, 7);
        let state = self.sample.state.name();
        let color = voice_color(&self.sample.state, &theme);
        let unavailable = match &self.sample.state {
            VoiceState::Unavailable(reason) => Some(reason.clone()),
            _ => None,
        };
        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Progress)
            .text(state)
            .value(state)
            .range(0.0, 1.0, self.sample.level)
            .busy(self.sample.state.active())
            .invalid(matches!(self.sample.state, VoiceState::Unavailable(_)));
        if let Some(reason) = unavailable.clone() {
            spec = spec.description(reason.clone());
        }

        div()
            .column()
            .items_center()
            .justify_center()
            .gap_token(&theme, Space::Xs)
            // The bars sit in a track. Loose ticks on the page gave the
            // reading no floor and no ceiling, so nothing said how loud full
            // scale is or where silence would be drawn. With no signal at
            // all the track holds a flat line rather than a waveform: a
            // refused microphone has no levels to show, and drawing seven
            // red bars for one claimed a signal that was never captured.
            .child(
                div()
                    .row()
                    .items_center()
                    .justify_center()
                    .gap(px(theme.space(Space::Xxs) * 1.5))
                    .h(px(42.0))
                    .px_token(&theme, Space::Xs)
                    .radius(&theme, Radius::Control)
                    .well(&theme)
                    .map(|track| {
                        if unavailable.is_some() {
                            track.child(
                                div()
                                    .w(px(58.0))
                                    .h(px(2.0))
                                    .rounded_full()
                                    .bg(theme.colors.text_faint),
                            )
                        } else {
                            track.children(bars.into_iter().map(|level| {
                                div()
                                    .w(px(4.0))
                                    .h(px(5.0 + level * 31.0))
                                    .rounded_full()
                                    .bg(color)
                            }))
                        }
                    }),
            )
            .children(unavailable.map(|reason| {
                div()
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.danger)
                    .child(reason)
            }))
            .semantic_in(cx, spec)
    }
}

#[derive(Debug, Default)]
struct VoiceClock(Option<Instant>);

fn voice_elapsed(
    ident: &Ident,
    sample: &VoiceSample,
    sample_at: Option<Duration>,
    motion: ResolvedMotion,
    window: &mut Window,
    cx: &mut App,
) -> Duration {
    if !motion.animates() {
        return Duration::ZERO;
    }
    if let Some(elapsed) = sample_at {
        return elapsed;
    }
    if !sample.state.active() {
        return Duration::ZERO;
    }

    let slot = keyed::slot::<VoiceClock>(
        &ident.child("clock").semantic_id(),
        window.window_handle().window_id(),
        cx,
    );
    let now = cx.background_executor().now();
    let elapsed = {
        let mut clock = slot.borrow_mut();
        let started = *clock.0.get_or_insert(now);
        now.saturating_duration_since(started)
    };
    window.request_animation_frame();
    elapsed
}

fn voice_motion(state: &VoiceState, cx: &App) -> ResolvedMotion {
    let activity = match state {
        VoiceState::Speaking => Activity::Working,
        VoiceState::Listening | VoiceState::Silent | VoiceState::Unavailable(_) => {
            Activity::Deliberating
        }
    };
    MotionPolicy::resolve(MotionRole::Activity(activity), cx)
}

fn voice_levels(
    sample: &VoiceSample,
    elapsed: Duration,
    motion: ResolvedMotion,
    count: usize,
) -> Vec<f32> {
    let center = (count.saturating_sub(1)) as f32 / 2.0;
    let time = elapsed.as_secs_f32();
    (0..count)
        .map(|index| {
            let distance = ((index as f32 - center).abs() / center.max(1.0)).min(1.0);
            let envelope_shape = 1.0 - distance * 0.48;
            let carrier = if !motion.animates() || !sample.state.active() {
                0.58
            } else {
                let speed = std::f32::consts::TAU / motion.spec().total().as_secs_f32();
                0.48 + 0.52 * (time * speed + index as f32 * 0.82).sin().abs()
            };
            let floor = if sample.state.active() { 0.12 } else { 0.06 };
            (floor
                + sample.envelope * 0.34 * envelope_shape
                + sample.level * 0.58 * carrier * envelope_shape)
                .clamp(0.0, 1.0)
        })
        .collect()
}

fn voice_color(state: &VoiceState, theme: &gpui_kit_theme::Theme) -> Hsla {
    let color = match state {
        VoiceState::Silent => theme.colors.text_muted,
        VoiceState::Listening => theme.colors.info,
        VoiceState::Speaking => theme.colors.accent_strong,
        VoiceState::Unavailable(_) => theme.colors.danger,
    };
    theme.contrast_tint(color, ContrastTint::Soft)
}

/// A large expressive portrait built on the standard [`AgentAvatar`].
///
/// An optional [`EffectPlan`] is a decorative layer only. Its semantic event,
/// budget, replay, quality, and reduced-motion decision were already resolved
/// by the shared effects policy.
#[derive(Debug, IntoElement)]
pub struct PersonaPortrait {
    ident: Ident,
    agent: AgentSnapshot,
    expression: PersonaExpression,
    voice: Option<VoiceSample>,
    image: Option<SharedString>,
    tint: Option<Hsla>,
    effect: Option<EffectPlan>,
    sample_at: Option<Duration>,
    size: f32,
}

impl PersonaPortrait {
    pub fn new(ident: impl Into<Ident>, agent: AgentSnapshot) -> Self {
        Self {
            ident: ident.into(),
            agent,
            expression: PersonaExpression::default(),
            voice: None,
            image: None,
            tint: None,
            effect: None,
            sample_at: None,
            size: 88.0,
        }
    }

    pub fn expression(mut self, expression: PersonaExpression) -> Self {
        self.expression = expression;
        self
    }

    pub fn voice(mut self, sample: VoiceSample) -> Self {
        self.voice = Some(sample);
        self
    }

    /// A host-resolved resource path or URI. This component performs no fetch.
    pub fn image(mut self, image: impl Into<SharedString>) -> Self {
        self.image = Some(image.into());
        self
    }

    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn effect(mut self, plan: EffectPlan) -> Self {
        self.effect = Some(plan);
        self
    }

    pub fn sample_at(mut self, elapsed: Duration) -> Self {
        self.sample_at = Some(elapsed);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        if size.is_finite() {
            self.size = size.max(48.0);
        }
        self
    }
}

impl RenderOnce for PersonaPortrait {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let expression_name = self.expression.name().to_owned();
        let expression_color = match self.expression {
            PersonaExpression::Neutral | PersonaExpression::Custom(_) => theme.colors.text_muted,
            PersonaExpression::Warm => theme.colors.accent_strong,
            PersonaExpression::Focused => theme.colors.info,
            PersonaExpression::Concerned => theme.colors.warning,
            PersonaExpression::Celebrating => theme.colors.success,
        };
        let expression_color = theme.contrast_tint(expression_color, ContrastTint::Soft);
        let voice_state = self
            .voice
            .as_ref()
            .map(|voice| voice.state.name())
            .unwrap_or("silent");

        let mut avatar = AgentAvatar::new(self.ident.child("avatar"), self.agent.clone())
            .size(self.size - 12.0)
            .parent(self.ident.clone());
        if let Some(image) = self.image {
            avatar = avatar.image(image);
        }
        if let Some(tint) = self.tint {
            avatar = avatar.tint(tint);
        }

        let expression_marks = expression_marks(&self.expression);
        let indicator = div()
            .absolute()
            .top(px(2.0))
            .left(px((self.size - 24.0) / 2.0))
            .row()
            .items_center()
            .justify_center()
            .gap(px(theme.space(Space::Xxs)))
            .w(px(24.0))
            .h(px(10.0))
            .rounded_full()
            .bg(theme.colors.panel)
            .children((0..expression_marks).map(|index| {
                div()
                    .size(px(if index == 1 { 4.0 } else { 3.0 }))
                    .when(index % 2 == 0, |mark| mark.rounded_full())
                    .when(index % 2 == 1, |mark| {
                        mark.rounded(px(EXPRESSION_MARK_RADIUS))
                    })
                    .bg(expression_color)
            }));

        let voice = self.voice.map(|sample| {
            let motion = voice_motion(&sample.state, cx);
            let elapsed = voice_elapsed(
                &self.ident.child("voice"),
                &sample,
                self.sample_at,
                motion,
                window,
                cx,
            );
            let color = voice_color(&sample.state, &theme);
            // The voice reading sits under the portrait rather than on it.
            // Pinned inside the square it cut a panel-coloured notch out of
            // the state ring and landed in the same corner as the presence
            // mark, so three separate facts overlapped in one place.
            div()
                .row()
                .flex_none()
                .items_center()
                .justify_center()
                .gap(px(theme.space(Space::Xxs)))
                .w(px(38.0))
                .h(px(16.0))
                .rounded_full()
                .well(&theme)
                .children(
                    voice_levels(&sample, elapsed, motion, 5)
                        .into_iter()
                        .map(|level| {
                            div()
                                .w(px(3.0))
                                .h(px(3.0 + level * 10.0))
                                .rounded_full()
                                .bg(color)
                        }),
                )
        });

        let effect = self.effect.map(|plan| {
            let particles = EffectParticles::new(plan);
            let particles = match self.sample_at {
                Some(elapsed) => particles.sample_at(elapsed),
                None => particles,
            };
            div().absolute().inset_0().child(particles)
        });

        div()
            .column()
            .flex_none()
            .items_center()
            .gap(px(theme.space(Space::Xs)))
            .child(
                div()
                    .relative()
                    .size(px(self.size))
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .children(effect)
                    .child(avatar)
                    .child(indicator),
            )
            .children(voice)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .text(self.agent.descriptor.name)
                    .value(format!("{expression_name}:{voice_state}"))
                    .busy(self.agent.execution.busy()),
            )
    }
}

fn expression_marks(expression: &PersonaExpression) -> usize {
    match expression {
        PersonaExpression::Neutral | PersonaExpression::Custom(_) => 1,
        PersonaExpression::Warm | PersonaExpression::Concerned => 2,
        PersonaExpression::Focused | PersonaExpression::Celebrating => 3,
    }
}

/// Whether a dialogue choice can presently be requested.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DialogueChoiceAvailability {
    #[default]
    Available,
    Unavailable(SharedString),
}

/// One caller-owned action offered after a dialogue turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogueChoice {
    id: SharedString,
    label: SharedString,
    detail: Option<SharedString>,
    availability: DialogueChoiceAvailability,
    selected: bool,
}

impl DialogueChoice {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            availability: DialogueChoiceAvailability::Available,
            selected: false,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn unavailable(mut self, reason: impl Into<SharedString>) -> Self {
        self.availability = DialogueChoiceAvailability::Unavailable(reason.into());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// One complete caller-owned persona turn.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogueTurn {
    id: SharedString,
    agent: AgentSnapshot,
    body: MessageBody,
    streaming: bool,
    choices: Vec<DialogueChoice>,
}

impl DialogueTurn {
    pub fn new(
        id: impl Into<SharedString>,
        agent: AgentSnapshot,
        body: impl Into<MessageBody>,
    ) -> Self {
        Self {
            id: id.into(),
            agent,
            body: body.into(),
            streaming: false,
            choices: Vec::new(),
        }
    }

    pub fn markdown(
        id: impl Into<SharedString>,
        agent: AgentSnapshot,
        source: impl Into<SharedString>,
    ) -> Self {
        Self::new(id, agent, MessageBody::Markdown(source.into()))
    }

    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    pub fn choice(mut self, choice: DialogueChoice) -> Self {
        self.choices.push(choice);
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// What persona dialogue reports without applying it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonaDialogueEvent {
    ChoiceRequested {
        turn_id: SharedString,
        choice_id: SharedString,
    },
    Markdown {
        turn_id: SharedString,
        event: MarkdownEvent,
    },
}

type DialogueHandler = Rc<dyn Fn(&PersonaDialogueEvent, &mut Window, &mut App)>;
type DialogueImage =
    Rc<dyn Fn(&SharedString, &ImageRequest, &mut Window, &mut App) -> Option<AnyElement>>;
type DialogueHighlighter = Rc<dyn Fn(&SharedString, &CodeBlock) -> Vec<CodeSpan>>;

/// Persona-aware dialogue over the safe Markdown renderer.
///
/// Portrait placement, expression treatment, status, streaming presentation,
/// choice layout, RTL order, and disabled explanations are all owned here.
/// The component never advances the turn or applies a choice.
#[derive(IntoElement)]
pub struct PersonaDialogue {
    ident: Ident,
    turn: DialogueTurn,
    expression: PersonaExpression,
    voice: Option<VoiceSample>,
    image: Option<SharedString>,
    tint: Option<Hsla>,
    on_event: Option<DialogueHandler>,
    image_source: Option<DialogueImage>,
    highlighter: Option<DialogueHighlighter>,
}

impl fmt::Debug for PersonaDialogue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PersonaDialogue")
            .field("ident", &self.ident)
            .field("turn", &self.turn.id)
            .field("choices", &self.turn.choices.len())
            .field("streaming", &self.turn.streaming)
            .field("has_handler", &self.on_event.is_some())
            .finish()
    }
}

impl PersonaDialogue {
    pub fn new(ident: impl Into<Ident>, turn: DialogueTurn) -> Self {
        Self {
            ident: ident.into(),
            turn,
            expression: PersonaExpression::default(),
            voice: None,
            image: None,
            tint: None,
            on_event: None,
            image_source: None,
            highlighter: None,
        }
    }

    pub fn expression(mut self, expression: PersonaExpression) -> Self {
        self.expression = expression;
        self
    }

    pub fn voice(mut self, sample: VoiceSample) -> Self {
        self.voice = Some(sample);
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

    pub fn on_event(
        mut self,
        handler: impl Fn(&PersonaDialogueEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }

    /// Supplies an already-resolved image for a Markdown request.
    pub fn markdown_image(
        mut self,
        source: impl Fn(&SharedString, &ImageRequest, &mut Window, &mut App) -> Option<AnyElement>
        + 'static,
    ) -> Self {
        self.image_source = Some(Rc::new(source));
        self
    }

    /// Supplies host-computed code spans without moving grammar policy here.
    pub fn markdown_highlighter(
        mut self,
        highlighter: impl Fn(&SharedString, &CodeBlock) -> Vec<CodeSpan> + 'static,
    ) -> Self {
        self.highlighter = Some(Rc::new(highlighter));
        self
    }
}

impl RenderOnce for PersonaDialogue {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let turn_id = self.turn.id.clone();
        let streaming = self.turn.streaming;
        let has_choices = !self.turn.choices.is_empty();
        let mut portrait =
            PersonaPortrait::new(self.ident.child("portrait"), self.turn.agent.clone())
                .expression(self.expression)
                .size(72.0);
        if let Some(voice) = self.voice {
            portrait = portrait.voice(voice);
        }
        if let Some(image) = self.image {
            portrait = portrait.image(image);
        }
        if let Some(tint) = self.tint {
            portrait = portrait.tint(tint);
        }

        let body_ident = self.ident.child("body");
        let body = match self.turn.body {
            MessageBody::Text(text) => div()
                .w_full()
                .type_scale(&theme, TypeScale::Body)
                .text_tone(&theme, TextTone::Primary)
                .child(text)
                .into_any_element(),
            MessageBody::Markdown(source) => {
                let mut markdown = Markdown::new(body_ident, source);
                if let Some(handler) = self.on_event.clone() {
                    let event_turn = turn_id.clone();
                    markdown = markdown.on_event(move |event, window, cx| {
                        handler(
                            &PersonaDialogueEvent::Markdown {
                                turn_id: event_turn.clone(),
                                event: event.clone(),
                            },
                            window,
                            cx,
                        )
                    });
                }
                if let Some(source) = self.image_source {
                    let image_turn = turn_id.clone();
                    markdown = markdown
                        .image(move |request, window, cx| source(&image_turn, request, window, cx));
                }
                if let Some(highlighter) = self.highlighter {
                    let highlight_turn = turn_id.clone();
                    markdown = markdown.highlight(move |block| highlighter(&highlight_turn, block));
                }
                markdown.into_any_element()
            }
        };

        let choices = self.turn.choices.into_iter().map(|choice| {
            let choice_ident = self.ident.child("choice").child(choice.id.as_ref());
            let reason = match &choice.availability {
                DialogueChoiceAvailability::Available => None,
                DialogueChoiceAvailability::Unavailable(reason) => Some(reason.clone()),
            };
            let available = reason.is_none();
            let mut button = Button::new(choice_ident.clone())
                .label(choice.label)
                .secondary()
                .control_size(ControlSize::Sm)
                .selected(choice.selected)
                .disabled(!available);
            if let Some(detail) = choice.detail {
                button = button.accessible_description(detail);
            }
            if let Some(reason) = reason.clone() {
                button = button.accessible_description(reason);
            }
            if let Some(handler) = self.on_event.clone().filter(|_| available) {
                let action_turn = turn_id.clone();
                let choice_id = choice.id.clone();
                button = button.on_click(move |window, cx| {
                    handler(
                        &PersonaDialogueEvent::ChoiceRequested {
                            turn_id: action_turn.clone(),
                            choice_id: choice_id.clone(),
                        },
                        window,
                        cx,
                    )
                });
            }
            div()
                .column()
                .gap_token(&theme, Space::Xs)
                .child(button)
                .children(reason.map(|reason| {
                    div()
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.danger)
                        .child(reason)
                        .semantic_in(
                            cx,
                            NodeSpec::new(choice_ident.child("reason").semantic_id(), Role::Status)
                                .value("unavailable"),
                        )
                }))
        });

        let content = div()
            .min_w_0()
            .flex_1()
            .column()
            .gap_token(&theme, Space::Sm)
            .child(
                // "Streaming" belongs beside the name, on the name's own
                // line. Centred against the two-line block it floated at a
                // baseline neither the name nor the activity line shared,
                // which read as a third, unattached fact.
                div()
                    .column()
                    .gap_token(&theme, Space::Xs)
                    .child(
                        div()
                            .row_reading(direction)
                            .items_center()
                            .gap_token(&theme, Space::Md)
                            .child(
                                div()
                                    .type_scale(&theme, TypeScale::Label)
                                    .text_tone(&theme, TextTone::Primary)
                                    .child(self.turn.agent.descriptor.name.clone()),
                            )
                            .children(streaming.then(|| {
                                let label = cx.strings().text(StringKey::MessageStreaming);
                                div()
                                    .id(self.ident.child("streaming").element_id())
                                    .child(
                                        StatusDot::new(Tone::Accent)
                                            .busy(self.ident.child("streaming-mark")),
                                    )
                                    .semantic_in(
                                        cx,
                                        NodeSpec::new(
                                            self.ident.child("streaming").semantic_id(),
                                            Role::Status,
                                        )
                                        .text(label)
                                        .busy(true),
                                    )
                            })),
                    )
                    .child(AgentActivityLine::new(
                        self.ident.child("activity"),
                        self.turn.agent.execution.clone(),
                    )),
            )
            .child(body)
            .children(has_choices.then(|| {
                div()
                    .row_reading(direction)
                    .flex_wrap()
                    .gap_token(&theme, Space::Sm)
                    .children(choices)
            }));

        div()
            .row_reading(direction)
            .items_start()
            .gap_token(&theme, Space::Lg)
            .w_full()
            .p_token(&theme, Space::Lg)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .child(portrait)
            .child(content)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .text(self.turn.agent.descriptor.name)
                    .value(turn_id)
                    .busy(streaming || self.turn.agent.execution.busy()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_voice_facts_are_rejected_not_clamped() {
        assert_eq!(
            VoiceSample::new(VoiceState::Speaking, f32::NAN, 0.5),
            Err(VoiceSampleError {
                field: VoiceField::Level,
                kind: VoiceSampleErrorKind::NonFinite,
            })
        );
        assert_eq!(
            VoiceSample::new(VoiceState::Listening, 0.5, 1.1),
            Err(VoiceSampleError {
                field: VoiceField::Envelope,
                kind: VoiceSampleErrorKind::OutOfRange,
            })
        );
    }

    #[test]
    fn reduced_motion_voice_shape_is_static_and_symmetric() {
        let sample = VoiceSample::new(VoiceState::Speaking, 0.8, 0.6).expect("valid");
        let theme = gpui_kit_theme::Theme::studio_dark();
        let motion =
            MotionPolicy::resolve_for(MotionRole::Activity(Activity::Working), &theme, true);
        let levels = voice_levels(&sample, Duration::from_secs(9), motion, 7);
        assert_eq!(levels[0], levels[6]);
        assert_eq!(levels[1], levels[5]);
        assert_eq!(levels[2], levels[4]);
    }
}
