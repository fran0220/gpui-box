//! Validated raw series and shared Cartesian projection. Source values are
//! retained separately from projected/stacked geometry, including missing data.

use super::scale::{CategoryScale, NumericScale, ScaleError};
use gpui::{Hsla, SharedString};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub enum ChartValue {
    Number(f64),
    Category(SharedString),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum SourceX {
    Number(u64),
    Category(SharedString),
}

impl ChartValue {
    fn key(&self) -> SourceX {
        match self {
            Self::Number(value) => SourceX::Number(if *value == 0. { 0 } else { value.to_bits() }),
            Self::Category(id) => SourceX::Category(id.clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChartScale {
    Numeric(NumericScale),
    Category(CategoryScale),
}

impl ChartScale {
    pub fn map(&self, value: &ChartValue) -> Option<f64> {
        match (self, value) {
            (Self::Numeric(scale), ChartValue::Number(value)) => scale.map(*value),
            (Self::Category(scale), ChartValue::Category(id)) => scale.map(id),
            _ => None,
        }
    }
    pub fn invert(&self, fraction: f64) -> Option<ChartValue> {
        match self {
            Self::Numeric(scale) => scale.invert(fraction).map(ChartValue::Number),
            Self::Category(scale) => scale.invert(fraction).cloned().map(ChartValue::Category),
        }
    }
}

/// Axis identity is shared by series, references and linked caller state.
#[derive(Clone, Debug, PartialEq)]
pub struct ValueAxis {
    pub id: SharedString,
    pub label: SharedString,
    pub scale: NumericScale,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawPoint {
    pub id: SharedString,
    pub x: ChartValue,
    /// None breaks a path and omits a mark; it is never coerced to zero.
    pub y: Option<f64>,
    pub label: SharedString,
    /// Exact caller wording; used verbatim by semantics and tooltips.
    pub formatted: SharedString,
    /// Absolute lower/upper readings, not normalized offsets.
    pub error: Option<[f64; 2]>,
    /// Required start endpoint for Range marks; y is the end endpoint.
    /// Either ordering is valid. Equal endpoints draw a one-pixel body.
    pub baseline: Option<f64>,
    /// Per-mark color for bars/ranges/scatter; line strokes use series color.
    pub color: Option<Hsla>,
}

impl RawPoint {
    pub fn new(id: impl Into<SharedString>, x: ChartValue, y: Option<f64>) -> Self {
        let id = id.into();
        Self {
            label: id.clone(),
            id,
            x,
            y,
            formatted: y.map(|v| v.to_string()).unwrap_or_default().into(),
            error: None,
            baseline: None,
            color: None,
        }
    }
    pub fn text(mut self, label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self.formatted = value.into();
        self
    }
    pub fn error(mut self, interval: [f64; 2]) -> Self {
        self.error = Some(interval);
        self
    }
    pub fn baseline(mut self, value: f64) -> Self {
        self.baseline = Some(value);
        self
    }
    pub fn tint(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
    /// All absolute endpoints for automatic domain inference, including error
    /// whiskers. Missing y excludes the entire mark, including its interval.
    pub fn values(&self) -> impl Iterator<Item = Option<f64>> {
        let visible = self.y.is_some();
        [
            self.y,
            self.baseline.filter(|_| visible),
            self.error.filter(|_| visible).map(|e| e[0]),
            self.error.filter(|_| visible).map(|e| e[1]),
        ]
        .into_iter()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Curve {
    #[default]
    Linear,
    StepBefore,
    StepAfter,
    Monotone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesMark {
    Line,
    Area,
    Bar,
    Range,
    Scatter,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stack {
    None,
    Absolute(SharedString),
    Percent(SharedString),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawSeries {
    pub id: SharedString,
    pub label: SharedString,
    pub axis: SharedString,
    pub points: Vec<RawPoint>,
    pub mark: SeriesMark,
    pub curve: Curve,
    pub stack: Stack,
    pub color: Option<Hsla>,
}

impl RawSeries {
    pub fn new(
        id: impl Into<SharedString>,
        axis: impl Into<SharedString>,
        mark: SeriesMark,
    ) -> Self {
        let id = id.into();
        Self {
            label: id.clone(),
            id,
            axis: axis.into(),
            points: Vec::new(),
            mark,
            curve: Curve::Linear,
            stack: Stack::None,
            color: None,
        }
    }
    pub fn points(mut self, points: impl IntoIterator<Item = RawPoint>) -> Self {
        self.points = points.into_iter().collect();
        self
    }
    pub fn curve(mut self, curve: Curve) -> Self {
        self.curve = curve;
        self
    }
    pub fn stack(mut self, stack: Stack) -> Self {
        self.stack = stack;
        self
    }
    pub fn tint(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DataError {
    DuplicateIdentity(SharedString),
    UnknownAxis(SharedString),
    InvalidPoint(SharedString),
    InvalidStack(SharedString),
    Scale(ScaleError),
}

/// Geometry references the original series/point indexes; never synthetic IDs.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectedPoint {
    pub source: usize,
    pub x: f64,
    pub y: f64,
    pub baseline: f64,
    pub error: Option<[f64; 2]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectedSeries {
    pub source: usize,
    pub points: Vec<Option<ProjectedPoint>>,
    /// Offset and width in fractions of the category band.
    pub bar_offset: f64,
    pub bar_width: f64,
}

/// Validate and normalize raw pie shares without overflowing their total.
/// The formatted value and business ID survive normalization unchanged.
pub fn pie_series(series: &RawSeries) -> Result<super::ChartSeries, DataError> {
    let mut ids = HashSet::new();
    let mut max = 0.0_f64;
    for p in &series.points {
        if !ids.insert(&p.id) {
            return Err(DataError::DuplicateIdentity(p.id.clone()));
        }
        let y =
            p.y.filter(|v| v.is_finite() && *v >= 0.)
                .ok_or_else(|| DataError::InvalidPoint(p.id.clone()))?;
        max = max.max(y);
    }
    let total = if max == 0. {
        0.
    } else {
        series
            .points
            .iter()
            .map(|p| p.y.expect("raw pie readings validated") / max)
            .sum::<f64>()
    };
    let mut result = super::ChartSeries::new(series.id.clone(), series.label.clone()).points(
        series.points.iter().map(|p| {
            let share = if total == 0. {
                0.
            } else {
                (p.y.expect("raw pie readings validated") / max) / total
            };
            super::ChartPoint::new(
                p.id.clone(),
                0.,
                share as f32,
                p.label.clone(),
                p.formatted.clone(),
            )
        }),
    );
    result.color = series.color;
    Ok(result)
}

/// Explicit radial axes, rejecting missing, extra and out-of-domain readings.
pub fn radial_series(
    series: &RawSeries,
    axes: &[ValueAxis],
) -> Result<super::ChartSeries, DataError> {
    let mut ids = HashSet::new();
    for axis in axes {
        if !ids.insert(&axis.id) {
            return Err(DataError::DuplicateIdentity(axis.id.clone()));
        }
    }
    let mut point_ids = HashSet::new();
    for p in &series.points {
        if !point_ids.insert(&p.id) {
            return Err(DataError::DuplicateIdentity(p.id.clone()));
        }
    }
    if axes.len() < 3 || point_ids != ids {
        return Err(DataError::InvalidPoint(series.id.clone()));
    }
    let points = axes
        .iter()
        .map(|axis| {
            let p = series
                .points
                .iter()
                .find(|p| p.id == axis.id)
                .expect("radial axis identities validated");
            let value =
                p.y.and_then(|v| axis.scale.map(v))
                    .filter(|v| (0.0..=1.0).contains(v))
                    .ok_or_else(|| DataError::InvalidPoint(p.id.clone()))?;
            Ok(super::ChartPoint::new(
                p.id.clone(),
                0.,
                value as f32,
                axis.label.clone(),
                p.formatted.clone(),
            ))
        })
        .collect::<Result<Vec<_>, DataError>>()?;
    let mut result =
        super::ChartSeries::new(series.id.clone(), series.label.clone()).points(points);
    result.color = series.color;
    Ok(result)
}

/// Validate all input, including hidden series; hide/show cannot reveal a
/// latent malformed dataset. Stacks match x coordinates, not array positions.
/// Positive and negative percentages each normalize to their own 100% total.
pub fn project(
    series: &[RawSeries],
    x: &ChartScale,
    axes: &[ValueAxis],
    hidden: &[SharedString],
) -> Result<Vec<ProjectedSeries>, DataError> {
    let mut axis_ids = HashSet::new();
    for axis in axes {
        if !axis_ids.insert(&axis.id) {
            return Err(DataError::DuplicateIdentity(axis.id.clone()));
        }
    }
    let mut ids = HashSet::new();
    let mut totals: HashMap<(SharedString, SharedString, SourceX, bool), f64> = HashMap::new();
    let mut modes = HashMap::new();
    let mut groups = Vec::new();
    for item in series {
        if !ids.insert(&item.id) {
            return Err(DataError::DuplicateIdentity(item.id.clone()));
        }
        let axis = axes
            .iter()
            .find(|a| a.id == item.axis)
            .ok_or_else(|| DataError::UnknownAxis(item.axis.clone()))?;
        let mut points = HashSet::new();
        let mut stack_x = HashSet::new();
        for p in &item.points {
            if !points.insert(&p.id) {
                return Err(DataError::DuplicateIdentity(p.id.clone()));
            }
            x.map(&p.x)
                .ok_or_else(|| DataError::InvalidPoint(p.id.clone()))?;
            if p.y.is_some_and(|y| axis.scale.map(y).is_none())
                || p.baseline.is_some_and(|y| axis.scale.map(y).is_none())
                || (item.mark == SeriesMark::Range && p.y.is_some() && p.baseline.is_none())
                || (p.baseline.is_some()
                    && (item.mark != SeriesMark::Range || !matches!(item.stack, Stack::None)))
                || p.error
                    .is_some_and(|e| e[0] > e[1] || e.iter().any(|v| axis.scale.map(*v).is_none()))
            {
                return Err(DataError::InvalidPoint(p.id.clone()));
            }
            if !matches!(item.stack, Stack::None) && !stack_x.insert(p.x.key()) {
                return Err(DataError::InvalidStack(item.id.clone()));
            }
        }
        if !matches!(item.stack, Stack::None)
            && (!matches!(item.mark, SeriesMark::Area | SeriesMark::Bar)
                || axis.scale.kind() == super::scale::ScaleKind::Log)
        {
            return Err(DataError::InvalidStack(item.id.clone()));
        }
        if matches!(item.mark, SeriesMark::Area | SeriesMark::Bar) && axis.scale.map(0.).is_none() {
            return Err(DataError::InvalidStack(item.id.clone()));
        }
        let group = match &item.stack {
            Stack::None => item.id.clone(),
            Stack::Absolute(id) | Stack::Percent(id) => id.clone(),
        };
        if !matches!(item.stack, Stack::None) {
            let percent = matches!(item.stack, Stack::Percent(_));
            if modes
                .insert((item.axis.clone(), group.clone()), percent)
                .is_some_and(|old| old != percent)
            {
                return Err(DataError::InvalidStack(item.id.clone()));
            }
        }
        if hidden.contains(&item.id) {
            continue;
        }
        // The boolean separates unstacked IDs from stack IDs with the same text.
        let group_key = (
            item.axis.clone(),
            group.clone(),
            matches!(item.stack, Stack::None),
        );
        if matches!(item.mark, SeriesMark::Bar | SeriesMark::Range) && !groups.contains(&group_key)
        {
            groups.push(group_key);
        }
        if !matches!(item.stack, Stack::None) {
            for p in &item.points {
                if let Some(y) = p.y {
                    let key = (item.axis.clone(), group.clone(), p.x.key(), y < 0.);
                    let total = totals.entry(key).or_default();
                    *total += y.abs();
                    if !total.is_finite() {
                        return Err(DataError::InvalidStack(item.id.clone()));
                    }
                }
            }
        }
    }
    let mut offsets: HashMap<(SharedString, SharedString, SourceX, bool), f64> = HashMap::new();
    series
        .iter()
        .enumerate()
        .filter(|(_, s)| !hidden.contains(&s.id))
        .map(|(source, item)| {
            let axis = axes
                .iter()
                .find(|a| a.id == item.axis)
                .expect("series axis validated");
            let group = match &item.stack {
                Stack::None => item.id.clone(),
                Stack::Absolute(id) | Stack::Percent(id) => id.clone(),
            };
            let group_key = (
                item.axis.clone(),
                group.clone(),
                matches!(item.stack, Stack::None),
            );
            let axis_groups = groups
                .iter()
                .filter(|g| g.0 == item.axis)
                .collect::<Vec<_>>();
            let count = axis_groups.len().max(1) as f64;
            let order = axis_groups
                .iter()
                .position(|g| **g == group_key)
                .unwrap_or(0) as f64;
            let points = item
                .points
                .iter()
                .enumerate()
                .map(|(source, p)| {
                    let Some(mut y) = p.y else {
                        return Ok(None);
                    };
                    let x = x.map(&p.x).expect("raw x validated");
                    let mut base = p.baseline.unwrap_or(0.);
                    if !matches!(item.stack, Stack::None) {
                        let key = (item.axis.clone(), group.clone(), p.x.key(), y < 0.);
                        if matches!(item.stack, Stack::Percent(_)) {
                            let total = totals[&key];
                            y = if total == 0. { 0. } else { y / total * 100. };
                        }
                        let offset = offsets.entry(key).or_default();
                        base = *offset;
                        *offset += y;
                        y = *offset;
                    }
                    Ok(Some(ProjectedPoint {
                        source,
                        x,
                        y: axis
                            .scale
                            .map(y)
                            .ok_or_else(|| DataError::InvalidPoint(p.id.clone()))?,
                        baseline: axis.scale.map(base).unwrap_or(0.),
                        error: p.error.map(|e| {
                            e.map(|v| axis.scale.map(v).expect("error interval validated"))
                        }),
                    }))
                })
                .collect::<Result<Vec<_>, DataError>>()?;
            Ok(ProjectedSeries {
                source,
                points,
                bar_offset: -0.4 + order * 0.8 / count,
                bar_width: 0.8 / count,
            })
        })
        .collect()
}

/// Hermite tangents with sign rejection and the radius-three monotonicity
/// limiter. Independently implemented from the mathematical constraint;
/// repeated or unordered x is rendered linearly by the renderer.
pub fn monotone_tangents(points: &[[f64; 2]]) -> Option<Vec<f64>> {
    if points.len() < 2 {
        return Some(vec![0.; points.len()]);
    }
    if points[0][0] > points[points.len() - 1][0] {
        let reversed = points.iter().rev().copied().collect::<Vec<_>>();
        let mut tangents = monotone_tangents(&reversed)?;
        tangents.reverse();
        return Some(tangents);
    }
    let mut slopes = Vec::new();
    let direction = (points[1][0] - points[0][0]).signum();
    for pair in points.windows(2) {
        let dx = pair[1][0] - pair[0][0];
        if dx == 0. || dx.signum() != direction {
            return None;
        }
        let slope = (pair[1][1] - pair[0][1]) / dx;
        if !slope.is_finite() {
            return None;
        }
        slopes.push(slope);
    }
    let mut tangents = vec![slopes[0]];
    tangents.extend(slopes.windows(2).map(|s| {
        if s[0].signum() != s[1].signum() {
            0.
        } else {
            s[0] / 2. + s[1] / 2.
        }
    }));
    tangents.push(*slopes.last().expect("at least two points"));
    for (i, slope) in slopes.iter().enumerate() {
        if *slope == 0. {
            tangents[i] = 0.;
            tangents[i + 1] = 0.;
            continue;
        }
        let a = tangents[i] / slope;
        let b = tangents[i + 1] / slope;
        let norm = a.hypot(b);
        if norm > 3. {
            let factor = 3. / norm;
            tangents[i] *= factor;
            tangents[i + 1] *= factor;
        }
    }
    Some(tangents)
}

#[cfg(test)]
mod tests {
    use super::super::scale::ScaleKind;
    use super::*;
    #[test]
    fn collapsed_pixels_never_merge_source_x_and_signed_zero_is_one_coordinate() {
        let x = ChartScale::Numeric(
            NumericScale::new(ScaleKind::Linear, [0., 1e300]).expect("large finite domain"),
        );
        let axes = [ValueAxis {
            id: "y".into(),
            label: "Y".into(),
            scale: NumericScale::new(ScaleKind::Linear, [0., 100.]).expect("finite value domain"),
        }];
        let series = RawSeries::new("s", "y", SeriesMark::Bar)
            .stack(Stack::Absolute("stack".into()))
            .points([
                RawPoint::new("first", ChartValue::Number(1e-300), Some(11.)),
                RawPoint::new("second", ChartValue::Number(2e-300), Some(29.)),
            ]);
        let out = project(&[series], &x, &axes, &[]).expect("distinct raw coordinates");
        assert_eq!(out[0].points[0].as_ref().expect("first source point").x, 0.);
        assert_eq!(
            out[0].points[1].as_ref().expect("second source point").x,
            0.
        );
        assert_eq!(
            out[0].points[1]
                .as_ref()
                .expect("second source point")
                .baseline,
            0.
        );
        assert_eq!(
            out[0].points[1].as_ref().expect("second source point").y,
            0.29
        );
        assert_eq!(ChartValue::Number(-0.).key(), ChartValue::Number(0.).key());
    }
    #[test]
    fn ranges_preserve_direction_and_absolute_whiskers_and_infer_all_endpoints() {
        let point = RawPoint::new("fall", ChartValue::Number(3.), Some(21.))
            .baseline(63.)
            .error([9., 78.]);
        assert_eq!(
            NumericScale::extent(ScaleKind::Linear, point.values(), false)
                .expect("finite interval endpoints")
                .domain(),
            [9., 78.]
        );
        let x = ChartScale::Numeric(
            NumericScale::new(ScaleKind::Linear, [0., 10.]).expect("finite x domain"),
        );
        let axes = [ValueAxis {
            id: "y".into(),
            label: "Y".into(),
            scale: NumericScale::new(ScaleKind::Linear, [100., 0.]).expect("reversed domain"),
        }];
        let series = RawSeries::new("s", "y", SeriesMark::Range).points([point]);
        let out = project(&[series], &x, &axes, &[]).expect("valid range");
        let p = out[0].points[0].as_ref().expect("present range");
        assert_eq!(p.y, 0.79);
        assert_eq!(p.baseline, 0.37);
        assert_eq!(p.error, Some([0.91, 0.22]));
    }
    #[test]
    fn pie_overflow_and_radial_axis_order_preserve_exact_readings() {
        let series = RawSeries::new("s", "", SeriesMark::Bar).points([
            RawPoint::new("large", ChartValue::Number(0.), Some(f64::MAX))
                .text("Large", "Exact large"),
            RawPoint::new("small", ChartValue::Number(1.), Some(f64::MAX / 3.)),
        ]);
        let pie = pie_series(&series).expect("finite nonnegative shares");
        assert_eq!(pie.points[0].position.y, 0.75);
        assert_eq!(pie.points[1].position.y, 0.25);
        assert_eq!(pie.points[0].value.as_ref(), "Exact large");
        let axes =
            [("a", [0., 40.]), ("b", [-10., 30.]), ("c", [20., 120.])].map(|(id, domain)| {
                ValueAxis {
                    id: id.into(),
                    label: id.into(),
                    scale: NumericScale::new(ScaleKind::Linear, domain)
                        .expect("finite radial domain"),
                }
            });
        let radial = RawSeries::new("s", "", SeriesMark::Line).points(
            [("c", 95.), ("a", 10.), ("b", 10.)]
                .map(|(id, y)| RawPoint::new(id, ChartValue::Number(0.), Some(y))),
        );
        let radial = radial_series(&radial, &axes).expect("identities match radial axes");
        assert_eq!(
            radial
                .points
                .iter()
                .map(|p| p.position.y)
                .collect::<Vec<_>>(),
            vec![0.25, 0.5, 0.75]
        );
    }
    #[test]
    fn diverging_stacks_match_x_not_index_and_keep_raw_values() {
        let x = ChartScale::Category(CategoryScale::new(["a", "b"]).expect("distinct categories"));
        let axes = [ValueAxis {
            id: "y".into(),
            label: "Value".into(),
            scale: NumericScale::new(ScaleKind::Linear, [-20., 80.]).expect("diverging domain"),
        }];
        let p = |id: &str, x: &str, y| {
            RawPoint::new(id, ChartValue::Category(x.to_string().into()), Some(y))
        };
        let a = RawSeries::new("a", "y", SeriesMark::Bar)
            .stack(Stack::Absolute("s".into()))
            .points([p("a-west", "a", 13.), p("a-east", "b", -7.)]);
        let b = RawSeries::new("b", "y", SeriesMark::Bar)
            .stack(Stack::Absolute("s".into()))
            .points([p("b-east", "b", -3.), p("b-west", "a", 29.)]);
        let series = [a, b];
        let out = project(&series, &x, &axes, &[]).expect("valid diverging stack");
        let east = out[1].points[0].as_ref().expect("east point");
        assert!((east.y - 0.1).abs() < 1e-12);
        assert!((east.baseline - 0.13).abs() < 1e-12);
        assert_eq!(out[1].points[1].as_ref().expect("west point").y, 0.62);
        assert_eq!(series[1].points[east.source].y, Some(-3.));
        assert_eq!(series[1].points[east.source].id.as_ref(), "b-east");
    }
    #[test]
    fn monotone_has_no_overshoot_even_next_to_flat_and_steep_segments() {
        let p = [[0., 2.], [0.1, 9.], [4., 9.], [5., -3.]];
        let m = monotone_tangents(&p).expect("strictly increasing x");
        for i in 0..p.len() - 1 {
            for n in 0..=100 {
                let t = n as f64 / 100.;
                let t2 = t * t;
                let t3 = t2 * t;
                let dx = p[i + 1][0] - p[i][0];
                let y = (2. * t3 - 3. * t2 + 1.) * p[i][1]
                    + (t3 - 2. * t2 + t) * dx * m[i]
                    + (-2. * t3 + 3. * t2) * p[i + 1][1]
                    + (t3 - t2) * dx * m[i + 1];
                assert!(
                    y >= p[i][1].min(p[i + 1][1]) - 1e-12 && y <= p[i][1].max(p[i + 1][1]) + 1e-12
                );
            }
        }
        assert!(monotone_tangents(&[[1., 2.], [1., 3.]]).is_none());
        let reversed = p.into_iter().rev().collect::<Vec<_>>();
        let mut reversed_tangents = monotone_tangents(&reversed).expect("reversed monotone x");
        reversed_tangents.reverse();
        assert_eq!(reversed_tangents, m);
        assert!(
            monotone_tangents(&[[0., 0.], [1e-300, 1.], [1e300, 2.]])
                .expect("finite extreme secants")
                .iter()
                .all(|v| v.is_finite())
        );
    }
}
