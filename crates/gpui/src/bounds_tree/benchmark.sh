#!/usr/bin/env bash
# CPU-only comparison of the actual index and actual GPUI generic geometry.
# Run from the repository root. No renderer, scene, window or GPU is involved.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
base=${1:-53ddf792b9ec641d8c5d3bcb47fb22c532f50a09}
target=$(cargo metadata --no-deps --format-version 1 | jq -r .target_directory)
out="$target/bounds-tree-bench"
mkdir -p "$out"
cargo test -p gpui-box --lib --no-default-features --no-run --message-format=json > "$out/build.jsonl"
gpui=$(jq -r 'select(.reason == "compiler-artifact" and .target.name == "gpui") | .filenames[] | select(endswith(".rlib"))' "$out/build.jsonl" | tail -1)
rand=$(jq -r 'select(.reason == "compiler-artifact" and .target.name == "rand") | .filenames[] | select(endswith(".rlib"))' "$out/build.jsonl" | tail -1)
test -n "$gpui" && test -n "$rand"
deps=$(dirname "$gpui")

# Instrument only the old descent-loop entry and search pop, matching the
# cfg(test) counters in the current implementation. Do not alter its algorithm.
git show "$base:crates/gpui/src/bounds_tree.rs" |
    sed '/^#\[cfg(test)\]/,$d' |
    sed -e '/    search_stack: Vec<NonNull<Node<U>>>,/a\    search_visits: usize,\n    insert_visits: usize,' \
        -e '/            search_stack: Vec::new(),/a\            search_visits: 0,\n            insert_visits: 0,' \
        -e '/        while let Some(node) = self.search_stack.pop() {/a\            self.search_visits += 1;' \
        -e '/        loop {/a\            self.insert_visits += 1;' > "$out/before.rs"
sed '/^#\[cfg(test)\]/,$d' "$root/crates/gpui/src/bounds_tree.rs" > "$out/after.rs"
for version in before after; do
    printf '\n#[path = "%s/crates/gpui/src/bounds_tree/tests.rs"]\nmod evidence;\n' "$root" >> "$out/$version.rs"
    printf 'pub use gpui::{Bounds, Half, Point, Size};\n#[path = "%s/%s.rs"]\nmod bounds_tree;\n' "$out" "$version" > "$out/driver-$version.rs"
done
for mode in release debug; do
    flags=(-C opt-level=0 -C debug-assertions=yes)
    if [[ $mode == release ]]; then flags=(-C opt-level=3 -C debug-assertions=no); fi
    for version in before after; do
        rustc --edition=2024 --test --crate-name bounds_tree_bench \
            "${flags[@]}" --extern "gpui=$gpui" --extern "rand=$rand" \
            -L "dependency=$deps" "$out/driver-$version.rs" -o "$out/$version-$mode"
        mkdir -p "$out/$version-$mode-orders"
        BOUNDS_TREE_ORDERS="$out/$version-$mode-orders" \
            "$out/$version-$mode" workload_evidence --ignored --nocapture --test-threads=1 |
            tee "$out/$version-$mode.log"
    done
    for before in "$out/before-$mode-orders/"*.bin; do
        cmp "$before" "$out/after-$mode-orders/$(basename "$before")"
    done
    echo "$mode: every assigned order matches byte-for-byte"
done
echo "Evidence: $out (index/geometry monomorphizations use opt-level=0/3; linked GPUI dependencies use the test profile)"
