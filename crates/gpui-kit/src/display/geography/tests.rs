#[cfg(test)]
use super::*;
use gpui::{IntoElement, ParentElement, Styled, div, px};
use gpui_kit_testkit::harness::Harness;
use std::{cell::RefCell, rc::Rc};

fn position(longitude: f64, latitude: f64) -> GeoPosition {
    GeoPosition {
        longitude,
        latitude,
    }
}
fn ring(coords: &[(f64, f64)]) -> Vec<GeoPosition> {
    coords.iter().map(|&(x, y)| position(x, y)).collect()
}
fn rectangle(w: f64, s: f64, e: f64, n: f64) -> Vec<GeoPosition> {
    ring(&[(w, s), (e, s), (e, n), (w, n), (w, s)])
}
fn feature(exterior: Vec<GeoPosition>, holes: Vec<Vec<GeoPosition>>) -> GeoFeature {
    GeoFeature {
        id: "region".into(),
        label: "Synthetic region".into(),
        polygons: vec![GeoPolygon { exterior, holes }],
        value: Some(0.0),
        formatted_value: "0 units".into(),
    }
}
fn domain() -> GeoColorDomain {
    GeoColorDomain {
        minimum: -10.0,
        maximum: 30.0,
        minimum_label: "-10".into(),
        maximum_label: "30".into(),
        missing_label: "Unobserved".into(),
    }
}
fn data(features: Vec<GeoFeature>) -> Result<GeoData, GeoRefusal> {
    GeoData::new(GeoProjection::Equirectangular, features, vec![], domain())
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-11, "{a} != {b}");
}

#[test]
fn independent_projection_values_and_bounds() {
    let p = GeoProjection::Equirectangular
        .project(position(-72.0, 36.0))
        .expect("finite equirectangular position");
    close(p.x, 0.3);
    close(p.y, 0.4);
    // Mercator at 45°: ln(1 + sqrt(2)); independently tabulated normalized y.
    let p = GeoProjection::WebMercator
        .project(position(90.0, 45.0))
        .expect("finite Mercator position");
    close(p.x, 0.75);
    close(p.y, 0.359725036915205);
    for projection in [GeoProjection::Equirectangular, GeoProjection::WebMercator] {
        for latitude in [
            -projection.latitude_limit(),
            -31.7,
            0.0,
            74.2,
            projection.latitude_limit(),
        ] {
            let p = projection
                .project(position(-123.4, latitude))
                .expect("inclusive projection domain");
            let roundtrip = projection
                .unproject(p)
                .expect("forward projection stays in inverse domain");
            close(roundtrip.longitude, -123.4);
            close(roundtrip.latitude, latitude);
        }
        assert_eq!(
            projection.project(position(180.1, 0.0)),
            Err(GeoRefusal::CoordinateBounds)
        );
        assert_eq!(
            projection.project(position(0.0, projection.latitude_limit() + 0.000001)),
            Err(GeoRefusal::CoordinateBounds)
        );
        assert_eq!(
            projection.project(position(f64::NAN, 0.0)),
            Err(GeoRefusal::CoordinateBounds)
        );
    }
    assert_eq!(
        GeoProjection::Equirectangular.unproject(GeoProjected { x: 0.4, y: 0.1 }),
        Err(GeoRefusal::CoordinateBounds)
    );
}

#[test]
fn polygon_holes_boundaries_winding_and_asymmetric_viewport() {
    let exterior = rectangle(-90.0, -45.0, 90.0, 45.0);
    let hole = rectangle(-18.0, -18.0, 36.0, 18.0);
    for reverse in [false, true] {
        let mut exterior = exterior.clone();
        let mut hole = hole.clone();
        if reverse {
            exterior.reverse();
            hole.reverse();
        }
        let data = data(vec![feature(exterior, vec![hole])])
            .expect("simple ring with strict interior hole");
        let view = GeoViewport::default();
        // In a 720×360 frame: one pixel per degree; center is (360,180).
        for p in [[280.0, 180.0], [270.0, 180.0], [400.0, 150.0]] {
            assert_eq!(
                data.hit_test(view, [720.0, 360.0], p).as_deref(),
                Some("region")
            );
        }
        for p in [
            [360.0, 180.0],
            [342.0, 180.0],
            [396.0, 180.0],
            [269.0, 180.0],
            [-1.0, 180.0],
        ] {
            assert_eq!(data.hit_test(view, [720.0, 360.0], p), None);
        }
        let view = GeoViewport {
            center: GeoProjected { x: 0.3, y: 0.5 },
            zoom: 2.0,
        };
        assert_eq!(
            data.hit_test(view, [300.0, 400.0], [150.0, 200.0])
                .as_deref(),
            Some("region")
        );
    }
}

#[test]
fn invalid_rings_and_holes_are_refusals() {
    for exterior in [
        ring(&[
            (0.0, 0.0),
            (30.0, 30.0),
            (0.0, 30.0),
            (30.0, 0.0),
            (0.0, 0.0),
        ]),
        ring(&[
            (0.0, 0.0),
            (30.0, 0.0),
            (15.0, 0.0),
            (30.0, 20.0),
            (0.0, 0.0),
        ]),
        ring(&[(0.0, 0.0), (30.0, 0.0), (30.0, 20.0)]),
        ring(&[
            (0.0, 0.0),
            (30.0, 0.0),
            (30.0, 0.0),
            (30.0, 20.0),
            (0.0, 0.0),
        ]),
    ] {
        assert_eq!(
            data(vec![feature(exterior, vec![])]).expect_err("invalid ring must refuse"),
            GeoRefusal::InvalidRing
        );
    }
    for holes in [
        vec![rectangle(-90.0, -10.0, 0.0, 10.0)], // touches exterior
        vec![rectangle(100.0, -10.0, 120.0, 10.0)], // outside
        vec![
            rectangle(-30.0, -30.0, 30.0, 30.0),
            rectangle(-10.0, -10.0, 10.0, 10.0),
        ], // nested
        vec![
            rectangle(-30.0, -30.0, 10.0, 10.0),
            rectangle(0.0, 0.0, 30.0, 30.0),
        ], // overlaps
    ] {
        assert_eq!(
            data(vec![feature(rectangle(-90.0, -45.0, 90.0, 45.0), holes)])
                .expect_err("invalid holes must refuse"),
            GeoRefusal::InvalidHoles
        );
    }
}

#[test]
fn antimeridian_requires_caller_cut_geometry() {
    assert_eq!(
        data(vec![feature(rectangle(170.0, 10.0, -170.0, 20.0), vec![])])
            .expect_err("uncut seam crossing must refuse"),
        GeoRefusal::AntimeridianEdge
    );
    let mut split = feature(rectangle(170.0, 10.0, 180.0, 20.0), vec![]);
    split.polygons.push(GeoPolygon {
        exterior: rectangle(-180.0, 10.0, -170.0, 20.0),
        holes: vec![],
    });
    let data = data(vec![split]).expect("caller-cut antimeridian geometry");
    for x in [180.0, 185.0, 535.0, 540.0] {
        assert_eq!(
            data.hit_test(GeoViewport::default(), [720.0, 360.0], [x, 165.0])
                .as_deref(),
            Some("region")
        );
    }
    assert!(
        data.hit_test(GeoViewport::default(), [720.0, 360.0], [360.0, 165.0])
            .is_none()
    );
}

#[test]
fn identities_values_and_overlay_priority() {
    let f = feature(rectangle(-90.0, -45.0, 90.0, 45.0), vec![]);
    assert_eq!(
        data(vec![f.clone(), f.clone()]).expect_err("duplicate identity must refuse"),
        GeoRefusal::DuplicateIdentity
    );
    let mut invalid = f.clone();
    invalid.value = Some(31.0);
    assert_eq!(
        data(vec![invalid]).expect_err("out-of-domain value must refuse"),
        GeoRefusal::InvalidValue
    );
    let points = vec![GeoPoint {
        id: "point".into(),
        label: "Observation".into(),
        position: position(0.0, 0.0),
    }];
    let data = GeoData::new(GeoProjection::Equirectangular, vec![f], points, domain())
        .expect("valid point overlay");
    assert_eq!(
        data.hit_test(GeoViewport::default(), [720.0, 360.0], [363.0, 184.0])
            .as_deref(),
        Some("point")
    );
    assert_eq!(
        data.hit_test(GeoViewport::default(), [720.0, 360.0], [364.0, 184.0])
            .as_deref(),
        Some("region")
    );
}

#[test]
fn pointer_anchor_camera_roundtrip_and_limits() {
    let view = GeoViewport::default();
    let anchor = GeoProjected { x: 0.7, y: 0.4 };
    let zoomed = view.zoom_at(2.0, anchor);
    close(zoomed.center.x, 0.6);
    close(zoomed.center.y, 0.45);
    for size in [[800.0, 300.0], [200.0, 500.0]] {
        let a = view.screen(anchor, size);
        let b = zoomed.screen(anchor, size);
        close(a[0], b[0]);
        close(a[1], b[1]);
    }
    assert_eq!(zoomed.zoom_at(0.5, anchor), view);
    assert_eq!(view.pan(9.0, -9.0).center, GeoProjected { x: 1.0, y: 0.0 });
    assert_eq!(view.zoom_at(1000.0, anchor).zoom, 64.0);
    assert_eq!(
        GeoViewport {
            zoom: f64::NAN,
            ..view
        }
        .validate(),
        Err(GeoRefusal::InvalidViewport)
    );
}

#[gpui::test]
fn geography_pointer_keyboard_wheel_and_host_refusal(cx: &mut gpui::TestAppContext) {
    let data = Rc::new(
        data(vec![feature(rectangle(-90.0, -45.0, 90.0, 45.0), vec![])]).expect("valid fixture"),
    );
    let events = Rc::new(RefCell::new(Vec::new()));
    let reported = events.clone();
    let mut harness = Harness::new(cx, crate::install, move |_, _| {
        let reported = reported.clone();
        div()
            .w(px(600.0))
            .child(
                GeoMap::new("geo", "Fixture")
                    .state(GeoState::Ready(data.clone()))
                    .on_event(move |event, _, _| reported.borrow_mut().push(event)),
            )
            .into_any_element()
    });
    harness.click("geo.map");
    assert_eq!(
        events.borrow().last(),
        Some(&GeoEvent::Select(Some("region".into())))
    );
    assert!(
        !harness
            .node("geo.feature.region")
            .expect("region readout")
            .selected
    );
    harness.keystrokes("tab");
    harness.keystrokes("right");
    assert!(
        events
            .borrow()
            .iter()
            .any(|e| matches!(e,GeoEvent::Viewport(v) if (v.center.x-0.6).abs()<1e-12))
    );
    harness.scroll("geo.map", 28.0);
    assert!(
        matches!(events.borrow().last(),Some(GeoEvent::Viewport(v)) if (v.center.y-0.6).abs()<1e-12)
    );
    assert_eq!(
        harness
            .node("geo.map")
            .expect("map camera")
            .value
            .as_deref(),
        Some("zoom 1; center 0.5, 0.5")
    );
}

#[gpui::test]
fn geography_stale_and_nonready_semantics(cx: &mut gpui::TestAppContext) {
    let data = Rc::new(
        data(vec![feature(rectangle(-90.0, -45.0, 90.0, 45.0), vec![])])
            .expect("valid stale fixture"),
    );
    let mut harness = Harness::new(cx, crate::install, move |_, _| {
        div()
            .w(px(600.0))
            .child(
                GeoMap::new("stale", "Fixture")
                    .state(GeoState::Stale {
                        data: data.clone(),
                        reason: "Refresh declined".into(),
                    })
                    .selected(Some("region".into())),
            )
            .child(
                GeoMap::new("refused", "Fixture")
                    .state(GeoState::Refused(GeoRefusal::AntimeridianEdge)),
            )
            .child(GeoMap::new("loading", "Fixture"))
            .child(GeoMap::new("empty", "Fixture").state(GeoState::Empty))
            .child(GeoMap::new("error", "Fixture").state(GeoState::Error("Failure".into())))
            .child(
                GeoMap::new("unavailable", "Fixture")
                    .state(GeoState::Unavailable("No local file".into())),
            )
            .into_any_element()
    });
    assert_eq!(
        harness.node("stale").expect("stale state").value.as_deref(),
        Some("stale")
    );
    let region = harness
        .node("stale.feature.region")
        .expect("retained region");
    assert!(region.selected);
    assert_eq!(region.value.as_deref(), Some("0 units"));
    assert!(harness.node("refused.map").is_none());
    for id in ["refused", "loading", "empty", "error", "unavailable"] {
        assert_eq!(
            harness.node(id).expect("declared state").value.as_deref(),
            Some(id)
        );
    }
}

#[gpui::test]
fn geography_accepts_input_and_hole_clicks_clear_selection(cx: &mut gpui::TestAppContext) {
    let data = Rc::new(
        GeoData::new(
            GeoProjection::Equirectangular,
            vec![feature(
                rectangle(-90.0, -45.0, 90.0, 45.0),
                vec![rectangle(-18.0, -18.0, 36.0, 18.0)],
            )],
            vec![GeoPoint {
                id: "sensor".into(),
                label: "Sensor".into(),
                position: position(60.0, 0.0),
            }],
            domain(),
        )
        .expect("valid geometry and sensor fixture"),
    );
    let state = Rc::new(RefCell::new((
        GeoViewport::default(),
        Some(SharedString::from("region")),
    )));
    let owner = state.clone();
    let mut harness = Harness::new(cx, crate::install, move |_, _| {
        let handler = owner.clone();
        let (viewport, selected) = owner.borrow().clone();
        div()
            .w(px(600.0))
            .child(
                GeoMap::new("accepted", "Fixture")
                    .state(GeoState::Ready(data.clone()))
                    .viewport(viewport)
                    .selected(selected)
                    .on_event(move |event, window, _| {
                        match event {
                            GeoEvent::Select(id) => handler.borrow_mut().1 = id,
                            GeoEvent::Viewport(viewport) => handler.borrow_mut().0 = viewport,
                        }
                        window.refresh();
                    }),
            )
            .into_any_element()
    });
    assert!(
        harness
            .node("accepted.feature.region")
            .expect("selected readout")
            .selected
    );
    let map_bounds = harness.bounds("accepted.map").expect("measured map");
    let geometry = harness
        .bounds("accepted.geometry.region")
        .expect("measured polygon");
    close(
        f64::from(f32::from(geometry.left() - map_bounds.left())),
        230.0,
    );
    close(
        f64::from(f32::from(geometry.top() - map_bounds.top())),
        105.0,
    );
    close(f64::from(f32::from(geometry.size.width)), 140.0);
    close(f64::from(f32::from(geometry.size.height)), 70.0);
    let sensor = harness
        .bounds("accepted.geometry.sensor")
        .expect("measured sensor");
    assert!((f32::from(sensor.left() - map_bounds.left()) - 341.66666).abs() <= 0.5);
    assert_eq!(sensor.size, gpui::size(px(10.0), px(10.0)));
    harness.click("accepted.map"); // Center lies in the hole, not its polygon.
    assert!(state.borrow().1.is_none());
    assert!(
        !harness
            .node("accepted.feature.region")
            .expect("cleared readout")
            .selected
    );
    harness.keystrokes("tab ]");
    assert_eq!(state.borrow().1.as_deref(), Some("region"));
    assert!(
        harness
            .node("accepted.feature.region")
            .expect("keyboard selected readout")
            .selected
    );
    harness.keystrokes("= right");
    close(state.borrow().0.zoom, 1.25);
    close(state.borrow().0.center.x, 0.58);
    let geometry = harness
        .bounds("accepted.geometry.region")
        .expect("transformed polygon");
    // GPUI snaps measured layout to device pixels, unlike the f64 camera.
    for (actual, expected) in [
        (geometry.left() - map_bounds.left(), 184.5),
        (geometry.top() - map_bounds.top(), 96.25),
        (geometry.size.width, 175.0),
        (geometry.size.height, 87.5),
    ] {
        assert!((f64::from(f32::from(actual)) - expected).abs() <= 0.5);
    }
    let sensor = harness
        .bounds("accepted.geometry.sensor")
        .expect("transformed sensor");
    assert!((f32::from(sensor.left() - map_bounds.left()) - 325.33334).abs() <= 0.5);
    assert_eq!(sensor.size, gpui::size(px(10.0), px(10.0)));
    harness.keystrokes("home escape");
    assert_eq!(state.borrow().0, GeoViewport::default());
    assert!(state.borrow().1.is_none());

    let bounds = harness.bounds("accepted.map").expect("reset map bounds");
    let pointer = gpui::point(bounds.center().x + px(56.0), bounds.center().y - px(28.0));
    harness.update(|window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::ScrollWheel(gpui::ScrollWheelEvent {
                position: pointer,
                delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(-138.62944))),
                modifiers: gpui::Modifiers {
                    control: true,
                    ..Default::default()
                },
                touch_phase: gpui::TouchPhase::Moved,
            }),
            cx,
        );
    });
    // ln(2)*200 wheel pixels doubles zoom. The off-center anchor stays put.
    let viewport = state.borrow().0;
    assert!((viewport.zoom - 2.0).abs() < 1e-6);
    assert!((viewport.center.x - 0.6).abs() < 1e-6);
    assert!((viewport.center.y - 0.45).abs() < 1e-6);
}

#[test]
fn geography_visible_envelopes_clip_geometry_not_just_bounding_boxes() {
    let prepared = data(vec![feature(
        rectangle(-90.0, -45.0, 90.0, 45.0),
        vec![rectangle(-18.0, -18.0, 36.0, 18.0)],
    )])
    .expect("valid polygon for clipping");
    let viewport = GeoViewport {
        zoom: 64.0,
        ..Default::default()
    };
    assert!(
        prepared.visual_bounds(viewport, [600.0, 280.0]).is_empty(),
        "camera entirely inside hole"
    );
    let viewport = GeoViewport {
        zoom: 4.0,
        center: GeoProjected { x: 0.3, y: 0.5 },
    };
    let bounds = prepared.visual_bounds(viewport, [200.0, 300.0]);
    assert_eq!(bounds.len(), 1);
    for (actual, expected) in bounds[0].1.into_iter().zip([60.0, 50.0, 140.0, 200.0]) {
        close(actual, expected);
    }
    let triangle = data(vec![feature(
        ring(&[(0.0, 0.0), (90.0, 0.0), (90.0, 45.0), (0.0, 0.0)]),
        vec![],
    )])
    .expect("valid triangle for clipping");
    assert!(
        triangle
            .visual_bounds(
                GeoViewport {
                    center: GeoProjected { x: 0.52, y: 0.39 },
                    zoom: 64.0
                },
                [100.0, 100.0]
            )
            .is_empty()
    );
}

#[gpui::test]
fn geography_shared_phase_and_localized_refusal(cx: &mut gpui::TestAppContext) {
    use crate::state::{HasPhase, Phase};
    let stale = GeoState::Stale {
        data: Rc::new(data(vec![]).expect("valid empty geometry")),
        reason: "offline".into(),
    };
    assert_eq!(stale.phase(), Phase::Error);
    assert!(stale.is_stale());
    assert_eq!(stale.reason(), Some("offline"));
    assert_eq!(
        GeoState::Refused(GeoRefusal::InvalidRing).phase(),
        Phase::Unavailable
    );
    let mut harness = Harness::new(
        cx,
        |cx| {
            crate::install(cx);
            cx.set_global(crate::strings::TranslationPack::SimplifiedChinese.strings());
        },
        |_, _| {
            div()
                .w(px(600.0))
                .child(
                    GeoMap::new("translated", "地图")
                        .state(GeoState::Refused(GeoRefusal::AntimeridianEdge)),
                )
                .child(
                    GeoMap::new("custom", "地图")
                        .state(GeoState::Loading)
                        .status_text("读取本地几何数据"),
                )
                .into_any_element()
        },
    );
    assert_eq!(
        harness
            .node("translated.status")
            .expect("localized status")
            .text
            .as_deref(),
        Some("不可用: 请将跨越反子午线的边拆分为局部多边形")
    );
    assert_eq!(
        harness
            .node("translated")
            .expect("localized map state")
            .description
            .as_deref(),
        Some("unavailable")
    );
    assert_eq!(
        harness
            .node("custom.status")
            .expect("caller status")
            .text
            .as_deref(),
        Some("读取本地几何数据")
    );
    assert!(harness.node("custom").expect("loading state").busy);
}
