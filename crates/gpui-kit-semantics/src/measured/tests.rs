use super::*;
use crate::{Semantic, Snapshot, install};
use gpui::{
    AnyElement, AnyWindowHandle, AppContext, ContentMask, Context, Corners, ParentElement, Render,
    RoundedClip, TestAppContext, div, point, px, size,
};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

#[derive(Clone)]
struct Config {
    keys: Vec<usize>,
    width: f32,
    revision: usize,
    mounted: bool,
    secrets: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            keys: vec![0, 1],
            width: 220.,
            revision: 0,
            mounted: true,
            secrets: false,
        }
    }
}

fn leaves(bounds: Bounds<Pixels>, config: &Config) -> Vec<MeasuredLeaf> {
    config
        .keys
        .iter()
        .map(|&key| {
            let (x, y, w, h) = match key {
                0 => (3.5, 4.5, f32::from(bounds.size.width) / 4., 7.5),
                1 => (f32::from(bounds.size.width) / 2. - 5., 25., 13., 9.),
                2 => (-20., 42., 5., 6.),
                3 => (42., 25., 0., 0.),
                _ => (-6., -7., 1., 1.),
            };
            let mut leaf = MeasuredLeaf::new(
                format!("leaf-{key}"),
                if key == 3 {
                    MeasuredLeafRole::Text
                } else {
                    MeasuredLeafRole::Image
                },
                Bounds::new(bounds.origin + point(px(x), px(y)), size(px(w), px(h))),
            )
            .text(format!("item-{key}"))
            .description(format!("description-{key}"))
            .value(format!("value-{key}-{}", config.revision))
            .selected(key == 1)
            .read_only(true);
            if config.secrets && key == 0 {
                leaf = leaf
                    .text("Bearer fixture-text")
                    .description("sk-fixture-help")
                    .value("token=fixture-value");
            }
            leaf
        })
        .collect()
}

struct Fixture {
    config: Rc<RefCell<Config>>,
    measured: Rc<Cell<usize>>,
    batch: bool,
    transform: bool,
}

impl Render for Fixture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SemanticCoordinator::global(cx).begin_frame(window);
        let config = self.config.borrow().clone();
        let mut parent = div()
            .absolute()
            .left(px(13.))
            .top(px(19.))
            .w(px(config.width))
            .h(px(80.));
        if config.mounted {
            if self.batch {
                let current = self.config.clone();
                let measured = self.measured.clone();
                parent = parent.child(
                    MeasuredLeafBatch::new("geometry-targets", move |bounds, _, _| {
                        measured.set(measured.get() + 1);
                        leaves(bounds, &current.borrow())
                    })
                    .diagnostic_parent("map")
                    .absolute()
                    .size_full(),
                );
            } else {
                for leaf in leaves(
                    Bounds::new(point(px(0.), px(0.)), size(px(config.width), px(80.))),
                    &config,
                ) {
                    parent = parent.child(
                        div()
                            .absolute()
                            .left(leaf.bounds.origin.x)
                            .top(leaf.bounds.origin.y)
                            .w(leaf.bounds.size.width)
                            .h(leaf.bounds.size.height)
                            .semantic_in(cx, leaf.spec.parent("map")),
                    );
                }
            }
        }
        let child = div()
            .relative()
            .size_full()
            .child(parent.semantic_in(cx, NodeSpec::new("map", Role::Group)))
            .into_any_element();
        if self.transform {
            Scoped(child).into_any_element()
        } else {
            child
        }
    }
}

// Mounted framework scopes, not a hand-implemented bounds transform or clip.
struct Scoped(AnyElement);
impl IntoElement for Scoped {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Scoped {
    fn scope<R>(window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
        window.with_content_mask(
            Some(ContentMask {
                bounds: Bounds::new(point(px(10.), px(15.)), size(px(160.), px(60.))),
            }),
            |window| {
                window.with_visual_scale(1.25, point(px(-10.), px(-5.)), |window| {
                    window.with_rounded_content_mask(
                        RoundedClip::new(
                            Bounds::new(point(px(0.), px(0.)), size(px(120.), px(60.))),
                            Corners {
                                top_left: px(40.),
                                ..Corners::default()
                            },
                        ),
                        f,
                    )
                })
            },
        )
    }
}
impl Element for Scoped {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        w: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.0.request_layout(w, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        w: &mut Window,
        cx: &mut App,
    ) {
        Self::scope(w, |w| self.0.prepaint(w, cx));
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        w: &mut Window,
        cx: &mut App,
    ) {
        Self::scope(w, |w| self.0.paint(w, cx));
    }
}

fn draw(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
) -> (Snapshot, Option<Value>, gpui::FrameStats) {
    cx.update_window(handle, |_, window, cx| {
        window.draw(cx).clear(cx);
        (
            SemanticCoordinator::global(cx)
                .snapshot(handle.window_id())
                .expect("snapshot"),
            window
                .debug_a11y_tree_json()
                .map(|s| serde_json::from_str(&s).expect("native JSON")),
            window.frame_stats(),
        )
    })
    .expect("mounted window")
}

fn native_leaves(tree: &Value) -> BTreeMap<String, Value> {
    tree["nodes"]
        .as_object()
        .expect("nodes")
        .values()
        .filter(|n| n["aria"]["role"] == "Image" || n["aria"]["role"] == "Label")
        .map(|n| {
            (
                n["aria"]["label"].as_str().expect("label").to_owned(),
                n.clone(),
            )
        })
        .collect()
}

fn close(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, _| window.remove_window())
        .expect("remove window");
    cx.run_until_parked();
    cx.update(|cx| assert!(cx.windows().is_empty()));
}

#[gpui::test]
fn measured_outputs_match_ordinary_semantic_leaves(cx: &mut TestAppContext) {
    cx.update(install);
    let _arm = cx.update(|cx| SemanticCoordinator::global(cx).arm());
    for transform in [false, true] {
        let mut outputs = Vec::new();
        for batch in [false, true] {
            let config = Rc::new(RefCell::new(Config::default()));
            let window = cx
                .add_window(|_, _| Fixture {
                    config,
                    measured: Rc::default(),
                    batch,
                    transform,
                })
                .into();
            cx.activate_accessibility(window);
            let (snapshot, native, _) = draw(cx, window);
            let native = native_leaves(&native.expect("active native"));
            for leaf in native.values() {
                assert!(leaf["aria"]["on_action"].is_null());
                assert!(leaf["aria"]["selected"].is_null());
                assert_eq!(leaf["aria"]["read_only"], true);
            }
            outputs.push((
                snapshot
                    .nodes
                    .into_iter()
                    .filter(|n| n.id.starts_with("leaf-"))
                    .collect::<Vec<_>>(),
                native,
            ));
            close(cx, window);
        }
        assert_eq!(outputs[0].0, outputs[1].0, "diagnostic parity");
        for (key, old) in &outputs[0].1 {
            let new = &outputs[1].1[key];
            assert_eq!(old["aria"], new["aria"], "native property parity {key}");
            assert_eq!(old["bounds"], new["bounds"], "native bounds parity {key}");
        }
    }
}

#[gpui::test]
fn current_frame_geometry_stable_keys_replacement_and_unmount(cx: &mut TestAppContext) {
    cx.update(install);
    let _arm = cx.update(|cx| SemanticCoordinator::global(cx).arm());
    let config = Rc::new(RefCell::new(Config::default()));
    let window = cx
        .add_window(|_, _| Fixture {
            config: config.clone(),
            measured: Rc::default(),
            batch: true,
            transform: false,
        })
        .into();
    cx.activate_accessibility(window);
    let (first, native, _) = draw(cx, window);
    assert_eq!(
        first.find("leaf-0").expect("leaf").bounds,
        crate::Rect {
            x: 16.5,
            y: 23.5,
            width: 55.,
            height: 7.5
        }
    );
    let ids = native_leaves(&native.expect("native"));
    {
        let mut c = config.borrow_mut();
        c.keys = vec![1, 0];
        c.width = 300.;
        c.revision = 9;
    }
    let (second, native, _) = draw(cx, window);
    assert_eq!(
        second.find("leaf-0").expect("resized leaf").bounds.width,
        75.
    );
    assert_eq!(second.find("leaf-1").expect("shifted leaf").bounds.x, 158.);
    assert_eq!(
        second
            .find("leaf-1")
            .expect("selected leaf")
            .parent
            .as_deref(),
        Some("map")
    );
    assert!(second.find("leaf-1").expect("selected leaf").selected);
    let next = native_leaves(&native.expect("native"));
    for key in ["item-0", "item-1"] {
        assert_eq!(ids[key]["accesskit_id"], next[key]["accesskit_id"]);
    }
    assert_eq!(next["item-0"]["aria"]["value"], "value-0-9");
    config.borrow_mut().keys = vec![1];
    let (removed, native, _) = draw(cx, window);
    assert!(!removed.contains("leaf-0"));
    assert_eq!(native_leaves(&native.expect("native")).len(), 1);
    config.borrow_mut().keys.clear();
    let (empty, native, _) = draw(cx, window);
    assert_eq!(empty.ids(), vec!["map"]);
    assert!(native_leaves(&native.expect("native")).is_empty());
    config.borrow_mut().keys = vec![0];
    config.borrow_mut().secrets = true;
    let (redacted, native, _) = draw(cx, window);
    let node = redacted.find("leaf-0").expect("redacted leaf");
    assert_eq!(node.text.as_deref(), Some("[REDACTED]"));
    assert_eq!(node.description.as_deref(), Some("[REDACTED]"));
    assert_eq!(node.value.as_deref(), Some("[REDACTED]"));
    let redacted_native = native_leaves(&native.expect("native"));
    let node = &redacted_native["[REDACTED]"];
    assert_eq!(node["accesskit_id"], ids["item-0"]["accesskit_id"]);
    assert_eq!(node["aria"]["description"], "[REDACTED]");
    assert_eq!(node["aria"]["value"], "[REDACTED]");
    config.borrow_mut().mounted = false;
    let (gone, native, _) = draw(cx, window);
    assert_eq!(gone.ids(), vec!["map"]);
    assert!(native_leaves(&native.expect("native")).is_empty());
    close(cx, window);
}

#[gpui::test]
fn fractional_transform_rounded_clip_and_zero_area_native_bounds(cx: &mut TestAppContext) {
    cx.update(install);
    let _arm = cx.update(|cx| SemanticCoordinator::global(cx).arm());
    let config = Rc::new(RefCell::new(Config {
        keys: vec![0, 1, 2, 3, 4],
        ..Config::default()
    }));
    let window = cx
        .add_window(|_, _| Fixture {
            config,
            measured: Rc::default(),
            batch: true,
            transform: true,
        })
        .into();
    cx.activate_accessibility(window);
    let (diagnostic, native, _) = draw(cx, window);
    let tree = native.expect("native");
    assert_eq!(tree["frame"]["scale_factor"], 2.0);
    let native = native_leaves(&tree);
    // Independently calculated physical bounds: display=(logical-origin)*1.25+origin,
    // device scale 2; inherited conservative clip is x=10..152.5,y=15..75 logical.
    for (key, expected) in [
        ("item-0", [46.25, 61.25, 183.75, 80.]),
        ("item-1", [300., 112.5, 305., 135.]),
        ("item-2", [20., 150., 20., 150.]),
        ("item-3", [142.5, 112.5, 142.5, 112.5]),
        // Inside the conservative rectangle but outside the rounded corner.
        ("item-4", [22.5, 32.5, 25., 35.]),
    ] {
        for (coordinate, value) in ["x0", "y0", "x1", "y1"].into_iter().zip(expected) {
            assert_eq!(
                native[key]["bounds"][coordinate], value,
                "{key} {coordinate}"
            );
        }
    }
    assert_eq!(
        diagnostic.find("leaf-1").expect("diagnostic").bounds,
        crate::Rect {
            x: 150.,
            y: 56.25,
            width: 16.25,
            height: 11.25
        }
    );
    assert!(!diagnostic.find("leaf-3").expect("zero diagnostic").visible);
    let nodes = tree["nodes"].as_object().expect("nodes");
    let owner = nodes
        .values()
        .find(|n| n["children"].as_array().is_some_and(|c| c.len() == 5))
        .expect("synthetic owner");
    assert_eq!(owner["aria"]["role"], "Group");
    close(cx, window);
}

#[gpui::test]
fn independent_outputs_and_diagnostic_disarming(cx: &mut TestAppContext) {
    cx.update(install);
    for native in [false, true] {
        let measured = Rc::new(Cell::new(0));
        let window = cx
            .add_window(|_, _| Fixture {
                config: Rc::new(RefCell::new(Config::default())),
                measured: measured.clone(),
                batch: true,
                transform: false,
            })
            .into();
        if native {
            cx.activate_accessibility(window);
        }
        for diagnostics in [false, true, false] {
            let arm = diagnostics.then(|| cx.update(|cx| SemanticCoordinator::global(cx).arm()));
            let before = measured.get();
            let (snapshot, tree, stats) = draw(cx, window);
            assert_eq!(measured.get() - before, usize::from(native || diagnostics));
            assert_eq!(snapshot.nodes.len(), if diagnostics { 3 } else { 0 });
            assert_eq!(stats.semantic_nodes, if diagnostics { 3 } else { 0 });
            assert_eq!(
                tree.as_ref().map(native_leaves).map(|n| n.len()),
                native.then_some(2)
            );
            drop(arm);
        }
        close(cx, window);
    }
}

struct Dense {
    count: usize,
    batch: bool,
    measured: Rc<Cell<usize>>,
}

#[gpui::test]
fn descriptive_batch_does_not_intercept_parent_pointer_capture(cx: &mut TestAppContext) {
    use gpui::{InteractiveElement as _, MouseButton, VisualTestContext};
    struct PointerFixture(Rc<Cell<usize>>);
    impl Render for PointerFixture {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let events = self.0.clone();
            div()
                .id("pointer-owner")
                .relative()
                .w(px(200.))
                .h(px(100.))
                .on_mouse_down_with_pointer_capture(MouseButton::Left, move |_, window, _| {
                    assert!(window.captured_hitbox().is_some());
                    events.set(events.get() + 1);
                })
                .child(
                    MeasuredLeafBatch::new("descriptions", |bounds, _, _| {
                        vec![
                            MeasuredLeaf::new("business-key", MeasuredLeafRole::Image, bounds)
                                .text("Description"),
                        ]
                    })
                    .absolute()
                    .size_full(),
                )
        }
    }
    let events = Rc::new(Cell::new(0));
    let handle = cx.add_window(|_, _| PointerFixture(events.clone())).into();
    // No semantics installation: native publication must still work.
    cx.activate_accessibility(handle);
    cx.update_window(handle, |_, w, cx| w.draw(cx).clear(cx))
        .expect("draw");
    let mut visual = VisualTestContext::from_window(handle, cx);
    visual.simulate_mouse_move(point(px(35.), px(21.)), None, gpui::Modifiers::none());
    visual.simulate_mouse_down(
        point(px(35.), px(21.)),
        MouseButton::Left,
        gpui::Modifiers::none(),
    );
    assert_eq!(events.get(), 1);
    visual.simulate_mouse_up(
        point(px(250.), px(140.)),
        MouseButton::Left,
        gpui::Modifiers::none(),
    );
    cx.update_window(handle, |_, w, _| assert!(w.captured_hitbox().is_none()))
        .expect("released capture");
    close(cx, handle);
}

#[gpui::test]
#[should_panic(expected = "measured leaf ids must be unique within a batch")]
fn duplicate_business_ids_are_rejected(cx: &mut TestAppContext) {
    cx.update(install);
    let _arm = cx.update(|cx| SemanticCoordinator::global(cx).arm());
    let config = Rc::new(RefCell::new(Config {
        keys: vec![0, 0],
        ..Config::default()
    }));
    let handle = cx
        .add_window(|_, _| Fixture {
            config,
            measured: Rc::default(),
            batch: true,
            transform: false,
        })
        .into();
    draw(cx, handle);
}
fn dense_leaves(bounds: Bounds<Pixels>, count: usize) -> Vec<MeasuredLeaf> {
    (0..count)
        .map(|i| {
            let x = 320. + (i % 1000) as f32 * 0.358 / 360. * 280.;
            let y = 140. - ((i / 1000) as f32 - 5.) * 1.6 / 360. * 280.;
            MeasuredLeaf::new(
                format!("dense-{i}"),
                MeasuredLeafRole::Image,
                Bounds::new(
                    bounds.origin + point(px(x - 5.), px(y - 5.)),
                    size(px(10.), px(10.)),
                ),
            )
            .text(format!("point-{i}"))
            .value(format!("reading-{i}"))
            .read_only(true)
        })
        .collect()
}
impl Render for Dense {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SemanticCoordinator::global(cx).begin_frame(window);
        let mut parent = div().relative().w(px(640.)).h(px(280.));
        if self.batch {
            let count = self.count;
            let measured = self.measured.clone();
            parent = parent.child(
                MeasuredLeafBatch::new("dense-targets", move |bounds, _, _| {
                    measured.set(measured.get() + count);
                    dense_leaves(bounds, count)
                })
                .absolute()
                .size_full(),
            );
        } else {
            for leaf in dense_leaves(Bounds::default(), self.count) {
                parent = parent.child(
                    div()
                        .absolute()
                        .left(leaf.bounds.origin.x)
                        .top(leaf.bounds.origin.y)
                        .w(leaf.bounds.size.width)
                        .h(leaf.bounds.size.height)
                        .semantic_in(cx, leaf.spec),
                );
            }
        }
        parent
    }
}

#[gpui::test]
fn ten_thousand_targets_have_constant_element_work(cx: &mut TestAppContext) {
    cx.update(install);
    let mut layouts = Vec::new();
    for count in [0, 1, 10_000] {
        for native in [false, true] {
            for diagnostics in [false, true] {
                let arm =
                    diagnostics.then(|| cx.update(|cx| SemanticCoordinator::global(cx).arm()));
                let measured = Rc::new(Cell::new(0));
                let window = cx
                    .add_window(|_, _| Dense {
                        count,
                        batch: true,
                        measured: measured.clone(),
                    })
                    .into();
                if native {
                    cx.activate_accessibility(window);
                }
                let before = measured.get();
                let (snapshot, tree, stats) = draw(cx, window);
                assert_eq!(
                    measured.get() - before,
                    if native || diagnostics { count } else { 0 }
                );
                assert_eq!(snapshot.nodes.len(), if diagnostics { count } else { 0 });
                if native {
                    assert_eq!(native_leaves(&tree.expect("native")).len(), count);
                }
                layouts.push((
                    stats.request_layout_calls,
                    stats.prepaint_calls,
                    stats.paint_calls,
                ));
                close(cx, window);
                drop(arm);
            }
        }
    }
    assert!(
        layouts.iter().all(|counts| *counts == layouts[0]),
        "constant work: {layouts:?}"
    );
    println!(
        "measured-leaf batch 0/1/10000 targets in all four modes: request/prepaint/paint={:?}",
        layouts[0]
    );
}

#[gpui::test]
#[ignore = "CPU-only 2x2 output matrix, five isolated warm frames per case"]
fn measured_leaf_work_breakdown(cx: &mut TestAppContext) {
    cx.update(install);
    for batch in [false, true] {
        for native in [false, true] {
            for diagnostics in [false, true] {
                cx.update(|cx| assert!(cx.windows().is_empty()));
                let arm =
                    diagnostics.then(|| cx.update(|cx| SemanticCoordinator::global(cx).arm()));
                let window = cx
                    .add_window(|_, _| Dense {
                        count: 10_000,
                        batch,
                        measured: Rc::default(),
                    })
                    .into();
                if native {
                    cx.activate_accessibility(window);
                }
                let (_, _, initial) = draw(cx, window);
                let mut frame_index = initial.frame_index;
                let mut times = Vec::new();
                for _ in 0..5 {
                    let start = std::time::Instant::now();
                    cx.update(|cx| cx.refresh_windows());
                    cx.run_until_parked();
                    times.push(start.elapsed());
                    let rendered = cx
                        .update_window(window, |_, w, _| w.frame_stats())
                        .expect("rendered frame");
                    assert!(
                        rendered.frame_index > frame_index,
                        "timed redraw must actually render"
                    );
                    assert_eq!(
                        rendered.request_layout_calls,
                        if batch { 3 } else { 10_002 }
                    );
                    frame_index = rendered.frame_index;
                }
                times.sort();
                let (snapshot, tree, stats) = draw(cx, window);
                assert_eq!(snapshot.nodes.len(), if diagnostics { 10_000 } else { 0 });
                if native {
                    assert_eq!(native_leaves(&tree.expect("native")).len(), 10_000);
                }
                println!(
                    "batch={batch},native={native},diagnostics={diagnostics},median_us={},min_us={},max_us={},layout={},prepaint={},paint={}",
                    times[2].as_micros(),
                    times[0].as_micros(),
                    times[4].as_micros(),
                    stats.request_layout_calls,
                    stats.prepaint_calls,
                    stats.paint_calls
                );
                close(cx, window);
                drop(arm);
            }
        }
    }
}
