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
    data: Option<Rc<GeoData>>,
    seam: Option<Rc<GeoData>>,
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
    let data = state
        .borrow_mut()
        .data
        .get_or_insert_with(|| fixture(GeoProjection::Equirectangular))
        .clone();
    let seam = state.borrow_mut().seam.get_or_insert_with(|| Rc::new(GeoData::from_geojson(
        r#"{"type":"Feature","id":"seam-island","properties":{},"geometry":{"type":"Polygon","coordinates":[[[170,-20],[-170,-20],[-170,30],[170,30],[170,-20]],[[175,-5],[-175,-5],[-175,10],[175,10],[175,-5]]]}}"#,
        GeoProjection::WebMercator, GeoWorldPolicy::Auto,
        GeoColorDomain { minimum:0.0, maximum:40.0,minimum_label:"0 samples".into(),maximum_label:"40 samples".into(),missing_label:"Unobserved".into() },
        |_,_| Ok(GeoProperties {label:"Synthetic seam island".into(),value:Some(23.0),formatted_value:"23 samples".into()}),
    ).expect("synthetic seam geometry"))).clone();
    let camera_target = state.clone();
    let value_target = state.clone();
    let geometry_target = state.clone();
    stack(&theme).w(px(900.0))
        .child(caption(&theme,"Original synthetic geography · local geometry, no tiles or real territories"))
        .child(caption(&theme,"Drag / touch to pan · pinch or Ctrl-wheel to zoom · F fits · arrows / +/- / Home · [ and ] select"))
        .child(div().row().gap_token(&theme,Space::Sm)
            .child(Button::new("scene.geography.camera-target").label("Retarget camera").on_click(move |window,_| {
                let mut target=camera_target.borrow_mut();
                target.viewport=if target.viewport.zoom>1.0 {GeoViewport::default()} else {GeoViewport {center:GeoProjected{x:0.4,y:0.48},zoom:1.8}};
                window.refresh();
            }))
            .child(Button::new("scene.geography.value-target").label("Retarget observations").on_click(move |window,_| {
                let mut target=value_target.borrow_mut();
                let source=target.data.as_ref().expect("prepared fixture");
                let mut features=source.features().to_vec();
                let value=if features[0].value==Some(12.0) {34.0} else {12.0};
                features[0].value=Some(value); features[0].formatted_value=format!("{value} samples").into();
                target.data=Some(Rc::new(GeoData::new(source.projection(),features,source.points().to_vec(),source.color_domain().clone()).expect("in-domain fixture observation")));
                window.refresh();
            }))
            .child(Button::new("scene.geography.geometry-target").label("Replace geometry").on_click(move |window,_| {
                let mut target=geometry_target.borrow_mut();
                let source=target.data.as_ref().expect("prepared fixture");
                if source.features().iter().any(|f| f.id.as_ref()=="unobserved") {
                    let mut features=source.features().to_vec();features.retain(|f| f.id.as_ref()!="unobserved");
                    for p in features[0].polygons.iter_mut().flat_map(|p| std::iter::once(&mut p.exterior).chain(&mut p.holes)).flatten() {p.longitude+=40.0;}
                    let mut points=source.points().to_vec(); points.push(GeoPoint{id:"arriving".into(),label:"New overlapping sensor".into(),position:GeoPosition{longitude:-55.0,latitude:12.0}});
                    target.data=Some(Rc::new(GeoData::new(source.projection(),features,points,source.color_domain().clone()).expect("translated valid fixture")));
                } else {target.data=Some(fixture(GeoProjection::Equirectangular));}
                window.refresh();
            })))
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
        .child(GeoMap::new("scene.geography.seam","Local GeoJSON · auto world cut · fitted island with a real hole")
            .viewport(seam.fit_viewport([850.0,280.0],16.0).expect("fit synthetic island"))
            .selected(Some("seam-island".into())).state(GeoState::Ready(seam)))
        .into_any_element()
}

pub(super) fn geography_scale(window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let state = crate::motion::keyed::slot::<FixtureCamera>(
        &"scene.geography.scale-state".into(),
        window.window_handle().window_id(),
        cx,
    );
    if state.borrow().data.is_none() {
        state.borrow_mut().viewport = GeoViewport {
            center: GeoProjected { x: 0.5, y: 0.5 },
            zoom: 8.0,
        };
    }
    let data = state
        .borrow_mut()
        .data
        .get_or_insert_with(|| {
            Rc::new(
                GeoData::new(
                    GeoProjection::Equirectangular,
                    vec![],
                    (0..1000)
                        .map(|i| GeoPoint {
                            id: format!("sample-{i}").into(),
                            label: format!("Synthetic sample {i}").into(),
                            position: GeoPosition {
                                longitude: -150.0 + f64::from(i % 100) * 3.0,
                                latitude: -45.0 + f64::from(i / 100) * 10.0,
                            },
                        })
                        .collect(),
                    GeoColorDomain {
                        minimum: 0.0,
                        maximum: 1.0,
                        minimum_label: "0".into(),
                        maximum_label: "1".into(),
                        missing_label: "Unobserved".into(),
                    },
                )
                .expect("original point grid"),
            )
        })
        .clone();
    let selected = state.borrow().selected.clone();
    let viewport = state.borrow().viewport;
    let dense = state.clone();
    stack(&theme).w(px(900.0))
        .child(caption(&theme,format!("{} original local point overlays · viewport culling · six-row virtual accessible readout",data.points().len())))
        .child(caption(&theme,"Scroll the readout or navigate it by keyboard; offscreen rows retain source identities, not measured bounds"))
        .child(Button::new("scene.geography.scale-density").label("Load 10,000 dense points").on_click(move |window,_| {
            let mut target=dense.borrow_mut();
            let points=(0..10000).map(|i| GeoPoint{id:format!("sample-{i}").into(),label:format!("Synthetic sample {i}").into(),position:GeoPosition{longitude:-179.0+f64::from(i%1000)*0.358,latitude:-80.0+f64::from(i/1000)*1.6}}).collect();
            let data=GeoData::new(GeoProjection::Equirectangular,vec![],points,target.data.as_ref().expect("prepared fixture").color_domain().clone()).expect("original point grid");
            target.viewport=GeoViewport{center:data.projection().project(data.points()[5000].position).expect("prepared fixture"),zoom:1.0};
            target.data=Some(Rc::new(data));
            window.refresh();
        }))
        .child(GeoMap::new("scene.geography.scale","Controlled point cloud").state(GeoState::Ready(data)).viewport(viewport).selected(selected)
            .on_event(move |event,window,_| {match event {GeoEvent::Select(id)=>state.borrow_mut().selected=id,GeoEvent::Viewport(v)=>state.borrow_mut().viewport=v};window.refresh();}))
        .into_any_element()
}
