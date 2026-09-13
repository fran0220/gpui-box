//! Retained graph-space bounds acceleration; queries preserve caller order.
//! Stable-cardinality edits refit only changed leaves and their ancestors.
//! Periodic rebuilding restores spatial partition quality after accumulated moves.

use gpui::{Bounds, point, size};

struct Node {
    bounds: Bounds<f32>,
    children: Option<(usize, usize)>,
    item: usize,
    parent: Option<usize>,
}

#[derive(Default)]
pub(super) struct BoundsIndex {
    nodes: Vec<Node>,
    root: Option<usize>,
    leaves: Vec<usize>,
    refits: usize,
}

pub(super) fn union(a: Bounds<f32>, b: Bounds<f32>) -> Bounds<f32> {
    let left = a.left().min(b.left());
    let top = a.top().min(b.top());
    Bounds::new(
        point(left, top),
        size(
            a.right().max(b.right()) - left,
            a.bottom().max(b.bottom()) - top,
        ),
    )
}

impl BoundsIndex {
    pub(super) fn new(bounds: impl IntoIterator<Item = Bounds<f32>>) -> Self {
        let mut items: Vec<_> = bounds.into_iter().enumerate().collect();
        let mut index = Self::default();
        index.leaves.resize(items.len(), 0);
        if !items.is_empty() {
            index.root = Some(index.build(&mut items));
        }
        index
    }

    fn build(&mut self, items: &mut [(usize, Bounds<f32>)]) -> usize {
        let bounds = items.iter().skip(1).fold(items[0].1, |a, b| union(a, b.1));
        let at = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            children: None,
            item: items[0].0,
            parent: None,
        });
        if items.len() > 1 {
            let x = bounds.size.width >= bounds.size.height;
            let middle = items.len() / 2;
            items.select_nth_unstable_by(middle, |a, b| {
                let a_center = if x { a.1.center().x } else { a.1.center().y };
                let b_center = if x { b.1.center().x } else { b.1.center().y };
                a_center.total_cmp(&b_center).then(a.0.cmp(&b.0))
            });
            let (left, right) = items.split_at_mut(middle);
            let left = self.build(left);
            let right = self.build(right);
            self.nodes[at].children = Some((left, right));
            self.nodes[left].parent = Some(at);
            self.nodes[right].parent = Some(at);
        } else {
            self.leaves[items[0].0] = at;
        }
        at
    }

    /// Input order is the query identity. A changed cardinality rebuilds; with
    /// equal cardinality each ordinal is updated in place, even after reorder.
    /// This still consumes the caller's iterator: it does not claim to discover
    /// arbitrary changes without visiting their authoritative bounds.
    pub(super) fn update(&mut self, bounds: impl ExactSizeIterator<Item = Bounds<f32>>) {
        if bounds.len() != self.leaves.len() {
            *self = Self::new(bounds);
            return;
        }
        for (item, bounds) in bounds.enumerate() {
            self.refit(item, bounds);
        }
        // Unbounded refitting preserves correctness but can destroy broad-phase
        // selectivity as objects cross the original split planes. Amortize a
        // repartition over changed leaves rather than over frames or queries.
        if self.refits >= self.leaves.len().max(64) {
            *self = Self::new(self.leaves.iter().map(|&at| self.nodes[at].bounds));
        }
    }

    fn refit(&mut self, item: usize, bounds: Bounds<f32>) -> usize {
        let mut at = self.leaves[item];
        if self.nodes[at].bounds == bounds {
            return 0;
        }
        self.refits += 1;
        self.nodes[at].bounds = bounds;
        let mut visits = 1;
        while let Some(parent) = self.nodes[at].parent {
            visits += 1;
            let Some((left, right)) = self.nodes[parent].children else {
                unreachable!()
            };
            let bounds = union(self.nodes[left].bounds, self.nodes[right].bounds);
            if self.nodes[parent].bounds == bounds {
                break;
            }
            self.nodes[parent].bounds = bounds;
            at = parent;
        }
        visits
    }

    pub(super) fn query(&self, view: Bounds<f32>, pad: f32) -> Vec<usize> {
        self.query_counted(view, pad).0
    }

    /// Stops at the first matching candidate without allocating a result list.
    /// The balanced tree bounds recursion depth; the predicate checks the exact
    /// geometry after this conservative broad phase.
    pub(super) fn find(
        &self,
        view: Bounds<f32>,
        mut matches: impl FnMut(usize) -> bool,
    ) -> Option<usize> {
        self.find_at(self.root?, view, &mut matches)
    }

    fn find_at(
        &self,
        at: usize,
        view: Bounds<f32>,
        matches: &mut impl FnMut(usize) -> bool,
    ) -> Option<usize> {
        let node = &self.nodes[at];
        if node.bounds.right() < view.left()
            || node.bounds.left() > view.right()
            || node.bounds.bottom() < view.top()
            || node.bounds.top() > view.bottom()
        {
            return None;
        }
        match node.children {
            Some((left, right)) => self
                .find_at(left, view, matches)
                .or_else(|| self.find_at(right, view, matches)),
            None => matches(node.item).then_some(node.item),
        }
    }

    fn query_counted(&self, view: Bounds<f32>, pad: f32) -> (Vec<usize>, usize) {
        let mut stack: Vec<_> = self.root.into_iter().collect();
        let mut result = Vec::new();
        let mut visits = 0;
        while let Some(at) = stack.pop() {
            visits += 1;
            let node = &self.nodes[at];
            if node.bounds.right() < view.left() - pad
                || node.bounds.left() > view.right() + pad
                || node.bounds.bottom() < view.top() - pad
                || node.bounds.top() > view.bottom() + pad
            {
                continue;
            }
            if let Some((left, right)) = node.children {
                stack.push(right);
                stack.push(left);
            } else {
                result.push(node.item);
            }
        }
        result.sort_unstable();
        (result, visits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refits_shrink_ancestors_and_preserve_queries_through_reorders_and_resize() {
        let mut bounds: Vec<_> = (0..137)
            .map(|i| Bounds::new(point(i as f32 * 31., (i % 7) as f32 * 19.), size(9., 13.)))
            .collect();
        let mut index = BoundsIndex::new(bounds.iter().copied());
        for turn in 0..200 {
            let item = turn % bounds.len();
            bounds[item].origin = point(
                (turn * 37 % 541) as f32 - 61.,
                (turn * 17 % 193) as f32 - 73.,
            );
            if turn % 19 == 0 {
                bounds.reverse();
            }
            if turn == 80 {
                bounds.truncate(103);
            }
            if turn == 140 {
                bounds.push(Bounds::new(point(-300., -200.), size(30., 40.)));
            }
            index.update(bounds.iter().copied());
            let view = Bounds::new(point((turn * 7 % 137) as f32 - 53., -29.), size(61., 47.));
            let expected: Vec<_> = bounds
                .iter()
                .enumerate()
                .filter_map(|(i, b)| {
                    (b.left() <= view.right()
                        && b.right() >= view.left()
                        && b.top() <= view.bottom()
                        && b.bottom() >= view.top())
                    .then_some(i)
                })
                .collect();
            assert_eq!(index.query(view, 0.), expected);
            assert_eq!(
                index.nodes[index.root.expect("root")].bounds,
                bounds.iter().copied().reduce(union).expect("union")
            );
        }
        index.update(std::iter::empty());
        assert!(index.root.is_none());
    }

    #[test]
    fn single_edit_touches_only_ancestor_chain_and_hits_do_no_refitting() {
        let bounds: Vec<_> = (0..100_000)
            .map(|i| Bounds::new(point(i as f32 * 20., 0.), size(7., 9.)))
            .collect();
        let mut index = BoundsIndex::new(bounds.iter().copied());
        let allocation = index.nodes.as_ptr();
        index.update(bounds.iter().copied());
        assert_eq!(index.refits, 0);
        let moved = Bounds::new(point(-73., -21.), size(8., 11.));
        assert!(index.refit(99_999, moved) <= 19);
        assert_eq!(index.refits, 1);
        assert_eq!(index.nodes.as_ptr(), allocation);
        assert_eq!(index.query(moved, 0.), vec![99_999]);
        assert!(index.query(bounds[99_999], 0.).is_empty());
        assert_eq!(index.refit(99_999, moved), 0);
        // A complete spatial permutation is repartitioned rather than leaving
        // the original broad-phase hierarchy indefinitely degraded.
        index.update(bounds.iter().rev().copied());
        assert_eq!(index.refits, 0);
        let (found, visits) = index.query_counted(bounds[36_501], 0.);
        assert_eq!(found, vec![63_498]);
        assert!(visits < 40);
    }

    #[test]
    fn bounds_queries_match_independent_inclusive_oracle_and_preserve_order() {
        let items: Vec<_> = (0..173)
            .map(|i| {
                Bounds::new(
                    point(((i * 71) % 193) as f32 - 85., ((i * 43) % 157) as f32 - 61.),
                    size((i % 19 + 1) as f32, (i % 13 + 3) as f32),
                )
            })
            .collect();
        let index = BoundsIndex::new(items.iter().copied());
        for x in -100..100 {
            for pad in [0., 3., 17.] {
                let view = Bounds::new(point(x as f32, 23.), size(29., 11.));
                let expected: Vec<_> = items
                    .iter()
                    .enumerate()
                    .filter_map(|(i, b)| {
                        let overlap_x =
                            b.left().max(view.left() - pad) <= b.right().min(view.right() + pad);
                        let overlap_y =
                            b.top().max(view.top() - pad) <= b.bottom().min(view.bottom() + pad);
                        (overlap_x && overlap_y).then_some(i)
                    })
                    .collect();
                assert_eq!(index.query(view, pad), expected);
            }
        }
        assert!(BoundsIndex::default().query(items[0], 0.).is_empty());
    }

    #[test]
    fn local_query_does_not_scan_one_hundred_thousand_separated_bounds() {
        let index = BoundsIndex::new(
            (0..100_000).map(|i| Bounds::new(point(i as f32 * 20., 0.), size(7., 9.))),
        );
        let (result, visits) =
            index.query_counted(Bounds::new(point(730_020., 2.), size(1., 3.)), 0.);
        assert_eq!(result, vec![36_501]);
        assert!(visits < 40, "query visited {visits} tree nodes");
    }
}
