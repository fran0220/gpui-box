//! Raw-data coordinates shared by Cartesian and specialized charts.
//!
//! Domain order is meaningful: reversed domains reverse the mapping. Values
//! outside a domain extrapolate; clipping is a renderer policy. Missing values
//! are not zero. Time coordinates are UTC Unix milliseconds; formatting and
//! timezone policy belong to the caller. No source is ported from D3/ECharts.

use gpui::SharedString;

/// A rejected coordinate contract, never silently replaced by an empty chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleError {
    Empty,
    NonFinite,
    InvalidLogDomain,
    DuplicateCategory,
    UnrepresentableDomain,
}

/// Continuous coordinate interpretation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleKind {
    Linear,
    /// UTC Unix milliseconds, with fixed-duration ticks (not calendar months).
    Time,
    Log,
}

/// An invertible f64 scale. Constant input expands by 5% (one unit at zero).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericScale {
    kind: ScaleKind,
    domain: [f64; 2],
    transformed: [f64; 2],
}

impl NumericScale {
    pub fn new(kind: ScaleKind, mut domain: [f64; 2]) -> Result<Self, ScaleError> {
        if domain.iter().any(|v| !v.is_finite()) {
            return Err(ScaleError::NonFinite);
        }
        if kind == ScaleKind::Log && domain.iter().any(|v| *v <= 0.0) {
            return Err(ScaleError::InvalidLogDomain);
        }
        if domain[0] == domain[1] {
            let original = domain[0];
            if kind == ScaleKind::Log {
                domain = [domain[0] / 2.0, domain[1] * 2.0];
            } else {
                let delta = if domain[0] == 0.0 {
                    1.0
                } else {
                    domain[0].abs() * 0.05
                };
                domain = [domain[0] - delta, domain[1] + delta];
            }
            if domain[0] == original {
                domain[0] = original.next_down();
            }
            if domain[1] == original {
                domain[1] = original.next_up();
            }
            if !domain[0].is_finite() || (kind == ScaleKind::Log && domain[0] <= 0.0) {
                domain[0] = original;
            }
            if !domain[1].is_finite() {
                domain[1] = original;
            }
        }
        let transformed = domain.map(|v| if kind == ScaleKind::Log { v.ln() } else { v });
        let span = transformed[1] - transformed[0];
        if !span.is_finite() || span == 0.0 || domain.iter().any(|v| !v.is_finite()) {
            return Err(ScaleError::UnrepresentableDomain);
        }
        Ok(Self {
            kind,
            domain,
            transformed,
        })
    }

    /// Infer from present values. Non-finite values are errors, not missing data.
    pub fn extent(
        kind: ScaleKind,
        values: impl IntoIterator<Item = Option<f64>>,
        include_zero: bool,
    ) -> Result<Self, ScaleError> {
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for value in values.into_iter().flatten() {
            if !value.is_finite() {
                return Err(ScaleError::NonFinite);
            }
            low = low.min(value);
            high = high.max(value);
        }
        if low == f64::INFINITY {
            return Err(ScaleError::Empty);
        }
        if include_zero {
            low = low.min(0.0);
            high = high.max(0.0);
        }
        Self::new(kind, [low, high])
    }

    pub fn domain(self) -> [f64; 2] {
        self.domain
    }
    pub fn kind(self) -> ScaleKind {
        self.kind
    }

    pub fn map(self, value: f64) -> Option<f64> {
        if !value.is_finite() || (self.kind == ScaleKind::Log && value <= 0.0) {
            return None;
        }
        if value == self.domain[0] {
            return Some(0.0);
        }
        if value == self.domain[1] {
            return Some(1.0);
        }
        let value = if self.kind == ScaleKind::Log {
            value.ln()
        } else {
            value
        };
        let result = (value - self.transformed[0]) / (self.transformed[1] - self.transformed[0]);
        result.is_finite().then_some(result)
    }

    pub fn invert(self, fraction: f64) -> Option<f64> {
        if !fraction.is_finite() {
            return None;
        }
        if fraction == 0.0 {
            return Some(self.domain[0]);
        }
        if fraction == 1.0 {
            return Some(self.domain[1]);
        }
        let value = (1.0 - fraction) * self.transformed[0] + fraction * self.transformed[1];
        let result = if self.kind == ScaleKind::Log {
            value.exp()
        } else {
            value
        };
        (result.is_finite() && (self.kind != ScaleKind::Log || result > 0.0)).then_some(result)
    }

    /// Pan in screen fractions. Positive motion moves the data to the right.
    pub fn pan(self, fraction: f64) -> Result<Self, ScaleError> {
        self.window([-fraction, 1.0 - fraction])
    }

    /// Zoom around a pointer fraction; factors above one zoom in.
    pub fn zoom(self, anchor: f64, factor: f64) -> Result<Self, ScaleError> {
        if !factor.is_finite() || factor <= 0.0 || !anchor.is_finite() {
            return Err(ScaleError::NonFinite);
        }
        self.window([anchor - anchor / factor, anchor + (1.0 - anchor) / factor])
    }

    /// Apply a brush selection in screen fractions, retaining its direction.
    pub fn window(self, fractions: [f64; 2]) -> Result<Self, ScaleError> {
        if fractions[0] == fractions[1] {
            return Err(ScaleError::UnrepresentableDomain);
        }
        Self::new(
            self.kind,
            [
                self.invert(fractions[0]).ok_or(ScaleError::NonFinite)?,
                self.invert(fractions[1]).ok_or(ScaleError::NonFinite)?,
            ],
        )
    }

    /// Bounded, in-domain ticks in display order; count is a density hint.
    pub fn ticks(self, count: usize) -> Vec<f64> {
        let low = self.transformed[0].min(self.transformed[1]);
        let high = self.transformed[0].max(self.transformed[1]);
        let rough = (high - low) / count.clamp(2, 100).saturating_sub(1) as f64;
        let magnitude = 10.0_f64.powf(rough.log10().floor());
        let ratio = rough / magnitude;
        let mut step = magnitude
            * if ratio <= 1.5 {
                1.0
            } else if ratio <= 3.0 {
                2.0
            } else if ratio <= 7.0 {
                5.0
            } else {
                10.0
            };
        if self.kind == ScaleKind::Time {
            const INTERVALS: &[f64] = &[
                1., 5., 10., 50., 100., 250., 500., 1000., 5000., 15000., 30000., 60000., 300000.,
                900000., 1800000., 3600000., 10800000., 21600000., 43200000., 86400000.,
                604800000.,
            ];
            if let Some(interval) = INTERVALS.iter().find(|interval| **interval >= rough) {
                step = *interval;
            }
        }
        if !step.is_finite() || step <= 0.0 {
            return self.domain.to_vec();
        }
        let first = (low / step).ceil() * step;
        let mut ticks = (0..=200)
            .map(|i| first + i as f64 * step)
            .take_while(|v| *v <= high)
            .filter(|v| *v >= low)
            .map(|v| {
                if self.kind == ScaleKind::Log {
                    v.exp()
                } else if v == 0.0 {
                    0.0
                } else {
                    v
                }
            })
            .filter(|v| {
                v.is_finite()
                    && *v >= self.domain[0].min(self.domain[1])
                    && *v <= self.domain[0].max(self.domain[1])
            })
            .collect::<Vec<_>>();
        ticks.dedup();
        if ticks.len() < 2 {
            ticks = vec![self.domain[0], self.domain[1]];
        } else if self.domain[0] > self.domain[1] {
            ticks.reverse();
        }
        ticks
    }
}

/// Equal-width category bands. Order and identity are caller-owned.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoryScale {
    categories: Vec<SharedString>,
}

impl CategoryScale {
    pub fn new(
        categories: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Result<Self, ScaleError> {
        let categories: Vec<_> = categories.into_iter().map(Into::into).collect();
        if categories.is_empty() {
            return Err(ScaleError::Empty);
        }
        let mut seen = std::collections::HashSet::new();
        if categories.iter().any(|id| !seen.insert(id)) {
            return Err(ScaleError::DuplicateCategory);
        }
        Ok(Self { categories })
    }
    pub fn categories(&self) -> &[SharedString] {
        &self.categories
    }
    pub fn bandwidth(&self) -> f64 {
        1.0 / self.categories.len() as f64
    }
    pub fn map(&self, id: &str) -> Option<f64> {
        self.categories
            .iter()
            .position(|item| item.as_ref() == id)
            .map(|i| (i as f64 + 0.5) * self.bandwidth())
    }
    /// Bands are half-open, except the final band includes the right edge.
    pub fn invert(&self, fraction: f64) -> Option<&SharedString> {
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
            return None;
        }
        self.categories.get(
            ((fraction * self.categories.len() as f64).floor() as usize)
                .min(self.categories.len() - 1),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asymmetric_extremes_retain_endpoints_and_identity_viewports() -> Result<(), ScaleError> {
        for domain in [[1e16, 1.0], [1e300, -2.0], [f64::MIN_POSITIVE, 1e200]] {
            let scale = NumericScale::new(ScaleKind::Linear, domain)?;
            assert_eq!(scale.invert(0.), Some(domain[0]));
            assert_eq!(scale.invert(1.), Some(domain[1]));
            assert_eq!(scale.pan(0.)?.domain(), domain);
            assert_eq!(scale.zoom(0.25, 1.)?.domain(), domain);
        }
        let tiny = f64::from_bits(1);
        assert_eq!(
            NumericScale::new(ScaleKind::Linear, [tiny, tiny])?.domain(),
            [0., tiny * 2.]
        );
        for kind in [ScaleKind::Linear, ScaleKind::Log] {
            let scale = NumericScale::new(kind, [f64::MAX, f64::MAX])?;
            assert_eq!(scale.domain()[1], f64::MAX);
            assert!(
                scale
                    .ticks(7)
                    .iter()
                    .all(|v| v.is_finite() && *v >= scale.domain()[0] && *v <= f64::MAX)
            );
        }
        assert_eq!(
            NumericScale::extent(ScaleKind::Linear, [Some(1.), Some(f64::INFINITY)], false),
            Err(ScaleError::NonFinite)
        );
        Ok(())
    }
    #[test]
    fn domains_are_explicit_and_inverse_survives_reversal() -> Result<(), ScaleError> {
        let scale = NumericScale::new(ScaleKind::Linear, [73.0, -11.0])?;
        assert_eq!(scale.map(52.0), Some(0.25));
        assert_eq!(scale.invert(0.75), Some(10.0));
        assert_eq!(scale.map(94.0), Some(-0.25));
        assert_eq!(
            NumericScale::extent(ScaleKind::Linear, [None], true),
            Err(ScaleError::Empty)
        );
        assert_eq!(
            NumericScale::new(ScaleKind::Log, [-1., 2.]),
            Err(ScaleError::InvalidLogDomain)
        );
        assert_eq!(
            NumericScale::new(ScaleKind::Linear, [f64::NAN, 2.]),
            Err(ScaleError::NonFinite)
        );
        assert_eq!(
            NumericScale::new(ScaleKind::Linear, [20., 20.])?.domain(),
            [19., 21.]
        );
        Ok(())
    }
    #[test]
    fn logarithmic_zoom_retains_pointer_value_and_time_retains_milliseconds()
    -> Result<(), ScaleError> {
        let log = NumericScale::new(ScaleKind::Log, [2., 2000.])?;
        let zoom = log.zoom(0.27, 3.0)?;
        assert!(
            (zoom.invert(0.27).expect("zoom anchor in domain")
                - log.invert(0.27).expect("original anchor in domain"))
            .abs()
                < 1e-10
        );
        let time = NumericScale::new(ScaleKind::Time, [1_700_000_000_001., 1_700_000_000_101.])?;
        assert_eq!(time.map(1_700_000_000_026.), Some(0.25));
        assert_eq!(time.invert(0.83), Some(1_700_000_000_084.));
        assert_eq!(
            NumericScale::new(ScaleKind::Time, [1_700_000_000_000., 1_700_000_600_000.])?.ticks(2),
            vec![1_700_000_000_000., 1_700_000_600_000.]
        );
        assert_eq!(
            NumericScale::new(ScaleKind::Linear, [-3., 18.])?.ticks(6),
            vec![0., 5., 10., 15.]
        );
        Ok(())
    }
    #[test]
    fn category_boundaries_keep_business_identity() -> Result<(), ScaleError> {
        let scale = CategoryScale::new(["west", "east", "north"])?;
        assert_eq!(scale.map("east"), Some(0.5));
        assert_eq!(scale.invert(0.333).map(|id| id.as_ref()), Some("west"));
        assert_eq!(scale.invert(1.0 / 3.0).map(|id| id.as_ref()), Some("east"));
        assert_eq!(scale.invert(1.).map(|id| id.as_ref()), Some("north"));
        assert_eq!(scale.invert(-0.01), None);
        Ok(())
    }
}
