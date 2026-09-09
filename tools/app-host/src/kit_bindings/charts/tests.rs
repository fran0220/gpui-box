use super::*;
use gpui::{ParentElement, Styled, TestAppContext, div, px};
use gpui_kit_testkit::harness::Harness;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

fn node(component: &str, id: &str, props: Value, events: Value) -> Node {
    let node: Node = serde_json::from_value(
        json!({"kind":"kit","component":component,"id":id,"props":props,"events":events}),
    )
    .expect("fixture node");
    super::super::validation::validate_descriptor(&node).expect("valid chart fixture");
    node
}

#[gpui::test]
fn all_chart_builders_render_caller_owned_native_semantics(cx: &mut TestAppContext) {
    let fixtures: serde_json::Map<String, Value> =
        serde_json::from_str(include_str!("fixture/props.json")).expect("fixtures");
    assert_eq!(fixtures.len(), COMPONENTS.len());
    for dark in [false, true] {
        let mut harness = Harness::new(
            cx,
            move |c| {
                gpui_kit::install(c);
                gpui_kit_theme::activate_theme(
                    if dark { "studio-dark" } else { "studio-light" },
                    c,
                );
            },
            |_, _| div().into_any_element(),
        );
        for (component, props) in &fixtures {
            let n = node(component, "subject", props.clone(), json!({}));
            harness.remount(move |w, c| {
                div()
                    .w(px(640.))
                    .child(render(&n, BTreeMap::new(), w, c, Rc::new(|_, _| {})))
                    .into_any_element()
            });
            assert!(harness.node("subject").is_some(), "{component}");
        }
    }
}

#[gpui::test]
fn chart_events_use_business_identity_and_disabled_legend_is_inert(cx: &mut TestAppContext) {
    let fixtures: Value =
        serde_json::from_str(include_str!("fixture/props.json")).expect("fixtures");
    let mut p = fixtures["ChartLegend"].clone();
    let active = node(
        "ChartLegend",
        "legend",
        p.clone(),
        json!({"toggle":"toggle-west"}),
    );
    p["disabled"] = json!(true);
    let disabled = node("ChartLegend", "disabled", p, json!({}));
    let mut plot_props = fixtures["Plot"].clone();
    plot_props["state"]["data"].as_array_mut().expect("marks").push(json!({"id":"second","label":"B","value":"19","bounds":{"x":0.6,"y":0.1,"width":0.2,"height":0.3}}));
    let plot = node(
        "Plot",
        "plot",
        plot_props,
        json!({"current":"current-mark"}),
    );
    let output = Rc::new(RefCell::new(Vec::new()));
    let captured = output.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |w, c| {
        let captured = captured.clone();
        let emit: Emit = Rc::new(move |a, v| captured.borrow_mut().push((a.to_owned(), v)));
        div()
            .flex()
            .flex_col()
            .w(px(640.))
            .children(
                [&active, &disabled, &plot]
                    .into_iter()
                    .map(|n| render(n, BTreeMap::new(), w, c, emit.clone())),
            )
            .into_any_element()
    });
    harness.click("legend.west");
    harness.click("disabled.west");
    harness.click("plot.plot");
    harness.keystrokes("right");
    assert_eq!(
        *output.borrow(),
        vec![
            (
                "toggle-west".to_owned(),
                json!({"id":"west","hidden":false})
            ),
            ("current-mark".to_owned(), json!("second"))
        ]
    );
}

#[gpui::test]
fn every_chart_query_uses_native_data_algorithms(cx: &mut TestAppContext) {
    let fixtures: Value =
        serde_json::from_str(include_str!("fixture/props.json")).expect("fixtures");
    let spark = node(
        "Sparkline",
        "spark",
        fixtures["Sparkline"].clone(),
        json!({}),
    );
    let sankey_node = node(
        "SankeyChart",
        "sankey",
        fixtures["SankeyChart"].clone(),
        json!({}),
    );
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| div().into_any_element());
    harness.update(|w,c|{
        assert_eq!(invoke(&spark,"published_points",&json!({}),true,w,c).expect("point count"),json!(3));
        let mut args=json!({"data":fixtures["SankeyChart"]["state"]["data"],"weights":[13],"nodeWidth":0.08,"gap":0.1,"alignment":"left"});
        let result=invoke(&sankey_node,"layout",&args,true,w,c).expect("native layout");
        assert!((result["scale"].as_f64().expect("scale")-1./13.).abs()<1e-8);
        assert_eq!(result["nodes"][0]["id"],"west");
        assert_eq!(result["nodes"][1]["id"],"east");
        assert_eq!(result["nodes"][0]["bounds"]["x"],json!(0.));
        assert!(result["nodes"][1]["bounds"]["x"].as_f64().expect("position")>0.9);
        args["weights"]=json!([-1]);assert!(invoke(&sankey_node,"layout",&args,true,w,c).is_err());
        assert!(invoke(&spark,"published_points",&json!({}),false,w,c).is_err());
        for kind in ["loading","empty","error","unavailable"] {
            let state=if matches!(kind,"error"|"unavailable"){json!({"kind":kind,"reason":"Refused"})}else{json!({"kind":kind})};
            let absent=node("Sparkline","absent",json!({"label":"No verified reading","state":state}),json!({}));
            assert!(invoke(&absent,"published_points",&json!({}),true,w,c).is_err(),"{kind} must not become zero points");
        }
    });
}

#[gpui::test]
fn chart_selection_events_follow_native_keyboard_identity(cx: &mut TestAppContext) {
    let fixtures: Value =
        serde_json::from_str(include_str!("fixture/props.json")).expect("fixtures");
    for component in [
        "AreaChart",
        "LineChart",
        "ScatterChart",
        "CandlestickChart",
        "SankeyChart",
    ] {
        let mut props = fixtures[component].clone();
        let expected = match component {
            "CandlestickChart" => {
                props["state"]["data"].as_array_mut().expect("candles").push(json!({"id":"later","x":0.8,"open":0.7,"high":0.9,"low":0.2,"close":0.3,"label":"Later","value":"3"}));
                json!("later")
            }
            "SankeyChart" => json!("node.west"),
            _ => {
                props["current"] = json!({"seriesId":"west","pointId":"mon"});
                props["crosshair"] = json!(true);
                json!({"seriesId":"west","pointId":"tue"})
            }
        };
        let n = node(component, "chart", props, json!({"current":"select"}));
        let output = Rc::new(RefCell::new(Vec::new()));
        let capture = output.clone();
        let mut harness = Harness::new(cx, gpui_kit::install, move |w, c| {
            let capture = capture.clone();
            div()
                .w(px(640.))
                .child(render(
                    &n,
                    BTreeMap::new(),
                    w,
                    c,
                    Rc::new(move |_, v| capture.borrow_mut().push(v)),
                ))
                .into_any_element()
        });
        harness.click("chart.plot");
        output.borrow_mut().clear();
        harness.keystrokes("right");
        assert_eq!(*output.borrow(), vec![expected], "{component}");
    }
}

#[test]
fn caller_series_preserve_identity_values_and_asymmetric_geometry() {
    let data = series(&json!([{ "id":"sales-west", "label":"West", "points":[
        {"id":"tuesday","x":0.2,"y":0.85,"label":"Tue","value":"17.3","weight":0.4},
        {"id":"monday","x":0.9,"y":0.1,"label":"Mon","value":"2.1"}
    ] }]));
    assert_eq!(data[0].id.as_ref(), "sales-west");
    assert_eq!(data[0].points[0].id.as_ref(), "tuesday");
    assert_eq!(data[0].points[0].position, SparklinePoint::new(0.2, 0.85));
    assert_eq!(data[0].points[0].value.as_ref(), "17.3");
    assert_eq!(data[0].points[0].weight, Some(0.4));
    assert_eq!(data[0].points[1].weight, None);
}

#[test]
fn native_chart_boundary_rejects_unknown_and_invalid_caller_geometry() {
    let fixtures: serde_json::Map<String, Value> =
        serde_json::from_str(include_str!("fixture/props.json")).expect("fixtures");
    let mut invalid: Vec<(String, Value)> = fixtures
        .into_iter()
        .map(|(component, mut props)| {
            props["url"] = json!("https://host/chart");
            (component, props)
        })
        .collect();
    invalid.extend([
        ("Plot".into(),json!({"label":"Bad mark","state":{"kind":"ready","data":[{"id":"bad","label":"Bad","value":"3","bounds":{"x":0.8,"y":0.1,"width":0.4,"height":0.2}}]}})),
        ("CandlestickChart".into(),json!({"label":"Bad OHLC","state":{"kind":"ready","data":[{"id":"day","x":0.2,"open":0.3,"close":0.8,"low":0.1,"high":0.5,"label":"Day","value":"8"}]}})),
        ("LineChart".into(),json!({"label":"Stale","state":{"kind":"stale","reason":"Refused"}})),
    ]);
    for (component, props) in invalid {
        let n: Node = serde_json::from_value(
            json!({"kind":"kit","component":component,"id":"bad","props":props}),
        )
        .expect("wire node");
        assert!(
            super::super::validation::validate_descriptor(&n).is_err(),
            "{component}: {props}"
        );
    }
}

#[gpui::test]
fn every_chart_state_slot_mounts_only_the_active_native_factory(cx: &mut TestAppContext) {
    let fixtures: Value =
        serde_json::from_str(include_str!("fixture/props.json")).expect("fixtures");
    for component in [
        "AreaChart",
        "BarChart",
        "GaugeChart",
        "LineChart",
        "PieChart",
        "RadarChart",
        "ScatterChart",
        "Sparkline",
        "StackedBarChart",
    ] {
        for (kind, active) in [
            ("empty", "empty"),
            ("loading", "loading"),
            ("error", "failed"),
        ] {
            let mut props = fixtures[component].clone();
            props["state"] = if kind == "error" {
                json!({"kind":kind,"reason":"Refused"})
            } else {
                json!({"kind":kind})
            };
            let n = node(component, "chart", props, json!({}));
            let mut harness = Harness::new(cx, gpui_kit::install, move |w, c| {
                let slots: KitSlots = ["empty", "loading", "failed"]
                    .into_iter()
                    .map(|name| {
                        (
                            name.to_owned(),
                            Rc::new(move |_: &mut Window, _: &mut App| {
                                assert_eq!(name, active, "inactive native slot");
                                gpui_kit::prelude::Tag::new("caller-slot", "Caller content")
                                    .into_any_element()
                            })
                                as Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
                        )
                    })
                    .collect();
                render(&n, slots, w, c, Rc::new(|_, _| {}))
            });
            assert!(harness.node("caller-slot").is_some(), "{component} {kind}");
        }
    }
}

#[test]
fn refresh_failure_retains_verified_data_and_reason() {
    let state = chart_state(
        &json!({"kind":"stale","reason":"rate limited","data":[{"id":"verified","label":"Revenue","points":[]}]}),
    );
    let ChartState::Stale { series, reason } = state else {
        panic!("lost stale state")
    };
    assert_eq!(reason.as_ref(), "rate limited");
    assert_eq!(series[0].id.as_ref(), "verified");
    assert!(
        matches!(chart_state(&json!({"kind":"unavailable","reason":"refused"})),ChartState::Unavailable(reason) if reason.as_ref()=="refused")
    );
    assert!(matches!(
        chart_state(&json!({"kind":"empty"})),
        ChartState::Empty
    ));
}

#[test]
fn ohlc_and_plot_conversion_do_not_swap_axes_or_values() {
    let c = candles(
        &json!([{"id":"day","x":0.3,"open":0.2,"high":0.9,"low":0.1,"close":0.7,"label":"Tuesday","value":"seven"}]),
    );
    assert!(c[0].is_bounded());
    assert_eq!(
        (c[0].open, c[0].high, c[0].low, c[0].close),
        (0.2, 0.9, 0.1, 0.7)
    );
    let m = marks(
        &json!([{"id":"mark","label":"A","value":"12","bounds":{"x":0.1,"y":0.4,"width":0.2,"height":0.5}}]),
    );
    assert!(m[0].is_bounded());
    assert_eq!(m[0].bounds, Bounds::new(point(0.1, 0.4), size(0.2, 0.5)));
}
