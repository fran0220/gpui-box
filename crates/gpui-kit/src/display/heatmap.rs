//! A density matrix over caller-owned cells.
//!
//! The host supplies every cell identity, its row and column, an optional
//! intensity on a five-step ladder, and the words a hover shows. This
//! component cuts the five steps from one ramp colour the caller may name,
//! and distinguishes a measured zero from a cell nobody observed.

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, TypeScale};

use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::state_view::StateView;
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
    Loading,
    Ready,
    Empty,
    Unavailable(SharedString),
    Error(SharedString),
}

impl HeatmapState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Empty => "empty",
            Self::Unavailable(_) => "unavailable",
            Self::Error(_) => "error",
        }
    }
}

impl HasPhase for HeatmapState {
    fn phase(&self) -> Phase {
        match self {
            Self::Loading => Phase::Loading,
            Self::Ready => Phase::Ready,
            Self::Empty => Phase::Empty,
            Self::Unavailable(_) => Phase::Unavailable,
            Self::Error(_) => Phase::Error,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(reason) | Self::Error(reason) => Some(reason.as_ref()),
            _ => None,
        }
    }
}

/// The five intensity steps, as a fraction of the ramp colour.
///
/// Perceptual rather than linear: the gap a reader has to see is between one
/// step and the next, and equal alpha increments do not produce equal steps
/// against either a dark or a light ground.
///
/// The first step is a measured zero, so it has to be a fill somebody can see
/// rather than a hint of one. At a fraction low enough to disappear into the
/// canvas behind it, a matrix reported nothing measured and a matrix that
/// measured nothing were the same picture.
const STEPS: [f32; 5] = [0.18, 0.34, 0.52, 0.72, 0.94];

/// How much of the ramp colour one intensity step takes. A step above the
/// ladder clamps to the top of it rather than wrapping to the bottom.
fn step_alpha(level: u8) -> f32 {
    STEPS[usize::from(level).min(STEPS.len() - 1)]
}

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
    /// The larger period this entry belongs to, if the host names one.
    pub group: Option<SharedString>,
}

impl HeatAxis {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            group: None,
        }
    }

    /// The period this column belongs to, printed once over the run of
    /// columns that share it.
    ///
    /// A column prints what fits over a cell, which for a calendar is a day
    /// and not a date. Repeated across a quarter that is four columns headed
    /// `3`, `10`, `17`, `24` three times over, and a reader has no way to
    /// tell the second run from the third. The host already knows which
    /// period each column came from; it says so here rather than being made
    /// to fit the whole date into sixteen pixels.
    pub fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = Some(group.into());
        self
    }
}

impl From<SharedString> for HeatAxis {
    fn from(value: SharedString) -> Self {
        Self {
            id: value.clone(),
            label: value,
            group: None,
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
        let description = self.state.reason().map(SharedString::from);
        let (body, value): (gpui::AnyElement, SharedString) = match &self.state {
            HeatmapState::Loading | HeatmapState::Error(_) => (
                StateView::new(self.ident.child(self.state.name()), &self.state).into_any_element(),
                SharedString::from(self.state.name()),
            ),
            HeatmapState::Empty => (
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    marked_empty(
                        self.ident.child("empty"),
                        cx.strings().text(StringKey::HeatmapEmpty),
                        EmptyKind::Empty,
                        None,
                    )
                }),
                SharedString::from(self.state.name()),
            ),
            HeatmapState::Unavailable(reason) => (
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    marked_empty(
                        self.ident.child("unavailable"),
                        cx.strings().text(StringKey::HeatmapUnavailable),
                        EmptyKind::Unavailable,
                        Some(reason.clone()),
                    )
                }),
                SharedString::from(self.state.name()),
            ),
            HeatmapState::Ready if self.rows.is_empty() || self.columns.is_empty() => (
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    marked_empty(
                        self.ident.child("empty"),
                        cx.strings().text(StringKey::HeatmapEmpty),
                        EmptyKind::Empty,
                        None,
                    )
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

        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Table)
            .text(self.label.clone())
            .value(value);
        if let Some(description) = description {
            spec = spec.description(description);
        }

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
            .semantic_in(cx, spec)
    }
}

fn marked_empty(
    ident: Ident,
    label: SharedString,
    kind: EmptyKind,
    detail: Option<SharedString>,
) -> gpui::AnyElement {
    let mut empty = EmptyState::new(ident.clone(), SharedString::default()).kind(kind);
    if let Some(detail) = detail {
        empty = empty.detail(detail);
    }
    let mark_ident = ident.child("mark");
    div()
        .id(mark_ident.element_id())
        .child(empty)
        .tip(mark_ident, label)
        .into_any_element()
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
                .surface(theme, Surface::Sunken),
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
    let groups = column_groups(columns);
    let gap = theme.space(Space::Xs);
    let group_header = (!groups.is_empty()).then(|| {
        div()
            .row()
            .items_center()
            .gap(px(gap))
            .child(div().w(px(ROW_LABEL)).flex_none())
            .children(groups.into_iter().map(|(label, span)| {
                div()
                    // The run of cells the period covers, gaps included, so
                    // the name sits over its own columns and not near them.
                    .w(px(
                        span as f32 * CELL + (span.saturating_sub(1)) as f32 * gap
                    ))
                    .flex_none()
                    .truncate()
                    .type_scale(theme, TypeScale::Caption)
                    .text_color(theme.colors.text_muted)
                    .child(label)
            }))
    });

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
        .children(group_header)
        .child(header)
        .children(body)
        .into_any_element()
}

/// The runs of adjacent columns that name the same period, and how many
/// columns each run covers.
///
/// Adjacency is what a header can span, so a period that the host interleaved
/// with another is reported as the two runs it was actually drawn in rather
/// than as one label stretched over columns that do not belong to it. One
/// column without a period cancels the whole row: a header with a hole in it
/// says the columns under the hole belong to the period before them.
fn column_groups(columns: &[HeatAxis]) -> Vec<(SharedString, usize)> {
    let mut runs: Vec<(SharedString, usize)> = Vec::new();
    for column in columns {
        match (&column.group, runs.last_mut()) {
            (Some(group), Some((last, span))) if last == group => *span += 1,
            (Some(group), _) => runs.push((group.clone(), 1)),
            (None, _) => return Vec::new(),
        }
    }
    runs
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
        Some(level) => (ramp.opacity(step_alpha(level)), false),
    };

    let mut square = div()
        .id(ident.element_id())
        .size(px(CELL))
        .flex_none()
        .radius(theme, Radius::Small)
        .bg(fill)
        // An unobserved cell recedes to the sunken plane. Against the lowest
        // ramp step this remains an absence without adding an outline.
        .when(missing, |element| element.surface(theme, Surface::Sunken));

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

/// A continuous color encoding with a required finite, strictly increasing
/// domain. Diverging maps interpolate each side independently: the neutral
/// color corresponds to the explicit center, not the arithmetic midpoint.
/// Errors are stable [`StringKey::name`] values, not display text; resolve them
/// through the active strings when presenting them outside [`ContinuousHeatmap`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatColorScale {
    domain: [f64; 3],
    colors: [gpui::Hsla; 3],
}

impl HeatColorScale {
    pub fn sequential(
        domain: [f64; 2],
        low: gpui::Hsla,
        high: gpui::Hsla,
    ) -> Result<Self, &'static str> {
        let center = domain[0] * 0.5 + domain[1] * 0.5;
        Self::diverging(
            [domain[0], center, domain[1]],
            low,
            mix_heat(low, high, 0.5),
            high,
        )
    }

    pub fn diverging(
        domain: [f64; 3],
        low: gpui::Hsla,
        center: gpui::Hsla,
        high: gpui::Hsla,
    ) -> Result<Self, &'static str> {
        if domain.iter().any(|v| !v.is_finite()) || domain[0] >= domain[1] || domain[1] >= domain[2]
        {
            return Err(StringKey::HeatmapInvalidDomain.name());
        }
        for pair in domain.windows(2) {
            super::chart::scale::NumericScale::new(
                super::chart::scale::ScaleKind::Linear,
                [pair[0], pair[1]],
            )
            .map_err(|_| StringKey::HeatmapDomainOverflow.name())?;
        }
        Ok(Self {
            domain,
            colors: [low, center, high],
        })
    }

    pub fn domain(self) -> [f64; 3] {
        self.domain
    }

    /// Out-of-domain and nonfinite readings are invalid, never silently clamped.
    pub fn color(self, value: f64) -> Result<gpui::Hsla, &'static str> {
        use super::chart::scale::{NumericScale, ScaleKind};
        if !value.is_finite() || value < self.domain[0] || value > self.domain[2] {
            return Err(StringKey::HeatmapOutsideDomain.name());
        }
        let side = usize::from(value > self.domain[1]);
        let scale = NumericScale::new(
            ScaleKind::Linear,
            [self.domain[side], self.domain[side + 1]],
        )
        .map_err(|_| StringKey::HeatmapDomainOverflow.name())?;
        let fraction = scale
            .map(value)
            .ok_or(StringKey::HeatmapReadingOverflow.name())?;
        Ok(mix_heat(
            self.colors[side],
            self.colors[side + 1],
            fraction as f32,
        ))
    }
}

/// Straight-alpha sRGB interpolation; all endpoints are explicit caller colors.
fn mix_heat(a: gpui::Hsla, b: gpui::Hsla, t: f32) -> gpui::Hsla {
    let a: gpui::Rgba = a.into();
    let b: gpui::Rgba = b.into();
    gpui::Rgba {
        r: a.r * (1.0 - t) + b.r * t,
        g: a.g * (1.0 - t) + b.g * t,
        b: a.b * (1.0 - t) + b.b * t,
        a: a.a * (1.0 - t) + b.a * t,
    }
    .into()
}

/// A raw observation; `None` is missing, not zero. The existing five-level
/// [`HeatCell`] contract remains unchanged and is not used as a numeric bin.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuousHeatCell {
    pub id: SharedString,
    pub row: SharedString,
    pub column: SharedString,
    pub label: SharedString,
    pub reading: Option<f64>,
}

impl ContinuousHeatCell {
    pub fn new(
        id: impl Into<SharedString>,
        row: impl Into<SharedString>,
        column: impl Into<SharedString>,
        label: impl Into<SharedString>,
        reading: Option<f64>,
    ) -> Self {
        Self {
            id: id.into(),
            row: row.into(),
            column: column.into(),
            label: label.into(),
            reading,
        }
    }
}

type HeatSelection = std::rc::Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// A raw-valued matrix with visible cell values and a numeric color legend.
/// Selection is caller controlled. Invalid coordinates, identities, and readings
/// produce an explicit error; a stale refresh retains verified cells and reason.
#[derive(IntoElement)]
pub struct ContinuousHeatmap {
    ident: Ident,
    label: SharedString,
    rows: Vec<HeatAxis>,
    columns: Vec<HeatAxis>,
    state: super::plot::PlotState<Vec<ContinuousHeatCell>>,
    scale: HeatColorScale,
    current: Option<SharedString>,
    on_current: Option<HeatSelection>,
}

impl ContinuousHeatmap {
    pub fn new(
        ident: impl Into<Ident>,
        label: impl Into<SharedString>,
        scale: HeatColorScale,
        state: super::plot::PlotState<Vec<ContinuousHeatCell>>,
    ) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            rows: Vec::new(),
            columns: Vec::new(),
            state,
            scale,
            current: None,
            on_current: None,
        }
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = impl Into<HeatAxis>>) -> Self {
        self.rows = rows.into_iter().map(Into::into).collect();
        self
    }

    pub fn columns(mut self, columns: impl IntoIterator<Item = impl Into<HeatAxis>>) -> Self {
        self.columns = columns.into_iter().map(Into::into).collect();
        self
    }

    pub fn current(mut self, id: impl Into<SharedString>) -> Self {
        self.current = Some(id.into());
        self
    }

    pub fn on_current(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_current = Some(std::rc::Rc::new(handler));
        self
    }
}

fn validate_continuous(
    rows: &[HeatAxis],
    columns: &[HeatAxis],
    cells: &[ContinuousHeatCell],
    scale: HeatColorScale,
) -> Result<(), &'static str> {
    use std::collections::HashSet;
    let row_ids = rows.iter().map(|r| &r.id).collect::<HashSet<_>>();
    let column_ids = columns.iter().map(|c| &c.id).collect::<HashSet<_>>();
    if row_ids.len() != rows.len() || column_ids.len() != columns.len() {
        return Err(StringKey::HeatmapDuplicateAxis.name());
    }
    let mut ids = HashSet::new();
    let mut coordinates = HashSet::new();
    for cell in cells {
        if !ids.insert(&cell.id) || !coordinates.insert((&cell.row, &cell.column)) {
            return Err(StringKey::HeatmapDuplicateCell.name());
        }
        if !row_ids.contains(&cell.row) || !column_ids.contains(&cell.column) {
            return Err(StringKey::HeatmapUnknownCoordinate.name());
        }
        if let Some(value) = cell.reading {
            scale.color(value)?;
        }
    }
    Ok(())
}

impl RenderOnce for ContinuousHeatmap {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        use super::plot::{Plot, PlotState};
        use gpui::StatefulInteractiveElement;
        let theme = cx.theme().clone();
        let (cells, stale) = match self.state {
            PlotState::Ready(cells) => (cells, None),
            PlotState::Stale { data, reason } => (data, Some(reason)),
            other => {
                return Plot::new(self.ident, self.label, other.map(|_| Vec::new()))
                    .into_any_element();
            }
        };
        if let Err(reason) = validate_continuous(&self.rows, &self.columns, &cells, self.scale) {
            let reason = cx
                .strings()
                .text(StringKey::from_name(reason).expect("heatmap error key"));
            return Plot::new(self.ident, self.label, PlotState::Error(reason)).into_any_element();
        }
        if self.rows.is_empty() || self.columns.is_empty() {
            return Plot::new(self.ident, self.label, PlotState::Empty).into_any_element();
        }
        let header = div()
            .row()
            .gap_token(&theme, Space::Xs)
            .child(div().w(px(ROW_LABEL)).flex_none())
            .children(self.columns.iter().map(|column| {
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .type_scale(&theme, TypeScale::Caption)
                    .child(column.label.clone())
            }));
        let rows = self.rows.iter().map(|row| {
            div()
                .row()
                .items_center()
                .gap_token(&theme, Space::Xs)
                .child(
                    div()
                        .w(px(ROW_LABEL))
                        .flex_none()
                        .truncate()
                        .child(row.label.clone()),
                )
                .children(self.columns.iter().map(|column| {
                    let cell = cells
                        .iter()
                        .find(|c| c.row == row.id && c.column == column.id);
                    let ident =
                        self.ident
                            .child("cell")
                            .child(cell.map(|c| c.id.clone()).unwrap_or_else(|| {
                                Ident::new(row.id.clone())
                                    .child(column.id.as_ref())
                                    .semantic_id()
                            }));
                    let value = cell
                        .and_then(|c| c.reading)
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| {
                            cx.strings().text(StringKey::HeatmapMissing).to_string()
                        });
                    let label = cell
                        .map(|c| c.label.clone())
                        .unwrap_or_else(|| format!("{} / {}", row.label, column.label).into());
                    let fill = cell
                        .and_then(|c| c.reading)
                        .map(|v| self.scale.color(v).expect("validated reading"))
                        .unwrap_or(theme.colors.canvas);
                    let selected = cell.is_some_and(|c| self.current.as_ref() == Some(&c.id));
                    let mut square = div()
                        .id(ident.element_id())
                        .flex_1()
                        .min_w_0()
                        .h(px(32.0))
                        .overflow_hidden()
                        .bg(fill)
                        .border_1()
                        .border_color(if selected {
                            theme.colors.text
                        } else {
                            theme.colors.control_hairline
                        })
                        .child(
                            div()
                                .bg(theme.colors.canvas)
                                .text_color(theme.colors.text)
                                .type_scale(&theme, TypeScale::Caption)
                                .truncate()
                                .child(value.clone()),
                        )
                        .tip(
                            ident.clone(),
                            cx.strings()
                                .format(StringKey::HeatmapCellReading, &[label.as_ref(), &value]),
                        );
                    if let (Some(cell), Some(report)) = (cell, self.on_current.clone()) {
                        let id = cell.id.clone();
                        let key_id = id.clone();
                        let key_report = report.clone();
                        square = square
                            .tab_index(0)
                            .on_click(move |_, window, cx| report(id.clone(), window, cx))
                            .on_key_down(move |event, window, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    key_report(key_id.clone(), window, cx);
                                }
                            });
                    }
                    square.semantic_in(
                        cx,
                        NodeSpec::new(ident.semantic_id(), Role::Cell)
                            .parent(self.ident.semantic_id())
                            .text(label)
                            .value(value)
                            .selected(selected),
                    )
                }))
        });
        let [low, center, high] = self.scale.domain();
        let legend_id = self.ident.child("legend");
        let legend = div()
            .column()
            .gap_token(&theme, Space::Xs)
            .child(div().row().w_full().h(px(12.0)).children((0..32).map(|i| {
                let (a, b, t) = if i < 16 {
                    (low, center, i as f64 / 15.0)
                } else {
                    (center, high, (i - 16) as f64 / 15.0)
                };
                div().flex_1().h_full().bg(self
                    .scale
                    .color(a * (1.0 - t) + b * t)
                    .expect("validated legend"))
            })))
            .child(
                div()
                    .row()
                    .justify_between()
                    .children([low, center, high].map(|v| div().child(v.to_string()))),
            )
            .semantic_in(
                cx,
                NodeSpec::new(legend_id.semantic_id(), Role::Image)
                    .text(cx.strings().text(StringKey::HeatmapColorDomain))
                    .value(cx.strings().format(
                        StringKey::HeatmapDomainLegend,
                        &[&low.to_string(), &center.to_string(), &high.to_string()],
                    )),
            );
        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Table)
            .text(self.label.clone())
            .value(if stale.is_some() { "stale" } else { "ready" });
        if let Some(reason) = &stale {
            spec = spec.description(reason.clone());
        }
        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .text_color(theme.colors.text)
            .type_scale(&theme, TypeScale::Caption)
            .child(self.label.clone())
            .children(stale.clone().map(|reason| {
                div()
                    .text_color(theme.colors.danger)
                    .child(reason.clone())
                    .semantic_in(
                        cx,
                        NodeSpec::new(self.ident.child("stale").semantic_id(), Role::Status)
                            .text(reason),
                    )
            }))
            .child(header)
            .children(rows)
            .child(legend)
            .semantic_in(cx, spec)
            .into_any_element()
    }
}

#[cfg(test)]
mod continuous_tests {
    use super::*;

    #[test]
    fn asymmetric_diverging_domain_places_neutral_at_explicit_center() {
        let blue = gpui::rgb(0x0000ff).into();
        let white = gpui::rgb(0xffffff).into();
        let red = gpui::rgb(0xff0000).into();
        let scale = HeatColorScale::diverging([-8.0, 2.0, 32.0], blue, white, red).expect("domain");
        assert_eq!(scale.color(-8.0).expect("low"), blue);
        assert_eq!(scale.color(2.0).expect("neutral"), white);
        assert_eq!(scale.color(32.0).expect("high"), red);
        let halfway_blue: gpui::Rgba = scale.color(-3.0).expect("blue midpoint").into();
        let halfway_red: gpui::Rgba = scale.color(17.0).expect("red midpoint").into();
        assert!(
            (halfway_blue.r - 0.5).abs() < 1e-6
                && (halfway_blue.g - 0.5).abs() < 1e-6
                && (halfway_blue.b - 1.0).abs() < 1e-6
        );
        assert!(
            (halfway_red.r - 1.0).abs() < 1e-6
                && (halfway_red.g - 0.5).abs() < 1e-6
                && (halfway_red.b - 0.5).abs() < 1e-6
        );
        assert!(scale.color(33.0).is_err());
        assert!(scale.color(f64::NAN).is_err());
    }

    #[test]
    fn continuous_coordinates_are_validated_without_hiding_missing_values() {
        let scale = HeatColorScale::sequential(
            [0.0, 10.0],
            gpui::rgb(0xffffff).into(),
            gpui::rgb(0x0000ff).into(),
        )
        .expect("domain");
        let cell = ContinuousHeatCell::new("a", "r", "c", "A", None);
        assert!(
            validate_continuous(
                &["r".into()],
                &["c".into()],
                std::slice::from_ref(&cell),
                scale
            )
            .is_ok()
        );
        assert!(
            validate_continuous(&["r".into()], &["c".into()], &[cell.clone(), cell], scale)
                .is_err()
        );
        assert!(
            HeatColorScale::sequential([1.0, 1.0], gpui::rgb(0).into(), gpui::rgb(0xffffff).into())
                .is_err()
        );
    }
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
    fn a_period_header_spans_only_the_columns_next_to_each_other() {
        let columns = [
            HeatAxis::new("w0", "3").group("January"),
            HeatAxis::new("w1", "10").group("January"),
            HeatAxis::new("w2", "7").group("February"),
            HeatAxis::new("w3", "14").group("January"),
        ];
        assert_eq!(
            column_groups(&columns),
            vec![
                (SharedString::from("January"), 2),
                (SharedString::from("February"), 1),
                (SharedString::from("January"), 1),
            ]
        );
    }

    #[test]
    fn one_column_without_a_period_cancels_the_header() {
        let columns = [
            HeatAxis::new("w0", "3").group("January"),
            HeatAxis::new("w1", "10"),
        ];
        assert!(column_groups(&columns).is_empty());
    }

    #[test]
    fn the_ramp_climbs_and_its_lowest_step_is_still_a_fill() {
        let ladder: Vec<f32> = (0..6).map(step_alpha).collect();
        // A measured zero has to be visible against the ground behind it.
        assert!(ladder[0] >= 0.15);
        assert!(ladder[..5].windows(2).all(|pair| pair[0] < pair[1]));
        // And a level past the ladder takes the top of it, not the bottom.
        assert_eq!(ladder[5], ladder[4]);
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
