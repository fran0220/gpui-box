//! NodeGraph proposes edits and keeps the caller's topology, positions and
//! viewport authoritative.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{
    Modifiers, MouseButton, ScrollDelta, ScrollWheelEvent, SharedString, TestAppContext,
    TouchPhase, div, point, prelude::*, px,
};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

type Calls = Rc<RefCell<Vec<NodeGraphEvent>>>;

fn port_id(node: &str, port: &str) -> String {
    format!("graph-port:{}:{}:{}:{}", node.len(), node, port.len(), port)
}

fn edge_id(edge: &str) -> String {
    format!("graph-edge:{}:{}", edge.len(), edge)
}

fn editor(cx: &mut TestAppContext) -> (Harness, Calls) {
    let calls = Calls::default();
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        div()
            .ml(px(37.0))
            .mt(px(29.0))
            .w(px(720.0))
            .h(px(360.0))
            .child(
                NodeGraph::new("graph")
                    .node(
                        GraphNode::new("graph.source", "Source")
                            .width(160.0)
                            .selected(true)
                            .port(GraphPort::input("config", "Config"))
                            .port(GraphPort::output("records", "Records")),
                        80.0,
                        80.0,
                    )
                    .node(
                        GraphNode::new("graph.target", "Target")
                            .width(160.0)
                            .port(GraphPort::input("records", "Records"))
                            .port(GraphPort::output("done", "Done")),
                        420.0,
                        80.0,
                    )
                    .edge(
                        GraphEdge::new("graph.source", "graph.target")
                            .id("graph.connection")
                            .ports("records", "records"),
                    )
                    .on_event(move |event, _, _| sink.borrow_mut().push(event.clone())),
            )
            .into_any_element()
    });
    // Publish the viewport bounds measured by the first frame.
    harness.frame();
    (harness, calls)
}

fn controlled_editor(cx: &mut TestAppContext) -> (Harness, Calls, Rc<Cell<usize>>) {
    let calls = Calls::default();
    let position = Rc::new(Cell::new(point(80.0, 80.0)));
    let clicks = Rc::new(Cell::new(0));
    let sink = Rc::clone(&calls);
    let rendered_position = Rc::clone(&position);
    let rendered_clicks = Rc::clone(&clicks);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        let position = Rc::clone(&rendered_position);
        let click_count = Rc::clone(&rendered_clicks);
        let at = position.get();
        div()
            .w(px(720.0))
            .h(px(360.0))
            .child(
                NodeGraph::new("controlled-graph")
                    .node(
                        GraphNode::new("controlled-graph.source", "Source")
                            .on_click(move |_, _| click_count.set(click_count.get() + 1)),
                        at.x,
                        at.y,
                    )
                    .on_event(move |event, window, _| {
                        sink.borrow_mut().push(event.clone());
                        if let NodeGraphEvent::NodeMoved { id, position: next } = event
                            && id == "controlled-graph.source"
                        {
                            position.set(*next);
                            window.refresh();
                        }
                    }),
            )
            .into_any_element()
    });
    harness.frame();
    (harness, calls, clicks)
}

#[gpui::test]
fn ports_publish_business_identity_direction_and_label(cx: &mut TestAppContext) {
    let (mut harness, _) = editor(cx);

    let input = harness
        .node(&port_id("graph.source", "config"))
        .expect("input port is published");
    assert_eq!(input.value.as_deref(), Some("input"));
    assert_eq!(input.text.as_deref(), Some("Config"));

    let output = harness
        .node(&port_id("graph.source", "records"))
        .expect("output port is published");
    assert_eq!(output.value.as_deref(), Some("output"));
    assert_eq!(output.text.as_deref(), Some("Records"));
}

#[gpui::test]
fn blank_canvas_drag_proposes_a_pan_but_does_not_apply_it(cx: &mut TestAppContext) {
    let (mut harness, calls) = editor(cx);
    let bounds = harness.bounds("graph").expect("graph bounds");
    let start = point(bounds.left() + px(30.0), bounds.bottom() - px(30.0));
    let end = start + point(px(85.0), px(-45.0));

    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(end, MouseButton::Left, Modifiers::none());

    assert!(calls.borrow().iter().any(|event| matches!(
        event,
        NodeGraphEvent::ViewportChanged(GraphViewport { offset, zoom })
            if *offset == point(85.0, -45.0) && *zoom == 1.0
    )));
    assert_eq!(
        harness
            .node("graph")
            .expect("graph remains published")
            .value
            .as_deref(),
        Some("state:ready;offset:0.000,0.000;zoom:1.000"),
        "the semantic state remains caller-controlled"
    );
}

#[gpui::test]
fn blank_canvas_capture_delivers_an_outside_release(cx: &mut TestAppContext) {
    let (mut harness, calls) = editor(cx);
    let bounds = harness.bounds("graph").expect("graph bounds");
    let start = point(bounds.left() + px(30.0), bounds.bottom() - px(30.0));
    let outside = point(bounds.right() + px(80.0), bounds.bottom() + px(60.0));

    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_move(outside, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());

    calls.borrow_mut().clear();
    harness.context().simulate_event(ScrollWheelEvent {
        position: bounds.center(),
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    assert!(
        calls
            .borrow()
            .iter()
            .any(|event| matches!(event, NodeGraphEvent::ViewportChanged(_))),
        "outside mouse-up clears the captured pan before the next gesture"
    );
}

#[gpui::test]
fn wheel_zoom_is_clamped_and_keeps_the_pointer_on_the_same_world_point(cx: &mut TestAppContext) {
    let (mut harness, calls) = editor(cx);
    let bounds = harness.bounds("graph").expect("graph bounds");
    let pointer = point(bounds.left() + px(213.0), bounds.top() + px(147.0));

    harness.context().simulate_event(ScrollWheelEvent {
        position: pointer,
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });

    let viewport = calls
        .borrow()
        .iter()
        .find_map(|event| match event {
            NodeGraphEvent::ViewportChanged(viewport) => Some(*viewport),
            _ => None,
        })
        .expect("wheel proposes a viewport");
    assert!((1.0..=2.0).contains(&viewport.zoom));
    let local = point(213.0, 147.0);
    assert!((local.x - (viewport.offset.x + local.x * viewport.zoom)).abs() < 0.001);
    assert!((local.y - (viewport.offset.y + local.y * viewport.zoom)).abs() < 0.001);
}

#[gpui::test]
fn node_drag_keeps_reporting_after_the_pointer_leaves_the_canvas(cx: &mut TestAppContext) {
    let (mut harness, calls) = editor(cx);
    let start = harness.point_in("graph.source");
    let graph = harness.bounds("graph").expect("graph bounds");
    let outside = point(graph.right() + px(120.0), graph.bottom() + px(80.0));

    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_move(outside, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());

    let moved = calls.borrow().iter().any(|event| {
        matches!(
            event,
            NodeGraphEvent::NodeMoved { id, position }
                if id == "graph.source" && position.x > 700.0 && position.y > 400.0
        )
    });
    assert!(
        moved,
        "pointer capture keeps the node gesture alive outside"
    );

    calls.borrow_mut().clear();
    harness.context().simulate_event(ScrollWheelEvent {
        position: graph.center(),
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    assert!(
        calls
            .borrow()
            .iter()
            .any(|event| matches!(event, NodeGraphEvent::ViewportChanged(_))),
        "outside mouse-up releases capture and clears the node gesture"
    );
}

#[gpui::test]
fn controlled_node_drag_survives_the_redraw_that_applies_its_first_proposal(
    cx: &mut TestAppContext,
) {
    let (mut harness, calls, _) = controlled_editor(cx);
    let start = harness.point_in("controlled-graph.source");
    let first = start + point(px(36.0), px(24.0));
    let graph = harness.bounds("controlled-graph").expect("graph bounds");
    let outside = point(graph.right() + px(90.0), graph.bottom() + px(60.0));

    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_move(first, MouseButton::Left, Modifiers::none());
    harness.frame();
    harness
        .context()
        .simulate_mouse_move(outside, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());

    let proposals = calls
        .borrow()
        .iter()
        .filter(|event| matches!(event, NodeGraphEvent::NodeMoved { .. }))
        .count();
    assert!(proposals >= 2, "events: {:?}", calls.borrow());
}

#[gpui::test]
fn a_node_drag_does_not_also_activate_the_nodes_click(cx: &mut TestAppContext) {
    let (mut harness, _, clicks) = controlled_editor(cx);
    let stationary = harness.point_in("controlled-graph.source");
    harness
        .context()
        .simulate_mouse_down(stationary, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(stationary, MouseButton::Left, Modifiers::none());
    assert_eq!(clicks.get(), 1, "a stationary gesture remains a click");

    let start = harness.point_in("controlled-graph.source");
    let end = start + point(px(40.0), px(20.0));
    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    assert_eq!(clicks.get(), 1, "dragging emits no click action");
}

#[gpui::test]
fn node_clicks_propose_single_and_extended_selection_without_applying_it(cx: &mut TestAppContext) {
    let (mut harness, calls) = editor(cx);
    harness.click("graph.target");
    assert!(calls.borrow().iter().any(|event| matches!(
        event,
        NodeGraphEvent::SelectionChanged { ids }
            if ids.as_slice() == [SharedString::from("graph.target")]
    )));
    assert!(!harness.node("graph.target").expect("target").selected);

    calls.borrow_mut().clear();
    let target = harness.point_in("graph.target");
    harness.context().simulate_mouse_down(
        target,
        MouseButton::Left,
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    harness.context().simulate_mouse_up(
        target,
        MouseButton::Left,
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    assert!(calls.borrow().iter().any(|event| matches!(
        event,
        NodeGraphEvent::SelectionChanged { ids }
            if ids.as_slice()
                == [
                    SharedString::from("graph.source"),
                    SharedString::from("graph.target"),
                ]
    )));
}

#[gpui::test]
fn blank_canvas_click_proposes_clearing_selection(cx: &mut TestAppContext) {
    let (mut harness, calls) = editor(cx);
    let bounds = harness.bounds("graph").expect("graph bounds");
    let blank = point(bounds.left() + px(24.0), bounds.bottom() - px(24.0));
    harness
        .context()
        .simulate_mouse_down(blank, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(blank, MouseButton::Left, Modifiers::none());
    assert!(calls.borrow().iter().any(|event| matches!(
        event,
        NodeGraphEvent::SelectionChanged { ids } if ids.is_empty()
    )));
}

#[gpui::test]
fn focused_node_delete_key_proposes_deletion(cx: &mut TestAppContext) {
    let (mut harness, calls) = editor(cx);
    harness.click("graph.source");
    calls.borrow_mut().clear();
    harness.keystrokes("delete");
    assert!(calls.borrow().iter().any(|event| matches!(
        event,
        NodeGraphEvent::NodeDeleted { id } if id == "graph.source"
    )));
    assert!(harness.node("graph.source").is_some());
}

#[gpui::test]
fn edge_action_proposes_disconnect_without_mutating_topology(cx: &mut TestAppContext) {
    let (mut harness, calls) = editor(cx);
    let action = edge_id("graph.connection");
    let semantic = harness.node(&action).expect("edge action is published");
    assert_eq!(semantic.text.as_deref(), Some("Disconnect"));
    assert_eq!(semantic.value.as_deref(), Some("graph.connection"));

    harness.click(&action);
    assert!(
        calls.borrow().iter().any(|event| matches!(
            event,
            NodeGraphEvent::DisconnectRequested { id } if id == "graph.connection"
        )),
        "events: {:?}",
        calls.borrow()
    );
    assert!(
        harness.node(&action).is_some(),
        "the edge remains until the caller applies the proposal"
    );
}

#[gpui::test]
fn thumbnail_slot_publishes_caller_content_and_participates_in_measurement(
    cx: &mut TestAppContext,
) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w(px(420.0))
            .h(px(320.0))
            .child(
                NodeGraph::new("picture-graph").node(
                    GraphNode::new("picture-node", "Picture")
                        .width(180.0)
                        .thumbnail(
                            div()
                                .size_full()
                                .bg(gpui::red())
                                .child("caller-owned preview"),
                        )
                        .thumbnail_ratio(1.0),
                    40.0,
                    40.0,
                ),
            )
            .into_any_element()
    });
    harness.frame();
    harness.frame();
    let node = harness.bounds("picture-node").expect("node");
    let thumbnail = harness
        .node("picture-node.thumbnail")
        .expect("thumbnail semantic slot");
    assert_eq!(thumbnail.text.as_deref(), Some("Picture"));
    assert!(f32::from(node.size.height) > 190.0, "bounds: {node:?}");
}

#[gpui::test]
fn wheel_zoom_is_ignored_while_a_node_gesture_is_active(cx: &mut TestAppContext) {
    let (mut harness, calls) = editor(cx);
    let pointer = harness.point_in("graph.source");
    harness
        .context()
        .simulate_mouse_down(pointer, MouseButton::Left, Modifiers::none());
    harness.context().simulate_event(ScrollWheelEvent {
        position: pointer,
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    harness
        .context()
        .simulate_mouse_up(pointer, MouseButton::Left, Modifiers::none());

    assert!(
        !calls
            .borrow()
            .iter()
            .any(|event| matches!(event, NodeGraphEvent::ViewportChanged(_)))
    );
}

#[gpui::test]
fn composite_port_ids_remain_distinct_for_delimiter_like_business_ids(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w(px(500.0))
            .h(px(300.0))
            .child(
                NodeGraph::new("collision-graph")
                    .node(
                        GraphNode::new("a", "A").port(GraphPort::output("b.port.c", "One")),
                        20.0,
                        40.0,
                    )
                    .node(
                        GraphNode::new("a.port.b", "B").port(GraphPort::output("c", "Two")),
                        280.0,
                        40.0,
                    ),
            )
            .into_any_element()
    });
    assert!(harness.node(&port_id("a", "b.port.c")).is_some());
    assert!(harness.node(&port_id("a.port.b", "c")).is_some());
    assert_ne!(port_id("a", "b.port.c"), port_id("a.port.b", "c"));
}

#[gpui::test]
fn ports_follow_the_prepainted_height_of_wrapped_node_content(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w(px(420.0))
            .h(px(320.0))
            .child(
                NodeGraph::new("measured-graph").node(
                    GraphNode::new("measured-graph.node", "Measured")
                        .width(104.0)
                        .metrics((0..6).map(|index| NodeMetric::new(format!("m{index}"), "123456")))
                        .port(GraphPort::output("result", "Result")),
                    40.0,
                    40.0,
                ),
            )
            .into_any_element()
    });
    harness.frame();
    harness.frame();
    let node = harness.bounds("measured-graph.node").expect("node bounds");
    let port = harness
        .bounds(&port_id("measured-graph.node", "result"))
        .expect("port bounds");
    assert!(f32::from(node.size.height) > 80.0, "node bounds: {node:?}");
    assert!(
        (f32::from(port.center().y - node.center().y)).abs() < 1.0,
        "port {port:?} follows measured node {node:?}"
    );
}

#[gpui::test]
fn output_drag_requests_only_a_valid_input_connection(cx: &mut TestAppContext) {
    let (mut harness, calls) = editor(cx);
    let from = harness.point_in(&port_id("graph.source", "records"));
    let valid = harness.point_in(&port_id("graph.target", "records"));

    harness
        .context()
        .simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_move(valid, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(valid, MouseButton::Left, Modifiers::none());

    assert!(
        calls.borrow().iter().any(|event| matches!(
            event,
            NodeGraphEvent::ConnectionRequested { from, to }
                if from == &GraphEndpoint::new("graph.source", "records")
                    && to == &GraphEndpoint::new("graph.target", "records")
        )),
        "events: {:?}",
        calls.borrow()
    );

    calls.borrow_mut().clear();
    let invalid = harness.point_in(&port_id("graph.target", "done"));
    harness
        .context()
        .simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_move(invalid, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(invalid, MouseButton::Left, Modifiers::none());

    assert!(
        !calls
            .borrow()
            .iter()
            .any(|event| matches!(event, NodeGraphEvent::ConnectionRequested { .. }))
    );
}

#[gpui::test]
fn output_capture_delivers_an_outside_release(cx: &mut TestAppContext) {
    let (mut harness, calls) = editor(cx);
    let from = harness.point_in(&port_id("graph.source", "records"));
    let graph = harness.bounds("graph").expect("graph bounds");
    let outside = point(graph.right() + px(100.0), graph.bottom() + px(70.0));

    harness
        .context()
        .simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_move(outside, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    assert!(
        !calls
            .borrow()
            .iter()
            .any(|event| matches!(event, NodeGraphEvent::ConnectionRequested { .. })),
        "dropping outside proposes no connection"
    );

    calls.borrow_mut().clear();
    harness.context().simulate_event(ScrollWheelEvent {
        position: graph.center(),
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    assert!(
        calls
            .borrow()
            .iter()
            .any(|event| matches!(event, NodeGraphEvent::ViewportChanged(_))),
        "outside mouse-up releases capture and clears the port gesture"
    );
}
