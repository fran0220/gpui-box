//! A surface that renders one [`crate::state::Phase`] without inventing one.

use std::time::Duration;

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space};

use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::failure_panel::FailurePanel;
use crate::display::loading::{RefreshVeil, Skeleton};
use crate::display::status::StaleMark;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Ident, StyledExt};
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
    /// The clock stays with the host. Past [`LONG_WAIT`] the surface names
    /// that the wait is longer than usual; it does not invent a duration.
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
        let long_wait = self
            .elapsed
            .is_some_and(|elapsed| elapsed >= LONG_WAIT && phase.is_busy());

        let body = match phase {
            Phase::Loading => self.slots.or_else(slot::LOADING, window, cx, |_, cx| {
                Skeleton::new(ident.child("loading"))
                    .label(cx.strings().text(StringKey::Loading))
                    .into_any_element()
            }),
            Phase::Idle => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(
                    ident.child("empty"),
                    cx.strings().text(StringKey::StateViewIdle),
                )
                .kind(EmptyKind::Unstarted)
                .into_any_element()
            }),
            Phase::Queued => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(
                    ident.child("empty"),
                    cx.strings().text(StringKey::StateViewQueued),
                )
                .kind(EmptyKind::Unstarted)
                .into_any_element()
            }),
            Phase::Blocked => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(
                    ident.child("empty"),
                    cx.strings().text(StringKey::StateViewBlocked),
                )
                .kind(EmptyKind::Unstarted)
                .into_any_element()
            }),
            Phase::Empty => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(
                    ident.child("empty"),
                    cx.strings().text(StringKey::StateViewEmpty),
                )
                .kind(EmptyKind::Empty)
                .into_any_element()
            }),
            Phase::Cancelled => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                let mut empty = EmptyState::new(
                    ident.child("empty"),
                    cx.strings().text(StringKey::StateViewCancelled),
                )
                .kind(EmptyKind::Empty);
                if let Some(reason) = reason.clone() {
                    empty = empty.detail(reason);
                }
                empty.into_any_element()
            }),
            Phase::Unavailable => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                let mut empty = EmptyState::new(
                    ident.child("empty"),
                    cx.strings().text(StringKey::StateViewUnavailable),
                )
                .kind(EmptyKind::Unavailable);
                if let Some(reason) = reason.clone() {
                    empty = empty.detail(reason);
                }
                empty.into_any_element()
            }),
            Phase::Error if !self.stale => self.slots.or_else(slot::FAILED, window, cx, |_, _| {
                FailurePanel::new(ident.child("failed"), reason.clone().unwrap_or_default())
                    .into_any_element()
            }),
            Phase::Ready | Phase::Refreshing | Phase::Error => {
                let content = self.content.unwrap_or_else(|| {
                    EmptyState::new(
                        ident.child("empty"),
                        cx.strings().text(StringKey::StateViewIdle),
                    )
                    .kind(EmptyKind::Unstarted)
                    .into_any_element()
                });
                let veiled = if matches!(phase, Phase::Refreshing) {
                    RefreshVeil::new(ident.child("veil"), content)
                        .label(cx.strings().text(StringKey::StateViewRefreshing))
                        .into_any_element()
                } else {
                    content
                };
                if self.stale && phase != Phase::Refreshing {
                    div()
                        .column()
                        .gap_token(&theme, Space::Sm)
                        .child(veiled)
                        .child(StaleMark::new(
                            ident.child("stale"),
                            reason.unwrap_or_else(|| cx.strings().text(StringKey::StatusStale)),
                        ))
                        .into_any_element()
                } else {
                    veiled
                }
            }
        };

        div()
            .w_full()
            .child(body)
            .when(long_wait, |element| {
                element.child(
                    div()
                        .type_scale(&theme, gpui_kit_theme::TypeScale::Caption)
                        .text_color(theme.colors.text_muted)
                        .child(cx.strings().text(StringKey::StateViewStillWorking)),
                )
            })
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .value(phase.name())
                    .busy(phase.is_busy()),
            )
    }
}
