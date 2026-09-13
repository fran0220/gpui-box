//! Local geographic visualization and bounded GeoJSON ingestion. No network,
//! tiles or provider assets.
//!
//! Coordinates are longitude/latitude degrees. Supported projections are a
//! spherical equirectangular and spherical Web Mercator, not geodetic distance
//! or area calculations. Edges are straight **after projection**, not geodesics.
//! Rings must be explicitly closed, simple, nondegenerate, and have no edge
//! spanning more than 180° longitude in the prepared world. Use [`GeoWorldPolicy`]
//! for an explicit or automatic longitude cut, or supply split polygons. Holes must be strictly
//! inside the exterior, mutually disjoint and not nested or touching. Either
//! winding is accepted. Exterior boundaries hit; hole boundaries do not.
//!
//! All math stays f64 until painting. Validation is quadratic in ring vertices;
//! prepare a [`GeoData`] once and reuse it. Unsupported input is a refusal,
//! never silently clipped, repaired or dropped. See docs/geography.md.
//!
//! ```
//! use gpui_kit::display::{GeoColorDomain, GeoData, GeoMap, GeoPoint,
//!     GeoPosition, GeoProjection, GeoState};
//! use std::rc::Rc;
//! let prepared = GeoData::new(GeoProjection::WebMercator, vec![], vec![
//!     GeoPoint { id: "station-a".into(), label: "Synthetic station".into(),
//!         position: GeoPosition { longitude: 24.0, latitude: -13.0 } }
//! ], GeoColorDomain { minimum: 0.0, maximum: 10.0,
//!     minimum_label: "0".into(), maximum_label: "10".into(),
//!     missing_label: "Unobserved".into() });
//! let state = match prepared {
//!     Ok(data) => GeoState::Ready(Rc::new(data)),
//!     Err(reason) => GeoState::Refused(reason),
//! };
//! let map = GeoMap::new("local-map", "Local observations").state(state);
//! ```

pub mod view;
pub use view::{GeoEvent, GeoMap, GeoState};
mod exploration;
mod ingest;
mod painting;
mod presentation;
mod spatial;
pub use ingest::{GeoProperties, GeoWorldPolicy};
use spatial::{Envelope, SpatialIndex};

use gpui::SharedString;
use std::collections::HashSet;

/// A position in degrees, east and north positive. No altitude is implied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeoPosition {
    pub longitude: f64,
    pub latitude: f64,
}

/// Projected unit-world position; x increases east, y south.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeoProjected {
    pub x: f64,
    pub y: f64,
}

/// Supported spherical projections with zero central meridian.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeoProjection {
    /// x = longitude / 360 + 1/2; y = 1/2 - latitude / 360.
    /// Domain ±180° longitude and ±90° latitude. World height is 1/2.
    Equirectangular,
    /// x = longitude / 360 + 1/2; y = 1/2 - ln(tan(π/4+φ/2))/(2π).
    /// Domain ±180° longitude and ±atan(sinh(π)) latitude. World height is 1.
    WebMercator,
}

/// Explicit input refusal. The whole data set is refused, never partly shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeoRefusal {
    CoordinateBounds,
    AntimeridianEdge,
    InvalidRing,
    InvalidHoles,
    DuplicateIdentity,
    InvalidValue,
    InvalidViewport,
    InvalidGeoJson,
    DocumentLimit,
    InvalidSimplification,
    Unsupported(SharedString),
}

impl GeoRefusal {
    fn string_key(&self) -> Option<crate::strings::StringKey> {
        use crate::strings::StringKey;
        Some(match self {
            Self::CoordinateBounds => StringKey::GeographyCoordinateBounds,
            Self::AntimeridianEdge => StringKey::GeographyAntimeridianEdge,
            Self::InvalidRing => StringKey::GeographyInvalidRing,
            Self::InvalidHoles => StringKey::GeographyInvalidHoles,
            Self::DuplicateIdentity => StringKey::GeographyDuplicateIdentity,
            Self::InvalidValue => StringKey::GeographyInvalidValue,
            Self::InvalidViewport => StringKey::GeographyInvalidViewport,
            Self::InvalidGeoJson => StringKey::GeographyInvalidGeoJson,
            Self::DocumentLimit => StringKey::GeographyDocumentLimit,
            Self::InvalidSimplification => StringKey::GeographyInvalidSimplification,
            Self::Unsupported(_) => return None,
        })
    }

    fn message(&self) -> &str {
        match self {
            Self::Unsupported(reason) => reason.as_ref(),
            _ => self.string_key().expect("built-in refusal key").english(),
        }
    }
}

impl std::fmt::Display for GeoRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for GeoRefusal {}

impl GeoProjection {
    pub fn latitude_limit(self) -> f64 {
        match self {
            Self::Equirectangular => 90.0,
            Self::WebMercator => std::f64::consts::PI.sinh().atan().to_degrees(),
        }
    }

    pub fn project(self, position: GeoPosition) -> Result<GeoProjected, GeoRefusal> {
        let GeoPosition {
            longitude,
            latitude,
        } = position;
        if !longitude.is_finite()
            || !latitude.is_finite()
            || longitude.abs() > 180.0
            || latitude.abs() > self.latitude_limit()
        {
            return Err(GeoRefusal::CoordinateBounds);
        }
        let y = match self {
            Self::Equirectangular => 0.5 - latitude / 360.0,
            Self::WebMercator => 0.5 - latitude.to_radians().tan().asinh() / std::f64::consts::TAU,
        };
        // Validated endpoints can overshoot by one floating-point step.
        Ok(GeoProjected {
            x: longitude / 360.0 + 0.5,
            y: y.clamp(0.0, 1.0),
        })
    }

    pub fn unproject(self, position: GeoProjected) -> Result<GeoPosition, GeoRefusal> {
        let (low, high) = match self {
            Self::Equirectangular => (0.25, 0.75),
            Self::WebMercator => (0.0, 1.0),
        };
        if !position.x.is_finite()
            || !position.y.is_finite()
            || !(0.0..=1.0).contains(&position.x)
            || !(low..=high).contains(&position.y)
        {
            return Err(GeoRefusal::CoordinateBounds);
        }
        Ok(GeoPosition {
            longitude: (position.x - 0.5) * 360.0,
            latitude: match self {
                Self::Equirectangular => (0.5 - position.y) * 360.0,
                Self::WebMercator => ((0.5 - position.y) * std::f64::consts::TAU)
                    .sinh()
                    .atan()
                    .to_degrees(),
            },
        })
    }
}

/// One polygon, with explicit closed rings. Winding direction does not matter.
#[derive(Clone, Debug)]
pub struct GeoPolygon {
    pub exterior: Vec<GeoPosition>,
    pub holes: Vec<Vec<GeoPosition>>,
}

/// Multiple polygons allow caller-cut antimeridian features and islands.
/// Polygon overlaps are allowed and painted as a union under one identity.
#[derive(Clone, Debug)]
pub struct GeoFeature {
    pub id: SharedString,
    pub label: SharedString,
    pub polygons: Vec<GeoPolygon>,
    /// None means no observation, not zero. Formatting belongs to the caller.
    pub value: Option<f64>,
    pub formatted_value: SharedString,
}

/// Caller-owned point overlay. Painted above all polygons; last point wins ties.
#[derive(Clone, Debug)]
pub struct GeoPoint {
    pub id: SharedString,
    pub label: SharedString,
    pub position: GeoPosition,
}

/// Finite, strictly increasing choropleth domain with caller-owned endpoint text.
#[derive(Clone, Debug)]
pub struct GeoColorDomain {
    pub minimum: f64,
    pub maximum: f64,
    pub minimum_label: SharedString,
    pub maximum_label: SharedString,
    pub missing_label: SharedString,
}

#[derive(Clone, Debug, PartialEq)]
struct ProjectedPolygon {
    exterior: Vec<GeoProjected>,
    holes: Vec<Vec<GeoProjected>>,
}

/// Validated immutable data. Validation failures must be presented as refusal.
#[derive(Clone, Debug)]
pub struct GeoData {
    projection: GeoProjection,
    features: Vec<GeoFeature>,
    points: Vec<GeoPoint>,
    polygons: Vec<Vec<ProjectedPolygon>>,
    projected_points: Vec<GeoProjected>,
    domain: GeoColorDomain,
    central_longitude: f64,
    spatial: SpatialIndex,
    point_readings: std::collections::HashMap<SharedString, GeoProperties>,
}

impl GeoData {
    pub fn new(
        projection: GeoProjection,
        features: Vec<GeoFeature>,
        points: Vec<GeoPoint>,
        domain: GeoColorDomain,
    ) -> Result<Self, GeoRefusal> {
        if !domain.minimum.is_finite()
            || !domain.maximum.is_finite()
            || domain.minimum >= domain.maximum
            || !(domain.maximum - domain.minimum).is_finite()
        {
            return Err(GeoRefusal::InvalidValue);
        }
        let mut ids = HashSet::new();
        for id in features
            .iter()
            .map(|f| &f.id)
            .chain(points.iter().map(|p| &p.id))
        {
            if id.is_empty() || !ids.insert(id.clone()) {
                return Err(GeoRefusal::DuplicateIdentity);
            }
        }
        let polygons = features
            .iter()
            .map(|feature| {
                if feature
                    .value
                    .is_some_and(|v| !v.is_finite() || v < domain.minimum || v > domain.maximum)
                {
                    return Err(GeoRefusal::InvalidValue);
                }
                if feature.polygons.is_empty() {
                    return Err(GeoRefusal::InvalidRing);
                }
                feature
                    .polygons
                    .iter()
                    .map(|polygon| {
                        let exterior = project_ring(projection, &polygon.exterior)?;
                        let holes = polygon
                            .holes
                            .iter()
                            .map(|ring| project_ring(projection, ring))
                            .collect::<Result<Vec<_>, _>>()?;
                        for (i, hole) in holes.iter().enumerate() {
                            if locate(&exterior, hole[0]) != Location::Inside
                                || rings_intersect(&exterior, hole)
                            {
                                return Err(GeoRefusal::InvalidHoles);
                            }
                            for other in &holes[..i] {
                                if rings_intersect(other, hole)
                                    || locate(other, hole[0]) != Location::Outside
                                    || locate(hole, other[0]) != Location::Outside
                                {
                                    return Err(GeoRefusal::InvalidHoles);
                                }
                            }
                        }
                        Ok(ProjectedPolygon { exterior, holes })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let projected_points = points
            .iter()
            .map(|p| projection.project(p.position))
            .collect::<Result<Vec<_>, _>>()?;
        let spatial = SpatialIndex::new(&polygons, &projected_points);
        Ok(Self {
            projection,
            features,
            points,
            polygons,
            projected_points,
            domain,
            central_longitude: 0.0,
            spatial,
            point_readings: Default::default(),
        })
    }

    pub fn projection(&self) -> GeoProjection {
        self.projection
    }
    pub fn features(&self) -> &[GeoFeature] {
        &self.features
    }
    pub fn points(&self) -> &[GeoPoint] {
        &self.points
    }
    pub fn color_domain(&self) -> &GeoColorDomain {
        &self.domain
    }

    /// Exact metadata supplied for a GeoJSON point. None means no reading was
    /// supplied (as with native GeoPoint literals); Some with value None is an
    /// explicitly unobserved reading. Point values need not fit the polygon
    /// color domain, but must be finite.
    pub fn point_reading(&self, id: &str) -> Option<&GeoProperties> {
        self.point_readings.get(id)
    }

    fn readout(&self, index: usize) -> (&SharedString, &SharedString, SharedString) {
        if let Some(f) = self.features.get(index) {
            (
                &f.id,
                &f.label,
                if f.value.is_some() {
                    f.formatted_value.clone()
                } else {
                    self.domain.missing_label.clone()
                },
            )
        } else {
            let p = &self.points[index - self.features.len()];
            let value =
                self.point_reading(p.id.as_ref())
                    .map_or_else(SharedString::default, |reading| {
                        if reading.value.is_none() && reading.formatted_value.is_empty() {
                            self.domain.missing_label.clone()
                        } else {
                            reading.formatted_value.clone()
                        }
                    });
            (&p.id, &p.label, value)
        }
    }

    // Semantic visual envelopes, not hit regions. Include clipped real ring
    // edges and covered frame corners; a viewport entirely in a hole has no
    // feature envelope. Rendering still uses GPUI's own clipping primitive.
    fn visual_bounds(&self, viewport: GeoViewport, size: [f64; 2]) -> Vec<(usize, [f64; 4])> {
        if size.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return vec![];
        }
        let mut result = Vec::new();
        let candidates = self.visible_indices(viewport, size);
        for index in candidates
            .iter()
            .copied()
            .take_while(|i| *i < self.polygons.len())
        {
            let polygons = &self.polygons[index];
            let mut visible = Vec::new();
            for polygon in polygons {
                for ring in std::iter::once(&polygon.exterior).chain(&polygon.holes) {
                    for edge in ring.windows(2) {
                        if let Some(segment) = clipped_segment(
                            viewport.screen(edge[0], size),
                            viewport.screen(edge[1], size),
                            size,
                        ) {
                            visible.extend(segment);
                        }
                    }
                }
                for corner in [
                    [0.0, 0.0],
                    [size[0], 0.0],
                    [size[0], size[1]],
                    [0.0, size[1]],
                ] {
                    let world = viewport.world(corner, size);
                    if locate(&polygon.exterior, world) != Location::Outside
                        && polygon
                            .holes
                            .iter()
                            .all(|hole| locate(hole, world) == Location::Outside)
                    {
                        visible.push(corner);
                    }
                }
            }
            if !visible.is_empty() {
                let minimum = visible
                    .iter()
                    .fold([f64::INFINITY; 2], |a, b| [a[0].min(b[0]), a[1].min(b[1])]);
                let maximum = visible.iter().fold([f64::NEG_INFINITY; 2], |a, b| {
                    [a[0].max(b[0]), a[1].max(b[1])]
                });
                if maximum[0] > minimum[0] && maximum[1] > minimum[1] {
                    result.push((
                        index,
                        [
                            minimum[0],
                            minimum[1],
                            maximum[0] - minimum[0],
                            maximum[1] - minimum[1],
                        ],
                    ));
                }
            }
        }
        for index in candidates
            .iter()
            .copied()
            .filter(|i| *i >= self.features.len())
            .map(|i| i - self.features.len())
        {
            let position = &self.projected_points[index];
            let p = viewport.screen(*position, size);
            let dx = (p[0] - p[0].clamp(0.0, size[0])).abs();
            let dy = (p[1] - p[1].clamp(0.0, size[1])).abs();
            if dx.hypot(dy) >= 5.0 {
                continue;
            }
            let rx = (25.0 - dy * dy).sqrt();
            let ry = (25.0 - dx * dx).sqrt();
            let left = (p[0] - rx).max(0.0);
            let top = (p[1] - ry).max(0.0);
            let right = (p[0] + rx).min(size[0]);
            let bottom = (p[1] + ry).min(size[1]);
            result.push((
                self.features.len() + index,
                [left, top, right - left, bottom - top],
            ));
        }
        result
    }

    /// Screen-local hit in pixels. Same transform and five-pixel point radius as
    /// painting. Outside-frame hits return None. Points win over polygons;
    /// overlapping polygons resolve to the last caller-supplied feature.
    pub fn hit_test(
        &self,
        viewport: GeoViewport,
        size: [f64; 2],
        position: [f64; 2],
    ) -> Option<SharedString> {
        self.hit_test_visible(viewport, size, position, |_| true)
    }

    fn hit_test_visible(
        &self,
        viewport: GeoViewport,
        size: [f64; 2],
        position: [f64; 2],
        visible: impl Fn(usize) -> bool,
    ) -> Option<SharedString> {
        if viewport.validate().is_err()
            || size.iter().any(|v| !v.is_finite() || *v <= 0.0)
            || position
                .iter()
                .zip(size)
                .any(|(p, limit)| !p.is_finite() || *p < 0.0 || *p > limit)
        {
            return None;
        }
        let world = viewport.world(position, size);
        let radius = 5.0 / (size[0].min(size[1]) * viewport.zoom);
        let candidates = self.spatial.query(Envelope([
            world.x - radius,
            world.y - radius,
            world.x + radius,
            world.y + radius,
        ]));
        for index in candidates.into_iter().rev() {
            if !visible(index) {
                continue;
            }
            if index >= self.features.len() {
                let point_index = index - self.features.len();
                let at = viewport.screen(self.projected_points[point_index], size);
                if (at[0] - position[0]).hypot(at[1] - position[1]) <= 5.0 {
                    return Some(self.points[point_index].id.clone());
                }
            } else if self.polygons[index].iter().any(|p| {
                locate(&p.exterior, world) != Location::Outside
                    && p.holes
                        .iter()
                        .all(|h| locate(h, world) == Location::Outside)
            }) {
                return Some(self.features[index].id.clone());
            }
        }
        None
    }
}

/// Controlled unit-world camera. Uniform scale preserves projection aspect.
/// The full unit square fits initially; equirectangular has unused polar bands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeoViewport {
    pub center: GeoProjected,
    pub zoom: f64,
}

impl Default for GeoViewport {
    fn default() -> Self {
        Self {
            center: GeoProjected { x: 0.5, y: 0.5 },
            zoom: 1.0,
        }
    }
}

impl GeoViewport {
    pub fn validate(self) -> Result<(), GeoRefusal> {
        if !self.center.x.is_finite()
            || !self.center.y.is_finite()
            || !self.zoom.is_finite()
            || !(0.0..=1.0).contains(&self.center.x)
            || !(0.0..=1.0).contains(&self.center.y)
            || !(1.0..=64.0).contains(&self.zoom)
        {
            Err(GeoRefusal::InvalidViewport)
        } else {
            Ok(())
        }
    }

    pub fn screen(self, position: GeoProjected, size: [f64; 2]) -> [f64; 2] {
        let scale = size[0].min(size[1]) * self.zoom;
        [
            size[0] / 2.0 + (position.x - self.center.x) * scale,
            size[1] / 2.0 + (position.y - self.center.y) * scale,
        ]
    }

    pub fn world(self, position: [f64; 2], size: [f64; 2]) -> GeoProjected {
        let scale = size[0].min(size[1]) * self.zoom;
        GeoProjected {
            x: self.center.x + (position[0] - size[0] / 2.0) / scale,
            y: self.center.y + (position[1] - size[1] / 2.0) / scale,
        }
    }

    /// Pan camera in unit-world coordinates; clamps center at the world edge.
    pub fn pan(self, dx: f64, dy: f64) -> Self {
        if !dx.is_finite() || !dy.is_finite() {
            return self;
        }
        Self {
            center: GeoProjected {
                x: (self.center.x + dx).clamp(0.0, 1.0),
                y: (self.center.y + dy).clamp(0.0, 1.0),
            },
            ..self
        }
    }

    /// Zoom about a projected anchor. At center limits the anchor may move.
    pub fn zoom_at(self, factor: f64, anchor: GeoProjected) -> Self {
        if !factor.is_finite() || factor <= 0.0 || !anchor.x.is_finite() || !anchor.y.is_finite() {
            return self;
        }
        let zoom = (self.zoom * factor).clamp(1.0, 64.0);
        Self {
            center: GeoProjected {
                x: (anchor.x - (anchor.x - self.center.x) * self.zoom / zoom).clamp(0.0, 1.0),
                y: (anchor.y - (anchor.y - self.center.y) * self.zoom / zoom).clamp(0.0, 1.0),
            },
            zoom,
        }
    }
}

// Predicates operate on normalized projected coordinates. Near-collinear
// inputs within 1e-12 unit-world are treated as touching and refused.
const EPSILON: f64 = 1e-12;

// Parametric segment restriction, used only to measure the visible semantic
// envelope of geographic geometry. It neither paints nor handles input.
fn clipped_segment(a: [f64; 2], b: [f64; 2], size: [f64; 2]) -> Option<[[f64; 2]; 2]> {
    let mut low: f64 = 0.0;
    let mut high: f64 = 1.0;
    for axis in 0..2 {
        let delta = b[axis] - a[axis];
        if delta == 0.0 {
            if a[axis] < 0.0 || a[axis] > size[axis] {
                return None;
            }
        } else {
            let start = -a[axis] / delta;
            let end = (size[axis] - a[axis]) / delta;
            low = low.max(start.min(end));
            high = high.min(start.max(end));
        }
    }
    if low > high {
        return None;
    }
    Some([low, high].map(|t| {
        [
            (a[0] + (b[0] - a[0]) * t).clamp(0.0, size[0]),
            (a[1] + (b[1] - a[1]) * t).clamp(0.0, size[1]),
        ]
    }))
}

fn cross(a: GeoProjected, b: GeoProjected, c: GeoProjected) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn on_segment(a: GeoProjected, b: GeoProjected, p: GeoProjected) -> bool {
    cross(a, b, p).abs() <= EPSILON
        && p.x >= a.x.min(b.x) - EPSILON
        && p.x <= a.x.max(b.x) + EPSILON
        && p.y >= a.y.min(b.y) - EPSILON
        && p.y <= a.y.max(b.y) + EPSILON
}

fn intersects(a: GeoProjected, b: GeoProjected, c: GeoProjected, d: GeoProjected) -> bool {
    on_segment(a, b, c)
        || on_segment(a, b, d)
        || on_segment(c, d, a)
        || on_segment(c, d, b)
        || ((cross(a, b, c) > 0.0) != (cross(a, b, d) > 0.0)
            && (cross(c, d, a) > 0.0) != (cross(c, d, b) > 0.0))
}

fn rings_intersect(a: &[GeoProjected], b: &[GeoProjected]) -> bool {
    a.windows(2)
        .any(|a| b.windows(2).any(|b| intersects(a[0], a[1], b[0], b[1])))
}

#[derive(PartialEq)]
enum Location {
    Outside,
    Boundary,
    Inside,
}

fn locate(ring: &[GeoProjected], p: GeoProjected) -> Location {
    let mut inside = false;
    for edge in ring.windows(2) {
        let (a, b) = (edge[0], edge[1]);
        if on_segment(a, b, p) {
            return Location::Boundary;
        }
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    if inside {
        Location::Inside
    } else {
        Location::Outside
    }
}

fn project_ring(
    projection: GeoProjection,
    ring: &[GeoPosition],
) -> Result<Vec<GeoProjected>, GeoRefusal> {
    if ring.len() < 4 || ring.first() != ring.last() {
        return Err(GeoRefusal::InvalidRing);
    }
    let points = ring
        .iter()
        .map(|p| projection.project(*p))
        .collect::<Result<Vec<_>, _>>()?;
    if ring
        .windows(2)
        .any(|e| (e[0].longitude - e[1].longitude).abs() > 180.0)
    {
        return Err(GeoRefusal::AntimeridianEdge);
    }
    validate_projected_ring(&points)?;
    Ok(points)
}

fn validate_projected_ring(points: &[GeoProjected]) -> Result<(), GeoRefusal> {
    if points.len() < 4 || points.first() != points.last() {
        return Err(GeoRefusal::InvalidRing);
    }
    let n = points.len() - 1;
    for i in 0..n {
        if (points[i].x - points[i + 1].x).hypot(points[i].y - points[i + 1].y) <= EPSILON {
            return Err(GeoRefusal::InvalidRing);
        }
        // Reject collinear reversals at adjacent edges as well as non-adjacent intersections.
        let previous = points[(i + n - 1) % n];
        if on_segment(previous, points[i], points[i + 1])
            || on_segment(points[i], points[i + 1], previous)
        {
            return Err(GeoRefusal::InvalidRing);
        }
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if intersects(points[i], points[i + 1], points[j], points[j + 1]) {
                return Err(GeoRefusal::InvalidRing);
            }
        }
    }
    let area: f64 = points
        .windows(2)
        .map(|e| e[0].x * e[1].y - e[1].x * e[0].y)
        .sum();
    if area.abs() <= EPSILON {
        return Err(GeoRefusal::InvalidRing);
    }
    Ok(())
}

#[cfg(test)]
mod experience_tests;
#[cfg(test)]
mod tests;
