//! Raw-value hierarchy and distribution layouts, rendered through [`Plot`].
//!
//! Layout rejects an entire invalid input instead of silently dropping values.
//! Caller identities survive layout and drive selection. This module deliberately
//! does not expose another general-purpose scale or axis system.

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use gpui::{
    App, Bounds, InteractiveElement, IntoElement, ParentElement, PathBuilder, Point, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, bounds, div, point,
    prelude::FluentBuilder, size,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TypeScale};

use super::chart::scale::{NumericScale, ScaleKind};
use super::plot::{Plot, PlotMark, PlotState};
use crate::foundation::{Ident, StyledExt};
use crate::strings::{ActiveStrings, StringKey, Strings};

/// A stable identity, human label, and finite raw quantity.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightedValue {
    pub id: SharedString,
    pub label: SharedString,
    pub value: f64,
}

impl WeightedValue {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, value: f64) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value,
        }
    }
}

/// Internal nodes contain children only; their quantity is the sum of leaves.
/// A leaf's weight is nonnegative. Zero leaves are retained in the textual key.
#[derive(Debug, Clone, PartialEq)]
pub struct HierarchyNode {
    pub item: WeightedValue,
    pub children: Vec<HierarchyNode>,
}

impl HierarchyNode {
    pub fn leaf(item: WeightedValue) -> Self {
        Self {
            item,
            children: Vec::new(),
        }
    }

    pub fn branch(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        children: Vec<Self>,
    ) -> Self {
        Self {
            item: WeightedValue::new(id, label, 0.0),
            children,
        }
    }
}

/// Invalid raw input never becomes a plausible but incomplete picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecializedError {
    DuplicateIdentity(SharedString),
    InvalidValue(SharedString),
    InvalidEdges,
    OutsideBins,
    EmptySample,
    InvalidRange,
    Overflow,
}

#[derive(Debug, Clone, PartialEq)]
struct Shape {
    item: WeightedValue,
    points: Vec<Point<f32>>,
}

/// Validated, reproducible geometry plus an always-visible exact-value key.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpecializedData {
    shapes: Vec<Shape>,
    key: Vec<WeightedValue>,
    labels: HashMap<SharedString, (StringKey, Vec<SharedString>)>,
}

fn validate(items: &[WeightedValue], nonnegative: bool) -> Result<(), SpecializedError> {
    let mut ids = HashSet::new();
    for item in items {
        if !ids.insert(item.id.clone()) {
            return Err(SpecializedError::DuplicateIdentity(item.id.clone()));
        }
        if !item.value.is_finite() || (nonnegative && item.value < 0.0) {
            return Err(SpecializedError::InvalidValue(item.id.clone()));
        }
    }
    Ok(())
}

fn rectangle(item: WeightedValue, x: f32, y: f32, w: f32, h: f32) -> Shape {
    // Layout arithmetic is validated before projection. Keep accumulated f32
    // rounding at the outer edge from escaping Plot's normalized contract.
    let right = (x + w).min(1.0);
    let bottom = (y + h).min(1.0);
    let x = x.min(1.0);
    let y = y.min(1.0);
    Shape {
        item,
        points: vec![
            point(x, y),
            point(right, y),
            point(right, bottom),
            point(x, bottom),
        ],
    }
}

fn hierarchy_weights(
    node: &HierarchyNode,
    all: &mut Vec<WeightedValue>,
) -> Result<f64, SpecializedError> {
    all.push(node.item.clone());
    if node.children.is_empty() {
        return Ok(node.item.value);
    }
    if node.item.value != 0.0 {
        return Err(SpecializedError::InvalidValue(node.item.id.clone()));
    }
    let total = node.children.iter().try_fold(0.0, |total, child| {
        hierarchy_weights(child, all).map(|weight| total + weight)
    })?;
    if !total.is_finite() {
        return Err(SpecializedError::Overflow);
    }
    Ok(total)
}

fn weight(node: &HierarchyNode) -> f64 {
    if node.children.is_empty() {
        node.item.value
    } else {
        node.children.iter().map(weight).sum()
    }
}

impl SpecializedData {
    // Layout retains caller text; apply Kit-owned wording once, at rendering,
    // to both the geometric marks and the persistent exact-value key.
    fn localize(mut self, strings: &Strings) -> Self {
        for item in self
            .key
            .iter_mut()
            .chain(self.shapes.iter_mut().map(|s| &mut s.item))
        {
            if let Some((key, extra)) = self.labels.get(&item.id) {
                let values = std::iter::once(item.label.as_ref())
                    .chain(extra.iter().map(SharedString::as_ref))
                    .collect::<Vec<_>>();
                item.label = strings.format(*key, &values);
            }
        }
        self
    }

    /// A sample box plot with an explicit shared linear domain. Each summary
    /// statistic remains labeled; outlier identities derive from value bits.
    pub fn box_plot(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        samples: &[f64],
        domain: [f64; 2],
    ) -> Result<Self, SpecializedError> {
        let id = id.into();
        let label = label.into();
        let summary = BoxSummary::from_samples(samples)?;
        let scale = NumericScale::new(ScaleKind::Linear, domain)
            .map_err(|_| SpecializedError::InvalidRange)?;
        let x = |value| {
            scale
                .map(value)
                .filter(|v| (0.0..=1.0).contains(v))
                .map(|v| v as f32)
                .ok_or(SpecializedError::InvalidRange)
        };
        let mut data = Self::default();
        let mut add_line = |suffix: &str,
                            key: StringKey,
                            value: f64,
                            y0: f32,
                            y1: f32|
         -> Result<(), SpecializedError> {
            let item = WeightedValue::new(
                Ident::new(id.clone()).child(suffix).semantic_id(),
                label.clone(),
                value,
            );
            data.labels.insert(item.id.clone(), (key, Vec::new()));
            data.key.push(item.clone());
            data.shapes.push(Shape {
                item,
                points: vec![point(x(value)?, y0), point(x(value)?, y1)],
            });
            Ok(())
        };
        add_line("low", StringKey::PlotBoxLow, summary.low, 0.35, 0.65)?;
        add_line("q1", StringKey::PlotBoxQ1, summary.q1, 0.2, 0.8)?;
        add_line("q3", StringKey::PlotBoxQ3, summary.q3, 0.2, 0.8)?;
        add_line("median", StringKey::PlotBoxMedian, summary.median, 0.2, 0.8)?;
        add_line("high", StringKey::PlotBoxHigh, summary.high, 0.35, 0.65)?;
        for (suffix, key, a, b, y) in [
            (
                "lower-whisker",
                StringKey::PlotBoxLowerWhisker,
                summary.low,
                summary.q1,
                0.5,
            ),
            (
                "upper-whisker",
                StringKey::PlotBoxUpperWhisker,
                summary.q3,
                summary.high,
                0.5,
            ),
            (
                "box-top",
                StringKey::PlotBoxTop,
                summary.q1,
                summary.q3,
                0.2,
            ),
            (
                "box-bottom",
                StringKey::PlotBoxBottom,
                summary.q1,
                summary.q3,
                0.8,
            ),
        ] {
            let item_id = Ident::new(id.clone()).child(suffix).semantic_id();
            data.labels.insert(item_id.clone(), (key, Vec::new()));
            data.shapes.push(Shape {
                item: WeightedValue::new(item_id, label.clone(), b - a),
                points: vec![point(x(a)?, y), point(x(b)?, y)],
            });
        }
        let mut seen = HashSet::new();
        for value in summary.outliers {
            if !seen.insert(value.to_bits()) {
                continue;
            }
            let item = WeightedValue::new(
                Ident::new(id.clone())
                    .child(format!("outlier-{:x}", value.to_bits()))
                    .semantic_id(),
                label.clone(),
                value,
            );
            data.labels
                .insert(item.id.clone(), (StringKey::PlotBoxOutlier, Vec::new()));
            data.key.push(item.clone());
            let x = x(value)?;
            data.shapes.push(Shape {
                item,
                points: vec![
                    point(x, 0.46),
                    point((x + 0.007).min(1.0), 0.5),
                    point(x, 0.54),
                    point((x - 0.007).max(0.0), 0.5),
                ],
            });
        }
        Ok(data)
    }

    /// One exact interval on an explicit shared linear domain. Reversed
    /// endpoints and out-of-domain values are errors, not clipped intervals.
    pub fn range(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        low: f64,
        high: f64,
        domain: [f64; 2],
    ) -> Result<Self, SpecializedError> {
        if !low.is_finite() || !high.is_finite() || low > high {
            return Err(SpecializedError::InvalidRange);
        }
        let scale = NumericScale::new(ScaleKind::Linear, domain)
            .map_err(|_| SpecializedError::InvalidRange)?;
        let a = scale
            .map(low)
            .filter(|v| (0.0..=1.0).contains(v))
            .ok_or(SpecializedError::InvalidRange)? as f32;
        let b = scale
            .map(high)
            .filter(|v| (0.0..=1.0).contains(v))
            .ok_or(SpecializedError::InvalidRange)? as f32;
        let id = id.into();
        let label = label.into();
        let labels = HashMap::from([(
            id.clone(),
            (
                StringKey::PlotRangeLabel,
                vec![low.to_string().into(), high.to_string().into()],
            ),
        )]);
        let item = WeightedValue::new(id, label, high - low);
        let key = vec![item.clone()];
        let shape = if a == b {
            Shape {
                item,
                points: vec![point(a, 0.35), point(a, 0.65)],
            }
        } else {
            rectangle(item, a.min(b), 0.35, (b - a).abs(), 0.3)
        };
        Ok(Self {
            shapes: vec![shape],
            key,
            labels,
        })
    }

    /// Alternating slice-and-dice treemap. Sibling order is caller order, each
    /// leaf area equals its fraction of the root, and children tile their parent.
    pub fn treemap(root: &HierarchyNode) -> Result<Self, SpecializedError> {
        let mut all = Vec::new();
        let total = hierarchy_weights(root, &mut all)?;
        validate(&all, true)?;
        let mut data = Self::default();
        fn visit(node: &HierarchyNode, b: Bounds<f32>, vertical: bool, out: &mut SpecializedData) {
            if node.children.is_empty() {
                out.key.push(node.item.clone());
                if node.item.value > 0.0 {
                    out.shapes.push(rectangle(
                        node.item.clone(),
                        b.origin.x,
                        b.origin.y,
                        b.size.width,
                        b.size.height,
                    ));
                }
                return;
            }
            let total = weight(node);
            let mut cursor = 0.0;
            for child in &node.children {
                let fraction = if total > 0.0 {
                    (weight(child) / total) as f32
                } else {
                    0.0
                };
                let child_bounds = if vertical {
                    bounds(
                        point(b.origin.x + cursor * b.size.width, b.origin.y),
                        size(fraction * b.size.width, b.size.height),
                    )
                } else {
                    bounds(
                        point(b.origin.x, b.origin.y + cursor * b.size.height),
                        size(b.size.width, fraction * b.size.height),
                    )
                };
                visit(child, child_bounds, !vertical, out);
                cursor += fraction;
            }
        }
        // Even an all-zero hierarchy retains its labels, without fabricated area.
        visit(
            root,
            bounds(
                point(0.0, 0.0),
                size(if total > 0.0 { 1.0 } else { 0.0 }, 1.0),
            ),
            true,
            &mut data,
        );
        Ok(data)
    }

    /// Equal radial depth bands, with angular span proportional to leaf totals.
    /// The root occupies the central disk; every child partitions its parent's angle.
    pub fn sunburst(root: &HierarchyNode) -> Result<Self, SpecializedError> {
        let mut all = Vec::new();
        hierarchy_weights(root, &mut all)?;
        validate(&all, true)?;
        fn depth(node: &HierarchyNode) -> usize {
            1 + node.children.iter().map(depth).max().unwrap_or(0)
        }
        fn visit(
            node: &HierarchyNode,
            level: usize,
            bands: usize,
            start: f32,
            end: f32,
            data: &mut SpecializedData,
        ) {
            let value = weight(node);
            let mut item = node.item.clone();
            item.value = value;
            item.label = format!("{}{}", "↳ ".repeat(level), item.label).into();
            if !node.children.is_empty() {
                data.labels
                    .insert(item.id.clone(), (StringKey::PlotSubtotal, Vec::new()));
            }
            data.key.push(item.clone());
            if value <= 0.0 {
                for child in &node.children {
                    visit(child, level + 1, bands, start, start, data);
                }
                return;
            }
            let inner = 0.48 * level as f32 / bands as f32;
            let outer = 0.48 * (level + 1) as f32 / bands as f32;
            let steps = (((end - start) * 32.0).ceil() as usize).max(2);
            let position = |angle: f32, radius: f32| {
                point(0.5 + angle.cos() * radius, 0.5 + angle.sin() * radius)
            };
            let mut points = (0..=steps)
                .map(|i| position(start + (end - start) * i as f32 / steps as f32, outer))
                .collect::<Vec<_>>();
            points.extend(
                (0..=steps)
                    .rev()
                    .map(|i| position(start + (end - start) * i as f32 / steps as f32, inner)),
            );
            data.shapes.push(Shape { item, points });
            let mut cursor = start;
            for child in &node.children {
                let next = cursor + (end - start) * (weight(child) / value) as f32;
                visit(child, level + 1, bands, cursor, next, data);
                cursor = next;
            }
        }
        let mut data = Self::default();
        visit(
            root,
            0,
            depth(root),
            -std::f32::consts::FRAC_PI_2,
            3.0 * std::f32::consts::FRAC_PI_2,
            &mut data,
        );
        Ok(data)
    }

    /// Centered stage bars: width, not trapezoid area, encodes raw quantity.
    /// Increasing stages are allowed and shown honestly rather than sorted.
    pub fn funnel(items: Vec<WeightedValue>) -> Result<Self, SpecializedError> {
        validate(&items, true)?;
        let max = items.iter().map(|item| item.value).fold(0.0, f64::max);
        let count = items.len();
        let shapes = items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.value > 0.0)
            .map(|(i, item)| {
                let width = (item.value / max) as f32;
                rectangle(
                    item.clone(),
                    (1.0 - width) / 2.0,
                    i as f32 / count as f32,
                    width,
                    0.85 / count as f32,
                )
            })
            .collect();
        Ok(Self {
            shapes,
            key: items,
            ..Self::default()
        })
    }

    /// Explicit finite increasing edges; bins are [left, right), except the
    /// final bin includes its right edge. Out-of-domain samples are errors.
    /// Bin identities derive from their edge values, not their list position.
    pub fn histogram(samples: &[f64], edges: &[f64]) -> Result<Self, SpecializedError> {
        if edges.len() < 2
            || edges.iter().any(|x| !x.is_finite())
            || edges.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(SpecializedError::InvalidEdges);
        }
        let mut counts = vec![0usize; edges.len() - 1];
        for &sample in samples {
            if !sample.is_finite() || sample < edges[0] || sample > edges[edges.len() - 1] {
                return Err(SpecializedError::OutsideBins);
            }
            let bin = edges
                .partition_point(|edge| *edge <= sample)
                .saturating_sub(1)
                .min(counts.len() - 1);
            counts[bin] += 1;
        }
        let span = edges[edges.len() - 1] - edges[0];
        if !span.is_finite() {
            return Err(SpecializedError::Overflow);
        }
        let max = counts.iter().copied().max().unwrap_or(0).max(1) as f32;
        let mut data = Self::default();
        for (i, pair) in edges.windows(2).enumerate() {
            let label = format!(
                "[{}, {}{}",
                pair[0],
                pair[1],
                if i == counts.len() - 1 { "]" } else { ")" }
            );
            let item = WeightedValue::new(
                format!("{:x}-{:x}", pair[0].to_bits(), pair[1].to_bits()),
                label,
                counts[i] as f64,
            );
            data.key.push(item.clone());
            if counts[i] > 0 {
                let height = counts[i] as f32 / max;
                data.shapes.push(rectangle(
                    item,
                    ((pair[0] - edges[0]) / span) as f32,
                    1.0 - height,
                    ((pair[1] - pair[0]) / span) as f32,
                    height,
                ));
            }
        }
        Ok(data)
    }

    /// Signed changes accumulate in input order, with a domain including zero
    /// and every cumulative endpoint. Overflow is rejected before rendering.
    pub fn waterfall(items: Vec<WeightedValue>) -> Result<Self, SpecializedError> {
        validate(&items, false)?;
        let mut sum: f64 = 0.0;
        let mut low: f64 = 0.0;
        let mut high: f64 = 0.0;
        let mut ranges = Vec::new();
        for item in &items {
            let next = sum + item.value;
            if !next.is_finite() {
                return Err(SpecializedError::Overflow);
            }
            ranges.push((sum, next));
            low = low.min(next);
            high = high.max(next);
            sum = next;
        }
        let span = high - low;
        if !span.is_finite() {
            return Err(SpecializedError::Overflow);
        }
        let count = items.len().max(1) as f32;
        let shapes = items
            .iter()
            .zip(ranges)
            .enumerate()
            .filter(|(_, (item, _))| item.value != 0.0)
            .map(|(i, (item, (a, b)))| {
                rectangle(
                    item.clone(),
                    i as f32 / count,
                    ((high - a.max(b)) / span) as f32,
                    0.8 / count,
                    ((a - b).abs() / span) as f32,
                )
            })
            .collect();
        Ok(Self {
            shapes,
            key: items,
            ..Self::default()
        })
    }
}

/// R-7 quantiles (linear interpolation at `(n - 1) p`), Tukey 1.5-IQR
/// fences, and whiskers at the outermost samples within those fences.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxSummary {
    pub low: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub high: f64,
    pub outliers: Vec<f64>,
}

impl BoxSummary {
    pub fn from_samples(samples: &[f64]) -> Result<Self, SpecializedError> {
        if samples.is_empty() {
            return Err(SpecializedError::EmptySample);
        }
        if samples.iter().any(|value| !value.is_finite()) {
            return Err(SpecializedError::InvalidRange);
        }
        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);
        let quantile = |p: f64| {
            let index = (sorted.len() - 1) as f64 * p;
            let fraction = index.fract();
            sorted[index.floor() as usize] * (1.0 - fraction)
                + sorted[index.ceil() as usize] * fraction
        };
        let q1 = quantile(0.25);
        let median = quantile(0.5);
        let q3 = quantile(0.75);
        let fence = 1.5 * (q3 - q1);
        if !fence.is_finite() {
            return Err(SpecializedError::Overflow);
        }
        let inside = sorted
            .iter()
            .copied()
            .filter(|v| *v >= q1 - fence && *v <= q3 + fence)
            .collect::<Vec<_>>();
        let outliers = sorted
            .iter()
            .copied()
            .filter(|v| *v < q1 - fence || *v > q3 + fence)
            .collect();
        Ok(Self {
            low: inside[0],
            q1,
            median,
            q3,
            high: inside[inside.len() - 1],
            outliers,
        })
    }
}

type Selection = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

#[derive(Default)]
struct SelectionState {
    declared: Option<SharedString>,
    current: Option<SharedString>,
}

fn color_index(id: &str) -> usize {
    id.bytes().fold(0usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    }) % 4
}

/// Specialized raw-value presentations with a persistent labeled value key,
/// keyboard selection, semantic marks, and retained-data refresh failures.
#[derive(IntoElement)]
pub struct SpecializedChart {
    ident: Ident,
    label: SharedString,
    state: PlotState<SpecializedData>,
    current: Option<SharedString>,
    on_current: Option<Selection>,
}

impl SpecializedChart {
    pub fn new(
        ident: impl Into<Ident>,
        label: impl Into<SharedString>,
        state: PlotState<SpecializedData>,
    ) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            state,
            current: None,
            on_current: None,
        }
    }

    pub fn current(mut self, id: impl Into<SharedString>) -> Self {
        self.current = Some(id.into());
        self
    }

    pub fn on_current(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_current = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SpecializedChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state = self.state.map(|data| data.localize(cx.strings()));
        let data = match &state {
            PlotState::Ready(data) | PlotState::Stale { data, .. } => data.clone(),
            _ => SpecializedData::default(),
        };
        let key = data.key.clone();
        let selectable = data
            .shapes
            .iter()
            .map(|s| s.item.id.clone())
            .collect::<HashSet<_>>();
        let interaction = crate::motion::keyed::slot::<SelectionState>(
            &self.ident.child("selection").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        {
            let mut state = interaction.borrow_mut();
            if state.declared != self.current {
                state.current = self.current.clone();
                state.declared = self.current;
            }
            if state
                .current
                .as_ref()
                .is_none_or(|id| !data.shapes.iter().any(|s| &s.item.id == id))
            {
                state.current = data.shapes.first().map(|s| s.item.id.clone());
            }
        }
        let current = interaction.borrow().current.clone();
        let painted_current = current.clone();
        let plot_state = interaction.clone();
        let plot_report = self.on_current.clone();
        let key_ident = self.ident.child("key");
        let state = match state {
            PlotState::Ready(data) if data.key.is_empty() => PlotState::Empty,
            other => other,
        };
        let marks = state.map(|data| {
            data.shapes
                .iter()
                .map(|shape| {
                    let min_x = shape.points.iter().map(|p| p.x).fold(1.0, f32::min);
                    let min_y = shape.points.iter().map(|p| p.y).fold(1.0, f32::min);
                    let max_x = shape.points.iter().map(|p| p.x).fold(0.0, f32::max);
                    let max_y = shape.points.iter().map(|p| p.y).fold(0.0, f32::max);
                    PlotMark::new(
                        shape.item.id.clone(),
                        shape.item.label.clone(),
                        shape.item.value.to_string(),
                        bounds(
                            point(min_x.min(0.999), min_y.min(0.999)),
                            size((max_x - min_x).max(0.001), (max_y - min_y).max(0.001)),
                        ),
                    )
                })
                .collect()
        });
        let colors = [
            theme.colors.accent,
            theme.colors.success,
            theme.colors.warning,
            theme.colors.danger,
        ];
        let hairline = theme.borders.hairline;
        let selected_tint = theme.colors.text;
        let radial = data.shapes.iter().any(|s| s.points.len() > 4);
        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .child(
                div().w_full().when(radial, |d| d.w(gpui::px(220.0))).child(
                    Plot::new(self.ident, self.label, marks)
                        .when_some(current.clone(), |plot, id| plot.current(id))
                        .on_current(move |id, window, cx| {
                            plot_state.borrow_mut().current = Some(id.clone());
                            if let Some(handler) = &plot_report {
                                handler(id, window, cx);
                            }
                        })
                        .paint(move |frame, window, _| {
                            for shape in &data.shapes {
                                let mut path = if shape.points.len() == 2 {
                                    PathBuilder::stroke(gpui::px(hairline))
                                } else {
                                    PathBuilder::fill()
                                };
                                for (j, &p) in shape.points.iter().enumerate() {
                                    if j == 0 {
                                        path.move_to(frame.point(p));
                                    } else {
                                        path.line_to(frame.point(p));
                                    }
                                }
                                if shape.points.len() > 2 {
                                    path.close();
                                }
                                if let Ok(path) = path.build() {
                                    window.paint_path(path, colors[color_index(&shape.item.id)]);
                                }
                                if painted_current.as_ref() == Some(&shape.item.id) {
                                    let mut outline = PathBuilder::stroke(gpui::px(hairline * 2.0));
                                    for (i, &p) in shape.points.iter().enumerate() {
                                        if i == 0 {
                                            outline.move_to(frame.point(p));
                                        } else {
                                            outline.line_to(frame.point(p));
                                        }
                                    }
                                    if shape.points.len() > 2 {
                                        outline.close();
                                    }
                                    if let Ok(path) = outline.build() {
                                        window.paint_path(path, selected_tint);
                                    }
                                }
                            }
                        }),
                ),
            )
            .children(key.into_iter().map(|item| {
                let id = key_ident.child(item.id.as_ref());
                let selected = current.as_ref() == Some(&item.id);
                let can_select = selectable.contains(&item.id);
                let state = interaction.clone();
                let report = self.on_current.clone();
                let clicked = item.id.clone();
                div()
                    .id(id.element_id())
                    .row()
                    .items_center()
                    .gap_token(&theme, Space::Xs)
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.text)
                    .child(
                        div()
                            .size(gpui::px(8.0))
                            .flex_none()
                            .bg(colors[color_index(&item.id)]),
                    )
                    .child(format!("{}: {}", item.label, item.value))
                    .when(can_select, |element| {
                        element.on_click(move |_, window, cx| {
                            state.borrow_mut().current = Some(clicked.clone());
                            if let Some(report) = &report {
                                report(clicked.clone(), window, cx);
                            }
                            window.refresh();
                        })
                    })
                    .semantic_in(
                        cx,
                        NodeSpec::new(
                            id.semantic_id(),
                            if can_select {
                                Role::Button
                            } else {
                                Role::Image
                            },
                        )
                        .text(item.label)
                        .value(item.value.to_string())
                        .selected(selected),
                    )
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &'static str, value: f64) -> WeightedValue {
        WeightedValue::new(id, id, value)
    }

    #[test]
    fn hierarchy_area_preserves_asymmetric_leaf_weights() {
        let root = HierarchyNode::branch(
            "root",
            "Root",
            vec![
                HierarchyNode::leaf(item("a", 6.0)),
                HierarchyNode::branch(
                    "branch",
                    "Branch",
                    vec![
                        HierarchyNode::leaf(item("b", 3.0)),
                        HierarchyNode::leaf(item("c", 1.0)),
                    ],
                ),
            ],
        );
        let data = SpecializedData::treemap(&root).expect("valid hierarchy");
        for (shape, expected) in data.shapes.iter().zip([0.6, 0.3, 0.1]) {
            let area =
                (shape.points[1].x - shape.points[0].x) * (shape.points[3].y - shape.points[0].y);
            assert!((area - expected).abs() < 1e-6);
        }
        assert_eq!(data.shapes[2].item.id.as_ref(), "c");
        let radial = SpecializedData::sunburst(&root).expect("valid hierarchy");
        assert_eq!(radial.shapes.len(), 5);
        let a = &radial.shapes[1];
        let end = a.points[a.points.len() / 2 - 1];
        // 60% of a turn starting at -90° ends at 126°. Second band radius is .32.
        assert!((end.x - (0.5 - 0.58778524 * 0.32)).abs() < 1e-6);
        assert!((end.y - (0.5 + 0.809017 * 0.32)).abs() < 1e-6);
    }

    #[test]
    fn invalid_values_duplicates_and_zero_have_distinct_outcomes() {
        assert!(SpecializedData::funnel(vec![item("a", -1.0)]).is_err());
        assert!(SpecializedData::funnel(vec![item("a", f64::NAN)]).is_err());
        assert!(SpecializedData::funnel(vec![item("a", 1.0), item("a", 2.0)]).is_err());
        let zero = SpecializedData::funnel(vec![item("zero", 0.0)]).expect("zero is valid");
        assert!(zero.shapes.is_empty());
        assert_eq!(zero.key[0].value, 0.0);
        let root =
            HierarchyNode::branch("root", "Root", vec![HierarchyNode::leaf(item("zero", 0.0))]);
        let zero = SpecializedData::sunburst(&root).expect("zero hierarchy");
        assert!(zero.shapes.is_empty());
        assert_eq!(zero.key.len(), 2);
    }

    #[test]
    fn histogram_edges_include_only_the_final_right_endpoint() {
        let data = SpecializedData::histogram(&[-2.0, -1.0, 0.0, 0.0, 3.0], &[-2.0, 0.0, 3.0])
            .expect("bounded sample");
        assert_eq!(
            data.key.iter().map(|i| i.value).collect::<Vec<_>>(),
            [2.0, 3.0]
        );
        assert!((data.shapes[0].points[1].x - 0.4).abs() < 1e-6);
        assert_eq!(
            SpecializedData::histogram(&[4.0], &[0.0, 3.0]),
            Err(SpecializedError::OutsideBins)
        );
        assert!(SpecializedData::histogram(&[], &[0.0, 0.0]).is_err());
    }

    #[test]
    fn quantiles_use_r7_and_whiskers_exclude_outliers() {
        let b =
            BoxSummary::from_samples(&[1.0, 2.0, 4.0, 7.0, 8.0, 100.0]).expect("finite samples");
        assert_eq!((b.q1, b.median, b.q3), (2.5, 5.5, 7.75));
        assert_eq!((b.low, b.high), (1.0, 8.0));
        assert_eq!(b.outliers, [100.0]);
        assert_eq!(
            BoxSummary::from_samples(&[]),
            Err(SpecializedError::EmptySample)
        );
        assert_eq!(
            BoxSummary::from_samples(&[3.0]).expect("singleton").median,
            3.0
        );
    }

    #[test]
    fn waterfall_crosses_zero_without_turning_deltas_into_absolute_bars() {
        let d = SpecializedData::waterfall(vec![
            item("up", 6.0),
            item("down", -9.0),
            item("back", 1.0),
        ])
        .expect("signed changes");
        assert!((d.shapes[0].points[3].y - 2.0 / 3.0).abs() < 1e-6);
        assert_eq!(d.shapes[1].points[0].y, 0.0);
        assert_eq!(d.shapes[1].points[3].y, 1.0);
        assert!((d.shapes[2].points[0].y - 8.0 / 9.0).abs() < 1e-6);
    }

    #[test]
    fn range_and_box_geometry_use_the_declared_shared_domain() {
        let range = SpecializedData::range("r", "Interval", -4.0, 18.0, [-10.0, 30.0])
            .expect("bounded interval");
        assert!((range.shapes[0].points[0].x - 0.15).abs() < 1e-6);
        assert!((range.shapes[0].points[1].x - 0.7).abs() < 1e-6);
        assert_eq!(range.key[0].id, range.shapes[0].item.id);
        assert!(SpecializedData::range("r", "", 2.0, 1.0, [0.0, 3.0]).is_err());
        assert!(SpecializedData::range("r", "", -1.0, 1.0, [0.0, 3.0]).is_err());
        let box_plot = SpecializedData::box_plot(
            "latency",
            "Latency",
            &[1.0, 2.0, 4.0, 7.0, 8.0, 100.0],
            [0.0, 100.0],
        )
        .expect("bounded sample");
        let q1 = box_plot
            .shapes
            .iter()
            .find(|s| s.item.id.as_ref() == "latency.q1")
            .expect("q1 geometry");
        assert!((q1.points[0].x - 0.025).abs() < 1e-6);
        assert_eq!(q1.item.value, 2.5);
    }
}
