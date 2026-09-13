//! Optional caller-side graph layout. NodeGraph never invokes this implicitly.
use std::collections::{BTreeMap, BTreeSet};

use gpui::{Bounds, SharedString, Size, point};

use super::GraphEdge;

/// How a layered layout handles a component without a remaining source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphCyclePolicy {
    /// Return an error; no partial layout is returned.
    Reject,
    /// Place the lexically first remaining node, ignoring its incoming edges.
    /// Such edges can point backwards; this is not a DAG ordering or an SCC layout.
    BreakIncoming,
}

/// Invalid input to [`layered_layout_sized`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphLayoutError {
    DuplicateId(SharedString),
    InvalidSize(SharedString),
    UnknownEndpoint(SharedString),
    InvalidGap,
    Cycle,
    Overflow,
}

/// Deterministic left-to-right layout using caller-measured node dimensions.
///
/// Nodes, components and nodes within a layer are ordered by identity, not input
/// order. Each weakly connected component gets its own horizontal band. Columns
/// advance by the widest member plus `column_gap`; rows by actual height plus
/// `row_gap`. Component bands are separated by `component_gap`. Duplicate edges
/// have no effect. Gaps must be finite and nonnegative, dimensions finite and
/// positive. Unknown endpoints and duplicate identities are errors.
///
/// This is a stable topological layering, not crossing minimization or obstacle
/// routing. Cyclic edges under `BreakIncoming` may run backwards. The returned
/// bounds are sorted by identity. No geometry is installed into NodeGraph.
pub fn layered_layout_sized<'a>(
    nodes: impl IntoIterator<Item = (SharedString, Size<f32>)>,
    edges: impl IntoIterator<Item = &'a GraphEdge>,
    column_gap: f32,
    row_gap: f32,
    component_gap: f32,
    cycles: GraphCyclePolicy,
) -> Result<Vec<(SharedString, Bounds<f32>)>, GraphLayoutError> {
    if [column_gap, row_gap, component_gap]
        .iter()
        .any(|gap| !gap.is_finite() || *gap < 0.0)
    {
        return Err(GraphLayoutError::InvalidGap);
    }
    let mut sizes = BTreeMap::new();
    for (id, size) in nodes {
        if !size.width.is_finite()
            || !size.height.is_finite()
            || size.width <= 0.0
            || size.height <= 0.0
        {
            return Err(GraphLayoutError::InvalidSize(id));
        }
        if sizes.insert(id.clone(), size).is_some() {
            return Err(GraphLayoutError::DuplicateId(id));
        }
    }
    let ids: Vec<_> = sizes.keys().cloned().collect();
    let indices: BTreeMap<_, _> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i))
        .collect();
    let mut outgoing = vec![BTreeSet::new(); ids.len()];
    let mut neighbors = vec![BTreeSet::new(); ids.len()];
    let mut incoming = vec![0usize; ids.len()];
    for edge in edges {
        let from = *indices
            .get(edge.from())
            .ok_or_else(|| GraphLayoutError::UnknownEndpoint(edge.from().clone()))?;
        let to = *indices
            .get(edge.to())
            .ok_or_else(|| GraphLayoutError::UnknownEndpoint(edge.to().clone()))?;
        if outgoing[from].insert(to) {
            incoming[to] += 1;
            neighbors[from].insert(to);
            neighbors[to].insert(from);
        }
    }
    let mut unseen: BTreeSet<_> = (0..ids.len()).collect();
    let mut placed = BTreeMap::new();
    let mut band_y = 0.0f32;
    while let Some(root) = unseen.pop_first() {
        let mut component = BTreeSet::from([root]);
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            for next in &neighbors[node] {
                if unseen.remove(next) {
                    component.insert(*next);
                    stack.push(*next);
                }
            }
        }
        let mut ready: BTreeSet<_> = component
            .iter()
            .copied()
            .filter(|i| incoming[*i] == 0)
            .collect();
        let mut x = 0.0f32;
        let mut band_height = 0.0f32;
        while !component.is_empty() {
            if ready.is_empty() {
                if cycles == GraphCyclePolicy::Reject {
                    return Err(GraphLayoutError::Cycle);
                }
                ready.insert(*component.first().expect("nonempty component"));
            }
            let layer = std::mem::take(&mut ready);
            let mut y = band_y;
            let mut width = 0.0f32;
            for node in &layer {
                component.remove(node);
                let size = sizes[&ids[*node]];
                let right = x + size.width;
                let bottom = y + size.height;
                if !right.is_finite() || !bottom.is_finite() || right <= x || bottom <= y {
                    return Err(GraphLayoutError::Overflow);
                }
                placed.insert(ids[*node].clone(), Bounds::new(point(x, y), size));
                width = width.max(size.width);
                band_height = band_height.max(bottom - band_y);
                y = bottom + row_gap;
            }
            for node in layer {
                for next in &outgoing[node] {
                    if component.contains(next) {
                        incoming[*next] -= 1;
                        if incoming[*next] == 0 {
                            ready.insert(*next);
                        }
                    }
                }
            }
            x += width + column_gap;
        }
        band_y += band_height + component_gap;
    }
    Ok(placed.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    #[test]
    fn dimensions_components_and_input_order_are_exact() {
        let nodes = vec![
            ("b".into(), size(70., 15.)),
            ("a".into(), size(20., 45.)),
            ("d".into(), size(13., 19.)),
            ("c".into(), size(110., 27.)),
        ];
        let edges = vec![GraphEdge::new("a", "c"), GraphEdge::new("b", "c")];
        let layout = |nodes: Vec<_>, edges: Vec<_>| {
            layered_layout_sized(nodes, &edges, 7., 11., 23., GraphCyclePolicy::Reject)
                .expect("valid DAG")
        };
        let result = layout(nodes.clone(), edges.clone());
        assert_eq!(
            result.iter().map(|(_, b)| b.origin).collect::<Vec<_>>(),
            vec![
                point(0., 0.),
                point(0., 56.),
                point(77., 0.),
                point(0., 94.)
            ]
        );
        assert_eq!(
            result,
            layout(
                nodes.into_iter().rev().collect(),
                edges.into_iter().rev().collect()
            )
        );
        for (i, (_, a)) in result.iter().enumerate() {
            for (_, b) in &result[i + 1..] {
                assert!(
                    a.right() <= b.left()
                        || b.right() <= a.left()
                        || a.bottom() <= b.top()
                        || b.bottom() <= a.top()
                );
            }
        }
    }

    #[test]
    fn cycles_are_rejected_or_broken_not_claimed_as_dag_order() {
        let nodes = vec![("b".into(), size(10., 30.)), ("a".into(), size(40., 20.))];
        let edges = [GraphEdge::new("b", "a"), GraphEdge::new("a", "b")];
        assert_eq!(
            layered_layout_sized(nodes.clone(), &edges, 5., 8., 10., GraphCyclePolicy::Reject),
            Err(GraphLayoutError::Cycle)
        );
        let result =
            layered_layout_sized(nodes, &edges, 5., 8., 10., GraphCyclePolicy::BreakIncoming)
                .expect("cycle breaking is explicit");
        assert_eq!(result[0].1.origin, point(0., 0.));
        assert_eq!(result[1].1.origin, point(45., 0.)); // b -> a necessarily goes backwards.
        let self_loop = [GraphEdge::new("a", "a")];
        assert_eq!(
            layered_layout_sized(
                [("a".into(), size(3., 9.))],
                &self_loop,
                0.,
                0.,
                0.,
                GraphCyclePolicy::Reject
            ),
            Err(GraphLayoutError::Cycle)
        );
    }

    #[test]
    fn malformed_identity_and_geometry_are_not_silently_dropped() {
        let node = ("a".into(), size(3., 9.));
        assert_eq!(
            layered_layout_sized(
                [node.clone(), node.clone()],
                [],
                0.,
                0.,
                0.,
                GraphCyclePolicy::Reject
            ),
            Err(GraphLayoutError::DuplicateId("a".into()))
        );
        assert_eq!(
            layered_layout_sized(
                [node],
                &[GraphEdge::new("a", "missing")],
                0.,
                0.,
                0.,
                GraphCyclePolicy::Reject
            ),
            Err(GraphLayoutError::UnknownEndpoint("missing".into()))
        );
        assert_eq!(
            layered_layout_sized(
                [("a".into(), size(f32::NAN, 9.))],
                [],
                0.,
                0.,
                0.,
                GraphCyclePolicy::Reject
            ),
            Err(GraphLayoutError::InvalidSize("a".into()))
        );
    }
}
