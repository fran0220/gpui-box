#!/usr/bin/env bash
set -euo pipefail
root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
output=$(mktemp -d)
trap 'rm -rf "$output"' EXIT
cc -std=c11 -Wall -Wextra -Werror "$root/crates/gpui_ios/native/geometry_test.c" -o "$output/geometry"
"$output/geometry"
rustc --edition=2024 --test "$root/crates/gpui_ios/src/ffi.rs" -o "$output/abi"
"$output/abi"
