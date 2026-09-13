use crate::{Bounds, Half};
use std::{
    cmp,
    fmt::Debug,
    ops::{Add, Sub},
};

/// Maximum children per internal node (R-tree style branching factor).
/// Higher values = shorter tree = fewer cache misses, but more work per node.
const MAX_CHILDREN: usize = 12;

/// A spatial tree optimized for finding maximum ordering among intersecting bounds.
///
/// Leaves have equal depth. Internal nodes hold 6..=12 children, except the root
/// which holds 2..=12. Overflow splits propagate to the root, so insertion depth
/// is logarithmic even for spatially ordered submissions. Splits sort a fixed
/// scratch array along the longest bounding-box axis; no coordinate conversion
/// or stronger unit bounds are required.
///
/// Search prunes by subtree bounds and maximum order and checks a global-max
/// leaf first. Spatial overlap can still require linear search; balance bounds
/// insertion depth, not the number of intersecting subtrees. Traversal and split
/// propagation are iterative, and node indices remain valid across Vec growth.
#[derive(Debug)]
pub(crate) struct BoundsTree<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    /// All nodes stored contiguously for cache efficiency.
    nodes: Vec<Node<U>>,
    /// Index of the root node, if any.
    root: Option<usize>,
    /// Index of the leaf with the highest ordering (for fast-path lookups).
    max_leaf: Option<usize>,
    /// Reusable stack for tree traversal during insertion.
    insert_path: Vec<usize>,
    /// Reusable stack for search operations.
    search_stack: Vec<usize>,
    #[cfg(test)]
    search_visits: usize,
    #[cfg(test)]
    insert_visits: usize,
}

/// A node in the bounds tree.
#[derive(Debug, Clone)]
struct Node<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    /// Bounding box containing this node and all descendants.
    bounds: Bounds<U>,
    /// Maximum ordering value in this subtree.
    max_order: u32,
    /// Node-specific data.
    kind: NodeKind,
}

#[derive(Debug, Clone)]
enum NodeKind {
    /// Leaf node containing actual bounds data.
    Leaf {
        /// The ordering assigned to this bounds.
        order: u32,
    },
    /// Internal node with children.
    Internal {
        /// Indices of child nodes (2 to MAX_CHILDREN).
        children: NodeChildren,
    },
}

/// Fixed-size array for child indices, avoiding heap allocation.
#[derive(Debug, Clone)]
struct NodeChildren {
    // Keeps an invariant where the max order child is always at the end
    indices: [usize; MAX_CHILDREN],
    len: u8,
}

impl NodeChildren {
    fn new() -> Self {
        Self {
            indices: [0; MAX_CHILDREN],
            len: 0,
        }
    }

    fn push(&mut self, index: usize) {
        debug_assert!((self.len as usize) < MAX_CHILDREN);
        self.indices[self.len as usize] = index;
        self.len += 1;
    }

    fn len(&self) -> usize {
        self.len as usize
    }

    fn as_slice(&self) -> &[usize] {
        &self.indices[..self.len as usize]
    }
}

impl<U> BoundsTree<U>
where
    U: Clone
        + Debug
        + PartialEq
        + PartialOrd
        + Add<U, Output = U>
        + Sub<Output = U>
        + Half
        + Default,
{
    /// Clears all nodes from the tree.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
        self.max_leaf = None;
        self.insert_path.clear();
        self.search_stack.clear();
        #[cfg(test)]
        {
            self.search_visits = 0;
            self.insert_visits = 0;
        }
    }

    /// Inserts bounds into the tree and returns its assigned ordering.
    ///
    /// The ordering is one greater than the maximum ordering of any
    /// existing bounds that intersect with the new bounds.
    pub fn insert(&mut self, new_bounds: Bounds<U>) -> u32 {
        // Find maximum ordering among intersecting bounds
        let max_intersecting = self.find_max_ordering(&new_bounds);
        let ordering = max_intersecting + 1;

        // Insert the new leaf
        let new_leaf_idx = self.insert_leaf(new_bounds, ordering);

        // Update max_leaf tracking
        self.max_leaf = match self.max_leaf {
            None => Some(new_leaf_idx),
            Some(old_idx) if self.nodes[old_idx].max_order < ordering => Some(new_leaf_idx),
            some => some,
        };

        ordering
    }

    /// Finds the maximum ordering among all bounds that intersect with the query.
    fn find_max_ordering(&mut self, query: &Bounds<U>) -> u32 {
        let Some(root_idx) = self.root else {
            return 0;
        };

        // Any intersecting global maximum proves the answer, including ties.
        // A miss proves nothing: another leaf can have the same maximum order.
        if let Some(max_idx) = self.max_leaf {
            let max_node = &self.nodes[max_idx];
            if query.intersects(&max_node.bounds) {
                return max_node.max_order;
            }
        }

        // Slow path: search the tree
        self.search_stack.clear();
        self.search_stack.push(root_idx);

        let mut max_found = 0u32;

        while let Some(node) = self.search_stack.pop() {
            #[cfg(test)]
            {
                self.search_visits += 1;
            }
            let node = &self.nodes[node];

            // Pruning: skip if this subtree can't improve our result
            if node.max_order <= max_found {
                continue;
            }

            // Spatial pruning: skip if bounds don't intersect
            if !query.intersects(&node.bounds) {
                continue;
            }

            match &node.kind {
                NodeKind::Leaf { order } => {
                    max_found = cmp::max(max_found, *order);
                }
                NodeKind::Internal { children } => {
                    // Children are maintained with highest max_order at the end.
                    // Push in forward order to highest (last) is popped first.
                    self.search_stack.extend(
                        children
                            .as_slice()
                            .iter()
                            .copied()
                            .filter(|&index| self.nodes[index].max_order > max_found),
                    );
                }
            }
        }

        max_found
    }

    /// Inserts a leaf node with the given bounds and ordering.
    /// Returns the index of the new leaf.
    fn insert_leaf(&mut self, bounds: Bounds<U>, order: u32) -> usize {
        let new_leaf_idx = self.nodes.len();
        self.nodes.push(Node {
            bounds: bounds.clone(),
            max_order: order,
            kind: NodeKind::Leaf { order },
        });

        let Some(root_idx) = self.root else {
            // Tree is empty, new leaf becomes root
            self.root = Some(new_leaf_idx);
            return new_leaf_idx;
        };

        // Only the root may be a bare leaf.
        if matches!(self.nodes[root_idx].kind, NodeKind::Leaf { .. }) {
            let mut children = NodeChildren::new();
            children.push(root_idx);
            children.push(new_leaf_idx);
            let root = self.internal_node(children);
            let new_root_idx = self.nodes.len();
            self.nodes.push(root);
            self.root = Some(new_root_idx);
            return new_leaf_idx;
        }

        // Descend to find the best internal node to insert into
        self.insert_path.clear();
        let mut current_idx = root_idx;

        loop {
            #[cfg(test)]
            {
                self.insert_visits += 1;
            }
            let current = &self.nodes[current_idx];
            let NodeKind::Internal { children } = &current.kind else {
                unreachable!("Should only traverse internal nodes");
            };

            self.insert_path.push(current_idx);

            // Equal leaf depth means all children here have the same kind.
            if matches!(self.nodes[children.indices[0]].kind, NodeKind::Leaf { .. }) {
                break;
            }

            // Minimize enlargement, then union perimeter. Stable ties keep
            // insertion deterministic without requiring Ord on generic units.
            let mut best_child_idx = children.as_slice()[0];
            let mut best_perimeter = bounds
                .union(&self.nodes[best_child_idx].bounds)
                .half_perimeter();
            let mut best_cost =
                best_perimeter.clone() - self.nodes[best_child_idx].bounds.half_perimeter();

            for &child_idx in children.as_slice().iter().skip(1) {
                let perimeter = bounds.union(&self.nodes[child_idx].bounds).half_perimeter();
                let cost = perimeter.clone() - self.nodes[child_idx].bounds.half_perimeter();
                if cost < best_cost || (cost == best_cost && perimeter < best_perimeter) {
                    best_cost = cost;
                    best_perimeter = perimeter;
                    best_child_idx = child_idx;
                }
            }
            current_idx = best_child_idx;
        }

        // Keep the original node index for one split half, and append the other.
        // Ancestors retain ownership of the original; no leaf ever moves.
        let mut pending = Some(new_leaf_idx);
        while let Some(node_idx) = self.insert_path.pop() {
            let NodeKind::Internal { mut children } = self.nodes[node_idx].kind.clone() else {
                unreachable!("insertion path contains only internal nodes");
            };
            if let Some(child) = pending.take() {
                if children.len() == MAX_CHILDREN {
                    pending = Some(self.split(node_idx, children, child));
                    continue;
                }
                children.push(child);
            }
            self.nodes[node_idx] = self.internal_node(children);
        }
        if let Some(sibling) = pending {
            let mut children = NodeChildren::new();
            children.push(root_idx);
            children.push(sibling);
            let root = self.internal_node(children);
            self.root = Some(self.nodes.len());
            self.nodes.push(root);
        }

        new_leaf_idx
    }

    /// Recompute exact metadata, keeping a maximum-order child last for search.
    fn internal_node(&self, mut children: NodeChildren) -> Node<U> {
        let first = &self.nodes[children.indices[0]];
        let mut bounds = first.bounds.clone();
        let mut max_order = first.max_order;
        let mut max_pos = 0;
        for (pos, &index) in children.as_slice().iter().enumerate().skip(1) {
            let child = &self.nodes[index];
            bounds = bounds.union(&child.bounds);
            if child.max_order > max_order {
                max_order = child.max_order;
                max_pos = pos;
            }
        }
        let last = children.len() - 1;
        children.indices.swap(max_pos, last);
        Node {
            bounds,
            max_order,
            kind: NodeKind::Internal { children },
        }
    }

    /// Split 13 same-level children into groups of 6 and 7. Fixed-size insertion
    /// sort also supports partially ordered units without a total-order adapter.
    fn split(&mut self, index: usize, children: NodeChildren, extra: usize) -> usize {
        let mut indices = [0; MAX_CHILDREN + 1];
        indices[..MAX_CHILDREN].copy_from_slice(children.as_slice());
        indices[MAX_CHILDREN] = extra;
        let bounds = self.nodes[index].bounds.union(&self.nodes[extra].bounds);
        let horizontal = bounds.size.width >= bounds.size.height;
        for i in 1..indices.len() {
            let mut j = i;
            while j > 0 {
                let a = &self.nodes[indices[j]].bounds.origin;
                let b = &self.nodes[indices[j - 1]].bounds.origin;
                let precedes = if horizontal { a.x < b.x } else { a.y < b.y };
                if !precedes {
                    break;
                }
                indices.swap(j, j - 1);
                j -= 1;
            }
        }
        let mut left = NodeChildren::new();
        let mut right = NodeChildren::new();
        for &child in &indices[..MAX_CHILDREN / 2] {
            left.push(child);
        }
        for &child in &indices[MAX_CHILDREN / 2..] {
            right.push(child);
        }
        self.nodes[index] = self.internal_node(left);
        let sibling = self.internal_node(right);
        let sibling_idx = self.nodes.len();
        self.nodes.push(sibling);
        sibling_idx
    }
}

impl<U> Default for BoundsTree<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    fn default() -> Self {
        BoundsTree {
            nodes: Vec::new(),
            root: None,
            max_leaf: None,
            insert_path: Vec::new(),
            search_stack: Vec::new(),
            #[cfg(test)]
            search_visits: 0,
            #[cfg(test)]
            insert_visits: 0,
        }
    }
}

#[cfg(test)]
#[path = "bounds_tree/tests.rs"]
mod balanced_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bounds, Point, Size};
    use rand::{Rng, SeedableRng};

    #[test]
    fn test_insert() {
        let mut tree = BoundsTree::<f32>::default();
        let bounds1 = Bounds {
            origin: Point { x: 0.0, y: 0.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        let bounds2 = Bounds {
            origin: Point { x: 5.0, y: 5.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        let bounds3 = Bounds {
            origin: Point { x: 10.0, y: 10.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };

        // Insert the bounds into the tree and verify the order is correct
        assert_eq!(tree.insert(bounds1), 1);
        assert_eq!(tree.insert(bounds2), 2);
        assert_eq!(tree.insert(bounds3), 3);

        // Insert non-overlapping bounds and verify they can reuse orders
        let bounds4 = Bounds {
            origin: Point { x: 20.0, y: 20.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        let bounds5 = Bounds {
            origin: Point { x: 40.0, y: 40.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        let bounds6 = Bounds {
            origin: Point { x: 25.0, y: 25.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        assert_eq!(tree.insert(bounds4), 1); // bounds4 does not overlap with bounds1, bounds2, or bounds3
        assert_eq!(tree.insert(bounds5), 1); // bounds5 does not overlap with any other bounds
        assert_eq!(tree.insert(bounds6), 2); // bounds6 overlaps with bounds4, so it should have a different order
    }

    #[test]
    fn test_random_iterations() {
        let max_bounds = 100;
        for seed in 1..=1000 {
            // let seed = 44;
            let mut tree = BoundsTree::default();
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed as u64);
            let mut expected_quads: Vec<(Bounds<f32>, u32)> = Vec::new();

            // Insert a random number of random AABBs into the tree.
            let num_bounds = rng.random_range(1..=max_bounds);
            for _ in 0..num_bounds {
                let min_x: f32 = rng.random_range(-100.0..100.0);
                let min_y: f32 = rng.random_range(-100.0..100.0);
                let width: f32 = rng.random_range(0.0..50.0);
                let height: f32 = rng.random_range(0.0..50.0);
                let bounds = Bounds {
                    origin: Point { x: min_x, y: min_y },
                    size: Size { width, height },
                };

                let expected_ordering = expected_quads
                    .iter()
                    .filter_map(|quad| quad.0.intersects(&bounds).then_some(quad.1))
                    .max()
                    .unwrap_or(0)
                    + 1;
                expected_quads.push((bounds, expected_ordering));

                // Insert the AABB into the tree and collect intersections.
                let actual_ordering = tree.insert(bounds);
                assert_eq!(actual_ordering, expected_ordering);
            }
        }
    }
}
