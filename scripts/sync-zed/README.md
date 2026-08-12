# Zed filtered sync

This directory owns the reproducible import of GPUI framework/support code from
official Zed into GPUI Box. `config.json` is the path and package authority;
`state.json` is the committed receipt. The tool uses only Python 3's standard
library and Git.

```sh
scripts/sync-zed/sync-zed verify
scripts/sync-zed/sync-zed verify --release
scripts/sync-zed/sync-zed status
scripts/sync-zed/sync-zed bootstrap [--dry-run]
scripts/sync-zed/sync-zed sync [--ref FULL_SHA_OR_REF] [--dry-run]
scripts/sync-zed/sync-zed overlay [--dry-run | --continue]
python3 -m unittest discover -s scripts/sync-zed/tests -v
```

`verify` validates receipt shape, package names, destinations, forbidden product
paths, and every configured path in the pinned bootstrap tree. `verify
--release` is the canonical CI and release command after bootstrap: it also
requires every receipt coordinate, reproduces the exact bootstrap and current
vendor commit OIDs from remote source objects, requires an exact two-parent
integration merge on `HEAD`'s first-parent history, and resolves the exact
official cursor from upstream. Release verification cannot disable source
checks. It also reconstructs the separately receipted fork overlay, verifies
both canonical vendor refs and their exact integration merges, and proves that
the two vendor lanes share only the filtered bootstrap as their merge base.
All replay work happens in temporary SHA-1 bare repositories and does not alter
local history or refs.

## History model

The private local ref `refs/heads/vendor/zed-gpui` contains synthetic, linear,
filtered commits. Trees are made with a temporary index (`read-tree`,
`update-index`, `write-tree`) and commits with `commit-tree`. Each preserves the
original author, committer, dates, and message and adds a `zed-upstream` trailer.
Filtered no-ops are omitted. Because each tree contains the complete filter,
Git detects renames across configured crates.

The `first-parent-v1` history algorithm follows the official baseline-to-cursor
first-parent chain and records each commit whose mapped paths changed. A merged
upstream branch is therefore represented by the merge commit's resulting
filtered snapshot, not by a later-discovered ordering of side-branch commits.
This makes every future cursor an append-only extension and lets release
verification deterministically rebuild the same vendor OIDs from the original
remote objects.

`refs/heads/vendor/zed-gpui-fork-overlay` is a separate, immutable source lane
for post-bootstrap changes made in the GPUI Box Zed fork before they exist in
official Zed. Its `exact-linear-overlay-v1` algorithm accepts only the ordered
full-SHA list in `config.json`: the first source commit must have the configured
bootstrap revision as its sole parent, and every subsequent commit must have
the preceding SHA as its sole parent. Every listed source commit must change
the filtered tree. Synthetic commits are rooted at `bootstrap_vendor_tip` and
carry `zed-fork-overlay-upstream` rather than `zed-upstream`, so they can never
be mistaken for official replay. The canonical SHA-256 digest of the shared
mapping list is stored in config, state, and provenance.

The PlatformView overlay is the exact fork chain:

```text
1755444d8efd9c7b34d8f2fbe36a327b85ca4e9b
f212b120ede8c5ffcc5c60ebe1ac92d64fab9db7
7bcda540a22cf9e8bbd946f954c8f28f266e452b
b46bf740a55c53612b14120f5dfbb7ceec463261
```

The fork's later `main` merge is intentionally not a source revision: it also
contains a sibling Windows implementation. Exact-SHA remote fetch and parent
validation prevent that sibling, a floating branch, a merge commit, or a
reordered chain from entering the overlay receipt.

Every official integration merge records `zed-sync-algorithm`, `zed-vendor-tip`, and
`zed-upstream-cursor` markers in its own commit message. Release verification
requires the receipt to name the newest marked integration on `HEAD`'s
first-parent history and also rejects a newer unmarked merge whose non-first
parent carries a canonical synthetic `zed-upstream` trailer. An interrupted
integration therefore cannot leave an older, apparently valid receipt behind.
Overlay integrations use a disjoint complete marker family containing the
overlay algorithm, source tip, filtered base, and filtered tip. The overlay is
integrated with a normal two-parent merge, not an ours merge. The receipt proves
that exact deterministic branch is a merge parent; platform build and behavior
tests prove how conflict resolutions behave in GPUI Box.

Bootstrap filters `fran0220/zed@0b9c8dc932b65cba2dc87464148984e93f60ae18`,
then records its ancestry with an ours merge while setting the official cursor
to `a6a23c7b80a5cefa0487b7856335be89ace7e483`. The immutable
`bootstrap_vendor_tip` keeps the initial deterministic tree independently
verifiable after later syncs advance `vendor_tip`. The fork is bootstrap
content, not the future remote. Sync fetches `zed-industries/zed`, requires the
new cursor to extend the previous cursor on first-parent history, creates
deterministic `sync/zed-<sha>` and linear vendor commits, and performs a normal
three-way integration merge. It never pushes or deletes a branch and never
creates vendor merge commits.

Successful bootstrap, official sync, and overlay operations write `state.json`
and the `[sync]` / `[sync_overlay]` receipt mirrors in `provenance.toml`
together and commit both. Verification rejects any disagreement between them.

If a fetched range produces no new filtered vendor commit, the receipt remains
at its last integrated cursor. This avoids claiming a cursor that has no
corresponding marked integration; a later sync safely inspects that range again.

Mutating commands require a clean, committed worktree. The integration
markers are supplied to `git merge`, so a merge conflict leaves them in
`MERGE_MSG` with the raw conflict evidence. Resolve and commit it without
replacing that message, then update the vendor ref, `state.json`, and the
`provenance.toml` receipt mirror together, or run `git merge --abort`. The tool
never invokes AI or automatically chooses a resolution. Dry runs do not change
the worktree, refs, index, or object database.

For an overlay conflict, preserve `MERGE_MSG`, resolve every path, stage the
resolution, then run `sync-zed overlay --continue`. Continuation re-fetches and
replays the exact source chain, requires the expected `MERGE_HEAD` and marker
set, commits the merge, updates the canonical overlay ref with compare-and-swap
semantics, and records state/provenance. It can also recover when the merge was
already committed but the receipt update was interrupted.

## Ownership boundary and bootstrap receipt

Only mapped source subtrees are synchronized. Root manifests/package policy,
GPUI Box package identities and dependency adaptation, local fonts, kit code,
docs, snapshots, release policy, and visual baselines remain GPUI Box-owned. The
initial bootstrap is committed: `state.json` names the deterministic vendor tip,
official baseline cursor, and exact integration merge, and `verify --release`
reconstructs and checks all three. To inspect the current receipt, run:

```sh
scripts/sync-zed/sync-zed verify
scripts/sync-zed/sync-zed verify --release
scripts/sync-zed/sync-zed status
```

Do not run `bootstrap` again. Future official updates use `sync` from a clean,
committed worktree and continue from the official cursor and official vendor
tip—not from the overlay. The historical overlay receipt and ref remain intact
even after official Zed later contains equivalent functionality; convergence is
resolved in the later mainline integration rather than by rewriting provenance.
