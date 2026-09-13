//! Retained preorder hierarchy. Range cover counts make collapsing a branch
//! independent of its descendant count; kth-visible queries feed uniform_list
//! without allocating a flattened copy on every viewport or selection update.
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use super::TraceSpan;
use gpui::SharedString;

#[derive(Clone, Copy, Default)]
struct Cover {
    minimum: i64,
    count: usize,
    lazy: i64,
}

#[derive(Default)]
pub(super) struct Hierarchy {
    spans: Rc<Vec<TraceSpan>>,
    ends: Vec<usize>,
    parents: Vec<Option<usize>>,
    identities: HashMap<SharedString, Vec<usize>>,
    collapsed: HashSet<SharedString>,
    tree: Vec<Cover>,
    pub(super) with_duration: bool,
    #[cfg(test)]
    visits: usize,
}

impl Hierarchy {
    pub(super) fn update(&mut self, spans: &Rc<Vec<TraceSpan>>, collapsed: &HashSet<SharedString>) {
        if !Rc::ptr_eq(&self.spans, spans) {
            *self = Self {
                spans: spans.clone(),
                ..Self::default()
            };
            self.ends = vec![spans.len(); spans.len()];
            self.parents = Vec::with_capacity(spans.len());
            let mut ancestors: Vec<usize> = Vec::new();
            for (index, span) in spans.iter().enumerate() {
                while ancestors
                    .last()
                    .is_some_and(|parent| spans[*parent].depth >= span.depth)
                {
                    self.ends[ancestors.pop().expect("known ancestor")] = index;
                }
                self.parents.push(ancestors.last().copied());
                ancestors.push(index);
                self.identities
                    .entry(span.id.clone())
                    .or_default()
                    .push(index);
                self.with_duration |= span.duration.is_some();
            }
            self.tree = vec![Cover::default(); spans.len().saturating_mul(4)];
            if !spans.is_empty() {
                self.build(0, 0, spans.len());
            }
        }
        // Only changed branch intervals are touched, preserving a nested
        // collapse when its ancestor is expanded again. Unknown IDs are kept
        // in the caller's set so a subsequent source revision can resolve them.
        let changes: Vec<_> = self
            .collapsed
            .symmetric_difference(collapsed)
            .cloned()
            .collect();
        if changes.is_empty() {
            return;
        }
        for id in changes {
            let delta = if collapsed.contains(&id) { 1 } else { -1 };
            let ranges: Vec<_> = self
                .identities
                .get(&id)
                .into_iter()
                .flatten()
                .map(|index| (index + 1, self.ends[*index]))
                .collect();
            for (start, end) in ranges {
                if start < end {
                    self.add(0, 0, self.spans.len(), start, end, delta);
                }
            }
        }
        self.collapsed.clone_from(collapsed);
    }

    fn build(&mut self, node: usize, start: usize, end: usize) {
        self.tree[node].count = end - start;
        if end - start > 1 {
            let mid = (start + end) / 2;
            self.build(node * 2 + 1, start, mid);
            self.build(node * 2 + 2, mid, end);
        }
    }

    fn add(&mut self, node: usize, start: usize, end: usize, lo: usize, hi: usize, delta: i64) {
        #[cfg(test)]
        {
            self.visits += 1;
        }
        if hi <= start || lo >= end {
            return;
        }
        if lo <= start && end <= hi {
            self.tree[node].minimum += delta;
            self.tree[node].lazy += delta;
            return;
        }
        let mid = (start + end) / 2;
        self.add(node * 2 + 1, start, mid, lo, hi, delta);
        self.add(node * 2 + 2, mid, end, lo, hi, delta);
        let a = self.tree[node * 2 + 1];
        let b = self.tree[node * 2 + 2];
        let minimum = a.minimum.min(b.minimum);
        self.tree[node].minimum = minimum + self.tree[node].lazy;
        self.tree[node].count = if a.minimum == minimum { a.count } else { 0 }
            + if b.minimum == minimum { b.count } else { 0 };
    }

    fn visible(&self, node: usize, inherited: i64) -> usize {
        let entry = self.tree[node];
        if entry.minimum + inherited == 0 {
            entry.count
        } else {
            0
        }
    }

    pub(super) fn len(&self) -> usize {
        if self.spans.is_empty() {
            0
        } else {
            self.visible(0, 0)
        }
    }

    pub(super) fn get(&self, mut rank: usize) -> Option<usize> {
        if rank >= self.len() {
            return None;
        }
        let (mut node, mut start, mut end, mut inherited) = (0, 0, self.spans.len(), 0);
        while end - start > 1 {
            inherited += self.tree[node].lazy;
            let left = node * 2 + 1;
            let count = self.visible(left, inherited);
            let mid = (start + end) / 2;
            if rank < count {
                node = left;
                end = mid;
            } else {
                rank -= count;
                node = left + 1;
                start = mid;
            }
        }
        Some(start)
    }

    fn rank(&self, index: usize) -> Option<usize> {
        let (mut node, mut start, mut end, mut inherited, mut rank) =
            (0, 0, self.spans.len(), 0, 0);
        while end - start > 1 {
            inherited += self.tree[node].lazy;
            let left = node * 2 + 1;
            let mid = (start + end) / 2;
            if index < mid {
                node = left;
                end = mid;
            } else {
                rank += self.visible(left, inherited);
                node = left + 1;
                start = mid;
            }
        }
        (self.visible(node, inherited) > 0).then_some(rank)
    }

    pub(super) fn position(&self, id: &SharedString) -> Option<usize> {
        self.identities
            .get(id)?
            .iter()
            .find_map(|index| self.rank(*index))
    }

    pub(super) fn parent(&self, index: usize) -> Option<usize> {
        self.parents[index].and_then(|parent| self.rank(parent))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_revision_rebuilds_depth_and_duration_and_releases_old_input() {
        let mut index = Hierarchy::default();
        let old = Rc::new(vec![
            TraceSpan::new("root", "Root", 0., 1.),
            TraceSpan::new("child", "Child", 0., 1.)
                .depth(2)
                .duration("1ms"),
        ]);
        let weak = Rc::downgrade(&old);
        let collapsed = HashSet::from(["root".into()]);
        index.update(&old, &collapsed);
        assert_eq!(index.len(), 1);
        assert!(index.with_duration);
        drop(old);
        let revised = Rc::new(vec![
            TraceSpan::new("child", "Now a root", 0., 1.),
            TraceSpan::new("root", "Now a child", 0., 1.).depth(1),
        ]);
        index.update(&revised, &collapsed);
        assert_eq!(index.len(), 2);
        assert_eq!(index.position(&"child".into()), Some(0));
        assert_eq!(index.parent(1), Some(0));
        assert!(!index.with_duration);
        assert!(
            weak.upgrade().is_none(),
            "old source is not retained by the index"
        );
        index.update(&Rc::new(Vec::new()), &collapsed);
        assert_eq!(index.len(), 0);
        assert_eq!(index.get(0), None);
        assert_eq!(index.position(&"root".into()), None);
    }

    #[test]
    fn nested_collapse_matches_preorder_oracle_across_all_transitions() {
        let spans = Rc::new(
            [0, 1, 3, 1, 0, 2, 4, 0]
                .into_iter()
                .enumerate()
                .map(|(i, depth)| TraceSpan::new(i.to_string(), "Span", 0., 1.).depth(depth))
                .collect::<Vec<_>>(),
        );
        let mut index = Hierarchy::default();
        for mask in (0..256).chain((0..256).rev()) {
            let collapsed = (0..8)
                .filter(|i| mask & (1 << i) != 0)
                .map(|i| i.to_string().into())
                .collect();
            index.update(&spans, &collapsed);
            let expected = super::super::tests::visible_indices(&spans, &collapsed);
            assert_eq!(index.len(), expected.len());
            assert_eq!(
                (0..index.len())
                    .filter_map(|i| index.get(i))
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(index.get(index.len()), None);
            for (i, span) in spans.iter().enumerate() {
                assert_eq!(
                    index.position(&span.id),
                    expected.iter().position(|v| *v == i)
                );
            }
        }
    }

    #[test]
    fn large_branch_updates_are_logarithmic_and_shared_redraw_does_no_tree_work() {
        let spans = Rc::new(
            (0..100_000)
                .map(|i| TraceSpan::new(i.to_string(), "Span", 0., 1.).depth(u32::from(i != 0)))
                .collect::<Vec<_>>(),
        );
        let mut index = Hierarchy::default();
        index.update(&spans, &HashSet::new());
        assert_eq!(index.len(), 100_000);
        let collapsed = HashSet::from(["0".into()]);
        index.update(&spans, &collapsed);
        assert_eq!(index.len(), 1);
        assert!(index.visits <= 70, "{} visited tree nodes", index.visits);
        let visits = index.visits;
        index.update(&spans, &collapsed);
        assert_eq!(index.visits, visits);
        index.update(&spans, &HashSet::new());
        assert_eq!(index.len(), 100_000);
        assert_eq!(index.get(99_999), Some(99_999));
        assert_eq!(index.position(&"99999".into()), Some(99_999));
    }
}
