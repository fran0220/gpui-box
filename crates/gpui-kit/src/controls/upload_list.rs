//! Files on their way somewhere, over the [`Dropzone`] that took them.
//!
//! Nothing here uploads anything: this crate has no network, the same reason
//! `ImageViewer` fetches nothing and `TransportBar` plays nothing. Every state
//! on the list is a fact the host established, and every control reports.
//!
//! # A refusal is not a failure
//!
//! A file the host would not take — too large, the wrong kind, past a quota —
//! never started, so it did not fail. [`UploadState::Refused`] is its own
//! state, carrying the host's reason and offering **no retry**, because trying
//! the same file against the same rule again cannot end differently. That is
//! [`Dropzone`]'s own distinction between refusing and idle, carried one step
//! further: the zone refuses a payload while it is over the zone, and this
//! list keeps the refusal afterwards where the file's row is.
//!
//! [`UploadState::Failed`] is the other thing entirely: it started, it did not
//! finish, the host's reason says why, and a retry is offered.
//!
//! # Overall progress is only claimed when it is known
//!
//! The list adds up the per-file fractions, and stops if any file that is
//! uploading does not have one. A file uploading against a length nobody
//! declared has no fraction, so the total has no extent either, and the bar
//! goes indeterminate exactly as [`ProgressBar`] already does rather than
//! inventing a percentage out of a file count.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, TypeScale};

use crate::controls::button::{Button, IconButton};
use crate::controls::dropzone::Dropzone;
use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::progress::ProgressBar;
use crate::display::status::StatusDot;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Disableable, Ident, Sizable, StyledExt, text as foundation_text};
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

type FileHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// Where one file has got to.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum UploadState {
    /// Accepted and waiting its turn. Nothing has been sent.
    #[default]
    Queued,
    /// On its way. `fraction` is `None` when nobody knows how much there is
    /// to send, which is a state and not a zero.
    Uploading {
        fraction: Option<f32>,
    },
    Done,
    /// It started and did not finish, in the host's own words.
    Failed {
        reason: SharedString,
    },
    /// Somebody stopped it. Distinct from failed: nothing went wrong.
    Cancelled,
    /// The host would not take it at all, in its own words.
    Refused {
        reason: SharedString,
    },
}

impl UploadState {
    /// The name the semantic tree publishes, so a test tells the six apart
    /// without reading a colour.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Uploading { .. } => "uploading",
            Self::Done => "done",
            Self::Failed { .. } => "failed",
            Self::Cancelled => "cancelled",
            Self::Refused { .. } => "refused",
        }
    }

    /// Whether anything more is going to happen to this file on its own.
    pub fn is_settled(&self) -> bool {
        matches!(
            self,
            Self::Done | Self::Failed { .. } | Self::Cancelled | Self::Refused { .. }
        )
    }
}

impl HasPhase for UploadState {
    fn phase(&self) -> Phase {
        match self {
            Self::Queued => Phase::Queued,
            Self::Uploading { .. } => Phase::Loading,
            Self::Done => Phase::Ready,
            Self::Failed { .. } => Phase::Error,
            Self::Cancelled => Phase::Cancelled,
            Self::Refused { .. } => Phase::Unavailable,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Failed { reason } | Self::Refused { reason } => Some(reason.as_ref()),
            _ => None,
        }
    }
}

impl UploadState {
    /// Whether trying again could end differently. A refusal could not.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Failed { .. } | Self::Cancelled)
    }

    fn tone(&self) -> Tone {
        match self {
            Self::Queued => Tone::Neutral,
            Self::Uploading { .. } => Tone::Accent,
            Self::Done => Tone::Success,
            Self::Failed { .. } => Tone::Danger,
            Self::Cancelled => Tone::Neutral,
            // A refusal is the host declining, not something breaking, and it
            // reads at the same weight `Dropzone` gives one.
            Self::Refused { .. } => Tone::Warning,
        }
    }

    /// The words beside the file's name.
    fn wording(&self, cx: &App) -> SharedString {
        match self {
            // The host's own reason outranks the catalogue's word for it.
            Self::Failed { reason } | Self::Refused { reason } => reason.clone(),
            Self::Queued => cx.strings().text(StringKey::UploadQueued),
            Self::Uploading { .. } => cx.strings().text(StringKey::UploadUploading),
            Self::Done => cx.strings().text(StringKey::UploadDone),
            Self::Cancelled => cx.strings().text(StringKey::UploadCancelled),
        }
    }

    /// How much of this file is done, when that is known at all.
    fn fraction(&self) -> Option<f32> {
        match self {
            Self::Uploading { fraction } => *fraction,
            Self::Done => Some(1.0),
            // Nothing has been sent, or nothing more will be.
            Self::Queued | Self::Cancelled | Self::Refused { .. } => Some(0.0),
            Self::Failed { .. } => Some(0.0),
        }
    }
}

/// One file on the list, identified by business identity.
#[derive(Debug, Clone, PartialEq)]
pub struct Upload {
    pub id: SharedString,
    pub name: SharedString,
    /// The size, already worded by the host. This crate formats no quantities.
    pub size: Option<SharedString>,
    pub state: UploadState,
}

impl Upload {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            size: None,
            state: UploadState::default(),
        }
    }

    /// The size as the host already worded it.
    pub fn size(mut self, size: impl Into<SharedString>) -> Self {
        self.size = Some(size.into());
        self
    }

    pub fn state(mut self, state: UploadState) -> Self {
        self.state = state;
        self
    }

    pub fn uploading(self, fraction: impl Into<Option<f32>>) -> Self {
        self.state(UploadState::Uploading {
            fraction: fraction.into(),
        })
    }

    pub fn done(self) -> Self {
        self.state(UploadState::Done)
    }

    pub fn failed(self, reason: impl Into<SharedString>) -> Self {
        self.state(UploadState::Failed {
            reason: reason.into(),
        })
    }

    pub fn cancelled(self) -> Self {
        self.state(UploadState::Cancelled)
    }

    pub fn refused(self, reason: impl Into<SharedString>) -> Self {
        self.state(UploadState::Refused {
            reason: reason.into(),
        })
    }
}

/// How much of the whole batch is done, if anybody knows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverallProgress {
    /// Every file in flight declared an extent, so the total has one.
    Known(f32),
    /// At least one file in flight has no extent, so neither has the total.
    Indeterminate,
    /// Nothing is in flight.
    Settled,
}

/// A list of files being uploaded, optionally over the zone that took them.
#[derive(IntoElement)]
pub struct UploadList {
    ident: Ident,
    uploads: Vec<Upload>,
    zone: Option<Dropzone>,
    size: ControlSize,
    disabled: bool,
    show_overall: bool,
    on_retry: Option<FileHandler>,
    on_cancel: Option<FileHandler>,
    on_remove: Option<FileHandler>,
    slots: Slots,
}

impl std::fmt::Debug for UploadList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UploadList")
            .field("ident", &self.ident)
            .field("uploads", &self.uploads.len())
            .field("has_zone", &self.zone.is_some())
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl UploadList {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            uploads: Vec::new(),
            zone: None,
            size: ControlSize::Sm,
            disabled: false,
            show_overall: true,
            on_retry: None,
            on_cancel: None,
            on_remove: None,
            slots: Slots::default(),
        }
    }

    pub fn upload(mut self, upload: Upload) -> Self {
        self.uploads.push(upload);
        self
    }

    pub fn uploads(mut self, uploads: impl IntoIterator<Item = Upload>) -> Self {
        self.uploads.extend(uploads);
        self
    }

    /// The zone the files arrive through, drawn above the list.
    ///
    /// Sharing the surface is the point: a payload the zone refuses while it
    /// is being dragged and a file the host refused after it landed are the
    /// same refusal, said in the same place.
    pub fn dropzone(mut self, zone: Dropzone) -> Self {
        self.zone = Some(zone);
        self
    }

    /// Whether the batch progress bar is drawn at all.
    pub fn show_overall(mut self, show: bool) -> Self {
        self.show_overall = show;
        self
    }

    /// Reports a file that should be tried again. The list retries nothing.
    ///
    /// A refused file never gets this control, whatever the handler says.
    pub fn on_retry(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_retry = Some(Rc::new(handler));
        self
    }

    /// Reports a file that should be stopped. Offered only while one is still
    /// on its way or waiting to be.
    pub fn on_cancel(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }

    /// Reports a file that should leave the list. Offered only once nothing
    /// more is going to happen to it.
    pub fn on_remove(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }

    /// How much of the batch is done.
    ///
    /// Known only when every file still in flight declared an extent. A single
    /// file uploading against an unknown length takes the whole batch to
    /// indeterminate, because a total assembled from a number nobody has is
    /// not a total.
    pub fn overall(&self) -> OverallProgress {
        let counted: Vec<&Upload> = self
            .uploads
            .iter()
            // A refused file was never part of the work, so it is not part of
            // the denominator either: a batch of nine that refused one is nine
            // eighths done otherwise.
            .filter(|upload| !matches!(upload.state, UploadState::Refused { .. }))
            .collect();
        if counted.is_empty() {
            return OverallProgress::Settled;
        }
        if counted.iter().all(|upload| upload.state.is_settled()) {
            return OverallProgress::Settled;
        }
        let mut total = 0.0f32;
        for upload in &counted {
            let Some(fraction) = upload.state.fraction() else {
                return OverallProgress::Indeterminate;
            };
            total += fraction;
        }
        OverallProgress::Known((total / counted.len() as f32).clamp(0.0, 1.0))
    }

    fn row(&self, upload: &Upload, cx: &mut App) -> AnyElement {
        let theme = cx.theme().clone();
        let ident = self.ident.child(upload.id.as_ref());
        let live = !self.disabled;
        let wording = upload.state.wording(cx);

        // A refusal never gets a retry: the same file against the same rule
        // cannot end differently, and a control that could not work is worse
        // than none.
        let retry = self
            .on_retry
            .clone()
            .filter(|_| live && upload.state.is_retryable())
            .map(|handler| {
                let id = upload.id.clone();
                Button::new(ident.child("retry"))
                    .label(cx.strings().text(StringKey::TryAgain))
                    .ghost()
                    .control_size(ControlSize::Xs)
                    .semantic_parent(ident.semantic_id())
                    .on_click(move |window, cx| handler(id.clone(), window, cx))
            });

        let cancel = self
            .on_cancel
            .clone()
            .filter(|_| live && !upload.state.is_settled())
            .map(|handler| {
                let id = upload.id.clone();
                IconButton::new(
                    ident.child("cancel"),
                    Icon::Stop,
                    cx.strings()
                        .format(StringKey::UploadCancel, &[upload.name.as_ref()]),
                )
                .control_size(ControlSize::Xs)
                .semantic_parent(ident.semantic_id())
                .on_click(move |window, cx| handler(id.clone(), window, cx))
            });

        let remove = self
            .on_remove
            .clone()
            .filter(|_| live && upload.state.is_settled())
            .map(|handler| {
                let id = upload.id.clone();
                IconButton::new(
                    ident.child("remove"),
                    Icon::Close,
                    cx.strings()
                        .format(StringKey::UploadRemove, &[upload.name.as_ref()]),
                )
                .control_size(ControlSize::Xs)
                .semantic_parent(ident.semantic_id())
                .on_click(move |window, cx| handler(id.clone(), window, cx))
            });

        let bar = matches!(upload.state, UploadState::Uploading { .. }).then(|| {
            let mut bar = ProgressBar::new(ident.child("progress"));
            if let UploadState::Uploading {
                fraction: Some(fraction),
            } = upload.state
            {
                bar = bar.fraction(fraction);
            }
            bar
        });

        div()
            .row()
            .items_start()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .px_token(&theme, Space::Sm)
            .py_token(&theme, Space::Xs)
            .child(div().flex_none().mt(px(5.0)).child({
                let dot = StatusDot::new(upload.state.tone());
                // Only a file actually on its way moves. A queued one is
                // waiting for a turn, which is not the same as working.
                match upload.state {
                    UploadState::Uploading { .. } => dot.busy(ident.child("mark")),
                    _ => dot,
                }
            }))
            .child(
                div()
                    .column()
                    .flex_1()
                    .min_w_0()
                    .gap_token(&theme, Space::Xs)
                    .child(
                        div()
                            .row()
                            .w_full()
                            .gap_token(&theme, Space::Sm)
                            .child(
                                foundation_text(&theme, TypeScale::Label, upload.name.clone())
                                    .flex_1()
                                    .min_w_0(),
                            )
                            .children(upload.size.clone().map(|size| {
                                foundation_text(&theme, TypeScale::Caption, size)
                                    .flex_none()
                                    .text_tone(&theme, gpui_kit_theme::TextTone::Faint)
                            })),
                    )
                    .child(match upload.state {
                        UploadState::Failed { .. } => {
                            foundation_text(&theme, TypeScale::Caption, wording.clone())
                                .text_color(theme.colors.danger)
                        }
                        UploadState::Refused { .. } => {
                            foundation_text(&theme, TypeScale::Caption, wording.clone())
                                .text_color(theme.colors.warning)
                        }
                        _ => foundation_text(&theme, TypeScale::Caption, wording.clone())
                            .text_tone(&theme, gpui_kit_theme::TextTone::Muted),
                    })
                    .children(bar),
            )
            .children(retry)
            .children(cancel)
            .children(remove)
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Row)
                    .parent(self.ident.semantic_id())
                    .text(upload.name.clone())
                    // The state is published by name, so a refusal and a
                    // failure cannot be mistaken for one another.
                    .value(upload.state.name())
                    .busy(matches!(upload.state, UploadState::Uploading { .. }))
                    .invalid(matches!(upload.state, UploadState::Failed { .. }))
                    // A refusal is the host declining, not the row breaking,
                    // so it is published as disabled rather than invalid.
                    .disabled(matches!(upload.state, UploadState::Refused { .. })),
            )
            .into_any_element()
    }
}

impl Disableable for UploadList {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for UploadList {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for UploadList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let overall = self.overall();
        let overall_ident = self.ident.child("overall");

        let progress = (self.show_overall && overall != OverallProgress::Settled).then(|| {
            let mut bar = ProgressBar::new(overall_ident.clone())
                .label(cx.strings().text(StringKey::UploadOverall));
            if let OverallProgress::Known(fraction) = overall {
                bar = bar.fraction(fraction);
            }
            bar
        });

        let rows: Vec<AnyElement> = self
            .uploads
            .iter()
            .map(|upload| self.row(upload, cx))
            .collect();

        let body = if rows.is_empty() {
            self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(
                    self.ident.child("empty"),
                    cx.strings().text(StringKey::UploadEmpty),
                )
                .kind(EmptyKind::Unstarted)
                .into_any_element()
            })
        } else {
            div().column().w_full().children(rows).into_any_element()
        };

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .radius(&theme, Radius::Card)
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .children(self.zone)
            .children(progress)
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::List)
                    .disabled(self.disabled)
                    .value(self.uploads.len().to_string()),
            )
    }
}

impl Slotted for UploadList {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(uploads: impl IntoIterator<Item = Upload>) -> UploadList {
        UploadList::new("attachments").uploads(uploads)
    }

    #[test]
    fn a_refusal_is_not_a_failure_and_offers_no_retry() {
        let refused = UploadState::Refused {
            reason: "larger than 25 MB".into(),
        };
        let failed = UploadState::Failed {
            reason: "the connection dropped".into(),
        };
        assert_ne!(refused.name(), failed.name());
        assert!(!refused.is_retryable());
        assert!(failed.is_retryable());
        assert!(refused.is_settled() && failed.is_settled());
    }

    #[test]
    fn an_unknown_extent_takes_the_whole_batch_indeterminate() {
        let known = list([
            Upload::new("a", "a.bin").uploading(0.5),
            Upload::new("b", "b.bin").done(),
        ]);
        assert_eq!(known.overall(), OverallProgress::Known(0.75));

        let unknown = list([
            Upload::new("a", "a.bin").uploading(None),
            Upload::new("b", "b.bin").done(),
        ]);
        assert_eq!(unknown.overall(), OverallProgress::Indeterminate);
    }

    #[test]
    fn a_batch_with_nothing_in_flight_claims_no_progress_at_all() {
        let settled = list([
            Upload::new("a", "a.bin").done(),
            Upload::new("b", "b.bin").failed("the connection dropped"),
        ]);
        assert_eq!(settled.overall(), OverallProgress::Settled);
        assert_eq!(list([]).overall(), OverallProgress::Settled);
    }

    #[test]
    fn a_refused_file_is_not_part_of_the_work_being_measured() {
        let batch = list([
            Upload::new("a", "a.bin").uploading(0.5),
            Upload::new("b", "b.exe").refused("this zone does not take programs"),
        ]);
        // Half of the one file that is actually being sent, not a quarter of
        // two files one of which was never taken.
        assert_eq!(batch.overall(), OverallProgress::Known(0.5));
    }
}

#[cfg(test)]
mod upload_phase_tests {
    use super::*;

    #[test]
    fn queued_cancelled_and_refused_are_three_phases() {
        assert_eq!(UploadState::Queued.phase(), Phase::Queued);
        assert_eq!(UploadState::Cancelled.phase(), Phase::Cancelled);
        let refused = UploadState::Refused {
            reason: "too large".into(),
        };
        assert_eq!(refused.phase(), Phase::Unavailable);
        assert_eq!(refused.reason(), Some("too large"));
        assert_eq!(
            UploadState::Uploading { fraction: None }.phase(),
            Phase::Loading
        );
    }
}
