//! A modal that asks one question and reports the answer.
//!
//! Open state and the element that had the keyboard before opening both
//! outlive a frame, so a dialog is a view rather than a builder. The body is
//! a callback instead of a stored element because an `AnyElement` can be
//! consumed once, while the dialog re-renders for as long as it stays open.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, SharedString, Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space};

use crate::controls::button::{Button, ButtonVariant};
use crate::foundation::{Ident, StyledExt};
use crate::motion::{Animated, Entrance};
use crate::overlay::focus::FocusTrap;
use crate::overlay::layer::{Overlay, OverlaySurface, surface};
use crate::overlay::panel::{self, Body};
use crate::overlay::stack;

/// What the dialog reports. The owner decides what any of it means.
///
/// An outcome is always followed by [`DialogEvent::Closed`], so a subscriber
/// that only cares that the dialog went away has one event to watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogEvent {
    Opened,
    /// The primary action was taken.
    Confirmed,
    /// The cancel action was taken.
    Cancelled,
    /// The dialog was waved away, by escape or by the scrim.
    Dismissed,
    Closed,
}

impl EventEmitter<DialogEvent> for Dialog {}

/// A composed modal: scrim, focus trap, title, body, and up to two actions.
pub struct Dialog {
    ident: Ident,
    focus_handle: FocusHandle,
    confirm_focus: FocusHandle,
    cancel_focus: FocusHandle,
    title: SharedString,
    description: Option<SharedString>,
    body: Option<Body>,
    confirm_label: Option<SharedString>,
    cancel_label: Option<SharedString>,
    dismissable: bool,
    destructive: bool,
    open: bool,
    /// Set by `open`, cleared by the first frame that can act on it.
    pending_focus: bool,
    trap: FocusTrap,
}

impl std::fmt::Debug for Dialog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Dialog")
            .field("ident", &self.ident)
            .field("title", &self.title)
            .field("has_body", &self.body.is_some())
            .field("dismissable", &self.dismissable)
            .field("destructive", &self.destructive)
            .field("open", &self.open)
            .finish()
    }
}

impl Dialog {
    pub fn new(ident: impl Into<Ident>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            confirm_focus: cx.focus_handle(),
            cancel_focus: cx.focus_handle(),
            title: SharedString::default(),
            description: None,
            body: None,
            confirm_label: None,
            cancel_label: None,
            dismissable: true,
            destructive: false,
            open: false,
            pending_focus: false,
            trap: FocusTrap::new(),
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Supplies the body, rebuilt on every frame the dialog is open.
    pub fn content(mut self, body: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> Self {
        self.body = Some(Rc::new(body));
        self
    }

    /// Whether escape and the scrim close the dialog. A dialog that is not
    /// dismissable installs neither handler.
    pub fn dismissable(mut self, dismissable: bool) -> Self {
        self.dismissable = dismissable;
        self
    }

    /// Marks the primary action as one that destroys something.
    pub fn destructive(mut self, destructive: bool) -> Self {
        self.destructive = destructive;
        self
    }

    pub fn confirm_label(mut self, label: impl Into<SharedString>) -> Self {
        self.confirm_label = Some(label.into());
        self
    }

    pub fn cancel_label(mut self, label: impl Into<SharedString>) -> Self {
        self.cancel_label = Some(label.into());
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_dismissable(&self) -> bool {
        self.dismissable
    }

    pub fn set_title(&mut self, title: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.title = title.into();
        cx.notify();
    }

    pub fn set_description(&mut self, description: Option<SharedString>, cx: &mut Context<Self>) {
        self.description = description;
        cx.notify();
    }

    /// Replaces the per-frame body factory without reopening the modal.
    /// `None` removes the body; focus and modal-stack membership are retained.
    pub fn set_content(&mut self, content: Option<Body>, cx: &mut Context<Self>) {
        self.body = content;
        cx.notify();
    }

    pub fn set_confirm_label(&mut self, label: Option<SharedString>, cx: &mut Context<Self>) {
        self.confirm_label = label;
        cx.notify();
    }

    pub fn set_cancel_label(&mut self, label: Option<SharedString>, cx: &mut Context<Self>) {
        self.cancel_label = label;
        cx.notify();
    }

    /// Changes user dismissal policy without closing the modal.
    pub fn set_dismissable(&mut self, dismissable: bool, cx: &mut Context<Self>) {
        self.dismissable = dismissable;
        cx.notify();
    }

    /// Changes action styling without moving focus. Initial focus policy is
    /// applied only when the dialog is next opened.
    pub fn set_destructive(&mut self, destructive: bool, cx: &mut Context<Self>) {
        self.destructive = destructive;
        cx.notify();
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        self.pending_focus = true;
        stack::push(self.ident.semantic_id(), window, cx);
        cx.emit(DialogEvent::Opened);
        cx.notify();
    }

    /// Closes without an outcome. The window is required because closing gives
    /// the keyboard back to whatever held it before the dialog opened.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.pending_focus = false;
        stack::pop(&self.ident.semantic_id(), window, cx);
        self.trap.begin_frame();
        cx.emit(DialogEvent::Closed);
        cx.notify();
    }

    pub fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        cx.emit(DialogEvent::Confirmed);
        self.close(window, cx);
    }

    pub fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        cx.emit(DialogEvent::Cancelled);
        self.close(window, cx);
    }

    /// Reports a wave-away. A dialog that is not dismissable cannot be waved
    /// away even by a host calling this directly.
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open || !self.dismissable {
            return;
        }
        cx.emit(DialogEvent::Dismissed);
        self.close(window, cx);
    }

    /// Where the keyboard lands when the dialog opens.
    ///
    /// A destructive confirmation opens on cancel, so a stray return key does
    /// not destroy anything.
    fn initial_focus(&self) -> FocusHandle {
        if self.destructive && self.cancel_label.is_some() {
            return self.cancel_focus.clone();
        }
        if self.confirm_label.is_some() {
            return self.confirm_focus.clone();
        }
        if self.cancel_label.is_some() {
            return self.cancel_focus.clone();
        }
        self.focus_handle.clone()
    }

    fn on_navigation_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.open || event.keystroke.key.as_str() != "tab" {
            return;
        }
        if event.keystroke.modifiers.shift {
            self.trap.focus_prev(window, cx);
        } else {
            self.trap.focus_next(window, cx);
        }
        cx.stop_propagation();
    }

    fn on_dismiss_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.open || event.keystroke.key.as_str() != "escape" {
            return;
        }
        if !stack::is_top(&self.ident.semantic_id(), window, cx) {
            return;
        }
        self.dismiss(window, cx);
        cx.stop_propagation();
    }

    fn actions(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.confirm_label.is_none() && self.cancel_label.is_none() {
            return None;
        }
        let theme = cx.theme().clone();
        let dialog = cx.entity().downgrade();
        let cancel = self.cancel_label.clone().map(|label| {
            let dialog = dialog.clone();
            Button::new(self.ident.child("cancel"))
                .label(label)
                .secondary()
                .track_focus(&self.cancel_focus)
                .on_click(move |window, cx| {
                    dialog
                        .update(cx, |dialog, cx| dialog.cancel(window, cx))
                        .ok();
                })
        });
        let confirm = self.confirm_label.clone().map(|label| {
            let dialog = dialog.clone();
            Button::new(self.ident.child("confirm"))
                .label(label)
                .variant(if self.destructive {
                    ButtonVariant::Danger
                } else {
                    ButtonVariant::Primary
                })
                .track_focus(&self.confirm_focus)
                .on_click(move |window, cx| {
                    dialog
                        .update(cx, |dialog, cx| dialog.confirm(window, cx))
                        .ok();
                })
        });

        Some(
            div()
                .row()
                .justify_end()
                .gap_token(&theme, Space::Sm)
                .children(cancel)
                .children(confirm)
                .into_any_element(),
        )
    }
}

impl Focusable for Dialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Dialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.trap.begin_frame();
        if !self.open {
            return div().into_any_element();
        }

        if self.cancel_label.is_some() {
            self.trap.register(self.cancel_focus.clone());
        }
        if self.confirm_label.is_some() {
            self.trap.register(self.confirm_focus.clone());
        }
        if self.trap.stops().is_empty() {
            self.trap.register(self.focus_handle.clone());
        }
        // A retained option update can remove the action currently holding
        // focus. Keep keyboard input inside this modal instead of leaving a
        // now-unmounted action handle focused.
        if (self.cancel_label.is_none() && self.cancel_focus.is_focused(window))
            || (self.confirm_label.is_none() && self.confirm_focus.is_focused(window))
        {
            self.pending_focus = true;
        }
        if self.pending_focus {
            // The handle can only take focus once this frame has put it in the
            // dispatch tree, which is why opening only records the intent.
            self.pending_focus = false;
            self.initial_focus().focus(window, cx);
        }

        let theme = cx.theme().clone();
        let title = self.title.clone();
        let description = self.description.clone();
        let body = self.body.clone().map(|body| body(window, cx));
        let actions = self.actions(cx);

        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Dialog)
            .modal(true)
            .focus(&self.focus_handle);
        if !title.is_empty() {
            spec = spec.text(title.clone());
        }
        if let Some(description) = description.clone() {
            spec = spec.description(description);
        }

        let heading = (!title.is_empty()).then(|| panel::heading(&self.ident, &theme, title, cx));
        let description =
            description.map(|description| panel::description(&self.ident, &theme, description, cx));

        let mut card = surface(self.ident.clone(), &theme, OverlaySurface::MODAL)
            .w(px(theme.measures.dialog_width))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_navigation_key));
        if self.dismissable {
            card = card.on_key_down(cx.listener(Self::on_dismiss_key));
        }
        let header = panel::band(&theme).children(heading).children(description);
        let body = body.map(|body| panel::band(&theme).child(body));
        // The actions sit in their own padded band. Placement and breathing
        // room distinguish the answer from the copy that asked without a
        // full-width seam across the modal.
        let footer = actions.map(|actions| panel::band(&theme).flex_none().child(actions));
        let card = card.child(header).children(body).children(footer);
        // The arrival wraps the card inside the element that publishes the
        // node, so the dialog is announced from its settled box and only the
        // pixels travel. The id lives under the dialog's own, so closing and
        // reopening replays the arrival rather than resuming a finished one.
        let card = div()
            .child(card.animate_in(self.ident.child("in").element_id(), cx, Entrance::Dialog))
            .semantic_in(cx, spec);

        let mut overlay = Overlay::modal(self.ident.child("overlay"))
            .stack(stack::depth(&self.ident.semantic_id(), window, cx))
            .child(card);
        if self.dismissable && stack::is_top(&self.ident.semantic_id(), window, cx) {
            let dialog = cx.entity().downgrade();
            overlay = overlay.on_dismiss(move |window, cx| {
                dialog
                    .update(cx, |dialog, cx| dialog.dismiss(window, cx))
                    .ok();
            });
        }
        overlay.into_any_element()
    }
}

#[cfg(test)]
mod retained_options_tests {
    use super::*;
    use gpui::{AppContext as _, TestAppContext};
    use gpui_kit_testkit::harness::Harness;
    use std::cell::RefCell;

    #[gpui::test]
    fn dialog_options_change_while_open_without_reopening(cx: &mut TestAppContext) {
        let slot = Rc::new(RefCell::new(None));
        let build = slot.clone();
        let mut harness = Harness::new(cx, crate::install, move |window, cx| {
            build
                .borrow_mut()
                .get_or_insert_with(|| {
                    cx.new(|cx| {
                        Dialog::new("retained.dialog", window, cx)
                            .title("Original")
                            .confirm_label("Confirm")
                            .cancel_label("Cancel")
                    })
                })
                .clone()
                .into_any_element()
        });
        let dialog = slot.borrow().clone().expect("dialog built");
        harness.update(|window, cx| dialog.update(cx, |dialog, cx| dialog.open(window, cx)));
        harness.frame();
        harness.update(|window, cx| {
            dialog.update(cx, |dialog, cx| {
                let focus = window.focused(cx);
                dialog.set_title("Updated", cx);
                dialog.set_description(Some("Current description".into()), cx);
                dialog.set_content(
                    Some(Rc::new(|_, _| {
                        Button::new("retained.body")
                            .label("Fresh body")
                            .into_any_element()
                    })),
                    cx,
                );
                dialog.set_confirm_label(Some("Apply".into()), cx);
                dialog.set_cancel_label(Some("Back".into()), cx);
                dialog.set_destructive(true, cx);
                dialog.set_dismissable(false, cx);
                assert!(dialog.is_open());
                assert_eq!(window.focused(cx), focus);
            })
        });
        harness.frame();
        assert!(harness.node("retained.body").is_some());
        assert_eq!(
            harness
                .node("retained.dialog.confirm")
                .expect("confirm")
                .text
                .as_deref(),
            Some("Apply")
        );
        harness.keystrokes("escape");
        harness.update(|window, cx| {
            dialog.update(cx, |dialog, cx| {
                assert!(dialog.is_open(), "dismissal policy takes effect while open");
                dialog.close(window, cx);
                dialog.open(window, cx);
            })
        });
        harness.frame();
        assert!(
            harness.node("retained.body").is_some(),
            "body factory must survive reopen"
        );
        harness.update(|_, cx| {
            dialog.update(cx, |dialog, cx| {
                dialog.set_content(None, cx);
                dialog.set_description(None, cx);
                dialog.set_confirm_label(None, cx);
                dialog.set_cancel_label(None, cx);
            })
        });
        harness.frame();
        assert!(harness.node("retained.body").is_none());
        assert!(harness.node("retained.dialog.confirm").is_none());
        harness.update(|window, cx| {
            assert!(
                dialog.read(cx).focus_handle.is_focused(window),
                "removing the focused action keeps focus inside the modal"
            );
        });
    }
}
