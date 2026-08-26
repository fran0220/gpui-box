//! A graph the pointer arranges.

use super::support::*;

#[derive(Debug)]
pub(super) struct SceneGraph {
    viewport: GraphViewport,
    ingest: gpui::Point<f32>,
    validate: gpui::Point<f32>,
    persist: gpui::Point<f32>,
    observe: gpui::Point<f32>,
    publish: gpui::Point<f32>,
    selected: Vec<SharedString>,
    deleted: Vec<SharedString>,
    edges: Vec<GraphEdge>,
}

impl Global for SceneGraph {}

/// A live processing graph with the visual states and editor gestures shown
/// together: traffic lanes, a running shockwave, explicit ports, labels,
/// feedback routing, pan, zoom, node movement, and connection creation.
pub(super) fn node_graph(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneGraph>() {
        cx.set_global(SceneGraph {
            viewport: GraphViewport::default(),
            ingest: gpui::point(24.0, 190.0),
            validate: gpui::point(244.0, 40.0),
            persist: gpui::point(492.0, 40.0),
            observe: gpui::point(452.0, 300.0),
            publish: gpui::point(648.0, 190.0),
            selected: vec!["scene.graph.validate".into()],
            deleted: Vec::new(),
            edges: vec![
                GraphEdge::new("scene.graph.ingest", "scene.graph.validate")
                    .id("scene.graph.edge.rows")
                    .ports("rows", "records")
                    .label("12.4k rows")
                    .active(true),
                GraphEdge::new("scene.graph.validate", "scene.graph.persist")
                    .id("scene.graph.edge.valid")
                    .ports("valid", "records")
                    .active(true),
                GraphEdge::new("scene.graph.validate", "scene.graph.observe")
                    .id("scene.graph.edge.telemetry")
                    .ports("telemetry", "events")
                    .label("telemetry")
                    .lane(-1),
                GraphEdge::new("scene.graph.persist", "scene.graph.publish")
                    .id("scene.graph.edge.commit")
                    .ports("commit", "artifact")
                    .label("commit 8f72")
                    .active(true),
                GraphEdge::new("scene.graph.observe", "scene.graph.validate")
                    .id("scene.graph.edge.retry")
                    .ports("retry", "retry")
                    .label("retry policy")
                    .lane(1)
                    .feedback(),
            ],
        });
    }
    let scene = cx.global::<SceneGraph>();
    let viewport = scene.viewport;
    let ingest = scene.ingest;
    let validate = scene.validate;
    let persist = scene.persist;
    let observe = scene.observe;
    let publish = scene.publish;
    let selected = scene.selected.clone();
    let deleted = scene.deleted.clone();
    let edges = scene.edges.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .child(
            div().w(px(860.0)).h(px(470.0)).child(
                NodeGraph::new("scene.graph")
                    .viewport(viewport)
                    .zoom_range(0.55, 1.8)
                    .minimap(true)
                    .when(
                        !deleted.iter().any(|id| id == "scene.graph.ingest"),
                        |graph| {
                            graph.node(
                                GraphNode::new("scene.graph.ingest", "Stream ingest")
                                    .color("teal")
                                    .width(176.0)
                                    .state(NodeState::Succeeded)
                                    .thumbnail(scene_picture("Input preview", cx))
                                    .action("orders.v2 · partition 18")
                                    .metric("rate", "3.2k/s")
                                    .port(GraphPort::input("source", "Source").side(PortSide::Top))
                                    .port(GraphPort::output("rows", "Rows"))
                                    .port(GraphPort::output("errors", "Errors"))
                                    .selected(selected.iter().any(|id| id == "scene.graph.ingest")),
                                ingest.x,
                                ingest.y,
                            )
                        },
                    )
                    .when(
                        !deleted.iter().any(|id| id == "scene.graph.validate"),
                        |graph| {
                            graph.node(
                                GraphNode::new("scene.graph.validate", "Validate & enrich")
                                    .color("indigo")
                                    .width(176.0)
                                    .state(NodeState::Running)
                                    .action("schema + fraud signals")
                                    .metric("p95", "18 ms")
                                    .port(GraphPort::input("records", "Records"))
                                    .port(GraphPort::input("retry", "Retry").side(PortSide::Bottom))
                                    .port(GraphPort::output("valid", "Valid"))
                                    .port(
                                        GraphPort::output("telemetry", "Events")
                                            .side(PortSide::Bottom),
                                    )
                                    .selected(
                                        selected.iter().any(|id| id == "scene.graph.validate"),
                                    ),
                                validate.x,
                                validate.y,
                            )
                        },
                    )
                    .when(
                        !deleted.iter().any(|id| id == "scene.graph.persist"),
                        |graph| {
                            graph.node(
                                GraphNode::new("scene.graph.persist", "Persist batch")
                                    .color("violet")
                                    .width(176.0)
                                    .state(NodeState::Succeeded)
                                    .action("warehouse / orders")
                                    .metric("written", "12.3k")
                                    .port(GraphPort::input("records", "Records"))
                                    .port(GraphPort::output("commit", "Commit"))
                                    .selected(
                                        selected.iter().any(|id| id == "scene.graph.persist"),
                                    ),
                                persist.x,
                                persist.y,
                            )
                        },
                    )
                    .when(
                        !deleted.iter().any(|id| id == "scene.graph.observe"),
                        |graph| {
                            graph.node(
                                GraphNode::new("scene.graph.observe", "Observe quality")
                                    .color("orange")
                                    .width(176.0)
                                    .state(NodeState::Failed)
                                    .action("drift threshold exceeded")
                                    .metric("rejected", "94")
                                    .port(GraphPort::input("events", "Events").side(PortSide::Top))
                                    .port(GraphPort::output("retry", "Retry").side(PortSide::Top))
                                    .selected(
                                        selected.iter().any(|id| id == "scene.graph.observe"),
                                    ),
                                observe.x,
                                observe.y,
                            )
                        },
                    )
                    .when(
                        !deleted.iter().any(|id| id == "scene.graph.publish"),
                        |graph| {
                            graph.node(
                                GraphNode::new("scene.graph.publish", "Publish artifact")
                                    .color("lime")
                                    .width(176.0)
                                    .state(NodeState::Pending)
                                    .action("waiting for commit")
                                    .port(
                                        GraphPort::input("artifact", "Artifact")
                                            .side(PortSide::Top),
                                    )
                                    .port(
                                        GraphPort::output("release", "Release")
                                            .side(PortSide::Bottom),
                                    )
                                    .selected(
                                        selected.iter().any(|id| id == "scene.graph.publish"),
                                    ),
                                publish.x,
                                publish.y,
                            )
                        },
                    )
                    .edges(edges)
                    .on_event(|event, _, cx| {
                        cx.update_global::<SceneGraph, ()>(|scene, _| match event {
                            NodeGraphEvent::ViewportChanged(viewport) => {
                                scene.viewport = *viewport;
                            }
                            NodeGraphEvent::SelectionChanged { ids } => {
                                scene.selected = ids.clone();
                            }
                            NodeGraphEvent::NodeMoved { id, position } => match id.as_ref() {
                                "scene.graph.ingest" => scene.ingest = *position,
                                "scene.graph.validate" => scene.validate = *position,
                                "scene.graph.persist" => scene.persist = *position,
                                "scene.graph.observe" => scene.observe = *position,
                                "scene.graph.publish" => scene.publish = *position,
                                _ => {}
                            },
                            NodeGraphEvent::NodeDeleted { id } => {
                                scene.deleted.push(id.clone());
                                scene.selected.retain(|selected| selected != id);
                                scene
                                    .edges
                                    .retain(|edge| edge.from() != id && edge.to() != id);
                            }
                            NodeGraphEvent::ConnectionRequested { from, to } => {
                                let id = format!(
                                    "scene.graph.edge.user.{}.{}.{}.{}",
                                    from.node, from.port, to.node, to.port
                                );
                                if !scene
                                    .edges
                                    .iter()
                                    .any(|edge| edge.from() == &from.node && edge.to() == &to.node)
                                {
                                    scene.edges.push(
                                        GraphEdge::new(from.node.clone(), to.node.clone())
                                            .id(id)
                                            .ports(from.port.clone(), to.port.clone())
                                            .label("new connection"),
                                    );
                                }
                            }
                            NodeGraphEvent::DisconnectRequested { id } => {
                                scene.edges.retain(|edge| edge.edge_id() != *id);
                            }
                        });
                        cx.refresh_windows();
                    }),
            ),
        )
        .child(
            div()
                .row()
                .w(px(860.0))
                .gap_token(&theme, Space::Sm)
                .children([
                    div()
                        .flex_1()
                        .h(px(132.0))
                        .child(NodeGraph::new("scene.graph.loading").state(GraphState::Loading)),
                    div().flex_1().h(px(132.0)).child(
                        NodeGraph::new("scene.graph.refused")
                            .state(GraphState::Refused("Graph access was refused".into())),
                    ),
                    div().flex_1().h(px(132.0)).child(
                        NodeGraph::new("scene.graph.failed")
                            .state(GraphState::Failed("Graph data could not be loaded".into())),
                    ),
                ]),
        )
        .into_any_element()
}

pub(super) fn canvas_tools(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let member = |label: &'static str| {
        div()
            .w(px(150.0))
            .p_token(&theme, Space::Sm)
            .card_surface(&theme, CardVariant::Outlined)
            .type_scale(&theme, TypeScale::Caption)
            .text_tone(&theme, TextTone::Primary)
            .child(label)
    };
    let members = |left: &'static str, right: &'static str| {
        div()
            .row()
            .gap_token(&theme, Space::Sm)
            .child(member(left))
            .child(member(right))
    };
    stack(&theme)
        .w(px(560.0))
        .child(caption(
            &theme,
            "overview, chrome, and a named region; the host still owns pan and zoom",
        ))
        .child(
            CanvasToolbar::new("scene.canvas.toolbar", "125%")
                .snap(true)
                .on_action(|_, _, _| {}),
        )
        .child(
            Minimap::new("scene.canvas.minimap")
                .marks([
                    MinimapMark::new("ingest", 0.08, 0.42, 0.18, 0.16),
                    MinimapMark::new("validate", 0.36, 0.18, 0.20, 0.16),
                    MinimapMark::new("publish", 0.68, 0.48, 0.18, 0.16),
                ])
                .view(MinimapView::new(0.22, 0.20, 0.40, 0.36))
                .on_pan(|_, _, _, _| {}),
        )
        .child(
            NodeGroup::new("scene.canvas.group", "Ingest")
                .selected(true)
                .child(members("Stream ingest", "Validate & enrich")),
        )
        .child(
            NodeGroup::new("scene.canvas.group.quiet", "Observe")
                .child(members("Observe quality", "Publish artifact")),
        )
        .into_any_element()
}
