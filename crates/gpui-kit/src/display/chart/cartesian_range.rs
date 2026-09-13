//! Dedicated horizontal raw-coordinate navigator. Plot pan/brush policy stays
//! separate. The shared range state machine owns all arithmetic and proposals.
use super::*;
use crate::interaction::range::{
    RangeIntent, RangeInteraction, RangeKey, RangeMapping, RangeTarget,
};
use gpui::AnyElement;

/// Caller-owned persistent selection over a numeric overview domain. The range
/// uses ascending raw Start/End identity even when the overview domain descends.
/// No selection is installed by the chart. Accept `CartesianEvent::Range` in
/// caller state and optionally derive a linked viewport from its exact values.
#[derive(Clone, Debug)]
pub struct CartesianRange {
    pub domain: NumericScale,
    pub value: Option<[f64; 2]>,
    pub label: SharedString,
    pub enabled: bool,
    /// Decorative overview of the same caller series and value axes.
    pub overview: bool,
}
impl CartesianRange {
    pub fn new(
        domain: NumericScale,
        value: Option<[f64; 2]>,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            domain,
            value,
            label: label.into(),
            enabled: true,
            overview: true,
        }
    }
}

#[derive(Clone, PartialEq)]
struct Mapping(NumericScale);
impl RangeMapping for Mapping {
    fn project(&self, value: f64) -> Option<f64> {
        self.0.map(value)
    }
    fn unproject(&self, fraction: f64) -> Option<f64> {
        self.0.invert(fraction)
    }
}
#[derive(Default)]
struct Editing {
    interaction: RangeInteraction<Mapping>,
    bounds: Bounds<Pixels>,
}

type OverviewPaths = Rc<Vec<(usize, Vec<[f64; 2]>)>>;
#[derive(Default)]
struct Overview {
    revision: Option<Rc<Vec<ProjectedSeries>>>,
    columns: usize,
    sampling: PathSampling,
    paths: OverviewPaths,
}
impl Overview {
    fn paths(
        &mut self,
        revision: Rc<Vec<ProjectedSeries>>,
        columns: usize,
        sampling: PathSampling,
    ) -> OverviewPaths {
        if self
            .revision
            .as_ref()
            .is_none_or(|old| !Rc::ptr_eq(old, &revision))
            || self.columns != columns
            || self.sampling != sampling
        {
            let mut paths = Vec::new();
            for series in revision.iter() {
                for run in series
                    .points
                    .split(Option::is_none)
                    .filter(|run| !run.is_empty())
                {
                    let points = run
                        .iter()
                        .flatten()
                        .map(|p| [p.x, p.y, p.baseline])
                        .collect::<Vec<_>>();
                    let indices = if sampling == PathSampling::MinMax {
                        sample_path(&points, columns)
                    } else {
                        (0..points.len()).collect()
                    };
                    paths.push((
                        series.source,
                        indices
                            .into_iter()
                            .map(|i| [points[i][0], points[i][1]])
                            .collect(),
                    ));
                }
            }
            self.paths = Rc::new(paths);
            self.revision = Some(revision);
            self.columns = columns;
            self.sampling = sampling;
        }
        self.paths.clone()
    }
}

fn fraction(bounds: Bounds<Pixels>, p: Point<Pixels>) -> f64 {
    f64::from(f32::from(p.x - bounds.left())) / f64::from(f32::from(bounds.size.width))
}

pub(super) fn build(
    chart: &CartesianChart,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let id = chart.ident.child("range");
    let state = crate::motion::keyed::slot::<Editing>(
        &id.semantic_id(),
        window.window_handle().window_id(),
        cx,
    );
    let enabled =
        chart.range.as_ref().is_some_and(|range| range.enabled) && chart.on_event.is_some();
    let cancellation = {
        let mut state = state.borrow_mut();
        if enabled {
            let range = chart.range.as_ref().expect("enabled range");
            state.interaction.sync(&Mapping(range.domain), range.value)
        } else {
            state.interaction.cancel()
        }
    };
    if let Some(event) = cancellation {
        window.release_pointer();
        if let Some(report) = &chart.on_event {
            report(CartesianEvent::Range(event), window, cx);
        }
    }
    let range = chart.range.as_ref()?;
    let theme = cx.theme().clone();
    let mapping = Mapping(range.domain);
    let preview = state.borrow().interaction.preview();
    let displayed = preview.or(range.value);
    let positions =
        displayed.and_then(|value| Some([mapping.project(value[0])?, mapping.project(value[1])?]));
    let measured = measure::cell(&id.child("bounds").semantic_id(), window, cx);
    let measurement = measured.clone();
    let mut strip = div()
        .id(id.child("strip").element_id())
        .relative()
        .w_full()
        .h(px(64.))
        .overflow_hidden()
        .bg(theme.colors.accent.opacity(theme.effects.area_wash_alpha))
        .child(
            canvas(
                move |bounds, window, _| measure::record(&measurement, bounds, window),
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        );
    if range.overview {
        let cache = crate::motion::keyed::slot::<ProjectionCache>(
            &id.child("projection").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        if let Ok(projected) = cache.borrow_mut().get(
            &chart.series,
            &ChartScale::Numeric(range.domain),
            &chart.axes,
            &chart.hidden,
        ) {
            let overview = crate::motion::keyed::slot::<Overview>(
                &id.child("paths").semantic_id(),
                window.window_handle().window_id(),
                cx,
            );
            let paths = overview.borrow_mut().paths(
                projected,
                f32::from(measured.get().size.width).max(1.) as usize,
                chart.sampling,
            );
            let raw = chart.series.clone();
            let colors = theme.colors.sequence.clone();
            strip = strip.child(
                canvas(
                    move |_, _, _| (),
                    move |bounds, _, window, _| {
                        for (source, points) in paths.iter() {
                            let color = raw[*source].color.unwrap_or(colors.get(*source));
                            let mut path = PathBuilder::stroke(px(1.));
                            for (i, p) in points.iter().enumerate() {
                                let point = gpui::point(
                                    bounds.left() + bounds.size.width * p[0] as f32,
                                    bounds.bottom() - bounds.size.height * p[1] as f32,
                                );
                                if i == 0 {
                                    path.move_to(point);
                                } else {
                                    path.line_to(point);
                                }
                            }
                            if let Ok(path) = path.build() {
                                window.paint_path(path, color);
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            );
        } else {
            strip = strip.child(
                div()
                    .child(
                        cx.strings()
                            .format(StringKey::ChartInvalidData, &[&range.label]),
                    )
                    .semantic_in(
                        cx,
                        NodeSpec::new(id.child("overview-error").semantic_id(), Role::Status)
                            .text(range.label.clone())
                            .value("error"),
                    ),
            );
        }
    }
    if let Some([a, b]) = positions.filter(|p| p.iter().all(|v| v.is_finite())) {
        let a = a.clamp(0., 1.);
        let b = b.clamp(0., 1.);
        strip = strip.child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(a.min(b) as f32))
                .w(relative((a - b).abs() as f32))
                .border_1()
                .border_color(theme.colors.accent)
                .bg(theme.colors.accent.opacity(theme.effects.area_wash_alpha))
                .semantic_in(
                    cx,
                    NodeSpec::new(id.child("selection").semantic_id(), Role::Status)
                        .parent(id.semantic_id())
                        .text(range.label.clone())
                        .value(
                            displayed
                                .map(|v| format!("{}..{}", v[0], v[1]))
                                .unwrap_or_default(),
                        ),
                ),
        );
        for (target, position, key, name) in [
            (
                RangeTarget::Start,
                a,
                "start",
                StringKey::RangeSelectionStart,
            ),
            (RangeTarget::End, b, "end", StringKey::RangeSelectionEnd),
        ] {
            let raw = displayed.expect("projected selection")
                [if target == RangeTarget::Start { 0 } else { 1 }];
            let mut handle = div()
                .id(id.child(key).element_id())
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(position as f32))
                .ml(px(-theme.space(Space::Sm) / 2.))
                .w(px(theme.space(Space::Sm)))
                .bg(theme.colors.accent)
                .semantic_in(
                    cx,
                    NodeSpec::new(id.child(key).semantic_id(), Role::Slider)
                        .parent(id.semantic_id())
                        .disabled(!enabled)
                        .text(cx.strings().text(name))
                        .value(raw.to_string()),
                );
            if enabled {
                let state = state.clone();
                let mapping = mapping.clone();
                let value = range.value;
                let report = chart.on_event.clone().expect("enabled handler");
                handle =
                    handle
                        .tab_index(0)
                        .focus_ring(&theme)
                        .on_key_down(move |event, window, cx| {
                            let key = match event.keystroke.key.as_str() {
                                "left" => Some(RangeKey::Step(-0.01)),
                                "right" => Some(RangeKey::Step(0.01)),
                                "home" => Some(RangeKey::Home),
                                "end" => Some(RangeKey::End),
                                _ => None,
                            };
                            if let (Some(key), Some(value)) = (key, value) {
                                let outcome = state
                                    .borrow_mut()
                                    .interaction
                                    .keyboard(&mapping, value, target, key);
                                if let Ok(outcome) = outcome {
                                    window.release_pointer();
                                    report(CartesianEvent::Range(outcome), window, cx);
                                    window.refresh();
                                    cx.stop_propagation();
                                }
                            }
                        });
            }
            strip = strip.child(handle);
        }
    }
    if enabled {
        let report = chart.on_event.clone().expect("enabled handler");
        let cancel = state.clone();
        let cancel_report = report.clone();
        strip = strip.child(crate::interaction::on_pointer_cancel(move |window, cx| {
            let event = cancel.borrow_mut().interaction.cancel();
            if let Some(event) = event {
                cancel_report(CartesianEvent::Range(event), window, cx);
                window.refresh();
            }
        }));
        let down = state.clone();
        let down_report = report.clone();
        let down_bounds = measured.clone();
        let value = range.value;
        let down_mapping = mapping.clone();
        strip = strip.on_mouse_down_with_pointer_capture(
            MouseButton::Left,
            move |event, window, cx| {
                let bounds = down_bounds.get();
                if bounds.size.width <= px(0.) {
                    return;
                }
                let pointer = fraction(bounds, event.position);
                let intent = if event.modifiers.shift {
                    RangeIntent::Create
                } else if let Some([a, b]) = value
                    .and_then(|v| Some([down_mapping.project(v[0])?, down_mapping.project(v[1])?]))
                {
                    let tolerance = 6. / f64::from(f32::from(bounds.size.width));
                    if (pointer - a).abs() <= tolerance {
                        RangeIntent::Resize(RangeTarget::Start)
                    } else if (pointer - b).abs() <= tolerance {
                        RangeIntent::Resize(RangeTarget::End)
                    } else if pointer >= a.min(b) && pointer <= a.max(b) {
                        RangeIntent::Move
                    } else {
                        RangeIntent::Create
                    }
                } else {
                    RangeIntent::Create
                };
                let result = {
                    let mut state = down.borrow_mut();
                    state.bounds = bounds;
                    state
                        .interaction
                        .begin(down_mapping.clone(), value, intent, pointer)
                };
                match result {
                    Ok(event) => down_report(CartesianEvent::Range(event), window, cx),
                    Err(_) => window.release_pointer(),
                }
                window.refresh();
                cx.stop_propagation();
            },
        );
        let moving = state.clone();
        let move_report = report.clone();
        strip = strip.on_mouse_move(move |event, window, cx| {
            let outcome = {
                let mut state = moving.borrow_mut();
                let bounds = state.bounds;
                if state.interaction.preview().is_none() {
                    return;
                }
                state.interaction.update(fraction(bounds, event.position))
            };
            if let Ok(Some(event)) = outcome {
                move_report(CartesianEvent::Range(event), window, cx);
                window.refresh();
            }
        });
        let up = state.clone();
        let up_report = report.clone();
        strip = strip.on_mouse_up(MouseButton::Left, move |event, window, cx| {
            let outcome = {
                let mut state = up.borrow_mut();
                let bounds = state.bounds;
                if state.interaction.preview().is_none() {
                    return;
                }
                state.interaction.release(fraction(bounds, event.position))
            };
            if let Ok(Some(event)) = outcome {
                up_report(CartesianEvent::Range(event), window, cx);
                window.refresh();
                cx.stop_propagation();
            }
        });
        let keyboard = state.clone();
        let value = range.value;
        strip = strip
            .tab_index(0)
            .focus_ring(&theme)
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" {
                    let outcome = keyboard.borrow_mut().interaction.cancel();
                    window.release_pointer();
                    if let Some(event) = outcome {
                        report(CartesianEvent::Range(event), window, cx);
                    }
                    window.refresh();
                    cx.stop_propagation();
                } else {
                    let key = match event.keystroke.key.as_str() {
                        "left" => Some(RangeKey::Step(-0.01)),
                        "right" => Some(RangeKey::Step(0.01)),
                        "home" => Some(RangeKey::Home),
                        "end" => Some(RangeKey::End),
                        _ => None,
                    };
                    if let (Some(key), Some(value)) = (key, value) {
                        let outcome = keyboard.borrow_mut().interaction.keyboard(
                            &mapping,
                            value,
                            RangeTarget::Window,
                            key,
                        );
                        if let Ok(event) = outcome {
                            window.release_pointer();
                            report(CartesianEvent::Range(event), window, cx);
                            window.refresh();
                            cx.stop_propagation();
                        }
                    }
                }
            });
    }
    let readout = displayed
        .map(|v| {
            cx.strings().format(
                if preview.is_some() {
                    StringKey::RangeSelectionPreview
                } else {
                    StringKey::RangeSelectionRange
                },
                &[&(chart.format)("x", v[0]), &(chart.format)("x", v[1])],
            )
        })
        .unwrap_or_else(|| cx.strings().text(StringKey::RangeSelectionEmpty));
    Some(
        div()
            .column()
            .gap_token(&theme, Space::Xs)
            .child(range.label.clone())
            .child(
                strip.semantic_in(
                    cx,
                    NodeSpec::new(id.child("strip").semantic_id(), Role::Group)
                        .parent(id.semantic_id())
                        .text(range.label.clone()),
                ),
            )
            .child(
                div().child(readout.clone()).semantic_in(
                    cx,
                    NodeSpec::new(id.child("readout").semantic_id(), Role::Status)
                        .text(range.label.clone())
                        .value(readout),
                ),
            )
            .child(
                div()
                    .type_scale(&theme, TypeScale::Caption)
                    .child(cx.strings().text(StringKey::RangeSelectionInstructions)),
            )
            .semantic_in(
                cx,
                NodeSpec::new(id.semantic_id(), Role::Group).text(range.label.clone()),
            )
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::chart::scale::ScaleKind;
    use crate::interaction::range::RangeEvent;
    use gpui::{Modifiers, TestAppContext};
    use gpui_kit_testkit::harness::Harness;
    use std::cell::{Cell, RefCell};

    #[gpui::test]
    fn persistent_range_acceptance_refusal_cancel_and_unmount(cx: &mut TestAppContext) {
        let epoch = 1_700_000_000_000.;
        let domain =
            NumericScale::new(ScaleKind::Time, [epoch + 1000., epoch]).expect("reversed time");
        let initial = [epoch + 230., epoch + 610.];
        let value = Rc::new(Cell::new(Some(initial)));
        let mode = Rc::new(Cell::new(0));
        let accept = Rc::new(Cell::new(true));
        let events = Rc::new(RefCell::new(Vec::new()));
        let rendering = (value.clone(), mode.clone(), accept.clone(), events.clone());
        let mut harness = Harness::new(
            cx,
            |cx| {
                crate::install(cx);
                cx.set_global(crate::strings::TranslationPack::SimplifiedChinese.strings());
            },
            move |_, _| {
                let (value, mode, accept, events) = &rendering;
                if mode.get() == 2 {
                    return div().into_any_element();
                }
                let mut range = CartesianRange::new(domain, value.get(), "Window");
                range.enabled = mode.get() == 0;
                let value = value.clone();
                let accept = accept.clone();
                let events = events.clone();
                div()
                    .w(px(400.))
                    .child(
                        CartesianChart::new(
                            "range-test",
                            "Range fixture",
                            ChartScale::Numeric(domain),
                            [ValueAxis {
                                id: "units".into(),
                                label: "Units".into(),
                                scale: NumericScale::new(ScaleKind::Linear, [0., 100.])
                                    .expect("value domain"),
                            }],
                        )
                        .series([
                            RawSeries::new("observed", "units", SeriesMark::Line).points([
                                RawPoint::new("west", ChartValue::Number(epoch + 230.), Some(19.)),
                                RawPoint::new("east", ChartValue::Number(epoch + 610.), Some(83.)),
                            ]),
                        ])
                        .range(range)
                        .format_ticks(move |axis, value| {
                            if axis == "x" {
                                format!("{:.2} 单位", value - epoch).into()
                            } else {
                                value.to_string().into()
                            }
                        })
                        .on_event(move |event, window, _| {
                            if let CartesianEvent::Range(event) = event {
                                if let RangeEvent::Update { value: next, .. }
                                | RangeEvent::Commit { value: next, .. } = event
                                    && accept.get()
                                {
                                    value.set(Some(next));
                                    window.refresh();
                                }
                                events.borrow_mut().push(event);
                            }
                        }),
                    )
                    .into_any_element()
            },
        );
        assert_eq!(
            harness
                .node("range-test.range.readout")
                .expect("selected wording")
                .value
                .as_deref(),
            Some("所选范围：230.00 单位 至 610.00 单位")
        );
        let strip = harness.bounds("range-test.range.strip").expect("strip");
        let start = harness
            .bounds("range-test.range.start")
            .expect("start handle")
            .center();
        harness
            .context()
            .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        harness.context().simulate_mouse_move(
            start + point(px(-40.), px(0.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        harness.frame();
        let accepted = value.get().expect("accepted range");
        assert!((accepted[0] - (epoch + 330.)).abs() < 0.001);
        assert_eq!(accepted[1], initial[1], "untouched epoch endpoint");
        accept.set(false);
        harness.context().simulate_mouse_move(
            start + point(px(-80.), px(0.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        harness.frame();
        assert_eq!(value.get(), Some(accepted));
        let preview = harness
            .node("range-test.range.start")
            .expect("preview start")
            .value
            .expect("raw preview");
        assert_eq!(preview, (epoch + 430.).to_string());
        assert_eq!(
            harness
                .node("range-test.range.readout")
                .expect("preview wording")
                .value
                .as_deref(),
            Some("选区预览：430.00 单位 至 610.00 单位")
        );
        let outside = point(strip.left() - px(93.), strip.bottom() + px(77.));
        harness
            .context()
            .simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
        harness.frame();
        assert_eq!(value.get(), Some(accepted));
        assert_eq!(
            harness
                .node("range-test.range.readout")
                .expect("refused wording")
                .value
                .as_deref(),
            Some("所选范围：330.00 单位 至 610.00 单位")
        );
        assert!(
            matches!(events.borrow().last(),Some(RangeEvent::Commit{value,..}) if value[0]==initial[1] && value[1]==initial[1])
        );
        assert_eq!(
            harness
                .node("range-test.range.start")
                .expect("caller restored")
                .value
                .as_deref(),
            Some(accepted[0].to_string().as_str())
        );
        for next in [1, 2] {
            mode.set(0);
            harness.frame();
            harness.drag_start("range-test.range.selection");
            assert!(harness.update(|window, _| window.captured_hitbox().is_some()));
            mode.set(next);
            harness.frame();
            assert!(harness.update(|window, _| window.captured_hitbox().is_none()));
            assert!(matches!(
                events.borrow().last(),
                Some(RangeEvent::Cancel { .. })
            ));
        }
        mode.set(0);
        accept.set(true);
        harness.frame();
        harness.drag_start("range-test.range.selection");
        harness.drag_to(strip.center() + point(px(20.), px(0.)));
        harness.frame();
        let latest = value.get();
        harness.update(|window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::MouseCancelled(gpui::MouseCancelEvent),
                cx,
            );
        });
        harness.frame();
        assert_eq!(
            value.get(),
            latest,
            "cancel never rolls back accepted input"
        );
        assert!(matches!(
            events.borrow().last(),
            Some(RangeEvent::Cancel { .. })
        ));
        value.set(None);
        harness.frame();
        assert_eq!(
            harness
                .node("range-test.range.readout")
                .expect("empty wording")
                .value
                .as_deref(),
            Some("未选择范围")
        );
        harness.update(|window, _| window.remove_window());
    }

    #[gpui::test]
    fn keyboard_endpoints_use_numeric_and_log_mapping_without_swapping_handles(
        cx: &mut TestAppContext,
    ) {
        for (kind, domain, value, expected) in [
            (ScaleKind::Linear, [0., 100.], [23., 61.], 24.),
            (ScaleKind::Log, [1., 1000.], [10., 100.], 10f64.powf(1.03)),
        ] {
            let scale = NumericScale::new(kind, domain).expect("fixture mapping");
            let events = Rc::new(RefCell::new(Vec::new()));
            let reports = events.clone();
            let mut harness =
                Harness::new(cx, crate::install, move |_, _| {
                    let reports = reports.clone();
                    div()
                        .w(px(400.))
                        .child(
                            CartesianChart::new(
                                "keyboard-range",
                                "Raw range",
                                ChartScale::Numeric(scale),
                                [ValueAxis {
                                    id: "units".into(),
                                    label: "Units".into(),
                                    scale: NumericScale::new(ScaleKind::Linear, [0., 100.])
                                        .expect("values"),
                                }],
                            )
                            .series([RawSeries::new("source", "units", SeriesMark::Scatter)
                                .points([RawPoint::new(
                                    "source-west",
                                    ChartValue::Number(value[0]),
                                    Some(17.),
                                )])])
                            .range(CartesianRange::new(scale, Some(value), "Selected interval"))
                            .on_event(move |event, _, _| {
                                if let CartesianEvent::Range(event) = event {
                                    reports.borrow_mut().push(event);
                                }
                            }),
                        )
                        .into_any_element()
                });
            harness.click("keyboard-range.range.start");
            events.borrow_mut().clear();
            harness.keystrokes("right");
            let event = *events.borrow().last().expect("keyboard range proposal");
            match event {
                RangeEvent::Commit {
                    intent: RangeIntent::Resize(RangeTarget::Start),
                    value: actual,
                } => {
                    assert!((actual[0] - expected).abs() < 1e-10);
                    assert_eq!(actual[1], value[1]);
                }
                other => panic!("logical Start must stay Start: {other:?}"),
            }
            harness.update(|window, _| window.remove_window());
        }
    }
}
