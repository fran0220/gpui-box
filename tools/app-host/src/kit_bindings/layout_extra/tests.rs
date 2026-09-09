// These tests deliberately panic at the operation that violates a fixture's contract.
#![allow(clippy::unwrap_used)]
use super::*;
#[cfg(feature = "capture")]
use gpui::{ParentElement, Styled, px};
#[test]
fn native_topology_restoration_preserves_asymmetric_geometry_and_floating_order() {
    let v = json!({"records":[{"id":"root","kind":"horizontal","ratio":0.27},{"id":"left","kind":"stack","parent":"root","panels":["a"]},{"id":"right","kind":"stack","parent":"root","panels":["b","c"],"active":"c"}],"floating":[{"stack":{"id":"float","kind":"stack","panels":["d"]},"bounds":{"x":0.13,"y":0.29,"width":0.4,"height":0.6}}]});
    let (tree, tiles) = dock_topology(&v).unwrap();
    assert_eq!(tree.to_records()[0].ratio, 0.27);
    assert_eq!(
        tree.find_stack("right").unwrap().active_panel().unwrap(),
        "c"
    );
    assert_eq!(tiles[0].bounds().origin, point(0.13, 0.29));
    let mut invalid = v.clone();
    invalid["floating"][0]["stack"]["panels"] = json!(["c"]);
    assert!(dock_topology(&invalid).is_err());
    invalid = v;
    invalid["floating"][0]["bounds"]["height"] = json!(0.8);
    assert!(dock_topology(&invalid).is_err());
}
#[test]
fn floating_events_keep_live_finished_and_cancelled_distinct() {
    let event = |finished| {
        dock_event(DockTreeEvent::FloatingChanged {
            stack: "float".into(),
            bounds: Bounds::new(point(0.1, 0.2), extent(0.3, 0.4)),
            finished,
        })
    };
    assert_eq!(event(false)["finished"], false);
    assert_eq!(event(true)["finished"], true);
    assert_eq!(event(false)["bounds"]["width"], json!(0.3_f32));
    let cancelled = dock_event(DockTreeEvent::FloatingCancelled {
        stack: "float".into(),
    });
    assert_eq!(
        cancelled,
        json!({"kind":"floatingCancelled","stack":"float"})
    );
}

#[cfg(feature = "capture")]
fn descriptor(component: &str, props: Value) -> Node {
    serde_json::from_value(json!({"kind":"kit","id":"layout","component":component,"props":props,"events":{"event":"event","click":"click","overflowSelect":"select"}})).unwrap()
}

#[cfg(feature = "capture")]
#[gpui::test]
fn all_layout_builders_render_fresh_slots_and_toolbar_reopens(cx: &mut gpui::TestAppContext) {
    use gpui_kit_testkit::harness::Harness;
    let cases = [
        ("AspectRatio", json!({"ratio":2.75,"fit":"height"})),
        (
            "Container",
            json!({"width":"custom","customWidth":317,"padding":"lg"}),
        ),
        (
            "Grid",
            json!({"columns":3,"items":[{"id":"a","span":2},{"id":"b"}]}),
        ),
        ("Responsive", json!({"threshold":300,"fill":true})),
        ("ScrollFade", json!({"top":false,"right":true})),
        ("ScrollEdgeEffect", json!({"kind":"hard","blur":7})),
        (
            "SplitTree",
            json!({"records":[{"id":"root","kind":"horizontal","ratio":0.27},{"id":"a","kind":"pane","parent":"root"},{"id":"b","kind":"pane","parent":"root"}]}),
        ),
        (
            "Dock",
            json!({"panels":[{"id":"a","title":"Alpha","region":"centre"},{"id":"b","title":"Beta","region":"centre"}],"regions":[{"id":"centre","active":"b"}]}),
        ),
        (
            "DesktopTitlebar",
            json!({"title":"Caller window","subtitle":"Native adapter","buttons":{"right":["close"]}}),
        ),
        (
            "StatusBar",
            json!({"items":[{"id":"a","label":"Ready","kind":"action"},{"id":"b","label":"Sync","kind":"progress","fraction":0.37}]}),
        ),
    ];
    for (component, props) in cases {
        let n = descriptor(component, props);
        validate(&n).unwrap();
        let state = State::default();
        let mut h = Harness::new(cx, gpui_kit::install, move |w, cx| {
            let mut slots = KitSlots::new();
            for key in ["a", "b", "content", "wide", "narrow", "unmeasured"] {
                slots.insert(
                    key.into(),
                    Rc::new(move |_: &mut Window, _: &mut App| {
                        Button::new(key).label(key).into_any_element()
                    }) as Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
                );
            }
            div()
                .w(px(800.))
                .h(px(600.))
                .child(state.render(&n, slots, w, cx, Rc::new(|_, _| {})))
                .into_any_element()
        });
        h.frame();
        assert!(
            h.node(if component == "Responsive" {
                "wide"
            } else {
                "layout"
            })
            .is_some(),
            "{component} native semantic root"
        );
    }
    let n = descriptor(
        "Toolbar",
        json!({"groups":[{"id":"g"}],"items":[{"id":"a","label":"Alpha","group":"g"},{"id":"b","label":"Beta","group":"g"}],"overflow":true,"overflowAfter":0}),
    );
    let state = Rc::new(State::default());
    let build = state.clone();
    let mut h = Harness::new(cx, gpui_kit::install, move |w, cx| {
        build.render(&n, KitSlots::new(), w, cx, Rc::new(|_, _| {}))
    });
    h.click("layout.overflow-menu.trigger");
    let entry = state.overflow.borrow()[&(0, "layout".into())].clone();
    h.update(|_, cx| assert!(entry.menu.read(cx).is_open()));
    h.keystrokes("escape");
    h.click("layout.overflow-menu.trigger");
    h.update(|_, cx| assert!(entry.menu.read(cx).is_open()));
    h.frame();
    assert!(Rc::ptr_eq(
        &entry,
        &state.overflow.borrow()[&(0, "layout".into())]
    ));
    let empty: Node = serde_json::from_value(json!({"kind":"column","id":"empty"})).unwrap();
    h.update(|_, cx| state.reconcile(&empty, cx));
    assert!(state.overflow.borrow().is_empty());
}

#[cfg(feature = "capture")]
#[gpui::test]
fn native_layout_actions_queries_and_floating_gestures(cx: &mut gpui::TestAppContext) {
    use gpui_kit_testkit::harness::Harness;
    let n = descriptor(
        "DockTree",
        json!({"records":[{"id":"main","kind":"stack","panels":["a","b"],"active":"b"}],"panels":[{"id":"a","title":"Alpha"},{"id":"b","title":"Beta"},{"id":"c","title":"Floating"}],"floating":[{"stack":{"id":"float","kind":"stack","panels":["c"]},"bounds":{"x":0.55,"y":0.3,"width":0.4,"height":0.5}}]}),
    );
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let state = State::default();
    let mut h = Harness::new(cx, gpui_kit::install, move |w, cx| {
        let output = output.clone();
        div()
            .w(px(800.))
            .h(px(600.))
            .child(state.render(
                &n,
                KitSlots::new(),
                w,
                cx,
                Rc::new(move |_, v| output.borrow_mut().push(v)),
            ))
            .into_any_element()
    });
    h.click("layout.main.tabs.a");
    assert!(
        events
            .borrow()
            .contains(&json!({"kind":"panelSelected","stack":"main","panel":"a"}))
    );
    let handle = "layout.floating.float.move";
    let start = h.bounds(handle).unwrap().center();
    h.drag_start(handle);
    h.drag_to(start + point(px(37.), px(19.)));
    h.drop_here();
    assert!(
        events
            .borrow()
            .iter()
            .any(|v| v["kind"] == "floatingChanged" && v["finished"] == false)
    );
    assert!(
        events
            .borrow()
            .iter()
            .any(|v| v["kind"] == "floatingChanged" && v["finished"] == true)
    );
    h.drag_start(handle);
    h.drag_to(start + point(px(23.), px(11.)));
    h.cancel_drag();
    assert!(
        events
            .borrow()
            .contains(&json!({"kind":"floatingCancelled","stack":"float"}))
    );
    let query = State::default();
    h.update(|w,cx| {
        assert_eq!(query.invoke(&descriptor("AspectRatio",json!({"ratio":2.75})),"ratio",&json!({}),true,w,cx).unwrap(),json!(2.75));
        assert_eq!(query.invoke(&descriptor("Toolbar",json!({"groups":[{"id":"g"}],"items":[{"id":"a","label":"A","group":"g"},{"id":"b","label":"B","group":"g"}]})),"item_count",&json!({}),true,w,cx).unwrap(),json!(2));
    });
}

#[cfg(feature = "capture")]
#[test]
#[ignore = "explicit offscreen visual review; writes GPUI_FAMILY_CAPTURE directory"]
fn capture_native_family_review() {
    use gpui::{Context, HeadlessAppContext, ParentElement, Render, Styled};
    use std::sync::Arc;
    struct Review {
        node: Node,
        layout: State,
        navigation: super::super::navigation_extra::State,
        dates: super::super::datetime::State,
    }
    impl Render for Review {
        fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let send: Emit = Rc::new(|_, _| {});
            let mut slots = KitSlots::new();
            for key in ["a", "b", "c"] {
                slots.insert(
                    key.into(),
                    Rc::new(move |_: &mut Window, _: &mut App| {
                        div()
                            .p_4()
                            .child(format!("Caller-owned panel {key}"))
                            .into_any_element()
                    }) as Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
                );
            }
            let content = match self.node.component.as_deref().unwrap() {
                "Calendar" => self.dates.render(&self.node, slots, w, cx, send),
                "Sidebar" => self.navigation.render(&self.node, slots, w, cx, send),
                _ => self.layout.render(&self.node, slots, w, cx, send),
            };
            div()
                .size_full()
                .p_4()
                .bg(gpui::rgb(0xf5f5f5))
                .child(content)
        }
    }
    let path = std::env::var("GPUI_FAMILY_CAPTURE").expect("set review output directory");
    std::fs::create_dir_all(&path).unwrap();
    let cases = [
        (
            "dock",
            descriptor(
                "DockTree",
                json!({"records":[{"id":"main","kind":"stack","panels":["a","b"],"active":"b"}],"panels":[{"id":"a","title":"Alpha"},{"id":"b","title":"Beta — selected"},{"id":"c","title":"Floating review"}],"floating":[{"stack":{"id":"float","kind":"stack","panels":["c"]},"bounds":{"x":0.55,"y":0.3,"width":0.4,"height":0.5}}]}),
            ),
        ),
        (
            "sidebar",
            descriptor(
                "Sidebar",
                json!({"sections":[{"id":"files","title":"Workspace"}],"items":[{"id":"a","label":"Projects","section":"files"},{"id":"b","label":"Native adapters","section":"files","within":"a"},{"id":"c","label":"Unavailable archive","section":"files","disabled":true}],"active":"b"}),
            ),
        ),
        (
            "calendar",
            descriptor(
                "Calendar",
                json!({"adapter":serde_json::from_str::<Value>(include_str!("../datetime/fixture/data.json")).unwrap(),"selected":[31,11],"multi":true,"month":4,"marks":[{"day":31,"label":"Review","tone":"warning"}]}),
            ),
        ),
    ];
    for (name, node) in cases {
        let mut cx = HeadlessAppContext::with_platform(
            gpui_platform::test_text_system("Geist"),
            Arc::new(gpui_kit::assets::Assets),
            gpui_platform::current_headless_renderer,
        );
        cx.update(|cx| {
            gpui_kit::install(cx);
            cx.set_reduce_motion(true);
        });
        let window = cx
            .open_window(extent(px(800.), px(600.)), |_, cx| {
                cx.new(|_| Review {
                    node,
                    layout: Default::default(),
                    navigation: Default::default(),
                    dates: Default::default(),
                })
            })
            .unwrap()
            .into();
        for _ in 0..3 {
            cx.run_until_parked();
            cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx))
                .unwrap();
        }
        cx.capture_screenshot(window)
            .unwrap()
            .save(format!("{path}/{name}.png"))
            .unwrap();
    }
}
