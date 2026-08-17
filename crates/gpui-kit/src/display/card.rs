use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TextTone, TypeScale};

use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{
    Disableable, FocusRing, HoverLift, Ident, Pressable, Selectable, StyledExt, text,
};

type ClickHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type SlotRender = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

pub use crate::foundation::CardVariant;

/// The titled top of a card.
///
/// The words are the caller's. A card is a container and has no opinion about
/// what is being contained, so it never authors a heading of its own.
pub struct CardHeader {
    title: SharedString,
    subtitle: Option<SharedString>,
    action: Option<SlotRender>,
}

impl std::fmt::Debug for CardHeader {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CardHeader")
            .field("title", &self.title)
            .field("subtitle", &self.subtitle)
            .field("has_action", &self.action.is_some())
            .finish()
    }
}

impl CardHeader {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            action: None,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// A control in the heading, such as a menu or a refresh button.
    ///
    /// A card whose whole surface is one action does not render this: two
    /// targets in one target is a click whose outcome depends on a pixel.
    pub fn action(
        mut self,
        action: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.action = Some(Rc::new(action));
        self
    }
}

/// A raised panel that groups related rows or content.
#[derive(IntoElement)]
pub struct Card {
    ident: Option<Ident>,
    name: Option<SharedString>,
    variant: CardVariant,
    header: Option<CardHeader>,
    media: Option<SlotRender>,
    footer: Option<SlotRender>,
    children: Vec<AnyElement>,
    divided: bool,
    padding: Option<Space>,
    selected: bool,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl std::fmt::Debug for Card {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Card")
            .field("ident", &self.ident)
            .field("variant", &self.variant)
            .field("header", &self.header)
            .field("has_media", &self.media.is_some())
            .field("has_footer", &self.footer.is_some())
            .field("children", &self.children.len())
            .field("divided", &self.divided)
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
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
            name: None,
            variant: CardVariant::default(),
            header: None,
            media: None,
            footer: None,
            children: Vec::new(),
            divided: false,
            padding: None,
            selected: false,
            disabled: false,
            on_click: None,
        }
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    /// What a card that is one action is called.
    ///
    /// A card holding a [`CardHeader`] is already named by its title, so this
    /// is for the card that is a picture, a chart or a set of rows and would
    /// otherwise be an action with nothing to announce.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn variant(mut self, variant: CardVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn header(mut self, header: CardHeader) -> Self {
        self.header = Some(header);
        self
    }

    /// An image, chart, or preview across the top, flush to the card's edges.
    ///
    /// The card clips it to its own radius and neither fetches nor decodes
    /// whatever it is: the caller renders it.
    pub fn media(mut self, media: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> Self {
        self.media = Some(Rc::new(media));
        self
    }

    /// A closing region, usually the actions that apply to the card.
    pub fn footer(
        mut self,
        footer: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.footer = Some(Rc::new(footer));
        self
    }

    /// Draws a hairline between every adjacent region and between body rows.
    ///
    /// This is the one place the library draws a line that is not focus,
    /// invalidity or a drop target, and it is a line between two pieces of
    /// content on one surface rather than a line around the surface. A list
    /// of rows and a footer of actions want it; a card holding one paragraph
    /// does not, which is why it is off by default.
    pub fn divided(mut self, divided: bool) -> Self {
        self.divided = divided;
        self
    }

    /// Adds interior padding. Row-based cards leave this off so a row's own
    /// hover wash can reach the card edge.
    ///
    /// A header, a footer and a caller's media are laid out by the card
    /// rather than by its children, so they carry their own padding either
    /// way and this setting does not reach them.
    pub fn padded(self, padded: bool) -> Self {
        self.padding(padded.then_some(Space::Lg))
    }

    /// The interior padding, when the default step is the wrong one.
    pub fn padding(mut self, padding: impl Into<Option<Space>>) -> Self {
        self.padding = padding.into();
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
        self.ident.is_some() && self.on_click.is_some() && !self.disabled
    }
}

impl Selectable for Card {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl Disableable for Card {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl ParentElement for Card {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Card {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let actionable = self.actionable();
        let disabled = self.disabled;
        let divided = self.divided;
        let announced = self
            .name
            .clone()
            .or_else(|| self.header.as_ref().map(|header| header.title.clone()));

        let rule = || {
            div()
                .w_full()
                .h(px(theme.borders.hairline))
                .flex_none()
                .bg(theme.colors.divider)
        };

        // A card that is one action renders no second target inside itself.
        let header = self.header.map(|header| {
            let title_tone = if disabled {
                TextTone::Disabled
            } else {
                TextTone::Primary
            };
            let subtitle_tone = if disabled {
                TextTone::Disabled
            } else {
                TextTone::Muted
            };
            div()
                .row_reading(direction)
                .w_full()
                .p_token(&theme, Space::Lg)
                .gap_token(&theme, Space::Sm)
                .child(
                    div()
                        .column()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(
                            text(&theme, TypeScale::Subtitle, header.title)
                                .text_tone(&theme, title_tone),
                        )
                        .children(header.subtitle.map(|subtitle| {
                            text(&theme, TypeScale::Caption, subtitle)
                                .text_tone(&theme, subtitle_tone)
                        })),
                )
                .children(
                    header
                        .action
                        .filter(|_| !actionable && !disabled)
                        .map(|action| action(window, cx)),
                )
        });

        let media = self.media.map(|media| {
            div()
                .w_full()
                .flex_none()
                .overflow_hidden()
                .child(media(window, cx))
        });

        let leads = header.is_some() || media.is_some();
        let trails = self.footer.is_some();
        let body = (!self.children.is_empty()).then(|| {
            // A region above or below already put space there, unless a rule
            // divides them, in which case both sides want their own margin.
            let mut body = div().w_full().column().when_some(
                self.padding.map(|padding| theme.space(padding)),
                |element, padding| {
                    element
                        .px(px(padding))
                        .pt(px(if leads && !divided { 0.0 } else { padding }))
                        .pb(px(if trails && !divided { 0.0 } else { padding }))
                },
            );
            for (index, child) in self.children.into_iter().enumerate() {
                if divided && index > 0 {
                    body = body.child(rule());
                }
                body = body.child(child);
            }
            body
        });

        let footer = self.footer.map(|footer| {
            div()
                .w_full()
                .flex_none()
                .p_token(&theme, Space::Lg)
                .child(footer(window, cx))
        });

        let regions: Vec<AnyElement> = [
            media.map(IntoElement::into_any_element),
            header.map(IntoElement::into_any_element),
            body.map(IntoElement::into_any_element),
            footer.map(IntoElement::into_any_element),
        ]
        .into_iter()
        .flatten()
        .collect();

        let mut frame = div()
            .w_full()
            .overflow_hidden()
            .column()
            .card_surface(&theme, self.variant);
        if self.selected {
            frame = frame.shadow(theme.selected_ring());
        }
        for (index, region) in regions.into_iter().enumerate() {
            if divided && index > 0 {
                frame = frame.child(rule());
            }
            frame = frame.child(region);
        }

        let Some(ident) = self.ident else {
            return frame.into_any_element();
        };
        let spec = NodeSpec::new(ident.semantic_id(), Role::Group)
            .selected(self.selected)
            .disabled(disabled);
        if !actionable {
            return frame.semantic_in(cx, spec).into_any_element();
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

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Button)
            .selected(self.selected)
            .disabled(disabled);
        if let Some(announced) = announced {
            spec = spec.text(announced);
        }
        card.semantic_in(cx, spec).into_any_element()
    }
}

/// One row inside a [`Card`].
#[derive(IntoElement)]
pub struct ListRow {
    ident: Option<Ident>,
    selected: bool,
    disabled: bool,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
    children: Vec<AnyElement>,
    on_click: Option<ClickHandler>,
}

impl std::fmt::Debug for ListRow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ListRow")
            .field("ident", &self.ident)
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .field("has_leading", &self.leading.is_some())
            .field("has_trailing", &self.trailing.is_some())
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
            selected: false,
            disabled: false,
            leading: None,
            trailing: None,
            children: Vec::new(),
            on_click: None,
        }
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    /// The slot at the reading start: an icon, an avatar, a status dot.
    ///
    /// It is a slot rather than a first child so that rows with and without
    /// one still line their text up, which a list of them needs and a bare
    /// child order cannot promise.
    pub fn leading(mut self, leading: impl IntoElement) -> Self {
        self.leading = Some(leading.into_any_element());
        self
    }

    /// The slot at the reading end: a badge, a value, a disclosure.
    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }

    /// Makes the row one action, which it can only be once it has an identity.
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    fn actionable(&self) -> bool {
        self.ident.is_some() && self.on_click.is_some() && !self.disabled
    }
}

impl Selectable for ListRow {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl Disableable for ListRow {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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
        let direction = cx.layout_direction();
        let selected = self.selected;
        let disabled = self.disabled;
        let actionable = self.actionable();
        let row = div()
            .w_full()
            .px(px(theme.spacing.lg + theme.spacing.xs))
            .py(px(theme.spacing.md + 2.0))
            .when(selected, |element| element.bg(theme.colors.selected))
            .when(!selected && !disabled, |element| {
                element.hover(|style| style.bg(theme.colors.hover.opacity(0.3)))
            })
            // A disabled row states that it is unavailable and stays legible
            // doing it: the text tone changes and the content does not fade
            // out from under the reader.
            .when(disabled, |element| {
                element.text_tone(&theme, TextTone::Disabled)
            })
            .row_reading(direction)
            .gap(px(theme.spacing.md + 2.0))
            .children(self.leading)
            .children(self.children)
            .children(self.trailing);

        let Some(ident) = self.ident else {
            return row.into_any_element();
        };
        let spec = NodeSpec::new(ident.semantic_id(), Role::Row)
            .selected(selected)
            .disabled(disabled);
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
