//! A surface that renders one [`crate::state::Phase`] without inventing one.

use std::time::Duration;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space};

use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::failure_panel::FailurePanel;
use crate::display::loading::{RefreshVeil, Skeleton};
use crate::display::status::{StaleMark, StatusDot};
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Ident, StyledExt};
use crate::overlay::tooltip::Tooltipped;
use crate::state::{AsyncValue, HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

/// How long a host-supplied wait has to run before the surface says so.
const LONG_WAIT: Duration = Duration::from_secs(8);

/// A region that picks the truthful waiting, empty, failed, or ready surface.
#[derive(IntoElement)]
pub struct StateView {
    ident: Ident,
    phase: Phase,
    reason: Option<SharedString>,
    stale: bool,
    content: Option<AnyElement>,
    elapsed: Option<Duration>,
    slots: Slots,
}

impl std::fmt::Debug for StateView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StateView")
            .field("ident", &self.ident)
            .field("phase", &self.phase)
            .field("stale", &self.stale)
            .finish()
    }
}

impl StateView {
    pub fn new(ident: impl Into<Ident>, phase: impl HasPhase) -> Self {
        Self {
            ident: ident.into(),
            phase: phase.phase(),
            reason: phase.reason().map(SharedString::from),
            stale: phase.is_stale(),
            content: None,
            elapsed: None,
            slots: Slots::default(),
        }
    }

    /// The last verified content, for Ready, Refreshing, and a stale Error.
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    /// How long the host says this wait has already run.
    ///
    /// The clock stays with the host. Past eight seconds the semantic
    /// description reports that the wait is longer than usual; it does not
    /// invent a duration.
    pub fn elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = Some(elapsed);
        self
    }

    pub fn from_async<T, E: AsRef<str>>(
        ident: impl Into<Ident>,
        value: &AsyncValue<T, E>,
        ready: impl FnOnce(&T) -> AnyElement,
    ) -> Self {
        let content = value.value.as_ref().map(ready);
        let mut view = Self::new(ident, value);
        if let Some(content) = content {
            view = view.content(content);
        }
        view
    }
}

impl Slotted for StateView {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for StateView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();
        let phase = self.phase;
        let reason = self.reason.clone();
        let semantic_reason = reason.clone();
        let long_wait = self
            .elapsed
            .is_some_and(|elapsed| elapsed >= LONG_WAIT && phase.is_busy());

        let body = match phase {
            Phase::Loading => self.slots.or_else(slot::LOADING, window, cx, |_, cx| {
                let loading_ident = ident.child("loading");
                let label = cx.strings().text(StringKey::Loading);
                div()
                    .id(loading_ident.element_id())
                    .child(Skeleton::new(loading_ident.child("skeleton")).label(label.clone()))
                    .tip(loading_ident, label)
                    .into_any_element()
            }),
            Phase::Idle => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                marked_empty(
                    ident.child("empty"),
                    cx.strings().text(StringKey::StateViewIdle),
                    EmptyKind::Unstarted,
                    None,
                )
            }),
            Phase::Queued => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                marked_empty(
                    ident.child("empty"),
                    cx.strings().text(StringKey::StateViewQueued),
                    EmptyKind::Queued,
                    None,
                )
            }),
            Phase::Blocked => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                marked_empty(
                    ident.child("empty"),
                    cx.strings().text(StringKey::StateViewBlocked),
                    EmptyKind::Blocked,
                    None,
                )
            }),
            Phase::Empty => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                marked_empty(
                    ident.child("empty"),
                    cx.strings().text(StringKey::StateViewEmpty),
                    EmptyKind::Empty,
                    None,
                )
            }),
            Phase::Cancelled => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                marked_empty(
                    ident.child("empty"),
                    cx.strings().text(StringKey::StateViewCancelled),
                    EmptyKind::Cancelled,
                    reason.clone(),
                )
            }),
            Phase::Unavailable => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                marked_empty(
                    ident.child("empty"),
                    cx.strings().text(StringKey::StateViewUnavailable),
                    EmptyKind::Unavailable,
                    reason.clone(),
                )
            }),
            Phase::Error if !self.stale => self.slots.or_else(slot::FAILED, window, cx, |_, _| {
                FailurePanel::new(ident.child("failed"), reason.clone().unwrap_or_default())
                    .into_any_element()
            }),
            Phase::Ready | Phase::Refreshing | Phase::Error => {
                let content = self.content.unwrap_or_else(|| {
                    marked_empty(
                        ident.child("empty"),
                        cx.strings().text(StringKey::StateViewIdle),
                        EmptyKind::Unstarted,
                        None,
                    )
                });
                let veiled = if matches!(phase, Phase::Refreshing) {
                    let veil_ident = ident.child("veil");
                    let mark_ident = veil_ident.child("mark");
                    let label = cx.strings().text(StringKey::StateViewRefreshing);
                    div()
                        .id(mark_ident.element_id())
                        .child(RefreshVeil::new(veil_ident, content))
                        .tip(mark_ident, label)
                        .into_any_element()
                } else {
                    content
                };
                if self.stale && phase != Phase::Refreshing {
                    let stale = match reason {
                        Some(reason) => {
                            StaleMark::new(ident.child("stale"), reason).into_any_element()
                        }
                        None => {
                            let stale_ident = ident.child("stale");
                            let label = cx.strings().text(StringKey::StatusStale);
                            div()
                                .id(stale_ident.element_id())
                                .row()
                                .child(StatusDot::new(Tone::Warning))
                                .tip(stale_ident.clone(), label.clone())
                                .semantic_in(
                                    cx,
                                    NodeSpec::new(stale_ident.semantic_id(), Role::Status)
                                        .text(label)
                                        .value("stale"),
                                )
                                .into_any_element()
                        }
                    };
                    div()
                        .column()
                        .gap_token(&theme, Space::Sm)
                        .child(veiled)
                        .child(stale)
                        .into_any_element()
                } else {
                    veiled
                }
            }
        };

        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Region)
            .value(phase.name())
            .busy(phase.is_busy());
        if let Some(reason) = semantic_reason {
            spec = spec.description(reason);
        } else if long_wait {
            spec = spec.description(cx.strings().text(StringKey::StateViewStillWorking));
        }

        div().w_full().child(body).semantic_in(cx, spec)
    }
}

fn marked_empty(
    ident: Ident,
    label: SharedString,
    kind: EmptyKind,
    detail: Option<SharedString>,
) -> AnyElement {
    let mut empty = EmptyState::new(ident.clone(), SharedString::default()).kind(kind);
    if let Some(detail) = detail {
        empty = empty.detail(detail);
    }
    let mark_ident = ident.child("mark");
    div()
        .id(mark_ident.element_id())
        .child(empty)
        .tip(mark_ident, label)
        .into_any_element()
}
