//! The connections between nodes on a graph canvas.
//!
//! An edge is drawn, not laid out: it is a path stroked underneath the nodes
//! it joins, so adding one never moves anything. Its geometry is computed from
//! the boxes the graph already knows, which is why this module takes rectangles
//! and returns paths and holds no state of its own.

use gpui::{Bounds, Hsla, PathBuilder, Pixels, Window, point, px};
use gpui_kit_theme::Theme;

/// What a connection means.
///
/// The two kinds are drawn differently because they are different claims about
/// the run, not because variety is nice to look at: a reader scanning a failed
/// graph needs to see at once which way the work was flowing and which way it
/// was sent back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeKind {
    /// Work moving forward from one step to the next.
    #[default]
    Flow,
    /// Work sent back for another attempt after something failed downstream.
    ///
    /// Dashed and in the danger colour, because a loop that looked like a
    /// flow would report a graph that ran cleanly in a circle rather than one
    /// that had to go back.
    Feedback,
}

impl EdgeKind {
    pub fn color(self, theme: &Theme) -> Hsla {
        match self {
            Self::Flow => theme.colors.hairline_strong,
            Self::Feedback => theme.colors.danger,
        }
    }

    fn dashes(self) -> Option<[Pixels; 2]> {
        match self {
            Self::Flow => None,
            Self::Feedback => Some([px(5.0), px(4.0)]),
        }
    }
}

/// One connection between two nodes, named by the identities they carry.
///
/// The endpoints are business ids rather than indices, so an edge survives the
/// nodes being reordered and a graph that names a node it does not have is a
/// mistake that can be reported rather than one that silently draws a line to
/// the wrong box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    pub from: gpui::SharedString,
    pub to: gpui::SharedString,
    pub kind: EdgeKind,
}

impl GraphEdge {
    pub fn new(from: impl Into<gpui::SharedString>, to: impl Into<gpui::SharedString>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            kind: EdgeKind::Flow,
        }
    }

    /// Marks the edge as a return path after a failure downstream.
    pub fn feedback(mut self) -> Self {
        self.kind = EdgeKind::Feedback;
        self
    }
}

/// How far an edge leaves its node before it starts turning.
const LEAD: f32 = 24.0;
/// How far below both nodes a feedback edge dips on its way back.
const RETURN_DROP: f32 = 36.0;

/// Strokes one edge between two node boxes.
///
/// A forward edge leaves the right of `from` and arrives at the left of `to`.
/// A feedback edge is going backwards, so routing it the same way would draw
/// it straight through the nodes in between; it drops below the pair and
/// returns underneath them instead, which is the shape that reads as "this
/// went back".
pub fn paint_edge(
    window: &mut Window,
    theme: &Theme,
    kind: EdgeKind,
    from: Bounds<Pixels>,
    to: Bounds<Pixels>,
    width: f32,
) {
    let mut builder = PathBuilder::stroke(px(width));
    if let Some(dashes) = kind.dashes() {
        builder = builder.dash_array(&dashes);
    }

    match kind {
        EdgeKind::Flow => {
            let start = point(from.right(), from.center().y);
            let end = point(to.left(), to.center().y);
            builder.move_to(start);
            // Two quadratics meeting at the midpoint make the S a single
            // cubic would, and `curve_to` is the quadratic this renderer
            // offers.
            let middle = point((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
            builder.curve_to(middle, point(start.x + px(LEAD), start.y));
            builder.curve_to(end, point(end.x - px(LEAD), end.y));
        }
        EdgeKind::Feedback => {
            let start = point(from.center().x, from.bottom());
            let end = point(to.center().x, to.bottom());
            let floor = from.bottom().max(to.bottom()) + px(RETURN_DROP);
            builder.move_to(start);
            builder.curve_to(point((start.x + end.x) / 2.0, floor), point(start.x, floor));
            builder.curve_to(end, point(end.x, floor));
        }
    }

    if let Ok(path) = builder.build() {
        window.paint_path(path, kind.color(theme));
    }
}

#[cfg(test)]
mod tests {
    use gpui::size;

    use super::*;

    fn theme() -> Theme {
        Theme::studio_dark()
    }

    #[test]
    fn a_flow_reads_as_structure_and_a_feedback_as_a_failure() {
        let theme = theme();
        assert_eq!(EdgeKind::Flow.color(&theme), theme.colors.hairline_strong);
        assert_eq!(EdgeKind::Feedback.color(&theme), theme.colors.danger);
    }

    /// The dash is the whole difference between "it went on" and "it went
    /// back" for anyone reading the graph in one colour, so only one kind
    /// carries it.
    #[test]
    fn only_a_feedback_edge_is_dashed() {
        assert!(EdgeKind::Flow.dashes().is_none());
        assert!(EdgeKind::Feedback.dashes().is_some());
    }

    #[test]
    fn an_edge_names_its_ends_by_identity() {
        let edge = GraphEdge::new("plan", "apply").feedback();
        assert_eq!(edge.from, "plan");
        assert_eq!(edge.to, "apply");
        assert_eq!(edge.kind, EdgeKind::Feedback);
    }

    #[test]
    fn a_new_edge_is_a_forward_flow_until_it_is_told_otherwise() {
        assert_eq!(GraphEdge::new("a", "b").kind, EdgeKind::Flow);
    }

    /// A feedback edge exists to be seen going back under the nodes, so its
    /// floor has to clear the deeper of the two boxes rather than either one.
    #[test]
    fn a_feedback_edge_clears_the_lower_of_the_two_nodes() {
        let shallow = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(40.0)));
        let deep = Bounds::new(point(px(200.0), px(0.0)), size(px(100.0), px(120.0)));
        let floor = shallow.bottom().max(deep.bottom()) + px(RETURN_DROP);
        assert!(floor > deep.bottom());
        assert!(floor > shallow.bottom());
    }
}
