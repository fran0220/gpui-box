//! NodeGraph proposes edits and keeps the caller's topology, positions and
//! viewport authoritative.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{
    Edges, Modifiers, MouseButton, ScrollDelta, ScrollWheelEvent, SharedString, TestAppContext,
    TouchPhase, div, point, prelude::*, px,
};
use gpui_kit::prelude::*;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
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

#[gpui::test]
fn graph_states_use_the_shared_state_surface_and_declared_slots(cx: &mut TestAppContext) {
    let mut defaults = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w(px(480.0))
            .h(px(240.0))
            .child(NodeGraph::new("failed-graph").state(GraphState::Failed("offline".into())))
            .into_any_element()
    });
    assert_eq!(
        defaults
            .node("failed-graph.state")
            .and_then(|node| node.value)
            .as_deref(),
        Some("error")
    );
    assert!(defaults.node("failed-graph.state.failed").is_some());

    let mut replacements = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .child(
                div().w(px(480.0)).h(px(240.0)).child(
                    NodeGraph::new("loading-graph")
                        .state(GraphState::Loading)
                        .slot(slot::LOADING, |_, _| {
                            Callout::new("Custom loading", Tone::Info)
                                .id("loading-graph.custom")
                                .into_any_element()
                        }),
                ),
            )
            .child(
                div().w(px(480.0)).h(px(240.0)).child(
                    NodeGraph::new("refused-graph")
                        .state(GraphState::Refused("policy".into()))
                        .slot(slot::EMPTY, |_, _| {
                            Callout::new("Custom refusal", Tone::Neutral)
                                .id("refused-graph.custom")
                                .into_any_element()
                        }),
                ),
            )
            .into_any_element()
    });
    assert!(replacements.node("loading-graph.custom").is_some());
    assert!(replacements.node("refused-graph.custom").is_some());
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

fn inspector(cx: &mut TestAppContext) -> (Harness, Calls) {
    let calls = Calls::default();
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        div()
            .w(px(560.0))
            .h(px(280.0))
            .child(
                NodeGraph::new("inspect")
                    .interaction(GraphInteraction::Inspect)
                    .node(
                        GraphNode::new("inspect.source", "Source")
                            .port(GraphPort::output("result", "Result")),
                        40.0,
                        60.0,
                    )
                    .node(
                        GraphNode::new("inspect.target", "Target")
                            .port(GraphPort::input("result", "Result")),
                        320.0,
                        60.0,
                    )
                    .edge(
                        GraphEdge::new("inspect.source", "inspect.target")
                            .id("inspect.edge")
                            .ports("result", "result"),
                    )
                    .on_event(move |event, _, _| sink.borrow_mut().push(event.clone())),
            )
            .into_any_element()
    });
    harness.frame();
    (harness, calls)
}

#[gpui::test]
fn graph_nodes_keep_distinct_execution_states(cx: &mut TestAppContext) {
    let states = [
        ("pending", NodeState::Pending, false, false),
        ("idle", NodeState::Idle, false, false),
        ("queued", NodeState::Queued, true, false),
        ("starting", NodeState::Starting, true, false),
        ("running", NodeState::Running, true, false),
        ("waiting", NodeState::Waiting, false, false),
        ("blocked", NodeState::Blocked, false, true),
        ("succeeded", NodeState::Succeeded, false, false),
        ("partial", NodeState::Partial, false, false),
        ("failed", NodeState::Failed, false, true),
        ("refused", NodeState::Refused, false, false),
        ("cancelling", NodeState::Cancelling, true, false),
        ("cancelled", NodeState::Cancelled, false, false),
        ("timed-out", NodeState::TimedOut, false, true),
        ("unavailable", NodeState::Unavailable, false, false),
    ];
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .column()
            .children(states.map(|(name, state, _, _)| {
                GraphNode::new(format!("state.{name}"), name).state(state)
            }))
            .into_any_element()
    });

    for (name, _, busy, invalid) in states {
        let node = harness
            .node(&format!("state.{name}"))
            .expect("state node is published");
        assert_eq!(node.value.as_deref(), Some(name));
        assert_eq!(node.busy, busy, "{name} busy state");
        assert_eq!(node.invalid, invalid, "{name} invalid state");
    }
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
fn inspect_mode_navigates_and_selects_without_editing_topology(cx: &mut TestAppContext) {
    let (mut harness, calls) = inspector(cx);

    let edge = harness
        .node(&edge_id("inspect.edge"))
        .expect("an inspected edge remains semantically visible");
    assert_eq!(edge.role, gpui_kit::semantics::Role::Group);
    assert_eq!(edge.text.as_deref(), Some("Connection"));
    assert_eq!(
        harness
            .node(&port_id("inspect.source", "result"))
            .expect("port remains visible")
            .role,
        gpui_kit::semantics::Role::Group,
        "an inspected port is information, not a connection handle"
    );

    harness.click("inspect.source");
    assert!(calls.borrow().iter().any(|event| matches!(
        event,
        NodeGraphEvent::SelectionChanged { ids } if ids == &["inspect.source"]
    )));

    calls.borrow_mut().clear();
    harness.keystrokes("delete");
    assert!(
        calls
            .borrow()
            .iter()
            .all(|event| !matches!(event, NodeGraphEvent::NodeDeleted { .. })),
        "inspection does not install the delete action"
    );

    let start = harness.point_in("inspect.source");
    let end = start + point(px(90.0), px(50.0));
    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    assert!(
        calls
            .borrow()
            .iter()
            .all(|event| !matches!(event, NodeGraphEvent::NodeMoved { .. })),
        "inspection does not propose a new layout"
    );
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

#[gpui::test]
fn canvas_toolbar_actions_share_pointer_keyboard_and_disabled_contracts(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let active_sink = Rc::clone(&calls);
    let disabled_sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let active_sink = Rc::clone(&active_sink);
        let disabled_sink = Rc::clone(&disabled_sink);
        div()
            .column()
            .child(
                CanvasToolbar::new("toolbar", "125%")
                    .glass(GlassPreset::Liquid)
                    .snap(true)
                    .on_action(move |action, _, _| active_sink.borrow_mut().push(action)),
            )
            .child(
                CanvasToolbar::new("toolbar.disabled", "100%")
                    .disabled(true)
                    .on_action(move |action, _, _| disabled_sink.borrow_mut().push(action)),
            )
            .child(CanvasToolbar::new("toolbar.read-only", "100%"))
            .into_any_element()
    });

    assert_eq!(
        harness.node("toolbar.fit").expect("fit").parent.as_deref(),
        Some("toolbar")
    );
    assert!(harness.node("toolbar.snap").expect("snap").checked == Some(true));
    for (id, key, action) in [
        ("toolbar.fit", "enter", CanvasToolbarAction::Fit),
        ("toolbar.snap", "space", CanvasToolbarAction::Snap),
        ("toolbar.arrange", "enter", CanvasToolbarAction::Arrange),
    ] {
        harness.click(id);
        calls.borrow_mut().clear();
        harness.keystrokes(key);
        assert_eq!(calls.borrow().as_slice(), [action]);
    }

    calls.borrow_mut().clear();
    let disabled = harness
        .node("toolbar.disabled.fit")
        .expect("disabled action remains published");
    assert!(disabled.disabled);
    harness.click("toolbar.disabled.fit");
    assert!(calls.borrow().is_empty());
    assert!(
        harness
            .node("toolbar.read-only.fit")
            .expect("read-only action remains published")
            .disabled,
        "an action without a caller-owned handler must not claim availability"
    );
}

#[gpui::test]
fn node_graph_seats_toolbar_and_frames_its_complete_world(cx: &mut TestAppContext) {
    let viewports = Calls::default();
    let actions = Rc::new(RefCell::new(Vec::new()));
    let viewport_sink = Rc::clone(&viewports);
    let action_sink = Rc::clone(&actions);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let viewport_sink = Rc::clone(&viewport_sink);
        let action_sink = Rc::clone(&action_sink);
        div()
            .w(px(720.0))
            .h(px(420.0))
            .child(
                NodeGraph::new("fit-graph")
                    .toolbar(
                        CanvasToolbar::new("fit-graph.toolbar", "100%")
                            .actions([CanvasToolbarAction::Fit])
                            .glass(GlassPreset::Liquid)
                            .on_action(move |action, _, _| action_sink.borrow_mut().push(action)),
                    )
                    .minimap(true)
                    .fit(GraphFit::Whole(7))
                    .band(GraphBand::new(
                        "fit-graph.band",
                        "Complete world",
                        -240.0,
                        -80.0,
                        1_360.0,
                        520.0,
                    ))
                    .node(GraphNode::new("fit-graph.node", "Node"), 120.0, 80.0)
                    .on_event(move |event, _, _| viewport_sink.borrow_mut().push(event.clone())),
            )
            .into_any_element()
    });

    // The graph waits for both the card and the finished toolbar subtree, then
    // proposes one caller-owned viewport for this token.
    for _ in 0..3 {
        harness.frame();
    }
    let proposed = viewports
        .borrow()
        .iter()
        .find_map(|event| match event {
            NodeGraphEvent::ViewportChanged(viewport) => Some(*viewport),
            _ => None,
        })
        .expect("the measured complete-world frame");
    assert!(
        proposed.zoom < 1.0,
        "the world-space band participates in the fit: {proposed:?}"
    );
    assert!(harness.node("fit-graph.minimap").is_some());
    assert_eq!(
        harness
            .node("fit-graph.toolbar.fit")
            .expect("the seated toolbar action")
            .parent
            .as_deref(),
        Some("fit-graph.toolbar")
    );
    harness.click("fit-graph.toolbar.fit");
    assert_eq!(actions.borrow().as_slice(), [CanvasToolbarAction::Fit]);
}

#[gpui::test]
fn fit_clearance_does_not_change_the_node_graph_semantic_tree(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w(px(720.0))
            .h(px(420.0))
            .child(
                NodeGraph::new("clearance-graph")
                    .fit(GraphFit::Whole(4))
                    .minimap(true)
                    .node(
                        GraphNode::new("clearance-graph.node", "Measured node"),
                        120.0,
                        80.0,
                    ),
            )
            .into_any_element()
    });
    let plain = harness.snapshot();

    harness.remount(|_, _| {
        div()
            .w(px(720.0))
            .h(px(420.0))
            .child(
                NodeGraph::new("clearance-graph")
                    .fit(GraphFit::Whole(4))
                    .fit_clearance(Edges {
                        top: 32.0,
                        right: 180.0,
                        bottom: 64.0,
                        left: 240.0,
                    })
                    .minimap(true)
                    .node(
                        GraphNode::new("clearance-graph.node", "Measured node"),
                        120.0,
                        80.0,
                    ),
            )
            .into_any_element()
    });

    assert_eq!(harness.snapshot().nodes, plain.nodes);
}

#[gpui::test]
fn minimap_pointer_and_keyboard_pan_report_normalized_caller_owned_points(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        Minimap::new("minimap")
            .marks([MinimapMark::new("node", 0.2, 0.3, 0.1, 0.1)])
            .view(MinimapView::new(0.4, 0.4, 0.2, 0.2))
            .on_pan(move |x, y, _, _| sink.borrow_mut().push((x, y)))
            .into_any_element()
    });
    harness.frame();

    let minimap = harness.node("minimap").expect("minimap");
    assert_eq!(minimap.role, Role::Slider);
    assert_eq!(
        minimap.value.as_deref(),
        Some("Horizontal 50%; vertical 50%.")
    );
    assert_eq!(
        (minimap.value_min, minimap.value_max, minimap.value_now),
        (Some(0.0), Some(1.0), Some(0.5))
    );
    let mark = harness.node("minimap.mark.node").expect("business mark");
    assert_eq!(mark.text.as_deref(), Some("node"));

    let bounds = harness.bounds("minimap").expect("measured minimap");
    let pointer = point(
        bounds.left() + bounds.size.width * 0.75,
        bounds.top() + bounds.size.height * 0.25,
    );
    harness
        .context()
        .simulate_mouse_down(pointer, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(pointer, MouseButton::Left, Modifiers::none());
    let (x, y) = calls.borrow()[0];
    assert!((x - 0.75).abs() < 0.01, "pointer x: {x}");
    assert!((y - 0.25).abs() < 0.01, "pointer y: {y}");

    calls.borrow_mut().clear();
    harness.click("minimap");
    calls.borrow_mut().clear();
    harness.keystrokes("right down");
    assert_eq!(calls.borrow().len(), 2);
    assert_eq!(calls.borrow()[0], (0.55, 0.5));
    assert_eq!(calls.borrow()[1], (0.5, 0.55));
    assert_eq!(
        harness
            .node("minimap")
            .expect("caller-owned view")
            .value
            .as_deref(),
        Some("Horizontal 50%; vertical 50%."),
        "the component reports pan requests and applies none"
    );
}

#[gpui::test]
fn node_group_publishes_its_boundary_selection_and_child_relationship(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, cx| {
        div()
            .w(px(320.0))
            .child(
                NodeGroup::new("group", "Ingest").selected(true).child(
                    div().w(px(120.0)).h(px(48.0)).semantic_in(
                        cx,
                        NodeSpec::new("group.member", Role::Group)
                            .parent("group")
                            .text("Member"),
                    ),
                ),
            )
            .into_any_element()
    });
    harness.frame();

    let group = harness.node("group").expect("group boundary");
    assert_eq!(group.role, Role::Group);
    assert_eq!(group.text.as_deref(), Some("Ingest"));
    assert!(group.selected);
    assert_eq!(
        harness
            .node("group.member")
            .expect("member")
            .parent
            .as_deref(),
        Some("group")
    );
    let group_bounds = harness.bounds("group").expect("group bounds");
    let child_bounds = harness.bounds("group.member").expect("child bounds");
    assert!(group_bounds.left() <= child_bounds.left());
    assert!(group_bounds.right() >= child_bounds.right());
    assert!(group_bounds.top() <= child_bounds.top());
    assert!(group_bounds.bottom() >= child_bounds.bottom());
}
