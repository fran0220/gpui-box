use super::*;
use crate::{Point, Size};
use rand::{Rng, SeedableRng};

fn rect(x: f32, y: f32, width: f32, height: f32) -> Bounds<f32> {
    Bounds {
        origin: Point { x, y },
        size: Size { width, height },
    }
}

// Deliberately independent of Bounds::intersects, union and the index.
// Strict comparisons preserve touching and zero-size behavior (a point inside
// a nonempty rectangle intersects it, while two coincident points do not).
fn overlaps(a: &Bounds<f32>, b: &Bounds<f32>) -> bool {
    a.origin.x < b.origin.x + b.size.width
        && b.origin.x < a.origin.x + a.size.width
        && a.origin.y < b.origin.y + b.size.height
        && b.origin.y < a.origin.y + a.size.height
}

fn oracle(bounds: &[Bounds<f32>]) -> Vec<u32> {
    let mut orders = Vec::with_capacity(bounds.len());
    for (i, b) in bounds.iter().enumerate() {
        orders.push(
            bounds[..i]
                .iter()
                .zip(&orders)
                .filter_map(|(a, &order)| overlaps(a, b).then_some(order))
                .max()
                .unwrap_or(0)
                + 1,
        );
    }
    orders
}

fn workload(name: &str, n: usize) -> Vec<Bounds<f32>> {
    (0..n)
        .map(|i| {
            let f = i as f32;
            match name {
                "horizontal" => rect(f * 3.0, -17.0, 2.0, 7.0),
                "vertical" => rect(11.0, f * 5.0, 3.0, 4.0),
                "reversed" => rect((n - i) as f32 * 3.0, -17.0, 2.0, 7.0),
                "grid" => rect((i % 97) as f32 * 3.0, (i / 97) as f32 * 5.0, 4.0, 6.0),
                "overlap" => rect(-3.0, 7.0, 11.0, 19.0),
                "nested" => rect(-f, -2.0 * f, 2.0 * f + 1.0, 4.0 * f + 3.0),
                "sparse" => rect((i % 2) as f32 * 100_000.0 + f * 2.0, f * 7.0, 1.0, 3.0),
                "touching" => rect(f * 2.0, 0.0, 2.0, 3.0),
                "zero" => rect(
                    (i % 7) as f32,
                    (i % 11) as f32,
                    (i % 3) as f32,
                    (i % 5) as f32,
                ),
                // Exact source order/projection from the Geo worker. Center is
                // source point 5000, destination bounds are a radius-5 circle.
                "geo" => {
                    let lon = -179.0 + (i % 1000) as f64 * 0.358;
                    let lat = -80.0 + (i / 1000) as f64 * 1.6;
                    let x = (lon + 180.0) / 360.0;
                    let y = 0.5 - lat / 360.0;
                    let center_x = 1.0 / 360.0;
                    let center_y = 0.5 - (-72.0 / 360.0);
                    rect(
                        (320.0 + (x - center_x) * 280.0) as f32 - 5.0,
                        (140.0 + (y - center_y) * 280.0) as f32 - 5.0,
                        10.0,
                        10.0,
                    )
                }
                _ => panic!("unknown workload {name}"),
            }
        })
        .collect()
}

// Iterative ownership/shape inspection is safe even for the old degenerate tree.
fn shape(tree: &BoundsTree<f32>, balanced: bool) -> (usize, usize) {
    let root = tree.root.expect("nonempty");
    let mut stack = vec![(root, 0)];
    let mut seen = vec![false; tree.nodes.len()];
    let mut min_depth = usize::MAX;
    let mut max_depth = 0;
    while let Some((index, depth)) = stack.pop() {
        assert!(!seen[index], "node {index} has multiple owners or a cycle");
        seen[index] = true;
        let node = &tree.nodes[index];
        match &node.kind {
            NodeKind::Leaf { order } => {
                assert_eq!(node.max_order, *order);
                min_depth = min_depth.min(depth);
                max_depth = max_depth.max(depth);
            }
            NodeKind::Internal { children } => {
                let minimum = if index == root || !balanced {
                    2
                } else {
                    MAX_CHILDREN / 2
                };
                assert!((minimum..=MAX_CHILDREN).contains(&children.len()));
                let mut union = tree.nodes[children.indices[0]].bounds;
                let mut maximum = 0;
                for &child in children.as_slice() {
                    union = union.union(&tree.nodes[child].bounds);
                    maximum = maximum.max(tree.nodes[child].max_order);
                    stack.push((child, depth + 1));
                }
                // Float union is not associative; geometric containment is
                // checked on integer-coordinate correctness workloads below.
                if balanced {
                    assert_eq!(node.bounds, union);
                }
                assert_eq!(node.max_order, maximum);
                assert_eq!(
                    tree.nodes[children.indices[children.len() - 1]].max_order,
                    maximum
                );
            }
        }
    }
    assert!(seen.into_iter().all(|visited| visited), "orphaned nodes");
    assert_eq!(
        tree.nodes[tree.max_leaf.expect("maximum")].max_order,
        tree.nodes[root].max_order
    );
    if balanced {
        assert_eq!(min_depth, max_depth, "unequal leaf depths");
    }
    (min_depth, max_depth)
}

#[test]
fn adversarial_orders_and_structure() {
    for name in [
        "horizontal",
        "vertical",
        "reversed",
        "grid",
        "overlap",
        "nested",
        "sparse",
        "touching",
        "zero",
    ] {
        let bounds = workload(name, 1100);
        let expected = oracle(&bounds);
        let mut tree = BoundsTree::default();
        for (i, (b, order)) in bounds.into_iter().zip(expected).enumerate() {
            assert_eq!(tree.insert(b), order, "{name} insertion {i}");
            if [1, 2, 12, 13, 72, 73, 144, 145, 1000, 1100].contains(&(i + 1)) {
                shape(&tree, true);
            }
        }
    }
}

#[test]
fn global_maximum_ties_and_clear_reuse() {
    let mut tree = BoundsTree::default();
    // Both disconnected components reach the same maximum. The stored leaf
    // remains in A; querying B must not mistake its miss for an empty result.
    for x in [0.0, 100.0, 0.0, 100.0] {
        tree.insert(rect(x, 0.0, 10.0, 13.0));
    }
    assert_eq!(tree.find_max_ordering(&rect(102.0, 3.0, 1.0, 2.0)), 2);
    let visits = tree.search_visits;
    assert_eq!(tree.find_max_ordering(&rect(2.0, 3.0, 1.0, 2.0)), 2);
    assert_eq!(
        tree.search_visits, visits,
        "global maximum should bypass search"
    );
    assert_eq!(tree.insert(rect(102.0, 3.0, 0.0, 0.0)), 3);
    assert_eq!(tree.insert(rect(110.0, 0.0, 2.0, 13.0)), 1, "touching edge");
    for b in workload("grid", 10_000) {
        tree.insert(b);
    }
    let capacity = (
        tree.nodes.capacity(),
        tree.insert_path.capacity(),
        tree.search_stack.capacity(),
    );
    tree.clear();
    assert!(tree.root.is_none() && tree.max_leaf.is_none());
    assert!(tree.nodes.is_empty() && tree.insert_path.is_empty() && tree.search_stack.is_empty());
    assert_eq!(
        capacity,
        (
            tree.nodes.capacity(),
            tree.insert_path.capacity(),
            tree.search_stack.capacity()
        )
    );
    assert_eq!(tree.insert(rect(102.0, 3.0, 0.0, 0.0)), 1);
    assert_eq!(tree.insert(rect(102.0, 3.0, 0.0, 0.0)), 1);
    shape(&tree, true);
}

#[test]
fn randomized_oracle_and_queries() {
    for seed in 0..24 {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let bounds: Vec<_> = (0..700)
            .map(|_| {
                rect(
                    rng.random_range(-300..400) as f32,
                    rng.random_range(-200..500) as f32,
                    rng.random_range(0..90) as f32,
                    rng.random_range(0..60) as f32,
                )
            })
            .collect();
        let expected = oracle(&bounds);
        let mut tree = BoundsTree::default();
        let mut repeat = BoundsTree::default();
        for (b, &order) in bounds.iter().zip(&expected) {
            assert_eq!(tree.insert(*b), order, "seed {seed}");
            assert_eq!(repeat.insert(*b), order);
        }
        shape(&tree, true);
        assert_eq!(format!("{:?}", tree.nodes), format!("{:?}", repeat.nodes));
        for query in bounds.iter().step_by(3) {
            let order = bounds
                .iter()
                .zip(&expected)
                .filter_map(|(b, &order)| overlaps(query, b).then_some(order))
                .max()
                .unwrap_or(0);
            assert_eq!(tree.find_max_ordering(query), order);
        }
    }
}

#[test]
fn non_copy_partially_ordered_units() {
    #[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
    struct Unit(f32);
    impl Add for Unit {
        type Output = Self;
        fn add(self, rhs: Self) -> Self {
            Self(self.0 + rhs.0)
        }
    }
    impl Sub for Unit {
        type Output = Self;
        fn sub(self, rhs: Self) -> Self {
            Self(self.0 - rhs.0)
        }
    }
    impl Half for Unit {
        fn half(&self) -> Self {
            Self(self.0 / 2.0)
        }
    }
    let bounds = workload("grid", 1000);
    let expected = oracle(&bounds);
    let mut tree = BoundsTree::default();
    for (b, order) in bounds.into_iter().zip(expected) {
        assert_eq!(
            tree.insert(Bounds {
                origin: Point {
                    x: Unit(b.origin.x),
                    y: Unit(b.origin.y)
                },
                size: Size {
                    width: Unit(b.size.width),
                    height: Unit(b.size.height)
                },
            }),
            order
        );
    }
    tree.clear();
    assert!(tree.nodes.is_empty());
}

#[test]
fn ordered_height_is_logarithmic_at_100k() {
    for name in [
        "horizontal",
        "vertical",
        "reversed",
        "grid",
        "overlap",
        "nested",
    ] {
        let mut tree = BoundsTree::default();
        for b in workload(name, 100_000) {
            tree.insert(b);
        }
        let (_, height) = shape(&tree, true);
        // A height-h root needs at least 2 * 6^(h-1) leaves.
        assert!(height <= 7, "{name}: {height}");
        assert!(
            tree.insert_visits <= 700_000,
            "{name}: {}",
            tree.insert_visits
        );
    }
}

#[test]
#[ignore = "isolated CPU evidence; run with --ignored --nocapture --test-threads=1"]
fn workload_evidence() {
    for name in ["horizontal", "overlap", "geo"] {
        for n in [1000, 10_000, 100_000] {
            if name == "geo" && n != 10_000 {
                continue;
            }
            let bounds = workload(name, n);
            let mut tree = BoundsTree::default();
            let mut orders = Vec::with_capacity(n);
            let start = std::time::Instant::now();
            for b in &bounds {
                orders.push(tree.insert(std::hint::black_box(*b)));
            }
            let elapsed = start.elapsed();
            let (min, max) = shape(&tree, false);
            if name == "horizontal" {
                assert!(orders.iter().all(|&order| order == 1));
            }
            if name == "overlap" {
                assert!(
                    orders
                        .iter()
                        .enumerate()
                        .all(|(i, &order)| order as usize == i + 1)
                );
            }
            if name == "geo" {
                assert_eq!(orders, oracle(&bounds));
            }
            if let Ok(directory) = std::env::var("BOUNDS_TREE_ORDERS") {
                let bytes: Vec<_> = orders
                    .iter()
                    .flat_map(|order| order.to_le_bytes())
                    .collect();
                std::fs::write(
                    std::path::Path::new(&directory).join(format!("{name}-{n}.bin")),
                    bytes,
                )
                .expect("write orders");
            }
            println!(
                "{name},{n},depth={min}..{max},insert_visits={},search_visits={},nodes={},us={}",
                tree.insert_visits,
                tree.search_visits,
                tree.nodes.len(),
                elapsed.as_micros()
            );
        }
    }
}
