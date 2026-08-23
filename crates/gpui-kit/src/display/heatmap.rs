//! A density matrix over caller-owned cells.
//!
//! The host supplies every cell identity, its row and column, an optional
//! intensity on a five-step ladder, and the words a hover shows. This
//! component maps intensity onto the theme accent and distinguishes a measured
//! zero from a cell nobody observed.

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, TypeScale};

use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Ident, StyledExt};
use crate::overlay::tooltip::Tooltipped;
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

/// One observation in the matrix, or the absence of one.
#[derive(Debug, Clone, PartialEq)]
pub struct HeatCell {
    pub id: SharedString,
    pub row: SharedString,
    pub column: SharedString,
    /// `None` is no observation. `Some(0)` is a measured empty. `Some(1..=4)`
    /// is increasing density. Values above 4 clamp to the top step.
    pub level: Option<u8>,
    pub label: SharedString,
    pub value: SharedString,
}

impl HeatCell {
    pub fn new(
        id: impl Into<SharedString>,
        row: impl Into<SharedString>,
        column: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            row: row.into(),
            column: column.into(),
            level: None,
            label: SharedString::default(),
            value: SharedString::default(),
        }
    }

    pub fn level(mut self, level: u8) -> Self {
        self.level = Some(level.min(4));
        self
    }

    pub fn empty(mut self) -> Self {
        self.level = Some(0);
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = value.into();
        self
    }
}

/// How the matrix was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeatmapState {
    Ready,
    Empty,
    Unavailable(SharedString),
}

impl HeatmapState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Empty => "empty",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

impl HasPhase for HeatmapState {
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

/// A labelled grid of intensity cells.
#[derive(Debug, IntoElement)]
pub struct Heatmap {
    ident: Ident,
    label: SharedString,
    rows: Vec<SharedString>,
    columns: Vec<SharedString>,
    cells: Vec<HeatCell>,
    state: HeatmapState,
    slots: Slots,
}

impl Heatmap {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            rows: Vec::new(),
            columns: Vec::new(),
            cells: Vec::new(),
            state: HeatmapState::Ready,
            slots: Slots::default(),
        }
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.rows = rows.into_iter().map(Into::into).collect();
        self
    }

    pub fn columns(mut self, columns: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.columns = columns.into_iter().map(Into::into).collect();
        self
    }

    pub fn cells(mut self, cells: impl IntoIterator<Item = HeatCell>) -> Self {
        self.cells = cells.into_iter().collect();
        self
    }

    pub fn state(mut self, state: HeatmapState) -> Self {
        self.state = state;
        self
    }
}

impl Slotted for Heatmap {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for Heatmap {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, value): (gpui::AnyElement, SharedString) = match &self.state {
            HeatmapState::Empty => (
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        self.ident.child("empty"),
                        cx.strings().text(StringKey::HeatmapEmpty),
                    )
                    .kind(EmptyKind::Empty)
                    .into_any_element()
                }),
                SharedString::from(self.state.name()),
            ),
            HeatmapState::Unavailable(reason) => (
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        self.ident.child("unavailable"),
                        cx.strings().text(StringKey::HeatmapUnavailable),
                    )
                    .kind(EmptyKind::Unavailable)
                    .detail(reason.clone())
                    .into_any_element()
                }),
                SharedString::from(self.state.name()),
            ),
            HeatmapState::Ready if self.rows.is_empty() || self.columns.is_empty() => (
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        self.ident.child("empty"),
                        cx.strings().text(StringKey::HeatmapEmpty),
                    )
                    .kind(EmptyKind::Empty)
                    .into_any_element()
                }),
                SharedString::from(HeatmapState::Empty.name()),
            ),
            HeatmapState::Ready => (
                matrix(
                    &self.ident,
                    &self.rows,
                    &self.columns,
                    &self.cells,
                    &theme,
                    cx,
                ),
                SharedString::from(self.state.name()),
            ),
        };

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .child(
                div()
                    .type_scale(&theme, TypeScale::Label)
                    .text_color(theme.colors.text)
                    .child(self.label.clone()),
            )
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Table)
                    .text(self.label)
                    .value(value),
            )
    }
}

fn matrix(
    ident: &Ident,
    rows: &[SharedString],
    columns: &[SharedString],
    cells: &[HeatCell],
    theme: &gpui_kit_theme::Theme,
    cx: &App,
) -> gpui::AnyElement {
    let header = div()
        .row()
        .items_center()
        .gap_token(theme, Space::Xs)
        .child(div().w(px(56.0)).flex_none())
        .children(columns.iter().map(|column| {
            div()
                .w(px(18.0))
                .flex_none()
                .truncate()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_faint)
                .child(column.clone())
        }));

    let body = rows.iter().map(|row| {
        let row_ident = ident.child(row.as_ref());
        div()
            .row()
            .items_center()
            .gap_token(theme, Space::Xs)
            .child(
                div()
                    .w(px(56.0))
                    .flex_none()
                    .truncate()
                    .type_scale(theme, TypeScale::Caption)
                    .text_color(theme.colors.text_muted)
                    .child(row.clone()),
            )
            .children(columns.iter().map(|column| {
                let cell = cells
                    .iter()
                    .find(|cell| cell.row == *row && cell.column == *column);
                heat_cell(&row_ident, cell, column, theme, cx)
            }))
            .semantic_in(
                cx,
                NodeSpec::new(row_ident.semantic_id(), Role::Row)
                    .parent(ident.semantic_id())
                    .text(row.clone()),
            )
    });

    div()
        .column()
        .gap_token(theme, Space::Xs)
        .child(header)
        .children(body)
        .into_any_element()
}

fn heat_cell(
    row: &Ident,
    cell: Option<&HeatCell>,
    column: &SharedString,
    theme: &gpui_kit_theme::Theme,
    cx: &App,
) -> gpui::AnyElement {
    let id = cell
        .map(|cell| cell.id.clone())
        .unwrap_or_else(|| column.clone());
    let ident = row.child(id.as_ref());
    let (fill, missing) = match cell.and_then(|cell| cell.level) {
        None => (theme.colors.canvas, true),
        Some(0) => (theme.colors.accent.opacity(0.08), false),
        Some(1) => (theme.colors.accent.opacity(0.28), false),
        Some(2) => (theme.colors.accent.opacity(0.48), false),
        Some(3) => (theme.colors.accent.opacity(0.70), false),
        Some(_) => (theme.colors.accent.opacity(0.92), false),
    };

    let mut square = div()
        .id(ident.element_id())
        .size(px(16.0))
        .flex_none()
        .radius(theme, Radius::Small)
        .bg(fill)
        .when(missing, |element| {
            element
                .border(px(theme.borders.hairline))
                .border_color(theme.colors.hairline)
                .surface(theme, Surface::Canvas)
        });

    if let Some(cell) = cell.filter(|cell| !cell.value.is_empty()) {
        square = square.tip(ident.clone(), cell.value.clone());
    }

    let mut spec = NodeSpec::new(ident.semantic_id(), Role::Cell)
        .parent(row.semantic_id())
        .text(
            cell.map(|cell| cell.label.clone())
                .filter(|label| !label.is_empty())
                .unwrap_or_else(|| column.clone()),
        );
    if let Some(cell) = cell {
        if let Some(level) = cell.level {
            spec = spec.value(level.to_string());
        } else {
            spec = spec.value("missing");
        }
        if !cell.value.is_empty() {
            spec = spec.description(cell.value.clone());
        }
    } else {
        spec = spec.value("missing");
    }
    square.semantic_in(cx, spec).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_measured_zero_is_not_a_missing_observation() {
        let empty = HeatCell::new("a", "Mon", "W1").empty();
        let missing = HeatCell::new("b", "Mon", "W2");
        assert_eq!(empty.level, Some(0));
        assert_eq!(missing.level, None);
        assert_ne!(empty, missing);
    }

    #[test]
    fn intensity_stops_at_the_fifth_step() {
        let cell = HeatCell::new("hot", "Fri", "W4").level(9);
        assert_eq!(cell.level, Some(4));
    }
}

#[cfg(test)]
mod heatmap_phase_tests {
    use super::*;

    #[test]
    fn unavailable_is_not_empty() {
        let state = HeatmapState::Unavailable("offline".into());
        assert_eq!(state.phase(), Phase::Unavailable);
        assert_eq!(state.name(), "unavailable");
        assert_eq!(state.reason(), Some("offline"));
        assert_ne!(HeatmapState::Empty.phase(), state.phase());
    }
}
