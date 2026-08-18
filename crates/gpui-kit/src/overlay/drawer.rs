//! A panel that slides in from one side of the window.
//!
//! A drawer is a [`Dialog`](crate::overlay::Dialog) that arrives from an edge
//! instead of the centre: the same scrim, the same focus trap, the same
//! escape and scrim dismissal, and the same body callback, because an open
//! surface re-renders for as long as it stays open.
//!
//! Where it differs is the exit. A drawer slides out, and an element cannot
//! animate after it has been dropped, so the drawer stays in the tree until
//! [`Presence`] says the exit has finished and only then reports
//! [`DrawerEvent::Closed`].

use gpui::{
    AnyElement, App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, Render, SharedString, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Space, Theme};

use crate::foundation::{Ident, StyledExt};
use crate::motion::{self, Easing, MotionSpec, Phase, Presence};
use crate::overlay::focus::FocusTrap;
use crate::overlay::layer::{Edge, Overlay, surface};
use crate::overlay::panel::{self, Body};
use crate::overlay::stack;
use crate::strings::{ActiveStrings, StringKey};

/// How wide a left or right drawer is, and how tall a top or bottom one is,
/// before the caller says otherwise. Neither value repeats elsewhere.
const DEFAULT_SIZE: f32 = 360.0;

/// What the drawer reports. The owner decides what any of it means.
///
/// [`DrawerEvent::Closed`] arrives when the panel has finished sliding out,
/// not when closing was asked for, so a subscriber that tears down state on
/// close does not tear it down mid-animation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawerEvent {
    Opened,
    /// The drawer was waved away, by escape or by the scrim.
    Dismissed,
    Closed,
    /// The free edge was dragged to this size. The drawer does not apply it.
    ResizeRequested(f32),
}

impl EventEmitter<DrawerEvent> for Drawer {}

/// A panel anchored to one side of the window.
pub struct Drawer {
    ident: Ident,
    focus_handle: FocusHandle,
    edge: Edge,
    size: f32,
    title: SharedString,
    description: Option<SharedString>,
    body: Option<Body>,
    footer: Option<Body>,
    dismissable: bool,
    resizable: bool,
    /// A size the pointer is holding, which is transient visual state until
    /// the host applies [`DrawerEvent::ResizeRequested`].
    preview: Option<f32>,
    drag: Option<(f32, f32)>,
    open: bool,
    /// Set by `open`, cleared by the first frame that can act on it.
    pending_focus: bool,
    presence: Option<Presence>,
    progress: f32,
    stops: Vec<FocusHandle>,
    trap: FocusTrap,
}

impl std::fmt::Debug for Drawer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Drawer")
            .field("ident", &self.ident)
            .field("edge", &self.edge)
            .field("title", &self.title)
            .field("has_body", &self.body.is_some())
            .field("dismissable", &self.dismissable)
            .field("open", &self.open)
            .field("rendered", &self.is_rendered())
            .finish()
    }
}

impl Drawer {
    pub fn new(ident: impl Into<Ident>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            edge: Edge::Right,
            size: DEFAULT_SIZE,
            title: SharedString::default(),
            description: None,
            body: None,
            footer: None,
            dismissable: true,
            resizable: false,
            preview: None,
            drag: None,
            open: false,
            pending_focus: false,
            presence: None,
            progress: 0.0,
            stops: Vec::new(),
            trap: FocusTrap::new(),
        }
    }

    /// Which side the panel hangs from and slides in from.
    pub fn edge(mut self, edge: Edge) -> Self {
        self.edge = edge;
        self
    }

    /// The width of a left or right drawer, or the height of a top or bottom
    /// one.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(0.0);
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Supplies the body, rebuilt on every frame the drawer is on screen.
    pub fn content(mut self, body: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> Self {
        self.body = Some(std::rc::Rc::new(body));
        self
    }

    /// Supplies the footer, rebuilt on every frame the drawer is on screen.
    pub fn footer(
        mut self,
        footer: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.footer = Some(std::rc::Rc::new(footer));
        self
    }

    /// Whether escape and the scrim close the drawer. A drawer that is not
    /// dismissable installs neither handler.
    pub fn dismissable(mut self, dismissable: bool) -> Self {
        self.dismissable = dismissable;
        self
    }

    /// Puts a grab handle on the free edge. A drag reports
    /// [`DrawerEvent::ResizeRequested`] and applies nothing.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn set_size(&mut self, size: f32, cx: &mut Context<Self>) {
        self.size = size.max(0.0);
        self.preview = None;
        cx.notify();
    }

    /// The controls tab walks between while the drawer is open.
    ///
    /// The body is the caller's, so the caller is the only one that knows
    /// which of its handles are focus stops.
    pub fn focus_stops(mut self, stops: impl IntoIterator<Item = FocusHandle>) -> Self {
        self.stops = stops.into_iter().collect();
        self
    }

    pub fn set_focus_stops(
        &mut self,
        stops: impl IntoIterator<Item = FocusHandle>,
        cx: &mut Context<Self>,
    ) {
        self.stops = stops.into_iter().collect();
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_dismissable(&self) -> bool {
        self.dismissable
    }

    /// True while the panel must stay in the tree, including for the whole
    /// slide out.
    pub fn is_rendered(&self) -> bool {
        self.presence
            .as_ref()
            .is_some_and(|presence| presence.is_rendered())
    }

    pub fn set_title(&mut self, title: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.title = title.into();
        cx.notify();
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        let theme = cx.theme().clone();
        self.open = true;
        self.pending_focus = true;
        let presence = self
            .presence
            .get_or_insert_with(|| Presence::hidden(enter_spec(&theme), exit_spec(&theme)));
        presence.show();
        stack::push(self.ident.semantic_id(), cx);
        self.trap.engage(window, cx);
        cx.emit(DrawerEvent::Opened);
        cx.notify();
    }

    /// Starts the slide out. The drawer stays on screen until it finishes.
    pub fn close(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.pending_focus = false;
        stack::pop(&self.ident.semantic_id(), cx);
        if let Some(presence) = self.presence.as_mut() {
            presence.hide();
        }
        cx.notify();
    }

    /// Reports a wave-away. A drawer that is not dismissable cannot be waved
    /// away even by a host calling this directly.
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open || !self.dismissable {
            return;
        }
        cx.emit(DrawerEvent::Dismissed);
        self.close(window, cx);
    }

    /// Finishes the slide that is running, for a host that wants the panel
    /// where it is going without the frames in between.
    pub fn settle(&mut self, cx: &mut Context<Self>) {
        if let Some(presence) = self.presence.as_mut() {
            presence.settle();
            self.progress = presence.progress();
        }
        cx.notify();
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
        if !stack::is_top(&self.ident.semantic_id(), cx) {
            return;
        }
        self.dismiss(window, cx);
        cx.stop_propagation();
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

    /// How far the panel still has to travel, in pixels along its edge.
    fn travel(&self) -> f32 {
        self.shown_size() * (1.0 - self.progress)
    }

    fn shown_size(&self) -> f32 {
        self.preview.unwrap_or(self.size)
    }

    fn resize_handle(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.resizable || !self.open {
            return None;
        }
        let ident = self.ident.child("resize");
        let label = cx.strings().text(StringKey::DrawerResize);
        let horizontal = self.edge.is_horizontal();
        let drawer = cx.entity().downgrade();
        let start_size = self.shown_size();
        let edge = self.edge;
        let min_size = theme.space(Space::Xl) * 4.0;
        let handle = div()
            .id(ident.element_id())
            .absolute()
            .when(horizontal, |element| element.w(px(6.0)).h_full().top_0())
            .when(!horizontal, |element| element.h(px(6.0)).w_full().left_0())
            .when(matches!(edge, Edge::Left), |element| element.right_0())
            .when(matches!(edge, Edge::Right), |element| element.left_0())
            .when(matches!(edge, Edge::Top), |element| element.bottom_0())
            .when(matches!(edge, Edge::Bottom), |element| element.top_0())
            .cursor_pointer()
            .bg(theme.colors.hairline)
            .on_mouse_down(MouseButton::Left, {
                let drawer = drawer.clone();
                move |event, _, cx| {
                    let at = if horizontal {
                        f32::from(event.position.x)
                    } else {
                        f32::from(event.position.y)
                    };
                    drawer
                        .update(cx, |drawer, _| {
                            drawer.drag = Some((start_size, at));
                        })
                        .ok();
                    cx.stop_propagation();
                }
            })
            .on_mouse_move({
                let drawer = drawer.clone();
                move |event, window, cx| {
                    if event.pressed_button != Some(MouseButton::Left) {
                        return;
                    }
                    drawer
                        .update(cx, |drawer, cx| {
                            let Some((from, at)) = drawer.drag else {
                                return;
                            };
                            let now = if horizontal {
                                f32::from(event.position.x)
                            } else {
                                f32::from(event.position.y)
                            };
                            let delta = match edge {
                                Edge::Left | Edge::Top => now - at,
                                Edge::Right | Edge::Bottom => at - now,
                            };
                            let next = (from + delta).max(min_size);
                            drawer.preview = Some(next);
                            cx.emit(DrawerEvent::ResizeRequested(next));
                            cx.notify();
                        })
                        .ok();
                    window.refresh();
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let drawer = drawer.clone();
                move |_, _, cx| {
                    drawer
                        .update(cx, |drawer, cx| {
                            drawer.drag = None;
                            cx.notify();
                        })
                        .ok();
                }
            });
        Some(
            handle
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Button)
                        .parent(self.ident.semantic_id())
                        .text(label)
                        .value(self.shown_size().to_string()),
                )
                .into_any_element(),
        )
    }
}

/// A drawer is a heavy thing being pulled out, so it arrives on a spring and
/// leaves on a curve: the pull has weight, the dismissal is just gone.
fn enter_spec(theme: &Theme) -> MotionSpec {
    motion::dialog_arrival(theme)
}

fn exit_spec(theme: &Theme) -> MotionSpec {
    MotionSpec::new(theme.motion.quick_ms, Easing::Exit.curve(theme))
}

impl Focusable for Drawer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Drawer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.trap.begin_frame();
        if self.presence.is_none() {
            return div().into_any_element();
        }

        self.progress = self
            .presence
            .as_mut()
            .map(|presence| presence.animate(window, cx))
            .unwrap_or_default();
        if self
            .presence
            .as_ref()
            .is_none_or(|presence| presence.phase() == Phase::Gone)
        {
            // The exit has run its course, so the panel leaves the tree now
            // and the keyboard goes back to whatever opened it.
            self.presence = None;
            self.progress = 0.0;
            self.trap.release(window, cx);
            cx.emit(DrawerEvent::Closed);
            return div().into_any_element();
        }

        for stop in self.stops.clone() {
            self.trap.register(stop);
        }
        if self.trap.stops().is_empty() {
            self.trap.register(self.focus_handle.clone());
        }
        if self.pending_focus {
            // The handle can only take focus once this frame has put it in the
            // dispatch tree, which is why opening only records the intent.
            self.pending_focus = false;
            self.trap.focus_first(window, cx);
        }

        let theme = cx.theme().clone();
        let title = self.title.clone();
        let description = self.description.clone();
        let body = self.body.clone().map(|body| body(window, cx));
        let footer = self.footer.clone().map(|footer| footer(window, cx));
        let horizontal = self.edge.is_horizontal();
        let travel = self.travel();

        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Dialog)
            .expanded(self.open)
            .focus(&self.focus_handle);
        if !title.is_empty() {
            spec = spec.text(title.clone());
        }

        let heading = (!title.is_empty()).then(|| panel::heading(&self.ident, &theme, title, cx));
        let description =
            description.map(|description| panel::description(&self.ident, &theme, description, cx));

        let shown = self.shown_size();
        let handle = self.resize_handle(&theme, cx);
        let mut card = surface(&theme, Elevation::Modal)
            .relative()
            .when(horizontal, |element| element.w(px(shown)).h_full())
            .when(!horizontal, |element| element.h(px(shown)).w_full())
            .p_token(&theme, Space::Lg)
            .gap_token(&theme, Space::Sm)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_navigation_key));
        card = match self.edge {
            Edge::Left => card.left(px(-travel)),
            Edge::Right => card.left(px(travel)),
            Edge::Top => card.top(px(-travel)),
            Edge::Bottom => card.top(px(travel)),
        };
        if self.dismissable {
            card = card.on_key_down(cx.listener(Self::on_dismiss_key));
        }
        let card = card
            .children(heading)
            .children(description)
            .children(body.map(|body| div().flex_1().overflow_hidden().child(body)))
            .children(footer)
            .children(handle)
            .semantic_in(cx, spec);

        let mut overlay = Overlay::edge(self.ident.child("overlay"), self.edge)
            .stack(stack::depth(&self.ident.semantic_id(), cx))
            .child(card);
        if self.dismissable && self.open && stack::is_top(&self.ident.semantic_id(), cx) {
            let drawer = cx.entity().downgrade();
            overlay = overlay.on_dismiss(move |window, cx| {
                drawer
                    .update(cx, |drawer, cx| drawer.dismiss(window, cx))
                    .ok();
            });
        }
        overlay.into_any_element()
    }
}
