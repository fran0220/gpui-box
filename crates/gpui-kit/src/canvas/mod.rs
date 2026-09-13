//! A run drawn as connected steps on a canvas.
//!
//! This is the shape an agent's work takes when it stops being a list: steps
//! as cards, connections as paths, and the failures that sent work back drawn
//! as the loops they are rather than flattened into the forward order.
//!
//! The division of labour is the same one the rest of the library keeps. The
//! caller owns the run, the positions and the actions; the canvas owns the
//! backdrop, the card, the path geometry and the states. There is no layout
//! algorithm here on purpose: where a step belongs is a claim about the run,
//! and a component that placed steps itself would be making that claim on
//! every host's behalf.
//!
//! ```no_run
//! use gpui_kit::prelude::*;
//!
//! let graph = NodeGraph::new("run.12")
//!     .node(
//!         GraphNode::new("run.12.plan", "Plan")
//!             .state(NodeState::Succeeded)
//!             .metric("tokens", "4.1k")
//!             .metric("took", "2.4s"),
//!         0.0,
//!         0.0,
//!     )
//!     .node(
//!         GraphNode::new("run.12.apply", "Apply")
//!             .state(NodeState::Failed)
//!             .action("write src/main.rs")
//!             .diff(Diff::new(24, 3)),
//!         280.0,
//!         0.0,
//!     )
//!     .edge(GraphEdge::new("run.12.plan", "run.12.apply"))
//!     .edge(GraphEdge::new("run.12.apply", "run.12.plan").feedback());
//! ```

mod band;
mod edge;
mod graph;
mod group;
mod layout;
mod minimap;
mod node;
mod toolbar;

pub use band::GraphBand;
pub use edge::{EdgeKind, EdgeMarker, EdgeState, GraphEdge, GraphEndpoint, GraphRouting, PortSide};
pub use graph::{
    GraphFit, GraphInteraction, GraphState, GraphViewport, NodeGraph, NodeGraphEvent, Placed,
    layered_layout,
};
pub use group::NodeGroup;
pub use layout::{GraphCyclePolicy, GraphLayoutError, layered_layout_sized};
pub use minimap::{Minimap, MinimapEvent, MinimapMark, MinimapView};
pub use node::{
    Diff, GraphNode, GraphPort, NODE_WIDTH, NodeMetric, NodeState, PortDirection, PortType,
};
pub use toolbar::{CanvasToolbar, CanvasToolbarAction, CanvasToolbarEvent};

/// How much of a port's pointer target the drawn socket fills.
///
/// The socket a reader aims at and the socket they see are two different
/// requirements, and `measure.nodePort` can only be one number. It is the
/// target — the resize grip borrows it for the same reason — so the mark is
/// taken as a fraction of it and the rest of the box stays invisible.
///
/// Drawn at the full target the socket was the loudest thing on a card:
/// larger than the icon saying what the step is, close to three times the dot
/// saying what it is doing, and ten times the wire it caps. A connection point
/// is a place to aim, not a fact about the run, so it now sits under the marks
/// that carry meaning while the target it answers to does not move.
pub(crate) const PORT_MARK_SCALE: f32 = 0.65;

/// A stable id built from a prefix and length-delimited parts, so two
/// different part lists can never collide by concatenation.
pub(crate) fn composite_id(prefix: &str, parts: &[&str]) -> gpui::SharedString {
    let mut id = prefix.to_string();
    for part in parts {
        id.push(':');
        id.push_str(&part.len().to_string());
        id.push(':');
        id.push_str(part);
    }
    id.into()
}
