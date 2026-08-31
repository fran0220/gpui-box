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
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, TypeScale};

use crate::controls::button::{ButtonVariant, IconButton};
use crate::display::badge::{Badge, Tone};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt};
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type CardHandler = Rc<dyn Fn(&KanbanCard, &mut Window, &mut App)>;
type MoveHandler = Rc<dyn Fn(&KanbanCard, SharedString, &mut Window, &mut App)>;
type AddHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// How tall a lane is before its cards decide, so three columns holding one,
/// three and no cards still stand on the same baseline.
const LANE_ROWS: f32 = 4.0;

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
    /// How many cards the host says belong here at once, when it says so.
    pub limit: Option<usize>,
}

impl KanbanColumn {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            limit: None,
        }
    }

    /// The work-in-progress limit the host keeps for this column.
    ///
    /// The board enforces nothing: it states the count against the limit and
    /// says when the column is over it, which is the whole point of a limit a
    /// reader can see.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// How the board was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KanbanState {
    Ready,
    Empty,
    Unavailable(SharedString),
}

impl KanbanState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Empty => "empty",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

impl HasPhase for KanbanState {
    fn phase(&self) -> Phase {
        match self {
            Self::Ready => Phase::Ready,
            Self::Empty => Phase::Empty,
            Self::Unavailable(_) => Phase::Unavailable,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(reason) => Some(reason.as_ref()),
            _ => None,
        }
    }
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
    on_add: Option<AddHandler>,
    slots: Slots,
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
            on_add: None,
            slots: Slots::default(),
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

    /// What starting a card in a column does. Without it no column offers to
    /// start one, because a control that cannot act is not drawn as one.
    pub fn on_add(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_add = Some(Rc::new(handler));
        self
    }
}

impl Slotted for KanbanBoard {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl Disableable for KanbanBoard {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for KanbanBoard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        // A board that cannot show its columns is still a board. Drawn on the
        // canvas instead, the empty and unavailable states are two paragraphs
        // floating where a board was, with nothing to say how much of the
        // surface they stand for.
        let board_frame = |content| {
            div()
                .w_full()
                .min_h(px(theme.control.md.height * LANE_ROWS))
                .flex()
                .items_center()
                .justify_center()
                .p_token(&theme, Space::Md)
                .radius(&theme, Radius::Card)
                .surface(&theme, Surface::Panel)
                .child(content)
                .into_any_element()
        };
        match &self.state {
            KanbanState::Empty => {
                let inner = self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        self.ident.child("empty"),
                        cx.strings().text(StringKey::KanbanEmpty),
                    )
                    .kind(EmptyKind::Empty)
                    .into_any_element()
                });
                board_frame(inner)
            }
            KanbanState::Unavailable(reason) => {
                let inner = self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        self.ident.child("unavailable"),
                        cx.strings().text(StringKey::KanbanUnavailable),
                    )
                    .kind(EmptyKind::Unavailable)
                    .detail(reason.clone())
                    .into_any_element()
                });
                board_frame(inner)
            }
            KanbanState::Ready => {
                let held = self.held.clone();
                let carried = held
                    .as_ref()
                    .and_then(|id| self.cards.iter().find(|card| &card.id == id))
                    .cloned();
                let columns = self
                    .columns
                    .iter()
                    .enumerate()
                    .map(|(order, column)| {
                        // A column's place in the board is what it is known
                        // by here: the host names no colour for a column, and
                        // a board that took one from the title would recolour
                        // itself when somebody renamed a lane.
                        let tint = theme.colors.sequence.get(order);
                        let count = self
                            .cards
                            .iter()
                            .filter(|card| card.column == column.id)
                            .count();
                        let over = column.limit.is_some_and(|limit| count > limit);
                        let cards = self
                            .cards
                            .iter()
                            .filter(|card| card.column == column.id)
                            .map(|card| {
                                let interactive = self.on_card.is_some();
                                let handler = self.on_card.clone().filter(|_| !self.disabled);
                                let lifted = held.as_ref() == Some(&card.id);
                                let mut tile = div()
                                    .id(self
                                        .ident
                                        .child("card")
                                        .child(card.id.as_ref())
                                        .element_id())
                                    .column()
                                    .gap(px(theme.space(Space::Xs)))
                                    .p_token(&theme, Space::Sm)
                                    .radius(&theme, Radius::Control)
                                    .surface(&theme, Surface::Raised)
                                    // A card is a thing on the lane, not a
                                    // region of it. Flat, a column of them
                                    // read as one striped panel and nothing
                                    // said which rectangle a reader could
                                    // pick up.
                                    .elevation(&theme, Elevation::Raised)
                                    // The card the reader is carrying stays
                                    // where the host still says it is and
                                    // says so by receding, not by leaving.
                                    .when(lifted, |tile| tile.opacity(theme.opacity.muted))
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
                                    });
                                if let Some(handler) = handler {
                                    let click_handler = Rc::clone(&handler);
                                    let click_card = card.clone();
                                    let key_card = card.clone();
                                    tile = tile
                                        .cursor_pointer()
                                        .tab_index(0)
                                        .focus_ring(&theme)
                                        .pressable(cx)
                                        .on_click(move |_, window, cx| {
                                            click_handler(&click_card, window, cx)
                                        })
                                        .on_key_down(move |event, window, cx| {
                                            if matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            ) {
                                                handler(&key_card, window, cx);
                                                cx.stop_propagation();
                                            }
                                        });
                                }
                                tile.semantic_in(
                                    cx,
                                    NodeSpec::new(
                                        self.ident
                                            .child("card")
                                            .child(card.id.as_ref())
                                            .semantic_id(),
                                        if interactive {
                                            Role::Button
                                        } else {
                                            Role::Group
                                        },
                                    )
                                    .parent(
                                        self.ident
                                            .child("column")
                                            .child(column.id.as_ref())
                                            .semantic_id(),
                                    )
                                    .text(card.title.clone())
                                    .value(card.column.clone())
                                    .disabled(interactive && self.disabled),
                                )
                            })
                            .collect::<Vec<_>>();
                        let empty = cards.is_empty();
                        let tally = match column.limit {
                            Some(limit) => cx.numbers().count_of_total(count, limit),
                            None => cx.numbers().count(count),
                        };
                        let adding =
                            self.on_add
                                .clone()
                                .filter(|_| !self.disabled)
                                .map(|handler| {
                                    let column_id = column.id.clone();
                                    IconButton::new(
                                        self.ident.child("add").child(column.id.as_ref()),
                                        Icon::Plus,
                                        cx.strings()
                                            .format(StringKey::KanbanAdd, &[column.title.as_ref()]),
                                    )
                                    .variant(ButtonVariant::Ghost)
                                    .control_size(ControlSize::Sm)
                                    .on_click(
                                        move |window, cx| handler(column_id.clone(), window, cx),
                                    )
                                });
                        // The lane the reader would drop into says where the
                        // card would land, rather than leaving the whole
                        // column as an invisible target.
                        let landing = carried
                            .as_ref()
                            .filter(|card| card.column != column.id)
                            .filter(|_| !self.disabled)
                            .zip(self.on_move.clone())
                            .map(|(card, handler)| {
                                let ident = self.ident.child("move").child(column.id.as_ref());
                                let parent = self
                                    .ident
                                    .child("column")
                                    .child(column.id.as_ref())
                                    .semantic_id();
                                let label = cx.strings().format(
                                    StringKey::KanbanMoveHere,
                                    &[card.title.as_ref(), column.title.as_ref()],
                                );
                                let click_handler = Rc::clone(&handler);
                                let click_card = card.clone();
                                let key_card = card.clone();
                                let click_column = column.id.clone();
                                let key_column = column.id.clone();
                                div()
                                    .id(ident.element_id())
                                    .w_full()
                                    .p_token(&theme, Space::Sm)
                                    .radius(&theme, Radius::Control)
                                    .border(px(theme.borders.hairline))
                                    .border_dashed()
                                    .border_color(
                                        theme
                                            .colors
                                            .accent
                                            .opacity(theme.effects.semantic_wash_strong_alpha),
                                    )
                                    .bg(theme.color_wash(
                                        theme.colors.accent,
                                        gpui_kit_theme::SemanticWash::Faint,
                                    ))
                                    .shadow(theme.glow(theme.colors.accent))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .type_scale(&theme, TypeScale::Caption)
                                    .text_color(theme.colors.text_muted)
                                    .cursor_pointer()
                                    .tab_index(0)
                                    .focus_ring(&theme)
                                    .pressable(cx)
                                    .child(label.clone())
                                    .on_click(move |_, window, cx| {
                                        click_handler(&click_card, click_column.clone(), window, cx)
                                    })
                                    .on_key_down(move |event, window, cx| {
                                        if matches!(event.keystroke.key.as_str(), "enter" | "space")
                                        {
                                            handler(&key_card, key_column.clone(), window, cx);
                                            cx.stop_propagation();
                                        }
                                    })
                                    .semantic_in(
                                        cx,
                                        NodeSpec::new(ident.semantic_id(), Role::Button)
                                            .parent(parent)
                                            .text(label),
                                    )
                            });
                        div()
                            .id(self
                                .ident
                                .child("column")
                                .child(column.id.as_ref())
                                .element_id())
                            .flex_1()
                            .min_w(px(theme.control.md.height * LANE_ROWS))
                            .min_h(px(theme.control.md.height * LANE_ROWS))
                            .column()
                            .gap_token(&theme, Space::Xs)
                            .p_token(&theme, Space::Sm)
                            .radius(&theme, Radius::Card)
                            .surface(&theme, Surface::Panel)
                            .child(
                                div()
                                    .row()
                                    .items_center()
                                    .gap_token(&theme, Space::Xs)
                                    // The lane's own mark, at the size of the
                                    // line it names. Three panels of the same
                                    // grey side by side said only that the
                                    // board had three of something.
                                    .child(
                                        div()
                                            .flex_none()
                                            .w(px(theme.effects.rail_width))
                                            .h(px(theme.typography.caption.line_height))
                                            .rounded_full()
                                            .bg(tint),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .type_scale(&theme, TypeScale::Caption)
                                            .text_color(theme.colors.text)
                                            .child(column.title.clone()),
                                    )
                                    .child(Badge::new(tally.clone()).tone(if over {
                                        Tone::Warning
                                    } else {
                                        Tone::Neutral
                                    }))
                                    .children(adding),
                            )
                            .when(over, |lane| {
                                lane.child(
                                    div()
                                        .type_scale(&theme, TypeScale::Caption)
                                        .text_color(theme.colors.warning)
                                        .child(cx.strings().format(
                                            StringKey::KanbanOverLimit,
                                            &[column.title.as_ref()],
                                        )),
                                )
                            })
                            .children(cards)
                            .when(empty && landing.is_none(), |lane| {
                                lane.child(
                                    div()
                                        .type_scale(&theme, TypeScale::Caption)
                                        .text_color(theme.colors.text_faint)
                                        .child(cx.strings().text(StringKey::KanbanColumnEmpty)),
                                )
                            })
                            .children(landing)
                            .semantic_in(
                                cx,
                                NodeSpec::new(
                                    self.ident
                                        .child("column")
                                        .child(column.id.as_ref())
                                        .semantic_id(),
                                    Role::List,
                                )
                                .text(column.title.clone())
                                .value(tally),
                            )
                    })
                    .collect::<Vec<_>>();
                div()
                    .row()
                    .items_stretch()
                    .gap_token(&theme, Space::Sm)
                    .w_full()
                    .children(columns)
                    .semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::Group))
                    .into_any_element()
            }
        }
    }
}

#[cfg(test)]
mod kanban_phase_tests {
    use super::*;

    #[test]
    fn unavailable_is_not_empty() {
        let state = KanbanState::Unavailable("offline".into());
        assert_eq!(state.phase(), Phase::Unavailable);
        assert_eq!(state.name(), "unavailable");
        assert_eq!(state.reason(), Some("offline"));
    }
}
