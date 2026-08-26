//! A region of a window whose contents the host could not produce.
//!
//! # This is not an error boundary, and is not called one
//!
//! A JavaScript error boundary catches an exception thrown while a subtree
//! renders and swaps in a fallback. Nothing here can do that, and the name
//! `ErrorBoundary` would promise it. What was established about GPUI at the
//! pinned revision:
//!
//! - `Render::render` and `RenderOnce::render` return `impl IntoElement`.
//!   There is no fallible render, no `Result`, and therefore no failure a
//!   parent could observe as a value coming back from a child.
//! - GPUI calls `panic::catch_unwind` in exactly one place, its own
//!   `#[gpui::test]` harness. Nothing on the platform draw path catches
//!   anything, so a panic in `render` unwinds straight out of `Window::draw`.
//! - It cannot simply be wrapped, either. A draw holds an element-arena scope
//!   guard whose exit hands back a clear token the caller owes, the arena
//!   itself hands out raw pointers into a bump allocation, and the window
//!   asserts its rendered-entity stack is empty around every draw. Unwinding
//!   past that leaves the window's own bookkeeping in a state the next frame
//!   does not expect. On top of which `&mut Window` and `&mut App` are not
//!   unwind-safe, so reaching for `catch_unwind` would mean asserting they
//!   were — which is precisely the claim the panic just disproved.
//!
//! So a render panic is not catchable here in any way worth shipping, and this
//! component does not pretend to catch one. It is for the ordinary case that
//! actually happens: **the host already holds a failure value**. It asked for
//! something and got an `Err`, a refusal, a timeout, a document it could not
//! parse. [`FailurePanel::from_result`] is the whole seam.
//!
//! What it guarantees is the part that matters either way: the failure stays
//! on screen in the host's own words, a retry belongs to the host, and a panel
//! that failed is never drawn as a panel that is empty.

use std::rc::Rc;

use gpui::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, TypeScale};

use crate::controls::button::Button;
use crate::foundation::direction::ActiveDirection;
use crate::foundation::{Ident, Sizable, StyledExt};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type RetryHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A panel that states what went wrong instead of what it was going to show.
#[derive(IntoElement)]
pub struct FailurePanel {
    ident: Ident,
    title: Option<SharedString>,
    /// The host's own words. Never authored here, and never rewritten.
    reason: SharedString,
    detail: Option<SharedString>,
    attempts: Option<usize>,
    retrying: bool,
    on_retry: Option<RetryHandler>,
}

impl std::fmt::Debug for FailurePanel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FailurePanel")
            .field("ident", &self.ident)
            .field("title", &self.title)
            .field("reason", &self.reason)
            .field("attempts", &self.attempts)
            .field("retrying", &self.retrying)
            .field("has_handler", &self.on_retry.is_some())
            .finish()
    }
}

impl FailurePanel {
    pub fn new(ident: impl Into<Ident>, reason: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            title: None,
            reason: reason.into(),
            detail: None,
            attempts: None,
            retrying: false,
            on_retry: None,
        }
    }

    /// The panel for a failure the host is already holding, or `None` when it
    /// is holding a value instead.
    ///
    /// This is the seam the whole component exists for: the caller renders its
    /// content in the `Ok` arm and this in the `Err` arm, and the failure it
    /// shows is the one the host actually has rather than one this crate
    /// caught.
    pub fn from_result<T, E: std::fmt::Display>(
        ident: impl Into<Ident>,
        result: &Result<T, E>,
    ) -> Option<Self> {
        match result {
            Ok(_) => None,
            Err(error) => Some(Self::new(ident, error.to_string())),
        }
    }

    /// What the panel was going to be, so the reader knows which part of the
    /// window is missing rather than only that something is.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// How many times this has been tried. A retry that keeps failing is a
    /// different fact from a first failure, and hiding the count would let a
    /// reader keep pressing a control that has never once worked.
    pub fn attempts(mut self, attempts: usize) -> Self {
        self.attempts = Some(attempts);
        self
    }

    /// Whether a retry is in flight. While it is, the failure stays on screen:
    /// nothing has replaced it yet.
    pub fn retrying(mut self, retrying: bool) -> Self {
        self.retrying = retrying;
        self
    }

    /// Reports that the typist asked for another attempt.
    ///
    /// Retrying belongs to the host — this panel has no idea what produced the
    /// failure — so it reports and does nothing. A panel with no handler shows
    /// no retry control at all rather than a dead one.
    pub fn on_retry(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for FailurePanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let title = self
            .title
            .clone()
            .unwrap_or_else(|| cx.strings().text(StringKey::FailureTitle));

        let retry = self.on_retry.clone().map(|handler| {
            // The one thing a reader can do about a failure is the loudest
            // control on the panel. A secondary chip beside a red message
            // reads as the disabled remains of something that already failed.
            Button::new(self.ident.child("retry"))
                .label(cx.strings().text(StringKey::TryAgain))
                .control_size(ControlSize::Sm)
                .semantic_parent(self.ident.semantic_id())
                // A retry already in flight refuses another one rather than
                // stacking attempts nobody asked for.
                .loading(self.retrying)
                .on_click(move |window, cx| handler(window, cx))
        });

        let attempts = self.attempts.filter(|count| *count > 1).map(|count| {
            let digits = cx.numbers().count(count);
            let wording = cx
                .strings()
                .format(StringKey::FailureAttempts, &[digits.as_ref()]);
            div()
                .type_scale(&theme, TypeScale::Caption)
                .text_color(theme.colors.text_faint)
                .child(wording.clone())
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("attempts").semantic_id(), Role::Text)
                        .parent(self.ident.semantic_id())
                        .text(wording)
                        .value(digits),
                )
        });

        let status = self.retrying.then(|| {
            let wording = cx.strings().text(StringKey::FailureRetrying);
            div()
                .type_scale(&theme, TypeScale::Caption)
                .text_color(theme.colors.text_muted)
                .child(wording.clone())
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("retrying").semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .text(wording)
                        .busy(true),
                )
        });

        let reason_ident = self.ident.child("reason");

        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .p_token(&theme, Space::Lg)
            .radius(&theme, Radius::Card)
            // A failure states its severity inside its own edge. A halo that
            // reaches into whatever is next to the panel makes the neighbour
            // look implicated, and it is the loudest thing in a frame whose
            // subject is the sentence, not the colour.
            .overflow_hidden()
            .relative()
            .bg(theme.colors.panel)
            .hairline(&theme)
            .child(crate::display::status::tone_rail(
                &theme,
                theme.colors.danger,
                cx.layout_direction(),
            ))
            .child(
                div()
                    .row()
                    .w_full()
                    .gap_token(&theme, Space::Sm)
                    .child(
                        icon(Icon::Danger)
                            .flex_none()
                            .size(px(theme.control.md.icon_size))
                            .text_color(theme.colors.danger),
                    )
                    .child(
                        // The name of what is missing outranks the sentence
                        // explaining it, which is the order they are read in.
                        // It wraps inside the panel: a headline that runs off
                        // the edge of a narrow column takes the last word of
                        // the failure with it.
                        div()
                            .min_w_0()
                            .flex_1()
                            .type_scale(&theme, TypeScale::Subtitle)
                            .text_color(theme.colors.text)
                            .child(title.clone()),
                    ),
            )
            // The reason is the host's sentence, shown word for word and given
            // a node of its own so a test can prove it survived.
            .child(
                div()
                    // The host's sentence is the panel's subject, so it is
                    // read at full strength. Muted, it sat at the same weight
                    // as the detail and the attempt count below it and the
                    // three read as one grey block.
                    .type_scale(&theme, TypeScale::Body)
                    .text_color(theme.colors.text)
                    .child(self.reason.clone())
                    .semantic_in(
                        cx,
                        NodeSpec::new(reason_ident.semantic_id(), Role::Text)
                            .parent(self.ident.semantic_id())
                            .text(self.reason.clone()),
                    ),
            )
            .when_some(self.detail.clone(), |element, detail| {
                element.child(
                    div()
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_muted)
                        .child(detail),
                )
            })
            .children(attempts)
            .children(status)
            .children(retry.map(|control| div().row().child(control)))
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(title)
                    // `failed` and never `empty`: a panel that could not be
                    // produced is not a panel with nothing in it.
                    .value("failed")
                    .invalid(true)
                    .busy(self.retrying),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_value_produces_no_panel() {
        let held: Result<u8, String> = Ok(7);
        assert!(FailurePanel::from_result("panel", &held).is_none());
    }

    #[test]
    fn a_held_failure_carries_the_hosts_own_words() {
        let held: Result<u8, String> = Err("the index is still building".into());
        let panel = FailurePanel::from_result("panel", &held).expect("a failure panel");
        assert_eq!(panel.reason, "the index is still building");
    }
}
