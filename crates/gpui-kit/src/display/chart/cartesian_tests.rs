use super::super::scale::ScaleKind;
use super::*;
use gpui::{Modifiers, TestAppContext};
use gpui_kit_testkit::harness::Harness;
use std::cell::RefCell;

#[test]
fn explicit_ticks_validate_raw_values_and_preserve_caller_labels() {
    let ticks = |values: &[f64]| {
        values
            .iter()
            .map(|v| ChartTick {
                value: ChartValue::Number(*v),
                label: format!("calendar {v}").into(),
            })
            .collect::<Vec<_>>()
    };
    let reversed = ChartScale::Numeric(
        NumericScale::new(ScaleKind::Linear, [19., -7.]).expect("reversed scale"),
    );
    let labels = explicit_ticks(&reversed, &ticks(&[-7., 13., 19.])).expect("in-domain ticks");
    assert_eq!(labels[0], (0., "calendar 19".into()));
    assert_eq!(labels[2], (1., "calendar -7".into()));
    assert_eq!(
        explicit_ticks(&reversed, &ticks(&[20.])),
        Err(TickError::OutsideDomain)
    );
    assert_eq!(
        explicit_ticks(&reversed, &ticks(&[f64::NAN])),
        Err(TickError::InvalidValue)
    );
    assert_eq!(
        explicit_ticks(&reversed, &ticks(&[-0., 0.])),
        Err(TickError::DuplicateValue)
    );
    let tiny =
        ChartScale::Numeric(NumericScale::new(ScaleKind::Linear, [0., 1e300]).expect("wide scale"));
    assert_eq!(
        explicit_ticks(&tiny, &ticks(&[-1e-300])),
        Err(TickError::OutsideDomain)
    );
    assert_eq!(
        explicit_ticks(&tiny, &ticks(&[1e-300, 2e-300]))
            .expect("distinct source ticks")
            .len(),
        2
    );
    let categories = ChartScale::Category(
        super::super::scale::CategoryScale::new(["west", "east"]).expect("categories"),
    );
    assert_eq!(
        explicit_ticks(
            &categories,
            &[ChartTick {
                value: ChartValue::Category("east".into()),
                label: "Est".into()
            }]
        )
        .expect("category override"),
        vec![(0.75, "Est".into())]
    );
    assert!(matches!(
        chart(NumericScale::new(ScaleKind::Linear, [0., 10.]).expect("scale"))
            .axis_ticks("absent", []),
        Err(TickError::UnknownAxis)
    ));
}

#[test]
fn independent_axes_overlay_but_same_axis_groups_share_body_and_wick_centers() {
    let scale = NumericScale::new(ScaleKind::Linear, [0., 100.]).expect("value domain");
    let axes = [
        ValueAxis {
            id: "price".into(),
            label: "Price".into(),
            scale,
        },
        ValueAxis {
            id: "volume".into(),
            label: "Volume".into(),
            scale,
        },
    ];
    let x = ChartScale::Numeric(NumericScale::new(ScaleKind::Linear, [0., 10.]).expect("x domain"));
    let candle = RawSeries::new("price", "price", SeriesMark::Range).points([RawPoint::new(
        "day",
        ChartValue::Number(3.),
        Some(63.),
    )
    .baseline(21.)
    .error([9., 78.])]);
    let volume = RawSeries::new("volume", "volume", SeriesMark::Bar).points([RawPoint::new(
        "day",
        ChartValue::Number(3.),
        Some(87.),
    )]);
    let out = project(&[candle.clone(), volume], &x, &axes, &[]).expect("independent axes");
    for s in &out {
        assert_eq!(s.bar_offset, -0.4);
        assert_eq!(s.bar_width, 0.8);
        let p = s.points[0].as_ref().expect("present mark");
        assert_eq!(mark_center(p, s, SeriesMark::Range, 0.5), 0.3);
    }
    let other = RawSeries::new("second", "price", SeriesMark::Range).points([RawPoint::new(
        "day",
        ChartValue::Number(3.),
        Some(52.),
    )
    .baseline(52.)
    .error([32., 71.])]);
    let out = project(&[candle, other], &x, &axes, &[]).expect("same-axis grouped ranges");
    for (s, expected) in out.iter().zip([0.2, 0.4]) {
        let p = s.points[0].as_ref().expect("present range");
        let center = mark_center(p, s, SeriesMark::Range, 0.5);
        assert!((center - expected).abs() < 1e-12);
        let rect = mark_rect(p, s, SeriesMark::Range, 0.5, 200., 100., None);
        assert!(((rect[0] + rect[2]) / 2. - expected).abs() < 1e-12);
        assert!(rect[1] < p.y && rect[3] > p.y);
    }
}

fn chart(scale: NumericScale) -> CartesianChart {
    CartesianChart::new(
        "test.chart",
        "Fixture",
        ChartScale::Numeric(scale),
        [ValueAxis {
            id: "y".into(),
            label: "Units".into(),
            scale: NumericScale::new(ScaleKind::Linear, [-10., 30.])
                .expect("finite fixture y domain"),
        }],
    )
    .series([RawSeries::new("sales", "y", SeriesMark::Bar).points([
        RawPoint::new("west", ChartValue::Number(3.), Some(-5.)).text("West", "−5 exact"),
        RawPoint::new("east", ChartValue::Number(8.), Some(23.)).text("East", "23 exact"),
    ])])
    .height(160.)
}

#[gpui::test]
fn horizontal_range_and_custom_mark_bounds_share_pointer_and_brush_coordinates(
    cx: &mut TestAppContext,
) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let collect = events.clone();
    let mut harness = Harness::new(cx, crate::install, move |_, _| {
        let collect = collect.clone();
        let mut chart = chart(NumericScale::new(ScaleKind::Linear, [0., 10.]).expect("x domain"))
            .orientation(ChartOrientation::Horizontal)
            .x_axis_side(AxisSide::Trailing)
            .axis_side("y", AxisSide::Leading)
            .expect("y axis")
            .custom_marks(|_, _| {
                Some(
                    CustomMark::new(24., 10., |bounds, color, window, _| {
                        window.paint_quad(gpui::fill(bounds, color))
                    })
                    .expect("positive custom mark"),
                )
            })
            .on_event(move |e, _, _| collect.borrow_mut().push(e));
        Rc::make_mut(&mut chart.series).push(
            RawSeries::new("audit", "y", SeriesMark::Scatter).points([RawPoint::new(
                "near-edge",
                ChartValue::Number(2.),
                Some(29.9),
            )
            .text("Audit", "29.9 exact")]),
        );
        div().w(px(380.)).child(chart).into_any_element()
    });
    let plot = harness.bounds("test.chart.plot").expect("horizontal plot");
    let west = harness
        .bounds("test.chart.series.sales.point.west")
        .expect("horizontal west");
    assert!((f32::from(west.size.width) - f32::from(plot.size.width) * 0.125).abs() <= 0.5);
    assert!((f32::from(west.size.height) - 64.).abs() <= 0.5);
    let custom = harness
        .bounds("test.chart.series.audit.point.near-edge")
        .expect("clipped custom");
    assert!(custom.size.width < px(24.) && custom.size.width > px(12.));
    assert!((f32::from(custom.size.height) - 10.).abs() <= 0.5);
    harness.click("test.chart.series.sales.point.west");
    assert!(
        events
            .borrow()
            .contains(&CartesianEvent::Select(Some(ChartSelection::new(
                "sales", "west"
            ))))
    );
    let shift = Modifiers {
        shift: true,
        ..Modifiers::none()
    };
    let a = point(plot.center().x, plot.top() + plot.size.height * 0.2);
    let b = point(plot.center().x, plot.top() + plot.size.height * 0.7);
    harness
        .context()
        .simulate_mouse_down(a, MouseButton::Left, shift);
    harness
        .context()
        .simulate_mouse_move(b, MouseButton::Left, shift);
    harness.frame();
    let preview = harness
        .bounds("test.chart.brush-preview")
        .expect("horizontal brush");
    assert_eq!(preview.size.width, plot.size.width);
    assert!((f32::from(preview.size.height) - 80.).abs() <= 0.5);
    harness
        .context()
        .simulate_mouse_up(b, MouseButton::Left, shift);
    harness.frame();
    assert!(events.borrow().iter().any(|e| matches!(e, CartesianEvent::Brush([ChartValue::Number(a),ChartValue::Number(b)]) if (*a-2.).abs()<1e-5 && (*b-7.).abs()<1e-5)));
    assert!(CustomMark::new(f32::NAN, 10., |_, _, _, _| {}).is_err());
    assert!(CustomMark::new(10., 0., |_, _, _, _| {}).is_err());
}

#[gpui::test]
fn animated_semantics_use_current_raw_text_and_reduced_motion_settles_bounds(
    cx: &mut TestAppContext,
) {
    let reading = Rc::new(RefCell::new(-5.));
    let input = reading.clone();
    let mut harness = Harness::new(cx, crate::install, move |_, _| {
        let mut chart = chart(NumericScale::new(ScaleKind::Linear, [0., 10.]).expect("x domain"));
        let series = Rc::make_mut(&mut chart.series);
        series[0].points[0].y = Some(*input.borrow());
        series[0].points[0].formatted = format!("{} exact", input.borrow()).into();
        if *input.borrow() > 0. {
            series[0].points.reverse();
        }
        div().w(px(320.)).child(chart).into_any_element()
    });
    *reading.borrow_mut() = 15.;
    harness.frame();
    let moving = harness
        .node("test.chart.series.sales.point.west")
        .expect("moving west");
    assert_eq!(moving.value.as_deref(), Some("15 exact"));
    assert!(
        (moving.bounds.height - 20.).abs() < 0.5,
        "starts at old geometry: {:?}",
        moving.bounds
    );
    harness.update(|window, cx| {
        cx.set_reduce_motion(true);
        window.refresh();
    });
    let settled = harness
        .node("test.chart.series.sales.point.west")
        .expect("settled west");
    assert_eq!(settled.value.as_deref(), Some("15 exact"));
    assert!(
        (settled.bounds.height - 60.).abs() < 0.5,
        "reduced-motion final body: {:?}",
        settled.bounds
    );
}

#[gpui::test]
fn bar_semantics_cover_clipped_body_and_click_reports_original_identity(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let collect = events.clone();
    let mut harness = Harness::new(cx, crate::install, move |_, _| {
        let collect = collect.clone();
        div()
            .w(px(270.))
            .child(
                chart(
                    NumericScale::new(ScaleKind::Linear, [0., 10.])
                        .expect("finite fixture x domain"),
                )
                .on_event(move |e, _, _| collect.borrow_mut().push(e)),
            )
            .into_any_element()
    });
    let plot = harness
        .bounds("test.chart.plot")
        .expect("measured chart plot");
    assert!(plot.size.width < px(320.));
    let west = harness
        .node("test.chart.series.sales.point.west")
        .expect("west semantic mark");
    assert_eq!(west.value.as_deref(), Some("−5 exact"));
    assert!((west.bounds.height - 20.).abs() < 0.1);
    // Semantic bounds snap to the half-pixel layout grid.
    assert!(
        (west.bounds.width - f32::from(plot.size.width) * 0.4).abs() <= 0.5,
        "bar={:?}, plot={plot:?}",
        west.bounds
    );
    harness.click("test.chart.series.sales.point.west");
    assert!(
        events
            .borrow()
            .contains(&CartesianEvent::Select(Some(ChartSelection::new(
                "sales", "west"
            ))))
    );
}

#[gpui::test]
fn capture_survives_controlled_redraw_refusal_outside_release_and_cancel(cx: &mut TestAppContext) {
    let original = NumericScale::new(ScaleKind::Linear, [10., 90.]).expect("finite pan domain");
    let scale = Rc::new(RefCell::new(original));
    let accepted = Rc::new(RefCell::new(true));
    let events = Rc::new(RefCell::new(Vec::new()));
    let input = scale.clone();
    let accept = accepted.clone();
    let collect = events.clone();
    let mut harness = Harness::new(cx, crate::install, move |_, _| {
        let state = input.clone();
        let collect = collect.clone();
        let accept = accept.clone();
        let current = *input.borrow();
        div()
            .w(px(400.))
            .child(chart(current).on_event(move |e, window, _| {
                if let CartesianEvent::Viewport(next) = &e
                    && *accept.borrow()
                {
                    *state.borrow_mut() = *next;
                    window.refresh();
                }
                collect.borrow_mut().push(e);
            }))
            .into_any_element()
    });
    let plot = harness
        .bounds("test.chart.plot")
        .expect("measured pan plot");
    let start = plot.center();
    harness.drag_start("test.chart.plot");
    harness.drag_to(start + point(plot.size.width * 0.2, px(0.)));
    harness.frame();
    let domain = scale.borrow().domain();
    assert!((domain[0] + 6.).abs() < 1e-4);
    assert!((domain[1] - 74.).abs() < 1e-4);
    *accepted.borrow_mut() = false;
    harness.drag_to(start + point(plot.size.width * 0.4, px(0.)));
    harness.frame();
    assert_eq!(scale.borrow().domain(), domain);
    let proposal = events
        .borrow()
        .iter()
        .rev()
        .find_map(|e| {
            if let CartesianEvent::Viewport(s) = e {
                Some(s.domain())
            } else {
                None
            }
        })
        .expect("pan emitted a viewport proposal");
    assert!((proposal[0] + 22.).abs() < 1e-4);
    harness.drag_to(point(plot.right() + px(50.), plot.center().y));
    harness.drop_here();
    assert!(harness.update(|window, _| window.captured_hitbox().is_none()));
    harness.drag_start("test.chart.plot");
    harness.update(|window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::MouseCancelled(gpui::MouseCancelEvent),
            cx,
        );
    });
    let before = events
        .borrow()
        .iter()
        .filter(|e| matches!(e, CartesianEvent::Viewport(_)))
        .count();
    harness.drag_to(start + point(px(31.), px(0.)));
    harness.drop_here();
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|e| matches!(e, CartesianEvent::Viewport(_)))
            .count(),
        before
    );
}

#[gpui::test]
fn brush_returns_raw_domain_and_nonready_states_install_no_plot(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let collect = events.clone();
    let mut harness = Harness::new(cx, crate::install, move |_, _| {
        let collect = collect.clone();
        div()
            .w(px(400.))
            .child(
                chart(
                    NumericScale::new(ScaleKind::Linear, [10., 90.]).expect("finite brush domain"),
                )
                .on_event(move |e, _, _| collect.borrow_mut().push(e)),
            )
            .into_any_element()
    });
    let plot = harness
        .bounds("test.chart.plot")
        .expect("measured brush plot");
    let a = point(plot.left() + plot.size.width * 0.2, plot.center().y);
    let b = point(plot.left() + plot.size.width * 0.7, plot.center().y);
    let shift = Modifiers {
        shift: true,
        ..Modifiers::none()
    };
    harness
        .context()
        .simulate_mouse_down(a, MouseButton::Left, shift);
    harness
        .context()
        .simulate_mouse_move(b, MouseButton::Left, shift);
    harness.frame();
    let preview = harness
        .bounds("test.chart.brush-preview")
        .expect("active brush preview");
    assert!((f32::from(preview.size.width) - f32::from(plot.size.width) * 0.5).abs() <= 0.5);
    harness
        .context()
        .simulate_mouse_up(b, MouseButton::Left, shift);
    harness.frame();
    let range = events
        .borrow()
        .iter()
        .find_map(|e| {
            if let CartesianEvent::Brush([ChartValue::Number(a), ChartValue::Number(b)]) = e {
                Some([*a, *b])
            } else {
                None
            }
        })
        .expect("brush emitted raw endpoints");
    assert!((range[0] - 26.).abs() < 1e-4);
    assert!((range[1] - 66.).abs() < 1e-4);
    for phase in [
        Phase::Loading,
        Phase::Empty,
        Phase::Unavailable,
        Phase::Error,
    ] {
        harness.remount(move |_, _| {
            div()
                .w(px(300.))
                .child(
                    chart(
                        NumericScale::new(ScaleKind::Linear, [0., 10.])
                            .expect("finite nonready fixture domain"),
                    )
                    .state(phase),
                )
                .into_any_element()
        });
        assert!(harness.node("test.chart.plot").is_none());
        assert!(harness.node("test.chart").is_some());
    }
}

#[gpui::test]
fn sampled_paths_keep_raw_selection_gaps_and_shared_input_revisions(cx: &mut TestAppContext) {
    let data = Rc::new(RefCell::new(Rc::new(vec![
        RawSeries::new("signal", "y", SeriesMark::Area).points((0..6000).map(|time| {
            RawPoint::new(
                format!("time-{time}"),
                ChartValue::Number(time as f64),
                if (1000..1100).contains(&time) {
                    None
                } else if time == 13 {
                    Some(73.125)
                } else {
                    Some(5.)
                },
            )
        })),
    ])));
    let events = Rc::new(RefCell::new(Vec::new()));
    let input = data.clone();
    let collect = events.clone();
    let mut harness = Harness::new(cx, crate::install, move |_, _| {
        let collect = collect.clone();
        div()
            .w(px(320.))
            .child(
                CartesianChart::new(
                    "sampled",
                    "Raw fixture",
                    ChartScale::Numeric(
                        NumericScale::new(ScaleKind::Linear, [0., 5999.]).expect("x scale"),
                    ),
                    [ValueAxis {
                        id: "y".into(),
                        label: "Units".into(),
                        scale: NumericScale::new(ScaleKind::Linear, [-10., 100.])
                            .expect("value scale"),
                    }],
                )
                .shared_series(input.borrow().clone())
                .animate(false)
                .sampling(PathSampling::MinMax)
                .selected(Some(ChartSelection::new("signal", "time-10")))
                .on_event(move |event, _, _| collect.borrow_mut().push(event)),
            )
            .into_any_element()
    });
    let original = harness
        .node("sampled.series.signal.point.time-10")
        .expect("non-extreme source mark");
    assert!(original.selected);
    assert_eq!(original.value.as_deref(), Some("5"));
    assert!(
        harness
            .node("sampled.series.signal.point.time-1050")
            .is_none()
    );
    assert_eq!(
        harness
            .current_snapshot()
            .nodes
            .iter()
            .filter(|n| n.id.starts_with("sampled.series.signal.point."))
            .count(),
        5900
    );
    harness.click("sampled.series.signal.point.time-13");
    assert!(
        events
            .borrow()
            .contains(&CartesianEvent::Select(Some(ChartSelection::new(
                "signal", "time-13"
            ))))
    );
    {
        let mut data = data.borrow_mut();
        let series = Rc::make_mut(&mut data);
        series[0].points[10].y = Some(-7.);
        series[0].points[10].formatted = "−7 current".into();
        series[0].points.reverse();
    }
    harness.frame();
    let changed = harness
        .node("sampled.series.signal.point.time-10")
        .expect("reordered source mark");
    assert!(changed.selected);
    assert_eq!(changed.value.as_deref(), Some("−7 current"));
    harness.click("sampled.series.signal.point.time-10");
    assert!(
        events
            .borrow()
            .contains(&CartesianEvent::Select(Some(ChartSelection::new(
                "signal", "time-10"
            ))))
    );
}
