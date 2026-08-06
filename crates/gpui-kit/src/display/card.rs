use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface};

use crate::foundation::{FocusRing, HoverLift, Ident, Pressable, Selectable, StyledExt};

type ClickHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A bordered panel that groups related rows or content.
#[derive(IntoElement)]
pub struct Card {
    ident: Option<Ident>,
    children: Vec<AnyElement>,
    padded: bool,
    on_click: Option<ClickHandler>,
}

impl std::fmt::Debug for Card {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Card")
            .field("ident", &self.ident)
            .field("children", &self.children.len())
            .finish()
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl Card {
    pub fn new() -> Self {
        Self {
            ident: None,
            children: Vec::new(),
            padded: false,
            on_click: None,
        }
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    /// Adds interior padding. Row-based cards leave this off so dividers can
    /// reach the card edge.
    pub fn padded(mut self, padded: bool) -> Self {
        self.padded = padded;
        self
    }

    /// Makes the whole card one action.
    ///
    /// Only a card that carries an identity can be one: an action nothing can
    /// address is an action no test and no reader can reach, so the handler is
    /// ignored without [`Card::id`].
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    fn actionable(&self) -> bool {
        self.ident.is_some() && self.on_click.is_some()
    }
}

impl ParentElement for Card {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let actionable = self.actionable();
        let frame = div()
            .w_full()
            .radius(&theme, Radius::Card)
            .hairline(&theme)
            .surface(&theme, Surface::Panel)
            .overflow_hidden()
            .column()
            .when(self.padded, |element| element.p_token(&theme, Space::Lg))
            .children(self.children);

        let Some(ident) = self.ident else {
            return frame.into_any_element();
        };
        if !actionable {
            return frame
                .semantic_in(cx, NodeSpec::new(ident.semantic_id(), Role::Group))
                .into_any_element();
        }

        // A card is a surface, so it is the one place in the library where
        // rising off the page reads as a response rather than as a component
        // climbing out of its own frame.
        let mut card = frame
            .id(ident.element_id())
            .cursor_pointer()
            .tab_index(0)
            .focus_ring(&theme)
            .hover_lift(cx)
            .pressable(cx);
        let handler = self.on_click.clone().expect("an actionable card has one");
        let click = Rc::clone(&handler);
        card.interactivity()
            .on_click(move |_, window, cx| click(window, cx));
        card.interactivity().on_key_down(move |event, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                handler(window, cx);
                cx.stop_propagation();
            }
        });

        card.semantic_in(cx, NodeSpec::new(ident.semantic_id(), Role::Button))
            .into_any_element()
    }
}

/// One row inside a [`Card`].
#[derive(IntoElement)]
pub struct ListRow {
    ident: Option<Ident>,
    first: bool,
    selected: bool,
    children: Vec<AnyElement>,
    on_click: Option<ClickHandler>,
}

impl std::fmt::Debug for ListRow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ListRow")
            .field("ident", &self.ident)
            .field("first", &self.first)
            .field("selected", &self.selected)
            .finish()
    }
}

impl Default for ListRow {
    fn default() -> Self {
        Self::new()
    }
}

impl ListRow {
    pub fn new() -> Self {
        Self {
            ident: None,
            first: false,
            selected: false,
            children: Vec::new(),
            on_click: None,
        }
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    /// Suppresses the top divider on the first row of a card.
    pub fn first(mut self, first: bool) -> Self {
        self.first = first;
        self
    }

    /// Makes the row one action, which it can only be once it has an identity.
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    fn actionable(&self) -> bool {
        self.ident.is_some() && self.on_click.is_some()
    }
}

impl Selectable for ListRow {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl ParentElement for ListRow {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ListRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let selected = self.selected;
        let actionable = self.actionable();
        let row = div()
            .w_full()
            .px(px(theme.spacing.lg + theme.spacing.xs))
            .py(px(theme.spacing.md + 2.0))
            .when(!self.first, |element| {
                element
                    .border_t(px(theme.borders.hairline))
                    .border_color(theme.colors.hairline)
            })
            .when(selected, |element| element.bg(theme.colors.selected))
            .when(!selected, |element| {
                element.hover(|style| style.bg(theme.colors.hover.opacity(0.3)))
            })
            .row()
            .gap(px(theme.spacing.md + 2.0))
            .children(self.children);

        let Some(ident) = self.ident else {
            return row.into_any_element();
        };
        let spec = NodeSpec::new(ident.semantic_id(), Role::Row).selected(selected);
        if !actionable {
            return row.semantic_in(cx, spec).into_any_element();
        }

        // A row lives inside a card's frame, so it takes the press response
        // and not the lift: a row that rose would leave the frame it belongs
        // to.
        let mut row = row
            .id(ident.element_id())
            .cursor_pointer()
            .tab_index(0)
            .focus_ring(&theme)
            .pressable(cx);
        let handler = self.on_click.clone().expect("an actionable row has one");
        let click = Rc::clone(&handler);
        row.interactivity()
            .on_click(move |_, window, cx| click(window, cx));
        row.interactivity().on_key_down(move |event, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                handler(window, cx);
                cx.stop_propagation();
            }
        });
        row.semantic_in(cx, spec).into_any_element()
    }
}
