//! Bounded local RFC 7946 ingestion. This module never resolves a URI.
use super::*;
use serde_json::Value;

/// One finite, non-repeating world. Auto chooses a longitude cut in the
/// largest empty vertex gap, putting seam-crossing local regions together.
/// It does not infer spherical interiors: if any edge still crosses the
/// chosen cut or spans more than a hemisphere the collection is refused.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum GeoWorldPolicy {
    #[default]
    Fixed,
    Auto,
    /// Central longitude in [-180,180]. Useful for stable camera coordinates
    /// across data refreshes; Auto may choose a different world on each load.
    Centered(f64),
}

/// Caller interpretation of a GeoJSON feature's properties. No application
/// property names, numeric formatting, or fallback IDs are invented by Kit.
#[derive(Clone, Debug)]
pub struct GeoProperties {
    pub label: SharedString,
    pub value: Option<f64>,
    pub formatted_value: SharedString,
}

impl GeoData {
    /// RFC 7946 Feature or FeatureCollection with explicit string/integer IDs;
    /// Polygon, MultiPolygon and Point only. Two-dimensional WGS84 degrees.
    /// Null geometry, foreign CRS, nonfinite coordinates, unsupported geometry
    /// types and additional dimensions refuse the entire document. Foreign
    /// properties are passed unchanged to the caller; bbox is not trusted for
    /// geometry. Maximum document size is 32 MiB; no network or URI resolution.
    pub fn from_geojson(
        json: &str,
        projection: GeoProjection,
        world: GeoWorldPolicy,
        domain: GeoColorDomain,
        mut properties: impl FnMut(&str, &Value) -> Result<GeoProperties, GeoRefusal>,
    ) -> Result<Self, GeoRefusal> {
        if json.len() > 32 * 1024 * 1024 {
            return Err(GeoRefusal::DocumentLimit);
        }
        let root: Value = serde_json::from_str(json).map_err(|_| GeoRefusal::InvalidGeoJson)?;
        if root.get("crs").is_some() {
            return Err(GeoRefusal::InvalidGeoJson);
        }
        let members: Vec<&Value> = match root.get("type").and_then(Value::as_str) {
            Some("Feature") => vec![&root],
            Some("FeatureCollection") => root
                .get("features")
                .and_then(Value::as_array)
                .ok_or(GeoRefusal::InvalidGeoJson)?
                .iter()
                .collect(),
            _ => return Err(GeoRefusal::InvalidGeoJson),
        };
        let mut features = Vec::new();
        let mut points = Vec::new();
        let mut readings = std::collections::HashMap::new();
        for member in members {
            if member.get("type").and_then(Value::as_str) != Some("Feature")
                || member.get("crs").is_some()
            {
                return Err(GeoRefusal::InvalidGeoJson);
            }
            let id = match member.get("id") {
                Some(Value::String(id)) => id.clone(),
                Some(Value::Number(id)) if id.is_i64() || id.is_u64() => id.to_string(),
                _ => return Err(GeoRefusal::DuplicateIdentity),
            };
            let attrs = member.get("properties").ok_or(GeoRefusal::InvalidGeoJson)?;
            if !attrs.is_null() && !attrs.is_object() {
                return Err(GeoRefusal::InvalidGeoJson);
            }
            let props = properties(&id, attrs)?;
            let geometry = member.get("geometry").ok_or(GeoRefusal::InvalidGeoJson)?;
            if geometry.get("crs").is_some() {
                return Err(GeoRefusal::InvalidGeoJson);
            }
            let coordinates = geometry
                .get("coordinates")
                .ok_or(GeoRefusal::InvalidGeoJson)?;
            let polygons = match geometry.get("type").and_then(Value::as_str) {
                Some("Point") => {
                    if props.value.is_some_and(|value| !value.is_finite()) {
                        return Err(GeoRefusal::InvalidValue);
                    }
                    let id: SharedString = id.into();
                    points.push(GeoPoint {
                        id: id.clone(),
                        label: props.label.clone(),
                        position: position(coordinates)?,
                    });
                    readings.insert(id, props);
                    continue;
                }
                Some("Polygon") => vec![polygon(coordinates)?],
                Some("MultiPolygon") => array(coordinates)?
                    .iter()
                    .map(polygon)
                    .collect::<Result<Vec<_>, _>>()?,
                _ => return Err(GeoRefusal::InvalidGeoJson),
            };
            features.push(GeoFeature {
                id: id.into(),
                label: props.label,
                polygons,
                value: props.value,
                formatted_value: props.formatted_value,
            });
        }
        let mut data = Self::with_world(projection, features, points, domain, world)?;
        data.point_readings = readings;
        Ok(data)
    }

    /// Prepare caller geometry in a fixed, centered or automatically chosen
    /// single world. Sources remain in original degrees in features()/points().
    /// Use project_position/unproject_position for camera and overlay work in
    /// that world, never the zero-meridian projection directly.
    pub fn with_world(
        projection: GeoProjection,
        features: Vec<GeoFeature>,
        points: Vec<GeoPoint>,
        domain: GeoColorDomain,
        world: GeoWorldPolicy,
    ) -> Result<Self, GeoRefusal> {
        let mut longitudes = Vec::new();
        for position in features
            .iter()
            .flat_map(|f| &f.polygons)
            .flat_map(|p| std::iter::once(&p.exterior).chain(&p.holes))
            .flatten()
            .chain(points.iter().map(|p| &p.position))
        {
            projection.project(*position)?;
            longitudes.push(position.longitude);
        }
        let center = match world {
            GeoWorldPolicy::Fixed => 0.0,
            GeoWorldPolicy::Centered(center)
                if center.is_finite() && (-180.0..=180.0).contains(&center) =>
            {
                center
            }
            GeoWorldPolicy::Centered(_) => return Err(GeoRefusal::CoordinateBounds),
            GeoWorldPolicy::Auto => {
                longitudes.sort_by(f64::total_cmp);
                longitudes.dedup();
                if longitudes.is_empty() {
                    0.0
                } else {
                    let mut best = (0.0, 0.0);
                    for i in 0..longitudes.len() {
                        let a = longitudes[i];
                        let b = if i + 1 == longitudes.len() {
                            longitudes[0] + 360.0
                        } else {
                            longitudes[i + 1]
                        };
                        if b - a > best.0 {
                            best = (b - a, (a + b) / 2.0);
                        }
                    }
                    normalize(best.1 + 180.0)
                }
            }
        };
        let transform = |p: &mut GeoPosition| {
            p.longitude = shifted(p.longitude, center);
        };
        let mut transformed = features.clone();
        let mut transformed_points = points.clone();
        for p in transformed
            .iter_mut()
            .flat_map(|f| &mut f.polygons)
            .flat_map(|p| std::iter::once(&mut p.exterior).chain(&mut p.holes))
            .flatten()
        {
            transform(p);
        }
        for p in &mut transformed_points {
            transform(&mut p.position);
        }
        let mut data = Self::new(projection, transformed, transformed_points, domain)?;
        data.features = features;
        data.points = points;
        data.central_longitude = center;
        Ok(data)
    }

    pub fn central_longitude(&self) -> f64 {
        self.central_longitude
    }

    pub fn project_position(&self, position: GeoPosition) -> Result<GeoProjected, GeoRefusal> {
        self.projection.project(position)?;
        self.projection.project(GeoPosition {
            longitude: shifted(position.longitude, self.central_longitude),
            ..position
        })
    }

    pub fn unproject_position(&self, position: GeoProjected) -> Result<GeoPosition, GeoRefusal> {
        let mut p = self.projection.unproject(position)?;
        p.longitude = normalize(p.longitude + self.central_longitude);
        Ok(p)
    }
}

fn shifted(longitude: f64, center: f64) -> f64 {
    if center == 0.0 {
        longitude
    } else {
        normalize(longitude - center)
    }
}
fn normalize(longitude: f64) -> f64 {
    (longitude + 180.0).rem_euclid(360.0) - 180.0
}
fn array(value: &Value) -> Result<&Vec<Value>, GeoRefusal> {
    value.as_array().ok_or(GeoRefusal::InvalidRing)
}
fn position(value: &Value) -> Result<GeoPosition, GeoRefusal> {
    let coords = array(value)?;
    if coords.len() != 2 {
        return Err(GeoRefusal::InvalidGeoJson);
    }
    Ok(GeoPosition {
        longitude: coords[0].as_f64().ok_or(GeoRefusal::CoordinateBounds)?,
        latitude: coords[1].as_f64().ok_or(GeoRefusal::CoordinateBounds)?,
    })
}
fn polygon(value: &Value) -> Result<GeoPolygon, GeoRefusal> {
    let mut rings = array(value)?.iter().map(|r| {
        array(r)?
            .iter()
            .map(position)
            .collect::<Result<Vec<_>, _>>()
    });
    let exterior = rings.next().ok_or(GeoRefusal::InvalidRing)??;
    let holes = rings.collect::<Result<Vec<_>, _>>()?;
    Ok(GeoPolygon { exterior, holes })
}
