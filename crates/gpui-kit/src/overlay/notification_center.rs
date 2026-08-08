//! Where a notification goes after the toast that showed it has gone.
//!
//! A [`Toast`] is transient by design: it reports what happened and then gets
//! out of the way. That is only acceptable if the report survives it, which is
//! what this is for.
//!
//! The two are one record rather than two implementations. A host calls
//! [`NotificationCenter::show`], which files the [`Notification`] here **and**
//! pushes the toast built from that same record, so a toast that timed out has
//! not been lost — it is in the centre, unread, with the same id, the same
//! wording, and the same severity. Nothing about toasts is reimplemented here:
//! timing, eviction, hover, and the rule that a failure never times out all
//! stay in [`crate::overlay::toast`].
//!
//! The unread count is allowed to say it does not know. The centre holds a
//! bounded number of records, and once a record has been dropped to make room
//! the centre can no longer count what it no longer holds — so the count
//! becomes [`UnreadCount::AtLeast`] and reads `9+` rather than a number that
//! would be wrong.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, TypeScale};

use crate::controls::button::{Button, IconButton};
use crate::display::badge::{Badge, Tone};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::status::StatusDot;
use crate::foundation::{Ident, Sizable, StyledExt};
use crate::overlay::toast::{self, Toast};
use crate::strings::{ActiveStrings, StringKey};

/// How many records the centre holds before the oldest is dropped.
const DEFAULT_CAPACITY: usize = 50;

type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One thing that happened, whether or not a toast ever showed it.
///
/// The severity is the library's existing [`Tone`] vocabulary rather than a
/// second severity scale, so a notification reads the same in the centre as
/// the toast of it read on screen.
pub struct Notification {
    ident: Ident,
    message: SharedString,
    detail: Option<SharedString>,
    tone: Tone,
    /// When it happened, as a string the host already put into words. The
    /// reasoning is `Timeline`'s: this crate owns no clock and no locale.
    at: Option<SharedString>,
    read: bool,
    action: Option<(SharedString, ActionHandler)>,
}

impl std::fmt::Debug for Notification {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Notification")
            .field("ident", &self.ident)
            .field("message", &self.message)
            .field("tone", &self.tone)
            .field("read", &self.read)
            .field("has_action", &self.action.is_some())
            .finish()
    }
}

impl Notification {
    pub fn new(ident: impl Into<Ident>, message: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            message: message.into(),
            detail: None,
            tone: Tone::default(),
            at: None,
            read: false,
            action: None,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// When it happened, already worded by the host.
    pub fn at(mut self, at: impl Into<SharedString>) -> Self {
        self.at = Some(at.into());
        self
    }

    /// Files it as already read, for a report the typist has plainly seen.
    pub fn read(mut self, read: bool) -> Self {
        self.read = read;
        self
    }

    /// The one thing that can be done about it, as on a toast.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(handler)));
        self
    }

    pub fn id(&self) -> SharedString {
        self.ident.semantic_id()
    }

    pub fn is_read(&self) -> bool {
        self.read
    }

    pub fn message(&self) -> &SharedString {
        &self.message
    }

    /// The transient showing of this same record.
    ///
    /// One record, two surfaces: the toast carries the same id, so dismissing
    /// the toast and finding the notification afterwards are the same thing
    /// seen twice rather than two things that happen to look alike.
    pub fn toast(&self) -> Toast {
        let mut toast = Toast::new(self.ident.clone(), self.message.clone()).tone(self.tone);
        if let Some(detail) = self.detail.clone() {
            toast = toast.detail(detail);
        }
        if let Some((label, handler)) = &self.action {
            let handler = Rc::clone(handler);
            toast = toast.action(label.clone(), move |window, cx| handler(window, cx));
        }
        toast
    }
}

/// How many unread notifications there are, or how many there are at least.
///
/// A badge that shows a number it cannot stand behind is worse than one that
/// says "more than this", so the second variant exists and is rendered
/// differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnreadCount {
    Exact(usize),
    /// The centre has dropped records it no longer holds, so it can only speak
    /// for the ones it still has.
    AtLeast(usize),
}

impl UnreadCount {
    pub fn value(self) -> usize {
        match self {
            Self::Exact(count) => count,
            Self::AtLeast(count) => count,
        }
    }

    pub fn is_zero(self) -> bool {
        self.value() == 0
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Exact(_) => "exact",
            Self::AtLeast(_) => "at-least",
        }
    }

    fn wording(self, cx: &App) -> SharedString {
        match self {
            Self::Exact(count) => SharedString::from(count.to_string()),
            Self::AtLeast(count) => cx
                .strings()
                .format(StringKey::NotificationsAtLeast, &[&count.to_string()]),
        }
    }
}

/// What the centre reports. It files records; it decides nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationCenterEvent {
    /// One notification was marked read, by being opened or by the control.
    Read(SharedString),
    /// One notification was taken out of the centre.
    Dismissed(SharedString),
    /// Every notification was taken out.
    Cleared,
    /// A notification's own action was taken. The handler has already run.
    ActionTaken(SharedString),
}

impl EventEmitter<NotificationCenterEvent> for NotificationCenter {}

/// The list of notifications a window has accumulated.
pub struct NotificationCenter {
    ident: Ident,
    focus_handle: FocusHandle,
    notifications: Vec<Notification>,
    capacity: usize,
    /// Whether anything has been dropped to make room. Once this is true the
    /// centre stops claiming an exact unread count, because it cannot know
    /// whether what it dropped had been read.
    dropped: bool,
    size: ControlSize,
}

impl std::fmt::Debug for NotificationCenter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NotificationCenter")
            .field("ident", &self.ident)
            .field("notifications", &self.notifications.len())
            .field("capacity", &self.capacity)
            .field("dropped", &self.dropped)
            .finish()
    }
}

impl NotificationCenter {
    pub fn new(ident: impl Into<Ident>, cx: &mut Context<Self>) -> Self {
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            notifications: Vec::new(),
            capacity: DEFAULT_CAPACITY,
            dropped: false,
            size: ControlSize::Sm,
        }
    }

    /// How many records the centre holds before it drops the oldest. A
    /// capacity below one is raised to one.
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity.max(1);
        self
    }

    /// Files a notification and shows a toast of it, reporting whether the
    /// toast was delivered.
    ///
    /// False means no [`ToastLayer`](crate::overlay::ToastLayer) is mounted,
    /// so nothing floated on screen — but the record is filed either way,
    /// which is exactly the point of the centre.
    pub fn show(&mut self, notification: Notification, cx: &mut Context<Self>) -> bool {
        let toast = notification.toast();
        self.record(notification, cx);
        toast::push(cx, toast)
    }

    /// Files a notification without showing a toast of it.
    ///
    /// A repeat of an id already held replaces it, the way a repeated toast
    /// refreshes rather than stacking: one identity is one notification.
    pub fn record(&mut self, notification: Notification, cx: &mut Context<Self>) {
        let id = notification.id();
        if let Some(existing) = self.notifications.iter_mut().find(|held| held.id() == id) {
            *existing = notification;
            cx.notify();
            return;
        }
        self.notifications.push(notification);
        while self.notifications.len() > self.capacity {
            self.notifications.remove(0);
            self.dropped = true;
        }
        cx.notify();
    }

    pub fn len(&self) -> usize {
        self.notifications.len()
    }

    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }

    /// Whether the centre still holds this notification.
    pub fn holds(&self, id: &str) -> bool {
        self.notifications.iter().any(|held| held.id() == id)
    }

    /// Whether one notification has been read, or `None` when it is not here.
    pub fn is_read(&self, id: &str) -> Option<bool> {
        self.notifications
            .iter()
            .find(|held| held.id() == id)
            .map(Notification::is_read)
    }

    pub fn unread(&self) -> UnreadCount {
        let unread = self.notifications.iter().filter(|held| !held.read).count();
        if self.dropped {
            UnreadCount::AtLeast(unread)
        } else {
            UnreadCount::Exact(unread)
        }
    }

    pub fn mark_read(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        let Some(held) = self.notifications.iter_mut().find(|held| held.id() == id) else {
            return false;
        };
        if held.read {
            return false;
        }
        held.read = true;
        cx.emit(NotificationCenterEvent::Read(SharedString::from(
            id.to_string(),
        )));
        cx.notify();
        true
    }

    pub fn mark_all_read(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<SharedString> = self
            .notifications
            .iter_mut()
            .filter(|held| !held.read)
            .map(|held| {
                held.read = true;
                held.id()
            })
            .collect();
        for id in ids {
            cx.emit(NotificationCenterEvent::Read(id));
        }
        cx.notify();
    }

    /// Takes one notification out, reporting whether it was here.
    pub fn dismiss(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.notifications.iter().position(|held| held.id() == id) else {
            return false;
        };
        self.notifications.remove(index);
        cx.emit(NotificationCenterEvent::Dismissed(SharedString::from(
            id.to_string(),
        )));
        cx.notify();
        true
    }

    /// Takes every notification out.
    ///
    /// Dismissing one and clearing them all are separate reports, because
    /// clearing is the gesture that loses reports nobody has read and a host
    /// may well want to ask about it.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.notifications.clear();
        // Nothing is held, so nothing was dropped from view either: an empty
        // centre counts zero exactly.
        self.dropped = false;
        cx.emit(NotificationCenterEvent::Cleared);
        cx.notify();
    }

    fn row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let notification = &self.notifications[index];
        let id = notification.id();
        let ident = notification.ident.clone();

        let action = notification.action.as_ref().map(|(label, handler)| {
            let handler = Rc::clone(handler);
            let centre = cx.entity().downgrade();
            let reported = id.clone();
            Button::new(ident.child("action"))
                .semantic_parent(id.clone())
                .label(label.clone())
                .ghost()
                .control_size(ControlSize::Xs)
                .on_click(move |window, cx| {
                    handler(window, cx);
                    let reported = reported.clone();
                    centre
                        .update(cx, |centre, cx| {
                            centre.mark_read(reported.as_ref(), cx);
                            cx.emit(NotificationCenterEvent::ActionTaken(reported));
                        })
                        .ok();
                })
        });

        let dismiss = {
            let centre = cx.entity().downgrade();
            let dismissed = id.clone();
            IconButton::new(
                ident.child("dismiss"),
                gpui_kit_assets::Icon::Close,
                cx.strings().text(StringKey::Dismiss),
            )
            .semantic_parent(id.clone())
            .control_size(ControlSize::Xs)
            .on_click(move |_, cx| {
                let dismissed = dismissed.clone();
                centre
                    .update(cx, |centre, cx| centre.dismiss(dismissed.as_ref(), cx))
                    .ok();
            })
        };

        let unread_mark = (!notification.read).then(|| {
            div()
                .flex_none()
                .size(px(6.0))
                .rounded_full()
                .bg(theme.colors.accent)
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child("unread").semantic_id(), Role::Status)
                        .parent(id.clone())
                        .text(cx.strings().text(StringKey::NotificationsUnread)),
                )
        });

        div()
            .row()
            .w_full()
            .items_start()
            .gap_token(&theme, Space::Sm)
            .px_token(&theme, Space::Sm)
            .py_token(&theme, Space::Xs)
            .child(
                div()
                    .flex_none()
                    .mt(px(5.0))
                    .child(StatusDot::new(notification.tone)),
            )
            .child(
                div()
                    .column()
                    .flex_1()
                    .min_w_0()
                    .gap_token(&theme, Space::Xs)
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Label)
                            // A record that has been read steps back rather
                            // than disappearing: it is still a report.
                            .text_color(if notification.read {
                                theme.colors.text_muted
                            } else {
                                theme.colors.text
                            })
                            .child(notification.message.clone()),
                    )
                    .children(notification.detail.clone().map(|detail| {
                        div()
                            .type_scale(&theme, TypeScale::Caption)
                            .text_color(theme.colors.text_muted)
                            .child(detail)
                    }))
                    .children(notification.at.clone().map(|at| {
                        div()
                            .type_scale(&theme, TypeScale::Caption)
                            .text_color(theme.colors.text_faint)
                            .child(at)
                    })),
            )
            .children(unread_mark)
            .children(action)
            .child(dismiss)
            .semantic_in(
                cx,
                NodeSpec::new(id, Role::Row)
                    .parent(self.ident.semantic_id())
                    .text(notification.message.clone())
                    // The severity is published by name, so a test reads the
                    // claim rather than the colour.
                    .value(notification.tone.name())
                    .checked(notification.read),
            )
            .into_any_element()
    }
}

impl Focusable for NotificationCenter {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Sizable for NotificationCenter {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Render for NotificationCenter {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let unread = self.unread();
        let count_ident = self.ident.child("unread");

        let badge = (!unread.is_zero()).then(|| {
            Badge::new(unread.wording(cx))
                .tone(Tone::Accent)
                .id(count_ident.clone())
        });

        let mark_all = (!unread.is_zero()).then(|| {
            let centre = cx.entity().downgrade();
            Button::new(self.ident.child("mark-all-read"))
                .label(cx.strings().text(StringKey::NotificationsMarkAllRead))
                .ghost()
                .control_size(ControlSize::Xs)
                .semantic_parent(self.ident.semantic_id())
                .on_click(move |_, cx| {
                    centre
                        .update(cx, |centre, cx| centre.mark_all_read(cx))
                        .ok();
                })
        });

        let clear_all = (!self.notifications.is_empty()).then(|| {
            let centre = cx.entity().downgrade();
            Button::new(self.ident.child("clear-all"))
                .label(cx.strings().text(StringKey::NotificationsClearAll))
                .ghost()
                .control_size(ControlSize::Xs)
                .semantic_parent(self.ident.semantic_id())
                .on_click(move |_, cx| {
                    centre.update(cx, |centre, cx| centre.clear(cx)).ok();
                })
        });

        let rows: Vec<AnyElement> = (0..self.notifications.len())
            // Newest first: the thing that just happened is the thing being
            // looked for.
            .rev()
            .map(|index| self.row(index, cx))
            .collect();

        let body = if rows.is_empty() {
            EmptyState::new(
                self.ident.child("empty"),
                cx.strings().text(StringKey::NotificationsEmpty),
            )
            .kind(EmptyKind::Empty)
            .detail(cx.strings().text(StringKey::NotificationsEmptyDetail))
            .into_any_element()
        } else {
            div().column().w_full().children(rows).into_any_element()
        };

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .child(
                div()
                    .row()
                    .w_full()
                    .gap_token(&theme, Space::Sm)
                    .px_token(&theme, Space::Sm)
                    .py_token(&theme, Space::Xs)
                    .surface(&theme, Surface::Raised)
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Label)
                            .child(cx.strings().text(StringKey::NotificationsTitle)),
                    )
                    .children(badge)
                    .child(div().flex_1())
                    .children(mark_all)
                    .children(clear_all),
            )
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::List)
                    // The value is the badge's own claim, so a test reads what
                    // the badge says rather than counting rows.
                    .text(cx.strings().format(
                        StringKey::NotificationsUnreadCount,
                        &[unread.wording(cx).as_ref()],
                    ))
                    .value(unread.wording(cx)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_that_lost_records_stops_claiming_to_be_exact() {
        assert_eq!(UnreadCount::Exact(3).value(), 3);
        assert_eq!(UnreadCount::AtLeast(3).value(), 3);
        assert_ne!(UnreadCount::Exact(3), UnreadCount::AtLeast(3));
        assert_eq!(UnreadCount::Exact(0).name(), "exact");
        assert_eq!(UnreadCount::AtLeast(0).name(), "at-least");
    }

    #[test]
    fn a_notification_and_its_toast_share_one_identity() {
        let notification = Notification::new("run.failed", "Refused").tone(Tone::Danger);
        assert_eq!(
            notification.toast().ident().semantic_id(),
            notification.id()
        );
    }
}
