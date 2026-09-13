#[cfg(test)]
use super::*;

fn domain() -> GeoColorDomain {
    GeoColorDomain {
        minimum: 0.0,
        maximum: 100.0,
        minimum_label: "0".into(),
        maximum_label: "100".into(),
        missing_label: "Missing".into(),
    }
}
fn props(id: &str, properties: &serde_json::Value) -> Result<GeoProperties, GeoRefusal> {
    Ok(GeoProperties {
        label: id.to_owned().into(),
        value: properties.get("value").and_then(serde_json::Value::as_f64),
        formatted_value: "caller format".into(),
    })
}
fn p(longitude: f64, latitude: f64) -> GeoPosition {
    GeoPosition {
        longitude,
        latitude,
    }
}
fn feature(id: String, ring: Vec<GeoPosition>) -> GeoFeature {
    GeoFeature {
        id: id.into(),
        label: "fixture".into(),
        polygons: vec![GeoPolygon {
            exterior: ring,
            holes: vec![],
        }],
        value: Some(31.0),
        formatted_value: "31".into(),
    }
}

#[test]
fn geojson_seam_hole_and_source_coordinates_survive_auto_world() {
    let json = r#"{"type":"FeatureCollection","features":[
      {"type":"Feature","id":"island","properties":{"value":31},"geometry":{"type":"Polygon","coordinates":[
        [[170,-20],[-170,-20],[-170,30],[170,30],[170,-20]],
        [[175,-5],[-175,-5],[-175,10],[175,10],[175,-5]]]}},
      {"type":"Feature","id":18446744073709551615,"properties":null,"geometry":{"type":"Point","coordinates":[172,20]}}
    ]}"#;
    assert!(matches!(
        GeoData::from_geojson(
            json,
            GeoProjection::WebMercator,
            GeoWorldPolicy::Fixed,
            domain(),
            props
        ),
        Err(GeoRefusal::AntimeridianEdge)
    ));
    let data = GeoData::from_geojson(
        json,
        GeoProjection::WebMercator,
        GeoWorldPolicy::Auto,
        domain(),
        props,
    )
    .expect("auto seam");
    assert_eq!(data.central_longitude(), -180.0);
    assert_eq!(data.features()[0].polygons[0].exterior[0], p(170.0, -20.0));
    assert_eq!(data.points()[0].id.as_ref(), "18446744073709551615");
    let view = data.fit_viewport([640.0, 360.0], 20.0).expect("fit");
    let hit = |position| {
        data.hit_test(
            view,
            [640.0, 360.0],
            view.screen(
                data.project_position(position).expect("project"),
                [640.0, 360.0],
            ),
        )
    };
    assert_eq!(hit(p(171.0, 0.0)).as_deref(), Some("island"));
    assert_eq!(hit(p(179.0, 0.0)), None, "hole crosses original seam too");
    assert_eq!(hit(p(172.0, 20.0)).as_deref(), Some("18446744073709551615"));
    let roundtrip = data
        .unproject_position(data.project_position(p(-177.0, 13.0)).expect("project"))
        .expect("inverse");
    assert!((roundtrip.longitude + 177.0).abs() < 1e-10);
    assert!((roundtrip.latitude - 13.0).abs() < 1e-10);
}

#[test]
fn geojson_refuses_unsupported_without_partial_success() {
    for geometry in [
        "null",
        r#"{"type":"Point","coordinates":[0,1,2]}"#,
        r#"{"type":"LineString","coordinates":[[0,1],[2,3]]}"#,
        r#"{"type":"Point","coordinates":[0,91]}"#,
        r#"{"type":"Polygon","coordinates":[[[0,0],[20,20],[0,20],[20,0],[0,0]]]}"#,
    ] {
        let json =
            format!(r#"{{"type":"Feature","id":"bad","properties":{{}},"geometry":{geometry}}}"#);
        assert!(
            GeoData::from_geojson(
                &json,
                GeoProjection::Equirectangular,
                GeoWorldPolicy::Auto,
                domain(),
                props
            )
            .is_err()
        );
    }
    let foreign = r#"{"type":"FeatureCollection","crs":null,"features":[]}"#;
    assert!(
        GeoData::from_geojson(
            foreign,
            GeoProjection::WebMercator,
            GeoWorldPolicy::Auto,
            domain(),
            props
        )
        .is_err()
    );
}

#[test]
fn indexed_hit_and_visible_candidates_preserve_topmost_identity() {
    let features = (0..1000)
        .map(|i| {
            let x = -170.0 + f64::from(i % 100) * 3.3;
            let y = -70.0 + f64::from(i / 100) * 14.0;
            feature(
                format!("region-{i}"),
                vec![
                    p(x, y),
                    p(x + 1.0, y),
                    p(x + 1.0, y + 3.0),
                    p(x, y + 3.0),
                    p(x, y),
                ],
            )
        })
        .collect();
    let data =
        GeoData::new(GeoProjection::Equirectangular, features, vec![], domain()).expect("grid");
    for i in [0, 51, 376, 999] {
        let position = p(
            -169.5 + f64::from(i % 100) * 3.3,
            -68.5 + f64::from(i / 100) * 14.0,
        );
        let center = data.project_position(position).expect("center");
        let view = GeoViewport { center, zoom: 64.0 };
        assert!(data.visible_indices(view, [500.0, 300.0]).len() < 8);
        assert_eq!(
            data.hit_test(view, [500.0, 300.0], [250.0, 150.0])
                .as_deref(),
            Some(format!("region-{i}").as_str())
        );
    }
}

#[test]
fn simplification_removes_vertices_and_hits_follow_display_geometry() {
    let ring = vec![
        p(-40.0, -20.0),
        p(0.0, -20.0),
        p(40.0, -20.0),
        p(40.0, 0.0),
        p(40.0, 20.0),
        p(0.0, 20.2),
        p(-40.0, 20.0),
        p(-40.0, -20.0),
    ];
    let data = GeoData::new(
        GeoProjection::Equirectangular,
        vec![feature("region".into(), ring)],
        vec![],
        domain(),
    )
    .expect("small bulge");
    let simplified = data.simplified(0.001).expect("simplified");
    assert!(simplified.vertex_count() < data.vertex_count());
    assert_eq!(simplified.features()[0].id, data.features()[0].id);
    assert_eq!(
        simplified.features()[0].polygons[0].exterior.len(),
        8,
        "source geometry retained"
    );
    let view = GeoViewport::default();
    let at = view.screen(
        data.project_position(p(0.0, 20.1)).expect("bulge"),
        [720.0, 360.0],
    );
    assert_eq!(
        data.hit_test(view, [720.0, 360.0], at).as_deref(),
        Some("region")
    );
    assert_eq!(
        simplified.hit_test(view, [720.0, 360.0], at),
        None,
        "no invisible original bulge hit"
    );
    assert_eq!(simplified.features()[0].formatted_value.as_ref(), "31");
}

#[test]
#[ignore = "explicit performance envelope; no wall-clock CI threshold"]
fn geography_preparation_and_query_envelope() {
    for count in [1000, 10000, 100000] {
        let points = (0..count)
            .map(|i| GeoPoint {
                id: format!("point-{i}").into(),
                label: "fixture".into(),
                position: p(
                    -179.0 + (i % 1000) as f64 * 0.358,
                    -80.0 + (i / 1000) as f64 * 1.6,
                ),
            })
            .collect();
        let start = std::time::Instant::now();
        let data = GeoData::new(GeoProjection::Equirectangular, vec![], points, domain())
            .expect("point cloud");
        let prepared = start.elapsed();
        let view = GeoViewport {
            center: data.projected_points[count / 2],
            zoom: 64.0,
        };
        assert_eq!(
            data.hit_test(view, [640.0, 360.0], [320.0, 180.0])
                .as_deref(),
            Some(format!("point-{}", count / 2).as_str())
        );
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            std::hint::black_box(data.hit_test(view, [640.0, 360.0], [320.0, 180.0]));
        }
        eprintln!(
            "geography points={count} prepare={prepared:?} query1000={:?} visible={}",
            start.elapsed(),
            data.visible_indices(view, [640.0, 360.0]).len()
        );
    }
}

use gpui::{div, point, prelude::*, px};
use gpui_kit_testkit::harness::Harness;
use std::{cell::RefCell, rc::Rc};

fn map_fixture() -> Rc<GeoData> {
    Rc::new(
        GeoData::new(
            GeoProjection::Equirectangular,
            vec![feature(
                "region".into(),
                vec![
                    p(-80.0, -40.0),
                    p(80.0, -40.0),
                    p(80.0, 40.0),
                    p(-80.0, 40.0),
                    p(-80.0, -40.0),
                ],
            )],
            vec![],
            domain(),
        )
        .expect("fixture"),
    )
}

#[gpui::test]
fn captured_drag_accepted_refused_outside_cancel_and_exact_hover(cx: &mut gpui::TestAppContext) {
    let data = map_fixture();
    let camera = Rc::new(RefCell::new(GeoViewport::default()));
    let accepted = Rc::new(std::cell::Cell::new(true));
    let events = Rc::new(RefCell::new(Vec::new()));
    let (owner, accept, sink) = (camera.clone(), accepted.clone(), events.clone());
    let mut h = Harness::new(cx, crate::install, move |_, _| {
        let (owner, accept, sink) = (owner.clone(), accept.clone(), sink.clone());
        let viewport = *owner.borrow();
        div()
            .w(px(600.0))
            .child(
                GeoMap::new("geo", "fixture")
                    .state(GeoState::Ready(data.clone()))
                    .viewport(viewport)
                    .on_event(move |event, window, _| {
                        if accept.get()
                            && let GeoEvent::Viewport(view) = event
                        {
                            *owner.borrow_mut() = view;
                        }
                        sink.borrow_mut().push(event);
                        window.refresh();
                    }),
            )
            .into_any_element()
    });
    let center = h.point_in("geo.map");
    h.update(|window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                position: center,
                ..Default::default()
            }),
            cx,
        );
    });
    h.frame();
    assert_eq!(
        h.node("geo.hover-readout")
            .expect("floating exact readout")
            .text
            .as_deref(),
        Some("fixture: 31")
    );
    let before = h.bounds("geo.geometry.region").expect("shape bounds");
    h.drag_start("geo.map");
    h.drag_to(center + point(px(28.0), px(14.0)));
    assert!((camera.borrow().center.x - 0.4).abs() < 1e-10);
    assert!((camera.borrow().center.y - 0.45).abs() < 1e-10);
    let after = h.bounds("geo.geometry.region").expect("moved shape bounds");
    assert!((f32::from(after.left() - before.left()) - 28.0).abs() < 0.1);
    assert!((f32::from(after.top() - before.top()) - 14.0).abs() < 0.1);
    h.update(|window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::MouseCancelled(gpui::MouseCancelEvent),
            cx,
        );
    });
    h.frame();
    assert_eq!(
        *camera.borrow(),
        GeoViewport::default(),
        "cancel rollback is accepted by owner"
    );
    let count = events.borrow().len();
    h.drop_here();
    assert_eq!(
        events.borrow().len(),
        count,
        "cancel is not a selection completion"
    );
    accepted.set(false);
    h.drag_start("geo.map");
    h.drag_to(center + point(px(350.0), px(160.0)));
    assert!(
        matches!(events.borrow().last(),Some(GeoEvent::Viewport(v)) if v.center.x==0.0),
        "capture delivered outside"
    );
    h.drop_here();
    assert_eq!(
        *camera.borrow(),
        GeoViewport::default(),
        "refused proposals are not displayed"
    );
    assert_eq!(
        h.bounds("geo.geometry.region")
            .expect("refused shape bounds"),
        before
    );
}

#[gpui::test]
fn touch_pinch_anchors_centroid_and_cancel_restores_accepted_camera(cx: &mut gpui::TestAppContext) {
    let data = map_fixture();
    let camera = Rc::new(RefCell::new(GeoViewport::default()));
    let owner = camera.clone();
    let mut h = Harness::new(cx, crate::install, move |_, _| {
        let target = owner.clone();
        div()
            .w(px(600.0))
            .child(
                GeoMap::new("geo", "fixture")
                    .state(GeoState::Ready(data.clone()))
                    .viewport(*owner.borrow())
                    .on_event(move |event, window, _| {
                        if let GeoEvent::Viewport(v) = event {
                            *target.borrow_mut() = v;
                        }
                        window.refresh();
                    }),
            )
            .into_any_element()
    });
    let center = h.point_in("geo.map");
    for (id, phase, x) in [
        (1, gpui::TouchPhase::Started, -50.0),
        (2, gpui::TouchPhase::Started, 50.0),
        (2, gpui::TouchPhase::Moved, 100.0),
    ] {
        h.update(|window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::Touch(gpui::TouchEvent {
                    id: gpui::TouchId(id),
                    phase,
                    position: center + point(px(x), px(0.0)),
                    ..Default::default()
                }),
                cx,
            );
        });
        h.frame();
    }
    assert!(
        (camera.borrow().zoom - 1.5).abs() < 1e-10,
        "camera {:?}",
        camera.borrow()
    );
    assert!(
        (camera.borrow().center.x - (0.5 - 25.0 / 420.0)).abs() < 1e-8,
        "original midpoint stays under moving centroid"
    );
    h.update(|window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::Touch(gpui::TouchEvent {
                id: gpui::TouchId(2),
                phase: gpui::TouchPhase::Cancelled,
                position: center,
                ..Default::default()
            }),
            cx,
        );
    });
    h.frame();
    assert_eq!(*camera.borrow(), GeoViewport::default());
}

#[gpui::test]
fn camera_motion_interrupts_from_displayed_bounds_and_direct_input_snaps(
    cx: &mut gpui::TestAppContext,
) {
    let data = map_fixture();
    let camera = Rc::new(RefCell::new(GeoViewport::default()));
    let owner = camera.clone();
    let mut h = Harness::new(cx, crate::install, move |_, _| {
        let target = owner.clone();
        div()
            .w(px(600.0))
            .child(
                GeoMap::new("geo", "fixture")
                    .state(GeoState::Ready(data.clone()))
                    .viewport(*owner.borrow())
                    .on_event(move |event, window, _| {
                        if let GeoEvent::Viewport(v) = event {
                            *target.borrow_mut() = v;
                        }
                        window.refresh();
                    }),
            )
            .into_any_element()
    });
    let before = h.bounds("geo.geometry.region").expect("start");
    camera.borrow_mut().zoom = 2.0;
    h.frame();
    h.advance(std::time::Duration::from_millis(60));
    let middle = h.bounds("geo.geometry.region").expect("in flight");
    assert!(middle.size.width > before.size.width && middle.size.width < before.size.width * 2.0);
    camera.borrow_mut().zoom = 1.2;
    h.frame();
    let interrupted = h.bounds("geo.geometry.region").expect("retarget");
    assert!(
        (f32::from(interrupted.size.width - middle.size.width)).abs() < 0.1,
        "no jump to previous target"
    );
    h.advance(std::time::Duration::from_millis(1000));
    let settled = h.bounds("geo.geometry.region").expect("settled");
    assert!(((settled.size.width / before.size.width) - 1.2).abs() < 0.001);
    h.click("geo.map");
    h.keystrokes("tab =");
    assert_eq!(
        h.node("geo.map").expect("direct zoom").value.as_deref(),
        Some("zoom 1.5; center 0.5, 0.5")
    );
    h.update(|_, cx| cx.set_reduce_motion(true));
    camera.borrow_mut().zoom = 2.0;
    h.frame();
    assert_eq!(
        h.node("geo.map")
            .expect("reduced-motion snap")
            .value
            .as_deref(),
        Some("zoom 2; center 0.5, 0.5")
    );
}

#[test]
fn simplification_preserves_holes_when_reduction_would_eject_them() {
    let mut f = feature(
        "bulge".into(),
        vec![
            p(-40.0, -20.0),
            p(40.0, -20.0),
            p(40.0, 20.0),
            p(0.0, 20.8),
            p(-40.0, 20.0),
            p(-40.0, -20.0),
        ],
    );
    f.polygons[0].holes = vec![vec![
        p(-0.1, 20.3),
        p(0.1, 20.3),
        p(0.1, 20.5),
        p(-0.1, 20.5),
        p(-0.1, 20.3),
    ]];
    let source = GeoData::new(GeoProjection::Equirectangular, vec![f], vec![], domain())
        .expect("hole in bulge");
    let display = source
        .simplified(0.003)
        .expect("topology-preserving fallback");
    assert_eq!(source.vertex_count(), display.vertex_count());
    let view = GeoViewport::default();
    let size = [800.0, 500.0];
    for (at, expected) in [(p(0.0, 20.4), None), (p(0.0, 20.6), Some("bulge"))] {
        assert_eq!(
            display
                .hit_test(
                    view,
                    size,
                    view.screen(display.project_position(at).expect("project"), size)
                )
                .as_deref(),
            expected
        );
    }
}

#[gpui::test]
fn raw_touch_pan_rejects_outside_start_and_preserves_refused_camera(cx: &mut gpui::TestAppContext) {
    let data = map_fixture();
    let camera = Rc::new(RefCell::new(GeoViewport::default()));
    let accept = Rc::new(std::cell::Cell::new(true));
    let events = Rc::new(RefCell::new(Vec::new()));
    let (owner, accepting, sink) = (camera.clone(), accept.clone(), events.clone());
    let mut h = Harness::new(cx, crate::install, move |_, _| {
        let (owner, accepting, sink) = (owner.clone(), accepting.clone(), sink.clone());
        let viewport = *owner.borrow();
        div()
            .w(px(600.0))
            .child(
                GeoMap::new("geo", "fixture")
                    .state(GeoState::Ready(data.clone()))
                    .viewport(viewport)
                    .on_event(move |event, window, _| {
                        if accepting.get()
                            && let GeoEvent::Viewport(viewport) = event
                        {
                            *owner.borrow_mut() = viewport;
                        }
                        sink.borrow_mut().push(event);
                        window.refresh();
                    }),
            )
            .into_any_element()
    });
    let center = h.point_in("geo.map");
    let touch = |h: &mut Harness, phase, at| {
        h.update(|window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::Touch(gpui::TouchEvent {
                    id: gpui::TouchId(9),
                    position: at,
                    phase,
                    ..Default::default()
                }),
                cx,
            );
        });
        h.frame();
    };
    touch(
        &mut h,
        gpui::TouchPhase::Started,
        center + point(px(340.0), px(0.0)),
    );
    touch(
        &mut h,
        gpui::TouchPhase::Moved,
        center + point(px(360.0), px(14.0)),
    );
    touch(&mut h, gpui::TouchPhase::Cancelled, center);
    assert!(
        events.borrow().is_empty(),
        "outside contact cannot acquire map pan"
    );
    touch(&mut h, gpui::TouchPhase::Started, center);
    touch(
        &mut h,
        gpui::TouchPhase::Moved,
        center + point(px(28.0), px(14.0)),
    );
    assert!((camera.borrow().center.x - 0.4).abs() < 1e-9);
    touch(&mut h, gpui::TouchPhase::Cancelled, center);
    assert_eq!(*camera.borrow(), GeoViewport::default());
    accept.set(false);
    touch(&mut h, gpui::TouchPhase::Started, center);
    touch(
        &mut h,
        gpui::TouchPhase::Moved,
        center + point(px(350.0), px(160.0)),
    );
    assert!(matches!(events.borrow().last(), Some(GeoEvent::Viewport(v)) if v.center.x == 0.0));
    touch(
        &mut h,
        gpui::TouchPhase::Ended,
        center + point(px(350.0), px(160.0)),
    );
    assert_eq!(*camera.borrow(), GeoViewport::default());
    assert!(
        events
            .borrow()
            .iter()
            .all(|e| matches!(e, GeoEvent::Viewport(_))),
        "pan does not select"
    );
}

#[gpui::test]
fn geojson_point_readings_reach_exact_hover_and_semantics(cx: &mut gpui::TestAppContext) {
    let json = r#"{"type":"Feature","id":"sample","properties":{"value":73.125},"geometry":{"type":"Point","coordinates":[0,0]}}"#;
    let data = GeoData::from_geojson(
        json,
        GeoProjection::Equirectangular,
        GeoWorldPolicy::Fixed,
        domain(),
        |_, value| {
            Ok(GeoProperties {
                label: "Station reading".into(),
                value: value.get("value").and_then(serde_json::Value::as_f64),
                formatted_value: "73.125 units".into(),
            })
        },
    )
    .expect("point reading");
    assert_eq!(
        data.point_reading("sample")
            .expect("retained metadata")
            .value,
        Some(73.125)
    );
    let data = Rc::new(data);
    let mut h = Harness::new(cx, crate::install, move |_, _| {
        div()
            .w(px(600.0))
            .child(
                GeoMap::new("geo", "fixture")
                    .state(GeoState::Ready(data.clone()))
                    .on_event(|_, _, _| {}),
            )
            .into_any_element()
    });
    for id in ["geo.geometry.sample", "geo.feature.sample"] {
        assert_eq!(
            h.node(id).expect("exact point target").value.as_deref(),
            Some("73.125 units")
        );
    }
    let center = h.point_in("geo.map");
    h.update(|window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                position: center,
                ..Default::default()
            }),
            cx,
        );
    });
    h.frame();
    assert_eq!(
        h.node("geo.hover-readout")
            .expect("exact point hover")
            .text
            .as_deref(),
        Some("Station reading: 73.125 units")
    );
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            GeoData::from_geojson(
                json,
                GeoProjection::Equirectangular,
                GeoWorldPolicy::Fixed,
                domain(),
                |_, _| Ok(GeoProperties {
                    label: "Station".into(),
                    value: Some(value),
                    formatted_value: "must refuse".into(),
                })
            ),
            Err(GeoRefusal::InvalidValue)
        ));
    }
}

#[gpui::test]
fn measured_point_preserves_fractional_bounds_native_values_and_camera(
    cx: &mut gpui::TestAppContext,
) {
    let data = Rc::new(GeoData::from_geojson(
        r#"{"type":"Feature","id":"station","properties":{},"geometry":{"type":"Point","coordinates":[13.37,7.19]}}"#,
        GeoProjection::Equirectangular, GeoWorldPolicy::Fixed, domain(),
        |_, _| Ok(GeoProperties {label:"Fractional station".into(), value:Some(73.125), formatted_value:"73.125 units".into()}),
    ).expect("fractional point"));
    let camera = Rc::new(RefCell::new(GeoViewport::default()));
    let owner = camera.clone();
    let mut h = Harness::new(cx, crate::install, move |_, _| {
        let target = owner.clone();
        div()
            .w(px(613.0))
            .child(
                GeoMap::new("geo", "fixture")
                    .state(GeoState::Ready(data.clone()))
                    .viewport(*owner.borrow())
                    .selected(Some("station".into()))
                    .on_event(move |event, window, _| {
                        if let GeoEvent::Viewport(viewport) = event {
                            *target.borrow_mut() = viewport;
                        }
                        window.refresh();
                    }),
            )
            .into_any_element()
    });
    h.click("geo.map");
    let mut native_id = None;
    for keys in ["", "= right"] {
        if !keys.is_empty() {
            h.keystrokes(keys);
        }
        let bounds = h.bounds("geo.map").expect("map bounds");
        let node = h
            .node("geo.geometry.station")
            .expect("current measured station");
        let view = *camera.borrow();
        let x = f64::from(f32::from(bounds.left()))
            + 613.0 / 2.0
            + (13.37 / 360.0 + 0.5 - view.center.x) * 280.0 * view.zoom
            - 5.0;
        let y = f64::from(f32::from(bounds.top()))
            + 140.0
            + (0.5 - 7.19 / 360.0 - view.center.y) * 280.0 * view.zoom
            - 5.0;
        assert!((f64::from(node.bounds.x) - x).abs() < 0.0001);
        assert!((f64::from(node.bounds.y) - y).abs() < 0.0001);
        assert_eq!(node.bounds.width, 10.0);
        assert_eq!(node.bounds.height, 10.0);
        assert_eq!(node.parent.as_deref(), Some("geo.map"));
        assert!(node.selected && node.read_only);
        assert_eq!(node.value.as_deref(), Some("73.125 units"));
        let scale = h.update(|window, _| f64::from(window.scale_factor()));
        let tree = h.accessibility_tree();
        let native = tree["nodes"]
            .as_object()
            .expect("native nodes")
            .values()
            .find(|node| {
                node["aria"]["role"] == "Image" && node["aria"]["label"] == "Fractional station"
            })
            .expect("native station");
        assert_eq!(native["aria"]["value"], "73.125 units");
        for (field, expected) in [("x0", x), ("y0", y), ("x1", x + 10.0), ("y1", y + 10.0)] {
            assert!(
                (native["bounds"][field]
                    .as_f64()
                    .expect("native physical bound")
                    - expected * scale)
                    .abs()
                    < 0.001
            );
        }
        let id = native["accesskit_id"].clone();
        if let Some(previous) = &native_id {
            assert_eq!(previous, &id);
        }
        native_id = Some(id);
    }
    assert_eq!(camera.borrow().zoom, 1.25);
    assert!((camera.borrow().center.x - 0.58).abs() < 1e-12);
}

#[gpui::test]
fn geometry_lifecycle_excludes_retired_shapes_from_authority(cx: &mut gpui::TestAppContext) {
    let old = map_fixture();
    let next = Rc::new(
        GeoData::new(
            GeoProjection::Equirectangular,
            vec![feature(
                "replacement".into(),
                vec![
                    p(100.0, -30.0),
                    p(150.0, -30.0),
                    p(150.0, 30.0),
                    p(100.0, 30.0),
                    p(100.0, -30.0),
                ],
            )],
            vec![],
            domain(),
        )
        .expect("replacement"),
    );
    let source = Rc::new(RefCell::new(old.clone()));
    let owner = source.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let mut h = Harness::new(cx, crate::install, move |_, _| {
        let sink = sink.clone();
        div()
            .w(px(600.0))
            .child(
                GeoMap::new("geo", "fixture")
                    .state(GeoState::Ready(owner.borrow().clone()))
                    .selected(Some("region".into()))
                    .on_event(move |event, _, _| sink.borrow_mut().push(event)),
            )
            .into_any_element()
    });
    // Initial mount is deliberately visible immediately, even with motion on;
    // only subsequent source changes create entering/decorative lifetimes.
    assert!(h.node("geo.geometry.region").is_some());
    let native_shapes = |h: &mut Harness| {
        h.accessibility_tree()["nodes"]
            .as_object()
            .expect("native nodes")
            .values()
            .filter(|node| node["aria"]["role"] == "Image" && node["aria"]["value"] == "31")
            .map(|node| node["accesskit_id"].clone())
            .collect::<Vec<_>>()
    };
    let original_native = native_shapes(&mut h);
    assert_eq!(original_native.len(), 1);
    h.update(|window, cx| {
        use gpui_kit_theme::ActiveTheme;
        cx.set_reduce_motion(false);
        let spec = crate::motion::state_change(cx.theme());
        let initial = super::presentation::Presentation::default().sample(
            old.clone(),
            spec,
            false,
            window,
            cx,
        );
        assert_eq!(initial.opacity(0), 1.0);
        assert!(initial.retired.is_empty());
    });
    *source.borrow_mut() = next;
    h.frame();
    assert!(
        h.node("geo.feature.region").is_none(),
        "retired readout removed immediately"
    );
    assert!(
        h.node("geo.geometry.region").is_none(),
        "decorative old geometry has no measured target"
    );
    assert!(
        h.node("geo.geometry.replacement").is_none(),
        "zero-opacity incoming shape is not a visible target"
    );
    assert!(
        native_shapes(&mut h).is_empty(),
        "retired and transparent shapes have no native authority"
    );
    assert_eq!(
        h.node("geo.feature.replacement")
            .expect("exact new readout")
            .value
            .as_deref(),
        Some("31")
    );
    h.advance(std::time::Duration::from_millis(60));
    let entering_native = native_shapes(&mut h);
    assert_eq!(entering_native.len(), 1);
    assert_ne!(entering_native, original_native);
    let shape = h
        .bounds("geo.geometry.replacement")
        .expect("visible incoming shape");
    let bounds = h.bounds("geo.map").expect("map");
    assert!(
        (f32::from(shape.left() - bounds.left()) - (300.0 + 100.0 * 280.0 / 360.0)).abs() <= 0.5
    );
    h.click("geo.map");
    assert_eq!(
        events.borrow().last(),
        Some(&GeoEvent::Select(None)),
        "retired center polygon is never picked"
    );
    *source.borrow_mut() = old;
    h.update(|_, cx| cx.set_reduce_motion(true));
    h.frame();
    assert!(h.node("geo.geometry.region").is_some());
    assert!(h.node("geo.geometry.replacement").is_none());
    assert_eq!(native_shapes(&mut h), original_native);
}

#[gpui::test]
fn geography_mounted_work_and_virtual_readout_envelope(cx: &mut gpui::TestAppContext) {
    for (count, zoom) in [
        (1000, 64.0),
        (10000, 64.0),
        (100000, 64.0),
        (1000, 1.0),
        (10000, 1.0),
    ] {
        assert!(cx.update(|cx| cx.windows().is_empty()));
        let data = Rc::new(
            GeoData::new(
                GeoProjection::Equirectangular,
                vec![],
                (0..count)
                    .map(|i| GeoPoint {
                        id: format!("point-{i}").into(),
                        label: "Synthetic point".into(),
                        position: p(
                            -179.0 + (i % 1000) as f64 * 0.358,
                            -80.0 + (i / 1000) as f64 * 1.6,
                        ),
                    })
                    .collect(),
                domain(),
            )
            .expect("indexed cloud"),
        );
        let view = GeoViewport {
            center: data.projected_points[count / 2],
            zoom,
        };
        let visible_count = data.visual_bounds(view, [640.0, 280.0]).len();
        if zoom == 1.0 {
            assert_eq!(
                visible_count, count,
                "the full-world fixture fits every source point"
            );
        }
        let start = std::time::Instant::now();
        let mut h = Harness::new(cx, crate::install, move |_, _| {
            div()
                .w(px(640.0))
                .child(
                    GeoMap::new("geo", "fixture")
                        .state(GeoState::Ready(data.clone()))
                        .viewport(view),
                )
                .into_any_element()
        });
        let mount = start.elapsed();
        let start = std::time::Instant::now();
        h.frame();
        let redraw = start.elapsed();
        let stats = h.frame_stats();
        let snapshot = h.current_snapshot();
        assert_eq!(
            snapshot
                .nodes
                .iter()
                .filter(|n| n.id.starts_with("geo.geometry."))
                .count(),
            visible_count
        );
        let native = h.accessibility_tree();
        assert_eq!(
            native["nodes"]
                .as_object()
                .expect("native nodes")
                .values()
                .filter(|node| node["aria"]["role"] == "Image"
                    && node["aria"]["label"] == "Synthetic point")
                .count(),
            visible_count
        );
        assert!(
            stats.paint_calls < 80,
            "source targets must not allocate per-feature elements"
        );
        let rows = snapshot
            .descendants_of("geo.feature")
            .iter()
            .filter(|n| n.role == gpui_kit_semantics::Role::Row)
            .count();
        assert!(
            (6..=12).contains(&rows),
            "virtual readout must not build the full source"
        );
        assert!(
            stats.paint_calls
                < if zoom > 1.0 {
                    1500
                } else {
                    count as u64 * 2 + 1500
                },
            "bounded viewport paint work, not a wall-clock promise"
        );
        eprintln!(
            "geography mounted points={count} zoom={zoom} test_platform_mount={mount:?} redraw={redraw:?} rows={rows} paint_calls={}; GPU submission not measured",
            stats.paint_calls
        );
        // Harness::frame refreshes all application windows; dropping a Harness
        // alone does not remove its window or release its retained fixture.
        h.update(|window, _| window.remove_window());
        drop(h);
        assert!(cx.update(|cx| cx.windows().is_empty()));
    }
}

#[gpui::test]
#[ignore = "isolated native/diagnostic target cost; no GPU timing"]
fn geography_semantic_work_breakdown(cx: &mut gpui::TestAppContext) {
    use gpui_kit_semantics::{NodeSpec, Role, Semantic, SemanticCoordinator};
    struct Targets;
    impl gpui::Render for Targets {
        fn render(
            &mut self,
            window: &mut gpui::Window,
            cx: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            SemanticCoordinator::global(cx).begin_frame(window);
            // Exact projected G4 dense fixture bounds at the same camera/size.
            div().size_full().child(
                div()
                    .relative()
                    .w(px(640.0))
                    .h(px(280.0))
                    .overflow_hidden()
                    .children((0..10000).map(|i| {
                        let x = 320.0 + f64::from(i % 1000) * 0.358 / 360.0 * 280.0;
                        let y = 140.0 - (f64::from(i / 1000) - 5.0) * 1.6 / 360.0 * 280.0;
                        div()
                            .absolute()
                            .left(px((x - 5.0) as f32))
                            .top(px((y - 5.0) as f32))
                            .w(px(10.0))
                            .h(px(10.0))
                            .semantic_in(
                                cx,
                                NodeSpec::new(format!("geometry.point-{i}"), Role::Image)
                                    .text("Synthetic point")
                                    .read_only(true),
                            )
                    })),
            )
        }
    }
    cx.update(crate::install);
    for (native, diagnostic) in [(false, false), (true, false), (false, true), (true, true)] {
        assert!(cx.update(|cx| cx.windows().is_empty()));
        let arm = diagnostic.then(|| cx.update(|cx| SemanticCoordinator::global(cx).arm()));
        let handle: gpui::AnyWindowHandle = cx.add_window(|_, _| Targets).into();
        if native {
            cx.activate_accessibility(handle);
        }
        cx.run_until_parked();
        let mut samples = Vec::new();
        for _ in 0..5 {
            let started = std::time::Instant::now();
            cx.update(|cx| cx.refresh_windows());
            cx.run_until_parked();
            samples.push(started.elapsed());
        }
        samples.sort();
        let elapsed = samples[2];
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(window.is_a11y_active(), native);
            let snapshot = SemanticCoordinator::global(cx).snapshot(handle.window_id()).expect("target snapshot");
            assert_eq!(snapshot.nodes.len(), if diagnostic {10000} else {0});
            if native {
                let json = window.debug_a11y_tree_json().expect("active native tree");
                let tree: serde_json::Value = serde_json::from_str(&json).expect("native tree JSON");
                assert_eq!(tree["nodes"].as_object().expect("native nodes").values()
                    .filter(|node| node["aria"]["role"] == "Image").count(), 10000);
            }
            eprintln!("10000 targets native={native} diagnostic={diagnostic} median5={elapsed:?} range={:?}..{:?} per_target_ns={} debug_assertions={}; no serialization/GPU in timer", samples[0], samples[4], elapsed.as_nanos()/10000, cfg!(debug_assertions));
            window.remove_window();
        }).expect("target window");
        cx.run_until_parked();
        drop(arm);
        assert!(cx.update(|cx| cx.windows().is_empty()));
    }
}

#[gpui::test]
fn virtual_geography_readout_reaches_unmounted_identity(cx: &mut gpui::TestAppContext) {
    let data = Rc::new(
        GeoData::new(
            GeoProjection::Equirectangular,
            vec![],
            (0..1000)
                .map(|i| GeoPoint {
                    id: format!("sample-{i}").into(),
                    label: format!("Sample {i}").into(),
                    position: p(i as f64 / 10.0, 0.0),
                })
                .collect(),
            domain(),
        )
        .expect("point readout"),
    );
    let selected = Rc::new(RefCell::new(None));
    let owner = selected.clone();
    let mut h = Harness::new(cx, crate::install, move |_, _| {
        let target = owner.clone();
        div()
            .w(px(640.0))
            .child(
                GeoMap::new("geo", "fixture")
                    .state(GeoState::Ready(data.clone()))
                    .selected(owner.borrow().clone())
                    .on_event(move |event, window, _| {
                        if let GeoEvent::Select(id) = event {
                            *target.borrow_mut() = id;
                        }
                        window.refresh();
                    }),
            )
            .into_any_element()
    });
    assert!(h.node("geo.feature.sample-999").is_none());
    h.click("geo.feature.sample-0");
    h.keystrokes("end");
    assert_eq!(selected.borrow().as_deref(), Some("sample-999"));
    assert!(
        h.node("geo.feature.sample-999")
            .expect("revealed last identity")
            .selected
    );
    assert!(h.node("geo.feature.sample-0").is_none());
}

#[gpui::test]
fn whole_map_unmount_cancels_capture_without_release_or_remount_draft(
    cx: &mut gpui::TestAppContext,
) {
    let mounted = Rc::new(std::cell::Cell::new(true));
    let camera = Rc::new(RefCell::new(GeoViewport::default()));
    let events = Rc::new(RefCell::new(Vec::new()));
    let (show, owner, sink) = (mounted.clone(), camera.clone(), events.clone());
    let data = map_fixture();
    let mut h = Harness::new(cx, crate::install, move |_, _| {
        let mut body = div().w(px(600.0));
        if show.get() {
            let (target, sink) = (owner.clone(), sink.clone());
            body = body.child(
                GeoMap::new("geo", "fixture")
                    .state(GeoState::Ready(data.clone()))
                    .viewport(*owner.borrow())
                    .on_event(move |event, window, _| {
                        if let GeoEvent::Viewport(view) = event {
                            *target.borrow_mut() = view;
                        }
                        sink.borrow_mut().push(event);
                        window.refresh();
                    }),
            );
        }
        body.into_any_element()
    });
    h.drag_start("geo.map");
    h.drag_to(h.pointer() + gpui::point(px(31.0), px(17.0)));
    assert_ne!(*camera.borrow(), GeoViewport::default());
    // Remove the entire component without calling its cancel adapter first.
    mounted.set(false);
    h.frame();
    assert!(h.node("geo.map").is_none());
    assert_eq!(*camera.borrow(), GeoViewport::default());
    let after_cancel = events.borrow().len();
    h.drop_here();
    assert_eq!(events.borrow().len(), after_cancel);
    mounted.set(true);
    h.frame();
    assert!(h.node("geo.map").is_some());
    h.drag_to(h.pointer() + gpui::point(px(13.0), px(7.0)));
    h.drop_here();
    assert_eq!(events.borrow().len(), after_cancel);
    assert_eq!(*camera.borrow(), GeoViewport::default());
}

#[gpui::test]
#[ignore = "explicit isolated CPU breakdown; run debug and release separately"]
fn geography_dense_work_breakdown(cx: &mut gpui::TestAppContext) {
    use gpui_kit_semantics::{NodeSpec, Role, Semantic};
    use gpui_kit_theme::ActiveTheme;
    let started = std::time::Instant::now();
    let data = Rc::new(
        GeoData::new(
            GeoProjection::Equirectangular,
            vec![],
            (0..10000)
                .map(|i| GeoPoint {
                    id: format!("point-{i}").into(),
                    label: "Synthetic point".into(),
                    position: p(
                        -179.0 + (i % 1000) as f64 * 0.358,
                        -80.0 + (i / 1000) as f64 * 1.6,
                    ),
                })
                .collect(),
            domain(),
        )
        .expect("indexed cloud"),
    );
    eprintln!(
        "dense preparation {:?}; debug_assertions={}",
        started.elapsed(),
        cfg!(debug_assertions)
    );
    let viewport = GeoViewport {
        center: data.projected_points[5000],
        zoom: 1.0,
    };
    let started = std::time::Instant::now();
    for _ in 0..100 {
        std::hint::black_box(data.visual_bounds(viewport, [640.0, 280.0]));
        std::hint::black_box(data.visible_indices(viewport, [640.0, 280.0]));
        std::hint::black_box(
            (0..data.count())
                .map(|i| data.identity(i).clone())
                .collect::<Vec<_>>(),
        );
    }
    eprintln!(
        "dense prepared bounds + candidates + readout keys per traversal {:?}",
        started.elapsed() / 100
    );
    for mode in ["full", "paint", "bounds", "batch"] {
        assert!(cx.update(|cx| cx.windows().is_empty()));
        let source = data.clone();
        let mut h = Harness::new(cx, crate::install, move |_, cx| {
            let theme = cx.theme().clone();
            let source = source.clone();
            let mut body = div().relative().w(px(640.0)).h(px(280.0)).overflow_hidden();
            if mode == "full" {
                return div()
                    .w(px(640.0))
                    .child(
                        GeoMap::new("geo", "fixture")
                            .state(GeoState::Ready(source))
                            .viewport(viewport),
                    )
                    .into_any_element();
            }
            if mode == "paint" {
                body = body.child(
                    gpui::canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let paint = super::painting::Paint {
                                viewport,
                                bounds,
                                theme: &theme,
                                selected: None,
                            };
                            for index in source.visible_indices(viewport, [640.0, 280.0]) {
                                paint.draw(&source, index, None, 1.0, window);
                            }
                        },
                    )
                    .size_full(),
                );
            } else if mode == "batch" {
                body = body.child(
                    gpui_kit_semantics::MeasuredLeafBatch::new("batch", move |current, _, _| {
                        source
                            .visual_bounds(viewport, [640.0, 280.0])
                            .into_iter()
                            .map(|(index, bounds)| {
                                gpui_kit_semantics::MeasuredLeaf::new(
                                    format!("geometry.{}", source.identity(index)),
                                    gpui_kit_semantics::MeasuredLeafRole::Image,
                                    gpui::Bounds::new(
                                        current.origin
                                            + gpui::point(
                                                px(bounds[0] as f32),
                                                px(bounds[1] as f32),
                                            ),
                                        gpui::size(px(bounds[2] as f32), px(bounds[3] as f32)),
                                    ),
                                )
                                .text("Synthetic point")
                                .read_only(true)
                            })
                            .collect()
                    })
                    .size_full(),
                );
            } else {
                for (index, bounds) in source.visual_bounds(viewport, [640.0, 280.0]) {
                    body = body.child(
                        div()
                            .absolute()
                            .left(px(bounds[0] as f32))
                            .top(px(bounds[1] as f32))
                            .w(px(bounds[2] as f32))
                            .h(px(bounds[3] as f32))
                            .semantic_in(
                                cx,
                                NodeSpec::new(
                                    format!("geometry.{}", source.identity(index)),
                                    Role::Image,
                                )
                                .text("Synthetic point")
                                .read_only(true),
                            ),
                    );
                }
            }
            body.into_any_element()
        });
        let started = std::time::Instant::now();
        h.frame();
        eprintln!(
            "dense mode={mode} isolated redraw {:?}; element paint calls {}; test platform, no GPU",
            started.elapsed(),
            h.frame_stats().paint_calls
        );
        h.update(|window, _| window.remove_window());
        drop(h);
        assert!(cx.update(|cx| cx.windows().is_empty()));
    }
}
