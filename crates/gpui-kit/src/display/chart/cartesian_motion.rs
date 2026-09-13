//! Keyed visual geometry only. Caller data and semantic values never interpolate.
use super::*;
use crate::motion::{Interpolate, MotionSpec, Transition};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Geometry([f64; 5]);

impl Interpolate for Geometry {
    fn lerp(self, other: Self, t: f32) -> Self {
        if t == 0. {
            return self;
        }
        if t == 1. {
            return other;
        }
        let t = f64::from(t);
        Self(std::array::from_fn(|i| {
            self.0[i] * (1. - t) + other.0[i] * t
        }))
    }
    fn distance(self, other: Self) -> f32 {
        self.0
            .iter()
            .zip(other.0)
            .map(|(a, b)| (a - b).abs())
            .fold(0., f64::max)
            .min(f64::from(f32::MAX)) as f32
    }
}

struct Animated {
    transition: Transition<Geometry>,
    shape: (SeriesMark, bool, f64, f64),
    axis: SharedString,
    stack: Stack,
}

#[derive(Default)]
pub(super) struct GeometryMotion {
    coordinates: Option<(ChartScale, Vec<ValueAxis>)>,
    points: HashMap<(SharedString, SharedString), Animated>,
}

impl GeometryMotion {
    /// New/missing/removed points do not manufacture zero observations. Changes
    /// to axes or mark topology snap; same-identity data updates retarget smoothly.
    fn update(
        &mut self,
        projected: &mut [ProjectedSeries],
        series: &[RawSeries],
        coordinates: (ChartScale, Vec<ValueAxis>),
        spec: MotionSpec,
        mut sample: impl FnMut(&mut Transition<Geometry>) -> Geometry,
    ) {
        if self.coordinates.as_ref() != Some(&coordinates) {
            self.points.clear();
            self.coordinates = Some(coordinates);
        }
        let mut live = HashSet::new();
        for s in projected {
            let raw = &series[s.source];
            for p in s.points.iter_mut().flatten() {
                let key = (raw.id.clone(), raw.points[p.source].id.clone());
                live.insert(key.clone());
                let error = p.error.unwrap_or([p.y; 2]);
                let target = Geometry([p.x, p.y, p.baseline, error[0], error[1]]);
                let shape = (raw.mark, p.error.is_some(), s.bar_offset, s.bar_width);
                let animated = self.points.entry(key).or_insert_with(|| Animated {
                    transition: Transition::new(target, spec),
                    shape,
                    axis: raw.axis.clone(),
                    stack: raw.stack.clone(),
                });
                animated.transition = animated.transition.spec(spec);
                if animated.shape == shape
                    && animated.axis == raw.axis
                    && animated.stack == raw.stack
                {
                    animated.transition.set(target);
                } else {
                    animated.transition.snap(target);
                    animated.shape = shape;
                    animated.axis = raw.axis.clone();
                    animated.stack = raw.stack.clone();
                }
                let value = sample(&mut animated.transition).0;
                p.x = value[0];
                p.y = value[1];
                p.baseline = value[2];
                if p.error.is_some() {
                    p.error = Some([value[3], value[4]]);
                }
            }
        }
        self.points.retain(|key, _| live.contains(key));
    }

    pub(super) fn animate(
        &mut self,
        projected: &mut [ProjectedSeries],
        series: &[RawSeries],
        coordinates: (ChartScale, Vec<ValueAxis>),
        window: &mut Window,
        cx: &mut App,
    ) {
        let spec = crate::motion::MotionPolicy::spec(crate::motion::MotionRole::Resize, cx.theme());
        self.update(projected, series, coordinates, spec, |transition| {
            transition.animate(window, cx)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::CubicBezier;
    use std::time::Duration;

    #[test]
    fn reorder_retarget_and_remove_keep_business_identity_and_exact_raw_data() {
        let scale = NumericScale::new(super::super::super::scale::ScaleKind::Linear, [0., 100.])
            .expect("fixture scale");
        let x = ChartScale::Numeric(scale);
        let axes = vec![ValueAxis {
            id: "y".into(),
            label: "units".into(),
            scale,
        }];
        let series = |west, east, reverse: bool| {
            let mut points = vec![
                RawPoint::new("west", ChartValue::Number(17.), Some(west))
                    .text("West", west.to_string()),
                RawPoint::new("east", ChartValue::Number(83.), Some(east))
                    .text("East", east.to_string()),
            ];
            if reverse {
                points.reverse();
            }
            vec![RawSeries::new("readings", "y", SeriesMark::Scatter).points(points)]
        };
        let spec = MotionSpec::new(1000, CubicBezier::new(0., 0., 1., 1.));
        let mut motion = GeometryMotion::default();
        let first = series(20., 80., false);
        let mut projected = project(&first, &x, &axes, &[]).expect("first projection");
        motion.update(
            &mut projected,
            &first,
            (x.clone(), axes.clone()),
            spec,
            |t| t.value(),
        );
        let second = series(60., 50., true);
        let mut projected = project(&second, &x, &axes, &[]).expect("reordered projection");
        motion.update(
            &mut projected,
            &second,
            (x.clone(), axes.clone()),
            spec,
            |t| {
                t.advance(Duration::from_millis(500));
                t.value()
            },
        );
        let east = projected[0].points[0].as_ref().expect("east geometry");
        let west = projected[0].points[1].as_ref().expect("west geometry");
        assert!((east.y - 0.65).abs() < 1e-6);
        assert!((west.y - 0.4).abs() < 1e-6);
        assert_eq!(second[0].points[west.source].formatted.as_ref(), "60");
        assert_eq!(second[0].points[east.source].y, Some(50.));
        let third = series(90., 70., false);
        let mut projected = project(&third, &x, &axes, &[]).expect("interrupted projection");
        motion.update(
            &mut projected,
            &third,
            (x.clone(), axes.clone()),
            spec,
            |t| t.value(),
        );
        assert!((projected[0].points[0].as_ref().expect("west").y - 0.4).abs() < 1e-6);
        motion.update(&mut [], &[], (x, axes), spec, |t| t.value());
        assert!(motion.points.is_empty());
        let endpoints = Geometry([1e16; 5]);
        assert_eq!(endpoints.lerp(Geometry([1.; 5]), 1.), Geometry([1.; 5]));
    }
}
