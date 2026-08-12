Rust interface for Apple's C language extension of blocks.

This is the `block` 0.1.6 crates.io source vendored by GPUI Box. It carries
ABI-neutral compatibility fixes: the opaque Objective-C class marker is
represented by an inhabited zero-sized C struct instead of an uninhabited
enum, and function pointers spell their existing C ABI explicitly. These
changes prevent current warnings from becoming errors in future Rust releases.

The public API, block layout, symbol names, and calling conventions are
unchanged. See the repository-level `PROVENANCE.md` and
`THIRD_PARTY_NOTICES` for the source receipt.
