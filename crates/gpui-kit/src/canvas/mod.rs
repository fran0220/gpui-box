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

mod edge;
mod graph;
mod node;

pub use edge::{EdgeKind, GraphEdge};
pub use graph::{GraphState, NodeGraph, Placed};
pub use node::{Diff, GraphNode, NODE_WIDTH, NodeMetric, NodeState};
