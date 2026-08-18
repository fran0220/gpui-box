//! A caller-owned board of columns and cards.
//!
//! Columns, cards, and their order are the host's. Clicking a card reports
//! its identity; dropping a card onto a column reports a move the host may
//! accept or refuse.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, TypeScale};

use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::{Disableable, Ident, StyledExt};
use crate::strings::{ActiveStrings, StringKey};

type CardHandler = Rc<dyn Fn(&KanbanCard, &mut Window, &mut App)>;
type MoveHandler = Rc<dyn Fn(&KanbanCard, SharedString, &mut Window, &mut App)>;

/// One card on the board. Identity is the host's.
#[derive(Debug, Clone, PartialEq)]
pub struct KanbanCard {
    pub id: SharedString,
    pub title: SharedString,
    pub detail: SharedString,
    pub column: SharedString,
}

impl KanbanCard {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        column: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: SharedString::default(),
            column: column.into(),
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = detail.into();
        self
    }
}

/// One named column. Identity is the host's.
#[derive(Debug, Clone, PartialEq)]
pub struct KanbanColumn {
    pub id: SharedString,
    pub title: SharedString,
}

impl KanbanColumn {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
        }
    }
}

/// How the board was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KanbanState {
    Ready,
    Empty,
    Unavailable(SharedString),
}

/// What the board reported.
#[derive(Debug, Clone, PartialEq)]
pub enum KanbanEvent {
    CardActivated(SharedString),
    CardMoved {
        card: SharedString,
        column: SharedString,
    },
}

/// A board of host-owned columns and cards.
#[derive(IntoElement)]
pub struct KanbanBoard {
    ident: Ident,
    columns: Vec<KanbanColumn>,
    cards: Vec<KanbanCard>,
    state: KanbanState,
    held: Option<SharedString>,
    disabled: bool,
    on_card: Option<CardHandler>,
    on_move: Option<MoveHandler>,
}

impl KanbanBoard {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            columns: Vec::new(),
            cards: Vec::new(),
            state: KanbanState::Ready,
            held: None,
            disabled: false,
            on_card: None,
            on_move: None,
        }
    }

    /// The card the host is currently holding, so a column click can report a move.
    pub fn held(mut self, card: impl Into<SharedString>) -> Self {
        self.held = Some(card.into());
        self
    }

    pub fn columns(mut self, columns: impl IntoIterator<Item = KanbanColumn>) -> Self {
        self.columns = columns.into_iter().collect();
        self
    }

    pub fn cards(mut self, cards: impl IntoIterator<Item = KanbanCard>) -> Self {
        self.cards = cards.into_iter().collect();
        self
    }

    pub fn state(mut self, state: KanbanState) -> Self {
        self.state = state;
        self
    }

    pub fn on_card(
        mut self,
        handler: impl Fn(&KanbanCard, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_card = Some(Rc::new(handler));
        self
    }

    pub fn on_move(
        mut self,
        handler: impl Fn(&KanbanCard, SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_move = Some(Rc::new(handler));
        self
    }
}

impl Disableable for KanbanBoard {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for KanbanBoard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        match &self.state {
            KanbanState::Empty => EmptyState::new(
                self.ident.child("empty"),
                cx.strings().text(StringKey::KanbanEmpty),
            )
            .kind(EmptyKind::Empty)
            .into_any_element(),
            KanbanState::Unavailable(reason) => EmptyState::new(
                self.ident.child("unavailable"),
                cx.strings().text(StringKey::KanbanUnavailable),
            )
            .kind(EmptyKind::Unavailable)
            .detail(reason.clone())
            .into_any_element(),
            KanbanState::Ready => {
                let held = self.held.clone();
                let columns = self
                    .columns
                    .iter()
                    .map(|column| {
                        let cards = self
                            .cards
                            .iter()
                            .filter(|card| card.column == column.id)
                            .map(|card| {
                                let handler = self.on_card.clone().filter(|_| !self.disabled);
                                let mut tile = div()
                                    .id(self
                                        .ident
                                        .child("card")
                                        .child(card.id.as_ref())
                                        .element_id())
                                    .column()
                                    .gap(px(2.0))
                                    .p_token(&theme, Space::Sm)
                                    .radius(&theme, Radius::Small)
                                    .surface(&theme, Surface::Raised)
                                    .child(
                                        div()
                                            .type_scale(&theme, TypeScale::Label)
                                            .child(card.title.clone()),
                                    )
                                    .when(!card.detail.is_empty(), |tile| {
                                        tile.child(
                                            div()
                                                .type_scale(&theme, TypeScale::Caption)
                                                .text_color(theme.colors.text_muted)
                                                .child(card.detail.clone()),
                                        )
                                    })
                                    .semantic_in(
                                        cx,
                                        NodeSpec::new(
                                            self.ident
                                                .child("card")
                                                .child(card.id.as_ref())
                                                .semantic_id(),
                                            Role::Button,
                                        )
                                        .text(card.title.clone())
                                        .value(card.column.clone()),
                                    );
                                if let Some(handler) = handler {
                                    let card = card.clone();
                                    tile = tile
                                        .on_click(move |_, window, cx| handler(&card, window, cx));
                                }
                                tile
                            })
                            .collect::<Vec<_>>();
                        let mut lane = div()
                            .id(self
                                .ident
                                .child("column")
                                .child(column.id.as_ref())
                                .element_id())
                            .flex_1()
                            .min_w(px(160.0))
                            .column()
                            .gap_token(&theme, Space::Xs)
                            .p_token(&theme, Space::Sm)
                            .radius(&theme, Radius::Card)
                            .surface(&theme, Surface::Panel)
                            .child(
                                div()
                                    .type_scale(&theme, TypeScale::Caption)
                                    .text_color(theme.colors.text_muted)
                                    .child(column.title.clone()),
                            )
                            .children(cards)
                            .semantic_in(
                                cx,
                                NodeSpec::new(
                                    self.ident
                                        .child("column")
                                        .child(column.id.as_ref())
                                        .semantic_id(),
                                    Role::List,
                                )
                                .text(column.title.clone()),
                            );
                        if let (Some(handler), Some(held_id)) = (
                            self.on_move.clone().filter(|_| !self.disabled),
                            held.clone(),
                        ) && let Some(card) =
                            self.cards.iter().find(|card| card.id == held_id).cloned()
                        {
                            let column_id = column.id.clone();
                            lane = lane.on_click(move |_, window, cx| {
                                handler(&card, column_id.clone(), window, cx);
                            });
                        }
                        lane
                    })
                    .collect::<Vec<_>>();
                div()
                    .row()
                    .items_start()
                    .gap_token(&theme, Space::Sm)
                    .w_full()
                    .children(columns)
                    .semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::Group))
                    .into_any_element()
            }
        }
    }
}
