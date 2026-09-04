//! A request for permission to do one specific thing.
//!
//! Everything here is arranged around one rule: **the default is refusal.**
//! Nothing in this component makes approving easier than declining by
//! accident.
//!
//! - The keyboard lands on the decline control when the prompt appears, the
//!   way [`Dialog`](crate::overlay::Dialog) opens a destructive confirmation
//!   on cancel.
//! - Return acts on the control that has the keyboard and on nothing else, so
//!   a return key pressed at the wrong moment declines. Approving with the
//!   keyboard takes a deliberate tab first.
//! - Escape declines, which is what escape does everywhere else in this
//!   library, and is the safe direction here rather than merely the
//!   conventional one.
//! - A resolved prompt installs no handler at all.
//!
//! # An unscoped "always" cannot be built
//!
//! [`AlwaysScope`] has no variant that means "always, everywhere". Every
//! variant either is the session itself or carries the one thing it is always
//! for, and the wording on the control is derived from that variant, so a
//! control offering "always" without saying what "always" covers is not a
//! thing a caller can construct.
//!
//! # A prompt that was never answered was not refused
//!
//! [`ApprovalStatus`] keeps `Declined`, `Expired` and `Superseded` apart.
//! They are three different facts about what happened — a person said no,
//! nobody said anything in time, and a later request took this one's place —
//! and each is rendered and published differently.

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, SharedString, Styled, Window, div, prelude::FluentBuilder,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TypeScale};

use crate::controls::button::{Button, ButtonVariant};
use crate::display::badge::Tone;
use crate::display::description_list::{DescriptionItem, DescriptionList};
use crate::display::status::{StatusDot, StatusLine};
use crate::foundation::{CardVariant, Ident, Sizable, StyledExt, text};
use crate::strings::{ActiveStrings, StringKey};

/// What a standing approval covers.
///
/// There is deliberately no bare `Always`. A caller states which of these
/// four kinds of "always" it is offering, and the control words itself from
/// that, so an unscoped standing permission has no representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlwaysScope {
    /// Until this session ends. The session is the scope, so it names nothing
    /// further.
    Session,
    /// Every future use of one named tool.
    Tool(SharedString),
    /// Anything under one named place on disk.
    Path(SharedString),
    /// Every future request to one named host.
    Host(SharedString),
}

impl AlwaysScope {
    pub fn tool(name: impl Into<SharedString>) -> Self {
        Self::Tool(name.into())
    }

    pub fn path(path: impl Into<SharedString>) -> Self {
        Self::Path(path.into())
    }

    pub fn host(host: impl Into<SharedString>) -> Self {
        Self::Host(host.into())
    }

    /// The stable name of the kind of scope, used for the control's id and
    /// published in the semantic tree.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Tool(_) => "tool",
            Self::Path(_) => "path",
            Self::Host(_) => "host",
        }
    }

    /// The thing the scope covers, when it is one named thing.
    pub fn subject(&self) -> Option<&SharedString> {
        match self {
            Self::Session => None,
            Self::Tool(name) | Self::Path(name) | Self::Host(name) => Some(name),
        }
    }

    /// The wording that appears on the control, which always states the
    /// scope.
    pub fn label(&self, cx: &App) -> SharedString {
        let strings = cx.strings();
        match self {
            Self::Session => strings.text(StringKey::ApprovalAlwaysSession),
            Self::Tool(name) => strings.format(StringKey::ApprovalAlwaysTool, &[name]),
            Self::Path(path) => strings.format(StringKey::ApprovalAlwaysPath, &[path]),
            Self::Host(host) => strings.format(StringKey::ApprovalAlwaysHost, &[host]),
        }
    }
}

/// How far an approval reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// This request and nothing else.
    Once,
    /// A standing permission, which always states what it covers.
    Always(AlwaysScope),
}

impl ApprovalDecision {
    /// The wording for the reach of the decision, for the sentence a resolved
    /// prompt shows.
    pub fn label(&self, cx: &App) -> SharedString {
        match self {
            Self::Once => cx.strings().text(StringKey::ApprovalOnceScope),
            Self::Always(scope) => scope.label(cx),
        }
    }
}

/// What has happened to this request.
///
/// Declined, expired and superseded are three different things and are never
/// collapsed into one another.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ApprovalStatus {
    /// Nobody has answered yet. The only status that offers controls.
    #[default]
    Pending,
    /// Somebody answered, and the answer was no.
    Declined,
    /// Somebody answered, and said how far the answer reaches.
    Approved(ApprovalDecision),
    /// The window in which this could be answered closed. Nobody refused it.
    Expired,
    /// A later request took this one's place. The wording naming the
    /// replacement belongs to the host and is shown verbatim.
    Superseded { by: SharedString },
}

impl ApprovalStatus {
    /// The stable name published in the semantic tree.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Declined => "declined",
            Self::Approved(_) => "approved",
            Self::Expired => "expired",
            Self::Superseded { .. } => "superseded",
        }
    }

    fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }
}

/// What the prompt reports. It applies none of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalEvent {
    /// The typist approved, and this is how far the approval reaches.
    Approved(ApprovalDecision),
    /// The typist declined. Also what escape and a stray return key report.
    Declined,
}

impl EventEmitter<ApprovalEvent> for ApprovalPrompt {}

/// One request for permission, with the answer left to the caller.
///
/// It is a view rather than a builder because where the keyboard is has to
/// survive a frame, and where the keyboard is *is* the safety property.
pub struct ApprovalPrompt {
    ident: Ident,
    focus_handle: FocusHandle,
    decline_focus: FocusHandle,
    approve_focus: FocusHandle,
    always_focus: Vec<FocusHandle>,
    /// Exactly what is being asked for, in the caller's words. Required by
    /// the constructor: a prompt with nothing specific to say is not one this
    /// component can render.
    action: SharedString,
    details: Vec<DescriptionItem>,
    always: Vec<AlwaysScope>,
    status: ApprovalStatus,
    /// Cleared by the first frame that can act on it, as in `Dialog`.
    pending_focus: bool,
}

impl std::fmt::Debug for ApprovalPrompt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ApprovalPrompt")
            .field("ident", &self.ident)
            .field("action", &self.action)
            .field("details", &self.details.len())
            .field("always", &self.always)
            .field("status", &self.status)
            .finish()
    }
}

impl ApprovalPrompt {
    /// `action` is what is about to happen, stated specifically. There is no
    /// way to describe the request as a category instead.
    pub fn new(
        ident: impl Into<Ident>,
        action: impl Into<SharedString>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            decline_focus: cx.focus_handle(),
            approve_focus: cx.focus_handle(),
            always_focus: Vec::new(),
            action: action.into(),
            details: Vec::new(),
            always: Vec::new(),
            status: ApprovalStatus::Pending,
            pending_focus: true,
        }
    }

    /// One more specific fact about the request: the exact path, the exact
    /// command, the exact host.
    pub fn detail(mut self, detail: DescriptionItem) -> Self {
        self.details.push(detail);
        self
    }

    pub fn details(mut self, details: impl IntoIterator<Item = DescriptionItem>) -> Self {
        self.details.extend(details);
        self
    }

    /// Offers a standing approval. The scope is part of the argument, so the
    /// control cannot be worded without it.
    pub fn always(mut self, scope: AlwaysScope) -> Self {
        self.always.push(scope);
        self
    }

    pub fn status(mut self, status: ApprovalStatus) -> Self {
        self.status = status;
        self
    }

    pub fn current_status(&self) -> &ApprovalStatus {
        &self.status
    }

    /// Records what happened to the request. A host that expires or supersedes
    /// a prompt says so through here rather than by removing it, so the reader
    /// finds out why the controls went away.
    pub fn set_status(&mut self, status: ApprovalStatus, cx: &mut Context<Self>) {
        self.status = status;
        cx.notify();
    }

    /// Reports an approval. Refused unless the prompt is still pending, so a
    /// host calling this on a resolved prompt cannot resurrect it.
    pub fn approve(&mut self, decision: ApprovalDecision, cx: &mut Context<Self>) {
        if !self.status.is_pending() {
            return;
        }
        cx.emit(ApprovalEvent::Approved(decision));
    }

    pub fn decline(&mut self, cx: &mut Context<Self>) {
        if !self.status.is_pending() {
            return;
        }
        cx.emit(ApprovalEvent::Declined);
    }

    /// The controls, in the order tab visits them. Decline is first, so the
    /// keyboard has to travel to reach an approval.
    fn stops(&self) -> Vec<FocusHandle> {
        let mut stops = vec![self.decline_focus.clone(), self.approve_focus.clone()];
        stops.extend(self.always_focus.iter().cloned());
        stops
    }

    /// Moves the keyboard within the prompt's own controls.
    ///
    /// This is not a trap: an approval prompt sits inline in a page, and the
    /// keyboard leaves it the way it leaves anything else. It is an order,
    /// which the decline-first rule needs in order to mean anything.
    fn step_focus(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        let stops = self.stops();
        let at = stops
            .iter()
            .position(|handle| handle.is_focused(window))
            .map(|at| {
                if back {
                    (at + stops.len() - 1) % stops.len()
                } else {
                    (at + 1) % stops.len()
                }
            })
            .unwrap_or(0);
        stops[at].clone().focus(window, cx);
    }

    /// Return acts on whatever holds the keyboard, and approval is never what
    /// holds it by default. Escape declines.
    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.status.is_pending() {
            return;
        }
        if event.keystroke.key.as_str() == "tab" {
            self.step_focus(event.keystroke.modifiers.shift, window, cx);
            cx.stop_propagation();
            return;
        }
        match event.keystroke.key.as_str() {
            "escape" => {
                self.decline(cx);
                cx.stop_propagation();
            }
            "enter" => {
                if self.approve_focus.is_focused(window) {
                    self.approve(ApprovalDecision::Once, cx);
                } else if let Some(scope) = self
                    .always_focus
                    .iter()
                    .position(|handle| handle.is_focused(window))
                    .and_then(|index| self.always.get(index).cloned())
                {
                    self.approve(ApprovalDecision::Always(scope), cx);
                } else {
                    // Anything else with the keyboard, including nothing at
                    // all, means the typist has not deliberately reached an
                    // approval. Declining is the answer this component gives
                    // when it is not sure.
                    self.decline(cx);
                }
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    fn outcome(&self, cx: &App) -> Option<(SharedString, Tone, bool)> {
        let strings = cx.strings();
        match &self.status {
            ApprovalStatus::Pending => None,
            ApprovalStatus::Declined => Some((
                strings.text(StringKey::ApprovalDeclined),
                Tone::Danger,
                false,
            )),
            ApprovalStatus::Approved(decision) => Some((
                strings.format(StringKey::ApprovalApproved, &[&decision.label(cx)]),
                Tone::Success,
                true,
            )),
            ApprovalStatus::Expired => Some((
                strings.text(StringKey::ApprovalExpired),
                Tone::Warning,
                true,
            )),
            ApprovalStatus::Superseded { by } => Some((
                strings.format(StringKey::ApprovalSuperseded, &[by]),
                Tone::Neutral,
                true,
            )),
        }
    }
}

impl Focusable for ApprovalPrompt {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ApprovalPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let pending = self.status.is_pending();

        while self.always_focus.len() < self.always.len() {
            self.always_focus.push(cx.focus_handle());
        }

        if pending && self.pending_focus {
            // The handle can only take focus once this frame has put it in the
            // dispatch tree.
            self.pending_focus = false;
            self.decline_focus.clone().focus(window, cx);
        }

        let outcome = self.outcome(cx);
        let prompt = cx.entity().downgrade();
        let standing_ident = self.ident.child("standing");

        let decline = pending.then(|| {
            let prompt = prompt.clone();
            Button::new(self.ident.child("decline"))
                .label(cx.strings().text(StringKey::ApprovalDecline))
                .secondary()
                .semantic_parent(self.ident.semantic_id())
                .track_focus(&self.decline_focus)
                .on_click(move |_window, cx| {
                    prompt.update(cx, |prompt, cx| prompt.decline(cx)).ok();
                })
        });

        let approve = pending.then(|| {
            let prompt = prompt.clone();
            Button::new(self.ident.child("approve"))
                .label(cx.strings().text(StringKey::ApprovalApproveOnce))
                // Consent is not a call to action: a primary approval next to
                // a secondary decline reads as the answer the surface wants,
                // which is exactly the pressure a consent prompt must not
                // apply. The two answers carry the same weight.
                .variant(ButtonVariant::Secondary)
                .semantic_parent(self.ident.semantic_id())
                .track_focus(&self.approve_focus)
                .on_click(move |_window, cx| {
                    prompt
                        .update(cx, |prompt, cx| prompt.approve(ApprovalDecision::Once, cx))
                        .ok();
                })
        });

        let always: Vec<_> = if pending {
            self.always
                .iter()
                .zip(self.always_focus.iter())
                .map(|(scope, handle)| {
                    let prompt = prompt.clone();
                    let chosen = scope.clone();
                    // The widest grant on the prompt used to be the only
                    // control with no chrome at all, so the most consequential
                    // thing a reader could press looked like a caption.
                    Button::new(self.ident.child("always").child(scope.name()))
                        .label(scope.label(cx))
                        .secondary()
                        .control_size(ControlSize::Sm)
                        .semantic_parent(standing_ident.semantic_id())
                        .track_focus(handle)
                        .on_click(move |_window, cx| {
                            let chosen = chosen.clone();
                            prompt
                                .update(cx, |prompt, cx| {
                                    prompt.approve(ApprovalDecision::Always(chosen), cx)
                                })
                                .ok();
                        })
                })
                .collect()
        } else {
            Vec::new()
        };

        let details = (!self.details.is_empty())
            .then(|| DescriptionList::new(self.ident.child("detail")).items(self.details.clone()));

        let spec = NodeSpec::new(self.ident.semantic_id(), Role::Form)
            .text(self.action.clone())
            .value(SharedString::new_static(self.status.name()))
            .focus(&self.focus_handle);

        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Md)
            .p_token(&theme, Space::Lg)
            .card_surface(&theme, CardVariant::Elevated)
            .track_focus(&self.focus_handle)
            .when(pending, |element| {
                element.on_key_down(cx.listener(Self::on_key))
            })
            .child(
                text(&theme, TypeScale::Body, self.action.clone()).semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("action").semantic_id(), Role::Text)
                        .text(self.action.clone())
                        .parent(self.ident.semantic_id()),
                ),
            )
            .children(details)
            .when_some(outcome, |element, (message, tone, visible)| {
                if visible {
                    element.child(
                        div().child(StatusLine::new(message, tone).id(self.ident.child("outcome"))),
                    )
                } else {
                    element.child(
                        div()
                            .id(self.ident.child("outcome").element_id())
                            .child(StatusDot::new(tone))
                            .semantic_in(
                                cx,
                                NodeSpec::new(
                                    self.ident.child("outcome").semantic_id(),
                                    Role::Status,
                                )
                                .parent(self.ident.semantic_id())
                                .text(message)
                                .value(SharedString::new_static(self.status.name())),
                            ),
                    )
                }
            })
            .when(pending, |element| {
                element.child(
                    div()
                        .column()
                        .gap_token(&theme, Space::Sm)
                        .child(
                            div()
                                .row()
                                .gap_token(&theme, Space::Sm)
                                .children(decline)
                                .children(approve),
                        )
                        .when(!always.is_empty(), |element| {
                            let label = cx.strings().text(StringKey::ApprovalStanding);
                            element.child(
                                div()
                                    .column()
                                    .gap_token(&theme, Space::Xs)
                                    .child(
                                        text(&theme, TypeScale::Caption, label.clone())
                                            .text_color(theme.colors.text_muted),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .flex_wrap()
                                            .gap_token(&theme, Space::Sm)
                                            .children(always),
                                    )
                                    .semantic_in(
                                        cx,
                                        NodeSpec::new(standing_ident.semantic_id(), Role::Group)
                                            .parent(self.ident.semantic_id())
                                            .text(label),
                                    ),
                            )
                        }),
                )
            })
            .semantic_in(cx, spec)
    }
}
