//! Sparse keyed style/presence work. Retired layers are paint-only: current
//! projection and caller data remain the only input/semantic authorities.
use super::*;
use crate::motion::{Presence, Transition};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
struct Emphasis {
    transitions: HashMap<SharedString, Transition<f32>>,
}

pub(super) fn emphasis(
    chart: &CartesianChart,
    window: &mut Window,
    cx: &mut App,
) -> HashMap<SharedString, f32> {
    let state = crate::motion::keyed::slot::<Emphasis>(
        &chart.ident.child("emphasis").semantic_id(),
        window.window_handle().window_id(),
        cx,
    );
    let mut state = state.borrow_mut();
    let target = chart
        .emphasized
        .as_ref()
        .filter(|id| chart.series.iter().any(|s| &s.id == *id) && !chart.hidden.contains(id));
    if target.is_none() && state.transitions.is_empty() {
        return HashMap::new();
    }
    let theme = cx.theme();
    let spec = chart
        .motion
        .unwrap_or_else(|| CartesianMotion::themed(theme))
        .update;
    let muted = theme.opacity.muted;
    let mut result = HashMap::new();
    for series in chart.series.iter() {
        let opacity = if target.is_some_and(|id| id != &series.id) {
            muted
        } else {
            1.
        };
        let transition = state
            .transitions
            .entry(series.id.clone())
            .or_insert_with(|| Transition::new(1., spec));
        *transition = transition.spec(spec);
        if chart.animate {
            transition.set(opacity);
        } else {
            transition.snap(opacity);
        }
        result.insert(series.id.clone(), transition.animate(window, cx));
    }
    state.transitions.retain(|id, transition| {
        chart.series.iter().any(|s| &s.id == id)
            && (transition.is_animating() || transition.value() != 1.)
    });
    result
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Key {
    Series(SharedString),
    Point(SharedString, SharedString),
}

struct Life {
    presence: Presence,
    color: Transition<Hsla>,
    live: bool,
}

#[derive(Clone)]
pub(super) struct Retired {
    pub projected: ProjectedSeries,
    pub band: f64,
    pub raw: Rc<Vec<RawSeries>>,
    pub custom: Rc<HashMap<(usize, usize), CustomMark>>,
}

#[derive(Default)]
pub(super) struct PaintStyles {
    colors: HashMap<Key, Hsla>,
    series_opacity: HashMap<SharedString, f32>,
    pub retired: Vec<Rc<Retired>>,
}
impl PaintStyles {
    pub fn series(&self, raw: &RawSeries, fallback: Hsla) -> Hsla {
        self.colors
            .get(&Key::Series(raw.id.clone()))
            .copied()
            .unwrap_or(fallback)
    }
    pub fn point(&self, raw: &RawSeries, point: &RawPoint, fallback: Hsla) -> Hsla {
        let color = self
            .colors
            .get(&Key::Point(raw.id.clone(), point.id.clone()))
            .copied()
            .or(point.color);
        color
            .map(|color| color.opacity(self.series_opacity.get(&raw.id).copied().unwrap_or(1.)))
            .unwrap_or(fallback)
    }
}

#[derive(Default)]
pub(super) struct Lifecycle {
    raw: Option<Rc<Vec<RawSeries>>>,
    projected: Option<Rc<Vec<ProjectedSeries>>>,
    revision: Option<Rc<Vec<ProjectedSeries>>>,
    coordinates: Option<(ChartScale, Vec<ValueAxis>, ChartOrientation)>,
    spec: Option<CartesianMotion>,
    active: HashMap<Key, Life>,
    retired: Vec<Rc<Retired>>,
    styles: Rc<PaintStyles>,
    custom: Rc<HashMap<(usize, usize), CustomMark>>,
    band: f64,
}

impl Lifecycle {
    fn aim(
        &mut self,
        key: Key,
        from: Hsla,
        to: Hsla,
        new: bool,
        live: bool,
        spec: CartesianMotion,
    ) {
        let life = self.active.entry(key).or_insert_with(|| Life {
            presence: if new {
                Presence::hidden(spec.enter, spec.exit)
            } else {
                Presence::visible(spec.enter, spec.exit)
            },
            color: Transition::new(from, spec.update),
            live,
        });
        life.live = live;
        if live {
            life.presence.show();
        } else {
            life.presence.hide();
        }
        life.color = life.color.spec(spec.update);
        life.color.set(to);
    }

    pub fn animate(
        &mut self,
        raw: Rc<Vec<RawSeries>>,
        projections: (Rc<Vec<ProjectedSeries>>, Rc<Vec<ProjectedSeries>>),
        coordinates: (ChartScale, Vec<ValueAxis>, ChartOrientation),
        spec: CartesianMotion,
        window: &mut Window,
        cx: &mut App,
    ) -> Rc<PaintStyles> {
        let (revision, projected) = projections;
        let theme = cx.theme().clone();
        let direct = self
            .coordinates
            .as_ref()
            .is_some_and(|old| old != &coordinates)
            || self.spec.is_some_and(|old| old != spec);
        if direct {
            *self = Self::default();
        }
        let revision_changed = self
            .revision
            .as_ref()
            .is_none_or(|old| !Rc::ptr_eq(old, &revision));
        // Value-only revisions need geometry retargeting, not rebuilding every
        // identity/style hash table. Preserve active presence/color clocks too.
        let unchanged_styles = revision_changed
            && self.raw.as_ref().zip(self.revision.as_ref()).is_some_and(
                |(old, old_projection)| {
                    old.len() == raw.len()
                        && old.iter().zip(raw.iter()).all(|(a, b)| {
                            a.id == b.id
                                && a.color == b.color
                                && a.points.len() == b.points.len()
                                && a.points
                                    .iter()
                                    .zip(&b.points)
                                    .all(|(a, b)| a.id == b.id && a.color == b.color)
                        })
                        && old_projection.len() == revision.len()
                        && old_projection.iter().zip(revision.iter()).all(|(a, b)| {
                            a.source == b.source
                                && a.points.len() == b.points.len()
                                && a.points
                                    .iter()
                                    .zip(&b.points)
                                    .all(|(a, b)| a.is_some() == b.is_some())
                        })
                },
            );
        let changed = revision_changed && !unchanged_styles;
        if changed {
            let old_raw = self.raw.clone().unwrap_or_default();
            let old_projected = self.projected.clone().unwrap_or_default();
            let current = projected
                .iter()
                .map(|s| (raw[s.source].id.clone(), s))
                .collect::<HashMap<_, _>>();
            let previous = old_projected
                .iter()
                .map(|s| (old_raw[s.source].id.clone(), s))
                .collect::<HashMap<_, _>>();
            let mut live_keys = HashSet::new();
            for s in projected.iter() {
                let source = &raw[s.source];
                let color = source.color.unwrap_or(theme.colors.sequence.get(s.source));
                let prior = previous.get(&source.id);
                let series_key = Key::Series(source.id.clone());
                live_keys.insert(series_key.clone());
                let before = prior.map(|s| {
                    old_raw[s.source]
                        .color
                        .unwrap_or(theme.colors.sequence.get(s.source))
                });
                if before != Some(color) || self.active.contains_key(&series_key) {
                    self.aim(
                        series_key,
                        before.unwrap_or(color),
                        color,
                        !direct && prior.is_none(),
                        true,
                        spec,
                    );
                }
                let old_points = prior
                    .map(|old| {
                        old.points
                            .iter()
                            .flatten()
                            .map(|p| {
                                let point = &old_raw[old.source].points[p.source];
                                (point.id.clone(), point)
                            })
                            .collect::<HashMap<_, _>>()
                    })
                    .unwrap_or_default();
                for p in s.points.iter().flatten() {
                    let point = &source.points[p.source];
                    let key = Key::Point(source.id.clone(), point.id.clone());
                    live_keys.insert(key.clone());
                    let old = old_points.get(&point.id);
                    let before = old.map(|p| p.color.unwrap_or(color));
                    let target = point.color.unwrap_or(color);
                    if (old.is_none() && prior.is_some())
                        || old.is_some_and(|p| p.color != point.color)
                        || self.active.contains_key(&key)
                    {
                        self.aim(
                            key,
                            before.unwrap_or(target),
                            target,
                            !direct && old.is_none(),
                            true,
                            spec,
                        );
                    }
                }
            }
            // Reentry retires the old paint copy immediately; Presence itself
            // reverses from the current opacity instead of restarting at zero.
            self.retired.retain_mut(|layer| {
                let layer = Rc::make_mut(layer);
                let source = &layer.raw[layer.projected.source];
                if live_keys.contains(&Key::Series(source.id.clone())) {
                    for p in &mut layer.projected.points {
                        if p.as_ref().is_some_and(|p| {
                            live_keys.contains(&Key::Point(
                                source.id.clone(),
                                source.points[p.source].id.clone(),
                            ))
                        }) {
                            *p = None;
                        }
                    }
                }
                layer.projected.points.iter().any(Option::is_some)
            });
            for old in old_projected.iter() {
                let source = &old_raw[old.source];
                let color = source
                    .color
                    .unwrap_or(theme.colors.sequence.get(old.source));
                let mut retired = old.clone();
                if current.contains_key(&source.id) {
                    for p in &mut retired.points {
                        if let Some(point) = p {
                            let raw_point = &source.points[point.source];
                            let key = Key::Point(source.id.clone(), raw_point.id.clone());
                            if live_keys.contains(&key) {
                                *p = None;
                            } else {
                                let color = raw_point.color.unwrap_or(color);
                                self.aim(key, color, color, false, false, spec);
                            }
                        }
                    }
                } else {
                    self.aim(
                        Key::Series(source.id.clone()),
                        color,
                        color,
                        false,
                        false,
                        spec,
                    );
                }
                if retired.points.iter().any(Option::is_some) {
                    self.retired.push(Rc::new(Retired {
                        projected: retired,
                        band: self.band,
                        raw: old_raw.clone(),
                        custom: self.custom.clone(),
                    }));
                }
            }
        }
        if !changed && self.active.is_empty() {
            self.raw = Some(raw);
            self.revision = Some(revision);
            self.projected = Some(projected);
            return self.styles.clone();
        }
        let mut colors = HashMap::new();
        let mut series_opacity = HashMap::new();
        self.active.retain(|key, life| {
            let opacity = life.presence.animate(window, cx).clamp(0., 1.);
            if let Key::Series(id) = key {
                series_opacity.insert(id.clone(), opacity);
            }
            let color = life.color.animate(window, cx).opacity(opacity);
            if life.presence.is_animating() || life.color.is_animating() || !life.live {
                colors.insert(key.clone(), color);
            }
            life.presence.is_rendered()
                && (life.presence.is_animating() || life.color.is_animating() || !life.live)
        });
        self.retired.retain_mut(|layer| {
            let source = &layer.raw[layer.projected.source];
            if self
                .active
                .get(&Key::Series(source.id.clone()))
                .is_some_and(|life| !life.live)
            {
                return true;
            }
            let layer = Rc::make_mut(layer);
            let source = &layer.raw[layer.projected.source];
            for p in &mut layer.projected.points {
                if p.as_ref().is_some_and(|p| {
                    !self
                        .active
                        .get(&Key::Point(
                            source.id.clone(),
                            source.points[p.source].id.clone(),
                        ))
                        .is_some_and(|life| !life.live)
                }) {
                    *p = None;
                }
            }
            layer.projected.points.iter().any(Option::is_some)
        });
        self.styles = Rc::new(PaintStyles {
            colors,
            series_opacity,
            retired: self.retired.clone(),
        });
        self.raw = Some(raw);
        self.projected = Some(projected);
        self.revision = Some(revision);
        self.coordinates = Some(coordinates);
        self.spec = Some(spec);
        self.styles.clone()
    }

    pub fn custom(&mut self, custom: Rc<HashMap<(usize, usize), CustomMark>>, band: f64) {
        self.custom = custom;
        self.band = band;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{CubicBezier, MotionSpec};
    use gpui_kit_testkit::harness::Harness;
    use std::{cell::RefCell, time::Duration};

    #[gpui::test]
    fn actual_chart_reverses_presence_retires_authority_and_reuses_resting_state(
        cx: &mut gpui::TestAppContext,
    ) {
        let red = gpui::hsla(0., 1., 0.5, 1.);
        let blue = gpui::hsla(0.6, 1., 0.5, 1.);
        let original = Rc::new(vec![
            RawSeries::new("observed", "units", SeriesMark::Bar)
                .tint(red)
                .points([
                    RawPoint::new("west", ChartValue::Number(23.), Some(61.))
                        .text("West", "61 exact"),
                    RawPoint::new("east", ChartValue::Number(71.), Some(37.))
                        .tint(blue)
                        .text("East", "37 exact"),
                ]),
            RawSeries::new("forecast", "units", SeriesMark::Bar).points([
                RawPoint::new("west", ChartValue::Number(23.), Some(19.)),
                RawPoint::new("east", ChartValue::Number(71.), Some(83.)),
            ]),
        ]);
        let input = Rc::new(RefCell::new((
            original.clone(),
            Vec::<SharedString>::new(),
            [0., 100.],
        )));
        let rendering = input.clone();
        let linear = MotionSpec::new(1000, CubicBezier::new(0., 0., 1., 1.));
        let mut harness = Harness::new(cx, crate::install, move |_, _| {
            let (raw, hidden, domain) = &*rendering.borrow();
            let scale = NumericScale::new(super::super::super::scale::ScaleKind::Linear, *domain)
                .expect("fixture domain");
            div()
                .w(px(400.))
                .child(
                    CartesianChart::new(
                        "life",
                        "Lifecycle fixture",
                        ChartScale::Numeric(scale),
                        [ValueAxis {
                            id: "units".into(),
                            label: "Units".into(),
                            scale,
                        }],
                    )
                    .shared_series(raw.clone())
                    .hidden(hidden.clone())
                    .motion(CartesianMotion {
                        enter: linear,
                        update: linear,
                        exit: linear,
                    })
                    .selected(Some(ChartSelection::new("observed", "west")))
                    .on_event(|_, _, _| {}),
                )
                .into_any_element()
        });
        harness.frame();
        let life = harness.update(|window, cx| {
            crate::motion::keyed::slot::<Lifecycle>(
                &"life.lifecycle".into(),
                window.window_handle().window_id(),
                cx,
            )
        });
        harness.advance(Duration::from_millis(250));
        let opacity = life.borrow().styles.series(&original[0], red).a;
        assert!((opacity - 0.25).abs() < 0.01, "configured entrance clock");
        let explicit = life
            .borrow()
            .styles
            .point(&original[0], &original[0].points[1], red);
        assert!(
            (explicit.a - opacity).abs() < 0.01,
            "explicit point colors must enter with series"
        );
        harness.advance(Duration::from_millis(1000));
        let resting = life.borrow().styles.clone();
        harness.frame();
        assert!(Rc::ptr_eq(&resting, &life.borrow().styles));
        assert!(life.borrow().active.is_empty());
        let original_bounds = harness
            .bounds("life.series.observed.point.west")
            .expect("original bar");
        let mut reordered = (*original).clone();
        reordered.reverse();
        input.borrow_mut().0 = Rc::new(reordered);
        harness.frame();
        assert_eq!(
            harness
                .bounds("life.series.observed.point.west")
                .expect("reordered start"),
            original_bounds
        );
        harness.advance(Duration::from_millis(250));
        let middle_bounds = harness
            .bounds("life.series.observed.point.west")
            .expect("moving bar");
        harness.advance(Duration::from_millis(1000));
        let final_bounds = harness
            .bounds("life.series.observed.point.west")
            .expect("reordered end");
        assert!(
            middle_bounds.origin.x > original_bounds.origin.x
                && middle_bounds.origin.x < final_bounds.origin.x
        );
        assert_eq!(middle_bounds.size.height, original_bounds.size.height);
        assert_eq!(
            harness
                .node("life.series.observed.point.west")
                .expect("exact moving data")
                .value
                .as_deref(),
            Some("61 exact")
        );
        input.borrow_mut().0 = original.clone();
        harness.frame();
        harness.advance(Duration::from_millis(1000));
        input.borrow_mut().1 = vec!["observed".into()];
        harness.frame();
        assert!(harness.node("life.series.observed.point.west").is_none());
        assert!(harness.node("life.tooltip").is_none());
        assert_eq!(life.borrow().styles.retired.len(), 1);
        harness.advance(Duration::from_millis(250));
        let exiting = life.borrow().styles.series(&original[0], red).a;
        assert!((exiting - 0.75).abs() < 0.01);
        input.borrow_mut().1.clear();
        harness.frame();
        assert!(
            (life.borrow().styles.series(&original[0], red).a - exiting).abs() < 0.01,
            "reentry must not restart opacity"
        );
        assert!(life.borrow().styles.retired.is_empty());
        assert!(harness.node("life.series.observed.point.west").is_some());
        harness.advance(Duration::from_millis(1000));
        let mut changed = (*original).clone();
        changed[0].color = Some(blue);
        changed[0].points.remove(0);
        input.borrow_mut().0 = Rc::new(changed);
        harness.frame();
        assert!(harness.node("life.series.observed.point.west").is_none());
        harness.advance(Duration::from_millis(300));
        assert_eq!(
            life.borrow().styles.retired[0]
                .projected
                .points
                .iter()
                .flatten()
                .count(),
            1
        );
        let mid = life.borrow().styles.series(&original[0], blue);
        assert!(mid != red && mid != blue, "series color interpolation");
        // Reinsert a retiring business key while another style is moving.
        input.borrow_mut().0 = original.clone();
        harness.frame();
        assert!(life.borrow().styles.retired.is_empty());
        harness.update(|_, cx| cx.set_reduce_motion(true));
        harness.frame();
        assert!(life.borrow().active.is_empty());
        assert!(life.borrow().styles.retired.is_empty());
        assert_eq!(
            harness
                .node("life.series.observed.point.west")
                .expect("live west")
                .value
                .as_deref(),
            Some("61 exact")
        );
        harness.update(|_, cx| cx.set_reduce_motion(false));
        input.borrow_mut().1 = vec!["observed".into()];
        harness.frame();
        input.borrow_mut().2 = [0., 200.];
        harness.frame();
        assert!(
            life.borrow().styles.retired.is_empty(),
            "domain gesture must discard old-coordinate ghosts immediately"
        );
        harness.update(|window, _| window.remove_window());
    }

    #[gpui::test]
    fn legend_emphasis_is_controlled_and_never_retires_other_targets(
        cx: &mut gpui::TestAppContext,
    ) {
        let chosen = Rc::new(RefCell::new(None::<SharedString>));
        let events = Rc::new(RefCell::new(Vec::new()));
        let inputs = (chosen.clone(), events.clone());
        let mut harness = Harness::new(cx, crate::install, move |_, _| {
            let (chosen, events) = &inputs;
            let events = events.clone();
            let scale =
                NumericScale::new(super::super::super::scale::ScaleKind::Linear, [0., 100.])
                    .expect("fixture domain");
            div()
                .w(px(400.))
                .child(
                    CartesianChart::new(
                        "emphasis-test",
                        "Emphasis",
                        ChartScale::Numeric(scale),
                        [ValueAxis {
                            id: "units".into(),
                            label: "Units".into(),
                            scale,
                        }],
                    )
                    .series([
                        RawSeries::new("actual", "units", SeriesMark::Bar).points([RawPoint::new(
                            "west",
                            ChartValue::Number(23.),
                            Some(61.),
                        )]),
                        RawSeries::new("forecast", "units", SeriesMark::Bar)
                            .points([RawPoint::new("west", ChartValue::Number(23.), Some(19.))]),
                    ])
                    .emphasized(chosen.borrow().clone())
                    .on_event(move |event, _, _| events.borrow_mut().push(event)),
                )
                .into_any_element()
        });
        let bounds = harness
            .bounds("emphasis-test.series.forecast.point.west")
            .expect("forecast geometry");
        let legend = harness
            .bounds("emphasis-test.legend.actual")
            .expect("legend item");
        harness.context().simulate_event(gpui::MouseMoveEvent {
            position: legend.center(),
            ..Default::default()
        });
        harness.frame();
        assert!(
            events
                .borrow()
                .contains(&CartesianEvent::Emphasis(Some("actual".into())))
        );
        assert!(
            chosen.borrow().is_none(),
            "legend must not accept its own proposal"
        );
        let state = harness.update(|window, cx| {
            crate::motion::keyed::slot::<Emphasis>(
                &"emphasis-test.emphasis".into(),
                window.window_handle().window_id(),
                cx,
            )
        });
        assert!(state.borrow().transitions.is_empty());
        *chosen.borrow_mut() = Some("actual".into());
        harness.frame();
        harness.advance(Duration::from_millis(1000));
        assert!(
            state
                .borrow()
                .transitions
                .get("forecast")
                .expect("dimmed forecast")
                .value()
                < 1.
        );
        assert_eq!(
            harness
                .bounds("emphasis-test.series.forecast.point.west")
                .expect("dimmed target remains"),
            bounds
        );
        harness.click("emphasis-test.series.forecast.point.west");
        assert!(
            events
                .borrow()
                .contains(&CartesianEvent::Select(Some(ChartSelection::new(
                    "forecast", "west"
                ))))
        );
        *chosen.borrow_mut() = Some("removed-series".into());
        harness.frame();
        harness.advance(Duration::from_millis(1000));
        assert!(
            state.borrow().transitions.is_empty(),
            "unknown emphasis clears without retaining stale style"
        );
        harness.update(|window, _| window.remove_window());
    }
}
