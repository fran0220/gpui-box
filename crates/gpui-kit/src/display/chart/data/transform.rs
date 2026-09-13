//! Derived readings retain the immutable input revision and source membership.
//! They are data transforms, not path sampling. No label, error interval or
//! baseline from a source reading is misrepresented as an aggregate value.

use super::{ChartValue, RawPoint};
use gpui::SharedString;
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    rc::Rc,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aggregate {
    Count,
    Sum,
    Mean,
    Min,
    Max,
    /// R-7 linear interpolation of sorted observations; probability in [0, 1].
    Quantile(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingPolicy {
    /// Ignore missing readings; an all-missing group stays missing except Count.
    Skip,
    /// Any missing source makes the result missing, including Count.
    Propagate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransformError {
    DuplicateIdentity(SharedString),
    InvalidPoint(SharedString),
    InconsistentGroup(SharedString),
    InvalidAggregate,
    InvalidBins,
    InvalidWindow,
    Overflow,
}

/// Membership refers only to the retained immutable `TransformedData::source`.
/// Contiguous rolling windows use ranges, avoiding O(rows × window) lineage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceRows {
    Range(Range<usize>),
    Indices(Vec<usize>),
}

impl SourceRows {
    pub fn indices(&self) -> impl Iterator<Item = usize> + '_ {
        let (range, indices) = match self {
            Self::Range(range) => (range.clone(), [].iter()),
            Self::Indices(indices) => (0..0, indices.iter()),
        };
        range.chain(indices.copied())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DerivedPoint {
    pub point: RawPoint,
    pub sources: SourceRows,
    pub observed: usize,
    pub missing: usize,
}

/// Exact original IDs, values and wording remain available through membership
/// even if a downstream source collection is replaced or reordered.
#[derive(Clone, Debug)]
pub struct TransformedData {
    pub source: Rc<Vec<RawPoint>>,
    pub points: Vec<DerivedPoint>,
}

impl Aggregate {
    fn validate(self) -> Result<(), TransformError> {
        if let Self::Quantile(p) = self
            && (!p.is_finite() || !(0. ..=1.).contains(&p))
        {
            return Err(TransformError::InvalidAggregate);
        }
        Ok(())
    }
}

fn validate(points: &[RawPoint], aggregate: Aggregate) -> Result<(), TransformError> {
    aggregate.validate()?;
    let mut ids = HashSet::new();
    for point in points {
        if !ids.insert(&point.id) {
            return Err(TransformError::DuplicateIdentity(point.id.clone()));
        }
        if point.y.is_some_and(|v| !v.is_finite())
            || matches!(point.x, ChartValue::Number(v) if !v.is_finite())
        {
            return Err(TransformError::InvalidPoint(point.id.clone()));
        }
    }
    Ok(())
}

fn reduce(
    values: &mut [f64],
    missing: usize,
    aggregate: Aggregate,
    policy: MissingPolicy,
) -> Result<Option<f64>, TransformError> {
    if policy == MissingPolicy::Propagate && missing != 0 {
        return Ok(None);
    }
    if aggregate == Aggregate::Count {
        return Ok(Some(values.len() as f64));
    }
    if values.is_empty() {
        return Ok(None);
    }
    let value = match aggregate {
        Aggregate::Sum | Aggregate::Mean => {
            // Normalize before adding: a finite mean or cancelling sum should
            // not fail merely because an intermediate unscaled sum overflows.
            let scale = values.iter().map(|v| v.abs()).fold(0., f64::max);
            if scale == 0. {
                0.
            } else {
                let (mut sum, mut correction) = (0f64, 0f64);
                for value in values.iter().map(|v| v / scale) {
                    let next = sum + value;
                    correction += if sum.abs() >= value.abs() {
                        (sum - next) + value
                    } else {
                        (value - next) + sum
                    };
                    sum = next;
                }
                let normalized = sum + correction;
                (if aggregate == Aggregate::Mean {
                    normalized / values.len() as f64
                } else {
                    normalized
                }) * scale
            }
        }
        Aggregate::Min => values.iter().copied().fold(f64::INFINITY, f64::min),
        Aggregate::Max => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        Aggregate::Quantile(p) => {
            values.sort_by(f64::total_cmp);
            let at = p * (values.len() - 1) as f64;
            let lower = at.floor() as usize;
            let upper = at.ceil() as usize;
            let weight = at - lower as f64;
            values[lower] * (1. - weight) + values[upper] * weight
        }
        Aggregate::Count => unreachable!(),
    };
    if value.is_finite() {
        Ok(Some(value))
    } else {
        Err(TransformError::Overflow)
    }
}

fn derived(
    source: &[RawPoint],
    id: SharedString,
    x: ChartValue,
    sources: SourceRows,
    aggregate: Aggregate,
    policy: MissingPolicy,
) -> Result<DerivedPoint, TransformError> {
    let mut values = Vec::new();
    let mut missing = 0;
    for index in sources.indices() {
        match source[index].y {
            Some(value) => values.push(value),
            None => missing += 1,
        }
    }
    let observed = values.len();
    let value = reduce(&mut values, missing, aggregate, policy)?;
    Ok(DerivedPoint {
        point: RawPoint::new(id, x, value),
        sources,
        observed,
        missing,
    })
}

/// Group in first-encounter order using caller-owned group identity/coordinate.
/// The same ID must always name the same coordinate. Apply standard iterator
/// filtering/sorting to source points before this call; source identity is never
/// replaced by array position. Caller formatting can be applied to output points.
pub fn aggregate_by(
    source: Rc<Vec<RawPoint>>,
    aggregate: Aggregate,
    policy: MissingPolicy,
    mut group: impl FnMut(&RawPoint) -> (SharedString, ChartValue),
) -> Result<TransformedData, TransformError> {
    validate(&source, aggregate)?;
    let mut groups: Vec<(SharedString, ChartValue, Vec<usize>)> = Vec::new();
    let mut lookup = HashMap::new();
    for (index, point) in source.iter().enumerate() {
        let (id, x) = group(point);
        if matches!(x, ChartValue::Number(v) if !v.is_finite()) {
            return Err(TransformError::InvalidPoint(id));
        }
        let slot = *lookup.entry(id.clone()).or_insert_with(|| {
            groups.push((id.clone(), x.clone(), Vec::new()));
            groups.len() - 1
        });
        if groups[slot].1 != x {
            return Err(TransformError::InconsistentGroup(id));
        }
        groups[slot].2.push(index);
    }
    let points = groups
        .into_iter()
        .map(|(id, x, members)| {
            derived(
                &source,
                id,
                x,
                SourceRows::Indices(members),
                aggregate,
                policy,
            )
        })
        .collect::<Result<_, _>>()?;
    Ok(TransformedData { source, points })
}

/// Aggregate y readings into explicit numeric-x bins. Bins are half-open except
/// the inclusive final edge; out-of-domain/category x is an error, not discarded.
/// Empty bins are emitted and have missing values except Count = 0. IDs derive
/// from exact edge bits (signed zero canonicalized), not bin position. Histogram
/// counts use Count; source values need not be manufactured to compute a sum.
pub fn bin_points(
    source: Rc<Vec<RawPoint>>,
    edges: &[f64],
    aggregate: Aggregate,
    policy: MissingPolicy,
) -> Result<TransformedData, TransformError> {
    validate(&source, aggregate)?;
    if edges.len() < 2
        || edges.iter().any(|v| !v.is_finite())
        || edges.windows(2).any(|w| w[0] >= w[1])
    {
        return Err(TransformError::InvalidBins);
    }
    let mut bins = vec![Vec::new(); edges.len() - 1];
    for (index, point) in source.iter().enumerate() {
        let ChartValue::Number(x) = point.x else {
            return Err(TransformError::InvalidPoint(point.id.clone()));
        };
        if x < edges[0] || x > edges[edges.len() - 1] {
            return Err(TransformError::InvalidPoint(point.id.clone()));
        }
        let slot = edges
            .partition_point(|edge| *edge <= x)
            .saturating_sub(1)
            .min(bins.len() - 1);
        bins[slot].push(index);
    }
    let bits = |value: f64| if value == 0. { 0 } else { value.to_bits() };
    let points = bins
        .into_iter()
        .zip(edges.windows(2))
        .map(|(members, pair)| {
            let id: SharedString =
                format!("bin-{:016x}-{:016x}", bits(pair[0]), bits(pair[1])).into();
            derived(
                &source,
                id,
                ChartValue::Number(pair[0] * 0.5 + pair[1] * 0.5),
                SourceRows::Indices(members),
                aggregate,
                policy,
            )
        })
        .collect::<Result<_, _>>()?;
    Ok(TransformedData { source, points })
}

/// Trailing observation-count windows in caller order, not elapsed-time windows.
/// Output retains each current source ID/x; a missing current point remains a
/// path gap. `min_observed` controls warm-up; an undersized window is missing.
/// Membership storage is O(n). Reduction is O(n × width), and quantiles also
/// sort each window; prepare once per data revision, never inside a paint loop.
pub fn rolling_points(
    source: Rc<Vec<RawPoint>>,
    width: usize,
    min_observed: usize,
    aggregate: Aggregate,
    policy: MissingPolicy,
) -> Result<TransformedData, TransformError> {
    validate(&source, aggregate)?;
    if width == 0 || min_observed == 0 || min_observed > width {
        return Err(TransformError::InvalidWindow);
    }
    let mut points = Vec::with_capacity(source.len());
    for (index, point) in source.iter().enumerate() {
        let start = (index + 1).saturating_sub(width);
        let mut result = derived(
            &source,
            point.id.clone(),
            point.x.clone(),
            SourceRows::Range(start..index + 1),
            aggregate,
            policy,
        )?;
        if point.y.is_none() || result.observed < min_observed {
            result.point.y = None;
            result.point.formatted = "".into();
        }
        points.push(result);
    }
    Ok(TransformedData { source, points })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn readings(values: &[Option<f64>]) -> Rc<Vec<RawPoint>> {
        Rc::new(
            values
                .iter()
                .enumerate()
                .map(|(i, y)| {
                    RawPoint::new(format!("source-{i}"), ChartValue::Number(i as f64), *y)
                })
                .collect(),
        )
    }

    #[test]
    fn aggregation_preserves_lineage_missing_and_independent_group_identity() {
        let source = readings(&[Some(2.), None, Some(11.), Some(-4.)]);
        let result = aggregate_by(source.clone(), Aggregate::Mean, MissingPolicy::Skip, |_| {
            ("region".into(), ChartValue::Category("west".into()))
        })
        .expect("mean");
        assert_eq!(result.points[0].point.y, Some(3.));
        assert_eq!(
            (result.points[0].observed, result.points[0].missing),
            (3, 1)
        );
        assert!(Rc::ptr_eq(&source, &result.source));
        assert_eq!(
            result.points[0]
                .sources
                .indices()
                .map(|i| result.source[i].id.as_ref())
                .collect::<Vec<_>>(),
            ["source-0", "source-1", "source-2", "source-3"]
        );
        let missing = aggregate_by(
            source.clone(),
            Aggregate::Mean,
            MissingPolicy::Propagate,
            |_| ("region".into(), ChartValue::Number(1.)),
        )
        .expect("missing group");
        assert_eq!(missing.points[0].point.y, None);
        assert!(matches!(
            aggregate_by(source, Aggregate::Sum, MissingPolicy::Skip, |p| (
                "same".into(),
                p.x.clone()
            )),
            Err(TransformError::InconsistentGroup(_))
        ));
    }

    #[test]
    fn bins_use_both_boundaries_and_preserve_empty_bins() {
        let source = readings(&[Some(7.), Some(13.), None, Some(19.)]);
        let result = bin_points(
            source,
            &[0., 1., 2., 2.5, 3.],
            Aggregate::Sum,
            MissingPolicy::Skip,
        )
        .expect("bins");
        assert_eq!(
            result.points.iter().map(|p| p.point.y).collect::<Vec<_>>(),
            [Some(7.), Some(13.), None, Some(19.)]
        );
        assert_eq!(result.points[1].sources.indices().collect::<Vec<_>>(), [1]);
        assert_eq!(result.points[3].sources.indices().collect::<Vec<_>>(), [3]);
        let shifted = bin_points(
            readings(&[Some(7.), Some(13.)]),
            &[-1., 0., 1., 2.],
            Aggregate::Count,
            MissingPolicy::Skip,
        )
        .expect("added earlier bin");
        let original = bin_points(
            readings(&[Some(7.), Some(13.)]),
            &[0., 1., 2.],
            Aggregate::Count,
            MissingPolicy::Skip,
        )
        .expect("original bins");
        assert_eq!(shifted.points[1].point.id, original.points[0].point.id);
        assert_eq!(shifted.points[0].point.y, Some(0.));
    }

    #[test]
    fn extreme_mean_cancelling_sum_and_quantiles_do_not_overflow_intermediates() {
        let all = |_: &RawPoint| ("all".into(), ChartValue::Number(0.));
        let mean = aggregate_by(
            readings(&[Some(f64::MAX), Some(f64::MAX)]),
            Aggregate::Mean,
            MissingPolicy::Skip,
            all,
        )
        .expect("finite mean");
        assert_eq!(mean.points[0].point.y, Some(f64::MAX));
        let sum = aggregate_by(
            readings(&[Some(f64::MAX), Some(f64::MAX), Some(-f64::MAX)]),
            Aggregate::Sum,
            MissingPolicy::Skip,
            all,
        )
        .expect("cancelling sum");
        assert_eq!(sum.points[0].point.y, Some(f64::MAX));
        assert!(matches!(
            aggregate_by(
                readings(&[Some(f64::MAX), Some(f64::MAX)]),
                Aggregate::Sum,
                MissingPolicy::Skip,
                all
            ),
            Err(TransformError::Overflow)
        ));
        let quantile = aggregate_by(
            readings(&[Some(2.), Some(5.), Some(19.), Some(43.)]),
            Aggregate::Quantile(0.25),
            MissingPolicy::Skip,
            all,
        )
        .expect("R-7 quartile");
        assert_eq!(quantile.points[0].point.y, Some(4.25));
    }

    #[test]
    fn rolling_keeps_gaps_and_compact_exact_membership() {
        let result = rolling_points(
            readings(&[Some(3.), Some(9.), None, Some(21.), Some(-3.)]),
            3,
            2,
            Aggregate::Mean,
            MissingPolicy::Skip,
        )
        .expect("rolling mean");
        assert_eq!(
            result.points.iter().map(|p| p.point.y).collect::<Vec<_>>(),
            [None, Some(6.), None, Some(15.), Some(9.)]
        );
        assert_eq!(result.points[3].sources, SourceRows::Range(1..4));
        assert_eq!(result.points[3].point.id.as_ref(), "source-3");
        assert_eq!(result.source[3].y, Some(21.));
    }
}
