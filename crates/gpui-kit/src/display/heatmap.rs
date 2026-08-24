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
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

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

/// The five intensity steps, as a fraction of the ramp colour.
///
/// Perceptual rather than linear: the gap a reader has to see is between one
/// step and the next, and equal alpha increments do not produce equal steps
/// against either a dark or a light ground.
const STEPS: [f32; 5] = [0.06, 0.22, 0.40, 0.62, 0.88];

/// One row or column of the grid: what a cell joins on, and what is printed.
///
/// These are two different things and were one string. A column is joined on
/// by identity and printed in the width of a cell, so a period a reader would
/// recognise — a week starting date, a build id — cannot be both without
/// either colliding with the next column or truncating to nothing. Passing a
/// bare string still works and names the axis entry after itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeatAxis {
    pub id: SharedString,
    pub label: SharedString,
}

impl HeatAxis {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

impl From<SharedString> for HeatAxis {
    fn from(value: SharedString) -> Self {
        Self {
            id: value.clone(),
            label: value,
        }
    }
}

impl From<&'static str> for HeatAxis {
    fn from(value: &'static str) -> Self {
        SharedString::from(value).into()
    }
}

impl From<String> for HeatAxis {
    fn from(value: String) -> Self {
        SharedString::from(value).into()
    }
}

impl<I: Into<SharedString>, L: Into<SharedString>> From<(I, L)> for HeatAxis {
    fn from((id, label): (I, L)) -> Self {
        Self::new(id, label)
    }
}

/// A labelled grid of intensity cells.
#[derive(Debug, IntoElement)]
pub struct Heatmap {
    ident: Ident,
    label: SharedString,
    rows: Vec<HeatAxis>,
    columns: Vec<HeatAxis>,
    cells: Vec<HeatCell>,
    state: HeatmapState,
    tint: Option<gpui::Hsla>,
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
            tint: None,
            slots: Slots::default(),
        }
    }

    /// The colour the density ramp is built from.
    ///
    /// Neutral by default: the ramp is one quantity, and one quantity needs
    /// a scale rather than a hue. A caller whose matrix already belongs to a
    /// colour — a series in a chart beside it, a person, a repository — hands
    /// that colour over here so the two agree.
    pub fn tint(mut self, tint: gpui::Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = impl Into<HeatAxis>>) -> Self {
        self.rows = rows.into_iter().map(Into::into).collect();
        self
    }

    pub fn columns(mut self, columns: impl IntoIterator<Item = impl Into<HeatAxis>>) -> Self {
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
                    ramp(&theme, self.tint),
                    &theme,
                    cx,
                ),
                SharedString::from(self.state.name()),
            ),
        };
        let legend = matches!(self.state, HeatmapState::Ready)
            .then(|| legend(ramp(&theme, self.tint), &theme, cx));

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .child(
                div()
                    .type_scale(&theme, TypeScale::Label)
                    .text_color(theme.colors.text)
                    .child(self.label.clone()),
            )
            .child(body)
            .children(legend)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Table)
                    .text(self.label)
                    .value(value),
            )
    }
}

/// The colour the density steps are cut from.
fn ramp(theme: &gpui_kit_theme::Theme, tint: Option<gpui::Hsla>) -> gpui::Hsla {
    tint.unwrap_or(theme.colors.text)
}

/// The scale, without which a shade is a colour rather than a quantity.
fn legend(ramp: gpui::Hsla, theme: &gpui_kit_theme::Theme, cx: &App) -> gpui::AnyElement {
    let caption = |content: SharedString| {
        div()
            .flex_none()
            .type_scale(theme, TypeScale::Caption)
            .text_color(theme.colors.text_faint)
            .child(content)
    };
    div()
        .row()
        .items_center()
        .gap_token(theme, Space::Xs)
        .child(caption(cx.strings().text(StringKey::HeatmapLess)))
        .children(STEPS.map(|step| {
            div()
                .size(px(CELL))
                .flex_none()
                .radius(theme, Radius::Small)
                .bg(ramp.opacity(step))
        }))
        .child(caption(cx.strings().text(StringKey::HeatmapMore)))
        .child(div().w(px(theme.space(Space::Sm))).flex_none())
        .child(
            div()
                .size(px(CELL))
                .flex_none()
                .radius(theme, Radius::Small)
                .border(px(theme.borders.hairline))
                .border_color(theme.colors.hairline_strong)
                .surface(theme, Surface::Canvas),
        )
        .child(caption(cx.strings().text(StringKey::HeatmapMissing)))
        .into_any_element()
}

/// The edge of one cell, and of one legend swatch, so the key is drawn from
/// the same square the matrix is.
const CELL: f32 = 16.0;

fn matrix(
    ident: &Ident,
    rows: &[HeatAxis],
    columns: &[HeatAxis],
    cells: &[HeatCell],
    ramp: gpui::Hsla,
    theme: &gpui_kit_theme::Theme,
    cx: &App,
) -> gpui::AnyElement {
    let header = div()
        .row()
        .items_center()
        .gap_token(theme, Space::Xs)
        .child(div().w(px(ROW_LABEL)).flex_none())
        .children(columns.iter().map(|column| {
            div()
                .w(px(CELL))
                .flex_none()
                .truncate()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.text_faint)
                .child(column.label.clone())
        }));

    let body = rows.iter().map(|row| {
        let row_ident = ident.child(row.id.as_ref());
        div()
            .row()
            .items_center()
            .gap_token(theme, Space::Xs)
            .child(
                div()
                    .w(px(ROW_LABEL))
                    .flex_none()
                    .truncate()
                    .type_scale(theme, TypeScale::Caption)
                    .text_color(theme.colors.text_muted)
                    .child(row.label.clone()),
            )
            .children(columns.iter().map(|column| {
                let cell = cells
                    .iter()
                    .find(|cell| cell.row == row.id && cell.column == column.id);
                heat_cell(&row_ident, cell, &column.id, ramp, theme, cx)
            }))
            .semantic_in(
                cx,
                NodeSpec::new(row_ident.semantic_id(), Role::Row)
                    .parent(ident.semantic_id())
                    .text(row.label.clone()),
            )
    });

    div()
        .column()
        .gap_token(theme, Space::Xs)
        .child(header)
        .children(body)
        .into_any_element()
}

/// How wide a row's name is allowed to be before it truncates.
const ROW_LABEL: f32 = 56.0;

fn heat_cell(
    row: &Ident,
    cell: Option<&HeatCell>,
    column: &SharedString,
    ramp: gpui::Hsla,
    theme: &gpui_kit_theme::Theme,
    cx: &App,
) -> gpui::AnyElement {
    let id = cell
        .map(|cell| cell.id.clone())
        .unwrap_or_else(|| column.clone());
    let ident = row.child(id.as_ref());
    let (fill, missing) = match cell.and_then(|cell| cell.level) {
        None => (theme.colors.canvas, true),
        Some(level) => (
            ramp.opacity(STEPS[usize::from(level).min(STEPS.len() - 1)]),
            false,
        ),
    };

    let mut square = div()
        .id(ident.element_id())
        .size(px(CELL))
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
            spec = spec.value(cx.numbers().count(usize::from(level)));
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
