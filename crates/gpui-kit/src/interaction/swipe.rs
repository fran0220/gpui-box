//! Horizontal reveal, with explicit activation and caller-owned operation state.

use crate::{
    controls::button::Button,
    foundation::{Disableable, Ident, Sizable, StyledExt},
    overlay::{
        panel::Body,
        sheet::{SheetAction, SheetActionState},
    },
};
use gpui::{
    Axis, Context, EventEmitter, IntoElement, ParentElement, Render, SharedString, Styled,
    TouchPanEvent, TouchPhase, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TypeScale};

/// Physical sides. The caller maps leading/trailing according to reading order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeSide {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwipeActionsEvent {
    Revealed(Option<SwipeSide>),
    ActionRequested(SharedString),
}

/// A stable row with two optional action trays. Revealing is transient visual
/// state, never selection or deletion. Vertical gestures remain scroll gestures.
pub struct SwipeActions {
    ident: Ident,
    body: Option<Body>,
    left: Vec<SheetAction>,
    right: Vec<SheetAction>,
    state: SheetActionState,
    enabled: bool,
    revealed: Option<SwipeSide>,
    drag_from: Option<f32>,
    preview: Option<f32>,
    reveal_label: SharedString,
}

impl EventEmitter<SwipeActionsEvent> for SwipeActions {}

impl SwipeActions {
    pub fn new(ident: impl Into<Ident>, reveal_label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            body: None,
            left: Vec::new(),
            right: Vec::new(),
            state: SheetActionState::Ready,
            enabled: true,
            revealed: None,
            drag_from: None,
            preview: None,
            reveal_label: reveal_label.into(),
        }
    }

    pub fn set_content(&mut self, body: Option<Body>, cx: &mut Context<Self>) {
        self.body = body;
        cx.notify();
    }

    pub fn set_actions(
        &mut self,
        side: SwipeSide,
        actions: Vec<SheetAction>,
        cx: &mut Context<Self>,
    ) {
        match side {
            SwipeSide::Left => self.left = actions,
            SwipeSide::Right => self.right = actions,
        }
        self.revealed = None;
        self.cancel();
        cx.notify();
    }

    pub fn set_state(&mut self, state: SheetActionState, cx: &mut Context<Self>) {
        self.state = state;
        self.cancel();
        cx.notify();
    }
    pub fn set_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.enabled = enabled;
        self.cancel();
        cx.notify();
    }

    /// Keyboard and assistive-technology alternative to the physical gesture.
    /// Does not perform a tray action. Missing sides are rejected.
    pub fn reveal(&mut self, side: Option<SwipeSide>, cx: &mut Context<Self>) -> bool {
        if !self.enabled || side.is_some_and(|side| self.actions(side).is_empty()) {
            return false;
        }
        self.cancel();
        self.revealed = side;
        cx.emit(SwipeActionsEvent::Revealed(side));
        cx.notify();
        true
    }

    pub fn revealed(&self) -> Option<SwipeSide> {
        self.revealed
    }

    fn actions(&self, side: SwipeSide) -> &[SheetAction] {
        match side {
            SwipeSide::Left => &self.left,
            SwipeSide::Right => &self.right,
        }
    }
    fn cancel(&mut self) {
        self.drag_from = None;
        self.preview = None;
    }
    fn offset(&self, width: f32) -> f32 {
        match self.revealed {
            Some(SwipeSide::Left) => width,
            Some(SwipeSide::Right) => -width,
            None => 0.0,
        }
    }

    fn pan(
        &mut self,
        event: &TouchPanEvent,
        width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = f32::from(event.position.x - event.start_position.x);
        match event.phase {
            TouchPhase::Started => {
                if !self.enabled || event.axis != Axis::Horizontal {
                    return;
                }
                if self.revealed.is_none()
                    && ((delta > 0.0 && self.left.is_empty())
                        || (delta < 0.0 && self.right.is_empty()))
                {
                    return;
                }
                self.drag_from = Some(self.offset(width));
                window.prevent_default();
            }
            TouchPhase::Cancelled => {
                self.cancel();
                cx.notify();
                return;
            }
            _ => {}
        }
        let Some(from) = self.drag_from else {
            return;
        };
        let offset = (from + delta).clamp(
            if self.right.is_empty() { 0.0 } else { -width },
            if self.left.is_empty() { 0.0 } else { width },
        );
        self.preview = Some(offset);
        if event.phase == TouchPhase::Ended {
            self.revealed = revealed_at(offset, width);
            self.cancel();
            cx.emit(SwipeActionsEvent::Revealed(self.revealed));
        }
        cx.notify();
    }
}

fn revealed_at(offset: f32, width: f32) -> Option<SwipeSide> {
    if offset >= width * 0.5 {
        Some(SwipeSide::Left)
    } else if offset <= -width * 0.5 {
        Some(SwipeSide::Right)
    } else {
        None
    }
}

impl Render for SwipeActions {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let width = theme.space(Space::Xl) * 5.0;
        let offset = self.preview.unwrap_or_else(|| self.offset(width));
        let side = if offset > 0.0 {
            Some(SwipeSide::Left)
        } else if offset < 0.0 {
            Some(SwipeSide::Right)
        } else {
            None
        };
        let actions = side
            .map(|side| self.actions(side).to_vec())
            .unwrap_or_default();
        let mut tray = div()
            .absolute()
            .top_0()
            .bottom_0()
            .w(px(offset.abs()))
            .overflow_hidden()
            .column()
            .justify_center()
            .gap_token(&theme, Space::Xs);
        if side == Some(SwipeSide::Right) {
            tray = tray.right_0();
        } else {
            tray = tray.left_0();
        }
        for action in actions {
            let disabled = action.disabled
                || !self.enabled
                || !self.state.allows_actions()
                || self.preview.is_some();
            let entity = cx.entity().downgrade();
            let id = action.id.clone();
            let mut button = Button::new(self.ident.child("action").child(&id))
                .label(action.label)
                .control_size(ControlSize::Touch)
                .disabled(disabled)
                .semantic_parent(self.ident.semantic_id());
            if action.destructive {
                button = button.danger();
            } else {
                button = button.secondary();
            }
            if !disabled {
                button = button.on_click(move |_, cx| {
                    entity
                        .update(cx, |_, cx| {
                            cx.emit(SwipeActionsEvent::ActionRequested(id.clone()))
                        })
                        .ok();
                });
            }
            tray = tray.child(button);
        }
        let body = self.body.as_ref().map(|body| body(window, cx));
        let entity = cx.entity().downgrade();
        let foreground = div()
            .relative()
            .left(px(offset))
            .w_full()
            .column()
            .p_token(&theme, Space::Sm)
            .bg(theme.colors.panel)
            .children(body);
        let alternatives = [SwipeSide::Left, SwipeSide::Right]
            .into_iter()
            .filter(|side| !self.actions(*side).is_empty())
            .map(|side| {
                let entity = cx.entity().downgrade();
                let suffix = match side {
                    SwipeSide::Left => "left",
                    SwipeSide::Right => "right",
                };
                let mut button = Button::new(self.ident.child("reveal").child(suffix))
                    .label(format!("{} · {}", self.reveal_label, suffix))
                    .control_size(ControlSize::Touch)
                    .secondary()
                    .disabled(!self.enabled);
                if self.enabled {
                    button = button.on_click(move |_, cx| {
                        entity
                            .update(cx, |swipe, cx| {
                                swipe.reveal(
                                    if swipe.revealed == Some(side) {
                                        None
                                    } else {
                                        Some(side)
                                    },
                                    cx,
                                );
                            })
                            .ok();
                    });
                }
                button
            });
        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .child(
                div()
                    .relative()
                    .w_full()
                    .overflow_hidden()
                    .child(tray)
                    .child(foreground)
                    .children(self.enabled.then(|| {
                        super::pan::touch_pan(
                            self.ident.child("pan").element_id(),
                            move |event, window, cx| {
                                entity
                                    .update(cx, |swipe, cx| swipe.pan(event, width, window, cx))
                                    .ok();
                            },
                        )
                    })),
            )
            .child(
                div()
                    .row()
                    .gap_token(&theme, Space::Xs)
                    .children(alternatives),
            )
            .children(self.state.message().map(|message| {
                crate::foundation::text(&theme, TypeScale::Body, message.clone()).semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("status").semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .text(message.clone()),
                )
            }))
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .expanded(self.revealed.is_some())
                    .disabled(!self.enabled),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asymmetric_release_distinguishes_physical_sides_and_threshold() {
        assert_eq!(revealed_at(61.0, 120.0), Some(SwipeSide::Left));
        assert_eq!(revealed_at(-83.0, 120.0), Some(SwipeSide::Right));
        assert_eq!(revealed_at(-59.0, 120.0), None);
        assert_eq!(revealed_at(19.0, 120.0), None);
    }
}
