//! Transient notifications that report what happened without hiding a failure.
//!
//! A toast is a floating surface on the toast layer, so it lives here with the
//! rest of the overlay vocabulary rather than in a module of its own: it is
//! ordered by the same `zIndex` tokens and painted through the same deferred
//! pinning as a dialog or a tooltip.
//!
//! The host mounts a [`ToastLayer`] in the window it wants notifications drawn
//! in, and any call site adds one with [`push`]. The stack lives in the mounted
//! layer and the global holds only a weak handle to it, so the library renders
//! into the window it was given and reports a push it cannot deliver instead of
//! choosing a window on the caller's behalf.
//!
//! Timing is truthful before it is convenient:
//!
//! - a failure ([`Tone::Danger`] or [`Tone::Warning`]) stays until it is
//!   dismissed, because a report nobody saw is a report that did not happen;
//! - a pointer resting on a toast pauses its timer, so something being read
//!   cannot vanish mid-sentence;
//! - the stack has a cap, and only a toast that both times out and can be
//!   dismissed is evicted to make room.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, Context, Entity, Global, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Layer, Space, Theme, TypeScale};
use web_time::Instant;

use crate::controls::button::Button;
use crate::display::badge::Tone;
use crate::display::status::StatusDot;
use crate::foundation::{FocusRing, Ident, Sizable, StyledExt};
use crate::motion::{Easing, Flipping, MotionSpec, Phase, Presence, flip};
use crate::overlay::layer::{pinned, priority, surface};

/// How many toasts stand in the stack before one is evicted to make room.
const DEFAULT_CAPACITY: usize = 3;

/// The width of one notification, and the distance it travels while entering
/// or leaving. Neither value repeats anywhere else, so both stay next to the
/// component instead of in the token document.
const CARD_WIDTH: f32 = 320.0;
const TRAVEL: f32 = 8.0;

type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Which corner the stack grows from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    #[default]
    BottomRight,
}

impl ToastCorner {
    fn is_top(self) -> bool {
        matches!(self, Self::TopLeft | Self::TopRight)
    }

    fn is_leading(self) -> bool {
        matches!(self, Self::TopLeft | Self::BottomLeft)
    }
}

/// How long a toast stays on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifetime {
    /// Decided by tone when the toast reaches the stack.
    ByTone,
    Timed(Duration),
    Persistent,
}

struct Action {
    label: SharedString,
    handler: ActionHandler,
}

/// One notification: what happened, how bad it is, and at most one thing to do
/// about it.
pub struct Toast {
    ident: Ident,
    message: SharedString,
    detail: Option<SharedString>,
    tone: Tone,
    action: Option<Action>,
    dismissable: bool,
    lifetime: Lifetime,
}

impl std::fmt::Debug for Toast {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Toast")
            .field("ident", &self.ident)
            .field("message", &self.message)
            .field("detail", &self.detail)
            .field("tone", &self.tone)
            .field("has_action", &self.action.is_some())
            .field("dismissable", &self.dismissable)
            .field("lifetime", &self.lifetime)
            .finish()
    }
}

impl Toast {
    pub fn new(ident: impl Into<Ident>, message: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            message: message.into(),
            detail: None,
            tone: Tone::default(),
            action: None,
            dismissable: true,
            lifetime: Lifetime::ByTone,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// A second line, for the part of the report that does not fit in one.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// The one thing that can be done about this. A later call replaces an
    /// earlier one, because a notification with two answers is a dialog.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some(Action {
            label: label.into(),
            handler: Rc::new(handler),
        });
        self
    }

    /// Whether the toast carries a dismiss control. A toast that cannot be
    /// dismissed renders no such control and installs no handler for it.
    pub fn dismissable(mut self, dismissable: bool) -> Self {
        self.dismissable = dismissable;
        self
    }

    /// Leaves on its own after `timeout`, whatever the tone says.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.lifetime = Lifetime::Timed(timeout);
        self
    }

    /// Stays until it is dismissed.
    pub fn persistent(mut self) -> Self {
        self.lifetime = Lifetime::Persistent;
        self
    }

    pub fn ident(&self) -> &Ident {
        &self.ident
    }

    /// How long this toast has, or `None` when it stays until dismissed.
    ///
    /// A tone that reports a failure defaults to staying: a timer that hides a
    /// failure turns it into something that was never reported.
    fn allowance(&self, theme: &Theme) -> Option<Duration> {
        match self.lifetime {
            Lifetime::Timed(timeout) => Some(timeout),
            Lifetime::Persistent => None,
            Lifetime::ByTone if reports_failure(self.tone) => None,
            Lifetime::ByTone => Some(Duration::from_millis(theme.motion.toast_ms)),
        }
    }
}

fn reports_failure(tone: Tone) -> bool {
    matches!(tone, Tone::Danger | Tone::Warning)
}

fn enter_spec(theme: &Theme) -> MotionSpec {
    MotionSpec::new(theme.motion.menu_ms, Easing::Settle.curve(theme))
}

fn exit_spec(theme: &Theme) -> MotionSpec {
    MotionSpec::new(theme.motion.quick_ms, Easing::Exit.curve(theme))
}

/// One toast on screen, with the state that outlives a frame.
struct Entry {
    toast: Toast,
    presence: Presence,
    /// Time left before this toast leaves, or `None` while it stays.
    remaining: Option<Duration>,
    hovered: bool,
    progress: f32,
}

impl Entry {
    fn new(toast: Toast, remaining: Option<Duration>, theme: &Theme) -> Self {
        let mut presence = Presence::hidden(enter_spec(theme), exit_spec(theme));
        presence.show();
        Self {
            toast,
            presence,
            remaining,
            hovered: false,
            progress: 0.0,
        }
    }

    fn id(&self) -> SharedString {
        self.toast.ident.semantic_id()
    }

    fn leaving(&self) -> bool {
        matches!(self.presence.phase(), Phase::Exiting | Phase::Gone)
    }

    /// A toast that neither times out nor offers a way out is never evicted:
    /// the stack would otherwise drop a report nobody has seen.
    fn evictable(&self) -> bool {
        !self.leaving() && self.toast.dismissable && self.remaining.is_some()
    }

    /// Spends `delta` of the allowance and reports whether a timer is still
    /// running, which is what decides if the window needs another frame.
    fn count_down(&mut self, delta: Duration) -> bool {
        if self.hovered || !matches!(self.presence.phase(), Phase::Entering | Phase::Present) {
            return false;
        }
        let Some(remaining) = self.remaining else {
            return false;
        };
        let left = remaining.saturating_sub(delta);
        self.remaining = Some(left);
        if left.is_zero() {
            self.presence.hide();
            return false;
        }
        true
    }
}

/// The stack of notifications for one window.
///
/// The host mounts one of these where it wants toasts drawn; mounting is also
/// what makes [`push`] deliverable.
pub struct ToastLayer {
    corner: ToastCorner,
    capacity: usize,
    entries: Vec<Entry>,
    /// When the last frame that advanced a timer happened.
    last_tick: Option<Instant>,
}

impl std::fmt::Debug for ToastLayer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToastLayer")
            .field("corner", &self.corner)
            .field("capacity", &self.capacity)
            .field("toasts", &self.entries.len())
            .finish()
    }
}

impl ToastLayer {
    /// Mounts a layer and makes it the one [`push`] delivers to.
    ///
    /// A second layer takes over from the first, so an application with two
    /// windows decides which one notifies by mounting there last.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let handle = cx.weak_entity();
        cx.set_global(ToastCenter {
            layer: Some(handle),
        });
        Self {
            corner: ToastCorner::default(),
            capacity: DEFAULT_CAPACITY,
            entries: Vec::new(),
            last_tick: None,
        }
    }

    pub fn corner(mut self, corner: ToastCorner) -> Self {
        self.corner = corner;
        self
    }

    /// How many toasts stand at once. A capacity below one is raised to one.
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity.max(1);
        self
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The lifecycle phase of one toast, or `None` when it is not in the tree.
    pub fn phase(&self, ident: &str) -> Option<Phase> {
        self.entry(ident).map(|entry| entry.presence.phase())
    }

    /// Adds a toast, or refreshes the one that already carries this identity.
    pub fn push(&mut self, toast: Toast, cx: &mut Context<Self>) {
        let theme = cx.theme().clone();
        let remaining = toast.allowance(&theme);
        let id = toast.ident.semantic_id();

        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id() == id) {
            // One identity is one notification: a repeat updates what is on
            // screen instead of publishing the same id twice.
            entry.toast = toast;
            entry.remaining = remaining;
            entry.hovered = false;
            entry.presence.show();
            cx.notify();
            return;
        }

        self.make_room();
        self.entries.push(Entry::new(toast, remaining, &theme));
        cx.notify();
    }

    /// Starts the exit of one toast. The element stays in the tree until the
    /// exit finishes.
    pub fn dismiss(&mut self, ident: &str, cx: &mut Context<Self>) -> bool {
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.id() == ident && !entry.leaving())
        else {
            return false;
        };
        entry.presence.hide();
        cx.notify();
        true
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        for entry in &mut self.entries {
            entry.presence.hide();
        }
        cx.notify();
    }

    fn entry(&self, ident: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id() == ident)
    }

    fn set_hovered(&mut self, ident: &SharedString, hovered: bool, cx: &mut Context<Self>) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id() == *ident) {
            entry.hovered = hovered;
            cx.notify();
        }
    }

    /// Evicts oldest first, and only what is evictable. When nothing may go,
    /// the new toast still arrives: the cap yields rather than swallow a
    /// report that cannot be dismissed.
    fn make_room(&mut self) {
        while self.standing() >= self.capacity {
            let Some(index) = self.entries.iter().position(Entry::evictable) else {
                break;
            };
            self.entries[index].presence.hide();
        }
    }

    fn standing(&self) -> usize {
        self.entries.iter().filter(|entry| !entry.leaving()).count()
    }

    /// Advances timers and lifecycles by one frame, and drops what has gone.
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = cx.background_executor().now();
        let delta = self
            .last_tick
            .map(|last| now.saturating_duration_since(last))
            .unwrap_or_default();

        let mut timing = false;
        for entry in &mut self.entries {
            timing |= entry.count_down(delta);
        }
        for entry in &mut self.entries {
            entry.progress = entry.presence.animate(window, cx);
        }
        self.entries.retain(|entry| entry.presence.is_rendered());

        self.last_tick = timing.then_some(now);
        if timing {
            window.request_animation_frame();
        }
    }

    fn card(
        &self,
        index: usize,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entry = &self.entries[index];
        let toast = &entry.toast;
        let id = toast.ident.semantic_id();
        let color = toast.tone.color(theme);
        let layer = cx.entity().downgrade();

        let action = toast.action.as_ref().map(|action| {
            let handler = Rc::clone(&action.handler);
            let layer = layer.clone();
            let id = id.clone();
            Button::new(toast.ident.child("action"))
                .semantic_parent(id.clone())
                .label(action.label.clone())
                .secondary()
                .small()
                .on_click(move |window, cx| {
                    handler(window, cx);
                    layer
                        .update(cx, |layer, cx| layer.dismiss(id.as_ref(), cx))
                        .ok();
                })
        });

        let dismiss = toast.dismissable.then(|| {
            let dismiss_ident = toast.ident.child("dismiss");
            let mut control = div()
                .id(dismiss_ident.element_id())
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(16.0))
                .cursor_pointer()
                .tab_index(0)
                .focus_ring(theme)
                .child(
                    icon(Icon::Close)
                        .size(px(11.0))
                        .text_color(theme.colors.text_faint),
                )
                .semantic_in(
                    cx,
                    NodeSpec::new(dismiss_ident.semantic_id(), Role::Button)
                        .parent(id.clone())
                        .text("Dismiss"),
                );
            let clicked = {
                let layer = layer.clone();
                let id = id.clone();
                move |_window: &mut Window, cx: &mut App| {
                    layer
                        .update(cx, |layer, cx| layer.dismiss(id.as_ref(), cx))
                        .ok();
                }
            };
            let keyed = clicked.clone();
            control
                .interactivity()
                .on_click(move |_, window, cx| clicked(window, cx));
            control
                .interactivity()
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        keyed(window, cx);
                        cx.stop_propagation();
                    }
                });
            control
        });

        let detail = toast.detail.clone().map(|detail| {
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_muted)
                .child(detail.clone())
                .semantic_in(
                    cx,
                    NodeSpec::new(toast.ident.child("detail").semantic_id(), Role::Text)
                        .parent(id.clone())
                        .text(detail),
                )
        });

        let travel = TRAVEL * (1.0 - entry.progress);
        let hovered = entry.hovered;
        let hover_id = id.clone();

        let card = surface(theme, Elevation::Overlay)
            .id(toast.ident.element_id())
            .row()
            .items_start()
            .w(px(CARD_WIDTH))
            .gap_token(theme, Space::Sm)
            .p_token(theme, Space::Md)
            .border_color(color.opacity(0.35))
            .opacity(entry.progress)
            .top(px(if self.corner.is_top() {
                -travel
            } else {
                travel
            }))
            .on_hover(cx.listener(move |layer, hovered: &bool, _window, cx| {
                layer.set_hovered(&hover_id, *hovered, cx);
            }))
            .child(
                div()
                    .flex_none()
                    .mt(px(5.0))
                    .child(StatusDot::new(toast.tone)),
            )
            .child(
                div()
                    .column()
                    .flex_1()
                    .min_w_0()
                    .gap_token(theme, Space::Xs)
                    .child(
                        div()
                            .type_scale(theme, TypeScale::Label)
                            .child(toast.message.clone()),
                    )
                    .children(detail),
            )
            .children(action)
            .children(dismiss)
            .semantic_in(
                cx,
                NodeSpec::new(id, Role::Toast)
                    .text(toast.message.clone())
                    .value(toast.tone.name())
                    .hovered(hovered),
            );

        // The slot the card sits in is what slides, not the card: the card is
        // already carrying its own arrival travel, and a flip that watched
        // that would mistake an arrival for a reorder. The slot moves only
        // when the stack itself reflows, which is what happens when a toast
        // in the middle of the stack leaves.
        let slot = flip(toast.ident.child("slot").semantic_id(), cx);
        div().child(card).flip(&slot, window, cx).into_any_element()
    }
}

impl Render for ToastLayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        self.tick(window, cx);
        if self.entries.is_empty() {
            return div().into_any_element();
        }

        // The newest toast sits nearest the corner, so the column runs oldest
        // first from a bottom corner and newest first from a top one.
        let mut order: Vec<usize> = (0..self.entries.len()).collect();
        if self.corner.is_top() {
            order.reverse();
        }
        let mut cards: Vec<AnyElement> = Vec::with_capacity(order.len());
        for index in order {
            cards.push(self.card(index, &theme, window, cx));
        }

        let leading = self.corner.is_leading();
        let top = self.corner.is_top();
        let viewport = window.viewport_size();
        let stack = div()
            .column()
            .gap_token(&theme, Space::Sm)
            .when(leading, |element| element.items_start())
            .when(!leading, |element| element.items_end())
            .children(cards);
        // The frame is painted out of its parent's layout, so it takes its
        // size from the window rather than from wherever the host mounted it.
        let frame = div()
            .absolute()
            .top_0()
            .left_0()
            .w(viewport.width)
            .h(viewport.height)
            .flex()
            .flex_col()
            .p_token(&theme, Space::Lg)
            .when(top, |element| element.justify_start())
            .when(!top, |element| element.justify_end())
            .when(leading, |element| element.items_start())
            .when(!leading, |element| element.items_end())
            .child(stack);

        pinned(
            gpui::deferred(frame)
                .priority(priority(&theme, Layer::Toast))
                .into_any_element(),
        )
    }
}

/// The weak handle to the mounted layer.
///
/// Weak on purpose: a window that closes takes its notifications with it, and
/// a push afterwards reports that it went nowhere instead of resurrecting a
/// dead view.
#[derive(Default)]
struct ToastCenter {
    layer: Option<WeakEntity<ToastLayer>>,
}

impl Global for ToastCenter {}

/// Prepares the process for toasts. Idempotent, and called by
/// [`crate::install`].
pub fn install(cx: &mut App) {
    if !cx.has_global::<ToastCenter>() {
        cx.set_global(ToastCenter::default());
    }
}

fn mounted(cx: &App) -> Option<Entity<ToastLayer>> {
    cx.try_global::<ToastCenter>()?.layer.as_ref()?.upgrade()
}

/// Whether a layer is mounted and can therefore show a notification.
pub fn is_mounted(cx: &App) -> bool {
    mounted(cx).is_some()
}

/// Shows a toast, reporting whether it was delivered.
///
/// False means no layer is mounted. The library will not pick a window it was
/// not given, so the caller learns the notification went nowhere rather than
/// believing something was shown.
pub fn push(cx: &mut App, toast: Toast) -> bool {
    let Some(layer) = mounted(cx) else {
        return false;
    };
    layer.update(cx, |layer, cx| layer.push(toast, cx));
    true
}

/// Starts the exit of one toast, reporting whether it was on screen.
pub fn dismiss(cx: &mut App, ident: &str) -> bool {
    let Some(layer) = mounted(cx) else {
        return false;
    };
    layer.update(cx, |layer, cx| layer.dismiss(ident, cx))
}

/// Starts the exit of every toast on screen.
pub fn clear(cx: &mut App) {
    if let Some(layer) = mounted(cx) {
        layer.update(cx, |layer, cx| layer.clear(cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme::studio_dark()
    }

    #[test]
    fn a_failure_stays_and_everything_else_times_out() {
        let theme = theme();
        for tone in [Tone::Danger, Tone::Warning] {
            assert_eq!(
                Toast::new("run.failed", "Refused")
                    .tone(tone)
                    .allowance(&theme),
                None
            );
        }
        for tone in [Tone::Success, Tone::Info, Tone::Neutral, Tone::Accent] {
            assert_eq!(
                Toast::new("run.saved", "Saved")
                    .tone(tone)
                    .allowance(&theme),
                Some(Duration::from_millis(theme.motion.toast_ms))
            );
        }
    }

    #[test]
    fn an_explicit_lifetime_outranks_the_tone() {
        let theme = theme();
        assert_eq!(
            Toast::new("run.failed", "Refused")
                .tone(Tone::Danger)
                .timeout(Duration::from_millis(200))
                .allowance(&theme),
            Some(Duration::from_millis(200))
        );
        assert_eq!(
            Toast::new("run.saved", "Saved")
                .tone(Tone::Success)
                .persistent()
                .allowance(&theme),
            None
        );
    }

    #[test]
    fn only_a_dismissable_toast_that_times_out_can_be_evicted() {
        let theme = theme();
        let entry = |toast: Toast| {
            let remaining = toast.allowance(&theme);
            Entry::new(toast, remaining, &theme)
        };
        assert!(entry(Toast::new("a", "Saved").tone(Tone::Success)).evictable());
        assert!(!entry(Toast::new("b", "Refused").tone(Tone::Danger)).evictable());
        assert!(
            !entry(
                Toast::new("c", "Saved")
                    .tone(Tone::Success)
                    .dismissable(false)
            )
            .evictable()
        );
    }

    #[test]
    fn a_hovered_toast_spends_none_of_its_allowance() {
        let theme = theme();
        let mut entry = Entry::new(
            Toast::new("a", "Saved").timeout(Duration::from_millis(300)),
            Some(Duration::from_millis(300)),
            &theme,
        );
        entry.hovered = true;
        assert!(!entry.count_down(Duration::from_millis(500)));
        assert_eq!(entry.remaining, Some(Duration::from_millis(300)));

        entry.hovered = false;
        assert!(entry.count_down(Duration::from_millis(100)));
        assert_eq!(entry.remaining, Some(Duration::from_millis(200)));
    }

    #[test]
    fn a_spent_allowance_starts_the_exit_rather_than_dropping_the_toast() {
        let theme = theme();
        let mut entry = Entry::new(
            Toast::new("a", "Saved"),
            Some(Duration::from_millis(100)),
            &theme,
        );
        entry.presence.settle();
        assert!(!entry.count_down(Duration::from_millis(100)));
        assert_eq!(entry.presence.phase(), Phase::Exiting);
        assert!(entry.presence.is_rendered());
    }

    #[test]
    fn the_stack_grows_from_the_corner_it_is_given() {
        assert!(!ToastCorner::default().is_top());
        assert!(!ToastCorner::default().is_leading());
        assert!(ToastCorner::TopLeft.is_top() && ToastCorner::TopLeft.is_leading());
    }
}
