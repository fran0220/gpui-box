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
    /// Condense strongly connected components into a DAG. Members of each
    /// component share a column, stacked by identity; only internal edges can
    /// run backwards. Unlike BreakIncoming, cycles never delay unrelated DAG
    /// predecessors. Uses iterative traversal, including for deep graphs.
    Condense,
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
/// Node input order never affects the result. Each weakly connected component
/// gets its own horizontal band. Columns
/// advance by the widest member plus `column_gap`; rows by actual height plus
/// `row_gap`. Component bands are separated by `component_gap`. Duplicate edges
/// have no effect. Gaps must be finite and nonnegative, dimensions finite and
/// positive. Unknown endpoints and duplicate identities are errors.
///
/// `Reject` and `BreakIncoming` preserve lexical order within layers. `Condense`
/// keeps strongly connected members together and runs eight alternating
/// barycenter sweeps on the condensation DAG, retaining the best adjacent-layer
/// crossing count (never worse than its initial lexical ordering). Long edges
/// are not split into dummy vertices; this is not optimal crossing minimization
/// or obstacle routing. Cyclic edges under `BreakIncoming` may run backwards.
/// The returned bounds are sorted by identity. No geometry is installed into
/// NodeGraph.
pub fn layered_layout_sized<'a>(
    nodes: impl IntoIterator<Item = (SharedString, Size<f32>)>,
    edges: impl IntoIterator<Item = &'a GraphEdge>,
    column_gap: f32,
    row_gap: f32,
    component_gap: f32,
    cycles: GraphCyclePolicy,
) -> Result<Vec<(SharedString, Bounds<f32>)>, GraphLayoutError> {
    layout(
        nodes,
        edges,
        column_gap,
        row_gap,
        component_gap,
        cycles,
        false,
    )
}

fn layout<'a>(
    nodes: impl IntoIterator<Item = (SharedString, Size<f32>)>,
    edges: impl IntoIterator<Item = &'a GraphEdge>,
    column_gap: f32,
    row_gap: f32,
    component_gap: f32,
    cycles: GraphCyclePolicy,
    reduce_crossings: bool,
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
    if cycles == GraphCyclePolicy::Condense {
        return condensed_layout(&ids, &sizes, &outgoing, column_gap, row_gap, component_gap);
    }
    let mut unseen: BTreeSet<_> = (0..ids.len()).collect();
    let mut placed = BTreeMap::new();
    let mut band_y = 0.0f32;
    let mut positions = vec![None; ids.len()];
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
        let mut layers = Vec::new();
        while !component.is_empty() {
            if ready.is_empty() {
                if cycles == GraphCyclePolicy::Reject {
                    return Err(GraphLayoutError::Cycle);
                }
                ready.insert(*component.first().expect("nonempty component"));
            }
            let layer = std::mem::take(&mut ready);
            for node in &layer {
                component.remove(node);
            }
            for node in &layer {
                for next in &outgoing[*node] {
                    if component.contains(next) {
                        incoming[*next] -= 1;
                        if incoming[*next] == 0 {
                            ready.insert(*next);
                        }
                    }
                }
            }
            layers.push(layer.into_iter().collect::<Vec<_>>());
        }
        if reduce_crossings {
            barycenter_order(&mut layers, &neighbors, &mut positions);
        }
        let mut x = 0.0f32;
        let mut band_height = 0.0f32;
        for layer in layers {
            let mut y = band_y;
            let mut width = 0.0f32;
            for node in layer {
                let size = sizes[&ids[node]];
                let right = x + size.width;
                let bottom = y + size.height;
                if !right.is_finite() || !bottom.is_finite() || right <= x || bottom <= y {
                    return Err(GraphLayoutError::Overflow);
                }
                placed.insert(ids[node].clone(), Bounds::new(point(x, y), size));
                width = width.max(size.width);
                band_height = band_height.max(bottom - band_y);
                y = bottom + row_gap;
            }
            x += width + column_gap;
        }
        band_y += band_height + component_gap;
    }
    Ok(placed.into_iter().collect())
}

/// Alternating barycenter sweeps of adjacent layers. Stable ties preserve the
/// current order. This bounded heuristic is not an optimal-crossing promise;
/// long edges contribute only when their endpoints occupy adjacent layers.
fn barycenter_order(
    layers: &mut [Vec<usize>],
    neighbors: &[BTreeSet<usize>],
    positions: &mut [Option<usize>],
) {
    let mut best = layers.to_vec();
    let mut best_count = crossing_count(layers, neighbors, positions);
    for pass in 0..8 {
        let order: Vec<_> = if pass % 2 == 0 {
            (1..layers.len()).collect()
        } else {
            (0..layers.len().saturating_sub(1)).rev().collect()
        };
        for layer in order {
            let previous = if pass % 2 == 0 { layer - 1 } else { layer + 1 };
            for (rank, node) in layers[previous].iter().enumerate() {
                positions[*node] = Some(rank);
            }
            let mut scores: Vec<_> = layers[layer]
                .iter()
                .enumerate()
                .map(|(rank, node)| {
                    let (sum, count) = neighbors[*node]
                        .iter()
                        .filter_map(|neighbor| positions[*neighbor])
                        .fold((0u128, 0u128), |(sum, count), at| {
                            (sum + at as u128, count + 1)
                        });
                    (
                        *node,
                        if count == 0 {
                            (rank as u128, 1)
                        } else {
                            (sum, count)
                        },
                    )
                })
                .collect();
            scores.sort_by(|a, b| (a.1.0 * b.1.1).cmp(&(b.1.0 * a.1.1)));
            layers[layer] = scores.into_iter().map(|(node, _)| node).collect();
            for node in &layers[previous] {
                positions[*node] = None;
            }
        }
        let count = crossing_count(layers, neighbors, positions);
        if count < best_count {
            best_count = count;
            best.clone_from_slice(layers);
        }
    }
    layers.clone_from_slice(&best);
}

/// Count strict inversions between neighboring layers with a Fenwick tree.
/// Edges sharing a source or destination are not crossings. Each source's
/// queries happen before its inserts; equal target ranks are excluded too.
fn crossing_count(
    layers: &[Vec<usize>],
    neighbors: &[BTreeSet<usize>],
    positions: &mut [Option<usize>],
) -> u128 {
    let mut crossings = 0u128;
    for pair in layers.windows(2) {
        for (rank, node) in pair[1].iter().enumerate() {
            positions[*node] = Some(rank);
        }
        let mut tree = vec![0u128; pair[1].len() + 1];
        let mut seen = 0u128;
        for source in &pair[0] {
            for rank in neighbors[*source].iter().filter_map(|n| positions[*n]) {
                let mut at = rank + 1;
                let mut before_or_equal = 0;
                while at > 0 {
                    before_or_equal += tree[at];
                    at &= at - 1;
                }
                crossings += seen - before_or_equal;
            }
            for rank in neighbors[*source].iter().filter_map(|n| positions[*n]) {
                seen += 1;
                let mut at = rank + 1;
                while at < tree.len() {
                    tree[at] += 1;
                    at += at & at.wrapping_neg();
                }
            }
        }
        for node in &pair[1] {
            positions[*node] = None;
        }
    }
    crossings
}

/// Two iterative DFS passes (Kosaraju). Stable integer identities come from
/// lexical node order. Original implementation; algorithm background:
/// https://cp-algorithms.com/graph/strongly-connected-components.html
fn strongly_connected(outgoing: &[BTreeSet<usize>]) -> Vec<Vec<usize>> {
    let mut incoming = vec![Vec::new(); outgoing.len()];
    for (from, targets) in outgoing.iter().enumerate() {
        for to in targets {
            incoming[*to].push(from);
        }
    }
    let mut seen = vec![false; outgoing.len()];
    let mut finished = Vec::with_capacity(outgoing.len());
    for root in 0..outgoing.len() {
        if seen[root] {
            continue;
        }
        seen[root] = true;
        let mut stack = vec![(root, outgoing[root].iter())];
        while let Some((node, children)) = stack.last_mut() {
            if let Some(next) = children.next() {
                if !seen[*next] {
                    seen[*next] = true;
                    stack.push((*next, outgoing[*next].iter()));
                }
            } else {
                finished.push(*node);
                stack.pop();
            }
        }
    }
    seen.fill(false);
    let mut components = Vec::new();
    for root in finished.into_iter().rev() {
        if seen[root] {
            continue;
        }
        seen[root] = true;
        let mut members = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            members.push(node);
            for next in &incoming[node] {
                if !seen[*next] {
                    seen[*next] = true;
                    stack.push(*next);
                }
            }
        }
        members.sort_unstable();
        components.push(members);
    }
    components.sort_by_key(|members| members[0]);
    components
}

fn condensed_layout(
    ids: &[SharedString],
    sizes: &BTreeMap<SharedString, Size<f32>>,
    outgoing: &[BTreeSet<usize>],
    column_gap: f32,
    row_gap: f32,
    component_gap: f32,
) -> Result<Vec<(SharedString, Bounds<f32>)>, GraphLayoutError> {
    let components = strongly_connected(outgoing);
    let mut membership = vec![0; ids.len()];
    let mut groups = Vec::with_capacity(components.len());
    for (group, members) in components.iter().enumerate() {
        let mut width = 0f32;
        let mut height = 0f32;
        for (row, node) in members.iter().enumerate() {
            membership[*node] = group;
            let size = sizes[&ids[*node]];
            width = width.max(size.width);
            if row > 0 {
                height += row_gap;
            }
            height += size.height;
        }
        if !height.is_finite() {
            return Err(GraphLayoutError::Overflow);
        }
        groups.push((ids[members[0]].clone(), gpui::size(width, height)));
    }
    let mut connections = BTreeSet::new();
    for (from, targets) in outgoing.iter().enumerate() {
        for to in targets {
            let (a, b) = (membership[from], membership[*to]);
            if a != b {
                connections.insert((a, b));
            }
        }
    }
    let edges: Vec<_> = connections
        .into_iter()
        .map(|(a, b)| GraphEdge::new(groups[a].0.clone(), groups[b].0.clone()))
        .collect();
    let group_bounds = layout(
        groups,
        &edges,
        column_gap,
        row_gap,
        component_gap,
        GraphCyclePolicy::Reject,
        true,
    )?;
    let mut result = Vec::with_capacity(ids.len());
    for ((_, bounds), members) in group_bounds.into_iter().zip(components) {
        let mut y = bounds.origin.y;
        for node in members {
            let size = sizes[&ids[node]];
            if !(y + size.height).is_finite() || y + size.height <= y {
                return Err(GraphLayoutError::Overflow);
            }
            result.push((
                ids[node].clone(),
                Bounds::new(point(bounds.origin.x, y), size),
            ));
            y += size.height + row_gap;
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    #[test]
    fn every_three_by_three_graph_counts_strict_crossings_and_never_worsens() {
        // Independent quadratic oracle, exhaustively including shared sources,
        // shared destinations, isolated vertices and complete bipartite input.
        let brute = |layers: &[Vec<usize>], neighbors: &[BTreeSet<usize>]| {
            let mut edges = Vec::new();
            for (x, from) in layers[0].iter().enumerate() {
                for (y, to) in layers[1].iter().enumerate() {
                    if neighbors[*from].contains(to) {
                        edges.push((x, y));
                    }
                }
            }
            edges
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    edges[i + 1..]
                        .iter()
                        .filter(|b| (a.0 < b.0 && a.1 > b.1) || (a.0 > b.0 && a.1 < b.1))
                        .count() as u128
                })
                .sum::<u128>()
        };
        for mask in 0..512 {
            let mut neighbors = vec![BTreeSet::new(); 6];
            for from in 0..3 {
                for to in 0..3 {
                    if mask & (1 << (from * 3 + to)) != 0 {
                        neighbors[from].insert(to + 3);
                        neighbors[to + 3].insert(from);
                    }
                }
            }
            let mut layers = vec![vec![0, 1, 2], vec![3, 4, 5]];
            let before = brute(&layers, &neighbors);
            assert_eq!(crossing_count(&layers, &neighbors, &mut [None; 6]), before);
            barycenter_order(&mut layers, &neighbors, &mut [None; 6]);
            assert!(brute(&layers, &neighbors) <= before, "mask={mask}");
            assert_eq!(
                crossing_count(&layers, &neighbors, &mut [None; 6]),
                brute(&layers, &neighbors)
            );
        }
    }

    #[test]
    fn crossing_sweeps_untangle_reversed_matching_with_stable_ties() {
        let mut layers = vec![vec![0, 1, 2], vec![3, 4, 5]];
        let neighbors = vec![
            BTreeSet::from([5]),
            BTreeSet::from([4]),
            BTreeSet::from([3]),
            BTreeSet::from([2]),
            BTreeSet::from([1]),
            BTreeSet::from([0]),
        ];
        barycenter_order(&mut layers, &neighbors, &mut [None; 6]);
        assert_eq!(layers, vec![vec![0, 1, 2], vec![5, 4, 3]]);
        let once = layers.clone();
        barycenter_order(&mut layers, &neighbors, &mut [None; 6]);
        assert_eq!(layers, once);
    }

    #[test]
    fn condensation_keeps_cycle_members_together_and_external_edges_forward() {
        let nodes = [
            ("a", 20., 31.),
            ("b", 50., 17.),
            ("c", 13., 45.),
            ("z", 29., 9.),
        ]
        .map(|(id, w, h)| (id.into(), size(w, h)));
        let edges = [
            GraphEdge::new("z", "a"),
            GraphEdge::new("a", "b"),
            GraphEdge::new("b", "a"),
            GraphEdge::new("b", "c"),
        ];
        let result = layered_layout_sized(
            nodes.clone(),
            &edges,
            7.,
            11.,
            19.,
            GraphCyclePolicy::Condense,
        )
        .expect("valid SCC layout");
        assert_eq!(result[3].1.origin, point(0., 0.));
        assert_eq!(result[0].1.origin, point(36., 0.));
        assert_eq!(result[1].1.origin, point(36., 42.));
        assert_eq!(result[2].1.origin, point(93., 0.));
        assert_eq!(
            result,
            layered_layout_sized(
                nodes.into_iter().rev(),
                edges.iter().rev(),
                7.,
                11.,
                19.,
                GraphCyclePolicy::Condense
            )
            .expect("reordered SCC layout")
        );
    }

    #[test]
    fn scc_traversal_handles_a_deep_cycle_without_call_stack_growth() {
        let count = 100_000;
        let edges: Vec<_> = (0..count)
            .map(|i| BTreeSet::from([(i + 1) % count]))
            .collect();
        let components = strongly_connected(&edges);
        assert_eq!(components.len(), 1);
        assert_eq!(components[0], (0..count).collect::<Vec<_>>());
    }

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
