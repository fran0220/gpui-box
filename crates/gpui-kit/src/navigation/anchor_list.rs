//! In-page navigation over caller-owned document anchors.
//!
//! `AnchorList` reports navigation intent; it never changes the active anchor.
//! Overflow is declarative and truthful: anchors move only when both a cut and
//! a caller-owned menu are supplied.

use std::rc::Rc;

use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Surface, TypeScale};

use crate::controls::button::ButtonVariant;
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::stepping::bounded_step;
use crate::foundation::{
    Disableable, FocusRing, Hoverable, Ident, Pressable, SelectedFill, Sizable, StyledExt, text,
};
use crate::overlay::{Menu, MenuItem};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type NavigateHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// One caller-owned document destination, identified independently of order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    id: SharedString,
    label: SharedString,
    disabled: bool,
}

impl Anchor {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    fn menu_row(&self, active: bool) -> MenuItem {
        if active {
            MenuItem::check(self.id.clone(), self.label.clone(), true).disabled(self.disabled)
        } else {
            MenuItem::command(self.id.clone(), self.label.clone()).disabled(self.disabled)
        }
    }
}

/// A horizontal reading-order list of links to sections in the current page.
#[derive(IntoElement)]
pub struct AnchorList {
    ident: Ident,
    anchors: Vec<Anchor>,
    active: Option<SharedString>,
    disabled: bool,
    size: ControlSize,
    on_navigate: Option<NavigateHandler>,
    overflow_after: Option<usize>,
    overflow_menu: Option<Entity<Menu>>,
}

impl std::fmt::Debug for AnchorList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AnchorList")
            .field("ident", &self.ident)
            .field("anchors", &self.anchors.len())
            .field("active", &self.active)
            .field("disabled", &self.disabled)
            .field("overflow_after", &self.overflow_after)
            .finish()
    }
}

impl AnchorList {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            anchors: Vec::new(),
            active: None,
            disabled: false,
            size: ControlSize::Md,
            on_navigate: None,
            overflow_after: None,
            overflow_menu: None,
        }
    }

    pub fn anchor(mut self, anchor: Anchor) -> Self {
        self.anchors.push(anchor);
        self
    }

    pub fn anchors(mut self, anchors: impl IntoIterator<Item = Anchor>) -> Self {
        self.anchors.extend(anchors);
        self
    }

    pub fn active(mut self, id: impl Into<SharedString>) -> Self {
        self.active = Some(id.into());
        self
    }

    pub fn on_navigate(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_navigate = Some(Rc::new(handler));
        self
    }

    /// Keeps `count` anchors inline and moves the remainder into the supplied
    /// menu. Without a menu this declaration hides nothing.
    pub fn overflow_after(mut self, count: usize) -> Self {
        self.overflow_after = Some(count);
        self
    }

    /// Supplies the caller-owned menu that can hold overflowed anchors.
    /// The menu reports an overflowed anchor through
    /// [`crate::overlay::MenuEvent::Invoked`] with the same stable id that
    /// `on_navigate` reports for inline and keyboard navigation.
    pub fn overflow_menu(mut self, menu: Entity<Menu>) -> Self {
        self.overflow_menu = Some(menu);
        self
    }

    fn cut(&self) -> usize {
        match (
            self.disabled,
            self.overflow_after,
            self.overflow_menu.is_some(),
        ) {
            (false, Some(cut), true) => cut,
            _ => usize::MAX,
        }
    }

    fn anchor_element(&self, anchor: &Anchor, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let ident = self.ident.child(anchor.id.as_ref());
        let active = self.active.as_ref() == Some(&anchor.id);
        let disabled = self.disabled || anchor.disabled;
        let actionable = !disabled && self.on_navigate.is_some();
        let color = if disabled {
            theme.colors.text_faint
        } else if active {
            theme.colors.text
        } else {
            theme.colors.text_muted
        };

        let mut element = div()
            .id(ident.element_id())
            .flex_none()
            .h(px(metrics.height))
            .px(px(metrics.padding_x))
            .flex()
            .items_center()
            .radius(&theme, Radius::Control)
            .child(
                text(&theme, TypeScale::Label, anchor.label.clone())
                    .text_size(px(metrics.font_size))
                    .text_color(color),
            )
            .selected_fill(&theme, active)
            .when(disabled, |element| element.opacity(theme.opacity.disabled))
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .when(!active, |element| element.hover_row(&theme))
                    .focus_ring(&theme)
            });

        if let (true, Some(handler)) = (actionable, self.on_navigate.clone()) {
            let id = anchor.id.clone();
            let clicked = id.clone();
            let click = Rc::clone(&handler);
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
            NodeSpec::new(ident.semantic_id(), Role::Link)
                .parent(self.ident.semantic_id())
                .selected(active)
                .disabled(disabled)
                .text(anchor.label.clone()),
        )
    }
}

impl Disableable for AnchorList {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for AnchorList {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for AnchorList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        if self.disabled
            && let Some(menu) = self.overflow_menu.as_ref()
            && menu.read(cx).is_open()
        {
            menu.update(cx, |menu, cx| menu.close(window, cx));
        }
        // The bar is a place, not a row of loose words: the track is what says
        // where the set of anchors starts and stops, and it is what the active
        // anchor's wash is read against.
        let mut strip = div()
            .id(self.ident.element_id())
            .row_reading(direction)
            .items_center()
            // The track ends where the anchors do. Stretched to whatever holds
            // it, the groove ran on past the last section and read as a bar
            // with something missing from it.
            .self_start()
            .max_w_full()
            .flex_wrap()
            .gap(px(theme.space(Space::Xs)))
            .p(px(theme.space(Space::Xs)))
            .radius(&theme, Radius::Card)
            .surface(&theme, Surface::Sunken);

        if let (false, Some(handler)) = (self.disabled, self.on_navigate.clone()) {
            let anchors = self.anchors.clone();
            let active = self.active.clone();
            strip = strip.on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.as_str();
                let next = match direction.arrow_step(key) {
                    Some(delta) => step(&anchors, active.as_ref(), delta as isize),
                    None => match key {
                        "home" => edge(&anchors, -1),
                        "end" => edge(&anchors, 1),
                        _ => return,
                    },
                };
                let Some(next) = next.filter(|next| Some(next) != active.as_ref()) else {
                    return;
                };
                handler(next, window, cx);
                cx.stop_propagation();
            });
        }

        let cut = self.cut();
        let mut hidden = Vec::new();
        for (index, anchor) in self.anchors.iter().enumerate() {
            if index >= cut {
                hidden.push(anchor.menu_row(self.active.as_ref() == Some(&anchor.id)));
            } else {
                strip = strip.child(self.anchor_element(anchor, cx));
            }
        }

        let hidden_count = hidden.len();
        let overflow = self
            .overflow_menu
            .clone()
            .filter(|_| hidden_count > 0)
            .map(|menu| {
                if menu.read(cx).offered() != hidden.as_slice() {
                    menu.update(cx, |menu, cx| menu.set_items(hidden, cx));
                }
                // The trigger is a way to the anchors that did not fit, not a
                // more important anchor. A filled control here out-shouts the
                // one anchor on the bar that says where the reader is.
                menu.update(cx, |menu, cx| {
                    menu.set_trigger_style(ButtonVariant::Ghost, self.size, cx)
                });
                let ident = self.ident.child("overflow");
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(theme.space(Space::Sm)))
                    .child(menu)
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.semantic_id(), Role::Group)
                            .parent(self.ident.semantic_id())
                            .text(cx.strings().text(StringKey::AnchorMoreSections))
                            .value(cx.numbers().count(hidden_count)),
                    )
            });

        strip.children(overflow).semantic_in(
            cx,
            NodeSpec::new(self.ident.semantic_id(), Role::List)
                .disabled(self.disabled)
                .value(cx.numbers().count(self.anchors.len())),
        )
    }
}

fn step(anchors: &[Anchor], active: Option<&SharedString>, delta: isize) -> Option<SharedString> {
    let from = active.and_then(|id| anchors.iter().position(|anchor| &anchor.id == id));
    bounded_step(anchors.len(), from, delta, |index| anchors[index].disabled)
        .map(|index| anchors[index].id.clone())
}

fn edge(anchors: &[Anchor], delta: isize) -> Option<SharedString> {
    step(anchors, None, -delta)
}
