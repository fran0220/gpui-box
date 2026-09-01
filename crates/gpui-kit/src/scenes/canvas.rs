//! A graph the pointer arranges.

use super::support::*;

#[derive(Debug)]
pub(super) struct SceneGraph {
    viewport: GraphViewport,
    fit: u64,
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
/// together: traffic lanes, a running aura, explicit ports, labels,
/// feedback routing, pan, zoom, node movement, and connection creation.
pub(super) fn node_graph(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneGraph>() {
        cx.set_global(SceneGraph {
            viewport: GraphViewport::default(),
            fit: 0,
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
                    .label("批量导入行 · src/ingest/rows.ts")
                    .active(true),
                GraphEdge::new("scene.graph.validate", "scene.graph.persist")
                    .id("scene.graph.edge.valid")
                    .ports("valid", "records")
                    .label("校验通过的记录 · src/validate/ok.ts")
                    .active(true),
                // A relationship label carrying a localized sentence, which
                // is what a host writing in Chinese actually hands this: the
                // ideographs advance about twice what a count of characters
                // suggests, and the sentence is longer than the route it
                // annotates. Fit has to frame the width the label really
                // takes, and the label has to stop rather than run out over
                // the cards either side of it.
                GraphEdge::new("scene.graph.validate", "scene.graph.observe")
                    .id("scene.graph.edge.telemetry")
                    .ports("telemetry", "events")
                    .label("跟随镜头 · src/camera/follow.ts")
                    .lane(-1),
                GraphEdge::new("scene.graph.persist", "scene.graph.publish")
                    .id("scene.graph.edge.commit")
                    .ports("commit", "artifact")
                    .label("提交产物快照 · src/publish/commit.ts")
                    .active(true),
                // A second long localized label, on the route that runs back
                // through the same band as the one above. Two annotations
                // competing for the same air is the case a real board hits
                // constantly and this catalogue did not cover: with one long
                // label the seating always found somewhere to put it, so
                // nothing here ever exercised what happens when it cannot.
                GraphEdge::new("scene.graph.observe", "scene.graph.validate")
                    .id("scene.graph.edge.retry")
                    .ports("retry", "retry")
                    .label("跳跃核心循环 · src/player/jump.ts")
                    .lane(1)
                    .feedback(),
            ],
        });
    }
    let scene = cx.global::<SceneGraph>();
    let viewport = scene.viewport;
    let fit = scene.fit;
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
                    .toolbar(
                        CanvasToolbar::new(
                            "scene.graph.toolbar",
                            cx.numbers().percent(viewport.zoom),
                        )
                        // The editor already owns direct wheel zoom and pan.
                        // Fit is the one extra navigation intent this fixture
                        // can keep without inventing arrangement or snapping
                        // policy.
                        .actions([CanvasToolbarAction::Fit])
                        .glass(GlassPreset::Frosted)
                        .on_action(|_, _, cx| {
                            cx.update_global::<SceneGraph, ()>(|scene, _| {
                                scene.fit = scene.fit.wrapping_add(1);
                            });
                            cx.refresh_windows();
                        }),
                    )
                    .fit(GraphFit::Whole(fit))
                    .when(
                        !deleted.iter().any(|id| id == "scene.graph.ingest"),
                        |graph| {
                            graph.node(
                                GraphNode::new("scene.graph.ingest", "Stream ingest")
                                    .color("teal")
                                    .icon(Icon::SoundWave)
                                    .width(176.0)
                                    .state(NodeState::Succeeded)
                                    .status("Synced")
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
                                    .icon(Icon::Checklist)
                                    .width(176.0)
                                    .state(NodeState::Running)
                                    .status("Running")
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
                                    .icon(Icon::ArchiveUp)
                                    .width(176.0)
                                    .state(NodeState::Succeeded)
                                    .status("Committed")
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
                                    .icon(Icon::Danger)
                                    .width(176.0)
                                    .state(NodeState::Failed)
                                    .status("Needs review")
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
                                    .icon(Icon::Global)
                                    .width(176.0)
                                    .state(NodeState::Pending)
                                    .status("Queued")
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
                            NodeGraphEvent::ConnectionDropped { .. } => {}
                            // This scene has no way to add a step, so there is
                            // nothing truthful to do with a place on it.
                            NodeGraphEvent::SurfacePressed { .. } => {}
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
        .child(caption(
            &theme,
            "the same canvas routed as curves, for a board sparse enough not to need lanes",
        ))
        .child(
            div().w(px(860.0)).h(px(212.0)).child(
                NodeGraph::new("scene.graph.curves")
                    .routing(GraphRouting::Curves)
                    .grid(false)
                    .node(
                        GraphNode::new("scene.graph.curves.brief", "Brief")
                            .color("indigo")
                            .kind("prompt")
                            .width(176.0)
                            .action("a lit interior, dusk")
                            .port(GraphPort::output("out", "Out")),
                        24.0,
                        44.0,
                    )
                    .node(
                        GraphNode::new("scene.graph.curves.render", "Render")
                            .color("teal")
                            .kind("image")
                            .width(176.0)
                            .state(NodeState::Succeeded)
                            .port(GraphPort::input("in", "In"))
                            .port(GraphPort::output("out", "Out")),
                        330.0,
                        20.0,
                    )
                    .node(
                        GraphNode::new("scene.graph.curves.grade", "Grade")
                            .color("orange")
                            .kind("adjust")
                            .width(176.0)
                            .action("warm, +8 contrast")
                            .port(GraphPort::input("in", "In")),
                        636.0,
                        84.0,
                    )
                    .edges(vec![
                        GraphEdge::new("scene.graph.curves.brief", "scene.graph.curves.render")
                            .id("scene.graph.curves.edge.render")
                            .ports("out", "in")
                            .marker(EdgeMarker::Arrow),
                        GraphEdge::new("scene.graph.curves.render", "scene.graph.curves.grade")
                            .id("scene.graph.curves.edge.grade")
                            .ports("out", "in")
                            .marker(EdgeMarker::Dot),
                    ]),
            ),
        )
        .into_any_element()
}

const NODE_STATES: [(NodeState, &str, &str); 15] = [
    (NodeState::Pending, "Pending", "pending"),
    (NodeState::Idle, "Idle", "idle"),
    (NodeState::Queued, "Queued", "queued"),
    (NodeState::Starting, "Starting", "starting"),
    (NodeState::Running, "Running", "running"),
    (NodeState::Waiting, "Waiting", "waiting"),
    (NodeState::Blocked, "Blocked", "blocked"),
    (NodeState::Succeeded, "Succeeded", "succeeded"),
    (NodeState::Partial, "Partial", "partial"),
    (NodeState::Failed, "Failed", "failed"),
    (NodeState::Refused, "Refused", "refused"),
    (NodeState::Cancelling, "Cancelling", "cancelling"),
    (NodeState::Cancelled, "Cancelled", "cancelled"),
    (NodeState::TimedOut, "Timed out", "timed-out"),
    (NodeState::Unavailable, "Unavailable", "unavailable"),
];

#[derive(Debug)]
pub(super) struct SceneGraphMotion {
    state: usize,
    viewport: GraphViewport,
    fit: u64,
}

impl Global for SceneGraphMotion {}

/// Every node state beside every edge state, plus one stable node identity a
/// reader can retarget to inspect OKLab crossover and the successful handoff.
pub(super) fn node_graph_motion(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneGraphMotion>() {
        cx.set_global(SceneGraphMotion {
            state: 4,
            viewport: GraphViewport::new(gpui::point(20.0, 12.0), 1.0),
            fit: 0,
        });
    }
    let theme = cx.theme().clone();
    let motion_scene = cx.global::<SceneGraphMotion>();
    let state_index = motion_scene.state;
    let viewport = motion_scene.viewport;
    let fit = motion_scene.fit;
    let (state, state_label, _) = NODE_STATES[state_index];
    let state_cards = NODE_STATES.into_iter().map(|(state, label, slug)| {
        div().w(px(164.0)).child(
            GraphNode::new(format!("scene.graph-motion.state.{slug}"), label)
                .width(164.0)
                .state(state)
                .action(slug),
        )
    });

    stack(&theme)
        .w(px(920.0))
        .child(caption(
            &theme,
            "state owns every aura; only Running breathes, and a successful handoff flashes once",
        ))
        .child(
            div()
                .row()
                .items_center()
                .gap_token(&theme, Space::Md)
                .child(
                    GraphNode::new("scene.graph-motion.transition", "Observed transition")
                        .width(210.0)
                        .state(state)
                        .action(state_label),
                )
                .child(
                    Button::new("scene.graph-motion.next-state")
                        .label("Next state")
                        .secondary()
                        .on_click(|_, cx| {
                            cx.update_global::<SceneGraphMotion, ()>(|scene, _| {
                                scene.state = (scene.state + 1) % NODE_STATES.len();
                            });
                            cx.refresh_windows();
                        }),
                )
                .child(
                    Button::new("scene.graph-motion.succeed")
                        .label("Complete successfully")
                        .on_click(|_, cx| {
                            cx.update_global::<SceneGraphMotion, ()>(|scene, _| {
                                scene.state = 7;
                            });
                            cx.refresh_windows();
                        }),
                ),
        )
        .child(
            div()
                .row()
                .flex_wrap()
                .items_start()
                .gap_token(&theme, Space::Sm)
                .children(state_cards),
        )
        .child(caption(
            &theme,
            "every route grades source to destination; hover a route, and compare the selected succeeded route with the active flow",
        ))
        .child(
            div()
                .w(px(880.0))
                .h(px(420.0))
                .child(
                    NodeGraph::new("scene.graph-motion.edges")
                    .interaction(GraphInteraction::Inspect)
                    .viewport(viewport)
                    .toolbar(
                        CanvasToolbar::new(
                            "scene.graph-motion.toolbar",
                            cx.numbers().percent(viewport.zoom),
                        )
                        .actions([CanvasToolbarAction::Fit])
                        .glass(GlassPreset::Frosted)
                        .on_action(|_, _, cx| {
                            cx.update_global::<SceneGraphMotion, ()>(|scene, _| {
                                scene.fit = scene.fit.wrapping_add(1);
                            });
                            cx.refresh_windows();
                        }),
                    )
                    .fit(GraphFit::Whole(fit))
                    .node(
                        GraphNode::new("scene.graph-motion.source", "Source")
                            .width(164.0)
                            .state(NodeState::Running)
                            .port(GraphPort::output("idle", "Idle"))
                            .port(GraphPort::output("active", "Active"))
                            .port(GraphPort::output("succeeded", "Succeeded"))
                            .port(GraphPort::output("failed", "Failed")),
                        12.0,
                        132.0,
                    )
                    .node(
                        GraphNode::new("scene.graph-motion.idle", "Idle route")
                            .width(164.0)
                            .state(NodeState::Idle)
                            .port(GraphPort::input("in", "In")),
                        648.0,
                        8.0,
                    )
                    .node(
                        GraphNode::new("scene.graph-motion.active", "Traffic crossing")
                            .width(164.0)
                            .state(NodeState::Running)
                            .port(GraphPort::input("in", "In")),
                        648.0,
                        104.0,
                    )
                    .node(
                        GraphNode::new("scene.graph-motion.succeeded", "Delivered")
                            .width(164.0)
                            .state(NodeState::Succeeded)
                            .port(GraphPort::input("in", "In")),
                        648.0,
                        200.0,
                    )
                    .node(
                        GraphNode::new("scene.graph-motion.failed", "Delivery failed")
                            .width(164.0)
                            .state(NodeState::Failed)
                            .port(GraphPort::input("in", "In")),
                        648.0,
                        296.0,
                    )
                    .edges([
                        GraphEdge::new("scene.graph-motion.source", "scene.graph-motion.idle")
                            .id("scene.graph-motion.edge.idle")
                            .ports("idle", "in")
                            .label("idle")
                            .lane(-3)
                            .state(EdgeState::Idle),
                        GraphEdge::new("scene.graph-motion.source", "scene.graph-motion.active")
                            .id("scene.graph-motion.edge.active")
                            .ports("active", "in")
                            .label("active · flowing")
                            .lane(-1)
                            .state(EdgeState::Active),
                        GraphEdge::new(
                            "scene.graph-motion.source",
                            "scene.graph-motion.succeeded",
                        )
                        .id("scene.graph-motion.edge.succeeded")
                        .ports("succeeded", "in")
                        .label("succeeded · selected")
                        .lane(1)
                        .state(EdgeState::Succeeded)
                        .selected(true),
                        GraphEdge::new("scene.graph-motion.source", "scene.graph-motion.failed")
                            .id("scene.graph-motion.edge.failed")
                            .ports("failed", "in")
                            .label("failed")
                            .lane(3)
                            .state(EdgeState::Failed),
                    ])
                    .on_event(|event, _, cx| {
                        if let NodeGraphEvent::ViewportChanged(viewport) = event {
                            cx.update_global::<SceneGraphMotion, ()>(|scene, _| {
                                scene.viewport = *viewport;
                            });
                            cx.refresh_windows();
                        }
                    }),
                ),
        )
        .into_any_element()
}

const CANVAS_TOOLS_FRAME_WIDTH: f32 = 660.0;
const CANVAS_TOOLS_FRAME_HEIGHT: f32 = 420.0;
const CANVAS_TOOLS_WORLD_WIDTH: f32 = 1_280.0;
const CANVAS_TOOLS_WORLD_HEIGHT: f32 = 700.0;

#[derive(Debug)]
pub(super) struct SceneCanvasTools {
    viewport: GraphViewport,
    fit: u64,
    snap: bool,
    selected: Vec<SharedString>,
    nodes: [gpui::Point<f32>; 4],
}

impl Global for SceneCanvasTools {}

fn arranged_canvas_tools_nodes() -> [gpui::Point<f32>; 4] {
    [
        gpui::point(40.0, 240.0),
        gpui::point(360.0, 60.0),
        gpui::point(360.0, 430.0),
        gpui::point(1_000.0, 240.0),
    ]
}

fn canvas_tools_minimap_view(viewport: GraphViewport) -> MinimapView {
    let zoom = viewport.zoom.max(f32::EPSILON);
    let width = (CANVAS_TOOLS_FRAME_WIDTH / zoom / CANVAS_TOOLS_WORLD_WIDTH).clamp(0.04, 1.0);
    let height = (CANVAS_TOOLS_FRAME_HEIGHT / zoom / CANVAS_TOOLS_WORLD_HEIGHT).clamp(0.04, 1.0);
    let center_x =
        (CANVAS_TOOLS_FRAME_WIDTH / 2.0 - viewport.offset.x) / zoom / CANVAS_TOOLS_WORLD_WIDTH;
    let center_y =
        (CANVAS_TOOLS_FRAME_HEIGHT / 2.0 - viewport.offset.y) / zoom / CANVAS_TOOLS_WORLD_HEIGHT;
    MinimapView::new(
        (center_x - width / 2.0).clamp(0.0, 1.0 - width),
        (center_y - height / 2.0).clamp(0.0, 1.0 - height),
        width,
        height,
    )
}

fn canvas_tools_viewport_at(viewport: GraphViewport, x: f32, y: f32) -> GraphViewport {
    GraphViewport::new(
        gpui::point(
            CANVAS_TOOLS_FRAME_WIDTH / 2.0
                - x.clamp(0.0, 1.0) * CANVAS_TOOLS_WORLD_WIDTH * viewport.zoom,
            CANVAS_TOOLS_FRAME_HEIGHT / 2.0
                - y.clamp(0.0, 1.0) * CANVAS_TOOLS_WORLD_HEIGHT * viewport.zoom,
        ),
        viewport.zoom,
    )
}

fn canvas_tools_snap(position: gpui::Point<f32>) -> gpui::Point<f32> {
    const STEP: f32 = 24.0;
    gpui::point(
        (position.x / STEP).round() * STEP,
        (position.y / STEP).round() * STEP,
    )
}

pub(super) fn canvas_tools(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneCanvasTools>() {
        cx.set_global(SceneCanvasTools {
            viewport: GraphViewport::default(),
            fit: 0,
            snap: true,
            selected: vec!["scene.canvas.graph.validate".into()],
            nodes: arranged_canvas_tools_nodes(),
        });
    }
    let scene = cx.global::<SceneCanvasTools>();
    let viewport = scene.viewport;
    let fit = scene.fit;
    let snap = scene.snap;
    let selected = scene.selected.clone();
    let [ingest, validate, observe, publish] = scene.nodes;
    let theme = cx.theme().clone();
    let graph_node =
        |id: &'static str, title: &'static str, color: &'static str, state: NodeState| {
            GraphNode::new(id, title)
                .color(color)
                .width(180.0)
                .state(state)
                .port(GraphPort::input("in", "In"))
                .port(GraphPort::output("out", "Out"))
                .selected(selected.iter().any(|selected| selected == id))
        };
    let minimap_mark = |id: &'static str, position: gpui::Point<f32>, color: &'static str| {
        MinimapMark::new(
            id,
            position.x / CANVAS_TOOLS_WORLD_WIDTH,
            position.y / CANVAS_TOOLS_WORLD_HEIGHT,
            180.0 / CANVAS_TOOLS_WORLD_WIDTH,
            120.0 / CANVAS_TOOLS_WORLD_HEIGHT,
        )
        .color(color)
    };
    let member = |id: &'static str,
                  title: &'static str,
                  color: &'static str,
                  state: NodeState,
                  action: &'static str,
                  metric: (&'static str, &'static str)| {
        GraphNode::new(id, title)
            .color(color)
            .width(236.0)
            .state(state)
            .action(action)
            .metric(metric.0, metric.1)
    };

    stack(&theme)
        .w(px(840.0))
        .child(caption(
            &theme,
            "the overview, glass chrome, snap, arrange, and fit all act on this controlled canvas",
        ))
        .child(
            div()
                .row()
                .items_start()
                .gap_token(&theme, Space::Md)
                .child(
                    div()
                        .w(px(CANVAS_TOOLS_FRAME_WIDTH))
                        .h(px(CANVAS_TOOLS_FRAME_HEIGHT))
                        .child(
                            NodeGraph::new("scene.canvas.graph")
                                .viewport(viewport)
                                .toolbar(
                                    CanvasToolbar::new(
                                        "scene.canvas.toolbar",
                                        cx.numbers().percent(viewport.zoom),
                                    )
                                    .snap(snap)
                                    .glass(GlassPreset::Frosted)
                                    .on_action(|action, _, cx| {
                                        cx.update_global::<SceneCanvasTools, ()>(
                                            |scene, _| match action {
                                                CanvasToolbarAction::Fit => {
                                                    scene.fit = scene.fit.wrapping_add(1);
                                                }
                                                CanvasToolbarAction::Snap => {
                                                    scene.snap = !scene.snap;
                                                }
                                                CanvasToolbarAction::Arrange => {
                                                    scene.nodes = arranged_canvas_tools_nodes();
                                                    scene.fit = scene.fit.wrapping_add(1);
                                                }
                                            },
                                        );
                                        cx.refresh_windows();
                                    }),
                                )
                                .fit(GraphFit::Whole(fit))
                                .interaction(GraphInteraction::Arrange)
                                .node(
                                    graph_node(
                                        "scene.canvas.graph.ingest",
                                        "Stream ingest",
                                        "teal",
                                        NodeState::Succeeded,
                                    ),
                                    ingest.x,
                                    ingest.y,
                                )
                                .node(
                                    graph_node(
                                        "scene.canvas.graph.validate",
                                        "Validate & enrich",
                                        "indigo",
                                        NodeState::Running,
                                    ),
                                    validate.x,
                                    validate.y,
                                )
                                .node(
                                    graph_node(
                                        "scene.canvas.graph.observe",
                                        "Observe quality",
                                        "orange",
                                        NodeState::Failed,
                                    ),
                                    observe.x,
                                    observe.y,
                                )
                                .node(
                                    graph_node(
                                        "scene.canvas.graph.publish",
                                        "Publish artifact",
                                        "lime",
                                        NodeState::Pending,
                                    ),
                                    publish.x,
                                    publish.y,
                                )
                                .edges([
                                    GraphEdge::new(
                                        "scene.canvas.graph.ingest",
                                        "scene.canvas.graph.validate",
                                    )
                                    .ports("out", "in")
                                    .active(true),
                                    GraphEdge::new(
                                        "scene.canvas.graph.validate",
                                        "scene.canvas.graph.observe",
                                    )
                                    .ports("out", "in"),
                                    GraphEdge::new(
                                        "scene.canvas.graph.observe",
                                        "scene.canvas.graph.publish",
                                    )
                                    .ports("out", "in"),
                                ])
                                .on_event(|event, _, cx| {
                                    cx.update_global::<SceneCanvasTools, ()>(
                                        |scene, _| match event {
                                            NodeGraphEvent::ViewportChanged(viewport) => {
                                                scene.viewport = *viewport;
                                            }
                                            NodeGraphEvent::SelectionChanged { ids } => {
                                                scene.selected = ids.clone();
                                            }
                                            NodeGraphEvent::NodeMoved { id, position } => {
                                                let position = if scene.snap {
                                                    canvas_tools_snap(*position)
                                                } else {
                                                    *position
                                                };
                                                match id.as_ref() {
                                                    "scene.canvas.graph.ingest" => {
                                                        scene.nodes[0] = position;
                                                    }
                                                    "scene.canvas.graph.validate" => {
                                                        scene.nodes[1] = position;
                                                    }
                                                    "scene.canvas.graph.observe" => {
                                                        scene.nodes[2] = position;
                                                    }
                                                    "scene.canvas.graph.publish" => {
                                                        scene.nodes[3] = position;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            NodeGraphEvent::NodeDeleted { .. }
                                            | NodeGraphEvent::SurfacePressed { .. }
                                            | NodeGraphEvent::ConnectionRequested { .. }
                                            | NodeGraphEvent::ConnectionDropped { .. }
                                            | NodeGraphEvent::DisconnectRequested { .. } => {}
                                        },
                                    );
                                    cx.refresh_windows();
                                }),
                            )
                )
                .child(
                    div()
                        .w(px(160.0))
                        .flex_none()
                        .child(
                            Minimap::new("scene.canvas.minimap")
                                .marks([
                                    minimap_mark("ingest", ingest, "teal"),
                                    minimap_mark("validate", validate, "indigo"),
                                    minimap_mark("observe", observe, "orange"),
                                    minimap_mark("publish", publish, "lime"),
                                ])
                                .view(canvas_tools_minimap_view(viewport))
                                .on_pan(|x, y, _, cx| {
                                    cx.update_global::<SceneCanvasTools, ()>(|scene, _| {
                                        scene.viewport =
                                            canvas_tools_viewport_at(scene.viewport, x, y);
                                    });
                                    cx.refresh_windows();
                                }),
                        ),
                ),
        )
        .child(caption(
            &theme,
            "a NodeGroup remains host layout: it names these related cards, not a world-space region",
        ))
        .child(
            div().relative().w(px(540.0)).child(
                NodeGroup::new("scene.canvas.group", "Ingest")
                    .selected(true)
                    .child(
                        div()
                            .row()
                            .items_start()
                            .gap_token(&theme, Space::Md)
                            .child(
                                member(
                                    "scene.canvas.node.ingest",
                                    "Stream ingest",
                                    "teal",
                                    NodeState::Succeeded,
                                    "orders.v2 · partition 18",
                                    ("rate", "3.2k/s"),
                                )
                                .into_any_element(),
                            )
                            .child(
                                member(
                                    "scene.canvas.node.validate",
                                    "Validate & enrich",
                                    "indigo",
                                    NodeState::Running,
                                    "schema + fraud signals",
                                    ("p95", "18 ms"),
                                )
                                .into_any_element(),
                            ),
                    ),
            ),
        )
        .into_any_element()
}

/// Two named regions of one canvas, an inspect-only toolbar beside them, and
/// the return path that crosses both.
///
/// The three things it exists to review are the three a host cannot fake.
/// A region is a rectangle of the graph's own world, so it pans and zooms
/// with the cards it encloses and a host cannot draw one by overlaying a box.
/// A toolbar on a canvas that arranges nothing offers only the intent it can
/// keep. And a return path is drawn as a return rather than as a failure, so
/// a run that retried and then succeeded is not reported as broken.
#[derive(Debug)]
pub(super) struct SceneCanvasRegions {
    viewport: GraphViewport,
    fit: u64,
}

impl Global for SceneCanvasRegions {}

pub(super) fn canvas_regions(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneCanvasRegions>() {
        cx.set_global(SceneCanvasRegions {
            viewport: GraphViewport::new(gpui::point(28.0, 18.0), 1.0),
            fit: 0,
        });
    }
    let scene = cx.global::<SceneCanvasRegions>();
    let viewport = scene.viewport;
    let fit = scene.fit;
    let theme = cx.theme().clone();
    let card = |id: &'static str,
                title: &'static str,
                color: &'static str,
                state: NodeState,
                action: &'static str| {
        GraphNode::new(id, title)
            .color(color)
            .width(168.0)
            .state(state)
            .action(action)
            .port(GraphPort::input("in", "In"))
            .port(GraphPort::output("out", "Out"))
    };
    stack(&theme)
        .child(caption(
            &theme,
            "regions live in canvas coordinates; the chrome offers only what this host can do",
        ))
        .child(
            div().w(px(660.0)).h(px(360.0)).child(
                NodeGraph::new("scene.regions")
                    .viewport(viewport)
                    .interaction(GraphInteraction::Inspect)
                    .toolbar(
                        CanvasToolbar::new(
                            "scene.regions.toolbar",
                            cx.numbers().percent(viewport.zoom),
                        )
                        // Inspect-only: this host frames the canvas
                        // and rearranges nothing, so it names the one
                        // intent it can carry out.
                        .actions([CanvasToolbarAction::Fit])
                        .glass(GlassPreset::Frosted)
                        .on_action(|_, _, cx| {
                            cx.update_global::<SceneCanvasRegions, ()>(|scene, _| {
                                scene.fit = scene.fit.wrapping_add(1);
                            });
                            cx.refresh_windows();
                        }),
                    )
                    .fit(GraphFit::Whole(fit))
                    .band(
                        GraphBand::new(
                            "scene.regions.baseline",
                            "Baseline",
                            0.0,
                            0.0,
                            560.0,
                            132.0,
                        )
                        .color("teal"),
                    )
                    .band(
                        GraphBand::new("scene.regions.scope", "Scope", 0.0, 168.0, 560.0, 132.0)
                            .color("violet")
                            .selected(true),
                    )
                    .node(
                        card(
                            "scene.regions.sample",
                            "Sample",
                            "teal",
                            NodeState::Succeeded,
                            "1,204 cases",
                        ),
                        28.0,
                        34.0,
                    )
                    .node(
                        card(
                            "scene.regions.score",
                            "Score",
                            "teal",
                            NodeState::Succeeded,
                            "reference model",
                        ),
                        340.0,
                        34.0,
                    )
                    .node(
                        card(
                            "scene.regions.candidate",
                            "Candidate",
                            "violet",
                            NodeState::Running,
                            "checkpoint 41",
                        ),
                        28.0,
                        202.0,
                    )
                    .node(
                        card(
                            "scene.regions.compare",
                            "Compare",
                            "violet",
                            NodeState::Partial,
                            "3 of 8 rubrics",
                        ),
                        340.0,
                        202.0,
                    )
                    .edge(
                        GraphEdge::new("scene.regions.sample", "scene.regions.score")
                            .ports("out", "in")
                            .active(true),
                    )
                    .edge(
                        GraphEdge::new("scene.regions.candidate", "scene.regions.compare")
                            .ports("out", "in")
                            .label("8 rubrics")
                            .active(true),
                    )
                    // A return path: the comparison sent work back to
                    // be scored again. It is a fact about the run's
                    // control flow, so it is not drawn as a failure.
                    .edge(
                        GraphEdge::new("scene.regions.compare", "scene.regions.score")
                            .ports("out", "in")
                            .label("re-score")
                            .lane(1)
                            .feedback()
                            .active(true),
                    )
                    .on_event(|event, _, cx| {
                        if let NodeGraphEvent::ViewportChanged(viewport) = event {
                            cx.update_global::<SceneCanvasRegions, ()>(|scene, _| {
                                scene.viewport = *viewport;
                            });
                            cx.refresh_windows();
                        }
                    }),
            ),
        )
        .into_any_element()
}
