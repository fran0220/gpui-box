# gpui-kit contributor guidance

## Boundaries

This repository is product-neutral UI infrastructure. Components may depend on
GPUI, tokens, theme, assets, and semantic testability. They must not depend on
application hosts, databases, credentials, RPC transports, or product models.

Components:

1. read caller-owned data;
2. emit caller-owned actions;
3. hold only visual transient state such as hover, focus, open, selection, and
   animation.

## Token authority

`tokens/studio-dark.json` is the source of truth. Repeated semantic color,
spacing, radius, typography, motion, and effect values belong there. Local
geometry that occurs once may stay next to the component.

After token changes:

```bash
cargo run -p xtask -- tokens generate
cargo run -p xtask -- tokens check
```

Do not hand-edit `docs/token-reference.md`.

## Truthful UI

- Loading, Empty, Unavailable, Error, and Ready are distinct states.
- A refresh failure keeps the last verified value visible.
- A disabled control must not install its action handler.
- Host refusals are displayed as refusals, not converted to empty data.
- Fixtures and product-backed data must be explicitly distinguishable.

## Testability

Every user-visible action and assertion target needs a stable semantic id.
IDs derive from business identity, never list position. Bounds are measured
during prepaint. Never put credentials or unredacted user-generated content in
semantic snapshots.

Tests assert behavior and generated artifacts, never source text.

## Provenance

Source ports and translations must update `PROVENANCE.md` and
`THIRD_PARTY_NOTICES`. Preserve upstream copyright notices and exact revisions.
Do not add product or provider trademarks to the generic asset crate.

## Validation

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- tokens check
```

UI changes additionally require a gallery capture and visual inspection.
