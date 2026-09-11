//! Detented bottom surfaces sharing Drawer modal ownership and motion.
//!
//! Heights are caller-owned logical pixels, not guesses about a phone model.
//! Dragging previews a height; only `set_detent` accepts the requested detent.

use std::rc::Rc;

use gpui::{
    App, AppContext, Axis, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Subscription, TouchPanEvent, TouchPhase, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TypeScale};

use super::{Drawer, DrawerEvent, Edge, panel::Body};
use crate::controls::button::Button;
use crate::foundation::{Disableable, Ident, Sizable, StyledExt};

/// A stable detent identity and its desired surface height in logical pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetDetent {
    pub id: SharedString,
    pub height: f32,
}

impl SheetDetent {
    pub fn new(id: impl Into<SharedString>, height: f32) -> Self {
        Self {
            id: id.into(),
            height,
        }
    }
}

/// Sheet lifecycle plus a request that never changes caller selection itself.
#[derive(Debug, Clone, PartialEq)]
pub enum BottomSheetEvent {
    Opened,
    Dismissed,
    Closed,
    DetentRequested(SharedString),
}

/// A retained bottom Drawer with explicit detents and cancellable drag preview.
pub struct BottomSheet {
    ident: Ident,
    drawer: Entity<Drawer>,
    detents: Vec<SheetDetent>,
    selected: SharedString,
    preview: Option<f32>,
    drag_from: Option<f32>,
    scroll: Option<ScrollHandle>,
    content: Option<Body>,
    content_focus: Vec<FocusHandle>,
    detent_focus: Vec<(SharedString, FocusHandle)>,
    _subscription: Subscription,
}

impl EventEmitter<BottomSheetEvent> for BottomSheet {}

impl BottomSheet {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let drawer = cx.new(|cx| {
            Drawer::new(ident.clone(), window, cx)
                .edge(Edge::Bottom)
                .avoid_insets(true)
                .close_control_size(ControlSize::Touch)
        });
        let subscription = cx.subscribe(&drawer, |sheet, _, event, cx| {
            let event = match event {
                DrawerEvent::Opened => BottomSheetEvent::Opened,
                DrawerEvent::Dismissed => BottomSheetEvent::Dismissed,
                DrawerEvent::Closed => BottomSheetEvent::Closed,
                DrawerEvent::ResizeRequested(_) => return,
            };
            sheet.preview = None;
            sheet.drag_from = None;
            cx.emit(event);
            cx.notify();
        });
        Self {
            ident,
            drawer,
            detents: vec![SheetDetent::new("default", 360.0)],
            selected: "default".into(),
            preview: None,
            drag_from: None,
            scroll: None,
            content: None,
            content_focus: Vec::new(),
            detent_focus: Vec::new(),
            _subscription: subscription,
        }
    }

    /// Replaces the detent set atomically. Empty, duplicate, nonfinite or
    /// nonpositive heights and a missing selected identity leave it unchanged.
    pub fn set_detents(
        &mut self,
        detents: Vec<SheetDetent>,
        selected: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> bool {
        let selected = selected.into();
        if !valid_detents(&detents, &selected) {
            return false;
        }
        self.detents = detents;
        self.detent_focus = self
            .detents
            .iter()
            .map(|detent| {
                let handle = self
                    .detent_focus
                    .iter()
                    .find(|(id, _)| *id == detent.id)
                    .map(|(_, handle)| handle.clone())
                    .unwrap_or_else(|| cx.focus_handle());
                (detent.id.clone(), handle)
            })
            .collect();
        self.selected = selected;
        self.preview = None;
        self.drag_from = None;
        self.sync_size(cx);
        cx.notify();
        true
    }

    /// Applies caller selection, also interrupting any visual drag preview.
    pub fn set_detent(&mut self, id: impl Into<SharedString>, cx: &mut Context<Self>) -> bool {
        let id = id.into();
        if !self.detents.iter().any(|detent| detent.id == id) {
            return false;
        }
        self.selected = id;
        self.preview = None;
        self.drag_from = None;
        self.sync_size(cx);
        cx.notify();
        true
    }

    pub fn selected_detent(&self) -> &SharedString {
        &self.selected
    }

    pub fn set_title(&mut self, title: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.drawer
            .update(cx, |drawer, cx| drawer.set_title(title, cx));
    }

    pub fn set_content(&mut self, content: Option<Body>, cx: &mut Context<Self>) {
        self.content = content;
        cx.notify();
    }

    /// The actual scrolling body handle. Downward motion at its start shrinks
    /// the sheet; upward motion expands a nonmaximum sheet before scrolling.
    /// Without a handle only the explicit grip acquires sheet drags.
    pub fn set_scroll_handle(&mut self, scroll: Option<ScrollHandle>, cx: &mut Context<Self>) {
        self.scroll = scroll;
        self.drag_from = None;
        self.preview = None;
        self.sync_size(cx);
        cx.notify();
    }

    pub fn set_focus_stops(
        &mut self,
        stops: impl IntoIterator<Item = FocusHandle>,
        cx: &mut Context<Self>,
    ) {
        self.content_focus = stops.into_iter().collect();
        cx.notify();
    }

    pub fn set_dismissable(&mut self, dismissable: bool, cx: &mut Context<Self>) {
        self.drawer
            .update(cx, |drawer, cx| drawer.set_dismissable(dismissable, cx));
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_size(cx);
        self.drawer.update(cx, |drawer, cx| drawer.open(window, cx));
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview = None;
        self.drag_from = None;
        self.sync_size(cx);
        self.drawer
            .update(cx, |drawer, cx| drawer.close(window, cx));
    }

    pub fn is_open(&self, cx: &App) -> bool {
        self.drawer.read(cx).is_open()
    }

    pub fn settle(&mut self, cx: &mut Context<Self>) {
        self.drawer.update(cx, |drawer, cx| drawer.settle(cx));
    }

    fn height(&self) -> f32 {
        self.detents
            .iter()
            .find(|detent| detent.id == self.selected)
            .expect("validated selected detent")
            .height
    }

    fn sync_size(&mut self, cx: &mut Context<Self>) {
        let height = self.preview.unwrap_or_else(|| self.height());
        self.drawer
            .update(cx, |drawer, cx| drawer.set_size(height, cx));
    }

    fn pan(
        &mut self,
        event: &TouchPanEvent,
        handle: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = f32::from(event.position.y - event.start_position.y);
        let insets = window.insets().effective();
        let available =
            f32::from(window.viewport_size().height - insets.top - insets.bottom).max(0.0);
        // Compare and drag what was actually laid out. A 650px detent in a
        // 350px usable viewport must respond to the first pixel of movement.
        let resolved: Vec<_> = self
            .detents
            .iter()
            .map(|detent| SheetDetent::new(detent.id.clone(), detent.height.min(available)))
            .collect();
        let current = self.height().min(available);
        let min = resolved
            .iter()
            .map(|detent| detent.height)
            .fold(f32::INFINITY, f32::min);
        let max = resolved
            .iter()
            .map(|detent| detent.height)
            .fold(0.0, f32::max);
        match event.phase {
            TouchPhase::Started => {
                if !self.is_open(cx) || event.axis != Axis::Vertical {
                    return;
                }
                let at_start = self
                    .scroll
                    .as_ref()
                    .is_some_and(|scroll| f32::from(scroll.offset().y).abs() < 0.5);
                if !handle
                    && !sheet_claim(delta, at_start, self.scroll.is_some(), current, min, max)
                {
                    return;
                }
                self.drag_from = Some(current);
                window.prevent_default();
            }
            TouchPhase::Cancelled => {
                self.drag_from = None;
                self.preview = None;
                self.sync_size(cx);
                cx.notify();
                return;
            }
            _ => {}
        }
        let Some(from) = self.drag_from else {
            return;
        };
        let preview = (from - delta).clamp(min, max);
        self.preview = Some(preview);
        if event.phase == TouchPhase::Ended {
            let nearest = nearest_detent(&resolved, preview);
            self.drag_from = None;
            self.preview = None;
            // Equal rendered heights are not a change of caller identity.
            if nearest.id != self.selected
                && (nearest.height - preview).abs() < (current - preview).abs()
            {
                cx.emit(BottomSheetEvent::DetentRequested(nearest.id.clone()));
            }
        }
        self.sync_size(cx);
        cx.notify();
    }
}

fn sheet_claim(
    delta: f32,
    at_start: bool,
    has_scroll: bool,
    height: f32,
    min: f32,
    max: f32,
) -> bool {
    has_scroll && ((delta > 0.0 && at_start && height > min) || (delta < 0.0 && height < max))
}

fn valid_detents(detents: &[SheetDetent], selected: &str) -> bool {
    !detents.is_empty()
        && detents.iter().any(|detent| detent.id == selected)
        && detents.iter().enumerate().all(|(index, detent)| {
            !detent.id.is_empty()
                && detent.height.is_finite()
                && detent.height > 0.0
                && !detents[..index].iter().any(|other| other.id == detent.id)
        })
}

fn nearest_detent(detents: &[SheetDetent], height: f32) -> &SheetDetent {
    detents
        .iter()
        .min_by(|a, b| {
            (a.height - height)
                .abs()
                .total_cmp(&(b.height - height).abs())
        })
        .expect("nonempty validated detents")
}

impl Focusable for BottomSheet {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.drawer.read(cx).focus_handle(cx)
    }
}

impl Render for BottomSheet {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = self.content.clone();
        let ident = self.ident.clone();
        let sheet = cx.entity().downgrade();
        let detents = self.detents.clone();
        let detent_focus = self.detent_focus.clone();
        let selected = self.selected.clone();
        let open = self.is_open(cx);
        let mut stops = self.content_focus.clone();
        if detents.len() > 1 {
            stops.extend(
                detent_focus
                    .iter()
                    .filter(|(id, _)| *id != selected)
                    .map(|(_, handle)| handle.clone()),
            );
        }
        self.drawer.update(cx, |drawer, cx| {
            drawer.set_focus_stops(stops, cx);
            drawer.set_content(
                Some(Rc::new(move |window, cx| {
                    let theme = cx.theme().clone();
                    let handle_pan = sheet.clone();
                    let content_pan = sheet.clone();
                    let grip = div()
                        .relative()
                        .flex()
                        .justify_center()
                        .flex_none()
                        .py_token(&theme, Space::Xs)
                        .child(
                            div()
                                .w(px(36.0))
                                .h(px(3.0))
                                .rounded_full()
                                .bg(theme.colors.divider),
                        )
                        .children(open.then(|| {
                            crate::interaction::pan::touch_pan(
                                ident.child("handle.pan").element_id(),
                                move |event, window, cx| {
                                    handle_pan
                                        .update(cx, |sheet, cx| sheet.pan(event, true, window, cx))
                                        .ok();
                                },
                            )
                        }))
                        .semantic_in(
                            cx,
                            NodeSpec::new(ident.child("handle").semantic_id(), Role::Group)
                                .parent(ident.semantic_id()),
                        );
                    let content = div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .children(body.as_ref().map(|body| body(window, cx)))
                        .children(open.then(|| {
                            crate::interaction::pan::touch_pan(
                                ident.child("body.pan").element_id(),
                                move |event, window, cx| {
                                    content_pan
                                        .update(cx, |sheet, cx| sheet.pan(event, false, window, cx))
                                        .ok();
                                },
                            )
                        }));
                    div()
                        .column()
                        .size_full()
                        .min_h_0()
                        .child(grip)
                        .children((detents.len() > 1).then(|| {
                            div()
                                .row()
                                .flex_none()
                                .gap_token(&theme, Space::Xs)
                                .children(detents.iter().map(|detent| {
                                    let sheet = sheet.clone();
                                    let id = detent.id.clone();
                                    let mut button = Button::new(ident.child("detent").child(&id))
                                        .label(id.clone())
                                        .control_size(ControlSize::Touch)
                                        .secondary()
                                        .disabled(!open || id == selected);
                                    if let Some((_, focus)) =
                                        detent_focus.iter().find(|(key, _)| *key == id)
                                    {
                                        button = button.track_focus(focus);
                                    }
                                    if open && id != selected {
                                        button = button.on_click(move |_, cx| {
                                            sheet
                                                .update(cx, |_, cx| {
                                                    cx.emit(BottomSheetEvent::DetentRequested(
                                                        id.clone(),
                                                    ))
                                                })
                                                .ok();
                                        });
                                    }
                                    button
                                }))
                        }))
                        .child(content)
                        .into_any_element()
                })),
                cx,
            )
        });
        self.drawer.clone()
    }
}

/// Caller-owned availability of an action group. Failure never removes rows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SheetActionState {
    #[default]
    Ready,
    Pending(SharedString),
    Loading(SharedString),
    Error(SharedString),
    Unavailable(SharedString),
}

impl SheetActionState {
    pub(crate) fn allows_actions(&self) -> bool {
        matches!(self, Self::Ready | Self::Error(_))
    }
    pub(crate) fn message(&self) -> Option<&SharedString> {
        match self {
            Self::Ready => None,
            Self::Pending(message)
            | Self::Loading(message)
            | Self::Error(message)
            | Self::Unavailable(message) => Some(message),
        }
    }
}

/// Stable caller identity and presentation for one action; no business action.
#[derive(Debug, Clone)]
pub struct SheetAction {
    pub id: SharedString,
    pub label: SharedString,
    pub destructive: bool,
    pub disabled: bool,
}

impl SheetAction {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            destructive: false,
            disabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionSheetEvent {
    ActionRequested(SharedString),
    Sheet(BottomSheetEvent),
}

/// A sheet of caller-owned actions. Choosing a row does not close the sheet or
/// assume success; the caller supplies pending/loading/error and closes it.
pub struct ActionSheet {
    sheet: Entity<BottomSheet>,
    actions: Vec<SheetAction>,
    focus: Vec<(SharedString, FocusHandle)>,
    state: SheetActionState,
    interactive: bool,
    _subscription: Subscription,
}

impl EventEmitter<ActionSheetEvent> for ActionSheet {}

impl ActionSheet {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let owner = cx.entity().downgrade();
        let sheet = cx.new(|cx| BottomSheet::new(ident.clone(), window, cx));
        let scroll = ScrollHandle::new();
        sheet.update(cx, |sheet, cx| {
            sheet.set_scroll_handle(Some(scroll.clone()), cx);
            sheet.set_content(
                Some(Rc::new(move |_, cx| {
                    let Some(owner) = owner.upgrade() else {
                        return div().into_any_element();
                    };
                    let theme = cx.theme().clone();
                    let actions = owner.read(cx).actions.clone();
                    let state = owner.read(cx).state.clone();
                    let focus = owner.read(cx).focus.clone();
                    let open = owner.read(cx).interactive;
                    div()
                        .id(ident.child("actions").element_id())
                        .column()
                        .size_full()
                        .overflow_y_scroll()
                        .track_scroll(&scroll)
                        .gap_token(&theme, Space::Sm)
                        .children(state.message().map(|message| {
                            crate::foundation::text(&theme, TypeScale::Body, message.clone())
                                .semantic_in(
                                    cx,
                                    NodeSpec::new(
                                        ident.child("status").semantic_id(),
                                        Role::Status,
                                    )
                                    .parent(ident.semantic_id())
                                    .text(message.clone())
                                    .busy(matches!(
                                        state,
                                        SheetActionState::Pending(_) | SheetActionState::Loading(_)
                                    )),
                                )
                        }))
                        .children(actions.into_iter().map(|action| {
                            let disabled = !open || action.disabled || !state.allows_actions();
                            let mut button = Button::new(ident.child("action").child(&action.id))
                                .label(action.label)
                                .control_size(ControlSize::Touch)
                                .semantic_parent(ident.semantic_id())
                                .disabled(disabled);
                            if let Some((_, focus)) = focus.iter().find(|(id, _)| *id == action.id)
                            {
                                button = button.track_focus(focus);
                            }
                            if action.destructive {
                                button = button.danger();
                            } else {
                                button = button.secondary();
                            }
                            if !disabled {
                                let owner = owner.downgrade();
                                button = button.on_click(move |_, cx| {
                                    owner
                                        .update(cx, |_, cx| {
                                            cx.emit(ActionSheetEvent::ActionRequested(
                                                action.id.clone(),
                                            ))
                                        })
                                        .ok();
                                });
                            }
                            button
                        }))
                        .into_any_element()
                })),
                cx,
            )
        });
        let subscription = cx.subscribe(&sheet, |actions, _, event, cx| {
            match event {
                BottomSheetEvent::Opened => actions.interactive = true,
                BottomSheetEvent::Dismissed | BottomSheetEvent::Closed => {
                    actions.interactive = false
                }
                BottomSheetEvent::DetentRequested(_) => {}
            }
            cx.emit(ActionSheetEvent::Sheet(event.clone()));
            cx.notify();
        });
        Self {
            sheet,
            actions: Vec::new(),
            focus: Vec::new(),
            state: SheetActionState::Ready,
            interactive: false,
            _subscription: subscription,
        }
    }

    /// Access to the same sheet, for title, detents, focus stops and dismissal.
    pub fn sheet(&self) -> Entity<BottomSheet> {
        self.sheet.clone()
    }

    pub fn set_actions(&mut self, actions: Vec<SheetAction>, cx: &mut Context<Self>) {
        self.focus = actions
            .iter()
            .map(|action| {
                let handle = self
                    .focus
                    .iter()
                    .find(|(id, _)| *id == action.id)
                    .map(|(_, handle)| handle.clone())
                    .unwrap_or_else(|| cx.focus_handle());
                (action.id.clone(), handle)
            })
            .collect();
        self.actions = actions;
        self.sync_focus(cx);
        cx.notify();
    }

    pub fn set_state(&mut self, state: SheetActionState, cx: &mut Context<Self>) {
        self.state = state;
        self.sync_focus(cx);
        cx.notify();
    }

    fn sync_focus(&mut self, cx: &mut Context<Self>) {
        let stops: Vec<_> = self
            .focus
            .iter()
            .filter(|(id, _)| {
                self.state.allows_actions()
                    && self
                        .actions
                        .iter()
                        .any(|action| action.id == *id && !action.disabled)
            })
            .map(|(_, handle)| handle.clone())
            .collect();
        self.sheet
            .update(cx, |sheet, cx| sheet.set_focus_stops(stops, cx));
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.interactive = true;
        self.sheet.update(cx, |sheet, cx| sheet.open(window, cx));
    }
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.interactive = false;
        self.sheet.update(cx, |sheet, cx| sheet.close(window, cx));
    }
}

impl Focusable for ActionSheet {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.sheet.read(cx).focus_handle(cx)
    }
}

impl Render for ActionSheet {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.sheet.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detents_validate_identity_and_nearest_uses_asymmetric_heights() {
        let detents = vec![
            SheetDetent::new("compact", 180.0),
            SheetDetent::new("full", 650.0),
            SheetDetent::new("half", 370.0),
        ];
        assert!(valid_detents(&detents, "half"));
        assert_eq!(nearest_detent(&detents, 250.0).id, "compact");
        assert_eq!(nearest_detent(&detents, 280.0).id, "half");
        assert_eq!(nearest_detent(&detents, 580.0).id, "full");
        assert!(!valid_detents(&detents, "missing"));
        assert!(!valid_detents(&[SheetDetent::new("bad", f32::NAN)], "bad"));
        assert!(!valid_detents(
            &[
                SheetDetent::new("same", 20.0),
                SheetDetent::new("same", 200.0)
            ],
            "same"
        ));
    }

    #[test]
    fn pending_and_unavailable_do_not_turn_into_empty_or_actionable() {
        assert!(SheetActionState::Error("Failed".into()).allows_actions());
        assert!(!SheetActionState::Pending("Waiting".into()).allows_actions());
        assert!(!SheetActionState::Loading("Sending".into()).allows_actions());
        assert!(!SheetActionState::Unavailable("Refused".into()).allows_actions());
    }
}
