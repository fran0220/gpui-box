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
python3 -m unittest discover -s scripts/sync-zed/tests -v
```

`verify` validates receipt shape, package names, destinations, forbidden product
paths, and every configured path in the pinned bootstrap tree. `verify
--release` is the canonical CI and release command after bootstrap: it also
requires every receipt coordinate, reproduces the exact bootstrap and current
vendor commit OIDs from remote source objects, requires an exact two-parent
integration merge on `HEAD`'s first-parent history, and resolves the exact
official cursor from upstream. Release verification cannot disable source
checks. All replay work happens in temporary bare repositories and does not
alter local history or refs.

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

Every integration merge records `zed-sync-algorithm`, `zed-vendor-tip`, and
`zed-upstream-cursor` markers in its own commit message. Release verification
requires the receipt to name the newest marked integration on `HEAD`'s
first-parent history and also rejects a newer unmarked merge whose non-first
parent carries a canonical synthetic `zed-upstream` trailer. An interrupted
integration therefore cannot leave an older, apparently valid receipt behind.

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

Successful bootstrap and sync operations write `state.json` and the `[sync]`
receipt mirror in `provenance.toml` together and commit both. Verification
rejects any disagreement between them.

If a fetched range produces no new filtered vendor commit, the receipt remains
at its last integrated cursor. This avoids claiming a cursor that has no
corresponding marked integration; a later sync safely inspects that range again.

Both mutating commands require a clean, committed worktree. The integration
markers are supplied to `git merge`, so a merge conflict leaves them in
`MERGE_MSG` with the raw conflict evidence. Resolve and commit it without
replacing that message, then update the vendor ref, `state.json`, and the
`provenance.toml` receipt mirror together, or run `git merge --abort`. The tool
never invokes AI or automatically chooses a resolution. Dry runs do not change
the worktree, refs, index, or object database.

## Ownership boundary and initial bootstrap

Only mapped source subtrees are synchronized. Root manifests/package policy,
GPUI Box package identities and dependency adaptation, local fonts, kit code,
docs, snapshots, release policy, and visual baselines remain GPUI Box-owned.
After this migration (including this directory and the imported sources) is
committed and the worktree is clean, run:

```sh
scripts/sync-zed/sync-zed verify
scripts/sync-zed/sync-zed bootstrap
```

Review the two local commits and vendor ref. Do not bootstrap in the current
uncommitted migration: the truthful checked-in receipt therefore has null
`vendor_tip`, `last_synced_sha`, and `integration_commit` fields.
