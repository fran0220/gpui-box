//! Original synthetic local geography, not a depiction of any real territory.
use super::support::*;
use crate::display::geography::*;

fn ring(points: &[(f64, f64)]) -> Vec<GeoPosition> {
    points
        .iter()
        .map(|&(longitude, latitude)| GeoPosition {
            longitude,
            latitude,
        })
        .collect()
}

fn fixture(projection: GeoProjection) -> Rc<GeoData> {
    let features = vec![
        GeoFeature {
            id: "lagoon".into(),
            label: "Lagoon region".into(),
            value: Some(12.0),
            formatted_value: "12 samples".into(),
            polygons: vec![GeoPolygon {
                exterior: ring(&[
                    (-110.0, -28.0),
                    (-28.0, -38.0),
                    (4.0, 2.0),
                    (-24.0, 58.0),
                    (-95.0, 48.0),
                    (-110.0, -28.0),
                ]),
                holes: vec![ring(&[
                    (-76.0, -3.0),
                    (-44.0, -3.0),
                    (-40.0, 24.0),
                    (-71.0, 28.0),
                    (-76.0, -3.0),
                ])],
            }],
        },
        GeoFeature {
            id: "ridge".into(),
            label: "Ridge".into(),
            value: Some(31.0),
            formatted_value: "31 samples".into(),
            polygons: vec![GeoPolygon {
                exterior: ring(&[
                    (20.0, -45.0),
                    (110.0, -23.0),
                    (82.0, 42.0),
                    (40.0, 55.0),
                    (12.0, 8.0),
                    (20.0, -45.0),
                ]),
                holes: vec![],
            }],
        },
        GeoFeature {
            id: "unobserved".into(),
            label: "Outer island".into(),
            value: None,
            formatted_value: "".into(),
            polygons: vec![GeoPolygon {
                exterior: ring(&[
                    (117.0, 32.0),
                    (154.0, 44.0),
                    (140.0, 65.0),
                    (115.0, 59.0),
                    (117.0, 32.0),
                ]),
                holes: vec![],
            }],
        },
    ];
    Rc::new(
        GeoData::new(
            projection,
            features,
            vec![
                GeoPoint {
                    id: "station".into(),
                    label: "Station A".into(),
                    position: GeoPosition {
                        longitude: 50.0,
                        latitude: 12.0,
                    },
                },
                GeoPoint {
                    id: "lagoon-sensor".into(),
                    label: "Lagoon sensor".into(),
                    position: GeoPosition {
                        longitude: -58.0,
                        latitude: 12.0,
                    },
                },
            ],
            GeoColorDomain {
                minimum: 0.0,
                maximum: 40.0,
                minimum_label: "0 samples".into(),
                maximum_label: "40 samples".into(),
                missing_label: "Unobserved".into(),
            },
        )
        .expect("original synthetic geometry is valid"),
    )
}

#[derive(Default)]
struct FixtureCamera {
    viewport: GeoViewport,
    selected: Option<SharedString>,
}

pub(super) fn geography(window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let state = crate::motion::keyed::slot::<FixtureCamera>(
        &"scene.geography.camera".into(),
        window.window_handle().window_id(),
        cx,
    );
    let viewport = state.borrow().viewport;
    let selected = state.borrow().selected.clone();
    let data = fixture(GeoProjection::Equirectangular);
    stack(&theme).w(px(900.0))
        .child(caption(&theme,"Original synthetic geography · local geometry, no tiles or real territories"))
        .child(caption(&theme,"Click geometry or labels · wheel pans · Ctrl-wheel zooms · arrows / +/- / Home · [ and ] select"))
        .child(div().row().items_start().gap_token(&theme,Space::Md)
            .child(div().w(px(500.0)).child(GeoMap::new("scene.geography.ready","Equirectangular · controlled camera")
                .state(GeoState::Ready(data)).viewport(viewport).selected(selected)
                .on_event(move |event,window,_| {
                    match event { GeoEvent::Select(id) => state.borrow_mut().selected=id, GeoEvent::Viewport(viewport) => state.borrow_mut().viewport=viewport }
                    window.refresh();
                })))
            .child(div().w(px(320.0)).child(GeoMap::new("scene.geography.zoomed","Web Mercator · narrow, zoomed, selected")
                .state(GeoState::Stale { data:fixture(GeoProjection::WebMercator), reason:"Refresh refused; verified geometry retained".into() })
                .viewport(GeoViewport { center:GeoProjected{x:0.38,y:0.46},zoom:2.2 }).selected(Some("lagoon".into())))))
        .child(div().row().items_start().gap_token(&theme,Space::Md)
            .child(div().w(px(280.0)).child(GeoMap::new("scene.geography.loading","Loading local features")))
            .child(div().w(px(280.0)).child(GeoMap::new("scene.geography.empty","Valid empty collection").state(GeoState::Empty)))
            .child(div().w(px(280.0)).child(GeoMap::new("scene.geography.refused","Unsupported input").state(GeoState::Refused(GeoRefusal::AntimeridianEdge)))))
        .into_any_element()
}
