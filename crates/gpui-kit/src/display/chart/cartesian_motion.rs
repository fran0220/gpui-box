//! Keyed visual geometry only. Caller data and semantic values never interpolate.
use super::*;
use crate::motion::{Interpolate, MotionSpec, Transition};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Geometry([f64; 5]);

impl Interpolate for Geometry {
    fn lerp(self, other: Self, t: f32) -> Self {
        Self(std::array::from_fn(|i| self.0[i].lerp(other.0[i], t)))
    }
    fn distance(self, other: Self) -> f32 {
        self.0
            .iter()
            .zip(other.0)
            .map(|(a, b)| a.distance(b))
            .fold(0., f32::max)
    }
}

struct Animated {
    transition: Transition<Geometry>,
    shape: (SeriesMark, bool),
    axis: SharedString,
    stack: Stack,
}

#[derive(Default)]
pub(super) struct GeometryMotion {
    pub spec: Option<MotionSpec>,
    coordinates: Option<(ChartScale, Vec<ValueAxis>)>,
    points: HashMap<(SharedString, SharedString), Animated>,
    target: Option<Rc<Vec<ProjectedSeries>>>,
    painted: Option<Rc<Vec<ProjectedSeries>>>,
    active: Vec<((SharedString, SharedString), usize, usize)>,
    groups: HashMap<SharedString, Transition<Geometry>>,
    active_groups: Vec<(SharedString, usize)>,
    live: HashSet<(SharedString, SharedString)>,
    live_groups: HashSet<SharedString>,
    needs_prune: bool,
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
            self.groups.clear();
            self.coordinates = Some(coordinates);
        }
        self.active.clear();
        self.active_groups.clear();
        let mut live = HashSet::new();
        let mut live_series = HashSet::new();
        for (series_index, s) in projected.iter_mut().enumerate() {
            let raw = &series[s.source];
            live_series.insert(raw.id.clone());
            for (point_index, p) in s
                .points
                .iter_mut()
                .enumerate()
                .filter_map(|(i, p)| p.as_mut().map(|p| (i, p)))
            {
                let key = (raw.id.clone(), raw.points[p.source].id.clone());
                live.insert(key.clone());
                let error = p.error.unwrap_or([p.y; 2]);
                let target = Geometry([p.x, p.y, p.baseline, error[0], error[1]]);
                let shape = (raw.mark, p.error.is_some());
                let animated = self.points.entry(key.clone()).or_insert_with(|| Animated {
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
                if animated.transition.is_animating() {
                    self.active.push((key, series_index, point_index));
                }
            }
            let target = Geometry([s.bar_offset, s.bar_width, 0., 0., 0.]);
            let group = self
                .groups
                .entry(raw.id.clone())
                .or_insert_with(|| Transition::new(target, spec));
            *group = group.spec(spec);
            group.set(target);
            let value = sample(group).0;
            s.bar_offset = value[0];
            s.bar_width = value[1];
            if group.is_animating() {
                self.active_groups.push((raw.id.clone(), series_index));
            }
        }
        for (key, point) in &mut self.points {
            if !live.contains(key) {
                point.transition = Transition::new(point.transition.value(), spec);
            }
        }
        for (key, group) in &mut self.groups {
            if !live_series.contains(key) {
                *group = Transition::new(group.value(), spec);
            }
        }
        self.live = live;
        self.live_groups = live_series;
        self.needs_prune = true;
    }

    /// Retain geometry while an exit can reverse, then reclaim it once no
    /// retired layer needs it. Settled redraws do not rescan the identity map.
    pub fn prune(&mut self, retiring: bool) {
        if retiring || !self.needs_prune {
            return;
        }
        self.points.retain(|key, _| self.live.contains(key));
        self.groups.retain(|key, _| self.live_groups.contains(key));
        self.needs_prune = false;
    }

    /// Retain immutable projection ownership while settled. Selection/hover
    /// redraws must not clone all projected points merely because motion is on.
    pub(super) fn animate(
        &mut self,
        projected: Rc<Vec<ProjectedSeries>>,
        series: &[RawSeries],
        coordinates: (ChartScale, Vec<ValueAxis>),
        window: &mut Window,
        cx: &mut App,
    ) -> Rc<Vec<ProjectedSeries>> {
        let unchanged = self
            .target
            .as_ref()
            .is_some_and(|target| Rc::ptr_eq(target, &projected));
        if unchanged && self.active.is_empty() && self.active_groups.is_empty() {
            return self
                .painted
                .as_ref()
                .expect("settled projection retained")
                .clone();
        }
        let spec = self.spec.unwrap_or_else(|| {
            crate::motion::MotionPolicy::spec(crate::motion::MotionRole::Resize, cx.theme())
        });
        let mut painted = (*projected).clone();
        if unchanged {
            self.active_groups.retain(|(id, s)| {
                let group = self.groups.get_mut(id).expect("active group retained");
                let value = group.animate(window, cx).0;
                painted[*s].bar_offset = value[0];
                painted[*s].bar_width = value[1];
                group.is_animating()
            });
            // Only transitions that were still moving at the previous frame
            // need a clock read. Do not rebuild the identity map per frame.
            self.active.retain(|(key, s, p)| {
                let animated = self.points.get_mut(key).expect("active point retained");
                let value = animated.transition.animate(window, cx).0;
                let point = painted[*s].points[*p]
                    .as_mut()
                    .expect("active geometry retained");
                point.x = value[0];
                point.y = value[1];
                point.baseline = value[2];
                if point.error.is_some() {
                    point.error = Some([value[3], value[4]]);
                }
                animated.transition.is_animating()
            });
        } else {
            self.update(&mut painted, series, coordinates, spec, |transition| {
                transition.animate(window, cx)
            });
        }
        let painted = if self.active.is_empty() && self.active_groups.is_empty() {
            projected.clone()
        } else {
            Rc::new(painted)
        };
        self.target = Some(projected);
        self.painted = Some(painted.clone());
        painted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::CubicBezier;
    use std::time::Duration;

    #[gpui::test]
    fn settled_shared_projection_is_not_cloned_and_only_changed_points_tick(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui_kit_testkit::harness::Harness;
        use std::cell::RefCell;
        let scale = NumericScale::new(super::super::super::scale::ScaleKind::Linear, [0., 100.])
            .expect("fixture domain");
        let x = ChartScale::Numeric(scale);
        let axes = vec![ValueAxis {
            id: "units".into(),
            label: "Units".into(),
            scale,
        }];
        let mut raw = vec![
            RawSeries::new("reading", "units", SeriesMark::Scatter).points([
                RawPoint::new("west", ChartValue::Number(17.), Some(23.)),
                RawPoint::new("east", ChartValue::Number(79.), Some(81.)),
            ]),
        ];
        let original = Rc::new(project(&raw, &x, &axes, &[]).expect("initial projection"));
        let motion = Rc::new(RefCell::new(GeometryMotion::default()));
        let input = Rc::new(RefCell::new((raw.clone(), original.clone())));
        let output = Rc::new(RefCell::new(original.clone()));
        let rendering = motion.clone();
        let data = input.clone();
        let result = output.clone();
        let coordinates = (x.clone(), axes.clone());
        let mut harness = Harness::new(cx, crate::install, move |window, cx| {
            let (raw, projected) = &*data.borrow();
            *result.borrow_mut() = rendering.borrow_mut().animate(
                projected.clone(),
                raw,
                coordinates.clone(),
                window,
                cx,
            );
            div().into_any_element()
        });
        for _ in 0..3 {
            harness.frame();
            assert!(Rc::ptr_eq(&output.borrow(), &original));
            assert!(motion.borrow().active.is_empty());
        }
        raw[0].points[0].y = Some(67.);
        let changed = Rc::new(project(&raw, &x, &axes, &[]).expect("changed projection"));
        *input.borrow_mut() = (raw, changed.clone());
        harness.frame();
        let first = output.borrow().clone();
        assert!(!Rc::ptr_eq(&first, &changed));
        assert_eq!(motion.borrow().active.len(), 1);
        assert_eq!(first[0].points[0].as_ref().expect("west").y, 0.23);
        assert_eq!(first[0].points[1].as_ref().expect("east").y, 0.81);
        harness.advance(Duration::from_millis(30));
        let middle = output.borrow().clone();
        let y = middle[0].points[0].as_ref().expect("west").y;
        assert!(y > 0.23 && y < 0.67);
        harness.update(|_, cx| cx.set_reduce_motion(true));
        harness.frame();
        let final_frame = output.borrow().clone();
        assert!(Rc::ptr_eq(&final_frame, &changed));
        assert!(motion.borrow().active.is_empty());
        assert_eq!(final_frame[0].points[0].as_ref().expect("west").y, 0.67);
    }

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
        motion.prune(false);
        assert!(motion.points.is_empty());
        let endpoints = Geometry([1e16; 5]);
        assert_eq!(endpoints.lerp(Geometry([1.; 5]), 1.), Geometry([1.; 5]));
    }
}
