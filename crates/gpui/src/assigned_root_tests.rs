//! Root assignment must reflow children, not merely report wrapper dimensions.
use crate::{
    AvailableSpace, Bounds, Context, InputEvent, Modifiers, MouseButton, MouseDownEvent, Pixels,
    Role, TestAppContext, Window, canvas, div, point, prelude::*, px, relative, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct AssignedRoot {
    child_bounds: Rc<Cell<Bounds<Pixels>>>,
    hits: Rc<Cell<usize>>,
    tiny: bool,
}

impl Render for AssignedRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let observed = self.child_bounds.clone();
        let hits = self.hits.clone();
        let tiny = self.tiny;
        canvas(
            move |_, window, cx| {
                let child_observed = observed.clone();
                let clipped_hits = hits.clone();
                let mut element = div()
                    .id("assigned-root")
                    .relative()
                    .w(px(200.0))
                    .h(px(100.0))
                    .min_w(px(180.0))
                    .max_w(px(220.0))
                    .min_h(px(90.0))
                    .max_h(px(110.0))
                    .aspect_ratio(2.0)
                    .p(px(8.0))
                    .border(px(2.0))
                    .overflow_hidden()
                    .flex()
                    .child(
                        div()
                            .id("assigned-child")
                            .w(relative(0.25))
                            .h_full()
                            .flex_none()
                            .role(Role::Button)
                            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                hits.set(hits.get() + 1)
                            })
                            .child(
                                canvas(
                                    move |bounds, _, _| child_observed.set(bounds),
                                    |_, _, _, _| {},
                                )
                                .size_full(),
                            ),
                    )
                    .child(div().flex_1().h_full())
                    .child(
                        div()
                            .id("assigned-overflow")
                            .absolute()
                            .left(px(100.0))
                            .top(px(0.0))
                            .w(px(80.0))
                            .h(px(40.0))
                            .role(Role::Button)
                            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                clipped_hits.set(clipped_hits.get() + 10)
                            }),
                    )
                    .into_any_element();
                let available = size(
                    AvailableSpace::Definite(px(400.0)),
                    AvailableSpace::Definite(px(300.0)),
                );
                assert_eq!(
                    element.layout_as_root(available, window, cx),
                    size(px(200.0), px(100.0))
                );
                let fractional = element.layout_as_root_with_size(
                    available,
                    size(px(120.0), px(70.0)),
                    window,
                    cx,
                );
                assert_eq!(fractional.width, px(120.0));
                assert!((f32::from(fractional.height) - 70.0).abs() <= 1.0 / window.scale_factor());
                assert_eq!(
                    element.layout_as_root_with_size(
                        available,
                        size(px(84.0), px(52.0)),
                        window,
                        cx
                    ),
                    size(px(84.0), px(52.0))
                );
                assert_eq!(
                    element.layout_as_root(available, window, cx),
                    size(px(200.0), px(100.0))
                );
                let assigned = if tiny {
                    size(px(2.0), px(3.0))
                } else {
                    size(px(120.0), px(70.0))
                };
                let actual = element.layout_as_root_with_size(available, assigned, window, cx);
                if tiny {
                    assert!(
                        actual.width >= px(20.0) && actual.height >= px(20.0),
                        "border and padding impose an actual minimum: {actual:?}"
                    );
                } else {
                    assert_eq!(actual.width, assigned.width);
                    assert!(
                        (actual.height - assigned.height).abs() <= px(1.0 / window.scale_factor())
                    );
                }
                // Cached assigned→natural→assigned measurements must still leave
                // the last assigned descendants for prepaint and accessibility.
                element.prepaint_at(point(px(10.0), px(20.0)), window, cx);
                element
            },
            |_, mut element, window, cx| element.paint(window, cx),
        )
        .size_full()
    }
}

#[test]
fn assigned_root_overrides_constraints_restores_style_and_reflows_percent_children() {
    let mut cx = TestAppContext::single();
    let observed = Rc::new(Cell::new(Bounds::default()));
    let hits = Rc::new(Cell::new(0));
    let window = cx.add_window({
        let observed = observed.clone();
        let hits = hits.clone();
        move |_, _| AssignedRoot {
            child_bounds: observed,
            hits,
            tiny: false,
        }
    });
    cx.activate_accessibility(window.into());
    for scale in [1.0, 1.25, 2.0] {
        cx.update_window(window.into(), |_, window, cx| {
            window.set_scale_factor(scale);
            window.draw(cx).clear(cx);
        })
        .expect("mounted window");
        let bounds = observed.get();
        // 120×70 border box minus 2×(8 padding + 2 border) =100×50;
        // first child takes 25% width. Device pixel rounding varies by scale.
        assert!((f32::from(bounds.size.width) - 25.0).abs() <= 1.0 / scale);
        assert!((f32::from(bounds.size.height) - 50.0).abs() <= 1.0 / scale);
        cx.update_window(window.into(), |_, window, _| {
            let tree: serde_json::Value =
                serde_json::from_str(&window.debug_a11y_tree_json().expect("active adapter"))
                    .expect("valid accessibility JSON");
            let node = tree["nodes"]
                .as_object()
                .expect("accessibility node map")
                .values()
                .find(|node| node["element_id"] == "Name(\"assigned-child\")")
                .expect("accessible child");
            let id = accesskit::NodeId(
                node["accesskit_id"]
                    .as_str()
                    .expect("string node id")
                    .parse()
                    .expect("numeric node id"),
            );
            assert_eq!(
                window.a11y.node_bounds[&id], bounds,
                "platform accessibility uses displayed child bounds"
            );
            let overflow = tree["nodes"]
                .as_object()
                .expect("accessibility node map")
                .values()
                .find(|node| node["element_id"] == "Name(\"assigned-overflow\")")
                .expect("clipped accessible child");
            let overflow = accesskit::NodeId(
                overflow["accesskit_id"]
                    .as_str()
                    .expect("string overflow id")
                    .parse()
                    .expect("numeric overflow id"),
            );
            let clipped = window.a11y.node_bounds[&overflow];
            assert!(clipped.size.width > px(0.0) && clipped.size.width < px(80.0));
            assert!(clipped.origin.x + clipped.size.width <= px(130.0));
        })
        .expect("mounted window");
    }
    cx.update_window(window.into(), |_, window, cx| {
        for position in [
            point(px(25.0), px(35.0)),
            point(px(70.0), px(35.0)),
            point(px(115.0), px(35.0)),
            point(px(160.0), px(35.0)),
        ] {
            window.dispatch_event(
                MouseDownEvent {
                    position,
                    button: MouseButton::Left,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                    first_mouse: false,
                }
                .to_platform_input(),
                cx,
            );
        }
    })
    .expect("mounted input window");
    assert_eq!(
        hits.get(),
        11,
        "displayed quarter-width and clipped overflow pick; outside targets do not"
    );
}

#[test]
fn impossible_assignment_reports_padding_border_minimum() {
    let mut cx = TestAppContext::single();
    let window = cx.add_window(|_, _| AssignedRoot {
        child_bounds: Rc::new(Cell::new(Bounds::default())),
        hits: Rc::new(Cell::new(0)),
        tiny: true,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .expect("mounted tiny root");
}

struct WrappedText(Rc<RefCell<Vec<Bounds<Pixels>>>>);
impl Render for WrappedText {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let results = self.0.clone();
        canvas(
            move |_, window, cx| {
                results.borrow_mut().clear();
                let mut roots = Vec::new();
                for (index, width) in [280.0, 80.0].into_iter().enumerate() {
                    let results = results.clone();
                    let mut root = div()
                        .w(px(300.0))
                        .h(px(200.0))
                        .child(
                            div()
                                .w_full()
                                .relative()
                                .child("alpha beta gamma delta epsilon zeta eta theta iota kappa")
                                .child(
                                    canvas(
                                        move |bounds, _, _| results.borrow_mut().push(bounds),
                                        |_, _, _, _| {},
                                    )
                                    .absolute()
                                    .size_full(),
                                ),
                        )
                        .into_any_element();
                    root.prepaint_as_root_with_size(
                        point(px(index as f32 * 320.0), px(0.0)),
                        size(AvailableSpace::MaxContent, AvailableSpace::MaxContent),
                        size(px(width), px(200.0)),
                        window,
                        cx,
                    );
                    roots.push(root);
                }
                roots
            },
            |_, mut roots, window, cx| {
                for root in &mut roots {
                    root.paint(window, cx);
                }
            },
        )
        .size_full()
    }
}

#[test]
fn assigned_root_reflows_wrapped_text_at_asymmetric_widths() {
    let mut cx = TestAppContext::single();
    let results = Rc::new(RefCell::new(Vec::new()));
    let window = cx.add_window({
        let results = results.clone();
        move |_, _| WrappedText(results)
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .expect("mounted text roots");
    let results = results.borrow();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].size.width, px(280.0));
    assert_eq!(results[1].size.width, px(80.0));
    assert!(
        results[1].size.height > results[0].size.height,
        "narrow descendants must actually wrap"
    );
}
