//! Deterministic crossing objective for layered DAG order refinement.

/// Weighted center-line crossing proxy, including links that skip columns.
/// Shared endpoints and zero-flow links are not crossings. Weight normalization
/// prevents finite but huge raw flow values from overflowing the objective.
pub(super) fn cost(
    columns: &[Vec<usize>],
    depth: &[usize],
    edges: &[(usize, usize)],
    weights: &[f64],
) -> f64 {
    let mut y = vec![0.0; depth.len()];
    for column in columns {
        for (rank, &node) in column.iter().enumerate() {
            y[node] = (rank as f64 + 0.5) / column.len() as f64;
        }
    }
    let max = weights.iter().copied().fold(0.0, f64::max);
    if max == 0.0 {
        return 0.0;
    }
    let mut total = 0.0;
    for (i, &(a, b)) in edges.iter().enumerate() {
        for (j, &(c, d)) in edges.iter().enumerate().skip(i + 1) {
            if a == c || b == d || weights[i] == 0.0 || weights[j] == 0.0 {
                continue;
            }
            let left = depth[a].max(depth[c]);
            let right = depth[b].min(depth[d]);
            if left >= right {
                continue;
            }
            let at = |start: usize, end: usize, x: usize| {
                let t = (x - depth[start]) as f64 / (depth[end] - depth[start]) as f64;
                y[start] * (1.0 - t) + y[end] * t
            };
            if (at(a, b, left) - at(c, d, left)) * (at(a, b, right) - at(c, d, right)) < 0.0 {
                total += (weights[i] / max).sqrt() * (weights[j] / max).sqrt();
            }
        }
    }
    total
}

pub(super) fn transpose(
    columns: &mut [Vec<usize>],
    depth: &[usize],
    edges: &[(usize, usize)],
    weights: &[f64],
) {
    let mut best = cost(columns, depth, edges, weights);
    // Bounded local refinement complements the alternating barycenter sweeps.
    for _ in 0..4 {
        let mut changed = false;
        for c in 0..columns.len() {
            for i in 1..columns[c].len() {
                columns[c].swap(i - 1, i);
                let candidate = cost(columns, depth, edges, weights);
                if candidate < best {
                    best = candidate;
                    changed = true;
                } else {
                    columns[c].swap(i - 1, i);
                }
            }
        }
        if !changed {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skip_links_count_and_zero_flows_do_not_change_order() {
        let mut columns = vec![vec![0, 1], vec![2, 3], vec![4, 5]];
        let depth = [0, 0, 1, 1, 2, 2];
        let edges = [(0, 5), (1, 4), (2, 4), (3, 5)];
        let weights = [9.0, 4.0, 0.0, 0.0];
        assert!((cost(&columns, &depth, &edges, &weights) - 2.0 / 3.0).abs() < 1e-12);
        transpose(&mut columns, &depth, &edges, &weights);
        assert_eq!(cost(&columns, &depth, &edges, &weights), 0.0);
        let huge = [9e300, 4e300, 0.0, 0.0];
        assert_eq!(cost(&columns, &depth, &edges, &huge), 0.0);
    }
}
