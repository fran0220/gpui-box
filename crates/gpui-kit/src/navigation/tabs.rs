//! A strip of tabs that reports which one was chosen.
//!
//! The selected tab is caller-owned. `Tabs` reports the id that was picked and
//! renders whatever the caller says is current, so a host that refuses a move
//! keeps the tab that still holds underlined.
//!
//! `Tabs` renders the strip only. The body belongs to the caller, which is why
//! no `Role::TabPanel` node is published here.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlMetrics, ControlSize, Space, Theme};

use crate::display::badge::Badge;
use crate::foundation::stepping::bounded_step;
use crate::foundation::{Disableable, FocusRing, Ident, Sizable, StyledExt};

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// One tab, identified by business identity rather than by position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabItem {
    pub id: SharedString,
    pub label: SharedString,
    pub icon: Option<Icon>,
    pub badge: Option<SharedString>,
    pub disabled: bool,
}

impl TabItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            badge: None,
            disabled: false,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// A count shown next to the label, such as how many items the tab holds.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A row of tabs. The strip publishes one [`Role::Tab`] node per tab.
#[derive(IntoElement)]
pub struct Tabs {
    ident: Ident,
    tabs: Vec<TabItem>,
    selected: Option<SharedString>,
    size: ControlSize,
    disabled: bool,
    on_select: Option<SelectHandler>,
}

impl std::fmt::Debug for Tabs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Tabs")
            .field("ident", &self.ident)
            .field("tabs", &self.tabs.len())
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_select.is_some())
            .finish()
    }
}

impl Tabs {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            tabs: Vec::new(),
            selected: None,
            size: ControlSize::Md,
            disabled: false,
            on_select: None,
        }
    }

    pub fn tab(mut self, tab: TabItem) -> Self {
        self.tabs.push(tab);
        self
    }

    pub fn tabs(mut self, tabs: impl IntoIterator<Item = TabItem>) -> Self {
        self.tabs.extend(tabs);
        self
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    fn tab_element(
        &self,
        tab: &TabItem,
        theme: &Theme,
        metrics: ControlMetrics,
        cx: &mut App,
    ) -> impl IntoElement {
        let selected = self.selected.as_ref() == Some(&tab.id);
        let disabled = self.disabled || tab.disabled;
        let actionable = !disabled && self.on_select.is_some();
        let ident = self.ident.child(tab.id.as_ref());
        let color = if disabled {
            theme.colors.text_faint
        } else if selected {
            theme.colors.text
        } else {
            theme.colors.text_muted
        };

        let mut element = div()
            .id(ident.element_id())
            .flex_none()
            .column()
            .child(
                div()
                    .row()
                    .h(px(metrics.height))
                    .px(px(metrics.padding_x))
                    .gap(px(metrics.gap))
                    .text_size(px(metrics.font_size))
                    .text_color(color)
                    .children(
                        tab.icon
                            .map(|glyph| icon(glyph).size(px(metrics.icon_size)).text_color(color)),
                    )
                    .child(tab.label.clone())
                    .children(tab.badge.clone().map(|badge| Badge::new(badge).neutral())),
            )
            // The underline is a sibling rather than a border so an unselected
            // tab reserves the same height and nothing shifts when it is
            // chosen.
            .child(
                div()
                    .h(px(theme.borders.thick))
                    .when(selected, |element| element.bg(theme.colors.accent)),
            )
            .when(disabled, |element| element.opacity(theme.opacity.disabled))
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .hover(|style| style.text_color(theme.colors.text))
                    .focus_ring(theme)
            });

        if let (true, Some(handler)) = (actionable, self.on_select.clone()) {
            let id = tab.id.clone();
            let click = Rc::clone(&handler);
            let clicked = id.clone();
            element = element
                .on_click(move |_, window, cx| click(clicked.clone(), window, cx))
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        handler(id.clone(), window, cx);
                        cx.stop_propagation();
                    }
                });
        }

        element.semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Tab)
                .parent(self.ident.semantic_id())
                .checked(selected)
                .disabled(disabled)
                .text(tab.label.clone()),
        )
    }
}

impl Disableable for Tabs {
    /// Refuses the whole strip. A disabled strip installs no handler at all,
    /// including the keyboard one.
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Tabs {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Tabs {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);

        let mut strip = div()
            .id(self.ident.element_id())
            .row()
            .items_end()
            .flex_wrap()
            .gap(px(theme.space(Space::Xs)))
            .border_b(px(theme.borders.hairline))
            .border_color(theme.colors.hairline);

        if let (false, Some(handler)) = (self.disabled, self.on_select.clone()) {
            let tabs = self.tabs.clone();
            let selected = self.selected.clone();
            strip = strip.on_key_down(move |event, window, cx| {
                let next = match event.keystroke.key.as_str() {
                    "left" => step(&tabs, selected.as_ref(), -1),
                    "right" => step(&tabs, selected.as_ref(), 1),
                    "home" => edge(&tabs, -1),
                    "end" => edge(&tabs, 1),
                    _ => return,
                };
                // A move that lands nowhere, or back on the tab that is
                // already current, is not a choice and is not reported.
                let Some(next) = next.filter(|next| Some(next) != selected.as_ref()) else {
                    return;
                };
                handler(next, window, cx);
                cx.stop_propagation();
            });
        }

        for tab in &self.tabs {
            strip = strip.child(self.tab_element(tab, &theme, metrics, cx));
        }

        strip.semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::List))
    }
}

/// The next tab that can be chosen in `delta`'s direction, skipping refusals.
///
/// Movement stops at the ends instead of wrapping, so arrowing past the last
/// tab reports nothing rather than jumping back to the first.
fn step(tabs: &[TabItem], selected: Option<&SharedString>, delta: isize) -> Option<SharedString> {
    let from = selected.and_then(|id| tabs.iter().position(|tab| &tab.id == id));
    bounded_step(tabs.len(), from, delta, |index| tabs[index].disabled)
        .map(|index| tabs[index].id.clone())
}

/// The first tab from the left when `delta` is negative, from the right when
/// it is positive.
fn edge(tabs: &[TabItem], delta: isize) -> Option<SharedString> {
    step(tabs, None, -delta)
}
