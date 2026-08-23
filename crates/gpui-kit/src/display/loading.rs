//! Loading indicators.
//!
//! Every animation here runs through GPUI's `with_animation`, which holds a
//! single static frame when the platform asks for reduced motion.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
    relative,
};
use gpui_kit_assets::Icon as Glyph;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Surface};

use crate::display::icon::{Icon, IconTone};
use crate::display::signature;
use crate::foundation::{Ident, Sizable};
use crate::motion::{self, AnimationExt as _, MotionSpec};

const PULSE_CELLS: usize = 5;
const MATRIX_SIDE: usize = 3;
/// How much of a placeholder row the moving highlight covers, and how far one
/// row's sweep trails the row above it.
const SHIMMER_BAND: f32 = 0.35;
const SHIMMER_ROW_OFFSET: f32 = 0.08;

/// A row of pulsing cells, used while a request is in flight.
#[derive(Debug, IntoElement)]
pub struct PulseLoader {
    ident: Ident,
    cell_size: f32,
    label: Option<gpui::SharedString>,
}

impl PulseLoader {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            cell_size: 8.0,
            label: None,
        }
    }

    /// What the wait is for. Announced with the busy state.
    pub fn label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn cell_size(mut self, cell_size: f32) -> Self {
        self.cell_size = cell_size;
        self
    }
}

impl RenderOnce for PulseLoader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = signature::stops(theme);
        let cell_size = self.cell_size;
        let period = MotionSpec::new(
            theme.motion.pulse_ms,
            motion::CubicBezier::new(0.25, 0.1, 0.25, 1.0),
        );
        let ident = self.ident.clone();
        let spec = busy_spec(&self.ident, self.label.clone());
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(cell_size / 2.0))
            .semantic_in(cx, spec)
            .children((0..PULSE_CELLS).map(move |index| {
                let color = colors[index % colors.len()];
                div()
                    .size(px(cell_size))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(px(cell_size))
                            .rounded(px(cell_size / 4.0))
                            .bg(color)
                            .with_animation(
                                ident.indexed_element_id(index),
                                period.repeating(),
                                move |element, delta| {
                                    let phase = motion::staggered_phase(delta, index, 0.0625);
                                    let wave = motion::pulse_wave(phase);
                                    element
                                        .opacity(0.08 + 0.92 * wave)
                                        .size(px(cell_size * (0.9 + 0.1 * wave)))
                                },
                            ),
                    )
            }))
    }
}

/// A three-by-three gradient matrix, used for longer indeterminate work.
#[derive(Debug, IntoElement)]
pub struct GradientSpinner {
    ident: Ident,
    cell_size: f32,
    label: Option<gpui::SharedString>,
}

impl GradientSpinner {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            cell_size: 5.0,
            label: None,
        }
    }

    pub fn label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn cell_size(mut self, cell_size: f32) -> Self {
        self.cell_size = cell_size;
        self
    }
}

impl RenderOnce for GradientSpinner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors.loader_gradient;
        let cell_size = self.cell_size;
        let period = MotionSpec::new(750, motion::CubicBezier::new(0.25, 0.1, 0.25, 1.0));
        let ident = self.ident.clone();
        let spec = busy_spec(&self.ident, self.label.clone());
        div()
            .flex()
            .flex_col()
            .gap(px(cell_size / 2.0))
            .semantic_in(cx, spec)
            .children((0..MATRIX_SIDE).map(move |row| {
                let ident = ident.clone();
                div()
                    .flex()
                    .flex_row()
                    .gap(px(cell_size / 2.0))
                    .children((0..MATRIX_SIDE).map(move |column| {
                        let index = row * MATRIX_SIDE + column;
                        let center = (MATRIX_SIDE as f32 - 1.0) / 2.0;
                        let distance =
                            MATRIX_SIDE as f32 - 1.0 - row as f32 + (column as f32 - center).abs();
                        let phase = distance / (MATRIX_SIDE as f32 + center);
                        div()
                            .size(px(cell_size))
                            .rounded_full()
                            .bg(colors[row])
                            .with_animation(
                                ident.indexed_element_id(index),
                                period.repeating(),
                                move |element, delta| {
                                    element.opacity(motion::gradient_opacity(delta + phase, 0.1))
                                },
                            )
                    }))
            }))
    }
}

/// One placeholder a skeleton can draw.
///
/// A row is the list case. The others exist so a card, an avatar, or a
/// paragraph can wait as themselves rather than as three identical bars.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SkeletonShape {
    Row { width: f32, height: f32 },
    Paragraph { lines: usize },
    Circle { size: f32 },
    Rect { width: f32, height: f32 },
    Card,
}

impl SkeletonShape {
    fn rows(self) -> Vec<(f32, f32, bool)> {
        match self {
            Self::Row { width, height } => vec![(width.clamp(0.16, 1.0), height, false)],
            Self::Paragraph { lines } => (0..lines.max(1))
                .map(|index| {
                    let width = if index + 1 == lines.max(1) { 0.62 } else { 1.0 };
                    (width, 12.0, false)
                })
                .collect(),
            Self::Circle { size } => vec![(size, size, true)],
            Self::Rect { width, height } => vec![(width.clamp(0.16, 1.0), height, false)],
            Self::Card => vec![(1.0, 72.0, false)],
        }
    }
}

/// Placeholder rows shown while a list's real shape is unknown.
#[derive(Debug, IntoElement)]
pub struct Skeleton {
    ident: Ident,
    rows: usize,
    row_height: f32,
    /// How wide each row is, as a fraction of the frame. A list that is
    /// still loading rarely has every row the same length; the pattern is
    /// the claim, not a measurement of content that does not exist yet.
    widths: Vec<f32>,
    shapes: Vec<SkeletonShape>,
    label: Option<gpui::SharedString>,
}

impl Skeleton {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            rows: 3,
            row_height: 28.0,
            widths: Vec::new(),
            shapes: Vec::new(),
            label: None,
        }
    }

    pub fn label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows;
        self
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = row_height;
        self
    }

    /// The width of each placeholder row, as a fraction of the frame.
    ///
    /// Missing entries reuse the last supplied width; an empty list leaves
    /// every row full-width. Values are clamped to `(0, 1]`.
    pub fn widths(mut self, widths: impl IntoIterator<Item = f32>) -> Self {
        self.widths = widths
            .into_iter()
            .map(|width| width.clamp(0.16, 1.0))
            .collect();
        self
    }

    /// Replaces the row list with an explicit sequence of shapes.
    ///
    /// [`Skeleton::rows`] and [`Skeleton::widths`] stay as the list case.
    pub fn shapes(mut self, shapes: impl IntoIterator<Item = SkeletonShape>) -> Self {
        self.shapes = shapes.into_iter().collect();
        self
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = theme.colors.track;
        let radius = theme.radii.control;
        let row_height = self.row_height;
        let period = MotionSpec::new(
            theme.motion.shimmer_ms,
            motion::CubicBezier::new(0.25, 0.1, 0.25, 1.0),
        );
        let ident = self.ident.clone();
        let spec = busy_spec(&self.ident, self.label.clone());
        let bands: Vec<(f32, f32, bool)> = if self.shapes.is_empty() {
            (0..self.rows)
                .map(|index| {
                    let width = self
                        .widths
                        .get(index)
                        .copied()
                        .or_else(|| self.widths.last().copied())
                        .unwrap_or(1.0);
                    (width, row_height, false)
                })
                .collect()
        } else {
            self.shapes
                .into_iter()
                .flat_map(SkeletonShape::rows)
                .collect()
        };
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .semantic_in(cx, spec)
            .children(
                bands
                    .into_iter()
                    .enumerate()
                    .map(move |(index, (width, height, circle))| {
                        let mut row = div()
                            .h(px(height))
                            .rounded(px(if circle { height / 2.0 } else { radius }))
                            .bg(color)
                            .relative()
                            .overflow_hidden();
                        row = if circle {
                            row.size(px(height))
                        } else {
                            row.w(relative(width))
                        };
                        row.child(signature::shimmer_band(theme).with_animation(
                            ident.indexed_element_id(index),
                            period.repeating(),
                            move |element, delta| {
                                let phase =
                                    motion::staggered_phase(delta, index, SHIMMER_ROW_OFFSET);
                                element.left(relative(motion::shimmer_offset(phase, SHIMMER_BAND)))
                            },
                        ))
                    }),
            )
    }
}

/// An inline turn, for a wait that sits next to a label.
///
/// Smaller than [`GradientSpinner`]: that one is a three-by-three matrix for
/// longer work. This is the mark a button or a status line already uses.
#[derive(Debug, IntoElement)]
pub struct Spinner {
    ident: Ident,
    size: ControlSize,
    label: Option<SharedString>,
}

impl Spinner {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            size: ControlSize::Sm,
            label: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl Sizable for Spinner {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Spinner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let spec = busy_spec(&self.ident, self.label.clone());
        div()
            .flex()
            .flex_none()
            .items_center()
            .child(
                Icon::new(Glyph::Refresh)
                    .control_size(self.size)
                    .tone(IconTone::Accent)
                    .spinning(self.ident.child("turn")),
            )
            .semantic_in(cx, spec)
    }
}

/// A veil over content that is still the last verified value.
///
/// The content stays. The veil says a refresh is in flight. Erasing the
/// content would be the lie [`crate::state::AsyncValue`] exists to prevent.
#[derive(IntoElement)]
pub struct RefreshVeil {
    ident: Ident,
    content: AnyElement,
    label: Option<SharedString>,
}

impl std::fmt::Debug for RefreshVeil {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RefreshVeil")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .finish()
    }
}

impl RefreshVeil {
    pub fn new(ident: impl Into<Ident>, content: impl IntoElement) -> Self {
        Self {
            ident: ident.into(),
            content: content.into_any_element(),
            label: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl RenderOnce for RefreshVeil {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let spec = busy_spec(&self.ident, self.label.clone());
        div()
            .relative()
            .w_full()
            .child(self.content)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme.surface(Surface::Panel).opacity(theme.opacity.scrim))
                    .child(
                        Spinner::new(self.ident.child("spin"))
                            .label(self.label.clone().unwrap_or_default()),
                    ),
            )
            .semantic_in(cx, spec)
    }
}

/// Loading is a distinct state, so every indicator publishes it rather than
/// leaving a test to infer a wait from an absence of content.
fn busy_spec(ident: &Ident, label: Option<gpui::SharedString>) -> NodeSpec {
    let mut spec = NodeSpec::new(ident.semantic_id(), Role::Progress).busy(true);
    if let Some(label) = label {
        spec = spec.text(label);
    }
    spec
}
