//! A composable attachment presentation. The host owns media decoding,
//! transfer execution, processing, retry policy and all action handlers.

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, prelude::FluentBuilder,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, TypeScale};

use crate::display::progress::ProgressBar;
use crate::foundation::slot::{Slots, Slotted};
use crate::foundation::{Disableable, Ident, StyledExt, text};
use crate::strings::{ActiveStrings, StringKey};

/// Facts reported by the caller. Finishing a transfer does not imply that
/// processing finished; only `Ready` makes that claim. Unknown totals stay
/// indeterminate, including a supplied zero total.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AttachmentState {
    #[default]
    Ready,
    Queued,
    Transferring {
        completed: usize,
        total: Option<usize>,
    },
    Paused {
        completed: usize,
        total: Option<usize>,
    },
    Processing,
    Unavailable(SharedString),
    Failed(SharedString),
    Cancelled,
}

impl AttachmentState {
    pub fn is_busy(&self) -> bool {
        matches!(self, Self::Transferring { .. } | Self::Processing)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Queued => "queued",
            Self::Transferring { .. } => "transferring",
            Self::Paused { .. } => "paused",
            Self::Processing => "processing",
            Self::Unavailable(_) => "unavailable",
            Self::Failed(_) => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn label(&self, cx: &App) -> SharedString {
        let key = match self {
            Self::Ready => StringKey::AttachmentReady,
            Self::Queued => StringKey::AttachmentQueued,
            Self::Transferring { .. } => StringKey::AttachmentTransferring,
            Self::Paused { .. } => StringKey::AttachmentPaused,
            Self::Processing => StringKey::AttachmentProcessing,
            Self::Unavailable(reason) | Self::Failed(reason) => return reason.clone(),
            Self::Cancelled => StringKey::AttachmentCancelled,
        };
        cx.strings().text(key)
    }
}

/// A standalone attachment tile, separate from message-list attachment
/// metadata. All four slots compose native elements: `media`, `title`,
/// `description`, and `actions`. A supplied preview remains visible on failure
/// because already-known media is not erased by a failed transfer or refresh.
#[derive(Debug, IntoElement)]
pub struct AttachmentTile {
    ident: Ident,
    title: SharedString,
    description: Option<SharedString>,
    state: AttachmentState,
    disabled: bool,
    slots: Slots,
}

impl AttachmentTile {
    pub fn new(ident: impl Into<Ident>, title: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            title: title.into(),
            description: None,
            state: AttachmentState::Ready,
            disabled: false,
            slots: Slots::default(),
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn state(mut self, state: AttachmentState) -> Self {
        self.state = state;
        self
    }
}

impl Disableable for AttachmentTile {
    /// Disabled tiles omit the action slot entirely, so it cannot install
    /// handlers. Media/title/description slots are presentation-only.
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Slotted for AttachmentTile {
    const SLOTS: &'static [&'static str] = &["media", "title", "description", "actions"];
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for AttachmentTile {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let title = self.slots.or_else("title", window, cx, |_, _| {
            text(&theme, TypeScale::Label, self.title.clone()).into_any_element()
        });
        let description = self.slots.render("description", window, cx).or_else(|| {
            self.description
                .clone()
                .map(|description| text(&theme, TypeScale::Caption, description).into_any_element())
        });
        let media = self.slots.render("media", window, cx);
        let actions = (!self.disabled)
            .then(|| self.slots.render("actions", window, cx))
            .flatten();
        let progress = match self.state {
            AttachmentState::Transferring { completed, total }
            | AttachmentState::Paused { completed, total } => Some(
                ProgressBar::new(self.ident.child("progress"))
                    .when_some(total.filter(|total| *total > 0), |bar, total| {
                        bar.count(completed, total)
                    })
                    .paused(matches!(self.state, AttachmentState::Paused { .. })),
            ),
            AttachmentState::Processing => Some(ProgressBar::new(self.ident.child("progress"))),
            _ => None,
        };
        let status = self.state.label(cx);
        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .p_token(&theme, Space::Md)
            .surface(&theme, Surface::Raised)
            .radius(&theme, Radius::Card)
            .children(media.map(|media| {
                div().w_full().child(media).semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("media").semantic_id(), Role::Group)
                        .parent(self.ident.semantic_id()),
                )
            }))
            .child(
                div().child(title).semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("title").semantic_id(), Role::Text)
                        .parent(self.ident.semantic_id())
                        .text(self.title.clone()),
                ),
            )
            .children(description.map(|description| {
                div().child(description).semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("description").semantic_id(), Role::Group)
                        .parent(self.ident.semantic_id()),
                )
            }))
            .child(
                text(&theme, TypeScale::Caption, status.clone()).semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("status").semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .text(status)
                        .value(self.state.name())
                        .busy(self.state.is_busy()),
                ),
            )
            .children(progress)
            .children(actions)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .text(self.title)
                    .value(self.state.name())
                    .busy(self.state.is_busy())
                    .disabled(self.disabled),
            )
    }
}
