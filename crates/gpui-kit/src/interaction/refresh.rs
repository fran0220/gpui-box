//! A downward touch intent at the actual scroll start, never an automatic fetch.

use crate::{
    controls::button::Button,
    foundation::{Disableable, Ident, Sizable, StyledExt},
    overlay::panel::Body,
};
use gpui::{
    Axis, Context, EventEmitter, IntoElement, ParentElement, Render, ScrollHandle, SharedString,
    Styled, TouchPanEvent, TouchPhase, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TypeScale};

/// Refresh activity is separate from the retained list's data state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RefreshState {
    #[default]
    Ready,
    Pending(SharedString),
    Loading(SharedString),
    Error(SharedString),
    Unavailable(SharedString),
}

impl RefreshState {
    fn enabled(&self) -> bool {
        matches!(self, Self::Ready | Self::Error(_))
    }
    fn busy(&self) -> bool {
        matches!(self, Self::Pending(_) | Self::Loading(_))
    }
    fn message(&self) -> Option<&SharedString> {
        match self {
            Self::Ready => None,
            Self::Pending(value)
            | Self::Loading(value)
            | Self::Error(value)
            | Self::Unavailable(value) => Some(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullToRefreshEvent {
    RefreshRequested,
}

/// Retains its caller-built content in every state, including a failed refresh.
/// The supplied scroll handle must be the handle used by that content.
pub struct PullToRefresh {
    ident: Ident,
    scroll: ScrollHandle,
    body: Option<Body>,
    state: RefreshState,
    enabled: bool,
    threshold: f32,
    pull: Option<f32>,
    label: SharedString,
    armed_label: SharedString,
}

impl EventEmitter<PullToRefreshEvent> for PullToRefresh {}

impl PullToRefresh {
    /// Labels are caller-authored/localized: accessible refresh alternative and
    /// the armed release instruction. No timing or transport lives here.
    pub fn new(
        ident: impl Into<Ident>,
        scroll: ScrollHandle,
        label: impl Into<SharedString>,
        armed_label: impl Into<SharedString>,
    ) -> Self {
        Self {
            ident: ident.into(),
            scroll,
            body: None,
            state: RefreshState::Ready,
            enabled: true,
            threshold: 72.0,
            pull: None,
            label: label.into(),
            armed_label: armed_label.into(),
        }
    }

    pub fn set_content(&mut self, body: Option<Body>, cx: &mut Context<Self>) {
        self.body = body;
        cx.notify();
    }

    pub fn set_state(&mut self, state: RefreshState, cx: &mut Context<Self>) {
        self.state = state;
        self.pull = None;
        cx.notify();
    }

    /// Revocation abandons a pull without reporting a refresh.
    pub fn set_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.enabled = enabled;
        self.pull = None;
        cx.notify();
    }

    pub fn set_threshold(&mut self, threshold: f32, cx: &mut Context<Self>) -> bool {
        if !threshold.is_finite() || threshold <= 0.0 {
            return false;
        }
        self.threshold = threshold;
        self.pull = None;
        cx.notify();
        true
    }

    fn can_refresh(&self) -> bool {
        self.enabled && self.state.enabled()
    }

    fn pan(&mut self, event: &TouchPanEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta = f32::from(event.position.y - event.start_position.y);
        match event.phase {
            TouchPhase::Started => {
                if !self.can_refresh()
                    || event.axis != Axis::Vertical
                    || !pull_claim(delta, f32::from(self.scroll.offset().y))
                {
                    return;
                }
                self.pull = Some(0.0);
                window.prevent_default();
            }
            TouchPhase::Cancelled => {
                self.pull = None;
                cx.notify();
                return;
            }
            _ => {}
        }
        if self.pull.is_none() {
            return;
        }
        self.pull = Some(delta.max(0.0).min(self.threshold * 1.5));
        if event.phase == TouchPhase::Ended {
            let armed = delta >= self.threshold;
            self.pull = None;
            if armed && self.can_refresh() {
                cx.emit(PullToRefreshEvent::RefreshRequested);
            }
        }
        cx.notify();
    }
}

fn pull_claim(delta: f32, scroll_offset: f32) -> bool {
    delta > 0.0 && scroll_offset.abs() < 0.5
}

impl Render for PullToRefresh {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let enabled = self.can_refresh();
        let entity = cx.entity().downgrade();
        let mut refresh = Button::new(self.ident.child("refresh"))
            .label(self.label.clone())
            .control_size(ControlSize::Touch)
            .secondary()
            .disabled(!enabled)
            .semantic_parent(self.ident.semantic_id());
        if enabled {
            refresh = refresh.on_click(move |_, cx| {
                entity
                    .update(cx, |refresh, cx| {
                        if refresh.can_refresh() {
                            cx.emit(PullToRefreshEvent::RefreshRequested);
                        }
                    })
                    .ok();
            });
        }
        let message = self.state.message().cloned().or_else(|| {
            self.pull.map(|pull| {
                if pull >= self.threshold {
                    self.armed_label.clone()
                } else {
                    self.label.clone()
                }
            })
        });
        let entity = cx.entity().downgrade();
        let body = self.body.as_ref().map(|body| body(window, cx));
        div()
            .relative()
            .column()
            .size_full()
            .min_h_0()
            .gap_token(&theme, Space::Xs)
            .child(refresh)
            .children(message.map(|message| {
                crate::foundation::text(&theme, TypeScale::Body, message.clone()).semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("status").semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .text(message)
                        .busy(self.state.busy()),
                )
            }))
            .child(div().h(px(self.pull.unwrap_or(0.0) * 0.35)).flex_none())
            .child(div().flex_1().min_h_0().overflow_hidden().children(body))
            .children(enabled.then(|| {
                super::pan::touch_pan(
                    self.ident.child("pan").element_id(),
                    move |event, window, cx| {
                        entity
                            .update(cx, |refresh, cx| refresh.pan(event, window, cx))
                            .ok();
                    },
                )
            }))
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .busy(self.state.busy())
                    .disabled(!enabled),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_downward_at_actual_start_claims_refresh() {
        assert!(pull_claim(19.0, 0.0));
        assert!(!pull_claim(-85.0, 0.0));
        assert!(!pull_claim(85.0, -30.0));
        assert!(!pull_claim(85.0, 30.0));
        assert!(!RefreshState::Pending("Queued".into()).enabled());
        assert!(!RefreshState::Unavailable("Refused".into()).enabled());
        assert!(RefreshState::Error("Failed".into()).enabled());
    }
}
