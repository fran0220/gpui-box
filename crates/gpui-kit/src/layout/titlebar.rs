//! Client-side desktop window chrome.
//!
//! [`DesktopTitlebar`] is the reusable composition above GPUI's window-control
//! hit-test primitive. The whole strip is a drag area; nested caller content is
//! explicitly restored to [`WindowControlArea::Client`], and each standard
//! button carries its native Min/Max/Close area. On Windows that preserves the
//! operating system's `HTMAXBUTTON` path and therefore Snap Layout. On Linux
//! the same buttons report an event for the host to apply through [`Window`].
//!
//! The titlebar never decides whether a requested close is accepted. It emits
//! [`DesktopTitlebarEvent::Close`]; a host that applies it with
//! [`Window::request_close`] still runs its registered close-refusal callback.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, MouseButton, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, WindowButton, WindowButtonLayout,
    WindowControlArea, WindowControls, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Surface, Theme, TypeScale};

use crate::foundation::{FocusRing, Ident, Pressable, StyledExt};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

const HEIGHT: f32 = 38.0;
const CONTROL_WIDTH: f32 = 46.0;
const GLYPH_SIZE: f32 = 10.0;
const MACOS_TRAFFIC_LIGHT_GUTTER: f32 = 76.0;

type EventHandler = Rc<dyn Fn(DesktopTitlebarEvent, &mut Window, &mut App)>;

/// A standard desktop-window request.
///
/// The component reports the request and changes no window state itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopTitlebarEvent {
    Minimize,
    ToggleMaximize,
    Close,
}

impl DesktopTitlebarEvent {
    /// Applies this standard request through GPUI's window contract.
    ///
    /// Calling this remains a host decision. In particular, `Close` uses the
    /// platform close path rather than removing the window directly, so an
    /// `on_window_should_close` callback can refuse it.
    pub fn apply(self, window: &mut Window) {
        match self {
            Self::Minimize => window.minimize_window(),
            Self::ToggleMaximize => window.zoom_window(),
            Self::Close => window.request_close(),
        }
    }
}

/// A client-rendered desktop titlebar with native window-control hit testing.
#[derive(IntoElement)]
pub struct DesktopTitlebar {
    ident: Ident,
    title: SharedString,
    subtitle: Option<SharedString>,
    left: Option<AnyElement>,
    right: Option<AnyElement>,
    button_layout: Option<WindowButtonLayout>,
    controls: Option<WindowControls>,
    on_event: Option<EventHandler>,
}

impl std::fmt::Debug for DesktopTitlebar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DesktopTitlebar")
            .field("ident", &self.ident)
            .field("title", &self.title)
            .field("subtitle", &self.subtitle)
            .field("has_left", &self.left.is_some())
            .field("has_right", &self.right.is_some())
            .field("has_handler", &self.on_event.is_some())
            .finish()
    }
}

impl DesktopTitlebar {
    pub fn new(ident: impl Into<Ident>, title: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            title: title.into(),
            subtitle: None,
            left: None,
            right: None,
            button_layout: None,
            controls: None,
            on_event: None,
        }
    }

    /// Secondary caller-owned context shown below the window title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Caller content on the physical left side of the strip.
    ///
    /// The slot is a normal client area even though the surrounding titlebar
    /// is draggable, so controls inside it retain their pointer behavior.
    pub fn left(mut self, content: impl IntoElement) -> Self {
        self.left = Some(content.into_any_element());
        self
    }

    /// Caller content on the physical right side of the strip.
    pub fn right(mut self, content: impl IntoElement) -> Self {
        self.right = Some(content.into_any_element());
        self
    }

    /// Overrides the platform's physical button order.
    ///
    /// This is useful for deterministic fixtures and for hosts that already
    /// own a desktop preference. Without it, Linux follows the platform's
    /// `button-layout`; Windows uses Minimize, Maximize, Close on the right;
    /// and macOS leaves controls to the native traffic lights.
    pub fn button_layout(mut self, layout: WindowButtonLayout) -> Self {
        self.button_layout = Some(layout);
        self
    }

    /// Overrides which standard window controls the platform reports.
    pub fn controls(mut self, controls: WindowControls) -> Self {
        self.controls = Some(controls);
        self
    }

    /// Receives standard window requests.
    ///
    /// No control buttons are rendered without this handler: an action the
    /// host has not agreed to apply is not shown as an operable control.
    pub fn on_event(
        mut self,
        handler: impl Fn(DesktopTitlebarEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for DesktopTitlebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let controls = self.controls.unwrap_or_else(|| window.window_controls());
        let layout = if cfg!(any(target_os = "macos", target_family = "wasm")) {
            empty_layout()
        } else {
            self.button_layout
                .or_else(|| cx.button_layout())
                .unwrap_or_else(default_layout)
        };
        let maximized = window.is_maximized();
        let handler = self.on_event.clone();

        #[cfg(target_os = "macos")]
        window.set_traffic_light_position(gpui::point(px(12.0), px((HEIGHT - 14.0) / 2.0)));

        let left_buttons = buttons(
            layout.left,
            &self.ident,
            controls,
            maximized,
            handler.clone(),
            &theme,
            cx,
        );
        let right_buttons = buttons(
            layout.right,
            &self.ident,
            controls,
            maximized,
            handler.clone(),
            &theme,
            cx,
        );
        let count = left_buttons.len() + right_buttons.len();
        let title = self.title.clone();
        let subtitle = self.subtitle.clone();

        let left = div()
            .flex()
            .h_full()
            .items_center()
            .flex_none()
            .when(cfg!(target_os = "macos"), |element| {
                element.pl(px(MACOS_TRAFFIC_LIGHT_GUTTER))
            })
            .children(left_buttons)
            .children(self.left.map(client_slot));
        let right = div()
            .flex()
            .h_full()
            .items_center()
            .flex_none()
            .children(self.right.map(client_slot))
            .children(right_buttons);

        let drag_handler = handler.clone();
        div()
            .relative()
            .flex()
            .items_center()
            .justify_between()
            .w_full()
            .h(px(HEIGHT))
            .surface(&theme, Surface::Panel)
            .border_b(px(theme.borders.hairline))
            .border_color(theme.colors.divider)
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                if event.click_count >= 2 && window.is_resizable() {
                    if let Some(handler) = drag_handler.as_ref() {
                        handler(DesktopTitlebarEvent::ToggleMaximize, window, cx);
                    }
                } else {
                    window.start_window_move();
                }
            })
            .on_mouse_down(MouseButton::Right, |event, window, cx| {
                window.show_window_menu(event.position);
                cx.stop_propagation();
            })
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px(px(MACOS_TRAFFIC_LIGHT_GUTTER))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .max_w_full()
                            .child(
                                div()
                                    .id(self.ident.child("title").element_id())
                                    .max_w_full()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .type_scale(&theme, TypeScale::Label)
                                    .text_color(theme.colors.text)
                                    .child(title.clone())
                                    .semantic_in(
                                        cx,
                                        NodeSpec::new(
                                            self.ident.child("title").semantic_id(),
                                            Role::Heading,
                                        )
                                        .parent(self.ident.semantic_id())
                                        .text(title),
                                    ),
                            )
                            .children(subtitle.map(|subtitle| {
                                div()
                                    .id(self.ident.child("subtitle").element_id())
                                    .max_w_full()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .type_scale(&theme, TypeScale::Caption)
                                    .text_color(theme.colors.text_muted)
                                    .child(subtitle.clone())
                                    .semantic_in(
                                        cx,
                                        NodeSpec::new(
                                            self.ident.child("subtitle").semantic_id(),
                                            Role::Text,
                                        )
                                        .parent(self.ident.semantic_id())
                                        .text(subtitle),
                                    )
                            })),
                    ),
            )
            .child(left)
            .child(right)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Toolbar)
                    .text(self.title)
                    .value(cx.numbers().count(count)),
            )
    }
}

fn empty_layout() -> WindowButtonLayout {
    WindowButtonLayout {
        left: [None; gpui::MAX_BUTTONS_PER_SIDE],
        right: [None; gpui::MAX_BUTTONS_PER_SIDE],
    }
}

fn default_layout() -> WindowButtonLayout {
    WindowButtonLayout {
        left: [None; gpui::MAX_BUTTONS_PER_SIDE],
        right: [
            Some(WindowButton::Minimize),
            Some(WindowButton::Maximize),
            Some(WindowButton::Close),
        ],
    }
}

fn buttons(
    layout: [Option<WindowButton>; gpui::MAX_BUTTONS_PER_SIDE],
    titlebar: &Ident,
    controls: WindowControls,
    maximized: bool,
    handler: Option<EventHandler>,
    theme: &Theme,
    cx: &mut App,
) -> Vec<AnyElement> {
    let Some(handler) = handler else {
        return Vec::new();
    };
    layout
        .into_iter()
        .flatten()
        .filter(|button| match button {
            WindowButton::Minimize => controls.minimize,
            WindowButton::Maximize => controls.maximize,
            WindowButton::Close => true,
        })
        .map(|button| {
            control_button(button, titlebar, maximized, handler.clone(), theme, cx)
                .into_any_element()
        })
        .collect()
}

fn client_slot(content: AnyElement) -> impl IntoElement {
    div()
        .h_full()
        .flex()
        .items_center()
        .window_control_area(WindowControlArea::Client)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .child(content)
}

fn control_button(
    button: WindowButton,
    titlebar: &Ident,
    maximized: bool,
    handler: EventHandler,
    theme: &Theme,
    cx: &mut App,
) -> impl IntoElement {
    let ident = titlebar.child(button.id());
    let (event, area, name) = match button {
        WindowButton::Minimize => (
            DesktopTitlebarEvent::Minimize,
            WindowControlArea::Min,
            cx.strings().text(StringKey::WindowMinimize),
        ),
        WindowButton::Maximize if maximized => (
            DesktopTitlebarEvent::ToggleMaximize,
            WindowControlArea::Max,
            cx.strings().text(StringKey::WindowRestore),
        ),
        WindowButton::Maximize => (
            DesktopTitlebarEvent::ToggleMaximize,
            WindowControlArea::Max,
            cx.strings().text(StringKey::WindowMaximize),
        ),
        WindowButton::Close => (
            DesktopTitlebarEvent::Close,
            WindowControlArea::Close,
            cx.strings().text(StringKey::WindowClose),
        ),
    };
    let hover = if button == WindowButton::Close {
        theme.colors.danger.opacity(0.84)
    } else {
        theme.colors.text.opacity(0.10)
    };
    let glyph = control_glyph(button, maximized, theme);
    let click_handler = handler.clone();
    let key_handler = handler;

    div()
        .id(ident.element_id())
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .w(px(CONTROL_WIDTH))
        .h_full()
        .flex_none()
        .cursor_pointer()
        .tab_index(0)
        .role(gpui::Role::Button)
        .focus_ring(theme)
        .pressable(cx)
        .hover(move |style| style.bg(hover))
        .window_control_area(area)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            click_handler(event, window, cx);
            cx.stop_propagation();
        })
        .on_key_down(move |key, window, cx| {
            if matches!(key.keystroke.key.as_str(), "enter" | "space") {
                key_handler(event, window, cx);
                cx.stop_propagation();
            }
        })
        .child(glyph)
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Button)
                .parent(titlebar.semantic_id())
                .text(name),
        )
}

fn control_glyph(button: WindowButton, maximized: bool, theme: &Theme) -> AnyElement {
    let color = theme.colors.text;
    match button {
        WindowButton::Minimize => div()
            .w(px(GLYPH_SIZE))
            .h(px(theme.borders.hairline.max(1.0)))
            .bg(color)
            .into_any_element(),
        WindowButton::Maximize if maximized => div()
            .relative()
            .size(px(GLYPH_SIZE + 2.0))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .size(px(GLYPH_SIZE - 2.0))
                    .border(px(theme.borders.hairline))
                    .border_color(color),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .size(px(GLYPH_SIZE - 2.0))
                    .border(px(theme.borders.hairline))
                    .border_color(color),
            )
            .into_any_element(),
        WindowButton::Maximize => div()
            .size(px(GLYPH_SIZE))
            .border(px(theme.borders.hairline))
            .border_color(color)
            .into_any_element(),
        WindowButton::Close => icon(Icon::Close)
            .size(px(theme.control.xs.icon_size))
            .text_color(color)
            .into_any_element(),
    }
}
