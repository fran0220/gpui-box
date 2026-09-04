# Frozen Zed import receipt

GPUI Box is the sole development authority for its GPUI framework, platform
implementations, media services, and Kit. The former two-repository development
and continuous Zed synchronization model is permanently retired. No command in
this directory imports source, advances a cursor, creates a branch, or changes
an existing Git ref.

This directory now preserves and verifies the historical import:

- `config.json` freezes the exact source URLs, revisions, filtered path set,
  package identities, and filter digest;
- `state.json` freezes the synthetic vendor tips and integration merges;
- `provenance.toml` mirrors the receipt for release tooling;
- `refs/heads/vendor/zed-gpui` and
  `refs/heads/vendor/zed-gpui-fork-overlay` retain the imported Git objects.

Run the read-only, offline verifier with:

```sh
scripts/sync-zed/sync-zed verify
scripts/sync-zed/sync-zed status
python3 -m unittest discover -s scripts/sync-zed/tests -v
```

`verify` requires both frozen refs at their exact recorded tips, validates every
source trailer and parent in the PlatformView overlay, checks both two-parent
integration merges and their first-parent ancestry through `HEAD`, proves that
the vendor lanes meet only at the bootstrap tip, and checks mappings, package
identities, forbidden product paths, license files, and machine provenance. It
uses only local Git objects and Python's standard library, so CI and releases do
not depend on either historical source repository being reachable.

To estimate the cost of one deliberately selected upstream revision, run:

```sh
scripts/sync-zed/sync-zed assess --revision <full-zed-commit-sha>
```

`assess` is networked but read-only. It fetches that exact revision from the
official Zed repository into a temporary Git repository, applies the frozen
path mappings and GPUI Box package-name mappings, and performs a three-way
merge with the frozen `vendor/zed-gpui` tip as base and the current committed
`HEAD` as ours. Its JSON report counts conflict files, conflict-marker hunks and
payload lines, conflict subsystems, and changed lines in automatically merged,
fully conflict-free files. The temporary repository is deleted afterward; the
current worktree and every existing ref remain untouched.

This command is an assessment tool, not an update lane. Landing any discrete
upstream import still requires an explicit policy decision, provenance review,
and the framework validation required by the root contributor guidance.

The historical source chain was:

```text
fran0220/zed@0b9c8dc932b65cba2dc87464148984e93f60ae18
  ├─ filtered bootstrap / official baseline receipt
  └─ PlatformView overlay
     1755444d8efd9c7b34d8f2fbe36a327b85ca4e9b
     f212b120ede8c5ffcc5c60ebe1ac92d64fab9db7
     7bcda540a22cf9e8bbd946f954c8f28f266e452b
     b46bf740a55c53612b14120f5dfbb7ceec463261
```

These SHAs are attribution and audit records, not future update points. New
framework work is implemented and reviewed directly in this repository. If new
third-party source is deliberately ported later, record it as a new immutable
provenance entry; do not reactivate this historical lane or restore a Zed Cargo
Git dependency.
