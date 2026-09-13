//! CPU reductions after projection, without replacing source observations.
//!
//! Sampling applies only to a single continuous linear path; the caller splits
//! missing readings and keeps all source marks for semantics and interaction.
//! Hit rectangles must come from the final painted geometry (including motion,
//! orientation and custom extents). Queries use the same normalized logical
//! coordinates, weighted by the physical pixel length of each logical axis.

use super::data::{ChartScale, DataError, ProjectedSeries, RawSeries, ValueAxis, project};
use gpui::SharedString;
use std::rc::Rc;

#[derive(Default)]
pub(super) struct ProjectionCache {
    cached: Option<Projection>,
}

struct Projection {
    series: Rc<Vec<RawSeries>>,
    x: ChartScale,
    axes: Vec<ValueAxis>,
    hidden: Vec<SharedString>,
    result: Result<Rc<Vec<ProjectedSeries>>, DataError>,
}

impl ProjectionCache {
    /// Immutable shared input identifies a dataset revision. Copy-on-write
    /// caller updates invalidate it; selection, hover and readouts do not.
    /// Projection is cached before motion, never after interpolated geometry.
    pub(super) fn get(
        &mut self,
        series: &Rc<Vec<RawSeries>>,
        x: &ChartScale,
        axes: &[ValueAxis],
        hidden: &[SharedString],
    ) -> Result<Rc<Vec<ProjectedSeries>>, DataError> {
        if let Some(cached) = &self.cached
            && Rc::ptr_eq(series, &cached.series)
            && *x == cached.x
            && axes == cached.axes
            && hidden == cached.hidden
        {
            return cached.result.clone();
        }
        let result = project(series, x, axes, hidden).map(Rc::new);
        self.cached = Some(Projection {
            series: series.clone(),
            x: x.clone(),
            axes: axes.to_vec(),
            hidden: hidden.to_vec(),
            result: result.clone(),
        });
        result
    }
}

/// Keep ordered endpoints and extrema of both sides of an area in each pixel
/// column. Reversed ordered x is supported. Nonmonotonic or nonfinite input is
/// left exact. Offscreen tails occupy one bucket each, retaining the points
/// needed to cross the viewport boundary. Returned offsets always name input.
pub(super) fn sample_path(points: &[[f64; 3]], columns: usize) -> Vec<usize> {
    let exact = || (0..points.len()).collect();
    if columns == 0 || points.len() <= columns.saturating_mul(6) {
        return exact();
    }
    if points.iter().flatten().any(|value| !value.is_finite())
        || !(points.windows(2).all(|pair| pair[0][0] <= pair[1][0])
            || points.windows(2).all(|pair| pair[0][0] >= pair[1][0]))
    {
        return exact();
    }
    let bucket = |x: f64| {
        if x < 0. {
            0
        } else if x >= 1. {
            columns.saturating_add(1)
        } else {
            (x * columns as f64).floor() as usize + 1
        }
    };
    let mut kept = Vec::new();
    let mut start = 0;
    while start < points.len() {
        let column = bucket(points[start][0]);
        let mut end = start + 1;
        let mut extrema = [start; 4];
        while end < points.len() && bucket(points[end][0]) == column {
            for (slot, coordinate, minimum) in
                [(0, 1, true), (1, 1, false), (2, 2, true), (3, 2, false)]
            {
                let old = points[extrema[slot]][coordinate];
                let new = points[end][coordinate];
                if if minimum { new < old } else { new > old } {
                    extrema[slot] = end;
                }
            }
            end += 1;
        }
        let mut candidates = [
            start,
            end - 1,
            extrema[0],
            extrema[1],
            extrema[2],
            extrema[3],
        ];
        candidates.sort_unstable();
        for index in candidates {
            if kept.last() != Some(&index) {
                kept.push(index);
            }
        }
        start = end;
    }
    kept
}

#[derive(Clone)]
struct Entry {
    source: usize,
    rect: [f64; 4],
}

struct Node {
    rect: [f64; 4],
    range: std::ops::Range<usize>,
    children: Option<[usize; 2]>,
}

/// Immutable rectangle tree. Exact nearest matches the exhaustive distance
/// query, including first-source tie breaking. Construction is O(n log n);
/// query work is data-dependent and can still be O(n) for overlapping marks.
pub(super) struct HitIndex {
    entries: Vec<Entry>,
    nodes: Vec<Node>,
}

impl HitIndex {
    pub(super) fn new(rectangles: impl IntoIterator<Item = [f64; 4]>) -> Self {
        let mut entries = rectangles
            .into_iter()
            .enumerate()
            .filter(|(_, r)| r.iter().all(|v| v.is_finite()) && r[0] <= r[2] && r[1] <= r[3])
            .map(|(source, rect)| Entry { source, rect })
            .collect::<Vec<_>>();
        let mut nodes = Vec::new();
        if !entries.is_empty() {
            Self::build(&mut entries, 0, &mut nodes);
        }
        Self { entries, nodes }
    }

    fn build(entries: &mut [Entry], offset: usize, nodes: &mut Vec<Node>) -> usize {
        let mut rect = entries[0].rect;
        for entry in &entries[1..] {
            rect[0] = rect[0].min(entry.rect[0]);
            rect[1] = rect[1].min(entry.rect[1]);
            rect[2] = rect[2].max(entry.rect[2]);
            rect[3] = rect[3].max(entry.rect[3]);
        }
        let node = nodes.len();
        nodes.push(Node {
            rect,
            range: offset..offset + entries.len(),
            children: None,
        });
        if entries.len() > 8 {
            let axis = usize::from(rect[3] - rect[1] > rect[2] - rect[0]);
            let middle = entries.len() / 2;
            entries.select_nth_unstable_by(middle, |a, b| {
                let center = |r: [f64; 4]| r[axis] * 0.5 + r[axis + 2] * 0.5;
                center(a.rect)
                    .total_cmp(&center(b.rect))
                    .then(a.source.cmp(&b.source))
            });
            let (left, right) = entries.split_at_mut(middle);
            let left = Self::build(left, offset, nodes);
            let right = Self::build(right, offset + middle, nodes);
            nodes[node].children = Some([left, right]);
        }
        node
    }

    /// Query normalized logical x/y with physical-axis length distance weights.
    /// `shared_axis` ignores logical y. The caller reverses the chart orientation
    /// for the pointer and swaps axis lengths for horizontal charts.
    /// Returns original rectangle ordinal and the exact candidates examined.
    pub(super) fn nearest(
        &self,
        position: [f64; 2],
        size: [f64; 2],
        shared_axis: bool,
    ) -> (Option<usize>, usize) {
        if self.nodes.is_empty()
            || position.iter().chain(size.iter()).any(|v| !v.is_finite())
            || size.iter().any(|v| *v <= 0.)
        {
            return (None, 0);
        }
        let distance = |r: [f64; 4]| {
            let x = (position[0] - position[0].clamp(r[0], r[2])) * size[0];
            let y = if shared_axis {
                0.
            } else {
                (position[1] - position[1].clamp(r[1], r[3])) * size[1]
            };
            x * x + y * y
        };
        let mut best = (f64::INFINITY, usize::MAX);
        let mut examined = 0;
        self.visit(0, &distance, &mut best, &mut examined);
        ((best.1 != usize::MAX).then_some(best.1), examined)
    }

    fn visit(
        &self,
        node: usize,
        distance: &impl Fn([f64; 4]) -> f64,
        best: &mut (f64, usize),
        examined: &mut usize,
    ) {
        let node = &self.nodes[node];
        if distance(node.rect) > best.0 {
            return;
        }
        if let Some([mut first, mut second]) = node.children {
            if distance(self.nodes[first].rect) > distance(self.nodes[second].rect) {
                std::mem::swap(&mut first, &mut second);
            }
            self.visit(first, distance, best, examined);
            self.visit(second, distance, best, examined);
        } else {
            for entry in &self.entries[node.range.clone()] {
                *examined += 1;
                let candidate = (distance(entry.rect), entry.source);
                if candidate < *best {
                    *best = candidate;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_cache_tracks_data_viewport_axes_and_visibility() -> Result<(), DataError> {
        use super::super::data::{ChartValue, RawPoint, SeriesMark};
        use super::super::scale::{NumericScale, ScaleKind};
        let scale = |domain| NumericScale::new(ScaleKind::Linear, domain).map_err(DataError::Scale);
        let mut series =
            Rc::new(vec![RawSeries::new("west", "y", SeriesMark::Line).points(
                [RawPoint::new("reading", ChartValue::Number(7.), Some(13.))],
            )]);
        let x = ChartScale::Numeric(scale([0., 20.])?);
        let mut axes = vec![ValueAxis {
            id: "y".into(),
            label: "Units".into(),
            scale: scale([0., 100.])?,
        }];
        let mut cache = ProjectionCache::default();
        let first = cache.get(&series, &x, &axes, &[])?;
        assert!(Rc::ptr_eq(&first, &cache.get(&series, &x, &axes, &[])?));
        Rc::make_mut(&mut series)[0].points[0].y = Some(61.);
        let changed = cache.get(&series, &x, &axes, &[])?;
        assert_eq!(
            changed[0].points[0]
                .as_ref()
                .expect("present changed point")
                .y,
            0.61
        );
        assert_eq!(
            first[0].points[0]
                .as_ref()
                .expect("immutable original point")
                .y,
            0.13
        );
        let moved = cache.get(&series, &ChartScale::Numeric(scale([5., 15.])?), &axes, &[])?;
        assert_eq!(moved[0].points[0].as_ref().expect("moved point").x, 0.2);
        axes[0].scale = scale([0., 200.])?;
        let rescaled = cache.get(&series, &x, &axes, &[])?;
        assert_eq!(
            rescaled[0].points[0].as_ref().expect("rescaled point").y,
            0.305
        );
        assert!(cache.get(&series, &x, &axes, &["west".into()])?.is_empty());
        assert_eq!(cache.get(&series, &x, &axes, &[])?.len(), 1);
        Rc::make_mut(&mut series)[0].points[0].y = Some(f64::NAN);
        assert!(cache.get(&series, &x, &axes, &["west".into()]).is_err());
        assert!(cache.get(&series, &x, &axes, &["west".into()]).is_err());
        Rc::make_mut(&mut series)[0].points[0].y = Some(47.);
        assert!(cache.get(&series, &x, &axes, &["west".into()])?.is_empty());
        Ok(())
    }

    #[test]
    fn sampling_retains_asymmetric_extrema_baselines_and_order() {
        let mut points = (0..100)
            .map(|i| [i as f64 / 100., 4., -2.])
            .collect::<Vec<_>>();
        points[13][1] = 91.;
        points[21][1] = -35.;
        points[27][2] = -70.;
        points[32][2] = 43.;
        let kept = sample_path(&points, 2);
        assert_eq!(kept, [0, 13, 21, 27, 32, 49, 50, 99]);
        points.reverse();
        assert_eq!(sample_path(&points, 2), [0, 49, 50, 67, 72, 78, 86, 99]);
    }

    #[test]
    fn boundary_crossings_and_missing_runs_do_not_merge() {
        let points = (0..100)
            .map(|i| [i as f64 / 10. - 4., i as f64, 0.])
            .collect::<Vec<_>>();
        let kept = sample_path(&points, 2);
        assert_eq!(kept, [0, 39, 40, 44, 45, 49, 50, 99]);
        // The renderer samples continuous runs separately; both sides of a
        // missing reading remain endpoints rather than drawing over the gap.
        assert_eq!(sample_path(&points[..30], 1), [0, 29]);
        assert_eq!(sample_path(&points[70..], 1), [0, 29]);
        let mut unordered = points.clone();
        unordered.swap(10, 80);
        assert_eq!(sample_path(&unordered, 2), (0..100).collect::<Vec<_>>());
    }

    #[test]
    fn rectangle_nearest_is_exact_weighted_and_preserves_ties() {
        let rects = [
            [0.2, 0.2, 0.3, 0.3],
            [0.55, 0.6, 0.9, 0.8],
            [0.2, 0.2, 0.3, 0.3],
            [0.9, 0., 0.8, 1.], // inverted clipped rectangle is not hittable
        ];
        let index = HitIndex::new(rects);
        assert_eq!(index.nearest([0.25, 0.25], [800., 90.], false).0, Some(0));
        assert_eq!(index.nearest([0.5, 0.25], [800., 90.], false).0, Some(1));
        assert_eq!(index.nearest([0.5, 0.25], [90., 800.], false).0, Some(0));
        assert_eq!(index.nearest([0.56, 0.1], [90., 800.], true).0, Some(1));
        assert_eq!(
            HitIndex::new([]).nearest([0., 0.], [1., 1.], false),
            (None, 0)
        );
    }

    #[test]
    fn overlapping_branches_keep_original_ties_after_invalid_entries() {
        let rects = std::iter::once([1., 1., 0., 0.]).chain((0..64).map(|_| [0.1, 0.2, 0.8, 0.9]));
        let index = HitIndex::new(rects);
        let (selected, examined) = index.nearest([0.4, 0.3], [751., 139.], false);
        assert_eq!(selected, Some(1));
        // Identical overlapping bounds are deliberately the worst case. Do
        // not prune equal-distance branches and silently change tie identity.
        assert_eq!(examined, 64);
    }

    #[test]
    fn tree_matches_exhaustive_queries_and_bounds_sparse_candidates() {
        for n in [1_000, 10_000, 100_000] {
            let rects = (0..n)
                .map(|i| {
                    let x = i as f64 / n as f64;
                    let y = (i % 13) as f64 / 15.;
                    [x, y, x + 0.1 / n as f64, y + 0.01]
                })
                .collect::<Vec<_>>();
            let index = HitIndex::new(rects.iter().copied());
            for shared in [false, true] {
                for p in [[0.314159_f64, 0.23], [0.9991, 0.71], [0.0003, 0.12]] {
                    let brute = rects
                        .iter()
                        .enumerate()
                        .min_by(|(_, a), (_, b)| {
                            let d = |r: &[f64; 4]| {
                                let dx = (p[0] - p[0].clamp(r[0], r[2])) * 917.;
                                let dy = if shared {
                                    0.
                                } else {
                                    (p[1] - p[1].clamp(r[1], r[3])) * 203.
                                };
                                dx * dx + dy * dy
                            };
                            d(a).total_cmp(&d(b))
                        })
                        .map(|(i, _)| i);
                    let (found, examined) = index.nearest(p, [917., 203.], shared);
                    assert_eq!(found, brute);
                    assert!(examined < n / 4, "{examined} candidates out of {n}");
                }
            }
        }
    }
}
