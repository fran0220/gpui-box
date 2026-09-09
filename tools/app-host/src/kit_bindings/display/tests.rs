use super::*;
use gpui::{AppContext, Styled, TestAppContext, px};
use gpui_kit_testkit::harness::Harness;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

fn node(component: &str, id: &str, props: Value, events: Value) -> Node {
    let node: Node = serde_json::from_value(
        json!({"kind":"kit","component":component,"id":id,"props":props,"events":events}),
    )
    .expect("fixture descriptor");
    super::super::validation::validate_descriptor(&node).expect("valid display fixture");
    node
}

#[gpui::test]
fn every_display_builder_publishes_native_semantics_in_both_themes(cx: &mut TestAppContext) {
    let fixtures: serde_json::Map<String, Value> =
        serde_json::from_str(include_str!("fixture/props.json")).expect("fixtures");
    assert_eq!(fixtures.len(), COMPONENTS.len());
    for dark in [false, true] {
        let mut harness = Harness::new(
            cx,
            move |cx| {
                gpui_kit::install(cx);
                gpui_kit_theme::activate_theme(
                    if dark { "studio-dark" } else { "studio-light" },
                    cx,
                );
            },
            |_, _| div().into_any_element(),
        );
        for (component, props) in &fixtures {
            let node = node(component, "subject", props.clone(), json!({}));
            harness.remount(move |w, c| {
                div()
                    .w(px(640.))
                    .child(render(&node, BTreeMap::new(), w, c, Rc::new(|_, _| {})))
                    .into_any_element()
            });
            if component == "StatusDot" {
                assert!(
                    harness.node("subject").is_none(),
                    "native status dot is decorative"
                );
            } else {
                assert!(
                    harness.node("subject").is_some(),
                    "{component} must publish native identity"
                );
            }
        }
    }
}

#[gpui::test]
fn display_actions_use_latest_handlers_and_disabled_controls_do_not_dispatch(
    cx: &mut TestAppContext,
) {
    let captured = Rc::new(RefCell::new(Vec::new()));
    let output = captured.clone();
    let nodes = [
        node(
            "Tag",
            "tag",
            json!({"label":"West"}),
            json!({"remove":"remove-west"}),
        ),
        node(
            "Tag",
            "disabled",
            json!({"label":"Disabled","disabled":true}),
            json!({}),
        ),
        node(
            "FailurePanel",
            "failed",
            json!({"reason":"Offline"}),
            json!({"retry":"retry-network"}),
        ),
        node(
            "DescriptionList",
            "details",
            json!({"items":[{"id":"region","term":"Region","value":{"kind":"text","text":"West"},"copyable":true}]}),
            json!({"copy":"copy-region"}),
        ),
    ];
    let mut harness = Harness::new(cx, gpui_kit::install, move |w, c| {
        let output = output.clone();
        let emit: Emit = Rc::new(move |a, v| output.borrow_mut().push((a.to_owned(), v)));
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .children(
                nodes
                    .iter()
                    .map(|n| render(n, BTreeMap::new(), w, c, emit.clone())),
            )
            .into_any_element()
    });
    harness.click("tag.remove");
    harness.click("failed.retry");
    harness.click("details.region.copy");
    assert_eq!(
        *captured.borrow(),
        vec![
            ("remove-west".to_owned(), Value::Null),
            ("retry-network".to_owned(), Value::Null),
            ("copy-region".to_owned(), json!("region"))
        ]
    );
    assert!(harness.node("disabled.remove").is_none());
    harness.remount(|_, _| div().into_any_element());
    assert!(harness.node("tag").is_none());
    assert_eq!(captured.borrow().len(), 3);
}

#[gpui::test]
fn every_display_query_calls_native_geometry_or_hit_filtering(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| div().into_any_element());
    let glyph = node(
        "Icon",
        "glyph",
        json!({"glyph":{"key":"arrow-left"},"tone":"danger","size":"lg"}),
        json!({}),
    );
    let hits = node(
        "HighlightedText",
        "hits",
        json!({"text":"aé中z","hits":[{"start":1,"end":3},{"start":2,"end":3},{"start":3,"end":6},{"start":6,"end":99}]}),
        json!({}),
    );
    harness.update(|w, c| {
        assert_eq!(
            invoke(&hits, "published_hits", &json!({}), true, w, c).expect("native hits"),
            json!(2)
        );
        assert_eq!(
            invoke(&glyph, "flips_in", &json!({"direction":"rtl"}), true, w, c).expect("rtl"),
            json!(true)
        );
        assert_eq!(
            invoke(&glyph, "flips_in", &json!({"direction":"ltr"}), true, w, c).expect("ltr"),
            json!(false)
        );
        let actual = invoke(&glyph, "resolved_color", &json!({}), true, w, c).expect("color");
        let expected = c.theme().colors.danger;
        assert_eq!(
            actual,
            json!({"h":expected.h,"s":expected.s,"l":expected.l,"a":expected.a})
        );
        assert!(
            invoke(&glyph, "resolved_size", &json!({}), true, w, c)
                .expect("size")
                .as_f64()
                .expect("number")
                > 0.
        );
        assert!(invoke(&glyph, "resolved_size", &json!({}), false, w, c).is_err());
    });
}

#[gpui::test]
fn remaining_display_actions_emit_native_values(cx: &mut TestAppContext) {
    let fixtures: Value =
        serde_json::from_str(include_str!("fixture/props.json")).expect("fixtures");
    for (component, event, target, expected) in [
        ("Banner", "dismiss", "subject.dismiss", Value::Null),
        ("Card", "click", "subject", Value::Null),
        ("ListRow", "click", "subject", Value::Null),
        ("LoadMore", "more", "subject.more", Value::Null),
        ("ProgressBar", "cancel", "subject.cancel", Value::Null),
        ("ProgressCircle", "cancel", "subject.cancel", Value::Null),
        ("Rating", "change", "subject.value-3", json!(3.)),
        (
            "PerformanceHud",
            "expanded",
            "subject.expanded",
            json!(false),
        ),
        ("TraceView", "select", "subject.child", json!("child")),
        ("SpanTimeline", "select", "subject.send", json!("send")),
    ] {
        let mut props = fixtures[component].clone();
        if component == "Rating" {
            props["precision"] = json!("whole");
        }
        let n = node(component, "subject", props, json!({event:"action"}));
        let output = Rc::new(RefCell::new(Vec::new()));
        let capture = output.clone();
        let mut harness = Harness::new(cx, gpui_kit::install, move |w, c| {
            let capture = capture.clone();
            div()
                .w(px(640.))
                .min_h(px(80.))
                .child(render(
                    &n,
                    BTreeMap::from([(
                        "content".into(),
                        Rc::new(|_: &mut Window, _: &mut App| {
                            div().h(px(50.)).child("Caller content").into_any_element()
                        })
                            as Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
                    )]),
                    w,
                    c,
                    Rc::new(move |_, v| capture.borrow_mut().push(v)),
                ))
                .into_any_element()
        });
        harness.click(target);
        assert_eq!(*output.borrow(), vec![expected], "{component}.{event}");
    }
}

#[test]
fn tone_and_variant_are_distinct_native_values() {
    assert_eq!(tone("warning"), Tone::Warning);
    assert_eq!(tone("danger"), Tone::Danger);
    assert_eq!(variant("light"), Variant::Light);
    assert_eq!(variant("transparent"), Variant::Transparent);
}

#[test]
fn native_boundary_rejects_unknown_fields_and_conditional_payloads() {
    let fixtures: serde_json::Map<String, Value> =
        serde_json::from_str(include_str!("fixture/props.json")).expect("fixtures");
    let mut invalid: Vec<(String, Value)> = fixtures
        .into_iter()
        .map(|(component, mut props)| {
            props["unknown"] = json!(true);
            (component, props)
        })
        .collect();
    invalid.extend([
        ("Avatar".into(),json!({"name":"Ada","image":{"key":"https://host/image"}})),
        ("FailurePanel".into(),json!({"result":{"ok":false}})),
        ("AnimatedNumber".into(),json!({"value":1,"spec":{"durationMs":12}})),
        ("Heatmap".into(),json!({"label":"Heat","state":{"kind":"error"}})),
        ("Timeline".into(),json!({"entries":[{"id":"shared","description":"A"}],"groups":[{"id":"today","label":"Today","entries":[{"id":"shared","description":"B"}]}]})),
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
fn native_slots_are_fresh_lazy_and_released_with_the_builder(cx: &mut TestAppContext) {
    let count = Rc::new(std::cell::Cell::new(0));
    let weak = Rc::downgrade(&count);
    let captured = count.clone();
    let n = node(
        "MetricCard",
        "metric",
        json!({"label":"Caller metric","state":{"kind":"error","reason":"Refused"}}),
        json!({}),
    );
    let mut harness = Harness::new(cx, gpui_kit::install, move |w, c| {
        let captured = captured.clone();
        let slots: KitSlots = BTreeMap::from([
            (
                "failed".into(),
                Rc::new(move |_: &mut Window, _: &mut App| {
                    captured.set(captured.get() + 1);
                    div().child("Caller failure details").into_any_element()
                }) as Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
            ),
            (
                "loading".into(),
                Rc::new(|_: &mut Window, _: &mut App| -> AnyElement {
                    panic!("inactive slot must remain lazy")
                }) as Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
            ),
        ]);
        render(&n, slots, w, c, Rc::new(|_, _| {}))
    });
    let first = count.get();
    assert!(first > 0);
    harness.update(|w, _| w.refresh());
    assert!(count.get() > first, "render must invoke a fresh factory");
    drop(count);
    assert!(weak.upgrade().is_some());
    harness.remount(|_, _| div().into_any_element());
    assert!(
        weak.upgrade().is_none(),
        "removed builder must release slot capture"
    );
}

#[gpui::test]
fn native_alternate_constructors_preserve_success_and_failure(cx: &mut TestAppContext) {
    let success = node(
        "FailurePanel",
        "success",
        json!({"result":{"ok":true}}),
        json!({}),
    );
    let failed = node(
        "FailurePanel",
        "failed",
        json!({"result":{"ok":false,"error":"Rejected"}}),
        json!({}),
    );
    let mut harness = Harness::new(cx, gpui_kit::install, move |w, c| {
        div()
            .children(
                [&success, &failed]
                    .into_iter()
                    .map(|n| render(n, BTreeMap::new(), w, c, Rc::new(|_, _| {}))),
            )
            .into_any_element()
    });
    assert!(harness.node("success").is_none());
    assert!(harness.node("failed").is_some());
    let ready = node(
        "StateView",
        "ready",
        json!({"state":{"kind":"ready"},"fromAsync":true}),
        json!({}),
    );
    harness.remount(move |w, c| {
        render(
            &ready,
            BTreeMap::from([(
                "content".into(),
                Rc::new(|_: &mut Window, _: &mut App| {
                    gpui_kit::prelude::Tag::new("ready-child", "Verified").into_any_element()
                }) as Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
            )]),
            w,
            c,
            Rc::new(|_, _| {}),
        )
    });
    assert!(harness.node("ready-child").is_some());
}

struct AvatarReview {
    source: gpui::ImageSource,
    owner: gpui::EffectOwner,
}
impl gpui::Render for AvatarReview {
    fn render(&mut self, w: &mut Window, c: &mut gpui::Context<Self>) -> impl IntoElement {
        gpui_kit_semantics::SemanticCoordinator::global(c).begin_frame(w);
        div()
            .size_full()
            .bg(c.theme().colors.canvas)
            .p(px(20.))
            .child(gpui::effect_owner(
                self.owner,
                Avatar::new("Ada")
                    .id("avatar")
                    .size(64.)
                    .image_source(self.source.clone()),
            ))
    }
}

#[test]
fn revoked_lazy_avatar_source_is_visibly_unavailable_not_initials() {
    use crate::resources::{Registration, Resources};
    use base64::Engine as _;
    use gpui_kit_semantics::SemanticCoordinator;
    for theme in ["studio-light", "studio-dark"] {
        let mut cx = gpui::HeadlessAppContext::with_platform(
            gpui_platform::test_text_system("Geist"),
            std::sync::Arc::new(gpui_kit::assets::Assets),
            gpui_platform::current_headless_renderer,
        );
        let owner = gpui::EffectOwner::new();
        let (mut store, source) = cx.update(|c| {
            gpui_kit::install(c);
            gpui_kit_theme::activate_theme(theme, c);
            let mut store = Resources::install(c);
            store.reconcile(&std::collections::HashMap::from([(owner, owner)]), c);
            let reference = store
                .register(
                    owner,
                    Registration {
                        key: "avatar".into(),
                        mime: "image/x.gpui-rgba8".into(),
                        data: base64::engine::general_purpose::STANDARD.encode(
                            [
                                8u32.to_le_bytes().as_slice(),
                                8u32.to_le_bytes().as_slice(),
                                &[255, 0, 0, 255].repeat(64),
                            ]
                            .concat(),
                        ),
                    },
                )
                .expect("register avatar");
            let source = c.with_effect_owner(Some(owner), |c| {
                Resources::image(&reference, c).expect("resolve avatar")
            });
            (store, source)
        });
        let _armed = cx.update(|c| SemanticCoordinator::global(c).arm());
        let handle = cx
            .open_window(gpui::size(px(110.), px(110.)), |_, c| {
                c.new(|_| AvatarReview { source, owner })
            })
            .expect("avatar window");
        let window = handle.into();
        cx.run_until_parked();
        cx.update_window(window, |_, w, c| w.draw(c).clear(c))
            .expect("ready draw");
        let ready = cx.capture_screenshot(window).expect("ready pixels");
        assert_eq!(
            ready.get_pixel(ready.width() / 2, ready.height() / 2).0,
            [255, 0, 0, 255]
        );
        cx.update_window(window, |_, _, c| store.revoke(owner, c))
            .expect("revoke");
        cx.update(|c| handle.update(c, |_, _, c| c.notify()).expect("redraw"));
        cx.run_until_parked();
        cx.update_window(window, |_, w, c| w.draw(c).clear(c))
            .expect("refusal draw");
        let refused = cx.capture_screenshot(window).expect("refused pixels");
        assert_ne!(ready.as_raw(), refused.as_raw());
        cx.update(|c| {
            let snapshot = SemanticCoordinator::global(c)
                .snapshot(window.window_id())
                .expect("snapshot");
            let snapshot = serde_json::to_value(snapshot).expect("snapshot data");
            assert!(
                snapshot["nodes"]
                    .as_array()
                    .expect("nodes")
                    .iter()
                    .any(|n| n["id"] == "avatar.image-unavailable"),
                "{snapshot}"
            );
        });
        if let Ok(directory) = std::env::var("GPUI_FAMILY_CAPTURE_DIR") {
            refused
                .save(std::path::Path::new(&directory).join(format!("avatar-refused-{theme}.png")))
                .expect("refusal artifact");
        }
    }
}

struct Review(Vec<Node>);
impl gpui::Render for Review {
    fn render(&mut self, w: &mut Window, c: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = c.theme();
        div()
            .size_full()
            .bg(theme.colors.canvas)
            .text_color(theme.colors.text)
            .flex()
            .flex_wrap()
            .gap(px(16.))
            .p(px(20.))
            .children(self.0.iter().map(|n| {
                let child = if super::super::charts::COMPONENTS
                    .contains(&n.component.as_deref().unwrap_or_default())
                {
                    super::super::charts::render(n, BTreeMap::new(), w, c, Rc::new(|_, _| {}))
                } else {
                    render(n, BTreeMap::new(), w, c, Rc::new(|_, _| {}))
                };
                div()
                    .w(px(330.))
                    .h(px(280.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(n.component.clone().unwrap_or_default())
                    .child(child)
            }))
    }
}

/// Opt-in actual renderer captures; output directory is supplied by the reviewer.
#[test]
fn native_review_captures() {
    let Ok(directory) = std::env::var("GPUI_FAMILY_CAPTURE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&directory).expect("capture directory");
    let display: Value =
        serde_json::from_str(include_str!("fixture/props.json")).expect("display fixtures");
    let charts: Value =
        serde_json::from_str(include_str!("../charts/fixture/props.json")).expect("chart fixtures");
    for theme in ["studio-light", "studio-dark"] {
        for (family, fixtures, names) in [
            (
                "display",
                &display,
                vec![
                    "AvatarGroup",
                    "Badge",
                    "MetricCard",
                    "FailurePanel",
                    "Rating",
                    "ProgressBar",
                    "DescriptionList",
                    "StateView",
                    "Timeline",
                ],
            ),
            (
                "charts",
                &charts,
                vec![
                    "AreaChart",
                    "CandlestickChart",
                    "Plot",
                    "SankeyChart",
                    "Sparkline",
                    "ChartLegend",
                    "LineChart",
                    "PieChart",
                    "ScatterChart",
                ],
            ),
        ] {
            let nodes = names
                .into_iter()
                .enumerate()
                .map(|(i, name)| {
                    let mut props = fixtures[name].clone();
                    if name == "Rating" || name == "ChartLegend" {
                        props["disabled"] = json!(true);
                    }
                    if name == "LineChart" {
                        props["state"] = json!({"kind":"error","reason":"Caller refused refresh"});
                    }
                    node(name, &format!("review-{i}"), props, json!({}))
                })
                .collect();
            let mut cx = gpui::HeadlessAppContext::with_platform(
                gpui_platform::test_text_system("Geist"),
                std::sync::Arc::new(gpui_kit::assets::Assets),
                gpui_platform::current_headless_renderer,
            );
            cx.update(|c| {
                gpui_kit::install(c);
                gpui_kit_theme::activate_theme(theme, c);
                c.set_reduce_motion(true);
            });
            let handle = cx
                .open_window(gpui::size(px(1100.), px(940.)), |_, c| {
                    c.new(|_| Review(nodes))
                })
                .expect("offscreen window");
            let window = handle.into();
            for _ in 0..3 {
                cx.run_until_parked();
                cx.update_window(window, |_, w, c| w.draw(c).clear(c))
                    .expect("draw");
            }
            cx.capture_screenshot(window)
                .expect("pixels")
                .save(std::path::Path::new(&directory).join(format!("{family}-{theme}.png")))
                .expect("save capture");
        }
    }
}
