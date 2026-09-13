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

fn port_name_id(node: &str, port: &str) -> String {
    format!("port-name:{}:{}:{}:{}", node.len(), node, port.len(), port)
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
fn cancelled_node_drag_emits_no_release_and_the_next_press_works(cx: &mut TestAppContext) {
    let (mut harness, calls, clicks) = controlled_editor(cx);
    let start = harness.point_in("controlled-graph.source");
    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    harness.context().simulate_event(gpui::MouseCancelEvent);
    harness.context().simulate_event(gpui::MouseCancelEvent);
    calls.borrow_mut().clear();
    harness.context().simulate_mouse_move(
        start + point(px(20.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    harness
        .context()
        .simulate_mouse_up(start, MouseButton::Left, Modifiers::none());
    assert!(calls.borrow().is_empty());
    assert_eq!(clicks.get(), 0);
    harness.click("controlled-graph.source");
    assert_eq!(clicks.get(), 1);
}

#[gpui::test]
fn controlled_drag_discards_deleted_peers_and_active_targets(cx: &mut TestAppContext) {
    let membership = Rc::new(Cell::new(2));
    let members = membership.clone();
    let calls = Calls::default();
    let sink = calls.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let mut graph = NodeGraph::new("live");
        if members.get() > 0 {
            graph = graph.node(GraphNode::new("a", "A").selected(true), 60.0, 60.0);
        }
        if members.get() > 1 {
            graph = graph.node(GraphNode::new("b", "B").selected(true), 400.0, 60.0);
        }
        let sink = sink.clone();
        div()
            .w(px(720.0))
            .h(px(360.0))
            .child(graph.on_event(move |event, _, _| sink.borrow_mut().push(event.clone())))
            .into_any_element()
    });
    harness.frame();
    let start = harness.point_in("a");
    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    membership.set(1);
    harness.update(|window, _| window.refresh());
    harness.frame();
    harness.context().simulate_mouse_move(
        start + point(px(30.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert!(
        calls
            .borrow()
            .iter()
            .any(|event| matches!(event, NodeGraphEvent::NodeMoved { id, .. } if id == "a"))
    );
    assert!(
        !calls
            .borrow()
            .iter()
            .any(|event| matches!(event, NodeGraphEvent::NodeMoved { id, .. } if id == "b"))
    );
    membership.set(0);
    harness.update(|window, _| window.refresh());
    harness.frame();
    calls.borrow_mut().clear();
    harness.context().simulate_mouse_move(
        start + point(px(50.0), px(30.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    harness
        .context()
        .simulate_mouse_up(start, MouseButton::Left, Modifiers::none());
    assert!(calls.borrow().is_empty());
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
fn node_notes_clamp_real_lines_preserve_full_semantics_and_scale(cx: &mut TestAppContext) {
    const TEXT: &str = "One line\n第二行\nThird line\n第四行\nFifth hidden line\n第六行";
    let expected = Rc::new(Cell::new(0.0));
    let measured = expected.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, cx| {
        let theme = cx.theme();
        measured.set(theme.typography.caption.line_height * 4.0 + theme.spacing.sm * 2.0);
        div()
            .flex()
            .children(
                [("full", 1.0), ("half", 0.5), ("compact", 0.3)].map(|(id, zoom)| {
                    div().w(px(320.0)).h(px(320.0)).child(
                        NodeGraph::new(format!("graph-{id}"))
                            .zoom_range(0.2, 2.0)
                            .viewport(GraphViewport::new(point(0.0, 0.0), zoom))
                            .node(GraphNode::new(id, "Note").width(260.0).note(TEXT), 0.0, 0.0),
                    )
                }),
            )
            .into_any_element()
    });
    harness.frame();
    harness.frame();
    let full = harness.node("full.note").expect("full note");
    assert_eq!(full.role, Role::Text);
    assert_eq!(full.description.as_deref(), Some(TEXT));
    let full = harness.bounds("full.note").expect("full note bounds");
    let half = harness.bounds("half.note").expect("half note bounds");
    assert!(
        (f32::from(full.size.height) - expected.get()).abs() < 1.0,
        "four shaped lines plus padding: {full:?}, expected {}",
        expected.get()
    );
    assert!((f32::from(full.size.height) - f32::from(half.size.height) * 2.0).abs() < 1.0);
    assert!((f32::from(full.size.width) - f32::from(half.size.width) * 2.0).abs() < 1.0);
    assert!(harness.node("compact.note").is_none());
}

#[derive(gpui::IntoElement)]
struct NodeThemeProbe(&'static str);

impl gpui::RenderOnce for NodeThemeProbe {
    fn render(self, _: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .w(px(theme.spacing.lg))
            .h(px(theme.typography.caption.line_height))
            .semantic_in(cx, NodeSpec::new(self.0, Role::Text))
    }
}

#[gpui::test]
fn node_note_height_follows_wrapping_and_the_requested_line_limit(cx: &mut TestAppContext) {
    let line_height = Rc::new(Cell::new(0.0));
    let expected = line_height.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, cx| {
        expected.set(cx.theme().typography.caption.line_height);
        div().flex().children([
            GraphNode::new("short", "Short").width(160.0).note("短句"),
            GraphNode::new("wrapped", "Wrapped").width(160.0)
                .note("这是一段没有空格的中文说明，用来检查文字自动换行后仍然只有两行可见，完整内容不被裁剪。")
                .note_lines(2),
            GraphNode::new("explicit", "Explicit").width(160.0)
                .note("First\n第二行\nThird").note_lines(2),
        ]).into_any_element()
    });
    harness.frame();
    let short = harness
        .bounds("short.note")
        .expect("short note bounds")
        .size
        .height;
    let wrapped = harness
        .bounds("wrapped.note")
        .expect("wrapped note bounds")
        .size
        .height;
    let explicit = harness
        .bounds("explicit.note")
        .expect("explicit note bounds")
        .size
        .height;
    assert_eq!(wrapped, explicit);
    assert_eq!(f32::from(wrapped - short), line_height.get());
}

#[gpui::test]
fn node_thumbnail_and_content_read_the_same_scaled_theme(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .flex()
            .children(
                [
                    ("full", "full-thumb", "full-body", 1.0),
                    ("half", "half-thumb", "half-body", 0.5),
                ]
                .map(|(id, thumb, body, zoom)| {
                    div().w(px(320.0)).h(px(320.0)).child(
                        NodeGraph::new(format!("graph-{id}"))
                            .viewport(GraphViewport::new(point(0.0, 0.0), zoom))
                            .node(
                                GraphNode::new(id, "Preview")
                                    .thumbnail(NodeThemeProbe(thumb))
                                    .child(NodeThemeProbe(body)),
                                0.0,
                                0.0,
                            ),
                    )
                }),
            )
            .into_any_element()
    });
    harness.frame();
    harness.frame();
    let full = harness
        .bounds("full-thumb")
        .expect("full thumbnail bounds")
        .size;
    let half = harness
        .bounds("half-thumb")
        .expect("half thumbnail bounds")
        .size;
    assert_eq!(
        full,
        harness
            .bounds("full-body")
            .expect("full content bounds")
            .size
    );
    assert_eq!(
        half,
        harness
            .bounds("half-body")
            .expect("half content bounds")
            .size
    );
    assert_eq!(full.width, half.width * 2.0);
    assert_eq!(full.height, half.height * 2.0);
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
    let row = harness
        .bounds(&port_name_id("measured-graph.node", "result"))
        .expect("port row bounds");
    assert!(f32::from(node.size.height) > 80.0, "node bounds: {node:?}");
    // The wrapped metrics push the card tall; the port stays on the row that
    // names it rather than drifting to the middle of whatever height the
    // content reached, and that row is inside the measured card.
    assert!(
        row.top() >= node.top() && row.bottom() <= node.bottom(),
        "row {row:?} sits inside measured node {node:?}"
    );
    assert!(
        (f32::from(port.center().y - row.center().y)).abs() < 1.0,
        "port {port:?} follows its row {row:?}"
    );
    assert!(
        (f32::from(port.center().x - node.right())).abs() < 1.0,
        "port {port:?} sits on the right edge of {node:?}"
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

#[gpui::test]
fn graph_culls_before_first_mount_and_uses_current_container_size(cx: &mut TestAppContext) {
    let width = Rc::new(Cell::new(200.));
    let far_mounts = Rc::new(Cell::new(0usize));
    let current_width = Rc::clone(&width);
    let mounts = Rc::clone(&far_mounts);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let mounts = Rc::clone(&mounts);
        div()
            .w(px(current_width.get()))
            .h(px(240.))
            .child(
                NodeGraph::new("bounded")
                    .placed(Placed::new(GraphNode::new("near", "Near"), 20., 20.).height(100.))
                    .placed(
                        Placed::new(
                            GraphNode::new("far", "Far").child(gpui::container_query(
                                move |_, _, _| {
                                    mounts.set(mounts.get() + 1);
                                    div().h(px(20.))
                                },
                            )),
                            1000.,
                            20.,
                        )
                        .height(100.),
                    ),
            )
            .into_any_element()
    });
    assert!(harness.node("near").is_some());
    assert!(harness.node("far").is_none());
    assert_eq!(far_mounts.get(), 0, "offscreen content never mounted");
    width.set(1300.);
    harness.frame();
    assert!(harness.node("far").is_some(), "resize applies this frame");
    assert!(far_mounts.get() > 0);
    width.set(200.);
    harness.frame();
    assert!(harness.node("far").is_none(), "shrunk targets are removed");
}

/// CPU test-platform work, separate from pure layout/routing workloads.
/// Explicit dimensions, edgeless or row-chain topology, no fit. The chain
/// cases are sparse, not evidence for dense 100k editor support.
#[gpui::test]
#[ignore = "explicit source mount/redraw/update evidence"]
fn graph_source_workload(cx: &mut TestAppContext) {
    for count in [1_000usize, 10_000, 100_000] {
        for connected in [false, true] {
            let placed = |i: usize, offset| {
                Placed::new(
                    GraphNode::new(format!("source.{i}"), "Fixture").width(120.),
                    (i % 20) as f32 * 200. + offset,
                    (i / 20) as f32 * 150.,
                )
                .height(80.)
            };
            let prepare = std::time::Instant::now();
            let source = GraphSource::new(
                (0..count).map(|i| placed(i, 0.)),
                (0..count)
                    .filter(|i| connected && i % 20 != 0)
                    .map(|i| GraphEdge::new(format!("source.{}", i - 1), format!("source.{i}"))),
            )
            .expect("source");
            let prepared = prepare.elapsed();
            let shown = source.clone();
            let start = std::time::Instant::now();
            let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
                div()
                    .w(px(640.))
                    .h(px(360.))
                    .child(
                        NodeGraph::new("source-workload")
                            .source(shown.clone())
                            .animate_layout(false)
                            .grid(false)
                            .axes(false)
                            .ground_light(false),
                    )
                    .into_any_element()
            });
            let mount = start.elapsed();
            harness.frame();
            let start = std::time::Instant::now();
            harness.frame();
            let redraw = start.elapsed();
            let start = std::time::Instant::now();
            source.upsert(placed(1, 23.)).expect("accepted node update");
            harness.frame();
            let update = start.elapsed();
            let stats = harness.frame_stats();
            assert!(stats.paint_calls < 2000);
            let bounds = harness.bounds("source.1").expect("updated visible node");
            let first = harness.bounds("source.0").expect("first visible node");
            assert!((f32::from(bounds.origin.x - first.origin.x) - 223.).abs() < 0.01);
            eprintln!(
                "source nodes={count} connected={connected} prepare={prepared:?} mount={mount:?} unchanged={redraw:?} one_node_update={update:?} paint_calls={}; CPU test platform, no GPU/FPS claim",
                stats.paint_calls
            );
            harness.update(|window, _| window.remove_window());
        }
    }
}

#[gpui::test]
#[ignore = "explicit graph mount/redraw evidence"]
fn graph_mount_workload(cx: &mut TestAppContext) {
    for (count, connected) in [
        (1_000usize, false),
        (10_000, false),
        (100_000, false),
        (1_000, true),
        (10_000, true),
        (100_000, true),
    ] {
        let edges = if connected {
            count - count.div_ceil(20)
        } else {
            0
        };
        let begin = std::time::Instant::now();
        let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
            let mut graph = NodeGraph::new("workload")
                .grid(false)
                .axes(false)
                .ground_light(false);
            for i in 0..count {
                graph = graph.placed(
                    Placed::new(
                        GraphNode::new(format!("node.{i}"), "Fixture").width(120.),
                        (i % 20) as f32 * 200.,
                        (i / 20) as f32 * 150.,
                    )
                    .height(80.),
                );
                if connected && i % 20 != 0 {
                    graph = graph.edge(GraphEdge::new(
                        format!("node.{}", i - 1),
                        format!("node.{i}"),
                    ));
                }
            }
            div()
                .w(px(640.))
                .h(px(360.))
                .child(graph)
                .into_any_element()
        });
        let mount = begin.elapsed();
        harness.frame(); // settle actual viewport measurements
        let begin = std::time::Instant::now();
        harness.frame();
        let redraw = begin.elapsed();
        let stats = harness.frame_stats();
        eprintln!(
            "graph nodes={count} edges={edges} test_platform_mount={mount:?} static_redraw={redraw:?} paint_calls={} prepaint_calls={}; includes builder/geometry scan; GPU submission not measured",
            stats.paint_calls, stats.prepaint_calls
        );
        assert!(
            stats.paint_calls < 2000,
            "settled viewport work must be culled"
        );
        // Harness::frame refreshes every open window. Drop alone leaves the
        // application window alive, contaminating the next dataset's timing.
        harness.update(|window, _| window.remove_window());
    }
}

#[gpui::test]
fn recorded_graph_card_keeps_paint_without_replaying_live_content(cx: &mut TestAppContext) {
    let live = Rc::new(Cell::new(true));
    let builds = Rc::new(Cell::new(0));
    let clicks = Rc::new(Cell::new(0));
    let replays = Rc::new(Cell::new(0));
    let replayed = replays.clone();
    let recording = Rc::new(RefCell::new(None::<gpui::PaintRecording>));
    let capture_error = Rc::new(RefCell::new(None));
    let (present, built, clicked, saved, errors) = (
        live.clone(),
        builds.clone(),
        clicks.clone(),
        recording.clone(),
        capture_error.clone(),
    );
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let live = present.get();
        let built = built.clone();
        let clicked = clicked.clone();
        let saved = saved.clone();
        let errors = errors.clone();
        let replayed = replayed.clone();
        div()
            .w(px(420.))
            .h(px(300.))
            .overflow_hidden()
            .child(
                gpui::canvas(
                    move |bounds, window, cx| {
                        if !live {
                            return None;
                        }
                        built.set(built.get() + 1);
                        let mut child = GraphNode::new("recorded-node", "Caller card")
                            .child(
                                div()
                                    .id("recorded-action")
                                    .w(px(90.))
                                    .h(px(27.))
                                    .child("Inspect")
                                    .on_click(move |_, _, _| clicked.set(clicked.get() + 1))
                                    .semantic_in(
                                        cx,
                                        NodeSpec::new("recorded-action", Role::Button),
                                    ),
                            )
                            .into_any_element();
                        child.prepaint_as_root(
                            bounds.origin,
                            bounds.size.map(gpui::AvailableSpace::Definite),
                            window,
                            cx,
                        );
                        Some(child)
                    },
                    move |bounds, child, window, cx| {
                        window.paint_layer(bounds, |window| {
                            if let Some(mut child) = child {
                                let mark = window.paint_mark();
                                child.paint(window, cx);
                                match window.record_paint_since(mark) {
                                    Ok(recording) => {
                                        *saved.borrow_mut() = Some(recording);
                                    }
                                    Err(error) => {
                                        saved.borrow_mut().take();
                                        *errors.borrow_mut() = Some(error);
                                    }
                                }
                            } else if let Some(recording) = saved.borrow().as_ref() {
                                window
                                    .paint_recording_with_offset(recording, point(px(19.), px(7.)))
                                    .expect("compatible graph replay");
                                replayed.set(replayed.get() + 1);
                            }
                        });
                    },
                )
                .size_full(),
            )
            .into_any_element()
    });
    assert_eq!(*capture_error.borrow(), None);
    assert!(
        recording.borrow().is_some(),
        "ordinary GraphNode must be recordable"
    );
    let button = harness
        .bounds("recorded-action")
        .expect("live caller action")
        .center();
    harness.click("recorded-action");
    assert_eq!(clicks.get(), 1);
    assert_eq!(*capture_error.borrow(), None);
    assert!(recording.borrow().is_some());
    live.set(false);
    harness.frame();
    assert!(replays.get() > 0, "retired frame must replay a recording");
    let retired_builds = builds.get();
    assert!(harness.node("recorded-action").is_none());
    assert!(harness.node("recorded-node").is_none());
    harness
        .context()
        .simulate_mouse_down(button, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(button, MouseButton::Left, Modifiers::none());
    harness.frame();
    assert_eq!(clicks.get(), 1, "frozen content has no live action");
    assert_eq!(
        builds.get(),
        retired_builds,
        "retirement never rebuilds opaque content"
    );
    recording.borrow_mut().take();
    harness.frame();
    assert!(harness.node("recorded-action").is_none());
}

#[gpui::test]
fn graph_source_mounts_only_visible_factories_and_publishes_updates_and_removal(
    cx: &mut TestAppContext,
) {
    let source = GraphSource::new(
        (0..10_000).map(|i| {
            Placed::new(
                GraphNode::new(format!("source.{i}"), "Canonical")
                    .width(151.)
                    .port(GraphPort::input("in", "Input")),
                i as f32 * 400.,
                23.,
            )
            .height(137.)
        }),
        [GraphEdge::new("source.0", "source.1").id("source.wire")],
    )
    .expect("finite source");
    let built = Rc::new(RefCell::new(
        std::collections::HashMap::<usize, usize>::new(),
    ));
    for i in 0..10_000 {
        let built = built.clone();
        source
            .set_content(&format!("source.{i}"), move |_, cx| {
                *built.borrow_mut().entry(i).or_default() += 1;
                div()
                    .child(format!("Body {i}"))
                    .semantic_in(cx, NodeSpec::new(format!("source.body.{i}"), Role::Group))
                    .into_any_element()
            })
            .expect("known identity");
    }
    let shown = source.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let log = events.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let log = log.clone();
        div()
            .w(px(600.))
            .h(px(280.))
            .child(
                NodeGraph::new("source.graph")
                    .source(shown.clone())
                    .animate_layout(false)
                    .fit(GraphFit::Whole(7))
                    .interaction(GraphInteraction::Edit)
                    .on_event(move |event, _, _| log.borrow_mut().push(event.clone())),
            )
            .into_any_element()
    });
    assert!(built.borrow().contains_key(&0));
    assert!(
        built.borrow().len() <= 3,
        "offscreen factories must not run"
    );
    assert!(!built.borrow().contains_key(&9999));
    assert!(events.borrow().iter().any(|event| matches!(event, NodeGraphEvent::ViewportChanged(viewport) if viewport.offset.x < -900_000.)), "Fit includes unmounted source geometry");
    assert!(harness.node(&port_id("source.0", "in")).is_some());
    assert!(
        harness.node(&port_id("source.9999", "in")).is_none(),
        "offscreen port controls must not mount"
    );
    assert_eq!(
        harness
            .node("source.0")
            .expect("canonical node")
            .text
            .as_deref(),
        Some("Canonical")
    );
    source
        .upsert(
            Placed::new(
                GraphNode::new("source.0", "Revised")
                    .width(193.)
                    .state(NodeState::Failed),
                37.,
                61.,
            )
            .height(173.),
        )
        .expect("metadata replacement");
    harness.frame();
    let bounds = harness.bounds("source.0").expect("updated node");
    assert_eq!(
        (
            f32::from(bounds.origin.x),
            f32::from(bounds.origin.y),
            f32::from(bounds.size.width),
            f32::from(bounds.size.height)
        ),
        (37., 61., 193., 173.)
    );
    assert_eq!(
        harness
            .node("source.0")
            .expect("updated node")
            .text
            .as_deref(),
        Some("Revised")
    );
    assert_eq!(
        harness
            .node("source.0")
            .expect("updated state")
            .value
            .as_deref(),
        Some("failed")
    );
    assert!(
        harness.node("source.body.0").is_some(),
        "upsert preserves lazy factory"
    );
    harness.drag_start("source.0");
    events.borrow_mut().clear();
    assert!(source.remove("source.0"));
    harness.frame();
    assert!(harness.node("source.0").is_none());
    assert!(harness.node("source.body.0").is_none());
    assert!(harness.update(|window, _| window.captured_hitbox().is_none()));
    harness.drop_here();
    assert!(
        events.borrow().is_empty(),
        "removed source cannot finish a stale gesture"
    );
    source
        .upsert(
            Placed::new(
                GraphNode::new("source.0", "Reinserted").width(113.),
                11.,
                17.,
            )
            .height(89.),
        )
        .expect("new incarnation");
    harness.frame();
    assert_eq!(
        harness
            .node("source.0")
            .expect("reinserted")
            .text
            .as_deref(),
        Some("Reinserted")
    );
    assert!(
        harness.node("source.body.0").is_none(),
        "removed factory never resurrects"
    );
}

#[gpui::test]
fn source_layout_retargets_displayed_cards_and_ports_but_not_caller_content(
    cx: &mut TestAppContext,
) {
    fn card(title: &'static str, x: f32, y: f32, w: f32, h: f32) -> Placed {
        Placed::new(
            GraphNode::new("moving-card", title)
                .width(w)
                .port(GraphPort::output("out", "Output").side(PortSide::Right)),
            x,
            y,
        )
        .height(h)
    }
    let source = GraphSource::new([card("First", 40., 50., 180., 130.)], []).expect("source");
    let shown = source.clone();
    let accept = Rc::new(Cell::new(true));
    let accepted = accept.clone();
    let moves = Rc::new(Cell::new(0));
    let moved = moves.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let source = shown.clone();
        let accepted = accepted.clone();
        let moved = moved.clone();
        div()
            .w(px(700.))
            .h(px(500.))
            .child(
                NodeGraph::new("moving-graph")
                    .source(shown.clone())
                    .interaction(GraphInteraction::Arrange)
                    .on_event(move |event, window, _| {
                        if let NodeGraphEvent::NodeMoved { position, .. } = event {
                            moved.set(moved.get() + 1);
                            if accepted.get() {
                                source
                                    .upsert(card(
                                        "Drag accepted",
                                        position.x,
                                        position.y,
                                        210.,
                                        150.,
                                    ))
                                    .expect("accepted position");
                            }
                            window.refresh();
                        }
                    }),
            )
            .into_any_element()
    });
    harness.update(|_, cx| cx.set_reduce_motion(false));
    let original = harness.bounds("moving-card").expect("first card");
    source
        .upsert(card("Current target", 310., 90., 270., 190.))
        .expect("new target");
    harness.frame();
    assert_eq!(harness.bounds("moving-card"), Some(original));
    assert_eq!(
        harness
            .node("moving-card")
            .expect("current content")
            .text
            .as_deref(),
        Some("Current target")
    );
    harness.advance(std::time::Duration::from_millis(80));
    let middle = harness.bounds("moving-card").expect("intermediate card");
    assert!(middle.left() > original.left() && middle.left() < px(310.));
    assert!(middle.size.width > original.size.width && middle.size.width < px(270.));
    let socket = harness
        .bounds(&port_id("moving-card", "out"))
        .expect("displayed port");
    assert!((f32::from(socket.center().x - middle.right())).abs() < 0.1);
    source
        .upsert(card("Retargeted now", 130., 240., 210., 150.))
        .expect("retarget");
    harness.frame();
    assert_eq!(harness.bounds("moving-card"), Some(middle));
    assert_eq!(
        harness
            .node("moving-card")
            .expect("retarget content")
            .text
            .as_deref(),
        Some("Retargeted now")
    );
    harness.update(|_, cx| cx.set_reduce_motion(true));
    harness.frame();
    let settled = harness
        .bounds("moving-card")
        .expect("reduced motion target");
    assert_eq!(settled.origin, point(px(130.), px(240.)));
    assert_eq!(settled.size, gpui::size(px(210.), px(150.)));
    harness.update(|_, cx| cx.set_reduce_motion(false));
    harness.drag_start("moving-card");
    harness.drag_to(settled.center() + point(px(37.), px(-19.)));
    harness.frame();
    let dragged = harness
        .bounds("moving-card")
        .expect("accepted direct position");
    assert_eq!(dragged.origin, settled.origin + point(px(37.), px(-19.)));
    harness.drop_here();
    assert!(moves.get() > 0, "real captured moves reached the caller");
    let dragged = harness
        .bounds("moving-card")
        .expect("released caller position");
    let port_before_refusal = harness
        .bounds(&port_id("moving-card", "out"))
        .expect("resting socket");
    let press_offset = harness.update(|_, cx| cx.theme().motion.press_offset);
    accept.set(false);
    let count = moves.get();
    harness.drag_start("moving-card");
    harness.drag_to(dragged.center() + point(px(-67.), px(43.)));
    harness.frame();
    assert!(moves.get() > count, "refused proposal reached the caller");
    // The socket follows its measured row through existing pressed feedback,
    // not the refused 67px/43px caller proposal.
    assert_eq!(
        harness
            .bounds(&port_id("moving-card", "out"))
            .expect("pressed socket")
            .origin,
        port_before_refusal.origin + point(px(0.), px(press_offset))
    );
    harness.drop_here();
    harness.advance(std::time::Duration::from_secs(1));
    assert_eq!(harness.bounds("moving-card"), Some(dragged));
}

#[gpui::test]
fn lane_transition_semantic_target_retargets_then_disabled_motion_settles(cx: &mut TestAppContext) {
    let lane = Rc::new(Cell::new(1));
    let shown = lane.clone();
    let animate = Rc::new(Cell::new(true));
    let animates = animate.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(700.))
            .h(px(450.))
            .child(
                NodeGraph::new("lane-graph")
                    .animate_layout(animates.get())
                    .placed(
                        Placed::new(GraphNode::new("a", "Source").width(100.), 40., 100.)
                            .height(80.),
                    )
                    .placed(
                        Placed::new(GraphNode::new("b", "Destination").width(100.), 500., 100.)
                            .height(80.),
                    )
                    .edge(GraphEdge::new("a", "b").id("lane").lane(shown.get())),
            )
            .into_any_element()
    });
    harness.update(|_, cx| cx.set_reduce_motion(false));
    harness.frame();
    let original = harness.bounds(&edge_id("lane")).expect("initial wire");
    lane.set(5);
    harness.frame();
    assert_eq!(harness.bounds(&edge_id("lane")), Some(original));
    harness.advance(std::time::Duration::from_millis(80));
    let middle = harness.bounds(&edge_id("lane")).expect("displayed wire");
    assert_ne!(middle, original);
    lane.set(3);
    harness.frame();
    assert_eq!(harness.bounds(&edge_id("lane")), Some(middle));
    animate.set(false);
    harness.frame();
    let settled = harness.bounds(&edge_id("lane")).expect("settled target");
    assert_ne!(settled, middle);
    animate.set(true);
    harness.frame();
    assert_eq!(harness.bounds(&edge_id("lane")), Some(settled));
    lane.set(1);
    harness.frame();
    harness.update(|_, cx| cx.set_reduce_motion(true));
    harness.frame();
    assert_eq!(harness.bounds(&edge_id("lane")), Some(original));
}
