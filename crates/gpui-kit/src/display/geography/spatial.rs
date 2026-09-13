//! Immutable projected bounding-volume tree. Original implementation; candidates
//! preserve caller order, while exact polygon predicates decide the final hit.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct Envelope(pub [f64; 4]);

impl Envelope {
    fn intersects(self, other: Self) -> bool {
        self.0[0] <= other.0[2]
            && self.0[2] >= other.0[0]
            && self.0[1] <= other.0[3]
            && self.0[3] >= other.0[1]
    }
    fn union(self, other: Self) -> Self {
        Self([
            self.0[0].min(other.0[0]),
            self.0[1].min(other.0[1]),
            self.0[2].max(other.0[2]),
            self.0[3].max(other.0[3]),
        ])
    }
    pub(super) fn points(points: impl Iterator<Item = GeoProjected>) -> Self {
        points.fold(
            Self([
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ]),
            |bounds, p| bounds.union(Self([p.x, p.y, p.x, p.y])),
        )
    }
}

#[derive(Clone, Debug)]
enum Node {
    Leaf {
        bounds: Envelope,
        items: Vec<(Envelope, usize)>,
    },
    Branch {
        bounds: Envelope,
        left: Box<Node>,
        right: Box<Node>,
    },
}

impl Node {
    fn build(mut items: Vec<(Envelope, usize)>) -> Self {
        let bounds = items.iter().fold(items[0].0, |all, (b, _)| all.union(*b));
        if items.len() <= 8 {
            return Self::Leaf { bounds, items };
        }
        let axis = usize::from(bounds.0[3] - bounds.0[1] > bounds.0[2] - bounds.0[0]);
        items.sort_by(|a, b| {
            (a.0.0[axis] + a.0.0[axis + 2]).total_cmp(&(b.0.0[axis] + b.0.0[axis + 2]))
        });
        let right = items.split_off(items.len() / 2);
        Self::Branch {
            bounds,
            left: Box::new(Self::build(items)),
            right: Box::new(Self::build(right)),
        }
    }
    fn query(&self, region: Envelope, into: &mut Vec<usize>) {
        match self {
            Self::Leaf { bounds, items } if bounds.intersects(region) => {
                into.extend(
                    items
                        .iter()
                        .filter(|(b, _)| b.intersects(region))
                        .map(|(_, i)| *i),
                );
            }
            Self::Branch {
                bounds,
                left,
                right,
            } if bounds.intersects(region) => {
                left.query(region, into);
                right.query(region, into);
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct SpatialIndex(Option<Node>);

impl SpatialIndex {
    pub(super) fn new(polygons: &[Vec<ProjectedPolygon>], points: &[GeoProjected]) -> Self {
        // Index each polygon independently so distant islands do not turn one
        // feature into a world-sized candidate. Query deduplicates identities.
        let mut items: Vec<_> = polygons
            .iter()
            .enumerate()
            .flat_map(|(i, parts)| {
                parts
                    .iter()
                    .map(move |p| (Envelope::points(p.exterior.iter().copied()), i))
            })
            .collect();
        items.extend(
            points
                .iter()
                .enumerate()
                .map(|(i, p)| (Envelope([p.x, p.y, p.x, p.y]), polygons.len() + i)),
        );
        Self((!items.is_empty()).then(|| Node::build(items)))
    }
    pub(super) fn query(&self, bounds: Envelope) -> Vec<usize> {
        let mut result = Vec::new();
        if let Some(root) = &self.0 {
            root.query(bounds, &mut result);
        }
        result.sort_unstable();
        result.dedup();
        result
    }
}

impl GeoData {
    /// Candidate source identities in original paint order. Exact geometry
    /// clipping can discard additional candidates. Includes the five-pixel
    /// point radius, but never changes which source identity a hit returns.
    pub fn visible_indices(&self, viewport: GeoViewport, size: [f64; 2]) -> Vec<usize> {
        if viewport.validate().is_err() || size.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return vec![];
        }
        let a = viewport.world([-5.0, -5.0], size);
        let b = viewport.world([size[0] + 5.0, size[1] + 5.0], size);
        self.spatial.query(Envelope([a.x, a.y, b.x, b.y]))
    }

    /// Fit the prepared geometry to a measured frame, with explicit pixel
    /// padding. Empty data, unusable frame or nonfinite padding returns None.
    /// Zoom and center respect the same camera limits as every other input.
    pub fn fit_viewport(&self, size: [f64; 2], padding: f64) -> Option<GeoViewport> {
        if size.iter().any(|v| !v.is_finite() || *v <= 0.0)
            || !padding.is_finite()
            || padding < 0.0
            || size[0].min(size[1]) <= 2.0 * (padding + 5.0)
        {
            return None;
        }
        let b = Envelope::points(
            self.polygons
                .iter()
                .flatten()
                .flat_map(|p| p.exterior.iter().copied())
                .chain(self.projected_points.iter().copied()),
        )
        .0;
        if !b[0].is_finite() {
            return None;
        }
        let unit = size[0].min(size[1]);
        let room = [
            size[0] - 2.0 * (padding + 5.0),
            size[1] - 2.0 * (padding + 5.0),
        ];
        let zoom = (room[0] / ((b[2] - b[0]) * unit))
            .min(room[1] / ((b[3] - b[1]) * unit))
            .clamp(1.0, 64.0);
        Some(GeoViewport {
            center: GeoProjected {
                x: ((b[0] + b[2]) / 2.0).clamp(0.0, 1.0),
                y: ((b[1] + b[3]) / 2.0).clamp(0.0, 1.0),
            },
            zoom,
        })
    }

    /// A separate immutable display level with the same source features,
    /// values and IDs. Tolerance is in projected unit-world distance, not
    /// degrees. Hit testing, clipping and painting all use the returned level.
    /// Each polygon is simplified only when its full topology remains valid;
    /// otherwise that polygon is retained exactly. No promise of shared-edge
    /// preservation between independent features is made.
    pub fn simplified(&self, tolerance: f64) -> Result<Self, GeoRefusal> {
        if !tolerance.is_finite() || !(0.0..=0.01).contains(&tolerance) {
            return Err(GeoRefusal::InvalidSimplification);
        }
        let mut result = self.clone();
        if tolerance == 0.0 {
            return Ok(result);
        }
        for parts in &mut result.polygons {
            for polygon in parts {
                let candidate = ProjectedPolygon {
                    exterior: simplify_ring(&polygon.exterior, tolerance),
                    holes: polygon
                        .holes
                        .iter()
                        .map(|h| simplify_ring(h, tolerance))
                        .collect(),
                };
                if valid_projected_polygon(&candidate) {
                    *polygon = candidate;
                }
            }
        }
        result.spatial = SpatialIndex::new(&result.polygons, &result.projected_points);
        Ok(result)
    }

    /// Prepared vertex count, including closing vertices. Useful for reporting
    /// actual simplification rather than promising a fixed reduction ratio.
    pub fn vertex_count(&self) -> usize {
        self.polygons
            .iter()
            .flatten()
            .map(|p| p.exterior.len() + p.holes.iter().map(Vec::len).sum::<usize>())
            .sum()
    }
}

fn simplify_ring(ring: &[GeoProjected], tolerance: f64) -> Vec<GeoProjected> {
    // Iterative Douglas-Peucker on two arcs split at the farthest vertex from
    // the first. Retained vertices are exact source vertices, never synthetic.
    let end = ring.len() - 1;
    let split = (1..end)
        .max_by(|&a, &b| {
            let distance = |i: usize| (ring[i].x - ring[0].x).hypot(ring[i].y - ring[0].y);
            distance(a).total_cmp(&distance(b))
        })
        .unwrap_or(1);
    let mut keep = vec![false; ring.len()];
    keep[0] = true;
    keep[split] = true;
    keep[end] = true;
    let mut work = vec![(0, split), (split, end)];
    while let Some((a, b)) = work.pop() {
        let (mut best, mut distance) = (a, tolerance);
        for i in a + 1..b {
            let dx = ring[b].x - ring[a].x;
            let dy = ring[b].y - ring[a].y;
            let t = (((ring[i].x - ring[a].x) * dx + (ring[i].y - ring[a].y) * dy)
                / (dx * dx + dy * dy))
                .clamp(0.0, 1.0);
            let d = (ring[i].x - ring[a].x - t * dx).hypot(ring[i].y - ring[a].y - t * dy);
            if d > distance {
                distance = d;
                best = i;
            }
        }
        if best != a {
            keep[best] = true;
            work.extend([(a, best), (best, b)]);
        }
    }
    let result: Vec<_> = ring
        .iter()
        .zip(keep)
        .filter_map(|(p, keep)| keep.then_some(*p))
        .collect();
    if result.len() < 4 {
        ring.to_vec()
    } else {
        result
    }
}

fn valid_projected_polygon(polygon: &ProjectedPolygon) -> bool {
    let valid_ring = |ring: &Vec<GeoProjected>| validate_projected_ring(ring).is_ok();
    if !valid_ring(&polygon.exterior) || polygon.holes.iter().any(|h| !valid_ring(h)) {
        return false;
    }
    for (i, hole) in polygon.holes.iter().enumerate() {
        if locate(&polygon.exterior, hole[0]) != Location::Inside
            || rings_intersect(&polygon.exterior, hole)
        {
            return false;
        }
        for other in &polygon.holes[..i] {
            if rings_intersect(hole, other)
                || locate(hole, other[0]) != Location::Outside
                || locate(other, hole[0]) != Location::Outside
            {
                return false;
            }
        }
    }
    true
}
