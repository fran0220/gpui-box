use super::*;
use gpui::{Modifiers, Styled, TestAppContext, px};
use gpui_kit_testkit::harness::Harness;
use std::{cell::RefCell, rc::Rc};

fn node(component: &str, props: Value, events: Value) -> Node {
    serde_json::from_value(
        json!({"kind":"kit","component":component,"id":"canvas","props":props,"events":events}),
    )
    .expect("canvas fixture descriptor")
}

#[gpui::test]
fn native_graph_proposes_world_coordinates_and_port_connections(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::<Value>::new()));
    let output = events.clone();
    let descriptor = node(
        "NodeGraph",
        json!({
            "viewport":{"offset":{"x":17,"y":31},"zoom":0.75},
            "nodes":[
                {"id":"source","x":80,"y":50,"props":{"title":"Source","width":160,"selected":true,"ports":[{"id":"out","label":"Output","direction":"output"}]}},
                {"id":"sink","x":450,"y":90,"props":{"title":"Sink","width":160,"ports":[{"id":"in","label":"Input","direction":"input"}]}},
                {"id":"blocked","x":450,"y":270,"props":{"title":"Refused connection","width":160,"ports":[{"id":"in","label":"Input","direction":"input"}]}}
            ],
            "can_connect":[{"from":{"node":"source","port":"out"},"to":{"node":"sink","port":"in"}}]
        }),
        json!({"event":"graph.event"}),
    );
    let mut h = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let output = output.clone();
        div()
            .ml(px(37.))
            .mt(px(29.))
            .w(px(720.))
            .h(px(420.))
            .child(render(
                &descriptor,
                KitSlots::new(),
                window,
                cx,
                Rc::new(move |action, value| {
                    assert_eq!(action, "graph.event");
                    output.borrow_mut().push(value);
                }),
            ))
            .into_any_element()
    });
    h.click("sink");
    assert!(
        events
            .borrow()
            .contains(&json!({"type":"selection_changed","ids":["sink"]}))
    );
    events.borrow_mut().clear();
    let start = h.bounds("source").expect("native source bounds").center();
    let end = start + point(px(45.), px(-18.));
    h.context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    h.context()
        .simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    h.context()
        .simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    assert!(
        events
            .borrow()
            .contains(&json!({"type":"node_moved","id":"source","position":{"x":140.0,"y":26.0}})),
        "{:?}",
        events.borrow()
    );
    events.borrow_mut().clear();
    h.drag("graph-port:6:source:3:out", "graph-port:4:sink:2:in");
    assert!(events.borrow().contains(&json!({"type":"connection_requested","from":{"node":"source","port":"out"},"to":{"node":"sink","port":"in"}})),"{:?}",events.borrow());
    events.borrow_mut().clear();
    h.drag("graph-port:6:source:3:out", "graph-port:7:blocked:2:in");
    // Native can_connect previews validity; the caller still owns the refusal.
    assert_eq!(
        *events.borrow(),
        vec![
            json!({"type":"connection_requested","from":{"node":"source","port":"out"},"to":{"node":"blocked","port":"in"}})
        ]
    );
    events.borrow_mut().clear();
    h.click("source");
    h.keystrokes("delete");
    assert!(
        events
            .borrow()
            .contains(&json!({"type":"node_deleted","id":"source"}))
    );
}

#[gpui::test]
fn toolbar_disabled_and_enabled_actions_are_native(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let disabled = Rc::new(RefCell::new(false));
    let state = disabled.clone();
    let mut h = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let output = output.clone();
        let n = node(
            "CanvasToolbar",
            json!({"zoom":"75%","actions":["arrange","fit"],"disabled":*state.borrow()}),
            json!({"action":"toolbar"}),
        );
        render(
            &n,
            KitSlots::new(),
            window,
            cx,
            Rc::new(move |_, value| output.borrow_mut().push(value)),
        )
    });
    h.click("canvas.arrange");
    assert_eq!(*events.borrow(), vec![json!("arrange")]);
    *disabled.borrow_mut() = true;
    h.frame();
    h.click("canvas.fit");
    assert_eq!(*events.borrow(), vec![json!("arrange")]);
}
