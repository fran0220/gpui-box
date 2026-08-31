//! The plan an agent is working through, drawn as marks rather than sentences.
//!
//! # Why this is not a list
//!
//! A plan is read at a glance, repeatedly, while something else holds the
//! reader's attention. [`StepList`](crate::agent::StepList) spends a full row
//! of prose on every step and says each state three times — a coloured dot, a
//! word at the trailing edge, and a tinted reason — which is right when the
//! steps are the content and each one carries a body. It is wrong when the
//! plan is context beside a transcript that is itself moving.
//!
//! So the marks carry the plan and the words are spent once, on the item
//! actually in flight. Everything else is a shape: what is behind the reader,
//! what is in front, and what stopped. Nothing is hidden by this — every
//! item's name and state are on its semantic node and in its tooltip, so a
//! screen reader, a test, and a pointer all still have the whole plan.
//!
//! # The component applies nothing
//!
//! Which item is running, which is blocked, and why are the caller's facts.
//! A blocked or dropped item carries the host's own words. The plan reports
//! the id that was picked and changes nothing by itself.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TypeScale};

use crate::display::badge::Tone;
use crate::display::status::StatusDot;
use crate::foundation::{FocusRing, Hoverable, Ident, Pressable, StyledExt, text};
use crate::motion::{self, Activity};
use crate::overlay::Tooltipped;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type PickHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// Where one item of a plan stands.
///
/// Ahead, doing and done are the three a plan is mostly made of. The other two
/// are the ones a plan cannot be honest without: work that stopped, and work
/// the agent decided against, each in the host's own words.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PlanState {
    /// Not started. The reader is not waiting on it yet.
    #[default]
    Ahead,
    /// In flight. At most one item should be, and the plan draws whichever
    /// ones are.
    Doing,
    Done,
    /// Started and stopped, in the host's words.
    Blocked(SharedString),
    /// Decided against before it ran, in the host's words.
    Dropped(SharedString),
}

impl PlanState {
    /// The name the semantic node publishes, so a test tells the five apart
    /// without reading a colour.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ahead => "ahead",
            Self::Doing => "doing",
            Self::Done => "done",
            Self::Blocked(_) => "blocked",
            Self::Dropped(_) => "dropped",
        }
    }

    /// The host's words for why this item is not moving, if it is not.
    pub fn reason(&self) -> Option<&SharedString> {
        match self {
            Self::Blocked(reason) | Self::Dropped(reason) => Some(reason),
            _ => None,
        }
    }

    fn tone(&self) -> Tone {
        match self {
            Self::Ahead | Self::Dropped(_) => Tone::Neutral,
            Self::Doing => Tone::Accent,
            Self::Done => Tone::Success,
            Self::Blocked(_) => Tone::Warning,
        }
    }

    fn counts_as_done(&self) -> bool {
        matches!(self, Self::Done)
    }
}

/// One item of a plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanItem {
    id: SharedString,
    label: SharedString,
    state: PlanState,
}

impl PlanItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state: PlanState::default(),
        }
    }

    pub fn state(mut self, state: PlanState) -> Self {
        self.state = state;
        self
    }

    pub fn doing(self) -> Self {
        self.state(PlanState::Doing)
    }

    pub fn done(self) -> Self {
        self.state(PlanState::Done)
    }

    pub fn blocked(self, reason: impl Into<SharedString>) -> Self {
        self.state(PlanState::Blocked(reason.into()))
    }

    pub fn dropped(self, reason: impl Into<SharedString>) -> Self {
        self.state(PlanState::Dropped(reason.into()))
    }

    /// What the reader is told this item is, in the caller's words.
    pub fn label(&self) -> &SharedString {
        &self.label
    }
}

/// A plan, as a run of marks and one line about the item in flight.
#[derive(IntoElement)]
pub struct AgentPlan {
    ident: Ident,
    items: Vec<PlanItem>,
    on_pick: Option<PickHandler>,
}

impl AgentPlan {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            items: Vec::new(),
            on_pick: None,
        }
    }

    pub fn item(mut self, item: PlanItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = PlanItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Reports the id of the item that was picked. Without this the marks are
    /// a report and install no handler at all.
    pub fn on_pick(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }

    /// How many items have finished, which is what the summary publishes.
    pub fn done(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.state.counts_as_done())
            .count()
    }

    /// The first item in flight, which is the one that gets the words.
    fn in_flight(&self) -> Option<&PlanItem> {
        self.items
            .iter()
            .find(|item| item.state == PlanState::Doing)
    }
}

impl std::fmt::Debug for AgentPlan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentPlan")
            .field("ident", &self.ident)
            .field("items", &self.items)
            .field("picks", &self.on_pick.is_some())
            .finish()
    }
}

impl RenderOnce for AgentPlan {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let total = self.items.len();
        let done = self.done();

        let marks: Vec<AnyElement> = self
            .items
            .iter()
            .map(|item| {
                let ident = self.ident.child(item.id.as_ref());
                let running = item.state == PlanState::Doing;
                let dot = StatusDot::new(item.state.tone())
                    // Work in flight breathes on the shared activity token
                    // rather than on a loop this component invented.
                    .when(running, |dot| {
                        dot.busy(ident.child("busy")).activity(Activity::Working)
                    });
                // The reason belongs to whatever stopped, so it rides with the
                // label instead of becoming a second line on the panel.
                let tip = match item.state.reason() {
                    Some(reason) => cx
                        .strings()
                        .format(StringKey::AgentPlanReason, &[&item.label, reason]),
                    None => item.label.clone(),
                };
                let picks = self.on_pick.clone();
                let id = item.id.clone();
                div()
                    .id(ident.element_id())
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(theme.control.sm.height / 2.0))
                    .radius(&theme, gpui_kit_theme::Radius::Pill)
                    // A dropped item is a fact to read, not one to hide, so it
                    // recedes rather than leaving a gap in the run.
                    .when(matches!(item.state, PlanState::Dropped(_)), |element| {
                        element.opacity(theme.opacity.disabled)
                    })
                    .child(dot)
                    .tip(ident.clone(), tip.clone())
                    .when_some(picks, |element, picks| {
                        element
                            .cursor_pointer()
                            .tab_index(0)
                            .pressable(cx)
                            .hover_row(&theme)
                            .focus_ring(&theme)
                            .on_click(move |_, window, cx| picks(id.clone(), window, cx))
                    })
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.semantic_id(), Role::Button)
                            .parent(self.ident.semantic_id())
                            .text(item.label.clone())
                            .value(item.state.as_str())
                            .selected(running),
                    )
                    .into_any_element()
            })
            .collect();

        // The one line of prose on the panel. It is keyed on the item, so
        // advancing the plan fades the new name in where the old one was
        // rather than swapping it between two frames.
        let saying = self.in_flight().map(|item| {
            motion::surface_in(
                self.ident
                    .child("saying")
                    .child(item.id.as_ref())
                    .element_id(),
                &theme,
                text(&theme, TypeScale::Body, item.label.clone()),
            )
        });

        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .child(
                div()
                    .row()
                    .flex_wrap()
                    .items_center()
                    .gap_token(&theme, Space::Xs)
                    .children(marks),
            )
            .children(saying)
            // How many are done is not written down. The filled marks are the
            // count, and a bare number beside them is the same fact twice —
            // in the one panel whose whole point is that it does not spend
            // words. It stays on the semantic node, where a reader who cannot
            // see the marks still gets it.
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::List)
                    .text(cx.strings().text(StringKey::AgentPlan))
                    .value(if total == 1 && done == 1 {
                        cx.strings().text(StringKey::AgentStepsDoneOne)
                    } else {
                        cx.strings().format(
                            StringKey::AgentStepsDoneMany,
                            &[cx.numbers().count(done).as_ref()],
                        )
                    }),
            )
    }
}
