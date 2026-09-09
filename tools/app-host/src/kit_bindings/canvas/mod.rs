//! Native controlled canvas builders. All coordinates and graph topology belong
//! to the caller; GPUI Kit owns transforms, routing, selection and hit testing.
use super::{Emit, KitSlots, Node};
use gpui::{AnyElement, App, Edges, IntoElement, MouseButton, ParentElement, Window, div, point};
use gpui_kit::prelude::*;
use gpui_kit_theme::{ColorChoice, SemanticColor};
use serde_json::{Value, json};

pub(super) const COMPONENTS: &[&str] = &[
    "CanvasToolbar",
    "GraphNode",
    "Minimap",
    "NodeGraph",
    "NodeGroup",
];

/// Cross-field checks after the shared closed grammar has validated types.
pub(super) fn validate_props(node: &Node) -> anyhow::Result<()> {
    use anyhow::ensure;
    if node.component.as_deref() != Some("NodeGraph") {
        return Ok(());
    }
    let p = Value::Object(node.props.clone());
    if let Some(range) = p.get("zoom_range") {
        ensure!(
            n(range, "min", 0.1) <= n(range, "max", 4.),
            "invalid graph zoom range"
        );
    }
    let nodes: std::collections::HashMap<_, _> =
        items(&p, "nodes").map(|v| (s(v, "id"), v)).collect();
    let mut identities = std::collections::HashSet::new();
    for key in ["nodes", "edges", "bands"] {
        for v in items(&p, key) {
            ensure!(identities.insert(s(v, "id")), "duplicate graph identity");
        }
    }
    let port = |id: &str, name: &str| {
        nodes
            .get(id)
            .and_then(|v| items(&v["props"], "ports").find(|port| port["id"] == name))
    };
    for e in items(&p, "edges") {
        ensure!(
            nodes.contains_key(&s(e, "from")) && nodes.contains_key(&s(e, "to")),
            "unknown graph endpoint node"
        );
        if let Some(ports) = e.get("ports") {
            ensure!(
                port(&s(e, "from"), &s(ports, "from")).is_some()
                    && port(&s(e, "to"), &s(ports, "to")).is_some(),
                "unknown graph endpoint port"
            );
        }
    }
    for pair in items(&p, "can_connect") {
        ensure!(
            port(&s(&pair["from"], "node"), &s(&pair["from"], "port"))
                .is_some_and(|p| p["direction"] == "output")
                && port(&s(&pair["to"], "node"), &s(&pair["to"], "port"))
                    .is_some_and(|p| p["direction"] == "input"),
            "connection must be output to input"
        );
    }
    Ok(())
}

fn s(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_owned()
}
fn n(v: &Value, key: &str, default: f32) -> f32 {
    v[key].as_f64().map_or(default, |n| n as f32)
}
fn b(v: &Value, key: &str, default: bool) -> bool {
    v[key].as_bool().unwrap_or(default)
}
fn items<'a>(v: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    v[key].as_array().into_iter().flatten()
}
fn content(slots: &KitSlots, name: &str, window: &mut Window, cx: &mut App) -> AnyElement {
    slots
        .get(name)
        .map_or_else(|| div().into_any_element(), |factory| factory(window, cx))
}
pub(super) fn glass(v: &Value) -> GlassPreset {
    match v.as_str() {
        Some("frosted") => GlassPreset::Frosted,
        Some("clear") => GlassPreset::Clear,
        Some("lens") => GlassPreset::Lens,
        _ => GlassPreset::Liquid,
    }
}
pub(super) fn color(v: &Value) -> ColorChoice {
    if let Some(palette) = v["palette"].as_str() {
        return ColorChoice::Palette(palette.to_owned().into());
    }
    if v.get("hsla").is_some() {
        let c = &v["hsla"];
        return ColorChoice::Custom(gpui::hsla(
            n(c, "h", 0.),
            n(c, "s", 0.),
            n(c, "l", 0.),
            n(c, "a", 1.),
        ));
    }
    ColorChoice::Semantic(match v["semantic"].as_str().unwrap_or_default() {
        "success" => SemanticColor::Success,
        "warning" => SemanticColor::Warning,
        "danger" => SemanticColor::Danger,
        "info" => SemanticColor::Info,
        "accent_strong" => SemanticColor::AccentStrong,
        _ => SemanticColor::Accent,
    })
}
fn toolbar(id: &str, p: &Value, action: Option<String>, emit: Emit) -> CanvasToolbar {
    let mut c = CanvasToolbar::new(id.to_owned(), s(p, "zoom"))
        .snap(b(p, "snap", false))
        .disabled(b(p, "disabled", false));
    if p.get("actions").is_some() {
        c = c.actions(items(p, "actions").map(|v| match v.as_str() {
            Some("snap") => CanvasToolbarAction::Snap,
            Some("arrange") => CanvasToolbarAction::Arrange,
            _ => CanvasToolbarAction::Fit,
        }));
    }
    if let Some(v) = p.get("glass") {
        c = c.glass(glass(v));
    }
    if !b(p, "disabled", false)
        && let Some(action) = action
    {
        c = c.on_action(move |value, _, _| emit(&action, json!(value.name())));
    }
    c
}
fn graph_node(
    id: &str,
    p: &Value,
    slots: &KitSlots,
    window: &mut Window,
    cx: &mut App,
    click: Option<String>,
    emit: Emit,
) -> GraphNode {
    let mut c = GraphNode::new(id.to_owned(), s(p, "title")).selected(b(p, "selected", false));
    c = c.state(match p["state"].as_str().unwrap_or_default() {
        "idle" => NodeState::Idle,
        "queued" => NodeState::Queued,
        "starting" => NodeState::Starting,
        "running" => NodeState::Running,
        "waiting" => NodeState::Waiting,
        "blocked" => NodeState::Blocked,
        "succeeded" => NodeState::Succeeded,
        "partial" => NodeState::Partial,
        "failed" => NodeState::Failed,
        "refused" => NodeState::Refused,
        "cancelling" => NodeState::Cancelling,
        "cancelled" => NodeState::Cancelled,
        "timed_out" => NodeState::TimedOut,
        "unavailable" => NodeState::Unavailable,
        _ => NodeState::Pending,
    });
    if let Some(v) = p.get("icon") {
        c = c.icon(super::icon::resolve(v).expect("validated builtin icon"));
    }
    if let Some(v) = p.get("color") {
        c = c.color(color(v));
    }
    if let Some(v) = p.get("active_glass") {
        c = c.active_glass(glass(v));
    }
    if p.get("action").is_some() {
        c = c.action(s(p, "action"));
    }
    if p.get("kind").is_some() {
        c = c.kind(s(p, "kind"));
    }
    if p.get("note").is_some() {
        c = c.note(s(p, "note"));
    }
    if let Some(v) = p["note_lines"].as_u64() {
        c = c.note_lines(v as usize);
    }
    if p.get("status").is_some() {
        c = c.status(s(p, "status"));
    }
    if p.get("width").is_some() {
        c = c.width(n(p, "width", 240.));
    }
    if p.get("thumbnail_ratio").is_some() {
        c = c.thumbnail_ratio(n(p, "thumbnail_ratio", 1.));
    }
    if p.get("progress").is_some() {
        c = c.progress(p["progress"].as_f64().map(|v| v as f32));
    }
    if let Some(v) = p.get("diff") {
        c = c.diff(Diff::new(
            v["added"].as_u64().unwrap_or(0) as usize,
            v["removed"].as_u64().unwrap_or(0) as usize,
        ));
    }
    c = c.metrics(items(p, "metrics").map(|v| {
        let metric = NodeMetric::new(s(v, "label"), s(v, "value"));
        if b(v, "labelled", false) {
            metric.labelled()
        } else {
            metric
        }
    }));
    c = c.ports(items(p, "ports").map(|v| {
        let mut port = if s(v, "direction") == "output" {
            GraphPort::output(s(v, "id"), s(v, "label"))
        } else {
            GraphPort::input(s(v, "id"), s(v, "label"))
        };
        if let Some(side) = v["side"].as_str() {
            port = port.side(match side {
                "top" => PortSide::Top,
                "bottom" => PortSide::Bottom,
                "left" => PortSide::Left,
                _ => PortSide::Right,
            });
        }
        if let Some(t) = v.get("typed") {
            let mut tpe = PortType::new(s(t, "id"), color(&t["color"]));
            if let Some(icon) = t.get("glyph") {
                tpe = tpe.glyph(super::icon::resolve(icon).expect("validated builtin icon"));
            }
            port = port.typed(tpe);
        }
        port
    }));
    if slots.contains_key("thumbnail") {
        c = c.thumbnail(content(slots, "thumbnail", window, cx));
    }
    if slots.contains_key("content") {
        c = c.child(content(slots, "content", window, cx));
    }
    if !b(p, "disabled", false)
        && let Some(action) = click
    {
        c = c.on_click(move |_, _| emit(&action, Value::Null));
    }
    c
}
fn endpoint(v: &GraphEndpoint) -> Value {
    json!({"node":v.node.as_ref(),"port":v.port.as_ref()})
}
fn event(v: &NodeGraphEvent) -> Value {
    match v {
        NodeGraphEvent::ViewportChanged(v) => {
            json!({"type":"viewport_changed","offset":{"x":v.offset.x,"y":v.offset.y},"zoom":v.zoom})
        }
        NodeGraphEvent::SelectionChanged { ids } => {
            json!({"type":"selection_changed","ids":ids.iter().map(|id|id.as_ref()).collect::<Vec<_>>()})
        }
        NodeGraphEvent::NodeMoved { id, position } => {
            json!({"type":"node_moved","id":id.as_ref(),"position":{"x":position.x,"y":position.y}})
        }
        NodeGraphEvent::NodeResized { id, size } => {
            json!({"type":"node_resized","id":id.as_ref(),"size":{"width":size.width,"height":size.height}})
        }
        NodeGraphEvent::NodeDeleted { id } => json!({"type":"node_deleted","id":id.as_ref()}),
        NodeGraphEvent::SurfacePressed {
            position,
            button,
            click_count,
        } => {
            json!({"type":"surface_pressed","position":{"x":position.x,"y":position.y},"button": match button { MouseButton::Left => "left",MouseButton::Right => "right",MouseButton::Middle => "middle",MouseButton::Navigate(gpui::NavigationDirection::Back) => "back",MouseButton::Navigate(gpui::NavigationDirection::Forward) => "forward"},"click_count":click_count})
        }
        NodeGraphEvent::ConnectionRequested { from, to } => {
            json!({"type":"connection_requested","from":endpoint(from),"to":endpoint(to)})
        }
        NodeGraphEvent::ConnectionDropped { from, at } => {
            json!({"type":"connection_dropped","from":endpoint(from),"at":{"x":at.x,"y":at.y}})
        }
        NodeGraphEvent::DisconnectRequested { id } => {
            json!({"type":"disconnect_requested","id":id.as_ref()})
        }
    }
}

pub(super) fn render(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
) -> AnyElement {
    let p = Value::Object(node.props.clone());
    let action = |name: &str| {
        (!b(&p, "disabled", false))
            .then(|| node.events.get(name).cloned())
            .flatten()
    };
    match node.component.as_deref().unwrap_or_default() {
        "CanvasToolbar" => toolbar(&node.id, &p, action("action"), emit).into_any_element(),
        "GraphNode" => {
            graph_node(&node.id, &p, &slots, window, cx, action("click"), emit).into_any_element()
        }
        "NodeGroup" => NodeGroup::new(node.id.clone(), s(&p, "label"))
            .selected(b(&p, "selected", false))
            .child(content(&slots, "content", window, cx))
            .into_any_element(),
        "Minimap" => {
            let mut c = Minimap::new(node.id.clone()).marks(items(&p, "marks").map(|v| {
                let c = MinimapMark::new(
                    s(v, "id"),
                    n(v, "x", 0.),
                    n(v, "y", 0.),
                    n(v, "width", 1.),
                    n(v, "height", 1.),
                );
                v.get("color").map_or(c.clone(), |v| c.color(color(v)))
            }));
            if let Some(v) = p.get("view") {
                c = c.view(MinimapView::new(
                    n(v, "x", 0.),
                    n(v, "y", 0.),
                    n(v, "width", 1.),
                    n(v, "height", 1.),
                ));
            }
            if let Some(action) = action("pan") {
                c = c.on_pan(move |x, y, _, _| emit(&action, json!({"x":x,"y":y})));
            }
            c.into_any_element()
        }
        "NodeGraph" => {
            let mut c = NodeGraph::new(node.id.clone())
                .grid(b(&p, "grid", true))
                .axes(b(&p, "axes", false))
                .ground_light(b(&p, "ground_light", false))
                .minimap(b(&p, "minimap", false));
            if let Some(v) = p.get("empty") {
                let mut empty = EmptyState::new(format!("{}.empty", node.id), s(v, "title")).kind(
                    match v["kind"].as_str() {
                        Some("unstarted") => EmptyKind::Unstarted,
                        Some("queued") => EmptyKind::Queued,
                        Some("blocked") => EmptyKind::Blocked,
                        Some("cancelled") => EmptyKind::Cancelled,
                        Some("unavailable") => EmptyKind::Unavailable,
                        Some("failed") => EmptyKind::Failed,
                        Some("unauthorized") => EmptyKind::Unauthorized,
                        _ => EmptyKind::Empty,
                    },
                );
                if v.get("detail").is_some() {
                    empty = empty.detail(s(v, "detail"));
                }
                if let Some(icon) = v.get("icon") {
                    empty = empty.icon(super::icon::resolve(icon).expect("validated icon"));
                }
                if let Some(factory) = slots.get("empty_action") {
                    empty = empty.action(factory(window, cx));
                }
                c = c.empty(empty);
            }
            for v in items(&p, "nodes") {
                let id = s(v, "id");
                let mut child_slots = KitSlots::new();
                for name in ["content", "thumbnail"] {
                    if let Some(factory) = slots.get(&format!("{id}:{name}")) {
                        child_slots.insert(name.to_owned(), factory.clone());
                    }
                }
                let mut child = graph_node(
                    &id,
                    &v["props"],
                    &child_slots,
                    window,
                    cx,
                    None,
                    emit.clone(),
                );
                if !b(&v["props"], "disabled", false)
                    && let Some(action) = action("node_click")
                {
                    let emit = emit.clone();
                    child = child.on_click(move |_, _| emit(&action, json!(id)));
                }
                let mut placed = Placed::new(child, n(v, "x", 0.), n(v, "y", 0.));
                if v.get("height").is_some() {
                    placed = placed.height(n(v, "height", 1.));
                }
                c = c.placed(placed);
            }
            for v in items(&p, "edges") {
                let mut e = GraphEdge::new(s(v, "from"), s(v, "to"))
                    .id(s(v, "id"))
                    .selected(b(v, "selected", false));
                if v.get("ports").is_some() {
                    e = e.ports(s(&v["ports"], "from"), s(&v["ports"], "to"));
                }
                if v.get("label").is_some() {
                    e = e.label(s(v, "label"));
                }
                if v.get("active").is_some() {
                    e = e.active(b(v, "active", false));
                }
                if let Some(state) = v["state"].as_str() {
                    e = e.state(match state {
                        "active" => EdgeState::Active,
                        "succeeded" => EdgeState::Succeeded,
                        "failed" => EdgeState::Failed,
                        _ => EdgeState::Idle,
                    });
                }
                e = e.marker(match v["marker"].as_str() {
                    Some("dot") => EdgeMarker::Dot,
                    Some("arrow") => EdgeMarker::Arrow,
                    _ => EdgeMarker::None,
                });
                if let Some(lane) = v["lane"].as_i64() {
                    e = e.lane(lane as i16);
                }
                if b(v, "feedback", false) {
                    e = e.feedback();
                }
                if let Some(v) = v.get("color") {
                    e = e.color(color(v));
                }
                c = c.edge(e);
            }
            for v in items(&p, "bands") {
                let mut band = GraphBand::new(
                    s(v, "id"),
                    s(v, "label"),
                    n(v, "x", 0.),
                    n(v, "y", 0.),
                    n(v, "width", 1.),
                    n(v, "height", 1.),
                )
                .selected(b(v, "selected", false));
                if let Some(v) = v.get("color") {
                    band = band.color(color(v));
                }
                c = c.band(band);
            }
            if let Some(v) = p.get("state") {
                c = c.state(match v["kind"].as_str() {
                    Some("loading") => GraphState::Loading,
                    Some("refused") => GraphState::Refused(s(v, "reason").into()),
                    Some("failed") => GraphState::Failed(s(v, "reason").into()),
                    _ => GraphState::Ready,
                });
            }
            if let Some(v) = p.get("viewport") {
                c = c.viewport(GraphViewport::new(
                    point(n(&v["offset"], "x", 0.), n(&v["offset"], "y", 0.)),
                    n(v, "zoom", 1.),
                ));
            }
            if let Some(v) = p.get("offset") {
                c = c.offset(n(v, "x", 0.), n(v, "y", 0.));
            }
            if let Some(v) = p.get("zoom").and_then(Value::as_f64) {
                c = c.zoom(v as f32);
            }
            if let Some(v) = p.get("zoom_range") {
                c = c.zoom_range(n(v, "min", 0.1), n(v, "max", 4.));
            }
            if let Some(v) = p["fit"].as_u64() {
                c = c.fit(GraphFit::Whole(v));
            }
            if let Some(v) = p.get("fit_clearance") {
                c = c.fit_clearance(Edges {
                    top: n(v, "top", 0.),
                    right: n(v, "right", 0.),
                    bottom: n(v, "bottom", 0.),
                    left: n(v, "left", 0.),
                });
            }
            c = c.interaction(match p["interaction"].as_str() {
                Some("inspect") => GraphInteraction::Inspect,
                Some("arrange") => GraphInteraction::Arrange,
                _ => GraphInteraction::Edit,
            });
            c = c.routing(if s(&p, "routing") == "curves" {
                GraphRouting::Curves
            } else {
                GraphRouting::Lanes
            });
            if let Some(v) = p.get("toolbar") {
                c = c.toolbar(toolbar(
                    &format!("{}.toolbar", node.id),
                    v,
                    action("toolbar_action"),
                    emit.clone(),
                ));
            }
            if let Some(v) = p.get("can_connect") {
                let pairs = v.clone();
                c = c.can_connect(move |from, to| {
                    pairs.as_array().is_some_and(|pairs| {
                        pairs
                            .iter()
                            .any(|v| v["from"] == endpoint(from) && v["to"] == endpoint(to))
                    })
                });
            }
            if let Some(action) = action("event") {
                c = c.on_event(move |v, _, _| emit(&action, event(v)));
            }
            for name in ["empty", "failed", "loading"] {
                if let Some(factory) = slots.get(name).cloned() {
                    c = c.slot(name, move |window, cx| factory(window, cx));
                }
            }
            c.into_any_element()
        }
        _ => unreachable!("validated canvas component"),
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests;
