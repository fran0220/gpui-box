//! A rich preview that opens on hover and can be reached.
//!
//! # What separates this from a tooltip and from a popover
//!
//! A [`Tooltip`](crate::overlay::Tooltip) is help. It is never actionable, it
//! never holds the only copy of anything, and it is allowed to vanish the
//! instant the pointer leaves, because nobody was ever going to point at it.
//!
//! A [`Popover`](crate::overlay::Popover) is a surface you opened on purpose,
//! by clicking. It has no hover behaviour to get wrong.
//!
//! A hover card is the awkward third thing: it opens by hover *and* holds
//! content worth reaching — a link, a button, text to read. So the pointer has
//! to be able to travel from the trigger into the card, and between the two
//! there is a gap the surface does not cover. A card that closed the moment
//! the pointer left the trigger would put its content behind a race the user
//! loses every time, which is a broken component rather than a fussy one.
//!
//! # The grace period
//!
//! Two facts are tracked, not one: whether the pointer is over the trigger and
//! whether it is over the card. Leaving *both* starts a countdown; entering
//! *either* cancels it. Only a countdown that runs out closes the card, so the
//! diagonal trip across the gap is a period during which the card is leaving
//! and has not left, and arriving anywhere inside it calls the whole thing
//! off.
//!
//! Opening has its own countdown, for the opposite reason: a card that opened
//! the instant a pointer crossed it would flash open and shut all the way
//! across a row of them. Leaving the trigger before that countdown runs out
//! cancels it, so a pointer passing through opens nothing.
//!
//! Both durations are caller-settable. Their defaults are motion tokens so a
//! product can tune interaction tempo consistently; the setters remain for a
//! trigger whose physical distance or policy genuinely differs.
//!
//! # The keyboard
//!
//! Hover is not the only way in. The trigger is a tab stop; focusing it opens
//! the card at once, with no delay, because a keyboard user did not wander
//! there by accident. The card is a tab stop of its own so its content can be
//! reached, and escape closes the card and hands the keyboard back to the
//! trigger.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space};
use web_time::Instant;

use crate::foundation::{FocusRing, Ident, StyledExt};
use crate::overlay::layer::{Hang, Overlay, OverlaySurface, Placement, surface};
use crate::overlay::popover::anchored_slot;

/// What the card is currently counting towards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Opening,
    Leaving,
}

/// What a hover card reports. The owner decides what any of it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverCardEvent {
    Opened,
    Closed,
}

impl EventEmitter<HoverCardEvent> for HoverCard {}

/// Builds the trigger or the card body for one frame.
type Content = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// A preview surface that opens on hover and can be pointed at.
pub struct HoverCard {
    ident: Ident,
    focus_handle: FocusHandle,
    trigger_focus: FocusHandle,
    trigger: Option<Content>,
    /// What the trigger is called, for a reader who has only the tree.
    name: Option<SharedString>,
    content: Option<Content>,
    placement: Placement,
    hang: Hang,
    open_delay: Duration,
    grace: Duration,
    over_trigger: bool,
    over_card: bool,
    open: bool,
    /// The phase in flight and how much of it is left.
    countdown: Option<(Phase, Duration)>,
    /// When the last frame that spent the countdown happened.
    last_tick: Option<Instant>,
    /// Set by opening, cleared by the first frame that can act on it.
    pending_focus: bool,
}

impl std::fmt::Debug for HoverCard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HoverCard")
            .field("ident", &self.ident)
            .field("open", &self.open)
            .field("over_trigger", &self.over_trigger)
            .field("over_card", &self.over_card)
            .field("countdown", &self.countdown)
            .finish()
    }
}

impl HoverCard {
    pub fn new(ident: impl Into<Ident>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            trigger_focus: cx.focus_handle(),
            trigger: None,
            name: None,
            content: None,
            placement: Placement::Below,
            hang: Hang::Start,
            open_delay: Duration::from_millis(cx.theme().motion.hover_card_open_ms),
            grace: Duration::from_millis(cx.theme().motion.hover_card_grace_ms),
            over_trigger: false,
            over_card: false,
            open: false,
            countdown: None,
            last_tick: None,
            pending_focus: false,
        }
    }

    /// Supplies what is hovered, rebuilt on every frame.
    pub fn trigger(
        mut self,
        trigger: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.trigger = Some(Rc::new(trigger));
        self
    }

    /// Names the trigger. A preview hanging off a picture or an avatar has no
    /// words of its own, and a control nobody can name is one nobody can
    /// reach.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Supplies the card body, rebuilt on every frame the card is open.
    pub fn content(
        mut self,
        content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.content = Some(Rc::new(content));
        self
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Which of the trigger's edges the surface hangs from.
    ///
    /// A trigger near the trailing edge of the window wants [`Hang::End`]: the
    /// surface then grows back across the page instead of being slid sideways
    /// off its trigger to stay inside the window.
    pub fn hang(mut self, hang: Hang) -> Self {
        self.hang = hang;
        self
    }

    /// How long the pointer rests before the card opens. Zero opens at once.
    pub fn open_delay(mut self, delay: Duration) -> Self {
        self.open_delay = delay;
        self
    }

    /// How long the card survives the pointer being over neither surface.
    pub fn grace(mut self, grace: Duration) -> Self {
        self.grace = grace;
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// True while the card is on screen with the pointer over neither surface,
    /// which is the window during which the trip can still be completed.
    pub fn is_leaving(&self) -> bool {
        matches!(self.countdown, Some((Phase::Leaving, _)))
    }

    pub fn grace_period(&self) -> Duration {
        self.grace
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        self.countdown = None;
        self.last_tick = None;
        if self.open {
            return;
        }
        self.open = true;
        self.pending_focus = true;
        cx.emit(HoverCardEvent::Opened);
        cx.notify();
    }

    /// Closes the card without giving the keyboard back, which is what a
    /// pointer leaving should do: the keyboard was never here.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.countdown = None;
        self.last_tick = None;
        if !self.open {
            return;
        }
        self.open = false;
        self.pending_focus = false;
        cx.emit(HoverCardEvent::Closed);
        cx.notify();
    }

    /// Closes and hands the keyboard back to the trigger, which is what escape
    /// should do.
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.close(cx);
        self.trigger_focus.focus(window, cx);
    }

    fn set_over_trigger(&mut self, over: bool, cx: &mut Context<Self>) {
        if self.over_trigger == over {
            return;
        }
        self.over_trigger = over;
        self.reconsider(cx);
    }

    fn set_over_card(&mut self, over: bool, cx: &mut Context<Self>) {
        if self.over_card == over {
            return;
        }
        self.over_card = over;
        self.reconsider(cx);
    }

    /// Decides what the two hover facts now mean.
    fn reconsider(&mut self, cx: &mut Context<Self>) {
        let inside = self.over_trigger || self.over_card;
        match (self.open, inside) {
            // Arriving anywhere inside calls off a departure in progress. This
            // is the line that makes the trip across the gap survivable.
            (true, true) => {
                if self.countdown.is_some() {
                    self.countdown = None;
                    self.last_tick = None;
                    cx.notify();
                }
            }
            (true, false) => self.start(Phase::Leaving, self.grace, cx),
            (false, true) => self.start(Phase::Opening, self.open_delay, cx),
            (false, false) => {
                if self.countdown.is_some() {
                    self.countdown = None;
                    self.last_tick = None;
                    cx.notify();
                }
            }
        }
    }

    fn start(&mut self, phase: Phase, duration: Duration, cx: &mut Context<Self>) {
        if matches!(self.countdown, Some((current, _)) if current == phase) {
            return;
        }
        if duration.is_zero() {
            match phase {
                Phase::Opening => self.open(cx),
                Phase::Leaving => self.close(cx),
            }
            return;
        }
        self.countdown = Some((phase, duration));
        self.last_tick = None;
        cx.notify();
    }

    /// Spends one frame of whichever countdown is running.
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((phase, remaining)) = self.countdown else {
            self.last_tick = None;
            return;
        };
        let now = cx.background_executor().now();
        let spent = self
            .last_tick
            .map(|last| now.saturating_duration_since(last))
            .unwrap_or_default();
        let left = remaining.saturating_sub(spent);
        if left.is_zero() {
            match phase {
                Phase::Opening => self.open(cx),
                Phase::Leaving => self.close(cx),
            }
            return;
        }
        self.countdown = Some((phase, left));
        self.last_tick = Some(now);
        window.request_animation_frame();
    }

    fn on_trigger_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "enter" | "space" => {
                if self.open {
                    self.dismiss(window, cx);
                } else {
                    self.open(cx);
                }
                cx.stop_propagation();
            }
            "escape" if self.open => {
                self.dismiss(window, cx);
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    fn on_card_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key.as_str() != "escape" {
            return;
        }
        self.dismiss(window, cx);
        cx.stop_propagation();
    }
}

impl Focusable for HoverCard {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.trigger_focus.clone()
    }
}

impl Render for HoverCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.tick(window, cx);
        let theme = cx.theme().clone();
        let trigger_ident = self.ident.child("trigger");
        let card_ident = self.ident.child("card");

        let trigger_body = self.trigger.clone().map(|build| build(window, cx));
        let trigger = div()
            .id(trigger_ident.element_id())
            .flex()
            .flex_none()
            .items_center()
            .tab_index(0)
            .track_focus(&self.trigger_focus)
            .focus_ring(&theme)
            .on_hover(cx.listener(|card, hovered: &bool, _, cx| {
                card.set_over_trigger(*hovered, cx);
            }))
            .on_key_down(cx.listener(Self::on_trigger_key))
            .children(trigger_body)
            .semantic_in(cx, {
                let mut spec = NodeSpec::new(trigger_ident.semantic_id(), Role::Button)
                    .parent(self.ident.semantic_id())
                    .expanded(self.open)
                    .focus(&self.trigger_focus);
                if let Some(name) = self.name.clone() {
                    spec = spec.text(name);
                }
                spec
            })
            .into_any_element();

        let overlay = self.open.then(|| {
            if self.pending_focus {
                // Focusing the trigger is what a keyboard opening should
                // leave behind; a pointer opening never took the keyboard in
                // the first place, so nothing else is moved here.
                self.pending_focus = false;
            }
            let body = self.content.clone().map(|build| build(window, cx));
            let card = surface(&theme, OverlaySurface::FLOATING)
                .id(card_ident.element_id())
                .max_w(px(theme.measures.compact_overlay_width))
                .p_token(&theme, Space::Sm)
                .gap_token(&theme, Space::Xs)
                .tab_index(0)
                .track_focus(&self.focus_handle)
                .focus_ring(&theme)
                .on_hover(cx.listener(|card, hovered: &bool, _, cx| {
                    card.set_over_card(*hovered, cx);
                }))
                .on_key_down(cx.listener(Self::on_card_key))
                .children(body)
                .semantic_in(
                    cx,
                    NodeSpec::new(card_ident.semantic_id(), Role::Group)
                        .parent(self.ident.semantic_id())
                        .focus(&self.focus_handle),
                );

            Overlay::new(self.ident.child("overlay"))
                .placement(self.placement)
                .hang(self.hang)
                .child(card)
                .into_any_element()
        });

        anchored_slot(self.placement, self.hang, trigger, overlay).semantic_in(
            cx,
            NodeSpec::new(self.ident.semantic_id(), Role::Group).expanded(self.open),
        )
    }
}
