# Releasing

This repository cannot be released the usual way, and pretending otherwise
would be the whole problem. `gpui-kit` depends on GPUI by Git revision, and
crates.io refuses a package that depends on a Git revision. That is not a
configuration to fix; it is a property of the dependency, and it decides the
shape of everything here.

So a release is **a tag on a commit that somebody has verified**. That is a
weaker promise than a registry version, and stating it plainly is better than
implying a stronger one with a number.

## What a tag means

`v0.2.0` names a commit at which:

- `cargo run -p xtask -- gate full` passed on macOS, including the scene
  images, so the catalog renders and matches its baseline;
- `cargo run -p xtask -- headless check` passed, so the Linux and Windows
  renderers agree;
- `CHANGELOG.md` describes every change since the previous tag, in terms of
  what the library now does and what it refuses to do;
- the three load-bearing surfaces named in `README.md` — the Rust API, the
  token keys, and the semantic ids — are either unchanged or their changes are
  written down as breaking.

It does not mean the version number resolves anywhere, that Cargo will pick it
up, or that a minor number implies compatibility. A consumer still pins a
revision; the tag exists so that revision has a name and a changelog entry
rather than being a hash somebody happened to be on.

## Cutting one

```bash
cargo run -p xtask -- gate full          # fmt, check, test, clippy, tokens, strings, api, docs, scenes
cargo run -p xtask -- headless check     # the Linux and Windows baseline
```

Both must be green on the commit being tagged, not on a nearby one. Then write
the `CHANGELOG.md` entry, commit it, and tag that commit:

```bash
git tag -a v0.2.0 -m "gpui-kit v0.2.0"
git push origin v0.2.0
```

Tags are not moved and not deleted. A consumer may already have pinned one, and
a tag that changes where it points is worse than no tag at all, because it
breaks the one thing a tag is for.

A consumer then pins it:

```toml
[dependencies]
gpui-kit = { git = "https://github.com/fran0220/gpui-kit", tag = "v0.2.0" }
```

Every crate taken from this workspace must use the same tag, and the
application must move its own GPUI pin to the revision in this workspace's
`Cargo.toml` in the same commit. The two have to match exactly, because they
are the same library.

## The one crate that could go to crates.io

`gpui-kit-tokens` does not depend on GPUI. It is a token document, its
validation, and typed access to it, and it packages cleanly:

```bash
cargo package -p gpui-kit-tokens
```

That is why `crates/gpui-kit-tokens/tokens/*.json` lives inside the crate
rather than at the repository root: a package may only carry files under its
own directory, and a crate that cannot carry its own inputs cannot be
published. The root `tokens/` directory used to hold them, and moving it was
the fix.

Whether to actually publish it is a separate decision, and the honest answer
today is not yet. A published `gpui-kit-tokens` would be a name and a version
that nothing else on crates.io can use, since every crate that consumes it is
unpublishable for as long as GPUI is. Keeping it *publishable* costs nothing
and keeps the option; publishing it now would claim a registry name to no
one's benefit. Revisit when GPUI is on crates.io, at which point the rest of
the workspace becomes publishable too and the question changes.

## When GPUI reaches crates.io

The blocker disappears and the whole workspace becomes publishable. At that
point:

- replace the `git` and `rev` keys in the workspace `Cargo.toml` with a version
  requirement;
- publish in dependency order: `gpui-kit-tokens`, `gpui-kit-assets`,
  `gpui-kit-semantics`, `gpui-kit-theme`, `gpui-kit-testkit`, `gpui-kit`;
- start bumping `version` in earnest and follow semver against the three
  surfaces in `README.md`, not just the Rust one.

Until then, the tag is the release.
