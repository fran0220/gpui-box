//! A question the agent must have answered before it can go on.
//!
//! # Why this is not an approval
//!
//! [`ApprovalPrompt`](crate::agent::ApprovalPrompt) asks for consent to do one
//! specific thing, and every rule in it exists to make refusal the accident-
//! proof default. Nothing here is like that. A clarification is the agent
//! admitting the request was ambiguous and offering its own guesses at what
//! was meant. There is no dangerous direction to protect, so there is no
//! decline-first ordering, no deliberate travel before the safe answer, and no
//! symmetric pair of buttons. Reusing the approval shape for it would tell the
//! reader they are consenting to something when they are only disambiguating.
//!
//! # The candidates are the agent's guesses
//!
//! An option can be unavailable, and it says why in the host's words rather
//! than vanishing from the list: a candidate that was withdrawn is a fact
//! about the question, and a list that silently shrinks is one the reader
//! cannot reason about.
//!
//! # One answer takes one gesture
//!
//! A single-answer question is answered by picking. Picking and then
//! confirming would be two gestures for one decision, and a confirm step only
//! earns its place when there is something to accumulate — which is exactly
//! when [`multiple`](ClarificationPanel::multiple) is on.
//!
//! # Filled means "this is your answer"
//!
//! Selection is the library's tonal fill and nothing else: no tick, no
//! checkbox, no outline, no edge rail. That holds after the question resolves,
//! so a settled panel still shows which candidate was chosen, next to the ones
//! that were not.
//!
//! # The component answers nothing by itself
//!
//! It reports what was picked and leaves the status to the caller, so a host
//! that withdraws or supersedes a question says so through
//! [`set_status`](ClarificationPanel::set_status) rather than by removing the
//! panel and leaving the reader to wonder where it went.

use gpui::{
    AnyElement, App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement,
    Styled, Window, div, prelude::FluentBuilder,
};
use gpui_kit_assets::Icon as Glyph;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};

use crate::controls::button::Button;
use crate::display::badge::Tone;
use crate::display::icon::Icon as IconView;
use crate::display::status::{StatusDot, StatusLine};
use crate::foundation::{
    CardVariant, Disableable, FocusRing, Hoverable, Ident, Pressable, SelectedFill, StyledExt, text,
};
use crate::strings::{ActiveStrings, StringKey};

/// One candidate answer the agent is offering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClarificationOption {
    id: SharedString,
    label: SharedString,
    detail: Option<SharedString>,
    /// Why this candidate cannot presently be picked, in the host's words.
    /// `None` is the available case, so an unavailable option cannot be built
    /// without saying what is wrong with it.
    unavailable: Option<SharedString>,
}

impl ClarificationOption {
    /// `id` is the business identity of the candidate, which is what the panel
    /// reports and what its semantic id derives from. It is never the
    /// position: reordering the candidates must not rename them.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            unavailable: None,
        }
    }

    /// One more line about this candidate: the path it resolves to, the
    /// version it means, what picking it would cost.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Marks the candidate as one that cannot be picked, and says why. The
    /// reason is shown; the option keeps its place and installs no handler.
    pub fn unavailable(mut self, reason: impl Into<SharedString>) -> Self {
        self.unavailable = Some(reason.into());
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    /// The extra line about this candidate, if the caller gave one.
    pub fn detail_text(&self) -> Option<&SharedString> {
        self.detail.as_ref()
    }

    /// Why this candidate cannot be picked, if it cannot.
    pub fn unavailable_reason(&self) -> Option<&SharedString> {
        self.unavailable.as_ref()
    }

    pub fn is_available(&self) -> bool {
        self.unavailable.is_none()
    }
}

/// What happened to the question.
///
/// Answered, skipped, withdrawn and superseded are four different facts and
/// are kept apart for the same reason approval keeps declined, expired and
/// superseded apart: a question nobody answered was not answered, and a
/// question the agent stopped needing was not skipped by the reader.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ClarificationStatus {
    /// Still waiting on the reader.
    #[default]
    Pending,
    /// Answered, with the ids that were picked.
    Answered(Vec<SharedString>),
    /// The reader declined to choose and let the agent decide for itself.
    /// Only reachable when the panel is
    /// [`skippable`](ClarificationPanel::skippable).
    Skipped,
    /// The agent stopped needing the answer, in its own words.
    Withdrawn(SharedString),
    /// A later question took this one's place.
    Superseded { by: SharedString },
}

impl ClarificationStatus {
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// The stable name published on the semantic node, so a test tells the
    /// five apart without reading a colour.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Answered(_) => "answered",
            Self::Skipped => "skipped",
            Self::Withdrawn(_) => "withdrawn",
            Self::Superseded { .. } => "superseded",
        }
    }

    /// The ids that were picked, if the question was answered.
    pub fn answer(&self) -> Option<&[SharedString]> {
        match self {
            Self::Answered(ids) => Some(ids),
            _ => None,
        }
    }
}

/// What the panel reports without applying it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClarificationEvent {
    /// The ids that were picked, in the order they were picked.
    Answered(Vec<SharedString>),
    /// The reader chose to let the agent decide.
    Skipped,
}

impl EventEmitter<ClarificationEvent> for ClarificationPanel {}

/// One clarifying question, with the answer left to the caller.
///
/// It is a view rather than a builder because a multiple-answer question
/// accumulates a selection across frames, and because where the keyboard is
/// has to survive one.
pub struct ClarificationPanel {
    ident: Ident,
    focus_handle: FocusHandle,
    option_focus: Vec<FocusHandle>,
    answer_focus: FocusHandle,
    skip_focus: FocusHandle,
    /// What the agent needs to know, in its own words. Required by the
    /// constructor: a clarification with no question is not one this component
    /// can render.
    question: SharedString,
    options: Vec<ClarificationOption>,
    multiple: bool,
    skippable: bool,
    /// The ids picked so far, in the order they were picked, which is the
    /// order they are reported in.
    picked: Vec<SharedString>,
    status: ClarificationStatus,
    /// Cleared by the first frame that can act on it, as in `ApprovalPrompt`.
    pending_focus: bool,
}

impl std::fmt::Debug for ClarificationPanel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClarificationPanel")
            .field("ident", &self.ident)
            .field("question", &self.question)
            .field("options", &self.options)
            .field("multiple", &self.multiple)
            .field("skippable", &self.skippable)
            .field("picked", &self.picked)
            .field("status", &self.status)
            .finish()
    }
}

impl ClarificationPanel {
    /// `question` is what the agent needs to know, stated in full. There is no
    /// way to render the question as a category instead.
    pub fn new(
        ident: impl Into<Ident>,
        question: impl Into<SharedString>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            option_focus: Vec::new(),
            answer_focus: cx.focus_handle(),
            skip_focus: cx.focus_handle(),
            question: question.into(),
            options: Vec::new(),
            multiple: false,
            skippable: false,
            picked: Vec::new(),
            status: ClarificationStatus::Pending,
            pending_focus: true,
        }
    }

    pub fn option(mut self, option: ClarificationOption) -> Self {
        self.options.push(option);
        self
    }

    pub fn options(mut self, options: impl IntoIterator<Item = ClarificationOption>) -> Self {
        self.options.extend(options);
        self
    }

    /// Accepts more than one answer, which is what adds the confirming
    /// control. Without it the question takes exactly one answer and picking
    /// is answering.
    pub fn multiple(mut self) -> Self {
        self.multiple = true;
        self
    }

    /// Offers the reader a way to decline to choose and let the agent decide.
    /// Off by default, because a question the agent cannot proceed without is
    /// the ordinary case and a skip control on one is a lie.
    pub fn skippable(mut self) -> Self {
        self.skippable = true;
        self
    }

    pub fn status(mut self, status: ClarificationStatus) -> Self {
        self.status = status;
        self
    }

    pub fn current_status(&self) -> &ClarificationStatus {
        &self.status
    }

    /// Records what happened to the question. A host that withdraws or
    /// supersedes one says so through here rather than by removing the panel,
    /// so the reader finds out why the controls went away.
    pub fn set_status(&mut self, status: ClarificationStatus, cx: &mut Context<Self>) {
        self.status = status;
        cx.notify();
    }

    /// The candidates the panel was given.
    pub fn candidates(&self) -> &[ClarificationOption] {
        &self.options
    }

    /// What is filled right now: the answer once there is one, and the
    /// selection in progress until then. One question, one place it is
    /// answered from.
    pub fn chosen(&self) -> &[SharedString] {
        match &self.status {
            ClarificationStatus::Answered(ids) => ids,
            _ => &self.picked,
        }
    }

    fn is_chosen(&self, id: &SharedString) -> bool {
        self.chosen().iter().any(|chosen| chosen == id)
    }

    fn option_at(&self, id: &SharedString) -> Option<&ClarificationOption> {
        self.options.iter().find(|option| option.id() == id)
    }

    /// Picks a candidate: toggles it when the question takes several answers,
    /// and answers the question outright when it takes one.
    ///
    /// Refused for an unavailable candidate, an unknown id, and a question
    /// that is no longer pending, so a host driving the panel cannot reach a
    /// state the pointer and the keyboard cannot.
    pub fn choose(&mut self, id: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = id.into();
        if !self.status.is_pending() {
            return;
        }
        if !self
            .option_at(&id)
            .is_some_and(ClarificationOption::is_available)
        {
            return;
        }
        if self.multiple {
            match self.picked.iter().position(|picked| picked == &id) {
                Some(at) => {
                    self.picked.remove(at);
                }
                None => self.picked.push(id),
            }
            cx.notify();
        } else {
            self.picked = vec![id];
            cx.emit(ClarificationEvent::Answered(self.picked.clone()));
            cx.notify();
        }
    }

    /// Reports the accumulated answer. Refused while nothing is picked, so the
    /// question cannot be answered with silence.
    pub fn answer(&mut self, cx: &mut Context<Self>) {
        if !self.status.is_pending() || self.picked.is_empty() {
            return;
        }
        cx.emit(ClarificationEvent::Answered(self.picked.clone()));
    }

    /// Reports that the reader would rather the agent decided. Refused unless
    /// the caller offered the choice.
    pub fn skip(&mut self, cx: &mut Context<Self>) {
        if !self.status.is_pending() || !self.skippable {
            return;
        }
        cx.emit(ClarificationEvent::Skipped);
    }

    /// The candidates the keyboard can land on, in the order it visits them.
    /// An unavailable candidate is read but not reached, the way a disabled
    /// control is everywhere else in this library.
    fn stops(&self) -> Vec<usize> {
        self.options
            .iter()
            .enumerate()
            .filter(|(_, option)| option.is_available())
            .map(|(at, _)| at)
            .collect()
    }

    /// Moves the keyboard within the candidates.
    ///
    /// This is not a trap: a clarification sits inline in a transcript, and
    /// tab leaves it the way it leaves anything else. Up and down are an
    /// order within the list, which is what makes a long list of candidates
    /// usable without one.
    fn step(&mut self, from: usize, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        let stops = self.stops();
        if stops.is_empty() {
            return;
        }
        let at = stops.iter().position(|stop| *stop == from).unwrap_or(0);
        let next = if back {
            (at + stops.len() - 1) % stops.len()
        } else {
            (at + 1) % stops.len()
        };
        if let Some(handle) = self.option_focus.get(stops[next]) {
            handle.clone().focus(window, cx);
        }
    }

    /// Escape lets go of a question the reader is allowed to let go of, which
    /// is what escape does everywhere else here. A question the agent cannot
    /// proceed without has nothing to escape to, so it does nothing.
    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.status.is_pending() && self.skippable && event.keystroke.key.as_str() == "escape" {
            self.skip(cx);
            cx.stop_propagation();
        }
    }

    /// The settled fact, its tone, and whether the fact carries information
    /// beyond the mark. A pending question says nothing here: the controls are
    /// the message.
    fn outcome(&self, cx: &App) -> Option<(SharedString, Tone, bool)> {
        let strings = cx.strings();
        match &self.status {
            ClarificationStatus::Pending => None,
            // What was answered is already drawn: the chosen candidates carry
            // the tonal fill. Repeating their names here would be the same
            // fact twice, and would need a list separator this library has no
            // token for.
            ClarificationStatus::Answered(_) => Some((
                strings.text(StringKey::ClarificationAnswered),
                Tone::Success,
                false,
            )),
            ClarificationStatus::Skipped => Some((
                strings.text(StringKey::ClarificationSkipped),
                Tone::Neutral,
                false,
            )),
            ClarificationStatus::Withdrawn(reason) => Some((
                strings.format(StringKey::ClarificationWithdrawn, &[reason]),
                Tone::Neutral,
                true,
            )),
            ClarificationStatus::Superseded { by } => Some((
                strings.format(StringKey::ClarificationSuperseded, &[by]),
                Tone::Neutral,
                true,
            )),
        }
    }
}

impl Focusable for ClarificationPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ClarificationPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let pending = self.status.is_pending();

        while self.option_focus.len() < self.options.len() {
            self.option_focus.push(cx.focus_handle());
        }

        let stops = self.stops();
        if pending && self.pending_focus {
            // Unlike an approval there is no answer here that has to be
            // reached deliberately, so the keyboard starts on the first
            // candidate the reader can actually take.
            self.pending_focus = false;
            if let Some(handle) = stops.first().and_then(|at| self.option_focus.get(*at)) {
                handle.clone().focus(window, cx);
            }
        }

        let options_ident = self.ident.child("options");
        let actionable = pending;
        let rows: Vec<AnyElement> = self
            .options
            .iter()
            .enumerate()
            .map(|(at, option)| {
                let ident = options_ident.child(option.id.as_ref());
                let chosen = self.is_chosen(&option.id);
                let available = option.is_available();
                let live = actionable && available;

                let body = div()
                    .column()
                    .min_w_0()
                    .flex_1()
                    .gap_token(&theme, Space::Xxs)
                    .child(text(&theme, TypeScale::Body, option.label.clone()))
                    .children(option.detail.clone().map(|detail| {
                        text(&theme, TypeScale::Caption, detail).text_color(theme.colors.text_muted)
                    }))
                    // Why a candidate cannot be taken belongs to the candidate,
                    // so it is read where the candidate is rather than as a
                    // second message somewhere on the panel.
                    .children(option.unavailable.clone().map(|reason| {
                        text(&theme, TypeScale::Caption, reason).text_color(theme.colors.text_muted)
                    }));

                let id = option.id.clone();
                let row = div()
                    .id(ident.element_id())
                    .row()
                    .w_full()
                    .items_start()
                    .gap_token(&theme, Space::Sm)
                    .p_token(&theme, Space::Sm)
                    .radius(&theme, Radius::Control)
                    // Filled is the answer. Hover is only drawn on a candidate
                    // that is not already the answer: the hover wash is the
                    // weaker of the two tokens, so drawing it over a chosen
                    // row would read as the selection letting go.
                    .selected_fill(&theme, chosen)
                    .when(live && !chosen, |element| element.hover_row(&theme))
                    // An unavailable candidate recedes rather than leaving a
                    // gap, so the list the agent offered stays legible.
                    .when(!available, |element| {
                        element.opacity(theme.opacity.disabled)
                    })
                    .child(body);

                let row = if live {
                    let click = id.clone();
                    let key = id.clone();
                    row.cursor_pointer()
                        .tab_index(0)
                        .track_focus(&self.option_focus[at])
                        .pressable(cx)
                        .focus_ring(&theme)
                        .on_click(cx.listener(move |panel, _, _, cx| {
                            panel.choose(click.clone(), cx);
                        }))
                        .on_key_down(cx.listener(move |panel, event: &KeyDownEvent, window, cx| {
                            match event.keystroke.key.as_str() {
                                // Space takes the candidate under the keyboard.
                                // For a single-answer question that is the
                                // answer; for several it is one more of them.
                                "space" => {
                                    panel.choose(key.clone(), cx);
                                    cx.stop_propagation();
                                }
                                "enter" => {
                                    // Enter finishes. On a question that takes
                                    // several answers that means the ones
                                    // gathered so far, so a typist can toggle
                                    // with space and finish without travelling
                                    // to the button.
                                    if panel.multiple {
                                        panel.answer(cx);
                                    } else {
                                        panel.choose(key.clone(), cx);
                                    }
                                    cx.stop_propagation();
                                }
                                "up" => {
                                    panel.step(at, true, window, cx);
                                    cx.stop_propagation();
                                }
                                "down" => {
                                    panel.step(at, false, window, cx);
                                    cx.stop_propagation();
                                }
                                _ => {}
                            }
                        }))
                } else {
                    row
                };

                row.semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Option)
                        .parent(options_ident.semantic_id())
                        .text(option.label.clone())
                        .selected(chosen)
                        .disabled(!available),
                )
                .into_any_element()
            })
            .collect();

        // With no candidate there is no control to draw. The archive mark says
        // the list is empty while the semantic node keeps the exact fact.
        let empty = self.options.is_empty().then(|| {
            let label = cx.strings().text(StringKey::ClarificationNoOptions);
            div()
                .id(self.ident.child("empty").element_id())
                .child(IconView::new(Glyph::Archive).faint())
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("empty").semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .text(label)
                        .value("empty"),
                )
        });

        // The selectable rows are the control form of the arity hint. Its
        // localized wording remains the list's semantic name without becoming
        // an instruction sentence under the question.
        let arity = cx.strings().text(if self.multiple {
            StringKey::ClarificationPickMany
        } else {
            StringKey::ClarificationPickOne
        });

        let panel = cx.entity().downgrade();

        // The confirming control exists only where there is something to
        // accumulate. A single-answer question is answered by picking, so a
        // button beside it would be a second gesture for one decision.
        let answer = (pending && self.multiple && !self.options.is_empty()).then(|| {
            let panel = panel.clone();
            let nothing_picked = self.picked.is_empty();
            Button::new(self.ident.child("answer"))
                .label(cx.strings().text(StringKey::ClarificationAnswer))
                .primary()
                .semantic_parent(self.ident.semantic_id())
                .track_focus(&self.answer_focus)
                // Answering with nothing picked is not an answer, so the
                // control is disabled and installs no handler rather than
                // accepting the gesture and dropping it.
                .disabled(nothing_picked)
                .on_click(move |_window, cx| {
                    panel.update(cx, |panel, cx| panel.answer(cx)).ok();
                })
        });

        let skip = (pending && self.skippable).then(|| {
            let panel = panel.clone();
            Button::new(self.ident.child("skip"))
                .label(cx.strings().text(StringKey::ClarificationSkip))
                .secondary()
                .semantic_parent(self.ident.semantic_id())
                .track_focus(&self.skip_focus)
                .on_click(move |_window, cx| {
                    panel.update(cx, |panel, cx| panel.skip(cx)).ok();
                })
        });

        let controls = (answer.is_some() || skip.is_some()).then(|| {
            div()
                .row()
                .gap_token(&theme, Space::Sm)
                .children(answer)
                .children(skip)
        });

        let outcome = self.outcome(cx);

        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Md)
            .p_token(&theme, Space::Lg)
            .card_surface(&theme, CardVariant::Elevated)
            .track_focus(&self.focus_handle)
            .when(pending && self.skippable, |element| {
                element.on_key_down(cx.listener(Self::on_key))
            })
            .child(
                div().column().gap_token(&theme, Space::Xxs).child(
                    text(&theme, TypeScale::Body, self.question.clone()).semantic_in(
                        cx,
                        NodeSpec::new(self.ident.child("question").semantic_id(), Role::Text)
                            .text(self.question.clone())
                            .parent(self.ident.semantic_id()),
                    ),
                ),
            )
            .children(empty)
            .when(!rows.is_empty(), |element| {
                element.child(
                    div()
                        .column()
                        .w_full()
                        .gap_token(&theme, Space::Xxs)
                        .children(rows)
                        .semantic_in(
                            cx,
                            NodeSpec::new(options_ident.semantic_id(), Role::List)
                                .parent(self.ident.semantic_id())
                                .text(arity.clone()),
                        ),
                )
            })
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
            .children(controls)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Form)
                    .text(self.question.clone())
                    .value(SharedString::new_static(self.status.name()))
                    .focus(&self.focus_handle),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{AppContext, Entity, TestAppContext};

    use super::*;

    struct Host {
        panel: Entity<ClarificationPanel>,
        _reports: gpui::Subscription,
    }

    impl Render for Host {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            self.panel.clone()
        }
    }

    /// Opens a panel in a window and records everything it reports, which is
    /// the whole of its contract with a host.
    fn open(
        cx: &mut TestAppContext,
        build: impl FnOnce(ClarificationPanel) -> ClarificationPanel + 'static,
    ) -> (
        Entity<ClarificationPanel>,
        Rc<RefCell<Vec<ClarificationEvent>>>,
    ) {
        cx.update(crate::install);
        let reports: Rc<RefCell<Vec<ClarificationEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let slot: Rc<RefCell<Option<Entity<ClarificationPanel>>>> = Rc::new(RefCell::new(None));
        let handle = slot.clone();
        let sink = reports.clone();
        cx.add_window(move |window, cx| {
            let panel = cx.new(|cx| {
                build(ClarificationPanel::new(
                    "test.clarification",
                    "Which one did you mean?",
                    window,
                    cx,
                ))
            });
            *handle.borrow_mut() = Some(panel.clone());
            let reports = cx.subscribe(&panel, move |_, _, event: &ClarificationEvent, _| {
                sink.borrow_mut().push(event.clone());
            });
            Host {
                panel,
                _reports: reports,
            }
        });
        let panel = slot.borrow().clone().expect("panel");
        (panel, reports)
    }

    fn candidates() -> [ClarificationOption; 2] {
        [
            ClarificationOption::new("first", "The first file"),
            ClarificationOption::new("second", "The second file"),
        ]
    }

    fn chosen(panel: &Entity<ClarificationPanel>, cx: &mut TestAppContext) -> Vec<SharedString> {
        cx.update(|cx| panel.read(cx).chosen().to_vec())
    }

    fn pick(panel: &Entity<ClarificationPanel>, id: &str, cx: &mut TestAppContext) {
        cx.update(|cx| panel.update(cx, |panel, cx| panel.choose(id.to_owned(), cx)));
    }

    #[test]
    fn every_status_publishes_its_own_name() {
        let names = [
            ClarificationStatus::Pending.name(),
            ClarificationStatus::Answered(vec!["a".into()]).name(),
            ClarificationStatus::Skipped.name(),
            ClarificationStatus::Withdrawn("done without it".into()).name(),
            ClarificationStatus::Superseded {
                by: "a later one".into(),
            }
            .name(),
        ];
        let mut unique = names.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len());
    }

    /// An unavailable candidate cannot be built without saying what is wrong
    /// with it, and an available one never carries a reason.
    #[test]
    fn only_an_unavailable_candidate_carries_a_reason() {
        let available = ClarificationOption::new("a", "A");
        assert!(available.is_available());
        assert!(available.unavailable_reason().is_none());

        let gone = ClarificationOption::new("b", "B").unavailable("It was deleted.");
        assert!(!gone.is_available());
        assert_eq!(
            gone.unavailable_reason().map(SharedString::to_string),
            Some("It was deleted.".to_string())
        );
    }

    /// One answer takes one gesture: picking a candidate on a single-answer
    /// question is the answer, with no confirming step.
    #[gpui::test]
    fn picking_answers_a_question_that_takes_one_answer(cx: &mut TestAppContext) {
        let (panel, reports) = open(cx, |panel| panel.options(candidates()));
        pick(&panel, "second", cx);

        assert_eq!(
            reports.borrow().as_slice(),
            [ClarificationEvent::Answered(vec!["second".into()])]
        );
        assert_eq!(chosen(&panel, cx), vec![SharedString::from("second")]);
    }

    /// A question that takes several answers accumulates them and reports
    /// nothing until it is answered, in the order they were picked.
    #[gpui::test]
    fn several_answers_accumulate_until_the_question_is_answered(cx: &mut TestAppContext) {
        let (panel, reports) = open(cx, |panel| panel.multiple().options(candidates()));

        pick(&panel, "second", cx);
        pick(&panel, "first", cx);
        assert!(reports.borrow().is_empty(), "picking is not answering here");
        assert_eq!(
            chosen(&panel, cx),
            vec![SharedString::from("second"), SharedString::from("first")]
        );

        cx.update(|cx| panel.update(cx, |panel, cx| panel.answer(cx)));
        assert_eq!(
            reports.borrow().as_slice(),
            [ClarificationEvent::Answered(vec![
                "second".into(),
                "first".into()
            ])]
        );
    }

    /// Picking a candidate that is already picked takes it back, which is the
    /// only way a reader can correct a multiple-answer question.
    #[gpui::test]
    fn picking_a_chosen_candidate_again_takes_it_back(cx: &mut TestAppContext) {
        let (panel, _) = open(cx, |panel| panel.multiple().options(candidates()));
        pick(&panel, "first", cx);
        pick(&panel, "first", cx);
        assert!(chosen(&panel, cx).is_empty());
    }

    /// A question cannot be answered with silence, so the accumulated answer
    /// has to contain something before it is reported.
    #[gpui::test]
    fn a_question_cannot_be_answered_with_nothing_picked(cx: &mut TestAppContext) {
        let (panel, reports) = open(cx, |panel| panel.multiple().options(candidates()));
        cx.update(|cx| panel.update(cx, |panel, cx| panel.answer(cx)));
        assert!(reports.borrow().is_empty());
    }

    /// An unavailable candidate installs no handler, and refuses a host that
    /// drives it directly too, so there is no way to reach a state the pointer
    /// and the keyboard cannot.
    #[gpui::test]
    fn an_unavailable_candidate_cannot_be_picked(cx: &mut TestAppContext) {
        let (panel, reports) = open(cx, |panel| {
            panel
                .option(ClarificationOption::new("here", "Still here"))
                .option(ClarificationOption::new("gone", "Deleted").unavailable("It was deleted."))
        });
        pick(&panel, "gone", cx);

        assert!(reports.borrow().is_empty());
        assert!(chosen(&panel, cx).is_empty());
    }

    #[gpui::test]
    fn a_candidate_that_was_never_offered_cannot_be_picked(cx: &mut TestAppContext) {
        let (panel, reports) = open(cx, |panel| panel.options(candidates()));
        pick(&panel, "invented", cx);

        assert!(reports.borrow().is_empty());
        assert!(chosen(&panel, cx).is_empty());
    }

    /// A settled question cannot be resurrected: picking, answering and
    /// skipping all refuse, so a late host call cannot report a second answer
    /// to a question that already has one.
    #[gpui::test]
    fn a_settled_question_refuses_every_answer(cx: &mut TestAppContext) {
        let (panel, reports) = open(cx, |panel| {
            panel
                .multiple()
                .skippable()
                .options(candidates())
                .status(ClarificationStatus::Answered(vec!["first".into()]))
        });

        pick(&panel, "second", cx);
        cx.update(|cx| {
            panel.update(cx, |panel, cx| {
                panel.answer(cx);
                panel.skip(cx);
            })
        });

        assert!(reports.borrow().is_empty());
        // What is filled is still the answer that was given, not a selection
        // the settled panel picked up afterwards.
        assert_eq!(chosen(&panel, cx), vec![SharedString::from("first")]);
    }

    /// Skipping is only reachable where the caller offered it. A question the
    /// agent cannot proceed without has no skip control and refuses the call.
    #[gpui::test]
    fn only_a_skippable_question_can_be_skipped(cx: &mut TestAppContext) {
        let (blocking, blocking_reports) = open(cx, |panel| panel.options(candidates()));
        cx.update(|cx| blocking.update(cx, |panel, cx| panel.skip(cx)));
        assert!(blocking_reports.borrow().is_empty());

        let (offered, offered_reports) = open(cx, |panel| panel.skippable().options(candidates()));
        cx.update(|cx| offered.update(cx, |panel, cx| panel.skip(cx)));
        assert_eq!(
            offered_reports.borrow().as_slice(),
            [ClarificationEvent::Skipped]
        );
    }

    /// The status is the single source of truth for what was answered once
    /// there is an answer, so a host that sets one is what the panel draws.
    #[gpui::test]
    fn the_answer_is_what_is_filled_once_there_is_one(cx: &mut TestAppContext) {
        let (panel, _) = open(cx, |panel| panel.multiple().options(candidates()));
        pick(&panel, "first", cx);
        assert_eq!(chosen(&panel, cx), vec![SharedString::from("first")]);

        cx.update(|cx| {
            panel.update(cx, |panel, cx| {
                panel.set_status(ClarificationStatus::Answered(vec!["second".into()]), cx);
            })
        });
        assert_eq!(chosen(&panel, cx), vec![SharedString::from("second")]);
    }
}
